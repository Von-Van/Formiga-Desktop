//! What a house can be decorated with, one drawing per kind, each hung from the anchor its slot
//! resolves on the house's own silhouette: the peak, the eaves, either side of the wall, and the
//! ground either side of the door.
//!
//! The first six are the decorations a colony could earn before 0.60.0 and are drawn exactly as
//! they always were. Everything is drawn at a fixed small size so that a cottage wears the same
//! wreath as the colony house, and every drawing stays inside the ground its house's lot claims.

use super::{ShelterFrame, fill_triangle, houses::mix};
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
const LEAF: Rgba = Rgba::new(92, 158, 80, 255);
const LEAF_DARK: Rgba = Rgba::new(58, 112, 62, 255);
const STONE: Rgba = Rgba::new(168, 164, 156, 255);
const METAL: Rgba = Rgba::new(150, 156, 168, 255);
const METAL_LIGHT: Rgba = Rgba::new(208, 212, 220, 255);
const PUMPKIN: Rgba = Rgba::new(232, 132, 52, 255);
const CREAM: Rgba = Rgba::new(240, 228, 204, 255);
const GLOW: Rgba = Rgba::new(255, 206, 118, 255);
const GLOW_CORE: Rgba = Rgba::new(255, 236, 178, 255);

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
    // A cottage's lot is narrower than the colony house's, so what stands beside it stands a
    // little closer in, and so does anything newer than the six a colony could earn before 0.60.0;
    // those six stand by the colony house exactly where they always have.
    let pull = if frame.cottage {
        4
    } else if kind.index() >= 6 {
        2
    } else {
        0
    };
    // Where a ground piece on either side stands.
    let left_x = frame.cx - frame.ground_half - 2 - drift + pull;
    let right_x = frame.cx + frame.ground_half + 2 + drift - pull;
    match kind {
        ShelterDecorationKind::Leaf => {
            let x = frame.cx - frame.wall_half + 3;
            let y = frame.wall_y - 4;
            canvas.line(x, y + 9, x + 2, y, 1, outline);
            canvas.fill_ellipse(x - 1, y + 2, 3, 2, coat);
            canvas.fill_ellipse(x + 3, y + 6, 3, 2, highlight);
            canvas.line(x, y + 3, x + 4, y + 6, 1, accent);
        }
        ShelterDecorationKind::Banner => {
            // Strung under the eaves, spanning the roof it actually hangs from.
            let span = (frame.eave_half - 2).max(6);
            let y = frame.eave_y;
            canvas.line(frame.cx - span, y, frame.cx + span, y, 1, outline);
            for (index, offset) in (-1..=1).enumerate() {
                let x = frame.cx + offset * (span - 3) - 2;
                fill_triangle(
                    canvas,
                    x + 2,
                    y + 1,
                    x,
                    y + 4,
                    x + 5,
                    y + 4,
                    if index % 2 == 0 { accent } else { highlight },
                );
            }
        }
        ShelterDecorationKind::Stone => {
            let x = left_x;
            let y = frame.ground_y - 2;
            canvas.fill_ellipse(x, y, 4, 3, outline);
            canvas.fill_ellipse(x, y - 1, 3, 2, coat);
            canvas.set(x + 2, y - 2, highlight);
        }
        ShelterDecorationKind::Flower => {
            let x = right_x;
            let base = frame.ground_y - 1;
            canvas.line(x, base, x, base - 9, 1, coat);
            canvas.fill_ellipse(x - 2, base - 10, 3, 2, accent);
            canvas.fill_ellipse(x + 2, base - 10, 3, 2, accent);
            canvas.fill_ellipse(x, base - 13, 2, 3, highlight);
            canvas.set(x, base - 10, outline);
        }
        ShelterDecorationKind::Lamp => {
            // Bracketed onto the wall face, just under the eaves.
            let x = frame.cx + frame.wall_half - 1;
            let y = frame.wall_y - 3;
            canvas.line(x - 5, y + 5, x, y + 5, 1, outline);
            canvas.line(x, y + 5, x, y + 3, 1, outline);
            canvas.fill_ellipse(x, y, 3, 4, outline);
            if lit {
                // Lit after dark, like the windows.
                canvas.fill_ellipse(x, y, 2, 3, Rgba::new(255, 206, 118, 255));
                canvas.set(x, y, Rgba::new(255, 236, 178, 255));
            } else {
                canvas.fill_ellipse(x, y, 2, 3, highlight);
                canvas.set(x, y, accent);
            }
        }
        ShelterDecorationKind::RoofOrnament => {
            let y = frame.peak_y - 4;
            canvas.line(frame.cx, frame.peak_y + 2, frame.cx, y, 1, outline);
            canvas.line(frame.cx - 3, y, frame.cx + 3, y, 1, accent);
            canvas.line(frame.cx, y - 3, frame.cx, y + 3, 1, accent);
            canvas.line(frame.cx - 2, y - 2, frame.cx + 2, y + 2, 1, highlight);
            canvas.line(frame.cx - 2, y + 2, frame.cx + 2, y - 2, 1, highlight);
        }
        // ---------------------------------------------------------------------------------
        // The roof.
        // ---------------------------------------------------------------------------------
        ShelterDecorationKind::WeatherVane => {
            // A post up from the peak, a ball, and an arrow across the top of it.
            let (x, top) = (frame.cx, frame.peak_y - 8);
            canvas.line(x, frame.peak_y + 1, x, top, 1, outline);
            canvas.fill_circle(x, top + 3, 1, METAL);
            canvas.line(x - 4, top, x + 3, top, 1, outline);
            fill_triangle(canvas, x + 4, top, x + 3, top + 1, x + 3, top - 1, accent);
            canvas.set(x + 4, top, accent);
            canvas.set(x + 3, top - 1, accent);
            canvas.set(x + 3, top + 1, accent);
            canvas.set(x - 4, top - 1, METAL_LIGHT);
            canvas.set(x - 4, top + 1, METAL_LIGHT);
            canvas.set(x - 3, top - 1, METAL);
            canvas.set(x - 3, top + 1, METAL);
        }
        ShelterDecorationKind::Pennant => {
            // A taller pole than any house has of its own, with a long pennant flying off it.
            let (x, top) = (frame.cx, frame.peak_y - 10);
            canvas.line(x, frame.peak_y + 1, x, top, 1, outline);
            for row in 0..5 {
                let reach = 7 - row * 3 / 2;
                canvas.fill_rect(x + 1, top + row, reach.max(1), 1, accent);
            }
            canvas.fill_rect(x + 1, top, 5, 1, highlight);
            canvas.set(x, top - 1, highlight);
        }
        ShelterDecorationKind::PerchedBird => {
            // A small round bird sitting on the very top, looking out to one side.
            let (x, y) = (frame.cx, frame.peak_y - 3);
            canvas.fill_ellipse(x, y, 3, 2, outline);
            canvas.fill_ellipse(x, y, 2, 1, coat);
            canvas.fill_circle(x + 2, y - 2, 2, outline);
            canvas.fill_circle(x + 2, y - 2, 1, coat);
            canvas.set(x + 2, y - 2, outline);
            canvas.set(x + 4, y - 2, accent);
            canvas.set(x - 3, y - 1, outline);
            canvas.set(x - 4, y - 2, outline);
            canvas.set(x - 1, y, highlight);
            canvas.set(x, y + 2, outline);
        }
        ShelterDecorationKind::Pinwheel => {
            // A four-sailed wheel on a short stick, two sails in each colour.
            let (x, y) = (frame.cx, frame.peak_y - 6);
            canvas.line(x, frame.peak_y + 1, x, y, 1, outline);
            for (dx, dy, color) in [
                (0, -1, accent),
                (1, 0, highlight),
                (0, 1, accent),
                (-1, 0, highlight),
            ] {
                for step in 1..=3 {
                    canvas.set(x + dx * step, y + dy * step, color);
                }
                canvas.set(x + dx * 3 - dy, y + dy * 3 + dx, color);
            }
            canvas.set(x, y, outline);
        }
        // ---------------------------------------------------------------------------------
        // The eaves.
        // ---------------------------------------------------------------------------------
        ShelterDecorationKind::FairyLights => {
            // A wire sagging under the eaves with little bulbs along it, glowing after dark.
            let span = (frame.eave_half - 1).max(6);
            let y = frame.eave_y;
            let mut previous = None;
            for x in frame.cx - span..=frame.cx + span {
                let t = (x - (frame.cx - span)) as f32 / (span * 2).max(1) as f32;
                let sag = (4.0 * t * (1.0 - t) * 3.0).round() as i32;
                canvas.set(x, y + sag, outline);
                if (x - frame.cx).rem_euclid(3) == 0 && previous != Some(x) {
                    let color = if lit {
                        if (x / 3) % 2 == 0 { GLOW } else { GLOW_CORE }
                    } else if (x / 3) % 2 == 0 {
                        accent
                    } else {
                        highlight
                    };
                    canvas.set(x, y + sag + 1, color);
                    previous = Some(x);
                }
            }
        }
        ShelterDecorationKind::WindChime => {
            // Hung from the end of the eave: a little bar and four tubes of different lengths.
            let (x, y) = (frame.cx - frame.eave_half + 2, frame.eave_y);
            canvas.line(x, y, x, y + 2, 1, outline);
            canvas.line(x - 3, y + 2, x + 3, y + 2, 1, WOOD);
            for (dx, length) in [(-3, 5), (-1, 7), (1, 6), (3, 4)] {
                canvas.line(x + dx, y + 3, x + dx, y + 2 + length, 1, METAL);
                canvas.set(x + dx, y + 3, METAL_LIGHT);
            }
            canvas.set(x, y + 9, accent);
        }
        ShelterDecorationKind::LeafGarland => {
            // A cord strung under the eaves with leaves along it, alternately up and down.
            let span = (frame.eave_half - 1).max(6);
            let y = frame.eave_y;
            canvas.line(frame.cx - span, y, frame.cx + span, y, 1, LEAF_DARK);
            for (index, x) in (frame.cx - span + 1..=frame.cx + span - 1)
                .step_by(3)
                .enumerate()
            {
                let dy = if index % 2 == 0 { 1 } else { -1 };
                canvas.set(x, y + dy, LEAF);
                canvas.set(x + 1, y + dy * 2, LEAF);
                canvas.set(x + 1, y + dy, mix(LEAF, CREAM, 0.3));
            }
        }
        ShelterDecorationKind::PaperLanterns => {
            // Three round paper lanterns on a string, lit from inside after dark.
            let span = (frame.eave_half - 2).max(6);
            let y = frame.eave_y;
            canvas.line(frame.cx - span, y, frame.cx + span, y, 1, outline);
            for (index, offset) in (-1..=1).enumerate() {
                let x = frame.cx + offset * (span - 2);
                canvas.set(x, y + 1, outline);
                canvas.fill_ellipse(x, y + 4, 2, 2, outline);
                let paper = if lit {
                    GLOW
                } else if index % 2 == 0 {
                    accent
                } else {
                    highlight
                };
                canvas.fill_ellipse(x, y + 4, 1, 1, paper);
                canvas.set(x, y + 4, if lit { GLOW_CORE } else { paper });
                canvas.set(x - 1, y + 3, paper);
                canvas.set(x + 1, y + 3, paper);
            }
        }
        // ---------------------------------------------------------------------------------
        // The left of the wall.
        // ---------------------------------------------------------------------------------
        ShelterDecorationKind::Wreath => {
            // A ring of green with a bow at the bottom.
            let (x, y) = (frame.cx - frame.wall_half + 4, frame.wall_y - 1);
            canvas.fill_circle(x, y, 3, outline);
            canvas.fill_circle(x, y, 2, LEAF);
            canvas.fill_circle(x, y, 1, outline);
            canvas.set(x, y, Rgba::TRANSPARENT);
            canvas.set(x - 1, y - 2, mix(LEAF, CREAM, 0.35));
            canvas.set(x + 2, y - 1, accent);
            canvas.set(x - 1, y + 3, accent);
            canvas.set(x + 1, y + 3, accent);
            canvas.set(x, y + 3, highlight);
        }
        ShelterDecorationKind::WindowBox => {
            // A little planter box on the wall with three flowers in it.
            let (x, y) = (frame.cx - frame.wall_half + 1, frame.wall_y + 1);
            canvas.fill_rect(x, y, 8, 3, outline);
            canvas.fill_rect(x + 1, y + 1, 6, 1, WOOD_LIGHT);
            for (dx, color) in [(1, accent), (4, highlight), (6, coat)] {
                canvas.set(x + dx, y - 1, LEAF);
                canvas.set(x + dx, y - 2, color);
                canvas.set(x + dx + 1, y - 1, LEAF_DARK);
            }
        }
        ShelterDecorationKind::Ivy => {
            // A vine climbing the left of the wall from the ground, leaves either side.
            let x = frame.cx - frame.wall_half + 2;
            let bottom = frame.ground_y - 1;
            let top = frame.wall_y - 5;
            let mut y = bottom;
            let mut turn = 0;
            while y > top {
                let sway = if (y / 3) % 2 == 0 { 0 } else { 1 };
                canvas.set(x + sway, y, LEAF_DARK);
                if y % 2 == 0 {
                    let side = if turn % 2 == 0 { -1 } else { 1 };
                    canvas.set(x + sway + side, y - 1, LEAF);
                    turn += 1;
                }
                y -= 1;
            }
            canvas.set(x, top, LEAF);
        }
        ShelterDecorationKind::HouseSign => {
            // A plank hung from two strings, with a small painted mark on it.
            let (x, y) = (frame.cx - frame.wall_half + 1, frame.wall_y - 3);
            canvas.set(x + 1, y - 1, outline);
            canvas.set(x + 6, y - 1, outline);
            canvas.fill_rect(x, y, 8, 5, outline);
            canvas.fill_rect(x + 1, y + 1, 6, 3, WOOD_LIGHT);
            canvas.fill_rect(x + 1, y + 3, 6, 1, WOOD);
            canvas.set(x + 3, y + 2, accent);
            canvas.set(x + 4, y + 2, accent);
            canvas.set(x + 4, y + 1, highlight);
        }
        // ---------------------------------------------------------------------------------
        // The right of the wall.
        // ---------------------------------------------------------------------------------
        ShelterDecorationKind::Clock => {
            // A round clock face on the wall with its two hands.
            let (x, y) = (frame.cx + frame.wall_half - 3, frame.wall_y - 2);
            canvas.fill_circle(x, y, 3, outline);
            canvas.fill_circle(x, y, 2, CREAM);
            canvas.set(x, y, outline);
            canvas.set(x, y - 1, outline);
            canvas.set(x + 1, y, outline);
            canvas.set(x, y - 3, accent);
        }
        ShelterDecorationKind::Birdhouse => {
            // A tiny house on a bracket, with a round door and a pitched roof.
            let (x, y) = (frame.cx + frame.wall_half - 4, frame.wall_y - 4);
            fill_triangle(canvas, x + 2, y - 3, x - 1, y, x + 5, y, outline);
            fill_triangle(canvas, x + 2, y - 2, x, y, x + 4, y, accent);
            canvas.fill_rect(x, y + 1, 5, 5, outline);
            canvas.fill_rect(x + 1, y + 1, 3, 4, WOOD_LIGHT);
            canvas.set(x + 2, y + 2, outline);
            canvas.line(x + 2, y + 6, x + 2, y + 7, 1, outline);
        }
        ShelterDecorationKind::Horseshoe => {
            // A lucky shoe nailed up the right way, so the luck stays in.
            let (x, y) = (frame.cx + frame.wall_half - 3, frame.wall_y - 3);
            for (dx, dy) in [
                (-2, 0),
                (-2, 1),
                (-2, 2),
                (-1, 3),
                (0, 3),
                (1, 3),
                (2, 2),
                (2, 1),
                (2, 0),
            ] {
                canvas.set(x + dx, y + dy, METAL);
            }
            canvas.set(x - 2, y, METAL_LIGHT);
            canvas.set(x + 2, y, METAL_LIGHT);
            canvas.set(x, y + 3, outline);
        }
        ShelterDecorationKind::Mailbox => {
            // A letterbox on the wall with its little flag up.
            let (x, y) = (frame.cx + frame.wall_half - 5, frame.wall_y - 2);
            canvas.fill_rect(x, y, 6, 4, outline);
            canvas.fill_rect(x + 1, y + 1, 4, 2, coat);
            canvas.fill_rect(x + 1, y + 1, 4, 1, highlight);
            canvas.line(x + 5, y, x + 5, y - 3, 1, outline);
            canvas.set(x + 6, y - 3, accent);
            canvas.set(x + 6, y - 2, accent);
        }
        // ---------------------------------------------------------------------------------
        // The ground on the left.
        // ---------------------------------------------------------------------------------
        ShelterDecorationKind::Woodpile => {
            // Logs stacked three and two, their cut ends to the front.
            let (x, y) = (left_x, frame.ground_y - 2);
            for (dx, dy) in [(-2, 0), (1, 0), (4, 0), (-1, -3), (2, -3)] {
                canvas.fill_circle(x + dx, y + dy, 2, outline);
                canvas.fill_circle(x + dx, y + dy, 1, WOOD_LIGHT);
                canvas.set(x + dx, y + dy, WOOD);
            }
        }
        ShelterDecorationKind::WateringCan => {
            // A can with its spout reaching up toward the house, and a handle over the top.
            let (x, y) = (left_x, frame.ground_y - 1);
            canvas.fill_rect(x - 2, y - 4, 6, 5, outline);
            canvas.fill_rect(x - 1, y - 3, 4, 3, coat);
            canvas.set(x - 1, y - 3, highlight);
            canvas.line(x + 4, y - 2, x + 6, y - 5, 1, outline);
            canvas.set(x + 7, y - 6, METAL);
            canvas.line(x - 1, y - 5, x + 2, y - 5, 1, outline);
        }
        ShelterDecorationKind::Boots => {
            // A pair of little boots left by the door.
            let (x, y) = (left_x, frame.ground_y - 1);
            for dx in [-3, 1] {
                canvas.fill_rect(x + dx, y - 4, 3, 5, outline);
                canvas.fill_rect(x + dx + 1, y - 3, 1, 3, accent);
                canvas.fill_rect(x + dx + 3, y - 1, 1, 2, outline);
                canvas.set(x + dx + 2, y, accent);
                canvas.set(x + dx + 1, y - 4, highlight);
            }
        }
        ShelterDecorationKind::Barrel => {
            // A water barrel with its hoops.
            let (x, y) = (left_x, frame.ground_y - 1);
            canvas.fill_rect(x - 3, y - 7, 7, 8, outline);
            canvas.fill_rect(x - 2, y - 6, 5, 6, WOOD);
            canvas.fill_rect(x - 2, y - 6, 1, 6, WOOD_LIGHT);
            for hoop in [y - 5, y - 2] {
                canvas.fill_rect(x - 3, hoop, 7, 1, METAL);
            }
            canvas.fill_rect(x - 2, y - 7, 5, 1, mix(METAL, accent, 0.5));
        }
        // ---------------------------------------------------------------------------------
        // The ground on the right.
        // ---------------------------------------------------------------------------------
        ShelterDecorationKind::PottedPlant => {
            // A clay pot with a round bushy plant in it.
            let (x, y) = (right_x, frame.ground_y - 1);
            canvas.fill_rect(x - 2, y - 3, 5, 4, outline);
            canvas.fill_rect(x - 1, y - 2, 3, 2, mix(PUMPKIN, WOOD, 0.5));
            canvas.fill_circle(x, y - 6, 3, outline);
            canvas.fill_circle(x, y - 6, 2, LEAF);
            canvas.set(x - 1, y - 7, mix(LEAF, CREAM, 0.35));
            canvas.set(x + 1, y - 5, LEAF_DARK);
        }
        ShelterDecorationKind::Pumpkin => {
            // A fat ribbed pumpkin with a stalk.
            let (x, y) = (right_x, frame.ground_y - 2);
            canvas.fill_ellipse(x, y, 4, 3, outline);
            canvas.fill_ellipse(x, y, 3, 2, PUMPKIN);
            canvas.line(x, y - 2, x, y + 2, 1, mix(PUMPKIN, WOOD, 0.45));
            canvas.set(x - 2, y - 1, mix(PUMPKIN, CREAM, 0.4));
            canvas.set(x, y - 4, LEAF_DARK);
            canvas.set(x + 1, y - 4, LEAF);
        }
        ShelterDecorationKind::Mushrooms => {
            // Two mushrooms, a big one and a small one, come up by the wall.
            let (x, y) = (right_x - 2, frame.ground_y - 1);
            for (dx, cap, stem) in [(0, 3, 3), (4, 2, 2)] {
                canvas.fill_rect(x + dx - 1, y - stem, 2, stem + 1, outline);
                canvas.set(x + dx, y - stem + 1, CREAM);
                canvas.fill_ellipse(x + dx, y - stem - 1, cap, cap - 1, outline);
                canvas.fill_ellipse(x + dx, y - stem - 1, cap - 1, (cap - 2).max(1), coat);
                canvas.set(x + dx - 1, y - stem - 2, CREAM);
            }
        }
        ShelterDecorationKind::Lantern => {
            // A lantern on a short post, lit after dark.
            let (x, y) = (right_x, frame.ground_y - 1);
            canvas.line(x, y, x, y - 6, 1, outline);
            canvas.fill_rect(x - 2, y - 11, 5, 6, outline);
            let glass = if lit {
                GLOW
            } else {
                mix(STONE, highlight, 0.4)
            };
            canvas.fill_rect(x - 1, y - 10, 3, 4, glass);
            if lit {
                canvas.set(x, y - 8, GLOW_CORE);
            } else {
                canvas.set(x - 1, y - 10, CREAM);
            }
            canvas.set(x, y - 12, accent);
        }
    }
}
