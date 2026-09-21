use crate::{Canvas, PALETTES, Rgba};
use formiga_core::{ShelterDecorationKind, ShelterGenome, ShelterStyle};

mod houses;

pub const SHELTER_SIZE: u32 = 64;

/// The houses one village atlas holds: the colony house, and a cottage for each full-size
/// companion after the first. A mini lives in its big version's house, so there is never more
/// than one house per companion.
pub const VILLAGE_HOUSES: usize = formiga_core::MAX_COLONY_CREATURES;

/// Four cells across and four down: the houses by day and the keepsake tree in the top two rows,
/// and the same houses lit from inside in the two rows below. The daylit half on its own is what
/// the Home page and the colony portrait draw from.
pub const VILLAGE_ATLAS_SIZE: u32 = SHELTER_SIZE * 4;
/// The height of the daylit half of the village atlas.
pub const VILLAGE_DAY_HEIGHT: u32 = SHELTER_SIZE * 2;

/// How large each dwelling draws, in twelfths of the colony house. A companion cottage is only
/// a little smaller than the house it stands beside — small enough that the colony house is
/// plainly the main building, big enough to read as a home next to a 48px creature.
pub const MAIN_SPAN: i32 = 12;
pub const COTTAGE_SPAN: i32 = 10;

/// One cell of the village atlas.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VillageCell {
    /// The house in village slot `slot` — the colony house is slot 0 — by day or lit after dark.
    House { slot: usize, lit: bool },
    /// The keepsake tree both ends of the village are drawn from.
    Tree,
}

/// Whose house it is, as the house shows it: the colours of the curtain its resident hangs in the
/// doorway, and which side the curtain is tied back to. Read from the resident's own colours, so a
/// house keeps its resident's look wherever in the village it stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResidentMark {
    pub cloth: Rgba,
    pub fold: Rgba,
    pub tie: Rgba,
    pub tied_left: bool,
}

impl ResidentMark {
    pub fn of(creature: &formiga_core::Creature) -> Self {
        let palette = crate::palette_for(&creature.appearance);
        Self {
            cloth: palette.coat,
            fold: palette.shadow,
            tie: palette.accent,
            tied_left: creature.behavior_seed[7] & 1 == 0,
        }
    }

    /// The mark for each house slot in the village, from the full-size companion who keeps it,
    /// with the cottages in the order the person at the desk arranged them.
    pub fn for_village(
        creatures: &[formiga_core::Creature],
        cottage_order: &[formiga_core::CreatureId],
    ) -> [Option<Self>; VILLAGE_HOUSES] {
        let mut marks = [None; VILLAGE_HOUSES];
        let owners = formiga_core::house_owners(creatures, cottage_order);
        for (mark, owner) in marks.iter_mut().zip(owners.as_slice()) {
            *mark = creatures
                .iter()
                .find(|creature| creature.id == *owner)
                .map(Self::of);
        }
        marks
    }
}

pub struct ShelterRenderer;

impl ShelterRenderer {
    pub fn render(genome: &ShelterGenome) -> Canvas {
        Self::render_with_decorations(genome, &[])
    }

    pub fn render_with_decorations(
        genome: &ShelterGenome,
        decorations: &[ShelterDecorationKind],
    ) -> Canvas {
        let mut canvas = Canvas::new(SHELTER_SIZE, SHELTER_SIZE);
        draw_dwelling(
            &mut canvas,
            genome,
            decorations,
            32,
            61,
            MAIN_SPAN,
            None,
            false,
        );
        canvas
    }

    /// Where a cell's top-left sits in the village atlas, in pixels.
    pub fn village_cell(cell: VillageCell) -> (u32, u32) {
        let index = match cell {
            VillageCell::House { slot, lit } => {
                slot.min(VILLAGE_HOUSES - 1) + if lit { 8 } else { 0 }
            }
            VillageCell::Tree => VILLAGE_HOUSES,
        } as u32;
        (index % 4 * SHELTER_SIZE, index / 4 * SHELTER_SIZE)
    }

