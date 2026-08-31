# Release test matrix

Automated checks are the baseline, not a substitute for native desktop testing.

## Automated

| Check | macOS | Windows |
|---|---:|---:|
| Format and Clippy warnings denied | CI | CI |
| Workspace tests | CI | CI |
| Deterministic 1,000-genome render test | CI | CI |
| v1/v2/v3/v4/v5→v6 migration, birth timestamps, names, and top-12 routines | CI | CI |
| Atomic round trip and corrupt-primary recovery from the previous-save backup | CI | CI |
| One-hour, one-week, and clamped calendar-month arrival boundaries | CI | CI |
| Fixed memory/routine limits and sub-2-KiB per-creature serialized growth | CI | CI |
| Learned utility monotonicity, ±0.35 cap, saturation, and contrary-experience recovery | CI | CI |
| Descriptor ±35/±25 hysteresis, badge persistence, and 12-active-hour bubble throttle | CI | CI |
| Unicode name validation, deterministic unique defaults, and user duplicate names | CI | CI |
| 60-active-second observation projection and hidden/paused suspension | CI | CI |
| 1,000 genomes × all actions and layered face checks | CI | CI |
| Generated passive-prop determinism and alpha coverage | CI | CI |
| Ambient cadence bounds, deterministic landmarks, pause/hidden suspension | CI | CI |
| Continuous upward traverse/climb/mantle, downward hops, and route interruption | CI | CI |
| Raised dangling GPU/proxy handhold placement contract | CI | CI |
| Eight deterministic opaque trinkets and hold playback | CI | CI |
| Exactly 90 unique body frames; layered atlas at or below 1.2 MB | CI | CI |
| Habitat region algebra | CI | CI |
| Pet/drag maximum-excursion classification across scale, placement, toss, cancel, and re-grab | CI | CI |
| Home petting without dismissal and threshold-crossing shelter dismissal | CI | CI |
| Temporary milestone-bubble pixels, global singularity, and idle resource release | CI | CI |
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
| Pet click, drag-out-and-back, shelter pet, and visible reaction at supported scales | pending | pending | pending | pending |
| Smooth climb/mantle, raised dangle contact, downward hop, inspection, and discovery | pending | pending | pending | pending |
| Colony names, descriptors, unread badges, age, places, and milestone bubble | pending | pending | pending | pending |
| Habitat create/move/resize/toggle/delete | pending | pending | pending | pending |
| Invalid habitat cannot replace valid policy | pending | pending | pending | pending |
| Selected app occlusion and rule removal | pending | pending | pending | pending |
| Window ordering/minimize/close | pending | pending | pending | pending |
| Moving/minimized/closed supporting window during climb or dangle | pending | pending | pending | pending |
| Multi-display, negative coordinates, hot-plug | pending | pending | pending | pending |
| Toss across monitor seams, negative coordinates, and custom habitat boundaries | pending | pending | pending | pending |
| Spaces/virtual desktops and default full-screen hiding | automated geometry; native pending | automated geometry; native pending | pending | pending |
| Sleep/wake and lock/unlock | pending | pending | pending | pending |
| Package install, relaunch, and representative v1/v2/v3/v4/v5 migration | pending | pending | pending | pending |
| Manual and automatic GitHub update check | pending | pending | pending | pending |
| Verified update download; corrupt checksum refusal | pending | pending | pending | pending |
| DMG/MSI handoff without silent installation | pending | pending | pending | pending |

Unsigned preview status must remain explicit until signing credentials are configured.
