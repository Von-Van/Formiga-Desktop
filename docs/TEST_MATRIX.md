# Release test matrix

Automated checks are the baseline, not a substitute for native desktop testing.

## Automated

| Check | macOS | Windows |
|---|---:|---:|
| Format and Clippy warnings denied | CI | CI |
| Workspace tests | CI | CI |
| Deterministic 1,000-genome render test | CI | CI |
| Cat and rabbit ears drawn for every appendage style and size, inside the frame margin | CI | CI |
| Raised cat tail and visible rabbit puff for every tail style | CI | CI |
| Resting cats plant all four paws on the same contact row as walking; gesture paws cap long generated reaches | CI | CI |
| v1–v18→v19 migration, creature/object/bond preservation, names, births, rituals, and top-12 routines | CI | CI |
| A v18 colony keeps what it earned: its decorations unlocked, the ones showing hung on the colony house in their places and the hidden ones taken down, every category topped up to three; one that never earned any hangs nothing and waits a day or two for its first new thing | CI | CI |
| A v14 colony migrates to an empty visitor state; the saved-field allowlist and runtime-only field denylist both hold | CI | CI |
| Atomic round trip and corrupt-primary recovery from the previous-save backup | CI | CI |
| One-hour, one-week, and clamped calendar-month arrival boundaries | CI | CI |
| Fixed memory/routine limits and sub-2-KiB per-creature serialized growth | CI | CI |
| Learned utility monotonicity, ±0.35 cap, saturation, and contrary-experience recovery | CI | CI |
| Descriptor ±35/±25 hysteresis, badge persistence, and 12-active-hour bubble throttle | CI | CI |
| Unicode name validation, deterministic unique defaults, and user duplicate names | CI | CI |
| 60-active-second observation projection and hidden/paused suspension | CI | CI |
| Six-pair relationship maximum, canonical IDs, four score bytes, saturation, and deterministic round trip | CI | CI |
| Five calm minutes, bounded bond utility, contrary avoidance reduction, and score projection | CI | CI |
| An afternoon at home builds no bond and spends none of the calm minutes a pair had already gathered | CI | CI |
| Targeted follow, shared sleep, gift, play/steal, greet, climb-watch, toss-concern, and squabble action reuse with stable side spacing | CI | CI |
| Bond target refresh/cancellation for moved, missing, sleeping, homebound, tossed, cross-surface, and removed companions | CI | CI |
| Deterministic 12–48-hour ritual scheduling, all nine kinds, safe eligibility, and no downtime catch-up flood | CI | CI |
| Ritual interruption, 2–6-hour retry, reduced motion, local hatch-day deduplication, and shared-plan caps | CI | CI |
| Multi-creature monitor-ID rebinding, presentation-buffer growth, and stalled-surface recovery path | CI | CI |
| Topology geometry-hash rebuild suppression and 64-window/96-landmark caps | CI | CI |
| Island/corner/slow-platform classification across overlap, negative coordinates, DPI, and rapid change | CI | CI |
| Cursor invitation 24-point, 1.5-second, and 25-points/second boundaries plus pause/hidden clearing | CI | CI |
| Four-hop window-graph cap, deterministic tier routing, and preference scoring bounds | CI | CI |
| Exact 10–28-point narrow-gap classification, support validation, and immediate geometry cancellation | CI | CI |
| Squeeze traversal-atlas reuse, 0.72× body/face scaling, and stable routine codes 0–23 | CI | CI |
| Deterministic 3–7-day object scheduling, one-after-downtime behavior, eight-object cap, and stable IDs | CI | CI |
| Eight-kind object-atlas determinism/alpha coverage, habitat recovery, cached static quads, and `+0.25` utility cap | CI | CI |
| Deterministic 4–9-day decoration scheduling, one-after-downtime behavior, six-unique-kind cap, and canonicalization | CI | CI |
| Memory/bond/ritual/object-driven decoration choice and deterministic single-texture 64×64 shelter baking | CI | CI |
| All four seed generations round-trip case-insensitively and reproduce innate identity byte-for-byte | CI | CI |
| Seed prefix/group/version/generation/length/alphabet/padding/checksum validation and corruption rejection | CI | CI |
| Imported fresh history, distinct companion lineage, explicit replacement gate, and unchanged save compatibility | CI | CI |
| Deterministic 960×600 opaque card pixels, Unicode names, long-name bounds, abbreviated seed, and zero-sized renderer state | CI | CI |
| Card save cancellation before rendering, Unicode-safe filenames, and PNG extension normalization | CI | CI |
| PNG/JPEG local decode limits, fixed 512-candidate matching, determinism, and full-size preview output | CI | CI |
| Modular recipes: bounded parts, 16-byte layout, all body/ear combinations, related minis | CI | CI |
| Classic parts: four-byte packing and range checks, a quarter modular and a quarter classic with every part reachable, minis keeping coat, face and limbs, and the v0.58.9 recipes, code and lineage byte for byte | CI | CI |
| Every classic body combination connected with a reserved face at extreme sizes, through every action and gesture, inside the frame and spacing boxes, with one limb per side | CI | CI |
| Both eyes readable in every classic face arrangement and expression, and candy colours keeping a dark face on any coat | CI | CI |
| A companion without a recipe has minis drawn from its own genes, replayed exactly by a version 1 code | CI | CI |
| A roaming leaning tips climbing without forbidding it, sends a homebody home, and over half an hour keeps floor-dwellers mostly off ledges and homebodies lower than the rest, saved only once chosen | CI | CI |
| Habits: at most two, one per kind of moment, kept once had and announced once; picked up at the measured pace; never taught at the door; hidden by reduced motion; done standing still, after any walk the action begins with, and ended by whatever replaces the action | CI | CI |
| Celebrations spread across hop, dance and twirl and fixed per creature; a cheer drawn as the creature's own; each habit drawn only while it is being done, with its own face; the stretch inside the frame and distinct on every body | CI | CI |
| A companion's profile lists its celebration and habits in order, and the journal names each habit picked up | CI | CI |
| Village moments: offered only while the houses are out, in sight, unpaused and with two companions free, rechecked when chosen; a picnic lines everyone up face-clear on the commons facing the middle, eats and drinks by turns, and hands them back with a journal line and a bond per pair | CI | CI |
| A dance poses every dancer and ends with each one's own celebration; reduced motion offers no dance and shares a moment in place; too few willing means no moment, and a sleeper stays asleep | CI | CI |
| Stopping, hiding or pausing ends a moment with no journal line and no bond; the houses wait for a moment under way; a pet leaves it going, something held out on the way takes one companion out and mid-moment is turned down, and a pick-up sends the houses and the moment away; the dance is never scheduled | CI | CI |
| Houses: every house a cell of its own by day and after dark, with and without somebody at home, plus the tree in one 512x256 atlas, the Home page's row exactly its top; a resident's curtain only in its doorway; after dark only lamplight changes; somebody at home draws the curtain across the door and lights a window without changing the silhouette; each house carries its keeper's mark and a mini adds none; doorways, footprints and decoration anchors held for every style, span, curtain and night | CI | CI |
| Decorations: thirty, each for one of a house's six places and five for every place; a house wears one per place and only what the village has; each drawn deterministically and changing the house; a house dressed in everything stays inside its own lot | CI | CI |
| The village gains something new every 24–48 hours on a deterministic schedule, one after downtime and in time everything, chosen by what the colony has been doing; a new decoration goes up on the colony house when its place there is free | CI | CI |
| Gardens grow round sprout, growing, grown and at their fullest by themselves at each kind's own pace, and the overlay draws each at its stage | CI | CI |
| Village life: gardens watered, sprouts looked in on, ripe patches picked and eaten, and something at its fullest shown to a friend who looks on pleased; a chore for each kind of house, answered by the house; somebody always outside, never more than two in, and the drawn curtain always the house somebody is in; residents up on their own roofs at the house's height and back down; a leaf on the face, a dropped snack and a missed cushion each run start to finish with somebody looking over; a find at home reaches the scrapbook; ending a visit, pausing and hiding bring everybody out and down; a roof sitter can be picked up and somebody indoors cannot; a plan made for the village as it was is let go when the corner changes or the resident's cottage moves, and only then | CI | CI |
| The simulation seats a roof sitter within two pixels of the top of every kind of house drawn at every height and width, colony house and cottage | CI | CI |
| Yawns go round: a friend looks over before catching one, never on top of the last; now and then a third holds out first; never further than three; had standing still; none in a paused, hidden or still colony | CI | CI |
| Something to wear: every accessory made from a different find; only what the colony has found can be worn; put on, swapped, shared between two, taken off and put back, worn as a pin; a file naming something unfound wears nothing; every accessory shows on every body in every kind of pose and sits on the body | CI | CI |
| Climbing: a companion on the floor reaches any window that leaves it room under the top of the screen; everybody comes down now and then and potters about before climbing again; a runner coming up from behind is let past; `measure_time_on_ledges` (ignored) reports the share of roaming time up high on five desktops | CI | CI |
| Hangout spots: one of each kind on the ground, normalized when the colony opens, kept with the village only once put down; placed on the standing span and never on one another; a cushion draws naps without taking over the afternoon, a lookout is looked out of toward the open desktop, and an invited picnic gathers at the blanket; the Home page's shelves put a spot, a garden or an ornament down in the widest gap left and take it up again | CI | CI |
| Village arrangement: the founder stays in the colony house whatever order is asked, cottages stand in the order arranged with the rest in arrival order, minis follow their big version's house, everyone walks home to the house now theirs, and an arrival-order arrangement writes nothing; a replacement keeps the cottage and a departure leaves the order; a named palette changes only the two palette indices and repaints every house and tree without reshaping a pixel; one garden patch of each kind, sharing the ground with the spots without touching; the reset puts back order, palette and gardens and leaves the spots; each kept with the village only once chosen; in the Home page's preview a cottage is carried along the row into a new place, the colony house cannot be carried, and a spot is carried along the ground; a palette is picked, and the reset happens only when asked twice | CI | CI |
| Home becomes Moment in the same cell; the moments strip opens beside the menu without moving it, on the right or at the display's edge on the left, answers hover and clicks, shares one click area, and draws a plain tray with its label level with the menu's | CI | CI |
| Today's recap reads only today's recorded moments and finds, newest first, and admits when a full journal has rolled some of today's out | CI | CI |
| The colony view lists every pair once, under how they get along, closest first, with an unrecorded pair still getting acquainted | CI | CI |
| A favorite visitor outlasts the guest book, is never kept twice, waits for room when eight are kept, and comes back exactly as it was kept | CI | CI |
| The fifth and sixth companions of a full colony feel their ride, keep favourite places, find a playmate, and are carried back from a removed display into places of their own | CI | CI |
| Connected modular silhouettes and reserved faces at minimum/maximum proportions | CI | CI |
| Wing styles: all three reachable, stable per recipe, and visibly textured | CI | CI |
| Image aspect/alpha handling, dominant/accent colors, blank and extreme references | CI | CI |
| v11 save preservation, v1/v2/v3 code validation, exact design add/replace/save/share, and a v16 colony opening unchanged in v17 | CI | CI |
| Mirrored village layout: separated lots, shared ground line, scale changes, missing/narrow displays, and a narrow corner giving up a tree before a house | CI | CI |
| Village atlas cells match their own dwelling and never bleed into a neighbour; both trees come from the fourth cell, the inward one mirrored | CI | CI |
| Companion houses per colony member, with matching half-size cottages for minis | CI | CI |
| Two trees bookending the houses: the widest village inside its 448-pixel span limit at the tightest scale and on a real display, both corners identical, and each tree's whole lot clear of the screen edge | CI | CI |
| Six companions with a house for every full-size one, a mini coming home to its big version's, and the widest village still inside its span limit | CI | CI |
| Companions roam the commons between the trees: on it, spaced, out of the yards, and still where a hidden or reduced-motion colony left them | CI | CI |
| Topping out a climb hauls straight up and then steps in, with no sideways drift | CI | CI |
| A visitor's tour: every stop inside the region, clear of each doorway, of every resting resident and of every belonging; the guest stands in as many places as it planned, goes over to each resident in turn, and a narrowed display replans the ring | CI | CI |
| Nobody turns round more than six times in a second with a companion near, across seeded colonies on a busy desktop | CI | CI |
| A keepsake is drawn square whatever the shape of the sheet it is cut from | CI | CI |
| The open habitat editor takes the pointer and leaves the overlay every other event, its redraw above all | CI | CI |
| The ground the colony is founded on clears a Dock or taskbar at its factory size | CI | CI |
| The overlap table holds every pair a full colony can make, and hands each pair its own record | CI | CI |
| A full colony over sixteen seeded sessions leaves no face covered for something one could sit and watch; a colony of four still meets the tighter bound | CI | CI |
| Keepsakes and belongings in the yards: sixteen hooks, one tree and one anchor each; the trees fill themselves until chosen, the first sixteen finds keeping their hooks; every hung keepsake drawn once inside its own tree's cell; four belongings to each yard fixed by slot, `BELONGING_CLEARANCE` held after the per-colony drift, and no resident standing on one | CI | CI |
| Collection and scrapbook: the Journal lists only what has been found; the Your colony page shows all hundred and sixty in a wrapped grid, hangs one in the trees or takes it down, and lets the trees fill themselves again | CI | CI |
| Belonging colors stay distinct from every creature palette they are carried against | CI | CI |
| Four-total/three-adult/two-minis-per-adult caps, even distribution, and oldest-adult tie-break | CI | CI |
| Keep replacement guard, bulk regeneration, final-adult protection, reparenting, and relationship normalization | CI | CI |
| 1,000 genomes × all actions and layered face checks | CI | CI |
| Generated passive-prop determinism and alpha coverage | CI | CI |
| Ambient cadence bounds, deterministic landmarks, pause/hidden suspension | CI | CI |
| Continuous upward traverse/climb/mantle, downward hops, and route interruption | CI | CI |
| Raised dangling GPU/proxy handhold placement contract | CI | CI |
| A hundred and sixty deterministic keepsakes, each its own object with a glint, readable inside its own cell and clear of the eyes; the original sixteen keep their numbers, names and circumstances | CI | CI |
| Exactly 134 unique body frames (92 action, 42 gesture), one slot each; 1,649,664 bytes a companion, a full colony under 10 MiB; layered atlas at or below 4,500,000 bytes | CI | CI |
| One limb per side: a raised paw or wing leaves no resting nub or folded wing behind, and winged bodies reach with their wings | CI | CI |
| Every action and gesture keeps one connected body and the reserved face on every body plan at extreme sizes | CI | CI |
| Twelve gestures are distinct, moving, in-frame poses on every body, the yawn among them; only an attention gesture replaces the action clip; covering the face closes the eyes | CI | CI |
| A gesture is presented at its own frame rate, not that of the action beneath it | CI | CI |
| Every body pose has a scene that strikes it; poses never appear over travel, hops, hangs, reduced motion, or an action whose own clip is the point | CI | CI |
| A tug of war reaches its pull and hauls, from the position its settling-in walk actually reaches | CI | CI |
| Habitat region algebra | CI | CI |
| Pet/drag maximum-excursion classification across scale, placement, toss, cancel, and re-grab | CI | CI |
| Home petting without dismissal and threshold-crossing shelter dismissal | CI | CI |
| House spawn preserves positions; walking/landing, pause/pet/relaunch, display crossing, and spaced arrivals | CI | CI |
| Blank fixed-size milestone-bubble pixels, global singularity, and idle resource release | CI | CI |
| Alpha-mask and occlusion geometry | CI | CI |
| Update version/asset selection and 24-hour throttle | CI | CI |
| Update digest validation and unsafe-name rejection | CI | CI |
| Offers: acceptance curve and clamp, sleep hard stops, the three cooldowns, every refusal condition, bounded reversible learning, and a behaviour stream left undisturbed | CI | CI |
| Send home starts an ordinary visit without the cooldown, refuses while paused, mid-interaction, or with no usable home display, and answers with one bubble each | CI | CI |
| Thought bubbles: 2.4-second lifetime, growth steps, holding a repeated icon and swapping a different one, the five-at-once cap, hidden and reduced-motion behaviour, and nothing saved | CI | CI |
| Creature menu: member and guest item sets, the swapped Stay/Copy-code cell, placement above and flipped below at every scale, every dismissal cause, and a hit region covering only the strip body | CI | CI |
| UI atlas: every bubble, frame, icon, and label tab inside one 256×80 texture, built on demand and released when idle | CI | CI |
| Visitors: one wanderer per block of four gatherings, the full visit timeline, residents answering without changing a bond, tendency, or counter, and every interruption path | CI | CI |
| Invitations: a duplicate or already-present code refused, a 24-hour stay attended at every gathering, one guest-book signature per visit, the 24-entry cap, and staying through the ordinary adoption path | CI | CI |
| Village layout: a porch sharing its own house's lot line, everyone resting beside its own door, both narrow-display fallbacks, and a guest spot clear of every resident | CI | CI |
| Doorstep moments: pacing bounds, one resident busy at a time, cancellation by pet, pick-up, dismissal, pause, hide, or changed geometry, and no tendency, counter, bond, or journal line moving | CI | CI |
| Spacing: the face and body boxes hold for every body plan at four sizes across every baked clip, frame, expression, and facing; source-site spacing for bonds, rituals, races, attention, and held formations; Gather Creatures leaving room | CI | CI |
| Overlap resolution: grace periods, per-pair cooldown and clearance margin, who moves and who never does, silent sleeper shuffles, the descent fallback, and a continuously moving synthetic desktop | CI | CI |
| An 18-point window is no landing rather than a panic; Gather Creatures settles every runtime plan, attention scenes included; regenerating past an unkept adult with no seed still lets the unkept minis go | CI | CI |
| Conditional discovery: a keepsake never turns up without its circumstance and a circumstance makes one likely without promising it; with nothing special going on only everyday and rare ones turn up; every keepsake in the catalogue can be found; one ambient draw on every path; game playthings staying everyday | CI | CI |
| Colony trinket atlas: a hundred and sixty variants × rest and glint cells inside one 256×320 sheet, coloured clear of every member's coat | CI | CI |
| The watching pose and window gaze: a geometry cue raising `Watch`, the near edge at mid-height rather than the creature's own feet, and a lean drawn on every body | CI | CI |
| Stickers: every clip and scale, byte-deterministic GIFs, the creature's own cadence in the frame delays, an infinite loop, and no comment/application/plain-text blocks | CI | CI |
| Colony portrait: deterministic 960×600 opaque pixels that stay identical with memories, tendencies, and relationships maxed out, and no PNG text chunks | CI | CI |
| House kinds: the colony house is the colony's own kind and every cottage its keeper's own until one is chosen; a choice is one per house, passes to a replacement, leaves with a departing companion, is cleared by putting the village back and undone like any layout change; every kind turns up among keepers about equally; the saved names are the ones colony files have always used; each kind keeps its doorway, footprint, curtain and lamplight rules, and every palette paints every kind without reshaping it; the Home page builds a house as another kind and gives it back its own | CI | CI |
| Sleep and strolls: a companion walking to bed is drawn walking, heavy-lidded, and lies down when it stops; a sleeper that has to make room is towed by a free friend at an unhurried pace without waking anyone, the rope let go at the end; with nobody free it wriggles over, and a tow cut short by a pick-up ends at once; the rope reaches from hand to sleeper without a gap and sags; residents stroll out and back with at most three out at once, and every visit starts with the walk home; a stroll at home ticks and draws at 10 Hz and anything brisker at 20 | CI | CI |
| Undo: a removed companion comes back with its id, place, memories, bonds, minis and cottage while everyone else keeps what they did since; a replaced one comes back and its replacement leaves no creature or bond behind; a welcomed one goes and a natural arrival stays; a full colony refuses and keeps the change; a layout change puts back only its own fields, keepsakes found since after the rest; failed and empty changes keep the earlier one; the footer offers the change on every page only while there is one | CI | CI |
| Postcards: every scene deterministic, opaque, and different from the others for a colony of one and of six; everyone in the picture, in order, inside the frame; a caption changes only the lines under the picture and is written without control characters, folded and cut to sixty characters; a cancelled export writes nothing, a sent one has no text chunks, names, or caption bytes, and a filename naming only the scene; the Home page picks a scene and a caption and uploads nothing | CI | CI |
| Resting: a six-frame loop with at least four postures on every body plan, the settle alone under reduced motion, both sides and both manners reached by generated companions, and no faster than the clip it replaced | CI | CI |
| Ground line: every standing clip stands on one row, for the classic families and 64 modular seeds, with and without reduced motion | CI | CI |
| Bubbles on transitions: a music note over every companion that joins a game and over none of its watchers; a Z over a companion as it drops off, by any route into a nap, and over nobody still standing | CI | CI |
| Updates folder: installers for the running version or an earlier one, and part-finished downloads untouched for a day, are cleared at start; later installers, other files, and names that are not Formiga's own are kept | CI | CI |
| Stroll cadence: every stroll a companion can take stays at or under `MAX_STROLL_SPEED` and every walk well over it, the briskest stroll still ticks and draws at 10 Hz, a walk home at 20, and a resting colony is redrawn three times a second | CI | CI |
| Nothing below the feet: sleeping, eating, and crouching frames of the three classic families and 64 modular seeds, with every snack and both motion settings, stay on or above the row the feet reach | CI | CI |
| Universal app / x64 package | CI | CI |

