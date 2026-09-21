# Privacy model

Formiga has no account system, analytics endpoint, advertising SDK, behavioral database, or cloud
colony service. Creature generation, simulation, artwork, settings, and save data remain entirely on
the user's computer.

Temporary hangouts, ride intensity, handholds, hesitation decisions, recent setbacks, spectator
roles, game roles and turns, a leader's few recent positions, and a temporary plaything exist only
in memory. A creature's movement cadence is derived from data it already has and is never stored. They do not add saved window histories, cursor trails, game scores, or event logs.
Established hangouts may reinforce the existing coarse preferred desktop region, without retaining
the window identity or exact remembered spot in the save.

The sole network feature is an assisted update checker. Automatic checks are enabled by default,
can be disabled in **Settings → About**, and run no more than once per 24 hours when Formiga starts.
A manual check is also available from the tray and About screen. A check requests only public release
metadata from `api.github.com` for the Formiga repository. If the user chooses to download an update,
Formiga requests that release asset and its checksum from GitHub's release hosting. GitHub receives
ordinary connection information such as the user's IP address and a Formiga user-agent; Formiga does
not send its colony seed, save data, settings, desktop activity, window list, or a device identifier.

Update downloads are limited to the exact macOS DMG or Windows MSI name for the selected semantic
version, capped at 250 MB, checked against GitHub's advertised size, and SHA-256 verified before the
installer can be opened. Formiga does not silently install an update. Windows launches the verified
MSI and exits; macOS opens the verified DMG for the user to replace the app manually. Update
preferences, the last-check timestamp, and any downloaded installer are stored in the normal local
application-data directory.

The desktop adapters read only:

- usable monitor geometry and scale, including each display's work area — the rectangle the
  system leaves once its menu bar, Dock, or taskbar is taken away (`NSScreen.visibleFrame` on
  macOS, `MONITORINFO.rcWork` on Windows), which says where those bars sit and nothing else;
- global cursor position and derived velocity;
- system idle duration;
- visible top-level window rectangles and front-to-back order;
- a stable application owner identity for user-selected visual occlusion rules.
- whether an ordinary application window matches a display's full bounds, used for default
  full-screen hiding.

On macOS, application identity is the public bundle identifier. On Windows it is preferably the
AppUserModel ID; conventional applications fall back to a SHA-256 digest of the canonical executable
path. Raw paths are discarded after hashing. The save keeps only that identity and a display label.

Formiga does **not** request or retain window titles, process paths, document names, URLs, keystrokes,
click history, clipboard contents, screenshots, pixels from other applications, or application
content. It does not require Accessibility, Screen Recording, Input Monitoring, administrator
privileges, or elevated process access.

The versioned JSON save contains the colony seed, resolved genomes, personality values, creature
names and birth timestamps, current drives and positions, compact counters, bounded learned
tendencies, twelve numeric routine slots, one unordered relationship record for each pair of
companions — fifteen once a colony of six is full — arrival state,
adult/mini roles, mini parent IDs, Keep preferences, and bounded per-adult mini-arrival bits,
the next ritual timestamp, last ritual kind, ritual ordinal, hatch-day acknowledgement, habitat
zones, application rules, settings, and at most eight colony objects. Each object stores only a
stable ID, kind, privacy-safe display key, normalized position, and semantic role, plus the single
next object timestamp and ordinal. The home also retains at most six typed decoration names plus
one next decoration timestamp and ordinal. A relationship stores stable creature IDs plus four
one-byte scores—affinity, familiarity, playfulness, and avoidance—not an encounter history. The save
now contains a journal of at most 64 typed colony moments and timestamps: arrivals, discoveries,
learned preferences, close friendships, completed rituals, objects, and decorations. It does not
contain desktop observations, ritual target positions, proximity samples, target paths, cursor
paths, sampled event coordinates, or past window layouts. Profile fields other than a
creature's name are read-only views of this local state.

