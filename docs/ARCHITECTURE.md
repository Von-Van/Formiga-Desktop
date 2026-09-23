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

## Start here

This section is a map for a first visit to the code. Everything after it is the detailed reference,
one feature at a time, and assumes you know where things are.

### The crates and where to start reading

Dependencies run one way: `formiga-art` depends on `formiga-core`, and `formiga-desktop` and
`formiga-tools` depend on both. Nothing depends on the desktop crate.

| Crate | Start with | Then |
|---|---|---|
| `formiga-core` | `world.rs`: `World`, `new`, `from_save`, `tick` | `model.rs` for the saved types, `DesktopSnapshot`, `WorldCommand`, and `WorldEvent`; `persistence.rs` for the save file and migrations; `behavior.rs` for how an action is chosen; `world/<theme>.rs` for each feature, and `world/attention.rs` with `world/attention/` for scenes, games, and watching |
| `formiga-art` | `renderer.rs`: `CreatureRenderer`, `AnimationSpec`, `BodyPresentation` | `renderer/pose.rs` for how a body moves on each frame; `renderer/modular.rs` and `renderer/classic.rs` for the body plans; `renderer/face.rs`, `props.rs`, and `effects.rs`; `shelter.rs` and `shelter/houses.rs` for the village; `card.rs`, `sticker.rs`, and `postcard.rs` for exports; `ui_atlas.rs` for bubbles and menus |
| `formiga-desktop` | `main.rs`, then `app.rs`: `FormigaApp` | `app/cadence.rs` for how often the colony is ticked and drawn, and `app/menus.rs`, `settings_window.rs`, `habitat_editor.rs`, and `updates.rs` for what the app does in response; `gpu.rs` and `gpu/` for the overlays; `interaction.rs` for hit-test proxies; `creature_menu.rs`; `settings.rs` and `clubhouse.rs` for the settings window; `tray.rs`; `updater.rs`; `platform/` for the macOS and Windows adapters |
| `formiga-tools` | `main.rs`: one function per subcommand | `tick_bench.rs` for the simulation benchmark |

### How the app starts

1. `main.rs` opens the log (`logs/formiga.log` in the data directory, rotated at 1 MB), builds a
   winit event loop that carries `UserEvent`s — tray-menu clicks and updater results — and hands it
   a `FormigaApp`.
2. `FormigaApp::new` resolves the data directory, loads the update preferences, and clears
   installers that have already been used.
3. When winit calls `resumed`, `initialize` opens one transparent, click-through, always-on-top
   overlay per display and takes a first `DesktopSnapshot`. It then loads the colony:
   `World::from_save` for an existing save, or `World::new` with a fresh random seed on first
   launch. If the save cannot be read, the unreadable files are kept, a fresh colony stands in, and
   Settings offers recovery choices. Last, it creates the tray icon and writes the save. On a first
   launch it opens Settings, and if a check is due it starts the daily update check on a worker
   thread.

### The loop

Nothing in Formiga runs on a timer of its own. Each time winit is about to wait, `about_to_wait`:

1. rescans the displays if two seconds have passed, adding or dropping overlays;
2. runs `FormigaApp::tick` if a tick is due. That takes a snapshot through the platform adapter
   (cursor, idle time, window rectangles), passes an ongoing drag to the world as `WorldCommand`s,
   and calls `World::tick(now, dt, &snapshot)`. It then drains the `WorldEvent`s, which decide how
   soon the save must be written, and writes it once that is due;
3. moves each creature's hit-test proxy window onto its current sprite;
4. sets `ControlFlow::WaitUntil` for when the next tick is due. That is every 50 ms while
   anything moves or is being handled, and 100 ms while the only movement is a stroll at home or a
   still creature has to stay quick to pick up. It is 200 ms for a colony that is otherwise still,
   and 250 ms while the colony is hidden, paused, or covered by a full-screen app. A still colony
   is redrawn only as often as the animations on show need.

Drawing happens in `window_event` on `RedrawRequested`. Each display's `OverlayRenderer` reads the
creatures, bubbles, and village from `World` and draws them from atlases that were baked once, when
each creature loaded. The settings window, the creature menu, and the tray act on the colony by
calling `World` methods: `handle_command` for handling a creature, and `edit` for a colony change
that the settings window can undo. The world never calls back into the desktop crate. Everything it
has to say goes out as `WorldEvent`s or as state the renderer reads.

### Where state lives

| State | Kept in | Lifetime |
|---|---|---|
| The colony: creatures, genomes, memories, bonds, village, journal, settings | `SaveFile` (`formiga-core` `model.rs`), held as `World::save` | Written to `colony.json`, with `colony.json.bak` beside it |
| Plans in flight: journeys, attention scenes, games, visits, bubbles | Other fields of `World` | Runtime only. A test keeps runtime-only fields out of the save |
| Displays, overlay windows, GPU atlases, proxies, open menus | `FormigaApp` and each `OverlayRenderer` | The life of the process |
| Update preferences and the last check | `updates.json` | Written when changed |
| Downloaded installers | `updates/` | Until the version they install is running |
| Logs | `logs/formiga.log` and `formiga.previous.log` | Rotated at 1 MB |

All of it lives in one data directory. That is `FORMIGA_DATA_DIR` when it is set, and otherwise
`~/Library/Application Support/com.Formiga.Formiga` on macOS and `%APPDATA%\Formiga\Formiga\data`
on Windows.

### Adding a feature

Most features touch the layers in the same order.

1. **Behaviour, in `formiga-core`.** Find the `world/<theme>.rs` module the feature belongs to, or
   add one: a child module adds methods to `World` rather than owning state. Runtime state goes on
   `World`. Anything that must survive a relaunch goes in the save, and then needs a
   `SAVE_VERSION` bump, a migration in `persistence.rs`, and a migration test. A new thing a
   creature can do is usually an `ActionKind`, and one the desktop can trigger is a `WorldCommand`.
2. **Art, in `formiga-art`.** A new pose or clip is authored against the rig in `renderer.rs` and
   baked into the atlas with everything else, so drawing it costs nothing extra at runtime. The
   atlas budget test fails if the new frames no longer fit the texture budget.
3. **Presentation, in `formiga-desktop`.** Often nothing is needed: the overlay draws whichever
   clip `BodyPresentation::for_creature` chooses. A new control in Settings or the creature menu
   calls a `World` method; it does not change world state directly.
4. **Tests.** Behaviour scenarios go in `world/tests/<theme>.rs`, driving `World::tick` over
   synthetic desktops with the fixtures in `world/tests/mod.rs`.
   [CONTRIBUTING.md](../CONTRIBUTING.md) lists the ways a scenario can quietly test the wrong thing.
5. **Look at it.** Only the review sheets (`formiga-tools gesture-sheet`, `habit-sheet`, and the
   rest) show whether a pose reads. [TEST_MATRIX.md](TEST_MATRIX.md) explains how to generate and
   read them.
6. **Write it down.** Add or update the section of this document the feature belongs to, and add a
   CHANGELOG entry.

### Design constraints

These constraints are deliberate. Each one closes off an easier design, and the reasons are what
keep them in place.

- **The core has no GUI or GPU code, and neither it nor the art crate can read the desktop.** All
  the simulation sees is a `DesktopSnapshot`, so the same seed and the same inputs give the same
  colony on any machine. That is what lets CI run hundreds of behaviour scenarios on synthetic
  desktops, and lets a differential harness compare whole sessions byte for byte. The cost is that
  anything the platform knows has to be carried into the snapshot explicitly.
- **No global input hooks and no special permissions.** Direct manipulation comes from a passive,
  click-through overlay plus a tiny non-activating proxy window over each creature, shaped by the
  sprite's alpha, so only opaque creature pixels can take a click. The proxies must follow their
  sprites every tick, and anything that needs system-wide input (keyboard shortcuts, clicks
  elsewhere on the desktop) is out of reach by design.
- **Geometry, never content.** Windows are rectangles and nothing more. Hiding creatures behind
  chosen apps is done by computing visible regions and discarding covered pixels in the shader,
  never by capturing the screen or reading titles. Formiga therefore cannot see what is drawn
  inside a window. [PRIVACY.md](PRIVACY.md) is the full boundary.
- **Everything bounded.** The save has fixed caps: six creatures, eight colony objects, 32 habitat
  zones, bounded journals and routines. Tests hold per-creature growth under 2 KiB, so a colony
  that runs for years does not grow without limit. New features have to fit inside a cap or add
  one.
