//! The playthings a companion picks up that are not its own toy: the colony's belongings in their
//! held size, and the props a game needs — a stick at each angle it is swung through, a fishing
//! rod and what it catches, an umbrella open, closed and upturned, a kite, a paper plane, a tower
//! of blocks going up and coming down, a drum, a paper boat. Each is drawn in its own colours with
//! the shared dark edge, like everything held.

use super::*;

const EDGE: Rgba = Rgba::new(58, 42, 50, 255);
const WOOD: Rgba = Rgba::new(150, 104, 62, 255);
const WOOD_LIGHT: Rgba = Rgba::new(204, 156, 100, 255);
const RED: Rgba = Rgba::new(214, 70, 64, 255);
const RED_LIGHT: Rgba = Rgba::new(250, 160, 146, 255);
const BLUE: Rgba = Rgba::new(84, 144, 214, 255);
const BLUE_LIGHT: Rgba = Rgba::new(188, 222, 250, 255);
const YELLOW: Rgba = Rgba::new(240, 198, 70, 255);
const YELLOW_LIGHT: Rgba = Rgba::new(255, 238, 176, 255);
const PAPER: Rgba = Rgba::new(246, 240, 226, 255);
const PAPER_SHADE: Rgba = Rgba::new(200, 192, 178, 255);
const STONE: Rgba = Rgba::new(158, 154, 148, 255);
const STONE_LIGHT: Rgba = Rgba::new(208, 204, 196, 255);
const WOOL: Rgba = Rgba::new(196, 116, 190, 255);
const WOOL_LIGHT: Rgba = Rgba::new(236, 180, 230, 255);
const FISH: Rgba = Rgba::new(120, 170, 200, 255);
const FISH_LIGHT: Rgba = Rgba::new(206, 232, 242, 255);
const CUSHION: Rgba = Rgba::new(230, 170, 120, 255);
const CUSHION_LIGHT: Rgba = Rgba::new(252, 216, 176, 255);
const STRING: Rgba = Rgba::new(120, 110, 104, 255);

/// A plaything's picture, other than the eight toys, the snacks, the cups and the foods.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Plaything {
    YarnBall,
    Pebble,
    Pillow,
    /// A stick at one of four angles: upright, leaning forward, level, and pointing down.
    Stick(u8),
    /// A fishing rod with its line out, the float riding high or pulled under.
    Rod(bool),
    Fish,
    UmbrellaClosed,
    UmbrellaOpen,
    UmbrellaUpturned,
    Kite,
    Plane,
    /// A tower of one, two or three blocks.
    Tower(u8),
    Toppled,
    Drum,
    MusicBox,
    Boat,
}

impl Plaything {
    pub const ALL: [Self; 22] = [
        Self::YarnBall,
        Self::Pebble,
        Self::Pillow,
        Self::Stick(0),
        Self::Stick(1),
        Self::Stick(2),
        Self::Stick(3),
        Self::Rod(false),
        Self::Rod(true),
        Self::Fish,
        Self::UmbrellaClosed,
        Self::UmbrellaOpen,
        Self::UmbrellaUpturned,
        Self::Kite,
        Self::Plane,
        Self::Tower(1),
        Self::Tower(2),
        Self::Tower(3),
        Self::Toppled,
        Self::Drum,
        Self::MusicBox,
        Self::Boat,
    ];

    pub fn index(self) -> u32 {
        Self::ALL
            .iter()
            .position(|other| *other == self)
            .unwrap_or(0) as u32
    }
}

fn blob(canvas: &mut Canvas, x: i32, y: i32, rx: i32, ry: i32, fill: Rgba) {
    canvas.fill_ellipse(x, y, rx + 1, ry + 1, EDGE);
    canvas.fill_ellipse(x, y, rx, ry, fill);
}

fn stick(canvas: &mut Canvas, from: (i32, i32), to: (i32, i32)) {
    canvas.line(from.0, from.1, to.0, to.1, 3, EDGE);
    canvas.line(from.0, from.1, to.0, to.1, 1, WOOD_LIGHT);
}

