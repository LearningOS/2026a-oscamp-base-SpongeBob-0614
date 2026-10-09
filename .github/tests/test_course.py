"""Reject cross-course, incomplete and falsely awarded grading results."""

from copy import deepcopy
import io
import json
from pathlib import Path
import shutil
import sys
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / ".github/scripts"))
import course
import publish


class CourseTests(unittest.TestCase):
    def setUp(self):
        self.repository = f"{course.COURSE['organization']}/{course.COURSE['name']}-student"
        self.result = {"schema": 1, "courseId": course.COURSE["courseId"],
                       "totalScore": course.COURSE["totalScore"], "repository": self.repository,
                       "commit": "measured-commit", "score": 0,
                       "exercises": [course.record(test, 101, False) for test in course.COURSE["tests"]]}

    def test_real_failure_result_and_partial_pass(self):
        self.assertEqual(course.validate_result(self.result, self.repository, "measured-commit"), 0)
        test = course.COURSE["tests"][0]
        self.result["exercises"][0] = course.record(test, 0, True)
        self.result["score"] = test["score"]
        self.assertEqual(course.validate_result(self.result, self.repository, "measured-commit"), test["score"])

    def test_cross_course_repository_and_commit_rejected(self):
        for key, value in [("courseId", -1), ("repository", "another/repo"), ("commit", "stale")]:
            result = deepcopy(self.result)
            result[key] = value
            with self.subTest(key=key), self.assertRaises(ValueError):
                course.validate_result(result, self.repository, "measured-commit")

    def test_missing_or_reordered_tests_rejected(self):
        for records in [self.result["exercises"][:-1], list(reversed(self.result["exercises"]))]:
            result = deepcopy(self.result)
            result["exercises"] = records
            with self.assertRaises(ValueError):
                course.validate_result(result, self.repository, "measured-commit")

    def test_failed_or_timed_out_command_cannot_receive_points(self):
        for code in (1, 101, 124, 137):
            result = deepcopy(self.result)
            test = course.COURSE["tests"][0]
            result["exercises"][0] = course.record(test, code, True)
            result["score"] = test["score"]
            with self.subTest(code=code), self.assertRaises(ValueError):
                course.validate_result(result, self.repository, "measured-commit")

    def test_total_score_and_weights_cannot_be_forged(self):
        result = deepcopy(self.result)
        result["score"] = 100
        with self.assertRaises(ValueError):
            course.validate_result(result, self.repository, "measured-commit")
        result = deepcopy(self.result)
        result["exercises"][0]["max"] += 1
        with self.assertRaises(ValueError):
            course.validate_result(result, self.repository, "measured-commit")

    def test_assigned_account_only(self):
        self.assertEqual(course.student_login(self.repository, "LearningOS", "student", "student"), "student")
        for repository, actor in [(self.repository, "maintainer"),
                                  ("LearningOS/" + course.COURSE["name"], "student")]:
            with self.assertRaises(ValueError):
                course.student_login(repository, "LearningOS", actor, "student")

    def test_api_business_error_and_secret_redaction(self):
        token = "private-test-value"
        for body in [json.dumps({"result": 0, "message": token}), "bad response " + token]:
            with patch.object(publish, "urlopen", return_value=io.BytesIO(body.encode())):
                with self.assertRaises(RuntimeError) as raised:
                    publish.upload_score({}, token)
                self.assertNotIn(token, str(raised.exception))
                self.assertIn("[REDACTED]", str(raised.exception))

    @unittest.skipUnless(shutil.which("timeout"), "GNU timeout is required by the Linux grader")
    def test_runner_retains_real_stdout_stderr_and_failure(self):
        log = io.StringIO()
        code, output = course.run([sys.executable, "-c",
                                   "import sys; print('test stdout'); print('test stderr', file=sys.stderr); sys.exit(7)"],
                                  log, ROOT, seconds=5)
        self.assertEqual(code, 7)
        self.assertIn("test stdout", output)
        self.assertIn("test stderr", log.getvalue())


if __name__ == "__main__":
    unittest.main()
