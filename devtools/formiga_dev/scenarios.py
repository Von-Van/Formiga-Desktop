"""`formiga scenario NAME`: open Formiga in a known state, without clicking your way there.

Every scenario starts from something fixed, so the same name always gives the same state:

  Desktop     one of `formiga-tools dev`'s fixed colonies, written as colony.json into a fresh
              data folder under .dev/scenarios/NAME/, which FORMIGA_DATA_DIR points the app at
              (first-run is an empty folder: Formiga's real first launch)
  trips       a fixed colony, with Desktop told to send it to Hill or Home a few seconds after
              opening, using the Hill or Home built from the checkout beside this one
  Hill, Home  their own fixed sample colony, in a fresh data folder of their own

The colony is grown to the moment you open it, so it is the age its name says and nothing is
caught up on launch; `--fixed-date` grows it to the fixtures' fixed date instead, for a file that is
byte-for-byte the same every time.

The Desktop app runs only on macOS and Windows. Elsewhere, and always with `--no-launch`, the
scenario is prepared and the command to open it is printed instead.

Hill and Home are built against this checkout's Desktop crates unless `--pinned` is given, so a
change to Desktop's drawing shows up in them too.
"""

from __future__ import annotations

import json
import os
import shlex
import shutil
import subprocess
import sys
import time
from dataclasses import dataclass, field
from pathlib import Path
from typing import Dict, List, Optional

from . import cargo
from .report import Problem, Result, Step
from .workspace import (
    APP_CRATE,
    DESKTOP,
    DEV,
    app_builds_here,
    find_expansion,
    lockfile_kept,
    patched,
    patched_target,
    run,
)


@dataclass
class Capture:
    """How a scenario is pictured. `render` draws it off-screen, exactly the same every time;
    `window` opens the real window and pictures it (the expansions' --snap)."""

    render: List[str]
    window: Optional[List[str]] = None


@dataclass
class Scenario:
    name: str
    app: str  # "desktop" | "hill" | "home"
    about: str
    fixture: Optional[str] = None  # a `formiga-tools dev` fixture, for Desktop and trips
    args: List[str] = field(default_factory=list)  # the expansion's own arguments
    trip: Optional[str] = None  # "hill" | "home": Desktop starts this visit by itself
    launchable: bool = True
    capture: Optional[Capture] = None


# Hill's places beyond the station, which `hill` itself covers. Each can be drawn off-screen, and
# pictured in a window opened there with --snap; Hill's window always opens at the station, so
# these are for capture only.
HILL_PLACES = {
    "green": "free play on the Village Green",
    "clubhouse": "the Clubhouse",
    "fairground": "the Fairground",
    "woods": "a rummage in the Woods",
    "hilltop": "the Hilltop with a sample of finds placed",
}

SCENARIOS: Dict[str, Scenario] = {}


def _add(scenario: Scenario) -> None:
    SCENARIOS[scenario.name] = scenario


_add(Scenario("first-run", "desktop", "Formiga's very first launch: no colony yet, the welcome "
              "and the first creature's arrival.", fixture=None))
for _fixture, _about in [
    ("new-colony", "The founder alone, just met: welcome finished, nothing else yet."),
    ("young-colony", "Ten days in: the first arrivals, a little history."),
    ("mature-colony", "Half a year in: a full household."),
    ("full-village", "Forty days in with every decoration, a full set of objects, a guest at the "
                     "door and a full guest book."),
    ("pond-village", "The half-year household laid out on the pond scenery, the houses coming out "
                     "as soon as it opens."),
]:
    _add(Scenario(_fixture, "desktop", _about, fixture=_fixture,
                  capture=Capture(render=["dev-capture"])))
_add(Scenario("hill-trip", "desktop", "The mature colony, sent to Formiga Hill five seconds "
              "after Desktop opens.", fixture="mature-colony", trip="hill"))
_add(Scenario("home-visit", "desktop", "The mature colony, visiting Formiga Home five seconds "
              "after Desktop opens.", fixture="mature-colony", trip="home"))
_add(Scenario("hill", "hill", "Formiga Hill on its sample colony, arriving at the station.",
              args=["--sample", "--hour", "12"],
              capture=Capture(render=["--render-station"], window=["--place", "station"])))
for _place, _about in HILL_PLACES.items():
    _add(Scenario(f"hill-{_place}", "hill", f"Formiga Hill's sample colony: {_about}.",
                  args=["--sample", "--hour", "12"], launchable=False,
                  capture=Capture(render=[f"--render-{_place}"], window=["--place", _place])))