- **Generation is append-only.** A creature is recomputed from its seed and recipe every time it
  loads, so changing what an existing seed produces would change creatures people already have.
  New options are added so that existing seeds, recipes, and seed codes resolve exactly as before,
  and codes that carry new parts are refused by older builds rather than misread.
- **One event loop, no async runtime.** Rendering, simulation, and input share winit's loop. Work
  that could block, such as the update check and downloads, runs on short-lived threads that
  report back through `UserEvent`. There is no daemon and no polling loop to keep awake.
- **At most twenty ticks a second, and frames only when needed.** Animation is authored at low
  frame rates and baked into atlases, so the cost of a still or slow colony is a few quads a few
  times a second. The price is that motion has to read at those rates.
- **Updates are never silent.** The updater verifies SHA-256 and hands the installer to the OS;
  Formiga never replaces itself.

## How the simulation is laid out

`world.rs` holds `World` itself: its fields, `new`, `from_save`, `tick`, and the small helpers that
belong to none of the themes. Everything else lives in a child module named after what it is about —
`world/arrivals.rs`, `beats.rs`, `bonds.rs`, `bubbles.rs`, `colony.rs`, `discovery.rs`,
`experience.rs`, `generation.rs`, `habits.rs`, `home.rs`, `interaction.rs`, `journeys.rs`,
`moments.rs`, `movement.rs`, `objects.rs`, `offers.rs`, `rides.rs`, `rituals.rs`, `routine.rs`,
`spacing.rs`, `surfaces.rs`, `tows.rs`, `undo.rs`, `village_life.rs`, `visitors.rs`, and the
`attention/` family. Each module adds
methods to the one `World` type rather than owning state of its own, so there is still a single
simulation object and a single tick.

Tests live in `world/tests/`, one file per theme — `ambient`, `arrivals`, `bonds`, `bubbles`,
`colony_management`, `companion`, `discovery`, `experience`, `habits`, `hangouts`, `home`,
`interaction`, `journeys`, `misc`, `moments`, `objects_and_decorations`, `offers`, `perches`,
`rituals`, `spacing`, `topology_and_attention`, `tows`, `undo`, `village`, `village_life`,
`visitors` — with the
shared desktop fixtures and colony builders in `world/tests/mod.rs`. The split into modules was
behaviour-preserving: a differential harness ran five seeds for 18,000 ticks each against 0.57.1
and compared the event streams and serialized saves byte for byte.

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

Resting is the clip a companion holds longest, so it is authored as a six-frame loop at three
frames a second rather than a bob: it settles square and screen-facing, shifts its weight onto one
foot, tips its head and pricks its ears toward that side, and settles back. Which side, and
whether the creature slumps into the shift or keeps its legs under it, are read from appearance
bytes it already carries, so two companions resting side by side are not in step and nothing is
stored to say so. The first frame is the plain settle, and it is the only frame reduced motion
draws.

Every standing clip puts the feet on one row, and nothing a body folds against itself or drops
goes below the lowest row its feet reach. A folded wing, a head over a body squashed flat by a
crouch, and crumbs all stop there; that row is one below the floor for the classic feet and two for
the rounder modular ones, and `PropHold::ground` carries it to the props. A cup is the exception. It
is held below the mouth, so a body that sits low holds it below its feet, because lifting it clear
of the ground would put it across the face.

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
The face texture contains twelve expressions — the twelfth the yawn's, eyes screwed shut and the
mouth wide — nine gaze directions, three eyelid states, and one eight-slot trinket row. That row is
still baked, at the same size and in the same place, but nothing samples it any more: the overlay's
discovery quad and the settings pages both read the colony trinket atlas instead. The body atlas
holds exactly 134 unique frames: 92 for actions, because `Tossed` reuses the dragged body clip, and
42 for twelve gesture poses, laid out as ten columns by fourteen rows. The yawn's four frames opened
the fourteenth row, which has six slots spare. Whatever a companion wears is drawn onto every body
frame as the atlas is baked, so it costs no quad and no texture of its own. Runtime work normally
selects two slots and draws two nearest-filtered quads; discovery alone adds one temporary quad, and
something the village has put in a companion's hands adds one or two from the object sheet. The
combined textures are exactly 1,649,664 bytes per creature — 9,897,984 for a full colony of six,
held under 10 MiB — and are enforced below a 4,500,000-byte test limit, raised deliberately from 1.5 MB so the
pose vocabulary has room to grow without the budget moving each time.

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

The colony seed also resolves a bottom-corner preference and a compact shelter genome. Tents,
mushrooms, pillow houses, and leaf houses are rasterized once to a static 64×64 texture. A
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

The save owns at most fifteen canonical unordered `CreatureRelationship` records, one for each
possible pair in a six-creature colony. Each record has two stable IDs and four `u8` scores: affinity,
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

The Colony page reads the same records back as "How everyone gets along": every pair once, under
the first of four headings it meets — keeping their distance (avoidance at least 160), close
friends (affinity at least 112), playmates (playfulness at least 72), and still getting to know each
other — ordered by closeness within a heading and described with the profile's own bond and play
labels. A pair with no record yet has not spent time together. Nothing is inferred beyond the four
scores, so the card never suggests an event the colony did not record.

At action boundaries, utility selection receives the preferred pair as a `BondContext`. A
runtime-only `BondPlan` can approach through `Follow` and then execute an existing targeted action.
Target points refresh from the current creature snapshot each tick; plans cancel to idle if the
target disappears, moves to an incompatible surface or display, sleeps, becomes homebound, is
tossed, or otherwise cannot participate. Follow, sleep, presentation, social play, greeting,
inspection, and window reaction reuse their existing body clips, leaving the atlas unchanged.

Each companion also carries a `RoamingLeaning` its owner chose — wherever it likes, homebody,
floor-dweller, or climber — kept apart from its innate personality and its learned tendencies and
never carried in a share code. It adds a bias to the same utility sum, on the scale learned
tendencies use: a climber's perching and riding rise, a floor-dweller's and a homebody's fall, and
a homebody rests more and sprints less. At an action boundary on a ledge, a floor-dweller comes down
first seven times in ten and a homebody four in ten, through the same hop to the floor below that
the colony takes to walk home, and a homebody that sets out walks toward the colony house on its
display seven times in ten. Nothing is forbidden: the habitat, hidden and paused states, and every
safety check still decide what a companion can do.

## Little habits

`habits.rs` gives each companion its own ways of doing ordinary things. Its `Celebration` — a hop,
a little dance, or a twirl — is read from its personality and a value mixed from its behaviour
seed, so it is never stored and is the same wherever the creature lives. Whenever a scene strikes
the cheer pose, the art draws that creature's celebration in its place: the dance is the existing
bop, and the twirl is the cheer turning round every quarter of a second.

Learned habits live in `CreatureMemory.habits`: at most two, one for each `HabitCue` — a meal, a
nap, a hello, a game. When an action with a cue starts on the main path, at a ritual's ceremony, or
on an accepted offer, `cue_habit` may pick one up. The chance is scaled by how often that kind of
moment comes round, so each is about as likely as another to become a habit, and by how well the
candidate suits the creature's temperament and learned tendencies; a nap goes to whichever of
stretching and circling suits it better. A second habit comes at three tenths of the first's rate.
A moment at the door while the houses are out shows habits but teaches none, and a visiting guest
shows the ones it came with. Picking one up bumps the profile revision and emits `HabitLearned`,
which the journal records and which is saved at once. The dice come from a runtime `habits` stream
of their own, so no other choice the colony makes shifts.

A companion with a habit does it four times in five as its moment comes round: a runtime-only
`Flourish` on `CreatureState` names the habit and the action it opens. A meal or a game alone
starts it at once; a nap, a greeting, or a visit to the pointer waits until the walk to the pillow,
the friend, or the pointer is over, and lets it go after eight seconds if the walk never ends.
While it shows, `execute_action` holds the creature still; the action's length, drives, bonds and
tendencies are untouched, and anything replacing the action ends it. Reduced motion never starts
one.

