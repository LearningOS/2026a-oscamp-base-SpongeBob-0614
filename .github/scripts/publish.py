"""Save the current measured score, then upload it to the assigned OpenCamp course."""

import json
import os
from pathlib import Path
import subprocess
import sys
from http.client import HTTPException
from urllib.error import HTTPError, URLError
from urllib.request import Request, urlopen

from course import COURSE, ROOT, student_login, validate_result

API_URL = "https://api.opencamp.cn/web/api/courseRank/createByThirdToken"


def git(*args, cwd=None):
    return subprocess.check_output(["git", *args], cwd=cwd or ROOT, text=True, timeout=30).strip()


def upload_score(payload, token):
    request = Request(API_URL, data=json.dumps(payload).encode(), method="POST",
                      headers={"Content-Type": "application/json", "Accept": "application/json",
                               "token": token})
    try:
        with urlopen(request, timeout=30) as response:
            body = response.read().decode()
    except HTTPError as error:
        details = error.read().decode(errors="replace").replace(token, "[REDACTED]")
        raise RuntimeError(f"OpenCamp HTTP {error.code}: {details}") from None
    except (TimeoutError, HTTPException) as error:
        raise RuntimeError(f"OpenCamp response interrupted: {str(error).replace(token, '[REDACTED]')}. Re-run the upload job.") from None
    except URLError as error:
        raise RuntimeError(f"OpenCamp connection failed: {str(error.reason).replace(token, '[REDACTED]')}") from None
    try:
        result = json.loads(body)
    except json.JSONDecodeError:
        raise RuntimeError(f"OpenCamp returned invalid JSON: {body.replace(token, '[REDACTED]')}") from None
    if not isinstance(result, dict) or type(result.get("result")) is not int or result["result"] != 1:
        raise RuntimeError(f"OpenCamp rejected the score: {body.replace(token, '[REDACTED]')}")
    print("OpenCamp accepted the score (result=1).")



def save_state(rank, state_path, state, message):
    state_path.write_text(json.dumps(state, indent=2) + "\n")
    git("add", state_path.name, cwd=rank)
    changed = subprocess.run(["git", "diff", "--cached", "--quiet"], cwd=rank, timeout=30)
    if changed.returncode == 1:
        git("-c", "user.name=github-actions[bot]", "-c",
            "user.email=41898282+github-actions[bot]@users.noreply.github.com", "commit",
            "-m", message, cwd=rank)
        git("push", "origin", "HEAD:gh-pages", cwd=rank)
    elif changed.returncode != 0:
        raise RuntimeError(f"Cannot inspect score changes (git exit {changed.returncode}).")


def current_commit():
    expected = os.environ["GITHUB_SHA"]
    lines = git("ls-remote", "--heads", "origin", "refs/heads/main").splitlines()
    if len(lines) != 1 or lines[0].split()[0] != expected:
        message = "Skipped an outdated grading result; main has changed. No score was uploaded."
        print(message, flush=True)
        if os.environ.get("GITHUB_STEP_SUMMARY"):
            with open(os.environ["GITHUB_STEP_SUMMARY"], "a") as summary:
                summary.write(message + "\n")
        return False
    return True


def main():
    token = os.environ.get("COURSE_TOKEN", "")
    if not token:
        raise ValueError("Missing organization course secret; contact the maintainer.")
    if os.environ.get("OSCAMP_COURSE_ID") != str(COURSE["courseId"]):
        raise ValueError("Unexpected course ID; no score was uploaded.")
    repository = os.environ["GITHUB_REPOSITORY"]
    student = student_login(repository, os.environ["GITHUB_REPOSITORY_OWNER"],
                            os.environ["GITHUB_ACTOR"], os.environ.get("STUDENT_GITHUB", ""))
    if os.environ["GITHUB_REF_NAME"] != "main":
        raise ValueError("Only main branch scores can be uploaded.")
    result = json.loads((ROOT / "tmp/grade/result.json").read_text())
    score = validate_result(result, repository, os.environ["GITHUB_SHA"])
    if not current_commit():
        return
    result["student"] = student
    result["runId"] = os.environ["GITHUB_RUN_ID"]
    result["runAttempt"] = os.environ["GITHUB_RUN_ATTEMPT"]
    result["upload"] = {"status": "pending"}
    rank = ROOT / "rank"
    if rank.exists():
        raise ValueError("rank already exists; use a fresh CI checkout.")
    refs = git("ls-remote", "--heads", "origin", "refs/heads/gh-pages")
    if refs:
        git("fetch", "--depth=1", "origin", "gh-pages")
        git("worktree", "add", "--detach", str(rank), "FETCH_HEAD")
    else:
        git("worktree", "add", "--detach", str(rank), "HEAD")
        git("switch", "--orphan", "gh-pages", cwd=rank)
    state_path = rank / f"course-{COURSE['courseId']}.json"
    if state_path.exists():
        old = json.loads(state_path.read_text())
        if (old.get("courseId") != COURSE["courseId"] or old.get("repository") != repository
                or old.get("student") != student):
            raise ValueError("Existing score belongs to another course or student.")
        if int(old.get("runId", 0)) > int(result["runId"]):
            print("Skipped an older workflow run; a newer result is already recorded.", flush=True)
            return
    if not current_commit():
        return
    save_state(rank, state_path, result, f"Record measured score for OpenCamp {COURSE['courseId']}")
    payload = {"channel": "github", "courseId": COURSE["courseId"], "name": student,
               "score": score, "totalScore": COURSE["totalScore"], "ext": "{}"}
    print(f"Submitting measured score: course={COURSE['courseId']}, student={student}, "
          f"score={score}/{COURSE['totalScore']}", flush=True)
    if not current_commit():
        return
    upload_score(payload, token)
    result["upload"]["status"] = "accepted"
    save_state(rank, state_path, result, f"Confirm OpenCamp {COURSE['courseId']} upload accepted")
    if os.environ.get("GITHUB_STEP_SUMMARY"):
        with open(os.environ["GITHUB_STEP_SUMMARY"], "a") as summary:
            summary.write(f"OpenCamp accepted course {COURSE['courseId']}, student {student}: "
                          f"**{score}/{COURSE['totalScore']}**.\n")


if __name__ == "__main__":
    try:
        main()
    except (ValueError, RuntimeError, OSError, subprocess.SubprocessError) as error:
        sys.exit(str(error))
