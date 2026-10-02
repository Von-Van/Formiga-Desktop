//! A review sheet for the train: a departure and a homecoming composited from the very scene the
//! overlay plays, with the colony's real sprites and the train drawn over them as the overlay
//! draws it, so a person can see the staging without sending a colony anywhere.
//!
//! ```sh
//! FORMIGA_TRAIN_REVIEW_DIR=/tmp/formiga-train cargo test -p formiga-desktop --bin formiga \
//!     train_review
//! ```
//!
//! `train-departure.png` and `train-homecoming.png` each hold ten moments of the scene from top to
//! bottom, evenly spread over its length, on the bottom strip of a 1440-point display at Medium
//! (3x) and 1x scale. Four companions start on the floor at both ends, on a ledge, and on a display
//! off to the left. Without the environment variable nothing is written; the scene is still played
//! and drawn, so the test keeps the composition code honest.

use super::*;
use formiga_art::{
    AccessoryArt, BodyPresentation, Canvas, CreatureRenderer, FramePlacement, Rgba, TrainLook,
    TrainRenderer, palette_for,
};
use formiga_core::{DesktopSnapshot, DisplayKey, World};
use time::macros::datetime;

const MOMENTS: u32 = 10;
/// The strip of the display shown: the bottom of it, where the ground is.
const STRIP: u32 = 230;

fn colony() -> (SaveFile, Vec<MonitorInfo>) {
    let monitor = |id: MonitorId, x: f32, primary: bool| MonitorInfo {
        id,
        display_key: DisplayKey([id as u8; 16]),
        bounds: DesktopRect {
            x,
            y: 0.0,
            width: 1440.0,
            height: 900.0,
        },
        usable_bounds: DesktopRect {
            x,
            y: 24.0,
            width: 1440.0,
            height: 826.0,
        },
        scale_factor: 1.0,
        primary,
    };
    let monitors = vec![monitor(1, 0.0, true), monitor(2, -1440.0, false)];
    let desktop = DesktopSnapshot {
        monitors: monitors.clone(),
        ..DesktopSnapshot::default()
    };
    let now = datetime!(2026-10-02 9:00 UTC);
    let mut world = World::new([8; 32], now, &desktop);
    for seed in 9..12 {
        world
            .add_designed_adult([seed; 32], None, now, &desktop)
            .unwrap();
    }
    let mut save = world.save.clone();
    save.settings.display_scale = 3;
    save.home.display = Some(monitors[0].display_key);
    let ground = monitors[0].usable_bounds.bottom() - 4.0;
    let spots = [
        (1, SurfaceKind::ScreenFloor, 120.0, ground),
        (1, SurfaceKind::ScreenFloor, 1320.0, ground),
        (1, SurfaceKind::WindowLedge, 900.0, 400.0),
        (2, SurfaceKind::ScreenFloor, -700.0, ground),
    ];
    for (creature, (monitor_id, kind, x, y)) in save.creatures.iter_mut().zip(spots) {
        creature.state.position = Point { x, y };
        creature.state.surface = SurfaceAttachment {
            kind,
            monitor_id,
            window_key: None,
            relative_x: 0.5,
        };
        creature.state.action = ActionKind::Idle;
    }
    (save, monitors)
}

/// Copy `source` onto `sheet` magnified `scale` times, its top left at `(x, y)`, mirrored if asked.
fn stamp(sheet: &mut Canvas, source: &Canvas, x: i32, y: i32, scale: i32, mirrored: bool) {
    let width = source.width() as i32;
    for sy in 0..source.height() as i32 {
        for sx in 0..width {
            let pixel = source.get(if mirrored { width - 1 - sx } else { sx }, sy);
            if pixel.a == 0 {
                continue;
            }
            for oy in 0..scale {
                for ox in 0..scale {
                    sheet.set(x + sx * scale + ox, y + sy * scale + oy, pixel);
                }
            }
        }
    }
}

