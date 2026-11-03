//! The talk sheet: every bubble a companion says something in, and every picture that goes inside
//! one (see [`formiga_core::talk`]).
//!
//! Bubbles are drawn twice as finely as the companions: each art pixel of a bubble is
//! [`TALK_DETAIL`] texture pixels across, so a heart, a ball or a moon inside one has room for a
//! curve and a highlight and reads at a glance, while the bubble still sits at the companions' own
//! size. Every bubble is the same shape and size, with the same edge and the same tail; how a thing
//! is said shows only in the colour of its paper, so the eye goes to the picture. A bubble is put
//! together as it is drawn: its frame, then its picture on it.

use crate::{Canvas, Rgba, SpriteRect};
use formiga_core::BubbleGrowth;
use formiga_core::talk::{TalkStyle, TalkTopic};

/// Texture pixels to each art pixel of a bubble.
pub const TALK_DETAIL: u32 = 2;
/// Every bubble, in art pixels, tail included.
pub const TALK_BUBBLE: (u32, u32) = (20, 17);
/// A picture, in art pixels.
pub const TALK_ICON: u32 = 10;
/// Where the picture goes on a bubble, in art pixels from its top-left.
pub const TALK_ICON_AT: (u32, u32) = (5, 2);
/// The two steps a bubble grows in by and shrinks out by.
const SMALL: (u32, u32) = (8, 7);
const MEDIUM: (u32, u32) = (14, 12);

const ICON_COLUMNS: u32 = 16;
const ROW_FULL: u32 = 0;
const ROW_GROWING: u32 = TALK_BUBBLE.1 * TALK_DETAIL;
const ROW_ICONS: u32 = ROW_GROWING + MEDIUM.1 * TALK_DETAIL;
pub const TALK_SHEET_WIDTH: u32 = ICON_COLUMNS * TALK_ICON * TALK_DETAIL;
pub const TALK_SHEET_HEIGHT: u32 =
    ROW_ICONS + (TalkTopic::ALL.len() as u32).div_ceil(ICON_COLUMNS) * TALK_ICON * TALK_DETAIL;

/// The one edge every bubble has.
const PLUM: Rgba = Rgba::new(66, 53, 72, 255);

/// The paper each way of saying something is written on: all light, so every picture reads on
/// every one, and different enough in hue to tell apart.
const fn paper(style: TalkStyle) -> Rgba {
    match style {
        TalkStyle::Chat => Rgba::new(255, 248, 230, 250),
        TalkStyle::Excited => Rgba::new(255, 232, 150, 250),
        TalkStyle::Whisper => Rgba::new(236, 228, 246, 250),
        TalkStyle::Thought => Rgba::new(238, 242, 248, 250),
        TalkStyle::Song => Rgba::new(212, 234, 252, 250),
        TalkStyle::Tender => Rgba::new(255, 222, 232, 250),
        TalkStyle::Grumble => Rgba::new(200, 204, 214, 250),
        TalkStyle::Shout => Rgba::new(255, 210, 176, 250),
    }
}

/// Where things are on the talk sheet, in texture pixels.
pub struct TalkSheet;

impl TalkSheet {
    /// The frame for `style` at `growth`. Growing in and out, every bubble is the same plain
    /// cream shape, a little smaller each step.
    pub fn frame(style: TalkStyle, growth: BubbleGrowth) -> SpriteRect {
        let column = TalkStyle::ALL.iter().position(|s| *s == style).unwrap_or(0) as u32;
        let (size, x, y) = match growth {
            BubbleGrowth::Small => (SMALL, 0, ROW_GROWING),
            BubbleGrowth::Medium => (MEDIUM, SMALL.0 * TALK_DETAIL, ROW_GROWING),
            BubbleGrowth::Full => (TALK_BUBBLE, column * TALK_BUBBLE.0 * TALK_DETAIL, ROW_FULL),
        };
        SpriteRect {
            x,
            y,
            width: size.0 * TALK_DETAIL,
            height: size.1 * TALK_DETAIL,
        }
    }

