//! The ground the village stands on: a shallow strip of grass seen a little from above, with a
//! trodden path along it and a path back to every door, small pale bushes beyond it, and grass and
//! flowers along its front edge.
//!
//! It is drawn as two layers on one strip-shaped sheet: the back layer under the houses — the
//! grass, its paths and what grows beyond it — and the front layer over their feet, under the
//! companions. Each comes by day and after dark. A house set back on the ground is drawn that many
//! rows higher than the front of the grass, and its path runs back to its door; the sheet is laid
//! out from the village's own [`formiga_core::VillageGroundPlan`], so it is redrawn only when the
//! village is laid out again or a house moves.

use crate::paint::{noise, over};
use crate::{Canvas, Rgba, VillageLook};
use formiga_core::{TreeEnd, VillageGroundPlan, VillageLot};

/// How long the sheet is, in shelter pixels: room for the widest village there is.
pub const GROUND_STRIP_WIDTH: u32 = 640;
/// How tall each layer of the sheet is, from the front of the grass up past the bushes beyond it.
pub const GROUND_STRIP_HEIGHT: u32 = 24;
/// The sheet's four layers, one above the other: back and front by day, then after dark.
pub const GROUND_ATLAS_HEIGHT: u32 = GROUND_STRIP_HEIGHT * 4;
/// How deep the grass is, front edge to back, in shelter pixels.
pub const GROUND_BAND: i32 = 11;

/// The row the front of the grass is on, the bottom row of a layer. A quad drawn from the sheet
/// stands with its foot on the village's ground line, which puts this row where the houses' own
/// shadows end.
const FRONT: i32 = GROUND_STRIP_HEIGHT as i32 - 1;

/// One of the sheet's layers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GroundLayer {
    /// The grass and its paths, and what grows beyond them: drawn before the houses.
    Back,
    /// The grass and flowers along the front edge: drawn over the houses' feet.
    Front,
}

/// Everything the ground is drawn from. Two that compare equal draw the same sheet.
#[derive(Clone, Debug, PartialEq)]
pub struct VillageGroundLook {
    /// How far the village reaches, in shelter pixels.
    pub length: u16,
    /// Every house shown: its middle, half its drawn width, and how far back it stands.
    pub houses: Vec<GroundHouse>,
    /// The middle of every tree shown.
    pub trees: Vec<i32>,
    /// The colony's own seed for the small choices, so no two villages' grass is the same.
    pub seed: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GroundHouse {
    pub slot: usize,
    pub centre: i32,
    pub half: i32,
    pub depth: u8,
}

impl VillageGroundLook {
    /// The ground under a village laid out by `plan`, with its houses drawn as `look` draws them.
    pub fn of(plan: &VillageGroundPlan, look: &VillageLook) -> Self {
        let mut houses = Vec::new();
        let mut trees = Vec::new();
        for (lot, centre, _) in &plan.lots {
            let centre = centre.round() as i32;
            match lot {
                VillageLot::Dwelling(slot) => houses.push(GroundHouse {
                    slot: *slot,
                    centre,
                    half: crate::shelter::drawn_half_width(&look.genome, *slot),
                    depth: look.depths[(*slot).min(look.depths.len() - 1)],
                }),
                VillageLot::Tree(TreeEnd::Outward | TreeEnd::Inward) => trees.push(centre),
            }
        }
        houses.sort_by_key(|house| house.centre);
        Self {
            length: plan.length.round().clamp(0.0, GROUND_STRIP_WIDTH as f32) as u16,
            houses,
            trees,
            seed: look.genome.detail_seed,
        }
    }

