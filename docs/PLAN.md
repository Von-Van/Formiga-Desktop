# Formiga roadmap

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

## Progressive releases after v0.39

- **v0.40 Creature Bonds:** replace legacy float maps with at most six unordered four-score pair
  records and target existing follow, sleep, presentation, play, greeting, inspection, and reaction
  actions at companions.
- **v0.41 Colony Rituals:** schedule one rare deterministic picnic, group nap, race, shelter
  gathering, catch game, presentation, hatch day, quiet huddle, or late-night sleep pile without a
  new loop or missed-event replay.
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

Relationship values, objects, decorations, rituals, topology, seed import, and card export remain
out of scope for v0.39 and retain their independent release gates.