## Companion interface checks (0.57.0)

Automated coverage includes v12→v13 and v13→v14 preservation and snapshot round trips,
corrupt-primary repair without backup loss, missing-primary recovery, exact source-generation
adoption, Keep and capacity refusal without mutation, replacement mini parentage, journal
bounds/throttling/privacy, quiet expiry across relaunch, behavior-preset scope, all native egui
pages at 940×720 and 760×560 in both the cream and the charcoal theme, and bounded UI artwork
release. The v14 keepsakes are covered directly: pins stay bounded at eight and only ever name a
moment the journal holds; the scrapbook keeps one record per variant with the first finder, however
often the same trinket is found again; invalid schedule rows and out-of-range text scales are
dropped by normalization. Schedule arithmetic is covered for overnight periods, weekends,
duplicate times, timezone changes, a daylight-saving gap, and a week of missed transitions, and
the world-level behavior for applying a routine once, respecting a manual choice until the next
change, skipping a routine that was never saved or whose habitat leaves nowhere to stand, and
leaving visibility, pause, and the quiet expiry alone. The home preview is exercised in both
corners, at two creature scales, under three habitat presets, and with the home inactive, asserting
that looking at it never requests a home visit and never changes the colony; its texture count is
the same for a full village as for one house. Theme coverage asserts body text contrasts with the
page in either theme and that text scaling reaches every style without running away. The optional
sprite outline is asserted to leave every one of the creature's own pixels untouched and to keep
the atlas the same size. Page PNGs can be generated without a live colony using:

