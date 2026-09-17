# Formiga architecture

Formiga separates platform observation, deterministic simulation, procedural art, and presentation.
`formiga-core` contains no GUI or GPU code; OS adapters cannot decide creature behavior, and the art
crate cannot read desktop state.

```text
macOS / Windows adapters
  monitors · cursor · idle time · safe window rectangles · application identity
                              │
                              ▼
                       DesktopSnapshot
                              │
                              ▼
formiga-core: drives → bounded utility selection → action state machines → surfaces
          │                         │
          │                         └── WorldEvent → compact state → ephemeral drain
          ▼
formiga-art: genome → family rig → normalized pose → deterministic atlas
          │
          ▼
 formiga-desktop: per-monitor overlays + interaction proxies + occlusion uniforms
```

The assisted updater is a separate desktop-host service, not an input to `DesktopSnapshot` or the
world simulation. At launch it may schedule one short-lived worker to read public GitHub release
metadata. User-approved downloads run on a second short-lived worker, stream into a temporary file,
enforce size limits, and become launchable only after SHA-256 verification. Completion returns to the
main event loop through `UserEvent`; no async runtime, updater daemon, or render-loop polling is added.

## Procedural identity and animation

A 256-bit colony seed derives named ChaCha streams for appearance, personality, markings, animation
flavor, runtime decisions, and each mini. Resolved genomes are stored in the save so future generator
changes cannot silently redesign an existing creature.

Blob, hopper, and soft-quadruped rigs share a stable two-eye face grammar. The renderer branches on
body family for the ear, tail, and forelimb passes, so soft quadrupeds read as cats and hoppers as
rabbits without a stored genome changing: ear and tail genes select a shape rather than whether the
feature exists, and a quadruped only swaps its front legs for arm-style forelimbs in the actions
that visibly use them. Every body frame records a face anchor and family-specific forelimb targets.
Authored clips manipulate those anchors, squash,
planted contacts, limb gestures, and secondary tail/head-appendage motion. Markings and temporary
activity effects remain body-local and are rasterized at integer coordinates.

Passive toys, snacks, and drinkware are deterministically derived from genes already stored in the
appearance genome. Their colors, shape variants, motion phases, and hand targets are baked into the
same action atlas as the creature. Eating and drinking therefore add no runtime asset lookup or
procedural work; sprinting reuses the existing movement tick with a distinct six-frame gait.

Climbing, dangling, inspection, and discovery presentation add four authored frames each. Upward
window transfers are staged as ordinary traversal to the nearest inner edge, a 44–62 point/second
vertical climb, and a 0.7-second continuous climbing mantle; downward transfers keep the existing
hop. The mantle begins slightly below the ledge and eases upward and inward without switching to a
landing pose. A shared `FramePlacement` contract seats normal art by its feet and dangling art by
its slightly raised handhold, so the GPU quad and alpha-aware interaction proxy resolve the same
bounds.

Eight 16×16 gems, keys, leaves, shells, charms, and relic variants are derived from the creature seed
and palette at atlas-build time. `activity_variant` selects one only while `PresentDiscovery` is
active. There is no inventory, history, runtime generation, or persistent collection.

The renderer caches one gaze-free 48×48 body atlas and one 16×16 layered face texture per creature.
The face texture contains eleven expressions, nine gaze directions, three eyelid states, and one
eight-slot trinket row. The body atlas holds exactly 118 unique frames: 90 for actions, because
`Tossed` reuses the dragged body clip, and 28 for nine gesture poses. Runtime work normally selects
two slots and draws two nearest-filtered quads; discovery alone adds one temporary quad. The
combined textures are exactly 1,437,696 bytes per creature and are enforced below a 1.5 MB test
limit.

Gestures — cheer, gasp, cover, worry, crouch, heave, balance, reach, and bop — are a runtime-only
`gesture` on `AttentionPose`, so saves never carry one. While one is set, `BodyClip::for_creature`
shows its baked clip in place of the action's; the action still owns movement, placement, facing,
and frame timing, and the GPU quad, the interaction mask, and the review sheets resolve the same
clip. One place decides whether a body is free to show a pose — `body_free` in `world/attention.rs`,
applied to every role after it has proposed one. A pose survives only over a planted presentation
action, never under reduced motion, while walking, hopping, hanging by the hands, or when the
creature's own journey or a toss still moves it this tick; what actually moves the creature decides
that, rather than its velocity, because an approach that has handed over to a journey leaves the
last stride on the books. Both renderers draw one limb per side: a gesture carries the resting paw, nub, or wing out
to where it points, rather than drawing another limb beside the one already there. A wing opens in
its own colors and texture; a long body lifts its near front paw off the ground. Covering the face
also closes the eyes, which the layered face still draws over the paws.

`PetReaction` maps to the existing greeting body clip, so lived experience does not grow that atlas.
A newly earned profile descriptor may allocate one small sprout thought-bubble texture for five
seconds; descriptor text remains available only in the Colony profile. Only one bubble exists
globally, and the GPU texture and CPU pixels are dropped at expiry, so there is no idle bubble
resource.