    /// Where a layer's top-left sits on the sheet, in pixels.
    pub fn layer_origin(layer: GroundLayer, night: bool) -> (u32, u32) {
        let row = match layer {
            GroundLayer::Back => 0,
            GroundLayer::Front => 1,
        } + if night { 2 } else { 0 };
        (0, row * GROUND_STRIP_HEIGHT)
    }
}

pub struct VillageGroundRenderer;

impl VillageGroundRenderer {
    /// The whole sheet: both layers, by day and after dark.
    pub fn render(look: &VillageGroundLook) -> Canvas {
        let mut sheet = Canvas::new(GROUND_STRIP_WIDTH, GROUND_ATLAS_HEIGHT);
        let back = back_layer(look);
        let front = front_layer(look);
        for (layer, canvas) in [(GroundLayer::Back, &back), (GroundLayer::Front, &front)] {
            for night in [false, true] {
                let (_, top) = VillageGroundLook::layer_origin(layer, night);
                for y in 0..GROUND_STRIP_HEIGHT as i32 {
                    for x in 0..GROUND_STRIP_WIDTH as i32 {
                        let pixel = canvas.get(x, y);
                        if pixel.a > 0 {
                            sheet.set(x, top as i32 + y, if night { dim(pixel) } else { pixel });
                        }
                    }
                }
            }
        }
        sheet
    }
}

const INK: Rgba = Rgba::new(62, 48, 44, 255);
const EARTH: [Rgba; 3] = [
    Rgba::new(124, 94, 68, 255),
    Rgba::new(160, 124, 88, 255),
    Rgba::new(192, 156, 112, 255),
];
const GRASS: [Rgba; 3] = [
    Rgba::new(78, 124, 62, 255),
    Rgba::new(112, 160, 80, 255),
    Rgba::new(160, 196, 104, 255),
];
const PETALS: [Rgba; 2] = [Rgba::new(244, 132, 150, 255), Rgba::new(250, 214, 96, 255)];
/// What the further rows of grass fade toward, so the back of the ground reads as further off.
const AIR: Rgba = Rgba::new(236, 240, 236, 255);

/// How pale a house set this far back is drawn, from nothing at the front to a seventh of the way
/// to the air at the back.
pub fn depth_haze(depth: u8) -> f32 {
    f32::from(depth.min(formiga_core::MAX_HOUSE_DEPTH)) * 0.018
}

/// The colours the night half is drawn in: the day's, darker and bluer, as the houses' own dark
/// cells are.
fn dim(color: Rgba) -> Rgba {
    Rgba::new(
        (f32::from(color.r) * 0.55) as u8,
        (f32::from(color.g) * 0.58) as u8,
        (f32::from(color.b) * 0.72) as u8,
        color.a,
    )
}

pub(crate) fn mix(a: Rgba, b: Rgba, t: f32) -> Rgba {
    let blend = |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * t).round() as u8;
    Rgba::new(blend(a.r, b.r), blend(a.g, b.g), blend(a.b, b.b), a.a)
}

fn put(canvas: &mut Canvas, top: &Canvas, x: i32, y: i32) {
    for row in 0..top.height() as i32 {
        for column in 0..top.width() as i32 {
            let pixel = top.get(column, row);
            if pixel.a > 0 {
                let under = canvas.get(x + column, y + row);
                canvas.set(x + column, y + row, over(pixel, under));
            }
        }
    }
}

/// A little sprite from rows of letters — `d`, `m` and `l` the dark, middle and light tones of
/// `ramp`, `a` and `b` two accents — ringed in ink on every side but its foot.
fn sprite(rows: &[&str], ramp: [Rgba; 3], accents: [Rgba; 2]) -> Canvas {
    let height = rows.len() as u32;
    let width = rows.iter().map(|row| row.len()).max().unwrap_or(0) as u32;
    let mut fill = Canvas::new(width + 2, height + 1);
    for (y, row) in rows.iter().enumerate() {
        for (x, letter) in row.chars().enumerate() {
            let color = match letter {
                'd' => ramp[0],
                'm' => ramp[1],
                'l' => ramp[2],
                'a' => accents[0],
                'b' => accents[1],
                _ => continue,
            };
            fill.set(x as i32 + 1, y as i32 + 1, color);
        }
    }
    let mut out = Canvas::new(fill.width(), fill.height());
    for y in 0..fill.height() as i32 {
        for x in 0..fill.width() as i32 {
            if fill.get(x, y).a > 0 {
                out.set(x, y, fill.get(x, y));
            } else if [(-1, 0), (1, 0), (0, -1), (0, 1)]
                .iter()
                .any(|(dx, dy)| fill.get(x + dx, y + dy).a > 0)
            {
                out.set(x, y, INK);
            }
        }
    }
    out
}

/// Stand a sprite with its foot on row `ground`, centred on `x`.
fn stand(canvas: &mut Canvas, top: &Canvas, x: i32, ground: i32) {
    put(
        canvas,
        top,
        x - top.width() as i32 / 2,
        ground - top.height() as i32 + 1,
    );
}

/// The same sprite faded toward the air by how far back it stands.
fn hazed(top: &Canvas, depth: i32) -> Canvas {
    let t = (depth as f32 * 0.018).min(0.3);
    let mut out = top.clone();
    for y in 0..out.height() as i32 {
        for x in 0..out.width() as i32 {
            let pixel = out.get(x, y);
            if pixel.a > 0 {
                out.set(x, y, mix(pixel, AIR, t));
            }
        }
    }
    out
}