```sh
FORMIGA_UI_REVIEW_DIR=/tmp/formiga-ui-review cargo test -p formiga-desktop all_pages_render
```

```sh
FORMIGA_ATTENTION_REVIEW_DIR="$PWD/target/attention-review" cargo test -p formiga-desktop attention_review
```

```sh
FORMIGA_UI_REVIEW_DIR="$PWD/target/formiga-ui-review" cargo test -p formiga-desktop --bin formiga desktop_ui_review
```

The third command writes `desktop-ui.png`: the production vertex code compositing real creature
sprites with thought bubbles and open menus at 2×, 3×, and 4×, both floating above the head and
flipped below it. It is the only sheet that shows the overlay's own geometry rather than the
settings window's.

Setting `FORMIGA_ATTENTION_REVIEW_DIR` to an absolute path writes the sheets each of those tests
draws. They show production sprite composition over synthetic geometry, not the native compositor,
and they are dense: crop a row and enlarge it with nearest-neighbour scaling before judging a face.
How to read them:

- `attention-stages.png` — columns are a timid rider, a bold rider, and two observers; rows are
  notice, reaction, and recovery.
- `inspection-approach.png` — notice, a short approach, and inspection, with a spaced audience.
- `cursor-and-display.png` — columns are a bold cursor investigation, a wary withdrawal, a display
  discovery, and a reorientation after a display is lost; rows are notice and movement.
