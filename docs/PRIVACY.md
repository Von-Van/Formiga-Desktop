# Privacy model

Formiga is local-only. It has no account system, network client, analytics endpoint, advertising SDK,
or behavioral database.

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

The v2 JSON save contains the colony seed, resolved genomes, personality values, current drives,
positions, habits, relationships, arrival state, habitat zones, application rules, and settings. A
one-file backup and local rotating diagnostic log support recovery and troubleshooting. Drag state is
never persisted midway through a grab.
