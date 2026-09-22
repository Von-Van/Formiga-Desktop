# Changelog

All notable changes are documented here.

## [Unreleased]

### Changed

- Dependencies are brought up to their latest compatible versions: egui and its companion crates
  from 0.36.1 to 0.36.2, ureq from 3.4.0 to 3.4.2, and the crates beneath them, 55 packages in
  all. Nothing drawn changes: every documentation image regenerates byte for byte, the
  settings-window review tests pass unchanged, and `cargo audit` finds no vulnerability. Its three
  remaining warnings are for crates that are only built on Linux.

### Fixed

- Nothing a companion folds against itself sinks below its own feet any more. A folded wing hangs
  from the shoulder, so on some winged bodies a sleeper's breath or a crouch carried the wingtip a
  pixel into the ground. A large head on a body a crouch had squashed flat did the same, and a
  blob's last mouthful dropped a crumb a row beneath its feet. Each now stops at the ground. Of
  20,280 frames rendered for 67 test bodies and the owner's colony, exactly 17 changed, and every
  one of them was a frame where something had gone below the feet. A test now holds sleeping,
  eating, and crouching to the ground for every body. A cup held by a body that sits low still
  hangs below its feet, because lifting it clear of the ground puts it across the face.

## [0.59.5] - 2026-09-22

### Added

- A music note over each companion as it joins a game: the pair that start one, and anyone
  recruited once it is under way. The ones who stop to watch stay quiet, and a player who steps
  out to cross a gap and comes back does not say it twice.
- A Z over a companion as it drops off to sleep, however it got there: a quiet moment at its own
  door, the nap cushion, or an ordinary sleep out on the desktop. Every way into a nap ends in the
  same event, so one pass over each tick's events catches them all. Both bubbles use icons the
  interface atlas already had, so neither adds any art.
- Formiga clears out the installers it has finished with. Each time it starts, it removes from its
  `updates` folder every installer it downloaded for the version now running or an earlier one,
  and any part-finished download left untouched for a day. An installer for a later version stays,
  since it may not have been installed yet. Files that are not named as Formiga's own downloads
  are never touched, and nothing is cleared while Formiga is running from a mounted disk image.

### Changed

- A companion with nothing to do now sits like it has something on its mind. Resting was four
  frames of a one-pixel bob, held square and facing the screen for as long as there was nothing
  else to do. It is now a six-frame loop at three frames a second: it settles, shifts its weight
  onto one foot with the other scuffing in and its shoulders dropping, tips its head and pricks
  its ears toward that side to look at something, and settles back. Which side it settles onto,
  and whether it slumps into the shift or keeps its legs under it, are read from bytes the
  creature's appearance already carries, so a colony sitting about is not a row of companions
  doing the same nothing in step, and nothing new is stored to say so. The plain square settle is
  still the first frame of the loop, and it is the only frame reduced motion draws.
- The liveliest companions now stroll at ten frames a second like everyone else, a step of about
  two and a half points with the walk cycle at four and a half frames a second. Their strolls ran
  just over the old threshold and were drawn at twenty. A walk home, and anything out on the
  desktop, still gets twenty.

### Fixed

- Standing companions keep to one ground line. For blobs and every modular body plan, a bob or a
  lift moved the floor along with the body. The resting pose therefore set the ground, and every
  other standing clip was drawn a pixel or two above it, so a companion rose as it set off walking
  and dropped back when it stopped. That was most noticeable in the village. The floor is now the
  same row in every standing clip, and the body settles over feet that stay planted; a blob, which
  has no legs to fold, squashes against the ground instead. A test holds every standing clip of the
  three classic families and 64 modular seeds to one row, with and without reduced motion.

### Security

- `rustls` is updated from 0.23.43 to 0.23.45 for RUSTSEC-2026-0285, a TLS 1.3 handshake flaw
  rated medium. It is the TLS library behind the update check and update downloads.

### Performance

- A strolling village costs about half what it did. With the houses out, the owner's colony of five
  averaged 1.32% CPU against 2.44% for 0.59.2 in the same sitting, and 1.30% against 2.47% in a
  second, independent pair of runs; the footprint is 110 MB against 111. Almost all of the cost was
  frames. The rule that lets a stroll tick and draw at ten frames a second tested movement against
  a round 24 points a second, but a stroll reaches 26.1 for the briskest companion, so a colony
  with one lively member held the whole village at twenty. The threshold is now the stroll's own
  ceiling, `MAX_STROLL_SPEED`, derived from the walk speed the village itself uses, and a test
  holds strolls under it and walks well over it. Frames presented at home fell from 11.5 a second
  to 7.1.
- The overlay binds its vertex buffer once per frame and each draw names its own range, rather
  than binding a new slice for every draw. That is 48.4 µs against 41.8 µs per pass in a headless
  benchmark; in the running app it is inside the spread between runs.
- Measured and left alone: skipping frames that changed nothing (one frame in 1,381 was unchanged),
  caching the village layout (0.26 µs a frame), and the interaction proxies (no native calls at all
  over two minutes of strolling). [PERFORMANCE.md](docs/PERFORMANCE.md) has the numbers.
- The longer rest costs nothing and saves a little. Its six frames are exactly the two spare slots
  left in the creature atlas's thirteenth row, so a creature's textures are the same 1,529,856
  bytes they were and a full colony of six the same 9,179,136. A still colony is drawn at the
  frame rate of whatever it is resting in, so resting at three frames a second is one redraw a
  second fewer than the four it used to be.

### Documentation

- The README is rewritten around what Formiga is, why it exists, its design priorities, what it
  does, its status (what is stable, what is still preview, and what has not been verified), how to
  build it, how it works, and how it is built with AI assistance under human direction. The
  feature tour it used to carry is now [docs/GUIDE.md](docs/GUIDE.md), and every earlier "New in"
  section is in [docs/RELEASE_NOTES.md](docs/RELEASE_NOTES.md), both moved word for word.
- ARCHITECTURE.md opens with an orientation for someone new to the code: the crates and where to
  start reading, how the app starts, the main loop, where state lives, how a feature usually
  touches each layer, and the design constraints and why they hold. CONTRIBUTING.md gains setup,
  a map of the code, the checks, and what to watch when changing persistence, generation, art,
  performance, or privacy. BUILD.md gains the full list of image commands and how a release is cut.
  The case study gains four sections on how the codebase has grown without breaking colonies.
- Out-of-date documentation is corrected. The architecture notes, case study, and test matrix
  described save versions 15 to 17 where the code is at 18, and the privacy notes had no entry for
  version 18. The performance notes said nothing had been measured since 0.57.0, the suggested
  repository description still said the colony tops out at four, and the architecture notes' lists
  of simulation modules and test files were out of date.
- `rust-toolchain.toml` pins rustfmt and Clippy along with Rust 1.97.1. The minimal profile it
  asked for has neither, so the documented checks failed on a fresh clone until they were added by
  hand. `formiga-tools`' usage text now lists `home-yard-sheet`, and `formiga-art` no longer names
  `time` as both a dependency and a dev-dependency.

## [0.59.2] - 2026-09-22

### Added

- Four kinds of house, each built from a shape of its own: a tent pitched from triangles of
  canvas with a triangle door, a pennant and guy ropes; a mushroom of circles; a pillow house of
  two plump cushions, one for the walls and a flatter one lying across the top for a roof, with
  tufted buttons and tassels; and a leaf house, the old paper house roofed in rows of overlapping
  leaves with a sprout on the ridge and a vine up the side. A village now mixes them: the colony
  house keeps the colony's own type, every cottage is built as the type its keeper would build,
  and the Home page can build any house as any type or give it back its own. Putting the village
  back as it grew returns every house to its own type, and the last change can be undone.
- A sleeper that has to make room is towed clear on a little rope. A nearby friend who is awake
  and doing nothing much walks over, takes up the rope, pulls the sleeper aside at an unhurried
  pace, lets go, and carries on; with nobody free, the sleeper wriggles over in its sleep instead.
  Nobody wakes, and the rope is drawn behind the pair so each end disappears into whoever holds it.
- `village-palette-sheet` and `home-yard-sheet` show every kind of house, and the yard sheet shows
  villages that mix them.

### Changed

- Companions stroll the village while the houses are out. Before, a resident walked home to its
  place and, in a colony of three or more, almost never left it: the ground between the trees
  was too short for the room each stroll kept clear, so 99.6% of strolls were called off. Now a
  resident strolls out from its own place to somewhere along the ground, looks about, and strolls
  back, at a little under half its walking pace with its walk slowed to match, and at most three
  are out at once. Residents stroll about a fifth of their time instead of under one percent, and
  a colony of six has one or two out most of the time.
- A companion on its way to bed walks there, heavy-lidded, and lies down when it arrives. It used
  to be drawn asleep for the whole walk, so a colony of four spent about 400 seconds an hour
  gliding across the floor in its sleep, some 16,000 points of it.

### Compatibility

- Save version 18 adds a house type chosen by hand for any house, absent until one is chosen, and
  migrates nothing. The four kinds keep the names colony files have always used for them, so every
  colony opens with the same houses it had, drawn anew.

### Performance

- Strolls cost drawing time. With the houses out, the finished build averaged 2.59% CPU over two
  minutes on the owner's colony against 0.98% for 0.59.0 on the same colony, with the same 112 MB
  footprint: something is nearly always moving now, and a moving village is drawn every tick where
  a still one was drawn a few times a second. Strolls at home tick and draw at 10 Hz rather than
  20, which is all a stroll needs and trims that by about a tenth. The simulation itself costs
  what it did, and the colony is written about four times a minute at home instead of three.
- A tow's rope is one two-texel texture, made the first time a rope is drawn, and one extra draw
  call while a tow is under way; a pillow house or a tent costs the same as any other house.

## [0.59.0] - 2026-09-21

### Added

- Classic parts bring the charm of the original one-piece companions into the modular recipe:
  candy colours with a lit top and a crescent of shade; a mask, a visor, beads, tall eyes, or square
  eyes; accent nubs or stick legs on forked feet; antennae or sprouts; stripes, spots, or patches
  across the coat; and an open curl or a star on the tail. Each part is chosen on its own, so a
  companion can sit anywhere between the tidy modular look and the older, scrappier one.
