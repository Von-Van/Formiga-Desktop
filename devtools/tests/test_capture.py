"""Checks for `formiga capture` running an app it was given: python3 -m unittest discover devtools/tests"""

from __future__ import annotations

import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))

from formiga_dev import capture  # noqa: E402


class CaptureLaunch(unittest.TestCase):
    def test_a_given_file_that_cannot_be_run_fails_the_capture_step(self):
        with tempfile.TemporaryDirectory() as folder:
            program = Path(folder) / "formiga-battle"
            program.write_text("")
            program.chmod(0o644)
            out = Path(folder) / "board.png"
            result = capture.capture("battle-cove", binary=str(program), out=str(out))
        self.assertIsNone(result.error)
        self.assertFalse(result.success)
        step = result.steps[-1]
        self.assertEqual((step.name, step.status), ("capture", "failed"))
        self.assertIn("could not run", step.problems[0].message)

    def test_a_given_app_that_never_finishes_fails_the_capture_step(self):
        with tempfile.TemporaryDirectory() as folder:
            program = Path(folder) / "formiga-battle"
            program.write_text("")
            out = Path(folder) / "board.png"
            real_run = subprocess.run

            def run(command, *args, **kwargs):
                if command[0] == str(program.resolve()):
                    raise subprocess.TimeoutExpired(command, kwargs["timeout"])
                return real_run(command, *args, **kwargs)

            with mock.patch.object(capture.subprocess, "run", side_effect=run):
                result = capture.capture("battle-cove", binary=str(program), out=str(out))
        self.assertFalse(result.success)
        self.assertIn("within 300 seconds", result.steps[-1].problems[0].message)


if __name__ == "__main__":
    unittest.main()
