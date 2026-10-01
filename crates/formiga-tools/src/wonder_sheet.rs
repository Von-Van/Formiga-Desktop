//! Every wonder, and companions playing on it.
//!
//! For each kind: a band with every frame of the wonder alone, then its script played out at a
//! run of moments with the companions the simulation would put there — the reference blob, hopper
//! and soft quadruped taking turns as the lead, and a second companion beside it where the wonder
//! takes two. Each companion is placed exactly as the simulation places it, from the same script,
//! and seated on its own resting feet as the overlay seats it. A review sheet, not a test.
//!
//!   cargo run -p formiga-tools -- wonder-sheet [--output PATH]

use anyhow::Result;
use formiga_art::{
    BodyClip, Canvas, CreatureRenderer, FRAME_SIZE, WONDER_CELL_HEIGHT, WONDER_CELL_WIDTH,
    WONDER_GROUND, WONDER_MIDDLE, WonderRenderer, wonder_frame, wonder_frames,
};
use formiga_core::{
    ActionKind, AppearanceGenome, WonderKind, WonderSeats, wonder_length, wonder_motion,
    wonder_poses,
};
use std::path::PathBuf;

const SCALE: u32 = 2;
const MOMENTS: u32 = 6;
/// Wide enough for a bike's whole ride and a tightrope's watcher at the far end.
const ROOM: u32 = 200;
const PAD: i32 = ((ROOM - WONDER_CELL_WIDTH) / 2) as i32;
const CELL_W: u32 = ROOM * SCALE;
const CELL_H: u32 = WONDER_CELL_HEIGHT * SCALE;
const LABEL: u32 = 14;

pub fn run(path: PathBuf) -> Result<()> {
    let subjects: Vec<AppearanceGenome> = crate::reference_creatures()
        .into_iter()
        .map(|creature| creature.appearance)
        .collect();
    let colony_seed = [61_u8; 32];
    let rows_for = |kind: WonderKind| match kind.seats() {
        WonderSeats::One => 1,
        WonderSeats::OneOrTwo => 2,
        WonderSeats::Two => 1,
    };
    let rows: u32 = WonderKind::ALL
        .into_iter()
        .map(|kind| 1 + rows_for(kind))
        .sum();
    let width = CELL_W * MOMENTS;
    let height = rows * (CELL_H + LABEL);
    let mut pixels = vec![0_u8; (width * height * 4) as usize];
    // Pale wallpaper, banded so each wonder's rows read together.
    for (index, chunk) in pixels.chunks_exact_mut(4).enumerate() {
        let y = index as u32 / width;
        let band = y / (CELL_H + LABEL);
        let shade = if band.is_multiple_of(2) { 236 } else { 226 };
        chunk.copy_from_slice(&[shade, shade - 2, shade - 8, 255]);
    }
    let mut row = 0_u32;
    for (kind_index, kind) in WonderKind::ALL.into_iter().enumerate() {
        let sprite = WonderRenderer::render(kind, colony_seed);
        // The frames alone.
        let top = row * (CELL_H + LABEL);
        label(
            &mut pixels,
            width,
            4,
            top + 3,
            &format!("{} frames", kind.label()).to_uppercase(),
        );
        for frame in 0..wonder_frames(kind).min(ROOM * MOMENTS / WONDER_CELL_WIDTH) {
            let cell = cut(&sprite, frame);
            stamp(
                &mut pixels,
                width,
                &cell,
                (frame * WONDER_CELL_WIDTH * SCALE) as i32,
                (top + LABEL) as i32,
            );
        }
        row += 1;
        let length = wonder_length(kind);
        for players in match kind.seats() {
            WonderSeats::One => vec![1],
            WonderSeats::OneOrTwo => vec![1, 2],
            WonderSeats::Two => vec![2],
        } {
            let top = row * (CELL_H + LABEL);
            label(
                &mut pixels,
                width,
                4,
                top + 3,
                &format!(
                    "{} {}",
                    kind.label(),
                    if players == 1 { "alone" } else { "for two" }
                )
                .to_uppercase(),
            );
            let lead = &subjects[kind_index % subjects.len()];
            let partner = &subjects[(kind_index + 1) % subjects.len()];
            for moment in 0..MOMENTS {
                let t = 0.35 + (length - 0.7) * moment as f32 / (MOMENTS - 1) as f32;
                let toss = moment.is_multiple_of(2);
                let (motion, ride) = wonder_motion(kind, t, toss);
                let frame = wonder_frame(kind, t, motion);
                let mut cell = Canvas::new(ROOM, WONDER_CELL_HEIGHT);
                let ride = ride.round() as i32;
                paste(&mut cell, &cut(&sprite, frame), PAD + ride, 0);
                for (index, pose) in wonder_poses(kind, players, t, toss).into_iter().enumerate() {
                    let genome = if index == 0 { lead } else { partner };
                    let clip: BodyClip = pose
                        .gesture
                        .map_or(BodyClip::Action(pose.action), BodyClip::Gesture);
                    let spec = formiga_art::AnimationSpec::for_clip(clip);
                    let clip_frame =
                        ((t * f32::from(spec.fps)) as u32 % u32::from(spec.frames)) as u8;
                    let face = match pose.gesture {
                        Some(gesture) => crate::gesture_face(gesture),
                        None if pose.action == ActionKind::Sleep => {
                            crate::gesture_face(formiga_core::Gesture::Yawn)
                        }
                        None => crate::gesture_face(formiga_core::Gesture::Watch),
                    };
                    let body = CreatureRenderer::render_composited_frame(
                        genome,
                        clip,
                        clip_frame,
                        pose.facing_right,
                        false,
                        face,
                    );
                    let baseline = CreatureRenderer::resting_baseline(genome, false) as i32;
                    let contact_x = PAD + WONDER_MIDDLE + pose.dx.round() as i32;
                    let contact_y = WONDER_GROUND + pose.dy.round() as i32;
                    paste(
                        &mut cell,
                        &body,
                        contact_x - FRAME_SIZE as i32 / 2,
                        contact_y + baseline - FRAME_SIZE as i32,
                    );
                }
                stamp(
                    &mut pixels,
                    width,
                    &cell,
                    (moment * CELL_W) as i32,
                    (top + LABEL) as i32,
                );
                label(
                    &mut pixels,
                    width,
                    moment * CELL_W + CELL_W - 40,
                    top + 3,
                    &format!("{t:.1}S"),
                );
            }
            row += 1;
        }
    }
    crate::write_png(&path, width, height, &pixels)?;
    println!("wrote {}", path.display());
    Ok(())
}

