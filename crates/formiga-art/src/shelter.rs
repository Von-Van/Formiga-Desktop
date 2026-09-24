use crate::{Canvas, PALETTES, Rgba};
use formiga_core::{ShelterDecorationKind, ShelterGenome, ShelterStyle};

mod decorations;
mod houses;

/// One cell of the village atlas: room for the largest house drawn a quarter larger than it was
/// until 0.61.0, standing on a ground line three pixels above the cell's foot.
pub const SHELTER_SIZE: u32 = 80;

/// Where a dwelling stands in its cell: across the middle, on the ground line the tree shares.
const CELL_CENTRE: i32 = SHELTER_SIZE as i32 / 2;
const CELL_GROUND: i32 = SHELTER_SIZE as i32 - 3;

/// The houses one village atlas holds: the colony house, and a cottage for each full-size
/// companion after the first. A mini lives in its big version's house, so there is never more
/// than one house per companion.
pub const VILLAGE_HOUSES: usize = formiga_core::MAX_COLONY_CREATURES;

/// Seven cells across, a house for each companion and the tree, and four down. The top row is
/// every house by day and the keepsake tree; the second row the same houses by day with somebody
/// at home; the third and fourth rows the same again lit from inside after dark. The daylit half
/// on its own is what the Home page and the colony portrait draw from. There were eight columns,
/// one of them never drawn in, until the cells grew in 0.61.0 and 512 pixels stopped being a
/// round width worth keeping an empty column for.
pub const VILLAGE_ATLAS_COLUMNS: u32 = VILLAGE_HOUSES as u32 + 1;
pub const VILLAGE_ATLAS_WIDTH: u32 = SHELTER_SIZE * VILLAGE_ATLAS_COLUMNS;
pub const VILLAGE_ATLAS_HEIGHT: u32 = SHELTER_SIZE * 4;
/// The height of the daylit half of the village atlas.
pub const VILLAGE_DAY_HEIGHT: u32 = SHELTER_SIZE * 2;

/// The span every drawing's proportions are written at: the colony house as it was drawn until
/// 0.61.0. A dwelling at any other span is the same drawing scaled by `span / DRAWN_SPAN`.
pub const DRAWN_SPAN: i32 = 24;

/// How large each dwelling draws, in twenty-fourths of the colony house as it was drawn until
/// 0.61.0: both a quarter larger now, beside the same 48px creatures, and a companion cottage
/// still five-sixths of the colony house — small enough that the colony house is plainly the
/// main building, big enough to read as a home.
pub const MAIN_SPAN: i32 = 30;
pub const COTTAGE_SPAN: i32 = 25;

/// One cell of the village atlas.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VillageCell {
    /// The house in village slot `slot` — the colony house is slot 0 — by day or lit after dark,
    /// and with its resident at home behind a drawn curtain or not.
    House {
        slot: usize,
        lit: bool,
        occupied: bool,
    },
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

/// Everything about how the village's houses look, in the order they stand: each house's type,
/// the decorations it wears, and whose curtain hangs in its door. Two villages that compare equal
/// draw the same atlas, so this is what a cached village texture is keyed on.
#[derive(Clone, Debug, PartialEq)]
pub struct VillageLook {
    pub genome: ShelterGenome,
    pub styles: [ShelterStyle; VILLAGE_HOUSES],
    pub decorations: [Vec<ShelterDecorationKind>; VILLAGE_HOUSES],
    pub marks: [Option<ResidentMark>; VILLAGE_HOUSES],
}

impl VillageLook {
    /// The village as this colony has it: painted in its palette, each house built as its own
    /// type, dressed as it was dressed, and hung with its keeper's curtain.
    pub fn of(home: &formiga_core::ColonyHome, creatures: &[formiga_core::Creature]) -> Self {
        Self {
            genome: home.drawn_shelter(),
            styles: home.house_style_list(creatures),
            decorations: home.house_decoration_list(creatures),
            marks: ResidentMark::for_village(creatures, &home.cottage_order),
        }
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
        draw_dwelling(&mut canvas, genome, decorations, Dwelling::main());
        canvas
    }

