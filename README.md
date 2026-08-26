# Formiga

![A small Formiga colony living among desktop windows](docs/assets/hero.png)

Formiga is a local-only desktop ecosystem for macOS and Windows. Seeded procedural creatures live
in transparent desktop overlays, develop simple habits, perch on ordinary windows, react to the
cursor, and eventually grow into a four-creature colony.

![Procedural demonstration of generation, dragging, habitat zones, occlusion, and colony growth](docs/assets/formiga-demo.gif)

The v0.25 expressive-art release adds a readable emotional and physical performance to the existing
desktop interactions:

- Read state and activity through eleven expressions, two-dimensional gaze, eyelids, and irregular blinks.
- See family-specific pseudopods, mitten hands, or front paws gesture in every activity.
- Notice tiny pre-baked sleep, investigation, play, greeting, and startle effects without a particle loop.
- Drag a creature by its opaque pixels without blocking unrelated desktop clicks.
- Limit the colony to presets or up to 32 allowed/excluded rectangles across displays.
- Let selected applications visually cover creatures without inspecting window content.
- Watch creatures hop to reachable ledges, patrol window tops, and startle when nearby windows move.
- Configure visibility, motion, ledges, cursor behavior, habitat, and applications in a native UI.
- Load v0.1 colonies through an identity-preserving save migration.

## Why it is technically interesting

Formiga does not choose from premade pets. A 256-bit seed resolves a constrained genome, family rig,
palette, markings, face grammar, forelimbs, personality, and independent RNG streams. Normalized
authored poses are evaluated against generated anatomy and rasterized to deterministic 48×48 body
atlases. A compact layered face atlas supplies expressions, nine gaze directions, and three eyelid
states without duplicating the full body texture.

The desktop host uses one click-through GPU overlay per monitor. Tiny native proxy windows expose
only a creature's current alpha mask for dragging. Application hiding is a local GPU occlusion mask
computed from safe window rectangles and stable application identities—not screen capture or true
per-app OS z-order manipulation.

![One hundred uncurated deterministic creature seeds](docs/assets/contact-sheet.png)

![Expression grammar across blob, hopper, and soft-quadruped families](docs/assets/expression-sheet.png)

![Activity-coordinated gestures across every action](docs/assets/gesture-sheet.png)

## Workspace

- `formiga-core` — deterministic simulation, behavior, habitats, colony timing, save migration.
- `formiga-art` — genomes, procedural rasterization, rigs, poses, and animation atlases.
- `formiga-desktop` — overlays, interaction proxies, settings, tray, GPU rendering, OS adapters.
- `formiga-tools` — contact sheets, animation diagnostics, demo assets, accelerated-time simulation.

## Development

Rust 1.97.1 is pinned.

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p formiga-desktop
```

Regenerate the portfolio assets with:

```sh
cargo run -p formiga-tools -- portfolio-hero
cargo run -p formiga-tools -- portfolio-demo
cargo run -p formiga-tools -- contact-sheet --output docs/assets/contact-sheet.png
cargo run -p formiga-tools -- animation-preview --seed 17 --output docs/assets/animation-preview.png
cargo run -p formiga-tools -- expression-sheet --output docs/assets/expression-sheet.png
cargo run -p formiga-tools -- gesture-sheet --output docs/assets/gesture-sheet.png
```

## Status

Formiga v0.25 is a portfolio preview, not a signed consumer release. macOS 14+ and Windows 10/11 x64
are the supported targets. CI builds both; downloadable previews are intentionally unsigned until
Developer ID and Authenticode credentials are available.

See the [case study](docs/CASE_STUDY.md), [architecture](docs/ARCHITECTURE.md),
[build guide](docs/BUILD.md), [privacy model](docs/PRIVACY.md), and
[test matrix](docs/TEST_MATRIX.md).
