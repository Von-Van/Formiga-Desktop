# Formiga v0.25 architecture

Formiga separates platform observation, deterministic simulation, procedural art, and presentation.
`formiga-core` contains no GUI or GPU code; OS adapters cannot decide creature behavior, and the art
crate cannot read desktop state.

```text
macOS / Windows adapters
  monitors · cursor · idle time · safe window rectangles · application identity
                              │
                              ▼
                       DesktopSnapshot
                              │
                              ▼
 formiga-core: drives → utility selection → action state machines → surfaces
          │                         │
          │                         └── in-memory WorldEvent stream
          ▼
 formiga-art: genome → family rig → normalized pose → deterministic atlas
          │
          ▼
 formiga-desktop: per-monitor overlays + interaction proxies + occlusion uniforms
```

## Procedural identity and animation

A 256-bit colony seed derives named ChaCha streams for appearance, personality, markings, animation
flavor, runtime decisions, and each mini. Resolved genomes are stored in the save so future generator
changes cannot silently redesign an existing creature.

Blob, hopper, and soft-quadruped rigs share a stable two-eye face grammar. Every body frame records a
face anchor and family-specific forelimb targets. Authored clips manipulate those anchors, squash,
planted contacts, limb gestures, and secondary tail/head-appendage motion. Markings and temporary
activity effects remain body-local and are rasterized at integer coordinates.

The renderer caches one gaze-free 48×48 body atlas and one 16×16 layered face atlas per creature.
The face atlas contains eleven expressions, nine gaze directions, and three eyelid states. Runtime
work is limited to selecting two texture slots and drawing two nearest-filtered quads; expression
changes add no simulation, particle, or desktop-polling loop. Together the cached textures remain
below 1 MB per creature.

## Desktop composition

Each monitor receives a transparent, always-on-top rendering overlay. It is non-activating and
click-through during normal use. Coordinates remain virtual-desktop logical points until the final
renderer conversion to physical pixels.

Direct manipulation uses one small proxy per visible creature. A pre-baked one-bit frame mask shapes
the Windows proxy and controls near-creature hit testing on macOS. The proxy preserves the grab
offset, captures through release, and sends only drag commands to the simulation. Active drags are
runtime-only; cancellation restores the last safe surface.

Habitat policies are the union of allowed rectangles (or a preset) minus excluded rectangles.
Rectangles are normalized against privacy-safe display identities, so DPI and resolution changes do
not invalidate them. Window-ledges are clipped to the same reachable habitat.

Selected applications are represented by bundle ID, AUMID, or a SHA-256 executable-path digest. The
renderer subtracts higher windows from each selected window's visible rectangles and sends up to 64
monitor-local rectangles to a fragment-shader uniform. Covered pixels are discarded; a dragged
creature opts out per vertex.

## Runtime cadence

- Simulation and cursor sampling: 20 Hz.
- Window geometry: 4 Hz while active, 1 Hz at rest.
- Behavior selection: action boundaries, capped at 2 Hz.
- Presentation: up to 30 Hz active, 4 Hz resting, 2 Hz paused.
- Display reconciliation: every 2 seconds.
- Persistence: transitions, settings changes, and every 30 seconds.

State uses a versioned JSON file written by temporary-file, flush, atomic replace, and one backup.
Version 3 migrates v1 habitat settings and deterministically resolves v2 face, forelimb, and effect
genes without replacing colony identity. There is deliberately no history database or telemetry layer.