- New companions lean a quarter of the time wholly modular, a quarter wholly classic, and otherwise
  mix the two a part at a time: about 29% come out plainly modular, and colours, face, and limbs
  are each classic about half the time. Minis keep their parent's colouring, face, and build, and
  usually its crown, pattern, and tail. Image references reach classic parts too, always in the
  reference's own colours, and match as closely as before.
- Each companion can be given a leaning from its profile — wherever they like, homebody,
  floor-dweller, or climber — kept apart from its nature and what it has learned. It nudges rather
  than forbids: in a colony that spent 96% of its time out on the desktop up on ledges, homebodies
  spent about half as much and floor-dwellers about 11%. A floor-dweller or homebody on a ledge
  comes down to do what it chose next, and a homebody that sets out usually heads for home.
- The Journal page opens with "Today in your colony": who the day was about, what kinds of thing
  happened, the treasures first found today, and the latest few moments, all read from what the
  journal actually recorded. It never fills in time the app was not running, and says so when
  some of today's earlier moments have already rolled out of the journal.
- The Colony page shows how everyone gets along: every pair of companions once, grouped as keeping
  their distance, close friends, playmates, or still getting to know each other, with the same bond
  and play words the profiles use. A name opens that companion's profile.
- Visitors can be kept as favorites and invited again from the Journal page, without copying codes
  around. Up to eight are kept apart from the guest book, so a favorite stays after the book moves
  on; inviting one uses the same invitation a friend's code does and brings back the same visitor.
- Companions have little ways of their own. Each celebrates its own way from the day it arrives —
  a hop, a little dance, or a twirl, from its temperament and its own seed — and over its first
  hours and days picks up as many as two habits, each for a different kind of moment: looking a
  snack over before eating it, stretching or turning in circles before a nap, waving hello, or a
  play bow before a game. It does a habit most times the moment comes round, standing still for
  the second or so it takes, and the action then carries on exactly as it would have. Its profile
  lists them, and the journal records the day each one was picked up. Across three simulated
  colonies of five, every companion had a habit within about ten hours and 11 of the 15 a second
  within a day. Reduced motion hides the flourishes, as it hides every other pose.
- While the houses are out, a companion's menu offers Moment in place of Home. It opens a second
  strip beside the menu — nothing in the menu moves — with whatever the village could share right
  now: a picnic, a dance, or a nap on the ground between the houses. Everyone home answers for
  themselves with a bubble, and whoever wants to join lines up, faces the middle, and does it
  together; a dance ends with each dancer celebrating its own way. The same strip stops a moment
  under way. A moment keeps the houses out until it is over, ends if the colony is hidden or
  paused or when a companion is picked up and the houses go, carries on through a pet, and happens
  where everyone stands under reduced motion, which never offers a dance. One that runs its course
  goes in the journal and brings everyone in it a little closer.
- The Home page can put down a nap cushion, a picnic blanket, and a lookout on the ground between
  the houses, one of each, each slid along the ground to wherever you like; they never land on one
  another or off the ground. While the houses are out the colony sometimes goes to nap on the
  cushion, snack or sip on the blanket, or look out over the desktop from beside the spyglass, a
  sleepy, hungry, or curious companion more often, and otherwise carries on as before. An invited
  picnic or nap gathers around its spot, and the spots appear in the Home page's preview and in
  the colony portrait.
- Every house has been rebuilt to stand beside the creatures: its own materials with a roof, walls
  and trim that each have a shade and a light, lit from the upper left, a recessed doorway on a
  threshold, and a sign that somebody lives there. The leaf tent is three leaves leaned on tied
  twigs over a straw floor, with a planter; the mushroom hut a spotted domed cap on a stem with a
  round window and a stepping stone; the cushion den an unmistakable pillow fort under a gingham
  blanket, with a patch, a pillow inside and a cushion on the doorstep; and the paper house folded
  card with a taped roof, a cut-out window and a mat. Each companion's door is hung with a curtain
  in its own colours, which goes wherever its house does, and from seven in the evening until seven
  in the morning the houses glow from inside, the colony house's lamp too if it has earned one.
  Saved shelter styles, seeds and decorations are drawn anew rather than rerolled.
- The Home page arranges the village. The cottages can be stood in any order, each keeper's
  curtain going with its house and everyone walking home to where theirs now stands; the colony
  house always stays first. The village can be painted in one of six named palettes — Meadow,
  Blossom, Harbour, Autumn, Twilight, or Pebble — or keep its own colours, and a palette recolours
  every house and both trees without changing a single house's shape. A flower bed, a vegetable
  patch, and a herb box can be planted along the ground, each slid to wherever you like and kept
  clear of the hangout spots and of each other. One button, asked twice, puts the order, the
  colours, and the gardens back as the village grew, and leaves the hangout spots where they are.
  A companion replaced from the Colony page keeps the cottage where it stood. The preview, the
  desktop, and the colony portrait all show the arrangement.
- The Home page sends postcards. Pick a scene — everyone asleep on a patchwork quilt one golden
  afternoon, round a gingham blanket with a snack or a cup each, playing on the grass with a ball in
  the air and a kite overhead, or waving goodnight in front of the village at dusk with every house
  lit — add a caption of up to sixty characters if you like, and export a 960×600 picture with a
  stamp and a postmark. The whole colony is in it, posed from the same frames the desktop draws,
  in front of its own village as it is arranged. A postcard carries no names, and nothing about it
  is saved.
- The last change to the colony can be undone from the settings window's footer, on any page, for
  as long as the app runs. It covers a companion removed, replaced, started over, or welcomed from
  the studio, and every change to how the village is laid out: cottages, colours, gardens, spots,
  the home's corner and display, decorations, and keepsakes. A companion brought back returns
  exactly as it left, with the same identity, memories, bonds, minis, and cottage, standing where it
  or its replacement last stood, while everyone else keeps whatever they did in the meantime and
  anyone who arrived on their own since stays. Only the one most recent change is kept, and never
  in the colony file.
- `classic-sheet` shows each classic part alone on every body plan, and wholly classic companions
  standing, walking, greeting, hanging, sleeping, and gesturing. `habit-sheet` shows every habit
  and celebration on every body, drawn the way the desktop draws them. `village-palette-sheet`
  shows every style of house in its own colours and in each named palette. `postcard` draws one
  postcard of a sample colony in any scene with any caption, and `postcard-sheet` all four.

### Changed

- The colony's ground follows where the system's own bars really are on each display — the Dock
  on either side or hidden, a larger or smaller Dock, a notched display's taller menu bar, or a
  taskbar along any edge — instead of assuming a menu bar 24 points tall and a Dock or taskbar of
  factory size along the bottom. A bar that moves is followed within a couple of seconds, and
  anyone standing on ground that moved is carried to where it now is. With a Dock that hides, the
  colony stands on the floor of the screen and the Dock slides over it when it shows. Only the
  rectangle each system leaves for windows is read.

### Fixed

- A companion that walked up to a snack, a drink, a game on its own, a ledge to hang from, a find to
  hold up, or something to look over kept the walk's speed through the whole of it and glided
  across the desktop mid-snack: about 30 px a second, 150 to 280 px over a snack. It now comes to a
  stop in about an eighth of a second, as it always did when it simply stood about.
- The fifth and sixth companions of a full colony were left out of things the first four had: a
  window they rode on was never felt as a ride, a ledge they loved was never remembered, nobody
  looked for a playmate among them, and when a display went away they could be left pointing at a
  monitor that no longer existed. A full colony carried back from a removed display could also
  stack its last companion on its first. Everything remembered about each companion is now sized
  by the colony; a scene still gathers at most four.
- A companion from before recipes gave its new minis an unrelated modular recipe. Its minis are
  now drawn from its own genes the way the originals always were, and share through a version 1
  code that replays them exactly.

### Compatibility

- Every existing companion keeps its exact appearance: a real five-companion colony and every
  modular body plan, ear, clip, and frame render pixel-identical to v0.58.9. Modular recipes are
  generated byte for byte as before, and their share codes and imported lineages are unchanged.
- A recipe with classic parts is shared as a version 3 code, a version 2 code whose four reserved
  bytes hold the parts; it needs v0.59.0 or newer to import.
- Save version 17 adds classic parts to stored recipes and migrates nothing. It moved so an older
  build refuses the colony rather than quietly dropping the parts.
- Hangout spots are kept with the village in the colony file, as a kind and a place along the
  ground, and are absent until one is put down. The cottage order, the chosen palette, and the
  garden patches are kept the same way, each absent until it is chosen; an order put back to the
  order everyone arrived in is not written down.
- A companion's habits are kept with it in the colony file, absent until it picks one up, and each
  is recorded in the journal. A share code carries none: a companion shared into another colony
  celebrates the same way, since that comes from its seed, and picks up habits of its own there.

### Performance

- A busy colony is no longer rewritten to disk every time a creature starts an action. Arrivals,
  the houses, rituals, and new belongings are still saved at once; everyday movement is gathered
  into a checkpoint at most every fifteen seconds, so a full colony on a busy desktop writes its
  file about 4 times a minute instead of about 85, saving roughly 390 ms of main-thread time and
  12 MB of disk writes a minute. At most fifteen seconds of everyday movement can be lost to a
  crash.
- `tick-bench` measures a full colony of six and a guest touring the village, holds each
  scenario's colony at its named size, keeps homebound scenarios at home for the whole run, and
  reports how often the colony asks to be saved and how often it is written, with the cost of a
  save. A full colony on a busy desktop costs 6.5 µs a tick, against v0.58.9's 6.7 in the same
  sitting.
- On the desktop it was written on, with the owner's own colony, the finished build averaged 0.81%
  CPU over three minutes against 0.86% for v0.58.9 freshly launched on the same colony, with the
  same 111 MB footprint. A companion that stops to eat or play no longer glides, so a lone creature
  is in motion 19% of the time instead of 30%, and the app ticks at its fastest only while something
  moves.
- A recipe grows from 16 to 22 bytes in memory. Classic parts are drawn into the existing body and
  face frames when the atlas is built, so they add no frame, quad, texture, or per-frame work.
