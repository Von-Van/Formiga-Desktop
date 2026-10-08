# Formiga developer toolkit

One command, `formiga`, for the things a change to Formiga needs every time: check it, open the
app in a known state, look at the result, and find out about any item without searching the code. It is for people and for coding agents alike: every
command prints a short summary, or with `--json` one document an agent can read without parsing
prose.

```sh
devtools/formiga validate              # did this change break Formiga?
devtools/formiga scenario list         # the known states
devtools/formiga scenario mature-colony
devtools/formiga capture hill-green    # .dev/captures/hill-green.png
devtools/formiga inspect "roof star"   # everything about one item
devtools/formiga audit content         # names, reachability, the expansions agreeing
devtools/formiga art check             # every item drawn, every pose, looked over
devtools/formiga save inspect colony.json   # a save, read the way Desktop reads it
devtools/formiga uses hangout bench    # can I rename or remove this?
devtools/formiga report                # where things stand: start here
devtools/formiga content add hangout "Rope bridge" --about "…" --like Swing
```

On Windows: `py devtools\formiga.py …`. It needs Python 3.9 or newer and nothing beyond its
standard library, plus the Rust toolchain Formiga already needs. Everything it makes goes in
`.dev/`, which git ignores.

The toolkit encodes how Formiga works rather than re-implementing it. Colonies are grown and
checked by `formiga-core` itself, through `formiga-tools dev` (`crates/formiga-tools/src/dev.rs`).
The content tools read Formiga's own tables through `formiga-tools dev catalog`
(`dev_catalog.rs`), the art check draws through `formiga-art` (`dev_art.rs`), and save inspect
reads a file with `formiga-core`'s own loader, checker and repair (`dev_save.rs`).
Pictures are drawn by `formiga-art`, or by Hill's and Home's own `--render-*` and `--snap`
options. Validation runs the checks the repository already relies on.

## Commands

### `formiga validate [--quick] [--expansions] [--step NAME]… [--json]`

Runs each check and says passed, failed or skipped, and why. Every failure names the step, the
crate, the file and line, and the Formiga object when there is one: a test, a fixture, a soak
colony. Failures also carry a rerun or replay command.

| Step | What it runs |
|---|---|
| `format` | `cargo fmt --all --check` |
| `lint` | clippy on every crate, warnings as errors, as CI runs it |
| `app` | the desktop app crate: built with the rest on macOS and Windows. On Linux it is linted as a Windows build when the `x86_64-pc-windows-gnu` target and MinGW are installed, and skipped with the reason otherwise. |
| `fixtures` | the fixed colonies below: nothing `violations` names, a save that reads back unchanged, ten seconds of running |
| `content` | `formiga audit content` below. Its errors fail the step; its warnings are counted, and listed by the audit itself |
| `tests` | `cargo test` on every crate |
| `soak` | the nightly soak, short: 120 randomized colonies living a simulated day, their files damaged and repaired |
| `hill`, `home`, `farm` | with `--expansions`: Formiga Hill's, Home's and Farm's own tests, built on *this* checkout's Desktop crates rather than the release they're tied to. This is how a Desktop change that breaks an expansion shows up before release. |

`--quick` runs `format`, `lint` and `fixtures`. `--step` runs only the steps named. The exit code
is 0 only when nothing failed.

### `formiga scenario NAME [--no-launch] [--wait] [--pinned] [--fixed-date] [--json]`

Opens Formiga in a fixed state, in a fresh data folder under `.dev/scenarios/NAME/`, so it never
touches your real colony. `formiga scenario list` shows them all.

| Scenario | Opens |
|---|---|
| `first-run` | Desktop with no colony: the real first launch |
| `new-colony`, `young-colony`, `mature-colony` | Desktop on a colony just begun, ten days old, or half a year old |
| `full-village` | Desktop forty days in, with every decoration, a full set of objects, a guest at the door and a full guest book |
| `hill-trip`, `home-visit` | Desktop on the mature colony, which sets off for Hill or Home five seconds after opening |
| `hill` | Formiga Hill on its sample colony |
| `home`, `home-lived-in` | Formiga Home's sample household, new or a few weeks on |
| `hill-green`, `hill-clubhouse`, `hill-fairground`, `hill-woods`, `hill-hilltop` | capture only, since Hill's window always opens at the station |

Desktop colonies are grown from fixed seeds to the moment you open them, so a scenario is always
the age its name says. `--fixed-date` grows them to a fixed date instead, for a byte-identical
file. The desktop app runs only on macOS and Windows. Elsewhere, or with `--no-launch`, the
scenario is prepared and the command that opens it is printed.