The save also holds four bounded keepsakes added in version 14. At most eight **pinned moments**,
each naming a journal entry that already exists by its timestamp, creature, and typed moment — a
pin cannot record anything the journal does not. At most one **scrapbook record** per trinket
variant — sixteen since 0.58.0 — each holding the variant number, the first-find timestamp, the finder's stable ID,
and the finder's name as it was then, so the record still reads after that companion leaves without
keeping a copy of the creature. **Appearance preferences** are a theme choice, a text-scale
percentage, and one outline flag. A **routine schedule** is an enabled flag, at most fourteen rows
of weekday bitmask plus local minute plus preset index, one override flag, and which preset was
last applied. None of these records a desktop observation, a window, a cursor position, or a time
the application was running: a schedule stores the times you chose, never the times you were there.

Save version 15 adds one record and nothing else: `visitors`, a bounded `VisitorState`. It holds
`gatherings`, a count of how many times the colony has gone home; `guest`, present only while
somebody is actually visiting, carrying that visitor's generated `creature`, its `source` (wanderer
or invited), an optional `stays_until_utc` for an invitation's twenty-four hours, `on_stage` for
whether it is out on the desktop right now, and `signed` for whether this visit is already in the
book; and `guest_book`, at most twenty-four entries of `visited_at_utc`, `name`, `origin`, and
`source`. A guest book's `origin` is the same `CreatureOrigin` a share code carries — appearance and
temperament — and nothing about the person who shared it, their computer, or their colony. One
`Visit` moment is added to the existing journal. `favorites` keeps at most eight visitors the reader
chose to invite again, each only a `kept_at_utc`, the `name` it went by, and the same `origin`; a
favorite stays until it is forgotten and never grows past eight on its own.

What a visit does *not* save is the visit itself: the scene's progress is `#[serde(skip)]`, so its
phase, beat, elapsed time, doorway, the places a guest tours, whom it has already gone over to,
and the residents' answers exist only while the program is running. The same is true of every other interaction added in 0.58.0 — thought bubbles, the open
menu, offer cooldowns, the overlap timers and per-pair cooldowns, and doorstep moments at the
houses. The circumstances a trinket was found in are not saved either: the scrapbook records what,
when, and who, exactly as it did before, and never why a particular keepsake qualified.

0.58.5 adds no field to the save and observes nothing new. The two keepsake trees, which of them
each find hangs in, where it hangs, and where each belonging lies in a yard are all worked out
while the program runs, from the colony's own seed and the scrapbook the save already holds: a
tree is a drawing of records you already had, not a record of its own. The save version is still 15, a
colony written by 0.58.0 opens unchanged, and there is no migration to run. Where the village
stands on the screen comes from the same monitor rectangles and habitat zones as before, and the
village being narrower reads nothing more about your desktop than the wider one did.

Opening the menu with Control-click on macOS reads the modifier flags from the same combined-session
event source Formiga already samples for the cursor and idle time. It is not a keyboard hook, adds
no event tap, and needs no permission Formiga did not already have; an interaction proxy never takes
keyboard focus, which is why the modifier has to come from there at all.

Sticker and colony-portrait exports contain only rendered pixels. A sticker is a GIF of one
creature's own frames: per-frame graphic-control blocks and exactly one loop block, with no comment,
application, or plain-text extension. A colony portrait is a 960×600 opaque PNG of every member,
their names, the month the colony began, how many of you there are, how many family lines, and the
village, resolved on a notional 1:1 desktop so no screen geometry reaches it. A postcard is a
960×600 opaque PNG of every member in the chosen scene in front of the village, with the scene's
name, a month, and the caption you typed, if you typed one; it carries no names at all, and its
filename names only the scene. None of them contains a seed, a share code, memories, learned
tendencies, relationship scores, the journal, the guest book, display keys, habitat zones, device
data, source paths, or hidden text metadata. The save dialog runs first, cancellation renders
nothing, and the buffers are released afterwards. A caption is never saved; it lives in the
settings window until the postcard is exported and is gone when the window closes. The colony as
it stood before your last change, kept so the change can be undone, lives in memory only: it is
never written to the colony file and is gone when the app quits.