The colony seed also resolves a bottom-corner preference and a compact shelter genome. Leaf tents,
mushroom huts, cushion dens, and paper houses are rasterized once to a static 64×64 texture. A
persisted home lifecycle alternates a maximum 15-minute visit with a minimum 15-minute cooldown.
While active, creatures use a calm `Homebound` pose at the resolved habitat-safe corner. A click can
pet them in place; only crossing the six-logical-point drag threshold dismisses the shelter and
starts the cooldown.

## Lived-experience projection

`World::emit` is the sole event queue path. Before events become visible through `drain_events`, a
projection updates compact typed memory, bounded `i8` tendencies, fixed numeric routines, and profile
revision state. Selected moments also project into the capped 64-entry typed journal. The event
vector is runtime-only and emptied by the desktop host; no coordinates, cursor paths, or window
layouts enter the journal.

The eight tendency fields stay in `-100..=100`. Learned action modifiers, including routine and
successful-window-ride confidence, are clamped to ±0.35 after combination. Innate personality still
sets the base utility and temperature, and contrary events move the same fields in the opposite
direction. Every 60 active visible seconds becomes at most one summarized observation per creature;
sampling stops while paused or hidden.

Legacy string habits become twelve compact slots keyed by packed time bucket, display third, surface,
and action. Repeated placement also records a recoverable preferred 3×3 display cell and can supply
an ordinary `Traverse` target through `ActionChoice`; it does not add a pathfinding loop or action.

## Creature-bond projection

The save owns at most six canonical unordered `CreatureRelationship` records, one for each possible
pair in a four-creature colony. Each record has two stable IDs and four `u8` scores: affinity,
familiarity, playfulness, and avoidance. The scores themselves therefore consume exactly four raw
bytes per pair. Pair values saturate in `0..=255`; positive and contrary contact can move them in
both directions without changing either creature's innate genome.

The existing 60-active-second observation pass also accumulates calm proximity in runtime-only
pair timers. Each completed five-minute interval emits one compact bond experience and discards the
exposure detail. Completed targeted actions emit the other bond experiences. No encounter list,
target route, object ownership, or social history is serialized.

At action boundaries, utility selection receives the preferred pair as a `BondContext`. A
runtime-only `BondPlan` can approach through `Follow` and then execute an existing targeted action.
Target points refresh from the current creature snapshot each tick; plans cancel to idle if the
target disappears, moves to an incompatible surface or display, sleeps, becomes homebound, is
tossed, or otherwise cannot participate. Follow, sleep, presentation, social play, greeting,
inspection, and window reaction reuse their existing body clips, leaving the atlas unchanged.

## Desktop composition

Each monitor receives a transparent, always-on-top rendering overlay. It is non-activating and
click-through during normal use. Coordinates remain virtual-desktop logical points until the final
renderer conversion to physical pixels.

Direct manipulation uses one small proxy per visible creature. A pre-baked one-bit frame mask shapes
the Windows proxy and controls near-creature hit testing on macOS. The proxy preserves the grab
offset, captures through release, and sends normalized cursor position and velocity to the
simulation. Three fixed-capacity velocity samples cover roughly the final 150 ms without allocation.
The interaction session tracks maximum excursion from the press point: at most six logical points is
a pet, even across DPI scales, while moving farther remains a drag even if the cursor returns before
release. Slow releases use precise placement. Fast releases enter a runtime-only `Tossed` state with gravity,
horizontal drag, swept downward support tests, at most one soft bounce, and a three-second recovery
limit. There is no rotation, wall ricochet, creature collision, or general physics dependency.

Habitat policies are the union of allowed rectangles (or a preset) minus excluded rectangles.
Rectangles are normalized against privacy-safe display identities, so DPI and resolution changes do
not invalidate them. Window-ledges are clipped to the same reachable habitat.

Selected applications are represented by bundle ID, AUMID, or a SHA-256 executable-path digest. The
renderer subtracts higher windows from each selected window's visible rectangles and sends up to 64
monitor-local rectangles to a fragment-shader uniform. Covered pixels are discarded; a dragged
creature opts out per vertex.

The overlay treats repeated surface timeouts or compositor-occluded acquisition as recoverable. It
reconfigures the existing surface after three consecutive stalls, invalidates cached presentation
state, and requests another frame. Its creature vertex buffer also grows to the next bounded power
of two if a valid colony frame exceeds the initial four-creature allocation. Simulation positions
are reconciled to current native monitor IDs before rendering, so display sleep or hot-plug changes
cannot strand a living creature outside every overlay.

## Colony rituals

The save stores one deterministic `RitualState`: next UTC timestamp, last kind, ordinal, and the
local year of the last acknowledged hatch day. When that time is overdue, an existing action
boundary may create one runtime-only `ColonyPlan` if every revealed creature has a safe shared floor
region. Missed rituals are not counted or replayed; the next timestamp is scheduled from the actual
start time.

