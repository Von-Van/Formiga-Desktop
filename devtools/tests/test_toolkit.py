"""Checks for the toolkit itself: python3 -m unittest discover devtools/tests

The cargo samples in fixtures/ are real output from a two-crate workspace: one with a type error,
then one with two failing tests (a panic and an assert_eq), run as validate runs cargo.
"""

from __future__ import annotations

import io
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))

from formiga_dev import (  # noqa: E402
    audit, cargo, catalog, content, save, scenarios, status, uses, validate,
)
from formiga_dev.report import Problem, Result, Step, emit  # noqa: E402
from formiga_dev import workspace  # noqa: E402
from formiga_dev.workspace import Expansion, lockfile_kept  # noqa: E402

FIXTURES = HERE / "fixtures"
WS = Path("/WS")


class CompilerMessages(unittest.TestCase):
    def test_a_type_error_names_its_crate_file_and_line(self):
        output = (FIXTURES / "cargo-out-compile.txt").read_text()
        problems = cargo.compiler_problems(output, "tests", WS)
        self.assertEqual(len(problems), 1)
        problem = problems[0]
        self.assertEqual(problem.system, "demo-tools")
        self.assertEqual(problem.file, "crates/demo-tools/src/lib.rs")
        self.assertEqual(problem.line, 1)
        self.assertIn("mismatched types", problem.message)
        self.assertIn("E0308", problem.message)


class TestFailures(unittest.TestCase):
    def setUp(self):
        output = (FIXTURES / "cargo-out-tests.txt").read_text()
        self.problems = {p.object: p for p in cargo.test_problems(output, "tests", WS)}

    def test_every_failure_is_found_once(self):
        self.assertEqual(sorted(self.problems),
                         ["tests::decoration_keeps_its_slot", "tests::panics_plainly"])

    def test_a_failure_carries_its_package_panic_site_and_message(self):
        problem = self.problems["tests::decoration_keeps_its_slot"]
        self.assertEqual(problem.system, "demo-core")
        self.assertEqual(problem.file, "crates/demo-core/src/lib.rs")
        self.assertEqual(problem.line, 11)
        self.assertEqual(problem.message, "assertion `left == right` failed: Banner lost its slot")
        self.assertIn("right: 3", problem.detail)
        self.assertIn("cargo test -p demo-core --lib -- --exact", problem.detail)

    def test_a_plain_panic_keeps_its_words(self):
        problem = self.problems["tests::panics_plainly"]
        self.assertEqual(problem.message, "the colony file has grown to 9000000 bytes")
        self.assertEqual(problem.line, 15)

    def test_passing_counts_are_read_from_result_lines(self):
        line = "test result: FAILED. 1 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out"
        self.assertEqual(validate._count(line, "passed"), 1)
        self.assertEqual(validate._count(line, "failed"), 2)
        self.assertEqual(validate._count("running 3 tests", "passed"), 0)


class Formatting(unittest.TestCase):
    def test_each_unformatted_file_is_named_once_at_its_first_change(self):
        output = ("Diff in /WS/crates/a/src/lib.rs:12:\n-x\n+y\n"
                  "Diff in /WS/crates/a/src/lib.rs:40:\n-x\n+y\n"
                  "Diff in /WS/crates/b/src/main.rs at line 3:\n")
        problems = cargo.format_problems(output, "format", WS)
        self.assertEqual([(p.file, p.line) for p in problems],
                         [("crates/a/src/lib.rs", 12), ("crates/b/src/main.rs", 3)])


