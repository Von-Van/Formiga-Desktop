//! Wonders: a chair, a bike, a fountain, a tightrope and the rest, drawn at the size companions
//! play on them.
//!
//! Each kind is a strip of frames in cells of one size, the wonder standing on a shared ground row
//! in the middle of its cell, so the overlay places every frame of every kind the same way: the
//! middle of the cell over where the wonder stands, the ground row on the ground. Positions in the
//! simulation's wonder scripts are art pixels from that same middle and ground, so a seat drawn
//! ten rows up is where a player sitting on it is put.
//!
//! Only the kind out now is ever baked, when it appears, and its texture goes with it: a colony
//! with no wonder out holds nothing for them.
//!
//! Materials are the village's own — timber, stone, water, rope, paper — with the colony's colours
//! for the parts that are made rather than grown: a cushion, a bike's frame, a hammock, a sign.
//! Everything is ringed in one dark outline, as the houses and decorations are, so a wonder holds
//! its shape against a pale desktop and a dark one alike.

use crate::{Canvas, PALETTES, Rgba};
use formiga_core::WonderKind;

/// One frame of a wonder, in art pixels.
pub const WONDER_CELL_WIDTH: u32 = 112;
pub const WONDER_CELL_HEIGHT: u32 = 64;
/// The row a wonder stands on, and the column its middle is on.
pub const WONDER_GROUND: i32 = 60;
pub const WONDER_MIDDLE: i32 = 56;

const OUTLINE: Rgba = Rgba::new(52, 38, 34, 255);
const PLANK: Rgba = Rgba::new(168, 118, 76, 255);
const PLANK_LIGHT: Rgba = Rgba::new(206, 158, 106, 255);
const PLANK_DARK: Rgba = Rgba::new(122, 82, 54, 255);
const BARK: Rgba = Rgba::new(110, 78, 56, 255);
const RINGS: Rgba = Rgba::new(222, 186, 136, 255);
const STONE: Rgba = Rgba::new(176, 172, 164, 255);
const STONE_LIGHT: Rgba = Rgba::new(214, 210, 202, 255);
const STONE_DARK: Rgba = Rgba::new(128, 124, 120, 255);
const WATER: Rgba = Rgba::new(98, 166, 222, 255);
const WATER_LIGHT: Rgba = Rgba::new(190, 228, 250, 255);
const ROPE: Rgba = Rgba::new(230, 200, 140, 255);
const METAL: Rgba = Rgba::new(150, 156, 168, 255);
const CREAM: Rgba = Rgba::new(244, 234, 212, 255);
const RED: Rgba = Rgba::new(212, 74, 64, 255);
const LEAFY: Rgba = Rgba::new(98, 160, 92, 255);

/// How many frames a kind has.
pub const fn wonder_frames(kind: WonderKind) -> u32 {
    match kind {
        WonderKind::Chair | WonderKind::BookStack => 1,
        WonderKind::Bike => 2,
        WonderKind::Fountain => 8,
        WonderKind::Tightrope => 3,
        WonderKind::StumpTable => 5,
        WonderKind::ArrowSign | WonderKind::Hammock => 5,
        WonderKind::Seesaw => 7,
    }
}

/// Which frame shows, from how long it has been out and what its moving part is doing.
pub fn wonder_frame(kind: WonderKind, elapsed: f32, motion: f32) -> u32 {
    let last = wonder_frames(kind) - 1;
    let along = |steps: f32| ((motion.clamp(-1.0, 1.0) + 1.0) / 2.0 * steps).round() as u32;
    let frame = match kind {
        WonderKind::Chair | WonderKind::BookStack => 0,
        WonderKind::Bike => {
            if motion.abs() > 0.1 {
                (elapsed * 8.0) as u32 % 2
            } else {
                0
            }
        }
        // Four frames of a gentle spout, then four of a splash.
        WonderKind::Fountain => {
            let base = if motion > 0.3 { 4 } else { 0 };
            base + (elapsed * 6.0) as u32 % 4
        }
        WonderKind::Tightrope => along(2.0),
        WonderKind::StumpTable => (motion.clamp(0.0, 1.0) * 4.0).round() as u32,
        // Full right at 1, edge on at 0, full left at -1.
        WonderKind::ArrowSign => 4 - along(4.0),
        WonderKind::Hammock => along(4.0),
        WonderKind::Seesaw => along(6.0),
    };
    frame.min(last)
}

pub struct WonderRenderer;

