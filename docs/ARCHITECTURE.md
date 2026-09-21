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

## How the simulation is laid out

`world.rs` holds `World` itself: its fields, `new`, `from_save`, `tick`, and the small helpers that
belong to none of the themes. Everything else lives in a child module named after what it is about —
`world/arrivals.rs`, `bonds.rs`, `bubbles.rs`, `colony.rs`, `discovery.rs`, `experience.rs`,
`generation.rs`, `home.rs`, `interaction.rs`, `journeys.rs`, `movement.rs`, `objects.rs`,
`offers.rs`, `rituals.rs`, `routine.rs`, `spacing.rs`, `visitors.rs`, alongside the existing
`rides.rs`, `surfaces.rs`, and the `attention/` family. Each module adds methods to the one `World`
type rather than owning state of its own, so there is still a single simulation object and a single
tick.

Tests live in `world/tests/`, one file per theme — `ambient`, `arrivals`, `bonds`, `bubbles`,
`colony_management`, `companion`, `discovery`, `experience`, `home`, `interaction`, `journeys`,
`misc`, `objects_and_decorations`, `offers`, `perches`, `rituals`, `spacing`,
`topology_and_attention`, `visitors` — with the shared desktop fixtures and colony builders in
`world/tests/mod.rs`. The split is behaviour-preserving: a differential harness ran five seeds for
18,000 ticks each against 0.57.1 and compared the event streams and serialized saves byte for byte.

Four questions that several modules had each answered in their own way are now answered once.

`SupportSpan` in `world/surfaces.rs` is the one primitive for what lies below a point: the
clearances, the surface kind, and the `relative_x` clamp live there, applied once, so a surface too
narrow to stand on yields no span at all rather than a backwards one. The three searches over it
keep their own question — a drop slides sideways and ranks by straight-line distance, a toss only
counts where its arc crosses and ranks by crossing order, a hangout looks down one column and ranks
by height — and `find_drop_support`, `find_swept_support`, `support_below`, and `drop_below` keep
their signatures.

`World::clear_runtime_plans` is the one "settle everything": window journeys and the routes they
belonged to, a throw still in the air, chosen actions, visits to another creature, and every
attention scene along with whoever had stopped to watch it. Creatures are left standing where they
are and nothing saved is touched; the caller decides where they go next. The village appearing, a
quiet spell starting, and **Gather Creatures** asked for at the desk are the same moment told three
ways, so all three call it rather than each clearing whichever plans its author thought of.

`replace_creature_at` is shared by adoption and by replacing a creature with a design. Both replace
in place, so the newcomer keeps the slot in `save.creatures` that the departing creature held —
which is both the draw order and the order the colony is listed in — along with its position, its
surface, and the minis that called it a parent.

`welcome_arrival` is the shared tail of both arrival passes, and both stagger through one counter,
so a calendar arrival and a mini due in the same tick take their turn rather than revealing on top
of one another.

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

