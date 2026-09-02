# Formiga roadmap

## v0.50 reference-guided creature and colony-management release

- PNG and JPEG references are decoded locally under strict byte, dimension, pixel, and candidate
  limits. A fixed search compares privacy-safe color and shape features with ordinary generated
  genomes, then discards the source and all extracted features.
- The Creature Studio previews reference-matched or random full-size creatures before an explicit
  add or replacement. Accepted creatures use ordinary seeds and fresh compact histories.
- A four-member colony permits at most three full-size adults. Keep toggles protect individuals
  during bulk regeneration; explicit remove and replace actions require confirmation or an unkept
  selection, and the final adult cannot be removed.
- Minis retain a parent ID, are limited to two per adult, and rebalance evenly when possible. With
  three adults and one mini, the oldest adult receives it. The one-month calendar arrival becomes a
  full-size adult when capacity permits.
- Save version 11 migrates every v1–v10 colony without removing or redesigning any existing
  creature. Legacy colonies that exceed a new role preference are grandfathered intact.

Release-page summary:

> Create, keep, replace, or remove colony members—and privately approximate a favorite character
> from a local image—while Formiga preserves every existing colony through a lossless migration.

## v0.47 exportable-creature-card release

- Colony profiles export a deterministic 960×600 PNG only after the native save dialog returns a
  destination. Cancelling does not render or write anything.
- The shareable pixel-art keepsake uses the creature's generated sprite and palette, family, up to
  three learned descriptors, custom name, UTC birth month/year, and an abbreviated seed glimpse.
- A stepped paper frame, moonlit desktop scene, palette-derived accents, motif stamp, and colony
  number keep the card visually playful and recognizably Formiga without adding image assets to the
  running application.
- Card-only fonts, canvases, and PNG buffers are temporary. The PNG contains no full seed, compact
  state, device data, hidden metadata, path, or screen information. Save version 10 is unchanged.

Release-page summary:

> Creatures can now be exported as cozy illustrated cards showing their appearance, name, family,
> learned personality, arrival date, and an abbreviated seed—ready to share without exposing their
> private colony state.

## v0.46 exact-offline-seed-sharing release

- A case-insensitive grouped Crockford Base32 code carries format version 1, source generation, the
  complete 256-bit `CreatureOrigin` seed, and a four-byte checksum.
- All four source generations reproduce innate appearance, personality, scale, ID, and behavior
  seed byte-for-byte. Custom names, memories, bonds, objects, and shelter state are excluded.
- Profile export copies the code. General-tab import validates every field offline and requires an
  explicit replacement acknowledgement before creating a fresh colony-order-zero creature.
- Import assigns a fresh birth and compact history while deriving a distinct deterministic colony
  seed for future companions. Save version 10 and all v1–v10 colonies remain unchanged.

Release-page summary:

> Any creature can now be shared and recreated exactly through a checksummed, fully offline seed
> code—without accounts or servers.

## v0.45 growing-home release

- Save version 10 stores at most six unique leaf, banner, stone, flower, lamp, and roof-ornament
  choices plus one future timestamp and ordinal. Deterministic v1–v9 migration does not replace or
  reset creatures, objects, bonds, rituals, names, memories, or the existing shelter.
- One decoration arrives every four to nine UTC days, never more than one after downtime. Its
  deterministic score reflects dominant compact memories, bonds, the last ritual, and colony
  objects.
- Decorations are rasterized into the existing 64×64 shelter texture only when state changes. They
  add no editor, physics, normal draw call, or idle regeneration.

Release-page summary:

> Shelters now grow deterministic decorations that reflect each colony's habits and history, while
> remaining a single cached 64×64 texture.

## v0.44 inhabited-ecosystem release

- Save version 9 stores at most eight static objects with type, stable ID, display key, normalized
  position, and semantic role. Deterministic v1–v8 migration does not replace or reset creatures.
- Pillows, toys, plants, blankets, paper scraps, pebbles, lamps, and cups arrive one at a time on a
  deterministic three-to-seven-day UTC schedule, with no catch-up replay after downtime.
- Invalid positions snap to a safe habitat floor or shelter area. Objects cannot be dragged,
  deleted, renamed, or simulated as physics bodies.
- One 128×16 atlas and at most eight cached static quads render the collection. Nearby objects add
  at most `+0.25` to relevant existing behavior utilities.

Release-page summary:

> Colonies now leave behind a bounded collection of pillows, toys, plants, blankets, scraps,
> pebbles, lamps, and cups that make the desktop feel inhabited.

## v0.43 window-routes release

- The bounded topology derives a graph of overlapping window tiers and plans deterministic routes
  of at most four hops or climbs.
- Horizontal 10–28-point gaps with at least 64 points of shared vertical space become short-lived
  squeeze routes. `SqueezeWindow` reuses the traversal atlas and temporarily narrows the rendered
  body and layered face without new art or physics.
- Learned climbing, exploration, cursor trust, invitations, and preferred regions influence route
  scoring without overriding innate personality.
- Runtime routes retain exact supporting rectangles and cancel immediately on topology changes.
  Save version 8 and every prior identity-preserving migration remain unchanged.

Release-page summary:

> Window stacks become small traversable constructions, while narrow gaps become short-lived
> squeeze routes through the user's procedural desktop level.

## v0.42 desktop-topology release

- A runtime-only `DesktopTopology` rebuilds only when a bounded visible-window geometry hash
  changes and retains at most 64 windows and 96 derived landmarks.
- Isolated islands, exposed corners, and calm moving platforms influence existing perch,
  inspection, gaze, climb, hop, and riding behavior without new actions, art, or polling loops.
- A cursor dwelling within 24 logical points of a ledge for 1.5 seconds at under 25 points per
  second becomes an invitation for sufficiently trusting creatures.
- Topology, landmarks, cursor dwell, and old window layouts are never persisted; save version 8 and
  all prior identity-preserving migrations remain unchanged.

Release-page summary:

> Creatures now recognize window islands, exposed corners, calm moving platforms, and cursor-created
> opportunities using geometry alone.

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

## Original progressive roadmap status

The original requested sequence is implemented through v0.50: calendar arrivals, lived experience,
compressed profiles, creature bonds, colony rituals, geometry-aware topology and routes, the
bounded object ecosystem, shelter evolution, offline seed sharing, illustrated creature cards, and
reference-guided colony management. Remaining release work is native interaction/performance
measurement and signing—not an unimplemented roadmap feature.
