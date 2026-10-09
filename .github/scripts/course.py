"""Course identity, measured-result validation, and grading evidence."""

import json
import os
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parents[2]
COURSE = json.loads((ROOT / "course.json").read_text())


def student_login(repository, owner, actor, student):
    if not re.fullmatch(r"[A-Za-z0-9](?:[A-Za-z0-9-]{0,37}[A-Za-z0-9])?", student):
        raise ValueError("STUDENT_GITHUB must be the student's GitHub login.")
    expected = f"{COURSE['organization']}/{COURSE['name']}-{student}"
    if owner.lower() != COURSE["organization"].lower() or repository.lower() != expected.lower():
        raise ValueError("Repository does not match the assigned course student.")
    if actor.lower() != student.lower():
        raise ValueError("Only the assigned student's runs can upload their score.")
    return student


def run(command, log, cwd, seconds):
    """GNU timeout also stops child processes; preserve stdout, stderr and status."""
    args = ["timeout", "--signal=TERM", "--kill-after=10s", f"{seconds}s", *command]
    print("$ " + " ".join(command), flush=True)
    with subprocess.Popen(args, cwd=cwd, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                          text=True, errors="replace") as process:
        lines = []
        for line in process.stdout:
            log.write(line)
            log.flush()
            print(line, end="", flush=True)
            lines.append(line)
        code = process.wait()
    message = f"Command exit status: {code}\n"
    log.write(message)
    print(message, end="", flush=True)
    return code, "".join(lines)


def validate_result(result, repository, commit):
    if not isinstance(result, dict) or any(type(result.get(key)) is not int for key in ("schema", "courseId", "totalScore")):
        raise ValueError("Invalid result metadata types.")
    if (result.get("schema") != 1 or result.get("courseId") != COURSE["courseId"]
            or result.get("totalScore") != COURSE["totalScore"]
            or result.get("repository") != repository or result.get("commit") != commit):
        raise ValueError("Result belongs to a different course, repository or commit.")
    records = result.get("exercises")
    if not isinstance(records, list) or len(records) != len(COURSE["tests"]):
        raise ValueError("Incomplete exercise results; no score can be uploaded.")
    score = 0
    for expected, record in zip(COURSE["tests"], records):
        if not isinstance(record, dict):
            raise ValueError("Invalid exercise record.")
        if record.get("name") != expected["name"] or record.get("max") != expected["score"]:
            raise ValueError("Exercise names or weights differ from the course rubric.")
        if record.get("status") not in ("pass", "fail", "timeout") or type(record.get("exitCode")) is not int:
            raise ValueError("Invalid exercise outcome.")
        passed = record["status"] == "pass"
        if passed and record["exitCode"] != 0:
            raise ValueError("A command that failed cannot receive points.")
        measured = expected["score"] if passed else 0
        if type(record.get("score")) is not int or record["score"] != measured:
            raise ValueError("Exercise score does not match the measured outcome.")
        score += measured
    if (sum(test["score"] for test in COURSE["tests"]) != COURSE["totalScore"]
            or type(result.get("score")) is not int or result["score"] != score):
        raise ValueError("Invalid total score.")
    return score


def write_result(records):
    repository = os.environ.get("GITHUB_REPOSITORY", "local")
    commit = os.environ.get("GITHUB_SHA") or subprocess.check_output(
        ["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    result = {"schema": 1, "courseId": COURSE["courseId"], "repository": repository,
              "commit": commit, "score": sum(item["score"] for item in records),
              "totalScore": COURSE["totalScore"], "exercises": records}
    score = validate_result(result, repository, commit)
    (ROOT / "tmp/grade/result.json").write_text(json.dumps(result, indent=2) + "\n")
    passed = sum(item["status"] == "pass" for item in records)
    text = (f"Course {COURSE['courseId']}: {score}/{COURSE['totalScore']} points; "
            f"{passed}/{len(records)} exercises passed.")
    print(text, flush=True)
    if os.environ.get("GITHUB_STEP_SUMMARY"):
        with open(os.environ["GITHUB_STEP_SUMMARY"], "a") as summary:
            summary.write(text + "\n\n| Exercise | Result | Score | Exit status |\n"
                          "| --- | --- | ---: | ---: |\n")
            for item in records:
                summary.write(f"| {item['name']} | {item['status']} | "
                              f"{item['score']}/{item['max']} | {item['exitCode']} |\n")
    if os.environ.get("GITHUB_OUTPUT"):
        with open(os.environ["GITHUB_OUTPUT"], "a") as output:
            output.write(f"ready=true\nscore={score}\npassed={str(passed == len(records)).lower()}\n")


def record(test, code, passed):
    return {"name": test["name"], "max": test["score"],
            "score": test["score"] if passed else 0,
            "status": "pass" if passed else "timeout" if code in (124, 137) else "fail",
            "exitCode": code}
