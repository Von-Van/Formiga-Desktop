use anyhow::{Context, Result, bail};
use formiga_art::{
    CreatureRenderer, ExpressionKind, EyelidPose, FRAME_SIZE, FaceRenderState, GazeDirection,
};
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
        Some("expression-sheet") => expression_sheet(output_argument_with_default(
            &args,
            "docs/assets/expression-sheet.png",
        )),
        Some("gesture-sheet") => gesture_sheet(output_argument_with_default(
            &args,
            "docs/assets/gesture-sheet.png",
        )),
        Some("portfolio-hero") => {
            portfolio_hero(output_argument_with_default(&args, "docs/assets/hero.png"))
        }
        Some("portfolio-demo") => portfolio_demo(output_argument_with_default(
            &args,
            "docs/assets/formiga-demo.gif",
        )),
        Some("simulate") => simulate(
            args.get(2)
                .and_then(|value| value.parse().ok())
                .unwrap_or(181),
        ),
        _ => {
            eprintln!(
                "usage:\n  formiga-tools contact-sheet [--output PATH]\n  formiga-tools animation-preview [--seed NUMBER] [--output PATH]\n  formiga-tools expression-sheet [--output PATH]\n  formiga-tools gesture-sheet [--output PATH]\n  formiga-tools portfolio-hero [--output PATH]\n  formiga-tools portfolio-demo [--output PATH]\n  formiga-tools simulate [DAYS]"
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

fn expression_sheet(path: PathBuf) -> Result<()> {
    const SCALE: u32 = 3;
    let creatures = reference_creatures();
    let cell = FRAME_SIZE * SCALE;
    let width = ExpressionKind::ALL.len() as u32 * cell;
    let height = creatures.len() as u32 * cell;
    let mut pixels = vec![0_u8; (width * height * 4) as usize];
    for (row, creature) in creatures.iter().enumerate() {
        for (column, expression) in ExpressionKind::ALL.into_iter().enumerate() {
            let rendered = CreatureRenderer::render_composited_frame(
                &creature.appearance,
                ActionKind::Idle,
                0,
                true,
                false,
                FaceRenderState {
                    expression,
                    eyelids: EyelidPose::Open,
                    gaze: GazeDirection::default(),
                },
            );
            blit_scaled(
                &mut pixels,
                width,
                column as u32 * cell,
                row as u32 * cell,
                &rendered.rgba_bytes(),
                SCALE,
            );
        }
    }
    write_png(&path, width, height, &pixels)?;
    println!("wrote {}", path.display());
    Ok(())
}

fn gesture_sheet(path: PathBuf) -> Result<()> {
    const COLS: u32 = 7;
    const SCALE: u32 = 3;
    let creatures = reference_creatures();
    let cell = FRAME_SIZE * SCALE;
    let rows_per_family = 2;
    let width = COLS * cell;
    let height = creatures.len() as u32 * rows_per_family * cell;
    let mut pixels = vec![0_u8; (width * height * 4) as usize];
    for (family_index, creature) in creatures.iter().enumerate() {
        for (action_index, action) in ActionKind::ALL.into_iter().enumerate() {
            let spec = formiga_art::AnimationSpec::for_action(action);
            let frame = (action_index as u8 + 1) % spec.frames;
            let rendered =
                CreatureRenderer::render_frame(&creature.appearance, action, frame, true);
            let column = action_index as u32 % COLS;
            let local_row = action_index as u32 / COLS;
            let row = family_index as u32 * rows_per_family + local_row;
            blit_scaled(
                &mut pixels,
                width,
                column * cell,
                row * cell,
                &rendered.rgba_bytes(),
                SCALE,
            );
        }
    }
    write_png(&path, width, height, &pixels)?;
    println!("wrote {}", path.display());
    Ok(())
}

fn reference_creatures() -> Vec<Creature> {
    let desktop = fixture_desktop();
    [
        BodyFamily::Blob,
        BodyFamily::Hopper,
        BodyFamily::SoftQuadruped,
    ]
    .into_iter()
    .map(|family| {
        (0_u64..1_000)
            .find_map(|index| {
                let mut seed = [0_u8; 32];
                seed.copy_from_slice(&Sha256::digest(format!(
                    "formiga-reference-{family:?}-{index}"
                )));
                let creature = World::new(seed, OffsetDateTime::UNIX_EPOCH, &desktop)
                    .save
                    .creatures
                    .remove(0);
                (creature.appearance.family == family).then_some(creature)
            })
            .expect("a deterministic reference seed exists for every family")
    })
    .collect()
}

fn portfolio_hero(path: PathBuf) -> Result<()> {
    const WIDTH: u32 = 1200;
    const HEIGHT: u32 = 630;
    let mut pixels = vec![0_u8; (WIDTH * HEIGHT * 4) as usize];
    fill_gradient(
        &mut pixels,
        WIDTH,
        HEIGHT,
        [14, 23, 28, 255],
        [24, 42, 42, 255],
    );
    draw_window(&mut pixels, WIDTH, 75, 90, 640, 390, [45, 65, 72, 255]);
    draw_window(&mut pixels, WIDTH, 650, 190, 450, 325, [53, 57, 78, 255]);
    draw_rect_alpha(&mut pixels, WIDTH, 740, 380, 330, 180, [61, 185, 125, 48]);
    let creatures = portfolio_colony();
    for (index, (x, y, action, scale)) in [
        (360, 480, ActionKind::Idle, 4),
        (795, 190, ActionKind::Perch, 3),
        (940, 515, ActionKind::SoloPlay, 3),
        (150, 480, ActionKind::Sleep, 3),
    ]
    .into_iter()
    .enumerate()
    {
        let creature = &creatures[index.min(creatures.len() - 1)];
        let spec = formiga_art::AnimationSpec::for_action(action);
        let rendered = CreatureRenderer::render_frame(
            &creature.appearance,
            action,
            (index as u8) % spec.frames,
            true,
        );
        blit_scaled_anchor(
            &mut pixels,
            WIDTH,
            HEIGHT,
            x,
            y,
            &rendered.rgba_bytes(),
            scale,
            None,
        );
    }
    write_png(&path, WIDTH, HEIGHT, &pixels)?;
    println!("wrote {}", path.display());
    Ok(())
}

fn portfolio_demo(path: PathBuf) -> Result<()> {
    const WIDTH: u32 = 480;
    const HEIGHT: u32 = 270;
    const FPS: u32 = 10;
    const FRAMES: u32 = 20 * FPS;
    let file = File::create(&path).with_context(|| format!("create {}", path.display()))?;
    let mut encoder = gif::Encoder::new(BufWriter::new(file), WIDTH as u16, HEIGHT as u16, &[])?;
    encoder.set_repeat(gif::Repeat::Infinite)?;
    let creatures = portfolio_colony();
    for frame_index in 0..FRAMES {
        let mut pixels = vec![0_u8; (WIDTH * HEIGHT * 4) as usize];
        fill_gradient(
            &mut pixels,
            WIDTH,
            HEIGHT,
            [13, 21, 27, 255],
            [23, 38, 40, 255],
        );
        let scene = frame_index / (4 * FPS);
        let phase = (frame_index % (4 * FPS)) as f32 / (4 * FPS - 1) as f32;
        draw_window(&mut pixels, WIDTH, 22, 32, 255, 150, [43, 63, 72, 255]);
        draw_window(&mut pixels, WIDTH, 285, 72, 170, 125, [55, 57, 78, 255]);
        match scene {
            0 => {
                let x = 70 + (phase * 150.0) as u32;
                draw_demo_creature(
                    &mut pixels,
                    WIDTH,
                    HEIGHT,
                    &creatures[0],
                    ActionKind::Traverse,
                    frame_index,
                    x,
                    245,
                    3,
                    None,
                );
            }
            1 => {
                let x = (150.0 + phase * 230.0) as u32;
                let y = (240.0 - (phase * std::f32::consts::PI).sin() * 100.0) as u32;
                draw_demo_creature(
                    &mut pixels,
                    WIDTH,
                    HEIGHT,
                    &creatures[0],
                    ActionKind::Dragged,
                    frame_index,
                    x,
                    y,
                    3,
                    None,
                );
                draw_cursor(&mut pixels, WIDTH, x + 8, y.saturating_sub(52));
            }
            2 => {
                draw_rect_alpha(&mut pixels, WIDTH, 250, 160, 215, 95, [54, 204, 132, 74]);
                draw_rect_alpha(&mut pixels, WIDTH, 20, 185, 150, 70, [226, 71, 69, 70]);
                let x = (285.0 + phase * 110.0) as u32;
                draw_demo_creature(
                    &mut pixels,
                    WIDTH,
                    HEIGHT,
                    &creatures[0],
                    ActionKind::Traverse,
                    frame_index,
                    x,
                    245,
                    3,
                    None,
                );
            }
            3 => {
                let selected_window = (150, 120, 250, 115);
                draw_window(
                    &mut pixels,
                    WIDTH,
                    selected_window.0,
                    selected_window.1,
                    selected_window.2,
                    selected_window.3,
                    [71, 61, 82, 255],
                );
                let x = (125.0 + phase * 160.0) as u32;
                draw_demo_creature(
                    &mut pixels,
                    WIDTH,
                    HEIGHT,
                    &creatures[0],
                    ActionKind::Traverse,
                    frame_index,
                    x,
                    210,
                    3,
                    Some(selected_window),
                );
            }
            _ => {
                for (index, (x, y, action, scale)) in [
                    (80, 245, ActionKind::Idle, 3),
                    (235, 182, ActionKind::Perch, 2),
                    (340, 245, ActionKind::SoloPlay, 2),
                    (430, 245, ActionKind::Sleep, 2),
                ]
                .into_iter()
                .enumerate()
                {
                    draw_demo_creature(
                        &mut pixels,
                        WIDTH,
                        HEIGHT,
                        &creatures[index],
                        action,
                        frame_index,
                        x,
                        y,
                        scale,
                        None,
                    );
                }
            }
        }
        let mut gif_frame =
            gif::Frame::from_rgba_speed(WIDTH as u16, HEIGHT as u16, pixels.as_mut_slice(), 20);
        gif_frame.delay = (100 / FPS) as u16;
        encoder.write_frame(&gif_frame)?;
    }
    println!("wrote {}", path.display());
    Ok(())
}

fn portfolio_colony() -> Vec<Creature> {
    let desktop = fixture_desktop();
    let created = OffsetDateTime::UNIX_EPOCH;
    let mut world = World::new([42; 32], created, &desktop);
    world.tick(created + time::Duration::days(181), 0.05, &desktop);
    world.save.creatures
}

#[allow(clippy::too_many_arguments)]
fn draw_demo_creature(
    target: &mut [u8],
    width: u32,
    height: u32,
    creature: &Creature,
    action: ActionKind,
    frame_index: u32,
    x: u32,
    y: u32,
    scale: u32,
    clip: Option<(u32, u32, u32, u32)>,
) {
    let spec = formiga_art::AnimationSpec::for_action(action);
    let rendered = CreatureRenderer::render_frame(
        &creature.appearance,
        action,
        (frame_index as u8) % spec.frames,
        true,
    );
    blit_scaled_anchor(
        target,
        width,
        height,
        x,
        y,
        &rendered.rgba_bytes(),
        scale,
        clip,
    );
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

#[allow(clippy::too_many_arguments)]
fn blit_scaled_anchor(
    target: &mut [u8],
    target_width: u32,
    target_height: u32,
    anchor_x: u32,
    anchor_y: u32,
    source: &[u8],
    scale: u32,
    clip: Option<(u32, u32, u32, u32)>,
) {
    let size = FRAME_SIZE * scale;
    let origin_x = anchor_x as i32 - size as i32 / 2;
    let origin_y = anchor_y as i32 - size as i32;
    for sy in 0..FRAME_SIZE {
        for sx in 0..FRAME_SIZE {
            let source_index = ((sy * FRAME_SIZE + sx) * 4) as usize;
            let color = [
                source[source_index],
                source[source_index + 1],
                source[source_index + 2],
                source[source_index + 3],
            ];
            if color[3] == 0 {
                continue;
            }
            for oy in 0..scale {
                for ox in 0..scale {
                    let x = origin_x + (sx * scale + ox) as i32;
                    let y = origin_y + (sy * scale + oy) as i32;
                    if x < 0 || y < 0 || x >= target_width as i32 || y >= target_height as i32 {
                        continue;
                    }
                    let x = x as u32;
                    let y = y as u32;
                    if clip.is_some_and(|(cx, cy, width, height)| {
                        x >= cx && x < cx + width && y >= cy && y < cy + height
                    }) {
                        continue;
                    }
                    blend_pixel(target, target_width, x, y, color);
                }
            }
        }
    }
}

fn fill_gradient(target: &mut [u8], width: u32, height: u32, top: [u8; 4], bottom: [u8; 4]) {
    for y in 0..height {
        let mix = y as f32 / height.max(1) as f32;
        let color = [
            (top[0] as f32 * (1.0 - mix) + bottom[0] as f32 * mix) as u8,
            (top[1] as f32 * (1.0 - mix) + bottom[1] as f32 * mix) as u8,
            (top[2] as f32 * (1.0 - mix) + bottom[2] as f32 * mix) as u8,
            255,
        ];
        for x in 0..width {
            let index = ((y * width + x) * 4) as usize;
            target[index..index + 4].copy_from_slice(&color);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_window(
    target: &mut [u8],
    target_width: u32,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    body: [u8; 4],
) {
    draw_rect_alpha(
        target,
        target_width,
        x + 8,
        y + 10,
        width,
        height,
        [0, 0, 0, 70],
    );
    draw_rect_alpha(target, target_width, x, y, width, height, body);
    draw_rect_alpha(
        target,
        target_width,
        x,
        y,
        width,
        24.min(height),
        [25, 36, 42, 255],
    );
    for (offset, color) in [
        (10, [233, 108, 101, 255]),
        (26, [235, 190, 91, 255]),
        (42, [102, 194, 128, 255]),
    ] {
        draw_rect_alpha(target, target_width, x + offset, y + 8, 7, 7, color);
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_rect_alpha(
    target: &mut [u8],
    target_width: u32,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    color: [u8; 4],
) {
    let target_height = (target.len() as u32 / 4) / target_width;
    for py in y..y.saturating_add(height).min(target_height) {
        for px in x..x.saturating_add(width).min(target_width) {
            blend_pixel(target, target_width, px, py, color);
        }
    }
}

fn draw_cursor(target: &mut [u8], target_width: u32, x: u32, y: u32) {
    for row in 0..18_u32 {
        for column in 0..=(row / 2) {
            blend_pixel(
                target,
                target_width,
                x + column,
                y + row,
                [248, 250, 247, 255],
            );
        }
    }
    draw_rect_alpha(
        target,
        target_width,
        x + 5,
        y + 13,
        5,
        10,
        [248, 250, 247, 255],
    );
}

fn blend_pixel(target: &mut [u8], width: u32, x: u32, y: u32, source: [u8; 4]) {
    let index = ((y * width + x) * 4) as usize;
    if index + 4 > target.len() {
        return;
    }
    let alpha = source[3] as f32 / 255.0;
    for channel in 0..3 {
        target[index + channel] =
            (source[channel] as f32 * alpha + target[index + channel] as f32 * (1.0 - alpha)) as u8;
    }
    target[index + 3] = 255;
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
