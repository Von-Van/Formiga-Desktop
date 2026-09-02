# Privacy model

Formiga has no account system, analytics endpoint, advertising SDK, behavioral database, or cloud
colony service. Creature generation, simulation, artwork, settings, and save data remain entirely on
the user's computer.

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

- usable monitor geometry and scale;
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
tendencies, twelve numeric routine slots, at most six unordered relationship records, arrival state,
the next ritual timestamp, last ritual kind, ritual ordinal, hatch-day acknowledgement, habitat
zones, application rules, settings, and at most eight colony objects. Each object stores only a
stable ID, kind, privacy-safe display key, normalized position, and semantic role, plus the single
next object timestamp and ordinal. The home also retains at most six typed decoration names plus
one next decoration timestamp and ordinal. A relationship stores stable creature IDs plus four
one-byte scores—affinity, familiarity, playfulness, and avoidance—not an encounter history. The save
does not contain an event log, ritual history, ritual target positions, proximity samples, target
paths, cursor paths, sampled event coordinates, or past window layouts. Profile fields other than a
creature's name are read-only views of this local state.

Save version 10 accepts and deterministically migrates every v1–v9 colony. Migration converts legacy
relationship floats locally, preserves current v7 bond records byte-for-byte, and preserves creature
identity, generated appearance, custom names, birth times, memories, learned tendencies, and
routines. A v8 colony receives an empty object collection and one deterministic future timestamp;
a v9 colony receives an empty decoration list and one deterministic future timestamp. Existing
colony state is otherwise unchanged. Migration performs no network request and does not upload
either the old or migrated save.

Seed sharing is fully offline. A code contains only a format nibble, original generation, immutable
256-bit creature-origin seed, and checksum. It does not contain the creature's custom name, birth
time, memory, tendencies, routines, relationships, current colony seed, objects, shelter, display
keys, settings, device data, or desktop information. Copying uses the local system clipboard;
Formiga does not transmit, register, resolve, or look up a code.

Import validates the complete code before replacing anything, requires an explicit replacement
acknowledgement, gives the creature a fresh local birth and history, and derives its companion
lineage locally. No account, analytics event, server, DNS request, or network permission is involved.
Save version 10 is unchanged because the immutable origin fields already exist in every migrated
creature record.

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
memory payloads. A blank milestone bubble is rasterized locally only while visible and is discarded
after five seconds; descriptor text is shown only when the user opens the local Colony profile. Drag
state and in-progress interactions are never persisted.