- The village texture grows from 128×128 to 256×256 so every house has a cell of its own, by day
  and after dark: 196,608 bytes more on the display the village is on. The settings window holds
  only the daylit half, 64 KiB more than before.
- The stretch before a nap is four new frames in each companion's atlas, in slots the atlas
  already had spare, so a creature's textures stay 1,529,856 bytes. Every other habit and
  celebration reuses frames that were already baked.
- A postcard, like the colony portrait, is drawn only once its save dialog has a destination and
  is dropped as soon as it is written; choosing a scene or typing a caption draws nothing.
- The colony's object sheet grows from eight 16×16 cells to fourteen for the three hangout spots
  and three garden patches, 6 KiB, and the settings window's artwork budget from 432 KiB to
  500 KiB with the Home page's larger village. A palette or a new cottage order redraws the
  village texture once; nothing about either is done per frame.

## [0.58.9] - 2026-09-21

### Fixed

- A colony that has filled up left companions standing on one another's faces. The table that
  watches which pairs are drawn through each other had one slot for every pair a colony of four
  can make; six companions make fifteen pairs, and a table too small to hold them all does not
  merely lose the odd pair — it hands back whichever record it landed on, so one pair's time was
  credited to another and another's cooldown was read in its place, and nobody was asked to step
  aside from an overlap that was really happening. Across sixteen seeded sessions of a full
  colony the worst episode falls from 22.2 seconds to 8.2, and the number of sessions with any
  episode longer than a moment and a walk falls from twelve to three. A colony of four is
  unaffected, measuring exactly as it did before.
- A companion asked to step aside was treated as having already moved. The pair was then left
  alone for five seconds whether or not anything came of it, so an overlap that the step did not
  clear simply sat there. The pair is now given only as long as the move itself takes, and asked
  again if they are still drawn through each other once the mover has arrived. A sleeper part way
  through shuffling over is left to finish, which it was not before.
- A companion walking over to a friend aimed at a spot measured against that friend and nobody
  else, so on a crowded floor it walked into a third companion and stood there — both of them
  exactly where their own errands had sent them, and so neither one able to be asked to move. On
  open ground the spot now slides clear of whoever is already standing there.

## [0.58.8] - 2026-09-20

### Fixed

- Setting a region for your companions on the desktop left the application unusable. The open
  editor claimed every event the overlay was sent, including its redraw, so the colony froze on
  its last frame and not one of the regions being dragged out was ever drawn; and because the
  editor's overlays take the mouse across the whole display, the first press on the desktop
  ordered one of them in front of the settings window, putting Apply and Cancel behind a
  full-screen window that swallows every click. Nothing but the tray menu answered after that, and
  only restarting cleared it. The editor now takes the pointer and leaves the overlay everything
  else, and the settings window is kept above the overlays for as long as the edit lasts.
- Using the desktop editor cost you every creature menu for the rest of the session. The windows
  that carry each companion's clickable silhouette were hidden behind their own bookkeeping while
  the editor was open, so nothing ordered them back in afterwards: right-clicking a companion,
  petting one and picking one up all stayed dead until Formiga was restarted.

### Changed

- The village stands on top of the Dock rather than behind it. The ground the colony is founded on
  was forty points above the bottom of the display, inside a Dock at its factory size; it now
  clears the strip the system keeps for itself, on Windows as well as macOS.

## [0.58.7] - 2026-09-20

### Added

- A colony can grow to six companions, and every full-size one has a house of its own — so the
  village can run to six houses where it used to stop at four. A mini has no house: it lives in
  its big version's, comes home to that door, and keeps it if it ever grows full-size. Each
  full-size companion can also raise one more little one than before.
- The ground in front of the houses belongs to the whole colony, and they walk it. A companion
  home for the afternoon goes somewhere, stays a while, and then wanders somewhere else, anywhere
  between the two trees — it keeps its distance from whoever is already standing there and prefers
  not to plant itself square in a doorway. The first place it goes when the houses appear is still
  its own door. A hidden colony and one with reduced motion stay where they are, as before.
- Anything held out to a companion is taken where it stands. A snack or a toy offered to somebody
  mid-stroll stops it there rather than being turned down for bad timing.

### Changed

- The village is tighter again. The standing places that used to be parcelled out between the
  houses are gone — the houses now stand a seam apart, and the ground in front of them is shared —
  so six houses take less room than four did with their doorsteps: 423 shelter pixels against 445,
  and a colony that has not filled up takes proportionally less. A founder on its own is 178.
- The save format moves to version 16. A colony of up to six needs more room for who-knows-whom,
  and an older build would quietly drop the sixth companion rather than admit it could not read
  the file; every colony from version 1 onward still opens here, with nothing to do.

### Fixed

- Climbing onto a window ended with the creature sliding diagonally along the ledge, which read as
  the body clipping across it. Topping out is two beats now: it hauls itself straight up where its
  hands are, and then steps in onto the ledge.

## [0.58.5] - 2026-09-20

### Added

- A tree stands at each end of the village, with the houses gathered between them, so the corner
  reads as one small place rather than a row of buildings. Every keepsake the colony has found
  hangs in the branches on a cord of its own, in the same spot every time: the eight everyday
  finds in the tree beside the colony house, and the eight that only ever turn up in a particular
  circumstance in the tree at the far end. A keepsake never moves once it has turned up, and the
  trees fill in exactly as the scrapbook does.
- The colony's belongings have left the lane between the houses and are scattered around the two
  trunks instead — four in each yard, some tucked in behind a trunk, some sitting out in front of
  the roots, and arranged a little differently for every colony. Which yard a belonging keeps to
  never changes, so nothing shuffles along when a new one turns up. The colony portrait draws both
  trees and both yards, so a picture of the village is the village.
- A visitor walks the village instead of standing at one end of it. After it has walked over and
  said hello it goes from place to place — between the houses where there is room, out to a tree's
  yard where there is not — stopping a few times on the way round, and at each stop going over to
  whichever companion it has not met yet. It looks up at a house it is standing beside, stoops to
  a belonging, or simply rests a moment, and it makes its way back to where it came in before it
  waves goodbye. Over a visit it gets to everybody rather than keeping one creature company, and
  nobody leaves their own doorstep to answer: they turn, they hold the look for as long as their
  temperament asks, and they go back to their afternoon.

### Changed

- The village takes much less of your desktop. A full colony of four with all eight belongings ran
  to 523 shelter pixels in 0.58.0; the same colony measures 445 now, with a tree at each end as
  well — 15% narrower while gaining two trees. The seam between one lot and the next is three
  pixels rather than five, a doorstep is narrower, and the belongings cost the strip nothing at
  all now that they live in the trees' yards. The houses themselves are exactly as big as they
  were.
- The save format is unchanged. It is still version 15, there is no migration, and a colony
  coming from 0.58.0 keeps everything it had and needs nothing done to it: the trees are drawn
  from the colony's own seed and the scrapbook it already keeps, and 0.58.5 adds no saved field.
- The simulation costs less per tick, for exactly the same behaviour. A tick on a busy desktop
  with the cursor moving is about a quarter cheaper than it was, and a tick with the colony at
  home about three fifths. Each window's neighbours are walked once to answer all
  three questions about the desktop's shape instead of once per question, the windows really on
  screen are gathered once and sorted front to back so the search for whatever covers one stops as
  soon as it can, the most expensive question an
  action asks — whether there is a ledge it could reach — is asked where the action is chosen
  instead of on every tick of one already under way, and the picture of the colony each creature
  answers against reuses its storage rather than being built again every tick. That the behaviour
  is unchanged was proved rather than assumed: six seeds across thirteen scenarios, 40,000 ticks
  each, driven through both builds and compared on the save, the events, the bubbles, every
  creature's pose and every interaction flag — seventy-eight identical results.
- Three separate answers to what lies below a point, three ways of settling the colony down, two
  ways of replacing a creature and two ways of welcoming one became one of each. Nothing about it
  is visible from the desk.
- The roadmap file is gone, and the performance document no longer carries forty-seven rows that
  never held a measurement. The changelog records every release, the architecture document
  describes what exists, and the performance document states plainly what has not been measured.
- Continuous integration builds and tests on `main` and on pull requests rather than on every
  branch push. Tagged releases are packaged exactly as before.

### Fixed

- A window narrower than 24 points could crash the simulation. Working out where a falling
  creature would land asks each window for the standable part of its top edge, which for a window
  that narrow is nothing at all, and the search then tried to hold a position between a minimum
  above its own maximum. A surface too narrow to stand on is no landing now, rather than an
  impossible one.
- **Gather Creatures** stood everyone down on the floor with room around them, then left whatever
  they had stopped to watch running over the top of them. Gathering now settles every plan the
  colony is part-way through, the same way going home already did. A quiet spell settles the same
  way too: it had been leaving a throw still in the air, and a scene creatures had gathered to
  watch still running under it.
- Regenerating a colony stopped at the first unkept adult it had no seed for and left everybody
  after it exactly as they were — including unkept minis, which owe nothing to a seed and should
  simply have gone. Running out of seeds now costs only the adults that had none left.
- A creature adopted over another one quietly moved to the back of the group. It was removed and
  added on the end rather than put into the place it was taking over, so it lost that creature's
  spot in the colony list and in the order the overlay draws. Adopting and replacing with a design
  both hand over the place in the colony now, and two creatures arriving in the same moment take
  their turn instead of appearing on top of one another.
- Living at the houses had begun to make friends of neighbours. The tighter village seats every
  resident a step from the one next door, which put every pair inside the distance that reads as
  two creatures choosing to be near each other, and a pair would have gained a calm-company moment
  for every five minutes the colony spent at home — enough to swamp every other way a friendship
  forms. Living next door is not the same as choosing somebody's company, so proximity no longer
  counts while the home is out. The village says where a creature lives, not who it likes. A pair
  keeps the calm minutes it had already gathered rather than losing them to an afternoon indoors.
