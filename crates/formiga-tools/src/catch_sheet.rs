//! A round of jump and catch, frame by frame, the way the overlay draws it: one companion up on a
//! window teetering at the edge, crouching, leaping off, and a friend on the ground dashing in
//! under it to catch it overhead — or not.
//!
//!   cargo run -p formiga-tools -- catch-sheet [--output docs/assets/catch-sheet.png]
//!
//! Each row is a real round, played by the simulation on a desktop with one window and two
//! companions on the ground beside it: the first by a quick, watchful catcher, the second by a
//! slow and absent-minded one. A frame is taken a third of a second apart from the moment the
//! jumper reaches the edge until the round is over, with the climb up shown once before it. Bodies
//! and faces come through `BodyPresentation` and the face the desktop resolves, placed at their
//! contact points as the overlay places them.

use crate::{draw_rect_alpha, reference_creatures, write_png};
use anyhow::{Result, bail};
use formiga_art::{CreatureRenderer, FRAME_SIZE, FramePlacement};
use formiga_core::*;
use std::path::PathBuf;
use time::{Duration, OffsetDateTime};

const SCALE: u32 = 2;
/// The window climbed, in points; at this display scale a point is an art pixel.
const WINDOW: DesktopRect = DesktopRect {
    x: 440.0,
    y: 600.0,
    width: 360.0,
    height: 200.0,
};
const FLOOR: f32 = 846.0;
/// The part of the desktop each frame shows, in points.
const VIEW_LEFT: f32 = 700.0;
const VIEW_TOP: f32 = 500.0;
const VIEW_WIDTH: u32 = 300;
const VIEW_HEIGHT: u32 = 352;
const COLUMNS: u32 = 7;
/// Seconds between frames.
const EVERY: f32 = 0.33;

pub fn run(path: PathBuf) -> Result<()> {
    let looks = reference_creatures();
    // A quick, watchful catcher, then a slow and absent-minded one.
    let rounds = [(0, 1, 1.0, 1.0), (1, 2, 0.0, 0.0)];
    let mut strips = Vec::new();
    for (jumper_look, catcher_look, activity, curiosity) in rounds {
        let frames = play_round(
            &looks[jumper_look].appearance,
            &looks[catcher_look].appearance,
            activity,
            curiosity,
        )?;
        strips.push(frames);
    }
    let rows: u32 = strips
        .iter()
        .map(|frames| (frames.len() as u32).div_ceil(COLUMNS))
        .sum();
    let (cell_w, cell_h) = (VIEW_WIDTH * SCALE, VIEW_HEIGHT * SCALE);
    let width = COLUMNS * cell_w;
    let height = rows * cell_h;
    let mut pixels = vec![0_u8; (width * height * 4) as usize];
    for chunk in pixels.chunks_exact_mut(4) {
        chunk.copy_from_slice(&[236, 234, 228, 255]);
    }
    let mut row = 0;
    for frames in &strips {
        for (index, frame) in frames.iter().enumerate() {
            let index = index as u32;
            let (x, y) = ((index % COLUMNS) * cell_w, (row + index / COLUMNS) * cell_h);
            for py in 0..cell_h {
                let start = (((y + py) * width + x) * 4) as usize;
                let from = ((py * cell_w) * 4) as usize;
                pixels[start..start + (cell_w * 4) as usize]
                    .copy_from_slice(&frame[from..from + (cell_w * 4) as usize]);
            }
            // A hairline between frames.
            draw_rect_alpha(&mut pixels, width, x, y, 1, cell_h, [0, 0, 0, 40]);
            draw_rect_alpha(&mut pixels, width, x, y, cell_w, 1, [0, 0, 0, 40]);
        }
        row += (frames.len() as u32).div_ceil(COLUMNS);
    }
    write_png(&path, width, height, &pixels)?;
    println!("wrote {}", path.display());
    Ok(())
}

