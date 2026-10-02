//! The colony's little train: an engine and one to three carriages that take the colony to Formiga
//! Hill and bring it home again.
//!
//! It is drawn the way the houses are: in the village's own palette, each material with a base, a
//! shade and a light, one light from the upper left, and one soft outline round the whole thing,
//! so it reads as a single toy rather than a stack of boxes. It faces left, engine first; the
//! overlay mirrors it to run the other way. Nothing on it outshines the faces getting on.

use crate::{Canvas, PALETTES, Palette, Rgba};

/// The frame every picture of the train is drawn in, smoke and all.
pub const TRAIN_HEIGHT: u32 = 50;
/// The row the train stands on, counted from the top of the frame: the outline under its wheels.
pub const TRAIN_GROUND: u32 = TRAIN_HEIGHT - 1;
/// Where the wheels meet the ground; the outline is the row below.
const WHEEL_BASE: u32 = TRAIN_GROUND - 1;
pub const ENGINE_WIDTH: u32 = 48;
pub const CAR_WIDTH: u32 = 42;
/// The gap a coupling spans between two parts of the train.
pub const COUPLING: u32 = 3;
pub const MAX_CARS: u8 = 3;
/// The frames baked for the train: four while it runs, wheels turning and smoke trailing, and two
/// while it stands, puffing gently.
pub const TRAIN_FRAMES: u8 = 6;
pub const RUNNING_FRAMES: u8 = 4;

/// How many carriages a colony of `travelers` fills: two to a carriage.
pub fn cars_for(travelers: usize) -> u8 {
    travelers.div_ceil(2).clamp(1, usize::from(MAX_CARS)) as u8
}

/// How wide the train is with `cars` carriages.
pub fn train_width(cars: u8) -> u32 {
    let cars = u32::from(cars.clamp(1, MAX_CARS));
    ENGINE_WIDTH + cars * (COUPLING + CAR_WIDTH)
}

/// Where each carriage's door is, from the engine's end of the train, facing left.
pub fn door_centers(cars: u8) -> Vec<u32> {
    (0..u32::from(cars.clamp(1, MAX_CARS)))
        .map(|car| car_left(car) + CAR_WIDTH / 2)
        .collect()
}

fn car_left(car: u32) -> u32 {
    ENGINE_WIDTH + COUPLING + car * (CAR_WIDTH + COUPLING)
}

/// The colours the train is painted in.
#[derive(Clone, Copy, Debug)]
pub struct TrainLook {
    /// The village's own palette: the carriages and the cab.
    pub body: Palette,
    /// The village's accent: the boiler, the roofs and the doors.
    pub accent: Palette,
    /// After dark the windows are lit.
    pub lit: bool,
}

impl TrainLook {
    /// The train in this colony's colours: whatever its village is painted in.
    pub fn of(home: &formiga_core::ColonyHome, lit: bool) -> Self {
        let genome = home.drawn_shelter();
        Self {
            body: PALETTES[genome.palette_index as usize % PALETTES.len()],
            accent: PALETTES[genome.accent_index as usize % PALETTES.len()],
            lit,
        }
    }
}

const WHITE: Rgba = Rgba::new(255, 252, 240, 255);
const BRASS: Rgba = Rgba::new(222, 176, 92, 255);
const IRON: Rgba = Rgba::new(66, 58, 74, 255);
const GLASS: Rgba = Rgba::new(40, 46, 64, 255);
const GLINT: Rgba = Rgba::new(118, 132, 158, 255);
const GLOW: Rgba = Rgba::new(255, 206, 118, 255);
const GLOW_CORE: Rgba = Rgba::new(255, 236, 178, 255);
const STEAM: Rgba = Rgba::new(246, 242, 232, 230);

fn mix(a: Rgba, b: Rgba, t: f32) -> Rgba {
    let channel = |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * t).round() as u8;
    Rgba::new(channel(a.r, b.r), channel(a.g, b.g), channel(a.b, b.b), 255)
}