- Keepsakes in the scrapbook were drawn squashed flat. The page cuts one square out of a sheet of
  sixteen laid side by side, and the drawing was being measured against the whole sheet rather
  than the square being shown, so it was letterboxed into a sliver an eighth of its proper height.
  Every keepsake is square again, on the page and in the village.
- Companions near one another would shiver on the spot, flicking left and right several times a
  second. Three separate things could cause it, and all three look the same from the desk. A walk
  covers one to three points in a tick and had nothing telling it to stop, so a creature that
  reached the spot it was walking to stepped over it, turned, stepped over it coming back, and
  carried on like that for the rest of the walk — which is why it happened beside a companion,
  since a spot beside a friend is a spot a creature actually arrives at. A walk now stops when it
  arrives, and the creature turns to face the companion it walked over for. A game worked out
  where its players should be going afresh every tick and would send a runner with no room ahead
  back past whoever was chasing it, then away again the tick after; a player now keeps the way it
  is going until it gets there, while a chaser still follows a lead that keeps running. And the
  rule that keeps faces from being hidden would pull a creature away from a spot its own walk was
  still carrying it toward, the two moving it a pixel at a time in opposite directions; a creature
  on its way somewhere is left to arrive, and asked to move once it has.

## [0.58.0] - 2026-09-18

### Added

- Right-click a creature and a small strip floats above its head: a snack, a toy, send it home,
  and its profile. Control-click does the same on macOS. Clicking an icon acts; the border and the
  gaps between icons do nothing, and everything around the strip stays click-through, so the
  desktop underneath is exactly as usable as it was. The menu closes when you choose something,
  when you right-click the same creature again, when the cursor wanders off for a moment, or after
  eight seconds of being left alone.
- A creature decides for itself what to do with a snack or a toy. A hungry one takes the food; a
  bored, playful one takes the toy; a tired one shows you it is sleepy and nothing else happens.
  Being offered the same thing over and over does not wear a creature down — it has just eaten, or
  just played, and says so. A timid creature thinks about it visibly before answering. Accepting
  something nudges how much a creature trusts your hand, the same small, reversible amount a pet
  does.
- Send home gathers the whole colony at the houses straight away, for the usual fifteen minutes,
  without waiting out the cooldown first. Everyone answers with a small house over its head.
- Creatures answer what you do with a picture over the head rather than a word: a heart for a pet,
  surprise at being picked up, a dizzy swirl after a toss lands, a house for going home, and the snack,
  toy, question mark, or gentle "no thanks" that goes with an offer. A bubble grows in, holds for a
  couple of seconds, and shrinks away; asking for the same thing again holds the one that is there
  instead of popping a second. At most five are ever up at once, none while the colony is hidden,
  and reduced motion skips the growing.
- Creatures from elsewhere come to visit. At about one home gathering in four, somebody nobody
  knows wanders in along the floor, says hello, spends the gathering pottering about with your
  colony, waves goodbye, and walks off before the houses close — never three gatherings in a row,
  never more than seven apart, and never while somebody else is already here. Your creatures turn
  and answer the hello in their own time, then carry on with their own afternoon; nothing about a
  visit changes a bond, a memory, or a habit.
- You can also invite a friend's creature for the day. Paste their seed code under
  **Settings → Creature studio → Adopt a shared companion from a code** and the preview now offers
  **Invite for a day** beside adopting: they turn up at the houses promptly and come back at every
  gathering for twenty-four hours, and then go home. The Journal page keeps a guest book of the last
  twenty-four visitors with the date each one came and a code to copy, and whoever is here right now
  sits on top of it with **Ask to stay** beside their code. A guest can be petted and offered snacks
  and toys, but not picked up — a drag on a visitor is only ever a friendly rub.
- The colony corner is a proper village now. Companion cottages and minis' cottages are visibly
  bigger, and every creature rests beside its own front door instead of lining up in front of the
  houses. While the colony is home they get on with small things: a nibble, a drink, a game with a
  toy, a look up at their own house, a nap in the sun, a wave that the neighbour waves back at, and
  a short errand out to a belonging and back. None of it counts for anything — it is what a quiet
  afternoon at home looks like.
- Creatures no longer stand on top of one another. A face covered by somebody else's body is
  cleared within a moment or two, either by whoever is on top taking a short step aside or, for a
  sleeping or ceremonial creature, by a quiet shuffle that does not wake anybody. Resting
  arrangements are still shoulder to shoulder, because that is how creatures rest; it is faces
  being hidden that gets fixed. Greetings, games, line-ups, and races all start from spacing that
  already works, and **Gather Creatures** now sets everyone down with room around them rather than
  in one stack.
- There are sixteen kinds of trinket to find rather than eight. Eight turn up on any ordinary day,
  and the other eight only ever turn up in a particular circumstance: after dark, high up on a ledge
  with real air beneath it, in the middle of a window ride, or standing beside a close friend. The
  scrapbook shows all sixteen slots from the start, with the ones nobody has found yet as a dim
  silhouette and a hint about where to look.
- Toys, snacks, and drinks are redrawn. A top, a rattle, and a pinwheel read at a glance, a mug is
  not a bowl is not a bottle, and whatever a creature is holding sits in the paw or at the mouth
  that is actually drawn, with a little trail as it moves.
- A creature that is genuinely interested in something now looks it: drawn up tall, head leaned
  toward whatever it is watching, ears pricked, forelimbs gathered in. It is the pose you will see
  when a window moves nearby and a creature has stopped to watch what happens next.
- Any creature can be exported as an animated sticker from its Colony profile — a walk, a wave, a
  cheer, playing, a snack, a nap, or a dance, at four or eight times size, as an ordinary GIF that
  loops. The timing is the creature's own, so a sticker moves the way that creature moves.
- The Home page can export a colony portrait: a 960×600 picture of every member in a friendly pose
  with its name, the month the colony began, how many of you there are, how many family lines, and
  your village behind them.
- Formiga now ships a small distribution kit alongside the app: winget manifest templates and a
  script that fills them in for a published release, an itch.io page kit, and suggested repository
  metadata.

### Changed

- Save version 15 adds only the visitor state: whoever is visiting, how many home gatherings the
  colony has held, and the guest book. Every v1–v14 colony migrates as before, and a v14 colony
  simply arrives with no visitors and an empty book; nothing else is touched, added, or invented.
- Each creature's artwork grows from 118 to 124 frames for the new watching pose: 1,529,856 bytes
  per creature, about 5.84 MiB for a full colony of four. The per-creature limit the tests enforce
  is now 4,500,000 bytes rather than 1.5 MB — a deliberate raise, so there is room for more poses
  without the budget having to move again every time one is added.
- The settings artwork budget is 432 KiB. The scrapbook's eight separate trinket drawings became
  one sheet carrying all sixteen trinkets and their glints: one texture instead of eight, and the
  only thing on any page that grew.
- The simulation's largest source file is now a directory of themed modules, with its tests beside
  them. Behaviour is unchanged — this matters to anyone reading the code and to nobody else.

### Fixed

- A find could be written into the scrapbook as the gem rather than the trinket that was actually
  held up. Since the scrapbook shipped in 0.57.0, a presentation that ended by the ordinary route
  back to everyday behaviour had already had its trinket cleared by the time the find was written
  down, so it went into the book as the first kind; finds that ended another way were recorded
  correctly. Finds are now recorded as what they were. Earlier entries are left as they are, since
  what was really found cannot be reconstructed.
- Two bonded creatures could greet each other from almost on top of one another. The walk that
  brought them together stopped a fixed short distance from the companion rather than from the
  spot it was aiming at, so the greeting could end up nose to nose with one face behind the other.
- A cottage and a belonging could land on the same spot in the village. Houses were placed against
  however many belongings the colony happened to own while belongings were placed as though there
  were always eight, so the two walks disagreed. One walk now lays out the whole strip.
- A creature reacting to a window looked at its own feet. The gaze aimed at the nearest point of
  the window, which for a creature standing beside one is the ground it is standing on; it now
  looks at the near edge halfway up, and a creature standing on the window looks at its middle and
  turns toward it.
- Five poses were authored with a lean that never appeared, because the older renderer did not draw
  it. Leaning is now drawn on every body, and a creature looking at a screen leans in slightly too.

## [0.57.1] - 2026-09-17

### Added

- Creatures now have bodies to match the things they do. Watchers cover their eyes at a risky
  moment, gasp at a catch, wring their paws while it is going on, and throw their arms up when it
  goes well. A jumper squares up at the edge, wobbles once it has been hauled over, and celebrates
  a hard landing. Someone reaching down for a hanging companion visibly hauls, then cheers when
  they are up. Along a ledge, a creature teeters at the brink and frets while it sizes up a gap.
- The games got their own shapes too: a tug of war heaves, keep-away leaves the empty-handed
  reaching, hide and seek has the seeker cover its eyes while it counts and the hider crouch out of
  sight until it is found, the floor-is-lava balances, leapfrog crouches under the vault, creeping
  around a sleeper stays low and the prank pays off with a dance, the field cheers a race winner
  in, whoever holds the middle of a ledge celebrates while the others shove, a staring contest's
  loser hides its face while the winners cheer, a copied flourish passes down the chain, and being
  tagged is a genuine surprise. A creature near the cursor reaches for it, and one whose window
  vanishes out from under it gasps before looking around.
- These poses are never saved, never appear with reduced motion, and never replace an action that
  is already using the body, such as walking, hanging, sleeping, eating, or showing off a find.

### Changed

- Each creature's artwork grows from 90 to 118 frames for the new poses: 1,437,696 bytes per
  creature under a 1.5 MB limit, about 5.48 MiB for a full colony of four. A pose is drawn at its
  own frame rate rather than that of the action underneath it.

### Fixed

- Waving, reaching, dangling, climbing, and presenting no longer grow a second arm beside the paw
  or wing a creature already has. The limb you can see is the one that extends: a nub stretches
  out, a winged creature raises its own wings, and a long-bodied one lifts a front paw off the
  ground.
- A tug of war never actually pulled. The settling-in walk stopped a hand's width short of a shared
  grip, because what was left was shorter than a stride either creature would take, and the tussle
  waited there for a step neither could make. It now starts from where they stand and hauls in
  steps the walk will take.