    /// Every house in the village and the keepsake tree, in one cached texture: the colony house
    /// with the decorations it has earned, a cottage in each later slot, each hung with its
    /// resident's curtain, and — with `after_dark` — the same houses lit from inside in the rows
    /// below. Without it the texture is only the daylit half.
    pub fn render_village(
        genome: &ShelterGenome,
        decorations: &[ShelterDecorationKind],
        marks: &[Option<ResidentMark>],
        after_dark: bool,
    ) -> Canvas {
        let height = if after_dark {
            VILLAGE_ATLAS_SIZE
        } else {
            VILLAGE_DAY_HEIGHT
        };
        let mut canvas = Canvas::new(VILLAGE_ATLAS_SIZE, height);
        // Each cell is drawn into its own cell-sized tile first. Art that would run past a cell
        // is clipped exactly as it is for a lone shelter, never bleeding into a neighbour. Only
        // the colony house carries the decorations it earned over time.
        for lit in [false, true] {
            if lit && !after_dark {
                continue;
            }
            for slot in 0..VILLAGE_HOUSES {
                let (span, decorations) = if slot == 0 {
                    (MAIN_SPAN, decorations)
                } else {
                    (COTTAGE_SPAN, &[][..])
                };
                let mark = marks.get(slot).copied().flatten();
                let mut tile = Canvas::new(SHELTER_SIZE, SHELTER_SIZE);
                draw_dwelling(&mut tile, genome, decorations, 32, 61, span, mark, lit);
                let (x, y) = Self::village_cell(VillageCell::House { slot, lit });
                blit_cell(&mut canvas, &tile, x as i32, y as i32);
            }
        }
        let mut tile = Canvas::new(SHELTER_SIZE, SHELTER_SIZE);
        crate::tree::draw_tree(&mut tile, genome, 32, 61);
        let (x, y) = Self::village_cell(VillageCell::Tree);
        blit_cell(&mut canvas, &tile, x as i32, y as i32);
        canvas
    }
}

/// Copies one cell-sized tile into the atlas, leaving whatever is already under its empty pixels.
fn blit_cell(canvas: &mut Canvas, tile: &Canvas, origin_x: i32, origin_y: i32) {
    for y in 0..SHELTER_SIZE as i32 {
        for x in 0..SHELTER_SIZE as i32 {
            let pixel = tile.get(x, y);
            if pixel.a > 0 {
                canvas.set(origin_x + x, origin_y + y, pixel);
            }
        }
    }
}

/// The drawn size of a dwelling at `span`, in shelter pixels. Cottages keep the colony house's
/// own proportions, so a mini's home is the same house seen smaller rather than a different one.
fn dwelling_size(genome: &ShelterGenome, span: i32) -> (i32, i32) {
    (
        (i32::from(genome.width).clamp(34, 42) * span / MAIN_SPAN).max(16),
        (i32::from(genome.height).clamp(27, 36) * span / MAIN_SPAN).max(13),
    )
}