/// A small caption in the sheets' own lettering.
fn label(pixels: &mut [u8], width: u32, x: u32, y: u32, text: &str) {
    let height = pixels.len() as i32 / 4 / width as i32;
    crate::social_preview::draw_text(
        pixels,
        width as i32,
        height,
        x as i32,
        y as i32,
        text,
        1,
        [70, 60, 56, 255],
    );
}

/// One frame of a wonder's strip.
fn cut(sheet: &Canvas, frame: u32) -> Canvas {
    let mut cell = Canvas::new(WONDER_CELL_WIDTH, WONDER_CELL_HEIGHT);
    paste(&mut cell, sheet, -((frame * WONDER_CELL_WIDTH) as i32), 0);
    cell
}

/// Draws `source` over `target` with its top left at `(x, y)`, clipped to the target.
fn paste(target: &mut Canvas, source: &Canvas, x: i32, y: i32) {
    for sy in 0..source.height() as i32 {
        for sx in 0..source.width() as i32 {
            let (tx, ty) = (x + sx, y + sy);
            if tx < 0 || ty < 0 || tx >= target.width() as i32 || ty >= target.height() as i32 {
                continue;
            }
            let pixel = source.get(sx, sy);
            if pixel.a > 0 {
                target.set(tx, ty, pixel);
            }
        }
    }
}

/// Nearest-neighbour, as the overlay magnifies.
fn stamp(pixels: &mut [u8], width: u32, canvas: &Canvas, x: i32, y: i32) {
    for sy in 0..canvas.height() as i32 {
        for sx in 0..canvas.width() as i32 {
            let pixel = canvas.get(sx, sy);
            if pixel.a == 0 {
                continue;
            }
            for oy in 0..SCALE as i32 {
                for ox in 0..SCALE as i32 {
                    let (px, py) = (x + sx * SCALE as i32 + ox, y + sy * SCALE as i32 + oy);
                    if px < 0 || py < 0 || px as u32 >= width {
                        continue;
                    }
                    crate::blend_pixel(
                        pixels,
                        width,
                        px as u32,
                        py as u32,
                        [pixel.r, pixel.g, pixel.b, pixel.a],
                    );
                }
            }
        }
    }
}