`BodyPresentation` in the art crate is the one place a frame is chosen: an attention gesture first,
with the cheer becoming the creature's celebration, then a flourish, then the action. The overlay,
the hit mask, the redraw cadence, and the review sheets all draw from it, and the face resolver
reads the same flourish: curious and looking down at a snack, eyes screwed shut at the top of a
stretch, and awake while turning round before a nap. Only the stretch is new art — four frames that
play once and hold, baked into slots the atlas already had spare, on all fours for long bodies and
with wings spread for winged ones. Every other habit reuses baked frames: the meal's opening frame
held, the walk stepped on the spot with the facing turned, the reach, and the crouch.

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

The ground a policy is measured against is `colony_bounds`: the display's work area, which
`platform::work_area_insets` reads as insets from each edge — the difference between a screen's
frame and `NSScreen.visibleFrame` on macOS, or between `rcMonitor` and `rcWork` on Windows — so the
ground moves with a Dock on the side, a taller menu bar, a taskbar along the top, a resized bar,
and a bar that hides, on every display separately. The display scan every two seconds picks up a
bar that moves, and display attention re-seats anyone whose ground moved under them. Where the
work area cannot be read, a 24-point menu bar and `platform::BOTTOM_RESERVED` — the factory Dock
on macOS, the taskbar on Windows — stand in. The colony is founded on the floor of that ground, so
the houses stand on top of a Dock that stays out rather than behind it.

While the desktop habitat editor is open, every overlay is hit-testable across its whole display
so a press anywhere draws a region. `habitat_editor_claims` limits what the editor takes to
`CursorMoved` and `MouseInput`: everything else, `RedrawRequested` above all, still belongs to the
overlay it was addressed to, so the editor draws what is being dragged instead of freezing the
colony on its last frame. Because those overlays cover the display and sit at the same window
level as the settings window, `platform::raise_above_overlays` holds the settings window above
them for the length of the edit — macOS by window level, Windows by re-asserting topmost after
each gesture — so Apply, Cancel and Reset never end up behind a window that swallows every click.
Interaction proxies are ordered out for the duration through `InteractionProxy::hide`, which keeps
the proxy's own visibility bookkeeping honest; `sync` only touches a window on a change, so a
proxy hidden behind its own record of itself would never be ordered back in.

Selected applications are represented by bundle ID, AUMID, or a SHA-256 executable-path digest. The
renderer subtracts higher windows from each selected window's visible rectangles and sends up to 64
monitor-local rectangles to a fragment-shader uniform. Covered pixels are discarded; a dragged
creature opts out per vertex.

The overlay treats repeated surface timeouts or compositor-occluded acquisition as recoverable. It
reconfigures the existing surface after three consecutive stalls, invalidates cached presentation
state, and requests another frame. Its creature vertex buffer also grows to the next bounded power
of two if a valid colony frame exceeds the initial colony allocation. Simulation positions
are reconciled to current native monitor IDs before rendering, so display sleep or hot-plug changes
cannot strand a living creature outside every overlay.

## The creature menu

A secondary click on a creature's existing interaction proxy opens one small strip above its head.
On macOS a primary click with Control held opens the same strip; the Control state is read from the
combined-session `CGEventSource` that `cursor_and_idle` already samples, because a proxy never takes
keyboard focus and so has no modifier state of its own. No new permission, event tap, or global hook
is involved. Only one menu exists at a time.

A colony member's strip holds Snack, Toy, Home, and Profile, with Moment in Home's cell while the
houses are out and the village has something it could share or is sharing one. A guest's holds
Snack, Toy, then Stay when `visitor_can_stay()` is true and Copy code otherwise, then Profile — the
two share one cell, so the other three never move under the cursor. Snack and Toy issue `WorldCommand::OfferSnack` and
`OfferToy`; Home issues `WorldCommand::SendHome`; a member's Profile opens the settings window on the
Colony page with that creature selected, and a guest's opens it on the Journal page, where the guest
book is. Stay calls `ask_visitor_to_stay`, logging a category and closing the menu if it fails. Copy
code routes through the application's only clipboard path — egui's, inside the settings window — so
the window appears on the Journal page with a "Visitor code copied" toast rather than a second
clipboard owner existing. The menu closes after any choice but Moment; the simulation answers with
bubbles.

Moment opens a second strip beside the first, on its row and level with its body: to the right
where the usable area has room and to the left where it does not, clamped inside it, and never
moving the menu. It holds Stop while a moment is under way, then whatever
`World::available_village_moments` offers — Picnic, Dance, Nap — so it is two or three cells. Its
tray is a frame sprite without the notch, since it points at nothing, and a hovered item's label
hangs level with the menu's own tabs. The menu's hover, stray, and untouched rules count the strip
as part of the menu, and the click proxy grows to the rectangle around both bodies. Choosing Moment
again puts the strip away; a moment issues `WorldCommand::InviteVillageMoment` and Stop issues
`StopVillageMoment`, and both close the menu.

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

### Village moments

A scheduled ritual never starts while the houses are out. `world/moments.rs` is the village's own
path, run inside the homebound tick: the owner asks for a picnic, a dance, or a nap from a
companion's menu, and `invite_village_moment` checks again that the houses are out and in sight,
nothing is paused or hidden, nobody is being carried, no ritual is under way, and at least two
companions are home on their own feet — reduced motion never offers a dance. Everyone home answers
for themselves from a runtime `village-moments` stream: a companion already asleep stays asleep,
tiredness and temperament make the rest more or less willing, and the one that was asked is twice
as willing. Each answers with a bubble; fewer than two willing means no moment, and the one who
would have come shows a question mark.

Those who join get places in one line on the commons, shoulder to shoulder at the village's own
face-clear spacing, centred on the one that was asked and kept inside the walkable ground; a line
longer than the ground keeps the companions nearest the host. Their quiet moments at the door and
their roaming are set aside, and the homebound tick walks each to its place, where it faces the
middle of the line. Once everyone has gathered, or after sixteen seconds, they do the moment
together — eating and drinking by turns, social play under a runtime `Bop` pose that turns into
each dancer's own celebration for the last 1.6 seconds, or sleep — for 14, 12, or 30 seconds, with
habits cued as each action starts. Under reduced motion the places are where everyone already
stands.

A moment that runs its course emits `RitualCompleted` for `Picnic`, `Dance`, or `GroupNap`, which
the journal records like any shared moment, and one bond experience for every pair who shared it.
Stopping it, hiding or pausing the colony, and the houses going all end it with
`RitualInterrupted` and nothing else; picking a participant up sends the houses away, as picking
up any resident does. A pet is answered in place and the moment carries on. Something held out to a
participant while the village is still gathering takes only that one out, the rest carrying on
while two remain; once the moment is under way a participant is busy and turns it down. The houses
stay out for a moment under way. A visiting guest neither greets nor draws answers from companions who are in one.
`RitualKind::Dance` exists only for these: the scheduler never chooses it, and nothing about a
moment is stored beyond the journal line.

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
placing dwellings and a keepsake tree at each end, and every belonging is scattered
inside one of the two trees' yards. The same walk drives rendering and nearby utility using the
home's corner, display, scale, and accessible region. Lots without room remain stored but hidden;
all objects hide while the house is inactive. Legacy normalized positions are rewritten to the
village. The renderer builds one 256×112 seed-derived sheet of 16×16 cells, sixteen to a row — the
twenty belongings, the fifteen hangout spots, every garden at each of its four stages, the fifteen
ornaments, and the small things a companion holds or chases in the village: a watering can and its
water, what is picked from each garden, an apple, a leaf, a blossom, and the Z that drifts up out of
a house somebody is asleep in. It rebuilds the ground's vertices only when object state, a garden's
stage, cottages, home state, habitat, display geometry, or scale changes.

Dwellings and trees come from one 512×256 village atlas of 64-pixel cells, eight across: the six
houses by day and the keepsake tree in the top row, the same six with their residents at home in
the second, and the same again lit from inside after dark in the two rows below. Every house has a
cell of its own because each is hung with its own resident's curtain and wears its own
decorations, so a village needs one cell per house rather than one per kind. A house somebody is
inside has the curtain drawn right across its door, hanging in folds with lamplight under the hem,
and a window glowing by day; its silhouette is the same pixel for pixel, so nothing about the house
moves when somebody goes in. Each cell is drawn into its own cell-sized tile so art that would
overrun a cell is clipped exactly as it is for a lone shelter. The first colony member shares the
colony house and each later one adds a single quad sampling its slot's cell — by day or after dark,
with somebody at home or not — and the two trees add one quad each, so a full village is eight
quads against one texture and bind group. A house being seen to by its keeper leans or settles
while the chore lasts: the top of its quad moves a pixel or so, its foot never. The texture is keyed
on `VillageLook` — the drawn genome, each house's kind, its decorations, and its curtain — so any of
those changing redraws it once. The Home page only ever draws the houses by day with nobody home,
so it holds just the top row, 512×64. Decorations resolve their attachment points from the style's
own silhouette — peak, eaves, walls, and ground line — so a banner hangs from the real roof rather
than a shared canvas height.

