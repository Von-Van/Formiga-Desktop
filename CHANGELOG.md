# Changelog

All notable changes are documented here.

## [0.25.0] - 2026-08-26

### Added

- Layered face atlas with eleven expressions, nine gaze directions, and open/half/closed eyelids.
- State- and activity-aware expression resolution with deterministic irregular blinking.
- Family-specific blob pseudopods, hopper mitten hands, and quadruped front-paw gestures.
- Pre-baked sleep, investigation, play, greeting, social, and startle effects.
- Expression and gesture art-lab contact-sheet commands.
- Deterministic v2→v3 save migration for resolved face, forelimb, and effect genes.

### Changed

- Replaced three gaze-duplicated body atlases with one body atlas and one compact face atlas.
- Interaction masks now use the fully composited expressive frame.
- Fixed one-pixel line rasterization, improving facial and appendage clarity.
- Updated portfolio assets and release metadata for v0.25.0.

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
