# Formiga roadmap

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

## Post-v0.25

- Behavioral observation storage, research/Lab UI, export, and ML experiments.
- Accessories, seasonal history, audio, breeding, and new colony mechanics.
- Cross-monitor autonomous travel, side climbing, direct petting, and auto-update.
- Linux investigation only after the macOS and Windows experience is stable.

The post-v0.25 data layer must subscribe to the existing in-memory `WorldEvent` interface rather than
coupling persistence or analytics to the behavior engine.
