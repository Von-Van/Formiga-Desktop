# Technical case study

## Problem

Formiga needed to feel like fauna living *among* desktop applications, while remaining unobtrusive,
deterministic, privacy-preserving, and visually coherent without premade creature sprite sheets.

## The unusual engineering choices

### Constrained generation instead of asset roulette

The generator resolves family-safe anatomy and a shared face grammar from named deterministic RNG
streams. Curated palettes and body-local pattern coordinates trade unrestricted variation for
readability and animation stability. The contact-sheet tool exercises uncurated seeds, while a
1,000-genome test renders every action and checks frame bounds.

### Animation contracts instead of per-creature sheets

Each family evaluates normalized poses against generated anchors, then rasterizes integer pixels.
This makes walk, rest, sleep, cursor, window, social, dragged, and landing actions reusable across
different proportions. Family-specific forelimbs use the same action contracts to reach, wave,
brace, balance, tuck, and play. Atlas baking moves procedural work out of the presentation loop.

### Expression without full-body atlas multiplication

The body atlas is gaze-free and records one face anchor per frame. A separate 16×16 atlas combines
eleven expressions, nine gaze directions, and three eyelid poses. Rendering one additional tiny quad
is cheaper than the prior three complete gaze-specific body copies, while deterministic blink timing
and drive-aware expression selection make the same face feel substantially more alive.

### Selective input without a global hook

A full-screen click-through overlay is excellent for rendering and terrible for direct manipulation.
Formiga keeps that overlay passive and creates tiny non-activating proxies shaped by current sprite
alpha. Only opaque creature pixels can begin a grab; keyboard focus and the rest of the desktop stay
with the underlying application. No Accessibility or global-input permission is needed.

### Application hiding without reading applications

True cross-platform per-application z-order is brittle. Formiga instead observes ordinary window
rectangles, associates selected owners with stable non-content identifiers, computes visible regions,
and discards covered creature pixels in the GPU shader. This produces the desired illusion without
screen capture, window titles, or content inspection.

## Reliability strategy

The simulation and RNG are platform-independent. Injected time accelerates the one-hour, one-week,
and clamped one-calendar-month colony schedule; save v11 migrates every v1–v10 colony without
regenerating identity; atomic writes retain a backup; removed supports or displays always resolve to
a safe habitat point. CI denies Clippy warnings and exercises both platform builds.

## Result

The portfolio project demonstrates procedural pixel art, deterministic simulation, native macOS and
Win32 interop, GPU composition, unusual input routing, privacy-oriented product decisions, save
migration, testing tools, and distributable desktop packaging in one compact Rust workspace.
