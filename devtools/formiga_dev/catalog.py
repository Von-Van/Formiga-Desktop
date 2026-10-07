"""Formiga's content, as its own code lists it, and where each item lives in the source.

The list comes from `formiga-tools dev catalog`, which reads Formiga's own tables: every kind of
thing a player meets (decorations, finds, accessories, souvenirs, bodies…), each item's name, the
identifier a save keeps, how it is come by, and whether its name fits where it is shown. Nothing
here re-reads the game's rules from the source.

What the source is used for is finding things: the line an item is defined on, the line that
names it, the function that draws it, and every place in Desktop, Hill and Home that mentions it.
"""

from __future__ import annotations

import re
from dataclasses import dataclass
from pathlib import Path
from typing import Dict, Iterator, List, Optional, Tuple

from . import cargo
from .report import Step
from .workspace import DESKTOP, find_expansion, relative

# Folders under a repository that are never searched: builds, the toolkit's own output, git.
SKIPPED = {"target", ".dev", ".git", "node_modules", "__pycache__"}
SEARCHED = {".rs", ".toml", ".json", ".ron"}


def load() -> Tuple[Optional[dict], Step]:
    """The catalog, and the step that fetched it."""
    document, done, problems = cargo.tools_dev(["catalog"], "catalog")
    if document is None:
        return None, Step("catalog", "failed", "could not list Formiga's content", done.seconds,
                          problems)
    count = sum(kind["count"] for kind in document["kinds"])
    return document, Step("catalog", "passed",
                          f"{count} items in {len(document['kinds'])} kinds", done.seconds)


def normal(text: str) -> str:
    """How names are compared when looking one up: case, spaces, `-` and `_` ignored."""
    return re.sub(r"[\s_\-]+", "", str(text)).lower()


def snake(variant: str) -> str:
    return re.sub(r"(?<!^)(?=[A-Z])", "_", variant).lower()


def variant_of(kind: dict, item: dict) -> Optional[str]:
    """The Rust variant an item is, where it is one."""
    if kind["kind"] == "trinket":
        return None
    return item.get("variant", item["id"])


def matches(item: dict, query: str) -> bool:
    wanted = normal(query)
    names = [item["id"], item["name"], item.get("variant", ""), str(item.get("saved_as", ""))]
    return any(normal(name) == wanted for name in names if name != "")


def find(catalog: dict, query: str, kind: Optional[str] = None) -> List[Tuple[dict, dict]]:
    """Every (kind, item) the query names: an id, a name, a saved name or a number."""
    found = []
    for entry in catalog["kinds"]:
        if kind and entry["kind"] != kind:
            continue
        for item in entry["items"]:
            if matches(item, query) or (kind and str(item.get("number")) == query):
                found.append((entry, item))
    return found


def kind_named(catalog: dict, name: str) -> Optional[dict]:
    wanted = normal(name)
    for entry in catalog["kinds"]:
        if normal(entry["kind"]) in (wanted, wanted.rstrip("s")):
            return entry
    return None


@dataclass
class Hit:
    repo: str
    path: str
    line: int
    text: str

    def where(self) -> str:
        return f"{self.path}:{self.line}"

    def to_json(self) -> Dict:
        return {"repo": self.repo, "file": self.path, "line": self.line, "text": self.text}


class Source:
    """The text of Desktop, Hill and Home (whichever are here), read once."""

    def __init__(self) -> None:
        self.roots: Dict[str, Path] = {"desktop": DESKTOP}
        for key in ("hill", "home"):
            path = find_expansion(key).path
            if path:
                self.roots[key] = path
        self._files: Dict[Tuple[str, str], List[str]] = {}
        self._loaded = False

    def _load(self) -> Dict[Tuple[str, str], List[str]]:
        if not self._loaded:
            self._loaded = True
            for repo, root in self.roots.items():
                for path in _walk(root):
                    try:
                        text = path.read_text(encoding="utf-8")
                    except (OSError, UnicodeDecodeError):
                        continue
                    self._files[(repo, relative(str(path), root))] = text.splitlines()
        return self._files

    def files(self) -> Iterator[Tuple[str, str, List[str]]]:
        for (repo, path), lines in self._load().items():
            yield repo, path, lines

    def lines(self, repo: str, path: str) -> List[str]:
        return self._load().get((repo, path), [])

    def grep(self, pattern: "re.Pattern[str]", suffixes: Optional[set] = None) -> List[Hit]:
        hits = []
        for repo, path, lines in self.files():
            if suffixes and Path(path).suffix not in suffixes:
                continue
            for number, text in enumerate(lines, 1):
                if pattern.search(text):
                    hits.append(Hit(repo, path, number, text.strip()))
        return hits

    def enum_variant(self, path: str, type_name: str, variant: str) -> Optional[int]:
        """The line `variant` is declared on, inside `pub enum type_name`."""
        lines = self.lines("desktop", path)
        start = next((i for i, text in enumerate(lines)
                      if re.match(rf"\s*pub enum {re.escape(type_name)}\b", text)), None)
        if start is None:
            return None
        for number in range(start + 1, len(lines)):
            text = lines[number]
            if text.startswith("}"):
                return None
            if re.match(rf"\s*{re.escape(variant)}\b\s*[,({{]?", text):
                return number + 1
        return None

    def first_line(self, path: str, needle: str, after: Optional[str] = None) -> Optional[int]:
        lines = self.lines("desktop", path)
        begun = after is None
        for number, text in enumerate(lines, 1):
            if not begun:
                begun = after in text
                continue
            if needle in text:
                return number
        return None

    def function(self, owner: Optional[str], name: str) -> Optional[Hit]:
        """Where `fn name` is defined in Desktop's crates, inside `impl owner` if one is given."""
        for repo, path, lines in self.files():
            if repo != "desktop" or not path.endswith(".rs") or "/tests" in path:
                continue
            inside = owner is None
            for number, text in enumerate(lines, 1):
                if owner and re.match(rf"\s*impl(<[^>]*>)?\s+{re.escape(owner)}\b", text):
                    inside = True
                elif owner and inside and text.startswith("}"):
                    inside = False
                if inside and re.match(rf"\s*pub(\([a-z]+\))?\s+(const\s+)?fn {re.escape(name)}\b",
                                       text):
                    return Hit(repo, path, number, text.strip())
        return None