Each style is drawn in the creatures' own pixel-art language, from `shelter/houses.rs`: separate
materials for the roof or canopy, the walls or supports, and the trim, each with a base, a shade
and a light, lit from the upper left; a recessed doorway in the same fixed near-black, with a
threshold; and one or two signs that someone lives there. Each kind is built from a shape of its
own. The tent is triangles: one canvas cut into four triangular panels meeting at the peak, lit on
the left and shaded on the right, a triangle doorway with its flap tied back over a straw floor, a
triangle pennant on the pole, and guy ropes out to pegs. The mushroom is circles: a domed cap with
its gills in shadow and round spots, on a cream stem with a round window and a stepping stone. The
pillow house is soft squares: one plump cushion standing on the ground for the walls, its sides
bowed out, piped along the top and buttoned with the fabric drawn in round each button, with a
flatter pillow lying across it for a roof, tasselled at the corners, a round window, and a pillow
glimpsed inside. The leaf house is leaves: a lit card front wall and a side wall folded back into
shade under a steep roof thatched in rows of overlapping leaves, each hanging point-down and veined,
no two neighbours quite the same tone, their tips scalloping the eave, with a sprout on the ridge,
a vine up the fold, a cut-out window and a striped mat. The colony's shelter palette dyes the
canvas, cap, pillows or leaves; bark, straw, cream stems, stone and card are fixed materials. The
four keep the names colony files have always stored them by — `LeafTent`, `MushroomHut`,
`CushionDen`, `PaperHouse` — so an existing colony's houses open as the same kind, drawn anew.

A village mixes them. `ColonyHome::house_style_list` gives every house slot its kind: one chosen
on the Home page if there is one, kept in `house_styles` by the companion who keeps the house,
else the colony's own kind for the colony house and `ShelterStyle::for_keeper` — the kind the
keeper's own seed would build — for a cottage. The village atlas bakes each slot as its own kind,
and its texture on the overlay and the Home page is keyed on the list alongside the curtains, so a
new choice redraws it once. A replaced companion's choice passes to its replacement, one that
leaves takes its choice with it, putting the village back clears them, and the last change can be
undone. `ResidentMark` reads each full-size companion's own colours for the curtain in its
doorway, tied back to a side its seed chooses, so a house keeps its resident's look wherever the
village moves it, and a mini shares its big version's. Customizing the village changes the house
itself; the curtain always stays the resident's. After dark — seven in the evening until seven in
the morning, read from the local clock at most once a minute — the overlay draws the lit cells:
lamplight filling each doorway inside a dark rim, glowing windows, and the colony house's lamp
lit if it has earned one. It is a still picture, so reduced motion changes nothing.
Nearby semantic roles add a bounded `+0.25` to existing action utility at ordinary selection
boundaries; objects have no physics body, interaction proxy, action state, or update loop.

## Growing shelter

Save version 19 keeps what the village has to choose from in `VillageUnlocks`: the decorations,
hangout spots, gardens and ornaments it has so far, when the next one arrives, and an ordinal. A new
colony starts with three of each. Every 24 to 48 hours — `scheduled_village_unlock_at`, from a named
seed stream — one more arrives, chosen by `preferred_village_unlock`. Categories take turns, the
one with the most still to come likeliest; within them, what the colony has been up to decides:
its companions' memory counters, its bonds, its last ritual, its belongings and its gardens score
eight themes — sky, nature, company, rest, curiosity, home, garden, play — and every item not yet
had scores its theme, with a little jitter from a seed stream of its own. A colony that has been
away gains one item when it opens and schedules the next from then, rather than catching up on
every one it missed. Each arrival is a `VillageUnlocked` event and a journal line, and a new
decoration goes straight up on the colony house if its place there is empty, as earned ones always
did.

Decorations belong to houses rather than to the colony. There are thirty, each for one of six places
on a house — `DecorationSlot::{Roof, Eaves, WallLeft, WallRight, GroundLeft, GroundRight}` — and
`HouseDressing` keeps, for the companion who keeps each house, at most one decoration per place.
`set_decoration` refuses one the village has not got or one that does not belong in that place, and
`house_decoration_list` lays them out by house slot for the village atlas. A colony from before
version 19 had one list of earned decorations for the colony house and a bitmask of hidden ones:
the migration makes every earned decoration unlocked and hangs the ones that were showing on the
colony house, in their places.

`ShelterRenderer` draws each house with its own decorations from `shelter/decorations.rs`, placed
from the house's own silhouette and pulled in to stay inside the house's lot. The GPU shelter cache
key is the whole `VillageLook`, so a change to any house's decorations replaces the single village
texture; presentation is still one quad per house, one bind group, and one draw call. Decorations
have no world position, action, physics, or render loop of their own.

## Village life

While the houses are out, a resident with time on its hands — the moment a quiet moment at its door
would otherwise come round — chooses between one of those moments and a plan about the village, in
`world/village_life.rs`. A plan is a few steps — walk somewhere, do the thing, perhaps find
something, go back to strolling — and owns the resident's feet while it lasts, the way a quiet
moment does:

- **The garden.** `GardenPatch::stage(now)` goes round sprout, growing, grown and bounty on each
  kind's own clock, from when it was planted, and back again; a patch planted before gardens grew
  is taken to have been planted long ago, somewhere round its cycle. A visit waters any patch, looks
  in on one still sprouting or growing, picks from an edible one that is grown or at its fullest and
  eats what it picked, or carries something from a patch at its fullest over to a resting friend,
  who looks on pleased.
- **A chore at its own house**, in the way the house's kind asks: retying a tent's flap, plumping a
  pillow fort, patting a mushroom's cap, tidying a leaf house's leaves. `World::house_motions`
  reports it with how far through it is, and the overlay leans or settles the house to answer.
- **Indoors** for a spell, pottering or napping: the resident is marked `indoors`, drawn nowhere and
  out of reach of the pointer, and `World::house_occupancy` names the house so the overlay draws its
  occupied cell and, for a nap, the Zs. At most `max_indoors(residents)` are in at once — none while
  there is only one resident, one for two or three, two beyond that.
- **Up on its own roof**, in a little hop to the height `house_roof_height` gives: measured from the
  house's proportions the way it is drawn, since the simulation cannot draw a house, and held to the
  drawing within a pixel for every kind at every height by a test in the art crate.
- **A mishap**: a leaf on the face, a snack that rolls away and is chased, or sitting down beside
  the nap cushion and shuffling onto it. `World::loose_props` reports the leaf or the apple and
  where it is.

Only one plan out and about runs at a time, beside the one quiet moment the village already
allowed, and a spell indoors counts against its own limit instead. Watering or looking in on a
garden, a chore, or a spell on a roof turns something up one time in seven, with the at-home
circumstances and the calendar's; the find completes as `PresentDiscovery` does anywhere, which is
what writes it into the scrapbook. A friend nearby stops to look at a mishap or a find.

A plan notes the ground it was made on — the commons, where the resident's own house stands and
which house it is, and where the patch it set off for is — and is let go the moment any of that
changes, so a cottage carried along the row or a corner changed never leaves somebody inside the
wrong house or sitting on a roof that has moved away. Every plan is runtime only: ending a visit,
pausing, hiding, reduced motion, an offer, a shared village moment, or picking the resident up ends
it, and puts the resident back on the ground outside — or, picked up off a roof, leaves it where it
was taken from, to hop down once it is let go.

## Beats, and a yawn going round

A `Beat` on a creature's state is a few seconds of something small: a yawn, holding one back, a look
over at somebody, a start at a leaf, watering, a chore, sitting up on a roof. `world/beats.rs` moves
each one on, lets it go when it is over or when something bigger takes over — being picked up, a
scene, a reaction — and holds the creature still while it lasts, on the desktop and at home alike;
the action it was doing waits rather than running out underneath it. The art reads the beat's
progress to choose a pose, a face, eyelids and a gaze, and anything held, and shows the action's own
clip for any stretch of it that asks for no pose.

