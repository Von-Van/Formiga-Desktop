"""`formiga content add KIND NAME --about TEXT --like ITEM [--write]`: add a village piece.

Does the fiddly steps of adding a hangout, garden or ornament, the way Formiga's own pieces are
added, and shows them as a diff before anything is changed:

  declare     the new variant at the end of its enum, and in its `ALL` list
  name        its name and its one-line description, beside the others
  like        everywhere the `--like` piece is matched on (how it behaves, how it is drawn), the
              new piece is matched too, so it starts out working exactly like it
  save        a new save version with an upgrade step that adds nothing, as every new saved
              thing gets, unless the save version has already been raised since the last release
              (then the new piece is noted on that version)

Before any change it checks the name: not already used in its kind, tidy, and fitting its tile
on the Village page (measured as the app draws it). Without `--write` that is all it does.

With `--write`, the files are changed only if none of them has uncommitted changes, so
`git diff` shows exactly what was done and `git checkout` undoes it. Then it formats, compiles,
and runs the content audit and the art check for the kind. The new piece is drawn like the
`--like` piece until it has a drawing of its own, so the art check names it as a look-alike:
that, and the behaviour copied from `--like`, are the to-do it ends with. Nothing is deleted.
"""

from __future__ import annotations

import difflib
import re
import subprocess
from dataclasses import dataclass, field
from typing import Dict, List, Optional, Tuple

from . import audit, cargo, catalog
from .report import Problem, Result, Step
from .workspace import APP_CRATE, DESKTOP, app_builds_here, desktop_version, run

# The kinds it can add, and the habitat's ranking that keeps each kind to so many.
SUPPORTED = ("hangout", "garden", "ornament")
LIB = "crates/formiga-core/src/lib.rs"
MIGRATIONS = "crates/formiga-core/src/persistence/migrations.rs"
HABITAT = "crates/formiga-core/src/habitat.rs"
# Where matches on the `--like` piece are looked for: Desktop's crates, the app and tests
# included, since an exhaustive match anywhere has to name the new piece.
SEARCHED = "crates"


class Refused(Exception):
    """Why the piece cannot be added as asked."""


@dataclass
class Plan:
    kind: str
    type_name: str
    variant: str
    name: str
    about: str
    like: str
    files: Dict[str, Tuple[str, str]] = field(default_factory=dict)  # path: (before, after)
    copied: List[Dict] = field(default_factory=list)  # each match on --like now naming it too
    unchanged: List[Dict] = field(default_factory=list)  # uses of --like left alone
    notes: List[str] = field(default_factory=list)
    save_version: Optional[int] = None  # the new one, when raised

    def text(self, path: str) -> str:
        if path not in self.files:
            before = (DESKTOP / path).read_text(encoding="utf-8")
            self.files[path] = (before, before)
        return self.files[path][1]

    def change(self, path: str, after: str) -> None:
        before = self.files[path][0] if path in self.files else after
        self.files[path] = (before, after)

    def diff(self) -> str:
        out = []
        for path, (before, after) in self.files.items():
            out += difflib.unified_diff(before.splitlines(True), after.splitlines(True),
                                        f"a/{path}", f"b/{path}", n=1)
        return "".join(out)


def camel(name: str) -> str:
    return "".join(word[:1].upper() + word[1:] for word in re.findall(r"[A-Za-z0-9]+", name))


def rust_string(text: str) -> str:
    return '"' + text.replace("\\", "\\\\").replace('"', '\\"') + '"'


def block(lines: List[str], start: int) -> int:
    """The index of the line closing the item that opens on `start`: the next line at the same
    indentation that closes a bracket."""
    indent = len(lines[start]) - len(lines[start].lstrip())
    for index in range(start + 1, len(lines)):
        line = lines[index]
        if line.strip() and len(line) - len(line.lstrip()) == indent \
                and line.strip()[0] in "}]":
            return index
    raise Refused(f"could not find the end of: {lines[start].strip()}")


def find_line(lines: List[str], pattern: str, start: int = 0, end: Optional[int] = None) -> int:
    regex = re.compile(pattern)
    for index in range(start, len(lines) if end is None else end):
        if regex.search(lines[index]):
            return index
    raise Refused(f"could not find /{pattern}/")


