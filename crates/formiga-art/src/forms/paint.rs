//! The few ways a sculpted form is painted: shapes filled into one 48-pixel frame, each with an
//! outline, a shade underneath and a light from the upper left, the way Formiga's companions are
//! drawn. A shape remembers where it lies, so the coat's markings can be laid out across it.
//!
//! The same frame can be painted in high definition: still 48 frame pixels across, with the same
//! shapes in the same places, but each frame pixel drawn as several, so curves step in smaller
//! stairs and there is room for finer shading and grain. It is still pixel art: every pixel is
//! one flat colour, all there or not there, and the outline is as heavy as a companion's.
//! Shapes are measured in frame pixels either way; only the sheet knows how finely it is
//! painted.

use crate::{Canvas, FRAME_SIZE, Rgba};

pub(crate) const SIZE: i32 = FRAME_SIZE as i32;

/// How thick the outline is in high definition, in frame pixels: a whole one, as heavy as every
/// companion's, so a sculpted form sits among them in the same style.
pub(crate) const HD_OUTLINE: f32 = 1.0;

/// `a` moved `t` (0 to 1) of the way toward `b`, alpha and all.
pub(crate) fn mix(a: Rgba, b: Rgba, t: f32) -> Rgba {
    let t = t.clamp(0.0, 1.0);
    let f = |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * t).round() as u8;
    Rgba::new(f(a.r, b.r), f(a.g, b.g), f(a.b, b.b), f(a.a, b.a))
}

