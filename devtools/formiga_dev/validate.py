"""`formiga validate`: did this change break Formiga?

Runs the checks Formiga already has, in the order that fails fastest, and reports each as passed,
failed or skipped with its reason:

  format      cargo fmt --check
  lint        clippy on every crate, warnings as errors (as CI runs it)
  app         the desktop app itself, which builds only on macOS and Windows; elsewhere it is linted
              as a Windows build when the cross toolchain is installed, and skipped otherwise
  fixtures    the toolkit's fixed colonies: nothing `violations` names, a save that reads back
              unchanged, ten seconds of running (`formiga-tools dev check-fixtures`)
  tests       cargo test on every crate
  soak        a short run of the nightly soak: randomized colonies living a simulated day,
              written and read back, their files damaged and repaired
  hill, home  (with --expansions) the expansion's own tests, built on this checkout's Desktop
              crates instead of the release they are tied to

--quick runs format, lint and fixtures: seconds once things are built.
"""

from __future__ import annotations

import json
import os
from typing import Callable, Dict, List, Optional

from . import cargo
from .report import Problem, Result, Step
from .workspace import (
    APP_CRATE,
    DESKTOP,
    DEV,
    app_builds_here,
    desktop_version,
    find_expansion,
    lockfile_kept,
    patched,
    patched_target,
    run,
    windows_cross_check_available,
)

QUICK = ["format", "lint", "fixtures"]
DEFAULT = ["format", "lint", "app", "fixtures", "tests", "soak"]
EXPANSION_STEPS = ["hill", "home"]
ALL_STEPS = DEFAULT + EXPANSION_STEPS

# The short soak: enough colonies to cross a simulated day's routines, reloads and damaged files
# in well under a minute. The nightly CI run is 1,500 colonies over three days in three time zones.
SOAK_COLONIES = 120
SOAK_SEED = 20261006


def _workspace_scope() -> List[str]:
    return ["--workspace"] + ([] if app_builds_here() else ["--exclude", APP_CRATE])


def step_format() -> Step:
    done = run(["cargo", "fmt", "--all", "--check"])
    problems = cargo.format_problems(done.output, "format")
    if done.code != 0 and not problems:
        problems = [Problem("format", done.output.strip().splitlines()[-1] if done.output.strip()
                            else "cargo fmt failed")]
    return Step("format", "failed" if done.code else "passed",
                f"{len(problems)} file(s) need formatting" if done.code else "formatted",
                done.seconds, problems)


def _clippy(step: str, extra: List[str], env: Optional[Dict[str, str]] = None) -> Step:
    done = run(["cargo", "clippy", *extra, "--all-targets", "--message-format=json",
                "--", "-D", "warnings"], env=env)
    problems = cargo.compiler_problems(done.output, step)
    if done.code != 0 and not problems:
        problems = [Problem(step, _last_error(done.output))]
    return Step(step, "failed" if done.code else "passed",
                f"{len(problems)} error(s)" if done.code else "no warnings",
                done.seconds, problems)


def step_lint() -> Step:
    step = _clippy("lint", _workspace_scope())
    if not app_builds_here() and step.status == "passed":
        step.summary += f" (all crates but {APP_CRATE}; see app)"
    return step


def step_app(with_lint: bool = False) -> Step:
    if app_builds_here():
        if with_lint:
            return Step("app", "skipped", "linted and tested with the other crates on this machine")
        step = _clippy("app", ["-p", APP_CRATE])
        if step.status == "passed":
            step.summary = "linted; its tests run with the tests step"
        return step
    available, why = windows_cross_check_available()
    if not available:
        return Step("app", "skipped",
                    f"{APP_CRATE} builds only on macOS and Windows, and {why}; CI checks it")
    step = _clippy("app", ["--target", "x86_64-pc-windows-gnu", "-p", APP_CRATE],
                   env={"CARGO_TARGET_DIR": str(DEV / "target" / "windows")})
    if step.status == "passed":
        step.summary = "linted as a Windows build; its tests run only on Windows, macOS and CI"
    return step


def step_fixtures() -> Step:
    done = run(["cargo", "run", "-q", "-p", "formiga-tools", "--", "dev", "check-fixtures"])
    report = _json_document(done.output)
    if report is None:
        problems = cargo.compiler_problems(done.output, "fixtures") or [
            Problem("fixtures", _last_error(done.output))]
        return Step("fixtures", "failed", "could not run the fixture check", done.seconds, problems)
    problems = [
        Problem("fixtures", message, system="formiga-core", object=f"fixture:{fixture['name']}",
                file="crates/formiga-tools/src/dev.rs")
        for fixture in report["fixtures"]
        for message in fixture["problems"]
    ]
    names = [fixture["name"] for fixture in report["fixtures"]]
    return Step("fixtures", "failed" if problems else "passed",
                f"{len(names)} colonies hold ({', '.join(names)})" if not problems
                else f"{len(problems)} broken rule(s)", done.seconds, problems)


