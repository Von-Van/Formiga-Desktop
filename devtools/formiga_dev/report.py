"""What every command hands back: a result a person can skim and an agent can parse.

Each command builds one `Result`. With `--json` it is printed as one JSON document; without, as a
short summary. Problems carry what an agent needs to act without re-reading the terminal: the step
that found them, the crate or app, the file and line, and the Formiga object when one is known.
"""

from __future__ import annotations

import json
import sys
from dataclasses import asdict, dataclass, field
from typing import Any, Dict, List, Optional


@dataclass
class Problem:
    step: str
    message: str
    system: Optional[str] = None
    file: Optional[str] = None
    line: Optional[int] = None
    object: Optional[str] = None
    detail: Optional[str] = None
    # "warning" for something worth a look that does not fail the step; errors leave it unset.
    level: Optional[str] = None

    @property
    def is_warning(self) -> bool:
        return self.level == "warning"

    def to_json(self) -> Dict[str, Any]:
        return {key: value for key, value in asdict(self).items() if value is not None}

    def where(self) -> str:
        if self.file and self.line:
            return f"{self.file}:{self.line}"
        return self.file or ""


@dataclass
class Step:
    """One part of a command's work: passed, failed, or skipped with the reason."""

    name: str
    status: str  # "passed" | "failed" | "skipped"
    summary: str = ""
    seconds: Optional[float] = None
    problems: List[Problem] = field(default_factory=list)

    def to_json(self) -> Dict[str, Any]:
        out: Dict[str, Any] = {"name": self.name, "status": self.status, "summary": self.summary}
        if self.seconds is not None:
            out["seconds"] = round(self.seconds, 1)
        if self.problems:
            out["problems"] = [problem.to_json() for problem in self.problems]
        return out


@dataclass
class Result:
    command: str
    steps: List[Step] = field(default_factory=list)
    data: Dict[str, Any] = field(default_factory=dict)
    error: Optional[str] = None
    # Whether the human summary ends with "All passed." / "Failed: …": not for a lookup.
    verdict: bool = True

    @property
    def success(self) -> bool:
        return self.error is None and all(step.status != "failed" for step in self.steps)

    def to_json(self) -> Dict[str, Any]:
        out: Dict[str, Any] = {"command": self.command, "success": self.success}
        if self.error:
            out["error"] = self.error
        if self.steps:
            out["steps"] = [step.to_json() for step in self.steps]
            out["errors"] = [
                problem.to_json() for step in self.steps for problem in step.problems
                if not problem.is_warning
            ]
            warnings = [problem.to_json() for step in self.steps for problem in step.problems
                        if problem.is_warning]
            if warnings:
                out["warnings"] = warnings
        out.update(self.data)
        return out


MARKS = {"passed": "✓", "failed": "✗", "skipped": "–"}
# How many problems a step lists before saying how many more there are; --json lists them all.
SHOWN_PER_STEP = 8


def emit(result: Result, as_json: bool, out=None) -> int:
    """Print the result and return the process exit code."""
    out = out or sys.stdout
    if as_json:
        json.dump(result.to_json(), out, indent=2, ensure_ascii=False)
        out.write("\n")
        return 0 if result.success else 1
    if result.error:
        out.write(f"✗ {result.error}\n")
    for step in result.steps:
        took = f" ({step.seconds:.0f}s)" if step.seconds is not None and step.seconds >= 1 else ""
        summary = f": {step.summary}" if step.summary else ""
        out.write(f"{MARKS.get(step.status, '?')} {step.name}{summary}{took}\n")
        # Errors first: a warning never hides one.
        problems = sorted(step.problems, key=lambda problem: problem.is_warning)
        for problem in problems[:SHOWN_PER_STEP]:
            where = problem.where()
            subject = f" [{problem.object}]" if problem.object else ""
            mark = "⚠ " if problem.is_warning else ""
            out.write(f"    {mark}{where + ' ' if where else ''}{problem.message}{subject}\n")
        hidden = len(step.problems) - SHOWN_PER_STEP
        if hidden > 0:
            out.write(f"    … and {hidden} more (--json lists them all)\n")
    for line in result.data.get("notes", []):
        out.write(f"{line}\n")
    if result.steps and (result.verdict or not result.success):
        failed = [step.name for step in result.steps if step.status == "failed"]
        out.write("\n" + ("All passed.\n" if not failed else f"Failed: {', '.join(failed)}\n"))
    return 0 if result.success else 1
