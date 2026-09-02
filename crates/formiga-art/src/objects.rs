use crate::{Canvas, PALETTES, Rgba};
use formiga_core::ColonyObjectKind;

pub const COLONY_OBJECT_SIZE: u32 = 16;
pub const COLONY_OBJECT_ATLAS_WIDTH: u32 = COLONY_OBJECT_SIZE * 8;
pub const COLONY_OBJECT_ATLAS_HEIGHT: u32 = COLONY_OBJECT_SIZE;

pub struct ColonyObjectRenderer;

impl ColonyObjectRenderer {
    pub fn render_atlas(colony_seed: [u8; 32]) -> Canvas {
        let mut canvas = Canvas::new(COLONY_OBJECT_ATLAS_WIDTH, COLONY_OBJECT_ATLAS_HEIGHT);
        let palette = PALETTES[colony_seed[16] as usize % PALETTES.len()];
        let secondary = PALETTES[colony_seed[17] as usize % PALETTES.len()];
        for kind in ColonyObjectKind::ALL {
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
        canvas
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
        for kind in ColonyObjectKind::ALL {
            let start = u32::from(kind.index()) * COLONY_OBJECT_SIZE;
            let opaque = (start..start + COLONY_OBJECT_SIZE)
                .flat_map(|x| (0..COLONY_OBJECT_SIZE).map(move |y| (x, y)))
                .filter(|(x, y)| first.get(*x as i32, *y as i32).a > 0)
                .count();
            assert!(opaque >= 12, "{kind:?} should have a readable silhouette");
        }
    }
}
