"""Run all 24 upstream Cargo test packages with their original 100-point rubric."""

import os
import re
import subprocess
import sys

from course import COURSE, ROOT, record, run, write_result


def main():
    os.chdir(ROOT)
    output = ROOT / "tmp/grade"
    output.mkdir(parents=True, exist_ok=True)
    os.environ["TMPDIR"] = str(ROOT / "tmp")
    os.environ["CARGO_TARGET_DIR"] = str(ROOT / "tmp/target")
    # Resolve infrastructure/dependency failures before awarding any course score.
    subprocess.run(["cargo", "fetch", "--locked"], check=True)
    results = []
    for test in COURSE["tests"]:
        command = ["cargo", "test", "--locked", "-p", test["name"]]
        if test["target"] == "riscv64":
            command += ["--target", "riscv64gc-unknown-linux-gnu"]
        print(f"::group::{test['name']} ({test['score']} points)", flush=True)
        with (output / (test["name"] + ".log")).open("w") as log:
            code, text = run(command, log, ROOT, seconds=90)
        # Empty test suites must not award the package's points.
        has_tests = bool(re.search(r"test result: ok\. [1-9][0-9]* passed; 0 failed;", text))
        results.append(record(test, code, code == 0 and has_tests))
        print("::endgroup::", flush=True)
    write_result(results)


if __name__ == "__main__":
    try:
        main()
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        sys.exit(str(error))
