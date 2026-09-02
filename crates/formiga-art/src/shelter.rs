use crate::{Canvas, PALETTES, Rgba};
use formiga_core::{ShelterDecorationKind, ShelterGenome, ShelterStyle};

pub const SHELTER_SIZE: u32 = 64;

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
        let palette = PALETTES[genome.palette_index as usize % PALETTES.len()];
        let accent = PALETTES[genome.accent_index as usize % PALETTES.len()];
        let width = i32::from(genome.width).clamp(34, 42);
        let height = i32::from(genome.height).clamp(27, 36);
        let left = 32 - width / 2;
        let bottom = 61;
        let top = bottom - height;

        // A single-pixel ground shadow keeps every generated shelter readable on bright desktops.
        canvas.fill_ellipse(32, bottom, width / 2 + 2, 3, Rgba::new(20, 24, 26, 95));
        match genome.style {
            ShelterStyle::LeafTent => {
                fill_triangle(
                    &mut canvas,
                    32,
                    top,
                    left - 2,
                    bottom - 2,
                    left + width + 2,
                    bottom - 2,
                    palette.outline,
                );
                fill_triangle(
                    &mut canvas,
                    32,
                    top + 3,
                    left + 2,
                    bottom - 3,
                    left + width - 2,
                    bottom - 3,
                    palette.coat,
                );
                canvas.line(32, top + 3, 32, bottom - 5, 1, palette.highlight);
            }
            ShelterStyle::MushroomHut => {
                canvas.fill_ellipse(
                    32,
                    bottom - height / 3,
                    width / 3,
                    height / 2,
                    palette.outline,
                );
                canvas.fill_ellipse(
                    32,
                    bottom - height / 3,
                    width / 3 - 2,
                    height / 2 - 2,
                    palette.highlight,
                );
                canvas.fill_ellipse(32, top + 8, width / 2 + 2, 10, palette.outline);
                canvas.fill_ellipse(32, top + 7, width / 2, 8, palette.coat);
            }
            ShelterStyle::CushionDen => {
                canvas.fill_ellipse(32, bottom - 8, width / 2 + 2, 10, palette.outline);
                canvas.fill_ellipse(32, bottom - 9, width / 2, 8, palette.coat);
                canvas.fill_ellipse(
                    32,
                    top + height / 2,
                    width / 2 - 3,
                    height / 2,
                    palette.outline,
                );
                canvas.fill_ellipse(
                    32,
                    top + height / 2 + 1,
                    width / 2 - 5,
                    height / 2 - 2,
                    palette.shadow,
                );
            }
            ShelterStyle::PaperHouse => {
                canvas.fill_rect(left, top + 10, width, height - 10, palette.outline);
                canvas.fill_rect(left + 2, top + 12, width - 4, height - 13, palette.coat);
                fill_triangle(
                    &mut canvas,
                    32,
                    top - 1,
                    left - 3,
                    top + 14,
                    left + width + 3,
                    top + 14,
                    palette.outline,
                );
                fill_triangle(
                    &mut canvas,
                    32,
                    top + 2,
                    left + 1,
                    top + 12,
                    left + width - 1,
                    top + 12,
                    accent.coat,
                );
            }
        }

        let door_width = (width / 4).max(8);
        let door_height = (height / 3).max(10);
        canvas.fill_ellipse(
            32,
            bottom - door_height / 2 - 1,
            door_width / 2 + 2,
            door_height / 2 + 2,
            palette.outline,
        );
        canvas.fill_rect(
            32 - door_width / 2 - 2,
            bottom - door_height / 2,
            door_width + 4,
            door_height / 2 + 1,
            palette.outline,
        );
        canvas.fill_ellipse(
            32,
            bottom - door_height / 2,
            door_width / 2,
            door_height / 2,
            Rgba::new(25, 23, 31, 255),
        );
        canvas.fill_rect(
            32 - door_width / 2,
            bottom - door_height / 2,
            door_width,
            door_height / 2 + 1,
            Rgba::new(25, 23, 31, 255),
        );
        canvas.set(32 + door_width / 3, bottom - door_height / 2, accent.accent);

        for index in 0..5_u32 {
            let byte = ((genome.detail_seed >> (index * 8)) & 0xff) as i32;
            let x = left + 5 + byte.rem_euclid((width - 10).max(1));
            let y = top + 5 + (byte / 7).rem_euclid((height / 2).max(1));
            if canvas.get(x, y).a > 0 {
                canvas.set(x, y, accent.accent);
            }
        }
        for kind in decorations
            .iter()
            .copied()
            .take(formiga_core::MAX_SHELTER_DECORATIONS)
        {
            draw_decoration(
                &mut canvas,
                kind,
                palette.outline,
                palette.coat,
                accent.accent,
                accent.highlight,
                genome.detail_seed,
            );
        }
        canvas
    }
}

fn draw_decoration(
    canvas: &mut Canvas,
    kind: ShelterDecorationKind,
    outline: Rgba,
    coat: Rgba,
    accent: Rgba,
    highlight: Rgba,
    detail_seed: u64,
) {
    let jitter = ((detail_seed >> (kind.index() * 7)) & 0x3) as i32 - 1;
    match kind {
        ShelterDecorationKind::Leaf => {
            let x = 18 + jitter;
            canvas.line(x, 31, x + 2, 21, 1, outline);
            canvas.fill_ellipse(x - 1, 23, 4, 2, coat);
            canvas.fill_ellipse(x + 3, 27, 4, 2, highlight);
            canvas.line(x, 24, x + 4, 27, 1, accent);
        }
        ShelterDecorationKind::Banner => {
            canvas.line(15, 18, 49, 18, 1, outline);
            for (index, x) in [18, 26, 34, 42].into_iter().enumerate() {
                fill_triangle(
                    canvas,
                    x + 2,
                    19,
                    x,
                    24,
                    x + 5,
                    24,
                    if index % 2 == 0 { accent } else { highlight },
                );
            }
        }
        ShelterDecorationKind::Stone => {
            let x = 12 + jitter;
            canvas.fill_ellipse(x, 57, 6, 4, outline);
            canvas.fill_ellipse(x, 56, 4, 2, coat);
            canvas.set(x + 2, 55, highlight);
        }
        ShelterDecorationKind::Flower => {
            let x = 15 + jitter;
            canvas.line(x, 59, x, 49, 1, coat);
            canvas.fill_ellipse(x - 2, 50, 3, 2, accent);
            canvas.fill_ellipse(x + 2, 50, 3, 2, accent);
            canvas.fill_ellipse(x, 47, 2, 3, highlight);
            canvas.set(x, 50, outline);
        }
        ShelterDecorationKind::Lamp => {
            let x = 50 + jitter;
            canvas.line(x, 59, x, 44, 1, outline);
            canvas.fill_rect(x - 4, 43, 9, 2, outline);
            canvas.fill_ellipse(x, 40, 5, 5, outline);
            canvas.fill_ellipse(x, 40, 3, 3, highlight);
            canvas.set(x, 40, accent);
        }
        ShelterDecorationKind::RoofOrnament => {
            let x = 32 + jitter;
            canvas.line(x, 27, x, 12, 1, outline);
            canvas.line(x - 4, 15, x + 4, 15, 1, accent);
            canvas.line(x, 11, x, 19, 1, accent);
            canvas.line(x - 3, 12, x + 3, 18, 1, highlight);
            canvas.line(x - 3, 18, x + 3, 12, 1, highlight);
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