The plan has only approach and ceremony phases, at most four participants, habitat-safe points, and
existing action kinds. It coordinates picnics, group naps, floor races, shelter gatherings,
two-creature catch, group presentations, hatch days, quiet huddles, and late-night sleep piles.
Local calendar eligibility uses the system offset when available and UTC otherwise. Hiding,
pausing, dragging, tossing, or changing the supporting display geometry discards the plan and
schedules a deterministic two-to-six-hour retry. No ritual history, path, animation, asset, or
dedicated update loop is created.

## Desktop topology

`DesktopTopology` is a runtime-only projection of the same privacy-safe window rectangles already
collected by the platform adapter. It sorts and truncates visible windows to 64, hashes their keys,
bounds, and z-order, and rebuilds only when that hash changes. A rebuild derives at most 96 isolated
island, exposed-corner, and slow-platform landmarks. Negative virtual-desktop coordinates and DPI
scales remain logical geometry; no raster content or title enters the projection.

A global bounded dwell record recognizes a cursor invitation only while the pointer remains within
24 logical points of a ledge for 1.5 seconds below 25 points per second. Hiding or pausing clears it.
At ordinary action boundaries, sufficiently trusting creatures can reuse `Perch` to approach the
ledge or `InspectScreen` to peek toward the cursor or an exposed corner. Window islands receive a
small bounded exploration preference. Existing window attachment and `RideWindow` behavior remain
responsible for calm moving platforms, and the existing 60-second observation projection records
successful riding time. The topology, dwell, landmarks, and previous layout never enter the save.

The same bounded windows form a runtime graph for v0.43. Vertically separated windows become tier
edges when their horizontal overlap is wide enough; horizontally separated windows become narrow-gap
edges only at 10–28 logical points with at least 64 points of shared height. Breadth-first search
retains the best deterministic path of at most four edges. Learned climbing, exploration, cursor
trust, a live invitation, and the compact preferred-region candidate adjust route scoring.

`WindowRoutePlan` holds the current geometry hash, remaining hops, and one repair bit. Each tier delegates to
the existing hop/climb journey. A gap uses `SqueezeWindow`, whose body action maps to the existing
traversal clip while the GPU narrows its body and layered face quads to 0.72×. Every hop retains both
supporting rectangles. Unrelated topology changes preserve valid steps. An affected route resolves
a safe support, then may attempt one replacement route with a brief reorientation; another change
cancels it. Squeezes include a short inspection at entry and exit.
Routes, graph edges, support rectangles, and progress are never serialized.

## Colony objects

Save version 9 adds one bounded `ColonyObjectState`: at most eight typed objects, the next UTC
arrival timestamp, and a deterministic ordinal. Each object retains only a stable ID, kind, display
key, normalized position, and semantic role. A named seed stream selects one of eight kinds every
three to seven days; overdue processing creates at most one object and schedules the next timestamp
from the current maximum-seen UTC value.

Object positions resolve on the village ground line whenever the world ticks. One pure layout
function walks outward from the house, interleaving companion cottages and loose objects so the two
can never occupy the same lot, and the same walk drives rendering and nearby utility using the
home's corner, display, scale, and accessible region. Lots without room remain stored but hidden;
all objects hide while the house is inactive. Legacy normalized positions are rewritten to the
village. The renderer builds one 128×16 seed-derived atlas, retains at most eight quads, and
rebuilds those vertices only when object state, cottages, home state, habitat, display geometry, or
scale changes.

Dwellings come from one 128×128 village atlas: a two-by-two grid of 64-pixel cells, three of which
carry art — the decorated colony house, a companion cottage, and a mini's cottage. Each is drawn
into its own cell-sized tile so art that would overrun a cell is clipped exactly as it is for a lone
shelter, and the fourth cell is deliberately left empty. The first colony member shares the
colony house and each later one adds a single quad sampling its cell, so a full village is four
quads against one texture and bind group. Shelter decorations resolve their attachment points from
the style's own silhouette — peak, eaves, wall, and ground line — so a banner hangs from the real
roof rather than a shared canvas height.
Nearby semantic roles add a bounded `+0.25` to existing action utility at ordinary selection
boundaries; objects have no physics body, interaction proxy, action state, or update loop.

## Growing shelter

Save version 10 nests one bounded `ShelterDecorationState` inside the existing home: at most six
unique typed decorations, one next UTC timestamp, and one ordinal. Every four to nine days, compact
memory counters, canonical bond scores, the last ritual kind, and colony-object kinds contribute to
six deterministic decoration scores. A named seed stream breaks ties, the highest unused kind is
stored, and an overdue colony schedules from the current maximum-seen time after adding at most one.

`ShelterRenderer::render_with_decorations` draws leaf, banner, stone, flower, lamp, and roof ornament
pixels onto the same deterministic 64×64 CPU canvas after the shelter genome is resolved. The GPU
shelter cache key contains only that genome and the bounded decoration list. A state change replaces
the single shelter texture; normal presentation still uses one shelter quad, one bind group, and one
draw call. Decorations have no world position, action, editor, animation, physics, or render loop.

## Exact offline seed sharing