/// One material: its colour, the same in shade, and where the light catches it.
#[derive(Clone, Copy)]
struct Tone {
    base: Rgba,
    shade: Rgba,
    light: Rgba,
}

impl Tone {
    fn of(base: Rgba, ink: Rgba) -> Self {
        Self {
            base,
            shade: mix(base, ink, 0.3),
            light: mix(base, WHITE, 0.38),
        }
    }
}

struct Paints {
    outline: Rgba,
    body: Tone,
    accent: Tone,
    trim: Tone,
    iron: Tone,
    brass: Tone,
    lit: bool,
}

impl Paints {
    fn of(look: &TrainLook) -> Self {
        let ink = look.body.outline;
        Self {
            outline: mix(ink, IRON, 0.25),
            body: Tone::of(look.body.coat, ink),
            accent: Tone::of(look.accent.coat, ink),
            trim: Tone::of(mix(look.accent.coat, ink, 0.35), ink),
            iron: Tone::of(IRON, ink),
            brass: Tone::of(BRASS, ink),
            lit: look.lit,
        }
    }
}

/// A rectangle with its corners rounded off by `radius`, filled in a material lit from the upper
/// left: a light rim along the top, shade along the bottom and the right.
fn rounded_block(canvas: &mut Canvas, x: i32, y: i32, w: i32, h: i32, radius: i32, tone: Tone) {
    let radius = radius.min((w - 1) / 2).min((h - 1) / 2).max(0);
    let inside = |px: i32, py: i32| {
        let cx = px.clamp(x + radius, x + w - 1 - radius);
        let cy = py.clamp(y + radius, y + h - 1 - radius);
        let (dx, dy) = (px - cx, py - cy);
        dx * dx + dy * dy <= radius * radius
    };
    for py in y..y + h {
        for px in x..x + w {
            if !inside(px, py) {
                continue;
            }
            // The top two rows of every column catch the light, and the left edge one column of it.
            let rim = !inside(px, py - 2) || (!inside(px - 1, py) && py < y + h / 2);
            let color = if rim && py < y + h - 2 {
                tone.light
            } else if py >= y + h - 2 || px >= x + w - 2 {
                tone.shade
            } else {
                tone.base
            };
            canvas.set(px, py, color);
        }
    }
}

/// A window: dark glass with a glint in its upper left corner, or lamplight after dark.
fn window(canvas: &mut Canvas, x: i32, y: i32, w: i32, h: i32, lit: bool) {
    let (pane, shine) = if lit {
        (GLOW, GLOW_CORE)
    } else {
        (GLASS, GLINT)
    };
    for py in y..y + h {
        for px in x..x + w {
            let corner = (px == x || px == x + w - 1) && (py == y || py == y + h - 1);
            if !corner {
                canvas.set(px, py, pane);
            }
        }
    }
    canvas.set(x + 1, y + 1, shine);
    canvas.set(x + 2, y + 1, shine);
    canvas.set(x + 1, y + 2, shine);
}

/// A wheel standing on the ground at `cx`, turned to `phase` of four quarter turns: a dark tyre,
/// a lighter disc, a hub, and one spoke to show it going round.
fn wheel(canvas: &mut Canvas, paints: &Paints, cx: i32, phase: u8) -> (i32, i32) {
    let ground = WHEEL_BASE as i32;
    let cy = ground - 5;
    canvas.fill_circle(cx, cy, 5, paints.iron.shade);
    canvas.fill_circle(cx, cy, 4, paints.iron.base);
    canvas.fill_circle(cx, cy, 2, paints.iron.light);
    canvas.set(cx, cy, paints.brass.base);
    let (dx, dy) = [(3, 0), (0, 3), (-3, 0), (0, -3)][usize::from(phase % 4)];
    canvas.set(cx + dx, cy + dy, paints.brass.light);
    canvas.set(cx + dx / 3 * 2, cy + dy / 3 * 2, paints.brass.base);
    (cx + dx, cy + dy)
}