/// Draws `thing` with its middle at `x`, `y`.
pub(super) fn draw_plaything(canvas: &mut Canvas, thing: Plaything, x: i32, y: i32) {
    match thing {
        Plaything::YarnBall => {
            blob(canvas, x, y, 4, 4, WOOL);
            canvas.line(x - 3, y - 1, x + 2, y - 3, 1, WOOL_LIGHT);
            canvas.line(x - 3, y + 2, x + 3, y - 1, 1, WOOL_LIGHT);
            canvas.line(x + 3, y + 3, x + 6, y + 5, 1, WOOL);
        }
        Plaything::Pebble => {
            blob(canvas, x, y + 1, 4, 3, STONE);
            canvas.set(x - 2, y, STONE_LIGHT);
            canvas.set(x - 1, y - 1, STONE_LIGHT);
        }
        Plaything::Pillow => {
            canvas.fill_rect(x - 6, y - 4, 13, 9, EDGE);
            canvas.fill_rect(x - 5, y - 3, 11, 7, CUSHION);
            canvas.fill_rect(x - 4, y - 2, 5, 2, CUSHION_LIGHT);
            canvas.set(x, y, EDGE);
        }
        Plaything::Stick(angle) => match angle % 4 {
            0 => stick(canvas, (x, y + 7), (x, y - 7)),
            1 => stick(canvas, (x - 5, y + 5), (x + 5, y - 5)),
            2 => stick(canvas, (x - 7, y), (x + 7, y)),
            _ => stick(canvas, (x - 3, y - 6), (x + 3, y + 7)),
        },
        Plaything::Rod(bite) => {
            stick(canvas, (x - 6, y + 4), (x + 3, y - 6));
            let float = if bite { y + 6 } else { y + 4 };
            canvas.line(x + 3, y - 6, x + 6, float - 1, 1, STRING);
            blob(canvas, x + 6, float, 1, 1, RED);
        }
        Plaything::Fish => {
            blob(canvas, x - 1, y, 4, 2, FISH);
            canvas.fill_rect(x + 3, y - 2, 1, 5, EDGE);
            canvas.fill_rect(x + 4, y - 3, 2, 7, EDGE);
            canvas.fill_rect(x + 4, y - 2, 1, 5, FISH);
            canvas.set(x - 3, y - 1, EDGE);
            canvas.line(x - 2, y + 1, x + 1, y + 1, 1, FISH_LIGHT);
        }
        Plaything::UmbrellaClosed => {
            stick(canvas, (x, y + 7), (x, y - 6));
            canvas.fill_rect(x - 2, y - 5, 5, 8, EDGE);
            canvas.fill_rect(x - 1, y - 4, 3, 6, RED);
            canvas.line(x, y + 7, x + 2, y + 7, 1, EDGE);
        }
        Plaything::UmbrellaOpen => {
            canvas.fill_ellipse(x, y - 1, 7, 4, EDGE);
            canvas.fill_ellipse(x, y - 1, 6, 3, RED);
            canvas.fill_rect(x - 7, y, 15, 4, Rgba::TRANSPARENT);
            canvas.fill_rect(x - 7, y, 15, 1, EDGE);
            for dx in [-4, 0, 4] {
                canvas.fill_rect(x + dx - 1, y - 3, 2, 3, RED_LIGHT);
            }
            canvas.line(x, y, x, y + 6, 1, EDGE);
            canvas.line(x, y + 6, x + 2, y + 6, 1, EDGE);
        }
        Plaything::UmbrellaUpturned => {
            canvas.fill_ellipse(x, y, 7, 4, EDGE);
            canvas.fill_ellipse(x, y, 6, 3, RED);
            canvas.fill_rect(x - 7, y - 4, 15, 4, Rgba::TRANSPARENT);
            canvas.fill_rect(x - 7, y, 15, 1, EDGE);
            for dx in [-4, 0, 4] {
                canvas.fill_rect(x + dx - 1, y + 1, 2, 2, RED_LIGHT);
            }
            canvas.line(x + 6, y, x + 7, y - 5, 1, EDGE);
        }
        Plaything::Kite => {
            for row in -4..=4_i32 {
                let half = 4 - row.abs();
                canvas.fill_rect(x - half - 1, y + row - 2, half * 2 + 3, 1, EDGE);
            }
            for row in -3..=3_i32 {
                let half = 3 - row.abs();
                let fill = if row < 0 { YELLOW } else { BLUE };
                canvas.fill_rect(x - half, y + row - 2, half * 2 + 1, 1, fill);
            }
            canvas.line(x, y + 2, x - 2, y + 7, 1, STRING);
            canvas.set(x - 1, y + 5, RED);
            canvas.set(x - 2, y + 7, RED);
        }
        Plaything::Plane => {
            canvas.fill_rect(x - 6, y, 13, 2, EDGE);
            canvas.fill_rect(x - 5, y, 11, 1, PAPER);
            for i in 0..5 {
                canvas.fill_rect(x - 4 + i, y - 4 + i, 6 - i, 1, PAPER_SHADE);
                canvas.set(x - 5 + i, y - 4 + i, EDGE);
            }
            canvas.set(x + 6, y - 1, EDGE);
        }
        Plaything::Tower(blocks) => {
            let fills = [RED, YELLOW, BLUE];
            for i in 0..i32::from(blocks.clamp(1, 3)) {
                let by = y + 5 - i * 5;
                canvas.fill_rect(x - 3, by - 2, 7, 6, EDGE);
                canvas.fill_rect(x - 2, by - 1, 5, 4, fills[i as usize]);
                canvas.set(x - 1, by, Rgba::new(255, 255, 255, 140));
            }
        }
        Plaything::Toppled => {
            let fills = [RED, YELLOW, BLUE];
            for (i, (dx, dy)) in [(-5, 5), (1, 6), (5, 3)].into_iter().enumerate() {
                canvas.fill_rect(x + dx - 2, y + dy - 2, 6, 5, EDGE);
                canvas.fill_rect(x + dx - 1, y + dy - 1, 4, 3, fills[i]);
            }
        }
        Plaything::Drum => {
            canvas.fill_rect(x - 5, y - 2, 11, 8, EDGE);
            canvas.fill_rect(x - 4, y - 1, 9, 6, RED);
            canvas.fill_ellipse(x, y - 2, 5, 2, EDGE);
            canvas.fill_ellipse(x, y - 2, 4, 1, PAPER);
            canvas.line(x - 4, y + 1, x + 4, y + 4, 1, YELLOW_LIGHT);
            canvas.line(x - 4, y + 4, x + 4, y + 1, 1, YELLOW_LIGHT);
        }
        Plaything::MusicBox => {
            canvas.fill_rect(x - 5, y - 3, 11, 8, EDGE);
            canvas.fill_rect(x - 4, y - 2, 9, 6, WOOD);
            canvas.fill_rect(x - 4, y - 2, 9, 1, WOOD_LIGHT);
            canvas.line(x + 5, y, x + 7, y - 2, 1, YELLOW);
            canvas.set(x + 7, y - 3, YELLOW_LIGHT);
        }
        Plaything::Boat => {
            for row in 0..3 {
                canvas.fill_rect(x - 6 + row, y + 2 + row, 13 - row * 2, 1, EDGE);
                canvas.fill_rect(x - 5 + row, y + 2 + row, 11 - row * 2, 1, PAPER);
            }
            for i in 0..5 {
                canvas.fill_rect(x - i / 2, y + 1 - i, 1 + i / 2 * 2, 1, PAPER_SHADE);
            }
            canvas.set(x, y - 4, EDGE);
            canvas.fill_rect(x - 6, y + 5, 13, 1, BLUE_LIGHT);
        }
    }
}