    /// Where a cell's top-left sits in the village atlas, in pixels.
    pub fn village_cell(cell: VillageCell) -> (u32, u32) {
        let (column, row) = match cell {
            VillageCell::House {
                slot,
                lit,
                occupied,
            } => (
                slot.min(VILLAGE_HOUSES - 1),
                usize::from(occupied) + if lit { 2 } else { 0 },
            ),
            VillageCell::Tree => (VILLAGE_HOUSES, 0),
        };
        (column as u32 * SHELTER_SIZE, row as u32 * SHELTER_SIZE)
    }

    /// Every house in the village by day, with and without somebody at home, and — with
    /// `after_dark` — the same houses lit from inside in the rows below, with the keepsake tree.
    /// Without it the texture is only the daylit half.
    pub fn render_look(look: &VillageLook, after_dark: bool) -> Canvas {
        Self::render_village(
            &look.genome,
            &look.decorations,
            &look.marks,
            &look.styles,
            after_dark,
        )
    }

    /// Every house by day with nobody at home, and the keepsake tree: the atlas's top row, which
    /// is all the Home page draws, as a texture of its own a quarter the size of the whole.
    pub fn render_home_row(look: &VillageLook) -> Canvas {
        let daylit = Self::render_look(look, false);
        let mut row = Canvas::new(VILLAGE_ATLAS_WIDTH, SHELTER_SIZE);
        for y in 0..SHELTER_SIZE as i32 {
            for x in 0..VILLAGE_ATLAS_WIDTH as i32 {
                row.set(x, y, daylit.get(x, y));
            }
        }
        row
    }

    /// The village atlas from its parts. `decorations`, `marks` and `styles` are by house slot; a
    /// slot any of them leaves out is bare, unmarked, and the colony's own type.
    pub fn render_village(
        genome: &ShelterGenome,
        decorations: &[Vec<ShelterDecorationKind>],
        marks: &[Option<ResidentMark>],
        styles: &[ShelterStyle],
        after_dark: bool,
    ) -> Canvas {
        let height = if after_dark {
            VILLAGE_ATLAS_HEIGHT
        } else {
            VILLAGE_DAY_HEIGHT
        };
        let mut canvas = Canvas::new(VILLAGE_ATLAS_WIDTH, height);
        // Each cell is drawn into its own cell-sized tile first. Art that would run past a cell
        // is clipped exactly as it is for a lone shelter, never bleeding into a neighbour.
        for lit in [false, true] {
            if lit && !after_dark {
                continue;
            }
            for occupied in [false, true] {
                for slot in 0..VILLAGE_HOUSES {
                    let dressed = decorations.get(slot).map_or(&[][..], Vec::as_slice);
                    let house = ShelterGenome {
                        style: styles.get(slot).copied().unwrap_or(genome.style),
                        ..*genome
                    };
                    let mut tile = Canvas::new(SHELTER_SIZE, SHELTER_SIZE);
                    let dwelling = Dwelling {
                        mark: marks.get(slot).copied().flatten(),
                        lit,
                        occupied,
                        ..Dwelling::in_slot(slot)
                    };
                    draw_dwelling(&mut tile, &house, dressed, dwelling);
                    let (x, y) = Self::village_cell(VillageCell::House {
                        slot,
                        lit,
                        occupied,
                    });
                    blit_cell(&mut canvas, &tile, x as i32, y as i32);
                }
            }
        }
        // The tree keeps its own size and its own cell, and stands in the middle of its village
        // cell on the ground line the houses share.
        let tree = crate::KeepsakeTreeRenderer::render(genome);
        let (x, y) = Self::village_cell(VillageCell::Tree);
        let (inset_x, inset_y) = crate::TREE_INSET;
        blit_cell(&mut canvas, &tree, x as i32 + inset_x, y as i32 + inset_y);
        canvas
    }

    /// How far above its ground line the top of a house of this type reaches, in shelter
    /// pixels, measured from the drawing itself: the height a companion sitting on its roof sits
    /// at. Only the middle of the house counts, so a pennant pole or a sprout does not.
    pub fn roof_height(genome: &ShelterGenome, style: ShelterStyle, colony_house: bool) -> i32 {
        let mut tile = Canvas::new(SHELTER_SIZE, SHELTER_SIZE);
        let house = ShelterGenome { style, ..*genome };
        let dwelling = if colony_house {
            Dwelling::main()
        } else {
            Dwelling::in_slot(1)
        };
        draw_dwelling(&mut tile, &house, &[], dwelling);
        (dwelling.cx - 3..=dwelling.cx + 3)
            .filter_map(|x| (0..SHELTER_SIZE as i32).find(|y| tile.get(x, *y).a > 0))
            .max()
            .map_or(0, |top| dwelling.bottom - top)
    }
}

