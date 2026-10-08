//! The colony's train: Formiga Hill's own little tank engine and its two coaches, drawn here to the
//! same design and the same size against the companions as at the Hill's station, so the train
//! that leaves the desktop is the one that arrives there.
//!
//! A green engine lined in gold with the colony's crest on its side tank, a brass dome and a black
//! chimney, and two coaches in cream over maroon under grey roofs, each with a door at either end
//! and three windows between. It faces right, engine first, the way it runs; the overlay mirrors it
//! only if asked. After dark its compartments are lit.
//!
//! The frames are baked once for a trip: eight while it runs, the wheels and their rods turning a
//! full revolution and the steam trailing back, and two while it stands, puffing gently.

use crate::paint::mix_truncated;
use crate::{Canvas, Rgba};

/// The train itself, from the back of the last coach to the front buffers.
pub const TRAIN_WIDTH: u32 = 202;
/// Room above the train for its steam.
const HEADROOM: i32 = 24;
/// The train's own height, roof vents to rails.
const BODY_HEIGHT: i32 = 64;
/// The frame every picture of the train is drawn in, steam and all.
pub const TRAIN_HEIGHT: u32 = (HEADROOM + BODY_HEIGHT) as u32;
/// The row the train stands on, counted from the top of the frame: the bottom of its wheels.
pub const TRAIN_GROUND: u32 = (HEADROOM + AXLE + 3) as u32;
/// Frames while it runs: one revolution of the driving wheels.
pub const RUNNING_FRAMES: u8 = 8;
/// Every frame baked for the train: running, then two standing.
pub const TRAIN_FRAMES: u8 = RUNNING_FRAMES + 2;

const COACH_LENGTH: i32 = 66;
const COUPLING: i32 = 4;
const LOCO_X: i32 = 2 * (COACH_LENGTH + COUPLING);

// A coach, top to bottom, from the top of the train's own height.
const ROOF_TOP: i32 = 8;
const BODY_TOP: i32 = 14;
const BELT: i32 = 34;
const BODY_BOTTOM: i32 = 54;
const AXLE: i32 = 59;
const BUFFERS: i32 = 46;

const WINDOW_SIZE: (i32, i32) = (14, 13);
const WINDOW_TOP: i32 = 18;
const WINDOW_XS: [i32; 3] = [8, 26, 44];

/// One material's tones, darkest first. `edge` outlines it; `shine` is used a pixel at a time.
#[derive(Clone, Copy)]
struct Ramp {
    edge: Rgba,
    shadow: Rgba,
    base: Rgba,
    light: Rgba,
    shine: Rgba,
}

impl Ramp {
    const fn new(edge: u32, shadow: u32, base: u32, light: u32, shine: u32) -> Self {
        Self {
            edge: rgb(edge),
            shadow: rgb(shadow),
            base: rgb(base),
            light: rgb(light),
            shine: rgb(shine),
        }
    }
}

const fn rgb(hex: u32) -> Rgba {
    Rgba::new((hex >> 16) as u8, (hex >> 8) as u8, hex as u8, 255)
}

const fn rgba(hex: u32, alpha: u8) -> Rgba {
    Rgba::new((hex >> 16) as u8, (hex >> 8) as u8, hex as u8, alpha)
}