/// Play one round between two companions wearing these looks and return its frames.
fn play_round(
    jumper_look: &AppearanceGenome,
    catcher_look: &AppearanceGenome,
    activity: f32,
    curiosity: f32,
) -> Result<Vec<Vec<u8>>> {
    let created = OffsetDateTime::UNIX_EPOCH + Duration::days(20_000);
    let now = created + Duration::days(40);
    let mut desktop = DesktopSnapshot {
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
        window_sample: Some(WindowSample {
            monotonic_millis: 0,
            reliable: true,
        }),
        ..DesktopSnapshot::default()
    };
    desktop.windows.push(DesktopWindow {
        key: 701,
        bounds: WINDOW,
        z_order: 0,
        visible: true,
        minimized: false,
        application: None,
        application_name: None,
    });
    let mut world = World::new([92; 32], created, &desktop);
    world.tick(now, 0.05, &desktop);
    world.save.settings.display_scale = 2;
    world.save.home.active_since_utc = None;
    world.save.home.last_disappeared_utc = Some(now);
    world.save.ritual.next_at_utc = now + Duration::days(1);
    world.save.creatures.truncate(2);
    for (index, c) in world.save.creatures.iter_mut().enumerate() {
        c.appearance = if index == 0 {
            jumper_look.clone()
        } else {
            catcher_look.clone()
        };
        c.role = CreatureRole::Adult;
        c.personality.playfulness = 1.0;
        c.personality.sociability = 0.9;
        c.personality.boldness = if index == 0 { 1.0 } else { 0.3 };
        c.personality.curiosity = if index == 0 { 1.0 } else { curiosity };
        c.personality.activity = if index == 0 { 1.0 } else { activity };
        c.state.arrival_delay_secs = 0.0;
        c.state.drives = Drives {
            energy: 0.9,
            social_need: 0.0,
            ..Drives::default()
        };
        c.state.surface = SurfaceAttachment {
            kind: SurfaceKind::ScreenFloor,
            monitor_id: 1,
            window_key: None,
            relative_x: 0.5,
        };
        c.state.position = Point {
            x: 860.0 + index as f32 * 140.0,
            y: FLOOR,
        };
        c.state.action = ActionKind::Idle;
        c.state.action_elapsed = 0.0;
        c.state.action_duration = 100.0;
        c.state.facing_right = index == 0;
    }
    let mut frames = Vec::new();
    let mut climb_shot = None;
    let mut at_edge = None;
    let mut next_shot = 0.0;
    for step in 1_u64..2_400 {
        desktop.window_sample.as_mut().unwrap().monotonic_millis = step * 50;
        world.tick(
            now + Duration::milliseconds(step as i64 * 50),
            0.05,
            &desktop,
        );
        let seconds = step as f32 * 0.05;
        let jumper = &world.save.creatures[0];
        let playing = world
            .save
            .creatures
            .iter()
            .any(|c| c.state.attention.is_some());
        // The climb: a playing companion going up the window's side, a second into it.
        if climb_shot.is_none() && jumper.state.action == ActionKind::ClimbWindow && playing {
            climb_shot = Some(seconds + 1.0);
        }
        if climb_shot.is_some_and(|at| (at..at + 0.05).contains(&seconds)) {
            frames.push(draw(&world));
        }
        let teetering = jumper
            .state
            .attention
            .is_some_and(|pose| pose.gesture == Some(Gesture::Balance))
            && jumper.state.surface.window_key == Some(701);
        if at_edge.is_none() && teetering {
            at_edge = Some(seconds);
            next_shot = seconds;
        }
        if at_edge.is_some() {
            if !playing {
                return Ok(frames);
            }
            if seconds >= next_shot {
                frames.push(draw(&world));
                next_shot += EVERY;
            }
        }
    }
    bail!("no round of jump and catch was played")
}

/// One frame of the view: the window, the ground, and both companions.
fn draw(world: &World) -> Vec<u8> {
    let (width, height) = (VIEW_WIDTH * SCALE, VIEW_HEIGHT * SCALE);
    let mut pixels = vec![0_u8; (width * height * 4) as usize];
    for chunk in pixels.chunks_exact_mut(4) {
        chunk.copy_from_slice(&[236, 234, 228, 255]);
    }
    let to_view = |x: f32, y: f32| ((x - VIEW_LEFT) as i32, (y - VIEW_TOP) as i32);
    // The window: a pale pane with its top edge, the ledge, drawn darker.
    let (wx, wy) = to_view(WINDOW.x, WINDOW.y);
    let (wr, wb) = to_view(WINDOW.right(), WINDOW.bottom());
    let (wx, wr) = (wx.max(0) as u32, wr.max(0) as u32);
    draw_rect_alpha(
        &mut pixels,
        width,
        wx * SCALE,
        wy as u32 * SCALE,
        (wr - wx) * SCALE,
        (wb - wy) as u32 * SCALE,
        [196, 204, 214, 255],
    );
    draw_rect_alpha(
        &mut pixels,
        width,
        wx * SCALE,
        wy as u32 * SCALE,
        (wr - wx) * SCALE,
        2 * SCALE,
        [120, 128, 140, 255],
    );
    // The ground.
    let (_, ground) = to_view(0.0, FLOOR);
    draw_rect_alpha(
        &mut pixels,
        width,
        0,
        ground as u32 * SCALE,
        width,
        SCALE,
        [150, 140, 120, 255],
    );
    for creature in &world.save.creatures {
        let body = formiga_art::BodyPresentation::for_creature(creature);
        let face = CreatureRenderer::resolve_face_state(creature, CursorSnapshot::default(), false);
        let frame = CreatureRenderer::render_dressed_composited_frame(
            &creature.appearance,
            None,
            body.clip,
            body.frame,
            body.facing_right,
            false,
            face,
        );
        let baseline = CreatureRenderer::resting_baseline(&creature.appearance, false);
        let placement = FramePlacement::for_creature(creature, baseline);
        let (x, y) = to_view(creature.state.position.x, creature.state.position.y);
        let (left, top) = (x - FRAME_SIZE as i32 / 2, y + placement.origin_y);
        if left < 0 || top < 0 {
            continue;
        }
        crate::blit_scaled_square_alpha(
            &mut pixels,
            width,
            left as u32 * SCALE,
            top as u32 * SCALE,
            &frame.rgba_bytes(),
            FRAME_SIZE,
            SCALE,
        );
    }
    pixels
}
