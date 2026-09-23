//! The colony's sheet of things on the ground: its belongings, the hangout spots, the garden
//! patches at every stage of growing, the ornaments, and the few small things a companion holds up
//! or chases in the village — a watering can, something just picked, an apple that got away.
//!
//! Every cell is 16x16 and drawn into a tile of its own before it goes on the sheet, so nothing can
//! bleed into the cell beside it. Sixteen cells to a row.

use crate::{Canvas, PALETTES, Rgba};
use formiga_core::{
    ColonyObjectKind, GardenKind, GardenStage, GroundItem, HangoutKind, OrnamentKind, VillageProp,
};

pub const COLONY_OBJECT_SIZE: u32 = 16;
/// Cells to a row of the sheet.
pub const COLONY_OBJECT_COLUMNS: u32 = 16;

/// Where each group of cells starts on the sheet.
const HANGOUTS: u32 = ColonyObjectKind::ALL.len() as u32;
const GARDENS: u32 = HANGOUTS + HangoutKind::ALL.len() as u32;
const ORNAMENTS: u32 = GARDENS + (GardenKind::ALL.len() * GardenStage::ALL.len()) as u32;
const PROPS: u32 = ORNAMENTS + OrnamentKind::ALL.len() as u32;

/// Every cell on the sheet.
pub const COLONY_OBJECT_CELLS: u32 = PROPS + PropSprite::ALL.len() as u32;
pub const COLONY_OBJECT_ROWS: u32 = COLONY_OBJECT_CELLS.div_ceil(COLONY_OBJECT_COLUMNS);
pub const COLONY_OBJECT_ATLAS_WIDTH: u32 = COLONY_OBJECT_SIZE * COLONY_OBJECT_COLUMNS;
pub const COLONY_OBJECT_ATLAS_HEIGHT: u32 = COLONY_OBJECT_SIZE * COLONY_OBJECT_ROWS;

/// Small things a companion holds up, carries, or chases in the village.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PropSprite {
    /// Held while watering a patch, spout forward.
    WateringCan,
    /// Water falling from the spout.
    Water,
    Carrot,
    Pumpkin,
    Tomato,
    PeaPod,
    Strawberry,
    Berries,
    HerbSprig,
    /// A snack that rolled away.
    Apple,
    /// A leaf drifting down.
    Leaf,
    /// A flower just picked.
    Blossom,
    /// A little Z drifting up out of a house somebody is asleep in.
    Snore,
}

impl PropSprite {
    pub const ALL: [Self; 13] = [
        Self::WateringCan,
        Self::Water,
        Self::Carrot,
        Self::Pumpkin,
        Self::Tomato,
        Self::PeaPod,
        Self::Strawberry,
        Self::Berries,
        Self::HerbSprig,
        Self::Apple,
        Self::Leaf,
        Self::Blossom,
        Self::Snore,
    ];

    const fn index(self) -> u32 {
        self as u32
    }

    /// How something the village holds or chases is drawn.
    pub const fn of(prop: VillageProp) -> Self {
        match prop {
            VillageProp::WateringCan => Self::WateringCan,
            VillageProp::Produce(kind) => Self::picked_from(kind),
            VillageProp::Apple => Self::Apple,
            VillageProp::Leaf => Self::Leaf,
        }
    }

    /// What a companion picks or holds up from a garden of this kind.
    pub const fn picked_from(kind: GardenKind) -> Self {
        match kind {
            GardenKind::Vegetables => Self::Carrot,
            GardenKind::Pumpkins => Self::Pumpkin,
            GardenKind::Tomatoes => Self::Tomato,
            GardenKind::PeaTrellis => Self::PeaPod,
            GardenKind::Strawberries => Self::Strawberry,
            GardenKind::BerryBush => Self::Berries,
            GardenKind::Herbs => Self::HerbSprig,
            GardenKind::Flowers
            | GardenKind::Sunflowers
            | GardenKind::MushroomRing
            | GardenKind::Tulips
            | GardenKind::Cactus => Self::Blossom,
        }
    }
}

/// What a garden is made of whatever the colony's colours: earth, the timber of its edging, and
/// the greens of what grows in it. Only the flowers take the colony's own colours.
const SOIL: Rgba = Rgba::new(112, 80, 58, 255);
const SOIL_DARK: Rgba = Rgba::new(80, 56, 44, 255);
const PLANK: Rgba = Rgba::new(160, 112, 72, 255);
const PLANK_LIGHT: Rgba = Rgba::new(198, 150, 100, 255);
const LEAF: Rgba = Rgba::new(92, 158, 80, 255);
const LEAF_DARK: Rgba = Rgba::new(56, 110, 62, 255);
const LEAF_LIGHT: Rgba = Rgba::new(152, 204, 112, 255);
const CARROT: Rgba = Rgba::new(236, 136, 56, 255);
const PUMPKIN_SHADE: Rgba = Rgba::new(196, 98, 40, 255);
const LAVENDER: Rgba = Rgba::new(166, 134, 216, 255);
const POLLEN: Rgba = Rgba::new(255, 222, 110, 255);
/// The rest of what the village is made of.
const STONE: Rgba = Rgba::new(170, 166, 158, 255);
const STONE_DARK: Rgba = Rgba::new(122, 118, 114, 255);
const STONE_LIGHT: Rgba = Rgba::new(210, 206, 198, 255);
const WATER: Rgba = Rgba::new(96, 160, 214, 255);
const WATER_LIGHT: Rgba = Rgba::new(182, 222, 246, 255);
const METAL: Rgba = Rgba::new(150, 156, 168, 255);
const METAL_LIGHT: Rgba = Rgba::new(210, 214, 222, 255);
const RED: Rgba = Rgba::new(214, 66, 60, 255);
const RED_DARK: Rgba = Rgba::new(150, 40, 44, 255);
const CREAM: Rgba = Rgba::new(242, 232, 208, 255);
const STRAW: Rgba = Rgba::new(226, 190, 116, 255);
const STRAW_DARK: Rgba = Rgba::new(176, 136, 76, 255);
const SAND: Rgba = Rgba::new(232, 206, 150, 255);
const FIRE: Rgba = Rgba::new(250, 150, 60, 255);
const FIRE_CORE: Rgba = Rgba::new(255, 226, 130, 255);
const SHADOW: Rgba = Rgba::new(20, 24, 26, 70);

pub struct ColonyObjectRenderer;

impl ColonyObjectRenderer {
    /// The sheet cell a belonging is drawn from.
    pub fn object_cell(kind: ColonyObjectKind) -> u32 {
        u32::from(kind.index())
    }

    /// The sheet cell a hangout spot is drawn from, after the belongings.
    pub fn hangout_cell(kind: HangoutKind) -> u32 {
        HANGOUTS + u32::from(kind.index())
    }

    /// The sheet cell a garden patch at one stage of growing is drawn from.
    pub fn garden_cell(kind: GardenKind, stage: GardenStage) -> u32 {
        GARDENS + u32::from(kind.index()) * GardenStage::ALL.len() as u32 + u32::from(stage.index())
    }

    /// The sheet cell an ornament is drawn from.
    pub fn ornament_cell(kind: OrnamentKind) -> u32 {
        ORNAMENTS + u32::from(kind.index())
    }

    /// The sheet cell a small held or chased thing is drawn from.
    pub fn prop_cell(prop: PropSprite) -> u32 {
        PROPS + prop.index()
    }

    /// The sheet cell anything put down on the village ground is drawn from. A garden is drawn
    /// at whatever stage it has grown to.
    pub fn ground_cell(item: GroundItem, stage: GardenStage) -> u32 {
        match item {
            GroundItem::Hangout(kind) => Self::hangout_cell(kind),
            GroundItem::Garden(kind) => Self::garden_cell(kind, stage),
            GroundItem::Ornament(kind) => Self::ornament_cell(kind),
        }
    }

    /// Whether something on the village ground is drawn turned round: the lookout, which looks
    /// out over the open desktop, is turned when it stands on the right-hand half.
    pub fn ground_mirrored(item: GroundItem, x: f32, middle: f32) -> bool {
        item == GroundItem::Hangout(HangoutKind::Lookout) && x > middle
    }

    /// Where a cell sits on the sheet, as its left, top, right and bottom texture coordinates.
    pub fn cell_uv(cell: u32) -> (f32, f32, f32, f32) {
        let columns = COLONY_OBJECT_COLUMNS as f32;
        let rows = COLONY_OBJECT_ROWS as f32;
        let (column, row) = (
            (cell % COLONY_OBJECT_COLUMNS) as f32,
            (cell / COLONY_OBJECT_COLUMNS) as f32,
        );
        (
            column / columns,
            row / rows,
            (column + 1.0) / columns,
            (row + 1.0) / rows,
        )
    }

    /// Where a cell's top-left sits on the sheet, in pixels.
    pub fn cell_origin(cell: u32) -> (u32, u32) {
        (
            cell % COLONY_OBJECT_COLUMNS * COLONY_OBJECT_SIZE,
            cell / COLONY_OBJECT_COLUMNS * COLONY_OBJECT_SIZE,
        )
    }

