//! Wonders: a chair, a leaf sled, a fountain, a tightrope and the rest, drawn at the size companions
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
//! for the parts that are made rather than grown: a cushion, a hammock, a sign.
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
const WATER_DEEP: Rgba = Rgba::new(62, 116, 180, 255);
const WATER_LIGHT: Rgba = Rgba::new(190, 228, 250, 255);
const ROPE: Rgba = Rgba::new(230, 200, 140, 255);
const CREAM: Rgba = Rgba::new(244, 234, 212, 255);
const RED: Rgba = Rgba::new(212, 74, 64, 255);
const LEAFY: Rgba = Rgba::new(98, 160, 92, 255);
const LEAF_DARK: Rgba = Rgba::new(62, 116, 70, 255);
const LEAF_LIGHT: Rgba = Rgba::new(160, 210, 120, 255);

/// How many frames a kind has.
pub const fn wonder_frames(kind: WonderKind) -> u32 {
    match kind {
        WonderKind::Chair | WonderKind::BookStack => 1,
        WonderKind::LeafSled => 5,
        WonderKind::Fountain => 12,
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
        // Still, or a scuff of dust behind it: behind on the left going out, on the right coming
        // back.
        WonderKind::LeafSled => {
            let puff = (elapsed * 8.0) as u32 % 2;
            if motion > 0.1 {
                1 + puff
            } else if motion < -0.1 {
                3 + puff
            } else {
                0
            }
        }
        // Six frames of a steady play, then six of a splash, at eight a second.
        WonderKind::Fountain => {
            let base = if motion > 0.3 { 6 } else { 0 };
            base + (elapsed * 8.0) as u32 % 6
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
        // A big leaf lying on the ground, seen a little from above: pointed at the stalk end
        // behind, its edges finely toothed, a pale midrib down the middle with veins branching
        // off it, and the front tip rolled up and back over itself like the front of a sled. The
        // moving frames kick up a scuff of dust behind it, on whichever side behind is.
        WonderKind::LeafSled => {
            let (back, front, mid_y) = (-19, 15, g - 4);
            // How far the leaf reaches either side of its midrib at `x`: pointed behind, fullest
            // a little forward of the middle, and narrowing into the curl.
            let half = |x: i32| -> i32 {
                let t = (x - back) as f32 / (front - back) as f32;
                (5.5 * (t * (1.0 - t) * 4.0).powf(0.8)).round() as i32
            };
            // The stalk first, so the leaf covers its root.
            bar(c, (m + back - 6, g - 2), (m + back + 1, mid_y), 1, BARK);
            for x in back..=front {
                let h = half(x);
                c.fill_rect(m + x, mid_y - h - 1, 1, 2 * h + 3, OUTLINE);
            }
            for x in back + 1..front {
                let h = half(x);
                if h == 0 {
                    continue;
                }
                // The far side of the leaf in the light, the near side curving down into shade.
                c.fill_rect(m + x, mid_y - h, 1, h, LEAFY);
                c.fill_rect(m + x, mid_y, 1, h + 1, LEAF_DARK);
                // Toothed edges: a notch every few pixels along both.
                if (x - back) % 4 == 0 {
                    c.set(m + x, mid_y - h, OUTLINE);
                    c.set(m + x, mid_y + h, OUTLINE);
                }
                // Veins running out and forward from the midrib.
                let from_vein = (x - back) % 6;
                if (1..=h).contains(&from_vein) {
                    c.set(m + x, mid_y - from_vein, LEAF_LIGHT);
                    if from_vein < h {
                        c.set(m + x, mid_y + from_vein, LEAFY);
                    }
                }
            }
            // The midrib, pale, from the stalk into the curl.
            c.line(m + back + 1, mid_y, m + front, mid_y, 1, LEAF_LIGHT);
            // The tip, rolled up and back over itself.
            let curl = [
                (front, -1),
                (front + 2, -2),
                (front + 3, -4),
                (front + 3, -6),
                (front + 2, -8),
                (front, -9),
                (front - 2, -8),
                (front - 2, -6),
            ];
            // Every outline down before any fill, so the roll reads as one piece of leaf.
            for (width, color) in [(4, OUTLINE), (2, LEAFY)] {
                for pair in curl.windows(2) {
                    let ((x0, y0), (x1, y1)) = (pair[0], pair[1]);
                    c.line(m + x0, mid_y + y0, m + x1, mid_y + y1, width, color);
                }
            }
            // Its underside in shade where it turns over, and the light along its top.
            c.set(m + front + 2, mid_y - 2, LEAF_DARK);
            c.set(m + front + 3, mid_y - 4, LEAF_DARK);
            c.set(m + front + 2, mid_y - 7, LEAF_LIGHT);
            c.set(m + front, mid_y - 8, LEAF_LIGHT);
            // A scuff of dust behind, two puffs that move off as it goes.
            if frame > 0 {
                let behind = if frame <= 2 { -1 } else { 1 };
                let drift = ((frame - 1) % 2) as i32;
                let from = if behind < 0 { back - 4 } else { front + 6 };
                for (dx, dy, r) in [(0, 0, 2), (4, -2, 1)] {
                    let x = m + from + behind * (dx + drift * 2);
                    c.fill_circle(x, g - 2 + dy - drift, r, STONE_LIGHT);
                }
            }
        }
        // A round stone basin seen a little from above, its front wall in courses of stone lit
        // from the left, the water lying inside its rim with rings spreading over it; a column
        // up the middle to a bowl, a jet out of the top, and two streams curving down from the
        // bowl into the basin. Six frames of a steady play, then six of a splash: a taller jet,
        // fuller streams, and water thrown out over the rim.
        WonderKind::Fountain => fountain(c, frame >= 6, (frame % 6) as i32),
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
        // A tree stump for a table, rings on top and roots spreading at its foot, with a low stump
        // for a stool on each side, far enough out that whoever sits there sits beside the table
        // rather than in front of it. The frames lay down cards: none, then one at a time up to
        // four.
        WonderKind::StumpTable => {
            for x in [m - 28, m + 28] {
                boxed(c, x - 6, g - 5, 12, 5, BARK);
                c.line(x - 3, g - 4, x - 3, g - 1, 1, PLANK_DARK);
                c.line(x + 2, g - 4, x + 2, g - 1, 1, PLANK_DARK);
                round(c, x, g - 6, 6, 2, RINGS);
                c.fill_ellipse(x, g - 6, 3, 1, PLANK_LIGHT);
                c.set(x, g - 6, PLANK_DARK);
            }
            boxed(c, m - 10, g - 11, 20, 11, BARK);
            c.fill_rect(m - 10, g - 11, 2, 11, PLANK_DARK);
            for x in [m - 5, m, m + 5] {
                c.line(x, g - 9, x, g - 1, 1, PLANK_DARK);
            }
            // Roots spreading at the foot.
            for (x, w) in [(m - 14, 4), (m + 10, 4)] {
                boxed(c, x, g - 2, w, 2, BARK);
            }
            round(c, m, g - 12, 12, 3, RINGS);
            c.fill_ellipse(m, g - 12, 7, 2, PLANK_LIGHT);
            c.fill_ellipse(m, g - 12, 3, 1, RINGS);
            c.set(m, g - 12, PLANK_DARK);
            let cards = [(-7, -13), (4, -12), (-2, -14), (7, -14)];
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

/// The fountain at one moment of its loop: `beat` of six, playing steadily or splashing.
fn fountain(c: &mut Canvas, splash: bool, beat: i32) {
    let (m, g) = (WONDER_MIDDLE, WONDER_GROUND);
    // The basin: a rim ellipse twelve rows up, and the front wall down from its near edge to the
    // ground, so its foot curves up at the sides the way the bottom of a round thing does.
    let (rim_y, rx, ry, wall) = (g - 12, 22, 6, 6);
    let near = |x: i32| {
        let along = (x as f32 / rx as f32).clamp(-1.0, 1.0);
        rim_y + (ry as f32 * (1.0 - along * along).sqrt()).round() as i32
    };
    // One outline round the whole silhouette first.
    c.fill_ellipse(m, rim_y, rx + 1, ry + 1, OUTLINE);
    for x in -rx..=rx {
        c.fill_rect(m + x, near(x), 1, wall + 1, OUTLINE);
    }
    // The wall, lit on the left and in shade on the right, laid in two courses of blocks.
    for x in -rx + 1..rx {
        let shade = if x < -rx / 3 {
            STONE_LIGHT
        } else if x > rx / 3 {
            STONE_DARK
        } else {
            STONE
        };
        let top = near(x);
        c.fill_rect(m + x, top, 1, wall, shade);
        // A course of mortar halfway down, and the joints between blocks, staggered.
        c.set(m + x, top + wall / 2, STONE_DARK);
        let course = if (x + rx) % 8 == 0 {
            Some(top)
        } else if (x + rx + 4) % 8 == 0 {
            Some(top + wall / 2 + 1)
        } else {
            None
        };
        if let Some(y) = course {
            c.fill_rect(m + x, y, 1, wall / 2, STONE_DARK);
        }
    }
    // The rim, lit along its top, and the inner wall at the back showing above the water.
    c.fill_ellipse(m, rim_y, rx, ry, STONE_LIGHT);
    c.fill_ellipse(m, rim_y, rx - 3, ry - 2, STONE_DARK);
    c.fill_ellipse(m, rim_y + 1, rx - 3, ry - 2, WATER);
    c.fill_ellipse(m - 2, rim_y, rx - 7, ry - 4, WATER_DEEP);
    c.fill_ellipse(m, rim_y + 2, rx - 6, ry - 4, WATER);
    // Rings spreading out over the water from where the streams come down, one after another.
    let water = |x: i32, y: i32| {
        let (dx, dy) = (
            (x - m) as f32 / (rx - 3) as f32,
            (y - rim_y - 1) as f32 / (ry - 2) as f32,
        );
        dx * dx + dy * dy <= 0.9
    };
    for side in [-1, 1] {
        let centre = m + side * 12;
        for ring in 0..2 {
            let r = (beat + ring * 3) % 6 + 1;
            for step in 0..24 {
                let angle = step as f32 / 24.0 * std::f32::consts::TAU;
                let x = centre + (angle.cos() * r as f32).round() as i32;
                let y = rim_y + 1 + (angle.sin() * r as f32 * 0.35).round() as i32;
                if water(x, y) {
                    c.set(x, y, WATER_LIGHT);
                }
            }
        }
    }
    // A glint on the water.
    c.fill_rect(m - 9, rim_y - 1, 3, 1, WATER_LIGHT);
    // The column, round and lit from the left, from the water up to the bowl.
    let (bowl_y, column_top) = (g - 28, g - 27);
    boxed(c, m - 2, column_top, 5, rim_y - column_top, STONE);
    c.fill_rect(m - 2, column_top, 1, rim_y - column_top, STONE_LIGHT);
    c.fill_rect(m + 2, column_top, 1, rim_y - column_top, STONE_DARK);
    // Where it meets the water, a ring of foam.
    c.fill_ellipse(m, rim_y + 1, 4, 1, WATER_LIGHT);
    // The bowl: its rim, its little wall, the water in it.
    for x in -9..=9 {
        let along = x as f32 / 9.0;
        let front = bowl_y + (3.0 * (1.0 - along * along).sqrt()).round() as i32;
        c.fill_rect(m + x, front, 1, 3, OUTLINE);
    }
    c.fill_ellipse(m, bowl_y, 10, 4, OUTLINE);
    for x in -8..=8 {
        let along = x as f32 / 9.0;
        let front = bowl_y + (3.0 * (1.0 - along * along).sqrt()).round() as i32;
        let shade = if x < -3 {
            STONE_LIGHT
        } else if x > 3 {
            STONE_DARK
        } else {
            STONE
        };
        c.fill_rect(m + x, front, 1, 2, shade);
    }
    c.fill_ellipse(m, bowl_y, 9, 3, STONE_LIGHT);
    c.fill_ellipse(m, bowl_y, 7, 1, WATER);
    // The jet, rising and falling a little with each beat, and a crown of drops at its top.
    let jet = if splash {
        12 + [0, 2, 3, 2, 0, -1][beat as usize]
    } else {
        7 + [0, 1, 1, 0, -1, 0][beat as usize]
    };
    let top = bowl_y - 1 - jet;
    c.line(m, bowl_y - 1, m, top, 2, WATER);
    c.line(m, bowl_y - 1, m, top, 1, WATER_LIGHT);
    for (dx, dy) in [(-2, 1), (2, 1), (-1, -1), (1, -1)] {
        if (dx + dy + beat).rem_euclid(2) == 0 {
            c.set(m + dx, top + dy, WATER_LIGHT);
        }
    }
    // Two streams from the bowl's lip curving down into the basin, with the water travelling
    // along them: every third pixel is a gap that moves on with each beat.
    let thick = if splash { 2 } else { 1 };
    for side in [-1, 1] {
        let (x0, y0) = (m + side * 9, bowl_y + 1);
        let (x1, y1) = (m + side * 12, rim_y + 1);
        let points = 18;
        for step in 0..=points {
            if (step + beat) % 3 == 0 {
                continue;
            }
            let t = step as f32 / points as f32;
            // Out first and then down, as falling water goes.
            let x = x0 as f32 + (x1 - x0) as f32 * (1.0 - (1.0 - t) * (1.0 - t));
            let y = y0 as f32 + (y1 - y0) as f32 * t * t;
            c.fill_rect(
                x.round() as i32,
                y.round() as i32,
                thick,
                1,
                if step % 2 == 0 { WATER_LIGHT } else { WATER },
            );
        }
        // A little splash where each stream comes down.
        let up = (beat % 2) + 1 + i32::from(splash);
        c.set(x1 - 1, y1 - up, WATER_LIGHT);
        c.set(x1 + 1, y1 - up, WATER_LIGHT);
    }
    if splash {
        // Water thrown up and out over the rim, each drop on its own arc.
        for (side, phase, reach) in [(-1, 0, 16), (-1, 3, 20), (1, 1, 17), (1, 4, 21)] {
            let t = ((beat + phase) % 6) as f32 / 5.0;
            let x = m + side * (12 + ((reach - 12) as f32 * t).round() as i32);
            let y = rim_y - 1 - (12.0 * t * (1.0 - t) * 2.0).round() as i32 + (t * 6.0) as i32;
            c.fill_rect(x, y, 2, 2, WATER_LIGHT);
            c.set(x + side, y + 1, WATER);
        }
    }
    // A tuft of grass at each side of the foot.
    for x in [m - rx - 2, m + rx + 1] {
        c.line(x, g, x - 1, g - 3, 1, LEAFY);
        c.line(x + 1, g, x + 2, g - 2, 1, LEAF_DARK);
    }
}
