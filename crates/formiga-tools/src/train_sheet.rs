//! The colony's train, for review.
//!
//! One band per village palette, by day on pale wallpaper and after dark on a dark one: the train
//! standing with three carriages and the reference companions waiting beside it at the scale the
//! overlay draws them, then one and two carriages, then every frame of a run. Companions stand on
//! their own resting feet on the same ground as the wheels. A review sheet, not a test.
//!
//!   cargo run -p formiga-tools -- train-sheet [--output PATH]

use anyhow::Result;
use formiga_art::{
    BodyClip, Canvas, CreatureRenderer, MAX_CARS, PALETTES, TRAIN_FRAMES, TRAIN_GROUND,
    TRAIN_HEIGHT, TrainLook, TrainRenderer, door_centers, train_width,
};
use formiga_core::ActionKind;
use std::path::PathBuf;

const SCALE: u32 = 3;
const LABEL: u32 = 14;
const GAP: u32 = 10;
/// Room above the train for the tallest companion's head.
const ROW: u32 = TRAIN_HEIGHT + 10;

pub fn run(path: PathBuf) -> Result<()> {
    let companions = crate::reference_creatures();
    // Three villages: the reference colours the village sheet opens with.
    let villages = [(3_u8, 7_u8), (0, 5), (9, 2)];
    let longest = train_width(MAX_CARS);
    let width = (longest + GAP + 3 * 52 + GAP) * SCALE;
    let bands = villages.len() as u32 * 2 + 2;
    let height = bands * (ROW * SCALE + LABEL);
    let mut pixels = vec![0_u8; (width * height * 4) as usize];
    let mut band = 0_u32;
    let top = |band: &mut u32| {
        let y = *band * (ROW * SCALE + LABEL);
        *band += 1;
        y
    };
    for (body, accent) in villages {
        for lit in [false, true] {
            let y = top(&mut band);
            wallpaper(&mut pixels, width, y, ROW * SCALE + LABEL, lit);
            let look = TrainLook {
                body: PALETTES[usize::from(body)],
                accent: PALETTES[usize::from(accent)],
                lit,
            };
            label(
                &mut pixels,
                width,
                4,
                y + 3,
                &format!(
                    "VILLAGE {body}/{accent} {}",
                    if lit { "AFTER DARK" } else { "BY DAY" }
                ),
                lit,
            );
            let ground = y + LABEL + (ROW - (TRAIN_HEIGHT - TRAIN_GROUND)) * SCALE;
            let train = TrainRenderer::render(&look, MAX_CARS, 4);
            stamp(
                &mut pixels,
                width,
                &train,
                4,
                ground as i32 - ((TRAIN_GROUND + 1) * SCALE) as i32,
            );
            // A companion standing at each door, as one about to step in would.
            let doors = door_centers(MAX_CARS);
            let mut x = (longest + GAP) * SCALE;
            for (index, companion) in companions.iter().enumerate() {
                let frame = CreatureRenderer::render_body_frame(
                    &companion.appearance,
                    BodyClip::Action(ActionKind::Idle),
                    0,
                    false,
                );
                let feet = CreatureRenderer::resting_baseline(&companion.appearance, false);
                let canvas = frame.canvas;
                stamp(
                    &mut pixels,
                    width,
                    &canvas,
                    x as i32,
                    ground as i32 - ((formiga_art::FRAME_SIZE - feet) * SCALE) as i32,
                );
                x += 52 * SCALE;
                let _ = (index, &doors);
            }
        }
    }
    // One, two and three carriages, standing.
    let y = top(&mut band);
    wallpaper(&mut pixels, width, y, ROW * SCALE + LABEL, false);
    label(&mut pixels, width, 4, y + 3, "ONE AND TWO CARRIAGES", false);
    let look = TrainLook {
        body: PALETTES[3],
        accent: PALETTES[7],
        lit: false,
    };
    let ground = y + LABEL + (ROW - (TRAIN_HEIGHT - TRAIN_GROUND)) * SCALE;
    let mut x = 4_i32;
    for cars in 1..MAX_CARS {
        let train = TrainRenderer::render(&look, cars, 5);
        stamp(
            &mut pixels,
            width,
            &train,
            x,
            ground as i32 - ((TRAIN_GROUND + 1) * SCALE) as i32,
        );
        x += ((train_width(cars) + GAP) * SCALE) as i32;
    }
    // Every frame of one carriage: four running, two standing.
    let y = top(&mut band);
    wallpaper(&mut pixels, width, y, ROW * SCALE + LABEL, false);
    label(
        &mut pixels,
        width,
        4,
        y + 3,
        "FRAMES: RUNNING 0-3, STANDING 4-5",
        false,
    );
    let ground = y + LABEL + (ROW - (TRAIN_HEIGHT - TRAIN_GROUND)) * SCALE;
    let mut x = 4_i32;
    for frame in 0..TRAIN_FRAMES {
        let train = crop_engine(&TrainRenderer::render(&look, 1, frame));
        stamp(
            &mut pixels,
            width,
            &train,
            x,
            ground as i32 - ((TRAIN_GROUND + 1) * SCALE) as i32,
        );
        x += ((train.width() + 4) * SCALE) as i32;
    }
    crate::write_png(&path, width, height, &pixels)?;
    println!("wrote {}", path.display());
    Ok(())
}

/// The engine alone, for the frame strip.
fn crop_engine(train: &Canvas) -> Canvas {
    let width = formiga_art::ENGINE_WIDTH + 4;
    let mut cell = Canvas::new(width, train.height());
    for y in 0..train.height() as i32 {
        for x in 0..width as i32 {
            cell.set(x, y, train.get(x, y));
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
