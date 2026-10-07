"""`formiga report`: where things stand, for a person or agent starting work.

One look at:

  desktop     its version, save version, branch and commit, and any changes not yet committed
  checks      the last `formiga validate`, `formiga audit content` and `formiga art check`: when,
              on which commit, and what they found
  content     how many of each kind of thing Formiga has
  expansions  Hill and Home beside this checkout: their commit, and the Desktop release each
              builds on, next to this Desktop's version
  scenarios   the known states, and which have been prepared
  captures    the newest pictures in .dev/captures/

Validate, the content audit and the art check keep their last result in .dev/last/ for this.
Nothing is run that changes anything; only the content count builds anything (the catalog).
"""

from __future__ import annotations

import json
import re
import subprocess
from datetime import datetime, timezone
from pathlib import Path
from typing import Dict, List, Optional

from . import catalog, scenarios, validate
from .report import Result
from .workspace import DESKTOP, DEV, desktop_version, find_expansion, relative

LAST = DEV / "last"
CAPTURES = DEV / "captures"
# The commands whose last result is kept, and the name each is kept under.
RECORDED = {"validate": "validate", "audit content": "audit-content", "art check": "art-check"}
CAPTURES_SHOWN = 5


def git(*args: str, cwd: Path = DESKTOP) -> str:
    """What a git command prints, without its trailing newline; nothing if it fails."""
    done = subprocess.run(["git", *args], cwd=cwd, capture_output=True, text=True)
    return done.stdout.rstrip("\n") if done.returncode == 0 else ""


def now() -> str:
    return datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def record(command: str, result: Result) -> None:
    """Keep a command's result as its last, with the commit it ran on, for the report."""
    name = RECORDED.get(command)
    if not name:
        return
    document = result.to_json()
    document["at"] = now()
    document["commit"] = git("rev-parse", "HEAD")
    document["local_changes"] = bool(git("status", "--porcelain", "--untracked-files=no"))
    try:
        LAST.mkdir(parents=True, exist_ok=True)
        (LAST / f"{name}.json").write_text(json.dumps(document, indent=2), encoding="utf-8")
    except OSError:
        pass  # A report without it says the check has not been run; nothing else depends on it.


def last(command: str) -> Optional[dict]:
    path = LAST / f"{RECORDED[command]}.json"
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return None