_add(Scenario("home", "home", "Formiga Home's sample household in its front room, at midday in "
              "June.", args=["--sample", "--hour", "12", "--month", "6"],
              capture=Capture(render=["--render-room"], window=[])))
_add(Scenario("home-lived-in", "home", "Formiga Home's sample household a few weeks on: three "
              "rooms, finds and keepsakes out.",
              args=["--sample", "--hour", "12", "--month", "6", "--lived-in", "--rooms", "3"],
              capture=Capture(render=["--render-room", "--at", "45"], window=[])))


def scenario_dir(name: str) -> Path:
    return DEV / "scenarios" / name


def fresh(folder: Path) -> Path:
    if folder.exists():
        shutil.rmtree(folder)
    folder.mkdir(parents=True)
    return folder


def write_fixture(fixture: str, folder: Path, fixed_date: bool) -> Step:
    command = ["cargo", "run", "-q", "-p", "formiga-tools", "--", "dev", "fixture", fixture,
               "--out", str(folder)]
    if not fixed_date:
        command += ["--now-unix", str(int(time.time()))]
    done = run(command)
    if done.code != 0:
        return Step("colony", "failed", f"could not write the {fixture} colony", done.seconds,
                    cargo.compiler_problems(done.output, "colony") or [
                        Problem("colony", done.output.strip().splitlines()[-1]
                                if done.output.strip() else "formiga-tools failed")])
    names = []
    start = done.output.find("{")
    try:
        names = [creature["name"] for creature in json.loads(done.output[start:])["creatures"]]
    except (ValueError, KeyError):
        pass
    return Step("colony", "passed",
                f"{fixture}: {len(names)} creature(s){': ' + ', '.join(names) if names else ''}",
                done.seconds)


def build_desktop() -> tuple:
    done = run(["cargo", "build", "--release", "-p", APP_CRATE])
    binary = DESKTOP / "target" / "release" / ("formiga.exe" if sys.platform == "win32" else "formiga")
    if done.code != 0:
        return None, Step("build", "failed", "the desktop app did not build", done.seconds,
                          cargo.compiler_problems(done.output, "build"))
    return binary, Step("build", "passed", f"built {binary.relative_to(DESKTOP)}", done.seconds)


def build_expansion(key: str, pinned: bool) -> tuple:
    """Build Hill or Home for release, on this checkout's Desktop unless `pinned`."""
    expansion = find_expansion(key)
    if not expansion.path:
        return None, Step("build", "failed", f"no {expansion.title} checkout found", problems=[
            Problem("build", f"put {expansion.title}'s checkout beside this one as "
                             f"Formiga-{key.title()}, or set FORMIGA_{key.upper()}_REPO")])
    exe = expansion.package + (".exe" if sys.platform == "win32" else "")
    if pinned:
        done = run(["cargo", "build", "--release", "-p", expansion.package], cwd=expansion.path)
        binary = expansion.path / "target" / "release" / exe
        on = "its own Desktop release"
    else:
        target = patched_target(expansion)
        with lockfile_kept(expansion.path):
            done = run(["cargo", "build", "--release", "-p", expansion.package,
                        *patched(expansion)],
                       cwd=expansion.path, env={"CARGO_TARGET_DIR": str(target)})
        binary = target / "release" / exe
        on = "this checkout's Desktop"
    if done.code != 0:
        return None, Step("build", "failed", f"{expansion.title} did not build on {on}",
                          done.seconds,
                          cargo.compiler_problems(done.output, "build", expansion.path))
    return binary, Step("build", "passed", f"built {expansion.title} on {on}", done.seconds)


def _command_line(env: Dict[str, str], command: List[str], platform: str = sys.platform) -> str:
    """The command as one line to paste into this machine's usual shell: PowerShell on Windows,
    sh elsewhere."""
    if platform == "win32":
        def literal(text: str) -> str:
            return "'" + text.replace("'", "''") + "'"

        settings = "".join(f"$env:{key} = {literal(value)}; " for key, value in env.items())
        return f"{settings}& {' '.join(literal(part) for part in command)}"
    assignments = " ".join(f"{key}={shlex.quote(value)}" for key, value in env.items())
    return f"{assignments} {' '.join(shlex.quote(part) for part in command)}".strip()