`CreatureOrigin` remains separate from mutable colony order. Its 256-bit source seed and original
generation are encoded with format version 1 and a four-byte domain-separated SHA-256 checksum into
60 Crockford Base32 characters, displayed as fifteen four-character groups after the `FORMIGA`
prefix. Decoding is case-insensitive but requires canonical grouping, alphabet, zero padding,
version, generation 0–3, length, and checksum before returning a typed `SharedCreatureSeed`.

Import reconstructs every source generation from the original named seed streams, including the
generation-zero parent needed for inherited traits. The resulting creature retains exact innate
appearance, personality, scale, ID, and behavior seed, but becomes colony order zero with a fresh
birth and compact history. A domain-separated derived colony seed drives its new shelter and future
companions, so the source lineage cannot reproduce itself. Import allocates no service, socket,
worker, or persistent code cache. Modular creatures use format version 2: the same seed and
generation followed by a 16-byte bounded design, four reserved zero bytes, and a four-byte checksum.
Its 57-byte payload is 92 Base32 characters in 23 groups. Legacy format 1 is unchanged; a recipe
is applied after legacy named-stream reconstruction so inherited traits and personalities replay
exactly. Design bytes also participate in the imported colony's lineage hash.

## On-demand creature cards

The Colony profile emits only a creature ID when the user selects export. A native save dialog runs
before any art allocation; cancellation ends the operation without rendering. Once a destination
exists, a stateless CPU renderer creates one 960×600 opaque canvas, rasterizes a fresh font atlas,
draws the creature's existing procedural greeting frame into palette-derived pixel scenery, and
writes one RGBA PNG. Every temporary value is dropped before returning, and no card texture enters
the overlay GPU cache.

Card fields are derived directly from the selected creature at export time: custom name, family, up
to three already-promoted profile descriptors, UTC birth month/year, colony order, and an
abbreviation of the existing share code. The encoder adds no text chunks or application metadata.
It never receives memory payloads, relationships, full seed text, screen geometry, device data, or
the save file itself. Export is therefore read-only and leaves the save untouched.

## Reference-guided generation and colony roles

The desktop host opens a user-selected PNG or JPEG only after an explicit Creature Studio action.
Decoding is capped at 16 MB, 4096 pixels per dimension, and 16 million pixels. The image is reduced
to a maximum 64×64 analysis surface without distorting aspect ratio. Alpha-aware foreground cues
summarize aspect, occupancy, symmetry, and upper/lower/side extensions. A fixed 512-bin color
histogram chooses dominant coat and contrasting accent colors. Exactly 512 named-stream candidate
recipes are adapted to these cues, rendered, and scored. The chosen preview contains its generated
creature, seed, 16-byte recipe, and display-only affinity score; image bytes, path, metadata,
analysis pixels, and feature vectors leave scope after matching. This is not semantic recognition.

Save version 12 adds optional `CreatureDesign` recipes to appearance and immutable origin. New
generation uses an independent `modular-design-v1` stream, leaving legacy gene streams intact.
Five body plans and six ear styles compose independently with bounded tails, proportions, muzzle
patches, markings, and two RGB colors. The blob plan draws no separate head and carries the face on
its body; it occupies the last index so earlier recipes and codes decode unchanged. The renderer reserves a large face with round eyes and a
mouth, draws connected rounded bodies and paired limbs, and keeps appendages in the existing
48×48 frame. Mini bodies scale down while retaining readable large faces. Colors are softened
once during atlas construction, with a fixed dark outline and eye color. Recipes without image
guidance use the same grammar; minis inherit body plans and gently varied parental colors.
Absent recipes preserve legacy rendering and version 1 seed codes. Preview acceptance carries
the exact recipe into add/replace; cards and overlays share the same palette resolver.

Save version 11 gives every creature a typed adult or mini role, a persistent Keep flag, and a
two-bit adult mini-arrival projection. Total colony size remains four, adult count is capped at
three, and normal generation limits each adult to two minis. Parent assignment is deterministic and
balanced by mini count, adult birth time, and colony order. Rebalancing is role metadata only—it
does not rewrite a mini's identity, genome, name, birth, or learned state. Legacy v1–v10 saves retain
every creature; the first becomes an adult and later members become grandfathered minis without
enforcing a destructive cap.

Add, replacement, removal, and bulk regeneration execute on the existing `World` and immediately
normalize the at-most-six pair records and bounded runtime maps. Replacement requires an unkept
target and starts a fresh full-size identity while preserving the occupied location. Removal cannot
delete the final adult; orphaned minis are reassigned to remaining adults. The one-month calendar
milestone selects a full-size adult when an adult slot exists, while one-hour and one-week arrivals
remain minis.

## Runtime cadence

- Simulation and cursor sampling: adaptive 4–20 Hz; spatial movement remains 20 Hz.
- Presentation: 20 Hz for movement and each authored clip's native 2–12 Hz for pose-only activity.
- Full-screen or empty monitor overlays stop presenting until they become visible or dirty again.
- Window geometry: 4 Hz while active, 1 Hz at rest.
- Behavior selection: action boundaries, capped at 2 Hz.
- Ambient countdowns: inspection 2–4 minutes per creature, dangling 4–8 minutes per perched
  creature, and discovery 10–20 minutes per colony; countdowns stop while paused or hidden.