- `ledge-and-gap.png` — a ledge peek, a refused gap, and a successful crossing, each with its
  audience.
- `catches-and-grips.png` — a marginal jump read left to right as airborne, hanging by the hands,
  mantling, and celebrating, alongside timid riders gripping a window that moved.
- `copycat-and-stare.png` — imitators raising a copied gesture in turn, and a staring contest whose
  loser closes its eyes.
- `spectator-reactions.png` — one marginal attempt at notice, preparation, the catch, the moment
  after, and the outcome, with three watchers drawn at 4×.
- `window-watching.png` — notice, watch, and settle beside a synthetic window, on nine reference
  bodies. This is the sheet to judge the watching pose by: the head should lean toward the window
  and the ears should be up, and the loop should keep breathing rather than freeze.
- `body-plans-and-scales.png`, `anchors-by-body-plan.png`, `prop-handoff.png`,
  `outline-on-busy-wallpaper.png`, and `narrow-gaps-and-short-ledges.png` — every body plan at Small,
  Medium, and Large, adult and mini; feet and grips against a drawn contact line; a carried prop in
  both facings; the optional outline over a busy checker; and squeezes and short ledges.

The sheets were reviewed for this release. `attention-stages.png` and
`spectator-reactions.png` show the watcher expressions as genuinely distinct: eye openness,
highlight, and mouth all change between curious, startled, concerned, averting, relieved, and
enjoying, and the closed-eye poses read unmistakably at the sizes the desktop draws.
`catches-and-grips.png` shows standing feet meeting the ledge line and gripping hands hanging from
it, matching the contact anchor the tests assert numerically. Nothing in those sheets needed a
change. The three geometry games have no sheet of their own; they are covered by the assertions
below.