/// A smooth step from 0 at `low` to 1 at `high`.
pub(crate) fn smoothstep(low: f32, high: f32, x: f32) -> f32 {
    let t = ((x - low) / (high - low)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The ink every companion is outlined in.
pub(crate) const OUTLINE: Rgba = Rgba::new(0x30, 0x2b, 0x3b, 255);

pub(crate) fn rgba(rgb: [u8; 3]) -> Rgba {
    Rgba::new(rgb[0], rgb[1], rgb[2], 255)
}

/// A colour softened toward white the way Desktop softens a recipe's coat, so a sculpted form
/// sits in the same light as every other companion.
pub(crate) fn soft(rgb: [u8; 3]) -> Rgba {
    rgba(rgb.map(|c| ((u16::from(c) * 3 + 220) / 4) as u8))
}

pub(crate) fn shade(color: Rgba) -> Rgba {
    let f = |c: u8| (u16::from(c) * 3 / 4) as u8;
    Rgba::new(f(color.r), f(color.g), f(color.b), color.a)
}

pub(crate) fn light(color: Rgba) -> Rgba {
    let f = |c: u8| ((u16::from(c) + 255) / 2) as u8;
    Rgba::new(f(color.r), f(color.g), f(color.b), color.a)
}

/// Halfway between a colour and its shade: the grain of fur, the line of a fold.
pub(crate) fn grain(color: Rgba) -> Rgba {
    let f = |c: u8| (u16::from(c) * 7 / 8) as u8;
    Rgba::new(f(color.r), f(color.g), f(color.b), color.a)
}

/// The same colour, a little toward a darker one: what a far limb is drawn in.
pub(crate) fn far(color: Rgba) -> Rgba {
    let f = |c: u8| (u16::from(c) * 6 / 7) as u8;
    Rgba::new(f(color.r), f(color.g), f(color.b), color.a)
}

/// A small, fixed hash: the same pixel and seed always give the same number.
pub(crate) fn hash(x: i32, y: i32, seed: u32) -> u32 {
    let mut h = (x as u32)
        .wrapping_mul(0x9e37_79b1)
        .wrapping_add((y as u32).wrapping_mul(0x85eb_ca77))
        .wrapping_add(seed.wrapping_mul(0xc2b2_ae3d));
    h ^= h >> 15;
    h = h.wrapping_mul(0x2c1b_3c6d);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297a_2d39);
    h ^ (h >> 15)
}

/// A value from 0 to 1 that changes smoothly across the frame, for soft blotches.
pub(crate) fn smooth_noise(x: f32, y: f32, cell: f32, seed: u32) -> f32 {
    let (gx, gy) = (x / cell, y / cell);
    let (x0, y0) = (gx.floor(), gy.floor());
    let (fx, fy) = (gx - x0, gy - y0);
    let corner =
        |dx: i32, dy: i32| (hash(x0 as i32 + dx, y0 as i32 + dy, seed) & 0xffff) as f32 / 65535.0;
    let ease = |t: f32| t * t * (3.0 - 2.0 * t);
    let (sx, sy) = (ease(fx), ease(fy));
    let top = corner(0, 0) + (corner(1, 0) - corner(0, 0)) * sx;
    let bottom = corner(0, 1) + (corner(1, 1) - corner(0, 1)) * sx;
    top + (bottom - top) * sy
}

/// One shape, in frame pixels. A pixel is inside when its centre is.
#[derive(Clone, Debug)]
pub(crate) enum Shape {
    /// An ellipse, turned `angle` radians from level.
    Ellipse {
        cx: f32,
        cy: f32,
        rx: f32,
        ry: f32,
        angle: f32,
    },
    /// A rounded stroke from `a` to `b` whose radius runs from `ra` to `rb`: limbs, necks, tails.
    Capsule {
        a: (f32, f32),
        b: (f32, f32),
        ra: f32,
        rb: f32,
    },
    /// Any polygon, filled even-odd.
    Polygon(Vec<(f32, f32)>),
}

impl Shape {
    pub(crate) fn ellipse(cx: f32, cy: f32, rx: f32, ry: f32) -> Self {
        Self::Ellipse {
            cx,
            cy,
            rx,
            ry,
            angle: 0.0,
        }
    }

    pub(crate) fn turned(cx: f32, cy: f32, rx: f32, ry: f32, angle: f32) -> Self {
        Self::Ellipse {
            cx,
            cy,
            rx,
            ry,
            angle,
        }
    }

    pub(crate) fn capsule(a: (f32, f32), b: (f32, f32), ra: f32, rb: f32) -> Self {
        Self::Capsule { a, b, ra, rb }
    }

    pub(crate) fn contains(&self, x: f32, y: f32) -> bool {
        match self {
            Self::Ellipse {
                cx,
                cy,
                rx,
                ry,
                angle,
            } => {
                let (dx, dy) = (x - cx, y - cy);
                let (sin, cos) = angle.sin_cos();
                let u = dx * cos + dy * sin;
                let v = -dx * sin + dy * cos;
                let (rx, ry) = (rx.max(0.5), ry.max(0.5));
                (u * u) / (rx * rx) + (v * v) / (ry * ry) <= 1.0
            }
            Self::Capsule { a, b, ra, rb } => {
                let (abx, aby) = (b.0 - a.0, b.1 - a.1);
                let length2 = abx * abx + aby * aby;
                let t = if length2 <= f32::EPSILON {
                    0.0
                } else {
                    (((x - a.0) * abx + (y - a.1) * aby) / length2).clamp(0.0, 1.0)
                };
                let (px, py) = (a.0 + abx * t, a.1 + aby * t);
                let radius = (ra + (rb - ra) * t).max(0.5);
                (x - px).powi(2) + (y - py).powi(2) <= radius * radius
            }
            Self::Polygon(points) => {
                let mut inside = false;
                let mut j = points.len().wrapping_sub(1);
                for i in 0..points.len() {
                    let (xi, yi) = points[i];
                    let (xj, yj) = points[j];
                    if (yi > y) != (yj > y) && x < (xj - xi) * (y - yi) / (yj - yi) + xi {
                        inside = !inside;
                    }
                    j = i;
                }
                inside
            }
        }
    }

    /// The sine and cosine of the angle an ellipse is turned by, worked out once for a shape
    /// measured at many points.
    fn turn(&self) -> (f32, f32) {
        match self {
            Self::Ellipse { angle, .. } => angle.sin_cos(),
            _ => (0.0, 1.0),
        }
    }

    /// How far `(x, y)` is from the shape's edge, in frame pixels, given its [`Shape::turn`]:
    /// negative inside, positive outside. Exact for strokes and polygons and very close for
    /// ellipses, which is all smooth edges and an even outline need.
    fn distance(&self, x: f32, y: f32, (sin, cos): (f32, f32)) -> f32 {
        match self {
            Self::Ellipse { cx, cy, rx, ry, .. } => {
                let (dx, dy) = (x - cx, y - cy);
                let u = dx * cos + dy * sin;
                let v = -dx * sin + dy * cos;
                let (rx, ry) = (rx.max(0.5), ry.max(0.5));
                let k0 = ((u / rx).powi(2) + (v / ry).powi(2)).sqrt();
                let k1 = ((u / (rx * rx)).powi(2) + (v / (ry * ry)).powi(2)).sqrt();
                if k1 <= f32::EPSILON {
                    -rx.min(ry)
                } else {
                    k0 * (k0 - 1.0) / k1
                }
            }
            Self::Capsule { a, b, ra, rb } => {
                let (abx, aby) = (b.0 - a.0, b.1 - a.1);
                let length2 = abx * abx + aby * aby;
                let t = if length2 <= f32::EPSILON {
                    0.0
                } else {
                    (((x - a.0) * abx + (y - a.1) * aby) / length2).clamp(0.0, 1.0)
                };
                let (px, py) = (a.0 + abx * t, a.1 + aby * t);
                let radius = (ra + (rb - ra) * t).max(0.5);
                ((x - px).powi(2) + (y - py).powi(2)).sqrt() - radius
            }
            Self::Polygon(points) => {
                let mut nearest = f32::MAX;
                let mut j = points.len().wrapping_sub(1);
                for i in 0..points.len() {
                    let (a, b) = (points[j], points[i]);
                    let (abx, aby) = (b.0 - a.0, b.1 - a.1);
                    let length2 = abx * abx + aby * aby;
                    let t = if length2 <= f32::EPSILON {
                        0.0
                    } else {
                        (((x - a.0) * abx + (y - a.1) * aby) / length2).clamp(0.0, 1.0)
                    };
                    let d = (x - a.0 - abx * t).powi(2) + (y - a.1 - aby * t).powi(2);
                    nearest = nearest.min(d);
                    j = i;
                }
                let d = nearest.sqrt();
                if self.contains(x, y) { -d } else { d }
            }
        }
    }

    /// Where the shape can be no more than [`FAR`] from: everywhere further is sure to be
    /// further from its edge than that, so need not be measured.
    fn reach(&self) -> Reach {
        match self {
            Self::Ellipse { cx, cy, rx, ry, .. } => {
                // Its edge is at least as far as the shortest radius times how many longest
                // radii out from the middle a point is, less one.
                let (rx, ry) = (rx.max(0.5), ry.max(0.5));
                let (short, long) = (rx.min(ry), rx.max(ry));
                Reach {
                    within: (*cx, *cy, *cx, *cy),
                    by: long * (1.0 + FAR / short),
                }
            }
            Self::Capsule { a, b, ra, rb } => Reach {
                within: (a.0.min(b.0), a.1.min(b.1), a.0.max(b.0), a.1.max(b.1)),
                by: ra.max(*rb).max(0.5) + FAR,
            },
            Self::Polygon(_) => Reach {
                within: self.bounds(),
                by: FAR,
            },
        }
    }

    /// The box the shape lies in, in frame pixels: left, top, right, bottom.
    pub(crate) fn bounds(&self) -> (f32, f32, f32, f32) {
        match self {
            Self::Ellipse { cx, cy, rx, ry, .. } => {
                let r = rx.max(*ry).max(0.5);
                (cx - r, cy - r, cx + r, cy + r)
            }
            Self::Capsule { a, b, ra, rb } => {
                let r = ra.max(*rb).max(0.5);
                (
                    a.0.min(b.0) - r,
                    a.1.min(b.1) - r,
                    a.0.max(b.0) + r,
                    a.1.max(b.1) + r,
                )
            }
            Self::Polygon(points) => points.iter().fold(
                (f32::MAX, f32::MAX, f32::MIN, f32::MIN),
                |(l, t, r, b), &(x, y)| (l.min(x), t.min(y), r.max(x), b.max(y)),
            ),
        }
    }
}

/// How far from a shape's edge a pixel can be and still be drawn differently from one further
/// out, in frame pixels: as far as the outline reaches, with room to spare. High definition only.
const FAR: f32 = HD_OUTLINE + 0.25;

/// A box and how far round it a shape's edge is within [`FAR`] of; see [`Shape::reach`].
struct Reach {
    within: (f32, f32, f32, f32),
    by: f32,
}

impl Reach {
    fn near(&self, x: f32, y: f32) -> bool {
        let (l, t, r, b) = self.within;
        let dx = (l - x).max(x - r).max(0.0);
        let dy = (t - y).max(y - b).max(0.0);
        dx * dx + dy * dy <= self.by * self.by
    }
}

/// Which pixels of the sheet a set of shapes covers, together. In high definition it also
/// knows how far each pixel is from the edge, so edges can be drawn smooth and outlines even.
#[derive(Clone)]
pub(crate) struct Mask {
    /// Sheet pixels to a frame pixel.
    res: i32,
    /// The box the shapes lie in, in sheet pixels: left, top, width, height.
    left: i32,
    top: i32,
    width: i32,
    height: i32,
    /// At one pixel to a frame pixel, -1 for covered and 1 for not; in high definition, how far
    /// each pixel's centre is from the edge, in frame pixels, negative inside. Past [`FAR`] it
    /// only says the pixel is at least that far out.
    cells: Vec<f32>,
}

impl Mask {
    pub(crate) fn of(shapes: &[Shape], res: i32) -> Self {
        let res = res.max(1);
        let size = SIZE * res;
        let (l, t, r, b) = shapes.iter().map(Shape::bounds).fold(
            (f32::MAX, f32::MAX, f32::MIN, f32::MIN),
            |(l, t, r, b), (sl, st, sr, sb)| (l.min(sl), t.min(st), r.max(sr), b.max(sb)),
        );
        if shapes.is_empty() || !(l <= r && t <= b) {
            return Self {
                res,
                left: 0,
                top: 0,
                width: 0,
                height: 0,
                cells: Vec::new(),
            };
        }
        // Room round the shapes for the outline and a pixel's smoothing.
        let margin = if res == 1 { 1.0 } else { HD_OUTLINE + 1.0 };
        let to_sheet = |v: f32| v * res as f32;
        let left = (to_sheet(l - margin).floor() as i32).clamp(0, size);
        let top = (to_sheet(t - margin).floor() as i32).clamp(0, size);
        let right = (to_sheet(r + margin).ceil() as i32).clamp(0, size);
        let bottom = (to_sheet(b + margin).ceil() as i32).clamp(0, size);
        let (width, height) = (right - left, bottom - top);
        let mut cells = Vec::with_capacity((width.max(0) * height.max(0)) as usize);
        let turns: Vec<(f32, f32)> = shapes.iter().map(Shape::turn).collect();
        let reaches: Vec<Reach> = shapes.iter().map(Shape::reach).collect();
        for y in top..bottom {
            for x in left..right {
                let (px, py) = ((x as f32 + 0.5) / res as f32, (y as f32 + 0.5) / res as f32);
                cells.push(if res == 1 {
                    if shapes.iter().any(|s| s.contains(px, py)) {
                        -1.0
                    } else {
                        1.0
                    }
                } else {
                    shapes
                        .iter()
                        .zip(&turns)
                        .zip(&reaches)
                        .filter(|(_, reach)| reach.near(px, py))
                        .map(|((s, &turn), _)| s.distance(px, py, turn))
                        .fold(f32::MAX, f32::min)
                });
            }
        }
        Self {
            res,
            left,
            top,
            width: width.max(0),
            height: height.max(0),
            cells,
        }
    }

    /// How far a pixel is from the edge, in frame pixels, negative inside. At one pixel to a
    /// frame pixel only the sign means anything.
    pub(crate) fn distance(&self, x: i32, y: i32) -> f32 {
        let (cx, cy) = (x - self.left, y - self.top);
        if cx < 0 || cy < 0 || cx >= self.width || cy >= self.height {
            return f32::MAX;
        }
        self.cells[(cy * self.width + cx) as usize]
    }

    pub(crate) fn has(&self, x: i32, y: i32) -> bool {
        self.distance(x, y) <= 0.0
    }

    /// How much of a pixel the shapes cover, from 0 to 1: always all or nothing at one pixel to
    /// a frame pixel, and smooth along the edge in high definition.
    pub(crate) fn coverage(&self, x: i32, y: i32) -> f32 {
        let d = self.distance(x, y);
        if self.res == 1 {
            if d <= 0.0 { 1.0 } else { 0.0 }
        } else {
            (0.5 - d * self.res as f32).clamp(0.0, 1.0)
        }
    }

    /// How far inside a pixel is, in frame pixels, or 0 outside. High definition only.
    pub(crate) fn depth(&self, x: i32, y: i32) -> f32 {
        (-self.distance(x, y)).max(0.0)
    }

    pub(crate) fn is_empty(&self) -> bool {
        !self.cells.iter().any(|c| *c <= 0.0)
    }

    /// Every pixel the mask covers, even in part, row by row.
    pub(crate) fn pixels(&self) -> impl Iterator<Item = (i32, i32)> + '_ {
        let edge = if self.res == 1 {
            0.0
        } else {
            0.5 / self.res as f32
        };
        self.measured()
            .filter_map(move |(x, y, d)| (d < edge || d <= 0.0).then_some((x, y)))
    }

    /// Every pixel of the box the mask was measured over, with how far it is from the edge.
    pub(crate) fn measured(&self) -> impl Iterator<Item = (i32, i32, f32)> + '_ {
        let width = self.width.max(1) as usize;
        self.cells
            .chunks(width)
            .zip(self.top..)
            .flat_map(move |(row, y)| row.iter().zip(self.left..).map(move |(&d, x)| (x, y, d)))
    }

    /// Whether a covered pixel is on the mask's edge: the last pixel inside, or in high
    /// definition, within half a frame pixel of it.
    pub(crate) fn edge(&self, x: i32, y: i32) -> bool {
        if self.res > 1 {
            return self.distance(x, y) > -0.5 && self.distance(x, y) < 0.5 / self.res as f32;
        }
        self.has(x, y)
            && [(1, 0), (-1, 0), (0, 1), (0, -1)]
                .into_iter()
                .any(|(dx, dy)| !self.has(x + dx, y + dy))
    }
}

