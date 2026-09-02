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

Blob, hopper, and soft-quadruped rigs share a stable two-eye face grammar. Every body frame records a
face anchor and family-specific forelimb targets. Authored clips manipulate those anchors, squash,
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
eight-slot trinket row. The body atlas remains exactly 90 unique frames because `Tossed` reuses the
dragged body clip. Runtime work normally selects two slots and draws two nearest-filtered quads;
discovery alone adds one temporary quad. The combined textures are exactly 1,161,216 bytes per
creature and are enforced below a 1.2 MB test limit.

`PetReaction` maps to the existing greeting body clip, so lived experience does not grow that atlas.
A newly earned profile descriptor may allocate one small blank thought-bubble texture for five
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
revision state. The event vector is runtime-only and is emptied by the desktop host; no coordinates,
cursor paths, window layouts, or historical events enter the save.

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

`WindowRoutePlan` holds only the current geometry hash and remaining hops. Each tier delegates to
the existing hop/climb journey. A gap uses `SqueezeWindow`, whose body action maps to the existing
traversal clip while the GPU narrows its body and layered face quads to 0.72×. Every hop retains both
supporting rectangles; any topology or support change discards the plan and resolves a safe support.
Routes, graph edges, support rectangles, and progress are never serialized.

## Colony objects

Save version 9 adds one bounded `ColonyObjectState`: at most eight typed objects, the next UTC
arrival timestamp, and a deterministic ordinal. Each object retains only a stable ID, kind, display
key, normalized position, and semantic role. A named seed stream selects one of eight kinds every
three to seven days; overdue processing creates at most one object and schedules the next timestamp
from the current maximum-seen UTC value.

Object positions resolve through the current habitat whenever the world ticks. A missing display or
invalid normalized point snaps to the nearest accessible floor or shelter area and rewrites the
compact position. The renderer builds one 128×16 seed-derived atlas, retains at most eight quads,
and rebuilds those vertices only when object state, habitat, display geometry, or scale changes.
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
worker, or persistent code cache and leaves the v10 save schema unchanged.

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
the save file itself. Export is therefore read-only and leaves save version 10 unchanged.

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
- Display reconciliation: every 2 seconds.
- Persistence: transitions, settings changes, and every 30 seconds.

State uses a versioned JSON file written by temporary-file, flush, atomic replace, and one backup.
Version 10 migrates v1 habitat settings, deterministically resolves v2 face/forelimb/effect genes,
assigns v3 colonies a deterministic shelter, gives v4 creatures stable birth timestamps, upgrades
v5 habits to the twelve strongest numeric routines, and converts v1–v6 relationship floats into
canonical shared four-score records. A v7 colony keeps those canonical records byte-for-byte while
receiving only its first deterministic ritual timestamp; v8 receives only its first deterministic
colony-object timestamp, and v9 receives only its first deterministic shelter-decoration timestamp.
Migration preserves creature IDs, resolved genomes, personality,
custom names, birth times, memories, tendencies, routines, positions, and settings. Raw memory plus
tendencies stay below 192 bytes per creature; their serialized incremental state stays below 2 KiB.

Additional creatures are earned one hour, one week, and one calendar month after colony creation;
end-of-month dates clamp in UTC, overdue reveals remain 15 seconds apart, and the colony remains
capped at four. Interrupted ambient, interaction, and toss actions reload as idle. Home and birth
timestamps survive relaunches, and clock rollback uses the maximum-seen UTC guard. There is
deliberately no history database or telemetry layer. Update preferences live in a separate
`updates.json` file so network policy and check timing cannot alter a colony save.
