# Performance budget

Target budgets at the default 3× scale:

- resting colony: under 1% average CPU;
- four moving creatures: under 3% average CPU;
- resident memory: under 100 MB, measured as **resident set size** — on macOS `ps -o rss=`, on
  Windows `WorkingSet64`. A transparent always-on-top overlay also carries graphics memory the
  operating system attributes to the process but does not resident-map: macOS `footprint` and
  Activity Monitor report roughly 200 MB for the same process that shows 30 MB of RSS, most of it
  IOSurface and unmapped graphics. That number is recorded in the notes column and is not the
  budget, because it measures the compositor as much as it measures Formiga;
- energy: no target yet. Record macOS `top -stats power` alongside CPU so a target can be set from
  evidence rather than guessed;
- presentation: no more than 20 frames per second;
- no busy loop while paused.

CPU is the average over the sample window, computed from cumulative process CPU time rather than
an instantaneous reading: on macOS `ps -o time=` sampled at both ends and divided by wall time,
on Windows `Get-Counter '\Process(formiga)\% Processor Time'`. `ps -o %cpu` is a decayed estimate
and must not be used. `powermetrics` needs root and is deliberately not part of this procedure.

Record measurements per release machine using a five-minute warm-up and a ten-minute sample, at
the default 3× scale — a colony at another scale is not comparable. Set the scale before the
warm-up, not during it.

Rows recorded before 0.57.0 use the older six-column shape and were never filled in; they are kept
as a record of when a build was due to be measured, not of any measurement.

| Build | Machine | State | CPU avg | Memory | Status |
|---|---|---|---:|---:|---|
| v0.41.0 preview | local macOS test machine | resting, after 5-minute warm-up | — | — | pending |
| v0.41.0 preview | local macOS test machine | four moving, after 5-minute warm-up | — | — | pending |
| v0.41.0 preview | Windows 10/11 test machine | resting, after 5-minute warm-up | — | — | pending |
| v0.41.0 preview | Windows 10/11 test machine | four moving, after 5-minute warm-up | — | — | pending |
| v0.42.0 preview | local macOS test machine | resting, after 5-minute warm-up | — | — | pending |
| v0.42.0 preview | local macOS test machine | four moving, after 5-minute warm-up | — | — | pending |
| v0.42.0 preview | Windows 10/11 test machine | resting, after 5-minute warm-up | — | — | pending |
| v0.42.0 preview | Windows 10/11 test machine | four moving, after 5-minute warm-up | — | — | pending |
| v0.43.0 preview | local macOS test machine | resting, after 5-minute warm-up | — | — | pending |
| v0.43.0 preview | local macOS test machine | four moving, after 5-minute warm-up | — | — | pending |
| v0.43.0 preview | Windows 10/11 test machine | resting, after 5-minute warm-up | — | — | pending |
| v0.43.0 preview | Windows 10/11 test machine | four moving, after 5-minute warm-up | — | — | pending |
| v0.44.0 preview | local macOS test machine | resting, after 5-minute warm-up | — | — | pending |
| v0.44.0 preview | local macOS test machine | four moving, after 5-minute warm-up | — | — | pending |
| v0.44.0 preview | Windows 10/11 test machine | resting, after 5-minute warm-up | — | — | pending |
| v0.44.0 preview | Windows 10/11 test machine | four moving, after 5-minute warm-up | — | — | pending |
| v0.45.0 preview | local macOS test machine | resting, after 5-minute warm-up | — | — | pending |
| v0.45.0 preview | local macOS test machine | four moving, after 5-minute warm-up | — | — | pending |
| v0.45.0 preview | Windows 10/11 test machine | resting, after 5-minute warm-up | — | — | pending |
| v0.45.0 preview | Windows 10/11 test machine | four moving, after 5-minute warm-up | — | — | pending |
| v0.46.0 preview | local macOS test machine | resting, after 5-minute warm-up | — | — | pending |
| v0.46.0 preview | local macOS test machine | four moving, after 5-minute warm-up | — | — | pending |
| v0.46.0 preview | Windows 10/11 test machine | resting, after 5-minute warm-up | — | — | pending |
| v0.46.0 preview | Windows 10/11 test machine | four moving, after 5-minute warm-up | — | — | pending |
| v0.47.0 preview | local macOS test machine | resting, after 5-minute warm-up | — | — | pending |
| v0.47.0 preview | local macOS test machine | four moving, after 5-minute warm-up | — | — | pending |
| v0.47.0 preview | Windows 10/11 test machine | resting, after 5-minute warm-up | — | — | pending |
| v0.47.0 preview | Windows 10/11 test machine | four moving, after 5-minute warm-up | — | — | pending |
| v0.51.0 preview | local macOS test machine | resting, after 5-minute warm-up | — | — | pending |
| v0.51.0 preview | local macOS test machine | four moving, after 5-minute warm-up | — | — | pending |
| v0.51.0 preview | Windows 10/11 test machine | resting, after 5-minute warm-up | — | — | pending |
| v0.51.0 preview | Windows 10/11 test machine | four moving, after 5-minute warm-up | — | — | pending |
| v0.51.5 preview | local macOS test machine | resting, after 5-minute warm-up | — | — | pending |
| v0.51.5 preview | local macOS test machine | four moving, after 5-minute warm-up | — | — | pending |
| v0.51.5 preview | Windows 10/11 test machine | resting, after 5-minute warm-up | — | — | pending |
| v0.51.5 preview | Windows 10/11 test machine | four moving, after 5-minute warm-up | — | — | pending |

