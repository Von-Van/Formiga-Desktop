# Changelog

All notable changes are documented here.

## [0.45.0] - 2026-09-01

### Added

- Six deterministic shelter decorations: leaf, banner, stone, flower, lamp, and roof ornament. A
  colony earns one unique decoration every four to nine UTC days and retains at most six.
- Decoration selection reflects the dominant compact colony state across creature memories, bond
  scores, the last ritual, and accumulated colony objects, with seed-derived deterministic
  tie-breaking.

### Changed

- Decorations are baked into the existing procedural 64×64 shelter canvas. The GPU regenerates and
  uploads that one texture only when the shelter genome or bounded decoration list changes; normal
  rendering retains the existing shelter quad and draw call.
- Save version 10 adds only the decoration list, next timestamp, and ordinal inside the existing
  home state. Deterministic v1–v9 migration preserves every creature, bond, ritual, object, setting,
  custom name, memory, tendency, routine, birth time, home identity, and resolved genome.
- Overdue colonies receive at most one decoration before scheduling the next date from the current
  maximum-seen UTC time. Duplicate or excess saved decorations are canonicalized to six unique
  typed values without affecting the rest of the colony.

## [0.44.0] - 2026-09-01

### Added

- Eight deterministic static colony-object families: pillows, toys, plants, blankets, paper scraps,
  pebbles, lamps, and cups. A colony retains at most eight objects, each with a stable ID, display,
  normalized position, and semantic role.
- A deterministic three-to-seven-day UTC object schedule. An overdue colony receives at most one
  object before the next timestamp is scheduled from the current time, so downtime never replays a
  backlog.
- One seed-derived 128×16 object atlas and at most eight cached static quads per display. Object
  vertices rebuild only when the objects, habitat, display geometry, scale, or colony seed changes.

### Changed

- Nearby objects contribute role-specific sleep, play, comfort, social, or curiosity utility at
  existing action-selection boundaries, capped at `+0.25` without adding actions or simulation.
- Save version 9 adds only the bounded colony-object projection and its next schedule timestamp.
  Deterministic v1–v8 migration preserves every existing creature, bond, ritual, setting, name,
  memory, tendency, routine, birth time, home, and resolved genome.

### Fixed

- Objects whose saved display or normalized position is no longer valid now snap to the nearest safe
  habitat floor or shelter area instead of disappearing or escaping the configured habitat.

## [0.43.0] - 2026-09-01

### Added

- A bounded runtime graph for overlapping window tiers, with deterministic routes of at most four
  hops or climbs through the existing window-journey system.
- Narrow-gap recognition for horizontally separated windows with 10–28 logical points of space and
  at least 64 points of overlapping height. Eligible creatures can traverse the gap with a new
  runtime `SqueezeWindow` state that reuses the ordinary six-frame traversal clip.
- A temporary 0.72× horizontal body-and-face quad scale during squeezes. No collision engine,
  physics body, texture regeneration, or additional atlas frame is introduced.

### Changed

- Route scoring combines bounded learned climbing, exploration, cursor trust, cursor invitations,
  and preferred-region hints while preserving the creature's innate utility behavior.
- New runtime action code 24 is appended after all prior routine codes, leaving persisted routine
  keys 0–23 byte-for-byte stable. Save version remains 8 and all v1–v8 colonies load unchanged.

### Fixed

- Multi-tier and squeeze routes retain the exact supporting rectangles and cancel immediately if
  the topology hash or either supporting window changes, settling the creature onto a safe habitat
  surface rather than following stale geometry.

## [0.42.0] - 2026-09-01

### Added

- A runtime-only `DesktopTopology` derived from safe visible-window rectangles, bounded to 64
  windows and 96 landmarks. It recognizes isolated window islands, exposed top corners, and slow
  moving platforms without reading titles, pixels, URLs, or document content.
- Privacy-safe cursor invitations: dwelling within 24 logical points of a ledge for 1.5 seconds at
  under 25 points per second can coax a sufficiently trusting creature toward that ledge.
