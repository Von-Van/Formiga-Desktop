"""`formiga audit content`: is every item named well, reachable, and known to Hill and Home?

Reads Formiga's content from its own code (`formiga-tools dev catalog`) and checks:

  names       every name is unique within its kind and tidy; a village piece's name fits its
              tile on the Village page, measured in egui's own font at the app's text sizes
              (an error at normal size, a warning at the largest); a find's or souvenir's name
              is short enough for Formiga Home to show whole
  arrival     every village piece arrives in colonies lived through day by day, and every kind
              of colony object turns up somewhere; every accessory can be worn once, and only
              once, its find is found; every find and wonder has a hint
  expansions  every souvenir Desktop knows is one Hill gives, and every story souvenir has a
              story that gives it; every souvenir and find Hill and Home name by id or number is
              one Desktop has

Errors fail the audit; warnings are listed for a look and do not.
"""

from __future__ import annotations

import re
from typing import Dict, List, Optional

from . import catalog, cargo
from .report import Problem, Result, Step
from .workspace import DEV, relative

ART_DOC = """`formiga art check`: draw every item in every pose, and look it over.

Every kind is drawn through Formiga's own renderers, each item into its own frame: colony
objects and village pieces, every decoration on every type of house by day and lit, every house
type and palette, every find resting and glinting, every souvenir, every frame of every wonder,
every accessory on six bodies in every clip facing both ways, every kind of body the generator
makes in every frame of every clip, and every face. Each drawing is checked for:

  blank       nothing drawn (an error); an accessory or decoration that never shows (an error)
  edge        art reaching the edge of its frame, where anything further is cut off (a warning)
  look-alike  two items of a kind drawn the same (an error) or nearly the same (a warning)

A sheet of each kind is written to .dev/art/KIND.png with anything flagged outlined, red for an
error and amber for a warning. It is drawn in a release build: a minute the first time, then
seconds.
"""

ART_DIR = DEV / "art"


def _problem(step: str, message: str, kind: str, item: dict, warning: bool = False,
             **extra) -> Problem:
    return Problem(step, message, system=kind, object=f"{kind}:{item['id']}",
                   level="warning" if warning else None, **extra)


def _step(name: str, problems: List[Problem], checked: str) -> Step:
    errors = [p for p in problems if not p.is_warning]
    warnings = len(problems) - len(errors)
    if errors:
        summary = f"{len(errors)} error(s)" + (f", {warnings} warning(s)" if warnings else "")
    else:
        summary = checked + (f"; {warnings} warning(s)" if warnings else "")
    return Step(name, "failed" if errors else "passed", summary, problems=problems)


def audit_content() -> Result:
    result = Result("audit content")
    data, step = catalog.load()
    result.steps.append(step)
    if data is None:
        return result
    source = catalog.Source()
    result.steps.append(names(data, source))
    result.steps.append(arrival(data))
    result.steps.append(expansions(data, source))
    result.data["counts"] = {kind["kind"]: kind["count"] for kind in data["kinds"]}
    return result


def _named_at(source: catalog.Source, kind: dict, item: dict) -> Dict:
    place = catalog.defined_at(source, kind, item)
    line = place["named"] or place["declared"]
    return {"file": place["file"], "line": line} if line else {"file": place["file"]}


def names(data: dict, source: catalog.Source) -> Step:
    problems: List[Problem] = []
    letters = {kind: shown["letters"] for shown in data["names_shown"] if "letters" in shown
               for kind in shown["kinds"]}
    for kind in data["kinds"]:
        seen: Dict[str, dict] = {}
        for item in kind["items"]:
            if item.get("unnamed"):
                continue
            name = item["name"]
            at = _named_at(source, kind, item)
            key = catalog.normal(name)
            if key in seen:
                problems.append(_problem(
                    "names", f"{name!r} is also the name of {seen[key]['id']}", kind["kind"],
                    item, **at))
            seen.setdefault(key, item)
            if not name.strip() or name != name.strip() or "  " in name:
                problems.append(_problem(
                    "names", f"{name!r} has stray spaces", kind["kind"], item, **at))
            limit = letters.get(kind["kind"])
            if limit and len(name) > limit:
                problems.append(_problem(
                    "names", f"{name!r} is {len(name)} letters; Formiga Home cuts it to {limit}",
                    kind["kind"], item, **at))
            for shown in item.get("shown", []):
                for size in shown["sizes"]:
                    if size["fits"]:
                        continue
                    if size["word_broken"]:
                        why = "a word in it is wider than the tile and is broken partway"
                    else:
                        why = (f"it takes {size['lines']} lines, {size['height']:g} high, where "
                               f"there is room for {size['room']:g}, and runs into the picture "
                               "above it")
                    problems.append(_problem(
                        "names", f"{name!r} does not fit its {shown['where']} at "
                                 f"{size['text_scale']}% text: {why}",
                        kind["kind"], item, warning=size["text_scale"] > 100, **at))
    named = sum(1 for kind in data["kinds"] for item in kind["items"] if not item.get("unnamed"))
    return _step("names", problems, f"{named} names unique, tidy and fitting")