Save version 17 accepts and deterministically migrates every v1–v16 colony. It adds the classic
parts of a recipe — six small numbers choosing a coat, face, limbs, crown, pattern, and tail — which
describe how a companion is drawn and nothing about the person or the desktop; the favorite
visitors described above; each companion's `leaning`, written only once its owner picks one of
the four roaming choices, which stays in this colony and is never part of a share code; and each
companion's `habits`, written only once it picks one up: at most two names from a fixed list of
five, with a journal line for each, and nothing about when, where, or how often it did them. A
share code carries no habits. How a companion celebrates is read from its seed and never stored.
The hangout spots put down on the village ground are stored with the home as a kind and a
fraction along the ground — never a screen position — and only once one is put down. The garden
patches are stored the same way, the chosen palette by its name, and a cottage order as the ids of
companions already in the colony, each only once it is chosen.
A moment the village is asked to share while the houses are out is runtime only: who was asked,
who answered what, and where anyone stood are never written, and one that runs its course leaves
only the same shared-moment line in the journal a ritual leaves. A v16 colony opens unchanged, its recipes reading as plain modular ones
and no favorites kept; the version moved so an older build refuses the file instead of quietly
dropping what it cannot hold.

Save version 16 accepted and deterministically migrated every v1–v15 colony. It adds no field of
its own: the version moved only because a colony may now hold six companions and the fifteen
bond records six of them make, and an older build reading such a file would quietly drop what it
could not hold rather than say so. A v15 colony opens unchanged.

Save version 15 accepts and deterministically migrates every v1–v14 colony. A v14 colony receives an
empty visitor state — no guest, no gatherings counted, an empty book — and nothing else about it is
read, rewritten, or invented.

Version 14 accepted and deterministically migrated every v1–v13 colony, and that chain is unchanged
beneath version 15. A v13 colony receives
empty pins, an empty scrapbook, default appearance preferences, and a disabled schedule; the
scrapbook is never populated from an existing aggregate discovery count, because that count cannot
say which trinket was found, when, or by whom. Migration converts legacy
relationship floats locally, preserves current v7 bond records byte-for-byte, and preserves creature
identity, generated appearance, custom names, birth times, memories, learned tendencies, and
routines. A v8 colony receives an empty object collection and one deterministic future timestamp;
a v9 colony receives an empty decoration list and one deterministic future timestamp. Legacy
creatures receive only adult/mini role, Keep, and bounded mini-schedule fields; none are deleted,
regenerated, or truncated. Missing modular recipes remain absent, preserving legacy art; stored
loose objects relocate to the village ground line on the next world tick. Migration performs no
network request and does not upload
either the old or migrated save.

Seed sharing is fully offline. A code contains only a format nibble, original generation, immutable
256-bit creature-origin seed, the sixteen-byte design recipe when the creature has one, four bytes of
classic parts when that recipe has any, and a checksum. It does not contain the creature's custom name, birth
time, memory, tendencies, routines, relationships, current colony seed, objects, shelter, display
keys, settings, device data, or desktop information. Copying uses the local system clipboard;
Formiga does not transmit, register, resolve, or look up a code.

Import validates the complete code before showing a preview. Adoption adds to an existing colony
within its capacity; replacement requires an unkept target and explicit acknowledgement. Both give
the incoming creature a fresh local birth and history while preserving its exact origin. Unrelated
colony members keep their histories. No account, analytics event, server, DNS request, or network permission is involved.
The shared code contains no journal, home preferences, or saved behavior routines.

Creature-card export is local and read-only. The 960×600 PNG contains ordinary rendered pixels for
the creature, custom name, family, up to three visible descriptors, UTC birth month/year, colony
number, and an abbreviated seed glimpse. It does not embed the full share code, memory JSON,
relationships, objects, shelter state, display keys, device data, screen content, source paths, or
hidden text metadata. The save dialog opens before rendering; cancellation creates no image, and
temporary card/font/PNG buffers are released after the export completes.

Reference-guided generation is also entirely local. Formiga accepts only PNG or JPEG input under
fixed byte, dimension, and decoded-pixel limits, reduces it to temporary color and silhouette cues,
and constructs exactly 512 bounded modular candidates. It does not read embedded text for behavior,
upload the image, call an AI service, retain the path, copy pixels into creature art, or write image
bytes or full extracted feature vectors to the save or logs. Cancelling or clearing the preview
releases its temporary rendered texture; accepting stores the creature seed, a compact design
recipe (parts, proportions, and derived coat/accent colors), and fresh local state. Shared codes
for modular creatures include that recipe, not the source image, its path, or metadata.