## 0.57.0 measurements

| Build | Machine | State | Colony | CPU avg | CPU peak | RSS | Energy | Status | Notes |
|---|---|---|---:|---:|---:|---:|---:|---|---|
| v0.55.6 release | Apple M5, 10 core, 16 GB, macOS 26.5.1, one Retina display | uncontrolled, long-running session | 3 | 5.91% | 8.17% | 30.8 MB | 5.12 | reference only | 4× scale, not 3×; menu and occlusion state unknown; footprint 208 MB. Not a gate result. |
| v0.57.0 preview | local macOS test machine | one creature, resting | 1 | — | — | — | — | pending | |
| v0.57.0 preview | local macOS test machine | one creature, moving | 1 | — | — | — | — | pending | |
| v0.57.0 preview | local macOS test machine | four resting | 4 | — | — | — | — | pending | |
| v0.57.0 preview | local macOS test machine | four moving | 4 | — | — | — | — | pending | |
| v0.57.0 preview | local macOS test machine | busy desktop | 4 | — | — | — | — | pending | |
| v0.57.0 preview | local macOS test machine | spectatorship and comedy | 4 | — | — | — | — | pending | |
| v0.57.0 preview | local macOS test machine | menu open | 4 | — | — | — | — | pending | |
| v0.57.0 preview | local macOS test machine | menu closed | 4 | — | — | — | — | pending | |
| v0.57.0 preview | local macOS test machine | occluded by a full-screen app | 4 | — | — | — | — | pending | |
| v0.57.0 preview | local macOS test machine | paused | 4 | — | — | — | — | pending | |
| v0.57.0 preview | Windows 10/11 test machine | the same ten states | — | — | — | — | — | pending | no Windows machine has been available |

The one reference row is a read-only sample of a session that happened to be running; it is above
the four-moving budget, but the scale, colony size and menu state were all wrong for a gate, so it
tells us where to start looking rather than whether the budget is met.

### What the simulation itself costs

Measured on the release profile, 40,000 ticks after a 4,000-tick warm-up, comparing this tree
against the v0.55.6 commit. Microseconds of wall time per `World::tick`:

| Scenario | v0.55.6 | 0.57.0 preview |
|---|---:|---:|
| one creature, quiet desktop | 0.47 | 0.71 |
| four creatures, quiet desktop | 1.28 | 1.70 |
| one creature, busy desktop with cursor | 2.98 | 5.61 |
| four creatures, busy desktop with cursor | 4.52 | 7.48 |
| four creatures, homebound | 1.20 | 1.69 |
| four creatures, paused | 2.28 | 2.64 |