- Experience observations: one summary per creature per 60 active visible seconds; no new loop.
- Calm proximity: accumulated from those same summaries and projected once per five active minutes;
  no separate pair polling loop.
- Ritual eligibility: checked only at existing action-selection boundaries after one persisted
  12–48-hour timestamp becomes due; at most one runtime plan exists.
- Desktop topology: rebuilds only after the existing bounded window-geometry input changes; cursor
  invitation dwell advances on ordinary visible, unpaused simulation ticks.
- Colony objects: one persisted three-to-seven-day timestamp evaluated during the existing world
  tick; static vertices rebuild only after object, habitat, display, or scale changes.
- Shelter decorations: one persisted four-to-nine-day timestamp evaluated in the same world tick;
  the existing 64×64 texture is regenerated only when the bounded decoration state changes.
- Seed sharing: encoding, validation, and import run only on an explicit settings action; there is
  no idle work, background task, or network operation.
- Creature cards: the save dialog, CPU canvas, font atlas, and PNG writer exist only during an
  explicit export; cancellation performs no render.
- Reference matching: one bounded synchronous decode and 512-candidate procedural search occurs
  only after file selection; no reference worker, cache, texture, or idle task persists.
- Display reconciliation: every 2 seconds.
- Persistence: transitions, settings changes, and every 30 seconds.

State uses a versioned JSON file written by temporary-file, flush, atomic replace, and one backup.
Version 14 migrates v1 habitat settings, deterministically resolves v2 face/forelimb/effect genes,
assigns v3 colonies a deterministic shelter, gives v4 creatures stable birth timestamps, upgrades
v5 habits to the twelve strongest numeric routines, and converts v1–v6 relationship floats into
canonical shared four-score records. A v7 colony keeps those canonical records byte-for-byte while
receiving only its first deterministic ritual timestamp; v8 receives only its first deterministic
colony-object timestamp, and v9 receives only its first deterministic shelter-decoration timestamp.
v1–v10 creatures receive adult/mini role metadata, Keep protection, and disabled legacy mini
schedules without replacement. Migration preserves creature IDs, resolved genomes, personality,
and absent modular recipes; v11 colonies retain their legacy appearances. Loose object positions
are reconciled to the house yard on the next tick. Migration also preserves
custom names, birth times, memories, tendencies, routines, positions, and settings. Raw memory plus
tendencies stay below 192 bytes per creature; their serialized incremental state stays below 2 KiB.

Additional minis are earned after one hour and one week; a full-size adult is earned after one
calendar month when an adult slot exists. End-of-month dates clamp in UTC, overdue reveals remain 15
seconds apart, and the colony remains capped at four. Interrupted ambient, interaction, and toss
actions reload as idle. Home and birth timestamps survive relaunches, and clock rollback uses the
maximum-seen UTC guard. There is
deliberately no history database or telemetry layer. Update preferences live in a separate
`updates.json` file so network policy and check timing cannot alter a colony save.


## Native colony interface (save v14)

`clubhouse.rs` holds only on-demand UI artwork and interaction state; `settings.rs` owns its egui
window and presentation. Four static portraits, four eight-frame candidate strips (six walk frames
and two expressions each), the village atlas, one object strip, and the scrapbook's eight trinket
drawings fit within 416 KiB of artwork textures. The home preview draws the whole corner from the
village and object atlases the desktop already samples, positioned by the very layout functions
the overlay uses, so a complete village costs the same two textures whatever its size and nothing
about looking at it calls the colony home or moves a creature.

Themes and text scaling live entirely in `configure_style`, which the window re-runs when the
saved preference changes or, under "match system", when the platform's own appearance changes.
Every colour the pages draw reads from one palette so a single switch moves the whole window
between cream and charcoal, and keyboard focus is drawn in the accent rather than the platform's
fainter default. The existing egui font atlas and window/GPU resources are separate. Artwork is regenerated
only when identity/design or home decoration state changes and released when the window closes.
Playback selects cached UVs at six fps only on the visible Studio page with explicit animation enabled;
reduced motion suppresses playback. Repaint deadlines join the existing event-loop deadline and are
cleared when hidden/occluded. No extra thread or timer loop is introduced.

`CompanionState` adds a maximum 64-entry typed journal, onboarding flag, optional quiet expiry, and
two optional behavior presets. Projection reuses existing events; continuous observations never
enter the journal. A close friendship is recorded only when affinity crosses the existing close-bond
threshold. Repeated equal moments are throttled for six hours. Quiet mode holds the existing home
cycle until its deadline, then resumes ordinary behavior without rewriting preferences.

Save version 14 adds four bounded keepsakes to `CompanionState`. **Pins** are at most eight
`PinnedMoment` records, each naming an existing journal entry by its timestamp, creature, and
typed moment rather than copying its words; a pinned entry is shown once, above the rolling list,
and never repeated inside it. The **scrapbook** holds at most one `ScrapbookRecord` per trinket
variant — the variant number itself is the stable identifier — recording the first find with its
date, the finder, and the finder's name at that time, so a record still reads after that companion
leaves without retaining a copy of the creature. **Appearance preferences** are the theme choice,
a text scale from 100 to 150 percent, and the optional sprite outline; they affect only the
settings window and the baked sprite atlas, never behavior. A **routine schedule** is an opt-in
list of at most fourteen `ScheduledTransition` rows, each a weekday bitmask, a local minute, and
which of the two saved presets to move to. Migration from v13 leaves every one of them empty:
nothing is derived from a legacy discovery count, and no earlier identity, bond, recipe, setting,
or journal entry is touched.

