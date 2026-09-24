# Formiga · v0.61.0

<p align="center"><img src="packaging/shared/Formiga.png" width="128" alt="Formiga mascot app icon"></p>

![A small Formiga colony living among desktop windows](docs/assets/hero.png)

Formiga is a desktop companion for macOS and Windows. Small creatures, each one drawn procedurally
from a seed of its own, live in a transparent layer over your desktop: they walk along the tops of
your windows, react to your cursor, play with one another, and over a few weeks grow into a colony
of up to six with a village of its own in a corner of the screen. Everything runs locally, it needs
no special permissions, and it is built to be left running all day.

**[Download](#download-and-run)** · **[What it does](#what-it-does)** · **[Status](#status)** ·
**[Build from source](#build-from-source)** · **[How it works](#how-it-works)** ·
**[Changelog](CHANGELOG.md)**

![Procedural demonstration of generation, dragging, habitat zones, occlusion, and colony growth](docs/assets/formiga-demo.gif)

## Why it exists

A desktop companion is something you share a screen with for hours at a time, so it only works if
it never gets in the way of the work on that screen. Formiga is an attempt at one that earns its
place: small, alive enough to be worth a glance, with a history that builds up the longer it runs,
and costing the rest of the computer as close to nothing as it can.

## Design priorities

These are constraints rather than features. Each one rules things out, and the code is organised
around them. [The architecture notes](docs/ARCHITECTURE.md#design-constraints) explain what each
one costs.

- **Out of the way.** The overlay never takes focus and passes every click through to the window
  underneath, except on a creature's own opaque pixels. Dragging a creature does not activate
  Formiga or move keyboard focus.
- **Light.** The app sleeps between ticks instead of polling. It ticks at most twenty times a
  second while something moves, and redraws a still colony only as often as its animations need.
  It is measured against budgets of under 1% CPU for a resting colony, under 3% with four
  creatures moving, under 100 MB resident memory, and at most twenty frames a second. On the Mac
  it is developed on, a colony of five with its village out and residents strolling averages about
  1.3% of one core. [PERFORMANCE.md](docs/PERFORMANCE.md) has the method, the measurements, and
  where they fall short.
- **Private.** Formiga sees window rectangles, the cursor, and idle time, and nothing else: no
  window contents, no titles, no screen capture. It asks for no Accessibility, Screen Recording,
  or Input Monitoring permission and no administrator rights, and it collects no telemetry. Its
  only network request is an optional once-a-day check for a new release.
- **Generated, not drawn.** There are no premade sprites. A 256-bit seed resolves a creature's body
  plan, parts, palette, face, and personality, and the art crate rasterises every animation frame
  into a texture atlas when the creature loads. The same seed gives the same creature on every
  machine.
- **Durable.** A colony is meant to last. The save is written atomically with a backup, every save
  format an earlier release wrote is migrated forward on load, and a creature can be shared as a
  checksummed seed code that decodes identically in later releases.

## What it does

- **Creatures with a life of their own.** Five body plans with generated ears, tails, markings, and
  palettes, twelve expressions, a gaze that follows whatever has caught their interest, and a
  personality of their own that how they are treated slowly nudges.
  [One hundred uncurated seeds](docs/assets/contact-sheet.png) show the range.
- **Your desktop as terrain.** They climb onto windows, ride them when they move, hop between them,
  squeeze through narrow gaps, watch a risky jump from the ledge, and notice the cursor — up on the
  windows about half the time they spend roaming, and down on the floor the rest.
- **Handled gently.** Click to pet, drag to move, and a quick release tosses one with a soft bounce.
  Right-click (Control-click on macOS) for a small menu: offer a snack or a toy, send everyone
  home, or open a profile. Creatures answer in pictures, not words.
- **A colony.** Friendships form, games start (chase, tag, hide and seek, a race across your
  windows), and now and then the whole colony gathers for a picnic, a nap, or a late-night sleep
  pile.
- **A village.** When the colony goes home, a village of up to six houses stands in a corner of the
  screen, with keepsake trees, gardens that grow, and visitors who drop by, and the colony gets on
  with village life: tending the gardens, seeing to its houses, napping indoors, sitting on the
  roofs. You can arrange it right in its picture on the Home page, and dress every house.
- **Keepsakes.** A journal of small moments, a hundred and sixty keepsakes to find and collect, some
  of them to wear, and a guest book. Creatures, the colony, and postcards of the village can be
  exported as images and animated GIFs.
- **Your own creatures.** Preview new ones, reinterpret a PNG or JPEG you already have (a local
  colour-and-shape reading, not an AI model), share any creature as a seed code, or invite a
  friend's creature for a day.
- **Settings that respect the desktop.** Regions where creatures may or may not go, apps allowed to
  cover them, reduced motion, pause and hide, and Work and Relax presets on a weekly routine.

[The guide](docs/GUIDE.md) covers all of it in detail, with pictures.

![Every action and gesture pose on three reference creatures](docs/assets/gesture-sheet.png)

## Download and run

Open the [Releases page](https://github.com/Von-Van/Formiga-Desktop/releases) and pick the file for
your computer. No terminal or development tools are required.

- **macOS 14+** — download the `.dmg`, open it, and drag Formiga to Applications.
- **Windows 10/11** — download the `.msi` and run it. It adds normal Desktop and Start-menu
  shortcuts.

Downloads are named after their release, for example `Formiga-0.61.0-macOS-universal.dmg`. Each one
ships with a matching `.sha256` file, so keep the original filename if you want to verify it.

These builds are not code-signed yet, so the first launch needs one extra step: on macOS,
Control-click the app and choose **Open**; on Windows, choose **More info → Run anyway**. Formiga
will never ask you to turn off any operating-system security feature.

Settings open automatically the first time you launch, with a short tour you can skip. After
that, the menu-bar or tray icon offers Show/Hide, Pause, Gather Creatures, Check for Updates,
Settings, and Quit. Formiga never installs an update on its own: when one is available it verifies
the download's SHA-256 and hands the installer to your operating system.

## New in 0.61.0

The houses are bigger. Every house in the village is drawn a quarter larger beside your
companions, so each one reads plainly as somebody's home now that the village can be arranged, and
the village is about a tenth wider to hold them. The keepsake trees at either end stand a little in
over the house beside them and are drawn in front of it where they meet, so the row sits tucked in
between its two trees. The colony portrait and the postcards draw the houses bigger too.

A new colony is shown round. The first time Formiga opens, a tour walks through the desktop —
petting a companion, carrying one, its right-click menu, and the icon in the menu bar or tray —
and notices when you try each one, then through what every page of the settings window is for,
turning the pages itself and outlining each part as it talks about it. Skip it whenever you like,
and take it again any time from Preferences.

Everything earlier releases brought is described in [the release notes](docs/RELEASE_NOTES.md),
and every change is itemised in [the changelog](CHANGELOG.md).

## Status

Formiga is a working preview. It runs for days at a time on the Mac it is developed on, but it is
still at 0.x, its releases are published as unsigned prereleases, and the manual checks on real
hardware are not done yet.

| Area | Status |
|---|---|
| Behaviour, art, the village, the settings window | **Preview**: working and shipped, still changing between releases |
| Save format and migrations | **Stable**: each release reads every save an earlier one wrote, and CI tests the migrations |
| Seed codes | **Stable**: versioned and checksummed, and codes from earlier releases decode exactly as before |
| Privacy model | **Stable**: what is read, stored, and sent is fixed by design and listed in [PRIVACY.md](docs/PRIVACY.md) |
| macOS 14+ | Tested in CI on Apple silicon and packaged as a universal app. Used day to day on Apple silicon |
| Windows 10/11 x64 | Built, tested, and packaged in CI. **Not yet checked by hand on Windows hardware** |
| Performance budgets | Measured on one Mac only. **Not yet measured on Windows** |
| [Manual release checks](docs/TEST_MATRIX.md#manual-release-gates) | **Not yet recorded** for any platform |
| Code signing | **Not signed.** Gatekeeper and SmartScreen warn on first launch |

## Build from source

You need [rustup](https://rustup.rs). The pinned toolchain (Rust 1.97.1, with rustfmt and Clippy)
installs itself from `rust-toolchain.toml` the first time you build. On macOS you also need the
Xcode Command Line Tools, and on Windows the Visual Studio C++ build tools.

```sh
git clone https://github.com/Von-Van/Formiga-Desktop.git
cd Formiga-Desktop
FORMIGA_DATA_DIR=/tmp/formiga-dev cargo run -p formiga-desktop
```

`FORMIGA_DATA_DIR` points the build at a scratch colony, so nothing you try can touch a colony you
already have. Leave it out to use the real one. (In PowerShell, set `$env:FORMIGA_DATA_DIR` first.)
Before sending a change anywhere, run the same checks CI runs:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

[docs/BUILD.md](docs/BUILD.md) covers packaging, the performance tools, and regenerating every
image in this repository. [CONTRIBUTING.md](CONTRIBUTING.md) is the guide to finding your way
around the code.

## How it works

```text
 macOS / Windows adapters ── window rectangles, cursor, idle time, displays
            │
            ▼  DesktopSnapshot, once per tick
 formiga-core ── deterministic simulation: drives → chosen actions → movement over surfaces
            │    owns the colony (the save file) and emits WorldEvents
            ▼
 formiga-art ── seeded genome → rig → poses → texture atlases, baked once per creature
            │
            ▼
 formiga-desktop ── one transparent overlay per display, drawn with wgpu; tiny hit-test windows
                    over each creature; tray menu; settings window (egui); updater
```

The workspace is four crates with dependencies that run one way:

| Crate | What it holds |
|---|---|
| [`formiga-core`](crates/formiga-core) | The simulation, behaviour, colony, habitat, save format, and migrations. No GUI or GPU code |
| [`formiga-art`](crates/formiga-art) | Procedural generation and rasterisation: creatures, animation atlases, houses, cards, stickers |
| [`formiga-desktop`](crates/formiga-desktop) | The app: event loop, overlays and GPU rendering, input, tray, settings, OS adapters, updater |
| [`formiga-tools`](crates/formiga-tools) | A command-line companion: review sheets, documentation images, benchmarks, simulated months |

The simulation never reads the desktop directly. Each tick the desktop crate hands it a snapshot of
what is on screen, and it hands back events. That boundary keeps the core deterministic and
testable on any machine, and it is why several hundred behaviour tests run on synthetic desktops in
CI. [ARCHITECTURE.md](docs/ARCHITECTURE.md) starts with an orientation (how the app starts, where
state lives, how to add a feature) before the detailed reference.

## How Formiga is built

Formiga is a one-person project developed with AI coding assistants, OpenAI GPT and Anthropic
Claude models. They do much of the hands-on work: writing and refactoring code, debugging,
reviewing changes, drafting documentation, prototyping, and trying alternative approaches before
one is chosen. Many commits carry a `Co-Authored-By` trailer for the assistant that worked on them.

The direction is human. What Formiga is for, which features it has, the design priorities above,
the architecture, the art direction, and the tradeoffs between them are all the owner's decisions.
Every change is reviewed, then accepted or sent back, before it ships. The same bar applies
whoever wrote the code:

- **One gate for every change.** Formatting, Clippy with warnings denied, and the full test suite
  run on macOS and on Windows before a release is tagged.
- **Measured, not assumed.** Performance claims in [PERFORMANCE.md](docs/PERFORMANCE.md) name the
  machine, the method, and the build they were compared against, including the regressions.
- **Refactors prove they changed nothing.** When the simulation was split into modules, a
  differential harness replayed seeded sessions against the previous release and compared the
  event streams and saves byte for byte.
- **Existing colonies come first.** Older saves migrate forward, and generation changes are checked
  against codes and recipes from earlier releases, so a creature someone already has stays the
  creature they know.

[The case study](docs/CASE_STUDY.md) walks through the engineering decisions behind all this.

## Privacy

The colony, its behaviour, and its history stay on your computer. Formiga uses window rectangles and
motion only — never window contents, never screen capture. The optional release check makes at most
one request per day and sends nothing about you or your colony. The daily check can be turned off
under **Settings → About**. [The privacy model](docs/PRIVACY.md) lists exactly what is read and
stored.

## Documentation

| Document | For |
|---|---|
| [Guide](docs/GUIDE.md) | Everything Formiga does and every setting, with pictures |
| [Release notes](docs/RELEASE_NOTES.md) · [Changelog](CHANGELOG.md) | What each release changed, in prose and itemised |
| [Architecture](docs/ARCHITECTURE.md) | How the pieces fit, where state lives, and how to add a feature |
| [Case study](docs/CASE_STUDY.md) | The unusual engineering decisions and why they were made |
| [Creature generation](docs/GENERATION.md) | Recipes, classic parts, and how to extend generation safely |
| [Build](docs/BUILD.md) | Building, packaging, performance tools, and regenerating images |
| [Performance](docs/PERFORMANCE.md) | Budgets, method, and every measurement so far |
| [Privacy](docs/PRIVACY.md) | Exactly what is observed, stored, and sent |
| [Test matrix](docs/TEST_MATRIX.md) | What is automated and what still needs a person at a real desktop |
| [Contributing](CONTRIBUTING.md) | Finding your way around the code, and making a fork of your own |

## License

MIT. See [LICENSE](LICENSE). Formiga is a personal project and does not take outside pull requests,
but forks are welcome. [CONTRIBUTING.md](CONTRIBUTING.md) explains how to make one your own.
