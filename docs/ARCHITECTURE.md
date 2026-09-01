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
- Display reconciliation: every 2 seconds.
- Persistence: transitions, settings changes, and every 30 seconds.

State uses a versioned JSON file written by temporary-file, flush, atomic replace, and one backup.
Version 8 migrates v1 habitat settings, deterministically resolves v2 face/forelimb/effect genes,
assigns v3 colonies a deterministic shelter, gives v4 creatures stable birth timestamps, upgrades
v5 habits to the twelve strongest numeric routines, and converts v1–v6 relationship floats into
canonical shared four-score records. A v7 colony keeps those canonical records byte-for-byte while
receiving only its first deterministic ritual timestamp. Migration preserves creature IDs, resolved genomes, personality,
custom names, birth times, memories, tendencies, routines, positions, and settings. Raw memory plus
tendencies stay below 192 bytes per creature; their serialized incremental state stays below 2 KiB.

Additional creatures are earned one hour, one week, and one calendar month after colony creation;
end-of-month dates clamp in UTC, overdue reveals remain 15 seconds apart, and the colony remains
capped at four. Interrupted ambient, interaction, and toss actions reload as idle. Home and birth
timestamps survive relaunches, and clock rollback uses the maximum-seen UTC guard. There is
deliberately no history database or telemetry layer. Update preferences live in a separate
`updates.json` file so network policy and check timing cannot alter a colony save.