`RoutineSchedule::intended` answers with the most recent transition at or before the current local
time, looking back one week, so only the state intended right now is ever applied — a machine that
slept through several transitions wakes into today's routine rather than replaying them. Two
transitions at the same minute are settled by their order in the list, so the answer never depends
on when the question is asked; a local hour that a daylight-saving jump skips becomes current once
the clock is past it and applies exactly once; and a timezone change is simply a different local
time read from the same instant. The world applies it inside the existing tick, adding no timer,
thread, or wakeup of its own. A routine that has never been saved, or one whose custom habitat no
longer leaves anywhere to stand, is skipped without reporting a change. A manual choice holds
until the next transition crosses, which is also what ends it; handing the routine back applies it
at once. Visibility, pause, and the separate quiet expiry are never a schedule's business.

Decoration visibility is six bits on the home. The village texture is rebuilt only when
its visible decoration set changes. Object order is the existing object vector: changing its order
moves keepsakes between the same bounded village slots and existing nearby-utility influences.

Shared adoption reconstructs the exact source generation before assigning a local colony slot and
fresh history. Capacity, Keep, duplicate identity, and mini reparenting are enforced before mutation.
The rest of the colony is preserved.

Persistence accepts save versions 1–14: version 14 is read directly, versions 1 through 13 are
migrated on load, and anything else is refused. A missing primary can load its backup; a corrupt
primary is preserved before repair, without rotating over a valid backup. If both files fail, the
host disables writes and presents recovery choices. Explicit restores and resets preserve uniquely named copies;
snapshot imports validate bounded input before confirmation and replacement.


### Geometry attention and local approaches

`GeometryObserver` compares actual native scan timestamps and at most 64 window rectangles, with
16 coalesced, expiring signals. A change counts as movement only when both opposing edges travel
together, so dragging one edge reads as a resize and a snap moves by its smaller edge displacement.
When a native identifier changes while a frame stays identical, and the pairing is one-to-one, it is
the same surface: no appearance/disappearance pair, and `update_surface_attachments` transfers the
creatures standing on it. Cached simulation snapshots do not count as fresh evidence.
Unreliable scans, observation gaps, monitor changes, and wholesale window-set changes reset the
baseline; a disappeared window requires another fresh scan before it can invite a search.

Every behavior added in this area is held to one contract. A sequence has eligibility rules,
deadlines for each stage, cooldowns, an interruption path, and a valid place to rest or land when
it is cut short; habitat, occlusion, reduced motion, quiet mode, and the existing preferences apply
to all of them, and decorative drama never delays recovery from invalid geometry. Physical comedy
is a library of short, authored, interruptible sequences composed from shared stages, routes,
poses, and outcome cues — not a physics simulation. A failed attempt is brief, harmless, and
recoverable; there are no collision bodies for windows, no coupled ragdoll for a shared tumble, and
no universal collision solver, only reserved standing and landing spots and creature-scale contact.
A spectator is always an existing colony member, and multiplayer means the colony playing together,
offline. Creatures observe only window rectangles and stacking order, monitor geometry, and cursor
motion, and they are not omniscient: a companion notices another creature's visible reaction
without detecting the desktop event that caused it, a startle propagates at most one hop, and an
observer's reaction never becomes a new spectacle for others. When a window stops being observed,
it is treated as a lost surface, without claiming to know whether it closed, minimized, was covered,
or moved to another workspace. Busy-desktop exploration follows usable geometry — exposed, reachable
tiers — rather than the raw number of windows, and a creature racing the cursor never touches input
or leaves its habitat.

`world/attention.rs` coordinates one scene of up to four actors/observers. Notice, optional short
approach, reaction, and recovery reuse existing actions and transient `AttentionPose` face, gaze,
and gesture hints. Individual and colony cooldowns prevent continuous window drags from restarting scenes.
An eight-second, four-creature support memory lets a confirmed loss prompt one search even after
a recent ride. Safety interruption releases the audience immediately; normal completion permits
a brief recovery. Grabbing, sleep, journeys, social plans, home, quiet mode, and hidden/paused state
retain priority over ambient attention.

`world/attention/motion.rs` bounds an approach to three seconds and 120 logical points on its current
surface. Each step validates the habitat corridor, visibility, supporting ledge, companion spacing,
and existing journey landing reservations. A blocked or changed approach ends in place; this is
not a new general-purpose route planner. Reduced motion uses stationary gaze.

`AmbienceTracker` estimates exposed overlapping window tiers and free desktop area per display,
with at most eight display records. Geometry targets recompute on changed scans and ease over
several seconds. Small utility bonuses encourage available exploration; empty-space roaming does
not compete with a reachable ledge. No new persisted action codes or schema fields are required.