- A creature invited to try a gap no longer keeps its pose when the edge it was invited from
  disappears.

## [0.57.0] - 2026-09-17

### Added

- Creatures peek over ledges, judge height and gap difficulty, take a running start, and celebrate
  difficult landings. A borderline gap brings visible hesitation: a look down, a step back, and up
  to two reconsiderations before a jump from the edge or a retreat. Recent slips and retreats make
  a creature briefly more cautious and stop it retrying the same gap at once. Marginal jumps can
  catch an edge, struggle up, attract a helper, or end in a rare shared tumble. Spectators
  anticipate, gasp at a catch or fall, avert their eyes, and react to the actual outcome.
- Window rides now include balance loss, panic grips, vertical glances, adventurous riders,
  nearby dismounts, boundary retreats, and brief dizziness. Close ledges permit crossings.
  Windows closing in prompt a retreat along the ledge, or a hop down to exposed support when both
  sides close; abrupt movement can briefly launch a bold rider.
  Existing staircase routes can repair one changed step, with clearer squeeze entry/exit poses.
- Frequently occupied surfaces become temporary hangouts. Bold creatures can make short dangling
  commutes, with continuous hand contact and a pull-up at the end.
- Ordinary encounters can become small games. Following turns into a chase, a procession, or a
  route to copy; a solo flourish gathers a dance circle or a tug over a plaything; sustained mutual
  gazes become staring contests; playful company vaults over one another, plays keep-away, passes
  "it" along, takes turns at a gap, or contests the middle of a ledge. Resting company piles up
  beside a companion. A creature can be invited to try a gap another just cleared, and answer with
  an attempt, a nervous approach, or a refusal. Every invitation can be declined, and a refusal is
  respected for a while.
- Company creeps around a sleeping companion, and a playful one may pester a rested friend awake in
  capped attempts. A creature that still needs its rest sleeps through it.
- Three games about the desktop's own shape. Two companions race across the windows to a ledge
  they can both reach, each by the route its own nerve allows, and the race replans or is called
  off if the finish line closes. Standing at the end of a high ledge can start a round of
  the-floor-is-lava, with visible reluctance near the edge, crossings that read as saves, and a
  round that ends the moment somebody touches the floor — safety always permits the landing. And
  one companion hides around the far end of a surface while the other counts and then searches,
  using only where it last actually saw them.
- The journal groups moments by Today, Yesterday, and date in your own local time, filters to one
  companion, and keeps up to eight moments pinned above the rolling sixty-four. A pin names an
  existing moment, so it can never say something the journal did not.
- A scrapbook records the first time each of the eight kinds of trinket was found, with a drawing,
  a short description, the date, and who found it — still named even if that companion has since
  left. Colonies from before this release simply start theirs empty.
- The home page now previews the corner as it really is: the colony house with the decorations it
  has earned, a cottage for each companion, and the belongings along the same ground line, placed
  by the same functions the desktop places them with. Looking at it never calls the colony home.
- A charcoal-and-sage dark appearance alongside the cream one, an option to follow the system, and
  modest text scaling up to 150%. Keyboard focus is drawn in the accent colour in either theme.
- An optional soft outline behind creatures, for bright or busy wallpaper. It is baked into the
  existing artwork, reads nothing from the desktop, and never changes where you can click.
- An opt-in weekly routine can move between your saved Work and Relax presets at times you choose,
  up to fourteen changes a week. It shows what is in force and what is next, yields to a manual
  choice until the next change, and applies only the routine intended right now — a machine that
  slept through a week of changes wakes into today's, not through every one it missed. Showing the
  colony, pausing, and a quiet moment stay yours.
- Two riders on one moving window watch each other and show off when the ride is steady, and a
  playful creature with an eye for the cursor races a fast pass in short dashes, never leaving its
  display or touching input.
- Games carry at most one temporary plaything, held in someone's hands and gone when the game ends.
  Every game has explicit joining, leaving, timeouts, bounded turns, and a cooldown before the next
  one, and none of them are written to the journal.
- Curious creatures can explore a newly connected display through a short, continuous habitat
  route. Display loss recovers displaced creatures immediately, with space between them, followed
  by reorientation. Stable display keys distinguish removal from identifier or DPI changes.
- Fast cursor passes draw brief glances; repeated local movement can invite investigation or
  cautious withdrawal. Personality and learned trust modify the response. Cursor warps, disabled
  reactions, missing input, and interrupted observations clear the invitation.
- Nearby creatures notice new windows, approach safe edges, react to sudden growth according to
  temperament, and briefly search after a recently used surface disappears. Several local window
  moves can attract curiosity; exposed overlapping tiers encourage climbing, while empty desktop
  space encourages roaming when no ledge is reachable.
- Watchers now react in their own time rather than together: each one notices, gasps briefly at a
  catch or a fall, shows concern, and settles. Timid companions cover their eyes through the
  frightening part, playful ones celebrate a real success, and a companion facing the other way is
  slower to look up. Window, cursor, and display reactions all read through the same stages.
- Creatures keep their own walking, resting, greeting, and recovery cadence, so the same creature
  always moves like itself without any change to how it looks.
- Staircases are chosen to suit the creature climbing them: how far it will rise or drop depends on
  temperament, liveliness, learned climbing, and tiredness, and a route that doubles back is passed
  over for one that keeps going. Each step is looked at before it is taken.
- Companions watch an investigator or startled rider with staggered gaze, concern, and recovery.
  Short approaches reserve space around companions and existing landings, stop when blocked, and
  respect habitat restrictions and reduced motion. Window observations and reactions remain transient.
- A native cream-and-forest colony interface inspired by the creature cards: portrait profiles,
  life-history tiles, a short optional introduction, journal, home editor, and dedicated studio.
- Four-candidate comparisons with optional six-fps movement/expression previews and color/body
  locks for the next random set. Reference images and shared codes have explicit previews.
- Shared-creature adoption into existing colonies or a confirmed, unkept replacement. Original
  appearance and personality survive exactly; unrelated creatures and their histories remain intact.
- A local journal capped at 64 typed moments: arrivals, discoveries, learned preferences, new close
  friendships, completed rituals, keepsakes, and decorations. Repeated moments are throttled.
- Home corner/display selection, earned-decoration visibility, and ordered keepsake slots using
  existing village placement and behavior influences.
- Temporary 15/30/60-minute quiet moments, early cancellation, and two saved Work/Relax behavior
  routines. Quiet expiry uses the normal world tick and survives relaunch.
- Full-colony JSON export/restore, bounded input validation, and preserved recovery copies.

### Changed

- An audience now watches what a game is actually about, and keeps up when it changes hands: the
  companion holding the toy, the one who is "it", the one standing on the contested spot. Watchers
  stay for as long as the game lasts rather than being released partway through a long one, and a
  companion leaping a gap during a game reads as the risk it is, so a timid watcher may look away.
  In hide and seek the audience follows the seeker, never the hider.
- Turning off "Explore application-window ledges" now keeps creatures on the floor. The preference
  was already respected everywhere else and not in the ordinary decision to go and perch.
- A kept journal moment stays kept. Once the rolling sixty-four moved past it, a pinned moment
  became invisible while still occupying one of the eight slots, with no way to unpin it.
- A desktop with more than sixty-four windows no longer switches every behavior off. The frontmost
  sixty-four are watched, chosen the same way the route planner chooses them, and windows past the
  end are never mistaken for windows that closed.
- A window dragged straight upward no longer sends its rider on a sideways scramble that could not
  have helped, or stretch the ride past its own ending.
- The settings window: "About & backups" could not be reached at the smallest window size with the
  largest text, checkboxes stayed small beside scaled-up labels, and the dark theme's navigation
  rail was brighter than the page it sat on, with inactive tabs standing out more than the
  selected one.
- Formiga uses about a twelfth of the CPU it did and about 40% less memory. Creatures' click
  targets no longer chase them across the screen while the cursor is elsewhere, the desktop is
  scanned quickly only while a window could be moving under a creature, which application owns
  each window is remembered rather than asked again every scan, and on a Retina display at an even
  creature size the overlay draws at half resolution without losing a single pixel of detail.
- Save version 14 migrates v1–v13 without replacing creature identities, bonds, recipes, learned
  history, or preferences. Pins, the scrapbook, appearance preferences, and weekly routines all
  start empty on an older colony; none of them is invented from an existing discovery count.
  Existing colonies skip onboarding; new colonies receive the introduction.
- The settings artwork budget is 416 KiB. The home preview now draws the whole village from the
  same 128px village atlas the desktop samples, and the scrapbook adds one 16px drawing per
  trinket variant; neither grows with the size of the colony.
- Anything a creature is carrying now rides at one fixed point in front of its face, mirrored with
  the way it is facing and kept inside its own frame, so a toy is clipped and covered along with
  whoever holds it and a hand-off reads as one creature passing something to another rather than
  two objects swapping places in mid-air.
- The tray groups secondary preferences and diagnostics, with quick access to a 30-minute quiet
  moment. Habitat presets use readable labels and show a monitor map; exact coordinates are advanced.
- Copy/save/apply/export actions provide feedback. Preferences show unapplied changes and are
  acknowledged after successful saving. Milestone bubbles carry a small growth sprout.

### Fixed

- Failed colony loading no longer overwrites either original file with a new colony. A temporary
  session offers restore or an explicit fresh start after preserving recovery copies. A valid backup
  is retained when repairing a corrupt primary, and a missing primary can recover from its backup.
- Settings texture deltas are explicitly consumed, including recoverable GPU-surface failures.
- A staring contest is decided by who is steadier rather than by chance, ends early if something
  nearby distracts one of them, and watchers look between both contestants before settling on
  whoever broke first.
- Dragging one edge of a window is treated as a resize rather than a move, so it no longer startles
  riders as though the whole window lurched. A window whose native identifier changes while its
  frame stays put keeps the creatures standing on it instead of dropping them.

### Resource budget

- No new dependency, runtime, background worker, polling loop, desktop atlas, or overlay draw call.
- UI artwork is cached on demand, bounded by a 416 KiB texture budget (excluding the existing egui
  font/window resources), and released on close. Studio playback is opt-in and respects reduced
  motion; hidden/occluded settings windows do not schedule preview redraws.