def arrival(data: dict) -> Step:
    problems: List[Problem] = []
    days = data["reach"]["days"]
    for kind in data["kinds"]:
        key = kind["kind"]
        for item in kind["items"]:
            reach = item.get("reach")
            if reach and not item.get("starting"):
                if reach["colonies"] == 0:
                    problems.append(_problem(
                        "arrival", f"never arrived in {reach['of']} colonies lived {days} days",
                        key, item))
                elif reach["colonies"] < reach["of"] and key != "object":
                    problems.append(_problem(
                        "arrival", f"arrived in only {reach['colonies']} of {reach['of']} "
                                   f"colonies lived {days} days", key, item, warning=True))
            rule = item.get("rule")
            if rule and (rule["worn_before_its_find"] or not rule["worn_once_found"]):
                problems.append(_problem(
                    "arrival", "can be worn before its find is found" if
                    rule["worn_before_its_find"] else "cannot be worn even once its find is found",
                    key, item))
            made = item.get("made_from")
            if made and not made.get("name"):
                problems.append(_problem(
                    "arrival", f"is made from find {made['find']}, which does not exist", key,
                    item))
            if key in ("trinket", "wonder") and not item.get("hint", "").strip():
                problems.append(_problem("arrival", "has no hint", key, item))
        if key == "object":
            reaches = [item["reach"]["colonies"] for item in kind["items"]]
            colonies = kind["items"][0]["reach"]["of"]
            each = sum(reaches) / colonies if colonies else 0
            if each < kind["count"]:
                problems.append(Problem(
                    "arrival", f"each colony lived {days} days got only {each:g} of the "
                               f"{kind['count']} kinds of colony object: a colony keeps "
                               f"{each:g} and never gives one up, so the rest never come",
                    system=key, object=f"{key}:*", level="warning"))
    return _step("arrival", problems, "everything can be come by")


_CATALOGUE_ENTRY = re.compile(
    r'\(\s*("[a-z0-9_]+"|[A-Z][A-Z0-9_]*)\s*,\s*"[^"]*"\s*,\s*Giver::(Story|Game)\s*,?\s*\)',
    re.DOTALL)
_CONSTANT = re.compile(r'pub const ([A-Z][A-Z0-9_]*): &str = "([a-z0-9_]+)";')
_PACKAGE_SOUVENIR = re.compile(r'^\s*souvenir\s*=\s*"([^"]*)"')
_SOUVENIR_ID = re.compile(
    r'DisplayId::souvenir\("([^"]+)"\)|"souvenir\.([^"]+)"|Souvenir::from_id\("([^"]+)"\)'
    r'|HillSouvenir\s*\{\s*id\s*\}\s*=>\s*id\s*==\s*"([^"]+)"')
_FIND_NUMBER = re.compile(r'"find\.(\d+)"|\bPin\((\d+)\)|DesktopFind\s*\{\s*variant:\s*(\d+)')


def hill_givers(source: catalog.Source) -> Optional[Dict[str, Dict]]:
    """Hill's souvenir catalogue: each id, who gives it, and the packages that name it."""
    if "hill" not in source.roots:
        return None
    constants: Dict[str, str] = {}
    givers: Dict[str, Dict] = {}
    for repo, path, lines in source.files():
        if repo == "hill" and path.endswith("story/souvenirs.rs"):
            text = "\n".join(lines)
            constants.update(dict(_CONSTANT.findall(text)))
            for name, giver in _CATALOGUE_ENTRY.findall(text):
                givers[name] = {"giver": giver, "file": path, "packages": []}
    resolved = {}
    for name, entry in givers.items():
        resolved[name.strip('"') if name.startswith('"') else constants.get(name, name)] = entry
    for hit in source.grep(_PACKAGE_SOUVENIR, {".toml"}):
        if hit.repo != "hill":
            continue
        souvenir = _PACKAGE_SOUVENIR.match(hit.text).group(1)
        resolved.setdefault(souvenir, {"giver": None, "file": None, "packages": []})
        resolved[souvenir]["packages"].append(hit)
    return resolved