### Display and cursor attention

`world/attention/displays.rs` tracks at most eight stable display keys and retains one expiring
new-territory opportunity plus four pending reorientations. Display changes are processed before
route recovery can erase an old attachment. Removed/invalid supports recover immediately; valid
resting contacts survive resolution changes, and identifier/scale churn alone produces no novelty.
If a native key also changes while its occupied rectangle stays identical, geometry supplies a
conservative fallback: no new territory means no disappearance/discovery pair.

Discovery planning is attempted at most once per second during a 25-second opportunity. A route
joins two touching horizontal habitat regions through their shared seam and ends a short distance
inside the new display. Its two segments total at most 360 logical points and ten seconds of travel.
No route crosses a gap or excluded strip. Every step checks visibility, spacing/landing reservations,
and current geometry; the renderer changes displays only after the creature crosses the seam.
Reduced motion uses a stationary look toward the new territory.

`CursorObserver` stores one previous timestamped point and one local movement aggregate, not a
trail. Fresh normalized positions determine speed; large jumps, stale samples, unavailable input,
and display crossings establish a new baseline. Repeated turns within a 64-point region can invite
interest. Short cursor scenes share the existing actor/observer stages; only an investigation or
withdrawal recruits an audience. Learned trust shifts confidence by at most 0.2. Ordinary cursor
utilities and dwell invitations also respect validated input and the cursor-reactions preference.
Cursor sample timestamps and every new plan/observation remain runtime-only.

### Surface comedy and shared play

`world/surfaces.rs` retains at most four hangouts per creature, sampling resting dwell once per
second. Missing places decay quickly; only an established coarse region can reinforce the existing
saved preference. Relative height comes from the nearest exposed support below the current ledge.

`world/rides.rs` uses actual scan intervals, velocity changes measured at the rider's own contact
point, and capped ride intensity for four riders. The attention library authors balance, grip, dismount, boundary retreat, elevator, and
dizziness stages. Gap journeys validate runway, sampled arcs, and landing reservations; confidence
combines temperament, energy, and learned climbing. Close compatible edges use a walking bridge.
Marginal attempts catch and pull up, with one optional helper and a rare bounded shared tumble.
Tumbles and accidental launches reuse existing swept toss recovery without recording a user toss.
`FramePlacement::for_creature` blends foot and hand anchors identically for rendering and hit proxies.

Gap decisions are commit, hesitate, or refuse. `world/attention/hesitation.rs` gives a borderline
margin a look down, a step back, an approach, and at most two reconsiderations, decided privately
at the start from temperament; it then commits from the edge (preparation skipped, revalidated,
same origin, forced catch) or retreats. The scene is bounded at 16 seconds and never begins under
reduced motion. A retreat, an attempt invalidated mid-scene, a shared tumble, or an accidental
launch records one of four runtime setbacks for 90 seconds, which raise perceived risk and block an
immediate retry of the same gap. Sleepiness lowers confidence alongside energy, height, and width.

`world/attention/geometry_comedy.rs` measures converging window edges against the creature's own
possibly moving support, and only windows in front of that support can crowd it. One closing side
becomes a walk toward free space; both sides, or no room, become a crouch and a hop down to the
nearest exposed support below; reduced motion braces in place. Journey targets are optional so an
escape may land on habitat floor.

Every role a creature can take publishes the same stages — notice, prepare, act, the catch a
marginal landing turns into, and recover, the last carrying one of four outcomes: completed,
declined, slipped, or settled. `world/attention/spectacle.rs` is the whole of that vocabulary:
`cue` maps each role onto it, and observers are the one role that publishes nothing, because they
consume cues and must never feed another origin back into the scene. Roles whose own machinery
owns a stage — a journey, a tumble, a play scene — report that stage unchanged; the rest derive it
from how far through the plan they are. A dare publishes only notice and prepare, because the
answer itself is published by whichever role takes over. A watcher therefore reads a jump, a
rescue, a game, and a glance at the desktop through one grammar rather than ten.

An audience watches whatever its scene is actually about, and keeps up when that changes hands:
whoever holds the plaything, else whoever is leading — the runner, the one who is "it", the one
standing on the best spot — and only then whichever member is taking a turn. Hide and seek is the
exception that proves it: the audience follows the seeker, because a row of companions staring at
the hiding place would give the hider away. Watchers last as long as the scene they came for, so a
sixteen-second race is not won in front of an audience that was released at nine. A game is
ordinarily safe to watch, but a companion in the air over a gap reads as the risk it is, and a
timid watcher may look away from a leap in a game exactly as it would from any other.

Visible journey stages publish one origin to nearby spectators, and window, cursor, and display
reactions publish the same stages. Cues carry the time spent in the current stage, so watchers gasp
briefly at the start of a catch, escape, or tumble before settling into concern; reduced motion
keeps calm expressions. Each watcher registers a stage change in its own time — its recruitment
delay doubles as a reaction lag — so a colony never gasps in unison, and every watcher gets its own
short notice even if it looks up mid-event. A watcher facing the other way is slower to look up.
Whatever else happens, a watcher always sees how the scene ended. A helper releases its old viewing
reservation and must actually reach the assist point. Multi-step routes update the watched
destination only after reaching a valid previous step. Successful difficult landings, refusals,
catches, and slips have distinct audience outcomes.

