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
warm-up, not during it. On macOS, `scripts/measure-macos.sh` performs the warm-up and the sample for
one state and prints the table row; set each state up by hand and leave it alone until it finishes.
Two checks it cannot make are visual: that no state presents faster than twenty frames a second,
and that CPU settles close to zero once the colony is paused. Measurements come from process
statistics and synthetic desktops only; never record, capture, or describe the real desktop being
used to take them.

## Measurements

| Build | Machine | State | Colony | CPU avg | CPU peak | RSS | Energy | Status | Notes |
|---|---|---|---:|---:|---:|---:|---:|---|---|
| v0.55.6 release | Apple M5, 10 core, 16 GB, macOS 26.5.1, one Retina display | uncontrolled, long-running session | 3 | 5.91% | 8.17% | 30.8 MB | 5.12 | reference only | 4× scale, not 3×; menu and occlusion state unknown; footprint 208 MB; 1.38% of one core averaged over its whole 2 d 12 h run. Not a gate result. |

The one row above is a read-only sample of a session that happened to be running; it is above the
four-moving budget, but the scale, colony size and menu state were all wrong for a gate, so it
tells us where to start looking rather than whether the budget is met.

Nothing else has been measured under the procedure above. The states it asks for — one creature
resting and moving, four resting and moving, a busy desktop, spectatorship, the menu open and
closed, occluded by a full-screen app, and paused — remain unmeasured on a release machine for
0.57.0 and later, and no Windows machine has been available at all. The figures in the rest of
this document are storage and per-tick costs, which are asserted by tests or measured with
`formiga-tools tick-bench`; they are not measurements of what the application costs a desktop.

### What the simulation itself costs

Measured with `cargo run --release -p formiga-tools -- tick-bench`, 40,000 ticks after a
4,000-tick warm-up, comparing this tree against the v0.55.6 commit. Microseconds of wall time per
`World::tick`:

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

The optimized macOS executable grew from 13,880,976 bytes before this release's behavior work
began to 13,947,712 bytes after its first three slices, and a host-architecture release build of
0.57.0 was 14,213,552 bytes. These are binary sizes, not memory measurements.

About 75 MB of the remaining footprint is graphics memory owned by the process in roughly 2 MB
IOAccelerator blocks. It is the same at full and half resolution, on a fresh start and after
several minutes, so it is neither the drawables nor a leak; attributing it needs Instruments'
Metal allocation tracing.

v0.31 uses adaptive 4–20 Hz simulation deadlines, caches native interaction-window state, and stops
presenting empty, hidden, and fully occluded monitor overlays. Full-screen application coverage also
hides the native overlay itself, avoiding transparent full-display compositor work while covered.

The procedural atlas budget is independently enforced in tests. The 124-frame body texture plus the
layered face/trinket texture total exactly 1,529,856 bytes per creature; four creatures use
6,119,424 bytes (about 5.84 MiB) for creature textures. The body atlas is ten columns wide, so it
grows a row at a time. Each step has been a deliberate raise: 0.57.0's 90 frames filled nine rows at
1,161,216 bytes; 0.57.1's 28 gesture frames took it to twelve rows, 276,480 bytes more per creature;
and 0.58.0's six watching frames take it to thirteen, another 92,160 bytes. The 331,776-byte face
texture is unchanged throughout. A release bake of the 0.57.1 atlas measured 1.4 ms. Selecting a gesture's slot is the
same fixed-size lookup as an action's, so it adds no draw call, texture, or per-frame allocation. Atlas generation occurs only
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
Only the winning seed, design recipe, and preview survive the matching call; clearing or
accepting the preview drops its one small settings texture. Colony role checks and mini balancing
operate across the existing four-creature bound and introduce no new simulation loop or draw call.