Native follow-up checks remain pending on macOS and Windows: introduction pet/drag observations,
open/close/minimize during preview playback, multi-display home edits and reconnects, native
export/restore cancellation and dialogs, tray quiet-mode expiry, keyboard navigation, the dark
palette and text scaling under each platform's own appearance setting and its system-appearance
changes, the sprite outline over bright and busy wallpaper, a scheduled routine transition across
sleep and a timezone change, and energy measurements with the menu open and closed.

## Manual release gates

Use `pass`, `fail`, or an issue link. Do not mark a row from compilation evidence alone.

| Scenario | macOS 14 arm64 | macOS x64/Rosetta | Windows 10 22H2 | Windows 11 |
|---|---|---|---|---|
| One-hour unrelated-click test | pending | pending | pending | pending |
| No focus activation while dragging | pending | pending | pending | pending |
| Opaque-pixel hit test at 100/150/200% | pending | pending | pending | pending |
| Drag release/cancel/pause/reduce-motion | pending | pending | pending | pending |
| Slow placement, fast toss, soft bounce, and mid-flight re-grab | pending | pending | pending | pending |
| Pet click, drag-out-and-back, shelter pet, and visible reaction at supported scales | pending | pending | pending | pending |
| House appearance: immediate walking, ledge descent, spaced resting, pause, pet, and drag dismissal during approach | pending | pending | pending | pending |
| Smooth climb/mantle, raised dangle contact, downward hop, inspection, and discovery | pending | pending | pending | pending |
| Colony names, descriptors, unread badges, age, places, and wordless milestone bubble | pending | pending | pending | pending |
| Bond profile labels and closest-companion changes remain read-only except for the name | pending | pending | pending | pending |
| Clearly separated follow, sleep-beside, gift, toy steal, shelter greet, climb watch, toss concern, and harmless squabble playback | pending | pending | pending | pending |
| Bond sequence cancellation when a target moves, sleeps, enters shelter, is tossed, or disappears | pending | pending | pending | pending |
| Picnic, group nap, floor race, shelter gathering, catch, presentation, hatch day, quiet huddle, and sleep pile playback | pending | pending | pending | pending |
| Ritual interruption by hide, pause, drag, toss, geometry change, and reduced-motion substitution | pending | pending | pending | pending |
| Chase, procession, dance, pile, leapfrog, keep-away, tug, tag, gap turns, copied route, ledge contest, window race, the-floor-is-lava, and hide-and-seek playback | pending | pending | pending | pending |
| Habitat create/move/resize/toggle/delete | pending | pending | pending | pending |
| The desktop editor draws the region under the cursor, the settings window stays in front of it throughout, and Apply, Cancel and Reset all remain clickable | pending | pending | pending | pending |
| Creature menus, petting and dragging all still work after an edit is applied and after one is cancelled | pending | pending | pending | pending |
| The village and a creature resting at the foot of the screen stand clear of the Dock, shown and auto-hidden, and of the taskbar | pending | pending | pending | pending |
| A colony of six out on the desktop: nobody is left drawn through anybody for more than a moment, stepping aside reads as walking rather than twitching, and a sleeper shuffles without waking | pending | pending | pending | pending |
| Invalid habitat cannot replace valid policy | pending | pending | pending | pending |
| Selected app occlusion and rule removal | pending | pending | pending | pending |
| Window ordering/minimize/close | pending | pending | pending | pending |
| Moving/minimized/closed supporting window during climb or dangle | pending | pending | pending | pending |
| Window islands, exposed-corner peeks, slow platform rides, and calm cursor invitations | pending | pending | pending | pending |
| Four-tier constructions, narrow-gap squeeze playback, and route cancellation during window motion | pending | pending | pending | pending |
| Eight object kinds, multi-display placement, habitat recovery, static rendering, and no downtime flood | pending | pending | pending | pending |
| Six shelter decoration kinds, history-reflective choice, overdue behavior, and unchanged single-quad rendering | pending | pending | pending | pending |
| Copy seed, mixed-case paste, invalid/corrupted refusal, explicit replacement, relaunch, and one-hour companion arrival | pending | pending | pending | pending |
| Creature-card save/cancel, visual legibility, Unicode name, PNG dimensions, metadata inspection, and reopen in native image viewer | pending | pending | pending | pending |
| Random/reference preview, PNG/JPEG matching, add/replace/remove, Keep, bulk regeneration, and preview cleanup | pending | pending | pending | pending |
| Two-adult even minis, three-adult oldest tie-break, one-month full-size arrival, relaunch, and removal reparenting | pending | pending | pending | pending |
| Multi-display, negative coordinates, hot-plug | pending | pending | pending | pending |
| Toss across monitor seams, negative coordinates, and custom habitat boundaries | pending | pending | pending | pending |
| Spaces/virtual desktops and default full-screen hiding | automated geometry; native pending | automated geometry; native pending | pending | pending |
| Sleep/wake and lock/unlock | pending | pending | pending | pending |
| Two- and six-creature rendering for one hour without disappearance; automatic surface recovery | pending | pending | pending | pending |
| Right-click and, on macOS, Control-click open the strip without stealing activation or focus; icon clicks act while the border and gaps do nothing | pending | pending | pending | pending |
| The notch, label tab, and everything around the strip stay click-through; on Windows the region clips to the strip body and WM_RBUTTONDOWN reaches the no-activate tool window without a taskbar flash | pending | pending | pending | pending |
| The menu survives a Space switch, hides under a full-screen application, sizes correctly on mixed DPI (100/125/150/200%), and closes when the creature is dragged between displays | pending | pending | pending | pending |
| Thought bubbles read over real wallpaper at every creature size, sit at the crown for adults and minis alike, and clamp inside the display | pending | pending | pending | pending |
| A full visit on a real desktop: arrival, greeting, residents answering, the calm beats, the farewell, and departure before the houses close | pending | pending | pending | pending |
| Ask to stay and Copy code from both the menu and the Journal page, including a full colony refusing a stay | pending | pending | pending | pending |
| Doorstep moments at the houses: each kind plays, only one resident is busy at a time, and a pet or pick-up cancels one at once | pending | pending | pending | pending |
| The village on a real desktop: both trees bookending the houses, keepsakes filling the branches as they are found, the belongings scattered in the two yards, and the whole corner on screen at every scale in both corners | pending | pending | pending | pending |
| Sticker, colony-portrait, and postcard export dialogs: save, cancel, Unicode names and captions, and reopening every file in a native viewer | pending | pending | pending | pending |
| Package install, relaunch, v18 round trip, and representative v1–v17 migration without creature loss | pending | pending | pending | pending |
| Manual and automatic GitHub update check | pending | pending | pending | pending |
| Verified update download; corrupt checksum refusal | pending | pending | pending | pending |
| DMG/MSI handoff without silent installation | pending | pending | pending | pending |

