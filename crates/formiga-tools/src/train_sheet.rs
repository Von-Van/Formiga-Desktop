//! The colony's train, for review.
//!
//! Formiga Hill's own engine and two coaches: standing by day on pale wallpaper and after dark on
//! a dark one, with the reference companions waiting beside it at the scale the overlay draws
//! them, standing on the same ground as the wheels; then the engine through every frame, running
//! and standing. A review sheet, not a test.
//!
//!   cargo run -p formiga-tools -- train-sheet [--output PATH]

use anyhow::Result;
use formiga_art::{
    BodyClip, Canvas, CreatureRenderer, FRAME_SIZE, RUNNING_FRAMES, TRAIN_FRAMES, TRAIN_GROUND,
    TRAIN_HEIGHT, TRAIN_WIDTH, TrainLook, TrainRenderer,
};
use formiga_core::ActionKind;
use std::path::PathBuf;

const SCALE: u32 = 3;
const LABEL: u32 = 14;
const GAP: u32 = 10;
/// How much of the engine end each frame of the strip shows.
const ENGINE: u32 = 70;

pub fn run(path: PathBuf) -> Result<()> {
    let companions = crate::reference_creatures();
    let band = TRAIN_HEIGHT * SCALE + LABEL;
    let standing_width = (TRAIN_WIDTH + GAP + companions.len() as u32 * (FRAME_SIZE + 4)) * SCALE;
    let frames_width = u32::from(TRAIN_FRAMES) * (ENGINE + 4) * SCALE;
    let width = standing_width.max(frames_width) + 8;
    let height = band * 3;
    let mut pixels = vec![0_u8; (width * height * 4) as usize];
    for (row, lit) in [false, true].into_iter().enumerate() {
        let top = row as u32 * band;
        wallpaper(&mut pixels, width, top, band, lit);
        label(
            &mut pixels,
            width,
            4,
            top + 3,
            if lit {
                "STANDING, AFTER DARK"
            } else {
                "STANDING, BY DAY"
            },
            lit,
        );
        let ground = top + LABEL + (TRAIN_GROUND + 1) * SCALE;
        let train = TrainRenderer::render(&TrainLook { lit }, RUNNING_FRAMES);
        stamp(
            &mut pixels,
            width,
            &train,
            4,
            (ground - (TRAIN_GROUND + 1) * SCALE) as i32,
        );
        // The reference companions waiting on the same ground, as they would at a door.
        let mut x = (TRAIN_WIDTH + GAP) * SCALE;
        for companion in &companions {
            let frame = CreatureRenderer::render_body_frame(
                &companion.appearance,
                BodyClip::Action(ActionKind::Idle),
                0,
                false,
            );
            let feet = CreatureRenderer::resting_baseline(&companion.appearance, false);
            stamp(
                &mut pixels,
                width,
                &frame.canvas,
                x as i32,
                (ground - (FRAME_SIZE - feet) * SCALE) as i32,
            );
            x += (FRAME_SIZE + 4) * SCALE;
        }
    }
    // Every frame of the engine end: running 0 to 7, standing 8 and 9.
    let top = 2 * band;
    wallpaper(&mut pixels, width, top, band, false);
    label(
        &mut pixels,
        width,
        4,
        top + 3,
        "FRAMES: RUNNING 0-7, STANDING 8-9",
        false,
    );
    let mut x = 4_i32;
    for frame in 0..TRAIN_FRAMES {
        let train = TrainRenderer::render(&TrainLook { lit: false }, frame);
        let engine = crop(&train, TRAIN_WIDTH - ENGINE, ENGINE);
        stamp(&mut pixels, width, &engine, x, (top + LABEL) as i32);
        x += ((ENGINE + 4) * SCALE) as i32;
    }
    crate::write_png(&path, width, height, &pixels)?;
    println!("wrote {}", path.display());
    Ok(())
}

/// A slice of the train, `width` wide from `left`.
fn crop(train: &Canvas, left: u32, width: u32) -> Canvas {
    let mut cell = Canvas::new(width, train.height());
    for y in 0..train.height() as i32 {
        for x in 0..width as i32 {
            cell.set(x, y, train.get(left as i32 + x, y));
        }
    }
    cell
}

fn wallpaper(pixels: &mut [u8], width: u32, top: u32, rows: u32, dark: bool) {
    let shade = if dark {
        [52, 50, 64, 255]
    } else {
        [234, 230, 222, 255]
    };
    for y in top..top + rows {
        for x in 0..width {
            let index = ((y * width + x) * 4) as usize;
            if index + 4 <= pixels.len() {
                pixels[index..index + 4].copy_from_slice(&shade);
            }
        }
    }
}

fn label(pixels: &mut [u8], width: u32, x: u32, y: u32, text: &str, dark: bool) {
    let height = pixels.len() as i32 / 4 / width as i32;
    crate::social_preview::draw_text(
        pixels,
        width as i32,
        height,
        x as i32,
        y as i32,
        text,
        1,
        if dark {
            [220, 214, 200, 255]
        } else {
            [70, 60, 56, 255]
        },
    );
}

/// Nearest-neighbour, as the overlay magnifies.
fn stamp(pixels: &mut [u8], width: u32, canvas: &Canvas, x: i32, y: i32) {
    let height = (pixels.len() / 4) as u32 / width;
    for sy in 0..canvas.height() as i32 {
        for sx in 0..canvas.width() as i32 {
            let pixel = canvas.get(sx, sy);
            if pixel.a == 0 {
                continue;
            }
            for oy in 0..SCALE as i32 {
                for ox in 0..SCALE as i32 {
                    let (px, py) = (x + sx * SCALE as i32 + ox, y + sy * SCALE as i32 + oy);
                    if px < 0 || py < 0 || px as u32 >= width || py as u32 >= height {
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
