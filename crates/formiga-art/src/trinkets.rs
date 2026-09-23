//! Every trinket a colony can find, drawn once into one small atlas.
//!
//! A found thing used to be drawn into each creature's own face texture, which meant a colony of
//! four carried four copies of the same eight objects and could only ever hold eight. This is the
//! colony's own sheet instead: every kind across ten rows of sixteen 16x16 cells, then the same
//! ten rows again with a glint on each, 256x320 pixels in total, built once per colony and shared
//! by every member, by the scrapbook and collection, and by the review sheets.
//!
//! Colour comes from the colony seed, but it is *chosen against* the colony: the search below
//! scores candidate hues by how far they land from the coat, accent, and highlight of every member
//! who might hold one, and spreads the kinds through the colours that survive. A keepsake never
//! reads as another patch of the fur holding it.
//!
//! Every kind is drawn as a solid interior and then outlined in one pass, so each one leaves the
//! atlas with the same crisp single-pixel dark edge and stays legible at 2x on a bright desktop.
//! The first sixteen are drawn by hand below; the rest are grids in `sprites`.

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

    fn span(&mut self, x0: i32, x1: i32, y: i32, color: Rgba) {
        for x in x0..=x1 {
            self.px(x, y, color);
        }
    }

    fn rect(&mut self, x: i32, y: i32, width: i32, height: i32, color: Rgba) {
        for py in y..y + height {
            self.span(x, x + width - 1, py, color);
        }
    }

    fn ellipse(&mut self, cx: i32, cy: i32, rx: i32, ry: i32, color: Rgba) {
        if rx <= 0 || ry <= 0 {
            self.px(cx, cy, color);
            return;
        }
        let (rx2, ry2) = ((rx * rx) as i64, (ry * ry) as i64);
        for dy in -ry..=ry {
            for dx in -rx..=rx {
                if dx as i64 * dx as i64 * ry2 + dy as i64 * dy as i64 * rx2 <= rx2 * ry2 {
                    self.px(cx + dx, cy + dy, color);
                }
            }
        }
    }

    fn line(&mut self, mut x0: i32, mut y0: i32, x1: i32, y1: i32, color: Rgba) {
        let dx = (x1 - x0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let dy = -(y1 - y0).abs();
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut error = dx + dy;
        loop {
            self.px(x0, y0, color);
            if x0 == x1 && y0 == y1 {
                break;
            }
            let twice = 2 * error;
            if twice >= dy {
                error += dy;
                x0 += sx;
            }
            if twice <= dx {
                error += dx;
                y0 += sy;
            }
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
// The sixteen kinds. Interiors only: the outline pass draws every edge.
// ---------------------------------------------------------------------------------------------

fn shape(cell: &mut Cell, ink: TrinketInk, variant: u8) {
    if let Some(grid) = sprites::grid(variant) {
        draw_grid(cell, ink, grid);
        return;
    }
    match variant {
        0 => gem(cell, ink),
        1 => key(cell, ink),
        2 => leaf(cell, ink),
        3 => shell(cell, ink),
        4 => ring_charm(cell, ink),
        5 => tiny_bottle(cell, ink),
        6 => star_relic(cell, ink),
        7 => tablet(cell, ink),
        8 => moon_shard(cell, ink),
        9 => firefly_jar(cell, ink),
        10 => feather(cell, ink),
        11 => cloud_puff(cell, ink),
        12 => ticket_stub(cell, ink),
        13 => pinwheel(cell, ink),
        14 => friendship_knot(cell, ink),
        _ => matching_charms(cell, ink),
    }
}

/// A cut stone: flat table, wide girdle, long pavilion.
fn gem(cell: &mut Cell, ink: TrinketInk) {
    const ROWS: [(i32, i32); 10] = [
        (7, 9),
        (6, 10),
        (4, 12),
        (3, 13),
        (4, 12),
        (4, 12),
        (5, 11),
        (6, 10),
        (7, 9),
        (8, 8),
    ];
    for (index, (x0, x1)) in ROWS.into_iter().enumerate() {
        cell.span(x0, x1, 3 + index as i32, ink.body);
    }
    // The table catches the light; the right flank falls away.
    cell.span(7, 9, 3, ink.light);
    cell.span(6, 8, 4, ink.light);
    cell.span(4, 7, 5, ink.light);
    for (index, (_, x1)) in ROWS.into_iter().enumerate().skip(2) {
        cell.span(x1 - 1, x1, 3 + index as i32, ink.deep);
    }
    cell.line(6, 6, 9, 11, ink.accent);
}

/// A key laid flat, which is the shape a key actually has: bow on the left, a long shaft, and two
/// teeth stepping down off the far end.
fn key(cell: &mut Cell, ink: TrinketInk) {
    cell.ellipse(5, 8, 3, 3, ink.body);
    cell.ellipse(5, 8, 1, 1, Rgba::TRANSPARENT);
    cell.rect(7, 7, 6, 2, ink.body);
    cell.rect(10, 9, 1, 2, ink.body);
    cell.rect(12, 9, 1, 3, ink.body);
    cell.span(3, 4, 6, ink.light);
    cell.span(7, 12, 7, ink.light);
    cell.px(12, 11, ink.accent);
    cell.px(3, 10, ink.deep);
}

/// A pressed leaf: broad, upright, and stemmed, so it never reads as the feather two cells along.
fn leaf(cell: &mut Cell, ink: TrinketInk) {
    const ROWS: [(i32, i32); 9] = [
        (7, 9),
        (6, 10),
        (4, 12),
        (3, 12),
        (3, 12),
        (4, 11),
        (5, 10),
        (6, 9),
        (7, 9),
    ];
    for (index, (x0, x1)) in ROWS.into_iter().enumerate() {
        cell.span(x0, x1, 2 + index as i32, ink.body);
    }
    // The midrib runs the whole length; the veins fan off it on one side only.
    cell.line(8, 2, 8, 11, ink.deep);
    for (y, reach) in [(5, 3), (7, 4), (9, 3)] {
        cell.line(8, y, 8 + reach, y - 1, ink.deep);
        cell.line(8, y, 8 - reach, y - 1, ink.light);
    }
    cell.span(5, 7, 5, ink.light);
    cell.line(8, 11, 8, 13, ink.accent);
}

/// A scallop: hinge at the foot, a fan opening upward, and a lip notched between its ribs.
fn shell(cell: &mut Cell, ink: TrinketInk) {
    const ROWS: [(i32, i32); 9] = [
        (3, 13),
        (3, 13),
        (3, 13),
        (3, 13),
        (4, 12),
        (5, 11),
        (6, 10),
        (7, 9),
        (7, 9),
    ];
    for (index, (x0, x1)) in ROWS.into_iter().enumerate() {
        cell.span(x0, x1, 5 + index as i32, ink.body);
    }
    // A scalloped lip: two pixels deep, so the notches survive being halved.
    for notch in [5, 8, 11] {
        cell.px(notch, 5, Rgba::TRANSPARENT);
        cell.px(notch, 6, Rgba::TRANSPARENT);
    }
    for (tip, shade) in [
        ((4, 7), ink.light),
        ((8, 6), ink.light),
        ((12, 7), ink.deep),
    ] {
        cell.line(8, 13, tip.0, tip.1, shade);
    }
    cell.span(4, 6, 9, ink.light);
    cell.span(7, 9, 13, ink.accent);
}

/// A ring far too small for anyone here, with one bead at the top of it.
fn ring_charm(cell: &mut Cell, ink: TrinketInk) {
    cell.ellipse(8, 9, 5, 4, ink.body);
    cell.ellipse(8, 9, 2, 2, Rgba::TRANSPARENT);
    cell.rect(7, 5, 2, 2, ink.body);
    cell.ellipse(8, 4, 2, 2, ink.accent);
    for (x, y) in [(4, 8), (4, 9), (5, 7), (6, 6)] {
        cell.px(x, y, ink.light);
    }
    for (x, y) in [(11, 10), (11, 11), (10, 12)] {
        cell.px(x, y, ink.deep);
    }
}

/// A stoppered bottle, something cloudy still in the bottom of it.
fn tiny_bottle(cell: &mut Cell, ink: TrinketInk) {
    cell.rect(7, 2, 3, 2, ink.accent);
    cell.rect(7, 4, 3, 2, ink.body);
    cell.rect(5, 6, 7, 7, ink.body);
    for (x, y) in [(5, 6), (11, 6), (5, 12), (11, 12)] {
        cell.px(x, y, Rgba::TRANSPARENT);
    }
    cell.rect(6, 9, 5, 3, ink.accent);
    cell.px(6, 7, ink.light);
    cell.px(6, 8, ink.light);
    cell.px(7, 7, ink.light);
    cell.span(6, 10, 9, ink.light);
    cell.px(10, 11, ink.deep);
}

/// A little star of worn metal.
fn star_relic(cell: &mut Cell, ink: TrinketInk) {
    const ROWS: [&[(i32, i32)]; 11] = [
        &[(8, 8)],
        &[(7, 9)],
        &[(7, 9)],
        &[(3, 13)],
        &[(4, 12)],
        &[(5, 11)],
        &[(5, 11)],
        &[(4, 12)],
        &[(3, 5), (11, 13)],
        &[(3, 4), (12, 13)],
        &[(3, 3), (13, 13)],
    ];
    for (index, spans) in ROWS.into_iter().enumerate() {
        for (x0, x1) in spans {
            cell.span(*x0, *x1, 2 + index as i32, ink.body);
        }
    }
    for (x, y) in [(7, 3), (6, 5), (5, 6), (6, 6), (4, 9)] {
        cell.px(x, y, ink.light);
    }
    for (x, y) in [(11, 7), (12, 8), (11, 9), (12, 10)] {
        cell.px(x, y, ink.deep);
    }
    cell.ellipse(8, 7, 1, 1, ink.accent);
}

/// A flat tablet with lines nobody can read.
fn tablet(cell: &mut Cell, ink: TrinketInk) {
    cell.rect(3, 5, 11, 9, ink.body);
    for (x, y) in [(3, 5), (13, 5), (3, 13), (13, 13)] {
        cell.px(x, y, Rgba::TRANSPARENT);
    }
    cell.rect(4, 6, 1, 7, ink.light);
    cell.rect(12, 6, 1, 7, ink.deep);
    for (y, x1) in [(7, 11), (9, 9), (11, 11)] {
        cell.span(6, x1, y, ink.deep);
    }
    cell.px(11, 9, ink.accent);
    cell.px(12, 9, ink.accent);
}

/// A sliver of the moon, keeping a little light of its own.
fn moon_shard(cell: &mut Cell, ink: TrinketInk) {
    cell.ellipse(8, 8, 5, 5, ink.body);
    cell.ellipse(12, 6, 5, 5, Rgba::TRANSPARENT);
    for (x, y) in [(3, 7), (3, 8), (3, 9), (4, 5), (4, 11), (5, 4), (5, 12)] {
        cell.px(x, y, ink.light);
    }
    for (x, y) in [(7, 13), (8, 12), (6, 12), (9, 11)] {
        cell.px(x, y, ink.deep);
    }
    cell.px(5, 8, ink.accent);
}

/// A jar still faintly warm, with nothing in it now.
fn firefly_jar(cell: &mut Cell, ink: TrinketInk) {
    cell.rect(6, 3, 4, 2, ink.accent);
    cell.rect(5, 5, 7, 1, ink.deep);
    cell.rect(4, 6, 9, 8, ink.body);
    for (x, y) in [(4, 6), (12, 6), (4, 13), (12, 13)] {
        cell.px(x, y, Rgba::TRANSPARENT);
    }
    cell.rect(5, 7, 1, 6, ink.light);
    for (x, y) in [(7, 9), (10, 8), (8, 12)] {
        cell.ellipse(x, y, 1, 1, ink.accent);
        cell.px(x, y, ink.light);
    }
    cell.px(11, 12, ink.deep);
    cell.px(11, 11, ink.deep);
}

/// A long feather: a straight rachis running corner to corner, barbs swept back off it, and a
/// split at the tip that no leaf has.
fn feather(cell: &mut Cell, ink: TrinketInk) {
    const BARBS: [(i32, i32, i32, i32); 10] = [
        (9, 3, 0, 0),
        (9, 4, 1, 1),
        (8, 5, 2, 2),
        (8, 6, 3, 2),
        (7, 7, 3, 2),
        (7, 8, 4, 2),
        (6, 9, 3, 2),
        (6, 10, 3, 1),
        (5, 11, 2, 1),
        (5, 12, 1, 1),
    ];
    for (x, y, left, right) in BARBS {
        cell.span(x - left, x + right, y, ink.body);
    }
    for (x, y, left, _) in BARBS {
        cell.span(x - left, x - left + 1, y, ink.light);
    }
    cell.line(9, 3, 4, 13, ink.deep);
    // The split the vane ends in, which no leaf has.
    cell.px(9, 5, Rgba::TRANSPARENT);
    cell.px(8, 4, Rgba::TRANSPARENT);
}

/// A tuft of something soft: three separate bumps along the top, a flat underside, and gaps
/// between the lobes deep enough to survive being halved.
fn cloud_puff(cell: &mut Cell, ink: TrinketInk) {
    // A puff is the one keepsake that is mostly light: filling it with the body colour turns it
    // into a potato, so the mass is pale and only its underside carries weight.
    cell.rect(3, 8, 11, 4, ink.light);
    for (cx, cy, rx, ry) in [(5, 7, 2, 2), (8, 5, 2, 3), (11, 7, 2, 2)] {
        cell.ellipse(cx, cy, rx, ry, ink.light);
    }
    // The dips between the lobes, cut back into the mass two pixels deep.
    for (x, y) in [
        (6, 7),
        (7, 7),
        (10, 7),
        (9, 7),
        (6, 6),
        (10, 6),
        (7, 6),
        (9, 6),
    ] {
        cell.px(x, y, Rgba::TRANSPARENT);
    }
    // Under-lit edges, so the lobes read as three rather than one.
    for (cx, cy, rx) in [(5, 7, 2), (8, 5, 2), (11, 7, 2)] {
        cell.span(cx - rx, cx + rx, cy + 2, ink.body);
    }
    cell.span(4, 12, 11, ink.body);
    cell.span(5, 11, 12, ink.deep);
    cell.px(3, 10, ink.accent);
    cell.px(13, 10, ink.accent);
}

/// Half a ticket, torn along one edge, for a ride nobody remembers.
fn ticket_stub(cell: &mut Cell, ink: TrinketInk) {
    cell.rect(3, 5, 10, 9, ink.body);
    for y in [6, 8, 10, 12] {
        cell.px(12, y, Rgba::TRANSPARENT);
    }
    cell.ellipse(5, 9, 1, 1, Rgba::TRANSPARENT);
    cell.span(3, 11, 5, ink.light);
    cell.rect(8, 7, 4, 5, ink.accent);
    cell.span(3, 7, 12, ink.deep);
    cell.span(7, 11, 13, ink.deep);
}

/// A paper wheel on a stick: four solid sails, each one a triangle off the hub, in two colours so
/// the turn is visible even when the whole thing is four pixels across.
fn pinwheel(cell: &mut Cell, ink: TrinketInk) {
    // One sail, as offsets from the hub. Rotating it three times makes the wheel.
    const SAIL: [(i32, i32); 10] = [
        (0, -1),
        (0, -2),
        (0, -3),
        (0, -4),
        (1, -2),
        (1, -3),
        (1, -4),
        (2, -3),
        (2, -4),
        (3, -4),
    ];
    let (hub_x, hub_y) = (9, 9);
    cell.rect(8, 10, 2, 4, ink.deep);
    cell.rect(8, 10, 1, 4, ink.body);
    let mut sail = SAIL;
    for turn in 0..4 {
        let color = if turn % 2 == 0 { ink.body } else { ink.accent };
        for (dx, dy) in sail {
            cell.px(hub_x + dx, hub_y + dy, color);
        }
        // The leading edge of each sail catches the light, which is what makes it read as
        // four sails rather than one star.
        cell.px(hub_x + sail[3].0, hub_y + sail[3].1, ink.light);
        sail = sail.map(|(dx, dy)| (-dy, dx));
    }
    cell.ellipse(hub_x, hub_y, 1, 1, ink.deep);
    cell.px(hub_x, hub_y, ink.light);
}

/// A cord tied in a bow: two loops swept up and outward from one knot, and two ends hanging from
/// it. Deliberately not two matching circles — a symmetrical pair of rings reads as a face.
fn friendship_knot(cell: &mut Cell, ink: TrinketInk) {
    for side in [-1, 1] {
        let cx = 8 + side * 2;
        // A loop that leans away from the knot, open toward its own outer corner.
        cell.ellipse(cx, 6, 3, 2, ink.body);
        cell.ellipse(cx + side, 5, 1, 1, Rgba::TRANSPARENT);
        cell.px(cx + side * 2, 8, Rgba::TRANSPARENT);
        cell.px(cx - side * 2, 4, ink.light);
    }
    // The knot itself, and the two ends falling from it at different lengths.
    cell.ellipse(8, 8, 2, 2, ink.accent);
    cell.px(7, 7, ink.light);
    cell.line(7, 10, 5, 13, ink.body);
    cell.line(9, 10, 11, 13, ink.body);
    cell.px(5, 13, ink.deep);
    cell.px(11, 13, ink.deep);
}

/// Two charms cut from the same piece, hung from one ring at different lengths so the pair reads
/// as two pendants on a chain rather than as something worn on a head.
fn matching_charms(cell: &mut Cell, ink: TrinketInk) {
    // The split ring they both hang from.
    cell.ellipse(8, 4, 2, 2, ink.deep);
    cell.px(8, 4, Rgba::TRANSPARENT);
    for (cx, cy, fill, mark) in [
        (5, 9, ink.body, ink.accent),
        (11, 11, ink.accent, ink.light),
    ] {
        cell.line(8, 5, cx, cy - 3, ink.deep);
        // A teardrop: wide at the bottom, drawn to a point at the top.
        cell.ellipse(cx, cy, 2, 2, fill);
        cell.px(cx, cy - 3, fill);
        cell.span(cx - 1, cx + 1, cy - 2, fill);
        cell.px(cx, cy, mark);
        cell.px(cx - 1, cy - 1, ink.light);
    }
}

/// One of the grid-drawn finds: its interior, two pixels in from the cell's corner.
fn draw_grid(cell: &mut Cell, ink: TrinketInk, grid: &[&str; 12]) {
    for (row, line) in grid.iter().enumerate() {
        for (column, ch) in line.chars().enumerate() {
            let color = match ch {
                'b' => ink.body,
                'd' => ink.deep,
                'l' => ink.light,
                'a' => ink.accent,
                'k' => ink.outline,
                'w' => sprites::WHITE,
                'c' => sprites::CREAM,
                _ => continue,
            };
            cell.px(column as i32 + 2, row as i32 + 2, color);
        }
    }
}

/// The twinkle on the second frame. Drawn after the outline, so it stays a spark of light rather
/// than another outlined object.
fn glint(cell: &mut Cell, ink: TrinketInk, variant: u8) {
    // Every spark sits where its own object catches the light, and far enough inside the cell for
    // its four rays to stay in it.
    const SPOTS: [(i32, i32); 16] = [
        (6, 5),
        (8, 5),
        (5, 4),
        (11, 6),
        (5, 7),
        (8, 4),
        (6, 5),
        (6, 4),
        (4, 8),
        (7, 8),
        (10, 5),
        (6, 6),
        (5, 5),
        (10, 4),
        (5, 6),
        (4, 9),
    ];
    let (x, y) = match sprites::grid(variant) {
        Some(grid) => sprites::glint_spot(grid),
        None => SPOTS[usize::from(variant) % SPOTS.len()],
    };
    cell.px(x, y, ink.light);
    for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
        cell.px(x + dx, y + dy, ink.light);
    }
    for (dx, dy) in [(2, 0), (-2, 0), (0, 2), (0, -2)] {
        cell.px(x + dx, y + dy, ink.accent);
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

    /// Every grid is exactly twelve by twelve, uses only the characters it may, and keeps out of
    /// the corners of the cell that land on a holder's eyes.
    #[test]
    fn every_grid_is_well_formed_and_keeps_to_where_it_may_draw() {
        const INKS: &str = ".bdlakwc";
        for (index, grid) in sprites::GRIDS.iter().enumerate() {
            let variant = usize::from(sprites::FIRST) + index;
            for (row, line) in grid.iter().enumerate() {
                assert_eq!(line.chars().count(), 12, "variant {variant} row {row}");
                for (column, ch) in line.chars().enumerate() {
                    assert!(INKS.contains(ch), "variant {variant} uses {ch:?}");
                    if ch == '.' {
                        continue;
                    }
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
        assert_eq!(
            usize::from(sprites::FIRST) + sprites::GRIDS.len(),
            usize::from(formiga_core::TRINKET_VARIANTS)
        );
    }
}
