//! What a house can be decorated with, one drawing per kind, each hung from the anchor its slot
//! resolves on the house's own silhouette: the peak, the eaves, either side of the wall, and the
//! ground either side of the door.
//!
//! Since 0.65.0 every decoration is drawn about twice the size it was, as one solid shape on a
//! sheet of its own, and then ringed with a one-pixel outline before it is set onto the house.
//! The outline is what makes a wreath read as a wreath at desktop scale: it holds the shape
//! against a cream wall, a coloured roof and the desktop behind alike. Strings, wires and poles are
//! drawn straight onto the house afterwards, thin, so they stay lines rather than turning into
//! bars. Everything is the same size on a cottage as on the colony house, keeps clear of the
//! doorway, and stays inside the ground its house's lot claims.

use super::{ShelterFrame, houses::mix};
use crate::{Canvas, Rgba};
use formiga_core::{ShelterDecorationKind, ShelterGenome};

/// The house's own colours a decoration is drawn in.
#[derive(Clone, Copy)]
pub(super) struct Inks {
    pub(super) outline: Rgba,
    pub(super) coat: Rgba,
    pub(super) accent: Rgba,
    pub(super) highlight: Rgba,
}

/// Materials that are what they are whatever the colony's colours: wood, leaves, stone, metal,
/// and the lamplight a lit decoration glows with.
const WOOD: Rgba = Rgba::new(140, 98, 64, 255);
const WOOD_LIGHT: Rgba = Rgba::new(186, 138, 92, 255);
const WOOD_DARK: Rgba = Rgba::new(104, 70, 46, 255);
const LEAF: Rgba = Rgba::new(92, 158, 80, 255);
const LEAF_LIGHT: Rgba = Rgba::new(140, 196, 110, 255);
const LEAF_DARK: Rgba = Rgba::new(58, 112, 62, 255);
const STONE: Rgba = Rgba::new(168, 164, 156, 255);
const STONE_LIGHT: Rgba = Rgba::new(204, 200, 190, 255);
const METAL: Rgba = Rgba::new(150, 156, 168, 255);
const METAL_LIGHT: Rgba = Rgba::new(208, 212, 220, 255);
const PUMPKIN: Rgba = Rgba::new(232, 132, 52, 255);
const CLAY: Rgba = Rgba::new(196, 112, 76, 255);
const CREAM: Rgba = Rgba::new(240, 228, 204, 255);
const GLOW: Rgba = Rgba::new(255, 206, 118, 255);
const GLOW_CORE: Rgba = Rgba::new(255, 236, 178, 255);

/// One decoration drawn on a sheet of its own, in coordinates measured from its anchor, before it
/// is outlined and set onto the house.
struct Piece {
    sheet: Canvas,
}

/// The sheet's size, and where on it a piece's anchor falls: room for the tallest roof piece above
/// the anchor and the longest chime below it.
const SHEET: u32 = 56;
const ORIGIN: (i32, i32) = (28, 38);

impl Piece {
    fn new() -> Self {
        Self {
            sheet: Canvas::new(SHEET, SHEET),
        }
    }

    fn set(&mut self, x: i32, y: i32, color: Rgba) {
        self.sheet.set(ORIGIN.0 + x, ORIGIN.1 + y, color);
    }

    /// A filled rectangle from `(x0, y0)` to `(x1, y1)`, both corners included.
    fn rect(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, color: Rgba) {
        self.sheet.fill_rect(
            ORIGIN.0 + x0.min(x1),
            ORIGIN.1 + y0.min(y1),
            (x1 - x0).abs() + 1,
            (y1 - y0).abs() + 1,
            color,
        );
    }

    fn ellipse(&mut self, x: i32, y: i32, rx: i32, ry: i32, color: Rgba) {
        self.sheet
            .fill_ellipse(ORIGIN.0 + x, ORIGIN.1 + y, rx, ry, color);
    }

    fn circle(&mut self, x: i32, y: i32, r: i32, color: Rgba) {
        self.sheet.fill_circle(ORIGIN.0 + x, ORIGIN.1 + y, r, color);
    }

