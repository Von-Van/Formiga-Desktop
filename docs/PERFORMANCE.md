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
| v0.31.0 preview | local macOS test machine | pending native measurement | — | — | pending |
| v0.31.0 preview | Windows 10/11 test machine | pending native measurement | — | — | pending |

v0.31 uses adaptive 4–20 Hz simulation deadlines, caches native interaction-window state, and stops
presenting empty, hidden, and fully occluded monitor overlays. Full-screen application coverage also
hides the native overlay itself, avoiding transparent full-display compositor work while covered.

The procedural atlas budget is independently enforced in tests: body and face textures total less
than 1 MB per creature. A local optimized test on 2026-08-26 produced an 866,304-byte layered atlas
in 0.51 ms, comfortably below the 75 ms bake budget. Atlas generation occurs only when a creature
loads or reduced-motion changes; runtime platform measurements remain pending in the table above.