- Geometry-aware corner peeks and island preferences that reuse existing inspection, gaze, perch,
  climb, hop, and riding actions with no new atlas frames.

### Changed

- Desktop topology rebuilds only when the bounded visible-window geometry hash changes. Calm
  platform motion uses the existing attachment behavior, while successful rides continue to feed
  compact experience memory through the existing 60-active-second observation projection.
- Save version remains 8. v1–v8 colonies load without an additional schema migration, and topology,
  cursor dwell, window landmarks, and prior geometry are never serialized.

### Fixed

- Exposed-corner and island classification remains bounded and deterministic across negative
  virtual-desktop coordinates, overlapping windows, display scaling, and rapid geometry changes.

## [0.41.0] - 2026-09-01

### Added

- Rare deterministic colony rituals scheduled 12–48 hours apart: picnics, group naps, floor races,
  shelter gatherings, two-creature catch games, group presentations, hatch days, quiet-day huddles,
  and late-night sleep piles.
- A runtime-only `ColonyPlan` that gathers eligible creatures and coordinates existing actions and
  habitat-safe target points. Rituals use the ordinary behavior-selection cadence and add no new
  animation atlas, physics system, polling loop, or persisted event history.
- Local-time hatch-day and late-night eligibility with UTC fallback. Reduced motion substitutes
  calm rituals for races, while idle and unchanged geometry can permit a privacy-safe quiet huddle.

### Changed

- Save version 8 persists only the next ritual timestamp, last kind, ordinal, and hatch-day
  acknowledgement. Deterministic v1–v7 migration preserves all creature identity, appearance,
  custom names, memories, tendencies, routines, current state, and bond scores.
- An overdue ritual waits for a safe action boundary, runs at most once, and schedules its next
  occurrence from the current time instead of replaying anything missed during downtime.
- Rituals cancel safely when the colony is hidden, paused, dragged, tossed, or loses valid display
  geometry. Interrupted rituals retry after a deterministic two-to-six-hour delay.

### Fixed

- A long-running multi-creature overlay could remain transparent after the presentation surface
  became repeatedly occluded or timed out. The renderer now reconfigures and redraws a stalled
  surface automatically, grows its bounded vertex buffer if needed, and no longer requires an app
  restart to recover.
- Creatures whose native monitor identifier changes after sleep, hot-plugging, or a display-mode
  transition are rebound to the monitor containing their saved position instead of remaining alive
  but absent from every overlay.
- v7→v8 migration preserves already-canonical relationship scores byte-for-byte rather than
  rebuilding them as legacy relationship records.
- Creatures could disappear from a desktop and stay missing until every window covering the screen
  was hidden, and revealing them on one macOS Space left the others empty. Full-screen occlusion
  hid the colony by ordering the shared overlay window out, which detached it from every Space at
  once; it then rejoined only whichever Space was active when it was ordered back in. Full-screen
  apps now suppress drawing instead, so the overlay stays attached to all Spaces, and its
  all-Spaces collection behavior is re-applied on every hide/show cycle.

## [0.40.0] - 2026-08-31

### Added

- Compact persistent bonds for every unordered creature pair. Affinity, familiarity, playfulness,
  and avoidance use exactly four raw score bytes per pair, with at most six pairs in a four-creature
  colony.
- Calm five-minute proximity observations and completed greetings, shared rest, play, discoveries,
  homecoming greetings, climb watching, toss concern, toy stealing, and harmless squabbles now
  adjust bond scores through the existing ephemeral `WorldEvent` projection.
- Target-aware sequences that reuse `Follow`, `Sleep`, `PresentDiscovery`, `SocialPlay`, `Greet`,
  `InspectScreen`, and `ReactToWindow`. Creatures can follow a preferred companion, sleep beside it,
  bring it a discovery, steal its temporary toy, greet it after shelter visits, watch it climb, and
  react when it is tossed without adding relationship-specific animation frames.
- Read-only qualitative bond and playfulness summaries in Colony profiles.

### Changed

- Social utility now combines innate personality with bounded pair scores. Positive contact can
  reduce avoidance, while stressful or competitive encounters can increase it; scores saturate
  safely and never rewrite a creature's genome.