impl WonderRenderer {
    /// Every frame of one kind in a row, left to right.
    pub fn render(kind: WonderKind, colony_seed: [u8; 32]) -> Canvas {
        let frames = wonder_frames(kind);
        let palette = PALETTES[colony_seed[16] as usize % PALETTES.len()];
        let colors = Colors {
            cloth: palette.coat,
            cloth_light: palette.highlight,
            cloth_dark: palette.shadow,
            trim: palette.accent,
        };
        let mut sheet = Canvas::new(WONDER_CELL_WIDTH * frames, WONDER_CELL_HEIGHT);
        for frame in 0..frames {
            let mut cell = Canvas::new(WONDER_CELL_WIDTH, WONDER_CELL_HEIGHT);
            draw(&mut cell, kind, frame, colors);
            let offset = frame * WONDER_CELL_WIDTH;
            for y in 0..WONDER_CELL_HEIGHT {
                for x in 0..WONDER_CELL_WIDTH {
                    let pixel = cell.get(x as i32, y as i32);
                    if pixel.a > 0 {
                        sheet.set((offset + x) as i32, y as i32, pixel);
                    }
                }
            }
        }
        sheet
    }
}

#[derive(Clone, Copy)]
struct Colors {
    cloth: Rgba,
    cloth_light: Rgba,
    cloth_dark: Rgba,
    trim: Rgba,
}

/// A rectangle ringed in the outline.
fn boxed(c: &mut Canvas, x: i32, y: i32, w: i32, h: i32, fill: Rgba) {
    c.fill_rect(x - 1, y - 1, w + 2, h + 2, OUTLINE);
    c.fill_rect(x, y, w, h, fill);
}

/// An ellipse ringed in the outline.
fn round(c: &mut Canvas, x: i32, y: i32, rx: i32, ry: i32, fill: Rgba) {
    c.fill_ellipse(x, y, rx + 1, ry + 1, OUTLINE);
    c.fill_ellipse(x, y, rx, ry, fill);
}

/// A thick line ringed in the outline.
fn bar(c: &mut Canvas, from: (i32, i32), to: (i32, i32), width: i32, fill: Rgba) {
    c.line(from.0, from.1, to.0, to.1, width + 2, OUTLINE);
    c.line(from.0, from.1, to.0, to.1, width, fill);
}

