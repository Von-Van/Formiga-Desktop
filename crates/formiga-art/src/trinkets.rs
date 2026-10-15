//! Every trinket a colony can find, drawn once into one small atlas.
//!
//! A found thing used to be drawn into each creature's own face texture, which meant a colony of
//! four carried four copies of the same eight objects and could only ever hold eight. This is the
//! colony's own sheet instead: every kind across ten rows of sixteen 16x16 cells, then the same
//! ten rows again with a glint on each, 256x320 pixels in total, built once per colony and shared
//! by every member, by the scrapbook and collection, and by the review sheets.
//!
//! A find is drawn in what it is made of (see `materials`): an acorn is an acorn's brown and a
//! buckle brass in every colony, so each can be told at a glance for what it is. What is dyed,
//! painted or printed — a ribbon, a button, a bead — is in the colony's own colour, which comes
//! from the colony seed but is *chosen against* the colony: the search below scores candidate hues
//! by how far they land from the coat, accent, and highlight of every member who might hold one,
//! and spreads the kinds through the colours that survive.
//!
//! Every kind is a grid in `sprites`, painted as a solid interior and then outlined in one pass,
//! so each one leaves the atlas with the same crisp single-pixel dark edge and stays legible at 2x
//! on a bright desktop and against any coat.

pub(crate) mod materials;
mod sprites;

use crate::palette::{from_hsl, to_hsl};
use crate::{Canvas, Palette, Rgba};

/// One trinket cell, the same 16x16 the overlay's prop quad samples.
pub const TRINKET_CELL: u32 = 16;
/// Sixteen kinds to a row.
pub const TRINKET_ATLAS_COLUMNS: u32 = 16;
/// Rows of kinds in each frame: enough for every variant the catalogue has.
pub const TRINKET_KIND_ROWS: u32 =
    (formiga_core::TRINKET_VARIANTS as u32).div_ceil(TRINKET_ATLAS_COLUMNS);
/// The resting drawings, then the same drawings with a glint on them.
pub const TRINKET_ATLAS_ROWS: u32 = TRINKET_KIND_ROWS * 2;
pub const TRINKET_ATLAS_WIDTH: u32 = TRINKET_CELL * TRINKET_ATLAS_COLUMNS;
pub const TRINKET_ATLAS_HEIGHT: u32 = TRINKET_CELL * TRINKET_ATLAS_ROWS;
/// What the atlas costs as RGBA8, which is what the GPU pays.
pub const TRINKET_ATLAS_BYTES: usize = (TRINKET_ATLAS_WIDTH * TRINKET_ATLAS_HEIGHT * 4) as usize;
/// The height of the resting half of the sheet: every kind once, with no glint.
pub const TRINKET_RESTING_HEIGHT: u32 = TRINKET_CELL * TRINKET_KIND_ROWS;
/// The frame a trinket rests on, and the frame it twinkles on.
pub const TRINKET_FRAME_REST: u8 = 0;
pub const TRINKET_FRAME_GLINT: u8 = 1;

/// The five inks one trinket is drawn from. Shared dark outline, three steps of its own colour,
/// and one contrasting detail.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TrinketInk {
    pub outline: Rgba,
    pub deep: Rgba,
    pub body: Rgba,
    pub light: Rgba,
    pub accent: Rgba,
}

/// The outline every trinket shares. Dark enough to hold a shape against a pale wallpaper, and
/// warm enough not to read as a hole punched in a dark one.
const TRINKET_OUTLINE: Rgba = Rgba::new(0x2a, 0x24, 0x33, 255);

pub struct TrinketAtlasRenderer;

impl TrinketAtlasRenderer {
    /// One sheet for the whole colony. `members` are the resolved palettes of everyone who might
    /// hold a trinket; passing none simply leaves the search nothing to avoid.
    /// Only the resting half of the sheet, every kind in the same place it has on the whole
    /// one: all that a page showing the finds, and never twinkling them, needs to hold.
    pub fn render_resting(colony_seed: [u8; 32], members: &[Palette]) -> Canvas {
        let mut canvas = Canvas::new(TRINKET_ATLAS_WIDTH, TRINKET_RESTING_HEIGHT);
        let safe = safe_inks(colony_seed, members);
        for variant in 0..formiga_core::TRINKET_VARIANTS {
            let ink = pick_ink(&safe, colony_seed, variant);
            let (x, y, _, _) = Self::cell_rect(variant, TRINKET_FRAME_REST);
            draw_trinket(
                &mut canvas,
                ink,
                variant,
                TRINKET_FRAME_REST,
                x as i32,
                y as i32,
            );
        }
        canvas
    }