- Runtime bond plans refresh a moving target's position and cancel safely if that target is missing,
  sleeping, homebound, tossed, removed, on another display, or otherwise unavailable for the
  selected interaction.
- Five-second milestone thought bubbles are now intentionally blank. Learned descriptors remain
  private until the user chooses to inspect the creature's read-only Colony profile.
- Save version 7 migrates every v1–v6 colony deterministically. Legacy per-creature relationship
  floats become canonical shared pair records, reciprocal values are averaged, and all creature
  identity, generated appearance, custom names, birth times, memories, tendencies, routines,
  positions, and settings are preserved.
- Relationship diagnostics retain only the `bond_interaction` event category; names, coordinates,
  scores, targets, and memory payloads remain absent from logs.

## [0.39.0] - 2026-08-31

### Added

- Click-without-drag petting with a visible affectionate response that reuses the greeting body
  clip and adds no creature-atlas frames.
- Compact typed memories for pets, tosses, placements, interrupted and uninterrupted sleep,
  ledges, window rides, climbs, discoveries, play, and home visits. One-minute active observations
  update accumulated state and are discarded; cursor paths, coordinates, layouts, and event history
  are never saved.
- Eight bounded learned tendencies for cursor trust, sociability, climbing, sleep security,
  exploration, play, home affinity, and routine. Their combined utility contribution is capped at
  ±0.35 and contrary experiences can reverse every learned preference.
- Deterministic cozy names and a Colony settings tab with learned descriptors, age, discoveries,
  favorite display/region, and closest companion. Only names are editable; 1–24 trimmed Unicode
  scalar values are accepted while control characters and line breaks are rejected.
- Persistent profile badges with ±35/±25 descriptor hysteresis and optional five-second milestone
  bubbles. Bubbles are globally singular, throttled to once per creature per 12 active hours, and
  rendered into a temporary texture that is released when the notice ends.
- `ActionChoice` targets for creatures and points, allowing repeated placement to gradually guide
  ordinary traversal toward a preferred 3×3 display region.

### Changed

- Save version 6 replaces unbounded string-keyed habits with a fixed twelve-slot numeric routine
  table and migrates the twelve strongest v1–v5 entries. It also adds stable creature origin,
  colony order, name, memory, and tendency records within a 192-byte raw-memory and 2 KiB serialized
  per-creature budget.
- Every world event passes through the compact projection path before it can be drained. Diagnostic
  logs retain event categories only, never names, coordinates, relationship values, or memory
  payloads.
- Direct manipulation now tracks maximum cursor excursion. Gestures at or below six logical points
  are pets; larger gestures retain precise placement, tossing, cancellation, and in-flight re-grab.
  Petting a homebound creature no longer dismisses its shelter.
- Dangling art is raised two source pixels so hands sit on the window edge. Upward routes now lift
  and move inward throughout a continuous climbing mantle, removing the brief landing-pose hitch
  before a creature settles onto a ledge.

## [0.38.1] - 2026-08-31

### Changed

- Additional creatures now join a colony one hour, one week, and one calendar month after it is
  created, while the default colony remains capped at four creatures.
- Calendar-month arrivals preserve the UTC time of day and clamp end-of-month dates safely, so a
  colony created on January 31 receives its monthly arrival on February 28 or 29.
- Overdue arrivals retain the existing 15-second reveal spacing and maximum-seen UTC guard, so
  relaunches and clock rollback cannot skip, duplicate, or remove a creature.
- Save version 5 records each creature's birth timestamp and deterministically reconstructs birth
  dates for v4 colonies without replacing their existing shelter state.

## [0.38.0] - 2026-08-28

### Added

- Low-frequency `ClimbWindow`, `Dangle`, `InspectScreen`, and `PresentDiscovery` activities with
  four-frame family-specific poses, expressions, gaze, and static reduced-motion variants.
- Staged upward window routes that traverse to the nearest inner edge, climb at 44–62 logical
  points/second, and mantle onto the ledge; downward transfers retain the existing hop.