Unsigned preview status must remain explicit until signing credentials are configured.


### Environmental attention coverage

Automated core scenarios cover new-edge approaches, timid growth retreats, confirmed support-loss
searches, distinct local movement bursts, exposed-tier/open-space preferences, fresh versus cached
scans, and reset after unreliable observations. They also cover staged audience timing, shared
origins, reserved viewing/landing spots, occlusion, exclusions, reduced motion, changed geometry,
and immediate cancellation after losing a supporting ledge. Existing one-minute ledge-discovery
coverage remains a regression gate for exploration utility changes.

The test-only desktop `attention_review` module checks production sprite composition for rider
notice/reaction/relief and new-window notice/approach/inspection. An absolute
`FORMIGA_ATTENTION_REVIEW_DIR` enables optional PNG sheets; release builds exclude this helper.
Native macOS/Windows compositor behavior, actual window manipulation, multi-display playback, and
CPU/memory/energy measurements remain manual release checks.


Display/cursor scenarios cover continuous seam crossings in both horizontal directions, differing
scale factors, disallowed/disconnected routes, removal during a route, paused multi-creature
recovery, stable contacts during resolution changes, identifier/DPI churn, and stale observations.
Cursor tests cover fast passes, local circles, stationary/straight motion, warps, unavailable input,
preference disablement, cooldowns, timid/bold responses, bounded learned confidence, blocked retreat,
and reduced motion. Production-renderer coverage adds cursor investigation/avoidance and display
exploration/reorientation. Real hot-plug, sleep/wake, and mixed-DPI native playback remain pending.