Colony management persists a role, parent ID for minis, Keep flag, and two bounded mini-arrival
bits. Keep is a local replacement preference, not behavioral analytics. Removing or replacing a
creature happens only after an explicit UI action; save migration never performs either operation.

Quiet-day ritual eligibility uses only the already-available system idle duration and whether safe
window rectangles remained unchanged. Hatch days and late-night sleep piles use local date/hour with
UTC fallback. Formiga does not query weather, screen content, titles, URLs, or location.

Desktop topology uses only the already-available visible window rectangles, order, monitor geometry,
and instantaneous cursor position and velocity. Its bounded island, exposed-corner, moving-platform,
and cursor-invitation projection exists only in memory. Formiga does not persist the topology,
landmarks, cursor dwell, cursor path, or prior window layouts. Save version 8 is unchanged in v0.42,
so loading an older colony adds no topology fields and cannot replace a creature.

Window-route edges, narrow-gap classification, exact supporting rectangles, path choices, and route
progress are likewise runtime-only geometry. They use no process metadata or pixels and disappear on
completion, cancellation, pause, hide, relaunch, or supporting-window change. v0.43 appends one
runtime action name but does not add a save field or renumber any existing persisted routine key.

Colony objects are generated locally from the existing colony seed and safe display geometry. They
contain no source image, window title, application identity, cursor sample, interaction history, or
screen content. Their positions are normalized against privacy-safe display keys and repaired into
the current habitat after geometry changes.

Shelter-decoration choice reads only the already-persisted compact counters, bond scores, last
ritual kind, and colony-object kinds. It does not store the score calculation, a change history, a
reason string, or any new observation. Decoration pixels are generated locally into the existing
temporary shelter canvas and contain no source image or external content.

A one-file backup and local rotating diagnostic log support recovery and troubleshooting. Logs name
event categories only; they do not record creature names, coordinates, relationship scores, or
memory payloads. A small sprout milestone bubble is rasterized locally only while visible and is discarded
after five seconds; descriptor text is shown only when the user opens the local Colony profile. Drag
state and in-progress interactions are never persisted.


Full-colony backup is deliberately different from a share code: it contains the complete private
save, including names, learned history, journal, habitat, and settings. Export writes only to a
user-selected local path. Restore reads at most 2 MiB, validates the snapshot, confirms replacement,
and preserves existing colony files first. No backup is uploaded. Recovery copies have unique local
filenames and are never automatically deleted; unreadable saves remain untouched until a user
chooses recovery or a valid backup safely repairs the primary.

The introduction completion flag, at most two saved behavior presets, a quiet-mode expiry timestamp,
and six decoration-visibility bits are also local. Studio comparisons retain at most four temporary
candidates; closing the window releases their artwork. Color/body locks use only generated design
recipes, never a retained source image.


Environmental curiosity uses the same rectangles, stacking order, and actual scan times. Its
previous/current shapes, short-lived event origins, viewing spots, recent supporting-window keys,
and per-display space estimates exist only in memory. They are bounded, reset on unavailable
observations, and excluded from saves, journals, exports, and creature codes. Failed native window
enumeration retains the previous rendering snapshot and marks observations unreliable; it is not
interpreted as an empty desktop. No window content, screenshot, or additional permission is used.


The optional sprite outline is drawn from the creature's own alpha silhouette into the frame it
already occupies. It reads no wallpaper, no screenshot, and no application pixel, requires no
additional permission, and leaves the clickable silhouette exactly as it was.

Hide and seek keeps the one position a seeker last actually had the hider in view, for the length
of one scene, in memory only. It is a creature position, never a cursor position, a window, or
anything about the desktop, and it is excluded from saves, journals, exports, and creature codes.
Being hidden means being out of a companion's line of sight along a surface; a creature is never
placed where the person at the desk cannot see it, so no window's contents are ever consulted or
inferred. The window race reads the same rectangles and stacking order every other behavior does.

Cursor curiosity retains one previous cursor point/time and a bounded local aggregate (center,
travel distance, turns, and elapsed time), with no trail or historical log. Native sample times,
monitor-change cues, crossing routes, and reorientation state are also excluded from saves and
exports. Turning off cursor reactions clears attention and prevents cursor-driven utility choices
and dwell invitations. These behaviors add no OS permissions or observation source.
