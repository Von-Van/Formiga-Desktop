//! Painting tools for the companion apps' places: Formiga Hill's scenery, Formiga Home's rooms, and
//! whatever comes after them.
//!
//! A place is drawn at Formiga's pixel scale with more care than Desktop's overlays can afford:
//! every material has a ramp of tones, light comes from the upper left, edges are outlined in a
//! darker shade of their own colour rather than in black (so the near-black-outlined creatures
//! always read first), and surfaces carry texture from a deterministic hash, so a place is the same
//! pixel for pixel every time it is drawn.
//!
//! These began as Formiga Home's own and are kept exactly as Home draws with them, so an app that
//! takes them from here draws every pixel as it did. Formiga Hill's earlier copy differs in three
//! small ways: its `mix` truncates rather than rounds, its `ellipse` draws nothing at all for a
//! zero radius, and its `polygon` takes whole-pixel corners and leaves out a pixel whose centre
//! lies exactly on a right-hand edge.

use crate::{Canvas, Rgba};

pub const fn rgb(hex: u32) -> Rgba {
    Rgba::new((hex >> 16) as u8, (hex >> 8) as u8, hex as u8, 255)
}

pub const fn rgba(hex: u32, alpha: u8) -> Rgba {
    Rgba::new((hex >> 16) as u8, (hex >> 8) as u8, hex as u8, alpha)
}

/// One material's tones, darkest first. `edge` outlines it; `shine` is used a pixel at a time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ramp {
    pub edge: Rgba,
    pub shadow: Rgba,
    pub base: Rgba,
    pub light: Rgba,
    pub shine: Rgba,
}

impl Ramp {
    pub const fn new(edge: u32, shadow: u32, base: u32, light: u32, shine: u32) -> Self {
        Self {
            edge: rgb(edge),
            shadow: rgb(shadow),
            base: rgb(base),
            light: rgb(light),
            shine: rgb(shine),
        }
    }
}

/// A well-mixed hash of a pixel and a salt: texture that never changes between runs.
pub fn noise(x: i32, y: i32, salt: u32) -> u32 {
    let mut hash = (x as u32).wrapping_mul(0x27d4_eb2d)
        ^ (y as u32).wrapping_mul(0x1656_67b1)
        ^ salt.wrapping_mul(0x9e37_79b9);
    hash ^= hash >> 15;
    hash = hash.wrapping_mul(0x2c1b_3c6d);
    hash ^= hash >> 12;
    hash = hash.wrapping_mul(0x297a_2d39);
    hash ^ (hash >> 15)
}

/// True for about `per_256` pixels in every 256.
pub fn chance(x: i32, y: i32, salt: u32, per_256: u32) -> bool {
    noise(x, y, salt) & 0xff < per_256
}

/// `top` laid over `bottom` by its alpha. Onto a clear or half-clear pixel the colours are
/// weighted by how much of each is there, so a glow drawn into an empty sprite keeps its colour.
pub fn over(top: Rgba, bottom: Rgba) -> Rgba {
    match (top.a, bottom.a) {
        (255, _) | (_, 0) => top,
        (0, _) => bottom,
        (top_alpha, bottom_alpha) => {
            let top_alpha = u32::from(top_alpha);
            let under = u32::from(bottom_alpha) * (255 - top_alpha);
            let total = top_alpha * 255 + under;
            let blend = |a: u8, b: u8| {
                ((u32::from(a) * top_alpha * 255 + u32::from(b) * under) / total) as u8
            };
            Rgba::new(
                blend(top.r, bottom.r),
                blend(top.g, bottom.g),
                blend(top.b, bottom.b),
                (total / 255).min(255) as u8,
            )
        }
    }
}

/// `a` moved `amount` of the way to `b`, alpha included.
pub fn mix(a: Rgba, b: Rgba, amount: f32) -> Rgba {
    let amount = amount.clamp(0.0, 1.0);
    let lerp = |a: u8, b: u8| (f32::from(a) + (f32::from(b) - f32::from(a)) * amount).round() as u8;
    Rgba::new(
        lerp(a.r, b.r),
        lerp(a.g, b.g),
        lerp(a.b, b.b),
        lerp(a.a, b.a),
    )
}

