//! A new companion arriving, for review: played by the colony itself, tick by tick, and drawn a
//! moment at a time down two columns.
//!
//! On the left, adopted while the colony is out on the desktop: it walks in along the floor from
//! beyond the nearer edge of the display, which is drawn as a line with the world past it shaded.
//! On the right, adopted with the village out between visits: it comes out of its own house door,
//! standing as far back as its house does, and walks on out, coming down to the front of the grass
//! as it goes. The door is drawn as a post from the ground up to how far back the house stands;
//! the houses themselves are on the `village-depth-sheet`. A review sheet, not a test.
//!
//!   cargo run -p formiga-tools -- arrival-sheet [--output PATH]

use crate::{blend_pixel, write_png};
use anyhow::Result;
use formiga_art::{BodyPresentation, Canvas, CreatureRenderer, FRAME_SIZE, FramePlacement};
use formiga_core::{
    Creature, CreatureId, CursorSnapshot, DesktopRect, DesktopSnapshot, DisplayKey, MonitorInfo,
    World, colony_cottage_list, home_dwelling_position, house_slot_for,
};
use std::path::PathBuf;
use time::macros::datetime;

/// Desktop points shown across each moment, and down it: the bottom of the display.
const SPAN: f32 = 480.0;
const STRIP: u32 = 150;
/// Moments down each column, and the time between them.
const MOMENTS: u32 = 10;
const EVERY_SECS: f32 = 0.5;
const LABEL: u32 = 14;
const PAPER: [u8; 4] = [234, 230, 222, 255];
const BEYOND: [u8; 4] = [200, 196, 190, 255];
const INK: [u8; 4] = [70, 60, 56, 255];
const DOOR: [u8; 4] = [150, 96, 64, 255];

fn display() -> DesktopSnapshot {
    DesktopSnapshot {
        monitors: vec![MonitorInfo {
            id: 1,
            display_key: DisplayKey([1; 16]),
            bounds: DesktopRect {
                x: 0.0,
                y: 0.0,
                width: 1440.0,
                height: 900.0,
            },
            usable_bounds: DesktopRect {
                x: 0.0,
                y: 24.0,
                width: 1440.0,
                height: 826.0,
            },
            scale_factor: 1.0,
            primary: true,
        }],
        ..DesktopSnapshot::default()
    }
}

pub fn run(path: PathBuf) -> Result<()> {
    let desktop = display();
    let width = (SPAN as u32) * 2 + 12;
    let height = LABEL + STRIP * MOMENTS;
    let mut pixels = vec![0_u8; (width * height * 4) as usize];
    fill(&mut pixels, width, 0, 0, width, height, PAPER);
    for (column, village_out) in [false, true].into_iter().enumerate() {
        let left = column as u32 * (SPAN as u32 + 12);
        crate::social_preview::draw_text(
            &mut pixels,
            width as i32,
            height as i32,
            left as i32 + 4,
            3,
            if village_out {
                "OUT OF ITS DOOR, THE VILLAGE OUT"
            } else {
                "IN FROM THE NEARER EDGE"
            },
            1,
            INK,
        );
        let (mut world, id, door) = arrival(village_out, &desktop);
        // Edge on: the strip from a little beyond the left edge. Door: the door near the end the
        // newcomer walks away from, so the whole walk out is in view.
        let leftward = world
            .save
            .creatures
            .iter()
            .any(|c| c.id == id && !c.state.facing_right);
        let view_left = door.map_or(-120.0, |(x, _)| x - SPAN * if leftward { 0.8 } else { 0.2 });
        let mut t = 0.0;
        for index in 0..MOMENTS {
            let top = LABEL + index * STRIP;
            moment(&mut pixels, width, (left, top), view_left, &world, id, door);
            let until = t + EVERY_SECS;
            while t < until - 0.001 {
                t += 0.05;
                let now = datetime!(2026-01-01 9:20 UTC) + time::Duration::seconds_f32(t);
                world.tick(now, 0.05, &desktop);
                world.drain_events().for_each(drop);
            }
        }
    }
    write_png(&path, width, height, &pixels)?;
    println!("wrote {}", path.display());
    Ok(())
}

