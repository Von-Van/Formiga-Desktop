"""`formiga uses [KIND] ITEM`: can I rename or remove this, and what would it take?

Finds every place Desktop, Hill and Home use an item (as `formiga inspect` does), sorts them by
what they mean for a change, and answers three questions:

  name        changing the name players see
  rename      renaming it in the code, which saves and the expansions may spell
  remove      removing it, which colonies that hold it, the expansions and the code all feel

The places are grouped as: saves (where a colony file keeps it, and data files holding it), the
expansions (Hill and Home), the trip and Home contracts, the save upgrades, Desktop's code, the
review tools, tests, and other places that spell its name out.

Nothing is changed: it only reads.
"""

from __future__ import annotations

import re
from typing import Dict, List

from . import catalog
from .catalog import Hit
from .inspect import matches
from .report import Result
from .workspace import find_expansion

# The groups, in the order they are shown, and what each holds.
GROUPS = {
    "data": "data files holding it (saves, fixtures, story packages)",
    "expansions": "Hill and Home",
    "contracts": "the trip and Home contracts in Desktop",
    "upgrades": "Desktop's save upgrades",
    "code": "Desktop's code",
    "tools": "the review and developer tools",
    "tests": "tests",
}
CONTRACT_CRATES = ("crates/formiga-travel/", "crates/formiga-home-contract/",
                   "crates/formiga-expansion-rulebook/")
MIGRATIONS = "crates/formiga-core/src/persistence/migrations.rs"
# How many places of each group the summary lists; --json lists them all.
SHOWN = 6


def group_of(hit: Hit) -> str:
    if hit.path.endswith((".json", ".toml", ".ron")):
        return "data"
    if hit.repo != "desktop":
        return "tests" if catalog._is_test(hit.path) else "expansions"
    if catalog._is_test(hit.path):
        return "tests"
    if hit.path.startswith(CONTRACT_CRATES):
        return "contracts"
    if hit.path == MIGRATIONS:
        return "upgrades"
    if hit.path.startswith(("crates/formiga-tools/", "devtools/")):
        return "tools"
    return "code"


def named_elsewhere(source: catalog.Source, kind: dict, item: dict, seen: List[Hit]) -> List[Hit]:
    """Places outside its own definition that spell out the name players see."""
    if item.get("unnamed"):
        return []
    place = catalog.defined_at(source, kind, item)
    pattern = re.compile(rf'"{re.escape(item["name"])}"')
    known = {(hit.repo, hit.path, hit.line) for hit in seen}
    return [hit for hit in source.grep(pattern)
            if (hit.repo, hit.path, hit.line) not in known
            and not (hit.repo == "desktop" and hit.path == kind["defined_in"]
                     and hit.line in (place["named"], place["declared"]))]


def uses(words: List[str]) -> Result:
    result = Result("uses", verdict=False)
    data, step = catalog.load()
    result.steps.append(step)
    if data is None:
        return result
    named = catalog.lookup(data, words, command="uses")
    if named.error or named.item is None:
        result.error = named.error or "name an item, for example `formiga uses hangout bench`"
        if named.matches:
            result.data["matches"] = named.matches
            result.data["notes"] = matches(named.matches)
        return result
    kind, item = named.kind, named.item
    source = catalog.Source()
    hits = catalog.uses(source, kind, item)
    groups: Dict[str, List[Hit]] = {key: [] for key in GROUPS}
    for hit in hits:
        groups[group_of(hit)].append(hit)
    spelled = named_elsewhere(source, kind, item, hits)
    place = catalog.defined_at(source, kind, item)
    pins = {key: find_expansion(key).pins() for key in ("hill", "home") if key in source.roots}
    answers = answer(kind, item, place, groups, spelled, pins)
    result.data.update({
        "kind": kind["kind"],
        "item": {"id": item["id"], "name": item["name"], "saved_as": item.get("saved_as")},
        "saved_in": kind["saved_in"],
        "groups": {key: [hit.to_json() for hit in found] for key, found in groups.items()},
        "name_spelled_out": [hit.to_json() for hit in spelled],
        "answers": answers,
        "pins": {key: sorted(set(found.values())) for key, found in pins.items()},
    })
    result.data["notes"] = summary(kind, item, groups, spelled, answers)
    return result