// The Hill's own livery.
const LOCO: Ramp = Ramp::new(0x163829, 0x21503b, 0x2d6b4e, 0x418a66, 0x62ab84);
const SOOT: Ramp = Ramp::new(0x111114, 0x1d1e23, 0x2a2c32, 0x40434b, 0x5d616b);
const BRASS: Ramp = Ramp::new(0x6b4a24, 0x9a7434, 0xc9a14e, 0xe4c06c, 0xf6e3a2);
const BUFFER_RED: Ramp = Ramp::new(0x5a1a1c, 0x8c2a2a, 0xb33a33, 0xd4564a, 0xea806c);
const CREAM: Ramp = Ramp::new(0x8a7a63, 0xd8c9a8, 0xf0e4c6, 0xfaf3e1, 0xffffff);
const MAROON: Ramp = Ramp::new(0x3a1418, 0x5c2026, 0x7a2c33, 0x963c42, 0xb4585a);
const ROOF: Ramp = Ramp::new(0x4a4950, 0x6d6c73, 0x8a8990, 0xa9a8ae, 0xcfced3);
const STEEL: Ramp = Ramp::new(0x3f4249, 0x5d626b, 0x80858e, 0xb2b7bf, 0xe3e7ec);
const LINING: Rgba = rgb(0xe4c06c);
const INTERIOR: Rgba = rgb(0x3a3440);
const INTERIOR_LOW: Rgba = rgb(0x5b4c4a);
/// A compartment after dark: lamplight, brightest under the lamp.
const LAMPLIT: Rgba = rgb(0xffecb2);
const LAMPLIT_LOW: Rgba = rgb(0xffce76);
const LAMP: Rgba = rgb(0xfff0bd);

/// A wheel of the train, relative to the train's own top left.
#[derive(Clone, Copy)]
struct Wheel {
    x: i32,
    y: i32,
    radius: i32,
}

const LOCO_WHEELS: [Wheel; 3] = [
    Wheel {
        x: LOCO_X + 14,
        y: AXLE - 4,
        radius: 7,
    },
    Wheel {
        x: LOCO_X + 29,
        y: AXLE - 4,
        radius: 7,
    },
    Wheel {
        x: LOCO_X + 44,
        y: AXLE - 4,
        radius: 7,
    },
];

/// Where each door of the two coaches is, across the train from its back end. A companion getting
/// on walks to one of these.
pub fn door_centers() -> [u32; 4] {
    let coach = |n: i32| n * (COACH_LENGTH + COUPLING);
    [
        coach(0) + 4,
        coach(0) + COACH_LENGTH - 4,
        coach(1) + 4,
        coach(1) + COACH_LENGTH - 4,
    ]
    .map(|x| x as u32)
}

/// How the train is lit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TrainLook {
    /// After dark, with its compartments lit.
    pub lit: bool,
}

/// The train's own drawing surface: everything is placed from the top of the train itself, below
/// the headroom left for steam, and every pixel is laid over what is already there.
struct Paint<'a> {
    canvas: &'a mut Canvas,
}

