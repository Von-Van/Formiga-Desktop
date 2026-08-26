use crate::{Canvas, PALETTES, Rgba};
use formiga_core::{ShelterGenome, ShelterStyle};

pub const SHELTER_SIZE: u32 = 64;

pub struct ShelterRenderer;

impl ShelterRenderer {
    pub fn render(genome: &ShelterGenome) -> Canvas {
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
        canvas
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
}