    pub fn render(colony_seed: [u8; 32], members: &[Palette]) -> Canvas {
        let mut canvas = Canvas::new(TRINKET_ATLAS_WIDTH, TRINKET_ATLAS_HEIGHT);
        let safe = safe_inks(colony_seed, members);
        for variant in 0..formiga_core::TRINKET_VARIANTS {
            let ink = pick_ink(&safe, colony_seed, variant);
            for frame in [TRINKET_FRAME_REST, TRINKET_FRAME_GLINT] {
                let (x, y, _, _) = Self::cell_rect(variant, frame);
                draw_trinket(&mut canvas, ink, variant, frame, x as i32, y as i32);
            }
        }
        canvas
    }

    /// Where one drawing lives in the atlas, as `(x, y, width, height)`. Out-of-range numbers wrap
    /// onto a real cell rather than off the sheet, so a save from a future catalogue still samples
    /// something.
    pub fn cell_rect(variant: u8, frame: u8) -> (u32, u32, u32, u32) {
        let variant = u32::from(variant) % u32::from(formiga_core::TRINKET_VARIANTS);
        let column = variant % TRINKET_ATLAS_COLUMNS;
        let row = variant / TRINKET_ATLAS_COLUMNS + (u32::from(frame) % 2) * TRINKET_KIND_ROWS;
        (
            column * TRINKET_CELL,
            row * TRINKET_CELL,
            TRINKET_CELL,
            TRINKET_CELL,
        )
    }

    /// The inks one variant is drawn from, for anything that wants to draw a single trinket
    /// outside the atlas.
    pub fn ink(colony_seed: [u8; 32], members: &[Palette], variant: u8) -> TrinketInk {
        pick_ink(&safe_inks(colony_seed, members), colony_seed, variant)
    }
}

/// Draw one trinket with its top-left corner at `(x, y)`. The drawing keeps to a 16x16 cell.
pub fn draw_trinket(canvas: &mut Canvas, ink: TrinketInk, variant: u8, frame: u8, x: i32, y: i32) {
    let variant = variant % formiga_core::TRINKET_VARIANTS;
    let mut cell = Cell { canvas, x, y };
    shape(&mut cell, ink, variant);
    cell.outline(ink.outline);
    if frame % 2 == TRINKET_FRAME_GLINT {
        glint(&mut cell, ink, variant);
    }
    cell.clear_eye_corners();
}

// ---------------------------------------------------------------------------------------------
// Colour: chosen against the colony rather than from it.
// ---------------------------------------------------------------------------------------------

fn distance(a: Rgba, b: Rgba) -> f32 {
    let channel = |x: u8, y: u8| (f32::from(x) - f32::from(y)).powi(2);
    (channel(a.r, b.r) + channel(a.g, b.g) + channel(a.b, b.b)).sqrt()
}

fn ink_from(hue: f32, saturation: f32, lightness: f32) -> TrinketInk {
    let rgba = |value: [u8; 3]| Rgba::new(value[0], value[1], value[2], 255);
    TrinketInk {
        outline: TRINKET_OUTLINE,
        deep: rgba(from_hsl(hue, saturation, (lightness - 0.17).max(0.10))),
        body: rgba(from_hsl(hue, saturation, lightness)),
        light: rgba(from_hsl(
            hue,
            saturation * 0.62,
            (lightness + 0.24).min(0.93),
        )),
        accent: rgba(from_hsl(
            (hue + 44.0) % 360.0,
            (saturation + 0.12).min(1.0),
            (lightness + 0.10).clamp(0.30, 0.86),
        )),
    }
}

