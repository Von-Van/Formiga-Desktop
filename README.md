# Formiga

Formiga is a local-only desktop ecosystem for macOS and Windows. A seeded pixel-art creature
lives in a transparent, click-through desktop layer, reacts to the cursor and ordinary application
windows, and eventually develops a small family colony.

## Workspace

- `formiga-core` — deterministic simulation, behavior, colony timing, and persistence models.
- `formiga-art` — procedural genomes, pixel rasterization, and family-specific animation poses.
- `formiga-desktop` — the native overlay application and OS adapters.
- `formiga-tools` — contact sheets, accelerated-time simulations, and art diagnostics.

## Development

```sh
cargo test --workspace
cargo run -p formiga-tools -- contact-sheet --output contact-sheet.png
cargo run -p formiga-tools -- animation-preview --seed 17 --output animation-preview.png
cargo run -p formiga-desktop
```

See [the build guide](docs/BUILD.md), [architecture](docs/ARCHITECTURE.md), and
[privacy model](docs/PRIVACY.md) for packaging and runtime details.

The desktop app stores only its current colony state. It does not collect behavioral history,
window titles, keystrokes, screenshots, or analytics.