Hill and Home are found in the checkout beside this one (`Formiga-Hill` or `Formiga Hill`, or set
`FORMIGA_HILL_REPO` / `FORMIGA_HOME_REPO`). From a git worktree of Desktop (under
`.claude/worktrees/`, say), they are also looked for beside the main checkout. Formiga Farm is
found the same way (`FORMIGA_FARM_REPO`) for `validate`, `audit`, `inspect`, `uses` and `report`. They are built on this checkout's Desktop crates, so a
change to Desktop's drawing shows up in them; `--pinned` builds them on their own Desktop release
instead. Their `Cargo.lock` is put back afterwards, and the build goes in `.dev/target/`.

### `formiga capture NAME|all [--window] [--pinned] [--out PNG] [--json]`

Writes `.dev/captures/NAME.png`, with `NAME.json` beside it recording what was drawn, from which
commit and by which command.

- **Desktop scenarios** are drawn as the colony card and every resident's card, by the same
  renderers the app shares and exports with. Desktop's live overlay can't be photographed: by design
  it never reads the screen, and the GPU has no picture to hand back.
- **Hill and Home** are drawn off-screen by their own `--render-*` options at midday: the same
  pixels every time, on any machine, Linux included.
- **`--window`** opens Hill's or Home's real window at its fixed size and pictures it a few seconds
  in with their own `--snap`, under `xvfb-run` on Linux without a display. That picture includes the
  on-screen controls, but it is timed by the clock, so it is close, not exact.

`formiga capture all` pictures every scenario that can be drawn.

### `formiga inspect [KIND] [ITEM] [--json]`

Everything about one item: its name and id, the line that declares it and the line that names
it, the name a save keeps and where, the functions that draw it, how a player comes by it,
whether its name fits where it is shown, and every place in Desktop, Hill, Home and Farm that mentions
it (real code first, tests last).

```sh
devtools/formiga inspect                   # every kind, with how many there are
devtools/formiga inspect garden            # every garden
devtools/formiga inspect "picnic blanket"  # by name, id or saved name, any kind
devtools/formiga inspect trinket 17        # by number, within a kind
```

Kinds: decorations, hangouts, gardens, ornaments, colony objects, house styles, palettes, finds
(`trinket`), accessories, souvenirs, wonders, body plans, ear styles, archetypes and habits.
Village pieces and colony objects also say when they turned up in 24 colonies lived through
day by day for 240 days. Finds are mostly referred to by number in tables, so their uses are the
places that name them outright.

### `formiga audit content [--json]`

Checks Formiga's content as its own code lists it. Errors fail it; warnings are for a look.

| Step | Checks |
|---|---|
| `names` | every name unique within its kind and tidy; a village piece's name fits its tile on the Village page, measured in egui's own font at the app's text sizes (an error at normal size, a warning at the largest); a find's or souvenir's name short enough for Formiga Home to show whole |
| `arrival` | every village piece arrives in the colonies lived through; every kind of colony object turns up somewhere; every accessory can be worn once its find is found and not before; every find and wonder has a hint |
| `expansions` | every souvenir Desktop knows is one Hill gives, and every story souvenir has a story that gives it; every souvenir and find Hill, Home and Farm name by id or number is one Desktop has |

### `formiga art check [--kind KIND] [--json]`

Draws every item in every pose through Formiga's own renderers, each into a frame of its own,
and checks each drawing:

- **blank**: nothing drawn, or an accessory or decoration that never shows (errors);
- **edge**: art reaching the edge of its frame, where anything further is cut off (a warning;
  ground pieces and souvenirs fill their cells by design and are exempt);
- **look-alike**: two items of a kind drawn the same in the same pose (an error), or nearly so
  (a warning).

What is drawn: colony objects, hangouts, gardens at every stage, ornaments, every decoration on
every house type by day and lit, house types, palettes, every find resting and glinting,
souvenirs, every frame of every wonder, every accessory on six bodies in every clip facing both
ways, every kind of body the generator makes in every frame of every clip, and every face. That
is about 22,000 drawings, in a release build: a minute the first time, then seconds.

A sheet of each kind is written to `.dev/art/KIND.png` for looking over by eye, with anything
flagged outlined in red (error) or amber (warning).

### `formiga save inspect FILE [--json]`

Reads a colony file with Formiga's own loader and says what Desktop would make of it:

| Step | Says |
|---|---|
| `read` | whether it can be read at all, and if not, why |
| `version` | the save version it names; an older file lists each upgrade that brings it forward, with what that version added |
| `rules` | every rule the file breaks before it is put right (`violations`): these fail it |
| `repair` | each value Desktop's repair would change on loading it, by path (warnings) |
| `after` | anything still broken after the repair, which would be a bug in `formiga-core` |
| `snapshot` | whether it would be accepted as a snapshot to restore from (snapshots are refused, not repaired) |
| `opens` | what Desktop would open if this were its colony file: the file, the backup kept beside it, or a new colony |

Then a summary of the colony: its companions, journal, unlocks, objects, finds and trips. It
passes only when the file reads as it stands with nothing to put right, so it doubles as the
check for a save. The file is never written to.

### `formiga uses [KIND] ITEM [--json]`

For "can I rename or remove this?". It finds every place Desktop, Hill, Home and Farm use an
item, as `inspect` does, groups them (data files holding it, the expansions, the trip, Home and
Farm contracts,
save upgrades, Desktop's code, tools, tests, and other places spelling its name out), and
answers three questions:

- **Change the name** players see: one line, and the places that spell it out too.
- **Rename it in the code:** whether colony files keep its Rust name, and the
  `#[serde(rename = "…")]` that keeps them loading if so; for finds, that they are kept by number.
- **Remove it:** colonies that hold it stop loading without a new save version and upgrade;
  later finds would be renumbered; and the expansions keep it until they move to a Desktop
  release without it.

### `formiga report [--json]`

The first command for a person or agent starting work: Desktop's version, save version, branch,
commit and uncommitted files; the last `validate`, `audit content` and `art check` (when, on
which commit, and what they found, kept in `.dev/last/`); how many of each kind of thing there
are; Hill's, Home's and Farm's checkouts and the Desktop release each builds on; the scenarios and which
are prepared; and the newest captures.

### `formiga content add KIND NAME --about TEXT --like ITEM [--id VARIANT] [--write] [--json]`

Adds a hangout, garden or ornament the way Formiga's own were added, and shows it as a diff
first:

- the variant at the end of its enum and of its `ALL` list, with its name and description;
- wherever the `--like` piece is matched on (how it behaves, how it is drawn), the new piece
  is matched too, so it starts out working exactly like it;
- a new save version with an upgrade that adds nothing, as every new saved thing gets, unless
  the save version has been raised since the last release (then the piece is noted on it).

It refuses a name already used in the kind, a name with stray spaces, a `--like` that is not
one of the kind, and a kind that is full; it measures the name on its Village tile. Nothing is
changed without `--write`, and `--write` refuses if any file it would change has uncommitted
changes, so `git diff` shows exactly what it did and `git checkout` undoes it. It then formats,
compiles, and runs the content audit and the art check for the kind, and ends with the to-do:
the drawing to make (until then it is drawn like the `--like` piece, which the art check and
`formiga-art`'s own test point out) and the behaviour copied from `--like` to review.

## Adding to it

- **A Desktop colony state:** add a `Fixture` in `crates/formiga-tools/src/dev.rs`, then a
  `Scenario` that names it in `formiga_dev/scenarios.py`. `formiga validate --step fixtures` holds
  every fixture to the save rules.
- **An expansion state:** add a `Scenario` with that app's own arguments and render option.
- **A validation step:** add a function to `formiga_dev/validate.py` that returns a `Step` with
  `Problem`s.
- **A new kind of item:** list it in `crates/formiga-tools/src/dev_catalog.rs` (inspect and the
  content audit pick it up from there), and draw it in `dev_art.rs`.
- **A kind `content add` can add:** add it to `SUPPORTED` in `formiga_dev/content.py`; it
  works for any type declared like the village pieces (an enum with `ALL`, `label` and
  `description`), and the toolkit's tests show the edits it makes.
- **A content check:** add it to the step it belongs to in `formiga_dev/audit.py`; mark a
  `Problem` with `level="warning"` when it should not fail the audit.

The toolkit's own checks: `python3 -m unittest discover devtools/tests`.

Every command maps one-to-one to a later MCP tool: `validate_formiga(steps)`,
`launch_formiga_scenario(name)`, `capture_formiga_view(scenario, window)`,
`inspect_formiga_item(kind, item)`, `audit_formiga_content()`, `check_formiga_art(kind)`,
`inspect_formiga_save(file)`, `find_formiga_uses(kind, item)`, `report_formiga_status()` and
`add_formiga_content(kind, name, about, like, write)`. Their `--json` documents are the results.