- The journal stores only bounded typed colony moments, never desktop observations or window history.

## [0.55.6] - 2026-09-12

### Changed

- Winged creatures grow actual wings rather than large accent nubs. Each membrane keeps a lit
  leading edge and a shaded underside, and carries a tip up and out past the shoulder so a wing
  is never read as another arm. One of three structures is drawn over that: feathered quills,
  ribs reaching a drawn-down tip, or a pale panel behind a darker outer rim.
- The wing a creature grows comes from bytes already in its recipe, so wings vary between
  creatures, stay the same every time one is drawn, and travel intact inside a shared code. The
  recipe is still 16 bytes and existing creatures keep their exact appearance.

## [0.55.5] - 2026-09-12

### Added

- The blob returns as a body plan of its own, alongside round, upright, four-pawed, and winged. It
  is one soft mass that carries its face directly, with no separate head, a rounder minimum
  footprint, and stubby feet, and it composes with the same ears, tails, markings, and colors as
  every other plan. Compact, blobby reference images can reach it too.
- Every colony member after the first gets a house of its own beside the colony house: a full-size
  cottage for an adult and a matching half-size one for a mini, in the house's own style and
  palette. The corner grows into a small village as the colony does.

### Changed

- Loose objects have left their keepsake cubbies. Houses and belongings are laid out by one walk
  along a shared ground line, so a belonging rests on the ground between the houses and can never
  land on one.
- Shelter decorations attach to the silhouette they belong to. Banners hang from the actual
  roofline, lamps mount on the wall, ornaments sit on the real peak, and stones and flowers rest on
  the ground. Only the two ground pieces still drift, and only by a pixel.
- Toys, snacks, drinkware, and found trinkets are coloured against the creature carrying them
  rather than from its coat, so a belonging reads as a separate object instead of another marking.
- The README covers what someone actually needs to know to use Formiga, and the project no longer
  describes itself as a portfolio preview. The `portfolio-hero` and `portfolio-demo` tools are now
  `hero-image` and `demo-animation`, and the release workflow is named `release`.
- The shelter sheet shows each style plain and fully decorated; the home-yard sheet shows a full
  village at both corners.

### Compatibility

- Existing creatures keep their stored design and appearance. The blob occupies the last body index,
  so earlier recipes and version 2 codes decode exactly as before; a code carrying a blob needs
  v0.55.5 or newer. Save version 12 is unchanged.

### Performance

- Dwellings share one 128×128 shelter atlas in place of the previous 64×64 texture, so a full
  village is at most four quads against a single texture, sampler, and bind group. Belonging colors
  are derived once per atlas build. Frame budgets, sprite dimensions, and the four-creature cap are
  unchanged.

## [0.55.0] - 2026-09-12

### Added

- A bounded modular creature framework: four rounded body plans combine with six ear styles,
  five tail choices, safe head/body/leg proportions, muzzle patches, seven marking treatments,
  and custom coat/accent colors. Big expressive eyes and a mouth remain part of every design.
- Image-guided construction uses temporary dominant/accent color bins and silhouette cues to
  adapt 512 safe candidates, preserving source aspect ratio and transparent backgrounds. This is
  a cute reinterpretation, not object recognition; no model, network request, or new dependency.
- Version 2 shared creature codes carry the exact compact design. Version 1 codes and legacy
  creatures retain their original appearance; save version 12 accepts every previous save version.
- Reproducible generation and house-yard preview sheets, plus compatibility and layout checks.

### Changed

- House objects occupy at most eight fixed spots in a compact two-row yard beside the house,
  following its corner, display, and scale. Unavailable spots stay hidden rather than scattering;
  the yard appears only with the house and uses its existing object atlas and draw budget.
- Objects sit in small stackable keepsake cubbies baked into that atlas; decorations now anchor
  to each shelter's actual roof and walls rather than floating at fixed canvas heights.
- Stones and flowers sit closer to the shelter, and the lamp is mounted beside the wall.
- New minis inherit their parent's design family and colors with bounded variation. Preview
  acceptance, saves, cards, and shared codes preserve custom designs and palettes.
- README, privacy, architecture, and performance documentation describe the current version.

### Performance

- Design recipes occupy 16 bytes each in memory; no source image survives generation. Existing
  48×48 body frames, 16×16 face frames, animation caches, and the four-creature cap are unchanged.

## [0.51.6] - 2026-09-05

### Fixed

- When the house appears, creatures immediately walk toward their resting spots from their current
  positions. Creatures on ledges descend first, and everyone uses the resting pose only on arrival.
  Walking respects pause and pet reactions, resumes after relaunch, and stops when the house is
  dismissed. The existing house timing, colony spacing, and save version 11 remain unchanged.

## [0.51.5] - 2026-09-05

### Changed

- Creature-to-creature greetings, play, following, discoveries, and shared rest now use stable
  side-by-side staging marks instead of converging on the same point. Companions continue tracking
  one another when they move, but their silhouettes, faces, paw gestures, and activity motifs stay
  readable during the interaction.
- Playful and dynamic choices receive a modest lift: solo and social play, greetings, and sprints
  compete more often with passive idling, while climb-watching, playful interruptions, and the rare
  squabble occur slightly more often when their existing relationship requirements are met.

### Fixed

- Soft-quadruped gesture paws use a compact cat-specific reach and chest-rooted placement, keeping
  long generated forelimbs from crossing the body or obscuring the face. Stored genomes and save
  version 11 remain unchanged.

## [0.51.0] - 2026-09-02

### Changed

- Soft quadrupeds read as cats. Filled triangular ears layered from outline, coat, and inner-ear
  colors sit on the crown, a smaller head carries a muzzle and nose, the tail is carried up off the
  rump, and all four paws stay planted in every action that does not visibly use them.
- Hoppers read as rabbits. Long upright ears—or a lop pair—carry an inner-ear streak, the tail is a
  cotton puff behind the rump, the hind feet are longer and planted forward, and a rounder, lower
  crouch replaces the upright egg while leaving headroom for the ears.
- Ear and tail genes now shape those features instead of removing them, so no cat or rabbit rolls a
  genome without ears or a tail. Style, size, and length still vary each one, and the rounded,
  folded, tufted, and lop variants remain distinguishable. Blobs are unchanged.
- Every change is in the renderer, so save version remains 11 and genome generation is untouched.
  Existing colony members keep their exact stored genome and simply get the new rig, and a
  `FORMIGA-…` seed code still imports the same creature it did in v0.50.0.

## [0.50.0] - 2026-09-02

### Added

- An entirely local PNG/JPEG reference matcher in the Colony Creature Studio. It decodes at most a
  16 MB, 4096×4096, 16-million-pixel image, compares palette, silhouette, proportion, symmetry, and
  appendage cues against exactly 512 valid procedural candidates, and presents the closest normal
  Formiga genome before any save change.
- Random full-size creature previews plus explicit add, replace, and remove controls. A persistent
  Keep toggle protects chosen creatures from both individual replacement and confirmed bulk
  regeneration.
- Persisted adult/mini roles and mini parentage. Colonies retain at most four creatures and three
  full-size adults; no adult receives more than two minis, and existing minis rebalance evenly when
  adults join, using the oldest adult as the deterministic tie-break.

### Changed

- The one-hour and one-week calendar arrivals are minis. The one-calendar-month arrival is a new
  full-size adult whenever an adult slot is open. Newly accepted full-size adults can receive minis
  on the same one-hour and one-week cadence, subject to colony and per-adult caps.
- Accepted reference matches store only an ordinary procedural seed, fresh birth, and fresh compact
  history. Source pixels, file paths, metadata, extracted features, and temporary preview textures
  are discarded and never enter the save or a network request.
- Save version 11 deterministically classifies legacy first creatures as adults and later creatures
  as minis cared for by the first. Every prior creature, ID, resolved appearance, name, birth,
  memory, tendency, routine, relationship, object, shelter, position, and setting is preserved;
  legacy colonies are never truncated to satisfy the new generation caps.

### Fixed

- Removing an adult now deterministically reparents its minis, and the final full-size adult cannot
  be removed. Replacement also reparents dependents and rebuilds bounded relationships without
  leaving stale runtime state.

## [0.47.0] - 2026-09-01

### Added

- On-demand 960×600 PNG creature cards from each Colony profile. The illustrated keepsake uses the
  creature's generated sprite and palette, cozy pixel scenery, learned descriptor badges, family,
  UTC birth month/year, and a deliberately abbreviated seed glimpse.
- A native save dialog with a Unicode-safe suggested filename and explicit PNG filter. Cancelling
  the dialog performs no render and creates no file.
- A deterministic `formiga-tools creature-card` preview command and checked-in reference card for
  visual review of the release artwork.

### Changed

- Card canvases, font atlases, and PNG buffers are created only after the user chooses a destination
  and are released when export finishes. The normal simulation and renderer retain no card state,
  texture, worker, or idle allocation.
- Exported PNGs contain only the visible read-only profile fields and ordinary pixel data. They omit
  the full seed, memory JSON, relationship values, device data, screen information, source paths,
  and hidden text metadata.
- Save version remains 10. Card export is read-only, so every v1–v10 colony remains compatible and
  no creature identity, name, history, relationship, object, or shelter state is rewritten.

## [0.46.0] - 2026-09-01

### Added

- Exact offline creature sharing through a case-insensitive `FORMIGA-…` code. The grouped
  Crockford Base32 payload contains a format version, original generation, full 256-bit origin
  seed, and four-byte checksum.
- “Copy seed” on each read-only Colony profile, plus a General-tab import field that validates the
  prefix, grouping, length, alphabet, version, generation, padding, and checksum before any state is
  changed.
- An explicit colony-replacement acknowledgement for import. The recreated creature begins as
  colony order zero with a fresh birth, memory, learned state, routines, relationships, objects,
  home, and companion schedule.

### Changed

- Imported appearances, innate personalities, display scales, IDs, and behavior seeds reproduce all
  four possible source generations byte-for-byte. Custom names and lived history are intentionally
  not included.