def declare(text: str, type_name: str, variant: str) -> str:
    """The variant added at the end of the enum and of its `ALL` list."""
    lines = text.splitlines(True)
    start = find_line(lines, rf"^pub enum {type_name} \{{")
    end = block(lines, start)
    lines.insert(end, f"    {variant},\n")
    impl = find_line(lines, rf"^impl {type_name} \{{")
    at = find_line(lines, r"pub const ALL: \[Self; \d+\] = \[$", impl, block(lines, impl))
    count = int(re.search(r"\[Self; (\d+)\]", lines[at]).group(1))
    lines[at] = lines[at].replace(f"[Self; {count}]", f"[Self; {count + 1}]")
    lines.insert(block(lines, at), f"        Self::{variant},\n")
    return "".join(lines)


def add_arm(text: str, type_name: str, function: str, variant: str, value: str) -> str:
    """`Self::Variant => "value",` after the last arm of `fn function` in the type's impl."""
    lines = text.splitlines(True)
    impl = find_line(lines, rf"^impl {type_name} \{{")
    start = find_line(lines, rf"pub const fn {function}\(self\)", impl, block(lines, impl))
    end = block(lines, start)
    arms = [i for i in range(start, end) if re.match(r'\s*Self::\w+ => ".*",\s*$', lines[i])]
    if not arms:
        raise Refused(f"could not find the arms of {type_name}::{function}")
    last = arms[-1]
    indent = lines[last][: len(lines[last]) - len(lines[last].lstrip())]
    lines.insert(last + 1, f"{indent}Self::{variant} => {rust_string(value)},\n")
    return "".join(lines)


def _alternative(text: str, start: int, end: int) -> bool:
    """Whether the path at text[start:end] is one alternative of a pattern: followed by `=>` or
    a lone `|`, or preceded by a lone `|`."""
    after = text[end:].lstrip()
    if after.startswith("=>") or (after.startswith("|") and not after.startswith("||")):
        return True
    before = text[:start].rstrip()
    return before.endswith("|") and not before.endswith("||")


def copy_matches(text: str, path: str, type_name: str, like: str, variant: str,
                 skip: Optional[Tuple[int, int]] = None) -> Tuple[str, List[Dict], List[Dict]]:
    """Every pattern alternative naming `like` gains `| …::variant` beside it. `Self::like`
    counts inside the type's own impl. Returns the text and the places copied and left."""
    copied: List[Dict] = []
    left: List[Dict] = []
    pattern = re.compile(rf"\b(?:{type_name}|Self)::{like}\b")
    lines = text.splitlines(True)
    impl = None
    try:
        impl_start = find_line(lines, rf"^impl {type_name} \{{")
        offset = sum(len(line) for line in lines[:impl_start])
        impl = (offset, offset + sum(len(line) for line in lines[impl_start:block(lines,
                                                                                    impl_start)]))
    except Refused:
        pass
    out = []
    last = 0
    for match in pattern.finditer(text):
        start, end = match.span()
        own = match.group(0).startswith("Self::")
        if own and not (impl and impl[0] <= start < impl[1]):
            continue
        if skip and skip[0] <= start < skip[1]:
            continue
        line_start = text.rfind("\n", 0, start) + 1
        line_text = text[line_start:text.find("\n", start)]
        # Comments, and the entry in its own `ALL` list, are not matches.
        if line_text.lstrip().startswith("//") or line_text.strip() == f"Self::{like},":
            continue
        number = text.count("\n", 0, start) + 1
        place = {"file": path, "line": number, "text": line_text.strip()}
        if _alternative(text, start, end):
            prefix = "Self" if own else type_name
            out.append(text[last:end] + f" | {prefix}::{variant}")
            last = end
            copied.append(place)
        else:
            left.append(place)
    out.append(text[last:])
    return "".join(out), copied, left