/// [`mix`] with each channel cut down to a whole number rather than rounded to the nearest, as the
/// train's compartment light is shaded.
pub fn mix_truncated(a: Rgba, b: Rgba, amount: f32) -> Rgba {
    let amount = amount.clamp(0.0, 1.0);
    let lerp = |a: u8, b: u8| (f32::from(a) + (f32::from(b) - f32::from(a)) * amount) as u8;
    Rgba::new(
        lerp(a.r, b.r),
        lerp(a.g, b.g),
        lerp(a.b, b.b),
        lerp(a.a, b.a),
    )
}

/// The same colour at another alpha.
pub fn faded(color: Rgba, alpha: u8) -> Rgba {
    Rgba::new(color.r, color.g, color.b, alpha)
}

/// One pixel, laid over what is there.
pub fn put(canvas: &mut Canvas, x: i32, y: i32, color: Rgba) {
    let under = canvas.get(x, y);
    canvas.set(x, y, over(color, under));
}

pub fn hline(canvas: &mut Canvas, x: i32, y: i32, width: i32, color: Rgba) {
    for dx in 0..width.max(0) {
        put(canvas, x + dx, y, color);
    }
}

pub fn vline(canvas: &mut Canvas, x: i32, y: i32, height: i32, color: Rgba) {
    for dy in 0..height.max(0) {
        put(canvas, x, y + dy, color);
    }
}

pub fn rect(canvas: &mut Canvas, x: i32, y: i32, width: i32, height: i32, color: Rgba) {
    for dy in 0..height.max(0) {
        hline(canvas, x, y + dy, width, color);
    }
}

pub fn ellipse(canvas: &mut Canvas, cx: i32, cy: i32, rx: i32, ry: i32, color: Rgba) {
    if rx <= 0 || ry <= 0 {
        put(canvas, cx, cy, color);
        return;
    }
    let (rx2, ry2) = (i64::from(rx * rx), i64::from(ry * ry));
    for dy in -ry..=ry {
        for dx in -rx..=rx {
            if i64::from(dx * dx) * ry2 + i64::from(dy * dy) * rx2 <= rx2 * ry2 {
                put(canvas, cx + dx, cy + dy, color);
            }
        }
    }
}

/// A box lit from the upper left: highlight along the top and left, shade along the bottom and
/// right, outlined in the material's own edge tone.
pub fn bevel(canvas: &mut Canvas, x: i32, y: i32, width: i32, height: i32, ramp: Ramp) {
    rect(canvas, x, y, width, height, ramp.edge);
    if width <= 2 || height <= 2 {
        return;
    }
    rect(canvas, x + 1, y + 1, width - 2, height - 2, ramp.base);
    hline(canvas, x + 1, y + 1, width - 2, ramp.light);
    vline(canvas, x + 1, y + 1, height - 2, ramp.light);
    hline(canvas, x + 1, y + height - 2, width - 2, ramp.shadow);
    vline(canvas, x + width - 2, y + 1, height - 2, ramp.shadow);
    put(canvas, x + 1, y + 1, ramp.shine);
}

/// Fill a polygon by the pixel centres it covers, whatever its winding. A pixel whose centre lies
/// exactly on the polygon's right-hand edge is covered too.
pub fn polygon(canvas: &mut Canvas, points: &[(f32, f32)], color: Rgba) {
    fill_polygon(points, |x, y| put(canvas, x, y, color));
}

