//! The few ways a sculpted form is painted: shapes filled into one 48-pixel frame, each with an
//! outline, a shade underneath and a light from the upper left, the way Formiga's companions are
//! drawn. A shape remembers where it lies, so the coat's markings can be laid out across it.

use crate::{Canvas, FRAME_SIZE, Rgba};

pub(crate) const SIZE: i32 = FRAME_SIZE as i32;

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
}

/// Which pixels of the frame a set of shapes covers, together.
#[derive(Clone)]
pub(crate) struct Mask {
    cells: Vec<bool>,
}

impl Mask {
    pub(crate) fn of(shapes: &[Shape]) -> Self {
        let mut cells = vec![false; (SIZE * SIZE) as usize];
        for y in 0..SIZE {
            for x in 0..SIZE {
                let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                cells[(y * SIZE + x) as usize] = shapes.iter().any(|s| s.contains(px, py));
            }
        }
        Self { cells }
    }

    pub(crate) fn has(&self, x: i32, y: i32) -> bool {
        (0..SIZE).contains(&x) && (0..SIZE).contains(&y) && self.cells[(y * SIZE + x) as usize]
    }

    pub(crate) fn is_empty(&self) -> bool {
        !self.cells.iter().any(|c| *c)
    }

    /// Every pixel the mask covers.
    pub(crate) fn pixels(&self) -> impl Iterator<Item = (i32, i32)> + '_ {
        (0..SIZE).flat_map(move |y| (0..SIZE).filter_map(move |x| self.has(x, y).then_some((x, y))))
    }

    /// Whether a covered pixel is on the mask's edge.
    pub(crate) fn edge(&self, x: i32, y: i32) -> bool {
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

    pub(crate) fn local(&self, x: i32, y: i32) -> (f32, f32) {
        let (dx, dy) = (x as f32 + 0.5 - self.cx, y as f32 + 0.5 - self.cy);
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
}

impl Sheet {
    pub(crate) fn new() -> Self {
        Self {
            canvas: Canvas::new(FRAME_SIZE, FRAME_SIZE),
        }
    }

    pub(crate) fn set(&mut self, x: i32, y: i32, color: Rgba) {
        self.canvas.set(x, y, color);
    }

    pub(crate) fn get(&self, x: i32, y: i32) -> Rgba {
        self.canvas.get(x, y)
    }

    /// The outline round a mask: every uncovered pixel beside a covered one.
    pub(crate) fn outline(&mut self, mask: &Mask, ink: Rgba) {
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
        let mask = Mask::of(shapes);
        self.outline(&mask, OUTLINE);
        for (x, y) in mask.pixels() {
            self.set(x, y, fill);
        }
        mask
    }

    /// One pixel line, for folds, scutes and feathers drawn onto what is already there.
    pub(crate) fn line(&mut self, a: (f32, f32), b: (f32, f32), color: Rgba, within: &Mask) {
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