class Scenarios(unittest.TestCase):
    def test_every_desktop_scenario_names_a_fixture_formiga_tools_has(self):
        dev = (HERE.parents[1] / "crates/formiga-tools/src/dev.rs").read_text()
        for scenario in scenarios.SCENARIOS.values():
            if scenario.fixture:
                self.assertIn(f'name: "{scenario.fixture}"', dev, scenario.name)

    def test_every_hill_place_is_a_render_and_a_snap_place_hill_knows(self):
        hill = workspace.find_expansion("hill").path
        if not hill:
            self.skipTest("no Formiga Hill checkout beside this one")
        main = hill / "crates/formiga-hill/src/main.rs"
        usage = main.read_text()
        if "--place <PLACE>" not in usage:
            self.skipTest(f"the Hill checkout at {hill} is older than its --snap places")
        for place in ["station", *scenarios.HILL_PLACES]:
            self.assertIn(f"--render-{place} ", usage)
            self.assertIn(place, usage.split("--place <PLACE>", 1)[1].split("\n--", 1)[0])

    def test_the_open_command_pastes_into_powershell_on_windows(self):
        line = scenarios._command_line({"FORMIGA_DATA_DIR": r"C:\Users\Jo's PC\data"},
                                       [r"C:\dev\formiga-desktop.exe"], platform="win32")
        self.assertEqual(line, "$env:FORMIGA_DATA_DIR = 'C:\\Users\\Jo''s PC\\data'; "
                               "& 'C:\\dev\\formiga-desktop.exe'")

    def test_the_open_command_pastes_into_sh_elsewhere(self):
        line = scenarios._command_line({"FORMIGA_DATA_DIR": "/tmp/my data"},
                                       ["cargo", "run", "-p", "formiga-desktop"], platform="linux")
        self.assertEqual(line, "FORMIGA_DATA_DIR='/tmp/my data' cargo run -p formiga-desktop")

    def test_scenarios_that_only_capture_say_so(self):
        for scenario in scenarios.SCENARIOS.values():
            if not scenario.launchable:
                self.assertIsNotNone(scenario.capture, scenario.name)


class AppStep(unittest.TestCase):
    def test_picked_alone_where_the_app_builds_it_is_linted(self):
        linted = Step("app", "passed", "no warnings")
        with mock.patch.object(validate, "app_builds_here", return_value=True), \
                mock.patch.object(validate, "_clippy", return_value=linted) as clippy:
            result = validate.validate(only=["app"])
        clippy.assert_called_once_with("app", ["-p", "formiga-desktop"])
        self.assertEqual(result.steps[0].status, "passed")

    def test_alongside_lint_it_is_not_linted_twice(self):
        with mock.patch.object(validate, "app_builds_here", return_value=True), \
                mock.patch.object(validate, "_clippy") as clippy:
            self.assertEqual(validate.step_app(with_lint=True).status, "skipped")
        clippy.assert_not_called()


class Expansions(unittest.TestCase):
    def test_pins_are_read_from_the_manifest(self):
        with tempfile.TemporaryDirectory() as folder:
            (Path(folder) / "Cargo.toml").write_text(
                '[dependencies]\n'
                'formiga-art = { git = "https://github.com/Von-Van/Formiga-Desktop", tag = "v0.67.1" }\n'
                'formiga-core = { git = "https://github.com/Von-Van/Formiga-Desktop", tag = "v0.67.1" }\n'
                'serde = "1"\n')
            expansion = Expansion("hill", "Formiga Hill", "formiga-hill", "X", Path(folder))
            self.assertEqual(expansion.pins(), {"formiga-art": "v0.67.1", "formiga-core": "v0.67.1"})

    def found_from(self, desktop, key="farm"):
        """Where `key` is found when the toolkit runs from the checkout at `desktop`."""
        fresh = {k: Expansion(e.key, e.title, e.package, e.data_env)
                 for k, e in workspace.EXPANSIONS.items()}
        with mock.patch.object(workspace, "DESKTOP", desktop), \
                mock.patch.dict(workspace.EXPANSIONS, fresh), \
                mock.patch.dict("os.environ", {f"FORMIGA_{key.upper()}_REPO": ""}):
            return workspace.find_expansion(key).path

    def test_a_sibling_is_found_beside_the_main_checkout_from_a_worktree(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder).resolve()
            desktop = root / "Formiga Desktop"
            desktop.mkdir()
            git = ["git", "-c", "user.name=t", "-c", "user.email=t@t", "-C", str(desktop)]
            subprocess.run([*git, "init", "-q"], check=True)
            subprocess.run([*git, "commit", "-q", "--allow-empty", "-m", "start"], check=True)
            tree = desktop / ".claude" / "worktrees" / "tidy"
            subprocess.run([*git, "worktree", "add", "-q", "-b", "tidy", str(tree)], check=True)
            farm = root / "Formiga Farm"
            farm.mkdir()
            (farm / "Cargo.toml").write_text('[workspace]\nmembers = ["crates/formiga-farm"]\n')
            self.assertEqual(self.found_from(desktop), farm)
            self.assertEqual(self.found_from(tree), farm)
            self.assertIsNone(self.found_from(tree, "hill"))

    def test_the_lockfile_comes_back_as_it_was(self):
        with tempfile.TemporaryDirectory() as folder:
            lock = Path(folder) / "Cargo.lock"
            lock.write_text("before")
            with lockfile_kept(Path(folder)):
                lock.write_text("patched")
            self.assertEqual(lock.read_text(), "before")


