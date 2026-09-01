# Performance budget

Target budgets at the default 3× scale:

- resting colony: under 1% average CPU;
- four moving creatures: under 3% average CPU;
- resident memory: under 100 MB;
- presentation: no more than 20 frames per second;
- no busy loop while paused.

Record measurements per release machine using a five-minute warm-up and ten-minute sample. Include
OS version, CPU/GPU, monitor count and scaling, colony size, activity state, average CPU, peak CPU,
resident memory, and notes. Do not publish target values as measured results.

| Build | Machine | State | CPU avg | Memory | Status |
|---|---|---|---:|---:|---|
| v0.41.0 preview | local macOS test machine | resting, after 5-minute warm-up | — | — | pending |
| v0.41.0 preview | local macOS test machine | four moving, after 5-minute warm-up | — | — | pending |
| v0.41.0 preview | Windows 10/11 test machine | resting, after 5-minute warm-up | — | — | pending |
| v0.41.0 preview | Windows 10/11 test machine | four moving, after 5-minute warm-up | — | — | pending |

v0.31 uses adaptive 4–20 Hz simulation deadlines, caches native interaction-window state, and stops
presenting empty, hidden, and fully occluded monitor overlays. Full-screen application coverage also
hides the native overlay itself, avoiding transparent full-display compositor work while covered.

The procedural atlas budget is independently enforced in tests. The 90-frame body texture plus the
layered face/trinket texture total exactly 1,161,216 bytes per creature, below the 1.2 MB limit; four
creatures use 4,644,864 bytes (about 4.43 MiB) for creature textures. Atlas generation occurs only
when a creature loads or reduced-motion changes. Ambient timers reuse simulation ticks, trinkets are
pre-baked, and toss integration runs only at the existing movement cadence while airborne.

Lived experience adds no polling or behavior loop: existing world events project into fixed counters,
eight bytes of tendency scores, and twelve bounded routine slots, while continuous exposure emits at
most one compact observation per creature per 60 active visible seconds. `PetReaction` reuses an
existing body clip, so the creature atlas remains unchanged. A milestone bubble allocates its small
CPU canvas and GPU texture only for a five-second notice, permits one globally, and releases both at
expiry. The v0.40 bubble is a fixed 28×17 blank texture rather than a variable-width text texture.
Native CPU and resident-memory measurements remain pending in the table above.

Creature bonds add at most six unordered records and four raw score bytes per pair, excluding the
already-required creature IDs and JSON representation. Calm proximity reuses the 60-second
experience observation pass, and targeted behavior is selected only at existing action boundaries.
Runtime target plans and pair timers are bounded by the four-creature cap. No relationship thread,
polling loop, atlas frame, draw call, or idle GPU resource is added.

Colony rituals add four small persisted scheduling fields and at most one runtime plan with four
participants. Eligibility is evaluated only at existing action boundaries after the 12–48-hour
timestamp is due. Every ritual reuses current atlas clips, quads, behavior ticks, and target-point
movement; there is no ritual timer thread, additional OS polling, or idle GPU allocation. The
presentation-recovery patch retains at most the existing per-creature atlases, reconfigures a
surface only after repeated acquisition stalls, and grows the vertex buffer only when a valid frame
actually exceeds its current capacity.