/// One moment of the scene: the platform's copy of the colony, then the train over it.
fn moment(sheet: &mut Canvas, scene: &TrainScene, save: &SaveFile, top: i32, night: bool) {
    let scale = i32::from(save.settings.display_scale);
    let strip_top = 850.0 - STRIP as f32;
    let paper = Rgba::new(226, 222, 214, 255);
    for y in top..top + STRIP as i32 {
        for x in 0..sheet.width() as i32 {
            sheet.set(
                x,
                y,
                if y == top {
                    Rgba::new(150, 146, 140, 255)
                } else {
                    paper
                },
            );
        }
    }
    let to_sheet_y = |desktop_y: f32| top + (desktop_y - strip_top).round() as i32;
    let staged = scene.stage(save);
    let members: Vec<_> = save
        .creatures
        .iter()
        .map(|creature| palette_for(&creature.appearance))
        .collect();
    for creature in staged
        .creatures
        .iter()
        .filter(|creature| creature.state.surface.monitor_id == scene.monitor_id())
    {
        let presentation = BodyPresentation::for_creature(creature);
        let dress = creature
            .accessory
            .map(|accessory| AccessoryArt::resolve(accessory, save.colony_seed, &members));
        let canvas = CreatureRenderer::render_dressed_frame(
            &creature.appearance,
            dress,
            creature.state.action,
            presentation.frame,
            presentation.facing_right,
        );
        let baseline = CreatureRenderer::resting_baseline(&creature.appearance, false);
        let placement = FramePlacement::for_creature(creature, baseline);
        let x = creature.state.position.x.round() as i32 - FRAME_SIZE as i32 * scale / 2;
        let y = to_sheet_y(creature.state.position.y) + placement.origin_y * scale;
        stamp(sheet, &canvas, x, y, scale, false);
    }
    if let Some(train) = scene.train() {
        let look = TrainLook::of(&save.home, night);
        let canvas = TrainRenderer::render(&look, train.cars, train.frame);
        let y = to_sheet_y(train.ground) - TRAIN_RISE as i32 * scale;
        stamp(
            sheet,
            &canvas,
            train.left.round() as i32,
            y,
            scale,
            train.facing_right,
        );
    }
}

fn sheet_of(mut scene: TrainScene, save: &SaveFile, night: bool) -> Canvas {
    let mut sheet = Canvas::new(1440, STRIP * MOMENTS);
    let step = scene.length() / (MOMENTS - 1) as f32;
    for index in 0..MOMENTS {
        moment(&mut sheet, &scene, save, (index * STRIP) as i32, night);
        scene.advance(step);
    }
    sheet
}

fn save_review(filename: &str, sheet: &Canvas) {
    if let Some(directory) = std::env::var_os("FORMIGA_TRAIN_REVIEW_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let file = std::fs::File::create(directory.join(filename)).unwrap();
        let mut encoder = png::Encoder::new(file, sheet.width(), sheet.height());
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(&sheet.rgba_bytes())
            .unwrap();
    }
}

#[test]
fn train_review() {
    let (save, monitors) = colony();
    let leaving = sheet_of(
        TrainScene::departure(&save, &monitors).unwrap(),
        &save,
        false,
    );
    let coming_home = sheet_of(TrainScene::arrival(&save, &monitors).unwrap(), &save, true);
    // Every moment shows something: a train, a companion, or both.
    for sheet in [&leaving, &coming_home] {
        // The first and last moments may be empty: the train is still off the display, or gone.
        for index in 1..MOMENTS - 1 {
            let band = (index * STRIP + 1)..((index + 1) * STRIP);
            let drawn = band
                .flat_map(|y| (0..sheet.width()).map(move |x| (x, y)))
                .any(|(x, y)| sheet.get(x as i32, y as i32) != Rgba::new(226, 222, 214, 255));
            assert!(drawn, "moment {index} is empty");
        }
    }
    save_review("train-departure.png", &leaving);
    save_review("train-homecoming.png", &coming_home);
}
