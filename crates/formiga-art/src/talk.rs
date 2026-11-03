//! The talk sheet: every bubble a companion says something in, and every picture that goes inside
//! one (see [`formiga_core::talk`]).
//!
//! Bubbles are drawn twice as finely as the companions: each art pixel of a bubble is
//! [`TALK_DETAIL`] texture pixels across, so a heart, a ball or a moon inside one has room for a
//! curve and a highlight and reads at a glance, while the bubble still sits at the companions' own
//! size. A bubble is put together as it is drawn: its frame, then one or two pictures on it.

use crate::{Canvas, Rgba, SpriteRect};
use formiga_core::BubbleGrowth;
use formiga_core::talk::{TalkStyle, TalkTopic};

/// Texture pixels to each art pixel of a bubble.
pub const TALK_DETAIL: u32 = 2;
/// A bubble with one picture, in art pixels, tail included.
pub const TALK_SINGLE: (u32, u32) = (21, 18);
/// A bubble with two pictures side by side.
pub const TALK_DOUBLE: (u32, u32) = (31, 18);
/// A picture, in art pixels.
pub const TALK_ICON: u32 = 10;
/// Where a single bubble's picture goes on it, and a double's two, in art pixels from its
/// top-left.
pub const TALK_ICON_AT: [(u32, u32); 3] = [(5, 3), (5, 3), (16, 3)];
const SMALL: (u32, u32) = (9, 8);
const MEDIUM: (u32, u32) = (13, 12);

const ICON_COLUMNS: u32 = 16;
const ROW_SINGLE: u32 = 0;
const ROW_DOUBLE: u32 = TALK_SINGLE.1 * TALK_DETAIL;
const ROW_GROWING: u32 = ROW_DOUBLE + TALK_DOUBLE.1 * TALK_DETAIL;
const ROW_ICONS: u32 = ROW_GROWING + MEDIUM.1 * TALK_DETAIL;
pub const TALK_SHEET_WIDTH: u32 = 8 * TALK_DOUBLE.0 * TALK_DETAIL;
pub const TALK_SHEET_HEIGHT: u32 =
    ROW_ICONS + (TalkTopic::ALL.len() as u32).div_ceil(ICON_COLUMNS) * TALK_ICON * TALK_DETAIL;

const PLUM: Rgba = Rgba::new(66, 53, 72, 255);
const CREAM: Rgba = Rgba::new(255, 246, 224, 246);
const SUN: Rgba = Rgba::new(255, 226, 120, 250);
const SUN_EDGE: Rgba = Rgba::new(196, 120, 40, 255);
const MIST: Rgba = Rgba::new(250, 248, 244, 190);
const MIST_EDGE: Rgba = Rgba::new(150, 140, 156, 220);
const CLOUD: Rgba = Rgba::new(244, 246, 252, 246);
const SKY: Rgba = Rgba::new(208, 230, 252, 248);
const SKY_EDGE: Rgba = Rgba::new(60, 98, 160, 255);
const ROSE: Rgba = Rgba::new(255, 222, 230, 248);
const ROSE_EDGE: Rgba = Rgba::new(176, 70, 104, 255);
const SLATE: Rgba = Rgba::new(98, 100, 118, 250);
const SLATE_EDGE: Rgba = Rgba::new(40, 40, 54, 255);

/// Where things are on the talk sheet, in texture pixels.
pub struct TalkSheet;

