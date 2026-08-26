# Formiga roadmap

## v0.2 portfolio interaction release

Implemented in this release:

- decorated settings application;
- alpha-aware, non-activating creature dragging;
- habitat presets and normalized allowed/excluded rectangles;
- interactive desktop habitat editing and safe relocation;
- privacy-safe application selection and local visual occlusion;
- explicit v1→v2 save migration;
- stable display/application identities;
- portfolio assets, documentation, CI, and preview packaging.

Release validation still requires completing every real-device row in `TEST_MATRIX.md`, recording
measured energy use in `PERFORMANCE.md`, and signing/notarizing builds when credentials exist.

## Post-v0.2

- Behavioral observation storage, research/Lab UI, export, and ML experiments.
- Accessories, seasonal history, audio, breeding, and new colony mechanics.
- Cross-monitor autonomous travel, side climbing, direct petting, and auto-update.
- Linux investigation only after the macOS and Windows experience is stable.

The post-v0.2 data layer must subscribe to the existing in-memory `WorldEvent` interface rather than
coupling persistence or analytics to the behavior engine.
