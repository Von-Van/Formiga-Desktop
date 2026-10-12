//! The readme's hero image and demo animation: a colony among drawn windows.

use crate::{
    blend_pixel, blit_scaled_anchor, draw_rect_alpha, fill_gradient, fixture_desktop, write_png,
};
use anyhow::{Context, Result};
use formiga_art::CreatureRenderer;
use formiga_core::*;
use std::fs::File;
use std::io::BufWriter;
use std::path::PathBuf;
use time::OffsetDateTime;

pub fn hero_image(path: PathBuf) -> Result<()> {
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
    let creatures = demo_colony();
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

pub fn demo_animation(path: PathBuf) -> Result<()> {
    const WIDTH: u32 = 480;
    const HEIGHT: u32 = 270;
    const FPS: u32 = 10;
    const FRAMES: u32 = 20 * FPS;
    let file = File::create(&path).with_context(|| format!("create {}", path.display()))?;
    let mut encoder = gif::Encoder::new(BufWriter::new(file), WIDTH as u16, HEIGHT as u16, &[])?;
    encoder.set_repeat(gif::Repeat::Infinite)?;
    let creatures = demo_colony();
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

fn demo_colony() -> Vec<Creature> {
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