fn tuft(kind: u32) -> Canvas {
    let rows: &[&str] = match kind % 4 {
        0 => &["..l..", "m.l.m", ".mlm.", "dmmmd"],
        1 => &[".l.", "lm.", "dmm"],
        2 => &["l...l", ".m.m.", "dmlmd"],
        _ => &["..l", ".m.", "dm."],
    };
    sprite(rows, GRASS, [GRASS[0]; 2])
}

/// A taller clump with a flower or two in it, to stand in front of a house's foot.
fn flowers(kind: u32) -> Canvas {
    let rows: &[&str] = match kind % 2 {
        0 => &[
            ".a.....", "ala..b.", ".l..bab", ".l.l.b.", "m.lml.l", ".mlmlm.", "dmmmmmd",
        ],
        _ => &[
            "..b..a.", ".bab.l.", "..b..l.", "l.l.ll.", ".mlml.m", "dmmmmmd",
        ],
    };
    sprite(rows, GRASS, PETALS)
}

fn bush(kind: u32) -> Canvas {
    let rows: &[&str] = match kind % 3 {
        0 => &["..lll..", ".llmmm.", "lmmmmmd", "mmmmmdd", ".dmmdd."],
        1 => &[".ll..", "lmmm.", "mmmmd", ".mdd."],
        _ => &[
            "...ll...", "..lmmm..", ".lmmmmd.", "lmmmmmmd", "mmmmmddd", ".dmmmdd.",
        ],
    };
    sprite(rows, GRASS, [GRASS[0]; 2])
}

fn sapling() -> Canvas {
    sprite(
        &[
            ".lll.", "lmmmd", "mmmmd", ".mdd.", "..a..", "..a..", "..a..",
        ],
        GRASS,
        [EARTH[0], EARTH[0]],
    )
}

/// The gaps between neighbouring houses wide enough to stand something in, by their middles.
fn gaps(look: &VillageGroundLook) -> Vec<i32> {
    look.houses
        .windows(2)
        .filter_map(|pair| {
            let (left, right) = (pair[0].centre + pair[0].half, pair[1].centre - pair[1].half);
            (right - left >= 8).then_some((left + right) / 2)
        })
        .collect()
}

fn back_layer(look: &VillageGroundLook) -> Canvas {
    let mut canvas = Canvas::new(GROUND_STRIP_WIDTH, GROUND_STRIP_HEIGHT);
    let salt = (look.seed ^ (look.seed >> 32)) as u32;
    let length = i32::from(look.length);
    let back = FRONT - GROUND_BAND + 1;
    // Beyond the grass: small pale bushes and a sapling in the gaps between houses, half behind
    // the houses on either side.
    for (index, gap) in gaps(look).into_iter().enumerate() {
        let index = index as u32;
        let shift = (noise(gap, 0, salt) % 7) as i32 - 3;
        if (index + salt) % 3 == 2 {
            stand(
                &mut canvas,
                &hazed(&sapling(), GROUND_BAND + 6),
                gap + shift,
                back,
            );
        } else {
            let kind = noise(gap, 1, salt) % 3;
            stand(
                &mut canvas,
                &hazed(&bush(kind), GROUND_BAND + 4),
                gap + shift,
                back + 1,
            );
        }
    }
    // The grass: rounded off at both ends, paler toward the back, its back edge ragged with a
    // blade standing up here and there.
    for x in 0..length {
        let reach = (x.min(length - 1 - x) as f32 / 14.0).min(1.0);
        let height = (GROUND_BAND as f32 * (1.0 - (1.0 - reach) * (1.0 - reach))).round() as i32
            - i32::from(noise(x, 0, salt ^ 21).is_multiple_of(3));
        if height <= 0 {
            continue;
        }
        let top = FRONT - height + 1;
        for y in top..=FRONT {
            let further = (FRONT - y) as f32 / GROUND_BAND as f32;
            let base = if y == FRONT {
                GRASS[0]
            } else if noise(x, y, salt ^ 23).is_multiple_of(7) {
                GRASS[2]
            } else if noise(x, y, salt ^ 29).is_multiple_of(9) {
                GRASS[0]
            } else {
                GRASS[1]
            };
            canvas.set(x, y, mix(base, AIR, further * 0.35));
        }
        canvas.set(x, top, mix(GRASS[0], AIR, 0.3));
        if noise(x, 1, salt ^ 25).is_multiple_of(5) {
            canvas.set(x, top - 1, mix(GRASS[1], AIR, 0.3));
            if noise(x, 2, salt ^ 25).is_multiple_of(3) {
                canvas.set(x, top - 2, mix(GRASS[2], AIR, 0.3));
            }
        }
    }
    // A path trodden along the middle of the grass, from the first house to the last, in lengths.
    if let (Some(first), Some(last)) = (look.houses.first(), look.houses.last()) {
        let row = FRONT - 5;
        let (from, to) = (first.centre - first.half, last.centre + last.half);
        let mut x = from;
        while x < to {
            let length = 10 + (noise(x, 0, salt ^ 43) % 14) as i32;
            for column in x..(x + length).min(to) {
                let top = if noise(column, row, salt ^ 47).is_multiple_of(4) {
                    EARTH[2]
                } else {
                    EARTH[1]
                };
                canvas.set(column, row, mix(top, AIR, 0.15));
                canvas.set(column, row + 1, mix(EARTH[0], AIR, 0.12));
            }
            x += length + 3 + (noise(x, 1, salt ^ 43) % 4) as i32;
        }
    }
    // And a path in from the front of the grass to every door, narrowing as it goes back.
    for house in &look.houses {
        let rise = i32::from(house.depth) + 1;
        for row in 0..=rise {
            let y = FRONT - row;
            let half = 4 - (row * 2 / rise.max(1));
            let further = row as f32 / GROUND_BAND as f32;
            for x in house.centre - half..=house.centre + half {
                let edge = x == house.centre - half || x == house.centre + half;
                let base = if edge {
                    EARTH[0]
                } else if noise(x, y, salt ^ 41).is_multiple_of(5) {
                    EARTH[2]
                } else {
                    EARTH[1]
                };
                canvas.set(x, y, mix(base, AIR, further * 0.35));
            }
        }
    }
    canvas
}

