use anyhow::{Context, Result, bail};
use formiga_art::{CreatureRenderer, FRAME_SIZE};
use formiga_core::*;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use time::OffsetDateTime;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("contact-sheet") => contact_sheet(output_argument(&args)),
        Some("animation-preview") => animation_preview(
            output_argument_with_default(&args, "animation-preview.png"),
            seed_argument(&args),
        ),
        Some("simulate") => simulate(
            args.get(2)
                .and_then(|value| value.parse().ok())
                .unwrap_or(181),
        ),
        _ => {
            eprintln!(
                "usage:\n  formiga-tools contact-sheet [--output PATH]\n  formiga-tools animation-preview [--seed NUMBER] [--output PATH]\n  formiga-tools simulate [DAYS]"
            );
            Ok(())
        }
    }
}

fn output_argument(args: &[String]) -> PathBuf {
    output_argument_with_default(args, "contact-sheet.png")
}

fn output_argument_with_default(args: &[String], default: &str) -> PathBuf {
    args.windows(2)
        .find(|window| window[0] == "--output")
        .map(|window| PathBuf::from(&window[1]))
        .unwrap_or_else(|| PathBuf::from(default))
}

fn seed_argument(args: &[String]) -> u64 {
    args.windows(2)
        .find(|window| window[0] == "--seed")
        .and_then(|window| window[1].parse().ok())
        .unwrap_or(17)
}

fn animation_preview(path: PathBuf, seed_number: u64) -> Result<()> {
    const COLS: u32 = 6;
    const SCALE: u32 = 3;
    let rows = ActionKind::ALL.len() as u32;
    let cell = FRAME_SIZE * SCALE;
    let width = COLS * cell;
    let height = rows * cell;
    let mut pixels = vec![0_u8; (width * height * 4) as usize];
    let mut seed = [0_u8; 32];
    seed.copy_from_slice(&Sha256::digest(seed_number.to_le_bytes()));
    let world = World::new(seed, OffsetDateTime::UNIX_EPOCH, &fixture_desktop());
    let creature = &world.save.creatures[0];
    for (row, action) in ActionKind::ALL.into_iter().enumerate() {
        let spec = formiga_art::AnimationSpec::for_action(action);
        for frame in 0..spec.frames {
            let rendered =
                CreatureRenderer::render_frame(&creature.appearance, action, frame, true);
            blit_scaled(
                &mut pixels,
                width,
                u32::from(frame) * cell,
                row as u32 * cell,
                &rendered.rgba_bytes(),
                SCALE,
            );
        }
    }
    write_png(&path, width, height, &pixels)?;
    println!(
        "wrote {} for seed {} ({:?})",
        path.display(),
        seed_number,
        creature.appearance.family
    );
    Ok(())
}

fn contact_sheet(path: PathBuf) -> Result<()> {
    const COLS: u32 = 10;
    const ROWS: u32 = 10;
    const SCALE: u32 = 3;
    let cell = FRAME_SIZE * SCALE;
    let mut pixels = vec![0_u8; (COLS * cell * ROWS * cell * 4) as usize];
    let desktop = fixture_desktop();
    for index in 0..COLS * ROWS {
        let mut root = [0_u8; 32];
        root.copy_from_slice(&Sha256::digest(format!("formiga-contact-{index}")));
        let world = World::new(root, OffsetDateTime::UNIX_EPOCH, &desktop);
        let creature = &world.save.creatures[0];
        let frame = CreatureRenderer::render_frame(
            &creature.appearance,
            ActionKind::Idle,
            (index % 4) as u8,
            true,
        );
        blit_scaled(
            &mut pixels,
            COLS * cell,
            index % COLS * cell,
            index / COLS * cell,
            &frame.rgba_bytes(),
            SCALE,
        );
    }
    write_png(&path, COLS * cell, ROWS * cell, &pixels)?;
    println!("wrote {}", path.display());
    Ok(())
}

fn simulate(days: i64) -> Result<()> {
    if days < 0 {
        bail!("days must be non-negative");
    }
    let desktop = fixture_desktop();
    let created = OffsetDateTime::UNIX_EPOCH;
    let mut world = World::new([17; 32], created, &desktop);
    for day in 0..=days {
        let now = created + time::Duration::days(day);
        for _ in 0..1_200 {
            world.tick(now, 0.05, &desktop);
            world.drain_events().for_each(drop);
        }
    }
    println!(
        "simulated {days} days; colony contains {} creature(s)",
        world.save.creatures.len()
    );
    for creature in &world.save.creatures {
        println!(
            "- {}: {:?}, generation {}, {:?}",
            creature.id, creature.appearance.family, creature.generation, creature.state.action
        );
    }
    Ok(())
}

fn fixture_desktop() -> DesktopSnapshot {
    DesktopSnapshot {
        monitors: vec![MonitorInfo {
            id: 1,
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
            scale_factor: 2.0,
            primary: true,
        }],
        cursor: CursorSnapshot {
            position: Point { x: 720.0, y: 420.0 },
            velocity: Point::default(),
            available: true,
        },
        ..DesktopSnapshot::default()
    }
}

fn blit_scaled(
    target: &mut [u8],
    target_width: u32,
    origin_x: u32,
    origin_y: u32,
    source: &[u8],
    scale: u32,
) {
    for sy in 0..FRAME_SIZE {
        for sx in 0..FRAME_SIZE {
            let source_index = ((sy * FRAME_SIZE + sx) * 4) as usize;
            for oy in 0..scale {
                for ox in 0..scale {
                    let x = origin_x + sx * scale + ox;
                    let y = origin_y + sy * scale + oy;
                    let target_index = ((y * target_width + x) * 4) as usize;
                    target[target_index..target_index + 4]
                        .copy_from_slice(&source[source_index..source_index + 4]);
                }
            }
        }
    }
}

fn write_png(path: &Path, width: u32, height: u32, pixels: &[u8]) -> Result<()> {
    let file = File::create(path).with_context(|| format!("create {}", path.display()))?;
    let mut encoder = png::Encoder::new(BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(pixels)?;
    Ok(())
}