def _walk(root: Path) -> Iterator[Path]:
    for path in sorted(root.iterdir()):
        if path.name in SKIPPED or path.name.startswith("."):
            continue
        if path.is_dir():
            yield from _walk(path)
        elif path.suffix in SEARCHED:
            yield path


_FUNCTION = re.compile(r"\b([A-Z][A-Za-z0-9]+)::([a-z_][a-z0-9_]*)\b|\b(draw_[a-z_]+)\b")


def drawn_by(source: Source, kind: dict) -> List[Dict]:
    """The art functions a kind's `drawn_by` names, each with where it is defined."""
    found = []
    for match in _FUNCTION.finditer(kind["drawn_by"]):
        owner, name = (match.group(1), match.group(2)) if match.group(2) else (None, match.group(3))
        hit = source.function(owner, name)
        label = f"{owner}::{name}" if owner else name
        if hit and not any(entry["function"] == label for entry in found):
            found.append({"function": label, "file": hit.path, "line": hit.line})
    return found


def defined_at(source: Source, kind: dict, item: dict) -> Dict[str, Optional[int]]:
    """The line an item is declared on, and the line that gives it its name."""
    path = kind["defined_in"]
    variant = variant_of(kind, item)
    type_name = kind["type"].split()[0]
    declared = source.enum_variant(path, type_name, variant) if variant else None
    named = None
    if kind["kind"] == "trinket":
        named = source.first_line(path, f'"{item["name"]}"', after="static TRINKETS")
        declared = named
    elif not item.get("unnamed"):
        named = (source.first_line(path, f'=> "{item["name"]}"')
                 or source.first_line(path, f'"{item["name"]}"'))
    return {"file": path, "declared": declared, "named": named}


def uses(source: Source, kind: dict, item: dict) -> List[Hit]:
    """Every place in Desktop, Hill and Home that mentions the item by its Rust name, by the name
    a save or a trip file keeps, or (for finds) by its number where a find is expected."""
    patterns = []
    variant = variant_of(kind, item)
    type_name = kind["type"].split()[0]
    if variant:
        patterns.append(rf"\b{re.escape(type_name)}::{re.escape(variant)}\b")
        # Mirrors of Desktop's types in the trip and Home contracts, and Hill's and Home's own
        # copies, spell it in snake case.
        patterns.append(rf'"{re.escape(snake(variant))}"')
    saved = item.get("saved_as")
    if isinstance(saved, str) and saved != variant:
        patterns.append(rf'"{re.escape(saved)}"')
    if kind["kind"] == "souvenir":
        patterns.append(rf'"souvenir\.{re.escape(item["id"])}"')
    if kind["kind"] == "trinket":
        number = item["number"]
        patterns += [rf"\bPin\({number}\)", rf"\btrinket_info\({number}\)",
                     rf'"find\.{number}"', rf"\bvariant: {number}\b"]
    pattern = re.compile("|".join(patterns))
    declared = defined_at(source, kind, item)
    hits = []
    for hit in source.grep(pattern):
        # The declaration itself, and the catalogue's own name and number tables, are not uses.
        if hit.repo == "desktop" and hit.path == kind["defined_in"] and (
                hit.line in (declared["declared"], declared["named"])
                or re.match(rf"Self::{re.escape(variant or '')}\b", hit.text)):
            continue
        hits.append(hit)
    order = {"hill": 0, "home": 1, "desktop": 2}
    return sorted(hits, key=lambda hit: (_is_test(hit.path), order.get(hit.repo, 3), hit.path,
                                         hit.line))


def _is_test(path: str) -> bool:
    return "test" in path or "review" in path or path.endswith(".json")