/// Copies one cell-sized tile into the atlas, leaving whatever is already under its empty pixels.
fn blit_cell(canvas: &mut Canvas, tile: &Canvas, origin_x: i32, origin_y: i32) {
    for y in 0..tile.height() as i32 {
        for x in 0..tile.width() as i32 {
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
        (i32::from(genome.width).clamp(34, 42) * span / DRAWN_SPAN).max(16),
        (i32::from(genome.height).clamp(27, 36) * span / DRAWN_SPAN).max(13),
    )
}

/// Where and how one dwelling is drawn in its cell: centred on `cx` and standing on `bottom`, at
/// `span` twelfths of the colony house, hung with its resident's curtain, lit from inside after
/// dark, and with its resident at home behind the curtain or not.
#[derive(Clone, Copy)]
struct Dwelling {
    cx: i32,
    bottom: i32,
    span: i32,
    mark: Option<ResidentMark>,
    lit: bool,
    occupied: bool,
}

impl Dwelling {
    const fn main() -> Self {
        Self {
            cx: CELL_CENTRE,
            bottom: CELL_GROUND,
            span: MAIN_SPAN,
            mark: None,
            lit: false,
            occupied: false,
        }
    }

    const fn in_slot(slot: usize) -> Self {
        Self {
            span: if slot == 0 { MAIN_SPAN } else { COTTAGE_SPAN },
            ..Self::main()
        }
    }
}

/// Draws one dwelling into `canvas`.
fn draw_dwelling(
    canvas: &mut Canvas,
    genome: &ShelterGenome,
    decorations: &[ShelterDecorationKind],
    dwelling: Dwelling,
) {
    let Dwelling {
        cx,
        bottom,
        span,
        mark,
        lit,
        occupied,
    } = dwelling;
    let palette = PALETTES[genome.palette_index as usize % PALETTES.len()];
    let accent = PALETTES[genome.accent_index as usize % PALETTES.len()];
    let (width, height) = dwelling_size(genome, span);
    // Style details scale with the dwelling, so a mini's house keeps the proportions of the
    // colony house it matches. At full span this resolves to the original constants exactly.
    let unit = |value: i32| (value * span / DRAWN_SPAN).max(1);

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
            occupied,
            mark,
        },
        houses::Materials::for_style(genome.style, palette, accent),
    );
    let frame = ShelterFrame::resolve(genome, cx, bottom, span);
    for kind in decorations
        .iter()
        .copied()
        .take(formiga_core::MAX_HOUSE_DECORATIONS)
    {
        decorations::draw(
            canvas,
            kind,
            decorations::Inks {
                outline: palette.outline,
                coat: palette.coat,
                accent: accent.accent,
                highlight: accent.highlight,
            },
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
    /// A cottage, which keeps what stands beside it a little closer in: its lot is narrower than
    /// the colony house's.
    cottage: bool,
}

impl ShelterFrame {
    fn resolve(genome: &ShelterGenome, cx: i32, bottom: i32, span: i32) -> Self {
        let (width, height) = dwelling_size(genome, span);
        let top = bottom - height;
        let half = width / 2;
        let unit = |value: i32| (value * span / DRAWN_SPAN).max(1);
        let cottage = span < MAIN_SPAN;
        match genome.style {
            ShelterStyle::Tent => Self {
                cx,
                peak_y: top,
                eave_y: top + height * 6 / 10,
                eave_half: (half + unit(2)) * 6 / 10,
                wall_y: top + height * 7 / 10,
                wall_half: (half + unit(2)) * 7 / 10,
                ground_y: bottom - unit(2),
                ground_half: half + unit(2),
                cottage,
            },
            ShelterStyle::Mushroom => {
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
                    cottage,
                }
            }
            ShelterStyle::PillowFort => {
                // Along the lower edge of the roof pillow, and across the wall pillow below it:
                // the same proportions `houses::pillow_fort` draws them in.
                let roof_bottom = top + height * 3 / 10 + unit(1);
                Self {
                    cx,
                    peak_y: top - unit(2),
                    eave_y: roof_bottom,
                    eave_half: half + unit(3),
                    wall_y: (roof_bottom + bottom) / 2,
                    wall_half: half,
                    ground_y: bottom - unit(1),
                    ground_half: half + unit(2),
                    cottage,
                }
            }
            ShelterStyle::LeafHouse => {
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
                    cottage,
                }
            }
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

    /// A dwelling drawn in the middle of its own cell at `span`.
    fn dwelling(span: i32, mark: Option<ResidentMark>, lit: bool) -> Dwelling {
        Dwelling {
            span,
            mark,
            lit,
            ..Dwelling::main()
        }
    }

    #[test]
    fn every_shelter_style_is_deterministic_and_inside_the_canvas() {
        for (index, style) in [
            ShelterStyle::Tent,
            ShelterStyle::Mushroom,
            ShelterStyle::PillowFort,
            ShelterStyle::LeafHouse,
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
            ShelterStyle::Tent,
            ShelterStyle::Mushroom,
            ShelterStyle::PillowFort,
            ShelterStyle::LeafHouse,
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
            // Every house dressed differently: one decoration per slot, chosen by the house's
            // own slot number so no two houses wear the same set.
            let dressed: Vec<Vec<ShelterDecorationKind>> = (0..VILLAGE_HOUSES)
                .map(|slot| {
                    formiga_core::DecorationSlot::ALL
                        .into_iter()
                        .filter_map(|place| {
                            ShelterDecorationKind::ALL
                                .into_iter()
                                .filter(|kind| kind.slot() == place)
                                .nth(slot % 5)
                        })
                        .collect()
                })
                .collect();
            let village = ShelterRenderer::render_village(&genome, &dressed, &marks, &[], true);
            // One 560x320 texture however full the village: every house by day and after dark,
            // with and without its resident home, and the tree.
            assert_eq!(VILLAGE_ATLAS_WIDTH, 560);
            assert_eq!(VILLAGE_ATLAS_HEIGHT, 320);
            assert_eq!(village.width(), VILLAGE_ATLAS_WIDTH);
            assert_eq!(village.height(), VILLAGE_ATLAS_HEIGHT);
            for (lit, occupied) in [(false, false), (false, true), (true, false), (true, true)] {
                for (slot, mark) in marks.iter().copied().enumerate() {
                    let span = if slot == 0 { MAIN_SPAN } else { COTTAGE_SPAN };
                    let mut tile = Canvas::new(SHELTER_SIZE, SHELTER_SIZE);
                    draw_dwelling(
                        &mut tile,
                        &genome,
                        &dressed[slot],
                        Dwelling {
                            occupied,
                            ..dwelling(span, mark, lit)
                        },
                    );
                    assert!(tile.alpha_bounds().is_some());
                    let (x0, y0) = ShelterRenderer::village_cell(VillageCell::House {
                        slot,
                        lit,
                        occupied,
                    });
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
            // The seventh cell of the top row holds the keepsake tree, and nothing else: the tree
            // in its own smaller cell, in the middle of this one and on the same ground line.
            let mut tree = Canvas::new(SHELTER_SIZE, SHELTER_SIZE);
            let (inset_x, inset_y) = crate::TREE_INSET;
            crate::tree::draw_tree(&mut tree, &genome, 32 + inset_x, 61 + inset_y);
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
            let day = ShelterRenderer::render_village(&genome, &dressed, &marks, &[], false);
            assert_eq!(day.height(), VILLAGE_DAY_HEIGHT);
            for y in 0..VILLAGE_DAY_HEIGHT as i32 {
                for x in 0..VILLAGE_ATLAS_WIDTH as i32 {
                    assert_eq!(day.get(x, y), village.get(x, y));
                }
            }
            // Without anyone's mark, the colony house cell is exactly the standalone shelter.
            let unmarked = ShelterRenderer::render_village(&genome, &dressed, &[], &[], false);
            let alone = ShelterRenderer::render_with_decorations(&genome, &dressed[0]);
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
            ShelterStyle::Tent,
            ShelterStyle::Mushroom,
            ShelterStyle::PillowFort,
            ShelterStyle::LeafHouse,
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
                    draw_dwelling(&mut tile, &genome, &[], dwelling(span, mark, lit));
                    tile
                };
                let plain = draw(None, false);
                let hung = draw(Some(mark(3)), false);
                let (door_width, door_height) = houses::House {
                    style,
                    cx: CELL_CENTRE,
                    bottom: CELL_GROUND,
                    width: dwelling_size(&genome, span).0,
                    height: dwelling_size(&genome, span).1,
                    span,
                    seed: 0,
                    lit: false,
                    occupied: false,
                    mark: None,
                }
                .door();
                let mut changed = 0;
                for y in 0..SHELTER_SIZE as i32 {
                    for x in 0..SHELTER_SIZE as i32 {
                        if plain.get(x, y) != hung.get(x, y) {
                            changed += 1;
                            assert!(
                                (x - CELL_CENTRE).abs() <= door_width / 2
                                    && y > CELL_GROUND - door_height,
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
            let own = ShelterRenderer::render_village(&home.drawn_shelter(), &[], &[], &[], true);

            let mut seen = vec![own.clone()];
            for palette in formiga_core::VillagePalette::ALL {
                home.palette = Some(palette);
                let painted =
                    ShelterRenderer::render_village(&home.drawn_shelter(), &[], &[], &[], true);
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
            ShelterStyle::Tent,
            ShelterStyle::Mushroom,
            ShelterStyle::PillowFort,
            ShelterStyle::LeafHouse,
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
                        draw_dwelling(&mut tile, &genome, &[], dwelling(span, Some(mark(1)), true));
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
            ShelterStyle::Tent,
            ShelterStyle::Mushroom,
            ShelterStyle::PillowFort,
            ShelterStyle::LeafHouse,
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
                        draw_dwelling(&mut tile, &genome, &[], dwelling(span, None, false));
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
    fn every_decoration_is_drawn_deterministically_and_changes_the_house() {
        let genome = ShelterGenome {
            style: ShelterStyle::LeafHouse,
            palette_index: 3,
            accent_index: 8,
            width: 38,
            height: 32,
            detail_seed: 0x1234_5678,
        };
        let undecorated = ShelterRenderer::render(&genome);
        let mut seen = vec![undecorated.clone()];
        for kind in ShelterDecorationKind::ALL {
            let decorated = ShelterRenderer::render_with_decorations(&genome, &[kind]);
            assert_eq!(
                decorated,
                ShelterRenderer::render_with_decorations(&genome, &[kind])
            );
            assert!(
                !seen.contains(&decorated),
                "{kind:?} draws nothing of its own"
            );
            seen.push(decorated);
        }
    }

    /// A house's decorations stay on the ground its lot claims, so a dressed cottage never reaches
    /// over into the house beside it, whichever style it is and whatever size its genome makes it.
    #[test]
    fn a_house_dressed_in_everything_stays_inside_its_own_lot() {
        use formiga_core::DwellingKind;
        for style in ShelterStyle::ALL {
            for (width, height) in [(34_u8, 27_u8), (42, 36)] {
                let genome = ShelterGenome {
                    style,
                    palette_index: 5,
                    accent_index: 2,
                    width,
                    height,
                    detail_seed: 0xdead_beef_0bad_f00d,
                };
                for (span, kind) in [
                    (MAIN_SPAN, DwellingKind::Main),
                    (COTTAGE_SPAN, DwellingKind::Cottage),
                ] {
                    for choice in 0..5 {
                        let dressed: Vec<ShelterDecorationKind> = formiga_core::DecorationSlot::ALL
                            .into_iter()
                            .filter_map(|place| {
                                ShelterDecorationKind::ALL
                                    .into_iter()
                                    .filter(|kind| kind.slot() == place)
                                    .nth(choice)
                            })
                            .collect();
                        // The six a colony could earn before were only ever hung on the colony
                        // house, and still draw exactly as they did there; they may lean a pixel
                        // or two into the gap beside it, never as far as the next house.
                        let legacy = dressed.iter().all(|kind| kind.index() < 6);
                        let slack = if legacy && span == MAIN_SPAN { 3 } else { 0 };
                        for lit in [false, true] {
                            let mut tile = Canvas::new(SHELTER_SIZE, SHELTER_SIZE);
                            draw_dwelling(&mut tile, &genome, &dressed, dwelling(span, None, lit));
                            let (x0, _, x1, _) = tile.alpha_bounds().expect("drawn");
                            let lot = kind.width() as i32 / 2 + slack;
                            assert!(
                                CELL_CENTRE - x0 as i32 <= lot && x1 as i32 - CELL_CENTRE < lot,
                                "{style:?} {width}x{height} {kind:?} dressed {dressed:?} reaches {x0}..={x1}, past its {} lot",
                                kind.width()
                            );
                        }
                    }
                }
            }
        }
    }

    /// With its resident at home a house draws the curtain across its whole doorway and lights a
    /// lamp, by day as well as by night, and nothing else about it changes.
    #[test]
    fn an_occupied_house_draws_its_curtain_and_lights_up() {
        for style in ShelterStyle::ALL {
            for span in [MAIN_SPAN, COTTAGE_SPAN] {
                let genome = ShelterGenome {
                    style,
                    palette_index: 7,
                    accent_index: 1,
                    width: 38,
                    height: 32,
                    detail_seed: 0x0123_4567_89ab_cdef,
                };
                let draw = |occupied: bool, lit: bool| {
                    let mut tile = Canvas::new(SHELTER_SIZE, SHELTER_SIZE);
                    draw_dwelling(
                        &mut tile,
                        &genome,
                        &[],
                        Dwelling {
                            occupied,
                            ..dwelling(span, Some(mark(4)), lit)
                        },
                    );
                    tile
                };
                for lit in [false, true] {
                    let empty = draw(false, lit);
                    let home = draw(true, lit);
                    assert_ne!(empty, home, "{style:?} {span} {lit}: nobody can tell");
                    let count = |tile: &Canvas, colors: &[Rgba]| {
                        (0..SHELTER_SIZE as i32)
                            .flat_map(|y| (0..SHELTER_SIZE as i32).map(move |x| (x, y)))
                            .filter(|(x, y)| colors.contains(&tile.get(*x, *y)))
                            .count()
                    };
                    let curtain = [mark(4).cloth, mark(4).fold];
                    assert!(
                        count(&home, &curtain) > count(&empty, &curtain),
                        "{style:?} {span} {lit}: the curtain is not drawn across"
                    );
                    // Drawn right across: no bare doorway shows anywhere.
                    assert_eq!(
                        count(&home, &[houses::DOORWAY]),
                        0,
                        "{style:?} {span} {lit}: the doorway still shows"
                    );
                    // The silhouette never changes: only what is inside the doorway and the
                    // windows does.
                    for y in 0..SHELTER_SIZE as i32 {
                        for x in 0..SHELTER_SIZE as i32 {
                            assert_eq!(empty.get(x, y).a, home.get(x, y).a, "{style:?} {x},{y}");
                        }
                    }
                }
            }
        }
    }

    /// A companion sitting on its roof sits at the top of the house, which is somewhere a house
    /// actually reaches, lower on a cottage than on the colony house.
    #[test]
    fn a_roof_is_as_high_as_the_house_it_tops() {
        for style in ShelterStyle::ALL {
            let genome = ShelterGenome {
                style,
                palette_index: 2,
                accent_index: 6,
                width: 38,
                height: 32,
                detail_seed: 7,
            };
            let main = ShelterRenderer::roof_height(&genome, style, true);
            let cottage = ShelterRenderer::roof_height(&genome, style, false);
            // A quarter taller than the 20 to 40 they reached until 0.61.0.
            assert!((25..=50).contains(&main), "{style:?}: {main}");
            assert!(cottage < main, "{style:?}: {cottage} against {main}");
        }
    }

    /// The simulation cannot draw a house, so it works out where a roof is from the house's
    /// proportions. It has to land on the drawing: within a couple of pixels of the top of every
    /// type of house at every height a colony's houses can be, measured from the ground line
    /// three rows below the walls.
    #[test]
    fn the_simulation_seats_a_roof_sitter_on_the_drawn_roof() {
        for style in ShelterStyle::ALL {
            for height in 27..=36 {
                for width in [34, 38, 42] {
                    let genome = ShelterGenome {
                        style,
                        palette_index: 1,
                        accent_index: 4,
                        width,
                        height,
                        detail_seed: 11,
                    };
                    for colony_house in [true, false] {
                        let drawn = ShelterRenderer::roof_height(&genome, style, colony_house) + 3;
                        let reckoned =
                            formiga_core::house_roof_height(&genome, style, colony_house);
                        assert!(
                            (reckoned - drawn as f32).abs() <= 2.0,
                            "{style:?} height {height} width {width} colony house \
                             {colony_house}: drawn {drawn}, reckoned {reckoned}"
                        );
                    }
                }
            }
        }
    }
}
