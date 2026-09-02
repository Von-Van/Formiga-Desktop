# Release test matrix

Automated checks are the baseline, not a substitute for native desktop testing.

## Automated

| Check | macOS | Windows |
|---|---:|---:|
| Format and Clippy warnings denied | CI | CI |
| Workspace tests | CI | CI |
| Deterministic 1,000-genome render test | CI | CI |
| v1/v2/v3/v4/v5/v6/v7/v8/v9→v10 migration, creature/object/bond preservation, names, births, rituals, and top-12 routines | CI | CI |
| Atomic round trip and corrupt-primary recovery from the previous-save backup | CI | CI |
| One-hour, one-week, and clamped calendar-month arrival boundaries | CI | CI |
| Fixed memory/routine limits and sub-2-KiB per-creature serialized growth | CI | CI |
| Learned utility monotonicity, ±0.35 cap, saturation, and contrary-experience recovery | CI | CI |
| Descriptor ±35/±25 hysteresis, badge persistence, and 12-active-hour bubble throttle | CI | CI |
| Unicode name validation, deterministic unique defaults, and user duplicate names | CI | CI |
| 60-active-second observation projection and hidden/paused suspension | CI | CI |
| Six-pair relationship maximum, canonical IDs, four score bytes, saturation, and deterministic round trip | CI | CI |
| Five calm minutes, bounded bond utility, contrary avoidance reduction, and score projection | CI | CI |
| Targeted follow, shared sleep, gift, play/steal, greet, climb-watch, toss-concern, and squabble action reuse | CI | CI |
| Bond target refresh/cancellation for moved, missing, sleeping, homebound, tossed, cross-surface, and removed companions | CI | CI |
| Deterministic 12–48-hour ritual scheduling, all nine kinds, safe eligibility, and no downtime catch-up flood | CI | CI |
| Ritual interruption, 2–6-hour retry, reduced motion, local hatch-day deduplication, and shared-plan caps | CI | CI |
| Multi-creature monitor-ID rebinding, presentation-buffer growth, and stalled-surface recovery path | CI | CI |
| Topology geometry-hash rebuild suppression and 64-window/96-landmark caps | CI | CI |
| Island/corner/slow-platform classification across overlap, negative coordinates, DPI, and rapid change | CI | CI |
| Cursor invitation 24-point, 1.5-second, and 25-points/second boundaries plus pause/hidden clearing | CI | CI |
| Four-hop window-graph cap, deterministic tier routing, and preference scoring bounds | CI | CI |
| Exact 10–28-point narrow-gap classification, support validation, and immediate geometry cancellation | CI | CI |
| Squeeze traversal-atlas reuse, 0.72× body/face scaling, and stable routine codes 0–23 | CI | CI |
| Deterministic 3–7-day object scheduling, one-after-downtime behavior, eight-object cap, and stable IDs | CI | CI |
| Eight-kind object-atlas determinism/alpha coverage, habitat recovery, cached static quads, and `+0.25` utility cap | CI | CI |
| Deterministic 4–9-day decoration scheduling, one-after-downtime behavior, six-unique-kind cap, and canonicalization | CI | CI |
| Memory/bond/ritual/object-driven decoration choice and deterministic single-texture 64×64 shelter baking | CI | CI |
| All four seed generations round-trip case-insensitively and reproduce innate identity byte-for-byte | CI | CI |
| Seed prefix/group/version/generation/length/alphabet/padding/checksum validation and corruption rejection | CI | CI |
| Imported fresh history, distinct companion lineage, explicit replacement gate, and unchanged v10 save compatibility | CI | CI |
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
| Blank fixed-size milestone-bubble pixels, global singularity, and idle resource release | CI | CI |
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
| Colony names, descriptors, unread badges, age, places, and wordless milestone bubble | pending | pending | pending | pending |
| Bond profile labels and closest-companion changes remain read-only except for the name | pending | pending | pending | pending |
| Follow, sleep-beside, gift, toy steal, shelter greet, climb watch, toss concern, and harmless squabble playback | pending | pending | pending | pending |
| Bond sequence cancellation when a target moves, sleeps, enters shelter, is tossed, or disappears | pending | pending | pending | pending |
| Picnic, group nap, floor race, shelter gathering, catch, presentation, hatch day, quiet huddle, and sleep pile playback | pending | pending | pending | pending |
| Ritual interruption by hide, pause, drag, toss, geometry change, and reduced-motion substitution | pending | pending | pending | pending |
| Habitat create/move/resize/toggle/delete | pending | pending | pending | pending |
| Invalid habitat cannot replace valid policy | pending | pending | pending | pending |
| Selected app occlusion and rule removal | pending | pending | pending | pending |
| Window ordering/minimize/close | pending | pending | pending | pending |
| Moving/minimized/closed supporting window during climb or dangle | pending | pending | pending | pending |
| Window islands, exposed-corner peeks, slow platform rides, and calm cursor invitations | pending | pending | pending | pending |
| Four-tier constructions, narrow-gap squeeze playback, and route cancellation during window motion | pending | pending | pending | pending |
| Eight object kinds, multi-display placement, habitat recovery, static rendering, and no downtime flood | pending | pending | pending | pending |
| Six shelter decoration kinds, history-reflective choice, overdue behavior, and unchanged single-quad rendering | pending | pending | pending | pending |
| Copy seed, mixed-case paste, invalid/corrupted refusal, explicit replacement, relaunch, and one-hour companion arrival | pending | pending | pending | pending |
| Multi-display, negative coordinates, hot-plug | pending | pending | pending | pending |
| Toss across monitor seams, negative coordinates, and custom habitat boundaries | pending | pending | pending | pending |
| Spaces/virtual desktops and default full-screen hiding | automated geometry; native pending | automated geometry; native pending | pending | pending |
| Sleep/wake and lock/unlock | pending | pending | pending | pending |
| Two- and four-creature rendering for one hour without disappearance; automatic surface recovery | pending | pending | pending | pending |
| Package install, relaunch, v10 round trip, and representative v1/v2/v3/v4/v5/v6/v7/v8/v9 migration without creature loss | pending | pending | pending | pending |
| Manual and automatic GitHub update check | pending | pending | pending | pending |
| Verified update download; corrupt checksum refusal | pending | pending | pending | pending |
| DMG/MSI handoff without silent installation | pending | pending | pending | pending |

Unsigned preview status must remain explicit until signing credentials are configured.