def expansions(data: dict, source: catalog.Source) -> Step:
    problems: List[Problem] = []
    kinds = {kind["kind"]: kind for kind in data["kinds"]}
    souvenirs = {item["id"]: item for item in kinds["souvenir"]["items"]}
    finds = kinds["trinket"]["count"]
    missing = [key for key in ("hill", "home") if key not in source.roots]
    givers = hill_givers(source)
    if givers is not None:
        for souvenir_id, item in souvenirs.items():
            entry = givers.get(souvenir_id)
            if not entry or not entry["giver"]:
                problems.append(_problem(
                    "expansions", "Formiga Hill's souvenir catalogue does not have it, so no "
                                  "trip can bring it back", "souvenir", item))
            elif entry["giver"] == "Story" and not entry["packages"]:
                problems.append(_problem(
                    "expansions", "Hill lists it as given by a story, but no story Hill ships "
                                  "gives it yet", "souvenir", item, warning=True,
                    file=f"Formiga-Hill/{entry['file']}"))
        for souvenir_id, entry in givers.items():
            for hit in entry["packages"]:
                if souvenir_id not in souvenirs:
                    problems.append(Problem(
                        "expansions", f"a Hill story gives {souvenir_id!r}, which Desktop has "
                                      "no souvenir called", system="hill",
                        file=f"Formiga-Hill/{hit.path}", line=hit.line,
                        object=f"souvenir:{souvenir_id}"))
    # Matched in each file's whole text: rustfmt spreads a `{ id }` or `{ variant: N }` over
    # several lines.
    for hit, match in source.search(_SOUVENIR_ID, {".rs", ".toml", ".json"}):
        if hit.repo == "desktop":
            continue
        souvenir_id = next(group for group in match.groups() if group)
        if souvenir_id not in souvenirs:
            problems.append(Problem(
                "expansions", f"names souvenir {souvenir_id!r}, which Desktop does not have",
                system=hit.repo, file=f"Formiga-{hit.repo.title()}/{hit.path}",
                line=hit.line, object=f"souvenir:{souvenir_id}"))
    for hit, match in source.search(_FIND_NUMBER, {".rs", ".toml", ".json"}):
        if hit.repo == "desktop":
            continue
        number = int(next(group for group in match.groups() if group))
        if number >= finds:
            problems.append(Problem(
                "expansions", f"names find {number}; Desktop's finds go up to {finds - 1}",
                system=hit.repo, file=f"Formiga-{hit.repo.title()}/{hit.path}",
                line=hit.line, object=f"trinket:{number}"))
    present = sorted(key for key in ("hill", "home") if key in source.roots)
    checked = f"souvenirs and finds agree with {', '.join(present) or 'nothing'}"
    step = _step("expansions", problems, checked)
    if missing and len(missing) == 2:
        step.status = "skipped"
        step.summary = "no Hill or Home checkout beside this one"
    elif missing:
        step.summary += f" (no {missing[0].title()} checkout beside this one)"
    return step


def art_check(kind: Optional[str] = None) -> Result:
    result = Result("art check", data={"kind": kind} if kind else {})
    args = ["art-check", "--out", str(ART_DIR)] + (["--kind", kind] if kind else [])
    document, done, problems = cargo.tools_dev(args, "art", release=True)
    if document is None:
        result.steps.append(Step("draw", "failed", "could not draw Formiga's art", done.seconds,
                                 problems))
        return result
    sheets = []
    for report in document["kinds"]:
        found = [
            Problem("art", p["message"] + (f" ({p['pose']})" if p.get("pose") else ""),
                    system=report["kind"], object=f"{report['kind']}:{p['item']}",
                    level="warning" if p["level"] == "warning" else None)
            for p in report["problems"]
        ]
        width, height = report["frame"]
        drew = (f"{report['items']} items, {report['drawings']} drawings at "
                f"{width}×{height}")
        step = _step(report["kind"], found, drew)
        result.steps.append(step)
        sheets.append(relative(report["sheet"]))
    result.data["sheets"] = sheets
    result.data["seconds"] = round(done.seconds, 1)
    result.data["notes"] = [f"Sheets: {relative(str(ART_DIR))}/ ({len(sheets)} kinds)"]
    return result


