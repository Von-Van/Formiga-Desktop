"""`formiga inspect [KIND] [ITEM]`: everything about one item, without searching the code.

  formiga inspect                       every kind of thing, with how many there are
  formiga inspect decoration            every decoration
  formiga inspect "roof star"           one item, whatever kind it is
  formiga inspect trinket 17            one item of a kind, by id, name, saved name or number

For an item it shows its name and id, the line that declares it and the line that names it, the
name a save keeps and where, what draws it, how a player comes by it (and, for village pieces and
objects, when it turned up in colonies lived through day by day), whether its name fits where it
is shown, and every place in Desktop, Hill and Home that mentions it. Finds are mostly referred to
by number in tables, so their list of uses is the places that name them outright.
"""

from __future__ import annotations

import json
from typing import Dict, List

from . import catalog
from .report import Result

# How many uses the summary lists before saying how many more there are; --json lists them all.
USES_SHOWN = 12


def inspect(words: List[str]) -> Result:
    result = Result("inspect", verdict=False)
    data, step = catalog.load()
    result.steps.append(step)
    if data is None:
        return result
    if not words:
        return _kinds(result, data)
    named = catalog.lookup(data, words)
    if named.error:
        result.error = named.error
        if named.matches:
            result.data["matches"] = named.matches
            result.data["notes"] = matches(named.matches)
        return result
    if named.item is None:
        return _items(result, named.kind)
    return _item(result, data, named.kind, named.item)


def matches(found: List[Dict]) -> List[str]:
    return [f"  {m['kind']:<12} {m['id']:<16} {m['name']}" for m in found]


def _kinds(result: Result, data: dict) -> Result:
    result.data["kinds"] = [{"kind": k["kind"], "type": k["type"], "count": k["count"],
                             "comes_by": k["comes_by"]} for k in data["kinds"]]
    result.data["notes"] = [f"{k['kind']:<12} {k['count']:>4}  {k['type']}" for k in data["kinds"]]
    return result


def _items(result: Result, kind: dict) -> Result:
    result.data["kind"] = kind["kind"]
    result.data["items"] = [{"id": i["id"], "name": i["name"]} for i in kind["items"]]
    notes = [f"{kind['kind']}: {kind['count']} ({kind['type']})"]
    notes += [f"  {item['id']:<16} {item['name']}"
              + (f"  [{item['group']}]" if item.get("group") else "") for item in kind["items"]]
    result.data["notes"] = notes
    return result


def _item(result: Result, data: dict, kind: dict, item: dict) -> Result:
    source = catalog.Source()
    place = catalog.defined_at(source, kind, item)
    drawn = catalog.drawn_by(source, kind)
    hits = catalog.uses(source, kind, item)
    report: Dict = {
        "kind": kind["kind"],
        "type": kind["type"],
        "item": item,
        "declared": {"file": place["file"], "line": place["declared"]},
        "named": {"file": place["file"], "line": place["named"]},
        "saved": {"as": item.get("saved_as"), "in": kind["saved_in"]},
        "drawn_by": drawn,
        "comes_by": kind["comes_by"],
        "uses": [hit.to_json() for hit in hits],
        "repos_searched": sorted(source.roots),
    }
    result.data.update(report)
    result.data["notes"] = _summary(kind, item, place, drawn, hits, data, source)
    return result


def _summary(kind, item, place, drawn, hits, data, source) -> List[str]:
    lines = [f"{item['name']}  ({kind['kind']} {item['id']})"]

    def row(label: str, text: str) -> None:
        lines.append(f"  {label:<10} {text}")

    position = kind["items"].index(item) + 1
    row("kind", f"{kind['type']}, {position} of {kind['count']}"
        + (f"; {item['group']}" if item.get("group") else ""))
    if place["declared"]:
        row("declared", f"{place['file']}:{place['declared']}")
    if place["named"] and place["named"] != place["declared"]:
        row("named", f"{place['file']}:{place['named']}")
    if item.get("unnamed"):
        row("named", "never named on screen")
    for key, label in (("about", "about"), ("hint", "hint")):
        if item.get(key):
            row(label, item[key])
    row("saved as", f"{_quoted(item.get('saved_as'))} in {', '.join(kind['saved_in'])}")
    for entry in drawn:
        row("drawn by", f"{entry['function']}  {entry['file']}:{entry['line']}")
    row("comes by", kind["comes_by"])
    if item.get("starting"):
        row("", "One of the three every colony starts with.")
    reach = item.get("reach")
    if reach and not item.get("starting"):
        if reach["colonies"]:
            row("", f"Turned up in {reach['colonies']} of {reach['of']} colonies lived "
                    f"{data['reach']['days']} days, between day {reach['first_day']} and day "
                    f"{reach['last_day']}.")
        else:
            row("", f"Never turned up in {reach['of']} colonies lived {data['reach']['days']} "
                    "days.")
    if item.get("made_from"):
        made = item["made_from"]
        row("made from", f"find {made['find']}, {made['name']}")
    if item.get("made_into"):
        row("made into", item["made_into"])
    if item.get("bodies"):
        row("bodies", ", ".join(item["bodies"]))
    for shown in item.get("shown", []):
        for size in shown["sizes"]:
            fits = "fits" if size["fits"] else "does not fit"
            row("shown", f"{shown['where']} at {size['text_scale']}% text: {size['lines']} "
                         f"line(s), {fits}")
    counts = ", ".join(f"{sum(1 for hit in hits if hit.repo == repo)} in {repo}"
                       for repo in sorted(source.roots))
    row("used in", f"{len(hits)} place(s): {counts}" + (", tests last:" if hits else ""))
    for hit in hits[:USES_SHOWN]:
        lines.append(f"    {hit.repo}  {hit.where()}  {hit.text[:90]}")
    if len(hits) > USES_SHOWN:
        lines.append(f"    … and {len(hits) - USES_SHOWN} more (--json lists them all)")
    return lines


def _quoted(value) -> str:
    return json.dumps(value, ensure_ascii=False)