/// Every pixel a polygon covers, by its centre, handed to `paint`.
pub fn fill_polygon(points: &[(f32, f32)], mut paint: impl FnMut(i32, i32)) {
    if points.len() < 3 {
        return;
    }
    let top = points.iter().map(|p| p.1).fold(f32::MAX, f32::min).floor() as i32;
    let bottom = points.iter().map(|p| p.1).fold(f32::MIN, f32::max).ceil() as i32;
    for y in top..=bottom {
        let row = y as f32 + 0.5;
        let mut crossings: Vec<f32> = Vec::with_capacity(4);
        for (index, &(x0, y0)) in points.iter().enumerate() {
            let (x1, y1) = points[(index + 1) % points.len()];
            if (y0 <= row && row < y1) || (y1 <= row && row < y0) {
                crossings.push(x0 + (row - y0) / (y1 - y0) * (x1 - x0));
            }
        }
        crossings.sort_by(f32::total_cmp);
        for pair in crossings.chunks_exact(2) {
            let (from, to) = (
                (pair[0] - 0.5).ceil() as i32,
                (pair[1] - 0.5).floor() as i32,
            );
            for x in from..=to {
                paint(x, y);
            }
        }
    }
}

/// A one-pixel line from one point to another.
pub fn line(canvas: &mut Canvas, from: (i32, i32), to: (i32, i32), color: Rgba) {
    let (mut x, mut y) = from;
    let (dx, dy) = ((to.0 - x).abs(), -(to.1 - y).abs());
    let (sx, sy) = (if x < to.0 { 1 } else { -1 }, if y < to.1 { 1 } else { -1 });
    let mut error = dx + dy;
    loop {
        put(canvas, x, y, color);
        if (x, y) == to {
            break;
        }
        let twice = 2 * error;
        if twice >= dy {
            error += dy;
            x += sx;
        }
        if twice <= dx {
            error += dx;
            y += sy;
        }
    }
}

/// A sprite laid over the canvas with its top-left at `(x, y)`.
pub fn blit(canvas: &mut Canvas, sprite: &Canvas, x: i32, y: i32) {
    blit_faded(canvas, sprite, x, y, 255);
}

/// A sprite laid over the canvas at a fraction of its own opacity, out of 255.
pub fn blit_faded(canvas: &mut Canvas, sprite: &Canvas, x: i32, y: i32, opacity: u8) {
    for sy in 0..sprite.height() as i32 {
        for sx in 0..sprite.width() as i32 {
            let pixel = sprite.get(sx, sy);
            if pixel.a == 0 {
                continue;
            }
            let alpha = (u32::from(pixel.a) * u32::from(opacity) / 255) as u8;
            put(canvas, x + sx, y + sy, faded(pixel, alpha));
        }
    }
}

/// A sprite laid over the canvas at an opacity, out of 255, that can differ pixel by pixel: how a
/// piece is drawn see-through only where it stands in front of something it would hide.
pub fn blit_through(
    canvas: &mut Canvas,
    sprite: &Canvas,
    x: i32,
    y: i32,
    opacity: impl Fn(i32, i32) -> u8,
) {
    for sy in 0..sprite.height() as i32 {
        for sx in 0..sprite.width() as i32 {
            let pixel = sprite.get(sx, sy);
            if pixel.a == 0 {
                continue;
            }
            let (px, py) = (x + sx, y + sy);
            let alpha = (u32::from(pixel.a) * u32::from(opacity(px, py)) / 255) as u8;
            put(canvas, px, py, faded(pixel, alpha));
        }
    }
}

/// A sprite laid over the canvas washed towards `tint`, for a piece being carried: green where it
/// can go, red where it cannot.
pub fn blit_tinted(canvas: &mut Canvas, sprite: &Canvas, x: i32, y: i32, tint: Rgba, opacity: u8) {
    for sy in 0..sprite.height() as i32 {
        for sx in 0..sprite.width() as i32 {
            let pixel = sprite.get(sx, sy);
            if pixel.a == 0 {
                continue;
            }
            let washed = mix(pixel, faded(tint, pixel.a), 0.45);
            let alpha = (u32::from(pixel.a) * u32::from(opacity) / 255) as u8;
            put(canvas, x + sx, y + sy, faded(washed, alpha));
        }
    }
}