Surface/comedy coverage adds dwell/decay and coarse preference serialization, relative height,
peeking/refusal/running jumps, hand contact, pull-up/help/shared-fall recovery, reserved landings,
continuous rides and cached scans, reversal/dizziness, boundary retreats, and calm cancellation.
Bridge crossing, dangling commutes, accidental launch, and one-time staircase repair have synthetic
geometry fixtures. Converging-window coverage includes a retreat along the ledge, a hop down when
both exits close, a braced reduced-motion alternative, and a support that carries its rider toward
a stationary window. Hesitation coverage includes the full two-attempt reconsideration and retreat,
a blocked immediate retry, committing from the edge with the same origin and a spectator gasp,
interruption by a moved destination or reduced motion, and setback and sleepiness effects on the
decision. Geometry classification covers one-edge resizes, snapped windows, and one-to-one
identifier churn for both observation and an occupied perch. Production sprite sheets additionally exercise
catch/pull-up/helper/grip placement through the same contact calculation as native hit proxies.

Play coverage checks ordinary-gesture invitations, finite imitation chains with a remaining
spectator, sustained gaze and contest endings, personality refusal, no invented solitary partner,
runtime-only state, cooldowns, reduced motion, and interruption without false completion events.
Each game has its own scenario: chase swaps and endings, procession spacing and straggler release,
a gathered dance circle, a pile that leaves its anchor alone, leapfrog turns with a real arc,
keep-away hand-offs and a toy that never becomes a discovery, a tug that pulls both ways, tag
immunity, bounded turns at a gap with the others watching, a copied route with hesitation, a ledge
spot changing hands by stepping aside, creeping around a sleeper, pestering a rested one awake but
not an exhausted one, a cursor race gated by the cursor preference, two riders showing off on one
moving window, and that reduced motion never starts a travelling game.