Every two and a half to six minutes somebody free for it yawns, weighted to the sleepiest. The
nearest free friend on the same ground within three and a half creature widths catches it four
times in five: a look over after 0.4–0.9 seconds, lasting 0.7–1.4, and then its own yawn. Two times
in five the nearest free friend of that friend catches it too, and holds out for 1.3–2.1 seconds
before giving in. Each link waits in a queue under a chain number; a friend no longer free when its
turn comes takes the rest of its chain with it, and the next link of a chain follows straight on
from the last without the two having to line up to the tick. A beat stream of its own makes every
choice, so yawning never shifts any other choice the colony makes, and nothing about it is saved.

## Something to wear

`formiga-core::accessories` holds the twenty `AccessoryKind`s, each made from one catalogue variant
and worn on the head, round the neck, across the body, or pinned on the chest, and
`Accessory::{Worn, Pin}` — a find itself worn as a pin. What a colony can wear is read from its
scrapbook and nothing else: `available_accessories` is every kind whose find has turned up, then
every find as a pin. `World::set_accessory` puts one on, swaps it, or with `None` takes it off,
refusing anything not found; `from_save` takes off anything a file names that the colony has not
found. A share code never carries one.

`renderer/accessories.rs` draws the piece onto every body frame as the atlas is baked, placed from
the `Figure` that frame's body reports — the top of the head, the neck, the chest, the hip — so it
follows every pose at a mini's size as well as an adult's, in the inks the colony's trinket sheet
gives the find it is made from. The overlay rebakes a companion's atlas only when what it wears
changes. `cargo run -p formiga-tools -- accessory-sheet` draws every piece on every body plan.

The wardrobe on the Your colony page, in `clubhouse/collection.rs`, shows the companion wearing
whatever the pointer is over, a frame later. Nothing in it may change size with the pointer, or
the choice pointed at moves out from under it and the menu shakes: the try-on line is always one
truncated line, and `wear_choice` keeps a chip the same size whether or not it is pointed at.
`pointing_at_something_to_wear_leaves_every_choice_where_it_was` holds both.

## Arranging the village

The Home page's preview is drawn from the same layout functions the overlay uses, and in Arrange
mode each house and each thing on the ground is something to take hold of. A cottage carried along
the row drops into the gap it was carried to — the cottages ahead of where it was let go, in the
direction the village runs, is its new place — and the colony house, always first, cannot be
carried. Something on the ground carried along it is let go as a fraction of the ground a companion
may stand on, and the layout keeps its usual room between things. The arrow keys nudge whatever is
picked out: a thing on the ground by four hundredths of the ground, a cottage one place along the
row. A house picked out shows its keeper, anyone else who lives there, its kind and its six places
to decorate. Nothing is written until something is let go, and every change goes through the same
outcomes as the rest of the page, so each can be undone.

## How high companions go

`find_nearby_ledge` lets a companion on the floor think of any window at least 36 points above it
and within 420 points to either side, however tall it stands, so long as a companion sitting on top
of it would still fit beneath the top of the display's usable area — 0.85 of a creature frame of
headroom — since the overlay sits beneath the menu bar and a head up there would be cut off. From
one ledge to another the reach stays a single staircase step, within 360 points across and 640 up
or down, and planned routes keep their own limits.

Everybody comes down now and then. Each time a companion up on a ledge chooses something other
than perching or riding, it takes it to the floor with a chance that follows its leaning — 0.7 for
a floor-dweller, 0.4 for a homebody, 0.35 for one that likes to be anywhere, 0.05 for a climber —
and then does not think of climbing for a while: 35–90 seconds for one that likes to be anywhere,
15–40 for a climber, 60–150 for a homebody, 90–200 for a floor-dweller. On the measurement harness
in `world/tests/misc.rs` (`measure_time_on_ledges`, run with `--ignored`), four companions that
like to be anywhere spend 40–49% of their time up high on desktops with anything to climb.

## The village yard

A dwelling's ground footprint, in shelter pixels, is 60 for the colony house and 46 for a
companion's. There is no mini's cottage: a house belongs to a full-size companion, and a mini
lives in its big version's — `house_slot_for` answers which house any companion comes home to,
and a mini keeps that answer if it ever grows full-size. `house_owners` lists the keepers in the
order their houses stand: the founder in the colony house, then the cottages in the order the
owner arranged them, then anyone never arranged in the order they arrived.

One walk lays out the whole strip: `Tree(Outward)`, `Dwelling(0)` through `Dwelling(n)` a
`VILLAGE_GAP` apart, and `Tree(Inward)`. Neither belongings nor standing places are lots any more
— belongings live in the two trees' yards, and the ground in front of the houses is shared — so
the walk is at most `MAX_VILLAGE_LOTS` = 8 entries and is built in a fixed-capacity stack array
rather than a `Vec`, because the simulation asks for it several times a tick. Lots that cannot fit
the house's accessible region stay stored but hidden rather than spilling elsewhere, and because
the trees are the outermost lots at both ends, a corner that runs out of ground gives up a tree
before a house.

`HomeCommons` is that shared ground: the run from the outward tree's outer edge to the inward
one's, with the part in front of the houses — `stand_low_x` to `stand_high_x` — marked off as
where a companion standing still belongs, since the ends are the yards and the yards are full of
the colony's own belongings. `home_resting_position(slot, of, …)` spreads `of` companions along
the frontage at `REST_CLEAR_RATIO` of a frame apart, falls back to the whole walk when the
frontage cannot seat them face-clear, and packs them evenly rather than stacking them when even
that runs out. It is a resting place on shared ground, not an address: with six houses there is no
arrangement that leaves every doorway clear, and a colony living in front of its houses is what a
village looks like.

`VILLAGE_GAP` is 3 shelter pixels — a visible seam at every scale the overlay draws at, and no
more. `RESTING_WIDTH` is `CREATURE_FRAME_WIDTH − 2 × REST_WALL_SLIVER` = 30: the ground a settled
companion claims, less than the frame it draws, because the outermost pixels either side are a
wall's edge or air a neighbour may reach over. `REST_WALL_SLIVER` is 9, `OBJECT_WIDTH` is 10 (the
drawn width of a belonging, not the quad it is cut from), and `REST_CLEAR_RATIO` is unchanged and
still const-asserted equal to `world::spacing::FACE_CLEAR_RATIO`.

The widest village — six houses, eight belongings, both trees — measures 423 shelter pixels end to
end, against a `VILLAGE_SPAN_LIMIT` of 448 inherited from the four-companion village that used to
take 445. Six houses now fit in less ground than four did with a doorstep each, and a village lays
out only the houses it has: a founder on its own is 178, and three houses are 276. The 305 between
the trees is dwelling footprint and seam, and nothing else — everything the strip used to spend on
standing room and belongings it no longer spends at all.

A keepsake tree stands at each end of the walk, claiming `TREE_WIDTH` = 56 shelter pixels of lot.
The art reaches ±27 from the trunk across all nine lean-and-tilt combinations, so the lot is a
pixel of air past the widest thing in it. `HOME_EDGE_MARGIN` keeps its formula —
`DwellingKind::Main.width() / 2 + VILLAGE_GAP + TREE_WIDTH` — and so reserves the outward tree's
whole lot against the edge of the display. The inward tree needs no margin of its own: the strip
runs away from the edge, and a region that cannot take the inward end simply does not show it.

The two trees have sixteen hooks between them, `TREE_HOOKS`: hooks 0–7 are the outward tree's, by
the colony house, and 8–15 the inward tree's at the far end. `TreeEnd::of_hook` says which, and
`TRINKETS_PER_TREE` = 8 is also the length of `formiga_art::TRINKET_ANCHORS` — four pairs mirrored
about the middle of the cell, highest and most central filled first. `hung_keepsakes` decides what
hangs on each hook. With nothing chosen, the original sixteen finds keep the hooks numbered after
them, exactly where they always hung, and anything found since fills the hooks still empty in the
order it was found; with a `TreeKeepsakes` choice from the Collection, exactly what was chosen,
less anything the scrapbook does not hold. `formiga_art::hook_place(hook)` answers with the end and
the anchor as it is actually drawn there, and `hung_trinkets` pairs every hung keepsake with both.
Because the anchor set is mirror-symmetric, the inward tree is the same atlas cell sampled with its
horizontal UVs swapped: still one village texture and one extra bind group, and every keepsake
still meets the cord drawn down to it. The overlay draws one 16×16 quad per hung keepsake from the
colony's own trinket atlas, sixteen at most. `colony_card.rs` draws both trees the same way, the
inward one mirrored, so a portrait is not left with orphaned clusters at each end.

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