fn engine(canvas: &mut Canvas, paints: &Paints, phase: u8, moving: bool) {
    let g = WHEEL_BASE as i32;
    // The boiler, long and round, in the accent; a darker smokebox at its nose.
    rounded_block(canvas, 5, g - 25, 28, 15, 6, paints.accent);
    rounded_block(canvas, 3, g - 24, 6, 13, 3, paints.trim);
    // Bands round the boiler.
    for band in [14, 23] {
        for y in g - 24..g - 11 {
            canvas.set(band, y, paints.accent.shade);
        }
    }
    // The chimney, flared at the top, and the brass dome.
    rounded_block(canvas, 9, g - 32, 5, 8, 1, paints.iron);
    rounded_block(canvas, 8, g - 34, 7, 3, 1, paints.iron);
    rounded_block(canvas, 18, g - 28, 6, 4, 2, paints.brass);
    // The cab, in the village's own colour, with a roof in the accent and a window.
    rounded_block(canvas, 30, g - 30, 16, 21, 2, paints.body);
    rounded_block(canvas, 28, g - 33, 20, 4, 2, paints.trim);
    window(canvas, 34, g - 27, 8, 7, paints.lit);
    // A lamp on the nose: brass, and lit after dark or while it runs.
    rounded_block(canvas, 1, g - 21, 4, 4, 1, paints.brass);
    canvas.set(
        2,
        g - 20,
        if paints.lit || moving {
            GLOW_CORE
        } else {
            paints.brass.light
        },
    );
    // The footplate along the bottom, and the cowcatcher in front of it.
    rounded_block(canvas, 3, g - 11, 44, 4, 1, paints.iron);
    for row in 0..5 {
        for x in 0..(5 - row) {
            canvas.set(x + 1, g - 7 + row, paints.iron.base);
        }
    }
    // Three wheels, and the rod that turns them together.
    let pins: Vec<_> = [12, 24, 37]
        .into_iter()
        .map(|cx| wheel(canvas, paints, cx, phase))
        .collect();
    for pair in pins.windows(2) {
        let ((x0, y0), (x1, _)) = (pair[0], pair[1]);
        for x in x0..=x1 {
            canvas.set(x, y0, paints.brass.shade);
        }
    }
}

fn carriage(canvas: &mut Canvas, paints: &Paints, x0: i32, phase: u8) {
    let g = WHEEL_BASE as i32;
    let w = CAR_WIDTH as i32;
    // One rounded body in the village's colour under a roof in the accent.
    rounded_block(canvas, x0 + 1, g - 30, w - 2, 23, 4, paints.body);
    rounded_block(canvas, x0, g - 33, w, 5, 2, paints.accent);
    // A window either side of the door.
    window(canvas, x0 + 5, g - 25, 8, 7, paints.lit);
    window(canvas, x0 + w - 13, g - 25, 8, 7, paints.lit);
    // The door in the middle, with a round window and a brass knob.
    let door = x0 + w / 2 - 5;
    rounded_block(canvas, door, g - 28, 10, 21, 2, paints.trim);
    canvas.fill_circle(door + 5, g - 23, 2, if paints.lit { GLOW } else { GLASS });
    canvas.set(door + 4, g - 24, if paints.lit { GLOW_CORE } else { GLINT });
    canvas.set(door + 8, g - 16, paints.brass.light);
    // The underframe, and two wheels.
    rounded_block(canvas, x0 + 2, g - 10, w - 4, 3, 1, paints.iron);
    wheel(canvas, paints, x0 + 9, phase);
    wheel(canvas, paints, x0 + w - 10, phase);
}

fn coupling(canvas: &mut Canvas, paints: &Paints, x0: i32) {
    let g = WHEEL_BASE as i32;
    for x in x0 - 1..x0 + COUPLING as i32 + 1 {
        canvas.set(x, g - 9, paints.iron.shade);
        canvas.set(x, g - 8, paints.iron.base);
    }
}

