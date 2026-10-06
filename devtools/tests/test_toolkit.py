"""Checks for the toolkit itself: python3 -m unittest discover devtools/tests

The cargo samples in fixtures/ are real output from a two-crate workspace: one with a type error,
then one with two failing tests (a panic and an assert_eq), run as validate runs cargo.
"""

from __future__ import annotations

import io
import json
import sys
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))

from formiga_dev import cargo, scenarios, validate  # noqa: E402
from formiga_dev.report import Problem, Result, Step, emit  # noqa: E402
from formiga_dev.workspace import Expansion, lockfile_kept  # noqa: E402

FIXTURES = HERE / "fixtures"
WS = Path("/WS")


class CompilerMessages(unittest.TestCase):
    def test_a_type_error_names_its_crate_file_and_line(self):
        output = (FIXTURES / "cargo-out-compile.txt").read_text()
        problems = cargo.compiler_problems(output, "tests", WS)
        self.assertEqual(len(problems), 1)
        problem = problems[0]
        self.assertEqual(problem.system, "demo-tools")
        self.assertEqual(problem.file, "crates/demo-tools/src/lib.rs")
        self.assertEqual(problem.line, 1)
        self.assertIn("mismatched types", problem.message)
        self.assertIn("E0308", problem.message)


class TestFailures(unittest.TestCase):
    def setUp(self):
        output = (FIXTURES / "cargo-out-tests.txt").read_text()
        self.problems = {p.object: p for p in cargo.test_problems(output, "tests", WS)}

    def test_every_failure_is_found_once(self):
        self.assertEqual(sorted(self.problems),
                         ["tests::decoration_keeps_its_slot", "tests::panics_plainly"])

    def test_a_failure_carries_its_package_panic_site_and_message(self):
        problem = self.problems["tests::decoration_keeps_its_slot"]
        self.assertEqual(problem.system, "demo-core")
        self.assertEqual(problem.file, "crates/demo-core/src/lib.rs")
        self.assertEqual(problem.line, 11)
        self.assertEqual(problem.message, "assertion `left == right` failed: Banner lost its slot")
        self.assertIn("right: 3", problem.detail)
        self.assertIn("cargo test -p demo-core --lib -- --exact", problem.detail)

    def test_a_plain_panic_keeps_its_words(self):
        problem = self.problems["tests::panics_plainly"]
        self.assertEqual(problem.message, "the colony file has grown to 9000000 bytes")
        self.assertEqual(problem.line, 15)

    def test_passing_counts_are_read_from_result_lines(self):
        line = "test result: FAILED. 1 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out"
        self.assertEqual(validate._count(line, "passed"), 1)
        self.assertEqual(validate._count(line, "failed"), 2)
        self.assertEqual(validate._count("running 3 tests", "passed"), 0)


class Formatting(unittest.TestCase):
    def test_each_unformatted_file_is_named_once_at_its_first_change(self):
        output = ("Diff in /WS/crates/a/src/lib.rs:12:\n-x\n+y\n"
                  "Diff in /WS/crates/a/src/lib.rs:40:\n-x\n+y\n"
                  "Diff in /WS/crates/b/src/main.rs at line 3:\n")
        problems = cargo.format_problems(output, "format", WS)
        self.assertEqual([(p.file, p.line) for p in problems],
                         [("crates/a/src/lib.rs", 12), ("crates/b/src/main.rs", 3)])


class Scenarios(unittest.TestCase):
    def test_every_desktop_scenario_names_a_fixture_formiga_tools_has(self):
        dev = (HERE.parents[1] / "crates/formiga-tools/src/dev.rs").read_text()
        for scenario in scenarios.SCENARIOS.values():
            if scenario.fixture:
                self.assertIn(f'name: "{scenario.fixture}"', dev, scenario.name)

    def test_every_hill_place_is_a_render_and_a_snap_place_hill_knows(self):
        main = HERE.parents[2] / "Formiga-Hill/crates/formiga-hill/src/main.rs"
        if not main.exists():
            self.skipTest("no Formiga-Hill checkout beside this one")
        usage = main.read_text()
        for place in ["station", *scenarios.HILL_PLACES]:
            self.assertIn(f"--render-{place} ", usage)
            self.assertIn(place, usage.split("--place <PLACE>", 1)[1].split("\n--", 1)[0])

    def test_scenarios_that_only_capture_say_so(self):
        for scenario in scenarios.SCENARIOS.values():
            if not scenario.launchable:
                self.assertIsNotNone(scenario.capture, scenario.name)


class Expansions(unittest.TestCase):
    def test_pins_are_read_from_the_manifest(self):
        with tempfile.TemporaryDirectory() as folder:
            (Path(folder) / "Cargo.toml").write_text(
                '[dependencies]\n'
                'formiga-art = { git = "https://github.com/Von-Van/Formiga-Desktop", tag = "v0.67.1" }\n'
                'formiga-core = { git = "https://github.com/Von-Van/Formiga-Desktop", tag = "v0.67.1" }\n'
                'serde = "1"\n')
            expansion = Expansion("hill", "Formiga Hill", "formiga-hill", "X", Path(folder))
            self.assertEqual(expansion.pins(), {"formiga-art": "v0.67.1", "formiga-core": "v0.67.1"})

    def test_the_lockfile_comes_back_as_it_was(self):
        with tempfile.TemporaryDirectory() as folder:
            lock = Path(folder) / "Cargo.lock"
            lock.write_text("before")
            with lockfile_kept(Path(folder)):
                lock.write_text("patched")
            self.assertEqual(lock.read_text(), "before")


class Output(unittest.TestCase):
    def test_json_lists_every_problem_and_says_whether_it_passed(self):
        result = Result("validate", steps=[
            Step("lint", "failed", problems=[Problem("lint", "unused variable", file="a.rs", line=3)]),
            Step("app", "skipped", "macOS and Windows only"),
        ])
        out = io.StringIO()
        code = emit(result, as_json=True, out=out)
        document = json.loads(out.getvalue())
        self.assertEqual(code, 1)
        self.assertFalse(document["success"])
        self.assertEqual(document["errors"], [
            {"step": "lint", "message": "unused variable", "file": "a.rs", "line": 3}])

    def test_skipped_steps_do_not_fail_a_run(self):
        result = Result("validate", steps=[Step("app", "skipped", "macOS and Windows only")])
        self.assertTrue(result.success)


if __name__ == "__main__":
    unittest.main()