Staying on the display and out of one another's faces wins over a clear view of a house that
display cannot show properly anyway. `home_guest_position` stands a visitor past the outermost *visible* lot and past every
resting spot, plus a gap and half a frame, and returns `None` when it cannot keep face-clear
distance from the residents.

While the colony is home, residents stroll. The first place a resident goes when the houses appear
is its own doorstep — beside the door, not across it — which for a mini is its big version's, and
then its own resting place. From there `roam_target` in `world/home.rs` sends it on strolls: out to
somewhere along the commons at least a step and a half from home, a look about for
`STROLL_PAUSE`, back to its own place, and a rest of `STROLL_REST` before the next. Its place is
kept for it while it is out, so the village always has somewhere clear to come back to, and only
the far end of a stroll is chosen: `stroll_to` considers `ROAM_TRIES` places, preferring one clear
of everybody standing or headed there and not square in a doorway, and where none is clear it is
only a place to turn round, with a pause short enough never to stand on anybody's face. At most
`MAX_STROLLING` residents are out at once; the rest wait their turn. A stroll goes at
`STROLL_PACE` of the companion's walk, and its walk cycle is slowed to match so its feet keep up
with the ground; the walk home stays at full pace. Until 0.59.2 every destination had to be clear
of every other resident's position and destination, which a commons of three or more could not
offer, so 99.6% of strolls were called off and residents stood still. Strolling is a fifth of a
resident's time now. A quiet moment owns the creature's feet while it lasts, so a companion doing
its small thing is not also walking somewhere. A hidden colony and one under reduced motion do not
stroll at all. Stroll state is runtime-only and cleared each time the houses go or come, so every
visit starts with the walk home. While the only thing moving at home is a stroll, the app ticks
and draws at 10 Hz instead of 20.

A visit is a tour rather than a stand. `plan_tour` asks the same layout functions where everything
is, turns each house, resting resident and belonging into a span of ground the guest may not
stand on, merges those spans, and keeps the free gaps between them; the arrival spot is always the
first place on the ring, and at most `MAX_TOUR_STOPS` are kept. Every candidate is then re-checked
against the four rules it has to pass — inside the accessible region, clear of each house by the
same arithmetic that keeps a resident off a doorway, at least the face-clear distance from every resting
resident, and clear of every belonging the colony keeps — and dropped rather than shaved if it does
not plainly pass. A full strip has no legal ground between the houses at all — they stand a seam
apart — so the ring there is the arrival spot and the trees' yards; a smaller colony threads the
gaps as well. The guest walks the
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

The owner can put down up to three hangout spots from the Home page — a nap cushion, a picnic
blanket, and a lookout, one of each — stored in `ColonyHome::hangouts` as a kind and a fraction
along the ground, normalized to one of each kind on the ground whenever the colony opens.
`home_ground_positions` puts each spot, and each garden patch, at its fraction of the commons'
standing span and pushes them all apart to their own width and a little more, so however they were
placed they stand on the ground a companion may use and never on one another; a ground too short
keeps the ones that fit. `home_hangout_positions` is the spots among them, so a garden planted
beside a spot moves it exactly as far on the desktop as in the simulation. The overlay draws them
from the colony object sheet, which grew from eight cells to fourteen for the spots and patches,
while the houses are out, with the lookout's spyglass turned toward the middle of the display; the Home page
preview and the colony portrait place them the same way. Each free spot adds one weighted choice
to a resident's quiet moment: the cushion a nap, the blanket a snack or a drink, the lookout a look
out over the desktop from beside it — weight two, or four for a companion who feels like it, a
sleepy one, a hungry one, or a curious one — so a spot draws moments without taking them over. The
companion walks there, taking at most thirty seconds, and does it on the spot. A spot someone is
standing at is not offered, and a village without spots draws exactly the choices it always did. An
invited picnic lines up around the blanket and an invited nap around the cushion.

The owner can also arrange the village itself, and the arrangement is three more optional fields
of `ColonyHome`, each absent until chosen. `cottage_order` lists the companions whose cottages have
been moved, in the order they now stand; `house_owners` reads it, ignoring anyone who is not a
full-size member and never moving the founder out of the colony house, and `arrange_cottages`
writes nothing down when the order asked for is only the order everyone arrived in. A replaced
companion hands its place in the order to its replacement, and one that leaves takes its place with
it. `palette` names one of six `VillagePalette`s, each a hand-made pairing of two of the twelve
creature palettes chosen so both halves suit its name, because some styles and the tree wear the
main colour most and others the accent; `drawn_shelter` swaps only the genome's two palette
indices, so style, size, and details are untouched, and every renderer — the overlay, the Home
page, the colony portrait — draws from it. `gardens` holds up to three `GardenPatch`es, a flower
bed, a vegetable patch, and a herb box, placed like the spots; they are something to look at and
draw no choices. `reset_arrangement` clears all three and leaves the spots alone. Because the
village texture is keyed on the drawn genome and the curtains, a new palette or order redraws it
once and costs nothing per frame.

The layout allocates next to nothing, because the simulation walks it several times a tick.
`village_walk` is the fixed-capacity stack array above, `colony_cottage_list` and `house_owners`
pick the members in place, `home_resting_position` and `home_guest_position` allocate nothing at all,
`VillageGround::resolve` lists the habitat's accessible regions once and keeps the one the anchor
landed in rather than listing them again for every lot, and `home_object_positions` resolves the
village once for all eight belongings. `reconcile_colony_objects` went from roughly 34 allocations
a tick to 1, and `tick_homebound_creatures` from roughly 76 to 8.

`home-yard-sheet.png` shows fourteen cases at both corners: four shelter styles with a full colony
of six and the trees filling up as the scrapbook does — nothing found, five, eleven, all sixteen —
then colonies of one to six with their residents on the commons, ending on the widest village there
is, and last the four styles after dark, each door hung with its resident's curtain. Along the
way the spots and the garden patches are spread out, bunched up, and mixed together, and three
villages are painted in a named palette. `shelter-sheet.png` gives each style a lane — plain house,
decorated house, a cottage with its resident's curtain, the same cottage lit after dark, the tree,
and the resident at the same scale — and `village-palette-sheet.png` shows each style in its own
colours and then in every named palette.

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

`visitor_can_stay()` answers whether the colony has room: fewer than six members, the same bound the
adult cap sets, and not a duplicate of somebody already here. `ask_visitor_to_stay(now, desktop)` then runs
the exact adoption path an imported creature takes — a fresh history, standing where the guest stood,
and a Stay bubble. `visitor_share_code()` returns the guest's code. The settings Journal page carries
the guest book, newest first, with Copy code on every entry and the current visitor on top with Ask
to stay beside its code. Any visitor there can be kept as a favorite: `VisitorState::favorites` holds
at most `MAX_FAVORITE_VISITORS` (eight) names and origins apart from the book, so a favorite outlasts
the book moving on, is never kept twice, and a full list waits for one to be forgotten rather than
dropping one. Inviting a favorite goes through `invite_visitor` exactly as a pasted code does, so the
same checks answer and the same visitor, under the same name, comes back. The Creature studio's "Adopt a shared companion from a code" offers "Invite
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
table that is never saved: one slot for every unordered pair a full colony can make, and one
shuffle per member. Those sizes come from `MAX_COLONY_CREATURES` rather than from a number written
down beside it, because `watch` hands back whichever record it lands on — a table too small to hold
every pair credits one pair's time to another and reads another's cooldown, so nobody is asked to
step off a face that is genuinely covered. A covered face acts after 1.25 seconds and plain
crowding after 2.0. A step aside is ordered, not finished: the pair is held only long enough not to
be asked again on the next tick, and the episode's own clock keeps running, so a pair still drawn
through one another once the mover has arrived is asked afresh instead of sitting out a cooldown
granted on the assumption the move worked. Coming apart is what clears the clock, and the 1.08
clearance margin is what stops a separated pair landing back on the threshold. A companion
walking to a spot of its own, or on its way through a game, is left to carry on for a while; once a
face has been covered for two graces, 2.5 seconds, neither excuses it any more and whichever of the
two can be asked is asked. One caught up from behind by the other going the same way stops for
1.5 seconds to let it past rather than stepping on ahead of it, which only ends with it caught
again — a sprinter on its way into a game had carried a playing friend's face behind its body for
over seven seconds that way. Who moves is decided by cost: never someone dragged, tossed, airborne, climbing,
hanging, homebound, or owned by a scene; +8 for being asleep and up to 4 more the longer it has
slept, so the lighter sleeper moves; +7 for a ritual participant, +6 for a pile anchor, +5 for
holding a prop, +2 for a bond plan; ties by id. An awake creature takes an ordinary short `Traverse`
to the nearest spot that clears everybody, respecting habitat, ledge extent, and reservations;
ritual participants get a quiet position-only shuffle, and a sleeper is towed: `world/tows.rs`
finds a friend on the same surface within six widths who is awake, standing about or walking, and
wanted by nothing else — the closest friend first, then the nearest — which walks over, takes up
the rope a little under a width ahead of the sleeper, pulls it clear at `TOW_SPEED`, lets go, and
goes back to standing about. The tow walks the friend itself, so its own choices wait until it
lets go. The sleeper's runtime `SleepNudge::Towed` tells the overlay to draw a sagging rope, one
art pixel thick with a shaded underside, from the friend's hand to the sleeper, behind both, from
a two-texel texture made the first time a rope is needed. With no friend free, or a rope that would
take either off its surface, the sleeper wriggles over by itself at `WRIGGLE_SPEED`, its breaths
coming four times as fast; and a tow cut short — either of the two picked up, tossed, climbing, or
wanted by a scene, or the sleeper waking — lets go at once and the sleeper wriggles the rest of the
way. None of it emits `SleepInterrupted`, `CreatureWoke`, or `CreatureRested`, and under reduced
motion a sleeper is simply placed.