- Future companions use a deterministic colony seed derived from the shared origin, preventing the
  source generation from reappearing in its own imported lineage. Existing visibility, habitat,
  motion, and application settings are retained.
- Save version remains 10. Seed codes use the already-persisted immutable `CreatureOrigin`, so every
  v1–v10 colony remains compatible and no save migration or network access is introduced.

## [0.45.0] - 2026-09-01

### Added

- Six deterministic shelter decorations: leaf, banner, stone, flower, lamp, and roof ornament. A
  colony earns one unique decoration every four to nine UTC days and retains at most six.
- Decoration selection reflects the dominant compact colony state across creature memories, bond
  scores, the last ritual, and accumulated colony objects, with seed-derived deterministic
  tie-breaking.

### Changed

- Decorations are baked into the existing procedural 64×64 shelter canvas. The GPU regenerates and
  uploads that one texture only when the shelter genome or bounded decoration list changes; normal
  rendering retains the existing shelter quad and draw call.
- Save version 10 adds only the decoration list, next timestamp, and ordinal inside the existing
  home state. Deterministic v1–v9 migration preserves every creature, bond, ritual, object, setting,
  custom name, memory, tendency, routine, birth time, home identity, and resolved genome.
- Overdue colonies receive at most one decoration before scheduling the next date from the current
  maximum-seen UTC time. Duplicate or excess saved decorations are canonicalized to six unique
  typed values without affecting the rest of the colony.

## [0.44.0] - 2026-09-01

### Added

- Eight deterministic static colony-object families: pillows, toys, plants, blankets, paper scraps,
  pebbles, lamps, and cups. A colony retains at most eight objects, each with a stable ID, display,
  normalized position, and semantic role.
- A deterministic three-to-seven-day UTC object schedule. An overdue colony receives at most one
  object before the next timestamp is scheduled from the current time, so downtime never replays a
  backlog.
- One seed-derived 128×16 object atlas and at most eight cached static quads per display. Object
  vertices rebuild only when the objects, habitat, display geometry, scale, or colony seed changes.

### Changed

- Nearby objects contribute role-specific sleep, play, comfort, social, or curiosity utility at
  existing action-selection boundaries, capped at `+0.25` without adding actions or simulation.
- Save version 9 adds only the bounded colony-object projection and its next schedule timestamp.
  Deterministic v1–v8 migration preserves every existing creature, bond, ritual, setting, name,
  memory, tendency, routine, birth time, home, and resolved genome.

### Fixed

- Objects whose saved display or normalized position is no longer valid now snap to the nearest safe
  habitat floor or shelter area instead of disappearing or escaping the configured habitat.

## [0.43.0] - 2026-09-01

### Added

- A bounded runtime graph for overlapping window tiers, with deterministic routes of at most four
  hops or climbs through the existing window-journey system.
- Narrow-gap recognition for horizontally separated windows with 10–28 logical points of space and
  at least 64 points of overlapping height. Eligible creatures can traverse the gap with a new
  runtime `SqueezeWindow` state that reuses the ordinary six-frame traversal clip.
- A temporary 0.72× horizontal body-and-face quad scale during squeezes. No collision engine,
  physics body, texture regeneration, or additional atlas frame is introduced.

### Changed

- Route scoring combines bounded learned climbing, exploration, cursor trust, cursor invitations,
  and preferred-region hints while preserving the creature's innate utility behavior.
- New runtime action code 24 is appended after all prior routine codes, leaving persisted routine
  keys 0–23 byte-for-byte stable. Save version remains 8 and all v1–v8 colonies load unchanged.

### Fixed

- Multi-tier and squeeze routes retain the exact supporting rectangles and cancel immediately if
  the topology hash or either supporting window changes, settling the creature onto a safe habitat
  surface rather than following stale geometry.

## [0.42.0] - 2026-09-01

### Added

- A runtime-only `DesktopTopology` derived from safe visible-window rectangles, bounded to 64
  windows and 96 landmarks. It recognizes isolated window islands, exposed top corners, and slow
  moving platforms without reading titles, pixels, URLs, or document content.
- Privacy-safe cursor invitations: dwelling within 24 logical points of a ledge for 1.5 seconds at
  under 25 points per second can coax a sufficiently trusting creature toward that ledge.
- Geometry-aware corner peeks and island preferences that reuse existing inspection, gaze, perch,
  climb, hop, and riding actions with no new atlas frames.

### Changed

- Desktop topology rebuilds only when the bounded visible-window geometry hash changes. Calm
  platform motion uses the existing attachment behavior, while successful rides continue to feed
  compact experience memory through the existing 60-active-second observation projection.
- Save version remains 8. v1–v8 colonies load without an additional schema migration, and topology,
  cursor dwell, window landmarks, and prior geometry are never serialized.

### Fixed

- Exposed-corner and island classification remains bounded and deterministic across negative
  virtual-desktop coordinates, overlapping windows, display scaling, and rapid geometry changes.

## [0.41.0] - 2026-09-01

### Added

- Rare deterministic colony rituals scheduled 12–48 hours apart: picnics, group naps, floor races,
  shelter gatherings, two-creature catch games, group presentations, hatch days, quiet-day huddles,
  and late-night sleep piles.
- A runtime-only `ColonyPlan` that gathers eligible creatures and coordinates existing actions and
  habitat-safe target points. Rituals use the ordinary behavior-selection cadence and add no new
  animation atlas, physics system, polling loop, or persisted event history.
- Local-time hatch-day and late-night eligibility with UTC fallback. Reduced motion substitutes
  calm rituals for races, while idle and unchanged geometry can permit a privacy-safe quiet huddle.

### Changed

- Save version 8 persists only the next ritual timestamp, last kind, ordinal, and hatch-day
  acknowledgement. Deterministic v1–v7 migration preserves all creature identity, appearance,
  custom names, memories, tendencies, routines, current state, and bond scores.
- An overdue ritual waits for a safe action boundary, runs at most once, and schedules its next
  occurrence from the current time instead of replaying anything missed during downtime.
- Rituals cancel safely when the colony is hidden, paused, dragged, tossed, or loses valid display
  geometry. Interrupted rituals retry after a deterministic two-to-six-hour delay.

### Fixed

- A long-running multi-creature overlay could remain transparent after the presentation surface
  became repeatedly occluded or timed out. The renderer now reconfigures and redraws a stalled
  surface automatically, grows its bounded vertex buffer if needed, and no longer requires an app
  restart to recover.
- Creatures whose native monitor identifier changes after sleep, hot-plugging, or a display-mode
  transition are rebound to the monitor containing their saved position instead of remaining alive
  but absent from every overlay.
- v7→v8 migration preserves already-canonical relationship scores byte-for-byte rather than
  rebuilding them as legacy relationship records.
- Creatures could disappear from a desktop and stay missing until every window covering the screen
  was hidden, and revealing them on one macOS Space left the others empty. Full-screen occlusion
  hid the colony by ordering the shared overlay window out, which detached it from every Space at
  once; it then rejoined only whichever Space was active when it was ordered back in. Full-screen
  apps now suppress drawing instead, so the overlay stays attached to all Spaces, and its
  all-Spaces collection behavior is re-applied on every hide/show cycle.

## [0.40.0] - 2026-08-31

### Added

- Compact persistent bonds for every unordered creature pair. Affinity, familiarity, playfulness,
  and avoidance use exactly four raw score bytes per pair, with at most six pairs in a four-creature
  colony.
- Calm five-minute proximity observations and completed greetings, shared rest, play, discoveries,
  homecoming greetings, climb watching, toss concern, toy stealing, and harmless squabbles now
  adjust bond scores through the existing ephemeral `WorldEvent` projection.
- Target-aware sequences that reuse `Follow`, `Sleep`, `PresentDiscovery`, `SocialPlay`, `Greet`,
  `InspectScreen`, and `ReactToWindow`. Creatures can follow a preferred companion, sleep beside it,
  bring it a discovery, steal its temporary toy, greet it after shelter visits, watch it climb, and
  react when it is tossed without adding relationship-specific animation frames.
- Read-only qualitative bond and playfulness summaries in Colony profiles.

### Changed

- Social utility now combines innate personality with bounded pair scores. Positive contact can
  reduce avoidance, while stressful or competitive encounters can increase it; scores saturate
  safely and never rewrite a creature's genome.
- Runtime bond plans refresh a moving target's position and cancel safely if that target is missing,
  sleeping, homebound, tossed, removed, on another display, or otherwise unavailable for the
  selected interaction.
- Five-second milestone thought bubbles are now intentionally blank. Learned descriptors remain
  private until the user chooses to inspect the creature's read-only Colony profile.
- Save version 7 migrates every v1–v6 colony deterministically. Legacy per-creature relationship
  floats become canonical shared pair records, reciprocal values are averaged, and all creature
  identity, generated appearance, custom names, birth times, memories, tendencies, routines,
  positions, and settings are preserved.
- Relationship diagnostics retain only the `bond_interaction` event category; names, coordinates,
  scores, targets, and memory payloads remain absent from logs.

## [0.39.0] - 2026-08-31

### Added

- Click-without-drag petting with a visible affectionate response that reuses the greeting body
  clip and adds no creature-atlas frames.
- Compact typed memories for pets, tosses, placements, interrupted and uninterrupted sleep,
  ledges, window rides, climbs, discoveries, play, and home visits. One-minute active observations
  update accumulated state and are discarded; cursor paths, coordinates, layouts, and event history
  are never saved.
- Eight bounded learned tendencies for cursor trust, sociability, climbing, sleep security,
  exploration, play, home affinity, and routine. Their combined utility contribution is capped at
  ±0.35 and contrary experiences can reverse every learned preference.
- Deterministic cozy names and a Colony settings tab with learned descriptors, age, discoveries,
  favorite display/region, and closest companion. Only names are editable; 1–24 trimmed Unicode
  scalar values are accepted while control characters and line breaks are rejected.
- Persistent profile badges with ±35/±25 descriptor hysteresis and optional five-second milestone
  bubbles. Bubbles are globally singular, throttled to once per creature per 12 active hours, and
  rendered into a temporary texture that is released when the notice ends.