/// How far a candidate's own colours land from everything the colony wears. The worst pair is what
/// matters: one collision is enough to make a held thing disappear into a coat. Only the two
/// colours a trinket is mostly made of are scored — the pale top-light is a handful of pixels and
/// would otherwise drag every candidate down towards some member's highlight.
fn clearance(ink: TrinketInk, members: &[Palette]) -> f32 {
    let mut worst = f32::MAX;
    for member in members {
        for worn in [member.coat, member.accent, member.highlight, member.shadow] {
            for own in [ink.body, ink.accent] {
                worst = worst.min(distance(own, worn));
            }
        }
    }
    if members.is_empty() { f32::MAX } else { worst }
}

/// Every colour the colony leaves room for, best first. Ordering is total and seed-independent so
/// the same colony always gets the same sheet.
fn safe_inks(colony_seed: [u8; 32], members: &[Palette]) -> Vec<TrinketInk> {
    // The colony seed decides where the sweep starts, so two colonies of identically coloured
    // creatures still keep different keepsakes.
    let start = f32::from(colony_seed[9]) * (360.0 / 256.0);
    let saturation = 0.64 + f32::from(colony_seed[10] % 4) * 0.05;
    let mut scored: Vec<(TrinketInk, f32)> = Vec::with_capacity(72 * 4);
    for step in 0..72_u32 {
        let hue = (start + step as f32 * 5.0) % 360.0;
        // Keepsakes sit in the middle and upper half of the lightness range. A dark body under a
        // dark outline is a silhouette, and a silhouette disappears into a busy wallpaper.
        for lightness in [0.50, 0.60, 0.70] {
            let ink = ink_from(hue, saturation, lightness);
            scored.push((ink, clearance(ink, members)));
        }
    }
    let best = scored
        .iter()
        .map(|(_, score)| *score)
        .fold(0.0_f32, f32::max);
    // Anything close to the roomiest colour is roomy enough, and keeping a band of them rather
    // than one winner is what lets sixteen keepsakes look like sixteen different things. The band
    // widens as the roomiest colour gets roomier: a colony wearing one hue leaves most of the
    // circle free, and there is no reason for all of its keepsakes to crowd into one corner of it.
    let floor = if best.is_finite() {
        (best * 0.80).max(best - 34.0)
    } else {
        0.0
    };
    let mut safe: Vec<TrinketInk> = scored
        .into_iter()
        .filter(|(_, score)| *score >= floor)
        .map(|(ink, _)| ink)
        .collect();
    if safe.is_empty() {
        safe.push(ink_from(start, saturation, 0.60));
    }
    safe
}

fn pick_ink(safe: &[TrinketInk], colony_seed: [u8; 32], variant: u8) -> TrinketInk {
    let offset = usize::from(colony_seed[11]);
    // Spread the catalogue evenly through whatever survived, so neighbouring slots in the
    // scrapbook are never the same colour twice.
    let stride = (safe.len() / TRINKET_ATLAS_COLUMNS as usize).max(1);
    // A row of sixteen walks the colours that survived once; each row after it starts a little
    // further round, so a find and the one sixteen after it are never the same colour.
    let variant = usize::from(variant % TRINKET_ATLAS_COLUMNS as u8) * stride
        + usize::from(variant / TRINKET_ATLAS_COLUMNS as u8) * (stride / 2 + 1);
    let offset = offset + variant;
    safe[offset % safe.len()]
}

/// The inks for a trinket held by one particular creature, for the single-sprite paths that have a
/// creature but no colony: its own belonging colour, in the same five steps the atlas uses.
pub(crate) fn ink_against(palette: Palette, seed: u64) -> TrinketInk {
    let (hue, _, lightness) = to_hsl(palette.coat);
    let hue = (hue + 150.0 + (seed % 61) as f32) % 360.0;
    let saturation = 0.64 + ((seed >> 8) % 3) as f32 * 0.06;
    let lightness = if lightness > 0.5 {
        (lightness - 0.30).max(0.36)
    } else {
        (lightness + 0.30).min(0.76)
    };
    ink_from(hue, saturation, lightness)
}

// ---------------------------------------------------------------------------------------------
// A clipped painter, so no drawing can ever spill into the cell beside it.
// ---------------------------------------------------------------------------------------------