impl Paint<'_> {
    fn put(&mut self, x: i32, y: i32, color: Rgba) {
        if color.a == 0 {
            return;
        }
        let (x, y) = (x, y + HEADROOM);
        let under = self.canvas.get(x, y);
        let top_alpha = u32::from(color.a);
        let blended = match (color.a, under.a) {
            (255, _) | (_, 0) => color,
            (0, _) => under,
            _ => {
                let below = u32::from(under.a) * (255 - top_alpha);
                let total = top_alpha * 255 + below;
                let channel = |a: u8, b: u8| {
                    ((u32::from(a) * top_alpha * 255 + u32::from(b) * below) / total) as u8
                };
                Rgba::new(
                    channel(color.r, under.r),
                    channel(color.g, under.g),
                    channel(color.b, under.b),
                    (total / 255).min(255) as u8,
                )
            }
        };
        self.canvas.set(x, y, blended);
    }

    fn hline(&mut self, x: i32, y: i32, width: i32, color: Rgba) {
        for dx in 0..width {
            self.put(x + dx, y, color);
        }
    }

    fn vline(&mut self, x: i32, y: i32, height: i32, color: Rgba) {
        for dy in 0..height {
            self.put(x, y + dy, color);
        }
    }

    fn rect(&mut self, x: i32, y: i32, width: i32, height: i32, color: Rgba) {
        for dy in 0..height {
            self.hline(x, y + dy, width, color);
        }
    }

    fn ellipse(&mut self, cx: i32, cy: i32, rx: i32, ry: i32, color: Rgba) {
        if rx <= 0 || ry <= 0 {
            return;
        }
        let (rx2, ry2) = (i64::from(rx * rx), i64::from(ry * ry));
        for y in -ry..=ry {
            for x in -rx..=rx {
                if i64::from(x * x) * ry2 + i64::from(y * y) * rx2 <= rx2 * ry2 {
                    self.put(cx + x, cy + y, color);
                }
            }
        }
    }

    /// A box lit from the upper left, outlined in its material's own edge tone.
    fn bevel(&mut self, x: i32, y: i32, width: i32, height: i32, ramp: Ramp) {
        self.rect(x, y, width, height, ramp.edge);
        if width <= 2 || height <= 2 {
            return;
        }
        self.rect(x + 1, y + 1, width - 2, height - 2, ramp.base);
        self.hline(x + 1, y + 1, width - 2, ramp.light);
        self.vline(x + 1, y + 1, height - 2, ramp.light);
        self.hline(x + 1, y + height - 2, width - 2, ramp.shadow);
        self.vline(x + width - 2, y + 1, height - 2, ramp.shadow);
        self.put(x + 1, y + 1, ramp.shine);
    }

    fn line(&mut self, from: (i32, i32), to: (i32, i32), color: Rgba) {
        let ((mut x0, mut y0), (x1, y1)) = (from, to);
        let (dx, dy) = ((x1 - x0).abs(), -(y1 - y0).abs());
        let (sx, sy) = (if x0 < x1 { 1 } else { -1 }, if y0 < y1 { 1 } else { -1 });
        let mut error = dx + dy;
        loop {
            self.put(x0, y0, color);
            if (x0, y0) == (x1, y1) {
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

    /// A painted panel of the engine's green, lit on its top and left edges.
    fn panel(&mut self, x: i32, y: i32, width: i32, height: i32) {
        self.rect(x, y, width, height, LOCO.base);
        self.hline(x, y, width, LOCO.light);
        self.vline(x, y, height, LOCO.light);
        self.hline(x, y + height - 1, width, LOCO.shadow);
        self.vline(x + width - 1, y, height, LOCO.edge);
        self.put(x + 1, y + 1, LOCO.shine);
    }
}

fn coupling(paint: &mut Paint<'_>, x: i32) {
    paint.rect(x - 1, BUFFERS + 2, COUPLING + 2, 2, SOOT.base);
    paint.put(x + 1, BUFFERS + 2, SOOT.light);
    for side in [x - 1, x + COUPLING - 1] {
        paint.bevel(side, BUFFERS, 2, 5, SOOT);
    }
}

/// A compartment's light, from the top of a window to its bottom.
fn compartment(lit: bool, t: f32) -> Rgba {
    if lit {
        mix_truncated(LAMPLIT, LAMPLIT_LOW, t)
    } else {
        mix_truncated(INTERIOR, INTERIOR_LOW, t)
    }
}

fn coach(paint: &mut Paint<'_>, x: i32, lit: bool) {
    let length = COACH_LENGTH;
    // A curved roof with vents along it.
    paint.hline(x + 3, ROOF_TOP, length - 6, ROOF.edge);
    paint.hline(x + 1, ROOF_TOP + 1, length - 2, ROOF.shine);
    paint.hline(x, ROOF_TOP + 2, length, ROOF.light);
    for row in 3..5 {
        paint.hline(x, ROOF_TOP + row, length, ROOF.base);
    }
    paint.hline(x, ROOF_TOP + 5, length, ROOF.shadow);
    for vent in [x + 15, x + 33, x + 51] {
        paint.bevel(vent - 2, ROOF_TOP - 2, 5, 3, ROOF);
    }

    // Cream above the waist, maroon below, lined in gold.
    paint.rect(x, BODY_TOP, length, BELT - BODY_TOP, CREAM.base);
    paint.hline(x, BODY_TOP, length, CREAM.shadow);
    paint.hline(x, BODY_TOP + 1, length, CREAM.light);
    paint.rect(x, BELT, length, BODY_BOTTOM - BELT, MAROON.base);
    paint.hline(x, BELT, length, MAROON.edge);
    paint.hline(x, BELT + 1, length, MAROON.light);
    paint.hline(x + 2, BELT + 3, length - 4, LINING);
    paint.hline(x + 2, BODY_BOTTOM - 3, length - 4, LINING);
    paint.hline(x, BODY_BOTTOM - 1, length, MAROON.shadow);
    for panel in [x + 23, x + 41] {
        paint.vline(panel, BELT + 5, BODY_BOTTOM - BELT - 9, MAROON.shadow);
        paint.vline(panel + 1, BELT + 5, BODY_BOTTOM - BELT - 9, MAROON.light);
    }
    // Each end and each door is lined in its own band's tone, so no maroon line runs up through
    // the cream.
    for end in [x, x + length - 1] {
        paint.vline(end, BODY_TOP, BELT - BODY_TOP, CREAM.edge);
        paint.vline(end, BELT, BODY_BOTTOM - BELT, MAROON.edge);
    }

    // Doors at both ends, each with a droplight and a brass handle.
    for door in [x + 1, x + length - 7] {
        for side in [door, door + 6] {
            paint.vline(side, BODY_TOP + 2, BELT - BODY_TOP - 2, CREAM.shadow);
            paint.vline(side, BELT + 2, BODY_BOTTOM - BELT - 3, MAROON.edge);
        }
        paint.rect(door + 2, WINDOW_TOP, 3, 9, compartment(lit, 0.0));
        paint.put(door + 2, WINDOW_TOP, rgba(0xa6c8d2, 200));
        paint.put(door + 4, BELT + 6, BRASS.light);
        paint.put(door + 4, BELT + 5, BRASS.shine);
    }

    for column in WINDOW_XS {
        let (wx, wy, (width, height)) = (x + column, WINDOW_TOP, WINDOW_SIZE);
        // A frame, lit along its top, around a softly lit compartment, under glass.
        paint.rect(wx - 1, wy - 1, width + 2, height + 2, CREAM.edge);
        for row in 0..height {
            paint.hline(
                wx,
                wy + row,
                width,
                compartment(lit, row as f32 / height as f32),
            );
        }
        paint.hline(wx - 1, wy - 2, width + 2, CREAM.shine);
        paint.hline(wx - 1, wy + height + 1, width + 2, CREAM.shadow);
        glass(paint, wx, wy);
    }

    // Underframe, truss rods and the two bogies.
    paint.rect(x + 2, BODY_BOTTOM, length - 4, 3, SOOT.base);
    paint.hline(x + 2, BODY_BOTTOM, length - 4, SOOT.light);
    paint.line(
        (x + 22, BODY_BOTTOM + 2),
        (x + 28, BODY_BOTTOM + 5),
        SOOT.light,
    );
    paint.line(
        (x + 28, BODY_BOTTOM + 5),
        (x + 38, BODY_BOTTOM + 5),
        SOOT.light,
    );
    paint.line(
        (x + 38, BODY_BOTTOM + 5),
        (x + 44, BODY_BOTTOM + 2),
        SOOT.light,
    );
    for bogie in [x + 12, x + 54] {
        paint.rect(bogie - 7, AXLE - 3, 14, 3, SOOT.shadow);
        paint.hline(bogie - 7, AXLE - 3, 14, SOOT.light);
        paint.bevel(bogie - 2, AXLE - 3, 4, 4, SOOT);
    }
    // Buffers at each end.
    for buffer in [x - 1, x + length - 1] {
        paint.bevel(buffer, BUFFERS, 2, 4, STEEL);
    }
}

/// The glass over a window: a faint sheen and a glint.
fn glass(paint: &mut Paint<'_>, x: i32, y: i32) {
    let (width, height) = WINDOW_SIZE;
    paint.rect(x, y, width, height, rgba(0xa6c8d2, 30));
    paint.hline(x, y, width, rgba(0x2e2a33, 110));
    for (dx, dy) in [(1, 4), (2, 3), (3, 2), (4, 1)] {
        paint.put(x + width - 6 + dx, y + dy, rgba(0xffffff, 150));
    }
    paint.put(x + 1, y + height - 2, rgba(0xffffff, 60));
}

fn locomotive(paint: &mut Paint<'_>, x: i32, lit: bool) {
    let plate = BODY_BOTTOM - 4;
    // The cab, at the back, with a curved roof and a round-cornered window.
    paint.rect(x - 1, 4, 24, 2, SOOT.base);
    paint.hline(x, 3, 22, SOOT.light);
    paint.hline(x - 1, 6, 24, SOOT.edge);
    paint.panel(x, 7, 21, plate - 7);
    paint.rect(x + 5, 12, 11, 11, CREAM.edge);
    for row in 0..9 {
        paint.hline(x + 6, 13 + row, 9, compartment(lit, row as f32 / 9.0));
    }
    paint.put(x + 6, 13, CREAM.edge);
    paint.put(x + 14, 13, CREAM.edge);
    paint.rect(x + 6, 13, 9, 3, rgba(0xa6c8d2, 110));
    paint.put(x + 13, 14, rgba(0xffffff, 200));
    paint.put(x + 12, 15, rgba(0xffffff, 140));
    // A brass number plate on the cab side.
    paint.bevel(x + 5, 31, 11, 6, BRASS);
    paint.hline(x + 7, 33, 7, BRASS.shadow);

    // The boiler: a cylinder, lit along its top, banded in brass.
    let (boiler_left, boiler_right, boiler_top, boiler_bottom) = (x + 21, x + 52, 16, 42);
    for y in boiler_top..boiler_bottom {
        let t = (y - boiler_top) as f32 / (boiler_bottom - boiler_top) as f32;
        let color = match t {
            t if t < 0.06 => LOCO.edge,
            t if t < 0.14 => LOCO.shine,
            t if t < 0.34 => LOCO.light,
            t if t < 0.72 => LOCO.base,
            t if t < 0.93 => LOCO.shadow,
            _ => LOCO.edge,
        };
        paint.hline(boiler_left, y, boiler_right - boiler_left, color);
    }
    for band in [x + 29, x + 42] {
        paint.vline(
            band,
            boiler_top + 1,
            boiler_bottom - boiler_top - 2,
            BRASS.base,
        );
        paint.put(band, boiler_top + 2, BRASS.shine);
    }
    paint.hline(
        boiler_left + 1,
        boiler_top + 6,
        boiler_right - boiler_left - 2,
        BRASS.shadow,
    );

    // Side tanks over the lower half of the boiler, lined out in cream.
    paint.panel(x + 21, 27, 29, plate - 27);
    for y in [30, plate - 4] {
        paint.hline(x + 24, y, 23, CREAM.base);
    }
    for column in [x + 24, x + 46] {
        paint.vline(column, 30, plate - 33, CREAM.base);
    }
    // The colony's crest: a hill under a sun, in a ring.
    let (crest_x, crest_y) = (x + 35, 36);
    paint.ellipse(crest_x, crest_y, 4, 4, CREAM.base);
    paint.ellipse(crest_x, crest_y, 3, 3, LOCO.shadow);
    paint.ellipse(crest_x, crest_y + 3, 3, 2, LOCO.light);
    paint.put(crest_x + 1, crest_y - 2, BRASS.shine);
    paint.put(crest_x + 1, crest_y - 1, BRASS.light);

    // Dome and safety valve.
    paint.ellipse(x + 36, 15, 4, 4, BRASS.edge);
    paint.ellipse(x + 36, 15, 3, 3, BRASS.base);
    paint.rect(x + 32, 15, 9, 2, BRASS.base);
    paint.put(x + 35, 13, BRASS.shine);
    paint.put(x + 34, 14, BRASS.light);
    paint.bevel(x + 24, 11, 3, 6, BRASS);

    // Smokebox and chimney, in black.
    let (smoke_top, smoke_bottom) = (15, plate);
    for y in smoke_top..smoke_bottom {
        let t = (y - smoke_top) as f32 / (smoke_bottom - smoke_top) as f32;
        let color = if t < 0.08 {
            SOOT.light
        } else if t < 0.4 {
            SOOT.base
        } else if t < 0.9 {
            SOOT.shadow
        } else {
            SOOT.edge
        };
        paint.hline(x + 50, y, 8, color);
    }
    paint.vline(x + 57, smoke_top, smoke_bottom - smoke_top, SOOT.edge);
    for rivet in (smoke_top + 3..smoke_bottom - 2).step_by(4) {
        paint.put(x + 51, rivet, SOOT.light);
    }
    paint.rect(x + 51, 4, 6, 11, SOOT.base);
    paint.vline(x + 51, 4, 11, SOOT.light);
    paint.vline(x + 56, 4, 11, SOOT.edge);
    paint.rect(x + 50, 1, 8, 3, SOOT.base);
    paint.hline(x + 50, 1, 8, SOOT.light);
    paint.put(x + 51, 2, BRASS.light);
    // A lamp on the front.
    paint.bevel(x + 56, BUFFERS - 9, 4, 5, CREAM);
    paint.put(x + 57, BUFFERS - 8, LAMP);

    // Running plate, buffer beam, buffers and cylinders.
    paint.rect(x - 1, plate, 61, 2, SOOT.base);
    paint.hline(x - 1, plate, 61, SOOT.light);
    paint.hline(x, plate + 2, 58, BUFFER_RED.base);
    paint.bevel(x + 57, BUFFERS - 2, 4, 9, BUFFER_RED);
    paint.bevel(x + 59, BUFFERS, 3, 4, STEEL);
    paint.bevel(x - 2, BUFFERS, 3, 4, STEEL);
    paint.bevel(x + 49, plate + 1, 10, 7, SOOT);
    paint.hline(x + 50, plate + 2, 8, SOOT.light);
    paint.put(x + 57, plate + 4, STEEL.light);
}

fn wheel(paint: &mut Paint<'_>, wheel: Wheel, rolled: f32, spokes: usize, ramp: Ramp) {
    let (cx, cy, radius) = (wheel.x, wheel.y, wheel.radius);
    let angle = rolled / radius as f32;
    paint.ellipse(cx, cy, radius, radius, SOOT.edge);
    paint.ellipse(cx, cy, radius - 1, radius - 1, ramp.base);
    if radius > 3 {
        paint.ellipse(cx, cy, radius - 2, radius - 2, ramp.shadow);
    }
    for spoke in 0..spokes {
        let a = angle + spoke as f32 * std::f32::consts::TAU / spokes as f32;
        let reach = (radius - 1) as f32;
        let tip = (
            cx + (a.cos() * reach).round() as i32,
            cy + (a.sin() * reach).round() as i32,
        );
        paint.line((cx, cy), tip, ramp.light);
    }
    // The steel tyre catches the light along its top.
    for dx in -radius / 2..=radius / 2 {
        paint.put(cx + dx, cy - radius, STEEL.light);
    }
    paint.put(cx, cy, BRASS.light);
}

/// The coupling rod joining the driving wheels, and the connecting rod from the cylinder.
fn rods(paint: &mut Paint<'_>, rolled: f32) {
    let radius = LOCO_WHEELS[0].radius;
    let angle = rolled / radius as f32;
    let crank = (radius - 3) as f32;
    let pin = |wheel: Wheel| {
        (
            wheel.x + (angle.cos() * crank).round() as i32,
            wheel.y + (angle.sin() * crank).round() as i32,
        )
    };
    let (first, middle, last) = (
        pin(LOCO_WHEELS[0]),
        pin(LOCO_WHEELS[1]),
        pin(LOCO_WHEELS[2]),
    );
    paint.line((first.0, first.1 + 1), (last.0, last.1 + 1), STEEL.shadow);
    paint.line(first, last, STEEL.light);
    let crosshead = (LOCO_X + 55, BUFFERS + 4);
    paint.line(crosshead, middle, STEEL.base);
    for point in [first, middle, last] {
        paint.put(point.0, point.1, BRASS.shine);
    }
}

/// Steam from the chimney: six puffs at their ages through `now`, trailing back as far as the
/// train's pace carries them.
fn steam(paint: &mut Paint<'_>, now: f32, speed: f32) {
    const PUFFS: usize = 6;
    let chimney = (LOCO_X + 54, 1);
    for index in 0..PUFFS {
        let age = (now * 1.3 + index as f32 / PUFFS as f32).fract();
        let trail = 4.0 + speed.max(0.0) * 0.4;
        let x = chimney.0 as f32 - age * trail;
        let y = chimney.1 as f32 - 2.0 - age * 16.0;
        let radius = (1.5 + age * 4.0).round() as i32;
        let fade = ((1.0 - age) * 200.0) as u8;
        let (cx, cy) = (x.round() as i32, y.round() as i32);
        paint.ellipse(cx + 1, cy + 1, radius, radius, rgba(0xbdb8bc, fade / 2));
        paint.ellipse(cx, cy, radius, radius, rgba(0xf7f5f2, fade));
    }
}

pub struct TrainRenderer;

impl TrainRenderer {
    /// One frame of the train, facing right. Frames below [`RUNNING_FRAMES`] are it running, the
    /// wheels a step further round each frame; the rest are it standing.
    pub fn render(look: &TrainLook, frame: u8) -> Canvas {
        let frame = frame % TRAIN_FRAMES;
        let mut canvas = Canvas::new(TRAIN_WIDTH, TRAIN_HEIGHT);
        let mut paint = Paint {
            canvas: &mut canvas,
        };
        let running = frame < RUNNING_FRAMES;
        // Running, one frame is an eighth of a turn of the driving wheels.
        let turn = std::f32::consts::TAU * LOCO_WHEELS[0].radius as f32;
        let rolled = if running {
            f32::from(frame) * turn / f32::from(RUNNING_FRAMES)
        } else {
            0.0
        };
        // Shade under the train, among its wheels.
        paint.rect(
            2,
            BODY_BOTTOM + 2,
            TRAIN_WIDTH as i32 - 4,
            7,
            rgba(0x1e1a1e, 70),
        );
        coach(&mut paint, 0, look.lit);
        coupling(&mut paint, COACH_LENGTH);
        coach(&mut paint, COACH_LENGTH + COUPLING, look.lit);
        coupling(&mut paint, LOCO_X - COUPLING);
        locomotive(&mut paint, LOCO_X, look.lit);
        for coach_x in [0, COACH_LENGTH + COUPLING] {
            for wheel_x in [8, 16, 50, 58] {
                let at = Wheel {
                    x: coach_x + wheel_x,
                    y: AXLE,
                    radius: 3,
                };
                wheel(&mut paint, at, rolled, 2, STEEL);
            }
        }
        for at in LOCO_WHEELS {
            wheel(&mut paint, at, rolled, 6, LOCO);
        }
        rods(&mut paint, rolled);
        if running {
            steam(
                &mut paint,
                f32::from(frame) / f32::from(RUNNING_FRAMES),
                40.0,
            );
        } else {
            steam(&mut paint, f32::from(frame - RUNNING_FRAMES) * 0.38, 0.0);
        }
        canvas
    }

    /// Every frame side by side, left to right: what the overlay bakes once for a trip.
    pub fn render_strip(look: &TrainLook) -> Canvas {
        let mut strip = Canvas::new(TRAIN_WIDTH * u32::from(TRAIN_FRAMES), TRAIN_HEIGHT);
        for frame in 0..TRAIN_FRAMES {
            let picture = Self::render(look, frame);
            let offset = (u32::from(frame) * TRAIN_WIDTH) as i32;
            for y in 0..TRAIN_HEIGHT as i32 {
                for x in 0..TRAIN_WIDTH as i32 {
                    let color = picture.get(x, y);
                    if color.a != 0 {
                        strip.set(offset + x, y, color);
                    }
                }
            }
        }
        strip
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY: TrainLook = TrainLook { lit: false };

    #[test]
    fn the_train_fills_its_frame_and_stands_on_the_ground() {
        for frame in 0..TRAIN_FRAMES {
            let canvas = TrainRenderer::render(&DAY, frame);
            let (left, top, right, bottom) = canvas.alpha_bounds().unwrap();
            assert!(left <= 2, "the last coach reaches the back: {left}");
            assert!(
                right + 1 >= TRAIN_WIDTH - 1,
                "the buffers reach the front: {right}"
            );
            assert!(top >= 1, "the steam stays inside the frame: {top}");
            assert_eq!(bottom, TRAIN_GROUND, "the wheels stand on the ground");
        }
    }

    #[test]
    fn a_companion_getting_on_goes_wholly_behind_the_coach() {
        // A companion stands on the same ground as the wheels; the roof stands taller than any
        // companion's frame, so one at a door is hidden by the coach rather than peering over it.
        let roof_rise = TRAIN_GROUND as i32 - (HEADROOM + ROOF_TOP);
        assert!(
            roof_rise >= crate::FRAME_SIZE as i32,
            "the roof rises {roof_rise}"
        );
    }

    #[test]
    fn every_door_is_on_its_coach() {
        let canvas = TrainRenderer::render(&DAY, RUNNING_FRAMES);
        for door in door_centers() {
            assert!(door < TRAIN_WIDTH);
            let at_door = canvas.get(door as i32, HEADROOM + BELT + 6);
            assert_eq!(at_door.a, 255, "the door at {door} is solid");
        }
    }

    #[test]
    fn standing_still_the_wheels_hold_and_the_steam_breathes() {
        let a = TrainRenderer::render(&DAY, RUNNING_FRAMES);
        let b = TrainRenderer::render(&DAY, RUNNING_FRAMES + 1);
        assert_ne!(a.pixels(), b.pixels());
        let below_steam = |canvas: &Canvas| -> Vec<Rgba> {
            (HEADROOM + BODY_BOTTOM..=TRAIN_GROUND as i32)
                .flat_map(|y| (0..TRAIN_WIDTH as i32).map(move |x| (x, y)))
                .map(|(x, y)| canvas.get(x, y))
                .collect()
        };
        assert_eq!(below_steam(&a), below_steam(&b), "the wheels hold still");
        let running: Vec<_> = (0..RUNNING_FRAMES)
            .map(|frame| below_steam(&TrainRenderer::render(&DAY, frame)))
            .collect();
        for pair in running.windows(2) {
            assert_ne!(pair[0], pair[1], "running, the wheels turn every frame");
        }
    }

    #[test]
    fn after_dark_the_compartments_are_lit() {
        let warm = |canvas: &Canvas| {
            canvas
                .pixels()
                .iter()
                .filter(|p| p.a == 255 && p.r > 230 && p.g > 180 && p.b < 200)
                .count()
        };
        let day = TrainRenderer::render(&DAY, RUNNING_FRAMES);
        let night = TrainRenderer::render(&TrainLook { lit: true }, RUNNING_FRAMES);
        assert!(warm(&night) > warm(&day) + 600);
    }

    #[test]
    fn the_strip_holds_every_frame_in_order() {
        let strip = TrainRenderer::render_strip(&DAY);
        assert_eq!(strip.width(), TRAIN_WIDTH * u32::from(TRAIN_FRAMES));
        for frame in 0..TRAIN_FRAMES {
            let single = TrainRenderer::render(&DAY, frame);
            for y in 0..TRAIN_HEIGHT as i32 {
                for x in 0..TRAIN_WIDTH as i32 {
                    assert_eq!(
                        strip.get((u32::from(frame) * TRAIN_WIDTH) as i32 + x, y),
                        single.get(x, y)
                    );
                }
            }
        }
    }

    #[test]
    fn the_strip_fits_any_texture_the_overlay_may_make() {
        // Downlevel GPUs guarantee 2048 texels a side.
        assert!(TRAIN_WIDTH * u32::from(TRAIN_FRAMES) <= 2048);
    }
}