The v0.55.6 generator adds no dependency, model weights, external service, source-image texture,
or idle worker. Each optional recipe is 22 bytes — sixteen modular and six classic parts — plus its
option discriminant, stored in appearance and immutable origin. Classic parts are drawn into the same
body and face frames at atlas construction, so they add no frame, quad, texture, or per-frame work. Existing body/face frame dimensions, animation counts, and GPU atlas budgets
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

## 0.58.0

Creature textures are 1,529,856 bytes each and 6,119,424 bytes for a full colony of four. The
per-creature test limit moved from 1,500,000 to 4,500,000 bytes. That is not a measurement of
anything: it is the owner's decision that the budget should stop a creature costing more than a
creature should, not stop it having poses, so there is room for the next ones without the ceiling
moving again each time.

The overlay gains one 256×80 RGBA interface atlas: 81,920 bytes, built the first time a bubble or a
menu appears, released after 240 renders with neither up, and released immediately when the colony
is hidden or the overlay is torn down. While something is up it costs one extra bind group and one
extra draw call inside the existing pass; while nothing is, it costs nothing. The colony trinket
atlas is one 256×32 RGBA texture, 32,768 bytes per loaded colony, and it replaces an eight-slot row
that was previously baked into every creature's face texture — so the more creatures a colony has,
the less it holds than before.

The settings artwork budget is 432 KiB (442,368 bytes), raised from 416 KiB. Everything the window
can hold at once — four 48×48 portraits, four 384×48 candidate strips, one 128×128 village atlas,
one 128×16 object atlas, and the one 256×32 trinket sheet — measures 438,272 bytes. The sheet is the
whole of the increase: sixteen trinkets and their glint frames in one texture, where eight separate
16×16 drawings used to be eight.

The simulation tick is unchanged at 4–20 Hz; the host's tick interval drops to 50 ms only while a
creature menu is open, and a menu is opened by the person at the desk and closes itself within eight
seconds. Everything the new behavior remembers is a bounded runtime table, never serialized: six
overlap pairs and four shuffles with their per-pair cooldowns, three offer cooldowns per creature,
at most five thought bubbles, one doorstep moment per resident, and one visitor's scene progress.

Stickers and the colony portrait have zero idle cost, like the creature card: the native save dialog
runs before any canvas exists, cancellation allocates nothing, and every buffer is dropped before
returning. Neither image is uploaded to the overlay GPU and neither touches the save. The `gif`
crate, already used by `formiga-tools`, is now also a dependency of `formiga-art`, so it is linked
into the shipped application for the first time; it is a pure-Rust encoder with no runtime,
thread, or allocation of its own outside an export.

No native CPU, memory, or energy measurement has been taken for this release. The budgets above are
storage and resource assertions, not measurements of whole-process cost, and the release-machine
protocol at the top of this document remains the only thing that can answer whether the budgets are
met.
## 0.58.5

`World::tick` costs less on a busy desktop, for behaviour that is byte-identical to 0.58.0.
Measured with

```sh
cargo run --release -p formiga-tools -- tick-bench --ticks 40000 --warmup 4000
```

on an Apple M5, macOS 26.5.1, release profile: 40,000 ticks after a 4,000-tick warm-up,
interleaving a binary built from 0.58.0 with one built from this tree, three rounds, medians.
Microseconds of wall time per `World::tick`:

| Scenario | 0.58.0 | 0.58.5 |
|---|---:|---:|
| one creature, quiet desktop | 1.14 | 1.12 |
| four creatures, quiet desktop | 3.67 | 3.68 |
| one creature, busy desktop with cursor | 5.87 | 4.10 |
| four creatures, busy desktop with cursor | 9.40 | 6.97 |
| four creatures, homebound | 3.50 | 3.53 |
| four creatures, paused, busy desktop | 2.64 | 2.16 |

A busy desktop is between a quarter and a third cheaper. The window-free scenarios are unchanged
because nothing that changed runs without windows; the small wobble there is round-to-round
spread, not a result.