    fn line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, color: Rgba) {
        self.sheet.line(
            ORIGIN.0 + x0,
            ORIGIN.1 + y0,
            ORIGIN.0 + x1,
            ORIGIN.1 + y1,
            1,
            color,
        );
    }

    /// A triangle with a flat side along row `base_y` from `x0` to `x1`, coming to a point at
    /// `(tip_x, tip_y)`, which may be above or below the base.
    fn triangle(&mut self, x0: i32, x1: i32, base_y: i32, tip_x: i32, tip_y: i32, color: Rgba) {
        let rows = (tip_y - base_y).abs().max(1);
        let step = if tip_y < base_y { -1 } else { 1 };
        for row in 0..=rows {
            let t = row as f32 / rows as f32;
            let left = (x0 as f32 + (tip_x - x0) as f32 * t).round() as i32;
            let right = (x1 as f32 + (tip_x - x1) as f32 * t).round() as i32;
            self.rect(left, base_y + row * step, right, base_y + row * step, color);
        }
    }

    /// Clear a pixel again, for a hole the outline should ring.
    fn clear(&mut self, x: i32, y: i32) {
        self.set(x, y, Rgba::TRANSPARENT);
    }

    /// Ring every drawn pixel that meets empty air with `outline`, one pixel wide, and set the
    /// whole piece onto `canvas` with its anchor at `(x, y)`.
    fn place(self, canvas: &mut Canvas, x: i32, y: i32, outline: Rgba) {
        let size = SHEET as i32;
        let filled = |px: i32, py: i32| self.sheet.get(px, py).a > 0;
        let mut ring = Vec::new();
        for py in 0..size {
            for px in 0..size {
                if !filled(px, py)
                    && [(1, 0), (-1, 0), (0, 1), (0, -1)]
                        .iter()
                        .any(|(dx, dy)| filled(px + dx, py + dy))
                {
                    ring.push((px, py));
                }
            }
        }
        let (left, top) = (x - ORIGIN.0, y - ORIGIN.1);
        for (px, py) in ring {
            canvas.set(left + px, top + py, outline);
        }
        for py in 0..size {
            for px in 0..size {
                let pixel = self.sheet.get(px, py);
                if pixel.a > 0 {
                    canvas.set(left + px, top + py, pixel);
                }
            }
        }
    }
}

/// Lamplight spilling a little way round a lit decoration: warmer where it falls on the house,
/// and only faintly on the air beside it.
fn glow(canvas: &mut Canvas, x: i32, y: i32, radius: i32) {
    for dy in -radius..=radius {
        for dx in -radius..=radius {
            let distance = ((dx * dx + dy * dy) as f32).sqrt();
            if distance > radius as f32 + 0.25 {
                continue;
            }
            let strength = 0.45 * (1.0 - distance / (radius as f32 + 1.0));
            let under = canvas.get(x + dx, y + dy);
            if under.a == 255 {
                canvas.set(x + dx, y + dy, mix(under, GLOW, strength));
            } else if under.a == 0 {
                let alpha = (strength * 150.0) as u8;
                if alpha > 8 {
                    canvas.set(x + dx, y + dy, Rgba { a: alpha, ..GLOW });
                }
            }
        }
    }
}

/// A thin line drawn straight onto the house, row by row along a gentle sag: a string, a wire or
/// a cord. Returns the row the line passes through at each column, so whatever hangs from it can
/// hang from the right pixel.
fn sagging_line(
    canvas: &mut Canvas,
    left: i32,
    right: i32,
    y: i32,
    sag: f32,
    color: Rgba,
) -> impl Fn(i32) -> i32 + use<> {
    let width = (right - left).max(1) as f32;
    let at = move |x: i32| {
        let t = (x - left) as f32 / width;
        y + (4.0 * t * (1.0 - t) * sag).round() as i32
    };
    for x in left..=right {
        canvas.set(x, at(x), color);
    }
    at
}