    /// A picture.
    pub fn icon(topic: TalkTopic) -> SpriteRect {
        let index = topic.index() as u32;
        let cell = TALK_ICON * TALK_DETAIL;
        SpriteRect {
            x: index % ICON_COLUMNS * cell,
            y: ROW_ICONS + index / ICON_COLUMNS * cell,
            width: cell,
            height: cell,
        }
    }

    pub fn render() -> Canvas {
        let mut sheet = Canvas::new(TALK_SHEET_WIDTH, TALK_SHEET_HEIGHT);
        for style in TalkStyle::ALL {
            let rect = Self::frame(style, BubbleGrowth::Full);
            let mut tile = Canvas::new(rect.width, rect.height);
            draw_frame(&mut tile, paper(style));
            blit(&mut sheet, &tile, rect.x, rect.y);
        }
        for growth in [BubbleGrowth::Small, BubbleGrowth::Medium] {
            let rect = Self::frame(TalkStyle::Chat, growth);
            let mut tile = Canvas::new(rect.width, rect.height);
            draw_frame(&mut tile, paper(TalkStyle::Chat));
            blit(&mut sheet, &tile, rect.x, rect.y);
        }
        for topic in TalkTopic::ALL {
            let rect = Self::icon(topic);
            let mut tile = Canvas::new(rect.width, rect.height);
            draw_icon(&mut tile, topic);
            blit(&mut sheet, &tile, rect.x, rect.y);
        }
        sheet
    }
}

fn blit(target: &mut Canvas, source: &Canvas, x0: u32, y0: u32) {
    for y in 0..source.height() as i32 {
        for x in 0..source.width() as i32 {
            let pixel = source.get(x, y);
            if pixel.a > 0 {
                target.set(x0 as i32 + x, y0 as i32 + y, pixel);
            }
        }
    }
}

/// A bubble filling `tile`: a rounded body with one art pixel of plum edge, and a tail tapering
/// to the middle of the bottom row.
fn draw_frame(tile: &mut Canvas, paper: Rgba) {
    let (w, h) = (tile.width() as i32, tile.height() as i32);
    let d = TALK_DETAIL as i32;
    let tail_rows = 3 * d;
    let (bx0, by0, bx1, by1) = (d, d, w - d - 1, h - tail_rows - 1);
    let corner = (3 * d).min((by1 - by0) / 2);
    let rounded = |tile: &mut Canvas, grow: i32, colour: Rgba| {
        let (x0, y0, x1, y1) = (bx0 - grow, by0 - grow, bx1 + grow, by1 + grow);
        let r = corner + grow;
        tile.fill_rect(x0 + r, y0, x1 - x0 + 1 - 2 * r, y1 - y0 + 1, colour);
        tile.fill_rect(x0, y0 + r, x1 - x0 + 1, y1 - y0 + 1 - 2 * r, colour);
        for (x, y) in [
            (x0 + r, y0 + r),
            (x1 - r, y0 + r),
            (x0 + r, y1 - r),
            (x1 - r, y1 - r),
        ] {
            tile.fill_circle(x, y, r, colour);
        }
    };
    let tail = |tile: &mut Canvas, grow: i32, colour: Rgba| {
        for row in 0..tail_rows + grow {
            let half = (tail_rows - row) / 2 + grow;
            tile.fill_rect(w / 2 - half, by1 + row, half * 2, 1, colour);
        }
    };
    rounded(tile, d, PLUM);
    tail(tile, d, PLUM);
    rounded(tile, 0, paper);
    tail(tile, 0, paper);
}