def item(id, name, **extra):
    return {"id": id, "name": name, **extra}


def kind(key, items, **extra):
    return {"kind": key, "type": extra.pop("type", "Thing"), "defined_in": "crates/x.rs",
            "saved_in": [], "drawn_by": "", "comes_by": "", "count": len(items),
            "items": items, **extra}


def sample_catalog(*kinds):
    return {"reach": {"colonies": 4, "days": 30}, "names_shown": [
        {"where": "Formiga Home", "kinds": ["trinket", "souvenir"], "letters": 12}],
        "kinds": list(kinds)}


class FakeSource(catalog.Source):
    """Source text given outright: {(repo, path): text}."""

    def __init__(self, files):
        self.roots = {repo: Path("/" + repo) for repo, _ in files}
        self.roots.setdefault("desktop", Path("/desktop"))
        self._files = {key: text.splitlines() for key, text in files.items()}
        self._loaded = True


def problems(step):
    return [(p.object, "warning" if p.is_warning else "error") for p in step.problems]


class Lookup(unittest.TestCase):
    CATALOG = sample_catalog(
        kind("decoration", [item("RoofOrnament", "Roof star", saved_as="RoofOrnament", number=6)]),
        kind("trinket", [item("17", "Button", saved_as=17, number=17)]))

    def test_an_item_is_found_by_its_name_its_id_or_its_number(self):
        for query in ["roof star", "Roof Star", "roof_ornament", "RoofOrnament"]:
            self.assertEqual([i["id"] for _, i in catalog.find(self.CATALOG, query)],
                             ["RoofOrnament"], query)
        self.assertEqual([i["name"] for _, i in catalog.find(self.CATALOG, "17", "trinket")],
                         ["Button"])
        self.assertEqual(catalog.kind_named(self.CATALOG, "decorations")["kind"], "decoration")

    def test_a_label_two_items_share_is_found_on_its_own_arm(self):
        source = FakeSource({("desktop", "crates/x.rs"): (
            "pub enum PartKind {\n    EarsLong,\n    TailLong,\n}\n"
            "fn label(self) {\n    K::EarsLong => \"Long\",\n    K::TailLong => \"Long\",\n}\n")})
        parts = kind("part", [item("TailLong", "Long (tail)", label="Long")], type="PartKind")
        place = catalog.defined_at(source, parts, parts["items"][0])
        self.assertEqual((place["declared"], place["named"]), (3, 7))
        found = catalog.lookup(sample_catalog(parts), ["part", "long", "(tail)"])
        self.assertEqual(found.item["id"], "TailLong")

    def test_a_kind_is_named_as_people_say_it(self):
        data = sample_catalog(kind("accessory", []), kind("object", []), kind("trinket", []),
                              kind("house-style", []))
        for name, wanted in [("accessories", "accessory"), ("colony objects", "object"),
                             ("finds", "trinket"), ("find", "trinket"),
                             ("house styles", "house-style"), ("objects", "object")]:
            self.assertEqual(catalog.kind_named(data, name)["kind"], wanted, name)
        self.assertIsNone(catalog.kind_named(data, "roof star"))

    def test_rust_names_become_the_snake_case_trips_use(self):
        self.assertEqual(catalog.snake("FlowerCrown"), "flower_crown")


