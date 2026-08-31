# Release test matrix

Automated checks are the baseline, not a substitute for native desktop testing.

## Automated

| Check | macOS | Windows |
|---|---:|---:|
| Format and Clippy warnings denied | CI | CI |
| Workspace tests | CI | CI |
| Deterministic 1,000-genome render test | CI | CI |
| v1/v2/v3/v4→v5 migration and birth timestamps | CI | CI |
| One-hour, one-week, and clamped calendar-month arrival boundaries | CI | CI |
| 1,000 genomes × all actions and layered face checks | CI | CI |
| Generated passive-prop determinism and alpha coverage | CI | CI |
| Ambient cadence bounds, deterministic landmarks, pause/hidden suspension | CI | CI |
| Upward traverse/climb/mantle stages, downward hops, route interruption | CI | CI |
| Shared dangling GPU/proxy placement contract | CI | CI |
| Eight deterministic opaque trinkets and hold playback | CI | CI |
| Exactly 90 unique body frames; layered atlas at or below 1.2 MB | CI | CI |
| Habitat region algebra | CI | CI |
| Drag placement, toss threshold/sampling, swept landing, one bounce, timeout, and re-grab | CI | CI |
| Alpha-mask and occlusion geometry | CI | CI |
| Update version/asset selection and 24-hour throttle | CI | CI |
| Update digest validation and unsafe-name rejection | CI | CI |
| Universal app / x64 package | CI | CI |

## Manual release gates

Use `pass`, `fail`, or an issue link. Do not mark a row from compilation evidence alone.

| Scenario | macOS 14 arm64 | macOS x64/Rosetta | Windows 10 22H2 | Windows 11 |
|---|---|---|---|---|
| One-hour unrelated-click test | pending | pending | pending | pending |
| No focus activation while dragging | pending | pending | pending | pending |
| Opaque-pixel hit test at 100/150/200% | pending | pending | pending | pending |
| Drag release/cancel/pause/reduce-motion | pending | pending | pending | pending |
| Slow placement, fast toss, soft bounce, and mid-flight re-grab | pending | pending | pending | pending |
| Climb, mantle, downward hop, dangle, inspection, and discovery at supported scales | pending | pending | pending | pending |
| Habitat create/move/resize/toggle/delete | pending | pending | pending | pending |
| Invalid habitat cannot replace valid policy | pending | pending | pending | pending |
| Selected app occlusion and rule removal | pending | pending | pending | pending |
| Window ordering/minimize/close | pending | pending | pending | pending |
| Moving/minimized/closed supporting window during climb or dangle | pending | pending | pending | pending |
| Multi-display, negative coordinates, hot-plug | pending | pending | pending | pending |
| Toss across monitor seams, negative coordinates, and custom habitat boundaries | pending | pending | pending | pending |
| Spaces/virtual desktops and default full-screen hiding | automated geometry; native pending | automated geometry; native pending | pending | pending |
| Sleep/wake and lock/unlock | pending | pending | pending | pending |
| Package install, relaunch, and v1/v2 migration | pending | pending | pending | pending |
| Manual and automatic GitHub update check | pending | pending | pending | pending |
| Verified update download; corrupt checksum refusal | pending | pending | pending | pending |
| DMG/MSI handoff without silent installation | pending | pending | pending | pending |

Unsigned preview status must remain explicit until signing credentials are configured.
