use crate::{Canvas, Rgba};

pub struct MilestoneBubbleRenderer;

impl MilestoneBubbleRenderer {
    pub fn render(text: &str) -> Canvas {
        let text = text.to_ascii_uppercase();
        let character_count = text.chars().count().min(24) as u32;
        let width = (character_count * 6 + 8).max(28);
        let mut canvas = Canvas::new(width, 17);
        let outline = Rgba::new(66, 53, 72, 255);
        let paper = Rgba::new(255, 246, 224, 244);
        let ink = Rgba::new(66, 53, 72, 255);
        canvas.fill_rect(2, 1, width as i32 - 4, 13, outline);
        canvas.fill_rect(1, 3, width as i32 - 2, 9, outline);
        canvas.fill_rect(3, 2, width as i32 - 6, 11, paper);
        canvas.fill_rect(2, 4, width as i32 - 4, 7, paper);
        canvas.set(width as i32 / 2 - 2, 14, outline);
        canvas.set(width as i32 / 2 - 1, 15, outline);
        canvas.set(width as i32 / 2, 16, outline);
        canvas.set(width as i32 / 2 - 1, 14, paper);
        for (index, character) in text.chars().take(24).enumerate() {
            draw_character(&mut canvas, 4 + index as i32 * 6, 4, character, ink);
        }
        canvas
    }
}

fn draw_character(canvas: &mut Canvas, x: i32, y: i32, character: char, color: Rgba) {
    let rows = glyph(character);
    for (row, bits) in rows.into_iter().enumerate() {
        for column in 0..5 {
            if bits & (1 << (4 - column)) != 0 {
                canvas.set(x + column, y + row as i32, color);
            }
        }
    }
}

fn glyph(character: char) -> [u8; 7] {
    match character {
        'A' => [14, 17, 17, 31, 17, 17, 17],
        'B' => [30, 17, 17, 30, 17, 17, 30],
        'C' => [14, 17, 16, 16, 16, 17, 14],
        'D' => [30, 17, 17, 17, 17, 17, 30],
        'E' => [31, 16, 16, 30, 16, 16, 31],
        'F' => [31, 16, 16, 30, 16, 16, 16],
        'G' => [14, 17, 16, 23, 17, 17, 15],
        'H' => [17, 17, 17, 31, 17, 17, 17],
        'I' => [14, 4, 4, 4, 4, 4, 14],
        'J' => [7, 2, 2, 2, 18, 18, 12],
        'K' => [17, 18, 20, 24, 20, 18, 17],
        'L' => [16, 16, 16, 16, 16, 16, 31],
        'M' => [17, 27, 21, 21, 17, 17, 17],
        'N' => [17, 25, 21, 19, 17, 17, 17],
        'O' => [14, 17, 17, 17, 17, 17, 14],
        'P' => [30, 17, 17, 30, 16, 16, 16],
        'Q' => [14, 17, 17, 17, 21, 18, 13],
        'R' => [30, 17, 17, 30, 20, 18, 17],
        'S' => [15, 16, 16, 14, 1, 1, 30],
        'T' => [31, 4, 4, 4, 4, 4, 4],
        'U' => [17, 17, 17, 17, 17, 17, 14],
        'V' => [17, 17, 17, 17, 17, 10, 4],
        'W' => [17, 17, 17, 21, 21, 21, 10],
        'X' => [17, 17, 10, 4, 10, 17, 17],
        'Y' => [17, 17, 10, 4, 4, 4, 4],
        'Z' => [31, 1, 2, 4, 8, 16, 31],
        '-' => [0, 0, 0, 31, 0, 0, 0],
        ' ' => [0; 7],
        _ => [14, 17, 2, 4, 4, 0, 4],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bubble_is_created_only_to_fit_its_message() {
        let bubble = MilestoneBubbleRenderer::render("Adventurous");
        assert_eq!(bubble.height(), 17);
        assert_eq!(bubble.width(), 74);
        assert!(bubble.alpha_bounds().is_some());
    }
}