pub(super) fn draw(
    canvas: &mut Canvas,
    kind: ShelterDecorationKind,
    ink: Inks,
    genome: &ShelterGenome,
    frame: ShelterFrame,
    lit: bool,
) {
    let Inks {
        outline,
        coat,
        accent,
        highlight,
    } = ink;
    // Mounted pieces take their position from the shelter itself. Only the ground pieces shift,
    // and by a single pixel, so a home still varies without looking scattered.
    let drift = ((genome.detail_seed >> ((kind.index() * 7) % 64)) & 0x1) as i32;
    // The ground pieces stand against the corners of the house, half in front of the wall, so
    // they read as the house's own rather than as something left out on the grass; a cottage's
    // lot is narrower, so its pieces stand a pixel further in.
    let inset = if frame.cottage { 4 } else { 3 };
    let left_x = frame.cx - frame.ground_half + inset - drift;
    let right_x = frame.cx + frame.ground_half - inset + drift;
    // Wall pieces sit between the edge of the wall and the doorway, never over the door.
    const WALL_PIECE_HALF: i32 = 6;
    let wall_left = ((frame.cx - frame.wall_half) + (frame.cx - frame.door_half)) / 2;
    let wall_left = wall_left.min(frame.cx - frame.door_half - 1 - WALL_PIECE_HALF);
    let wall_right = ((frame.cx + frame.wall_half) + (frame.cx + frame.door_half)) / 2;
    let wall_right = wall_right.max(frame.cx + frame.door_half + 1 + WALL_PIECE_HALF);
    let mut piece = Piece::new();
    match kind {
        // -----------------------------------------------------------------------------------
        // The roof. Each stands on a short pole up from the peak, thin, and the piece on top.
        // -----------------------------------------------------------------------------------
        ShelterDecorationKind::RoofOrnament => {
            // A plump four-pointed star with a bright heart.
            let (x, top) = (frame.cx, frame.peak_y - 5);
            canvas.line(x, frame.peak_y + 1, x, top, 1, outline);
            for (dy, half) in [
                (-5, 0),
                (-4, 0),
                (-3, 1),
                (-2, 1),
                (-1, 3),
                (0, 5),
                (1, 3),
                (2, 1),
                (3, 1),
                (4, 0),
                (5, 0),
            ] {
                piece.rect(-half, dy - 6, half, dy - 6, accent);
            }
            piece.rect(-1, -7, 1, -5, highlight);
            piece.set(0, -6, CREAM);
            piece.place(canvas, x, top, outline);
        }
        ShelterDecorationKind::WeatherVane => {
            // A pole, a ball, and an arrow across the top with a fletched tail.
            let (x, top) = (frame.cx, frame.peak_y - 14);
            canvas.line(x, frame.peak_y + 1, x, top, 1, outline);
            piece.circle(0, 8, 2, METAL);
            piece.set(-1, 7, METAL_LIGHT);
            // The arrow's head, pointing into the wind.
            piece.rect(4, -3, 4, 3, accent);
            piece.rect(5, -2, 6, 2, accent);
            piece.rect(7, -1, 8, 1, accent);
            piece.set(9, 0, accent);
            piece.set(5, -1, highlight);
            // Its shaft, and the tail feathers.
            piece.rect(-6, 0, 4, 0, METAL);
            piece.rect(-8, -3, -6, -2, highlight);
            piece.rect(-8, 2, -6, 3, highlight);
            piece.rect(-7, -1, -5, 1, highlight);
            piece.place(canvas, x, top, outline);
        }
        ShelterDecorationKind::Pennant => {
            // A tall pole, a bead on top, and a long pennant flying off it in two stripes.
            let (x, top) = (frame.cx, frame.peak_y - 18);
            canvas.line(x, frame.peak_y + 1, x, top, 1, outline);
            for row in 0..8 {
                let distance = (row as f32 - 3.5).abs();
                let reach = (11.0 * (1.0 - distance / 4.2)).round() as i32;
                let color = if row < 4 { accent } else { highlight };
                piece.rect(1, row, reach.max(1), row, color);
            }
            piece.circle(0, -1, 1, highlight);
            piece.place(canvas, x, top, outline);
        }
        ShelterDecorationKind::PerchedBird => {
            // A round little bird sitting on the very top, looking out to one side.
            let (x, y) = (frame.cx, frame.peak_y);
            piece.ellipse(0, -5, 4, 3, coat);
            piece.circle(4, -9, 3, coat);
            // Its tail, sticking out behind.
            piece.triangle(-4, -4, -6, -8, -9, coat);
            piece.rect(-6, -6, -4, -5, coat);
            piece.ellipse(-1, -5, 2, 1, accent);
            piece.rect(0, -3, 3, -3, highlight);
            piece.set(5, -10, outline);
            piece.set(4, -11, highlight);
            // The beak.
            piece.rect(7, -9, 8, -9, PUMPKIN);
            piece.set(7, -8, PUMPKIN);
            piece.place(canvas, x, y, outline);
            canvas.set(x - 1, y - 1, outline);
            canvas.set(x + 1, y - 1, outline);
        }
        ShelterDecorationKind::Pinwheel => {
            // Four sails turning about a pin, two in each colour.
            let (x, hub) = (frame.cx, frame.peak_y - 11);
            canvas.line(x, frame.peak_y + 1, x, hub + 2, 1, outline);
            let sail = [
                (0, -1),
                (0, -2),
                (0, -3),
                (0, -4),
                (0, -5),
                (1, -2),
                (1, -3),
                (1, -4),
                (2, -3),
                (2, -4),
                (3, -4),
            ];
            for (turn, color) in [accent, highlight, accent, highlight]
                .into_iter()
                .enumerate()
            {
                for (sx, sy) in sail {
                    // A quarter turn each: (x, y) to (-y, x).
                    let (mut px, mut py) = (sx, sy);
                    for _ in 0..turn {
                        (px, py) = (-py, px);
                    }
                    piece.set(px, py, color);
                }
            }
            piece.set(0, 0, CREAM);
            piece.place(canvas, x, hub, outline);
            canvas.set(x, hub, outline);
        }
        // -----------------------------------------------------------------------------------
        // The eaves. A string or a wire across under the roof, and what hangs along it.
        // -----------------------------------------------------------------------------------
        ShelterDecorationKind::Banner => {
            // Bunting: a string of triangular flags, three colours turn about.
            let span = (frame.eave_half - 2).max(8);
            let (left, right) = (frame.cx - span, frame.cx + span);
            let at = sagging_line(canvas, left, right, frame.eave_y, 2.0, outline);
            let flags = ((span * 2) / 8).clamp(3, 7);
            let colors = [accent, highlight, coat];
            for index in 0..flags {
                let fx = left + 3 + (span * 2 - 6) * index / (flags - 1).max(1);
                let mut flag = Piece::new();
                flag.triangle(-2, 2, 0, 0, 5, colors[index as usize % 3]);
                flag.set(-1, 0, mix(colors[index as usize % 3], CREAM, 0.35));
                flag.place(canvas, fx, at(fx) + 1, outline);
            }
            // The string again over the flags' tops, so it reads as one line they hang from.
            let _ = sagging_line(canvas, left, right, frame.eave_y, 2.0, outline);
        }
        ShelterDecorationKind::FairyLights => {
            // A sagging wire with round bulbs along it, glowing after dark.
            let span = (frame.eave_half - 1).max(8);
            let (left, right) = (frame.cx - span, frame.cx + span);
            let at = sagging_line(canvas, left, right, frame.eave_y, 3.0, outline);
            let colors = [accent, highlight, coat];
            for (index, bx) in (left + 2..=right - 2).step_by(5).enumerate() {
                let mut bulb = Piece::new();
                let color = if lit {
                    GLOW
                } else {
                    colors[index % colors.len()]
                };
                bulb.rect(0, 1, 1, 3, color);
                bulb.set(
                    0,
                    1,
                    if lit {
                        GLOW_CORE
                    } else {
                        mix(color, CREAM, 0.5)
                    },
                );
                bulb.place(canvas, bx, at(bx) + 1, outline);
                if lit {
                    glow(canvas, bx, at(bx) + 3, 3);
                }
            }
        }
        ShelterDecorationKind::WindChime => {
            // From the end of the eave: a wooden bar, four tubes of different lengths, and the
            // striker hanging in the middle of them.
            let (x, y) = (frame.cx - frame.eave_half + 5, frame.eave_y);
            canvas.line(x, y, x, y + 2, 1, outline);
            piece.rect(-5, 3, 5, 4, WOOD_LIGHT);
            piece.rect(-5, 4, 5, 4, WOOD);
            for (dx, length) in [(-4, 8), (-1, 11), (2, 10), (5, 7)] {
                piece.rect(dx, 6, dx, 5 + length, METAL);
                piece.set(dx, 6, METAL_LIGHT);
            }
            piece.circle(0, 13, 1, accent);
            piece.place(canvas, x, y, outline);
            for dx in [-4, -1, 2, 5] {
                canvas.set(x + dx, y + 5, outline);
            }
        }
        ShelterDecorationKind::LeafGarland => {
            // A cord strung under the eaves, with leaves along it turning up and down.
            let span = (frame.eave_half - 1).max(8);
            let (left, right) = (frame.cx - span, frame.cx + span);
            let at = sagging_line(canvas, left, right, frame.eave_y, 2.0, LEAF_DARK);
            for (index, lx) in (left + 2..=right - 2).step_by(4).enumerate() {
                let mut leaf = Piece::new();
                let up = index % 2 == 0;
                let dy = if up { -2 } else { 2 };
                leaf.ellipse(0, dy, 2, 1, LEAF);
                leaf.set(-1, dy, LEAF_LIGHT);
                leaf.place(canvas, lx, at(lx), LEAF_DARK);
            }
            let _ = sagging_line(canvas, left, right, frame.eave_y, 2.0, LEAF_DARK);
        }
        ShelterDecorationKind::PaperLanterns => {
            // Round paper lanterns on a string, ribbed, lit from inside after dark.
            let span = (frame.eave_half - 2).max(8);
            let (left, right) = (frame.cx - span, frame.cx + span);
            let at = sagging_line(canvas, left, right, frame.eave_y, 2.0, outline);
            let count = if span >= 18 { 3 } else { 2 };
            for index in 0..count {
                let lx = left + 4 + (span * 2 - 8) * index / (count - 1);
                let paper = if lit {
                    GLOW
                } else if index % 2 == 0 {
                    accent
                } else {
                    highlight
                };
                let mut lantern = Piece::new();
                lantern.rect(-1, 1, 1, 1, WOOD_DARK);
                lantern.ellipse(0, 5, 3, 3, paper);
                lantern.rect(0, 2, 0, 8, mix(paper, WOOD_DARK, 0.3));
                lantern.set(
                    -2,
                    4,
                    if lit {
                        GLOW_CORE
                    } else {
                        mix(paper, CREAM, 0.45)
                    },
                );
                lantern.rect(-1, 9, 1, 9, WOOD_DARK);
                lantern.set(0, 10, accent);
                lantern.place(canvas, lx, at(lx), outline);
                if lit {
                    glow(canvas, lx, at(lx) + 5, 5);
                }
            }
        }
        // -----------------------------------------------------------------------------------
        // The left of the wall, between its edge and the door.
        // -----------------------------------------------------------------------------------
        ShelterDecorationKind::Leaf => {
            // A sprig of three leaves on a curving stem, with a berry.
            let (x, y) = (wall_left, frame.wall_y);
            piece.line(-1, 6, 1, -5, LEAF_DARK);
            piece.ellipse(-3, -3, 3, 2, LEAF);
            piece.ellipse(3, 0, 3, 2, LEAF);
            piece.ellipse(-2, 3, 3, 2, LEAF);
            piece.set(-4, -4, LEAF_LIGHT);
            piece.set(4, -1, LEAF_LIGHT);
            piece.set(-3, 2, LEAF_LIGHT);
            piece.line(-1, 6, 1, -5, LEAF_DARK);
            piece.circle(2, -5, 1, accent);
            piece.place(canvas, x, y, outline);
        }
        ShelterDecorationKind::Wreath => {
            // A ring of leaves with berries, and a bow at the bottom.
            let (x, y) = (wall_left, frame.wall_y - 1);
            piece.circle(0, 0, 5, LEAF);
            for (dx, dy) in [(-4, -2), (-2, -4), (2, -4), (4, -2), (4, 2), (-4, 2)] {
                piece.set(dx, dy, LEAF_DARK);
            }
            for (dx, dy) in [(-3, -3), (3, -3), (-1, -5)] {
                piece.set(dx, dy, LEAF_LIGHT);
            }
            piece.circle(0, 0, 2, Rgba::TRANSPARENT);
            for (dx, dy) in [(-2, -2), (2, -2), (-2, 2), (2, 2)] {
                piece.clear(dx, dy);
            }
            for (dx, dy) in [(-5, 0), (5, 1), (-3, 4), (3, -4)] {
                piece.set(dx, dy, accent);
            }
            // The bow.
            piece.rect(-3, 5, -1, 6, highlight);
            piece.rect(1, 5, 3, 6, highlight);
            piece.set(0, 5, accent);
            piece.rect(-1, 7, -1, 8, highlight);
            piece.rect(1, 7, 1, 8, highlight);
            piece.place(canvas, x, y, outline);
        }
        ShelterDecorationKind::WindowBox => {
            // A planter box on the wall, flowers of three colours coming up out of it.
            let (x, y) = (wall_left, frame.wall_y + 1);
            piece.rect(-6, 1, 6, 4, WOOD_LIGHT);
            piece.rect(-6, 4, 6, 4, WOOD);
            piece.rect(-6, 1, 6, 1, mix(WOOD_LIGHT, CREAM, 0.3));
            for (dx, color) in [(-4, accent), (-1, highlight), (2, coat), (5, accent)] {
                piece.rect(dx, -1, dx, 0, LEAF);
                piece.rect(dx - 1, -3, dx, -2, color);
                piece.set(dx + 1, -1, LEAF_DARK);
            }
            piece.place(canvas, x, y, outline);
        }
        ShelterDecorationKind::Ivy => {
            // A vine climbing the left of the wall from the ground, in leaves the whole way up.
            let x = frame.cx - frame.wall_half + 3;
            let (bottom, top) = (frame.ground_y - 1, frame.wall_y - 7);
            let origin_y = bottom;
            let mut y = bottom;
            let mut turn = 0;
            while y > top {
                let sway = if (y / 4) % 2 == 0 { 0 } else { 1 };
                piece.set(sway, y - origin_y, LEAF_DARK);
                if (bottom - y) % 3 == 1 {
                    let side = if turn % 2 == 0 { -2 } else { 2 };
                    let (lx, ly) = (sway + side, y - origin_y);
                    piece.rect(lx - 1, ly, lx + 1, ly, LEAF);
                    piece.set(lx, ly - 1, LEAF);
                    piece.set(lx, ly, LEAF_LIGHT);
                    turn += 1;
                }
                y -= 1;
            }
            piece.set(0, top - origin_y, LEAF_LIGHT);
            piece.place(canvas, x, origin_y, LEAF_DARK);
        }
        ShelterDecorationKind::HouseSign => {
            // A plank hung from a nail on two strings, a painted heart on it.
            let (x, y) = (wall_left, frame.wall_y);
            piece.rect(-5, -2, 5, 3, WOOD_LIGHT);
            piece.rect(-5, 3, 5, 3, WOOD);
            piece.rect(-4, 0, -2, 0, mix(WOOD_LIGHT, WOOD, 0.4));
            // The heart.
            piece.rect(-2, -1, -1, -1, accent);
            piece.rect(1, -1, 2, -1, accent);
            piece.rect(-2, 0, 2, 0, accent);
            piece.rect(-1, 1, 1, 1, accent);
            piece.set(0, 2, accent);
            piece.set(-2, -1, highlight);
            piece.place(canvas, x, y, outline);
            canvas.line(x - 4, y - 3, x, y - 6, 1, outline);
            canvas.line(x + 4, y - 3, x, y - 6, 1, outline);
            canvas.set(x, y - 7, METAL);
        }
        // -----------------------------------------------------------------------------------
        // The right of the wall, between the door and its edge.
        // -----------------------------------------------------------------------------------
        ShelterDecorationKind::Lamp => {
            // A lamp on a bracket under the eaves: cap, glass and a finial, lit after dark.
            let (x, y) = (wall_right, frame.wall_y - 3);
            piece.rect(-5, 3, -1, 3, WOOD_DARK);
            piece.rect(-1, 1, -1, 3, WOOD_DARK);
            piece.rect(-1, -4, 3, 1, WOOD_DARK);
            let glass = if lit {
                GLOW
            } else {
                mix(highlight, CREAM, 0.3)
            };
            piece.rect(0, -3, 2, 0, glass);
            piece.set(1, -2, if lit { GLOW_CORE } else { CREAM });
            piece.rect(-2, -5, 4, -5, accent);
            piece.rect(0, -6, 2, -6, accent);
            piece.set(1, -7, highlight);
            piece.place(canvas, x, y, outline);
            if lit {
                glow(canvas, x + 1, y - 1, 5);
            }
        }
        ShelterDecorationKind::Clock => {
            // A round clock with a coloured rim, its hours marked and both hands showing.
            let (x, y) = (wall_right, frame.wall_y - 2);
            piece.circle(0, 0, 4, accent);
            piece.circle(0, 0, 3, CREAM);
            for (dx, dy) in [(0, -3), (3, 0), (0, 3), (-3, 0)] {
                piece.set(dx, dy, WOOD_DARK);
            }
            piece.rect(0, -2, 0, 0, WOOD_DARK);
            piece.rect(0, 0, 2, 0, WOOD_DARK);
            piece.set(-2, -4, highlight);
            piece.place(canvas, x, y, outline);
        }
        ShelterDecorationKind::Birdhouse => {
            // A little house on a post: a pitched roof, a round door and a perch.
            let (x, y) = (wall_right, frame.wall_y - 2);
            piece.triangle(-5, 5, -2, 0, -7, accent);
            piece.rect(-3, -1, 3, 5, WOOD_LIGHT);
            piece.rect(-3, 5, 3, 5, WOOD);
            piece.set(-1, -5, highlight);
            piece.circle(0, 1, 1, WOOD_DARK);
            piece.rect(-1, 4, 1, 4, WOOD_DARK);
            piece.rect(0, 6, 0, 8, WOOD);
            piece.place(canvas, x, y, outline);
        }
        ShelterDecorationKind::Horseshoe => {
            // A lucky shoe nailed up the right way, so the luck stays in.
            let (x, y) = (wall_right, frame.wall_y - 3);
            piece.rect(-3, -3, -2, 2, METAL);
            piece.rect(2, -3, 3, 2, METAL);
            piece.rect(-2, 3, 2, 4, METAL);
            piece.set(-3, 3, METAL);
            piece.set(3, 3, METAL);
            piece.rect(-3, -3, -2, -3, METAL_LIGHT);
            piece.rect(2, -3, 3, -3, METAL_LIGHT);
            for (dx, dy) in [(-3, -1), (-3, 1), (3, -1), (3, 1), (0, 4)] {
                piece.set(dx, dy, WOOD_DARK);
            }
            piece.place(canvas, x, y, outline);
        }
        ShelterDecorationKind::Mailbox => {
            // A letterbox on the wall, round-topped, with its slot and its little flag up.
            let (x, y) = (wall_right, frame.wall_y - 1);
            piece.rect(-4, -2, 4, 3, coat);
            piece.rect(-3, -3, 3, -3, coat);
            piece.rect(-3, -3, 3, -3, mix(coat, CREAM, 0.4));
            piece.rect(-2, 0, 2, 0, WOOD_DARK);
            piece.rect(-4, 3, 4, 3, mix(coat, WOOD_DARK, 0.4));
            piece.rect(5, -6, 5, 1, WOOD_DARK);
            piece.rect(6, -6, 8, -4, accent);
            piece.place(canvas, x, y, outline);
        }
        // -----------------------------------------------------------------------------------
        // The ground on the left, against the corner of the house.
        // -----------------------------------------------------------------------------------
        ShelterDecorationKind::Stone => {
            // A broad flat doorstone set in the grass, a little moss along its edge.
            let (x, y) = (left_x, frame.ground_y);
            piece.ellipse(0, -2, 6, 2, STONE);
            piece.rect(-5, -1, 5, -1, mix(STONE, WOOD_DARK, 0.35));
            piece.rect(-3, -4, 2, -4, STONE_LIGHT);
            piece.rect(-5, -3, -3, -3, STONE_LIGHT);
            piece.rect(3, -3, 5, -2, LEAF);
            piece.set(4, -4, LEAF_LIGHT);
            piece.place(canvas, x, y, outline);
        }
        ShelterDecorationKind::Woodpile => {
            // Logs stacked three, two and one, their cut ends to the front.
            let (x, y) = (left_x, frame.ground_y - 2);
            for (dx, dy) in [(-4, 0), (0, 0), (4, 0), (-2, -4), (2, -4), (0, -8)] {
                piece.circle(dx, dy, 2, WOOD_LIGHT);
                piece.set(dx, dy, WOOD);
                piece.set(dx - 1, dy - 1, mix(WOOD_LIGHT, CREAM, 0.4));
            }
            piece.place(canvas, x, y, WOOD_DARK);
            canvas.fill_rect(x - 6, y + 2, 13, 1, outline);
        }
        ShelterDecorationKind::WateringCan => {
            // A can with its spout reaching up toward the house, and a handle over the top.
            let (x, y) = (left_x, frame.ground_y);
            piece.rect(-4, -7, 2, -1, coat);
            piece.rect(-4, -5, 2, -5, highlight);
            piece.rect(-3, -9, 0, -9, coat);
            piece.set(-4, -8, coat);
            piece.set(1, -8, coat);
            piece.line(3, -4, 6, -8, coat);
            piece.line(3, -3, 6, -7, coat);
            piece.rect(6, -10, 8, -9, METAL);
            piece.set(7, -10, METAL_LIGHT);
            piece.place(canvas, x, y, outline);
        }
        ShelterDecorationKind::Boots => {
            // A pair of wellington boots left by the door, toes to the front: a tall leg, a
            // rounded toe, a turned-down cuff and a dark sole.
            let (x, y) = (left_x, frame.ground_y);
            for dx in [-5, 1] {
                piece.rect(dx, -9, dx + 2, -3, accent);
                piece.rect(dx, -3, dx + 4, -1, accent);
                piece.set(dx + 4, -3, Rgba::TRANSPARENT);
                piece.rect(dx - 1, -10, dx + 3, -9, highlight);
                piece.rect(dx, 0, dx + 4, 0, WOOD_DARK);
                piece.set(dx + 1, -6, mix(accent, CREAM, 0.35));
            }
            piece.place(canvas, x, y, outline);
        }
        ShelterDecorationKind::Barrel => {
            // A water barrel, its staves and hoops, rainwater at the brim.
            let (x, y) = (left_x, frame.ground_y);
            piece.rect(-4, -11, 4, -1, WOOD);
            piece.rect(-5, -9, 5, -3, WOOD);
            piece.rect(-3, -11, -3, -1, WOOD_LIGHT);
            piece.rect(1, -11, 1, -1, WOOD_LIGHT);
            for hoop in [-9, -3] {
                piece.rect(-5, hoop, 5, hoop, METAL);
            }
            piece.rect(-3, -12, 3, -12, mix(METAL_LIGHT, accent, 0.4));
            piece.place(canvas, x, y, outline);
        }
        // -----------------------------------------------------------------------------------
        // The ground on the right, against the other corner.
        // -----------------------------------------------------------------------------------
        ShelterDecorationKind::Flower => {
            // A tall flower: two leaves on its stem and a five-petalled bloom.
            let (x, y) = (right_x, frame.ground_y);
            piece.ellipse(-2, -5, 2, 1, LEAF);
            piece.ellipse(2, -7, 2, 1, LEAF);
            for (dx, dy) in [(0, -16), (-3, -13), (3, -13), (-2, -10), (2, -10)] {
                piece.circle(dx, dy, 2, accent);
            }
            piece.circle(0, -13, 1, highlight);
            piece.set(0, -13, CREAM);
            piece.place(canvas, x, y, outline);
            canvas.line(x, y - 1, x, y - 10, 1, LEAF_DARK);
        }
        ShelterDecorationKind::PottedPlant => {
            // A clay pot with a rim, and a round bushy plant in it.
            let (x, y) = (right_x, frame.ground_y);
            for (row, half) in [(-5, 3), (-4, 3), (-3, 3), (-2, 2), (-1, 2)] {
                piece.rect(-half, row, half, row, CLAY);
            }
            piece.rect(-4, -6, 4, -6, mix(CLAY, CREAM, 0.3));
            for (dx, dy) in [(-2, -9), (2, -9), (0, -11)] {
                piece.circle(dx, dy, 3, LEAF);
            }
            piece.set(-3, -10, LEAF_LIGHT);
            piece.set(0, -12, LEAF_LIGHT);
            piece.set(2, -8, LEAF_DARK);
            piece.place(canvas, x, y, outline);
        }
        ShelterDecorationKind::Pumpkin => {
            // A fat ribbed pumpkin with a stalk and a curl of leaf.
            let (x, y) = (right_x, frame.ground_y - 4);
            piece.ellipse(0, 0, 5, 3, PUMPKIN);
            for rib in [-2, 0, 2] {
                piece.rect(rib, -2, rib, 2, mix(PUMPKIN, WOOD, 0.4));
            }
            piece.rect(-4, -1, -3, -1, mix(PUMPKIN, CREAM, 0.45));
            piece.rect(0, -5, 0, -4, WOOD_DARK);
            piece.rect(1, -5, 2, -5, LEAF);
            piece.place(canvas, x, y, outline);
        }
        ShelterDecorationKind::Mushrooms => {
            // A big mushroom and a small one, spotted, come up by the wall.
            let (x, y) = (right_x, frame.ground_y);
            piece.rect(-3, -5, -1, -1, CREAM);
            piece.ellipse(-2, -7, 4, 2, coat);
            piece.set(-3, -8, CREAM);
            piece.set(0, -7, CREAM);
            piece.rect(3, -3, 4, -1, CREAM);
            piece.ellipse(4, -4, 2, 1, coat);
            piece.set(4, -5, CREAM);
            piece.place(canvas, x, y, outline);
        }
        ShelterDecorationKind::Lantern => {
            // A lantern on a post: cap and ring, glass, a base, lit after dark.
            let (x, y) = (right_x, frame.ground_y);
            piece.rect(0, -6, 0, -1, WOOD);
            piece.rect(-2, -7, 2, -7, accent);
            piece.rect(-2, -13, 2, -8, WOOD_DARK);
            let glass = if lit {
                GLOW
            } else {
                mix(STONE, highlight, 0.4)
            };
            piece.rect(-1, -12, 1, -9, glass);
            piece.set(0, -11, if lit { GLOW_CORE } else { CREAM });
            piece.rect(-3, -14, 3, -14, accent);
            piece.rect(-1, -15, 1, -15, accent);
            piece.set(0, -16, highlight);
            piece.place(canvas, x, y, outline);
            if lit {
                glow(canvas, x, y - 10, 5);
            }
        }
    }
}