class ContentAudit(unittest.TestCase):
    def test_a_name_used_twice_in_a_kind_is_an_error(self):
        data = sample_catalog(kind("hangout", [item("Bench", "Bench"), item("Seat", "bench")]))
        self.assertEqual(problems(audit.names(data, FakeSource({}))), [("hangout:Seat", "error")])

    def test_a_name_too_big_for_its_tile_is_an_error_at_normal_size_and_a_warning_when_larger(self):
        size = {"lines": 2, "height": 36, "room": 28, "word_broken": False, "fits": False}
        sizes = [dict(size, text_scale=100), dict(size, text_scale=150)]
        data = sample_catalog(kind("garden", [
            item("Long", "Long name", shown=[{"where": "ground tile", "sizes": sizes[1:]}]),
            item("Longer", "Longer name", shown=[{"where": "ground tile", "sizes": sizes}])]))
        self.assertEqual(problems(audit.names(data, FakeSource({}))), [
            ("garden:Long", "warning"), ("garden:Longer", "error"), ("garden:Longer", "warning")])

    def test_a_name_formiga_home_would_cut_is_an_error(self):
        data = sample_catalog(kind("souvenir", [item("x", "A very long souvenir name")]))
        self.assertEqual(problems(audit.names(data, FakeSource({}))), [("souvenir:x", "error")])

    def test_a_village_piece_that_never_arrives_is_an_error(self):
        never = {"colonies": 0, "of": 4, "first_day": None, "last_day": None}
        some = {"colonies": 3, "of": 4, "first_day": 2, "last_day": 9}
        data = sample_catalog(kind("ornament", [
            item("Well", "Well", reach=never), item("Fence", "Fence", reach=some),
            item("Post", "Post", reach=never, starting=True)]))
        self.assertEqual(problems(audit.arrival(data)),
                         [("ornament:Well", "error"), ("ornament:Fence", "warning")])

    def test_an_accessory_must_wait_for_its_find(self):
        early = {"worn_before_its_find": True, "worn_once_found": True}
        data = sample_catalog(kind("accessory", [
            item("Hat", "Hat", rule=early, made_from={"find": 3, "name": "Leaf"}),
            item("Cap", "Cap", made_from={"find": 999, "name": None})]))
        self.assertEqual(problems(audit.arrival(data)),
                         [("accessory:Hat", "error"), ("accessory:Cap", "error")])

    def test_souvenirs_and_finds_hill_and_home_name_must_be_desktops(self):
        data = sample_catalog(
            kind("souvenir", [item("picnic_ribbon", "Ribbon"), item("well_penny", "Penny"),
                              item("fair_ticket", "Ticket"), item("lost_sock", "Sock")]),
            kind("trinket", [item(str(n), f"Find {n}", number=n) for n in range(3)]))
        hill = """
const CATALOGUE: [(&str, &str, Giver); 3] = [
    ("picnic_ribbon", "A ribbon", Giver::Story),
    (
        "well_penny",
        "A penny",
        Giver::Story,
    ),
    (FAIR_TICKET, "A ticket", Giver::Game),
];
pub const FAIR_TICKET: &str = "fair_ticket";
"""
        source = FakeSource({
            ("hill", "crates/formiga-hill/src/story/souvenirs.rs"): hill,
            ("hill", "crates/formiga-hill/content/a/content/a.toml"):
                'souvenir = "picnic_ribbon"\nsouvenir = "golden_key"\n',
            ("home", "crates/formiga-home/src/life.rs"):
                'DisplaySource::HillSouvenir { id } => id == "chest_marble",\n'
                'let pin = Accessory::Pin(2);\nlet far = Accessory::Pin(3);\n',
            ("home", "crates/formiga-home/src/shelf.rs"):
                'let find = DesktopFind {\n    variant: 7,\n};\n',
            ("farm", "crates/formiga-farm/src/shelf.rs"):
                'let worn = "souvenir.paper_crown";\nlet pin = Pin(1);\n',
        })
        found = problems(audit.expansions(data, source))
        self.assertEqual(sorted(found), sorted([
            ("souvenir:well_penny", "warning"),   # a story souvenir no story gives
            ("souvenir:lost_sock", "error"),      # Desktop has it, Hill never gives it
            ("souvenir:golden_key", "error"),     # a story gives one Desktop doesn't have
            ("souvenir:chest_marble", "error"),   # Home names one Desktop doesn't have
            ("trinket:3", "error"),               # Home names a find past the last one
            ("trinket:7", "error"),               # …and one written over several lines
            ("souvenir:paper_crown", "error"),    # Farm names one Desktop doesn't have
        ]))


