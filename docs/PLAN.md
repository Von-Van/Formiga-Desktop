# Formiga roadmap

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

## Post-v0.31

- Behavioral observation storage, research/Lab UI, export, and ML experiments.
- Accessories, seasonal history, audio, breeding, and new colony mechanics.
- Cross-monitor autonomous travel and direct petting.
- Linux investigation only after the macOS and Windows experience is stable.

The post-v0.31 data layer must subscribe to the existing in-memory `WorldEvent` interface rather than
coupling persistence or analytics to the behavior engine.