    pub fn render_atlas(colony_seed: [u8; 32]) -> Canvas {
        let mut canvas = Canvas::new(COLONY_OBJECT_ATLAS_WIDTH, COLONY_OBJECT_ATLAS_HEIGHT);
        let palette = PALETTES[colony_seed[16] as usize % PALETTES.len()];
        let secondary = PALETTES[colony_seed[17] as usize % PALETTES.len()];
        let ink = Ink {
            outline: palette.outline,
            coat: palette.coat,
            highlight: palette.highlight,
            accent: secondary.accent,
        };
        let mut place = |cell: u32, draw: &dyn Fn(&mut Canvas)| {
            let mut tile = Canvas::new(COLONY_OBJECT_SIZE, COLONY_OBJECT_SIZE);
            draw(&mut tile);
            let (x0, y0) = Self::cell_origin(cell);
            for y in 0..COLONY_OBJECT_SIZE as i32 {
                for x in 0..COLONY_OBJECT_SIZE as i32 {
                    let pixel = tile.get(x, y);
                    if pixel.a > 0 {
                        canvas.set(x0 as i32 + x, y0 as i32 + y, pixel);
                    }
                }
            }
        };
        for kind in ColonyObjectKind::ALL {
            // Objects rest directly on the village ground line. A soft contact shadow is the
            // only thing under them, so a keepsake reads as set down rather than shelved.
            place(Self::object_cell(kind), &|tile| {
                tile.fill_ellipse(8, 14, 6, 2, SHADOW);
                draw_object(tile, 0, kind, ink);
            });
        }
        for kind in HangoutKind::ALL {
            place(Self::hangout_cell(kind), &|tile| {
                tile.fill_ellipse(8, 14, 7, 2, SHADOW);
                draw_hangout(tile, 0, kind, ink);
            });
        }
        for kind in GardenKind::ALL {
            for stage in GardenStage::ALL {
                place(Self::garden_cell(kind, stage), &|tile| {
                    tile.fill_ellipse(8, 14, 7, 2, SHADOW);
                    draw_garden(tile, 0, kind, stage, ink);
                });
            }
        }
        for kind in OrnamentKind::ALL {
            place(Self::ornament_cell(kind), &|tile| {
                tile.fill_ellipse(8, 14, 6, 2, SHADOW);
                draw_ornament(tile, kind, ink);
            });
        }
        for prop in PropSprite::ALL {
            place(Self::prop_cell(prop), &|tile| draw_prop(tile, prop, ink));
        }
        canvas
    }
}

/// The colony's own colours, as everything on the sheet is drawn in them.
#[derive(Clone, Copy)]
struct Ink {
    outline: Rgba,
    coat: Rgba,
    highlight: Rgba,
    accent: Rgba,
}

// ---------------------------------------------------------------------------------------------
// Gardens.
// ---------------------------------------------------------------------------------------------