- Geometry-only inspection landmarks at safe-region and window thirds, per-creature dangle and
  inspection cadence, and one colony-wide discovery cadence. All ambient countdowns stop while
  Formiga is paused or hidden.
- Eight deterministic gems, keys, leaves, shells, charms, and relics per creature, pre-baked into
  one appended layered-texture row and shown with a temporary third quad only during discovery.
- Fast-release creature tossing with a fixed three-sample cursor history, capped launch velocity,
  gravity, swept ledge/floor collision, one soft bounce, a three-second recovery limit, and final
  landing events. Slow releases retain precise placement.
- An accelerated ambient-art sheet covering every family, new action frame, placement mode, and
  discovery variant.

### Changed

- The body atlas remains at 90 unique frames by reusing the dragged clip for `Tossed`; the exact
  combined body/face/trinket texture budget is 1,161,216 bytes per creature under a 1.2 MB limit.
- GPU sprite placement and alpha-aware interaction proxies share a handhold-relative contract for
  dangling art.
- Drag update and release commands now carry normalized cursor velocity and distinguish placed from
  tossed outcomes. Paused, reduced-motion, and disabled-window-ledge modes retain non-ballistic safe
  behavior.
- A defaulted transient `activity_variant` keeps existing v4 saves compatible without a version bump;
  interrupted ambient activity and toss flight reload as idle and no discovery collection persists.

## [0.37.1] - 2026-08-27

### Fixed

- Creatures were hidden on any monitor covered by a driver HUD. The NVIDIA GeForce overlay keeps a
  window sized to the primary display for as long as the driver is loaded, and fullscreen app
  occlusion counted it as a fullscreen application, so creatures disappeared on that monitor
  whenever the default occlusion setting was on. Windows that cannot be activated, are
  click-through, or are layered tool windows are no longer treated as ordinary application windows,
  which also stops creatures from walking along an invisible ledge across the screen.

## [0.37.0] - 2026-08-27

### Fixed

- Creatures were invisible on Windows overlays. The overlay needs `WS_EX_LAYERED` for reliable
  cross-process click-through, but a plain HWND swap chain cannot keep per-pixel transparency while
  that style is set, so every creature rendered blank. Overlays now present through
  DirectComposition, which can target a layered HWND without discarding alpha.
- The Windows overlay surface now requests premultiplied alpha. DirectComposition composites
  `DXGI_ALPHA_MODE_PREMULTIPLIED` only, and the previously preferred `PostMultiplied` mode maps to
  `DXGI_ALPHA_MODE_STRAIGHT`, which DXGI does not support for composition swap chains. macOS keeps
  `PostMultiplied`, the only transparent mode CAMetalLayer reports.
- Layered window attributes are initialized so the overlay HWND is presented at all, and the
  requested input mode is reapplied after each hide/show cycle because winit rebuilds native window
  styles when showing a window.

### Changed

- Windows overlays now require a DX12-capable adapter. DirectComposition is the only path that
  preserves per-pixel alpha on a layered, click-through window, so the overlay instance no longer
  falls back to the Vulkan or OpenGL backends.

## [0.36.6] - 2026-08-27

### Added

- Optional once-daily and manual GitHub release checks in the tray and About screen.
- Background downloads for exact-platform DMG/MSI assets with size limits and mandatory SHA-256
  verification before the installer can be opened.
- Windows MSI handoff with clean Formiga shutdown and macOS DMG handoff for manual replacement.
- Autonomous eating, drinking, and short sprinting behaviors with distinct drive outcomes.
- Deterministic balls, yarn, tossed leaves, snacks, cups, and bowls derived from each creature's
  existing appearance genome and pre-baked into its animation atlas.
- Family-specific hand and front-paw poses for play, eating, drinking, and sprinting.
- A generated passive-activity art sheet for reviewing every body family.

### Changed

- Tagged release versions now flow into the binary, macOS bundle, Windows MSI, and artifact names.
- Solo play now visibly manipulates a generated toy instead of relying on an effect motif alone.
- Desktop scheduling treats sprinting as spatial motion while eating and drinking remain low-cost
  pose-only clips.