/// Draws one dwelling into `canvas`, centred on `cx` and standing on `bottom`. `span` scales
/// the house in twelfths of the colony house: see `MAIN_SPAN` and its companions.
#[allow(clippy::too_many_arguments)]
fn draw_dwelling(
    canvas: &mut Canvas,
    genome: &ShelterGenome,
    decorations: &[ShelterDecorationKind],
    cx: i32,
    bottom: i32,
    span: i32,
    mark: Option<ResidentMark>,
    lit: bool,
) {
    let palette = PALETTES[genome.palette_index as usize % PALETTES.len()];
    let accent = PALETTES[genome.accent_index as usize % PALETTES.len()];
    let (width, height) = dwelling_size(genome, span);
    // Style details scale with the dwelling, so a mini's house keeps the proportions of the
    // colony house it matches. At full span this resolves to the original constants exactly.
    let unit = |value: i32| (value * span / MAIN_SPAN).max(1);

    // A single-pixel ground shadow keeps every generated shelter readable on bright desktops.
    canvas.fill_ellipse(
        cx,
        bottom,
        width / 2 + unit(2),
        unit(3),
        Rgba::new(20, 24, 26, 95),
    );
    houses::draw_house(
        canvas,
        houses::House {
            style: genome.style,
            cx,
            bottom,
            width,
            height,
            span,
            seed: genome.detail_seed,
            lit,
            mark,
        },
        houses::Materials::for_style(genome.style, palette, accent),
    );
    let frame = ShelterFrame::resolve(genome, cx, bottom, span);
    for kind in decorations
        .iter()
        .copied()
        .take(formiga_core::MAX_SHELTER_DECORATIONS)
    {
        draw_decoration(
            canvas,
            kind,
            palette.outline,
            palette.coat,
            accent.accent,
            accent.highlight,
            genome,
            frame,
            lit,
        );
    }
}

/// Where a decoration may attach on this shelter. Every style resolves the same six anchors
/// from its own silhouette, so an addition lands on a real roof, wall, or ground line instead
/// of a shared guess at the middle of the canvas.
#[derive(Clone, Copy)]
struct ShelterFrame {
    cx: i32,
    peak_y: i32,
    eave_y: i32,
    eave_half: i32,
    wall_y: i32,
    wall_half: i32,
    ground_y: i32,
    ground_half: i32,
}