Per-tick cost rose by a third to two thirds. Part of that is the new behavior itself — the two
builds do not walk the same trajectories, because this one has games, hesitation and hangouts to
run — and part is genuine overhead; the two are not separable from these numbers alone. In
absolute terms the worst case is 7.5 µs per tick, which at the 20 Hz ceiling is 0.015% of one
core, so `World::tick` is roughly a quarter of one percent of the CPU the application actually
uses.

### Where the CPU actually went, and what changed

A `sample` profile of the release build on the reference machine (three creatures at 4×, one
3024×1964 Retina display, a live desktop) put almost all of the application's own CPU in four
system calls rather than in the simulation:

- **Moving the interaction proxy windows.** Every creature has a small native window that follows
  it so it can be petted and dragged. Moving a window is a window-server transaction, and AppKit
  answers each one by re-reading the display configuration and copying the window list. A walking
  creature moved its proxy twenty times a second. Proxies now follow their creature only while the
  cursor is within a creature's width of it or a drag is under way. Hit-testing was already
  decided in-process from the creature's bounds and mask, so where the native window sits while
  the cursor is elsewhere changes nothing, and the window is always put in place before
  hit-testing is switched on.
- **Asking which application owns each window.** The window scan resolved every window's bundle
  identifier and name through `NSRunningApplication` — a round trip to another process — afresh
  on every scan. The answer never changes while a process runs, so it is now remembered between
  scans and forgotten when the process no longer owns a window.
- **The window list itself.** It was copied four times a second whenever any creature was merely
  walking. It is now copied that often only while a creature's footing could be moving: while it
  rides, climbs, lands, squeezes, or is carried, or while it stands on a ledge and the mouse button
  is held — the only way a window gets dragged out from under it. Otherwise once a second.
- **Full-screen drawables.** The overlay presented full-resolution full-screen images from a pool
  of three. At an even creature size on a Retina display every art pixel is a whole number of
  points, so the overlay now draws at half resolution and Core Animation magnifies it without
  smoothing — the same picture pixel for pixel. Every quad is snapped to the drawable's own pixel
  grid, occlusion rectangles are converted to drawable pixels, and the pool holds two drawables
  instead of three. At an odd size, on a display that is not Retina, or on Windows (where a smaller
  swapchain would be smoothed), the overlay keeps full resolution.

Each step was measured on the same machine against the running application, sixty seconds after
a sixty-second warm-up, with CPU taken from cumulative process time:

| Build | CPU avg | RSS | IOSurface | Footprint |
|---|---:|---:|---:|---:|
| before | 7.05% | 64.8 MB | 69 MB | 170 MB |
| proxies parked, owners remembered, floor walking scans once a second | 1.93% | 55.6 MB | 46 MB | 145 MB |
| window list fast only during drags, half-resolution drawables | 0.58% | 54.9 MB | 12 MB | 105 MB |

These are single sixty-second windows on a live desktop with creatures doing whatever they chose
to, not the controlled gate procedure above, and they are recorded as development measurements
rather than gate results. The last row's five-second intervals ranged from 0.40% to 0.60%.

WindowServer was checked the same way, because a full-screen overlay's cost can land in the
compositor rather than in the application: 40.5% with Formiga running, 39.3% with it stopped, and
37.8% running again. The difference is inside the run-to-run noise, so after these changes the
overlay adds no compositor load that can be told apart from everything else on the desktop.

About 75 MB of the remaining footprint is graphics memory owned by the process in roughly 2 MB
IOAccelerator blocks. It is the same at full and half resolution, on a fresh start and after
several minutes, so it is neither the drawables nor a leak; attributing it needs Instruments'
Metal allocation tracing.

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

Desktop topology retains at most 64 compact window records, 96 compact landmarks, and one cursor
dwell record. It rebuilds only when the bounded geometry hash changes, reuses the existing desktop
scan and behavior tick, and creates no thread, OS query, texture, atlas frame, draw call, or idle GPU
allocation. Corner peeks, islands, invitations, and moving platforms reuse existing actions.