Sixteen 16×16 trinkets — gems, keys, leaves, shells, charms, relics, and the conditional keepsakes
described under [conditional discoveries](#conditional-discoveries) — are rasterized once per colony
into a shared atlas rather than into each creature's own texture. `activity_variant` selects one only
while `PresentDiscovery` is active. There is still no inventory, runtime generation, or carried
collection; the only durable record is the scrapbook's one first-find row per variant.

The renderer caches one gaze-free 48×48 body atlas and one 16×16 layered face texture per creature.
The face texture contains eleven expressions, nine gaze directions, three eyelid states, and one
eight-slot trinket row. That row is still baked, at the same size and in the same place, but nothing
samples it any more: the overlay's discovery quad and the settings scrapbook both read the colony
trinket atlas instead. The body atlas holds exactly 124 unique frames: 90 for actions, because
`Tossed` reuses the dragged body clip, and 34 for ten gesture poses, laid out as ten columns by
thirteen rows. Runtime work normally selects two slots and draws two nearest-filtered quads;
discovery alone adds one temporary quad. The combined textures are exactly 1,529,856 bytes per
creature — 6,119,424 for a full colony of four — and are enforced below a 4,500,000-byte test limit,
raised deliberately from 1.5 MB so the pose vocabulary has room to grow without the budget moving
each time.

Gestures — cheer, gasp, cover, worry, crouch, heave, balance, reach, bop, and watch — are a runtime-only
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
seconds; descriptor text remains available only in the Colony profile. Only one sprout exists
globally, and the GPU texture and CPU pixels are dropped at expiry, so there is no idle sprout
resource. It is unrelated to the [thought bubbles](#thought-bubbles) that answer direct
interaction, which come from the shared UI atlas.

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
exposure detail. Proximity does not count while the home is out. The village seats every resident a
step from the one next door, so who is near whom there says where the village put them rather than
whose company they chose; the pair timers are left untouched rather than cleared, so an afternoon at
home neither builds a bond nor spends the calm minutes a pair had already gathered. Completed
targeted actions emit the other bond experiences. No encounter list, target route, object
ownership, or social history is serialized.

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

## The creature menu

A secondary click on a creature's existing interaction proxy opens one small strip above its head.
On macOS a primary click with Control held opens the same strip; the Control state is read from the
combined-session `CGEventSource` that `cursor_and_idle` already samples, because a proxy never takes
keyboard focus and so has no modifier state of its own. No new permission, event tap, or global hook
is involved. Only one menu exists at a time.

A colony member's strip holds Snack, Toy, Home, and Profile. A guest's holds Snack, Toy, then Stay
when `visitor_can_stay()` is true and Copy code otherwise, then Profile — the two share one cell, so
the other three never move under the cursor. Snack and Toy issue `WorldCommand::OfferSnack` and
`OfferToy`; Home issues `WorldCommand::SendHome`; a member's Profile opens the settings window on the
Colony page with that creature selected, and a guest's opens it on the Journal page, where the guest
book is. Stay calls `ask_visitor_to_stay`, logging a category and closing the menu if it fails. Copy
code routes through the application's only clipboard path — egui's, inside the settings window — so
the window appears on the Journal page with a "Visitor code copied" toast rather than a second
clipboard owner existing. The menu closes after any choice; the simulation answers with bubbles.

The strip is a hidden-not-dropped native window built from the creature-proxy recipe: borderless,
transparent, always on top, never activating. It covers only the framed body of the strip, so the
notch, the label tab, and everything around them stay click-through. macOS hit-tests the frame and
the shape call is a no-op exactly as it is for creature proxies; Windows clips the window with one
`CreateRectRgn`/`SetWindowRgn` region.

`MENU_RISE` is 35 art pixels above the crown of the head — the 21-pixel strip, a 1-pixel gap, the
9-pixel label tab, and 4 pixels of clearance — and is fixed whether or not an item is hovered, so
the strip never jumps as the cursor crosses it. With no room above, the strip flips below the feet
at `MENU_NOTCH_GAP` (15 art pixels) with the frame sprite vertically flipped. It is clamped
horizontally inside the monitor's usable bounds with the notch tracking the creature's centre, and
snapped to the sprite pixel grid, at every creature size and both integer scale factors.

A menu closes on a choice, on a second secondary click on the same creature, when the cursor stays
farther than 120 logical points from both the strip and the creature for 0.8 seconds, after 8
seconds without a hover, when the creature is dragged, tossed, hidden, paused, occluded, or gone (a
guest that left), when its monitor changes, and when the settings window *gains* focus — the
transition, not the standing state. There is no global click capture, and Escape is deliberately
unavailable, because a proxy never takes keyboard focus.

## Offers

`world/offers.rs` answers `OfferSnack` and `OfferToy`. The creature decides. A score adds want,
trust, and manner and subtracts tiredness; the chance of acceptance is
`logistic((score − 0.55) × 2.6)` clamped to `0.03..=0.97`. Want for a snack rises with low energy
and want for a toy with boredom and playfulness; trust combines learned `cursor_trust`, innate
sociability, and the difference between times petted and times tossed, capped at ±50. The roll is
drawn from a private per-creature stream, `SeedStream::new(behavior_seed).rng("offer", …)`, so
offering something never disturbs the behaviour RNG and the rest of the simulation runs identically
whether or not anything was offered.

Two states skip the roll entirely: `sleep_pressure` at or above 0.82, or an asleep creature whose
pressure is still above 0.35. Both show a Sleepy bubble. Three runtime cooldowns bound pestering —
a 6-second window against repeated offers, 90 seconds after an accepted snack, 45 seconds after an
accepted toy — and none of them is saved. An offer is refused outright, with nothing shown, for a
hidden or paused colony, an unknown or not-yet-arrived creature, one being dragged, tossed,
climbing, squeezing, or dangling, one mid-journey or on a route, one in a colony ritual, and one
walking home.

Accepting shows a Snack or Toy bubble and then the existing Eat or SoloPlay clip at its usual
duration; a sufficiently rested sleeper wakes, emitting the same `SleepInterrupted` and
`CreatureWoke` events any other interruption does. Declining shows Decline, or Sleepy when tired, or
Question for a toy offered to a creature whose playfulness is below 0.35; a timid creature — boldness
below 0.38 — shows Ellipsis for 0.55 seconds first. `WorldEvent::OfferAnswered { creature_id, kind,
accepted }` then nudges `cursor_trust` by +3 for an acceptance and +1 for a decline, and sociability
by +2 for an acceptance, through the same bounded `LearnedTendencies::adjust` a pet uses: ±100 mapped
to at most ±0.35 of utility, and reversible by handling the creature badly. No saved field is added.
A guest answers offers and learns nothing from them; nothing about a visitor is recorded. At home an
accepted offer is enjoyed at the doorstep through `begin_home_moment`, and a colony ritual will not
start while an offer response is still playing.

## Send home

`WorldCommand::SendHome` starts an ordinary home visit on demand rather than waiting out the
15-minute cooldown. It is the same visit in every other respect: the same 15-minute length, the same
cooldown afterwards, and the same ways of ending early. It is refused while paused, during an
interaction, and when no usable home display exists. Every member answers with a Home bubble.

## Thought bubbles

`world/bubbles.rs` keeps a runtime-only list of icons over creatures' heads. Nothing about a bubble
is saved, journaled, or counted. `BubbleIcon` has fourteen members: Heart, Snack, Toy, Home, Sleepy,
Surprise, Question, Ellipsis, Decline, Music, Dizzy, Hello, Sparkle, and Stay. A bubble lives 2.4
seconds, growing in over two 0.12-second steps and shrinking out the same way through
`BubbleGrowth::{Small, Medium, Full}`. Asking for the icon a creature is already showing holds that
bubble open rather than re-popping it; a different icon swaps in place. At most five exist at once,
none while the colony is hidden, and reduced motion skips the growth steps.

Every trigger is something the person at the desk did: a pet shows Heart, a pick-up Surprise, a toss
landing Dizzy, send home Home, an offer one of Snack, Toy, Decline, Sleepy, Question, or Ellipsis, a
visitor's greeting Hello, and a visitor agreeing to stay Stay. Nothing a creature does on its own
raises one.

The overlay draws bubbles from the UI atlas, anchored at the real crown of the creature's current
baked frame — the per-frame silhouette rows measured during the existing atlas bake — so a mini's
bubble sits as close to its head as an adult's. Bubbles are clamped inside the monitor and are
occluded and hidden exactly as their creature is.

## The interface atlas

One 256×80 RGBA texture (81,920 bytes) carries every bubble sprite in a 17×16 cell whose anchor
pixel (8, 15) sits just above the head, the menu frames for strips of two to four cells (16n + 2 art
pixels wide, 21 tall including the notch), six menu icons — Snack, Toy, Home, Profile, Stay, Copy
code — in normal and hovered states, and the pixel-font label tabs. It is built on first use,
released after 240 renders with neither a bubble nor a menu up, and released immediately when the
colony is hidden or the overlay is torn down. It costs one extra bind group and one extra draw call
inside the existing pass while something is up, and nothing at all otherwise.

`render` takes `(save, cursor, habitat_editor, windows, milestone, ui: OverlayUi { bubbles,
reduce_motion, menu: Option<MenuView> })`, and `needs_redraw` takes the same `ui_active` flag. While
a menu is open the host's tick interval drops to 50 ms; the menu is owner-initiated and closes
itself within eight seconds, so that rate is bounded by the interaction rather than by a timer.
`cargo run -p formiga-tools -- ui-sheet` draws the whole atlas for review.

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
island, exposed-corner, and slow-platform landmarks, and asks each window's neighbours all three of
those questions in one walk rather than walking them once per question. The window and landmark
lists keep the storage they have grown into between rebuilds. Negative virtual-desktop
coordinates and DPI scales remain logical geometry; no raster content or title enters the
projection.

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

Object positions resolve on the village ground line whenever the world ticks. Since 0.58.5 a
belonging is not a lot on the strip at all: one pure layout function walks outward from the house
placing dwellings, porches, and a keepsake tree at each end, and every belonging is scattered
inside one of the two trees' yards. The same walk drives rendering and nearby utility using the
home's corner, display, scale, and accessible region. Lots without room remain stored but hidden;
all objects hide while the house is inactive. Legacy normalized positions are rewritten to the
village. The renderer builds one 128×16 seed-derived atlas, retains at most eight quads, and
rebuilds those vertices only when object state, cottages, home state, habitat, display geometry, or
scale changes.

Dwellings and trees come from one 128×128 village atlas: a two-by-two grid of 64-pixel cells, all
four of which now carry art — the decorated colony house, a companion cottage, a mini's cottage,
and, since 0.58.5, the keepsake tree in the cell that used to be left empty. Each is drawn into its
own cell-sized tile so art that would overrun a cell is clipped exactly as it is for a lone
shelter. The first colony member shares the colony house and each later one adds a single quad
sampling its cell, and the two trees add one quad each, so a full village is six quads against one
texture and bind group. Shelter decorations resolve their attachment points from the style's own
silhouette — peak, eaves, wall, and ground line — so a banner hangs from the real
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

## The village yard

A dwelling's ground footprint, in shelter pixels, is 60 for the colony house, 46 for a companion
cottage, and 36 for a mini's. The cottages grew in 0.58.0 and are unchanged; the atlas is still one
128×128 texture and a dwelling is still one quad. What moved in 0.58.5 is everything around them.

One walk lays out the whole strip: `Tree(Outward)`, `Dwelling(0)`, `Porch(0)`, then `Dwelling(i)`,
`Porch(i)` for every later member, and `Tree(Inward)` past the last porch. `VillageLot::Object` is
gone — belongings are not lots any more, they live in the two trees' yards — so the walk is at most
`MAX_VILLAGE_LOTS` = 10 entries and is built in a fixed-capacity stack array rather than a `Vec`,
because the simulation asks for it several times a tick. A porch shares a lot line with its own
house, so its resident waits at its own door rather than a step down the lane. Lots that cannot fit
the house's accessible region stay stored but hidden rather than spilling elsewhere, and because
the trees are the outermost lots at both ends, a corner that runs out of ground gives up a tree
before a house.

The strip is tighter than 0.58.0's at every seam. `VILLAGE_GAP` is 3 shelter pixels rather than 5
— a visible seam at every scale the overlay draws at, and no more; the old 5 came to fifty-five
pixels of empty lane once the strip was laid end to end. `PORCH_WIDTH` is
`CREATURE_FRAME_WIDTH − 2 × REST_WALL_SLIVER` = 30 rather than 38, and is no longer tied to the gap.
`REST_WALL_SLIVER` is 9: the outermost pixels of a wall or an eave and the ground decoration
standing against it, which a resting frame may reach across and inside which no doorway ever sits
— a mini's cottage, the narrowest, has eighteen pixels between its door's middle and the edge of
its lot and a doorway five wide. `OBJECT_WIDTH` is 10 rather than 16: the drawn width of a
belonging, not the quad it is cut from. `REST_CLEAR_RATIO` is unchanged and still const-asserted
equal to `world::spacing::FACE_CLEAR_RATIO`.

The widest village — four adults, eight belongings, both trees — measures 445 shelter pixels end to
end, against a `VILLAGE_SPAN_LIMIT` of 448. 0.58.0's strip ran to 523, so this is 15% narrower
while gaining two trees. The houses and doorsteps between the trees account for 327 of the 445,
and four dwelling footprints are 198 of that: the buildings are the floor the whole thing rests
on, and everything saved came out of the air between them and out of the belongings, which cost
the strip nothing at all now.

A keepsake tree stands at each end of the walk, claiming `TREE_WIDTH` = 56 shelter pixels of lot.
The art reaches ±27 from the trunk across all nine lean-and-tilt combinations, so the lot is a
pixel of air past the widest thing in it. `HOME_EDGE_MARGIN` keeps its formula —
`DwellingKind::Main.width() / 2 + VILLAGE_GAP + TREE_WIDTH` — and so reserves the outward tree's
whole lot against the edge of the display. The inward tree needs no margin of its own: the strip
runs away from the edge, and a region that cannot take the inward end simply does not show it.

`TreeEnd::of_trinket(variant)` splits the catalogue in half: the eight `TrinketCondition::Anywhere`
finds, variants 0–7, hang in the outward tree by the colony house, and the eight conditional ones,
variants 8–15, in the inward tree at the far end. `TRINKETS_PER_TREE` is `TRINKET_VARIANTS / 2` = 8,
which is also the length of `formiga_art::TRINKET_ANCHORS` — four pairs mirrored about the middle
of the cell, highest and most central filled first — so neither tree can overflow, and a keepsake
depends on its variant alone, so it never moves once found and never changes ends when the village
mirrors into the other corner. `formiga_art::trinket_place(variant)` answers with the end and the
anchor as it is actually drawn there. Because the anchor set is mirror-symmetric, the inward tree
is the same atlas cell sampled with its horizontal UVs swapped: still one 128×128 texture and one
extra bind group, and every keepsake still meets the cord drawn down to it. The overlay draws one
16×16 quad per found keepsake from the colony's own trinket atlas, so the trees fill in exactly as
the scrapbook does. `colony_card.rs` draws both trees the same way, the inward one mirrored, so a
portrait is not left with orphaned clusters at each end.

Belongings alternate by slot — even slots to the outward yard, odd to the inward — four each, so a
colony with three things has two at one end and one at the other rather than a full yard and an
empty one. Slot to end is a pure function of the slot, so nothing hops ends when the next
belonging arrives. `BELONGING_SPOTS` is written once as `(end, toward, forward)` in shelter pixels
from the tree's own middle, positive `toward` being a step toward the houses, and the inward yard
mirrors it so a spot means the same thing at both ends. A yard scatters across about 40 shelter
pixels, well inside its tree's own 56, which is why the yards cost the strip no ground of their
own. `BELONGING_DEPTH` is 7, so some items sit behind the trunk and some a pixel in front of the
ground line, and a per-colony deterministic drift of a pixel either way keeps no two yards reading
the same without ever bringing two items within `BELONGING_CLEARANCE` = 6. No resting resident
stands on a belonging: the tightest case is the inward yard's outermost item, 31 shelter pixels
from the last resident, where half a frame plus half a belonging is 29.

`home_resting_position(home, slot, cottages, monitors, policy, display_scale)` places each member
beside its own door. No resting frame covers a door, or overlaps any house by more than a gap's
worth. When a porch does not fit — a narrow display, a habitat cut to a sliver — the member stands
on the free ground past the outermost lot that did fit, spaced by the face-clear ratio; when even
that fails, the whole colony is lined up from the region's far edge inward. Staying on the display
and out of one another's faces wins over a clear view of a house that display cannot show properly
anyway. `home_guest_position` stands a visitor past the outermost *visible* lot and past every
resting spot, plus a gap and half a frame, and returns `None` when it cannot keep face-clear
distance from the residents.

A visit is a tour rather than a stand. `plan_tour` asks the same layout functions where everything
is, turns each house, porch, resting resident and belonging into a span of ground the guest may not
stand on, merges those spans, and keeps the free gaps between them; the arrival spot is always the
first place on the ring, and at most `MAX_TOUR_STOPS` are kept. Every candidate is then re-checked
against the four rules it has to pass — inside the accessible region, clear of each house by the
same arithmetic that puts a porch where it is, at least the face-clear distance from every resting
resident, and clear of every belonging the colony keeps — and dropped rather than shaved if it does
not plainly pass. A full four-house strip has no legal ground between the houses at all, since two
bodies need more than the porches leave and what remains sits in a doorway, so the ring there is the
arrival spot and the trees' yards; a smaller colony threads the gaps as well. The guest walks the
ring repeatedly, spending a stay at each place: the first seconds are one small thing — going over
to whichever resident it has not met yet, looking up at the house beside it, stooping to a
belonging, or resting — and the remainder is the calm rotation a visit already had. It returns to
the arrival spot before the departure lead, so the goodbye and the walk out happen from where it
came in. A greeting at a stop prompts that resident to answer through the same `ResidentAnswer`
machinery the hello uses, timed off one clock, so a resident turns, holds, and goes back to its
afternoon without leaving its door. The tour is replanned whenever the village moves underneath it,
and a visit still writes no bond, tendency, memory counter or journal line beyond the single visit
moment.

While the colony is home, each member has passive doorstep moments, drawn from the seeded
`home-moments` stream and held only in memory. The first comes 12–80 seconds into a visit and the
rest 60–180 seconds apart per creature, with at most one resident busy at a time plus one neighbour
answering a wave. The moments are Eat (6–12 s), Drink (5–10 s), SoloPlay (8–16 s), InspectScreen
(5–9 s, turning to look at its own house), Greet (4–7 s, with a neighbour within four frames waving
back), Sleep (60–150 s), and an errand: walk to the nearest belonging within 1.5 frames, pause 2.5
seconds, and walk back. None starts under reduced motion or while hidden, and one is cancelled at
once by a pet or a pick-up, by dismissal, by pause or hide, and by a changed display, habitat, or
scale. They are cosmetic by construction: a moment emits only `ActionStarted`, so no tendency,
counter, bond, or journal line moves, and petting a dozing resident at home costs it no sleep
security.

The layout allocates next to nothing, because the simulation walks it several times a tick.
`village_walk` is the fixed-capacity stack array above, `colony_cottage_list` picks the members in
place, `home_resting_position` and `home_guest_position` allocate nothing at all,
`VillageGround::resolve` lists the habitat's accessible regions once and keeps the one the anchor
landed in rather than listing them again for every lot, and `home_object_positions` resolves the
village once for all eight belongings. `reconcile_colony_objects` went from roughly 34 allocations
a tick to 1, and `tick_homebound_creatures` from roughly 76 to 8.

`home-yard-sheet.png` shows eight cases at both corners: four shelter styles with a grown colony
and the trees filling up as the scrapbook does — nothing found, five, eleven, all sixteen — then
colonies of one to four with their residents on their porches, ending on the widest village there
is. `shelter-sheet.png` gives each style a lane — plain house, decorated house, cottage, mini, and
a creature at the same scale.

## Visiting creatures

`world/visitors.rs` and `visitor.rs` add one saved `VisitorState`: the guest, if any, a count of how
many home gatherings the colony has held, and a guest book. Whether a wanderer turns up at gathering
`n` is `SeedStream::bytes("visitor-cadence-v1", n / 4)[0] % 4 == n % 4` — exactly one gathering in
every block of four, at a place in the block taken from the colony's own seed. Successive visits are
therefore one to seven gatherings apart: two can fall back to back where one block's slot is last
and the next block's is first, but a third never can, and no drought runs past seven. No wanderer
arrives while a guest is already present or
when the village has no room to stand one. A brand-new colony's very first gathering is not counted,
because `World::new` activates that home directly.

A wanderer is a freshly generated creature — seed derived from the colony seed and the ordinal, a
random modular design, a cosy name, usually an adult and less often a mini — never a copy of a
colony member. Its visit runs on a timeline: the home appears, 12 seconds pass so residents can
settle, it walks in along the floor from the far side (capped at 520 points and 40 seconds), greets
with one Hello bubble for 3.2 seconds, then alternates 15-second idles with 6.5-second beats of Eat,
PresentDiscovery, SoloPlay, or Perch until 90 seconds remain, says goodbye with a greet and a reach
for 2.6 seconds, and walks out — gone around 825 seconds into a 900-second gathering. Residents
answer the hello with a turn and a runtime pose only, staggered by sociability: a bop from a playful
one, a reach from a bold one, a look from a timid one. Everyone resting at the village answers. No
resident moves, and no bond, tendency, or memory counter changes; a control gathering with no
visitor is asserted to project identically.

Interruptions behave like every other scene. Pause freezes the visit; hidden runs it undrawn; quiet
mode still lets the guest leave on schedule; reduced motion drops the walk in and out, holds the
hello stationary, and shows no gestures. Dismissing the home takes the guest off the desktop at once,
signing the book only if it had already said hello, and a lost display or habitat ends the visit
immediately. A pet holds the scene for the `PetReaction`, and an accepted snack or toy holds it for
the clip. A relaunch leaves the guest waiting off stage to walk in again next gathering, and a clock
rollback cannot extend a stay, because the deadline is measured against `maximum_seen_utc`. A guest
can be petted and offered things, but never picked up: a drag on a guest is a pet, and it never
dismisses the home.

`World::invite_visitor(shared, now, desktop)` accepts a friend's seed code. It refuses a code that
is already a colony member (`VisitorError::AlreadyHome`) and one offered while somebody is visiting
(`GuestPresent`). An invited guest stays 24 hours and attends the houses for every gathering in that
time; a gathering starts right away so the friend turns up promptly, and the book is signed once for
the whole stay. `VisitorState.guest_book` holds at most 24 entries, oldest dropped, each with the
visit timestamp, the name, the origin — the same appearance-and-temperament seed a share code
carries — and whether the visitor was a wanderer or invited. One `JournalMoment::Visit` is recorded
per visit.

`visitor_can_stay()` answers whether the colony has room: fewer than four members, fewer than three
adults, and not a duplicate of somebody already here. `ask_visitor_to_stay(now, desktop)` then runs
the exact adoption path an imported creature takes — a fresh history, standing where the guest stood,
and a Stay bubble. `visitor_share_code()` returns the guest's code. The settings Journal page carries
the guest book, newest first, with Copy code on every entry and the current visitor on top with Ask
to stay beside its code. The Creature studio's "Adopt a shared companion from a code" offers "Invite
for a day" from the same explicit preview that adoption uses, disabled while somebody is visiting or
when the code names a creature already at home.

## Keeping faces clear

`world/spacing.rs` enforces one rule: no creature's face stays covered by another creature for
longer than a moment. Resting arrangements stay shoulder to shoulder, and awake idle companions
separate fully.

The model is evidence, not art: `formiga-core` cannot depend on `formiga-art`, so the boxes it
spaces by were measured. Every creature draws as a 48×48 frame centred on its contact x, and the
overlay draws in `save.creatures` order, so only an earlier creature's face can end up behind a
later one's body. A drawn face reaches at most 15 art pixels from the frame centre — the watching
pose on a long body sets that; every other clip stays within 14 — and a drawn body at most 23. Their
sum, 38 of 48, rounds up to `FACE_CLEAR_RATIO = 0.80`, which still lets bodies overlap by about 8
pixels; twice the body box, 46 of 48, rounds up to `FULL_CLEAR_RATIO = 0.96`, at which no pixel is
shared. The art test `every_face_and_body_stays_inside_the_boxes_the_simulation_spaces_by` keeps
those two numbers honest, sweeping every body plan at four logical sizes that bracket every size a
colony can hold, across every baked clip, every frame, every expression, eyelid and gaze, and both
facings. The village's `REST_CLEAR_RATIO` is const-asserted equal to the face-clear ratio.

Most of the work is at the source sites rather than in a corrector. Bond staging offsets moved from
flat 28/26/30/37-point constants to `frame_width × 0.80` for greeting, social play, and sleeping
beside, `× 0.88` for a presentation, and `× 1.00` plus a 5-point settle for a follow; the bond stop
distance moved from 42/30 points to 2 points from the mark, because the mark already carries the
spacing — which is what let a pair greet from ten points away. Ritual line-ups went from
`(width × 0.72).max(22)` to `width × 0.80` plus tolerance, race staggers use full spacing, and
attention viewing and approach spacing went from `(width × 0.65).clamp(24, 96)` to `width × 0.80`
with no ceiling. Held game formations — pile, dance circle, tug grip, ledge contest, prank, hush,
leapfrog landing — use a face-clear `held_gap()`, while momentary contact in tag, a catch, a
keep-away take, and a leapfrog run-up is unchanged, because those are contact and are meant to be. A
helper's assist spot is face-clear, two riders on one window are walked apart by `share_the_ledge`,
and Gather Creatures spaces everyone by the full-clear ratio instead of stacking them.

`resolve_overlaps` runs at the end of every ordinary tick as a safety net, over a bounded runtime
table of six pairs and four shuffles that is never saved. A covered face acts after 1.25 seconds and
plain crowding after 2.0; a 5-second per-pair cooldown and a 1.08 clearance margin stop it
ping-ponging. Who moves is decided by cost: never someone dragged, tossed, airborne, climbing,
hanging, homebound, or owned by a scene; +8 for being asleep and up to 4 more the longer it has
slept, so the lighter sleeper moves; +7 for a ritual participant, +6 for a pile anchor, +5 for
holding a prop, +2 for a bond plan; ties by id. An awake creature takes an ordinary short `Traverse`
to the nearest spot that clears everybody, respecting habitat, ledge extent, and reservations;
sleepers and ritual participants get a quiet position-only shuffle that emits no `SleepInterrupted`,
`CreatureWoke`, or `CreatureRested`, and under reduced motion a sleeper is simply placed. If no spot
clears every face and the situation has lasted five times the grace period, the creature leaves the
ledge through the existing descent journey. No event is emitted for a sidestep. A user drag or toss,
an airborne creature, a climbing or hanging one, and a play scene inside its own bounded deadline
(at most 21 seconds) are exempt. So is a creature still walking to a spot it chose for itself: the
resolver would drag it off the mark while its own walk pushed back, the two moving it a pixel at a
time in opposite directions, and once it arrives it is standing still and can be asked properly.

Nothing that moves a creature may step past what it was aiming at. A walk covers one to three
points in a tick, so a target with no stop distance around it is overshot, turned toward, overshot
coming back, and the creature shivers there for the rest of the action — which shows up beside a
companion, because a mark beside a friend is a spot a creature actually reaches. `execute_action`
never steps further than the distance that is left, stands still once it is within a step or the
action's own stop distance, and turns a companion-seeking creature to face the companion rather
than flickering with the last fraction of a point. `motion::step` already clamped the same way. The
third source of the same picture was a play scene working its goals out afresh every tick: a runner
with no room ahead was sent back past its chaser and away again the tick after, so `steer_play` now
keeps the way a player is going until it gets there and refuses only a reversal, which leaves a
chaser free to track a lead that keeps running. `nobody_shivers_on_the_spot_when_a_companion_is_near`
pins the result at no more than six turns in any one second, and fails on each of the three causes
on its own.

The acceptance test runs four seeded four-creature colonies, minis included, on a synthetic desktop
whose three windows slide and resize continuously, for 1,400 ticks each, sampled every tick. It
asserts the bound the rule implies rather than a recorded number: the grace period plus four
seconds, which is the longest an ordinary walk takes to carry a companion a frame and a half out of
the way. When the work was done, the worst face-cover episode measured 5.55 seconds before and 2.20
after — 3.60 with the source-site spacing alone — and the worst crowding episode 0.00.

## Conditional discoveries

`world/discovery.rs` reads a find's circumstances from state the simulation already holds. Nothing
new is observed and nothing new is saved: the scrapbook still records only what, when, and who —
variant, first find, finder, and the finder's name at the time — and never why a trinket qualified.

- **Night** is the user's local hour outside `6..20`, through `after_dark(local_time_or_utc(now))`.
  It is deliberately wider than the late-night ritual's 22–05, which is about the person being up
  late rather than about the world being dark.
- **High up** is standing on a window ledge with at least 140 logical points of clear drop beneath
  it, measured by the same relative `surfaces::drop_below` the ledge and gap decisions use, so no
  absolute screen coordinate is involved.
- **Mid-ride** is a previous action of `RideWindow`, or `RideMemory::riding(id)` — measured motion,
  not merely standing on a window that happens to be still.
- **Beside a close friend** is a bond whose affinity is at least `CLOSE_FRIENDSHIP_AFFINITY` (112,
  the same threshold the journal uses to record a new close friendship) whose other member has
  arrived and stands on the same monitor, the same kind of surface, and the same window, within two
  frame widths (96 art-scaled points).

With no circumstance holding, the choice is uniform over the eight everyday trinkets, exactly as
before. Otherwise half the draws go to an everyday trinket and half to a qualifying conditional one,
and among the qualifying ones three in four go to those not yet in the scrapbook when any are
missing. Exactly one draw is taken from the ambient stream on every path, and the conditional
decisions run on a private generator keyed from that stream's state without advancing it. The
everyday sequence is therefore bit-identical to 0.57, and the rest of the simulation's randomness is
unchanged whether or not any circumstance holds — asserted across all sixteen circumstance masks.

Game playthings are kept out of this. Keep-away and tug-of-war still show the holder in
`PresentDiscovery`, but the plaything is always an everyday variant chosen from the scene's own seed
(`seed % 8`), set on every change of hands, never a conditional keepsake — and a game still reaches
neither the scrapbook nor the journal.

The catalogue itself lives in `formiga-core::trinkets`: sixteen entries of name, description, hint,
and condition, with `TRINKET_VARIANTS = 16`. `formiga-art::TrinketAtlasRenderer` bakes them into one
256×32 sheet — sixteen columns by two rows, a rest frame and a glint frame, 32,768 bytes — whose
inks are drawn from the colony seed and scored against every member's coat at once, so a keepsake
reads as a separate object whoever is holding it. A single holder's `prop_palette` keeps a belonging
90 away from that one coat; dodging a whole colony at once is necessarily a little softer, and the
tests report the worst distance rather than fixing a threshold, because the number that matters is
comfortably past the roughly 25 at which two colours start to read as the same.
The overlay's discovery quad and the settings scrapbook both sample it,
so a trinket costs one texture per colony instead of one row per creature. The scrapbook shows all
sixteen slots, undiscovered ones as a dim silhouette with the catalogue's hint.

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

### Stickers and the colony portrait

Two more exports follow the same contract: the native save dialog first, nothing allocated on
cancellation, every buffer dropped afterwards, nothing uploaded to the overlay GPU, and the save
untouched.

`StickerRenderer::render(&Creature, StickerClip, scale) -> Sticker` and `Sticker::encode_gif()`
produce an animated GIF of one creature. The clips are Walk (Traverse), Wave (Greet), Cheer
(`Gesture::Cheer`), Play (SoloPlay), Snack (Eat), Sleep, and Dance (`Gesture::Bop`); each carries its
matching expression plus one blink near the end of the loop, and Sleep keeps its eyes shut
throughout. Scales are 4× and 8×, with 8× the default. Timing is read from the creature's own
`MotionSignature` by walking its timeline a centisecond at a time, so the GIF's frame delays are
that creature's real cadence rather than a nominal frame rate. The encoder uses one global palette
with a transparent index, quantising gracefully above 255 colours, "restore to background"
disposal, an infinite loop, and no dithering, and it is byte-deterministic. A sticker file holds
only pixels: per-frame graphic-control blocks and exactly one NETSCAPE loop block, with no comment,
application, or plain-text extension — verified by walking the encoded GIF block by block. The
filename is `<Name>-<clip>.gif`, Unicode-safe.

`ColonyCardRenderer::render(&SaveFile) -> Canvas` produces one 960×600 opaque PNG: every member in a
friendly pose with its name on a cream plaque, the title and the month the colony began, two stat
chips for the member count and the number of family lines, and the village behind them, resolved on
a notional 1:1 desktop so no screen information enters the card. It contains no seeds, memories,
relationship scores, journal, display keys, visitors, or guest book, and the encoder adds no PNG
text chunks; a test maxes out memories, tendencies, and relationships and asserts the card comes out
pixel-identical. The default filename is `Formiga-colony.png`.

The Colony profile gains "Export sticker…" with a clip choice, and the Home page gains "Export
colony portrait…". The `gif` crate, already in the workspace for `formiga-tools`, is now also a
dependency of `formiga-art`, so the application and the tools share one encoder; it is the first
time it is linked into the shipped application.

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
- Behavior selection: action boundaries, capped at 2 Hz. The most expensive question an action
  asks — whether there is a window ledge within reach — is answered there, at the boundary that
  reads it, rather than for every creature on every tick of an action already under way.
- The colony picture each creature answers against — the creature and relationship views — is two
  buffers reused between ticks rather than two clones of the whole colony made afresh each tick.
- Ambient countdowns: inspection 2–4 minutes per creature, dangling 4–8 minutes per perched
  creature, and discovery 10–20 minutes per colony; countdowns stop while paused or hidden.
- Experience observations: one summary per creature per 60 active visible seconds; no new loop.
- Calm proximity: accumulated from those same summaries and projected once per five active minutes;
  no separate pair polling loop. It does not accumulate at all while the home is out, and the
  timers a pair had already filled are left as they are.
- Ritual eligibility: checked only at existing action-selection boundaries after one persisted
  12–48-hour timestamp becomes due; at most one runtime plan exists.
- Desktop topology: rebuilds only after the existing bounded window-geometry input changes; cursor
  invitation dwell advances on ordinary visible, unpaused simulation ticks.
- Colony objects: one persisted three-to-seven-day timestamp evaluated during the existing world
  tick; static vertices rebuild only after object, habitat, display, or scale changes. Resolving
  where the eight belongings stand resolves the village once, not once per belonging.
- Shelter decorations: one persisted four-to-nine-day timestamp evaluated in the same world tick;
  the existing 64×64 texture is regenerated only when the bounded decoration state changes.
- Seed sharing: encoding, validation, and import run only on an explicit settings action; there is
  no idle work, background task, or network operation.
- Creature cards: the save dialog, CPU canvas, font atlas, and PNG writer exist only during an
  explicit export; cancellation performs no render.
- Reference matching: one bounded synchronous decode and 512-candidate procedural search occurs
  only after file selection; no reference worker, cache, texture, or idle task persists.
- Thought bubbles: aged on the ordinary world tick; each lives 2.4 seconds and at most five exist,
  so there is no bubble timer or wakeup of its own.
- Creature menu: the host's tick interval drops to 50 ms only while a menu is open, and a menu is
  owner-initiated and self-closing within eight seconds.
- Offers: answered at the moment of the command, with runtime-only 6-second, 90-second, and
  45-second cooldowns advanced by the existing tick.
- Visitors: whether a wanderer comes is decided per home gathering from the colony seed and the
  gathering ordinal, never from a clock; a visit's own timeline advances on the ordinary tick and
  its progress is never serialized.
- Doorstep moments: the first 12–80 seconds into a home visit, then 60–180 seconds apart per
  creature, with at most one resident busy at a time; evaluated in the same world tick.
- Overlap resolution: checked at the end of every ordinary tick, acting after 1.25 seconds of a
  covered face or 2.0 seconds of crowding, with a 5-second per-pair cooldown.
- Stickers and the colony portrait: the save dialog, CPU canvases, and encoders exist only during an
  explicit export; cancellation renders nothing.
- Display reconciliation: every 2 seconds.
- Persistence: transitions, settings changes, and every 30 seconds.

State uses a versioned JSON file written by temporary-file, flush, atomic replace, and one backup.
Version 15 adds only `visitors`; a v14 colony receives an empty `VisitorState` and nothing else is
touched. 0.58.5 adds no saved field and needs no migration: the version is still 15, and the two
keepsake trees, where every find hangs, and where every belonging lies are all derived at runtime
from the colony seed and the scrapbook the save already holds. The chain below it is unchanged:
migration migrates v1 habitat settings, deterministically resolves v2 face/forelimb/effect genes,
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
and two expressions each), the village atlas, one object strip, and the one colony trinket sheet
that carries all sixteen trinkets fit within 432 KiB of artwork textures — raised from 416 KiB
because that sheet replaced eight separate 16×16 drawings with 24 KiB more pixels in a single
texture, and nothing else on any page grew. The home preview draws the whole corner — houses, both
trees, the keepsakes hung in them, and the belongings in the yards — from the village, trinket,
and object atlases the desktop already samples, positioned by the very layout functions the
overlay uses, so a complete village costs the same three textures whatever its size and nothing
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
moves belongings between the same eight fixed spots in the two yards and existing nearby-utility
influences.

Shared adoption reconstructs the exact source generation before assigning a local colony slot and
fresh history. Capacity, Keep, duplicate identity, and mini reparenting are enforced before mutation.
The rest of the colony is preserved.

Persistence accepts save versions 1–15: version 15 is read directly, versions 1 through 14 are
migrated on load, and anything else is refused. A missing primary can load its backup; a corrupt
primary is preserved before repair, without rotating over a valid backup. If both files fail, the
host disables writes and presents recovery choices. Explicit restores and resets preserve uniquely named copies;
snapshot imports validate bounded input before confirmation and replacement.


### Geometry attention and local approaches

`GeometryObserver` compares actual native scan timestamps and at most 64 window rectangles, with
16 coalesced, expiring signals. It matches each window to the last scan and counts it in one pass
rather than two, and asks whether a window moved before counting its neighbours. Its storage is
reserved once and is asserted at or below 9 KiB, measured with every capped list — the last two
scans, the frames the display preferences are sampled from, and the signals in flight — filled to
its cap rather than as it happened to be filled. A change counts as movement only when both
opposing edges travel together, so dragging one edge reads as a resize and a snap moves by its
smaller edge displacement.
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
several seconds. Whether the scan changed at all is a signature mixed the same cheap FNV-1a way
the topology's own geometry hash is, and a scan that did change gathers the visible windows once
into a reused list and sorts it front to back, so the search for whatever covers a window's top
edge stops at the frames it has already passed instead of reading the whole list again per window.
Small utility bonuses encourage available exploration; empty-space roaming does not compete with a
reachable ledge. No new persisted action codes or schema fields are required.

### Watching, and where a creature looks

`Gesture::Watch` is the tenth gesture: six frames at 3 fps, a two-second loop of a creature drawn up
tall with its head leaned two or three pixels toward what it is watching, ears pricked with a
one-frame flick, a braced stance, and its forelimbs gathered in. It is struck after the notice beat
by a planted, unstartled actor in a window scene, by a curious creature inspecting a ledge, and by
the spectators of geometry scenes — window, cursor, display, and ledge — through a new `geometry`
flag on `Cue`. It is never used for a dance or a copy chain, where the point is the movement itself.

`window_gaze` was also wrong, and in a way that only showed on the desktop. It aimed at the point of
the window nearest the creature, which for a creature standing beside a window is the ground under
its own feet. It now aims at the near edge at the window's mid-height, and a creature standing on
the window looks at the window's centre and turns toward it.

`Pose::lean` had been authored into five gestures and never drawn, because the legacy renderer
ignored it. All three legacy bodies — blob, hopper, and quadruped — now draw it, alongside the
modular renderer that already clamped it, and `InspectScreen` gained a slight lean of its own.

The attention review tests draw `window-watching.png`: notice, watch, and settle beside a synthetic
window, on nine reference bodies — six modular body-and-ear combinations and the three legacy
families. The gesture sheet was regenerated at 1008×4752 with a Watch-loop band including the mini
size.

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
