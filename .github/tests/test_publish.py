"""Regression tests for stale uploads, interrupted HTTP and persisted status."""

from copy import deepcopy
from http.client import IncompleteRead
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / ".github/scripts"))
import course
import publish


class PublishTests(unittest.TestCase):
    def setUp(self):
        (ROOT / "tmp").mkdir(exist_ok=True)
        self.directory = tempfile.TemporaryDirectory(dir=ROOT / "tmp")
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.commit = "a" * 40
        self.remote_commit = self.commit
        self.repository = f"LearningOS/{course.COURSE['name']}-student"
        self.result = {"schema": 1, "courseId": course.COURSE["courseId"],
                       "totalScore": course.COURSE["totalScore"], "repository": self.repository,
                       "commit": self.commit, "score": 0,
                       "exercises": [course.record(t, 101, False) for t in course.COURSE["tests"]]}
        (self.root / "tmp/grade").mkdir(parents=True)
        (self.root / "tmp/grade/result.json").write_text(json.dumps(self.result))
        self.env = {"COURSE_TOKEN": "placeholder-only", "OSCAMP_COURSE_ID": str(course.COURSE["courseId"]),
                    "GITHUB_REPOSITORY": self.repository, "GITHUB_REPOSITORY_OWNER": "LearningOS",
                    "GITHUB_ACTOR": "student", "STUDENT_GITHUB": "student", "GITHUB_REF_NAME": "main",
                    "GITHUB_SHA": self.commit, "GITHUB_RUN_ID": "10", "GITHUB_RUN_ATTEMPT": "2"}
        self.old = None
        self.writes = []
        self.state_path = self.root / "rank" / f"course-{course.COURSE['courseId']}.json"
        patch.dict(os.environ, self.env).start()
        patch.object(publish, "ROOT", self.root).start()
        patch.object(publish, "git", side_effect=self.git).start()
        patch.object(publish.subprocess, "run", return_value=subprocess.CompletedProcess([], 1)).start()
        self.upload = patch.object(publish, "upload_score").start()
        self.addCleanup(patch.stopall)

    def git(self, *args, **options):
        if args[:2] == ("ls-remote", "--heads"):
            if args[-1] == "refs/heads/main":
                return self.remote_commit + "\trefs/heads/main"
            return "old\trefs/heads/gh-pages" if self.old else ""
        if args[:2] == ("worktree", "add"):
            self.state_path.parent.mkdir()
            if self.old:
                self.state_path.write_text(json.dumps(self.old))
        if args[0] == "push":
            self.writes.append(json.loads(self.state_path.read_text()))
        return ""

    def test_old_commit_never_writes_or_uploads(self):
        self.remote_commit = "b" * 40
        publish.main()
        self.assertEqual(self.writes, [])
        self.upload.assert_not_called()

    def test_older_run_with_same_commit_never_overwrites_newer_result(self):
        self.old = dict(self.result, student="student", runId="20")
        publish.main()
        self.assertEqual(self.writes, [])
        self.upload.assert_not_called()

    def test_acknowledgement_is_persisted_only_after_upload(self):
        self.upload.side_effect = lambda *args: self.assertEqual(self.writes[-1]["upload"]["status"], "pending")
        publish.main()
        self.assertEqual([s["upload"]["status"] for s in self.writes], ["pending", "accepted"])
        self.assertEqual(self.writes[-1]["runAttempt"], "2")
        self.upload.assert_called_once()

    def test_failed_upload_keeps_measured_score_pending(self):
        self.upload.side_effect = RuntimeError("Synthetic OpenCamp outage")
        with self.assertRaisesRegex(RuntimeError, "Synthetic OpenCamp outage"):
            publish.main()
        self.assertEqual(len(self.writes), 1)
        self.assertEqual(self.writes[0]["score"], 0)
        self.assertEqual(self.writes[0]["upload"]["status"], "pending")

    def test_invalid_result_shapes_are_rejected(self):
        for data in [[], None, dict(self.result, schema=True), dict(self.result, exercises=[None] * len(course.COURSE['tests']))]:
            with self.assertRaises(ValueError):
                course.validate_result(data, self.repository, self.commit)


class UploadResponseTests(unittest.TestCase):
    def test_boolean_result_is_not_an_api_acknowledgement(self):
        with patch.object(publish, "urlopen", return_value=io.BytesIO(b'{"result":true}')):
            with self.assertRaisesRegex(RuntimeError, "rejected"):
                publish.upload_score({}, "placeholder-only")

    def test_timeout_and_truncated_response_are_clear_and_redacted(self):
        for error in [TimeoutError("placeholder-only timed out"), IncompleteRead(b"partial", 100)]:
            with patch.object(publish, "urlopen", side_effect=error):
                with self.assertRaisesRegex(RuntimeError, "response interrupted") as caught:
                    publish.upload_score({}, "placeholder-only")
            self.assertNotIn("placeholder-only", str(caught.exception))


if __name__ == "__main__":
    unittest.main()