/// Where a painted region lies, so a marking can be laid across it: `u` runs along it from -1 to
/// 1, `v` across it from -1 to 1. A level body runs `u` from tail to head and `v` from back to
/// belly; a leg runs `u` from hip to foot.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Frame {
    pub(crate) cx: f32,
    pub(crate) cy: f32,
    /// The direction `u` runs in, as a unit vector.
    pub(crate) axis: (f32, f32),
    pub(crate) half_length: f32,
    pub(crate) half_width: f32,
}

impl Frame {
    pub(crate) fn level(cx: f32, cy: f32, rx: f32, ry: f32) -> Self {
        Self {
            cx,
            cy,
            axis: (1.0, 0.0),
            half_length: rx.max(0.5),
            half_width: ry.max(0.5),
        }
    }

    /// Along a stroke from `a` to `b`, `width` either side of it.
    pub(crate) fn along(a: (f32, f32), b: (f32, f32), width: f32) -> Self {
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let length = (dx * dx + dy * dy).sqrt().max(0.5);
        Self {
            cx: (a.0 + b.0) / 2.0,
            cy: (a.1 + b.1) / 2.0,
            axis: (dx / length, dy / length),
            half_length: length / 2.0 + width,
            half_width: width.max(0.5),
        }
    }

    /// Where the point `(x, y)`, in frame pixels, lies across the region.
    pub(crate) fn at(&self, x: f32, y: f32) -> (f32, f32) {
        let (dx, dy) = (x - self.cx, y - self.cy);
        let (ax, ay) = self.axis;
        // Across runs a quarter turn clockwise from along, so across a level body points down.
        let u = (dx * ax + dy * ay) / self.half_length;
        let v = (-dx * ay + dy * ax) / self.half_width;
        (u, v)
    }
}