What changed. The topology answers island, left-corner, and right-corner in one walk per window
instead of three. The ambience sampler mixes its change signature the way the topology's geometry
hash already does, gathers the visible windows once into a reused list, and sorts that list front
to back so a cover test stops at the frames it has already passed. The ledge search that fills
`reachable_window_ledge` moved from every creature every tick to the action boundary that actually
reads it. The geometry observer matches and counts windows in one pass and asks whether a window
moved before counting its neighbours. The bounded window lists reserve once instead of growing five
or six times a tick, and the per-tick clones of every creature and relationship became two reused
buffers.

Behaviour was proved unchanged rather than assumed. A throwaway differential harness driving only
the public API ran six seeds across thirteen scenarios, 40,000 ticks each, against both builds, and
hashed the save, the event stream, the bubbles, each creature's attention pose, and the interaction
flags: 78 identical digests, with coverage counters printed beside each so that an identical digest
could not mean neither build had done anything.

### What the village costs a tick

The village layout was measured separately, with the same tool over 4,000 ticks, comparing the
village work before and after its own allocation pass — not 0.58.0 against 0.58.5. The absolute
numbers are therefore comparable with each other and not with the table above. Microseconds per
`World::tick`:

| Scenario | before | after |
|---|---:|---:|
| four creatures, homebound | 4.56 | 1.77 |
| four creatures, quiet desktop | 2.31 | 1.69 |
| four creatures, busy desktop | unchanged | unchanged |

`reconcile_colony_objects` went from roughly 34 allocations a tick to 1, and
`tick_homebound_creatures` from roughly 76 to 8. `village_walk` is a fixed-capacity stack array,
`colony_cottage_list` picks the members in place, `home_resting_position` and `home_guest_position`
allocate nothing, `VillageGround::resolve` lists the habitat's regions once rather than three
times, and `home_object_positions` resolves the village once for all eight belongings.

### Storage

The geometry observer's asserted storage bound moves from 8 KiB to 9 KiB, and the measurement
behind it is stricter than it was: 8,312 bytes with every capped list at its cap, where the old
test measured 6,264 as the lists happened to be filled. The budget covers three capped window
lists — the last two scans, and the frames the display preferences are now sampled from — plus the
per-display records and the signals in flight. The frame list is the only thing that grew, and it
is the reused list that removed the ambience sampler's per-window work.

The save is unchanged: version 15, no new field, no migration. Both keepsake trees, every keepsake
hanging in them, and every belonging in their yards are derived at runtime from the colony seed and
the scrapbook the save already holds, so none of it costs a byte on disk. The village atlas is
still one 128×128 texture and one bind group; the two trees share its fourth cell, the inward one
sampling it with its horizontal UVs swapped, and add one quad each to a full village's six.

No native CPU, memory, or energy measurement has been taken for this release. The figures above are
per-tick costs and storage assertions, not measurements of whole-process cost, and the
release-machine protocol at the top of this document remains the only thing that can answer whether
the budgets are met.

## 0.59.0

### A full colony, measured

`tick-bench` now holds every scenario's colony at the size it names, adds a full colony of six on
each kind of desktop and at the houses, adds an invited guest touring the village, and prints the
share of ticks with the houses out beside the share in motion, so a "homebound" label can be checked
the way "resting" and "moving" already could. Two earlier flaws make its numbers from before this
release incomparable with these: once 0.58.7 raised the cap to six, each "four creatures" scenario
grew to six partway through its run, because every adult's own minis kept arriving; and the
homebound scenario kept its houses out only for the fifteen simulated minutes a gathering lasts, a
third of a default run.

Measured with

```sh
cargo run --release -p formiga-tools -- tick-bench --ticks 40000 --warmup 4000
```

on an Apple M5, macOS 26.5.1, release profile, three rounds, medians. Microseconds per
`World::tick` plus `World::drain_events`:

| Scenario | mean | p95 | in motion | at home |
|---|---:|---:|---:|---:|
| one creature, quiet desktop | 0.90 | 1.08 | 19% | 45% |
| four creatures, quiet desktop | 2.24 | 2.88 | 48% | 45% |
| six creatures, quiet desktop | 3.14 | 3.62 | 55% | 45% |
| one creature, busy desktop with cursor | 3.92 | 4.92 | 40% | 45% |
| four creatures, busy desktop with cursor | 5.54 | 6.62 | 54% | 45% |
| six creatures, busy desktop with cursor | 6.48 | 8.50 | 56% | 45% |
| four creatures, homebound | 2.01 | 2.12 | 1% | 100% |
| six creatures, homebound | 3.13 | 3.29 | 0% | 100% |
| six creatures and a visitor, homebound | 4.08 | 4.46 | 0% | 100% |
| four creatures, paused, busy desktop | 1.68 | 1.75 | 0% | 45% |
| six creatures, paused, busy desktop | 1.71 | 1.79 | 0% | 45% |

A full colony on a busy desktop costs 6.5 µs a tick: at the 20 Hz cadence, where 500 µs a tick is
one percent of one core, that is about 0.013%. A guest touring the village adds about a
microsecond. The simulation is not where the application's CPU goes.

This machine ran about twice as fast in one sitting during the release as in the others,
v0.58.9's own `tick-bench` included, so only numbers from one sitting are compared. The table is
the finished release's. In the same sitting, v0.58.9's `tick-bench` gave 3.9 µs for one creature on
a busy desktop and 6.7 µs for the colony its "four creatures" scenario grew into, six on a busy
desktop, against 3.9 and 6.5 here: the habits, village moments, hangout spots, arranged cottages,
and a kept undo point cost nothing measurable per tick. `house_owners`, which the homebound walk
asks for every tick, holds its answer in place like `Cottages` rather than allocating.

The one column that moved is "in motion". A companion that stopped to eat, drink, play on its
own, dangle, look something over, or hold up a find used to keep the speed it walked in with and
glide through the whole action; now it stops. A lone creature on a quiet desktop is in motion 19%
of the time instead of 30%, and the application only ticks at its 20 Hz moving cadence while
something is.

### On the desktop

The finished build ran for three minutes on the desktop it was written on — an Apple M5 with the
Dock hidden, the owner's own five-companion colony copied into a scratch data directory with
`FORMIGA_DATA_DIR`, the houses out, and a browser playing video alongside — after a minute to
settle. Beside it, the same three minutes of the installed v0.58.9, freshly launched on the same
colony:

| | average CPU | energy impact | physical footprint |
|---|---:|---:|---:|
| v0.58.9 | 0.86% | 0.7–1.4 | 111 MB |
| 0.59.0 | 0.81% | 0.7–1.4 | 111 MB |

CPU is the process's own CPU time over the window; energy impact is `top`'s power column, sampled
every thirty seconds; the footprint is `footprint`'s `phys_footprint`. The v0.58.9 process that had
been running for fifteen hours before it was restarted for this measured 152 MB and 1.9% over the
same window, busier at first and settling toward the fresh figures; its footprint has not been
looked into.

### Saving

The same run counts, per simulated minute, the ticks whose events ask for the colony to be saved,
and how often it is actually written. Until this release every one of those asks wrote the whole
file at once, as well as the half-minute periodic write:

| Scenario | asked for | written |
|---|---:|---:|
| six creatures, busy desktop with cursor | 85.0 a minute | 3.6 a minute |
| six creatures, quiet desktop | 28.6 | 3.5 |
| four creatures, busy desktop with cursor | 58.8 | 3.5 |
| six creatures, homebound | 4.7 | 3.1 |
| six creatures, paused, busy desktop | 0.1 | 2.0 |