fn front_layer(look: &VillageGroundLook) -> Canvas {
    let mut canvas = Canvas::new(GROUND_STRIP_WIDTH, GROUND_STRIP_HEIGHT);
    for (index, gap) in gaps(look).into_iter().enumerate() {
        stand(&mut canvas, &tuft(index as u32), gap - 4, FRONT);
    }
    // Grass and flowers half over the corners of the houses at the front, which is what puts them
    // in front of the rest.
    for house in look.houses.iter().filter(|house| house.depth <= 2) {
        let slot = house.slot as u32;
        let clump = if slot.is_multiple_of(2) {
            flowers(slot / 2)
        } else {
            tuft(slot + 1)
        };
        stand(&mut canvas, &clump, house.centre - house.half + 3, FRONT);
        stand(
            &mut canvas,
            &tuft(slot + 2),
            house.centre + house.half - 2,
            FRONT,
        );
    }
    canvas
}

#[cfg(test)]
mod tests {
    use super::*;

    fn look(depths: &[u8]) -> VillageGroundLook {
        VillageGroundLook {
            length: 420,
            houses: depths
                .iter()
                .enumerate()
                .map(|(slot, depth)| GroundHouse {
                    slot,
                    centre: 90 + slot as i32 * 66,
                    half: 26,
                    depth: *depth,
                })
                .collect(),
            trees: vec![28, 392],
            seed: 7,
        }
    }

    #[test]
    fn the_ground_stays_inside_its_layers_and_the_front_of_the_grass_is_its_foot() {
        let sheet = VillageGroundRenderer::render(&look(&[0, 7, 2, 8]));
        for layer in [GroundLayer::Back, GroundLayer::Front] {
            for night in [false, true] {
                let (_, top) = VillageGroundLook::layer_origin(layer, night);
                let bottom = top as i32 + FRONT;
                assert!(
                    (0..420).any(|x| sheet.get(x, bottom).a > 0),
                    "{layer:?} reaches its foot"
                );
            }
        }
        // Nothing beyond the village's own length.
        for y in 0..GROUND_ATLAS_HEIGHT as i32 {
            for x in 440..GROUND_STRIP_WIDTH as i32 {
                assert_eq!(sheet.get(x, y).a, 0, "ground drawn at {x},{y}");
            }
        }
    }

    #[test]
    fn a_house_further_back_has_a_longer_path_to_its_door() {
        let path_rows = |depth: u8| {
            let canvas = back_layer(&look(&[depth]));
            (0..GROUND_STRIP_HEIGHT as i32)
                .filter(|y| {
                    let pixel = canvas.get(90, *y);
                    pixel.a > 0 && pixel.r > pixel.g
                })
                .count()
        };
        assert!(path_rows(8) > path_rows(0));
    }

    #[test]
    fn the_same_village_draws_the_same_ground() {
        let one = VillageGroundRenderer::render(&look(&[0, 7, 2]));
        let two = VillageGroundRenderer::render(&look(&[0, 7, 2]));
        assert_eq!(one, two);
        assert_ne!(one, VillageGroundRenderer::render(&look(&[0, 3, 2])));
    }
}