def age(stamp: str) -> str:
    try:
        then = datetime.strptime(stamp, "%Y-%m-%dT%H:%M:%SZ").replace(tzinfo=timezone.utc)
    except ValueError:
        return stamp
    minutes = int((datetime.now(timezone.utc) - then).total_seconds() // 60)
    if minutes < 60:
        return f"{minutes} min ago"
    if minutes < 48 * 60:
        return f"{minutes // 60} h ago"
    return f"{minutes // (24 * 60)} days ago"


def since(commit: str, head: str) -> str:
    """How the commit a check ran on stands against the checkout now."""
    if not commit:
        return "on an unknown commit"
    if commit == head:
        return "on this commit"
    ancestor = subprocess.run(["git", "merge-base", "--is-ancestor", commit, head], cwd=DESKTOP,
                              capture_output=True).returncode == 0
    if not ancestor:
        return f"on {commit[:7]}, not in this branch's history"
    return f"{git('rev-list', '--count', f'{commit}..{head}')} commit(s) ago"


def save_version() -> str:
    text = (DESKTOP / "crates/formiga-core/src/lib.rs").read_text(encoding="utf-8")
    match = re.search(r"pub const SAVE_VERSION: u32 = (\d+);", text)
    return match.group(1) if match else "unknown"


def report() -> Result:
    result = Result("report", verdict=False)
    head = git("rev-parse", "HEAD")
    changed = [line for line in git("status", "--porcelain").splitlines() if line.strip()]
    version = desktop_version()
    desktop = {
        "version": version,
        "save_version": save_version(),
        "branch": git("rev-parse", "--abbrev-ref", "HEAD"),
        "commit": head,
        "subject": git("log", "-1", "--format=%s"),
        "uncommitted": [line[3:] for line in changed],
    }
    notes = [f"Desktop {version} (save version {desktop['save_version']}) on "
             f"{desktop['branch']} at {head[:7]}: {desktop['subject']}"]
    if changed:
        notes.append(f"  {len(changed)} file(s) changed and not committed: "
                     + ", ".join(desktop["uncommitted"][:6])
                     + (" …" if len(changed) > 6 else ""))

    checks = {}
    notes.append("")
    notes.append("Last checks:")
    for command in RECORDED:
        found = last(command)
        checks[command] = found
        if not found:
            notes.append(f"  {command:<14} not run yet here")
            continue
        failed = [s["name"] for s in found.get("steps", []) if s["status"] == "failed"]
        verdict = "passed" if found.get("success") else (
            f"failed: {', '.join(failed)}" if failed else f"failed: {found.get('error', '')}")
        warnings = len(found.get("warnings", []))
        local = ", with changes then not committed" if found.get("local_changes") else ""
        ran = [s["name"] for s in found.get("steps", [])]
        if command == "validate" and set(ran) != set(validate.DEFAULT) and ran:
            verdict += f" (ran {', '.join(ran)})"
        if command == "art check" and found.get("kind"):
            verdict += f" ({found['kind']} only)"
        notes.append(f"  {command:<14} {verdict}"
                     + (f", {warnings} warning(s)" if warnings else "")
                     + f" ({age(found.get('at', ''))}, {since(found.get('commit', ''), head)}"
                     + f"{local})")

    data, step = catalog.load()
    result.steps.append(step)
    counts = {kind["kind"]: kind["count"] for kind in data["kinds"]} if data else {}
    if counts:
        notes.append("")
        notes.append("Content: " + ", ".join(f"{count} {key}" for key, count in counts.items()))

    expansions = {}
    notes.append("")
    notes.append("Expansions:")
    for key in ("hill", "home"):
        expansion = find_expansion(key)
        if not expansion.path:
            notes.append(f"  {expansion.title:<14} no checkout beside this one")
            expansions[key] = None
            continue
        tags = sorted(set(expansion.pins().values()))
        entry = {
            "path": str(expansion.path),
            "branch": git("rev-parse", "--abbrev-ref", "HEAD", cwd=expansion.path),
            "commit": git("rev-parse", "HEAD", cwd=expansion.path),
            "builds_on": tags,
        }
        expansions[key] = entry
        on = ", ".join(tags) or "no tagged Desktop release"
        behind = "" if tags == [f"v{version}"] else f" (this Desktop is {version})"
        notes.append(f"  {expansion.title:<14} {entry['branch']} at {entry['commit'][:7]}, "
                     f"builds on Desktop {on}{behind}")

    prepared = [name for name in scenarios.SCENARIOS
                if (DEV / "scenarios" / name).is_dir()]
    notes.append("")
    notes.append(f"Scenarios: {len(scenarios.SCENARIOS)} ("
                 + (f"{len(prepared)} prepared: {', '.join(prepared)}" if prepared
                    else "none prepared yet")
                 + "); `formiga scenario list` names them")

    captures = newest_captures()
    if captures:
        notes.append("Newest captures:")
        for capture in captures:
            notes.append(f"  {capture['path']}  ({age(capture.get('made_at', ''))}, Desktop "
                         f"{capture.get('desktop_commit', 'unknown')})")
    else:
        notes.append("Captures: none yet; `formiga capture NAME` makes one")

    result.data.update({"desktop": desktop, "checks": checks, "content": counts,
                        "expansions": expansions, "scenarios": list(scenarios.SCENARIOS),
                        "prepared": prepared, "captures": captures, "notes": notes})
    return result


def newest_captures() -> List[Dict]:
    found = []
    for path in sorted(CAPTURES.glob("*.json")) if CAPTURES.is_dir() else []:
        try:
            meta = json.loads(path.read_text(encoding="utf-8"))
        except (OSError, ValueError):
            continue
        picture = path.with_suffix(".png")
        if picture.is_file():
            meta["path"] = relative(str(picture))
            found.append(meta)
    found.sort(key=lambda meta: meta.get("made_at", ""), reverse=True)
    return found[:CAPTURES_SHOWN]