def answer(kind: dict, item: dict, place: dict, groups: Dict[str, List[Hit]],
           spelled: List[Hit], pins: Dict[str, Dict[str, str]]) -> Dict[str, str]:
    saved = item.get("saved_as")
    kept = bool(kind["saved_in"])
    variant = catalog.variant_of(kind, item)
    where = f"{place['file']}:{place['named'] or place['declared']}"
    used = {key: len(found) for key, found in groups.items()}
    outside = used["expansions"] + used["contracts"]
    tied = ", ".join(f"{key.title()} builds on Desktop {', '.join(sorted(set(found.values())))}"
                     for key, found in pins.items() if found)

    if item.get("unnamed"):
        name = "It has no name on screen."
    else:
        name = f"Safe: one line, {where}."
        if spelled:
            name += (f" {len(spelled)} other place(s) spell the name out too, mostly tests "
                     "and review sheets; they are listed under 'name'.")
        name += " `formiga audit content` then checks the new name fits where it is shown."

    if kind["kind"] == "trinket":
        rename = ("Finds are kept by number, not by name, so saves are unaffected. Renumbering "
                  "one would change it everywhere a number names it.")
    elif kept and isinstance(saved, str) and saved not in (variant, catalog.snake(variant or "")):
        rename = (f"Colony files keep it as \"{saved}\" (in {', '.join(kind['saved_in'])}), "
                  "which its #[serde(rename)] already holds steady: keep that line through any "
                  "rename.")
    elif kept and isinstance(saved, str) and saved != variant:
        rename = (f"Colony files keep it as \"{saved}\" (in {', '.join(kind['saved_in'])}), "
                  "spelled from its Rust name. Give the renamed variant "
                  f"#[serde(rename = \"{saved}\")] to keep that spelling; without it, every "
                  "colony holding one stops loading.")
    elif kept and isinstance(saved, str):
        rename = (f"Colony files keep it as \"{saved}\" (in {', '.join(kind['saved_in'])}). Give "
                  f"the renamed variant #[serde(rename = \"{saved}\")], as WonderKind::LeafSled "
                  "keeps \"Bike\"; without it, every colony holding one stops loading.")
    elif kept:
        rename = (f"Colony files keep it as {saved!r} (in {', '.join(kind['saved_in'])}), "
                  "which a rename leaves alone.")
    else:
        rename = "Colony files do not keep it, so only the code changes."
    code = used["code"] + used["tools"] + used["tests"] + used["upgrades"]
    rename += f" {code} place(s) in Desktop's code, tools and tests name it."
    if outside:
        rename += (f" {outside} place(s) in the contracts, Hill or Home spell it too: change "
                   "them in step, or a trip or visit carrying it is refused.")

    steps = []
    if kind["kind"] == "trinket":
        steps.append("finds are numbered by their place in the table and colony files keep "
                     "them by number, so every later find would be renumbered, in saves, Hill "
                     "and Home alike")
    elif kept:
        steps.append("colonies holding it would stop loading: it needs a new save version whose "
                     f"upgrade drops or replaces it ({MIGRATIONS})")
    if item.get("starting"):
        steps.append("it is one of the three every new colony starts with, so another must take "
                     "its place")
    if code:
        steps.append(f"{code} place(s) in Desktop name it")
    if outside:
        steps.append(f"{outside} place(s) in the contracts, Hill or Home name it"
                     + (f" ({tied}; they keep working there, and break when moved to a Desktop "
                        "without it)" if tied else ""))
    if used["data"]:
        steps.append(f"{used['data']} data file(s) hold it")
    remove = "; ".join(steps) + "." if steps else "Nothing else uses it."
    remove = remove[0].upper() + remove[1:]
    return {"name": name, "rename": rename, "remove": remove}


def summary(kind: dict, item: dict, groups: Dict[str, List[Hit]], spelled: List[Hit],
            answers: Dict[str, str]) -> List[str]:
    lines = [f"{item['name']}  ({kind['kind']} {item['id']})"]
    if kind["saved_in"]:
        lines.append(f"  Saved as {item.get('saved_as')!r} in {', '.join(kind['saved_in'])}.")
    else:
        lines.append("  Not kept in colony files.")
    lines.append("")
    for key, label in (("name", "Change the name"), ("rename", "Rename in code"),
                       ("remove", "Remove")):
        lines.append(f"{label}: {answers[key]}")
    lines.append("")
    for key, found in list(groups.items()) + [("name", spelled)]:
        if not found:
            continue
        title = GROUPS.get(key, "other places spelling its name out")
        lines.append(f"{title}: {len(found)}")
        for hit in found[:SHOWN]:
            lines.append(f"    {hit.repo}  {hit.where()}  {hit.text[:80]}")
        if len(found) > SHOWN:
            lines.append(f"    … and {len(found) - SHOWN} more (--json lists them all)")
    return lines