/// A one-pixel ring round a sprite's opaque pixels, drawn where it would sit at `(x, y)`: how a
/// hovered or chosen thing is picked out without covering any of it.
pub fn ring(canvas: &mut Canvas, sprite: &Canvas, x: i32, y: i32, color: Rgba) {
    let solid = |sx: i32, sy: i32| sprite.get(sx, sy).a > 40;
    for sy in -1..=sprite.height() as i32 {
        for sx in -1..=sprite.width() as i32 {
            if solid(sx, sy) {
                continue;
            }
            let touching = [(1, 0), (-1, 0), (0, 1), (0, -1)]
                .iter()
                .any(|(dx, dy)| solid(sx + dx, sy + dy));
            if touching {
                put(canvas, x + sx, y + sy, color);
            }
        }
    }
}

/// Every opaque pixel of a sprite, outlined in `edge` wherever it meets clear space: one pass that
/// gives a whole drawing a crisp edge in a darker shade of its own colours.
pub fn outline_inside(sprite: &mut Canvas, edge: impl Fn(Rgba) -> Rgba) {
    let source = sprite.clone();
    let clear = |x: i32, y: i32| source.get(x, y).a == 0;
    for y in 0..source.height() as i32 {
        for x in 0..source.width() as i32 {
            let pixel = source.get(x, y);
            if pixel.a == 0 {
                continue;
            }
            if clear(x - 1, y) || clear(x + 1, y) || clear(x, y - 1) || clear(x, y + 1) {
                sprite.set(x, y, edge(pixel));
            }
        }
    }
}

/// A colour a step darker and a touch warmer: the edge of a material drawn in it.
pub fn darker(color: Rgba, amount: f32) -> Rgba {
    let warm = Rgba::new(0x3a, 0x22, 0x1c, color.a);
    mix(color, warm, amount)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_polygon_fills_exactly_the_pixels_it_covers() {
        let mut canvas = Canvas::new(8, 8);
        polygon(
            &mut canvas,
            &[(1.0, 1.0), (5.0, 1.0), (5.0, 4.0), (1.0, 4.0)],
            rgb(0xff0000),
        );
        let filled = canvas
            .pixels()
            .iter()
            .filter(|pixel| pixel.a == 255)
            .count();
        assert_eq!(filled, 12);
        assert_eq!(canvas.get(1, 1).a, 255);
        assert_eq!(canvas.get(5, 1).a, 0);
    }

    #[test]
    fn noise_is_stable_and_spread() {
        assert_eq!(noise(3, 4, 5), noise(3, 4, 5));
        let hits = (0..64)
            .flat_map(|y| (0..64).map(move |x| (x, y)))
            .filter(|&(x, y)| chance(x, y, 9, 64))
            .count();
        assert!((800..1250).contains(&hits), "{hits} of 4096 for a quarter");
    }

    #[test]
    fn two_half_clear_layers_build_up_rather_than_darken() {
        let shadow = rgba(0x2a4a2a, 70);
        let twice = over(shadow, shadow);
        assert!(twice.a > shadow.a && twice.g == shadow.g, "{twice:?}");
    }

    #[test]
    fn a_bevel_is_lit_from_the_upper_left_and_edged_in_its_own_tone() {
        let ramp = Ramp::new(0x3a2a1c, 0x6b4a30, 0x8a6240, 0xa87c52, 0xd8b080);
        let mut canvas = Canvas::new(6, 5);
        bevel(&mut canvas, 0, 0, 6, 5, ramp);
        assert_eq!(canvas.get(0, 0), ramp.edge);
        assert_eq!(canvas.get(5, 4), ramp.edge);
        assert_eq!(canvas.get(1, 1), ramp.shine);
        assert_eq!(canvas.get(2, 1), ramp.light);
        assert_eq!(canvas.get(4, 3), ramp.shadow);
        assert_eq!(canvas.get(2, 2), ramp.base);
    }

    #[test]
    fn half_clear_over_clear_keeps_its_own_colour() {
        let glow = rgba(0xffcc66, 128);
        assert_eq!(over(glow, Rgba::TRANSPARENT), glow);
        let mixed = over(glow, rgb(0x000000));
        assert_eq!(mixed.a, 255);
        assert!(mixed.r > 100 && mixed.r < 160);
    }
}