def prepare(scenario: Scenario, pinned: bool, fixed_date: bool) -> tuple:
    """Everything up to opening the app: the data folder, the colony, the builds.

    Returns (command, env, steps); command is None when something failed."""
    steps: List[Step] = []
    folder = fresh(scenario_dir(scenario.name))
    if scenario.app == "desktop":
        data = folder / "data"
        data.mkdir()
        env = {"FORMIGA_DATA_DIR": str(data)}
        if scenario.fixture:
            steps.append(write_fixture(scenario.fixture, data, fixed_date))
        else:
            steps.append(Step("colony", "passed", "an empty data folder: a first launch"))
        if scenario.trip:
            binary, built = build_expansion(scenario.trip, pinned)
            steps.append(built)
            if binary:
                expansion = find_expansion(scenario.trip)
                env[f"FORMIGA_{scenario.trip.upper()}_PATH"] = str(binary)
                env[expansion.data_env] = str(fresh(folder / f"{scenario.trip}-data"))
                env["FORMIGA_HILL_TRIP_AFTER" if scenario.trip == "hill"
                    else "FORMIGA_HOME_VISIT_AFTER"] = "5"
        if not app_builds_here():
            return (["cargo", "run", "--release", "-p", APP_CRATE], env, steps)
        binary, built = build_desktop()
        steps.append(built)
        command = [str(binary)] if binary else None
        return (command, env, steps)
    expansion = find_expansion(scenario.app)
    binary, built = build_expansion(scenario.app, pinned)
    steps.append(built)
    env = {expansion.data_env: str(fresh(folder / "data"))}
    return ([str(binary), *scenario.args] if binary else None, env, steps)


def launch(name: str, no_launch: bool = False, wait: bool = False, pinned: bool = False,
           fixed_date: bool = False) -> Result:
    result = Result("scenario", data={"scenario": name})
    scenario = SCENARIOS.get(name)
    if not scenario:
        result.error = f"no scenario called {name!r}; `formiga scenario list` shows them"
        return result
    result.data.update({"app": scenario.app, "about": scenario.about})
    if not scenario.launchable:
        result.error = (f"{name} can be captured but not opened for play: Hill's window always "
                        f"opens at the station. Try `formiga capture {name}` (add --window for "
                        "the real window), or `formiga scenario hill`.")
        return result
    command, env, steps = prepare(scenario, pinned, fixed_date)
    result.steps += steps
    if any(step.status == "failed" for step in steps) or command is None:
        return result
    line = _command_line(env, command)
    result.data.update({"command": command, "env": env, "folder": str(scenario_dir(name))})
    if scenario.app == "desktop" and not app_builds_here():
        result.steps.append(Step("open", "skipped",
                                 "the desktop app runs only on macOS and Windows; on one of "
                                 "those, run the command below"))
        result.data["notes"] = [f"To open it: {line}"]
        return result
    if no_launch:
        result.steps.append(Step("open", "skipped", "--no-launch"))
        result.data["notes"] = [f"To open it: {line}"]
        return result
    started = time.monotonic()
    process = subprocess.Popen(command, cwd=DESKTOP, env={**os.environ, **env})
    result.data["pid"] = process.pid
    if wait:
        code = process.wait()
        result.steps.append(Step("open", "passed" if code == 0 else "failed",
                                 f"closed with exit code {code}", time.monotonic() - started))
    else:
        result.steps.append(Step("open", "passed", f"opened (process {process.pid}); the data "
                                 f"folder is {scenario_dir(name).relative_to(DESKTOP)}"))
    return result


def listing() -> Result:
    rows = []
    for scenario in SCENARIOS.values():
        rows.append({
            "name": scenario.name,
            "app": scenario.app,
            "about": scenario.about,
            "opens": scenario.launchable and (scenario.app != "desktop" or app_builds_here()),
            "launchable": scenario.launchable,
            "capture": None if not scenario.capture else (
                "render and window" if scenario.capture.window is not None else "render"),
        })
    result = Result("scenario list", data={"scenarios": rows})
    width = max(len(row["name"]) for row in rows)
    notes = []
    for row in rows:
        marks = []
        if not row["launchable"]:
            marks.append("capture only")
        elif row["app"] == "desktop" and not app_builds_here():
            marks.append("opens on macOS/Windows")
        if row["capture"]:
            marks.append(f"capture: {row['capture']}")
        notes.append(f"{row['name']:<{width}}  {row['app']:<7} {row['about']}"
                     + (f"  ({'; '.join(marks)})" if marks else ""))
    result.data["notes"] = notes
    return result
