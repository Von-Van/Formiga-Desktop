use crate::{Canvas, PALETTES, Rgba};
use formiga_core::{ColonyObjectKind, GardenKind, GroundItem, HangoutKind};

pub const COLONY_OBJECT_SIZE: u32 = 16;
/// Cells in the colony's object sheet: the eight belongings, the three hangout spots, then the
/// three garden patches.
pub const COLONY_OBJECT_CELLS: u32 = 14;

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
pub const COLONY_OBJECT_ATLAS_WIDTH: u32 = COLONY_OBJECT_SIZE * COLONY_OBJECT_CELLS;
pub const COLONY_OBJECT_ATLAS_HEIGHT: u32 = COLONY_OBJECT_SIZE;

pub struct ColonyObjectRenderer;

impl ColonyObjectRenderer {
    /// The sheet cell a belonging is drawn from.
    pub fn object_cell(kind: ColonyObjectKind) -> u32 {
        u32::from(kind.index())
    }

    /// The sheet cell a hangout spot is drawn from, after the belongings.
    pub fn hangout_cell(kind: HangoutKind) -> u32 {
        ColonyObjectKind::ALL.len() as u32 + u32::from(kind.index())
    }

    /// The sheet cell a garden patch is drawn from, after the hangout spots.
    pub fn garden_cell(kind: GardenKind) -> u32 {
        (ColonyObjectKind::ALL.len() + HangoutKind::ALL.len()) as u32 + u32::from(kind.index())
    }

    /// The sheet cell anything put down on the village ground is drawn from.
    pub fn ground_cell(item: GroundItem) -> u32 {
        match item {
            GroundItem::Hangout(kind) => Self::hangout_cell(kind),
            GroundItem::Garden(kind) => Self::garden_cell(kind),
        }
    }

    /// Whether something on the village ground is drawn turned round: only the lookout, which
    /// looks out over the open desktop, so it is turned when it stands on the right-hand half.
    pub fn ground_mirrored(item: GroundItem, x: f32, middle: f32) -> bool {
        item == GroundItem::Hangout(HangoutKind::Lookout) && x > middle
    }

    /// Where a cell sits across the sheet, as the left and right texture coordinates.
    pub fn cell_u(cell: u32) -> (f32, f32) {
        let cells = COLONY_OBJECT_CELLS as f32;
        (cell as f32 / cells, (cell + 1) as f32 / cells)
    }

    pub fn render_atlas(colony_seed: [u8; 32]) -> Canvas {
        let mut canvas = Canvas::new(COLONY_OBJECT_ATLAS_WIDTH, COLONY_OBJECT_ATLAS_HEIGHT);
        let palette = PALETTES[colony_seed[16] as usize % PALETTES.len()];
        let secondary = PALETTES[colony_seed[17] as usize % PALETTES.len()];
        for kind in ColonyObjectKind::ALL {
            // Objects rest directly on the village ground line. A soft contact shadow is the
            // only thing under them, so a keepsake reads as set down rather than shelved.
            let x = i32::from(kind.index()) * COLONY_OBJECT_SIZE as i32;
            canvas.fill_ellipse(x + 8, 14, 6, 2, Rgba::new(20, 24, 26, 70));
            draw_object(
                &mut canvas,
                i32::from(kind.index()) * COLONY_OBJECT_SIZE as i32,
                kind,
                palette.outline,
                palette.coat,
                palette.highlight,
                secondary.accent,
            );
        }
        for kind in HangoutKind::ALL {
            let x = Self::hangout_cell(kind) as i32 * COLONY_OBJECT_SIZE as i32;
            canvas.fill_ellipse(x + 8, 14, 7, 2, Rgba::new(20, 24, 26, 70));
            draw_hangout(
                &mut canvas,
                x,
                kind,
                palette.outline,
                palette.coat,
                palette.highlight,
                secondary.accent,
            );
        }
        for kind in GardenKind::ALL {
            let x = Self::garden_cell(kind) as i32 * COLONY_OBJECT_SIZE as i32;
            canvas.fill_ellipse(x + 8, 14, 7, 2, Rgba::new(20, 24, 26, 70));
            draw_garden(
                &mut canvas,
                x,
                kind,
                palette.outline,
                palette.coat,
                palette.highlight,
                secondary.accent,
            );
        }
        canvas
    }
}

/// The patches the person at the desk plants: low and wide, set into the ground rather than put
/// down on it, and green first, so a garden reads as growing where a belonging reads as kept.
fn draw_garden(
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
        GardenKind::Herbs => {
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

/// The spots the person at the desk puts down: bigger and lower than a belonging, since they are
/// somewhere to be rather than something to have. The lookout's spyglass points to the right;
/// the overlay mirrors it where the open desktop lies to its left.
fn draw_hangout(
    canvas: &mut Canvas,
    x: i32,
    kind: HangoutKind,
    outline: Rgba,
    coat: Rgba,
    highlight: Rgba,
    accent: Rgba,
) {
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
    }
}

fn draw_object(
    canvas: &mut Canvas,
    x: i32,
    kind: ColonyObjectKind,
    outline: Rgba,
    coat: Rgba,
    highlight: Rgba,
    accent: Rgba,
) {
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn object_atlas_is_deterministic_bounded_and_every_kind_is_visible() {
        let first = ColonyObjectRenderer::render_atlas([42; 32]);
        let second = ColonyObjectRenderer::render_atlas([42; 32]);
        assert_eq!(first, second);
        assert_eq!(first.width(), COLONY_OBJECT_ATLAS_WIDTH);
        assert_eq!(first.height(), COLONY_OBJECT_ATLAS_HEIGHT);
        let cells = ColonyObjectKind::ALL
            .into_iter()
            .map(|kind| (format!("{kind:?}"), ColonyObjectRenderer::object_cell(kind)))
            .chain(HangoutKind::ALL.into_iter().map(|kind| {
                (
                    format!("{kind:?}"),
                    ColonyObjectRenderer::hangout_cell(kind),
                )
            }))
            .chain(
                GardenKind::ALL
                    .into_iter()
                    .map(|kind| (format!("{kind:?}"), ColonyObjectRenderer::garden_cell(kind))),
            );
        let mut seen = Vec::new();
        for (name, cell) in cells {
            assert!(cell < COLONY_OBJECT_CELLS, "{name} is outside the sheet");
            let start = cell * COLONY_OBJECT_SIZE;
            let pixels: Vec<Rgba> = (start..start + COLONY_OBJECT_SIZE)
                .flat_map(|x| (0..COLONY_OBJECT_SIZE).map(move |y| (x, y)))
                .map(|(x, y)| first.get(x as i32, y as i32))
                .collect();
            let opaque = pixels.iter().filter(|pixel| pixel.a > 0).count();
            assert!(opaque >= 12, "{name} should have a readable silhouette");
            assert!(!seen.contains(&pixels), "{name} looks like another cell");
            seen.push(pixels);
        }
        assert_eq!(ColonyObjectRenderer::cell_u(0), (0.0, 1.0 / 14.0));
        assert_eq!(
            ColonyObjectRenderer::garden_cell(GardenKind::Herbs),
            COLONY_OBJECT_CELLS - 1
        );
    }
}