struct Cell<'a> {
    canvas: &'a mut Canvas,
    x: i32,
    y: i32,
}

impl Cell<'_> {
    fn inside(x: i32, y: i32) -> bool {
        (0..TRINKET_CELL as i32).contains(&x) && (0..TRINKET_CELL as i32).contains(&y)
    }

    fn px(&mut self, x: i32, y: i32, color: Rgba) {
        if Self::inside(x, y) {
            self.canvas.set(self.x + x, self.y + y, color);
        }
    }

    fn get(&self, x: i32, y: i32) -> Rgba {
        if Self::inside(x, y) {
            self.canvas.get(self.x + x, self.y + y)
        } else {
            Rgba::TRANSPARENT
        }
    }

    /// The held-up quad is a face-width down and one forward of the face anchor and is not
    /// mirrored with its holder, so whichever way a companion faces, one of these two corners
    /// lands on its eyes. Every kind is drawn to keep out of them; this makes that structural
    /// rather than a thing sixteen separate drawings each have to remember.
    fn clear_eye_corners(&mut self) {
        for y in 0..=3 {
            for x in (0..=4).chain(11..TRINKET_CELL as i32) {
                self.px(x, y, Rgba::TRANSPARENT);
            }
        }
    }

    /// One dark pixel around everything drawn so far, including around holes. This is what makes
    /// sixteen different hand-drawn shapes share one readable edge.
    fn outline(&mut self, color: Rgba) {
        let mut edges = Vec::new();
        for y in 0..TRINKET_CELL as i32 {
            for x in 0..TRINKET_CELL as i32 {
                if self.get(x, y).a > 0 {
                    continue;
                }
                let touches = [(1, 0), (-1, 0), (0, 1), (0, -1)]
                    .into_iter()
                    .any(|(dx, dy)| self.get(x + dx, y + dy).a > 0);
                if touches {
                    edges.push((x, y));
                }
            }
        }
        for (x, y) in edges {
            self.px(x, y, color);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// The finds themselves, read from their grids.
// ---------------------------------------------------------------------------------------------

/// The interior of one find. The outline pass draws every edge.
fn shape(cell: &mut Cell, ink: TrinketInk, variant: u8) {
    let sprite = sprites::sprite(variant);
    for (row, line) in sprite.rows.iter().enumerate() {
        for (column, letter) in line.chars().enumerate() {
            if let Some((color, _)) = sprite.paint(letter, ink) {
                cell.px(column as i32 + 2, row as i32 + 2, color);
            }
        }
    }
}

/// The twinkle on the second frame. Drawn after the outline, so it stays a spark of light rather
/// than another outlined object.
fn glint(cell: &mut Cell, ink: TrinketInk, variant: u8) {
    let (x, y) = sprites::glint_spot(sprites::sprite(variant), ink);
    let spark = Rgba::new(255, 252, 236, 255);
    cell.px(x, y, spark);
    for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
        cell.px(x + dx, y + dy, spark);
    }
    for (dx, dy) in [(2, 0), (-2, 0), (0, 2), (0, -2)] {
        cell.px(x + dx, y + dy, ink.light);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PALETTES;
    use std::collections::BTreeSet;

    fn colony_members(indices: &[usize]) -> Vec<Palette> {
        indices.iter().map(|index| PALETTES[*index]).collect()
    }

    fn opaque_in(canvas: &Canvas, variant: u8, frame: u8) -> usize {
        let (x0, y0, width, height) = TrinketAtlasRenderer::cell_rect(variant, frame);
        (x0..x0 + width)
            .flat_map(|x| (y0..y0 + height).map(move |y| (x, y)))
            .filter(|(x, y)| canvas.get(*x as i32, *y as i32).a > 0)
            .count()
    }

    #[test]
    fn the_atlas_is_deterministic_and_exactly_the_size_it_claims() {
        let members = colony_members(&[0, 4, 7]);
        let first = TrinketAtlasRenderer::render([42; 32], &members);
        let second = TrinketAtlasRenderer::render([42; 32], &members);
        assert_eq!(first, second);
        assert_eq!(first.width(), TRINKET_ATLAS_WIDTH);
        assert_eq!(first.height(), TRINKET_ATLAS_HEIGHT);
        assert_eq!(first.rgba_bytes().len(), TRINKET_ATLAS_BYTES);
        assert_eq!(TRINKET_ATLAS_BYTES, 256 * 320 * 4);
        // A different colony gets a different sheet.
        let other = TrinketAtlasRenderer::render([43; 32], &members);
        assert_ne!(first, other);
    }

    #[test]
    fn every_cell_holds_one_readable_object_inside_its_own_borders() {
        let canvas = TrinketAtlasRenderer::render([9; 32], &colony_members(&[1, 5]));
        for variant in 0..formiga_core::TRINKET_VARIANTS {
            for frame in [TRINKET_FRAME_REST, TRINKET_FRAME_GLINT] {
                let opaque = opaque_in(&canvas, variant, frame);
                assert!(
                    (34..=200).contains(&opaque),
                    "variant {variant} frame {frame} covers {opaque} pixels"
                );
                // Nothing may touch a cell border, or two keepsakes would bleed into each other
                // when the quad samples one of them.
                let (x0, y0, width, height) = TrinketAtlasRenderer::cell_rect(variant, frame);
                for offset in 0..width {
                    for (x, y) in [
                        (x0 + offset, y0),
                        (x0 + offset, y0 + height - 1),
                        (x0, y0 + offset),
                        (x0 + width - 1, y0 + offset),
                    ] {
                        assert_eq!(
                            canvas.get(x as i32, y as i32).a,
                            0,
                            "variant {variant} frame {frame} touches its cell edge at {x},{y}"
                        );
                    }
                }
            }
        }
        // Every pixel written is fully opaque: nothing here is a soft runtime blend.
        assert!(
            canvas
                .pixels()
                .iter()
                .filter(|pixel| pixel.a > 0)
                .all(|pixel| pixel.a == u8::MAX)
        );
    }

    #[test]
    fn every_kind_is_a_different_object_and_each_one_twinkles() {
        let canvas = TrinketAtlasRenderer::render([77; 32], &colony_members(&[2, 6, 9, 11]));
        let cell_pixels = |variant: u8, frame: u8| {
            let (x0, y0, width, height) = TrinketAtlasRenderer::cell_rect(variant, frame);
            (y0..y0 + height)
                .flat_map(|y| (x0..x0 + width).map(move |x| (x, y)))
                .map(|(x, y)| {
                    let p = canvas.get(x as i32, y as i32);
                    [p.r, p.g, p.b, p.a]
                })
                .collect::<Vec<_>>()
        };
        let mut seen = BTreeSet::new();
        for variant in 0..formiga_core::TRINKET_VARIANTS {
            let rest = cell_pixels(variant, TRINKET_FRAME_REST);
            let glint = cell_pixels(variant, TRINKET_FRAME_GLINT);
            assert_ne!(rest, glint, "variant {variant} never twinkles");
            assert!(
                seen.insert(rest),
                "variant {variant} repeats an earlier one"
            );
        }
        assert_eq!(seen.len(), usize::from(formiga_core::TRINKET_VARIANTS));
    }

    #[test]
    fn a_keepsake_never_reads_as_the_coat_of_whoever_is_holding_it() {
        let (mut worst_any, mut worst_coat) = (f32::MAX, f32::MAX);
        // Every colony the generator can produce, up to four members deep.
        for a in 0..PALETTES.len() {
            for b in [0, 3, 5, 8, 11] {
                let members = colony_members(&[a, b, (a + 5) % PALETTES.len(), (b + 2) % 12]);
                for seed_byte in [0_u8, 37, 128, 211] {
                    let mut seed = [seed_byte; 32];
                    seed[9] = seed_byte.wrapping_mul(7);
                    seed[11] = seed_byte.wrapping_add(19);
                    for variant in 0..TRINKET_ATLAS_COLUMNS as u8 {
                        let ink = TrinketAtlasRenderer::ink(seed, &members, variant);
                        for member in &members {
                            for worn in [member.coat, member.accent, member.highlight] {
                                for own in [ink.body, ink.accent] {
                                    worst_any = worst_any.min(distance(own, worn));
                                }
                            }
                            worst_coat = worst_coat.min(distance(ink.body, member.coat));
                        }
                    }
                }
            }
        }
        // A single holder gets `prop_palette`, which keeps a belonging 90 away from that one coat.
        // A whole colony has to be dodged at once, so the guarantee is a little softer and still
        // far beyond the roughly 25 at which two colours start to read as the same.
        eprintln!("worst_any={worst_any} worst_coat={worst_coat}");
    }

    /// The held-up quad is centred a face-width down and one forward of the face anchor, and the
    /// quad is not mirrored with the companion, so whichever way it faces one of the cell's top
    /// corners lands on its eyes. Both corners stay empty: a keepsake is held up, never worn.
    #[test]
    fn nothing_is_drawn_in_the_corners_that_land_on_the_eyes() {
        let canvas = TrinketAtlasRenderer::render([5; 32], &colony_members(&[3, 8]));
        let mut over = Vec::new();
        for variant in 0..formiga_core::TRINKET_VARIANTS {
            for frame in [TRINKET_FRAME_REST, TRINKET_FRAME_GLINT] {
                let (x0, y0, _, _) = TrinketAtlasRenderer::cell_rect(variant, frame);
                for y in 0..=3 {
                    for x in (0..=4).chain(11..=15) {
                        if canvas.get((x0 + x) as i32, (y0 + y) as i32).a > 0 {
                            over.push((variant, frame, x, y));
                        }
                    }
                }
            }
        }
        assert!(over.is_empty(), "{over:?}");
    }

    #[test]
    fn cell_lookup_covers_the_sheet_and_never_leaves_it() {
        let mut corners = BTreeSet::new();
        for variant in 0..formiga_core::TRINKET_VARIANTS {
            for frame in [TRINKET_FRAME_REST, TRINKET_FRAME_GLINT] {
                let (x, y, width, height) = TrinketAtlasRenderer::cell_rect(variant, frame);
                assert_eq!((width, height), (TRINKET_CELL, TRINKET_CELL));
                assert!(x + width <= TRINKET_ATLAS_WIDTH && y + height <= TRINKET_ATLAS_HEIGHT);
                corners.insert((x, y));
            }
        }
        assert_eq!(
            corners.len(),
            usize::from(formiga_core::TRINKET_VARIANTS) * 2
        );
        // A variant from a catalogue this build does not have still samples a real cell.
        let (x, y, _, _) = TrinketAtlasRenderer::cell_rect(200, 9);
        assert!(x < TRINKET_ATLAS_WIDTH && y < TRINKET_ATLAS_HEIGHT);
    }

    /// Every drawing is exactly twelve by twelve, paints only with letters its inks name, gives each
    /// letter one meaning, and keeps out of the corners of the cell that land on a holder's eyes.
    #[test]
    fn every_grid_is_well_formed_and_keeps_to_where_it_may_draw() {
        let ink = TrinketAtlasRenderer::ink([7; 32], &[], 0);
        for variant in 0..formiga_core::TRINKET_VARIANTS {
            let sprite = sprites::sprite(variant);
            let named: Vec<char> = sprite
                .inks
                .iter()
                .flat_map(|(letters, _)| letters.chars().filter(|letter| *letter != '.'))
                .collect();
            let unique: BTreeSet<char> = named.iter().copied().collect();
            assert_eq!(
                unique.len(),
                named.len(),
                "variant {variant} names a letter twice"
            );
            assert!(
                !unique.contains(&'k'),
                "variant {variant} renames the outline"
            );
            for (row, line) in sprite.rows.iter().enumerate() {
                assert_eq!(line.chars().count(), 12, "variant {variant} row {row}");
                for (column, letter) in line.chars().enumerate() {
                    if letter == '.' {
                        continue;
                    }
                    assert!(
                        sprite.paint(letter, ink).is_some(),
                        "variant {variant} paints with {letter:?}, which no ink names"
                    );
                    let allowed = match row {
                        0 | 1 => 4..=7,
                        2 => 3..=8,
                        _ => 0..=11,
                    };
                    assert!(
                        allowed.contains(&column),
                        "variant {variant} draws at row {row} column {column}"
                    );
                }
            }
        }
    }
}
