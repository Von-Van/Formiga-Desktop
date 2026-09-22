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
and clamped one-calendar-month colony schedule; the current save format, version 18, migrates every
colony an earlier release wrote, back to version 1, without regenerating identity; atomic writes
retain a backup; removed supports or displays always resolve to a safe habitat point. CI denies
Clippy warnings and exercises both platform builds.

## Growing it without breaking it

The first version was small. The decisions that mattered most since then are about changing a
large, stateful program that people leave running, without changing what they already have.

### Splitting a 10,000-line simulation without changing behaviour

By 0.57, `world.rs` had grown to 10,453 lines. In 0.58.0 it became one `World` type spread across
themed modules, each adding methods rather than owning state, so the simulation still has a single
object and a single tick. The split was checked rather than trusted: a differential harness ran
five seeds for 18,000 ticks each against 0.57.1 and compared the event streams and serialized saves
byte for byte.

### Optimizing only what is proven equivalent

0.58.5 made a tick on a busy desktop between a quarter and a third cheaper: one walk over the
windows where there had been three, reused buffers instead of per-tick clones, and a ledge search
moved from every tick to the moment it is read. None of it was accepted on inspection. A harness
driving only the public API hashed the save, the event stream, the bubbles, every attention pose,
and the interaction flags across six seeds and thirteen scenarios, and got 78 identical digests.
Coverage counters were printed beside each one, so that an identical digest could not simply mean
that neither build had done anything.

### Paying for a feature in the open

In 0.59.2 residents started strolling around their village, and a colony at home went from 0.98% to
2.90% CPU on the development machine. The cost was not hidden, and the feature was not dropped:
strolls tick at 10 Hz rather than 20, which brought it to 2.59%, and
[PERFORMANCE.md](PERFORMANCE.md) records the numbers, the method, and the cause next to the
budgets they miss. In 0.59.5 an instrumented build of the running app showed that nearly all of
the remaining cost was presenting frames, not simulating. The rule meant to draw strolls at 10 Hz
compared speeds against a round number that the liveliest companions' strolls exceeded, so one
lively resident held the whole village at 20. Deriving the threshold from the stroll's own speed
ceiling brought the same colony to 1.32%. Three other ideas were measured in the same pass and
dropped because the numbers showed nothing to gain.

### Letting the generator grow without changing anyone's creature

Every creature is recomputed from its seed and recipe each time it loads, so any change to what an
existing seed produces would quietly redraw creatures people already know. The generator is
therefore append-only. A new body plan takes a new index. Classic parts went into bytes that had
been reserved, and a recipe without them is still written byte for byte as before. When saved data
gains something an older build would silently drop, the save or code version moves, so the older
build refuses the file instead of misreading it.

## Result

The project demonstrates procedural pixel art, deterministic simulation, native macOS and
Win32 interop, GPU composition, unusual input routing, privacy-oriented product decisions, save
migration, testing tools, and distributable desktop packaging in one compact Rust workspace.