The three geometry games are covered in the same way. A race is asserted to agree on one finish
line two hops away, to keep that finish line while it exists, to actually carry a racer off the
starting ledge, and to be replanned or called off when the destination window closes mid-race.
The-floor-is-lava is asserted to show reluctance at the brink, to move the pair in off the edge,
never to push anyone off the ledge itself, and — when a member does end up on the floor — to end
the round with the others reacting rather than ending the creature. Hide and seek is asserted to
count without the seeker moving, to get the hider away from where it started, to search afterwards,
and for its remembered position to change only while the hider is genuinely in view. A further
scenario asserts that no two playing creatures ever claim the same spot and that no hop is aimed at
a spot another creature has taken or reserved — the honest-space contract, without a collision
solver. Reduced motion starts none of the three.

Body poses are covered as a layer over the action, never as a replacement for one. A recorder runs
inside every scene test and fails on a pose struck over travel, a hop, a hanging creature, reduced
motion, or an action whose own clip is the point; a sweep of a slipped rescue, a leapfrog, and a
race asserts that those scenes genuinely did travel, hang, hop, and fall while striking none. Every
pose in the vocabulary is asserted to have a scene that strikes it, and the moments have focused
tests of their own: a ledge peek fretting over the drop, a brink teetering, a dare crouching or
fretting according to nerve, a helper hauling and then cheering, a seeker counting with its eyes
covered, a race field cheering the winner in, a dance on the beat, an empty-handed player reaching
after the toy, a cursor close enough to reach for, a tug that hauls on the toy between them, and a
support that vanishes underfoot, gasping once and then only looking around.

Every game asserts its own audience, through a shared recorder that tracks each watcher's gaze
targets, feelings, assigned participant, and whether it cheered, and fails on four faults: a gaze
that strays from the participant it was given, a watcher quietly conscripted into the scene, a
plan alive past its notice window with no pose, and a plan from another origin. The gaze is
asserted to follow the toy as it changes hands, to follow the best spot as it changes owner, to
track a runner across the ledge, to rest on a pile's anchor, and — in hide and seek — to follow
the seeker rather than the hider. Temperament is asserted where it shows: a playful watcher cheers
a tag ending that a quiet one only finds a relief.

Stage coverage asserts the vocabulary itself: every role a creature can take publishes a cue,
only observers stay silent, roles that own their own stage report it unchanged, and the rest move
through at least two stages and end on one a scene can end on. A companion test asserts that one
cue produces each watcher's own response — a timid watcher averts and its gaze turns with it, a
bold one stays concerned, and a watcher that has only just looked up begins at the beginning.
Anchor coverage asserts a carried prop mirrors with its holder's facing, rides in front and
lifted, and stays inside the body frame so it clips and occludes with whoever holds it.

Spectator coverage runs one attempt with two, three, and four creatures through both outcomes,
checking each watcher notices, responds, and settles, that only a real success draws delight, and
that an actor which becomes hidden loses its audience. `spectator-reactions.png` captures notice,
preparation, catch, the moment after, and the outcome, and the test asserts the sequence it draws.

Route coverage adds per-creature reach limits in both directions, learned climbing and fatigue
moving the same creature across that limit, and a reversing staircase scoring below a monotonic
one. Movement signatures are checked for stability per creature, staying inside every clip, visiting
every frame of a loop, leaving one-shot clips alone, and differing with temperament.
Production renderer fixtures capture performer handoffs and expressions. The game catalog is now
complete, so what remains is native playback on real desktops; these tests do not complete the
0.57.0 release gate.