Window routes add a bounded breadth-first search across at most 64 topology windows, retain no more
than four hops per active creature, and run only when an existing perch choice needs a destination.
Squeezes reuse the traversal atlas and change only body/face vertex width, so the 90-frame atlas,
texture bytes, draw-call count, physics cost, and idle allocation remain unchanged.

This release adds no dependency, thread, worker, polling loop, texture, atlas frame, or draw call
to the overlay. The optional sprite outline is baked into the existing 90-frame body atlas from the
creature's own alpha, so the atlas stays at 1,161,216 bytes and the outline costs one extra bake
when the preference changes and nothing per frame; it is drawn by the same quad as the creature and
leaves the interaction mask untouched, because the mask is built from the un-outlined composite. A
carried prop moved from an ad-hoc offset to the shared `PropAnchor`, which changes where an existing
quad sits and not how many there are. The weekly routine is evaluated inside the existing world
tick — no timer, and no new event-loop deadline. The three geometry games cost the play session one
new field between them: the race stores the window it runs to, hide and seek reuses the four
remembered points a leader already had for the one place a seeker last saw someone, and the lava
round holds nothing at all, reading each member's support afresh every tick. All of it is
runtime-only.

Colony objects add one 128×16 RGBA atlas (8 KiB) per loaded colony and at most eight static quads.
Their vertex cache changes only when object state, habitat, display geometry, or scale changes. The
three-to-seven-day timestamp is evaluated in the existing world tick, and nearby role utility is
computed only for the bounded eight-object collection at existing action boundaries. There is no
physics body, interaction proxy, object thread, animation frame, per-object draw call, or idle
regeneration.

Shelter growth stores at most six enum values and three scheduling fields in the existing home. Its
four-to-nine-day timestamp is evaluated in the normal world tick. When decoration state changes,
the existing 64×64 shelter canvas and 16 KiB RGBA texture are regenerated once; every ordinary frame
continues to render the same single shelter quad and draw call. No decoration atlas, vertex, physics
body, animation, editor resource, thread, or idle allocation is added.

Seed sharing has zero idle cost. Encoding and validation handle a fixed payload — 37 bytes for a
legacy creature, 57 for one carrying a design recipe — only after an explicit profile/settings
action. Import reconstructs at most four temporary generated creatures,
keeps only the selected source generation, and immediately releases the rest. No texture, worker,
network client, code history, or background allocation persists afterward.

Creature cards also have zero idle cost. The card renderer is a zero-sized stateless type, and the
native save dialog runs before the one 960×600 canvas, ephemeral font atlas, and PNG encoder are
created. Cancellation allocates no card canvas. A completed export drops every card buffer before
returning and never uploads the image to the overlay GPU or changes the save. Native CPU and memory
measurements remain pending in the table above.

Reference matching has zero idle cost. A selected image is decoded once under 16 MB, 4096×4096,
and 16-million-pixel limits, downsampled within 64×64, and compared with exactly 512 temporary modular
creature frames. A fixed 512-bin color histogram and bounded geometry cues adapt candidate recipes.
Only the winning seed, 16-byte design recipe, and preview survive the matching call; clearing or
accepting the preview drops its one small settings texture. Colony role checks and mini balancing
operate across the existing four-creature bound and introduce no new simulation loop or draw call.

The v0.55.6 generator adds no dependency, model weights, external service, source-image texture,
or idle worker. Each optional recipe is 16 bytes plus its option discriminant, stored in appearance
and immutable origin. Existing body/face frame dimensions, animation counts, and GPU atlas budgets
are unchanged. Wing structure is a handful of extra lines inside the existing appendage, baked into
the same body frames at atlas construction, so it costs no quad, texture, or per-frame work. The
village still uses at most eight object quads against the existing 128×16 object
atlas, plus at most four dwelling quads against one 128×128 shelter atlas that replaces the previous
64×64 one; none of it is rendered while the house is inactive. Belonging colors are derived once per
atlas build, adding no per-frame work. Runtime CPU/GPU measurements on native
Windows and a full multi-display macOS session remain manual release checks, not inferred from
unit tests.


## Native colony interface and companion features (0.57.0)

