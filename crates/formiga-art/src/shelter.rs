use crate::{Canvas, PALETTES, Rgba};
use formiga_core::{ShelterDecorationKind, ShelterGenome, ShelterStyle};

pub const SHELTER_SIZE: u32 = 64;

/// Two-by-two grid of shelter cells: the colony house, a cottage, and a mini's cottage.
pub const VILLAGE_ATLAS_SIZE: u32 = SHELTER_SIZE * 2;

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
        draw_dwelling(&mut canvas, genome, decorations, 32, 61, 3);
        canvas
    }

    /// The colony house and its companion cottages in one cached texture. Cells share the
    /// house's own style, palette, and baseline, so a mini's home matches the one it grew up
    /// beside. Three cells of the existing shelter size: no new texture, sampler, or bind group.
    pub fn render_village(genome: &ShelterGenome, decorations: &[ShelterDecorationKind]) -> Canvas {
        let mut canvas = Canvas::new(VILLAGE_ATLAS_SIZE, VILLAGE_ATLAS_SIZE);
        let cell = SHELTER_SIZE as i32;
        // Each dwelling is drawn into its own cell-sized tile first. Art that would run past
        // a cell is clipped exactly as it is for a lone shelter, never bleeding into the
        // neighbour below. Only the colony house carries the decorations it earned over time.
        for (span, decorations, origin_x, origin_y) in [
            (3, decorations, 0, 0),
            (2, &[][..], cell, 0),
            (1, &[][..], 0, cell),
        ] {
            let mut tile = Canvas::new(SHELTER_SIZE, SHELTER_SIZE);
            draw_dwelling(&mut tile, genome, decorations, 32, 61, span);
            for y in 0..cell {
                for x in 0..cell {
                    let pixel = tile.get(x, y);
                    if pixel.a > 0 {
                        canvas.set(origin_x + x, origin_y + y, pixel);
                    }
                }
            }
        }
        canvas
    }
}

/// Draws one dwelling into `canvas`, centred on `cx` and standing on `bottom`. `span` scales
/// the house in thirds: 3 is the colony house, 2 a companion cottage, 1 a mini's.
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
    let width = (i32::from(genome.width).clamp(34, 42) * span / 3).max(16);
    let height = (i32::from(genome.height).clamp(27, 36) * span / 3).max(13);
    let left = cx - width / 2;
    let top = bottom - height;
    // Style details scale with the dwelling, so a mini's house keeps the proportions of the
    // colony house it matches. At full span this resolves to the original constants exactly.
    let unit = |value: i32| (value * span / 3).max(1);

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
        let width = (i32::from(genome.width).clamp(34, 42) * span / 3).max(16);
        let height = (i32::from(genome.height).clamp(27, 36) * span / 3).max(13);
        let top = bottom - height;
        let half = width / 2;
        match genome.style {
            ShelterStyle::LeafTent => Self {
                cx,
                peak_y: top,
                eave_y: top + height * 6 / 10,
                eave_half: (half + 2) * 6 / 10,
                wall_y: top + height * 7 / 10,
                wall_half: (half + 2) * 7 / 10,
                ground_y: bottom - 2,
                ground_half: half + 2,
            },
            ShelterStyle::MushroomHut => Self {
                cx,
                peak_y: top - 2,
                eave_y: top + 14,
                eave_half: (half + 2) * 4 / 5,
                wall_y: bottom - height / 3,
                wall_half: width / 3,
                ground_y: bottom,
                ground_half: width / 3,
            },
            ShelterStyle::CushionDen => Self {
                cx,
                peak_y: top,
                eave_y: top + height / 3,
                eave_half: (half - 3) * 9 / 10,
                wall_y: top + height / 2,
                wall_half: half - 3,
                ground_y: bottom - 2,
                ground_half: half + 2,
            },
            ShelterStyle::PaperHouse => Self {
                cx,
                peak_y: top - 1,
                eave_y: top + 13,
                eave_half: half + 3,
                wall_y: top + 10 + (height - 10) / 2,
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
            canvas.line(29, y, 35, y, 1, accent);
            canvas.line(frame.cx, y - 3, frame.cx, y + 3, 1, accent);
            canvas.line(30, y - 2, 34, y + 2, 1, highlight);
            canvas.line(30, y + 2, 34, y - 2, 1, highlight);
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
            assert_eq!(village.width(), VILLAGE_ATLAS_SIZE);
            assert_eq!(village.height(), VILLAGE_ATLAS_SIZE);
            let cell = SHELTER_SIZE as i32;
            for (span, decorations, origin_x, origin_y) in [
                (3, &ShelterDecorationKind::ALL[..], 0, 0),
                (2, &[][..], cell, 0),
                (1, &[][..], 0, cell),
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
            // The unused fourth cell stays empty, so nothing has spilled sideways or down.
            for y in cell..cell * 2 {
                for x in cell..cell * 2 {
                    assert_eq!(village.get(x, y).a, 0, "{style:?} bled into the spare cell");
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