/// The frame being painted.
pub(crate) struct Sheet {
    pub(crate) canvas: Canvas,
    /// Sheet pixels to a frame pixel: 1 for the frame Desktop draws, more for high definition.
    res: i32,
    /// In high definition, how much of each pixel is outline ink (0 to 1), so a later fill can
    /// tell a drawn line from the coat it lies on.
    ink: Vec<f32>,
}

impl Sheet {
    pub(crate) fn new() -> Self {
        Self::with_res(1)
    }

    /// A sheet `res` pixels to each frame pixel.
    pub(crate) fn with_res(res: i32) -> Self {
        let res = res.max(1);
        let size = FRAME_SIZE * res as u32;
        Self {
            canvas: Canvas::new(size, size),
            res,
            ink: if res > 1 {
                vec![0.0; (size * size) as usize]
            } else {
                Vec::new()
            },
        }
    }

    pub(crate) fn res(&self) -> i32 {
        self.res
    }

    pub(crate) fn hd(&self) -> bool {
        self.res > 1
    }

    /// The mask of `shapes`, as finely as this sheet is painted.
    pub(crate) fn mask(&self, shapes: &[Shape]) -> Mask {
        Mask::of(shapes, self.res)
    }

    /// The middle of a sheet pixel, in frame pixels.
    pub(crate) fn center(&self, x: i32, y: i32) -> (f32, f32) {
        let res = self.res as f32;
        ((x as f32 + 0.5) / res, (y as f32 + 0.5) / res)
    }

