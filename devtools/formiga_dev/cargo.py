"""Turning what cargo prints into problems with a file, a line and a crate.

Commands are run with `--message-format=json` and their output and errors merged in order, so one
stream holds compiler messages as JSON lines, cargo's own "Running …" lines, and each test
binary's plain-text report. These functions read that stream back.
"""

from __future__ import annotations

import json
import re
from pathlib import Path
from typing import Dict, List, Optional

from .report import Problem
from .workspace import DESKTOP, relative

_PACKAGE = re.compile(r"#(?:[^@]+@)?([A-Za-z0-9_-]+)@|/([A-Za-z0-9_-]+)#")


def _package_of(message: Dict) -> Optional[str]:
    target = message.get("target") or {}
    if target.get("name"):
        return target["name"].replace("_", "-")
    match = _PACKAGE.search(message.get("package_id", ""))
    return (match.group(1) or match.group(2)) if match else None


def compiler_problems(output: str, step: str, root: Path = DESKTOP) -> List[Problem]:
    """Every error the compiler or clippy reported, at its primary span."""
    problems: List[Problem] = []
    seen = set()
    for line in output.splitlines():
        if not line.startswith("{"):
            continue
        try:
            message = json.loads(line)
        except ValueError:
            continue
        if message.get("reason") != "compiler-message":
            continue
        diagnostic = message.get("message") or {}
        if diagnostic.get("level") not in ("error", "error: internal compiler error"):
            continue
        spans = [span for span in diagnostic.get("spans", []) if span.get("is_primary")]
        if not spans:
            # "aborting due to N previous errors" and the like repeat what is already listed.
            continue
        span = spans[0]
        code = (diagnostic.get("code") or {}).get("code")
        text = diagnostic.get("message", "").strip()
        problem = Problem(
            step=step,
            message=f"{text} ({code})" if code else text,
            system=_package_of(message),
            file=relative(str(root / span["file_name"]) if not Path(span["file_name"]).is_absolute()
                          else span["file_name"], root),
            line=span.get("line_start"),
            detail=(diagnostic.get("rendered") or "").strip() or None,
        )
        key = (problem.file, problem.line, problem.message)
        if key not in seen:
            seen.add(key)
            problems.append(problem)
    return problems


_RUNNING = re.compile(r"^\s*Running (?:unittests )?(\S+) \((?:.*[/\\])?([^/\\]+?)(?:-[0-9a-f]{6,})?(?:\.exe)?\)")
_FAILED = re.compile(r"^test (\S+) \.\.\. FAILED")
_SECTION = re.compile(r"^---- (\S+) stdout ----$")
_PANIC_NEW = re.compile(r"panicked at (.+?):(\d+):\d+:$")
_PANIC_OLD = re.compile(r"panicked at '(.*)', (.+?):(\d+):\d+")
_RERUN = re.compile(r"^error: test failed, to rerun pass `(.*)`")
_PACKAGE_FLAG = re.compile(r"-p (\S+)")


def test_problems(output: str, step: str, root: Path = DESKTOP) -> List[Problem]:
    """Every failed test, with the file, line and message of its panic.

    libtest prints `test NAME ... FAILED` as each test ends, then a `---- NAME stdout ----`
    section per failure, and cargo then names the package in `error: test failed, to rerun pass
    `-p PACKAGE …``. Failures are gathered per test binary and given their package at that line.
    """
    problems: List[Problem] = []
    binary: Optional[str] = None
    failed: List[str] = []
    sections: Dict[str, Problem] = {}

    def flush(package: Optional[str], rerun: Optional[str]) -> None:
        for name in failed:
            problem = sections.get(name) or Problem(step=step, message="failed", object=name)
            problem.system = package or binary
            if rerun:
                hint = f"rerun: cargo test {rerun} -- --exact {name}"
                problem.detail = f"{problem.detail}\n{hint}" if problem.detail else hint
            problems.append(problem)
        failed.clear()
        sections.clear()

    lines = output.splitlines()
    index = 0
    while index < len(lines):
        line = lines[index]
        running = _RUNNING.match(line)
        if running:
            flush(None, None)
            binary = running.group(2).replace("_", "-")
        test = _FAILED.match(line)
        if test and test.group(1) not in failed:
            failed.append(test.group(1))
        section = _SECTION.match(line)
        if section:
            body: List[str] = []
            index += 1
            while index < len(lines) and not (
                lines[index] == "failures:"
                or lines[index].startswith("---- ")
                or lines[index].startswith("test result:")
            ):
                body.append(lines[index])
                index += 1
            sections[section.group(1)] = _panic(section.group(1), body, step, binary, root)
            continue
        rerun = _RERUN.match(line)
        if rerun:
            package_match = _PACKAGE_FLAG.search(rerun.group(1))
            package = package_match.group(1) if package_match else None
            if not failed:
                # The binary stopped without a test failing: it crashed, or would not start.
                problems.append(Problem(step=step, message=line[len("error: "):],
                                        system=package or binary))
            flush(package, rerun.group(1))
        index += 1
    flush(None, None)
    return problems


def _panic(name: str, body: List[str], step: str, binary: Optional[str], root: Path) -> Problem:
    file = line_number = None
    message_lines: List[str] = []
    for position, text in enumerate(body):
        new = _PANIC_NEW.search(text)
        if new:
            file, line_number = new.group(1), int(new.group(2))
            for following in body[position + 1:]:
                if following.startswith("note:") or following.startswith("stack backtrace"):
                    break
                message_lines.append(following)
            break
        old = _PANIC_OLD.search(text)
        if old:
            message_lines = [old.group(1)]
            file, line_number = old.group(2), int(old.group(3))
            break
    message = "\n".join(message_lines).strip() or "failed"
    first = message.splitlines()[0] if message else "failed"
    return Problem(
        step=step,
        message=first if len(first) <= 300 else first[:297] + "…",
        system=binary,
        file=relative(str(root / file), root) if file and not Path(file).is_absolute() else file,
        line=line_number,
        object=name,
        detail=message if message != first else None,
    )


_FMT = re.compile(r"^Diff in (.+?)(?::(\d+):| at line (\d+):)\s*$")


def format_problems(output: str, step: str, root: Path = DESKTOP) -> List[Problem]:
    """One problem per file rustfmt would change, at the first line it would change."""
    problems: Dict[str, Problem] = {}
    for line in output.splitlines():
        match = _FMT.match(line.strip())
        if not match:
            continue
        file = relative(match.group(1), root)
        if file not in problems:
            problems[file] = Problem(
                step=step,
                message="not formatted; `cargo fmt --all` fixes it",
                file=file,
                line=int(match.group(2) or match.group(3)),
            )
    return list(problems.values())
