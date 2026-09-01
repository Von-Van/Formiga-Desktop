# Formiga roadmap

## v0.41 colony-rituals release

- Save version 8 persists only the next ritual UTC timestamp, last ritual kind, ordinal, and local
  hatch-day acknowledgement. The runtime plan and every event detail disappear after use.
- Deterministic ordinary rituals occur 12–48 hours apart and reuse existing actions for picnics,
  group naps, floor races, shelter gatherings, two-creature catch, group presentations, hatch days,
  quiet huddles, and late-night sleep piles.
- Overdue colonies wait for a safe behavior-selection boundary, perform at most one ritual, and
  never replay missed events. Interrupted plans retry two to six hours later.
- Local date/hour controls hatch-day and late-night eligibility with UTC fallback. Reduced motion
  excludes races; hidden, paused, dragged, tossed, or invalid-geometry colonies cancel safely.
- The multi-creature presentation patch automatically recovers stalled GPU surfaces and rebinds
  stale native monitor IDs, while keeping all creature records intact.

Release-page summary:

> Colonies now share rare picnics, naps, races, games, gatherings, presentations, hatch days, and
> late-night sleep piles using the existing low-cost behavior loop.

## v0.40 creature-bonds release

- Save version 7 replaces legacy one-way relationship floats with at most six canonical unordered
  pair records. Affinity, familiarity, playfulness, and avoidance consume four raw score bytes per
  pair and saturate safely.
- Deterministic v1–v6 migration preserves every creature's identity, resolved appearance,
  personality, custom name, birth time, compact memory, learned tendencies, routines, position,
  and settings. Reciprocal legacy floats are averaged into one shared bond.
- Five calm minutes together and completed social experiences project through ephemeral
  `WorldEvent`s; no proximity history, target path, or interaction log is persisted.
- Existing actions target preferred companions for following, shared sleep, discovery gifts, toy
  stealing, shelter-return greetings, climb watching, toss concern, and rare harmless squabbles.
  Runtime plans refresh moved targets and cancel when a companion becomes unavailable.
- Colony profiles show a read-only qualitative bond summary. Names remain the only editable
  creature field, new-trait thought bubbles are intentionally wordless, and the body atlas remains
  unchanged at 90 unique frames.

Release-page summary:

> Creatures now form compact, persistent bonds that influence who they follow, greet, sleep beside,
> play with, watch, and occasionally squabble with.

## v0.39 lived-experience release

- Ephemeral `WorldEvent`s project into typed counters, eight bounded learned tendencies, and a
  fixed twelve-slot routine table; no event history is persisted.
- Clicks within six logical points are pets, drag-out-and-back remains a drag, shelter petting does
  not dismiss the home, and `PetReaction` reuses greeting art.
- Repeated treatment, ledge time, riding, climbing, sleep, discoveries, play, home visits, and
  placement alter existing utility scores by at most ±0.35 and remain reversible.
- Deterministic cozy names and read-only Colony profiles surface up to three hysteretic descriptors,
  age, discoveries, favorite places, and closest companions. Only names can be edited.
- Profile badges persist until viewed; rare milestone bubbles use one temporary texture globally
  and are limited to once per creature per 12 active hours.
- Dangling handholds sit two source pixels higher, and the climb mantle now lifts inward continuously
  without switching through a hitching landing pose.
- Save version 6 migrates every v1–v5 colony, including the twelve strongest legacy habits.

Release-page summary:

> Creatures now remember how they are treated and gradually develop preferences for people, places,
> ledges, sleep, play, exploration, and home—without storing an activity log.

## v0.38.1 calendar-arrival patch

- Additional creatures arrive one hour, one week, and one calendar month after colony creation.
- Calendar-month arithmetic uses UTC time-of-day preservation and end-of-month clamping.
- Overdue creatures retain the existing 15-second reveal spacing and clock-rollback guard.
- Save version 5 persists birth timestamps and migrates v1-v4 colonies deterministically.
- The default colony remains capped at four; no learning, profile, relationship, or ecosystem work
  is included in this standalone patch.

## v0.38 passive creature life and toss update

- Four-frame family-specific climbing, dangling, screen-inspection, and discovery-presentation clips.
- Opportunistic geometry-only inspection landmarks plus low-frequency per-creature/per-colony timers
  that stop while Formiga is paused or hidden.
- Staged upward window traversal and climbing with unchanged downward hops and safe route cancellation.
- Eight deterministic, pre-baked temporary trinkets with no inventory, history, or runtime generation.
- Fast-release tossing with fixed-capacity cursor sampling, swept support tests, one soft bounce, and
  safe timeout recovery; slow drops and reduced-motion placement remain unchanged.
- The existing adaptive simulation, 20 FPS ceiling, window scan cadence, and v4 save version remain.

## v0.36 passive-activity pass

- Deterministic generated toys, snacks, cups, and bowls baked into creature atlases.
- Family-specific play, eating, and drinking gestures plus a six-frame sprint gait.
- Utility-driven passive actions with distinct energy, comfort, boredom, and arousal outcomes.
- No additional OS polling, particle system, runtime asset generation, or save migration.

## v0.36 assisted-update pass

- Optional once-daily and manual checks against public GitHub release metadata.
- Exact-platform MSI/DMG selection, bounded background download, and SHA-256 verification.
- Windows installer handoff and quit; macOS DMG handoff for manual app replacement.
- Release tags provide the app, package, and installer versions used by update comparison.
- No silent installation, telemetry, account, updater service, or extra simulation/rendering work.

## v0.31 background-efficiency release

- Full-screen applications cover the ecosystem by default on each affected display.
- Adaptive 4–20 Hz scheduling, cached native proxy state, dirty overlays, and clip-rate presentation keep
  background CPU/GPU work proportional to visible creature activity.
- No new permissions, monitoring, networking, or behavioral data storage are introduced.

## v0.25 expressive creature release

Implemented in this release:

- layered face atlas with state- and activity-aware expressions;
- two-dimensional cursor gaze and deterministic irregular blinking;
- family-specific gesture-capable forelimbs for every action;
- coordinated head-appendage and tail secondary motion;
- pre-baked temporary activity effects;
- deterministic v1/v2/v3→v4 save migration;
- expression, gesture, animation, and contact-sheet art tools;
- persistent bottom-corner homes, generated shelter art, and drag-to-dismiss cooldown behavior;
- v0.2 dragging, habitats, window reactions, and application occlusion retained.

Release validation still requires completing every real-device row in `TEST_MATRIX.md`, recording
measured energy use in `PERFORMANCE.md`, and signing/notarizing builds when credentials exist.

## Progressive releases after v0.41

- **v0.42 Desktop Topology:** derive bounded runtime-only islands, exposed corners, calm moving
  platforms, and cursor invitations from geometry.
- **v0.43 Window Routes:** derive bounded four-hop window constructions and short-lived narrow-gap
  squeeze routes that cancel as soon as supporting geometry changes.
- **v0.44 Inhabited Ecosystem:** add at most eight deterministic static colony objects with semantic
  utility contributions and no physics.
- **v0.45 Growing Home:** add at most six deterministic shelter decorations while preserving one
  cached 64×64 shelter texture and one normal draw call.
- **v0.46 Offline Seed Sharing:** encode original generation and a full 256-bit origin seed in a
  checksummed, versioned, case-insensitive Base32 code with no server.
- **v0.47 Creature Cards:** export a deterministic on-demand 960×600 PNG with no persistent export
  allocation or hidden metadata.

Objects, decorations, topology, seed import, and card export remain out of scope for v0.41 and
retain their independent release gates.