def _names_span(text: str, type_name: str) -> Tuple[int, int]:
    """Where the type's label and description are, which `declare` and `add_arm` handle."""
    lines = text.splitlines(True)
    impl = find_line(lines, rf"^impl {type_name} \{{")
    start = find_line(lines, r"pub const fn label\(self\)", impl, block(lines, impl))
    end = block(lines, find_line(lines, r"pub const fn description\(self\)", start,
                                 block(lines, impl)))
    offset = lambda index: sum(len(line) for line in lines[:index])  # noqa: E731
    return offset(start), offset(end + 1)


def released_save_version(version: str) -> Optional[int]:
    """The save version of the release this Desktop's version names, if its tag is here."""
    done = subprocess.run(["git", "show", f"v{version}:{LIB}"], cwd=DESKTOP,
                          capture_output=True, text=True)
    match = re.search(r"pub const SAVE_VERSION: u32 = (\d+);", done.stdout)
    return int(match.group(1)) if done.returncode == 0 and match else None


def raise_save_version(plan: Plan, what: str) -> None:
    lib = plan.text(LIB)
    current = int(re.search(r"pub const SAVE_VERSION: u32 = (\d+);", lib).group(1))
    released = released_save_version(desktop_version())
    lines = plan.text(MIGRATIONS).splitlines(True)
    table = find_line(lines, r"^const STEPS: &\[Step\] = &\[$")
    end = block(lines, table)
    if released is not None and current > released:
        # Raised already since the last release: one version carries everything new in it.
        step = find_line(lines, rf"^\s*Step::\w+\({current - 1}\b", table, end)
        lines.insert(step, f"    // Also {what}.\n")
        plan.change(MIGRATIONS, "".join(lines))
        plan.notes.append(f"The save version is already {current}, raised since "
                          f"v{desktop_version()}; {plan.name} is noted on it.")
        return
    lines[end:end] = [f"    // {current + 1}: {what}. An older colony has none.\n",
                      f"    Step::adds_only({current}),\n"]
    plan.change(MIGRATIONS, "".join(lines))
    plan.change(LIB, lib.replace(f"pub const SAVE_VERSION: u32 = {current};",
                                 f"pub const SAVE_VERSION: u32 = {current + 1};"))
    plan.save_version = current + 1
    if released is None:
        plan.notes.append(f"The release tag v{desktop_version()} is not here (`git fetch --tags` "
                          "brings it), so the save version is raised to be safe.")
    else:
        plan.notes.append(f"Save version {current} → {current + 1}: colony files holding "
                          f"{plan.name} cannot be read by an older Desktop, Hill or Home.")


def limit(kind: str) -> Optional[int]:
    """How many of a kind the habitat's ranking leaves room for."""
    text = (DESKTOP / HABITAT).read_text(encoding="utf-8")
    offsets = {name.lower(): int(at or 0) for name, at in re.findall(
        r"Self::(Hangout|Garden|Ornament)\(kind\) => (?:(\d+) \+ )?kind\.index\(\)", text)}
    if kind not in offsets:
        return None
    ordered = sorted(offsets.values())
    gaps = [b - a for a, b in zip(ordered, ordered[1:])]
    return min(gaps) if gaps else None


