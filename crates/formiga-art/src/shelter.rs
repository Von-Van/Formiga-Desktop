use crate::{Canvas, PALETTES, Rgba};
use formiga_core::{ShelterDecorationKind, ShelterGenome, ShelterStyle};

pub const SHELTER_SIZE: u32 = 64;

/// Two-by-two grid of shelter cells: the colony house, a cottage, a mini's cottage, and the
/// keepsake tree.
pub const VILLAGE_ATLAS_SIZE: u32 = SHELTER_SIZE * 2;

/// How large each dwelling draws, in twelfths of the colony house. A companion cottage is only
/// a little smaller than the house it stands beside — small enough that the colony house is
/// plainly the main building, big enough to read as a home next to a 48px creature — and a
/// mini's cottage is two thirds of a cottage rather than a model of one.
pub const MAIN_SPAN: i32 = 12;
pub const COTTAGE_SPAN: i32 = 10;
pub const MINI_COTTAGE_SPAN: i32 = 7;

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
        draw_dwelling(&mut canvas, genome, decorations, 32, 61, MAIN_SPAN);
        canvas
    }

    /// The colony house, its companion cottages, and the keepsake tree in one cached texture.
    /// Cells share the house's own style, palette, and baseline, so a mini's home matches the one
    /// it grew up beside and the tree belongs to the same yard. Four cells of the existing
    /// shelter size: no new texture, sampler, or bind group.
    pub fn render_village(genome: &ShelterGenome, decorations: &[ShelterDecorationKind]) -> Canvas {
        let mut canvas = Canvas::new(VILLAGE_ATLAS_SIZE, VILLAGE_ATLAS_SIZE);
        let cell = SHELTER_SIZE as i32;
        // Each cell is drawn into its own cell-sized tile first. Art that would run past a cell
        // is clipped exactly as it is for a lone shelter, never bleeding into the neighbour
        // below. Only the colony house carries the decorations it earned over time.
        for (span, decorations, origin_x, origin_y) in [
            (MAIN_SPAN, decorations, 0, 0),
            (COTTAGE_SPAN, &[][..], cell, 0),
            (MINI_COTTAGE_SPAN, &[][..], 0, cell),
        ] {
            let mut tile = Canvas::new(SHELTER_SIZE, SHELTER_SIZE);
            draw_dwelling(&mut tile, genome, decorations, 32, 61, span);
            blit_cell(&mut canvas, &tile, origin_x, origin_y);
        }
        let mut tile = Canvas::new(SHELTER_SIZE, SHELTER_SIZE);
        crate::tree::draw_tree(&mut tile, genome, 32, 61);
        blit_cell(&mut canvas, &tile, cell, cell);
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
fn draw_dwelling(
    canvas: &mut Canvas,
    genome: &ShelterGenome,
    decorations: &[ShelterDecorationKind],
    cx: i32,
    bottom: i32,
    span: i32,
) {
    let palette = PALETTES[genome.palette_index as usize % PALETTES.len()];
    let accent = PALETTES[genome.accent_index as usize % PALETTES.len()];
    let (width, height) = dwelling_size(genome, span);
    let left = cx - width / 2;
    let top = bottom - height;
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
    match genome.style {
        ShelterStyle::LeafTent => {
            fill_triangle(
                canvas,
                cx,
                top,
                left - 2,
                bottom - 2,
                left + width + 2,
                bottom - 2,
                palette.outline,
            );
            fill_triangle(
                canvas,
                cx,
                top + 3,
                left + 2,
                bottom - 3,
                left + width - 2,
                bottom - 3,
                palette.coat,
            );
            canvas.line(cx, top + 3, cx, bottom - 5, 1, palette.highlight);
        }
        ShelterStyle::MushroomHut => {
            canvas.fill_ellipse(
                cx,
                bottom - height / 3,
                width / 3,
                height / 2,
                palette.outline,
            );
            canvas.fill_ellipse(
                cx,
                bottom - height / 3,
                width / 3 - unit(2),
                height / 2 - unit(2),
                palette.highlight,
            );
            canvas.fill_ellipse(
                cx,
                top + unit(8),
                width / 2 + unit(2),
                unit(10),
                palette.outline,
            );
            canvas.fill_ellipse(cx, top + unit(7), width / 2, unit(8), palette.coat);
        }
        ShelterStyle::CushionDen => {
            canvas.fill_ellipse(
                cx,
                bottom - unit(8),
                width / 2 + unit(2),
                unit(10),
                palette.outline,
            );
            canvas.fill_ellipse(cx, bottom - unit(9), width / 2, unit(8), palette.coat);
            canvas.fill_ellipse(
                cx,
                top + height / 2,
                width / 2 - unit(3),
                height / 2,
                palette.outline,
            );
            canvas.fill_ellipse(
                cx,
                top + height / 2 + unit(1),
                width / 2 - unit(5),
                height / 2 - unit(2),
                palette.shadow,
            );
        }
        ShelterStyle::PaperHouse => {
            canvas.fill_rect(
                left,
                top + unit(10),
                width,
                height - unit(10),
                palette.outline,
            );
            canvas.fill_rect(
                left + unit(2),
                top + unit(12),
                width - unit(4),
                height - unit(13),
                palette.coat,
            );
            fill_triangle(
                canvas,
                cx,
                top - unit(1),
                left - unit(3),
                top + unit(14),
                left + width + unit(3),
                top + unit(14),
                palette.outline,
            );
            fill_triangle(
                canvas,
                cx,
                top + unit(2),
                left + unit(1),
                top + unit(12),
                left + width - unit(1),
                top + unit(12),
                accent.coat,
            );
        }
    }

    let door_width = (width / 4).clamp(5, width / 2);
    let door_height = (height / 3).clamp(6, height / 2);
    canvas.fill_ellipse(
        cx,
        bottom - door_height / 2 - 1,
        door_width / 2 + 2,
        door_height / 2 + 2,
        palette.outline,
    );
    canvas.fill_rect(
        cx - door_width / 2 - 2,
        bottom - door_height / 2,
        door_width + 4,
        door_height / 2 + 1,
        palette.outline,
    );
    canvas.fill_ellipse(
        cx,
        bottom - door_height / 2,
        door_width / 2,
        door_height / 2,
        Rgba::new(25, 23, 31, 255),
    );
    canvas.fill_rect(
        cx - door_width / 2,
        bottom - door_height / 2,
        door_width,
        door_height / 2 + 1,
        Rgba::new(25, 23, 31, 255),
    );
    canvas.set(cx + door_width / 3, bottom - door_height / 2, accent.accent);

    for index in 0..5_u32 {
        let byte = ((genome.detail_seed >> (index * 8)) & 0xff) as i32;
        let x = left + unit(5) + byte.rem_euclid((width - unit(10)).max(1));
        let y = top + unit(5) + (byte / 7).rem_euclid((height / 2).max(1));
        if canvas.get(x, y).a > 0 {
            canvas.set(x, y, accent.accent);
        }
    }
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
            ShelterStyle::MushroomHut => Self {
                cx,
                peak_y: top - unit(2),
                eave_y: top + unit(14),
                eave_half: (half + unit(2)) * 4 / 5,
                wall_y: bottom - height / 3,
                wall_half: width / 3,
                ground_y: bottom,
                ground_half: width / 3,
            },
            ShelterStyle::CushionDen => Self {
                cx,
                peak_y: top,
                eave_y: top + height / 3,
                eave_half: (half - unit(3)) * 9 / 10,
                wall_y: top + height / 2,
                wall_half: half - unit(3),
                ground_y: bottom - unit(2),
                ground_half: half + unit(2),
            },
            ShelterStyle::PaperHouse => Self {
                cx,
                peak_y: top - unit(1),
                eave_y: top + unit(13),
                eave_half: half + unit(3),
                wall_y: top + unit(10) + (height - unit(10)) / 2,
                wall_half: half,
                ground_y: bottom,
                ground_half: half,
            },
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
            canvas.fill_ellipse(x, y, 2, 3, highlight);
            canvas.set(x, y, accent);
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
            let village = ShelterRenderer::render_village(&genome, &ShelterDecorationKind::ALL);
            // One 128x128 texture, four cells, however much the village grows.
            assert_eq!(VILLAGE_ATLAS_SIZE, 128);
            assert_eq!(village.width(), VILLAGE_ATLAS_SIZE);
            assert_eq!(village.height(), VILLAGE_ATLAS_SIZE);
            let cell = SHELTER_SIZE as i32;
            for (span, decorations, origin_x, origin_y) in [
                (MAIN_SPAN, &ShelterDecorationKind::ALL[..], 0, 0),
                (COTTAGE_SPAN, &[][..], cell, 0),
                (MINI_COTTAGE_SPAN, &[][..], 0, cell),
            ] {
                let mut tile = Canvas::new(SHELTER_SIZE, SHELTER_SIZE);
                draw_dwelling(&mut tile, &genome, decorations, 32, 61, span);
                assert!(
                    tile.alpha_bounds().is_some(),
                    "{style:?} span {span} should draw a house"
                );
                for y in 0..cell {
                    for x in 0..cell {
                        assert_eq!(
                            village.get(origin_x + x, origin_y + y),
                            tile.get(x, y),
                            "{style:?} span {span} differs at ({x}, {y})"
                        );
                    }
                }
            }
            // The colony house cell is exactly the standalone shelter it replaces.
            let alone =
                ShelterRenderer::render_with_decorations(&genome, &ShelterDecorationKind::ALL);
            for y in 0..cell {
                for x in 0..cell {
                    assert_eq!(village.get(x, y), alone.get(x, y));
                }
            }
            // The fourth cell holds the keepsake tree, and nothing else has spilled into it.
            let mut tree = Canvas::new(SHELTER_SIZE, SHELTER_SIZE);
            crate::tree::draw_tree(&mut tree, &genome, 32, 61);
            assert!(tree.alpha_bounds().is_some(), "{style:?} draws no tree");
            for y in 0..cell {
                for x in 0..cell {
                    assert_eq!(
                        village.get(cell + x, cell + y),
                        tree.get(x, y),
                        "{style:?} differs from the tree at ({x}, {y})"
                    );
                }
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
                        draw_dwelling(&mut tile, &genome, &[], 32, 61, span);
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
                    for span in [MAIN_SPAN, COTTAGE_SPAN, MINI_COTTAGE_SPAN] {
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
                        draw_dwelling(&mut tile, &genome, &[], 32, 61, span);
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