    pub(crate) fn set(&mut self, x: i32, y: i32, color: Rgba) {
        self.canvas.set(x, y, color);
        if let Some(ink) = self.ink_at(x, y) {
            *ink = if color == OUTLINE { 1.0 } else { 0.0 };
        }
    }

    pub(crate) fn get(&self, x: i32, y: i32) -> Rgba {
        self.canvas.get(x, y)
    }

    fn ink_at(&mut self, x: i32, y: i32) -> Option<&mut f32> {
        let size = self.canvas.width() as i32;
        if self.ink.is_empty() || x < 0 || y < 0 || x >= size || y >= size {
            return None;
        }
        self.ink.get_mut((y * size + x) as usize)
    }

    /// How much of a pixel is outline ink, from 0 to 1.
    pub(crate) fn inked(&self, x: i32, y: i32) -> f32 {
        if self.ink.is_empty() {
            return if self.get(x, y) == OUTLINE { 1.0 } else { 0.0 };
        }
        let size = self.canvas.width() as i32;
        if x < 0 || y < 0 || x >= size || y >= size {
            return 0.0;
        }
        self.ink[(y * size + x) as usize]
    }

    /// `color` painted on a pixel `alpha` (0 to 1) covers: all of it from half on, as pixel
    /// art is either there or not.
    pub(crate) fn paint(&mut self, x: i32, y: i32, color: Rgba, alpha: f32) {
        if alpha >= 0.5 {
            self.set(x, y, color);
        }
    }