A nap starts with a walk to wherever it is taken: a pillow, a friend, or a place in a line. Until
0.59.2 the whole walk was drawn in the sleep pose, so a colony of four spent about 400 seconds an
hour gliding across the floor asleep. `CreatureState::walking_to_sleep` is true while a sleeper is
still moving under its own steam, and `BodyPresentation` walks it there instead, eyes half shut,
and lays it down when it stops. If no spot
clears every face and the situation has lasted five times the grace period, the creature leaves the
ledge through the existing descent journey. No event is emitted for a sidestep. A user drag or toss,
an airborne creature, a climbing or hanging one, and a play scene inside its own bounded deadline
(at most 21 seconds) are exempt. So is a creature still walking to a spot it chose for itself, or a
sleeper part way through a shuffle: the resolver would drag it off the mark while its own walk
pushed back, the two moving it a pixel at a time in opposite directions, and once it arrives it is
standing still and can be asked properly. An exemption covers the pair, not just the creature
holding it — asking the free one of the two to move instead was measured and is worse, because it
walks the movable companion along in front of whoever is busy rather than resolving anything.

Because the exemption covers the pair, a colony large enough to crowd one floor needs its marks to
be right in the first place. A bond mark measured against one companion keeps its actor clear of
that companion and of nobody else, so on open ground it now slides outward past anybody already
standing on it — always further from the companion, always on the side the actor is already on, so
that it does not turn round mid-walk. A ledge is left alone: there is nowhere to slide to, and
`leave_the_surface` is the answer for a perch with no room. Without this, two companions each
standing exactly where its own errand sent it stayed drawn through each other for the length of
both errands, with neither one eligible to be asked to move.

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

- **Home, garden, roof, visitor** are the circumstances of a find turned up about the village
  while the houses are out: at home always, in the garden after watering or looking in on a patch,
  on the roof after sitting up there, and with a visitor if a guest is on stage at the time.
- **Morning, weekend, full moon, colony birthday** are read from the local clock for every find:
  06:00 until 10:00, Saturday or Sunday, after dark within a day and a half of a full moon, and
  within three days either side of the day the colony began, once it has had one.
- **After a nap** is a previous action of `Sleep`.

With no circumstance holding, the choice is uniform over the forty-eight everyday keepsakes, as it
was over the original eight. Once in sixty finds, anywhere, it is instead one of the four rare
ones. Otherwise half the draws go to an everyday trinket and half to a qualifying conditional one,
and among the qualifying ones three in four go to those not yet in the scrapbook when any are
missing. Exactly one draw is taken from the ambient stream on every path, and the conditional
decisions run on a private generator keyed from that stream's state without advancing it. The
everyday sequence is therefore bit-identical to 0.57, and the rest of the simulation's randomness is
unchanged whether or not any circumstance holds — asserted across all sixteen circumstance masks.

Game playthings are kept out of this. Keep-away and tug-of-war still show the holder in
`PresentDiscovery`, but the plaything is always an everyday variant chosen from the scene's own seed
(`seed % 8`), set on every change of hands, never a conditional keepsake — and a game still reaches
neither the scrapbook nor the journal.

The catalogue itself lives in `formiga-core::trinkets`: a hundred and sixty entries of name,
description, hint, and condition, with `TRINKET_VARIANTS = 160`; the first sixteen are the original
ones, drawn exactly as they were. `formiga-art::TrinketAtlasRenderer` bakes them into one 256×320
sheet — sixteen columns by ten rows of resting drawings, then the same again with a glint, 327,680
bytes — whose
inks are drawn from the colony seed and scored against every member's coat at once, so a keepsake
reads as a separate object whoever is holding it. A single holder's `prop_palette` keeps a belonging
90 away from that one coat; dodging a whole colony at once is necessarily a little softer, and the
tests report the worst distance rather than fixing a threshold, because the number that matters is
comfortably past the roughly 25 at which two colours start to read as the same.
The overlay's discovery quad and the trees sample it, so a trinket costs one texture per colony
instead of one row per creature; the settings pages hold only its resting half, 256×160. The
Journal's scrapbook lists only what has been found, and the Collection on the Your colony page
shows all hundred and sixty, those still to find as the shadow of their shape with the
catalogue's hint.

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
Its 57-byte payload is 92 Base32 characters in 23 groups. A recipe with classic parts uses format
version 3, identical except that the four reserved bytes hold those parts two to a byte; a recipe
without them is still written as version 2, so its code is unchanged. Legacy format 1 is unchanged;
a recipe is applied after legacy named-stream reconstruction so inherited traits and personalities
replay exactly. Design bytes also participate in the imported colony's lineage hash, the classic
bytes only when there are classic parts, so an existing code's lineage is unchanged.

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

`PostcardRenderer::render(&SaveFile, PostcardScene, caption) -> Canvas` produces a 960×600 opaque
postcard of the whole colony in one of four scenes the sender picks: a nap on a patchwork quilt one
golden afternoon, a picnic round a gingham blanket, a game on the grass with a ball in the air and a
kite overhead, or the village at dusk with its houses lit. The picture is painted on a canvas of its
own inside the card's frame — banded, dithered skies, two rows of hills, a meadow of tufts and
flowers — with the colony's own village across the back, laid out by `village_lots` exactly as the
portrait lays it out and faded toward the sky, the lamplit windows staying lit after dark. Every
member is drawn in the scene's pose from the frames the desktop already bakes — asleep with its eyes
shut, eating or drinking with its own snack or cup, playing, cheering, dancing, or waving — turned
toward the middle of the group, at the largest whole scale the group fits at. A stamp with the
colony house and a postmark with the month sit on the corner, and under the picture the caption,
if any, and the scene's name. `postcard_caption` keeps a caption to one line of at most sixty
characters with no control characters; the card carries no names, seeds, memories, scores, journal,
visitors, or display information, and its default filename names only the scene.

The Colony profile gains "Export sticker…" with a clip choice, and the Home page gains "Export
colony portrait…" and, in 0.59.0, a postcard with a scene, an optional caption, and "Export
postcard…". The `gif` crate, already in the workspace for `formiga-tools`, is now also a
dependency of `formiga-art`, so the application and the tools share one encoder; it is the first
time it is linked into the shipped application.

## Reference-guided generation and colony roles

