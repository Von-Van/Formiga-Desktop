# Changelog

All notable changes are documented here.

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
