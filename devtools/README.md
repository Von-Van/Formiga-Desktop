# Formiga developer toolkit

One command, `formiga`, for the things a change to Formiga needs every time: check it, open the
app in a known state, and look at the result. It is for people and for coding agents alike: every
command prints a short summary, or with `--json` one document an agent can read without parsing
prose.

```sh
devtools/formiga validate              # did this change break Formiga?
devtools/formiga scenario list         # the known states
devtools/formiga scenario mature-colony
devtools/formiga capture hill-green    # .dev/captures/hill-green.png
```

On Windows: `py devtools\formiga.py …`. It needs Python 3.9 or newer and nothing beyond its
standard library, plus the Rust toolchain Formiga already needs. Everything it makes goes in
`.dev/`, which git ignores.

The toolkit encodes how Formiga works rather than re-implementing it. Colonies are grown and
checked by `formiga-core` itself, through `formiga-tools dev` (`crates/formiga-tools/src/dev.rs`).
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
| `tests` | `cargo test` on every crate |
| `soak` | the nightly soak, short: 120 randomized colonies living a simulated day, their files damaged and repaired |
| `hill`, `home` | with `--expansions`: Formiga Hill's and Formiga Home's own tests, built on *this* checkout's Desktop crates rather than the release they're tied to. This is how a Desktop change that breaks an expansion shows up before release. |

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
`FORMIGA_HILL_REPO` / `FORMIGA_HOME_REPO`). They are built on this checkout's Desktop crates, so a
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

## Adding to it

- **A Desktop colony state:** add a `Fixture` in `crates/formiga-tools/src/dev.rs`, then a
  `Scenario` that names it in `formiga_dev/scenarios.py`. `formiga validate --step fixtures` holds
  every fixture to the save rules.
- **An expansion state:** add a `Scenario` with that app's own arguments and render option.
- **A validation step:** add a function to `formiga_dev/validate.py` that returns a `Step` with
  `Problem`s.

The toolkit's own checks: `python3 -m unittest discover devtools/tests`.

Every command maps one-to-one to a later MCP tool: `validate_formiga(steps)`,
`launch_formiga_scenario(name)`, `capture_formiga_view(scenario, window)`. Their `--json`
documents are the results.