/// A garden patch at one stage of growing. The bed it grows in is the same at every stage, so a
/// patch reads as one place changing rather than four different things; what grows in it goes
/// from a few shoots, to leaves, to how it looks grown, to its fullest.
fn draw_garden(canvas: &mut Canvas, x: i32, kind: GardenKind, stage: GardenStage, ink: Ink) {
    let Ink {
        outline,
        coat,
        highlight,
        accent,
    } = ink;
    // The three patches a village always had look grown exactly as they always have.
    if stage == GardenStage::Grown
        && matches!(
            kind,
            GardenKind::Flowers | GardenKind::Vegetables | GardenKind::Herbs
        )
    {
        draw_grown_classic(canvas, x, kind, outline, coat, highlight, accent);
        return;
    }
    match kind {
        GardenKind::Flowers => {
            flower_bed(canvas, x, outline);
            match stage {
                GardenStage::Sprout => shoots(canvas, x, &[4, 8, 12], 10, 8),
                GardenStage::Growing => {
                    for (stem, top) in [(4, 7), (8, 5), (12, 8)] {
                        canvas.line(x + stem, 10, x + stem, top, 1, LEAF_DARK);
                        canvas.set(x + stem - 1, top + 1, LEAF);
                        canvas.set(x + stem + 1, top + 2, LEAF);
                        canvas.set(x + stem, top - 1, LEAF_LIGHT);
                    }
                }
                _ => {
                    for (stem, top, petals) in [
                        (3, 6, accent),
                        (6, 4, highlight),
                        (8, 3, coat),
                        (10, 5, accent),
                        (13, 6, highlight),
                    ] {
                        canvas.line(x + stem, 10, x + stem, top + 2, 1, LEAF_DARK);
                        blossom(canvas, x + stem, top, petals, outline);
                    }
                }
            }
        }
        GardenKind::Vegetables => {
            mound(canvas, x, outline);
            match stage {
                GardenStage::Sprout => shoots(canvas, x, &[4, 8, 12], 11, 9),
                GardenStage::Growing => {
                    canvas.fill_circle(x + 4, 9, 2, LEAF);
                    canvas.set(x + 4, 8, LEAF_LIGHT);
                    canvas.line(x + 8, 10, x + 7, 6, 1, LEAF);
                    canvas.line(x + 8, 10, x + 9, 6, 1, LEAF_DARK);
                    canvas.line(x + 12, 10, x + 11, 8, 1, LEAF);
                    canvas.set(x + 13, 8, LEAF_DARK);
                }
                _ => {
                    draw_grown_classic(canvas, x, kind, outline, coat, highlight, accent);
                    // At its fullest, the pumpkin has swollen and the carrot is showing more.
                    canvas.fill_ellipse(x + 12, 10, 3, 3, outline);
                    canvas.fill_ellipse(x + 12, 10, 2, 2, CARROT);
                    canvas.set(x + 12, 10, PUMPKIN_SHADE);
                    canvas.set(x + 11, 9, POLLEN);
                    canvas.set(x + 8, 9, CARROT);
                    sparkle(canvas, x + 2, 4, POLLEN);
                }
            }
        }
        GardenKind::Herbs => {
            herb_box(canvas, x, outline);
            match stage {
                GardenStage::Sprout => shoots(canvas, x, &[5, 8, 11], 8, 6),
                GardenStage::Growing => {
                    for (sprig, top) in [(4, 5), (7, 6), (10, 5), (12, 6)] {
                        canvas.line(x + sprig, 8, x + sprig, top, 1, LEAF_DARK);
                        canvas.set(x + sprig - 1, top + 1, LEAF);
                        canvas.set(x + sprig + 1, top + 2, LEAF);
                    }
                }
                _ => {
                    draw_grown_classic(canvas, x, kind, outline, coat, highlight, accent);
                    canvas.set(x + 13, 3, LAVENDER);
                    canvas.set(x + 11, 4, LAVENDER);
                    canvas.set(x + 7, 4, LEAF_LIGHT);
                    canvas.set(x + 3, 2, LEAF);
                }
            }
        }
        GardenKind::Sunflowers => {
            dirt_patch(canvas, x, outline);
            match stage {
                GardenStage::Sprout => shoots(canvas, x, &[5, 11], 12, 10),
                GardenStage::Growing => {
                    for stem in [5, 11] {
                        canvas.line(x + stem, 12, x + stem, 5, 1, LEAF_DARK);
                        canvas.set(x + stem - 1, 8, LEAF);
                        canvas.set(x + stem + 1, 10, LEAF);
                        canvas.set(x + stem, 4, LEAF_LIGHT);
                    }
                }
                GardenStage::Grown | GardenStage::Bounty => {
                    let (head, face) = if stage == GardenStage::Bounty {
                        (3, 2)
                    } else {
                        (2, 1)
                    };
                    for (stem, top) in [(5, 5), (11, 3)] {
                        canvas.line(x + stem, 12, x + stem, top + 2, 1, LEAF_DARK);
                        canvas.set(x + stem - 1, 9, LEAF);
                        canvas.set(x + stem + 1, 8, LEAF);
                        canvas.fill_circle(x + stem, top, head, POLLEN);
                        canvas.fill_circle(x + stem, top, face, SOIL);
                        canvas.set(x + stem - 1, top - 1, SOIL_DARK);
                    }
                }
            }
        }
        GardenKind::Pumpkins => {
            dirt_patch(canvas, x, outline);
            match stage {
                GardenStage::Sprout => shoots(canvas, x, &[6, 10], 12, 10),
                _ => {
                    // A vine running along the ground with its big leaves.
                    canvas.line(x + 2, 11, x + 14, 11, 1, LEAF_DARK);
                    for leaf_x in [3, 7, 13] {
                        canvas.fill_ellipse(x + leaf_x, 9, 2, 1, LEAF);
                        canvas.set(x + leaf_x, 8, LEAF_LIGHT);
                    }
                    let (size, color) = match stage {
                        GardenStage::Growing => (0, LEAF),
                        GardenStage::Grown => (2, LEAF_LIGHT),
                        _ => (3, CARROT),
                    };
                    if size > 0 {
                        canvas.fill_ellipse(x + 10, 12 - size / 2, size + 1, size, outline);
                        canvas.fill_ellipse(x + 10, 12 - size / 2, size, size - 1, color);
                        canvas.line(
                            x + 10,
                            12 - size,
                            x + 10,
                            12,
                            1,
                            if color == CARROT { PUMPKIN_SHADE } else { LEAF },
                        );
                        canvas.set(x + 10, 11 - size - 1, LEAF_DARK);
                    }
                }
            }
        }
        GardenKind::Strawberries => {
            // A terracotta pot with runners over its rim.
            canvas.fill_rect(x + 3, 9, 10, 6, outline);
            canvas.fill_rect(x + 4, 10, 8, 4, PUMPKIN_SHADE);
            canvas.fill_rect(x + 4, 10, 8, 1, CARROT);
            canvas.set(x + 3, 9, outline);
            match stage {
                GardenStage::Sprout => shoots(canvas, x, &[6, 10], 9, 7),
                _ => {
                    for (lx, ly) in [(5, 7), (8, 6), (11, 7), (7, 8), (10, 8)] {
                        canvas.fill_ellipse(x + lx, ly, 1, 1, LEAF);
                    }
                    canvas.set(x + 8, 5, LEAF_LIGHT);
                    match stage {
                        GardenStage::Growing => {}
                        GardenStage::Grown => {
                            for fx in [5, 11] {
                                canvas.set(x + fx, 5, CREAM);
                                canvas.set(x + fx, 6, POLLEN);
                            }
                        }
                        _ => {
                            for (bx, by) in [(3, 11), (13, 11), (6, 9)] {
                                canvas.fill_rect(x + bx - 1, by, 2, 2, RED);
                                canvas.set(x + bx - 1, by, LEAF);
                                canvas.set(x + bx, by + 1, RED_DARK);
                            }
                        }
                    }
                }
            }
        }
        GardenKind::MushroomRing => {
            // A patch of grass with mushrooms coming up round it.
            canvas.fill_ellipse(x + 8, 13, 7, 2, LEAF_DARK);
            canvas.fill_ellipse(x + 8, 13, 6, 1, LEAF);
            let spots: &[(i32, i32)] = &[(3, 12), (6, 11), (10, 11), (13, 12)];
            for (index, (mx, my)) in spots.iter().enumerate() {
                let size = match stage {
                    GardenStage::Sprout => usize::from(index % 2 == 0),
                    GardenStage::Growing => 1,
                    GardenStage::Grown => 2,
                    GardenStage::Bounty => 2 + index % 2,
                } as i32;
                if size == 0 {
                    continue;
                }
                canvas.fill_rect(x + mx, my - size + 1, 1, size, CREAM);
                canvas.fill_ellipse(x + mx, my - size, size, (size - 1).max(1), outline);
                canvas.fill_ellipse(x + mx, my - size, (size - 1).max(1), 1, coat);
                if size >= 2 {
                    canvas.set(x + mx - 1, my - size - 1, CREAM);
                }
            }
        }
        GardenKind::BerryBush => {
            dirt_patch(canvas, x, outline);
            match stage {
                GardenStage::Sprout => {
                    canvas.line(x + 8, 12, x + 8, 7, 1, SOIL_DARK);
                    canvas.set(x + 7, 8, LEAF);
                    canvas.set(x + 9, 9, LEAF);
                }
                _ => {
                    canvas.fill_ellipse(x + 8, 8, 6, 5, outline);
                    canvas.fill_ellipse(x + 8, 8, 5, 4, LEAF);
                    canvas.fill_ellipse(x + 6, 6, 2, 1, LEAF_LIGHT);
                    canvas.set(x + 11, 10, LEAF_DARK);
                    let (color, count) = match stage {
                        GardenStage::Growing => (LEAF_DARK, 0),
                        GardenStage::Grown => (CREAM, 4),
                        _ => (accent, 7),
                    };
                    for (bx, by) in [(5, 8), (9, 6), (11, 9), (7, 10), (10, 11), (4, 10), (8, 4)]
                        .into_iter()
                        .take(count)
                    {
                        canvas.set(x + bx, by, color);
                    }
                }
            }
        }
        GardenKind::Tulips => {
            flower_bed(canvas, x, outline);
            let stems = [3, 6, 9, 12];
            match stage {
                GardenStage::Sprout => shoots(canvas, x, &stems, 10, 9),
                _ => {
                    for (index, stem) in stems.into_iter().enumerate() {
                        let top = 5 + (index as i32 % 2);
                        canvas.line(x + stem, 10, x + stem, top + 1, 1, LEAF_DARK);
                        canvas.set(x + stem + 1, 9, LEAF);
                        let color = if index % 2 == 0 { accent } else { coat };
                        match stage {
                            GardenStage::Growing => canvas.set(x + stem, top, LEAF),
                            GardenStage::Grown => {
                                canvas.fill_rect(x + stem, top - 1, 1, 2, color);
                                canvas.set(x + stem, top - 2, outline);
                            }
                            _ => {
                                canvas.fill_rect(x + stem - 1, top - 1, 3, 2, color);
                                canvas.set(x + stem - 1, top - 2, color);
                                canvas.set(x + stem + 1, top - 2, color);
                                canvas.set(x + stem, top - 1, highlight);
                            }
                        }
                    }
                }
            }
        }
        GardenKind::Cactus => {
            // Two clay pots, each with a cactus that grows arms and, at last, a flower.
            for (px, height) in [(4, 5), (11, 7)] {
                canvas.fill_rect(x + px - 3, 11, 6, 4, outline);
                canvas.fill_rect(x + px - 2, 12, 4, 2, PUMPKIN_SHADE);
                canvas.fill_rect(x + px - 2, 12, 4, 1, CARROT);
                let height = match stage {
                    GardenStage::Sprout => 2,
                    GardenStage::Growing => height - 2,
                    _ => height,
                };
                canvas.fill_rect(x + px - 1, 11 - height, 3, height, outline);
                canvas.fill_rect(x + px, 11 - height + 1, 1, height - 1, LEAF);
                if stage >= GardenStage::Grown {
                    canvas.set(x + px + 2, 11 - height + 2, outline);
                    canvas.set(x + px + 2, 11 - height + 1, LEAF);
                    canvas.set(x + px - 2, 11 - height + 3, outline);
                    canvas.set(x + px - 2, 11 - height + 2, LEAF);
                }
                if stage == GardenStage::Bounty {
                    canvas.set(x + px, 11 - height - 1, accent);
                    canvas.set(x + px, 11 - height, highlight);
                }
            }
        }
        GardenKind::PeaTrellis => {
            dirt_patch(canvas, x, outline);
            // Two sticks crossed into a little lattice.
            for stick in [4, 12] {
                canvas.line(x + stick, 13, x + 8, 2, 1, PLANK);
            }
            canvas.line(x + 5, 8, x + 11, 8, 1, PLANK_LIGHT);
            match stage {
                GardenStage::Sprout => shoots(canvas, x, &[6, 10], 12, 11),
                _ => {
                    let reach = if stage == GardenStage::Growing { 7 } else { 3 };
                    canvas.line(x + 6, 12, x + 7, reach, 1, LEAF);
                    canvas.line(x + 10, 12, x + 9, reach + 1, 1, LEAF_DARK);
                    canvas.set(x + 5, 10, LEAF);
                    canvas.set(x + 11, 9, LEAF);
                    match stage {
                        GardenStage::Grown => {
                            canvas.set(x + 6, 6, CREAM);
                            canvas.set(x + 10, 7, CREAM);
                        }
                        GardenStage::Bounty => {
                            for (px, py) in [(5, 7), (11, 6)] {
                                canvas.fill_rect(x + px, py, 1, 3, LEAF_LIGHT);
                                canvas.set(x + px, py + 1, outline);
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        GardenKind::Tomatoes => {
            dirt_patch(canvas, x, outline);
            canvas.line(x + 8, 13, x + 8, 2, 1, PLANK);
            match stage {
                GardenStage::Sprout => shoots(canvas, x, &[6], 12, 10),
                _ => {
                    for (lx, ly) in [(6, 10), (10, 8), (6, 6), (10, 4)] {
                        canvas.fill_ellipse(x + lx, ly, 2, 1, LEAF);
                    }
                    canvas.set(x + 9, 3, LEAF_LIGHT);
                    let color = match stage {
                        GardenStage::Growing => None,
                        GardenStage::Grown => Some(LEAF_LIGHT),
                        _ => Some(RED),
                    };
                    if let Some(color) = color {
                        for (tx, ty) in [(5, 8), (11, 10), (11, 6)] {
                            canvas.fill_circle(x + tx, ty, 1, color);
                            canvas.set(x + tx - 1, ty - 1, CREAM);
                        }
                    }
                }
            }
        }
    }
}

/// A few green shoots coming up out of the ground.
fn shoots(canvas: &mut Canvas, x: i32, stems: &[i32], ground: i32, top: i32) {
    for stem in stems {
        canvas.line(x + stem, ground, x + stem, top, 1, LEAF_DARK);
        canvas.set(x + stem - 1, top, LEAF);
        canvas.set(x + stem + 1, top - 1, LEAF_LIGHT);
    }
}

/// A flower seen face on: four petals round a pollen centre, outlined.
fn blossom(canvas: &mut Canvas, x: i32, y: i32, petals: Rgba, outline: Rgba) {
    canvas.fill_circle(x, y, 2, outline);
    for (dx, dy) in [(0, -1), (-1, 0), (1, 0), (0, 1)] {
        canvas.set(x + dx, y + dy, petals);
    }
    canvas.set(x, y, POLLEN);
}

/// A four-pointed glint, for something at its best.
fn sparkle(canvas: &mut Canvas, x: i32, y: i32, color: Rgba) {
    canvas.set(x, y, color);
    for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
        canvas.set(x + dx, y + dy, color);
    }
}

/// The flower bed's edging board, the same at every stage.
fn flower_bed(canvas: &mut Canvas, x: i32, outline: Rgba) {
    canvas.fill_rect(x + 1, 11, 14, 4, outline);
    canvas.fill_rect(x + 2, 11, 12, 1, SOIL);
    canvas.fill_rect(x + 2, 12, 12, 2, PLANK);
    canvas.line(x + 2, 12, x + 13, 12, 1, PLANK_LIGHT);
    for seam in [6, 10] {
        canvas.set(x + seam, 13, SOIL_DARK);
    }
}

/// The vegetable patch's mound of dug earth.
fn mound(canvas: &mut Canvas, x: i32, outline: Rgba) {
    canvas.fill_ellipse(x + 8, 12, 7, 2, outline);
    canvas.fill_ellipse(x + 8, 12, 6, 1, SOIL);
    canvas.line(x + 3, 13, x + 13, 13, 1, SOIL_DARK);
}

/// The herb planter box.
fn herb_box(canvas: &mut Canvas, x: i32, outline: Rgba) {
    canvas.fill_rect(x + 2, 9, 12, 6, outline);
    canvas.fill_rect(x + 3, 10, 10, 4, PLANK);
    canvas.line(x + 3, 10, x + 12, 10, 1, PLANK_LIGHT);
    canvas.line(x + 3, 12, x + 12, 12, 1, SOIL_DARK);
    canvas.set(x + 5, 11, outline);
    canvas.set(x + 10, 11, outline);
}

/// A plain patch of turned earth.
fn dirt_patch(canvas: &mut Canvas, x: i32, outline: Rgba) {
    canvas.fill_ellipse(x + 8, 13, 7, 2, outline);
    canvas.fill_ellipse(x + 8, 13, 6, 1, SOIL);
    canvas.set(x + 5, 13, SOIL_DARK);
    canvas.set(x + 11, 13, SOIL_DARK);
}

/// The three patches as a village has always drawn them grown.
fn draw_grown_classic(
    canvas: &mut Canvas,
    x: i32,
    kind: GardenKind,
    outline: Rgba,
    coat: Rgba,
    highlight: Rgba,
    accent: Rgba,
) {
    match kind {
        // A bed edged with a low board, three flowers in the colony's colours standing up out of
        // it on leafy stems, the middle one tallest.
        GardenKind::Flowers => {
            canvas.fill_rect(x + 1, 11, 14, 4, outline);
            canvas.fill_rect(x + 2, 11, 12, 1, SOIL);
            canvas.fill_rect(x + 2, 12, 12, 2, PLANK);
            canvas.line(x + 2, 12, x + 13, 12, 1, PLANK_LIGHT);
            for seam in [6, 10] {
                canvas.set(x + seam, 13, SOIL_DARK);
            }
            for (stem, top, petals) in [(4, 6, accent), (8, 4, coat), (12, 7, highlight)] {
                canvas.line(x + stem, 10, x + stem, top + 2, 1, LEAF_DARK);
                canvas.fill_circle(x + stem, top, 2, outline);
                for (dx, dy) in [(0, -1), (-1, 0), (1, 0), (0, 1)] {
                    canvas.set(x + stem + dx, top + dy, petals);
                }
                canvas.set(x + stem, top, POLLEN);
            }
            for (leaf_x, leaf_y) in [(3, 9), (5, 8), (7, 8), (9, 7), (11, 9), (13, 8)] {
                canvas.set(x + leaf_x, leaf_y, LEAF);
            }
        }
        // A mound of dug earth with a cabbage at one end, a carrot showing its shoulder under a
        // feathery top in the middle, and a squat ribbed pumpkin at the other end.
        GardenKind::Vegetables => {
            canvas.fill_ellipse(x + 8, 12, 7, 2, outline);
            canvas.fill_ellipse(x + 8, 12, 6, 1, SOIL);
            canvas.line(x + 3, 13, x + 13, 13, 1, SOIL_DARK);
            canvas.fill_circle(x + 4, 9, 3, outline);
            canvas.fill_circle(x + 4, 9, 2, LEAF);
            canvas.set(x + 3, 8, LEAF_LIGHT);
            canvas.set(x + 4, 9, LEAF_LIGHT);
            canvas.set(x + 5, 10, LEAF_DARK);
            canvas.set(x + 2, 10, LEAF_DARK);
            canvas.fill_rect(x + 7, 10, 3, 2, outline);
            canvas.set(x + 8, 10, CARROT);
            canvas.set(x + 8, 11, CARROT);
            canvas.line(x + 8, 9, x + 7, 5, 1, LEAF);
            canvas.line(x + 8, 9, x + 9, 5, 1, LEAF_DARK);
            canvas.set(x + 8, 6, LEAF_LIGHT);
            canvas.fill_ellipse(x + 12, 10, 3, 2, outline);
            canvas.fill_ellipse(x + 12, 10, 2, 1, CARROT);
            canvas.set(x + 12, 10, PUMPKIN_SHADE);
            canvas.set(x + 11, 9, POLLEN);
            canvas.set(x + 12, 7, LEAF_DARK);
            canvas.set(x + 13, 7, LEAF);
        }
        // A wooden planter box with two sprigs of rosemary, a round bush of basil and a stalk of
        // lavender growing in it.
        _ => {
            canvas.fill_rect(x + 2, 9, 12, 6, outline);
            canvas.fill_rect(x + 3, 10, 10, 4, PLANK);
            canvas.line(x + 3, 10, x + 12, 10, 1, PLANK_LIGHT);
            canvas.line(x + 3, 12, x + 12, 12, 1, SOIL_DARK);
            canvas.set(x + 5, 11, outline);
            canvas.set(x + 10, 11, outline);
            for (sprig, top) in [(4, 3), (6, 5)] {
                canvas.line(x + sprig, 8, x + sprig, top, 1, LEAF_DARK);
                for needle in (top + 1..8).step_by(2) {
                    canvas.set(x + sprig - 1, needle, LEAF);
                    canvas.set(x + sprig + 1, needle + 1, LEAF);
                }
            }
            canvas.fill_ellipse(x + 9, 6, 2, 2, LEAF);
            canvas.set(x + 8, 5, LEAF_LIGHT);
            canvas.set(x + 10, 6, LEAF_LIGHT);
            canvas.set(x + 9, 8, LEAF_DARK);
            canvas.set(x + 11, 7, LEAF_DARK);
            canvas.line(x + 12, 8, x + 12, 4, 1, LEAF_DARK);
            for bud in 2..6 {
                canvas.set(x + 12 + (bud % 2), bud, LAVENDER);
            }
            canvas.set(x + 12, 2, outline);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Hangout spots.
// ---------------------------------------------------------------------------------------------

/// The spots the person at the desk puts down: bigger and lower than a belonging, since they are
/// somewhere to be rather than something to have. The lookout's spyglass points to the right;
/// the overlay mirrors it where the open desktop lies to its left.
fn draw_hangout(canvas: &mut Canvas, x: i32, kind: HangoutKind, ink: Ink) {
    let Ink {
        outline,
        coat,
        highlight,
        accent,
    } = ink;
    match kind {
        // A plump floor cushion, puffed up round a button in its middle, piped along its top and
        // with a tassel at each end: taller and softer than a pillow lying about.
        HangoutKind::Cushion => {
            canvas.fill_ellipse(x + 8, 11, 7, 4, outline);
            canvas.fill_ellipse(x + 8, 10, 6, 3, accent);
            canvas.line(x + 3, 8, x + 12, 8, 1, highlight);
            canvas.set(x + 8, 10, outline);
            canvas.set(x + 6, 11, coat);
            canvas.set(x + 10, 11, coat);
            canvas.set(x + 7, 9, coat);
            canvas.set(x + 9, 9, coat);
            for end in [0, 15] {
                canvas.set(x + end, 12, highlight);
                canvas.set(x + end, 13, outline);
            }
        }
        // A blanket spread flat on the grass, checked, with its near corner turned up and an apple
        // set down on it.
        HangoutKind::Blanket => {
            canvas.fill_rect(x + 1, 11, 14, 4, outline);
            for column in 0..6 {
                for row in 0..2 {
                    let color = if (column + row) % 2 == 0 {
                        accent
                    } else {
                        highlight
                    };
                    canvas.fill_rect(x + 2 + column * 2, 12 + row, 2, 1, color);
                }
            }
            canvas.set(x + 14, 11, highlight);
            // The apple: round, with a glint, a stalk and a leaf.
            for (dx, dy) in [
                (0, 0),
                (1, 0),
                (2, 0),
                (-1, 1),
                (3, 1),
                (-1, 2),
                (3, 2),
                (0, 3),
            ] {
                canvas.set(x + 9 + dx, 7 + dy, outline);
            }
            canvas.set(x + 11, 10, outline);
            canvas.fill_rect(x + 9, 8, 3, 2, coat);
            canvas.set(x + 10, 10, coat);
            canvas.set(x + 9, 8, highlight);
            canvas.set(x + 10, 6, outline);
            canvas.set(x + 11, 5, accent);
        }
        // A spyglass on a three-legged stand, tipped up at the sky.
        HangoutKind::Lookout => {
            for foot in [4, 8, 12] {
                canvas.line(x + 8, 9, x + foot, 14, 1, outline);
            }
            canvas.line(x + 3, 9, x + 12, 4, 3, outline);
            canvas.line(x + 4, 8, x + 11, 4, 1, coat);
            canvas.line(x + 5, 9, x + 12, 5, 1, accent);
            canvas.fill_circle(x + 13, 4, 1, highlight);
        }
        // Slung between two posts, sagging in the middle.
        HangoutKind::Hammock => {
            for post in [1, 14] {
                canvas.fill_rect(x + post, 5, 2, 10, outline);
                canvas.fill_rect(x + post, 6, 1, 8, PLANK);
            }
            for column in 3..=13 {
                let t = (column - 3) as f32 / 10.0;
                let sag = (4.0 * t * (1.0 - t) * 4.0).round() as i32;
                canvas.set(x + column, 7 + sag, outline);
                canvas.set(
                    x + column,
                    8 + sag,
                    if column % 2 == 0 { accent } else { coat },
                );
                canvas.set(x + column, 9 + sag, outline);
            }
            canvas.set(x + 3, 6, outline);
            canvas.set(x + 13, 6, outline);
        }
        // A plank seat hung on two ropes from a little frame.
        HangoutKind::Swing => {
            canvas.line(x + 2, 14, x + 5, 2, 1, outline);
            canvas.line(x + 14, 14, x + 11, 2, 1, outline);
            canvas.line(x + 4, 2, x + 12, 2, 1, PLANK);
            canvas.line(x + 4, 1, x + 12, 1, 1, outline);
            for rope in [6, 10] {
                canvas.line(x + rope, 3, x + rope, 10, 1, STRAW);
            }
            canvas.fill_rect(x + 5, 10, 7, 2, outline);
            canvas.fill_rect(x + 6, 10, 5, 1, coat);
        }
        // A little round table set for tea.
        HangoutKind::TeaTable => {
            canvas.fill_ellipse(x + 8, 8, 6, 2, outline);
            canvas.fill_ellipse(x + 8, 8, 5, 1, highlight);
            canvas.line(x + 8, 10, x + 8, 14, 1, outline);
            canvas.line(x + 5, 14, x + 11, 14, 1, outline);
            // A teapot and a cup on top.
            canvas.fill_ellipse(x + 6, 5, 2, 2, outline);
            canvas.fill_ellipse(x + 6, 5, 1, 1, accent);
            canvas.set(x + 4, 4, outline);
            canvas.set(x + 6, 2, outline);
            canvas.fill_rect(x + 10, 5, 3, 2, outline);
            canvas.set(x + 11, 5, CREAM);
            canvas.set(x + 13, 5, outline);
        }
        // A stack of three books, the top one open.
        HangoutKind::BookNook => {
            for (row, color, inset) in [(12, accent, 1), (10, coat, 2), (8, highlight, 1)] {
                canvas.fill_rect(x + 2 + inset, row, 11 - inset, 3, outline);
                canvas.fill_rect(x + 3 + inset, row + 1, 9 - inset, 1, color);
                canvas.set(x + 12, row + 1, CREAM);
            }
            canvas.fill_rect(x + 4, 5, 9, 3, outline);
            canvas.fill_rect(x + 5, 5, 3, 2, CREAM);
            canvas.fill_rect(x + 9, 5, 3, 2, CREAM);
            canvas.set(x + 8, 6, outline);
        }
        // A ring of stones round a little fire.
        HangoutKind::Campfire => {
            for (sx, sy) in [(3, 12), (6, 13), (10, 13), (13, 12)] {
                canvas.fill_ellipse(x + sx, sy, 2, 1, outline);
                canvas.set(x + sx, sy, STONE);
            }
            canvas.line(x + 5, 11, x + 11, 9, 2, SOIL_DARK);
            canvas.line(x + 5, 9, x + 11, 11, 2, PLANK);
            canvas.fill_ellipse(x + 8, 7, 2, 3, FIRE);
            canvas.fill_ellipse(x + 8, 8, 1, 1, FIRE_CORE);
            canvas.set(x + 8, 3, FIRE);
            canvas.set(x + 6, 5, FIRE);
        }
        // A box of sand with a spade in it.
        HangoutKind::Sandbox => {
            canvas.fill_rect(x + 1, 9, 14, 6, outline);
            canvas.fill_rect(x + 2, 10, 12, 1, SAND);
            canvas.fill_rect(x + 2, 11, 12, 3, PLANK);
            canvas.line(x + 2, 11, x + 13, 11, 1, PLANK_LIGHT);
            canvas.fill_ellipse(x + 6, 9, 3, 1, SAND);
            canvas.line(x + 11, 9, x + 12, 4, 1, outline);
            canvas.fill_rect(x + 10, 3, 3, 2, accent);
        }
        // A puddle kept on purpose, for splashing in.
        HangoutKind::Puddle => {
            canvas.fill_ellipse(x + 8, 12, 7, 2, outline);
            canvas.fill_ellipse(x + 8, 12, 6, 1, WATER);
            canvas.line(x + 4, 12, x + 7, 12, 1, WATER_LIGHT);
            canvas.set(x + 11, 11, WATER_LIGHT);
            canvas.set(x + 4, 9, WATER_LIGHT);
            canvas.set(x + 12, 8, WATER_LIGHT);
        }
        // A bench of two planks on stubby legs.
        HangoutKind::Bench => {
            canvas.fill_rect(x + 1, 8, 14, 3, outline);
            canvas.fill_rect(x + 2, 9, 12, 1, PLANK_LIGHT);
            canvas.fill_rect(x + 2, 5, 12, 2, outline);
            canvas.fill_rect(x + 3, 5, 10, 1, PLANK);
            for leg in [3, 12] {
                canvas.fill_rect(x + leg, 11, 2, 4, outline);
            }
            canvas.set(x + 3, 7, outline);
            canvas.set(x + 12, 7, outline);
        }
        // A hollow stump with a skin drawn over its top.
        HangoutKind::DrumStump => {
            canvas.fill_rect(x + 4, 7, 9, 8, outline);
            canvas.fill_rect(x + 5, 8, 7, 6, PLANK);
            canvas.line(x + 7, 9, x + 7, 13, 1, SOIL_DARK);
            canvas.line(x + 10, 9, x + 10, 13, 1, SOIL_DARK);
            canvas.fill_ellipse(x + 8, 7, 4, 1, CREAM);
            canvas.line(x + 3, 4, x + 6, 6, 1, outline);
            canvas.set(x + 2, 3, accent);
        }
        // A feeder on a pole, and a bird at it.
        HangoutKind::BirdFeeder => {
            canvas.line(x + 8, 14, x + 8, 6, 1, outline);
            canvas.fill_rect(x + 5, 5, 7, 3, outline);
            canvas.fill_rect(x + 6, 6, 5, 1, SAND);
            canvas.line(x + 5, 4, x + 11, 4, 1, accent);
            canvas.fill_ellipse(x + 12, 7, 2, 1, outline);
            canvas.set(x + 12, 7, coat);
            canvas.set(x + 14, 6, outline);
            canvas.set(x + 14, 7, POLLEN);
        }
        // A mat to lie back on, spotted with stars.
        HangoutKind::StargazingMat => {
            canvas.fill_rect(x + 1, 11, 14, 4, outline);
            canvas.fill_rect(x + 2, 12, 12, 2, coat);
            for sx in [3, 7, 11] {
                canvas.set(x + sx, 12, POLLEN);
            }
            canvas.set(x + 5, 13, highlight);
            canvas.set(x + 9, 13, highlight);
            // A cushion at its head.
            canvas.fill_ellipse(x + 3, 10, 2, 1, outline);
            canvas.set(x + 3, 10, accent);
        }
        // A flat rock that holds the sun's warmth.
        HangoutKind::SunnyRock => {
            canvas.fill_ellipse(x + 8, 12, 7, 3, outline);
            canvas.fill_ellipse(x + 8, 11, 6, 2, STONE);
            canvas.line(x + 4, 10, x + 9, 10, 1, STONE_LIGHT);
            canvas.set(x + 12, 12, STONE_DARK);
            sparkle(canvas, x + 12, 6, POLLEN);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Ornaments.
// ---------------------------------------------------------------------------------------------

fn draw_ornament(canvas: &mut Canvas, kind: OrnamentKind, ink: Ink) {
    let Ink {
        outline,
        coat,
        highlight,
        accent,
    } = ink;
    match kind {
        // A lamp on a post.
        OrnamentKind::LampPost => {
            canvas.fill_rect(7, 5, 2, 10, outline);
            canvas.fill_rect(5, 14, 6, 1, outline);
            canvas.fill_rect(5, 1, 6, 5, outline);
            canvas.fill_rect(6, 2, 4, 3, FIRE_CORE);
            canvas.set(7, 3, FIRE);
            canvas.fill_rect(4, 0, 8, 1, accent);
        }
        // A stone bowl of water on a pedestal.
        OrnamentKind::BirdBath => {
            canvas.fill_rect(7, 8, 3, 6, outline);
            canvas.fill_rect(5, 13, 7, 2, outline);
            canvas.set(8, 10, STONE);
            canvas.fill_ellipse(8, 6, 6, 2, outline);
            canvas.fill_ellipse(8, 5, 5, 1, WATER);
            canvas.line(4, 5, 7, 5, 1, WATER_LIGHT);
            canvas.fill_rect(3, 7, 11, 1, STONE);
        }
        // A post with two arrows pointing different ways.
        OrnamentKind::Signpost => {
            canvas.fill_rect(7, 3, 2, 12, outline);
            canvas.fill_rect(2, 3, 10, 3, outline);
            canvas.fill_rect(3, 4, 8, 1, PLANK_LIGHT);
            canvas.set(12, 4, outline);
            canvas.fill_rect(5, 7, 9, 3, outline);
            canvas.fill_rect(6, 8, 7, 1, PLANK);
            canvas.set(4, 8, outline);
            canvas.set(8, 4, accent);
        }
        // A round stone well with a little roof and a bucket.
        OrnamentKind::WishingWell => {
            canvas.fill_rect(3, 9, 10, 6, outline);
            canvas.fill_rect(4, 10, 8, 4, STONE);
            canvas.line(4, 12, 11, 12, 1, STONE_DARK);
            canvas.set(7, 11, STONE_DARK);
            for post in [4, 11] {
                canvas.fill_rect(post, 3, 1, 6, PLANK);
            }
            canvas.fill_rect(2, 2, 12, 2, outline);
            canvas.fill_rect(3, 2, 10, 1, coat);
            canvas.line(8, 4, 8, 6, 1, outline);
            canvas.fill_rect(7, 6, 3, 2, METAL);
        }
        // A short run of white fence.
        OrnamentKind::PicketFence => {
            for picket in [1, 5, 9, 13] {
                canvas.fill_rect(picket, 6, 3, 9, outline);
                canvas.fill_rect(picket + 1, 7, 1, 7, CREAM);
                canvas.set(picket + 1, 5, outline);
            }
            canvas.fill_rect(0, 8, 16, 1, CREAM);
            canvas.fill_rect(0, 12, 16, 1, CREAM);
        }
        // Flat stones set in the grass.
        OrnamentKind::SteppingStones => {
            for (sx, sy, rx) in [(3, 13, 2), (8, 12, 3), (13, 13, 2)] {
                canvas.fill_ellipse(sx, sy, rx + 1, 2, outline);
                canvas.fill_ellipse(sx, sy, rx, 1, STONE);
                canvas.set(sx - 1, sy - 1, STONE_LIGHT);
            }
            canvas.set(5, 14, LEAF);
            canvas.set(11, 14, LEAF);
        }
        // A scarecrow in a hat that scares nothing at all.
        OrnamentKind::Scarecrow => {
            canvas.fill_rect(7, 4, 2, 11, PLANK);
            canvas.fill_rect(2, 6, 12, 2, PLANK);
            canvas.fill_rect(5, 7, 6, 5, outline);
            canvas.fill_rect(6, 8, 4, 3, accent);
            canvas.fill_circle(8, 4, 2, outline);
            canvas.fill_circle(8, 4, 1, SAND);
            canvas.fill_rect(5, 1, 6, 1, outline);
            canvas.fill_rect(6, 0, 4, 1, STRAW_DARK);
            canvas.set(2, 8, STRAW);
            canvas.set(13, 8, STRAW);
        }
        // A spinner on a pole, turning.
        OrnamentKind::WindSpinner => {
            canvas.line(8, 14, 8, 6, 1, outline);
            for (dx, dy, color) in [(0, -1, coat), (1, 0, accent), (0, 1, coat), (-1, 0, accent)] {
                for step in 1..=3 {
                    canvas.set(8 + dx * step, 5 + dy * step, color);
                }
                canvas.set(8 + dx * 3 - dy, 5 + dy * 3 + dx, color);
            }
            canvas.set(8, 5, highlight);
        }
        // A wheelbarrow, parked with a few things in it.
        OrnamentKind::Wheelbarrow => {
            canvas.fill_rect(3, 7, 9, 4, outline);
            canvas.fill_rect(4, 8, 7, 2, coat);
            canvas.fill_circle(10, 12, 2, outline);
            canvas.set(10, 12, METAL_LIGHT);
            canvas.line(3, 10, 0, 11, 1, PLANK);
            canvas.line(4, 11, 4, 14, 1, outline);
            canvas.set(6, 6, LEAF);
            canvas.set(8, 6, CARROT);
            canvas.set(7, 5, LEAF_DARK);
        }
        // A round straw hive and its bees.
        OrnamentKind::Beehive => {
            canvas.fill_ellipse(8, 9, 6, 6, outline);
            canvas.fill_ellipse(8, 9, 5, 5, STRAW);
            for band in [6, 9, 12] {
                canvas.line(3, band, 13, band, 1, STRAW_DARK);
            }
            canvas.fill_rect(7, 12, 3, 2, outline);
            canvas.fill_rect(2, 14, 13, 1, PLANK);
            for (bx, by) in [(13, 3), (2, 5)] {
                canvas.set(bx, by, POLLEN);
                canvas.set(bx + 1, by, outline);
            }
        }
        // Stones balanced one on another.
        OrnamentKind::StoneCairn => {
            for (cy, rx, ry) in [(13, 5, 2), (9, 4, 2), (6, 3, 1), (3, 2, 1)] {
                canvas.fill_ellipse(8, cy, rx + 1, ry + 1, outline);
                canvas.fill_ellipse(8, cy, rx, ry, STONE);
                canvas.set(8 - rx + 1, cy - ry + 1, STONE_LIGHT);
            }
        }
        // A tiny pond with a lily pad and a flower on it.
        OrnamentKind::LilyPond => {
            canvas.fill_ellipse(8, 12, 7, 2, outline);
            canvas.fill_ellipse(8, 12, 6, 1, WATER);
            canvas.fill_ellipse(6, 12, 2, 1, LEAF);
            canvas.set(6, 12, LEAF_DARK);
            canvas.set(10, 11, highlight);
            canvas.set(11, 11, accent);
            canvas.set(12, 12, WATER_LIGHT);
            for reed in [14, 15] {
                canvas.line(reed, 12, reed, 7 + (reed - 14) * 2, 1, LEAF_DARK);
            }
        }
        // A post box on a pole.
        OrnamentKind::MailboxPost => {
            canvas.fill_rect(7, 7, 2, 8, PLANK);
            canvas.fill_rect(3, 3, 10, 5, outline);
            canvas.fill_rect(4, 4, 8, 3, coat);
            canvas.fill_rect(4, 4, 8, 1, highlight);
            canvas.line(12, 4, 12, 1, 1, outline);
            canvas.fill_rect(13, 1, 2, 2, accent);
        }
        // A tall pole with the colony's own flag.
        OrnamentKind::FlagPole => {
            canvas.fill_rect(4, 0, 1, 15, outline);
            canvas.set(4, 0, METAL_LIGHT);
            for row in 0..5 {
                let reach = 9 - row / 2;
                canvas.fill_rect(5, 1 + row, reach, 1, if row < 3 { coat } else { accent });
            }
            canvas.fill_rect(5, 1, 9, 1, highlight);
            canvas.fill_rect(2, 14, 5, 1, outline);
        }
        // A round of log to sit on, its rings showing on top.
        OrnamentKind::LogStool => {
            canvas.fill_rect(4, 8, 9, 7, outline);
            canvas.fill_rect(5, 9, 7, 5, PLANK);
            canvas.line(7, 10, 7, 13, 1, SOIL_DARK);
            canvas.fill_ellipse(8, 8, 4, 2, outline);
            canvas.fill_ellipse(8, 8, 3, 1, PLANK_LIGHT);
            canvas.set(8, 8, PLANK);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Things held up, carried, or chased.
// ---------------------------------------------------------------------------------------------

/// Small, and drawn in the middle of their cell so the overlay can centre them on a paw.
fn draw_prop(canvas: &mut Canvas, prop: PropSprite, ink: Ink) {
    let Ink {
        outline,
        coat,
        highlight,
        accent,
    } = ink;
    match prop {
        // A watering can, spout to the right.
        PropSprite::WateringCan => {
            canvas.fill_rect(4, 7, 6, 5, outline);
            canvas.fill_rect(5, 8, 4, 3, coat);
            canvas.set(5, 8, highlight);
            canvas.line(10, 9, 13, 6, 1, outline);
            canvas.set(14, 5, METAL_LIGHT);
            canvas.line(5, 6, 8, 6, 1, outline);
            canvas.set(4, 7, outline);
        }
        // Water falling from a spout, in drops.
        PropSprite::Water => {
            for (dx, dy) in [(0, 0), (2, 2), (-1, 3), (1, 5), (3, 6), (0, 7)] {
                canvas.set(8 + dx, 5 + dy, WATER_LIGHT);
                canvas.set(8 + dx, 6 + dy, WATER);
            }
        }
        PropSprite::Carrot => {
            canvas.line(6, 6, 10, 11, 2, outline);
            canvas.line(6, 6, 9, 10, 1, CARROT);
            canvas.line(5, 5, 4, 2, 1, LEAF);
            canvas.line(6, 5, 7, 2, 1, LEAF_DARK);
        }
        PropSprite::Pumpkin => {
            canvas.fill_ellipse(8, 9, 4, 3, outline);
            canvas.fill_ellipse(8, 9, 3, 2, CARROT);
            canvas.line(8, 7, 8, 11, 1, PUMPKIN_SHADE);
            canvas.set(6, 8, POLLEN);
            canvas.set(8, 5, LEAF_DARK);
        }
        PropSprite::Tomato => {
            canvas.fill_circle(8, 9, 3, outline);
            canvas.fill_circle(8, 9, 2, RED);
            canvas.set(7, 8, CREAM);
            canvas.set(8, 6, LEAF);
            canvas.set(7, 6, LEAF_DARK);
            canvas.set(9, 6, LEAF_DARK);
        }
        PropSprite::PeaPod => {
            canvas.fill_rect(4, 8, 9, 3, outline);
            canvas.fill_rect(5, 9, 7, 1, LEAF);
            for pea in [6, 8, 10] {
                canvas.set(pea, 9, LEAF_LIGHT);
            }
            canvas.set(13, 8, LEAF_DARK);
        }
        PropSprite::Strawberry => {
            canvas.fill_ellipse(8, 9, 3, 3, outline);
            canvas.fill_ellipse(8, 9, 2, 2, RED);
            canvas.set(8, 12, outline);
            canvas.set(7, 9, CREAM);
            canvas.set(9, 10, POLLEN);
            canvas.fill_rect(7, 6, 3, 1, LEAF);
        }
        PropSprite::Berries => {
            for (bx, by) in [(7, 9), (9, 9), (8, 11)] {
                canvas.fill_circle(bx, by, 1, outline);
                canvas.set(bx, by, accent);
            }
            canvas.set(8, 7, LEAF_DARK);
            canvas.set(9, 6, LEAF);
        }
        PropSprite::HerbSprig => {
            canvas.line(8, 12, 8, 5, 1, LEAF_DARK);
            for needle in (5..12).step_by(2) {
                canvas.set(7, needle, LEAF);
                canvas.set(9, needle + 1, LEAF_LIGHT);
            }
        }
        // An apple is an apple whatever the colony's colours: red, with a stalk and a leaf.
        PropSprite::Apple => {
            canvas.fill_circle(8, 9, 3, outline);
            canvas.fill_circle(8, 9, 2, RED);
            canvas.set(9, 10, RED_DARK);
            canvas.set(7, 8, CREAM);
            canvas.set(8, 6, outline);
            canvas.set(9, 5, LEAF);
        }
        PropSprite::Leaf => {
            canvas.fill_ellipse(8, 8, 3, 2, outline);
            canvas.fill_ellipse(8, 8, 2, 1, LEAF);
            canvas.line(6, 8, 10, 8, 1, LEAF_DARK);
            canvas.set(11, 9, LEAF_DARK);
            canvas.set(7, 7, LEAF_LIGHT);
        }
        PropSprite::Blossom => {
            canvas.line(8, 12, 8, 9, 1, LEAF_DARK);
            blossom(canvas, 8, 7, accent, outline);
            canvas.set(9, 11, LEAF);
        }
        PropSprite::Snore => {
            let z = |canvas: &mut Canvas, dx: i32, dy: i32, colour: Rgba| {
                canvas.line(5 + dx, 5 + dy, 10 + dx, 5 + dy, 1, colour);
                canvas.line(10 + dx, 5 + dy, 5 + dx, 10 + dy, 1, colour);
                canvas.line(5 + dx, 10 + dy, 10 + dx, 10 + dy, 1, colour);
            };
            for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                z(canvas, dx, dy, outline);
            }
            z(canvas, 0, 0, CREAM);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Belongings.
// ---------------------------------------------------------------------------------------------

fn draw_object(canvas: &mut Canvas, x: i32, kind: ColonyObjectKind, ink: Ink) {
    let Ink {
        outline,
        coat,
        highlight,
        accent,
    } = ink;
    match kind {
        ColonyObjectKind::Pillow => {
            canvas.fill_ellipse(x + 8, 11, 6, 3, outline);
            canvas.fill_ellipse(x + 8, 10, 5, 2, coat);
            canvas.set(x + 5, 9, highlight);
            canvas.set(x + 11, 11, accent);
        }
        ColonyObjectKind::Toy => {
            canvas.fill_circle(x + 8, 10, 5, outline);
            canvas.fill_circle(x + 8, 9, 4, accent);
            canvas.fill_circle(x + 6, 7, 1, highlight);
            canvas.line(x + 5, 12, x + 11, 6, 1, coat);
        }
        ColonyObjectKind::Plant => {
            canvas.fill_rect(x + 5, 11, 7, 3, outline);
            canvas.fill_rect(x + 6, 10, 5, 3, coat);
            canvas.line(x + 8, 10, x + 8, 4, 1, outline);
            canvas.fill_ellipse(x + 5, 6, 3, 2, accent);
            canvas.fill_ellipse(x + 11, 5, 3, 2, accent);
            canvas.set(x + 8, 3, highlight);
        }
        ColonyObjectKind::Blanket => {
            canvas.fill_rect(x + 2, 8, 12, 6, outline);
            canvas.fill_rect(x + 3, 8, 10, 5, coat);
            for stripe in [5, 9] {
                canvas.line(x + stripe, 8, x + stripe + 2, 13, 1, accent);
            }
        }
        ColonyObjectKind::Paper => {
            canvas.fill_rect(x + 4, 3, 9, 11, outline);
            canvas.fill_rect(x + 5, 4, 7, 9, highlight);
            canvas.line(x + 6, 7, x + 10, 7, 1, coat);
            canvas.line(x + 6, 10, x + 11, 10, 1, accent);
            canvas.set(x + 11, 4, coat);
        }
        ColonyObjectKind::Pebble => {
            canvas.fill_ellipse(x + 8, 11, 6, 3, outline);
            canvas.fill_ellipse(x + 8, 10, 5, 2, coat);
            canvas.fill_ellipse(x + 6, 9, 2, 1, highlight);
        }
        ColonyObjectKind::Lamp => {
            canvas.fill_rect(x + 7, 6, 2, 7, outline);
            canvas.fill_rect(x + 5, 13, 6, 2, outline);
            canvas.fill_ellipse(x + 8, 5, 5, 3, outline);
            canvas.fill_ellipse(x + 8, 5, 4, 2, accent);
            canvas.fill_circle(x + 8, 5, 1, highlight);
        }
        ColonyObjectKind::Cup => {
            canvas.fill_rect(x + 4, 7, 8, 7, outline);
            canvas.fill_rect(x + 5, 8, 6, 5, coat);
            canvas.fill_circle(x + 12, 10, 3, outline);
            canvas.fill_circle(x + 12, 10, 1, Rgba::TRANSPARENT);
            canvas.fill_rect(x + 5, 7, 6, 1, highlight);
            canvas.set(x + 7, 5, accent);
            canvas.set(x + 9, 4, accent);
        }
        // A diamond kite propped on its tail.
        ColonyObjectKind::Kite => {
            for row in 0..9 {
                let reach = if row < 4 { row } else { 8 - row };
                canvas.fill_rect(x + 8 - reach - 1, 2 + row, reach * 2 + 3, 1, outline);
            }
            for row in 0..9 {
                let reach = if row < 4 { row } else { 8 - row };
                if reach > 0 {
                    canvas.fill_rect(
                        x + 8 - reach,
                        2 + row,
                        reach * 2 + 1,
                        1,
                        if row < 4 { coat } else { accent },
                    );
                }
            }
            canvas.line(x + 8, 3, x + 8, 9, 1, highlight);
            canvas.line(x + 8, 11, x + 10, 14, 1, outline);
            canvas.set(x + 9, 13, accent);
        }
        // A round teapot with its spout and handle.
        ColonyObjectKind::Teapot => {
            canvas.fill_ellipse(x + 8, 10, 4, 3, outline);
            canvas.fill_ellipse(x + 8, 10, 3, 2, coat);
            canvas.set(x + 6, 9, highlight);
            canvas.line(x + 4, 10, x + 1, 7, 1, outline);
            canvas.fill_rect(x + 12, 8, 2, 4, outline);
            canvas.set(x + 12, 9, Rgba::TRANSPARENT);
            canvas.set(x + 12, 10, Rgba::TRANSPARENT);
            canvas.fill_rect(x + 7, 6, 3, 1, outline);
            canvas.set(x + 8, 5, accent);
        }
        // A book standing on end.
        ColonyObjectKind::Book => {
            canvas.fill_rect(x + 5, 3, 7, 11, outline);
            canvas.fill_rect(x + 6, 4, 5, 9, coat);
            canvas.fill_rect(x + 6, 4, 1, 9, highlight);
            canvas.line(x + 8, 6, x + 10, 6, 1, accent);
            canvas.line(x + 8, 8, x + 10, 8, 1, accent);
        }
        // A woven basket with a handle.
        ColonyObjectKind::Basket => {
            canvas.fill_rect(x + 3, 9, 10, 5, outline);
            canvas.fill_rect(x + 4, 10, 8, 3, STRAW);
            for weave in [5, 8, 11] {
                canvas.line(x + weave, 10, x + weave, 12, 1, STRAW_DARK);
            }
            for column in 4..=11 {
                let t = (column - 4) as f32 / 7.0;
                let lift = (4.0 * t * (1.0 - t) * 5.0).round() as i32;
                canvas.set(x + column, 9 - lift, outline);
            }
            canvas.set(x + 6, 8, accent);
        }
        // A ball of yarn with a loose end.
        ColonyObjectKind::YarnBall => {
            canvas.fill_circle(x + 7, 10, 4, outline);
            canvas.fill_circle(x + 7, 10, 3, accent);
            canvas.line(x + 5, 8, x + 9, 12, 1, highlight);
            canvas.line(x + 4, 11, x + 8, 7, 1, coat);
            canvas.line(x + 11, 12, x + 14, 13, 1, accent);
        }
        // A little drum and a stick.
        ColonyObjectKind::Drum => {
            canvas.fill_rect(x + 3, 8, 10, 6, outline);
            canvas.fill_rect(x + 4, 9, 8, 4, coat);
            for zig in 0..4 {
                canvas.set(x + 4 + zig * 2, 10 + zig % 2, highlight);
            }
            canvas.fill_ellipse(x + 8, 8, 5, 1, CREAM);
            canvas.line(x + 11, 6, x + 14, 3, 1, outline);
        }
        // A parasol stuck in the ground.
        ColonyObjectKind::Umbrella => {
            canvas.fill_ellipse(x + 8, 6, 7, 4, outline);
            canvas.fill_ellipse(x + 8, 6, 6, 3, coat);
            canvas.fill_rect(x + 1, 6, 15, 5, Rgba::TRANSPARENT);
            canvas.fill_rect(x + 1, 6, 15, 1, outline);
            canvas.line(x + 4, 4, x + 6, 3, 1, accent);
            canvas.line(x + 8, 6, x + 8, 14, 1, outline);
        }
        // A pail with a handle.
        ColonyObjectKind::Bucket => {
            canvas.fill_rect(x + 4, 8, 8, 6, outline);
            canvas.fill_rect(x + 5, 9, 6, 4, METAL);
            canvas.fill_rect(x + 5, 9, 6, 1, METAL_LIGHT);
            canvas.fill_rect(x + 5, 9, 1, 4, METAL_LIGHT);
            for column in 4..=11 {
                let t = (column - 4) as f32 / 7.0;
                let lift = (4.0 * t * (1.0 - t) * 3.0).round() as i32;
                canvas.set(x + column, 7 - lift, outline);
            }
            canvas.set(x + 8, 10, accent);
        }
        // A candle in a holder, alight.
        ColonyObjectKind::Candle => {
            canvas.fill_ellipse(x + 8, 13, 4, 1, outline);
            canvas.fill_rect(x + 5, 12, 6, 1, accent);
            canvas.fill_rect(x + 6, 6, 4, 6, outline);
            canvas.fill_rect(x + 7, 7, 2, 5, CREAM);
            canvas.set(x + 8, 5, outline);
            canvas.set(x + 8, 3, FIRE);
            canvas.set(x + 8, 4, FIRE_CORE);
        }
        // A music box with its lid up and its crank.
        ColonyObjectKind::MusicBox => {
            canvas.fill_rect(x + 3, 9, 10, 5, outline);
            canvas.fill_rect(x + 4, 10, 8, 3, coat);
            canvas.fill_rect(x + 4, 10, 8, 1, highlight);
            canvas.line(x + 3, 8, x + 5, 4, 1, outline);
            canvas.line(x + 5, 4, x + 12, 4, 1, outline);
            canvas.line(x + 4, 7, x + 6, 5, 1, accent);
            canvas.line(x + 13, 11, x + 14, 9, 1, outline);
        }
        // A spinning top, point down.
        ColonyObjectKind::SpinningTop => {
            for row in 0..6 {
                let reach = 5 - row;
                canvas.fill_rect(x + 8 - reach - 1, 7 + row, reach * 2 + 3, 1, outline);
                if reach > 0 {
                    canvas.fill_rect(
                        x + 8 - reach,
                        7 + row,
                        reach * 2 + 1,
                        1,
                        if row % 2 == 0 { coat } else { accent },
                    );
                }
            }
            canvas.fill_rect(x + 7, 4, 3, 3, outline);
            canvas.set(x + 8, 5, highlight);
        }
        // A glass jar with a lid and something in it.
        ColonyObjectKind::Jar => {
            canvas.fill_rect(x + 4, 6, 8, 8, outline);
            canvas.fill_rect(x + 5, 7, 6, 6, METAL_LIGHT);
            canvas.fill_rect(x + 5, 10, 6, 3, accent);
            canvas.fill_rect(x + 4, 4, 8, 2, outline);
            canvas.fill_rect(x + 5, 4, 6, 1, coat);
            canvas.set(x + 6, 8, CREAM);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cell_pixels(canvas: &Canvas, cell: u32) -> Vec<Rgba> {
        let (x0, y0) = ColonyObjectRenderer::cell_origin(cell);
        (y0..y0 + COLONY_OBJECT_SIZE)
            .flat_map(|y| (x0..x0 + COLONY_OBJECT_SIZE).map(move |x| (x, y)))
            .map(|(x, y)| canvas.get(x as i32, y as i32))
            .collect()
    }

    #[test]
    fn object_atlas_is_deterministic_bounded_and_every_cell_is_visible_and_its_own() {
        let first = ColonyObjectRenderer::render_atlas([42; 32]);
        let second = ColonyObjectRenderer::render_atlas([42; 32]);
        assert_eq!(first, second);
        assert_eq!(first.width(), COLONY_OBJECT_ATLAS_WIDTH);
        assert_eq!(first.height(), COLONY_OBJECT_ATLAS_HEIGHT);
        let mut cells: Vec<(String, u32)> = Vec::new();
        cells.extend(
            ColonyObjectKind::ALL
                .map(|kind| (format!("{kind:?}"), ColonyObjectRenderer::object_cell(kind))),
        );
        cells.extend(HangoutKind::ALL.map(|kind| {
            (
                format!("{kind:?}"),
                ColonyObjectRenderer::hangout_cell(kind),
            )
        }));
        for kind in GardenKind::ALL {
            for stage in GardenStage::ALL {
                cells.push((
                    format!("{kind:?} {stage:?}"),
                    ColonyObjectRenderer::garden_cell(kind, stage),
                ));
            }
        }
        cells.extend(OrnamentKind::ALL.map(|kind| {
            (
                format!("{kind:?}"),
                ColonyObjectRenderer::ornament_cell(kind),
            )
        }));
        cells.extend(
            PropSprite::ALL
                .map(|prop| (format!("{prop:?}"), ColonyObjectRenderer::prop_cell(prop))),
        );
        assert_eq!(cells.len() as u32, COLONY_OBJECT_CELLS);
        let mut seen = Vec::new();
        for (name, cell) in cells {
            assert!(cell < COLONY_OBJECT_CELLS, "{name} is outside the sheet");
            let pixels = cell_pixels(&first, cell);
            let opaque = pixels.iter().filter(|pixel| pixel.a > 100).count();
            assert!(
                opaque >= 12,
                "{name} should have a readable silhouette ({opaque})"
            );
            assert!(!seen.contains(&pixels), "{name} looks like another cell");
            seen.push(pixels);
        }
        assert_eq!(
            ColonyObjectRenderer::cell_uv(0),
            (
                0.0,
                0.0,
                1.0 / COLONY_OBJECT_COLUMNS as f32,
                1.0 / COLONY_OBJECT_ROWS as f32
            )
        );
    }

    /// The patches a village already had look grown exactly as they did before gardens grew.
    #[test]
    fn a_classic_patch_grown_is_the_patch_it_always_was() {
        let ink = Ink {
            outline: Rgba::new(10, 20, 30, 255),
            coat: Rgba::new(200, 100, 50, 255),
            highlight: Rgba::new(250, 240, 200, 255),
            accent: Rgba::new(60, 160, 220, 255),
        };
        for kind in [
            GardenKind::Flowers,
            GardenKind::Vegetables,
            GardenKind::Herbs,
        ] {
            let mut grown = Canvas::new(16, 16);
            draw_garden(&mut grown, 0, kind, GardenStage::Grown, ink);
            let mut classic = Canvas::new(16, 16);
            draw_grown_classic(
                &mut classic,
                0,
                kind,
                ink.outline,
                ink.coat,
                ink.highlight,
                ink.accent,
            );
            assert_eq!(grown, classic, "{kind:?}");
        }
    }
}