    /// The outline round a mask: every uncovered pixel beside a covered one, or in high
    /// definition, a fine smooth line round it.
    pub(crate) fn outline(&mut self, mask: &Mask, ink: Rgba) {
        if self.hd() {
            let res = self.res as f32;
            for (x, y, d) in mask.measured() {
                if d > -1.0 / res {
                    self.paint(x, y, ink, ((HD_OUTLINE - d) * res + 0.5).clamp(0.0, 1.0));
                }
            }
            return;
        }
        for y in -1..=SIZE {
            for x in -1..=SIZE {
                if !mask.has(x, y)
                    && [(1, 0), (-1, 0), (0, 1), (0, -1)]
                        .into_iter()
                        .any(|(dx, dy)| mask.has(x + dx, y + dy))
                {
                    self.set(x, y, ink);
                }
            }
        }
    }

    /// A mask filled flat, outlined, and nothing else: small hard parts like claws and beaks.
    pub(crate) fn flat(&mut self, shapes: &[Shape], fill: Rgba) -> Mask {
        let mask = self.mask(shapes);
        self.outline(&mask, OUTLINE);
        for (x, y) in mask.pixels() {
            let alpha = mask.coverage(x, y);
            self.paint(x, y, fill, alpha);
        }
        mask
    }

    /// Shapes filled in one colour with smooth edges and no outline: a nose, a pad, a claw.
    pub(crate) fn blob(&mut self, shapes: &[Shape], fill: Rgba) {
        let mask = self.mask(shapes);
        for (x, y) in mask.pixels() {
            let alpha = mask.coverage(x, y);
            self.paint(x, y, fill, alpha);
        }
    }