fn draw(c: &mut Canvas, kind: WonderKind, frame: u32, k: Colors) {
    let (m, g) = (WONDER_MIDDLE, WONDER_GROUND);
    match kind {
        // A little wooden chair seen from the front: four legs, a cushioned seat ten rows up, and
        // a slatted back rising behind whoever sits in it.
        WonderKind::Chair => {
            // Back posts and slats, behind the seat.
            for x in [m - 9, m + 8] {
                boxed(c, x, g - 30, 2, 20, PLANK);
            }
            for y in [g - 29, g - 23] {
                boxed(c, m - 8, y, 16, 2, PLANK_LIGHT);
            }
            // Legs.
            for x in [m - 10, m + 8] {
                boxed(c, x, g - 9, 2, 9, PLANK_DARK);
            }
            for x in [m - 7, m + 5] {
                boxed(c, x, g - 8, 2, 8, PLANK);
            }
            // The seat and its cushion.
            boxed(c, m - 11, g - 11, 22, 3, PLANK);
            c.fill_rect(m - 11, g - 11, 22, 1, PLANK_LIGHT);
            round(c, m, g - 12, 9, 2, k.cloth);
            c.fill_rect(m - 7, g - 13, 10, 1, k.cloth_light);
        }
        // A small bike side on, front to the right: two spoked wheels, a frame in the colony's
        // colour, a saddle fourteen rows up and handlebars ahead of it.
        WonderKind::Bike => {
            let (back, front, axle) = (m - 11, m + 11, g - 6);
            for x in [back, front] {
                round(c, x, axle, 6, 6, Rgba::new(70, 70, 76, 255));
                c.fill_ellipse(x, axle, 4, 4, CREAM);
                let spokes: [(i32, i32); 2] = if frame == 0 {
                    [(4, 0), (0, 4)]
                } else {
                    [(3, 3), (3, -3)]
                };
                for (dx, dy) in spokes {
                    c.line(x - dx, axle - dy, x + dx, axle + dy, 1, METAL);
                }
                c.fill_circle(x, axle, 1, OUTLINE);
            }
            bar(c, (back, axle), (m, g - 13), 2, k.cloth);
            bar(c, (m, g - 13), (front - 2, g - 15), 2, k.cloth);
            bar(c, (back, axle), (m + 2, axle), 1, k.cloth_dark);
            bar(c, (m + 2, axle), (front - 2, g - 15), 2, k.cloth);
            bar(c, (front - 2, g - 15), (front, axle), 1, METAL);
            // Handlebars, and a bell.
            bar(c, (front - 2, g - 15), (front - 1, g - 20), 1, METAL);
            bar(c, (front - 3, g - 20), (front + 2, g - 20), 1, METAL);
            c.fill_circle(front + 2, g - 22, 1, k.trim);
            // The saddle.
            round(c, m - 1, g - 15, 4, 1, PLANK_DARK);
        }
        // A round stone basin with a pedestal and a spout of water: four frames of a gentle
        // trickle, then four of a splash, with water thrown out over the rim.
        WonderKind::Fountain => {
            let splash = frame >= 4;
            let beat = (frame % 4) as i32;
            // The basin, and the water inside its rim.
            round(c, m, g - 5, 19, 5, STONE);
            c.fill_ellipse(m, g - 7, 16, 3, WATER);
            c.fill_ellipse(m - 4, g - 8, 6, 1, WATER_LIGHT);
            c.fill_rect(m - 19, g - 5, 38, 1, STONE_LIGHT);
            c.fill_rect(m - 18, g - 2, 36, 1, STONE_DARK);
            // The pedestal and the bowl on top.
            boxed(c, m - 2, g - 22, 4, 15, STONE);
            c.fill_rect(m - 2, g - 22, 1, 15, STONE_LIGHT);
            round(c, m, g - 23, 7, 2, STONE);
            c.fill_ellipse(m, g - 24, 5, 1, WATER);
            // The spout, rising and falling to either side.
            let height = if splash {
                10 + beat % 2 * 2
            } else {
                6 + beat % 2
            };
            c.line(m, g - 25, m, g - 25 - height, 2, WATER_LIGHT);
            for side in [-1, 1] {
                let drop = (beat + if side < 0 { 0 } else { 2 }) % 4;
                c.fill_circle(m + side * (3 + drop), g - 26 - height + drop * 3, 1, WATER);
                c.set(m + side * (5 + drop), g - 22 + drop, WATER_LIGHT);
            }
            if splash {
                // Water thrown up out of the basin on both sides.
                for (dx, dy) in [(-15_i32, 0), (-11, 3), (12, 1), (16, 4)] {
                    let lift = (beat + dx.abs()) % 4;
                    c.fill_circle(m + dx, g - 12 - dy - lift * 2, 1, WATER_LIGHT);
                    c.set(m + dx + 1, g - 14 - dy - lift * 2, WATER);
                }
            }
        }
        // A rope strung between two posts, each with a little platform on top to stand on. The
        // rope sags less or more as it wobbles.
        WonderKind::Tightrope => {
            let sag = [1, 3, 5][frame as usize];
            for x in [m - 44, m + 44] {
                boxed(c, x - 1, g - 24, 3, 24, PLANK);
                c.fill_rect(x - 1, g - 24, 1, 24, PLANK_LIGHT);
                boxed(c, x - 4, g - 26, 9, 2, PLANK_DARK);
                // Guy ropes out to pegs.
                let out = if x < m { -6 } else { 6 };
                c.line(x, g - 22, x + out, g - 1, 1, ROPE);
                boxed(c, x + out - 1, g - 2, 2, 2, PLANK_DARK);
            }
            for i in 0..=84 {
                let t = i as f32 / 84.0;
                let dip = (4.0 * t * (1.0 - t) * sag as f32).round() as i32;
                let x = m - 42 + i;
                c.set(x, g - 25 + dip, OUTLINE);
                c.set(x, g - 24 + dip, ROPE);
            }
        }
        // A tree stump for a table, rings on top, with a small stump for a stool on each side.
        // The frames lay down cards: none, then one at a time up to four.
        WonderKind::StumpTable => {
            for x in [m - 20, m + 20] {
                round(c, x, g - 3, 6, 3, BARK);
                round(c, x, g - 6, 6, 2, RINGS);
                c.set(x, g - 6, PLANK_DARK);
            }
            boxed(c, m - 9, g - 11, 18, 11, BARK);
            for x in [m - 6, m - 1, m + 4] {
                c.line(x, g - 9, x, g - 1, 1, PLANK_DARK);
            }
            // Roots spreading at the foot.
            for (x, w) in [(m - 12, 3), (m + 9, 3)] {
                boxed(c, x, g - 2, w, 2, BARK);
            }
            round(c, m, g - 12, 10, 3, RINGS);
            c.fill_ellipse(m, g - 12, 5, 1, PLANK_LIGHT);
            c.set(m, g - 12, PLANK_DARK);
            let cards = [(-6, -13), (4, -12), (-2, -14), (6, -14)];
            for (dx, dy) in cards.into_iter().take(frame as usize) {
                boxed(c, m + dx, g + dy, 3, 2, CREAM);
                c.set(m + dx + 1, g + dy, RED);
            }
        }
        // A tall signpost with a big arrow board. The frames turn the board from pointing right,
        // through edge on, to pointing left.
        WonderKind::ArrowSign => {
            boxed(c, m - 1, g - 44, 3, 44, PLANK);
            c.fill_rect(m - 1, g - 44, 1, 44, PLANK_LIGHT);
            // A tuft of grass at the foot.
            for dx in [-3, -1, 2, 4] {
                c.line(m + dx, g, m + dx + dx.signum(), g - 3, 1, LEAFY);
            }
            let width = [16, 11, 2, 11, 16][frame as usize];
            let way = if frame < 2 {
                1
            } else if frame > 2 {
                -1
            } else {
                0
            };
            let top = g - 42;
            if way == 0 {
                boxed(c, m - 1, top, 3, 10, k.cloth_dark);
            } else {
                // The board, and the point of the arrow beyond it.
                let back = m - way * 6;
                let tip = m + way * width;
                let (left, right) = (back.min(tip - way * 5), back.max(tip - way * 5));
                boxed(c, left, top + 2, right - left, 6, k.cloth);
                c.fill_rect(left, top + 2, right - left, 1, k.cloth_light);
                for row in 0..=5 {
                    let x = tip - way * 5 + way * row;
                    for y in (top + row)..=(top + 10 - row) {
                        c.set(x, y, k.cloth);
                    }
                    c.set(x, top + row, OUTLINE);
                    c.set(x, top + 10 - row, OUTLINE);
                }
                c.set(tip + way, top + 5, OUTLINE);
                // A painted stripe along it.
                c.fill_rect(left + 2, top + 5, (right - left - 4).max(1), 1, k.trim);
            }
            round(c, m, g - 45, 2, 1, PLANK_DARK);
        }
        // A hammock slung between two posts. The frames swing its middle from one side to the
        // other.
        WonderKind::Hammock => {
            let sway = (frame as i32 - 2) * 3 / 2;
            for x in [m - 38, m + 38] {
                boxed(c, x - 1, g - 28, 3, 28, PLANK);
                c.fill_rect(x - 1, g - 28, 1, 28, PLANK_LIGHT);
                round(c, x, g - 29, 2, 1, PLANK_DARK);
            }
            // Ropes from the posts down to the cloth.
            c.line(m - 37, g - 24, m - 26 + sway, g - 15, 1, ROPE);
            c.line(m + 37, g - 24, m + 26 + sway, g - 15, 1, ROPE);
            // The cloth: a deep sag, thickest in the middle, edged in the colony's trim.
            for i in -26..=26 {
                let t = (i + 26) as f32 / 52.0;
                let dip = (4.0 * t * (1.0 - t) * 6.0).round() as i32;
                let x = m + i + sway;
                let y = g - 16 + dip;
                c.set(x, y - 1, OUTLINE);
                c.set(x, y, k.trim);
                c.set(x, y + 1, if i % 4 == 0 { k.cloth_light } else { k.cloth });
                c.set(x, y + 2, k.cloth);
                c.set(x, y + 3, OUTLINE);
            }
        }
        // A stack of four books, one sticking out on the left for a step, open on top.
        WonderKind::BookStack => {
            let books = [
                (m - 11, g - 6, 22, 5, k.cloth),
                (m - 10, g - 11, 20, 4, RED),
                (m - 14, g - 15, 18, 3, k.trim),
                (m - 8, g - 19, 18, 3, k.cloth_dark),
            ];
            for (x, y, w, h, color) in books {
                boxed(c, x, y, w, h, color);
                // Pages along the spine side.
                c.fill_rect(x + w - 2, y + 1, 2, (h - 2).max(1), CREAM);
                c.fill_rect(x, y, w, 1, CREAM);
            }
            // An open book on top.
            boxed(c, m - 7, g - 22, 6, 2, CREAM);
            boxed(c, m + 1, g - 22, 6, 2, CREAM);
            c.set(m, g - 21, OUTLINE);
            c.fill_rect(m - 6, g - 21, 4, 1, STONE);
            c.fill_rect(m + 2, g - 21, 4, 1, STONE);
        }
        // A plank on a log. The frames tip it from the left end up to the right end up.
        WonderKind::Seesaw => {
            let tilt = (frame as f32 - 3.0) / 3.0;
            // The log it pivots on.
            round(c, m, g - 4, 5, 4, BARK);
            c.fill_ellipse(m, g - 4, 2, 2, RINGS);
            let rise = |x: f32| (tilt * 7.0 * x / 34.0).round() as i32;
            let (left, right) = ((m - 42, g - 9 + rise(-42.0)), (m + 42, g - 9 + rise(42.0)));
            bar(c, left, right, 3, PLANK);
            c.line(left.0, left.1 - 1, right.0, right.1 - 1, 1, PLANK_LIGHT);
            // A handle at each seat.
            for (x, sign) in [(-36_i32, -1.0_f32), (36, 1.0)] {
                let y = g - 9 + rise(sign * 36.0);
                bar(c, (m + x, y - 1), (m + x, y - 5), 1, k.cloth);
            }
        }
    }
}