impl TalkSheet {
    /// The frame for `style` at `growth`, wide enough for two pictures or one. Growing in and
    /// out, every bubble is the same plain shape.
    pub fn frame(style: TalkStyle, double: bool, growth: BubbleGrowth) -> SpriteRect {
        let column = TalkStyle::ALL.iter().position(|s| *s == style).unwrap_or(0) as u32;
        let (size, x, y) = match growth {
            BubbleGrowth::Small => (SMALL, 0, ROW_GROWING),
            BubbleGrowth::Medium => (MEDIUM, SMALL.0 * TALK_DETAIL, ROW_GROWING),
            BubbleGrowth::Full if double => (
                TALK_DOUBLE,
                column * TALK_DOUBLE.0 * TALK_DETAIL,
                ROW_DOUBLE,
            ),
            BubbleGrowth::Full => (
                TALK_SINGLE,
                column * TALK_SINGLE.0 * TALK_DETAIL,
                ROW_SINGLE,
            ),
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
            for double in [false, true] {
                let rect = Self::frame(style, double, BubbleGrowth::Full);
                let mut tile = Canvas::new(rect.width, rect.height);
                draw_frame(&mut tile, style);
                blit(&mut sheet, &tile, rect.x, rect.y);
            }
        }
        for growth in [BubbleGrowth::Small, BubbleGrowth::Medium] {
            let rect = Self::frame(TalkStyle::Chat, false, growth);
            let mut tile = Canvas::new(rect.width, rect.height);
            draw_frame(&mut tile, TalkStyle::Chat);
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

/// A bubble frame filling `tile`: its body above, its tail tapering to the middle of the
/// bottom row.
fn draw_frame(tile: &mut Canvas, style: TalkStyle) {
    let (w, h) = (tile.width() as i32, tile.height() as i32);
    let d = TALK_DETAIL as i32;
    // The body, and the three art rows of tail under it.
    let (bx0, by0, bx1, by1) = (d, d, w - d - 1, h - 3 * d - 1);
    let (cx, cy) = ((bx0 + bx1) / 2, (by0 + by1) / 2);
    let (rx, ry) = ((bx1 - bx0) / 2, (by1 - by0) / 2);
    let (paper, edge) = match style {
        TalkStyle::Chat => (CREAM, PLUM),
        TalkStyle::Excited => (SUN, SUN_EDGE),
        TalkStyle::Whisper => (MIST, MIST_EDGE),
        TalkStyle::Thought => (CLOUD, PLUM),
        TalkStyle::Song => (SKY, SKY_EDGE),
        TalkStyle::Tender => (ROSE, ROSE_EDGE),
        TalkStyle::Grumble => (SLATE, SLATE_EDGE),
        TalkStyle::Shout => (CREAM, PLUM),
    };
    let rounded = |tile: &mut Canvas, grow: i32, colour: Rgba| {
        let corner = 3 * d;
        tile.fill_rect(
            bx0 - grow + corner,
            by0 - grow,
            bx1 - bx0 + 1 + 2 * grow - 2 * corner,
            by1 - by0 + 1 + 2 * grow,
            colour,
        );
        tile.fill_rect(
            bx0 - grow,
            by0 - grow + corner,
            bx1 - bx0 + 1 + 2 * grow,
            by1 - by0 + 1 + 2 * grow - 2 * corner,
            colour,
        );
        for (x, y) in [
            (bx0 + corner, by0 + corner),
            (bx1 - corner, by0 + corner),
            (bx0 + corner, by1 - corner),
            (bx1 - corner, by1 - corner),
        ] {
            tile.fill_circle(x, y, corner + grow, colour);
        }
    };
    let tail = |tile: &mut Canvas, colour: Rgba, grow: i32| {
        for row in 0..3 * d + grow {
            let half = (3 * d - row) / 2 + grow;
            tile.fill_rect(w / 2 - half, by1 + row, half * 2 + 1, 1, colour);
        }
    };
    match style {
        // A cloud of overlapping puffs, and two little puffs trailing down for a tail.
        TalkStyle::Thought => {
            let puffs = |tile: &mut Canvas, grow: i32, colour: Rgba| {
                let step = (rx * 2) / 4;
                for i in 0..=4 {
                    let x = bx0 + i * step;
                    tile.fill_circle(
                        x.clamp(bx0 + ry / 2, bx1 - ry / 2),
                        by0 + ry / 2,
                        ry / 2 + 2 + grow,
                        colour,
                    );
                    tile.fill_circle(
                        x.clamp(bx0 + ry / 2, bx1 - ry / 2),
                        by1 - ry / 2,
                        ry / 2 + 2 + grow,
                        colour,
                    );
                }
                tile.fill_ellipse(cx, cy, rx - 1 + grow, ry - 1 + grow, colour);
            };
            puffs(tile, d, edge);
            puffs(tile, 0, paper);
            for (dx, dy, r) in [(1, 2 * d, 2), (3, 4 * d, 1)] {
                tile.fill_circle(w / 2 - dx * d / 2, by1 + dy - d, r * d / 2 + d, edge);
                tile.fill_circle(w / 2 - dx * d / 2, by1 + dy - d, r * d / 2, paper);
            }
        }
        // Sunny yellow with a bright rim inside its edge: lit up.
        TalkStyle::Excited => {
            rounded(tile, d, edge);
            tail(tile, edge, d);
            rounded(tile, 0, GOLD_LIGHT);
            tail(tile, GOLD_LIGHT, 0);
            rounded(tile, -1, paper);
            tail(tile, paper, -1);
        }
        // Dotted round instead of drawn round, and pale.
        TalkStyle::Whisper => {
            rounded(tile, 0, paper);
            tail(tile, paper, 0);
            for x in (bx0..=bx1).step_by((2 * d) as usize) {
                tile.fill_rect(x, by0 - d, d, d, edge);
                tile.fill_rect(x, by1 + 1, d, d, edge);
            }
            for y in (by0..=by1).step_by((2 * d) as usize) {
                tile.fill_rect(bx0 - d, y, d, d, edge);
                tile.fill_rect(bx1 + 1, y, d, d, edge);
            }
        }
        // A wavy edge, top and bottom.
        TalkStyle::Song => {
            rounded(tile, d, edge);
            tail(tile, edge, d);
            rounded(tile, 0, paper);
            tail(tile, paper, 0);
            for x in bx0 + 2 * d..bx1 - 2 * d {
                let wave = (((x as f32) / (2.0 * d as f32)).sin() * d as f32).round() as i32;
                tile.set(x, by0 + d + wave, SKY_EDGE);
            }
        }
        // Jagged little bites out of a dark cloud.
        TalkStyle::Grumble => {
            rounded(tile, d, edge);
            tail(tile, edge, d);
            rounded(tile, 0, paper);
            tail(tile, paper, 0);
            for x in (bx0 + 2 * d..bx1 - d).step_by((3 * d) as usize) {
                tile.fill_rect(x, by0 - d, d, 2 * d, Rgba::TRANSPARENT);
                tile.set(x, by0 + d, edge);
                tile.set(x + 1, by0 + d, edge);
            }
        }
        // A bold edge, and lines flying off beside the tail.
        TalkStyle::Shout => {
            rounded(tile, 2 * d, edge);
            tail(tile, edge, 2 * d);
            rounded(tile, 0, paper);
            tail(tile, paper, 0);
        }
        // Round and soft, with little hearts at its top corners.
        TalkStyle::Tender => {
            rounded(tile, d, edge);
            tail(tile, edge, d);
            rounded(tile, 0, paper);
            tail(tile, paper, 0);
            for x in [bx0 + d, bx1 - d] {
                tile.fill_circle(x - 1, by0, 1, edge);
                tile.fill_circle(x + 1, by0, 1, edge);
                tile.set(x, by0 + 2, edge);
            }
        }
        TalkStyle::Chat => {
            rounded(tile, d, edge);
            tail(tile, edge, d);
            rounded(tile, 0, paper);
            tail(tile, paper, 0);
        }
    }
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
        TalkTopic::CrackedHeart => {
            heart(tile, 10, 7, 3, GREEN);
            tile.line(10, 5, 8, 9, 1, INK);
            tile.line(8, 9, 11, 12, 1, INK);
        }
        TalkTopic::Wave => {
            blob(tile, 10, 11, 4, 4, SKIN);
            for (i, x) in [6, 9, 12].into_iter().enumerate() {
                tile.fill_rect(x - 1, 4 + i as i32 % 2, 3, 6, INK);
                tile.fill_rect(x, 5 + i as i32 % 2, 1, 5, SKIN);
            }
            tile.line(15, 4, 17, 2, 1, INK);
            tile.line(16, 7, 18, 6, 1, INK);
        }
        TalkTopic::Hand => {
            blob(tile, 10, 11, 5, 4, SKIN);
            tile.fill_rect(4, 9, 3, 5, INK);
            tile.fill_rect(5, 10, 1, 3, SKIN);
        }
        TalkTopic::Food => {
            blob(tile, 10, 11, 5, 5, RED);
            tile.set(7, 9, HEART_LIGHT);
            tile.set(8, 8, HEART_LIGHT);
            tile.line(10, 6, 11, 3, 2, WOOD);
            blob(tile, 13, 4, 2, 1, LEAFY);
        }
        TalkTopic::Ball => {
            blob(tile, 10, 10, 6, 6, BALL);
            tile.line(4, 10, 16, 10, 1, INK);
            tile.line(10, 4, 10, 16, 1, INK);
            tile.fill_circle(7, 7, 1, BALL_LIGHT);
        }
        TalkTopic::Stick => {
            stroke(tile, (4, 16), (16, 4), WOOD);
            tile.line(12, 3, 17, 8, 3, INK);
            tile.line(12, 3, 17, 8, 1, GOLD);
        }
        TalkTopic::Umbrella => {
            tile.fill_ellipse(10, 8, 8, 5, INK);
            tile.fill_ellipse(10, 8, 7, 4, RED);
            tile.fill_rect(2, 9, 17, 6, Rgba::TRANSPARENT);
            tile.fill_rect(2, 8, 17, 1, INK);
            tile.line(10, 8, 10, 16, 1, INK);
            tile.line(10, 16, 12, 16, 1, INK);
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
        TalkTopic::Fish => {
            blob(tile, 9, 10, 6, 3, BALL);
            tile.fill_rect(15, 7, 2, 7, INK);
            tile.fill_rect(16, 6, 2, 9, INK);
            tile.set(6, 9, INK);
            tile.line(7, 12, 12, 12, 1, BALL_LIGHT);
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
        TalkTopic::Moon => {
            blob(tile, 9, 10, 6, 6, NIGHT);
            tile.fill_circle(12, 8, 5, Rgba::TRANSPARENT);
            tile.set(16, 4, GOLD);
            tile.set(15, 4, GOLD);
            tile.set(16, 3, GOLD);
        }
        // Z, as a Z: a bar across, a stroke down from its right end to the left, a bar across.
        TalkTopic::Sleep => {
            for (x, y, s) in [(3, 7, 8), (13, 2, 5)] {
                tile.fill_rect(x - 1, y - 1, s + 2, 4, INK);
                tile.fill_rect(x - 1, y + s - 2, s + 2, 4, INK);
                tile.line(x + s - 1, y + 1, x, y + s - 1, 3, INK);
                tile.fill_rect(x, y, s, 2, BLUE);
                tile.fill_rect(x, y + s - 1, s, 2, BLUE);
                tile.line(x + s - 1, y + 1, x, y + s - 1, 1, BLUE);
            }
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
        TalkTopic::Spiral => {
            for (i, r) in [6, 4, 2].into_iter().enumerate() {
                tile.fill_circle(10, 10, r + 1, INK);
                tile.fill_circle(10, 10, r, if i % 2 == 0 { BALL_LIGHT } else { BALL });
            }
            tile.fill_rect(10, 3, 7, 7, Rgba::TRANSPARENT);
        }
        TalkTopic::Blush => {
            for x in [5, 15] {
                tile.fill_ellipse(x, 11, 3, 2, HEART_LIGHT);
            }
            tile.line(4, 7, 7, 7, 1, INK);
            tile.line(13, 7, 16, 7, 1, INK);
            tile.line(8, 14, 12, 14, 1, INK);
        }
        TalkTopic::Eye => {
            tile.fill_ellipse(10, 10, 8, 5, INK);
            tile.fill_ellipse(10, 10, 7, 4, Rgba::new(255, 255, 255, 255));
            tile.fill_circle(10, 10, 3, BLUE);
            tile.fill_circle(10, 10, 1, INK);
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
        TalkTopic::Footprints => {
            for (x, y) in [(6, 13), (13, 6)] {
                blob(tile, x, y, 2, 3, WOOD);
                for dx in [-2, 0, 2] {
                    tile.fill_circle(x + dx, y - 4, 1, INK);
                }
            }
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
        TalkTopic::Sun => {
            for i in 0..8 {
                let t = i as f32 / 8.0 * std::f32::consts::TAU;
                tile.line(
                    10,
                    10,
                    10 + (8.0 * t.cos()) as i32,
                    10 + (8.0 * t.sin()) as i32,
                    1,
                    GOLD,
                );
            }
            blob(tile, 10, 10, 4, 4, GOLD);
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
            for double in [false, true] {
                rects.push(TalkSheet::frame(style, double, BubbleGrowth::Full));
            }
        }
        rects.push(TalkSheet::frame(
            TalkStyle::Chat,
            false,
            BubbleGrowth::Small,
        ));
        rects.push(TalkSheet::frame(
            TalkStyle::Chat,
            false,
            BubbleGrowth::Medium,
        ));
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

    /// A Z for sleep reads as a Z: its stroke runs from the top bar's right end down to the
    /// bottom bar's left end, not the other way.
    #[test]
    fn the_sleep_picture_is_a_z() {
        let mut tile = Canvas::new(TALK_ICON * TALK_DETAIL, TALK_ICON * TALK_DETAIL);
        draw_icon(&mut tile, TalkTopic::Sleep);
        // Between the big Z's bars, high on the right and low on the left is drawn; high on the
        // left and low on the right is not.
        assert!(tile.get(9, 9).a > 0 && tile.get(5, 12).a > 0);
        assert_eq!(tile.get(4, 10), Rgba::TRANSPARENT);
        assert_eq!(tile.get(9, 12), Rgba::TRANSPARENT);
    }
}
