# Modular creature generation · v0.55.0

The design goal is a cute reinterpretation, never image tracing. A photo, illustration, logo,
or unusual reference should resolve to a readable pixel companion with a connected rounded body,
paired limbs, two large expressive eyes, and a mouth. Clear subjects with plain or transparent
backgrounds provide stronger cues. No semantic classifier or model weights are included.

## Small construction recipes

`formiga-core::CreatureDesign` stores 16 bytes of choices and colors. Four body plans (round,
upright, long/four-pawed, winged) combine independently with six ear styles, five tail choices,
bounded body/head/leg dimensions, three muzzle-patch choices, seven marking treatments, and two
RGB colors. All entry points clamp recipe dimensions; shared-code decoding rejects invalid parts.

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
behind the body; gestures stay outside the face; outlines connect paws to bodies. The palette
resolver softens colors once during atlas construction and guarantees dark facial contrast.

An absent recipe selects the legacy rendering path. Old saves never acquire a replacement design
just by loading them. Version 1 seed codes replay legacy creatures unchanged. Version 2 codes carry
the exact recipe alongside the original seed/generation; keep the serialized enum ordering and
`modular-design-v1` generation stream stable. Incompatible recipe changes require a new format,
not reinterpretation of existing codes. Source provenance duplicates the bounded recipe so sharing
does not depend on a reference file or a temporary preview.

## Extending safely

- Add a bounded part or body plan with an explicit face-safe area and connected limb roots.
- Keep reduced-motion behavior, mirrored rendering, miniature proportions, and gesture poses.
- Update code versioning before changing the byte layout or meaning of a serialized choice.
- Preserve atlas dimensions, clip counts, and the four-creature limit unless a separate resource
  budget change is intended. Generation must not introduce an idle worker or retain source pixels.
- Run workspace tests and review `generation-sheet`, `contact-sheet`, and `animation-preview`.
  Automated coverage includes all body/ear/tail combinations at extreme sizes, connected silhouettes,
  reserved faces, 1,000 generated creatures across every action, legacy migration, and exact sharing.

The house yard is independent of creature generation. It uses one shared core layout function for
eight fixed slots beside the house, with stackable cubbies baked into the existing object atlas.
Slots outside the house's accessible region stay hidden, and inactive houses hide the whole yard.
`home-yard-sheet` shows all four shelters and both corners using that same layout function.