const INK: Rgba = Rgba::new(58, 42, 50, 255);
const HEART: Rgba = Rgba::new(224, 70, 96, 255);
const HEART_LIGHT: Rgba = Rgba::new(255, 170, 186, 255);
const GOLD: Rgba = Rgba::new(244, 190, 60, 255);
const GOLD_LIGHT: Rgba = Rgba::new(255, 236, 160, 255);
const LEAFY: Rgba = Rgba::new(90, 160, 80, 255);
const BALL: Rgba = Rgba::new(80, 140, 214, 255);
const BALL_LIGHT: Rgba = Rgba::new(196, 224, 252, 255);
const WOOD: Rgba = Rgba::new(160, 110, 64, 255);
const RED: Rgba = Rgba::new(214, 70, 64, 255);
const BLUE: Rgba = Rgba::new(70, 120, 200, 255);
const NIGHT: Rgba = Rgba::new(250, 226, 130, 255);
const STORM: Rgba = Rgba::new(150, 154, 170, 255);
const BOLT: Rgba = Rgba::new(255, 222, 80, 255);
const SKIN: Rgba = Rgba::new(250, 214, 180, 255);
const GREEN: Rgba = Rgba::new(120, 190, 96, 255);

fn heart(tile: &mut Canvas, x: i32, y: i32, r: i32, fill: Rgba) {
    tile.fill_circle(x - r, y, r + 1, INK);
    tile.fill_circle(x + r, y, r + 1, INK);
    for row in 0..=2 * r + 2 {
        let half = 2 * r + 1 - row;
        if half >= 0 {
            tile.fill_rect(x - half - 1, y + row, half * 2 + 3, 1, INK);
        }
    }
    tile.fill_circle(x - r, y, r, fill);
    tile.fill_circle(x + r, y, r, fill);
    for row in 0..=2 * r {
        let half = 2 * r - row;
        if half >= 0 {
            tile.fill_rect(x - half, y + row, half * 2 + 1, 1, fill);
        }
    }
}

/// `rows` drawn at `x`, `y` in `fill`, one pixel of ink round every filled pixel.
fn stamp(tile: &mut Canvas, x: i32, y: i32, rows: &[&str], fill: Rgba) {
    let filled = |dx: i32, dy: i32| {
        dy >= 0
            && dx >= 0
            && rows
                .get(dy as usize)
                .and_then(|row| row.as_bytes().get(dx as usize))
                == Some(&b'#')
    };
    for (dy, row) in rows.iter().enumerate() {
        for (dx, code) in row.bytes().enumerate() {
            if code != b'#' {
                continue;
            }
            for (ox, oy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                let (nx, ny) = (dx as i32 + ox, dy as i32 + oy);
                if !filled(nx, ny) {
                    tile.set(x + nx, y + ny, INK);
                }
            }
        }
    }
    for (dy, row) in rows.iter().enumerate() {
        for (dx, code) in row.bytes().enumerate() {
            if code == b'#' {
                tile.set(x + dx as i32, y + dy as i32, fill);
            }
        }
    }
}

/// A four-pointed twinkle, `r` from its middle to each point.
fn star(tile: &mut Canvas, x: i32, y: i32, r: i32, fill: Rgba) {
    for (grow, colour) in [(1, INK), (0, fill)] {
        for i in -r - grow..=r + grow {
            let half = ((r + grow - i.abs()) / 3).max(0) + grow;
            tile.fill_rect(x - half, y + i, half * 2 + 1, 1, colour);
            tile.fill_rect(x + i, y - half, 1, half * 2 + 1, colour);
        }
    }
}

fn blob(tile: &mut Canvas, x: i32, y: i32, rx: i32, ry: i32, fill: Rgba) {
    tile.fill_ellipse(x, y, rx + 1, ry + 1, INK);
    tile.fill_ellipse(x, y, rx, ry, fill);
}

fn stroke(tile: &mut Canvas, from: (i32, i32), to: (i32, i32), fill: Rgba) {
    tile.line(from.0, from.1, to.0, to.1, 3, INK);
    tile.line(from.0, from.1, to.0, to.1, 1, fill);
}

