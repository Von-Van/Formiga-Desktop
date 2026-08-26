# Formiga v0.1 architecture

Formiga is deliberately split at platform boundaries. `formiga-core` has no windowing or GPU
dependencies and owns deterministic colony simulation, current-state persistence, utility behavior,
calendar progression, habits, relationships, and the in-memory `WorldEvent` stream. `formiga-art`
turns resolved appearance genomes and normalized poses into 48×48 RGBA frames. The desktop host
provides safe geometry snapshots and draws those frames; it never decides creature behavior.

The runtime creates one transparent, click-through overlay per monitor. Simulation coordinates are
canonical virtual-desktop logical points; each renderer converts them to its monitor's physical
pixels, while art remains integer logical pixels.
The host polls the cursor at the simulation rate and generic window rectangles at an adaptive 1–4
Hz. It never requests titles, contents, keystrokes, screenshots, Accessibility, Screen Recording,
or Input Monitoring.

State is written as versioned JSON using temporary-file, flush, rename, and one backup. There is no
behavioral history store in v0.1. Future observation code can subscribe to `WorldEvent` without
changing the simulator or exposing the hidden genome to an analysis layer.

## Runtime cadence

- Fixed movement and drive update: 20 Hz
- Behavior selection: action boundaries only
- Cursor sampling: 20 Hz
- Window geometry: 4 Hz while active, 1 Hz at rest
- Presentation: up to 30 Hz while active, 4 Hz at rest, 2 Hz paused
- Display topology reconciliation: every 2 seconds
- Persistence: action/surface transitions, settings changes, and every 30 seconds

## Capability fallbacks

Window enumeration failure is an empty ledge list, not a fatal error. Creatures fall back to screen
floors. A removed display causes the core to relocate its creatures to the primary display. Exclusive
full-screen applications may cover Formiga; this is treated as dormancy.