A save of that colony — serialize, write and flush a temporary file, read and validate the current
one, copy it to the backup, and replace it — took 4.73 ms on average over 200 saves of its
51,395-byte file, 5.31 ms at the 95th percentile and 6.42 ms at worst, medians of three runs, on
the main thread. At 87 writes a minute a busy full colony spent about 410 ms a minute saving and
wrote about 13 MB; it now spends about 17 ms and writes about half a megabyte.

What changed. `WorldEvent::save_urgency` sorts events into those kept at once — an arrival, the
houses coming or going, a ritual, a new belonging or decoration — and everyday movement: an action
starting or a creature stepping onto another surface. `formiga_core::save_due` writes a prompt
change immediately, gathers movement into a checkpoint at most every fifteen seconds, and still
writes a colony with nothing waiting every thirty. Explicit actions and quitting save at once as
before, and recovery is unchanged: at most fifteen seconds of who was doing what, and where, can be
lost to a crash. Writes stay on the main thread; at a few a minute a worker would add ordering
problems without a measurable gain.

### Storage

Everything the simulation remembers about each resident is now sized by the colony instead of a
scene. Measured with `size_of`, every structure stays inside the bound its test asserts:

| Structure | 0.58.9 | 0.59.0 | bound |
|---|---:|---:|---:|
| `SurfaceMemory` (favourite places, four each) | 656 | 976 | 1,024 |
| `RideMemory` | 280 | 408 | 1,024 |
| `PlayRuntime` | 400 | 464 | 512 |
| `DisplayAttention` | 720 | 752 | 1,024 |

A full colony's creature textures are 9,179,136 bytes, six of the 1,529,856-byte atlases; the test
holds a full colony under 9 MiB, the same 1.5 MiB a creature the budget for four set. A recipe is
22 bytes in memory, 16 modular and 6 classic, and classic parts are drawn into the existing body and
face frames, so they add no frame, quad, texture, or per-frame work.

### Artwork

Every house now has a cell of its own, by day and lit after dark, so the village texture on the
display the village is on grows from 128×128 to 256×256: 262,144 bytes, 196,608 more. The colony's
object sheet grows from eight 16×16 cells to fourteen for the three hangout spots and three garden
patches, 14,336 bytes. A chosen palette or cottage order is in the village texture's key, so either
redraws it once and costs nothing per frame. The settings window holds only the daylit half of the
village, 131,072 bytes, and everything it can hold at once measures 509,952 bytes against a budget
raised from 432 KiB to 500 KiB, the house cells and the object sheet being the whole of the rise.

A postcard, like the colony portrait, is drawn only once its save dialog has a destination: one
960×600 canvas, the colony's village atlas, and the members' frames, all dropped as soon as the
PNG is written. Choosing a scene or typing a caption draws and uploads nothing, which a test holds.

The one change that can be undone keeps the colony's creatures, bonds, home, and keepsakes as they
stood before it, a single clone replaced by the next change and never written to disk. For the
five-companion colony that the smoke test loaded, the same data is about 19 KB of its 30 KB
compact file.

### One GPU device per display (O3)

Each overlay creates its own wgpu device and pipelines. A headless probe that builds the same
pipelines, buffers, and sampler for one to six overlays, measured with `footprint` on the same
Apple M5, found each additional display costs about 0.7 MB of physical footprint and 3 ms of setup
with a device of its own, against about 0.06 MB and 0.7 ms sharing one: sharing would save roughly
0.65 MB and 2.4 ms per extra display. It was not done. A shared device would have to be compatible
with every display's surface, which a Mac with a second GPU or a Windows PC with several adapters
does not promise, and a display unplugged mid-run would take recovery for every overlay with it;
the saving is under half of one creature's 1.5 MB textures, which are only ever held by the display
the creature is on.

No native CPU, memory, or energy measurement has been taken for a full colony. The figures above are
per-tick costs, save costs, and storage assertions, not measurements of whole-process cost; the
release-machine protocol at the top of this document remains the only thing that can answer whether
the budgets are met, and no Windows hardware has been available to run it there.