/// A soft puff of steam, a little see-through at the edge.
fn puff(canvas: &mut Canvas, cx: i32, cy: i32, radius: i32) {
    canvas.fill_circle(cx, cy, radius, Rgba::new(STEAM.r, STEAM.g, STEAM.b, 170));
    canvas.fill_circle(cx, cy, (radius - 1).max(1), STEAM);
    canvas.set(cx - radius / 2, cy - radius / 2, WHITE);
}

fn steam(canvas: &mut Canvas, frame: u8) {
    let top = WHEEL_BASE as i32 - 36;
    if frame < RUNNING_FRAMES {
        // Running: puffs trail back over the carriages and grow as they go.
        let drift = i32::from(frame) * 3;
        puff(canvas, 12 + drift / 2, top - 1, 2);
        puff(canvas, 20 + drift, top - 4, 3);
        puff(canvas, 31 + drift, top - 7, 4);
    } else {
        // Standing: one small puff, breathing in and out.
        let size = if frame == RUNNING_FRAMES { 2 } else { 3 };
        puff(canvas, 11, top - 2, size);
    }
}

/// The outline round the whole train: one line in the village's ink, full where a side touches,
/// soft at a corner, so the engine and its carriages read as one shape.
fn outline(canvas: &mut Canvas, ink: Rgba) {
    let (width, height) = (canvas.width() as i32, canvas.height() as i32);
    let solid = |canvas: &Canvas, x: i32, y: i32| canvas.get(x, y).a > 200;
    let mut edges = Vec::new();
    for y in 0..height {
        for x in 0..width {
            if canvas.get(x, y).a != 0 {
                continue;
            }
            let cardinal = [(1, 0), (-1, 0), (0, 1), (0, -1)]
                .into_iter()
                .any(|(dx, dy)| solid(canvas, x + dx, y + dy));
            let diagonal = [(1, 1), (1, -1), (-1, 1), (-1, -1)]
                .into_iter()
                .any(|(dx, dy)| solid(canvas, x + dx, y + dy));
            if cardinal {
                edges.push((x, y, 255));
            } else if diagonal {
                edges.push((x, y, 110));
            }
        }
    }
    for (x, y, alpha) in edges {
        canvas.set(x, y, Rgba::new(ink.r, ink.g, ink.b, alpha));
    }
}

pub struct TrainRenderer;

impl TrainRenderer {
    /// One frame of the train with `cars` carriages, facing left. Frames below
    /// [`RUNNING_FRAMES`] are it running; the rest are it standing.
    pub fn render(look: &TrainLook, cars: u8, frame: u8) -> Canvas {
        let cars = cars.clamp(1, MAX_CARS);
        let frame = frame % TRAIN_FRAMES;
        let moving = frame < RUNNING_FRAMES;
        let phase = if moving { frame } else { 0 };
        let paints = Paints::of(look);
        let mut canvas = Canvas::new(train_width(cars), TRAIN_HEIGHT);
        engine(&mut canvas, &paints, phase, moving);
        for car in 0..u32::from(cars) {
            let left = car_left(car) as i32;
            coupling(&mut canvas, &paints, left - COUPLING as i32);
            carriage(&mut canvas, &paints, left, phase);
        }
        outline(&mut canvas, paints.outline);
        // Steam is drawn after the outline: it is weather, not part of the toy.
        steam(&mut canvas, frame);
        canvas
    }