No production dependency, worker, overlay draw call, or desktop atlas was added. Journal projection
runs only with existing events and retains at most 64 typed entries, with six-hour duplicate
throttling. Quiet expiry is one optional timestamp checked by the existing world tick. Decoration
visibility filters at most six values on the stack; object arrangement reuses the existing vector
and village layout.

The settings artwork budget is 416 KiB for all four portraits, four eight-frame 48px candidate
strips, the village atlas, the object atlas, and the scrapbook's eight fixed 16px trinket drawings
together. Everything the window can hold at once — four 48×48 portraits, four 384×48 strips, one
128×128 village atlas, one 128×16 object atlas, and eight 16×16 drawings — comes to 413,696 bytes
against that 425,984-byte ceiling. The home preview draws from the same 128px village atlas and
object atlas the desktop samples, so a complete village costs two textures however many houses and
belongings it holds; it adds 48 KiB over the single-shelter preview it replaces. The scrapbook
accounts for the other 8 KiB: one drawing per trinket variant, generated from the colony seed when
the page opens and released with the rest of the artwork. Preview positions come from the same
layout functions the overlay uses and are arithmetic only — no extra texture, and no redraw unless
the colony, home, or habitat changes. This excludes the pre-existing egui font atlas,
window GPU resources, and short-lived upload buffers. The headless UI test enforces the artwork
budget and verifies resource release. Hidden/occluded windows do not schedule preview redraws;
closing frees artwork, and Studio animation is opt-in at six fps. Static pages are event driven,
with two exceptions: the live Colony page refreshes at most once a second, and a running quiet
moment repaints its countdown every thirty seconds.

Normal and minimum-size pages are rendered in a test-only software rasterizer for layout review;
that renderer is excluded from production. Native CPU, GPU, energy, and resident-memory measurements
still require the release-machine protocol above. Texture-budget tests are not measurements of
whole-process memory or energy use.


Geometry attention adds reserved storage for 64 previous/current window shapes, 16 expiring cues,
and eight coarse display preferences. A focused test caps these observation buffers, including
struct overhead, at 8 KiB. Only changed native scans recompute exposed-tier/free-space targets;
preferences ease on subsequent fresh scans. Four-creature attention plans, support memory, short
approaches, and viewing/landing reservations reuse the normal simulation tick and existing atlas.
There are no added OS scans, threads, dependencies, texture frames, or overlay draw calls. The
storage assertion and release binary size do not substitute for native CPU/memory/energy sampling.


Display/cursor attention adds fixed-size observer records, separately tested at no more than
1,024 bytes and 256 bytes respectively. Display-route planning is throttled to at most one attempt
per second during a bounded opportunity; two-region routes and four participants reuse the normal
world tick. There is no additional native polling loop, worker, GPU resource, or atlas frame.
Native mixed-DPI/compositor and CPU/memory/energy verification remains a release requirement.

One colony game runs at a time, with at most four members, bounded turns, hand-offs, and length,
and a cooldown before the next. A scene stores a kind, a seed, roles, an expiry, four remembered
positions, at most one plaything, and one optional finish-line window; a declined invitation is
four fixed records. The
plaything reuses the existing carried-object quad, so no artwork, atlas frame, or draw call is
added. Movement signatures are computed from identity and personality when a frame is chosen, and
add no state.

Risk memory is four fixed setback records that decay over 90 seconds. A hesitation scene is bounded
at 16 seconds and revalidates only at its phase boundaries. Crowding checks compare the existing
64-window cap against at most four creatures, and only while no other scene is active.

Surface familiarity stores sixteen total hangouts, sampled once per second, in at most 1,024 bytes.
Ride memory also stays within 1,024 bytes; copycat/staring encounter and scene storage stays within
512 bytes. These are struct-storage assertions, not measurements of whole-process memory. Gap
selection samples sixteen arc intervals, then rechecks each actual step. Routes keep the existing
64-window/four-hop caps and allow only one repair. Social encounters consider at most four creatures;
copycat visits each member once and all scenes expire. Body and face atlas counts remain unchanged:
contact changes reuse sprite placement, and shared falls reuse the existing integrator. Native
profiling remains pending for the expanded library.