/// A colony out on the desktop, its own arrivals all made, and one more companion adopted into
/// it: with the village out between visits or not. Also where the newcomer's door is and how far
/// back, in desktop points, when it comes out of one.
fn arrival(
    village_out: bool,
    desktop: &DesktopSnapshot,
) -> (World, CreatureId, Option<(f32, f32)>) {
    let created = datetime!(2026-01-01 9:00 UTC);
    let mut world = World::new([41; 32], created, desktop);
    world.save.settings.display_scale = 2;
    world.save.home.stays_out = village_out;
    world.tick(created, 0.05, desktop);
    let now = datetime!(2026-01-01 9:20 UTC);
    world.save.home.active_since_utc = None;
    world.save.home.last_disappeared_utc = Some(now);
    world.save.arrival_state.arrived = [true; 3];
    for creature in &mut world.save.creatures {
        creature.mini_arrivals.arrived = [true; 2];
    }
    world.tick(now, 0.05, desktop);
    // The founder out of the way, at the far end of the display.
    for creature in &mut world.save.creatures {
        creature.state.position.x = 1300.0;
    }
    let id = world
        .add_designed_adult([42; 32], None, now, desktop)
        .expect("room for one more");
    let door = village_out.then(|| {
        let creatures = &world.save.creatures;
        let newcomer = creatures.iter().find(|c| c.id == id).unwrap();
        let slot = house_slot_for(newcomer, creatures, &world.save.home.cottage_order);
        let (_, at) = home_dwelling_position(
            &world.save.home,
            slot,
            colony_cottage_list(creatures).as_slice(),
            &desktop.monitors,
            &world.save.settings.habitat,
            world.save.settings.display_scale,
        )
        .expect("a house");
        let depth = world.save.home.house_depth_list(creatures)[slot];
        (at.x, f32::from(depth) * points_per_pixel(&world))
    });
    (world, id, door)
}

fn points_per_pixel(world: &World) -> f32 {
    f32::from(world.save.settings.display_scale)
}

/// One moment: the bottom of the display from `view_left`, the newcomer on it, and the edge or
/// the door it came by.
fn moment(
    pixels: &mut [u8],
    width: u32,
    (left, top): (u32, u32),
    view_left: f32,
    world: &World,
    id: CreatureId,
    door: Option<(f32, f32)>,
) {
    let ground = 846.0;
    let strip_top = ground - STRIP as f32 + 12.0;
    let to_x = |x: f32| left as i32 + (x - view_left).round() as i32;
    let to_y = |y: f32| top as i32 + (y - strip_top).round() as i32;
    // Off the display: shaded, with the edge as a line.
    if view_left < 0.0 {
        let edge = to_x(0.0);
        fill(
            pixels,
            width,
            left,
            top,
            (edge - left as i32).max(0) as u32,
            STRIP,
            BEYOND,
        );
        fill(pixels, width, edge as u32, top, 1, STRIP, INK);
    }
    // The ground, and the door as a post as tall as its house stands back.
    fill(
        pixels,
        width,
        left,
        to_y(ground) as u32,
        SPAN as u32,
        1,
        INK,
    );
    if let Some((x, back)) = door {
        let post = back.max(2.0) as u32;
        fill(
            pixels,
            width,
            to_x(x) as u32,
            (to_y(ground) - post as i32) as u32,
            2,
            post,
            DOOR,
        );
    }
    let Some(creature) = world.save.creatures.iter().find(|c| c.id == id) else {
        return;
    };
    stamp_creature(pixels, width, creature, world, to_x, to_y, (left, top));
    fill(pixels, width, left, top, SPAN as u32, 1, INK);
}

fn stamp_creature(
    pixels: &mut [u8],
    width: u32,
    creature: &Creature,
    world: &World,
    to_x: impl Fn(f32) -> i32,
    to_y: impl Fn(f32) -> i32,
    (left, top): (u32, u32),
) {
    let scale = i32::from(world.save.settings.display_scale);
    let presentation = BodyPresentation::for_creature(creature);
    let face = CreatureRenderer::resolve_face_state(creature, CursorSnapshot::default(), false);
    let canvas: Canvas = CreatureRenderer::render_composited_frame(
        &creature.appearance,
        presentation.clip,
        presentation.frame,
        presentation.facing_right,
        false,
        face,
    );
    let baseline = CreatureRenderer::resting_baseline(&creature.appearance, false);
    let placement = FramePlacement::for_creature(creature, baseline);
    let at = creature.state.drawn_position(points_per_pixel(world));
    let x0 = to_x(at.x) - FRAME_SIZE as i32 * scale / 2;
    let y0 = to_y(at.y) + placement.origin_y * scale;
    let (right, bottom) = (left as i32 + SPAN as i32, top as i32 + STRIP as i32);
    for sy in 0..canvas.height() as i32 {
        for sx in 0..canvas.width() as i32 {
            let pixel = canvas.get(sx, sy);
            if pixel.a == 0 {
                continue;
            }
            for oy in 0..scale {
                for ox in 0..scale {
                    let (x, y) = (x0 + sx * scale + ox, y0 + sy * scale + oy);
                    if x < left as i32 || x >= right || y < top as i32 || y >= bottom {
                        continue;
                    }
                    blend_pixel(
                        pixels,
                        width,
                        x as u32,
                        y as u32,
                        [pixel.r, pixel.g, pixel.b, pixel.a],
                    );
                }
            }
        }
    }
}

fn fill(pixels: &mut [u8], width: u32, x: u32, y: u32, w: u32, h: u32, color: [u8; 4]) {
    let height = (pixels.len() / 4) as u32 / width;
    for py in y..(y + h).min(height) {
        for px in x..(x + w).min(width) {
            let index = ((py * width + px) * 4) as usize;
            pixels[index..index + 4].copy_from_slice(&color);
        }
    }
}