impl ShelterFrame {
    fn resolve(genome: &ShelterGenome, cx: i32, bottom: i32, span: i32) -> Self {
        let (width, height) = dwelling_size(genome, span);
        let top = bottom - height;
        let half = width / 2;
        let unit = |value: i32| (value * span / MAIN_SPAN).max(1);
        match genome.style {
            ShelterStyle::LeafTent => Self {
                cx,
                peak_y: top,
                eave_y: top + height * 6 / 10,
                eave_half: (half + unit(2)) * 6 / 10,
                wall_y: top + height * 7 / 10,
                wall_half: (half + unit(2)) * 7 / 10,
                ground_y: bottom - unit(2),
                ground_half: half + unit(2),
            },
            ShelterStyle::MushroomHut => {
                // Under the rim of the cap, across the front of the stem.
                let cap_bottom = top + height * 11 / 20;
                let stem_half = width * 7 / 20;
                Self {
                    cx,
                    peak_y: top - unit(1),
                    eave_y: cap_bottom + unit(2),
                    eave_half: stem_half + unit(1),
                    wall_y: (cap_bottom + bottom) / 2,
                    wall_half: stem_half,
                    ground_y: bottom,
                    ground_half: stem_half + unit(2),
                }
            }
            ShelterStyle::CushionDen => {
                // Across the lower band of the blanket, and on the cushion stacks below it.
                let door_height = (height / 3).clamp(6, height / 2);
                let roof_bottom = bottom - door_height - unit(2);
                Self {
                    cx,
                    peak_y: top,
                    eave_y: roof_bottom - unit(4),
                    eave_half: half - unit(2),
                    wall_y: roof_bottom + unit(4),
                    wall_half: half - unit(2),
                    ground_y: bottom - unit(1),
                    ground_half: half + unit(2),
                }
            }
            ShelterStyle::PaperHouse => {
                let eave = top + unit(12);
                Self {
                    cx,
                    peak_y: top - unit(1),
                    eave_y: eave + unit(2),
                    eave_half: half,
                    wall_y: (eave + bottom) / 2,
                    wall_half: half - unit(2),
                    ground_y: bottom,
                    ground_half: half,
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_decoration(
    canvas: &mut Canvas,
    kind: ShelterDecorationKind,
    outline: Rgba,
    coat: Rgba,
    accent: Rgba,
    highlight: Rgba,
    genome: &ShelterGenome,
    frame: ShelterFrame,
    lit: bool,
) {
    // Mounted pieces take their position from the shelter itself. Only the two ground pieces
    // shift, and by a single pixel, so a home still varies without looking scattered.
    let drift = ((genome.detail_seed >> (kind.index() * 7)) & 0x1) as i32;
    match kind {
        ShelterDecorationKind::Leaf => {
            let x = frame.cx - frame.wall_half + 3;
            let y = frame.wall_y - 4;
            canvas.line(x, y + 9, x + 2, y, 1, outline);
            canvas.fill_ellipse(x - 1, y + 2, 3, 2, coat);
            canvas.fill_ellipse(x + 3, y + 6, 3, 2, highlight);
            canvas.line(x, y + 3, x + 4, y + 6, 1, accent);
        }
        ShelterDecorationKind::Banner => {
            // Strung under the eaves, spanning the roof it actually hangs from.
            let span = (frame.eave_half - 2).max(6);
            let y = frame.eave_y;
            canvas.line(frame.cx - span, y, frame.cx + span, y, 1, outline);
            for (index, offset) in (-1..=1).enumerate() {
                let x = frame.cx + offset * (span - 3) - 2;
                fill_triangle(
                    canvas,
                    x + 2,
                    y + 1,
                    x,
                    y + 4,
                    x + 5,
                    y + 4,
                    if index % 2 == 0 { accent } else { highlight },
                );
            }
        }
        ShelterDecorationKind::Stone => {
            let x = frame.cx - frame.ground_half - 2 - drift;
            let y = frame.ground_y - 2;
            canvas.fill_ellipse(x, y, 4, 3, outline);
            canvas.fill_ellipse(x, y - 1, 3, 2, coat);
            canvas.set(x + 2, y - 2, highlight);
        }
        ShelterDecorationKind::Flower => {
            let x = frame.cx + frame.ground_half + 2 + drift;
            let base = frame.ground_y - 1;
            canvas.line(x, base, x, base - 9, 1, coat);
            canvas.fill_ellipse(x - 2, base - 10, 3, 2, accent);
            canvas.fill_ellipse(x + 2, base - 10, 3, 2, accent);
            canvas.fill_ellipse(x, base - 13, 2, 3, highlight);
            canvas.set(x, base - 10, outline);
        }
        ShelterDecorationKind::Lamp => {
            // Bracketed onto the wall face, just under the eaves.
            let x = frame.cx + frame.wall_half - 1;
            let y = frame.wall_y - 3;
            canvas.line(x - 5, y + 5, x, y + 5, 1, outline);
            canvas.line(x, y + 5, x, y + 3, 1, outline);
            canvas.fill_ellipse(x, y, 3, 4, outline);
            if lit {
                // Lit after dark, like the windows.
                canvas.fill_ellipse(x, y, 2, 3, Rgba::new(255, 206, 118, 255));
                canvas.set(x, y, Rgba::new(255, 236, 178, 255));
            } else {
                canvas.fill_ellipse(x, y, 2, 3, highlight);
                canvas.set(x, y, accent);
            }
        }
        ShelterDecorationKind::RoofOrnament => {
            let y = frame.peak_y - 4;
            canvas.line(frame.cx, frame.peak_y + 2, frame.cx, y, 1, outline);
            canvas.line(frame.cx - 3, y, frame.cx + 3, y, 1, accent);
            canvas.line(frame.cx, y - 3, frame.cx, y + 3, 1, accent);
            canvas.line(frame.cx - 2, y - 2, frame.cx + 2, y + 2, 1, highlight);
            canvas.line(frame.cx - 2, y + 2, frame.cx + 2, y - 2, 1, highlight);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn fill_triangle(
    canvas: &mut Canvas,
    top_x: i32,
    top_y: i32,
    left_x: i32,
    bottom_y: i32,
    right_x: i32,
    _right_y: i32,
    color: Rgba,
) {
    let height = (bottom_y - top_y).max(1);
    for y in top_y..=bottom_y {
        let progress = (y - top_y) as f32 / height as f32;
        let start = (top_x as f32 + (left_x - top_x) as f32 * progress).round() as i32;
        let end = (top_x as f32 + (right_x - top_x) as f32 * progress).round() as i32;
        canvas.fill_rect(start, y, end - start + 1, 1, color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_shelter_style_is_deterministic_and_inside_the_canvas() {
        for (index, style) in [
            ShelterStyle::LeafTent,
            ShelterStyle::MushroomHut,
            ShelterStyle::CushionDen,
            ShelterStyle::PaperHouse,
        ]
        .into_iter()
        .enumerate()
        {
            let genome = ShelterGenome {
                style,
                palette_index: index as u8,
                accent_index: index as u8 + 3,
                width: 38,
                height: 32,
                detail_seed: 91,
            };
            let first = ShelterRenderer::render(&genome);
            let second = ShelterRenderer::render(&genome);
            assert_eq!(first, second);
            assert!(first.alpha_bounds().is_some());
            assert_eq!(first.width(), SHELTER_SIZE);
            assert_eq!(first.height(), SHELTER_SIZE);
        }
    }

    fn mark(seed: u8) -> ResidentMark {
        ResidentMark {
            cloth: Rgba::new(200, 60 + seed, 90, 255),
            fold: Rgba::new(150, 40 + seed, 70, 255),
            tie: Rgba::new(250, 220, 90 + seed, 255),
            tied_left: seed.is_multiple_of(2),
        }
    }

    #[test]
    fn village_cells_match_their_own_dwelling_and_never_bleed_into_a_neighbour() {
        for style in [
            ShelterStyle::LeafTent,
            ShelterStyle::MushroomHut,
            ShelterStyle::CushionDen,
            ShelterStyle::PaperHouse,
        ] {
            let genome = ShelterGenome {
                style,
                palette_index: 2,
                accent_index: 5,
                // The widest, tallest house is the one whose art can reach past its cell.
                width: 42,
                height: 36,
                detail_seed: 0xfeed_face_1234_5678,
            };
            let marks: Vec<Option<ResidentMark>> = (0..VILLAGE_HOUSES as u8)
                .map(|seed| Some(mark(seed)))
                .collect();
            let village =
                ShelterRenderer::render_village(&genome, &ShelterDecorationKind::ALL, &marks, true);
            // One 256x256 texture however full the village: every house by day and after dark,
            // and the tree.
            assert_eq!(VILLAGE_ATLAS_SIZE, 256);
            assert_eq!(village.width(), VILLAGE_ATLAS_SIZE);
            assert_eq!(village.height(), VILLAGE_ATLAS_SIZE);
            for lit in [false, true] {
                for (slot, mark) in marks.iter().copied().enumerate() {
                    let (span, decorations) = if slot == 0 {
                        (MAIN_SPAN, &ShelterDecorationKind::ALL[..])
                    } else {
                        (COTTAGE_SPAN, &[][..])
                    };
                    let mut tile = Canvas::new(SHELTER_SIZE, SHELTER_SIZE);
                    draw_dwelling(&mut tile, &genome, decorations, 32, 61, span, mark, lit);
                    assert!(tile.alpha_bounds().is_some());
                    let (x0, y0) = ShelterRenderer::village_cell(VillageCell::House { slot, lit });
                    for y in 0..SHELTER_SIZE as i32 {
                        for x in 0..SHELTER_SIZE as i32 {
                            assert_eq!(
                                village.get(x0 as i32 + x, y0 as i32 + y),
                                tile.get(x, y),
                                "{style:?} slot {slot} lit {lit} differs at ({x}, {y})"
                            );
                        }
                    }
                }
            }
            // The fourth cell of the second row holds the keepsake tree, and nothing else.
            let mut tree = Canvas::new(SHELTER_SIZE, SHELTER_SIZE);
            crate::tree::draw_tree(&mut tree, &genome, 32, 61);
            let (tx, ty) = ShelterRenderer::village_cell(VillageCell::Tree);
            for y in 0..SHELTER_SIZE as i32 {
                for x in 0..SHELTER_SIZE as i32 {
                    assert_eq!(
                        village.get(tx as i32 + x, ty as i32 + y),
                        tree.get(x, y),
                        "{style:?} differs from the tree at ({x}, {y})"
                    );
                }
            }
            // The daylit half is exactly the top of the whole.
            let day = ShelterRenderer::render_village(
                &genome,
                &ShelterDecorationKind::ALL,
                &marks,
                false,
            );
            assert_eq!(day.height(), VILLAGE_DAY_HEIGHT);
            for y in 0..VILLAGE_DAY_HEIGHT as i32 {
                for x in 0..VILLAGE_ATLAS_SIZE as i32 {
                    assert_eq!(day.get(x, y), village.get(x, y));
                }
            }
            // Without anyone's mark, the colony house cell is exactly the standalone shelter.
            let unmarked =
                ShelterRenderer::render_village(&genome, &ShelterDecorationKind::ALL, &[], false);
            let alone =
                ShelterRenderer::render_with_decorations(&genome, &ShelterDecorationKind::ALL);
            for y in 0..SHELTER_SIZE as i32 {
                for x in 0..SHELTER_SIZE as i32 {
                    assert_eq!(unmarked.get(x, y), alone.get(x, y));
                }
            }
        }
    }

    /// A resident's curtain is hung in its own doorway and nowhere else, and after dark a house
    /// changes only where light comes out of it: the doorway and the windows.
    #[test]
    fn a_curtain_hangs_in_the_doorway_and_the_night_only_lights_what_is_lit() {
        for style in [
            ShelterStyle::LeafTent,
            ShelterStyle::MushroomHut,
            ShelterStyle::CushionDen,
            ShelterStyle::PaperHouse,
        ] {
            for span in [MAIN_SPAN, COTTAGE_SPAN] {
                let genome = ShelterGenome {
                    style,
                    palette_index: 4,
                    accent_index: 9,
                    width: 38,
                    height: 32,
                    detail_seed: 0x0123_4567_89ab_cdef,
                };
                let draw = |mark: Option<ResidentMark>, lit: bool| {
                    let mut tile = Canvas::new(SHELTER_SIZE, SHELTER_SIZE);
                    draw_dwelling(&mut tile, &genome, &[], 32, 61, span, mark, lit);
                    tile
                };
                let plain = draw(None, false);
                let hung = draw(Some(mark(3)), false);
                let (door_width, door_height) = houses::House {
                    style,
                    cx: 32,
                    bottom: 61,
                    width: dwelling_size(&genome, span).0,
                    height: dwelling_size(&genome, span).1,
                    span,
                    seed: 0,
                    lit: false,
                    mark: None,
                }
                .door();
                let mut changed = 0;
                for y in 0..SHELTER_SIZE as i32 {
                    for x in 0..SHELTER_SIZE as i32 {
                        if plain.get(x, y) != hung.get(x, y) {
                            changed += 1;
                            assert!(
                                (x - 32).abs() <= door_width / 2 && y > 61 - door_height,
                                "{style:?} {span}: the curtain reached ({x}, {y})"
                            );
                        }
                    }
                }
                assert!(changed >= 6, "{style:?} {span}: no curtain to be seen");
                let lit = draw(None, true);
                assert_ne!(lit, plain, "{style:?} {span}: nothing lit up");
                for y in 0..SHELTER_SIZE as i32 {
                    for x in 0..SHELTER_SIZE as i32 {
                        if plain.get(x, y) != lit.get(x, y) {
                            let glow = lit.get(x, y);
                            assert!(
                                glow.r >= 200 && glow.g >= 150,
                                "{style:?} {span}: ({x}, {y}) changed to {glow:?}, not lamplight"
                            );
                        }
                    }
                }
            }
        }
    }

    /// Each house carries the mark of the full-size companion who keeps it, and a mini, who lives
    /// in its big version's house, adds no house or mark of its own.
    #[test]
    fn every_house_carries_the_mark_of_whoever_keeps_it() {
        use formiga_core::{CreatureRole, DesktopSnapshot, World};
        let mut creatures: Vec<_> = (0..4_u8)
            .map(|index| {
                let mut creature = World::preview_adult(
                    [index + 10; 32],
                    time::OffsetDateTime::UNIX_EPOCH,
                    &DesktopSnapshot::default(),
                );
                creature.id = u64::from(index) + 1;
                creature.colony_order = index;
                creature
            })
            .collect();
        creatures[2].role = CreatureRole::Mini {
            parent_id: creatures[0].id,
        };
        let marks = ResidentMark::for_village(&creatures, &[]);
        assert_eq!(marks[0], Some(ResidentMark::of(&creatures[0])));
        assert_eq!(marks[1], Some(ResidentMark::of(&creatures[1])));
        assert_eq!(marks[2], Some(ResidentMark::of(&creatures[3])));
        assert!(marks[3..].iter().all(Option::is_none));

        // Rearranged, each curtain moves with its keeper; the founder keeps the colony house
        // whatever the order says, and a mini is never given a house of its own.
        let order = [creatures[3].id, creatures[0].id, creatures[2].id];
        let marks = ResidentMark::for_village(&creatures, &order);
        assert_eq!(marks[0], Some(ResidentMark::of(&creatures[0])));
        assert_eq!(marks[1], Some(ResidentMark::of(&creatures[3])));
        assert_eq!(marks[2], Some(ResidentMark::of(&creatures[1])));
        assert!(marks[3..].iter().all(Option::is_none));
    }

    /// Each named palette paints the village its own way and never reshapes it: every style of
    /// house, both trees and the lit windows fill exactly the pixels they fill in the colony's own
    /// colours, and only the colours differ.
    #[test]
    fn every_named_palette_paints_the_same_village_in_its_own_colours() {
        for style in 0..4_u8 {
            let mut seed = [12_u8; 32];
            seed[1] = style;
            let mut home = formiga_core::ColonyHome::from_seed(seed, None, None, None);
            let own = ShelterRenderer::render_village(&home.drawn_shelter(), &[], &[], true);
            let mut seen = vec![own.clone()];
            for palette in formiga_core::VillagePalette::ALL {
                home.palette = Some(palette);
                let painted =
                    ShelterRenderer::render_village(&home.drawn_shelter(), &[], &[], true);
                for y in 0..painted.height() as i32 {
                    for x in 0..painted.width() as i32 {
                        assert_eq!(
                            painted.get(x, y).a,
                            own.get(x, y).a,
                            "{palette:?} reshapes style {style} at {x},{y}"
                        );
                    }
                }
                assert!(!seen.contains(&painted), "{palette:?} paints nothing new");
                seen.push(painted);
            }
        }
    }

    /// The village spaces its lots by `DwellingKind::width`, so a house that grew past its own
    /// footprint would start touching the neighbour the simulation thinks it is clear of. Every
    /// genome the generator can produce has to stay inside the ground its kind claims, and a
    /// cottage has to stay plainly smaller than the colony house it matches.
    #[test]
    fn every_dwelling_stays_inside_the_footprint_the_village_reserves_for_it() {
        use formiga_core::DwellingKind;
        for style in [
            ShelterStyle::LeafTent,
            ShelterStyle::MushroomHut,
            ShelterStyle::CushionDen,
            ShelterStyle::PaperHouse,
        ] {
            for width in [34_u8, 38, 42] {
                for height in [27_u8, 32, 36] {
                    let genome = ShelterGenome {
                        style,
                        palette_index: 1,
                        accent_index: 4,
                        width,
                        height,
                        detail_seed: 0x0bad_f00d_dead_beef,
                    };
                    let mut previous = f32::MAX;
                    for (span, kind) in [
                        (MAIN_SPAN, DwellingKind::Main),
                        (COTTAGE_SPAN, DwellingKind::Cottage),
                    ] {
                        let mut tile = Canvas::new(SHELTER_SIZE, SHELTER_SIZE);
                        draw_dwelling(&mut tile, &genome, &[], 32, 61, span, Some(mark(1)), true);
                        let bounds = tile.alpha_bounds().expect("a dwelling is drawn");
                        let drawn = (bounds.2 - bounds.0 + 1) as f32;
                        assert!(
                            drawn <= kind.width(),
                            "{style:?} {width}x{height}: {kind:?} draws {drawn} wide but the \
                             village reserves {}",
                            kind.width()
                        );
                        assert!(
                            drawn < previous,
                            "{style:?} {width}x{height}: {kind:?} is not smaller than the \
                             dwelling above it"
                        );
                        previous = drawn;
                    }
                }
            }
        }
    }

    /// The village keeps resting companions off every doorway by a fixed number of pixels, which
    /// only works if no genome can draw a doorway wider than that number assumes. Seven pixels
    /// either side of a dwelling's middle is what `habitat::tests::WIDEST_DOOR_HALF` promises.
    #[test]
    fn no_dwelling_draws_a_doorway_wider_than_the_village_expects() {
        const WIDEST_DOOR_HALF: i32 = 7;
        for style in [
            ShelterStyle::LeafTent,
            ShelterStyle::MushroomHut,
            ShelterStyle::CushionDen,
            ShelterStyle::PaperHouse,
        ] {
            for width in 34..=42_u8 {
                for height in 27..=36_u8 {
                    for span in [MAIN_SPAN, COTTAGE_SPAN] {
                        let genome = ShelterGenome {
                            style,
                            palette_index: 1,
                            accent_index: 4,
                            width,
                            height,
                            detail_seed: 0x0bad_f00d_dead_beef,
                        };
                        // The doorway is the one hole every style punches in the same fixed
                        // near-black, so its own pixels say exactly how wide it is. The dark
                        // frame drawn around it adds two more either side.
                        let mut tile = Canvas::new(SHELTER_SIZE, SHELTER_SIZE);
                        draw_dwelling(&mut tile, &genome, &[], 32, 61, span, None, false);
                        let doorway = Rgba::new(25, 23, 31, 255);
                        let open = |x: i32| (55..=60).any(|y| tile.get(x, y) == doorway);
                        let reach = (0..=WIDEST_DOOR_HALF + 6)
                            .filter(|offset| open(32 + offset) || open(32 - offset))
                            .max()
                            .unwrap_or(0);
                        assert!(
                            reach + 2 <= WIDEST_DOOR_HALF,
                            "{style:?} {width}x{height} span {span} opens a doorway {reach} \
                             pixels from its middle, {} with its frame",
                            reach + 2
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn all_six_decorations_are_baked_deterministically_into_one_shelter_canvas() {
        let genome = ShelterGenome {
            style: ShelterStyle::PaperHouse,
            palette_index: 3,
            accent_index: 8,
            width: 38,
            height: 32,
            detail_seed: 0x1234_5678,
        };
        let undecorated = ShelterRenderer::render(&genome);
        let decorated =
            ShelterRenderer::render_with_decorations(&genome, &ShelterDecorationKind::ALL);
        assert_eq!(
            decorated,
            ShelterRenderer::render_with_decorations(&genome, &ShelterDecorationKind::ALL)
        );
        assert_ne!(decorated, undecorated);
        assert_eq!(decorated.width(), SHELTER_SIZE);
        assert_eq!(decorated.height(), SHELTER_SIZE);
        assert!(decorated.alpha_bounds().is_some());
    }
}