`topology.rs` route planning takes each creature's own reach: the rise and drop it will take in one
step come from boldness, liveliness, learned climbing, energy, and tiredness, halved again for a
mini. Routes that reverse direction score below ones that keep going, and each step after the first
pauses to look before it is taken.

Contact is anchored rather than eyeballed. `FramePlacement` fixes the top of the body frame
against the simulation's contact point, so feet meet a ledge in every pose and a dangling creature
hangs from its hands; the native hit proxy and the review sheets take the same placement, which is
why a catch, a pull-up, a helper's reach, and a grip all land on the pixel the simulation thinks
they do. `PropAnchor` does the same for a carried thing: one explicit offset in front of the
holder's face, mirrored with facing, kept inside the frame so a prop is clipped and occluded with
whoever is holding it. A prop changing hands is then two anchors meeting, which reads as a
hand-off rather than as two objects swapping places in mid-air.

Routes are revalidated, never re-searched. A plan is re-examined only when the topology hash
changes, and then every remaining hop's rectangles are checked against the live snapshot, so a
stale rectangle can never be walked. A plan whose geometry still holds keeps going; one whose
geometry moved is dropped, the journey is settled safely, and exactly one bounded local
alternative may be planned — once, recorded on the plan itself, and only at that transition. A
second disruption cancels rather than repairs again.

`formiga-art`'s `MotionSignature` gives every creature a stable cadence and phase for walking,
resting, greeting, and recovery, derived from its identity and personality. The overlay, the
interaction hit proxy, and the review sheets all take their frame from it, so a creature's timing
is the same everywhere. One-shot clips keep their exact timing, no frame is added, and reduced
motion is unaffected.

`world/attention/play.rs` adds one transient encounter and one scene with at most four members.
A sustained visible gesture or mutual gaze can become an invitation; there is no random game-start
timer. Copycat recruits eligible observers in turn and remembers visited members, while a staring
contest ends when the less composed of the pair looks away, or early when something nearby
distracts one of them.

Eighteen kinds of scene exist, each described by its kind, a seed, per-member roles, an expiry, at
most one temporary plaything, four remembered points, and — for a race — the window it runs to.
`world/attention/games.rs` holds the ones that move: a chase that swaps roles on
contact, a procession that keeps station and releases stragglers, a dance circle that gathers on
staggered beats, a pile beside a resting companion, leapfrog using ordinary validated hops,
keep-away and tug-of-war over one toy, tag with an immunity interval, turns at a gap, a route to
copy, and a contest for the middle of a ledge. Walks carry a spacing factor so players may come to
creature-scale contact where an audience keeps its distance, and a scene may travel for up to
twelve seconds. Invitations can be declined, and a refused pair is left alone for 75 seconds.

`world/attention/geometry_games.rs` holds the three games about the desktop's own shape. A race
picks one finish line both runners can reach — the furthest window the first one's own route
planner will take it to, confirmed against the second one's own traversal ability — and each racer
then runs its own route, leaving the shared plan for the moment it crosses a gap and rejoining on
landing. The finish line is revalidated every tick, replanned when it disappears, and the race is
called off when no replacement is reachable. The-floor-is-lava starts when a playful pair is
standing at the end of a ledge with real air under it: members keep to the ledges, cross where
they can, and back away from a brink they cannot cross, showing their own feeling about where they
are standing rather than the shared scene mood. Nothing in it prevents a fall — touching the floor
simply ends the round, and the member who did keeps its plan for exactly as long as the reaction
takes. Hide and seek defines hiding as being out of a companion's line of sight, not out of the
user's: a creature is never sent somewhere the person at the desk cannot see it. The seeker counts
facing away, then searches from the one position it last actually had the hider in view, and the
memory refreshes only while the hider is on the same ledge, within sight range, and in front of it.

`world/attention/dares.rs` offers a gap someone just cleared to a watcher standing where the same
gap is reachable; it walks to its own edge and answers through the ordinary attempt machinery,
while the creature that landed becomes its audience. A gap landing steps further into the
destination when the usual spot is taken.

Sleeping companions are the subject of a scene, never a member of one: company creeps and freezes
around them, and a playful creature may pester a rested sleeper awake in capped bouts, emitting the
same interruption and waking events as any other interruption. A shared ride and a passing cursor
are handled inside the ride and cursor scenes that already own those creatures, rather than as
separate sessions. Actor/observer transfers share the existing
origin, reservations, and outcome contract. Needs, support changes, unavailable observations,
manipulation, and colony routines interrupt play. Each kind carries its own length, from 3.2
seconds for a copied gesture to twenty for hide and seek, with a staring contest settled somewhere
between 3.5 and seven depending on how steady the pair is; every scene is then followed by a
45-second play cooldown, and interruption never claims a completed interaction. Reduced motion
retains stationary looks and expressions. No game writes a journal entry.
