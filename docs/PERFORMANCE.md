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
| v0.38.1 preview | local macOS test machine | resting, after 5-minute warm-up | — | — | pending |
| v0.38.1 preview | local macOS test machine | four moving, after 5-minute warm-up | — | — | pending |
| v0.38.1 preview | Windows 10/11 test machine | resting, after 5-minute warm-up | — | — | pending |
| v0.38.1 preview | Windows 10/11 test machine | four moving, after 5-minute warm-up | — | — | pending |

v0.31 uses adaptive 4–20 Hz simulation deadlines, caches native interaction-window state, and stops
presenting empty, hidden, and fully occluded monitor overlays. Full-screen application coverage also
hides the native overlay itself, avoiding transparent full-display compositor work while covered.

The procedural atlas budget is independently enforced in tests. The 90-frame body texture plus the
layered face/trinket texture total exactly 1,161,216 bytes per creature, below the 1.2 MB limit; four
creatures use 4,644,864 bytes (about 4.43 MiB) for creature textures. Atlas generation occurs only
when a creature loads or reduced-motion changes. Ambient timers reuse simulation ticks, trinkets are
pre-baked, and toss integration runs only at the existing movement cadence while airborne. Native
CPU and resident-memory measurements remain pending in the table above.