class SaveInspect(unittest.TestCase):
    def test_each_version_says_what_it_added(self):
        text = """    // 27: the souvenirs a colony has brought home. An older colony has
    // brought none home.
    Step::adds_only(26),
    // 28: four more souvenirs.
    // Also the rope bridge, a new hangout.
    Step::adds_only(27),
"""
        self.assertEqual(save.upgrade_notes(text), {
            27: "the souvenirs a colony has brought home. An older colony has brought none home.",
            28: "four more souvenirs. Also the rope bridge, a new hangout.",
        })

    def test_broken_rules_fail_and_repairs_are_warnings(self):
        document = {"broken": ["two companions share an id"],
                    "repairs": {"count": 1, "shown": [
                        {"path": ".creatures[id=7]", "was": "a second copy", "now": None}]},
                    "after_repair": [], "snapshot": {"accepted": True},
                    "opens": {"opens": "file"}}
        self.assertEqual(save._rules(document).status, "failed")
        repair = save._repair(document)
        self.assertEqual(repair.status, "passed")
        self.assertEqual([p.message for p in repair.problems],
                         [".creatures[id=7]: 'a second copy' → nothing"])
        self.assertTrue(repair.problems[0].is_warning)
        self.assertEqual(save._after(document).status, "passed")
        document["after_repair"] = ["no companion is grown up"]
        self.assertEqual(save._after(document).status, "failed")

    def test_what_desktop_would_open(self):
        self.assertEqual(save._opens({"opens": {"opens": "file"}}, Path("c.json")).status,
                         "passed")
        backup = save._opens({"opens": {"opens": "backup", "backup": "c.json.bak"}},
                             Path("c.json"))
        self.assertEqual((backup.status, backup.problems[0].level), ("passed", "warning"))
        self.assertEqual(save._opens({"opens": {"opens": "nothing", "why": "bad"}},
                                     Path("c.json")).status, "failed")


class Uses(unittest.TestCase):
    def hit(self, repo, path):
        return catalog.Hit(repo, path, 1, "")

    def test_places_are_grouped_by_what_a_change_means_for_them(self):
        for repo, path, group in [
            ("desktop", "crates/formiga-core/src/world/home.rs", "code"),
            ("desktop", "crates/formiga-core/src/world/tests/hangouts.rs", "tests"),
            ("desktop", "crates/formiga-travel/src/lib.rs", "contracts"),
            ("desktop", "crates/formiga-core/src/persistence/migrations.rs", "upgrades"),
            ("desktop", "crates/formiga-tools/src/dev_art.rs", "tools"),
            ("desktop", "crates/formiga-travel/tests/fixtures/snapshot-v3.json", "data"),
            ("hill", "crates/formiga-hill/src/story/souvenirs.rs", "expansions"),
            ("farm", "crates/formiga-forms/src/sculpt.rs", "expansions"),
            ("farm", "crates/formiga-farm-contract/tests/accept.rs", "tests"),
            ("desktop", "crates/formiga-farm-contract/src/replies.rs", "contracts"),
        ]:
            self.assertEqual(uses.group_of(self.hit(repo, path)), group, path)

    def answers(self, kind_key, item_, saved_in=("home.hangouts[].kind",), **groups):
        found = {key: [] for key in uses.GROUPS}
        for key, count in groups.items():
            found[key] = [self.hit("desktop", "x.rs")] * count
        return uses.answer(kind(kind_key, [item_], saved_in=list(saved_in)), item_,
                           {"file": "m.rs", "named": 3, "declared": 2}, found, [], {})

    def test_a_saved_rust_name_needs_a_serde_rename_to_change(self):
        said = self.answers("hangout", item("Bench", "Bench", saved_as="Bench"), code=2)
        self.assertIn('#[serde(rename = "Bench")]', said["rename"])
        self.assertIn("Colonies holding it would stop loading", said["remove"])

    def test_a_snake_case_save_name_is_spelled_from_the_rust_name(self):
        said = self.answers("souvenir", item("well_penny", "Well penny", saved_as="well_penny",
                                             variant="WellPenny"), expansions=1)
        self.assertIn("spelled from its Rust name", said["rename"])
        self.assertIn("contracts or the expansions", said["rename"])

    def test_an_explicit_save_name_is_already_held_steady(self):
        said = self.answers("wonder", item("LeafSled", "Leaf sled", saved_as="Bike"))
        self.assertIn("already holds steady", said["rename"])

    def test_finds_are_kept_by_number(self):
        said = self.answers("trinket", item("4", "Acorn", saved_as=4, number=4))
        self.assertIn("kept by number", said["rename"])
        self.assertIn("renumbered", said["remove"])