- `ActionChoice` targets for creatures and points, allowing repeated placement to gradually guide
  ordinary traversal toward a preferred 3×3 display region.

### Changed

- Save version 6 replaces unbounded string-keyed habits with a fixed twelve-slot numeric routine
  table and migrates the twelve strongest v1–v5 entries. It also adds stable creature origin,
  colony order, name, memory, and tendency records within a 192-byte raw-memory and 2 KiB serialized
  per-creature budget.
- Every world event passes through the compact projection path before it can be drained. Diagnostic
  logs retain event categories only, never names, coordinates, relationship values, or memory
  payloads.
- Direct manipulation now tracks maximum cursor excursion. Gestures at or below six logical points
  are pets; larger gestures retain precise placement, tossing, cancellation, and in-flight re-grab.
  Petting a homebound creature no longer dismisses its shelter.
- Dangling art is raised two source pixels so hands sit on the window edge. Upward routes now lift
  and move inward throughout a continuous climbing mantle, removing the brief landing-pose hitch
  before a creature settles onto a ledge.

## [0.38.1] - 2026-08-31

### Changed

- Additional creatures now join a colony one hour, one week, and one calendar month after it is
  created, while the default colony remains capped at four creatures.
- Calendar-month arrivals preserve the UTC time of day and clamp end-of-month dates safely, so a
  colony created on January 31 receives its monthly arrival on February 28 or 29.
- Overdue arrivals retain the existing 15-second reveal spacing and maximum-seen UTC guard, so
  relaunches and clock rollback cannot skip, duplicate, or remove a creature.
- Save version 5 records each creature's birth timestamp and deterministically reconstructs birth
  dates for v4 colonies without replacing their existing shelter state.

## [0.38.0] - 2026-08-28

### Added

- Low-frequency `ClimbWindow`, `Dangle`, `InspectScreen`, and `PresentDiscovery` activities with
  four-frame family-specific poses, expressions, gaze, and static reduced-motion variants.
- Staged upward window routes that traverse to the nearest inner edge, climb at 44–62 logical
  points/second, and mantle onto the ledge; downward transfers retain the existing hop.
- Geometry-only inspection landmarks at safe-region and window thirds, per-creature dangle and
  inspection cadence, and one colony-wide discovery cadence. All ambient countdowns stop while
  Formiga is paused or hidden.
- Eight deterministic gems, keys, leaves, shells, charms, and relics per creature, pre-baked into
  one appended layered-texture row and shown with a temporary third quad only during discovery.
- Fast-release creature tossing with a fixed three-sample cursor history, capped launch velocity,
  gravity, swept ledge/floor collision, one soft bounce, a three-second recovery limit, and final
  landing events. Slow releases retain precise placement.
- An accelerated ambient-art sheet covering every family, new action frame, placement mode, and
  discovery variant.

### Changed

- The body atlas remains at 90 unique frames by reusing the dragged clip for `Tossed`; the exact
  combined body/face/trinket texture budget is 1,161,216 bytes per creature under a 1.2 MB limit.
- GPU sprite placement and alpha-aware interaction proxies share a handhold-relative contract for
  dangling art.
- Drag update and release commands now carry normalized cursor velocity and distinguish placed from
  tossed outcomes. Paused, reduced-motion, and disabled-window-ledge modes retain non-ballistic safe
  behavior.
- A defaulted transient `activity_variant` keeps existing v4 saves compatible without a version bump;
  interrupted ambient activity and toss flight reload as idle and no discovery collection persists.

## [0.37.1] - 2026-08-27

### Fixed

- Creatures were hidden on any monitor covered by a driver HUD. The NVIDIA GeForce overlay keeps a
  window sized to the primary display for as long as the driver is loaded, and fullscreen app
  occlusion counted it as a fullscreen application, so creatures disappeared on that monitor
  whenever the default occlusion setting was on. Windows that cannot be activated, are
  click-through, or are layered tool windows are no longer treated as ordinary application windows,
  which also stops creatures from walking along an invisible ledge across the screen.

## [0.37.0] - 2026-08-27

### Fixed

- Creatures were invisible on Windows overlays. The overlay needs `WS_EX_LAYERED` for reliable
  cross-process click-through, but a plain HWND swap chain cannot keep per-pixel transparency while
  that style is set, so every creature rendered blank. Overlays now present through
  DirectComposition, which can target a layered HWND without discarding alpha.
- The Windows overlay surface now requests premultiplied alpha. DirectComposition composites
  `DXGI_ALPHA_MODE_PREMULTIPLIED` only, and the previously preferred `PostMultiplied` mode maps to
  `DXGI_ALPHA_MODE_STRAIGHT`, which DXGI does not support for composition swap chains. macOS keeps
  `PostMultiplied`, the only transparent mode CAMetalLayer reports.
- Layered window attributes are initialized so the overlay HWND is presented at all, and the
  requested input mode is reapplied after each hide/show cycle because winit rebuilds native window
  styles when showing a window.

### Changed

- Windows overlays now require a DX12-capable adapter. DirectComposition is the only path that
  preserves per-pixel alpha on a layered, click-through window, so the overlay instance no longer
  falls back to the Vulkan or OpenGL backends.

## [0.36.6] - 2026-08-27

### Added

- Optional once-daily and manual GitHub release checks in the tray and About screen.
- Background downloads for exact-platform DMG/MSI assets with size limits and mandatory SHA-256
  verification before the installer can be opened.
- Windows MSI handoff with clean Formiga shutdown and macOS DMG handoff for manual replacement.
- Autonomous eating, drinking, and short sprinting behaviors with distinct drive outcomes.
- Deterministic balls, yarn, tossed leaves, snacks, cups, and bowls derived from each creature's
  existing appearance genome and pre-baked into its animation atlas.
- Family-specific hand and front-paw poses for play, eating, drinking, and sprinting.
- A generated passive-activity art sheet for reviewing every body family.

### Changed

- Tagged release versions now flow into the binary, macOS bundle, Windows MSI, and artifact names.
- Solo play now visibly manipulates a generated toy instead of relying on an effect motif alone.
- Desktop scheduling treats sprinting as spatial motion while eating and drinking remain low-cost
  pose-only clips.
- Homebound creatures are spaced by their drawn width instead of a fixed 18 points, so the colony
  no longer stacks onto a single point at the shelter.
- Creature sprites are seated by each creature's authored under-body clearance, putting their feet
  on the surface they stand on rather than hovering above it.

### Fixed

- Creatures at the shelter could not be picked up. Interaction proxies were positioned before they
  were resized, and on macOS that offsets the window by the size difference; because both calls
  only run when the value changes, a motionless creature never corrected it.
- A press landing on an overlapping interaction proxy is resolved against the creature alpha masks,
  so clicks reach the creature actually drawn under the cursor.
- A drag whose mouse release was never delivered to the proxy window no longer wedges the session
  and blocks every later grab.

## [0.31.0] - 2026-08-26

### Added

- Default-on full-screen application occlusion on macOS and Windows, detected exclusively from
  safe display and window geometry.
- An Applications setting for opting out of automatic full-screen hiding.
- A close-up coral mascot app icon, peeking from a mint shelter with expressive eyes and hands,
  designed to remain readable down to system-tray sizes.

### Changed

- Replaced the fixed 50 Hz active wake-up with adaptive 4–20 Hz simulation deadlines.
- Capped moving presentation at the 20 Hz simulation rate and matched pose-only presentation to
  each authored animation's actual frame rate.
- Native drag-proxy position, size, visibility, and generated alpha masks are now cached.
- Empty or fully occluded monitor overlays stop presenting after one clearing frame.
- Full-screen displays hide their overlay and interaction proxies, preventing compositor work and
  invisible input interception while Formiga is covered.
- Application owner identity lookups are deduplicated within each desktop-window scan.
- Application-occlusion geometry is recalculated and its GPU uniform uploaded only when window or
  rule state changes.

## [0.25.0] - 2026-08-26

### Added

- Layered face atlas with eleven expressions, nine gaze directions, and open/half/closed eyelids.
- State- and activity-aware expression resolution with deterministic irregular blinking.
- Family-specific blob pseudopods, hopper mitten hands, and quadruped front-paw gestures.
- Pre-baked sleep, investigation, play, greeting, social, and startle effects.
- Expression and gesture art-lab contact-sheet commands.
- Deterministic v2→v3 save migration for resolved face, forelimb, and effect genes.
- Drag-to-Applications macOS DMG and normal per-user Windows installer packaging.
- Generated cross-platform application icon and first-launch Settings onboarding.
- Seeded bottom-corner colony homes with four procedurally rendered shelter families.
- Persistent 15-minute home visits and 15-minute minimum reappearance cooldowns.
- Drag-to-dismiss home behavior and calm activity-coordinated homebound poses.
- Deterministic v3→v4 migration for shelter identity and durable home timing.

### Changed

- Replaced three gaze-duplicated body atlases with one body atlas and one compact face atlas.
- Interaction masks now use the fully composited expressive frame.
- Fixed one-pixel line rasterization, improving facial and appendage clarity.
- Updated portfolio assets and release metadata for v0.25.0.
- Creatures now transfer between reachable window ledges at different heights instead of treating
  the first ledge as a permanent horizontal track.
- Release automation now publishes the current-version DMG, MSI, and portable ZIP artifacts.

## [0.2.0] - 2026-08-25

### Added

- Native settings window with General, Habitat, Applications, and About sections.
- Alpha-aware direct creature dragging, safe landing, cancellation, and gather command.
- Visible ledge journeys and immediate reactions to nearby window creation, movement, and closure.
- Habitat presets plus allowed/excluded per-display rectangles and desktop editing.
- Privacy-safe application rules and GPU visual occlusion with window-order subtraction.
- Stable display identities, macOS bundle IDs, Windows AUMIDs, and executable-hash fallback.
- Explicit v1 save migration and portfolio/release documentation.

### Changed

- Upgraded the renderer/UI stack to wgpu 30.0.1 and egui 0.36.1.
- Detailed controls moved from the tray into Settings.

## [0.1.0] - 2026-08-25

- Initial procedural desktop ecosystem foundation for macOS and Windows.
