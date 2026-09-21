# Modular creature generation · v0.58.7

The design goal is a cute reinterpretation, never image tracing. A photo, illustration, logo,
or unusual reference should resolve to a readable pixel companion with a connected rounded body,
paired limbs, two large expressive eyes, and a mouth. Clear subjects with plain or transparent
backgrounds provide stronger cues. No semantic classifier or model weights are included.

## Small construction recipes

`formiga-core::CreatureDesign` stores 16 bytes of choices and colors. Five body plans (round,
upright, long/four-pawed, winged, blob) combine independently with six ear styles, five tail
choices, bounded body/head/leg dimensions, three muzzle-patch choices, seven marking treatments,
and two RGB colors. All entry points clamp recipe dimensions; shared-code decoding rejects invalid
parts.

The blob plan is a single soft mass: it draws no separate head, carries the face high on the body,
keeps a rounder minimum footprint, and stands on stubby feet. It occupies index 4 so recipes and
version 2 codes written before it decode exactly as they did. A code carrying a blob needs v0.55.5
or newer to import.

The winged plan draws a membrane with a lit leading edge, a shaded underside, and a tip carried past
the shoulder, plus one of three structures: feathered quills, ribs to a drawn-down tip, or a pale
panel behind a darker rim. The structure is a pure function of accent, marking, and tail bytes the
recipe already holds, so wings vary between creatures, never change for a given creature, and travel
inside a version 2 code without another byte. Every mark stays inside the membrane's own outline, so
the silhouette, its connection to the body, and the reserved face are unchanged.

Random creatures use a named stream separate from legacy appearance/personality generation.
Minis inherit the parent's body plan and accent color, usually retain its ear style, and receive
small coat-color variations. Their smaller body proportions retain a large readable face.

Image generation analyzes at most 64×64 temporary pixels with aspect ratio preserved. Transparency
or border contrast separates foreground; fixed color bins identify dominant and contrasting colors.
Aspect and appendage cues adapt a fixed 512 candidates, with a quarter left exploratory. The
candidate's rendered color and shape are compared to the summary. The affinity score is not a
confidence that Formiga recognized the subject. Source images, paths, and feature vectors are
discarded; only the accepted parts/proportions/colors become durable state.

## Rendering and compatibility

The modular renderer lives beside the legacy renderer in `formiga-art`. It uses the same 48×48
body frames, 16×16 face frames, poses, expressions, and animation atlases. Tail/ear geometry sits
behind the body; gestures keep the reserved face area covered and never draw over the layered face;
outlines connect paws to bodies, and each side has exactly one limb that a gesture carries out. The palette
resolver softens colors once during atlas construction and guarantees dark facial contrast. There
are ten gesture poses; the tenth, `Watch`, leans the head toward what the creature is looking at and
pricks its ears, so every body plan needs a head that can lean without leaving the frame or
uncovering the reserved face.

An absent recipe selects the legacy rendering path. Old saves never acquire a replacement design
just by loading them. Version 1 seed codes replay legacy creatures unchanged. Version 2 codes carry
the exact recipe alongside the original seed/generation; keep the serialized enum ordering and
`modular-design-v1` generation stream stable. Incompatible recipe changes require a new format,
not reinterpretation of existing codes. Source provenance duplicates the bounded recipe so sharing
does not depend on a reference file or a temporary preview.

## Belongings and keepsakes

What a creature owns is generated from bytes it already carries. `prop_variants(genome)` reads one
signature out of the appearance genome and picks one of eight toys, one of four snacks, and one of
three kinds of drinkware; none of them adds a gene, and a creature's belongings never change. They
are baked into the same action atlas as the body, anchored to the paw or the mouth that is actually
drawn through `modular::prop_hold`, so a toy is held in a real hand rather than at a guessed offset.
`prop-sheet` shows all fifteen against the bodies that carry them.

Trinkets are a colony's, not a creature's. `formiga-core::trinkets` is the catalogue — sixteen
entries of name, description, hint, and condition, with `TRINKET_VARIANTS = 16` — and
`formiga-art::TrinketAtlasRenderer` bakes it into one 256×32 sheet of sixteen columns by two rows, a
rest frame and a glint frame. Its colours come from the colony seed and are chosen to stay clear of
every member's coat, so a found keepsake reads as a separate object however the colony is coloured
and whoever is holding it. A new variant is a new column, not a new texture per creature.

## Extending safely

- Add a bounded part or body plan with an explicit face-safe area and connected limb roots.
- Keep reduced-motion behavior, mirrored rendering, miniature proportions, and gesture poses. A new
  pose moves the limb a side already has; it never draws a second one beside it.
- Update code versioning before changing the byte layout or meaning of a serialized choice.
- Preserve atlas dimensions, clip counts, and the four-creature limit unless a separate resource
  budget change is intended. Generation must not introduce an idle worker or retain source pixels.
- Run workspace tests and review `generation-sheet`, `contact-sheet`, and `animation-preview`.
  Automated coverage includes all body/ear/tail combinations at extreme sizes, connected silhouettes,
  reserved faces, 1,000 generated creatures across every action, legacy migration, and exact sharing.

The village is independent of creature generation. `formiga-core::habitat` walks one ground line
outward from the colony house, laying out a keepsake tree, the colony house, and a cottage and a
porch for every later member, and closing with a second tree. The first colony member shares the
colony house; each later one gets a cottage of its own, half-size for a mini. Belongings are not
lots at all: they are scattered in the two trees' yards, four to each, so one can never land on a
house. Lots outside the house's accessible region stay hidden, and an inactive house hides the
whole village.

`ShelterRenderer::render_village` bakes the house, both cottage sizes, and the keepsake tree into
one 128×128 atlas of 64px cells, each drawn into its own cell-sized tile first so nothing bleeds
into a neighbour. The tree shares the house's own style, palette, and baseline, so it belongs to
the same yard. Every dwelling and every tree is one quad sampling that single texture — the inward
tree samples the tree cell with its horizontal UVs swapped — so the village adds no texture,
sampler, or bind group. Style details scale in thirds, which leaves the colony house
byte-identical to the standalone shelter render. `home-yard-sheet` shows all four styles and both
corners through the same layout function the renderer uses.
