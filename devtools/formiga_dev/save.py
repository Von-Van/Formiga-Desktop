"""`formiga save inspect FILE`: a colony file, read the way Desktop reads one.

Reads the file with Formiga's own loader (`formiga-tools dev save`) and says, step by step:

  read        whether it can be read at all, and if not, why
  version     the save version it names, and each upgrade that brings it to today's
  rules       every rule the file breaks before it is put right (errors)
  repair      each change Desktop's repair would make on loading it (warnings)
  after       anything still broken after the repair, which would be a bug in formiga-core
  snapshot    whether it would be accepted as a snapshot to restore from
  opens       what Desktop would open if this were its colony file: the file, the backup kept
              beside it, or a new colony

and then summarizes the colony: its companions, journal, unlocks, finds and trips.

It passes only when the file reads as it stands with nothing to put right, so it is also the
check for a save: `formiga save inspect colony.json` exits 0 or names what is wrong. Nothing is
ever written to the file.
"""

from __future__ import annotations

import re
from pathlib import Path
from typing import Dict, List

from . import cargo
from .report import Problem, Result, Step
from .workspace import DESKTOP

MIGRATIONS = "crates/formiga-core/src/persistence/migrations.rs"
# How many of the repair's changes the summary lists; --json lists up to the 200 Formiga gives.
REPAIRS_SHOWN = 12
_VERSION_NOTE = re.compile(r"^\s*// (\d+): (.*)$")


def upgrade_notes(text: str) -> Dict[int, str]:
    """What each save version added, from the comment above its step in the upgrade table."""
    notes: Dict[int, str] = {}
    current = None
    for line in text.splitlines():
        match = _VERSION_NOTE.match(line)
        if match:
            current = int(match.group(1))
            notes[current] = match.group(2).strip()
        elif current is not None and line.strip().startswith("//"):
            notes[current] += " " + line.strip()[2:].strip()
        else:
            current = None
    return notes


def inspect(file: str) -> Result:
    path = Path(file).expanduser().resolve()
    result = Result("save inspect", data={"file": str(path)})
    if not path.is_file():
        result.error = f"no file at {path}"
        return result
    document, done, problems = cargo.tools_dev(["save", str(path)], "read")
    if document is None:
        result.steps.append(Step("read", "failed", "could not run Formiga's loader", done.seconds,
                                 problems))
        return result
    result.data["save"] = document
    if not document["success"]:
        result.steps.append(Step("read", "failed", "Desktop cannot read this file", done.seconds,
                                 [Problem("read", document["error"], file=str(path))]))
        result.steps.append(_opens(document, path))
        return result
    result.steps.append(Step("read", "passed", f"{document['bytes']:,} bytes", done.seconds))
    result.steps.append(_version(document))
    result.steps.append(_rules(document))
    result.steps.append(_repair(document))
    result.steps.append(_after(document))
    result.steps.append(_snapshot(document))
    result.steps.append(_opens(document, path))
    upgrades = [f"  to {up['to']}: {up['adds']}" for up in document["version"].get("steps", [])]
    result.data["notes"] = (["Upgrades on reading it:"] + upgrades + [""] if upgrades else [])
    result.data["notes"] += summary(document["colony"])
    return result


def _version(document: dict) -> Step:
    version = document["version"]
    if not version["upgrades"]:
        return Step("version", "passed", f"version {version['current']}, today's")
    path = DESKTOP / MIGRATIONS
    notes = upgrade_notes(path.read_text(encoding="utf-8")) if path.is_file() else {}
    upgrades = [{"to": step + 1, "adds": notes.get(step + 1, "")} for step in version["upgrades"]]
    version["steps"] = upgrades
    return Step("version", "passed",
                f"version {version['named']}, brought forward to {version['current']} by "
                f"{len(upgrades)} upgrade(s)")


def _rules(document: dict) -> Step:
    broken = document["broken"]
    if not broken:
        return Step("rules", "passed", "breaks none of Formiga's rules")
    return Step("rules", "failed", f"breaks {len(broken)} rule(s)",
                problems=[Problem("rules", rule) for rule in broken])


def _repair(document: dict) -> Step:
    repairs = document["repairs"]
    if not repairs["count"]:
        return Step("repair", "passed", "nothing to put right")
    problems = [Problem("repair", f"{change['path'] or 'the file'}: {_short(change['was'])} → "
                                  f"{_short(change['now'])}", level="warning")
                for change in repairs["shown"]]
    return Step("repair", "passed", f"loading it would change {repairs['count']} value(s)",
                problems=problems)


def _after(document: dict) -> Step:
    left = document["after_repair"]
    if not left:
        return Step("after", "passed", "nothing left broken after the repair")
    return Step("after", "failed", "the repair leaves rules broken: a bug in formiga-core",
                problems=[Problem("after", rule, system="formiga-core",
                                  file="crates/formiga-core/src/persistence/validation.rs")
                          for rule in left])


def _snapshot(document: dict) -> Step:
    snapshot = document["snapshot"]
    if snapshot["accepted"]:
        return Step("snapshot", "passed", "would be accepted as a snapshot to restore from")
    return Step("snapshot", "passed", "would be refused as a snapshot to restore from",
                problems=[Problem("snapshot", snapshot["why"], level="warning")])


def _opens(document: dict, path: Path) -> Step:
    opens = document["opens"]
    which = opens["opens"]
    if which == "file":
        return Step("opens", "passed", "Desktop would open this file")
    if which == "backup":
        return Step("opens", "passed", "Desktop would open the backup beside it instead",
                    problems=[Problem("opens", "this file cannot be read, so the backup is used",
                                      file=opens["backup"], level="warning")])
    if which == "new colony":
        return Step("opens", "failed", "Desktop would start a new colony")
    return Step("opens", "failed", "Desktop would show its recovery screen",
                problems=[Problem("opens", opens.get("why", ""), file=str(path))])


def _short(value) -> str:
    if value is None:
        return "nothing"
    text = str(value) if not isinstance(value, str) else repr(value)
    return text if len(text) <= 60 else text[:57] + "…"


def summary(colony: dict) -> List[str]:
    lines = [f"Colony {colony['seed'][:12]}…, {colony['days']} days old "
             f"(begun {colony['created'][:10]}, last seen {colony['last_seen'][:16]})"]
    for creature in colony["creatures"]:
        wears = f", wears {creature['wears']}" if creature["wears"] else ""
        lines.append(f"  {creature['name']:<12} {creature['role']}, generation "
                     f"{creature['generation']}, {creature['body']}{wears}")
    journal = colony["journal"]
    moments = ", ".join(f"{m['moment']} {m['count']}" for m in journal["moments"][:6])
    lines.append(f"  journal    {journal['entries']} entries ({moments}), {journal['pinned']} "
                 "pinned")
    unlocked = colony["unlocked"]
    counts = ", ".join(f"{len(unlocked[key])} {key}"
                       for key in ("decorations", "hangouts", "gardens", "ornaments"))
    lines.append(f"  unlocked   {counts}; next {unlocked['next'][:16]}")
    lines.append(f"  objects    {', '.join(colony['objects']) or 'none'}")
    lines.append(f"  finds      {len(colony['finds'])}; wonders {len(colony['wonders'])}; "
                 f"{colony['relationships']} relationships")
    souvenirs = f": {', '.join(colony['souvenirs'])}" if colony["souvenirs"] else ""
    lines.append(f"  trips      {colony['trips']}, {len(colony['souvenirs'])} souvenirs"
                 f"{souvenirs}")
    return lines