def plan_addition(data: dict, kind_key: str, name: str, about: str, like: str,
                  variant: Optional[str] = None) -> Plan:
    if kind_key not in SUPPORTED:
        raise Refused(f"it can add a {', a '.join(SUPPORTED[:-1])} or a {SUPPORTED[-1]}; "
                      f"{kind_key}s are added by hand")
    kind = next(k for k in data["kinds"] if k["kind"] == kind_key)
    type_name = kind["type"].split()[0]
    variant = variant or camel(name)
    if not re.fullmatch(r"[A-Z][A-Za-z0-9]*", variant):
        raise Refused(f"{variant!r} cannot be a Rust name; give one with --id")
    if name != name.strip() or "  " in name or not name.strip():
        raise Refused(f"{name!r} has stray spaces")
    if not about.strip():
        raise Refused("--about needs a sentence saying what it is, as the others have")
    for item in kind["items"]:
        if catalog.normal(item["name"]) == catalog.normal(name):
            raise Refused(f"there is already a {kind_key} called {item['name']!r} "
                          f"({type_name}::{item['id']})")
        if item["id"] == variant:
            raise Refused(f"{type_name}::{variant} already exists; give another with --id")
    likes = [item for item in kind["items"] if catalog.matches(item, like)]
    if len(likes) != 1:
        raise Refused(f"--like names no {kind_key}; `formiga inspect {kind_key}` lists them")
    like_id = likes[0]["id"]
    room = limit(kind_key)
    if room is not None and kind["count"] >= room:
        raise Refused(f"there is room for {room} {kind_key}s in the habitat's ranking "
                      f"({HABITAT}), and there are {kind['count']}")

    plan = Plan(kind_key, type_name, variant, name, about, like_id)
    model = kind["defined_in"]
    text = declare(plan.text(model), type_name, variant)
    text = add_arm(text, type_name, "label", variant, name)
    text = add_arm(text, type_name, "description", variant, about)
    plan.change(model, text)
    for path in sorted((DESKTOP / SEARCHED).rglob("*.rs")):
        rel = path.relative_to(DESKTOP).as_posix()
        before = plan.text(rel) if rel in plan.files else path.read_text(encoding="utf-8")
        if like_id not in before:
            continue
        skip = _names_span(before, type_name) if rel == model else None
        after, copied, left = copy_matches(before, rel, type_name, like_id, variant, skip)
        plan.copied += copied
        plan.unchanged += left
        if after != before:
            if rel not in plan.files:
                plan.files[rel] = (before, before)
            plan.change(rel, after)
    raise_save_version(plan, f"the {name.lower()}, a new {kind_key}")
    return plan


def add(kind: str, name: str, about: str, like: str, variant: Optional[str] = None,
        write: bool = False) -> Result:
    result = Result("content add")
    data, step = catalog.load()
    result.steps.append(step)
    if data is None:
        return result
    try:
        plan = plan_addition(data, kind, name, about, like, variant)
    except Refused as why:
        result.error = str(why)
        return result
    result.data.update({
        "kind": kind, "variant": f"{plan.type_name}::{plan.variant}", "name": name,
        "like": plan.like, "files": sorted(plan.files), "save_version": plan.save_version,
        "copied": plan.copied, "unchanged": plan.unchanged, "diff": plan.diff(),
    })
    result.steps.append(_fits(name))
    result.steps.append(Step("plan", "passed",
                             f"{len(plan.files)} file(s): declared, named, matched wherever "
                             f"{plan.like} is ({len(plan.copied)} place(s))"))
    if not write:
        result.data["notes"] = ([plan.diff(), ""] + plan.notes
                                + _left_alone(plan)
                                + ["", "Nothing is changed yet: run again with --write."])
        result.verdict = False
        return result
    dirty = _uncommitted(list(plan.files))
    if dirty:
        result.steps.append(Step("write", "failed", "these files have uncommitted changes",
                                 problems=[Problem("write", "commit or stash it first", file=f)
                                           for f in dirty]))
        return result
    for path, (_, after) in plan.files.items():
        (DESKTOP / path).write_text(after, encoding="utf-8")
    result.steps.append(Step("write", "passed", f"changed {len(plan.files)} file(s); "
                                                "`git diff` shows them"))
    result.steps.append(_build())
    if result.steps[-1].status == "passed":
        result.steps += _checks(plan)
        result.steps.append(_tests())
    undo = ["", "`git diff` shows every change; `git checkout -- "
                + " ".join(sorted(plan.files)) + "` undoes them."]
    result.data["notes"] = plan.notes + _todo(plan) + _left_alone(plan) + undo
    return result


def _fits(name: str) -> Step:
    document, done, problems = cargo.tools_dev(["fits", name], "fits")
    if document is None:
        return Step("fits", "failed", "could not measure the name", done.seconds, problems)
    found = []
    for size in document["shown"][0]["sizes"]:
        if not size["fits"]:
            found.append(Problem(
                "fits", f"{name!r} takes {size['lines']} lines on its Village tile at "
                        f"{size['text_scale']}% text, where there is room for one",
                level="warning" if size["text_scale"] > 100 else None))
    errors = [p for p in found if not p.is_warning]
    return Step("fits", "failed" if errors else "passed",
                "does not fit its tile" if errors else "fits its tile on the Village page",
                done.seconds, found)