class Report(unittest.TestCase):
    def test_a_check_is_dated_in_plain_words(self):
        with mock.patch.object(status, "datetime", wraps=status.datetime) as clock:
            clock.now.return_value = status.datetime(2026, 10, 7, 12, 0,
                                                     tzinfo=status.timezone.utc)
            self.assertEqual(status.age("2026-10-07T11:30:00Z"), "30 min ago")
            self.assertEqual(status.age("2026-10-07T02:00:00Z"), "10 h ago")
            self.assertEqual(status.age("2026-10-01T12:00:00Z"), "6 days ago")

    def test_only_the_three_checks_are_kept(self):
        with tempfile.TemporaryDirectory() as folder, \
                mock.patch.object(status, "LAST", Path(folder)), \
                mock.patch.object(status, "git", return_value="abc"):
            status.record("inspect", Result("inspect"))
            status.record("validate", Result("validate", steps=[Step("format", "passed")]))
            self.assertEqual(sorted(p.name for p in Path(folder).iterdir()), ["validate.json"])
            kept = status.last("validate")
            self.assertEqual((kept["success"], kept["commit"]), (True, "abc"))


MODEL = """pub enum HangoutKind {
    Cushion,
    Swing,
}

impl HangoutKind {
    pub const ALL: [Self; 2] = [
        Self::Cushion,
        Self::Swing,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Cushion => "Nap cushion",
            Self::Swing => "Swing",
        }
    }

    pub const fn description(self) -> &'static str {
        match self {
            Self::Cushion => "Somewhere soft.",
            Self::Swing => "A plank on two ropes.",
        }
    }

    pub const fn lively(self) -> bool {
        matches!(self, Self::Swing)
    }
}
"""


