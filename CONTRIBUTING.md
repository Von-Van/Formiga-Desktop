# Contributing and making your own version

Formiga is a personal project rather than an openly contributed community project. I am
not currently accepting pull requests, feature submissions, or requests to maintain changes in this
repository. Unsolicited contributions may be closed without review.

You are warmly encouraged to use the code as a starting point for your own experiments, creatures,
and desktop ecosystems. Fork it, remix it, improve it, or take it in a completely different direction
under the terms of the [MIT License](LICENSE).

If you publish a version of your own, please:

- Preserve the license and copyright notice.
- Give the project a distinct name and icon so people do not mistake it for an official Formiga
  release.
- Clearly describe it as an independent fork or derivative.

## Working on your version

### Setting up

- Install [rustup](https://rustup.rs). The pinned toolchain, Rust 1.97.1 with rustfmt and Clippy,
  installs itself from `rust-toolchain.toml` the first time you run `cargo`.
- On macOS 14 or later, install the Xcode Command Line Tools (`xcode-select --install`). On Windows
  10 or 11, install the Visual Studio C++ build tools. Those are the only supported platforms.
- Packaging needs more than this (WiX 4 on Windows, for example); [docs/BUILD.md](docs/BUILD.md)
  has the details, and you will not need it to work on the code.

Run the app against a scratch colony while you work:

```sh
FORMIGA_DATA_DIR=/tmp/formiga-dev cargo run -p formiga-desktop
```

`FORMIGA_DATA_DIR` replaces the application-data directory outright. The colony, its backup, the
logs, and the update preferences all go there, so an experiment, a migration, or a reset cannot
touch a colony you actually live with. Without it, a development build uses the same colony as an
installed copy. The log is `logs/formiga.log` in whichever directory is in use.

### Finding your way around

Start with [the orientation at the top of ARCHITECTURE.md](docs/ARCHITECTURE.md#start-here). It
covers the four crates, how the app starts, the main loop, where state lives, and the order in
which a feature usually touches the layers. The short version:

- `formiga-core`: the simulation, colony, and save format. No GUI or GPU code, and no view of
  the desktop beyond the snapshot it is handed.
- `formiga-art`: generation and rasterisation of everything drawn.
- `formiga-desktop`: the app, meaning the event loop, overlays, input, settings, tray, updater, and
  OS adapters.
- `formiga-tools`: a command-line companion. Run `cargo run -p formiga-tools` with no arguments
  for its subcommands: review sheets, documentation images, `simulate` for an accelerated-time
  colony, and `tick-bench` for the simulation's own cost.

### Checks

Before sharing a build, run the same gate CI runs on macOS and Windows for every push to `main`
and every pull request:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Windows cannot be fully checked from a Mac; see [docs/BUILD.md](docs/BUILD.md) for why and what
to do instead.

### Changing things safely

- **Persistence.** A new saved field needs a `SAVE_VERSION` bump, an explicit migration, and
  migration and round-trip tests. Every save an earlier release wrote must still load.
- **Generation.** Keep it append-only, so that existing seeds, recipes, and seed codes resolve
  exactly as before. [docs/GENERATION.md](docs/GENERATION.md#extending-safely) has the rules.
  Regenerate the contact sheet and keep the 1,000-genome render test passing.
- **Art and animation.** Look at the relevant review sheet rather than trusting the numbers, and
  keep the atlas inside the texture budget its test enforces.
- **Platform integration.** Check against the relevant cases in `docs/TEST_MATRIX.md`.
- **Performance.** Compare `tick-bench` before and after in one sitting, with a build of the
  previous commit alongside; timings drift between sittings. For whole-process numbers, follow
  [docs/PERFORMANCE.md](docs/PERFORMANCE.md).
- **Privacy.** Nothing may read beyond the boundary in [docs/PRIVACY.md](docs/PRIVACY.md). If what
  is stored changes, that document changes with it.
- **Documentation.** Add or update the section of `docs/ARCHITECTURE.md` your change belongs to,
  and add a CHANGELOG entry.

### Writing behavior scenarios

The simulation is one `World` split across themed modules: `world.rs` holds the type, `new`,
`from_save`, and `tick`, and everything else lives in `world/<theme>.rs` — `offers.rs`, `home.rs`,
`spacing.rs`, `visitors.rs`, `discovery.rs`, and the rest — each adding methods to that same type.
The tests mirror it: `world/tests/<theme>.rs`, with the shared desktop fixtures and colony builders
in `world/tests/mod.rs`. Put a new scenario in the file named after what it is about, and reach for
the fixtures in `mod.rs` before writing another one.

The behavior tests drive `World::tick` over synthetic desktops in 50 ms steps. A few things about
the simulation make a scenario quietly test the wrong thing:

- A play encounter only forms between two creatures on the same surface, within four points of the
  same height, and 46–180 points apart, scaled by creature size. Creatures placed closer never pair.
- The encounter rules in `play.rs` are a priority list, so a fixture meant for one game can be
  claimed by an earlier one. Check which kind actually started before asserting anything about it.
- Nothing new begins while an attention scene, a colony cooldown, pending geometry signals, or a
  cursor cue is live. Adding a window to a fixture desktop starts a scene of its own: tick for a few
  seconds, then call `clear_attention()` and reset positions before the part being tested.
- Topology routes exist only across a 10–28 point gap, or between windows overlapping by at least
  48 points with a 36–360 point change in height. A 60-point gap can be jumped but has no route.
- Short walks refuse a destination more than one stride away, one that is not exposed, or one
  another creature has taken or reserved. Use `step_toward` for anything further.
- A creature in the air still holds an attention plan, a journey. Test that its plan is not a
  `Role::Play`, not that it has no plan.
- Window `z_order` is a `u32`, and only a lower value is in front of a creature's support.
- Watchers look up after a staggered delay and a short notice, so assert across a span of ticks
  rather than on one.
- No expected value may depend on the local timezone: the schedule reads the local offset, which is
  not UTC on most developer machines. Use a routine in force at every hour, or pass explicit offsets.
- Two creatures left standing on one another will be separated by `world/spacing.rs` at the end of
  an ordinary tick, so a fixture that places them on top of each other may not stay that way. Place
  them a frame apart, or assert across the grace period rather than on one tick.
- Only the review sheets show whether a pose reads. They are dense; see `docs/TEST_MATRIX.md` for how
  to generate and read them.
- Run the desktop against a scratch colony — `FORMIGA_DATA_DIR=/tmp/formiga-dev cargo run -p
  formiga-desktop` — so an experiment, a migration, or a reset cannot touch your real one. See
  `docs/BUILD.md`.

Formiga intentionally avoids telemetry, global input hooks, Accessibility, Screen Recording, Input
Monitoring, administrator requirements, and application-content inspection. Forks are encouraged
to preserve those privacy-friendly defaults and to explain clearly if they choose a different model.
