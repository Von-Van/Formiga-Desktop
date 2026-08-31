# Changelog

All notable changes are documented here.

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
