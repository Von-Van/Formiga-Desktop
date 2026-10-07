"""`formiga capture NAME`: a picture of a scenario, for looking at a change.

By default the picture is drawn off-screen by Formiga's own drawing code, so the same scenario on
the same code gives the same pixels on any machine, Linux included:

  Desktop     the colony card and every resident's card, drawn by formiga-art from the
              scenario's fixed colony (`formiga-tools dev capture`). Desktop's live overlay can't
              be photographed: by design it never reads the screen and has no picture to hand back.
  Hill        `formiga-hill --sample --render-PLACE` at midday: the scene at its native 384×216,
              three times over.
  Home        `formiga-home --sample --render-room` at midday in June.

With `--window`, Hill and Home are opened for real at their own fixed size and pictured by their
own `--snap` a few seconds in (on Linux without a display, inside xvfb-run). Those pictures include
the on-screen controls and cards, but are timed by the clock, so they are close, not exact.

Each capture is written to .dev/captures/NAME.png (NAME-window.png with --window), with a
NAME.json beside it saying what was drawn, from which commit, by which command.
"""

from __future__ import annotations

import json
import os
import shutil
import struct
import subprocess
import sys
import tempfile
import time
from pathlib import Path
from typing import List, Optional

from . import cargo
from .report import Problem, Result, Step
from .scenarios import SCENARIOS, build_expansion
from .workspace import DESKTOP, DEV, find_expansion, run

CAPTURES = DEV / "captures"
# How far into the window's life --snap takes its picture.
WINDOW_SECONDS = "3"


def png_size(path: Path) -> Optional[tuple]:
    with path.open("rb") as file:
        head = file.read(24)
    if len(head) < 24 or head[:8] != b"\x89PNG\r\n\x1a\n":
        return None
    return struct.unpack(">II", head[16:24])


def commit() -> str:
    head = subprocess.run(["git", "rev-parse", "--short", "HEAD"], cwd=DESKTOP,
                          capture_output=True, text=True).stdout.strip()
    dirty = subprocess.run(["git", "status", "--porcelain", "--untracked-files=no"], cwd=DESKTOP,
                           capture_output=True, text=True).stdout.strip()
    return f"{head}{' with local changes' if dirty else ''}" if head else "unknown"


def _display_wrapper() -> tuple:
    """What to run a window under on this machine, or why it can't be."""
    if sys.platform in ("win32", "darwin") or os.environ.get("DISPLAY") \
            or os.environ.get("WAYLAND_DISPLAY"):
        return [], None
    xvfb = shutil.which("xvfb-run")
    if xvfb:
        return [xvfb, "-a", "-s", "-screen 0 1920x1080x24"], None
    return None, "no display, and xvfb-run is not installed to make one"


def capture(name: str, window: bool = False, pinned: bool = False,
            out: Optional[str] = None) -> Result:
    result = Result("capture", data={"scenario": name})
    scenario = SCENARIOS.get(name)
    if not scenario:
        result.error = f"no scenario called {name!r}; `formiga scenario list` shows them"
        return result
    if not scenario.capture:
        result.error = (f"{name} has nothing to draw: "
                        + ("a first launch has no colony yet." if name == "first-run"
                           else "it is a trip between apps; capture its parts, such as "
                                "mature-colony and hill."))
        return result
    if window and scenario.capture.window is None:
        result.error = ("Desktop's overlay can't be pictured in a window: it never reads the "
                        "screen. Leave out --window for its cards.")
        return result
    CAPTURES.mkdir(parents=True, exist_ok=True)
    target = Path(out).resolve() if out else CAPTURES / f"{name}{'-window' if window else ''}.png"
    target.parent.mkdir(parents=True, exist_ok=True)
    if target.exists():
        target.unlink()

    if scenario.app == "desktop":
        command = ["cargo", "run", "-q", "-p", "formiga-tools", "--", "dev", "capture",
                   scenario.fixture, "--out", str(target)]
        done = run(command)
        drew = None
        if done.code == 0:
            try:
                drew = json.loads(done.output[done.output.find("{"):])["drawn"]
            except (ValueError, KeyError):
                drew = None
        step = _finish(done.code, done.output, done.seconds, target, "colony and creature cards")
        result.steps.append(step)
        how = {"drawn": drew, "command": command}
    else:
        binary, built = build_expansion(scenario.app, pinned)
        result.steps.append(built)
        if not binary:
            return result
        expansion = find_expansion(scenario.app)
        with tempfile.TemporaryDirectory(prefix="formiga-capture-") as data:
            env = {**os.environ, expansion.data_env: data}
            if window:
                wrapper, why = _display_wrapper()
                if wrapper is None:
                    result.steps.append(Step("capture", "failed", why, problems=[
                        Problem("capture", why, system=expansion.package)]))
                    return result
                command = [*wrapper, str(binary), *scenario.args, "--snap", str(target),
                           *scenario.capture.window, "--at", WINDOW_SECONDS]
                what = "the real window, a few seconds in"
            else:
                render, *extra = scenario.capture.render
                command = [str(binary), *scenario.args, render, str(target), *extra]
                what = "drawn off-screen"
            started = time.monotonic()
            done = subprocess.run(command, cwd=expansion.path, env=env, capture_output=True,
                                  text=True, timeout=300)
            output = (done.stdout or "") + (done.stderr or "")
            result.steps.append(_finish(done.returncode, output, time.monotonic() - started,
                                        target, what))
        how = {"command": [part if part != str(target) else "OUT.png" for part in command]}

    if result.success:
        width, height = png_size(target) or (None, None)
        record = {
            "scenario": name,
            "app": scenario.app,
            "about": scenario.about,
            "kind": "window" if window else "render",
            "path": str(target),
            "width": width,
            "height": height,
            "desktop_commit": commit(),
            "desktop_crates": "the expansion's pinned Desktop release" if pinned
            and scenario.app != "desktop" else "this checkout",
            "made_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
            **how,
        }
        target.with_suffix(".json").write_text(json.dumps(record, indent=2), encoding="utf-8")
        result.data.update({"path": str(target), "width": width, "height": height,
                            "record": str(target.with_suffix(".json"))})
        try:
            shown = target.relative_to(DESKTOP)
        except ValueError:
            shown = target
        result.data["notes"] = [f"Captured: {shown} ({width}×{height})"]
    return result


def _finish(code: int, output: str, seconds: float, target: Path, what: str) -> Step:
    if code == 0 and target.is_file() and png_size(target):
        return Step("capture", "passed", what, seconds)
    problems: List[Problem] = cargo.compiler_problems(output, "capture")
    if not problems:
        lines = [line for line in output.strip().splitlines() if line.strip()]
        message = _reason(lines) or f"exited with code {code} and wrote no picture"
        if code == 0:
            message = f"finished but wrote no picture at {target}"
        problems = [Problem("capture", message, detail="\n".join(lines[-20:]) or None)]
    return Step("capture", "failed", "no picture", seconds, problems)


def _reason(lines: List[str]) -> Optional[str]:
    """The line that says why: a panic's message, else the last error, else the last line."""
    for index, line in enumerate(lines):
        if "panicked at" in line and index + 1 < len(lines):
            return lines[index + 1].strip()
    errors = [line for line in lines if line.lower().lstrip().startswith("error")]
    quiet = [line for line in lines if not line.startswith(("note:", "stack backtrace"))]
    return (errors or quiet or [None])[-1]