def step_tests() -> Step:
    done = run(["cargo", "test", *_workspace_scope(), "--no-fail-fast", "--message-format=json"])
    problems = cargo.compiler_problems(done.output, "tests") + cargo.test_problems(done.output, "tests")
    passed = sum(_count(line, "passed") for line in done.output.splitlines())
    failed = sum(_count(line, "failed") for line in done.output.splitlines())
    if done.code != 0 and not problems:
        problems = [Problem("tests", _last_error(done.output))]
    summary = f"{passed} passed" + (f", {failed} failed" if failed else "")
    return Step("tests", "failed" if done.code else "passed", summary, done.seconds, problems)


def step_soak() -> Step:
    out = DEV / "soak"
    threads = str(max(1, (os.cpu_count() or 2)))
    done = run(["cargo", "run", "-q", "--release", "-p", "formiga-tools", "--", "soak",
                "--colonies", str(SOAK_COLONIES), "--days", "1", "--seed", str(SOAK_SEED),
                "--damage", "2", "--threads", threads, "--out", str(out)])
    problems = []
    for line in done.output.splitlines():
        if line.startswith("colony ") and ":" in line:
            colony, what = line.split(":", 1)
            number = colony.split()[1]
            problems.append(Problem(
                "soak", what.strip(), system="formiga-core", object=colony,
                detail=f"replay: cargo run --release -p formiga-tools -- soak --seed {SOAK_SEED} "
                       f"--only {number} --days 1 --damage 2 --out {out}; its last good save is in "
                       f"{out}"))
    if done.code != 0 and not problems:
        problems = cargo.compiler_problems(done.output, "soak") or [
            Problem("soak", _last_error(done.output))]
    summary_line = next((line[2:] for line in done.output.splitlines()
                         if line.startswith("# lived")), "")
    return Step("soak", "failed" if done.code else "passed",
                f"{SOAK_COLONIES} colonies for a day" + (f"; {summary_line}" if summary_line else ""),
                done.seconds, problems)


def step_expansion(key: str) -> Step:
    expansion = find_expansion(key)
    if not expansion.path:
        return Step(key, "skipped",
                    f"no {expansion.title} checkout beside this one (or set FORMIGA_{key.upper()}_REPO)")
    pins = sorted(set(expansion.pins().values()))
    with lockfile_kept(expansion.path):
        done = run(["cargo", "test", "--workspace", "--no-fail-fast", "--message-format=json",
                    *patched(expansion)],
                   cwd=expansion.path, env={"CARGO_TARGET_DIR": str(patched_target(expansion))})
    problems = (cargo.compiler_problems(done.output, key, expansion.path)
                + cargo.test_problems(done.output, key, expansion.path))
    for problem in problems:
        problem.system = problem.system or expansion.package
        if problem.file and not problem.file.startswith("/"):
            problem.file = f"{expansion.path.name}/{problem.file}"
    if done.code != 0 and not problems:
        problems = [Problem(key, _last_error(done.output), system=expansion.package)]
    passed = sum(_count(line, "passed") for line in done.output.splitlines())
    tied = f"tied to Desktop {', '.join(pins)}" if pins else "tied to no Desktop tag"
    summary = (f"{passed} tests pass on this Desktop ({desktop_version()}); {tied}"
               if done.code == 0 else f"fails on this Desktop ({desktop_version()}); {tied}")
    return Step(key, "failed" if done.code else "passed", summary, done.seconds, problems)


STEPS: Dict[str, Callable[[], Step]] = {
    "format": step_format,
    "lint": step_lint,
    "app": step_app,
    "fixtures": step_fixtures,
    "tests": step_tests,
    "soak": step_soak,
    "hill": lambda: step_expansion("hill"),
    "home": lambda: step_expansion("home"),
}


def validate(quick: bool = False, expansions: bool = False,
             only: Optional[List[str]] = None) -> Result:
    names = only or (QUICK if quick else DEFAULT)
    if expansions and not only:
        names = names + EXPANSION_STEPS
    result = Result("validate", data={"desktop_version": desktop_version(),
                                      "checkout": str(DESKTOP)})
    for name in names:
        # The app is linted with every other crate where it builds, so lint covers it then.
        step = step_app("lint" in names) if name == "app" else STEPS[name]()
        result.steps.append(step)
    return result


def _count(line: str, what: str) -> int:
    """The number before `what` in a libtest `test result:` line."""
    if not line.startswith("test result:"):
        return 0
    for part in line.split(";"):
        words = part.strip().split()
        if len(words) >= 2 and words[-1] == what and words[-2].isdigit():
            return int(words[-2])
        if len(words) >= 2 and words[1] == what and words[0].isdigit():
            return int(words[0])
    return 0


def _json_document(output: str) -> Optional[dict]:
    start = output.find("{\n")
    if start < 0:
        return None
    try:
        return json.loads(output[start:])
    except ValueError:
        return None


def _last_error(output: str) -> str:
    lines = [line for line in output.strip().splitlines() if line.strip()]
    errors = [line for line in lines if line.lstrip().startswith("error")]
    return (errors or lines or ["failed with no output"])[-1].strip()