The desktop host opens a user-selected PNG or JPEG only after an explicit Creature Studio action.
Decoding is capped at 16 MB, 4096 pixels per dimension, and 16 million pixels. The image is reduced
to a maximum 64×64 analysis surface without distorting aspect ratio. Alpha-aware foreground cues
summarize aspect, occupancy, symmetry, and upper/lower/side extensions. A fixed 512-bin color
histogram chooses dominant coat and contrasting accent colors. Exactly 512 named-stream candidate
recipes are adapted to these cues, rendered, and scored. The chosen preview contains its generated
creature, seed, recipe, and display-only affinity score; image bytes, path, metadata,
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
Absent recipes preserve legacy rendering and version 1 seed codes, and a companion without a recipe
has minis drawn from its own genes the same way. Preview acceptance carries the exact recipe into
add/replace; cards and overlays share the same palette resolver.

Save version 17 adds six classic parts to the recipe — coat, face, limbs, crown, pattern, and tail —
each zero for the plain modular part, so every earlier recipe reads unchanged. They come from a
`classic-parts-v1` stream drawn after the modular one, whose draws are untouched;
`CreatureDesign::modular` returns a seed's recipe before them. A new companion leans a quarter of
the time wholly modular, a quarter wholly classic, and otherwise mixes the parts at a lean of 0.35
or 0.65, so about 29% come out plainly modular and each main part is classic about half the time. Candy coats derive a full palette from the recipe's coat and accent —
tinted near-black ink, a desaturated shade, a bright highlight — with coat lightness held so the
eyes stay readable. The lit top moves the fill inside an unchanged outlined silhouette, classic nubs
cover the same resting spot as modular paws, and stick legs lift the body over unmoved feet, so the
reserved face, one limb per side, and the spacing boxes hold for every combination.

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
- Stickers, the colony portrait, and postcards: the save dialog, CPU canvases, and encoders exist
  only during an explicit export; cancellation renders nothing, and choosing a scene or typing a
  caption draws and uploads nothing.
- Display reconciliation: every 2 seconds.
- Persistence: arrivals, the houses coming and going, rituals, new belongings, and settings changes
  at once; everyday movement in a checkpoint at most every 15 seconds (`save_due`); and every 30
  seconds regardless.

State uses a versioned JSON file written by temporary-file, flush, atomic replace, and one backup.
Version 18 adds a house kind chosen by hand for any house, absent until one is chosen, and migrates
nothing. Version 17 adds the classic parts inside recipes, favorite visitors, each companion's
roaming leaning, and the habits each has picked up, and migrates nothing; version 16 added no field
and moved only so an older build would refuse a colony of six. Version 15 adds only `visitors`; a
v14 colony receives an empty `VisitorState` and nothing else is touched. 0.58.5 adds no saved field
and needs no migration: the version is still 15, and the two keepsake trees, where every find hangs,
and where every belonging lies are all derived at runtime from the colony seed and the scrapbook the
save already holds. The chain below it is unchanged: migration migrates v1 habitat settings,
deterministically resolves v2 face/forelimb/effect genes, assigns v3 colonies a deterministic
shelter, gives v4 creatures stable birth timestamps, upgrades v5 habits to the twelve strongest
numeric routines, and converts v1–v6 relationship floats into canonical shared four-score records. A
v7 colony keeps those canonical records byte-for-byte while receiving only its first deterministic
ritual timestamp; v8 receives only its first deterministic colony-object timestamp, and v9 receives
only its first deterministic shelter-decoration timestamp. v1–v10 creatures receive adult/mini role
metadata, Keep protection, and disabled legacy mini schedules without replacement. Migration
preserves creature IDs, resolved genomes, personality, and absent modular recipes; v11 colonies
retain their legacy appearances. Loose object positions are reconciled to the house yard on the next
tick. Migration also preserves custom names, birth times, memories, tendencies, routines, positions,
and settings. Raw memory plus tendencies stay below 192 bytes per creature; their serialized
incremental state stays below 2 KiB.

Additional minis are earned after one hour and one week; a full-size adult is earned after one
calendar month when an adult slot exists. End-of-month dates clamp in UTC, overdue reveals remain 15
seconds apart, and the colony remains capped at four. Interrupted ambient, interaction, and toss
actions reload as idle. Home and birth timestamps survive relaunches, and clock rollback uses the
maximum-seen UTC guard. There is
deliberately no history database or telemetry layer. Update preferences live in a separate
`updates.json` file so network policy and check timing cannot alter a colony save.


## Native colony interface (save v14)

`clubhouse.rs` holds only on-demand UI artwork and interaction state, with the Home page's
preview and shelves in `clubhouse/arrange.rs` and the Collection, the Journal's scrapbook and each
companion's wardrobe in `clubhouse/collection.rs`; `settings.rs` owns its egui window and
presentation. Four static portraits, four eight-frame candidate strips (six walk frames and two
expressions each), the top row of the village atlas, the object sheet, the resting half of the
colony trinket sheet, and a companion trying something on in four poses fit within 760 KiB of
artwork textures. It was 416 KiB until the trinket sheet replaced eight separate 16×16 drawings with
24 KiB more pixels in a single texture, 432 KiB until 0.59.0 gave every house a cell of its own,
496 KiB until the object strip grew from eight cells to fourteen, and 500 KiB until 0.60.0 brought
ten times the keepsakes (128 KiB more, even holding only the resting half), an object sheet with
every garden stage, spot, ornament and village prop (98 KiB more), and the try-on poses (36 KiB);
the Home page's village shrank to the one row it draws and cost nothing more. Tiles in a wrapped
row — the Collection, the pins, the shelves — are each allocated whole and painted into, because an
egui `Frame` places itself before its row decides whether it still fits and so never wraps. The home preview draws the whole corner — houses, both
trees, the keepsakes hung in them, and the belongings in the yards — from the village, trinket,
and object atlases the desktop already samples, positioned by the very layout functions the
overlay uses, so a complete village costs the same three textures whatever its size and nothing
about looking at it calls the colony home or moves a creature.

The last change to who lives here or how the village is laid out can be taken back. The app makes
each such change through `World::edit(ColonyEdit, change)`, which applies it and, if it changed the
members, the home, or the keepsakes' order, keeps one `UndoPoint` in the world: the creatures,
bonds, home, and keepsakes as they stood before, and the ids of any companion the change itself
brought in. It is runtime only and holds one change; a later one replaces it, and a failed or empty
one leaves it. `undo_last_edit` puts back what the change touched and nothing else. For a companion
removed, replaced, started over, or welcomed from the studio, the membership is rebuilt from the
old list: everyone still here keeps their current self, with the role they had, so a mini has its
own big version back; anyone gone comes back exactly as saved, with its id, memories, and bonds
with those still here, standing where it or its stand-in last stood; whoever the change brought in
goes, runtime and all; anyone who arrived on their own since stays; and the cottage order goes back
with them. A colony with no room to bring everyone back refuses and keeps the change to try again.
For a layout change — cottages, colours, gardens, spots, corner, display, decorations, keepsakes —
exactly those fields go back, with keepsakes found since kept after the rest. Renames, keeps, and
preferences are not part of it. The settings window's footer offers "Undo …" on every page while
there is something to take back.

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

The Journal page opens on "Today in your colony", read by `clubhouse::today` from nothing but the
journal and the scrapbook: today's entries by the local date, newest first; portraits of the
companions they name; a count of each kind of moment; the treasures first found today; and the
latest four lines. It says nothing about time the app was not running, since nothing was written
then, and when the journal is full and its oldest entry is itself from today it says that some of
today's earlier moments have already rolled out. It stores nothing.

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

Persistence accepts save versions 1–18: version 18 is read directly, versions 1 through 17 are
migrated on load, and anything else is refused. Version 17 adds classic parts to stored recipes and
migrates nothing, since a recipe without them is a plain modular one; it moved so an older build
refuses the colony rather than quietly dropping the parts. Version 18 moved for the same reason, so
that an older build refuses a village with a house kind chosen by hand. A missing primary can load
its backup; a corrupt primary is preserved before repair, without rotating over a valid backup. If
both files fail, the host disables writes and presents recovery choices. Explicit restores and
resets preserve uniquely named copies; snapshot imports validate bounded input before confirmation
and replacement.


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
Scenes stay at four, but everything remembered per resident — supports, setbacks, refused
invitations, display moves, rides, hangouts — is sized by `MAX_COLONY_CREATURES`, so the fifth and
sixth companions are noticed, walked around, looked at, and invited like the first four. An
eight-second support memory for every resident lets a confirmed loss prompt one search even after
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
point, and capped ride intensity for every resident on a window. The attention library authors balance, grip, dismount, boundary retreat, elevator, and
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
launch records one runtime setback per resident for 90 seconds, which raise perceived risk and block an
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
