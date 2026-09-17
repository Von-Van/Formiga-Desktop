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
| v1–v13→v14 migration, creature/object/bond preservation, names, births, rituals, and top-12 routines | CI | CI |
| Atomic round trip and corrupt-primary recovery from the previous-save backup | CI | CI |
| One-hour, one-week, and clamped calendar-month arrival boundaries | CI | CI |
| Fixed memory/routine limits and sub-2-KiB per-creature serialized growth | CI | CI |
| Learned utility monotonicity, ±0.35 cap, saturation, and contrary-experience recovery | CI | CI |
| Descriptor ±35/±25 hysteresis, badge persistence, and 12-active-hour bubble throttle | CI | CI |
| Unicode name validation, deterministic unique defaults, and user duplicate names | CI | CI |
| 60-active-second observation projection and hidden/paused suspension | CI | CI |
| Six-pair relationship maximum, canonical IDs, four score bytes, saturation, and deterministic round trip | CI | CI |
| Five calm minutes, bounded bond utility, contrary avoidance reduction, and score projection | CI | CI |
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
| Connected modular silhouettes and reserved faces at minimum/maximum proportions | CI | CI |
| Wing styles: all three reachable, stable per recipe, and visibly textured | CI | CI |
| Image aspect/alpha handling, dominant/accent colors, blank and extreme references | CI | CI |
| v11 save preservation, v1/v2 code validation, exact design add/replace/save/share | CI | CI |
| Mirrored village layout: separated lots, shared ground line, scale changes, missing/narrow displays and lot cap | CI | CI |
| Village atlas cells match their own dwelling and never bleed into a neighbour | CI | CI |
| Companion houses per colony member, with matching half-size cottages for minis | CI | CI |
| Belonging colors stay distinct from every creature palette they are carried against | CI | CI |
| Four-total/three-adult/two-minis-per-adult caps, even distribution, and oldest-adult tie-break | CI | CI |
| Keep replacement guard, bulk regeneration, final-adult protection, reparenting, and relationship normalization | CI | CI |
| 1,000 genomes × all actions and layered face checks | CI | CI |
| Generated passive-prop determinism and alpha coverage | CI | CI |
| Ambient cadence bounds, deterministic landmarks, pause/hidden suspension | CI | CI |
| Continuous upward traverse/climb/mantle, downward hops, and route interruption | CI | CI |
| Raised dangling GPU/proxy handhold placement contract | CI | CI |
| Eight deterministic opaque trinkets and hold playback | CI | CI |
| Exactly 90 unique body frames; layered atlas at or below 1.2 MB | CI | CI |
| Habitat region algebra | CI | CI |
| Pet/drag maximum-excursion classification across scale, placement, toss, cancel, and re-grab | CI | CI |
| Home petting without dismissal and threshold-crossing shelter dismissal | CI | CI |
| House spawn preserves positions; walking/landing, pause/pet/relaunch, display crossing, and spaced arrivals | CI | CI |
| Blank fixed-size milestone-bubble pixels, global singularity, and idle resource release | CI | CI |
| Alpha-mask and occlusion geometry | CI | CI |
| Update version/asset selection and 24-hour throttle | CI | CI |
| Update digest validation and unsafe-name rejection | CI | CI |
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

Setting `FORMIGA_ATTENTION_REVIEW_DIR` to an absolute path writes the sheets each of those tests
draws, and they were reviewed for this release. `attention-stages.png` and
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
| Two- and four-creature rendering for one hour without disappearance; automatic surface recovery | pending | pending | pending | pending |
| Package install, relaunch, v14 round trip, and representative v1–v13 migration without creature loss | pending | pending | pending | pending |
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
