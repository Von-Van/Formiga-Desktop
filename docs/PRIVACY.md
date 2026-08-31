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

The versioned JSON save contains the colony seed, resolved genomes, personality values, birth
timestamps, current drives, positions, habits, relationships, arrival state, habitat zones,
application rules, and settings. A one-file backup and local rotating diagnostic log support
recovery and troubleshooting. Drag state is never persisted midway through a grab.