class ContentAdd(unittest.TestCase):
    def test_the_new_piece_is_declared_listed_and_named_at_the_end(self):
        text = content.declare(MODEL, "HangoutKind", "RopeBridge")
        text = content.add_arm(text, "HangoutKind", "label", "RopeBridge", 'Rope "bridge"')
        text = content.add_arm(text, "HangoutKind", "description", "RopeBridge", "Bouncy.")
        self.assertIn("    Swing,\n    RopeBridge,\n}", text)
        self.assertIn("pub const ALL: [Self; 3] = [", text)
        self.assertIn("        Self::Swing,\n        Self::RopeBridge,\n    ];", text)
        self.assertIn('Self::Swing => "Swing",\n            Self::RopeBridge => "Rope \\"bridge\\"",',
                      text)
        self.assertIn('            Self::RopeBridge => "Bouncy.",\n        }', text)

    def test_matches_on_the_like_piece_name_the_new_one_and_nothing_else(self):
        code = """match kind {
    HangoutKind::Swing => 1,
    HangoutKind::Lookout
        | HangoutKind::Swing
        | HangoutKind::Sandbox => 2,
}
let swinging = kind == HangoutKind::Swing || other;
// HangoutKind::Swing is the oldest
set(HangoutKind::Swing);
"""
        out, copied, left = content.copy_matches(code, "a.rs", "HangoutKind", "Swing",
                                                 "RopeBridge")
        self.assertIn("HangoutKind::Swing | HangoutKind::RopeBridge => 1,", out)
        self.assertIn("| HangoutKind::Swing | HangoutKind::RopeBridge\n", out)
        self.assertIn("kind == HangoutKind::Swing || other", out)
        self.assertEqual([place["line"] for place in copied], [2, 4])
        self.assertEqual([place["line"] for place in left], [7, 9])

    def test_inside_its_own_impl_self_counts_and_the_names_are_left_to_add_arm(self):
        span = content._names_span(MODEL, "HangoutKind")
        out, copied, _ = content.copy_matches(MODEL, "m.rs", "HangoutKind", "Swing",
                                              "RopeBridge", span)
        self.assertIn("matches!(self, Self::Swing)", out)  # one pattern, no `|`: listed only
        self.assertNotIn('Self::Swing | Self::RopeBridge => "Swing"', out)
        self.assertEqual(copied, [])

    def test_a_name_must_be_new_and_the_like_piece_real(self):
        data = sample_catalog(kind("hangout", [item("Swing", "Swing")], type="HangoutKind"))
        with mock.patch.object(content, "limit", return_value=32):
            for name, like, why in [("swing", "Swing", "already a hangout called"),
                                    (" Rope", "Swing", "stray spaces"),
                                    ("Rope", "Hammock", "--like names no hangout")]:
                with self.assertRaises(content.Refused) as refused:
                    content.plan_addition(data, "hangout", name, "About.", like)
                self.assertIn(why, str(refused.exception))
            with self.assertRaises(content.Refused):
                content.plan_addition(data, "decoration", "Rope", "About.", "Swing")

    def test_a_save_version_is_raised_once_per_release(self):
        lib = "pub const SAVE_VERSION: u32 = 28;\n"
        table = ("const STEPS: &[Step] = &[\n    // 28: souvenirs.\n    Step::adds_only(27),\n"
                 "];\n")
        for released, version, note in [(28, "29", "    // 29: the rope bridge, a new hangout. "
                                                     "An older colony has none.\n"),
                                         (27, "28", "    // Also the rope bridge, a new hangout.\n")]:
            plan = content.Plan("hangout", "HangoutKind", "RopeBridge", "Rope bridge", "x", "Swing")
            plan.files = {content.LIB: (lib, lib), content.MIGRATIONS: (table, table)}
            with mock.patch.object(content, "released_save_version", return_value=released), \
                    mock.patch.object(content, "desktop_version", return_value="0.67.3"):
                content.raise_save_version(plan, "the rope bridge, a new hangout")
            self.assertIn(f"SAVE_VERSION: u32 = {version};", plan.files[content.LIB][1])
            self.assertIn(note, plan.files[content.MIGRATIONS][1])


class Output(unittest.TestCase):
    def test_json_lists_every_problem_and_says_whether_it_passed(self):
        result = Result("validate", steps=[
            Step("lint", "failed", problems=[Problem("lint", "unused variable", file="a.rs", line=3)]),
            Step("app", "skipped", "macOS and Windows only"),
        ])
        out = io.StringIO()
        code = emit(result, as_json=True, out=out)
        document = json.loads(out.getvalue())
        self.assertEqual(code, 1)
        self.assertFalse(document["success"])
        self.assertEqual(document["errors"], [
            {"step": "lint", "message": "unused variable", "file": "a.rs", "line": 3}])

    def test_warnings_are_listed_apart_and_do_not_fail_a_run(self):
        result = Result("audit", steps=[Step("names", "passed", problems=[
            Problem("names", "a bit long", level="warning")])])
        document = result.to_json()
        self.assertTrue(document["success"])
        self.assertEqual(document["errors"], [])
        self.assertEqual(document["warnings"], [
            {"step": "names", "message": "a bit long", "level": "warning"}])

    def test_skipped_steps_do_not_fail_a_run(self):
        result = Result("validate", steps=[Step("app", "skipped", "macOS and Windows only")])
        self.assertTrue(result.success)


if __name__ == "__main__":
    unittest.main()
