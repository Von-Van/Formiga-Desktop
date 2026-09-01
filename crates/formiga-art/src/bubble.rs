use crate::{Canvas, Rgba};

pub struct MilestoneBubbleRenderer;

impl MilestoneBubbleRenderer {
    pub fn render() -> Canvas {
        let width = 28;
        let mut canvas = Canvas::new(width, 17);
        let outline = Rgba::new(66, 53, 72, 255);
        let paper = Rgba::new(255, 246, 224, 244);
        canvas.fill_rect(2, 1, width as i32 - 4, 13, outline);
        canvas.fill_rect(1, 3, width as i32 - 2, 9, outline);
        canvas.fill_rect(3, 2, width as i32 - 6, 11, paper);
        canvas.fill_rect(2, 4, width as i32 - 4, 7, paper);
        canvas.set(width as i32 / 2 - 2, 14, outline);
        canvas.set(width as i32 / 2 - 1, 15, outline);
        canvas.set(width as i32 / 2, 16, outline);
        canvas.set(width as i32 / 2 - 1, 14, paper);
        canvas
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn milestone_thought_bubble_is_blank_and_fixed_size() {
        let bubble = MilestoneBubbleRenderer::render();
        assert_eq!(bubble.height(), 17);
        assert_eq!(bubble.width(), 28);
        assert!(bubble.alpha_bounds().is_some());
        assert_eq!(bubble.get(14, 7), Rgba::new(255, 246, 224, 244));
    }
}