    /// One line, for folds, scutes and feathers drawn onto what is already there: a pixel line
    /// at one pixel to a frame pixel, and a fine smooth stroke kept off the edge in high
    /// definition.
    pub(crate) fn line(&mut self, a: (f32, f32), b: (f32, f32), color: Rgba, within: &Mask) {
        if self.hd() {
            let stroke = self.mask(&[Shape::capsule(a, b, 0.3, 0.3)]);
            let res = self.res as f32;
            for (x, y) in stroke.pixels() {
                let keep = ((within.depth(x, y) - 0.7) * res + 0.5).clamp(0.0, 1.0);
                let alpha = stroke.coverage(x, y) * keep;
                self.paint(x, y, color, alpha);
            }
            return;
        }
        let steps = ((b.0 - a.0).abs().max((b.1 - a.1).abs()).ceil() as i32).max(1);
        for step in 0..=steps {
            let t = step as f32 / steps as f32;
            let x = (a.0 + (b.0 - a.0) * t).floor() as i32;
            let y = (a.1 + (b.1 - a.1) * t).floor() as i32;
            if within.has(x, y) && !within.edge(x, y) {
                self.set(x, y, color);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Leaving far pixels unmeasured changes nothing that is drawn: every pixel within reach
    /// of an edge is measured exactly as before, and every other one is past the outline.
    #[test]
    fn far_pixels_left_unmeasured_change_nothing_drawn() {
        let mut seed = 12345u32;
        let mut next = move |lo: f32, hi: f32| {
            seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
            lo + (hi - lo) * ((seed >> 8) as f32 / (1u32 << 24) as f32)
        };
        for round in 0..300 {
            let mut shapes = Vec::new();
            for _ in 0..1 + round % 3 {
                let (x, y) = (next(4.0, 44.0), next(4.0, 44.0));
                shapes.push(match round % 4 {
                    0 => Shape::turned(x, y, next(0.2, 14.0), next(0.2, 3.0), next(-3.2, 3.2)),
                    1 => Shape::capsule(
                        (x, y),
                        (next(4.0, 44.0), next(4.0, 44.0)),
                        next(0.2, 5.0),
                        next(0.2, 5.0),
                    ),
                    2 => Shape::Polygon(vec![
                        (x, y),
                        (next(4.0, 44.0), next(4.0, 44.0)),
                        (x + next(-6.0, 6.0), y + next(-6.0, 6.0)),
                        (next(4.0, 44.0), next(4.0, 44.0)),
                    ]),
                    _ => Shape::ellipse(x, y, next(0.2, 12.0), next(0.2, 12.0)),
                });
            }
            for res in 2..=4 {
                let mask = Mask::of(&shapes, res);
                for (x, y, d) in mask.measured() {
                    let (px, py) = ((x as f32 + 0.5) / res as f32, (y as f32 + 0.5) / res as f32);
                    let exact = shapes
                        .iter()
                        .map(|s| s.distance(px, py, s.turn()))
                        .fold(f32::MAX, f32::min);
                    if exact <= FAR {
                        assert_eq!(d.to_bits(), exact.to_bits(), "round {round} at {x},{y}");
                    } else {
                        assert!(d > HD_OUTLINE, "round {round} at {x},{y}: {d} for {exact}");
                    }
                }
            }
        }
    }
}