- Homebound creatures are spaced by their drawn width instead of a fixed 18 points, so the colony
  no longer stacks onto a single point at the shelter.
- Creature sprites are seated by each creature's authored under-body clearance, putting their feet
  on the surface they stand on rather than hovering above it.

### Fixed

- Creatures at the shelter could not be picked up. Interaction proxies were positioned before they
  were resized, and on macOS that offsets the window by the size difference; because both calls
  only run when the value changes, a motionless creature never corrected it.
- A press landing on an overlapping interaction proxy is resolved against the creature alpha masks,
  so clicks reach the creature actually drawn under the cursor.
- A drag whose mouse release was never delivered to the proxy window no longer wedges the session
  and blocks every later grab.

## [0.31.0] - 2026-08-26

### Added

- Default-on full-screen application occlusion on macOS and Windows, detected exclusively from
  safe display and window geometry.
- An Applications setting for opting out of automatic full-screen hiding.
- A close-up coral mascot app icon, peeking from a mint shelter with expressive eyes and hands,
  designed to remain readable down to system-tray sizes.

### Changed

- Replaced the fixed 50 Hz active wake-up with adaptive 4–20 Hz simulation deadlines.
- Capped moving presentation at the 20 Hz simulation rate and matched pose-only presentation to
  each authored animation's actual frame rate.
- Native drag-proxy position, size, visibility, and generated alpha masks are now cached.
- Empty or fully occluded monitor overlays stop presenting after one clearing frame.
- Full-screen displays hide their overlay and interaction proxies, preventing compositor work and
  invisible input interception while Formiga is covered.
- Application owner identity lookups are deduplicated within each desktop-window scan.
- Application-occlusion geometry is recalculated and its GPU uniform uploaded only when window or
  rule state changes.

## [0.25.0] - 2026-08-26

### Added

- Layered face atlas with eleven expressions, nine gaze directions, and open/half/closed eyelids.
- State- and activity-aware expression resolution with deterministic irregular blinking.
- Family-specific blob pseudopods, hopper mitten hands, and quadruped front-paw gestures.
- Pre-baked sleep, investigation, play, greeting, social, and startle effects.
- Expression and gesture art-lab contact-sheet commands.
- Deterministic v2→v3 save migration for resolved face, forelimb, and effect genes.
- Drag-to-Applications macOS DMG and normal per-user Windows installer packaging.
- Generated cross-platform application icon and first-launch Settings onboarding.
- Seeded bottom-corner colony homes with four procedurally rendered shelter families.
- Persistent 15-minute home visits and 15-minute minimum reappearance cooldowns.
- Drag-to-dismiss home behavior and calm activity-coordinated homebound poses.
- Deterministic v3→v4 migration for shelter identity and durable home timing.

### Changed

- Replaced three gaze-duplicated body atlases with one body atlas and one compact face atlas.
- Interaction masks now use the fully composited expressive frame.
- Fixed one-pixel line rasterization, improving facial and appendage clarity.
- Updated portfolio assets and release metadata for v0.25.0.
- Creatures now transfer between reachable window ledges at different heights instead of treating
  the first ledge as a permanent horizontal track.
- Release automation now publishes the current-version DMG, MSI, and portable ZIP artifacts.

## [0.2.0] - 2026-08-25

### Added

- Native settings window with General, Habitat, Applications, and About sections.
- Alpha-aware direct creature dragging, safe landing, cancellation, and gather command.
- Visible ledge journeys and immediate reactions to nearby window creation, movement, and closure.
- Habitat presets plus allowed/excluded per-display rectangles and desktop editing.
- Privacy-safe application rules and GPU visual occlusion with window-order subtraction.
- Stable display identities, macOS bundle IDs, Windows AUMIDs, and executable-hash fallback.
- Explicit v1 save migration and portfolio/release documentation.

### Changed

- Upgraded the renderer/UI stack to wgpu 30.0.1 and egui 0.36.1.
- Detailed controls moved from the tray into Settings.

## [0.1.0] - 2026-08-25

- Initial procedural desktop ecosystem foundation for macOS and Windows.