def _uncommitted(paths: List[str]) -> List[str]:
    done = subprocess.run(["git", "status", "--porcelain", "--", *paths], cwd=DESKTOP,
                          capture_output=True, text=True)
    return [line[3:] for line in done.stdout.splitlines() if line.strip()]


def _build() -> Step:
    run(["cargo", "fmt", "--all"])
    app = app_builds_here()
    done = run(["cargo", "check", "--workspace", *([] if app else ["--exclude", APP_CRATE]),
                "--all-targets", "--message-format=json"])
    if done.code:
        problems = cargo.compiler_problems(done.output, "build") or [
            Problem("build", cargo.last_error(done.output))]
        return Step("build", "failed", "does not compile", done.seconds, problems)
    if app:
        return Step("build", "passed", "formatted and compiles", done.seconds)
    return Step("build", "passed", "formatted and compiles, all but the app, which builds only "
                                   "on macOS and Windows (`formiga validate --step app` lints it "
                                   "as a Windows build)", done.seconds)


def _checks(plan: Plan) -> List[Step]:
    steps = []
    content = audit.audit_content()
    mine = f"{plan.kind}:{plan.variant}"

    def about_it(problem: Problem) -> bool:
        return problem.object == mine or bool(re.search(rf"\b{plan.variant}\b", problem.message))

    found = [p for s in content.steps for p in s.problems if about_it(p)]
    steps.append(Step("audit", "failed" if any(not p.is_warning for p in found) else "passed",
                      "the content audit is happy with it" if not found else
                      f"the content audit says {len(found)} thing(s) about it",
                      problems=found))
    art = audit.art_check(plan.kind)
    drawn = [p for s in art.steps for p in s.problems if about_it(p)]
    # Drawn the same as the piece it is like, until it has a drawing of its own.
    pair = {f"{plan.kind}:{plan.like}", mine}
    expected = [p for p in drawn if p.object in pair
                and re.search(rf"\b({plan.like}|{plan.variant})\b", p.message)]
    for problem in expected:
        problem.level = "warning"
    steps.append(Step("art", "failed" if any(not p.is_warning for p in drawn) else "passed",
                      f"drawn like {plan.like} for now" if expected else "drawn",
                      problems=drawn))
    return steps


def _tests() -> Step:
    """The tests of the crates a new piece lives in. Some count the pieces or check every one is
    drawn its own way, and fail until the to-do is done; they say where."""
    done = run(["cargo", "test", "-p", "formiga-core", "-p", "formiga-art", "--no-fail-fast",
                "--message-format=json"])
    problems = cargo.test_problems(done.output, "tests")
    if done.code == 0:
        return Step("tests", "passed", "formiga-core's and formiga-art's tests pass", done.seconds)
    if not problems:
        problems = [Problem("tests", cargo.last_error(done.output))]
    return Step("tests", "failed", f"{len(problems)} test(s) fail until the to-do is done: they "
                                   "count the pieces or compare their drawings", done.seconds,
                problems)


def _todo(plan: Plan) -> List[str]:
    art = [p for p in plan.copied if p["file"].startswith("crates/formiga-art/")]
    rest = [p for p in plan.copied if not p["file"].startswith("crates/formiga-art/")]
    lines = ["", f"To do for {plan.name}:"]
    for place in art:
        lines.append(f"  draw it      {place['file']}:{place['line']}  (drawn as {plan.like})")
    for place in rest:
        lines.append(f"  behaves      {place['file']}:{place['line']}  {place['text'][:70]}")
    lines.append("  tests        update the ones above that count the pieces; formiga-art's "
                 "check that every cell is its own passes once it has its own drawing")
    lines.append("  then         formiga validate")
    return lines


def _left_alone(plan: Plan) -> List[str]:
    if not plan.unchanged:
        return []
    lines = ["", f"{plan.like} is also named in these places, which were left as they are:"]
    for place in plan.unchanged[:12]:
        lines.append(f"  {place['file']}:{place['line']}  {place['text'][:70]}")
    if len(plan.unchanged) > 12:
        lines.append(f"  … and {len(plan.unchanged) - 12} more (--json lists them all)")
    return lines