/// A picture filling its 20-pixel tile.
fn draw_icon(tile: &mut Canvas, topic: TalkTopic) {
    match topic {
        TalkTopic::Heart => {
            heart(tile, 10, 7, 3, HEART);
            tile.set(6, 6, HEART_LIGHT);
            tile.set(7, 5, HEART_LIGHT);
        }
        // A broken heart, green with envy: a whole heart with a jagged crack open down its
        // middle, edged in ink on both sides.
        TalkTopic::CrackedHeart => {
            heart(tile, 10, 6, 4, GREEN);
            tile.set(5, 5, Rgba::new(190, 236, 170, 255));
            tile.set(6, 4, Rgba::new(190, 236, 170, 255));
            for y in 3..17 {
                let x = 10 + [0, 1, 1, 0, -1, -1][(y as usize) % 6];
                if tile.get(x, y).a == 0 || tile.get(x, y) == INK {
                    continue;
                }
                tile.set(x - 1, y, INK);
                tile.set(x, y, Rgba::TRANSPARENT);
                tile.set(x + 1, y, INK);
            }
        }
        // A paw held up and waving: a round pad with three toes close over it, and the lines of
        // the wave beside it.
        TalkTopic::Wave => {
            blob(tile, 9, 12, 5, 4, SKIN);
            for (x, y) in [(5, 7), (9, 5), (13, 7)] {
                blob(tile, x, y, 2, 2, SKIN);
            }
            for (x, y) in [(16, 10), (17, 14)] {
                tile.line(x, y, x + 2, y - 2, 1, INK);
            }
        }
        TalkTopic::Food => {
            blob(tile, 10, 11, 5, 5, RED);
            tile.set(7, 9, HEART_LIGHT);
            tile.set(8, 8, HEART_LIGHT);
            tile.line(10, 6, 11, 3, 2, WOOD);
            blob(tile, 13, 4, 2, 1, LEAFY);
        }
        // A blue ball with its seams curving round it and a shine on top.
        TalkTopic::Ball => {
            blob(tile, 10, 10, 7, 7, BALL);
            let white = Rgba::new(255, 255, 255, 255);
            for step in 0..14 {
                let t = step as f32 / 13.0 * std::f32::consts::PI;
                tile.set(5 + (t.sin() * 3.0) as i32, 4 + step, white);
                tile.set(15 - (t.sin() * 3.0) as i32, 4 + step, white);
            }
            tile.fill_circle(7, 6, 1, BALL_LIGHT);
        }
        // A wooden toy sword: play-fighting with sticks.
        TalkTopic::Stick => {
            // The blade, a short guard across it, and the grip below.
            stroke(tile, (8, 11), (16, 3), Rgba::new(210, 214, 222, 255));
            stroke(tile, (5, 10), (9, 14), GOLD);
            stroke(tile, (6, 13), (3, 16), WOOD);
        }
        // An open umbrella with its hooked handle.
        TalkTopic::Umbrella => {
            tile.fill_ellipse(10, 9, 8, 6, INK);
            tile.fill_ellipse(10, 9, 7, 5, RED);
            tile.fill_rect(1, 10, 19, 6, Rgba::TRANSPARENT);
            tile.fill_rect(2, 9, 17, 1, INK);
            for x in [5, 10, 15] {
                tile.line(x, 5, x, 8, 1, Rgba::new(250, 160, 146, 255));
            }
            tile.line(10, 9, 10, 16, 1, INK);
            tile.line(10, 16, 8, 17, 1, INK);
            tile.set(7, 16, INK);
        }
        TalkTopic::Kite => {
            for row in -5..=5_i32 {
                let half = 5 - row.abs();
                tile.fill_rect(10 - half - 1, 7 + row, half * 2 + 3, 1, INK);
            }
            for row in -4..=4_i32 {
                let half = 4 - row.abs();
                tile.fill_rect(
                    10 - half,
                    7 + row,
                    half * 2 + 1,
                    1,
                    if row < 0 { GOLD } else { BLUE },
                );
            }
            tile.line(10, 12, 7, 18, 1, INK);
        }
        // A fish side on: a round body, a forked tail, an eye and a fin.
        TalkTopic::Fish => {
            blob(tile, 8, 10, 6, 4, BALL);
            for row in -4..=4_i32 {
                let reach = 2 + row.abs() / 2;
                tile.fill_rect(14, 10 + row, reach + 1, 1, INK);
                tile.fill_rect(14, 10 + row, reach, 1, BALL);
            }
            tile.fill_circle(5, 9, 1, INK);
            tile.line(7, 13, 10, 13, 1, BALL_LIGHT);
            tile.line(8, 6, 10, 5, 1, INK);
        }
        TalkTopic::Note => {
            blob(tile, 7, 15, 3, 2, BLUE);
            tile.fill_rect(9, 4, 2, 11, INK);
            tile.fill_rect(9, 3, 7, 3, INK);
            tile.fill_rect(10, 4, 5, 1, BLUE);
        }
        TalkTopic::Question => {
            tile.fill_rect(6, 3, 8, 3, INK);
            tile.fill_rect(12, 3, 3, 6, INK);
            tile.fill_rect(9, 8, 5, 3, INK);
            tile.fill_rect(9, 8, 3, 5, INK);
            tile.fill_rect(9, 15, 3, 3, INK);
        }
        TalkTopic::Exclaim => {
            tile.fill_rect(8, 2, 4, 11, INK);
            tile.fill_rect(9, 3, 2, 9, RED);
            tile.fill_rect(8, 15, 4, 3, INK);
        }
        TalkTopic::Sparkle => {
            star(tile, 10, 10, 8, GOLD);
            tile.fill_circle(10, 10, 1, GOLD_LIGHT);
        }
        // A crescent moon with a small star, set pixel by pixel so its curve is smooth.
        TalkTopic::Moon => {
            stamp(
                tile,
                3,
                3,
                &[
                    "...####...",
                    ".######...",
                    ".#####....",
                    "#####.....",
                    "####......",
                    "####......",
                    "####......",
                    "####......",
                    "#####.....",
                    ".#####....",
                    ".######...",
                    "...####...",
                ],
                NIGHT,
            );
            stamp(tile, 13, 4, &[".#.", "###", ".#."], GOLD);
        }
        // Zz, as Zs: each a bar across, a clean stroke from its right end down to the left, and
        // a bar across, set pixel by pixel so no stray pixel hangs off the stroke.
        TalkTopic::Sleep => {
            stamp(
                tile,
                2,
                8,
                &[
                    "#########",
                    "#########",
                    ".....###.",
                    "....###..",
                    "...###...",
                    "..###....",
                    ".###.....",
                    "#########",
                    "#########",
                ],
                BLUE,
            );
            stamp(
                tile,
                12,
                2,
                &["#####", "...#.", "..#..", ".#...", "#####"],
                BLUE,
            );
        }
        TalkTopic::Home => {
            for row in 0..6 {
                tile.fill_rect(10 - row - 2, 3 + row, row * 2 + 5, 1, INK);
                tile.fill_rect(10 - row - 1, 3 + row, row * 2 + 3, 1, RED);
            }
            tile.fill_rect(4, 9, 13, 8, INK);
            tile.fill_rect(5, 9, 11, 7, SKIN);
            tile.fill_rect(9, 12, 3, 4, WOOD);
        }
        TalkTopic::Storm => {
            blob(tile, 7, 8, 4, 3, STORM);
            blob(tile, 12, 7, 5, 4, STORM);
            tile.line(11, 11, 8, 15, 2, BOLT);
            tile.line(8, 15, 12, 15, 2, BOLT);
            tile.line(12, 15, 9, 18, 2, BOLT);
        }
        // A spiral, wound in from the outside: dizzy, or something spinning.
        TalkTopic::Spiral => {
            // Two and a half turns, a fine line with a little room between them.
            let mut last = (10, 10);
            for step in 0..120 {
                let t = step as f32 * 0.13;
                let r = 0.48 * t;
                let at = (
                    10 + (r * t.cos()).round() as i32,
                    10 + (r * t.sin()).round() as i32,
                );
                tile.line(last.0, last.1, at.0, at.1, 1, INK);
                last = at;
            }
        }
        // A Formiga's face, shy: round with two ears, big dark eyes looking down and pink cheeks.
        TalkTopic::Blush => {
            let coat = Rgba::new(176, 206, 236, 255);
            blob(tile, 5, 4, 2, 2, coat);
            blob(tile, 15, 4, 2, 2, coat);
            blob(tile, 10, 11, 7, 6, coat);
            for x in [7, 13] {
                tile.fill_ellipse(x, 11, 1, 2, INK);
                tile.set(x, 10, Rgba::new(255, 255, 255, 255));
            }
            for x in [4, 16] {
                tile.fill_ellipse(x, 14, 2, 1, HEART_LIGHT);
            }
            tile.line(9, 15, 11, 15, 1, INK);
        }
        // Two big eyes looking over: keeping an eye on somebody.
        TalkTopic::Eye => {
            for x in [6, 14] {
                tile.fill_ellipse(x, 10, 4, 5, INK);
                tile.fill_ellipse(x, 10, 3, 4, Rgba::new(255, 255, 255, 255));
                tile.fill_ellipse(x + 1, 11, 2, 3, INK);
                tile.set(x + 2, 9, Rgba::new(255, 255, 255, 255));
            }
        }
        TalkTopic::Dots => {
            for x in [4, 10, 16] {
                tile.fill_circle(x, 11, 2, INK);
            }
        }
        TalkTopic::No => {
            tile.line(5, 5, 15, 15, 4, INK);
            tile.line(15, 5, 5, 15, 4, INK);
            tile.line(5, 5, 15, 15, 2, RED);
            tile.line(15, 5, 5, 15, 2, RED);
        }
        // A paw print: four toes over a round pad. Running about, chasing, tag.
        TalkTopic::Footprints => {
            stamp(
                tile,
                4,
                3,
                &[
                    "...##.##....",
                    "...##.##....",
                    "............",
                    "##.......##.",
                    "##.......##.",
                    "............",
                    "...######...",
                    "..########..",
                    ".##########.",
                    ".##########.",
                    "..########..",
                    "...######...",
                ],
                WOOD,
            );
        }
        TalkTopic::Flag => {
            tile.fill_rect(5, 3, 2, 15, INK);
            tile.fill_rect(7, 3, 10, 7, INK);
            for row in 0..5 {
                for col in 0..8 {
                    let fill = if (row + col) % 2 == 0 {
                        Rgba::new(255, 255, 255, 255)
                    } else {
                        INK
                    };
                    tile.set(8 + col, 4 + row, fill);
                }
            }
        }
        // A sun: a round face and eight short rays, set pixel by pixel.
        TalkTopic::Sun => {
            stamp(
                tile,
                3,
                3,
                &[
                    "......#......",
                    "......#......",
                    "..#.......#..",
                    "....#####....",
                    "...#######...",
                    "...#######...",
                    "##.#######.##",
                    "...#######...",
                    "...#######...",
                    "....#####....",
                    "..#.......#..",
                    "......#......",
                    "......#......",
                ],
                GOLD,
            );
        }
        TalkTopic::Flower => {
            for (dx, dy) in [(-3, 0), (3, 0), (0, -3), (0, 3)] {
                blob(tile, 10 + dx, 8 + dy, 2, 2, HEART_LIGHT);
            }
            blob(tile, 10, 8, 1, 1, GOLD);
            tile.line(10, 12, 10, 18, 1, LEAFY);
        }
        TalkTopic::Blocks => {
            for (x, y, fill) in [(4, 11, RED), (11, 11, BLUE), (7, 4, GOLD)] {
                tile.fill_rect(x, y, 7, 7, INK);
                tile.fill_rect(x + 1, y + 1, 5, 5, fill);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_talk_sheet_holds_every_frame_and_picture_in_its_own_place() {
        let sheet = TalkSheet::render();
        assert_eq!(sheet, TalkSheet::render());
        let mut rects = Vec::new();
        for style in TalkStyle::ALL {
            let rect = TalkSheet::frame(style, BubbleGrowth::Full);
            // Every bubble is the same size.
            assert_eq!(
                (rect.width, rect.height),
                (TALK_BUBBLE.0 * TALK_DETAIL, TALK_BUBBLE.1 * TALK_DETAIL)
            );
            rects.push(rect);
        }
        rects.push(TalkSheet::frame(TalkStyle::Chat, BubbleGrowth::Small));
        rects.push(TalkSheet::frame(TalkStyle::Chat, BubbleGrowth::Medium));
        let mut pictures = Vec::new();
        for topic in TalkTopic::ALL {
            let rect = TalkSheet::icon(topic);
            rects.push(rect);
            let pixels: Vec<Rgba> = (rect.y..rect.y + rect.height)
                .flat_map(|y| (rect.x..rect.x + rect.width).map(move |x| (x, y)))
                .map(|(x, y)| sheet.get(x as i32, y as i32))
                .collect();
            let opaque = pixels.iter().filter(|pixel| pixel.a > 100).count();
            assert!(opaque >= 20, "{topic:?} is too faint to read");
            assert!(!pictures.contains(&pixels), "{topic:?} looks like another");
            pictures.push(pixels);
        }
        for (i, a) in rects.iter().enumerate() {
            assert!(a.x + a.width <= TALK_SHEET_WIDTH && a.y + a.height <= TALK_SHEET_HEIGHT);
            for b in &rects[i + 1..] {
                assert!(!a.overlaps(b), "{a:?} and {b:?} overlap");
            }
        }
    }

    /// Every bubble is one shape: the same pixels are paper in every style and edge in every
    /// style, so only the paper's colour tells one from another.
    #[test]
    fn every_bubble_is_the_same_shape_in_its_own_colour() {
        let sheet = TalkSheet::render();
        let shape = |style| {
            let rect = TalkSheet::frame(style, BubbleGrowth::Full);
            (rect.y..rect.y + rect.height)
                .flat_map(|y| (rect.x..rect.x + rect.width).map(move |x| (x, y)))
                .map(|(x, y)| {
                    let pixel = sheet.get(x as i32, y as i32);
                    (pixel.a > 0, pixel == PLUM)
                })
                .collect::<Vec<_>>()
        };
        let chat = shape(TalkStyle::Chat);
        let mut papers = std::collections::BTreeSet::new();
        for style in TalkStyle::ALL {
            assert_eq!(shape(style), chat, "{style:?} is shaped differently");
            let c = paper(style);
            papers.insert((c.r, c.g, c.b));
        }
        assert_eq!(papers.len(), TalkStyle::ALL.len());
    }

    /// A Z for sleep reads as a Z: its stroke runs from the top bar's right end down to the
    /// bottom bar's left end, not the other way.
    #[test]
    fn the_sleep_picture_is_a_z() {
        let mut tile = Canvas::new(TALK_ICON * TALK_DETAIL, TALK_ICON * TALK_DETAIL);
        draw_icon(&mut tile, TalkTopic::Sleep);
        // Between the big Z's bars, high on the right and low on the left is drawn; high on the
        // left and low on the right is not.
        assert_eq!(tile.get(8, 10), BLUE);
        assert_eq!(tile.get(5, 13), BLUE);
        assert_eq!(tile.get(3, 11), Rgba::TRANSPARENT);
        assert_eq!(tile.get(9, 13), Rgba::TRANSPARENT);
    }
}
