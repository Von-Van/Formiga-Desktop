# Privacy model

Formiga is local-only and has no network client or telemetry endpoint. v0.1 stores one versioned JSON
file containing the colony's seed, resolved genomes, current drives, positions, habits, relationships,
arrival state, and settings. It also keeps a one-file backup and a small rotating diagnostic log.

The desktop adapters read only monitor rectangles, global cursor position and velocity, idle duration,
and generic visible top-level window rectangles. They do not read or retain window titles, process
names, documents, URLs, keystrokes, clicks, clipboard contents, screenshots, or application content.
No Accessibility, Screen Recording, Input Monitoring, administrator, account, or network permission
is required.