    /// Every frame side by side, left to right: what the overlay bakes once for a trip.
    pub fn render_strip(look: &TrainLook, cars: u8) -> Canvas {
        let cars = cars.clamp(1, MAX_CARS);
        let width = train_width(cars);
        let mut strip = Canvas::new(width * u32::from(TRAIN_FRAMES), TRAIN_HEIGHT);
        for frame in 0..TRAIN_FRAMES {
            let picture = Self::render(look, cars, frame);
            let offset = (u32::from(frame) * width) as i32;
            for y in 0..TRAIN_HEIGHT as i32 {
                for x in 0..width as i32 {
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

    fn look() -> TrainLook {
        TrainLook {
            body: PALETTES[3],
            accent: PALETTES[7],
            lit: false,
        }
    }

    #[test]
    fn a_carriage_for_every_two_travelers() {
        assert_eq!(cars_for(1), 1);
        assert_eq!(cars_for(2), 1);
        assert_eq!(cars_for(3), 2);
        assert_eq!(cars_for(6), 3);
        assert_eq!(cars_for(12), MAX_CARS);
    }

    #[test]
    fn the_train_fills_its_frame_and_stands_on_the_ground() {
        for cars in 1..=MAX_CARS {
            for frame in 0..TRAIN_FRAMES {
                let canvas = TrainRenderer::render(&look(), cars, frame);
                let (left, top, right, bottom) = canvas.alpha_bounds().unwrap();
                assert!(left <= 1, "the nose reaches the front: {left}");
                assert!(
                    right + 2 >= canvas.width() - 1,
                    "the last carriage reaches the back"
                );
                assert!(top >= 1, "the steam stays inside the frame");
                assert_eq!(bottom, TRAIN_GROUND, "the wheels stand on the ground");
            }
        }
    }

    #[test]
    fn every_door_is_on_its_carriage() {
        for cars in 1..=MAX_CARS {
            let doors = door_centers(cars);
            assert_eq!(doors.len(), usize::from(cars));
            let canvas = TrainRenderer::render(&look(), cars, 4);
            for door in doors {
                assert!(door < train_width(cars));
                let at_door = canvas.get(door as i32, WHEEL_BASE as i32 - 16);
                assert_eq!(at_door.a, 255, "the door at {door} is solid");
            }
        }
    }

    #[test]
    fn standing_still_the_wheels_do_not_turn_and_the_steam_breathes() {
        let a = TrainRenderer::render(&look(), 2, RUNNING_FRAMES);
        let b = TrainRenderer::render(&look(), 2, RUNNING_FRAMES + 1);
        assert_ne!(a.pixels(), b.pixels());
        let below_steam = |canvas: &Canvas| -> Vec<Rgba> {
            (WHEEL_BASE as i32 - 12..=TRAIN_GROUND as i32)
                .flat_map(|y| (0..canvas.width() as i32).map(move |x| (x, y)))
                .map(|(x, y)| canvas.get(x, y))
                .collect()
        };
        assert_eq!(below_steam(&a), below_steam(&b), "the wheels hold still");
        let running: Vec<_> = (0..RUNNING_FRAMES)
            .map(|frame| below_steam(&TrainRenderer::render(&look(), 2, frame)))
            .collect();
        assert_ne!(running[0], running[1], "running, the wheels turn");
    }

    #[test]
    fn after_dark_the_windows_are_lit() {
        let day = TrainRenderer::render(&look(), 1, 4);
        let night = TrainRenderer::render(
            &TrainLook {
                lit: true,
                ..look()
            },
            1,
            4,
        );
        let glowing = |canvas: &Canvas| canvas.pixels().iter().filter(|p| **p == GLOW).count();
        assert!(glowing(&night) > glowing(&day) + 40);
    }

    #[test]
    fn the_strip_holds_every_frame_in_order() {
        let strip = TrainRenderer::render_strip(&look(), 2);
        let width = train_width(2);
        assert_eq!(strip.width(), width * u32::from(TRAIN_FRAMES));
        for frame in 0..TRAIN_FRAMES {
            let single = TrainRenderer::render(&look(), 2, frame);
            for y in 0..TRAIN_HEIGHT as i32 {
                for x in 0..width as i32 {
                    assert_eq!(
                        strip.get((u32::from(frame) * width) as i32 + x, y),
                        single.get(x, y)
                    );
                }
            }
        }
    }
}
