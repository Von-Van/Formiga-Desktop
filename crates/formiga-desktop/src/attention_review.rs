//! Test-only visual review of actual world attention and the production composited sprite path.
use formiga_art::{
    AnimationSpec, Canvas, CreatureRenderer, ExpressionKind, FACE_FRAME_SIZE, FRAME_SIZE,
    FramePlacement, GazeDirection, MotionSignature, PixelPoint, PropAnchor, Rgba,
};
use formiga_core::*;
use time::{Duration, macros::datetime};

const PAPER: Rgba = Rgba::new(246, 242, 229, 255);
const PANEL: Rgba = Rgba::new(222, 234, 213, 255);
const SURFACE: Rgba = Rgba::new(129, 160, 126, 255);
const GUIDE: Rgba = Rgba::new(196, 104, 84, 255);

fn review_scene() -> (World, DesktopSnapshot, time::OffsetDateTime) {
    let created = datetime!(2026-01-01 0:00 UTC);
    let now = created + Duration::days(40);
    let desktop = DesktopSnapshot {
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
        windows: vec![DesktopWindow {
            key: 701,
            bounds: DesktopRect {
                x: 200.0,
                y: 600.0,
                width: 600.0,
                height: 200.0,
            },
            z_order: 0,
            visible: true,
            minimized: false,
            application: None,
            application_name: None,
        }],
        window_sample: Some(WindowSample {
            monotonic_millis: 0,
            reliable: true,
        }),
        cursor: CursorSnapshot {
            position: Point { x: 800.0, y: 900.0 },
            velocity: Point::default(),
            available: true,
        },
        ..Default::default()
    };
    let mut world = World::new([92; 32], created, &desktop);
    world.tick(now, 0.05, &desktop);
    world.save.home.active_since_utc = None;
    world.save.home.last_disappeared_utc = Some(now);
    world.save.ritual.next_at_utc = now + Duration::days(1);
    for (index, c) in world.save.creatures.iter_mut().enumerate() {
        c.state.arrival_delay_secs = 0.0;
        c.state.action = ActionKind::Idle;
        c.state.action_elapsed = 0.0;
        c.state.action_duration = 100.0;
        c.state.drives = Drives::default();
        c.personality.curiosity = if index == 1 { 0.6 } else { 1.0 };
        c.personality.sociability = 1.0;
        c.personality.boldness = if index == 1 { 1.0 } else { 0.0 };
        c.personality.window_tolerance = c.personality.boldness;
        let rider = index < 2;
        c.state.surface = SurfaceAttachment {
            kind: if rider {
                SurfaceKind::WindowLedge
            } else {
                SurfaceKind::ScreenFloor
            },
            monitor_id: 1,
            window_key: rider.then_some(701),
            relative_x: if rider { 0.2 + index as f32 * 0.4 } else { 0.5 },
        };
        c.state.position = if rider {
            Point {
                x: 320.0 + index as f32 * 240.0,
                y: 600.0,
            }
        } else {
            Point {
                x: 600.0 + (index - 2) as f32 * 100.0,
                y: 846.0,
            }
        };
    }
    world.tick(now, 0.05, &desktop);
    (world, desktop, now)
}

#[test]
fn riders_and_spectators_render_notice_reaction_and_relief() {
    let (mut world, mut desktop, now) = review_scene();
    desktop.window_sample.as_mut().unwrap().monotonic_millis = 250;
    desktop.windows[0].bounds.x += 120.0;
    world.tick(now + Duration::milliseconds(250), 0.05, &desktop);
    let mut sheet = Canvas::new(768, 576);
    sheet.fill_rect(0, 0, 768, 576, Rgba::new(246, 242, 229, 255));
    let mut row = 0;
    for step in 0..=58 {
        if step > 0 {
            world.tick(
                now + Duration::milliseconds(250 + step * 50),
                0.05,
                &desktop,
            );
        }
        if ![0, 26, 58].contains(&step) {
            continue;
        }
        for (index, c) in world.save.creatures.iter().enumerate() {
            let face = CreatureRenderer::resolve_face_state(c, desktop.cursor, true);
            if step == 26 {
                assert_eq!(
                    face.expression,
                    match index {
                        0 => ExpressionKind::Startled,
                        1 => ExpressionKind::Joy,
                        _ => ExpressionKind::Worried,
                    }
                );
                if index >= 2 {
                    assert_eq!(face.gaze, GazeDirection::new(-1, -1));
                    // Companion attention remains visible even if cursor reactions are disabled.
                    assert_eq!(
                        face,
                        CreatureRenderer::resolve_face_state(c, desktop.cursor, false)
                    );
                }
            }
            if step == 58 && index != 1 {
                assert_eq!(face.expression, ExpressionKind::Content);
            }
            let frame = CreatureRenderer::render_composited_frame(
                &c.appearance,
                c.state.action,
                MotionSignature::for_creature(c).frame(c.state.action, c.state.action_elapsed),
                c.state.facing_right,
                false,
                face,
            );
            assert!(frame.alpha_bounds().is_some());
            let x = index as i32 * 192;
            let y = row * 192;
            sheet.fill_rect(x + 8, y + 8, 176, 176, Rgba::new(222, 234, 213, 255));
            for py in 0..48 {
                for px in 0..48 {
                    let pixel = frame.get(px, py);
                    if pixel.a != 0 {
                        sheet.fill_rect(x + 24 + px * 3, y + 24 + py * 3, 3, 3, pixel);
                    }
                }
            }
        }
        row += 1;
    }
    save_review("attention-stages.png", &sheet);
}

fn save_review(filename: &str, sheet: &Canvas) {
    if let Some(directory) = std::env::var_os("FORMIGA_ATTENTION_REVIEW_DIR") {
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
fn new_window_inspection_renders_notice_approach_and_a_spaced_audience() {
    let (mut world, mut desktop, now) = review_scene();
    desktop.windows.clear();
    for (index, c) in world.save.creatures.iter_mut().enumerate() {
        c.state.surface = SurfaceAttachment {
            monitor_id: 1,
            window_key: None,
            kind: SurfaceKind::ScreenFloor,
            relative_x: 0.5,
        };
        c.state.position = Point {
            x: 480.0 - index as f32 * 65.0,
            y: 846.0,
        };
        c.personality.curiosity = if index == 0 { 1.0 } else { 0.7 };
        c.personality.activity = 0.7;
    }
    let mut world = World::from_save(world.save);
    world.tick(now, 0.05, &desktop);
    desktop.windows.push(DesktopWindow {
        key: 702,
        bounds: DesktopRect {
            x: 580.0,
            y: 650.0,
            width: 240.0,
            height: 150.0,
        },
        z_order: 0,
        visible: true,
        minimized: false,
        application: None,
        application_name: None,
    });
    desktop.window_sample.as_mut().unwrap().monotonic_millis = 250;
    world.tick(now + Duration::milliseconds(250), 0.05, &desktop);
    let mut sheet = Canvas::new(1320, 1440);
    sheet.fill_rect(0, 0, 1320, 1440, Rgba::new(246, 242, 229, 255));
    let mut row = 0;
    for step in 0..=65 {
        if step > 0 {
            world.tick(
                now + Duration::milliseconds(250 + step * 50),
                0.05,
                &desktop,
            );
        }
        if ![0, 20, 65].contains(&step) {
            continue;
        }
        let row_y = row * 480;
        // Synthetic window geometry and desktop floor, drawn at 2x logical size.
        sheet.fill_rect(760, row_y + 60, 480, 300, Rgba::new(129, 160, 126, 255));
        sheet.fill_rect(764, row_y + 64, 472, 292, Rgba::new(222, 234, 213, 255));
        sheet.fill_rect(0, row_y + 452, 1320, 2, Rgba::new(129, 160, 126, 255));
        if step == 20 {
            assert_eq!(world.save.creatures[0].state.action, ActionKind::Traverse);
            assert!(world.save.creatures[1].state.position.x > 415.0);
        }
        if step == 65 {
            assert_eq!(
                world.save.creatures[0].state.action,
                ActionKind::InspectScreen
            );
            assert!(world.save.creatures[0].state.position.x > 550.0);
        }
        for c in &world.save.creatures {
            let face = CreatureRenderer::resolve_face_state(c, desktop.cursor, false);
            let frame = CreatureRenderer::render_composited_frame(
                &c.appearance,
                c.state.action,
                MotionSignature::for_creature(c).frame(c.state.action, c.state.action_elapsed),
                c.state.facing_right,
                false,
                face,
            );
            let placement = formiga_art::FramePlacement::for_action(
                c.state.action,
                CreatureRenderer::resting_baseline(&c.appearance, false),
            );
            let left = ((c.state.position.x - 200.0) * 2.0) as i32 - 72;
            let top = row_y + ((c.state.position.y - 620.0) * 2.0) as i32 + placement.origin_y * 3;
            for py in 0..48 {
                for px in 0..48 {
                    let pixel = frame.get(px, py);
                    if pixel.a != 0 {
                        sheet.fill_rect(left + px * 3, top + py * 3, 3, 3, pixel);
                    }
                }
            }
        }
        row += 1;
    }
    save_review("inspection-approach.png", &sheet);
}

#[test]
fn cursor_and_monitor_attention_render_distinct_notice_and_movement() {
    let mut sheet = Canvas::new(768, 384);
    sheet.fill_rect(0, 0, 768, 384, Rgba::new(246, 242, 229, 255));
    for case in 0..4 {
        let (mut world, mut desktop, now) = review_scene();
        desktop.windows.clear();
        world.save.creatures.truncate(1);
        let c = &mut world.save.creatures[0];
        c.state.surface = SurfaceAttachment {
            monitor_id: 1,
            window_key: None,
            kind: SurfaceKind::ScreenFloor,
            relative_x: 0.5,
        };
        c.state.position = Point {
            x: if case < 2 { 480.0 } else { 1_360.0 },
            y: 846.0,
        };
        c.personality.activity = 0.7;
        c.personality.boldness = if case == 1 { 0.1 } else { 0.9 };
        c.personality.cursor_interest = 1.0;
        c.personality.curiosity = 1.0;
        c.tendencies.cursor_trust = 0;
        desktop.cursor.available = false;
        let mut added = desktop.monitors[0].clone();
        added.id = 2;
        added.display_key = DisplayKey([2; 16]);
        added.primary = false;
        added.bounds.x += 1_440.0;
        added.usable_bounds.x += 1_440.0;
        if case == 3 {
            desktop.monitors.push(added.clone());
            c.state.surface.monitor_id = 2;
            c.state.position.x = 1_700.0;
        }
        let mut world = World::from_save(world.save);
        world.save.creatures[0].state.action_duration = 100.0;
        world.tick(now, 0.05, &desktop);
        if case == 2 {
            desktop.monitors.push(added);
        }
        if case == 3 {
            desktop.monitors.pop();
        }
        let captures = if case < 2 { [18, 31] } else { [8, 30] };
        for step in 0..=captures[1] {
            desktop.cursor_sample_millis = Some(step as u64 * 50);
            if case < 2 {
                desktop.cursor.available = true;
                let angle = step as f32 * std::f32::consts::FRAC_PI_4;
                desktop.cursor.position = if step <= 20 {
                    Point {
                        x: 560.0 + angle.cos() * 24.0,
                        y: 800.0 + angle.sin() * 24.0,
                    }
                } else {
                    Point { x: 560.0, y: 800.0 }
                };
            }
            world.tick(now + Duration::milliseconds(step * 50), 0.05, &desktop);
            let Some(row) = captures.iter().position(|capture| *capture == step) else {
                continue;
            };
            let c = &world.save.creatures[0];
            let face = CreatureRenderer::resolve_face_state(c, desktop.cursor, true);
            assert!(c.state.attention.is_some(), "case {case}, row {row}");
            if row == 1 {
                assert_eq!(
                    face.expression,
                    if case == 1 || case == 3 {
                        ExpressionKind::Worried
                    } else {
                        ExpressionKind::Curious
                    }
                );
                assert_eq!(
                    c.state.action,
                    match case {
                        0 => ActionKind::InvestigateCursor,
                        1 => ActionKind::AvoidCursor,
                        _ => ActionKind::Traverse,
                    }
                );
            }
            let frame = CreatureRenderer::render_composited_frame(
                &c.appearance,
                c.state.action,
                MotionSignature::for_creature(c).frame(c.state.action, c.state.action_elapsed),
                c.state.facing_right,
                false,
                face,
            );
            let x = case * 192;
            let y = row as i32 * 192;
            sheet.fill_rect(x + 8, y + 8, 176, 176, Rgba::new(222, 234, 213, 255));
            for py in 0..48 {
                for px in 0..48 {
                    let pixel = frame.get(px, py);
                    if pixel.a != 0 {
                        sheet.fill_rect(x + 24 + px * 3, y + 24 + py * 3, 3, 3, pixel);
                    }
                }
            }
        }
    }
    save_review("cursor-and-display.png", &sheet);
}

#[test]
fn ledge_peeking_refusal_and_jump_outcomes_render_with_their_audience() {
    let mut sheet = Canvas::new(1920, 540);
    sheet.fill_rect(0, 0, 1920, 540, Rgba::new(246, 242, 229, 255));
    for case in 0..3 {
        let (mut world, mut desktop, now) = review_scene();
        world.save.settings.display_scale = 2;
        if case > 0 {
            let mut window = desktop.windows[0].clone();
            window.key = 702;
            window.bounds.x = 870.0;
            window.bounds.width = 300.0;
            desktop.windows.push(window);
        }
        for (index, c) in world.save.creatures.iter_mut().enumerate() {
            c.state.position = Point {
                x: [770.0, 670.0, 600.0, 530.0][index],
                y: 600.0,
            };
            c.state.surface = SurfaceAttachment {
                kind: SurfaceKind::WindowLedge,
                monitor_id: 1,
                window_key: Some(701),
                relative_x: (c.state.position.x - 200.0) / 600.0,
            };
            c.personality.boldness = if index == 0 && case == 2 { 1.0 } else { 0.0 };
            c.personality.curiosity = if index == 0 { 1.0 } else { 0.7 };
            c.personality.playfulness = if index == 1 { 1.0 } else { 0.0 };
        }
        let mut world = World::from_save(world.save);
        for c in &mut world.save.creatures {
            c.state.action_duration = 100.0;
        }
        let captures = [7, 24, 43, 58];
        let mut scene_step = None;
        let mut captured = 0;
        for step in 1..600 {
            desktop.window_sample.as_mut().unwrap().monotonic_millis = step * 50;
            world.tick(
                now + Duration::milliseconds(step as i64 * 50),
                0.05,
                &desktop,
            );
            if scene_step.is_none() && world.save.creatures[0].state.attention.is_some() {
                scene_step = Some(step);
            }
            let Some(age) = scene_step.map(|start| step - start) else {
                continue;
            };
            // The last column waits for the outcome itself — for a success, for the moment a
            // playful watcher celebrates it — so timing changes cannot miss it.
            let settled = if case == 2 {
                world.save.creatures[1].state.action == ActionKind::Greet
            } else {
                world.save.creatures[0]
                    .state
                    .attention
                    .is_some_and(|p| p.emotion == AttentionEmotion::Relieved)
            };
            let col = if captured == 3 {
                if !settled {
                    continue;
                }
                3
            } else {
                match captures.iter().position(|&t| t == age) {
                    Some(col) if col < 3 => col,
                    _ => continue,
                }
            };
            captured += 1;
            let x = col as i32 * 480;
            let y = case * 180;
            sheet.fill_rect(x + 4, y + 4, 472, 172, Rgba::new(222, 234, 213, 255));
            sheet.fill_rect(x, y + 100, 300, 70, Rgba::new(178, 199, 163, 255));
            if case > 0 {
                sheet.fill_rect(x + 370, y + 100, 106, 70, Rgba::new(178, 199, 163, 255));
            }
            for c in &world.save.creatures {
                let face = CreatureRenderer::resolve_face_state(c, desktop.cursor, false);
                let frame = CreatureRenderer::render_composited_frame(
                    &c.appearance,
                    c.state.action,
                    MotionSignature::for_creature(c).frame(c.state.action, c.state.action_elapsed),
                    c.state.facing_right,
                    false,
                    face,
                );
                let bx = x + (c.state.position.x - 500.0) as i32 - 24;
                let by = y + (c.state.position.y - 500.0) as i32 - 48
                    + CreatureRenderer::resting_baseline(&c.appearance, false) as i32;
                for py in 0..48 {
                    for px in 0..48 {
                        let pixel = frame.get(px, py);
                        if pixel.a > 0 {
                            sheet.fill_rect(bx + px, by + py, 1, 1, pixel);
                        }
                    }
                }
            }
            if col == 3 {
                if case == 2 {
                    assert_eq!(world.save.creatures[0].state.surface.window_key, Some(702));
                    assert_eq!(world.save.creatures[1].state.action, ActionKind::Greet);
                } else {
                    assert_eq!(world.save.creatures[0].state.surface.window_key, Some(701));
                }
                break;
            }
        }
        assert_eq!(captured, 4, "case {case} did not reach every scene stage");
    }
    save_review("ledge-and-gap.png", &sheet);
}

fn paint_scene(
    sheet: &mut Canvas,
    world: &World,
    desktop: &DesktopSnapshot,
    col: i32,
    row: i32,
    left: f32,
) {
    let mut panel = Canvas::new(480, 200);
    panel.fill_rect(0, 0, 480, 200, Rgba::new(222, 234, 213, 255));
    for w in desktop.windows.iter().rev() {
        panel.fill_rect(
            (w.bounds.x - left) as i32,
            (w.bounds.y - 500.0) as i32,
            w.bounds.width as i32,
            w.bounds.height as i32,
            Rgba::new(178, 199, 163, 255),
        );
    }
    for c in &world.save.creatures {
        let face = CreatureRenderer::resolve_face_state(c, desktop.cursor, false);
        let frame = CreatureRenderer::render_composited_frame(
            &c.appearance,
            c.state.action,
            MotionSignature::for_creature(c).frame(c.state.action, c.state.action_elapsed),
            c.state.facing_right,
            false,
            face,
        );
        let placement = FramePlacement::for_creature(
            c,
            CreatureRenderer::resting_baseline(&c.appearance, false),
        );
        if c.state.attention.is_some_and(|p| p.hanging > 0.99) {
            assert_eq!(placement.origin_y, -7);
        }
        for py in 0..48 {
            for px in 0..48 {
                let pixel = frame.get(px, py);
                if pixel.a > 0 {
                    panel.fill_rect(
                        (c.state.position.x - left) as i32 - 24 + px,
                        (c.state.position.y - 500.0) as i32 + placement.origin_y + py,
                        1,
                        1,
                        pixel,
                    );
                }
            }
        }
    }
    for y in 0..200 {
        for x in 0..480 {
            sheet.fill_rect(col * 480 + x, row * 200 + y, 1, 1, panel.get(x, y));
        }
    }
}

#[test]
fn ledge_catches_helpers_and_grips_use_the_production_contact_anchor() {
    let mut sheet = Canvas::new(2400, 600);
    for case in 0..3 {
        let (mut w, mut d, now) = review_scene();
        w.save.settings.display_scale = 2;
        if case < 2 {
            let mut target = d.windows[0].clone();
            target.key = 702;
            target.bounds.x = 870.0;
            target.bounds.width = 300.0;
            d.windows.push(target);
        }
        for (i, c) in w.save.creatures.iter_mut().enumerate() {
            let helper = case == 1 && i == 1;
            c.state.position = Point {
                x: if helper {
                    980.0
                } else {
                    [770.0, 670.0, 600.0, 530.0][i]
                },
                y: 600.0,
            };
            c.state.surface = SurfaceAttachment {
                kind: SurfaceKind::WindowLedge,
                monitor_id: 1,
                window_key: Some(if helper { 702 } else { 701 }),
                relative_x: if helper {
                    110.0 / 300.0
                } else {
                    (c.state.position.x - 200.0) / 600.0
                },
            };
            c.personality.boldness = if i == 0 && case < 2 { 0.65 } else { 0.0 };
            c.personality.window_tolerance = 0.0;
            c.personality.curiosity = if i == 0 { 1.0 } else { 0.7 };
            c.personality.playfulness = if i == 1 { 1.0 } else { 0.0 };
        }
        let mut w = World::from_save(w.save);
        for c in &mut w.save.creatures {
            c.state.action_duration = 100.0;
        }
        let mut start = None;
        let mut col = 0;
        let mut hand_seen = false;
        let mut helper_seen = false;
        for step in 1..650 {
            d.window_sample.as_mut().unwrap().monotonic_millis = step * 50;
            if start.is_none() {
                w.save.creatures[0].state.drives.energy = 0.8;
            }
            if case == 2 && step == 6 {
                d.windows[0].bounds.x += 66.0;
            }
            w.tick(now + Duration::milliseconds(step as i64 * 50), 0.05, &d);
            let c = &w.save.creatures[0];
            if c.state.attention.is_some() && start.is_none() {
                start = Some(step);
            }
            let Some(age) = start.map(|s| step - s) else {
                continue;
            };
            hand_seen |= c.state.attention.is_some_and(|p| p.hanging > 0.99);
            helper_seen |= case == 1 && w.save.creatures[1].state.position.x < 940.0;
            let capture = if case == 2 {
                [7, 16, 25, 36, 53][col] == age
            } else {
                match col {
                    0 => age >= 9,
                    1 => c.state.position.y < 575.0,
                    2 => c.state.attention.is_some_and(|p| p.hanging > 0.99),
                    3 => c.state.action == ActionKind::ClimbWindow,
                    _ => matches!(c.state.action, ActionKind::Greet | ActionKind::Tossed),
                }
            };
            if capture {
                paint_scene(&mut sheet, &w, &d, col as i32, case, 650.0);
                col += 1;
                if col == 5 {
                    break;
                }
            }
        }
        assert_eq!(col, 5, "case {case}");
        assert!(hand_seen, "case {case} never reached a hand contact pose");
        if case == 1 {
            assert!(helper_seen);
        }
    }
    save_review("catches-and-grips.png", &sheet);
}

#[test]
fn copycat_and_staring_contests_render_the_actor_handoff_and_outcome() {
    let mut sheet = Canvas::new(1920, 400);
    for case in 0..2 {
        let (mut w, mut d, now) = review_scene();
        w.save.settings.display_scale = 2;
        for (i, c) in w.save.creatures.iter_mut().enumerate() {
            c.state.position = Point {
                x: 360.0 + i as f32 * 80.0,
                y: 600.0,
            };
            c.state.surface = SurfaceAttachment {
                kind: SurfaceKind::WindowLedge,
                monitor_id: 1,
                window_key: Some(701),
                relative_x: (c.state.position.x - 200.0) / 600.0,
            };
            c.personality.boldness = 0.7;
            c.personality.curiosity = 0.8;
            c.personality.sociability = 0.9;
            c.personality.playfulness = if i == 3 { 0.0 } else { 1.0 };
            c.state.facing_right = i == 0;
        }
        let mut w = World::from_save(w.save);
        let mut start = None;
        let mut col = 0;
        let mut imitated = false;
        let mut blinked = false;
        for step in 1..600 {
            d.window_sample.as_mut().unwrap().monotonic_millis = step * 50;
            if start.is_none() {
                for (i, c) in w.save.creatures.iter_mut().enumerate() {
                    c.state.action = if case == 0 && i == 0 && step > 250 {
                        ActionKind::Greet
                    } else {
                        ActionKind::Perch
                    };
                    c.state.action_duration = 100.0;
                    c.state.facing_right = i == 0;
                }
            }
            w.tick(now + Duration::milliseconds(step as i64 * 50), 0.05, &d);
            if start.is_none() && w.save.creatures[1].state.attention.is_some() {
                start = Some(step);
            }
            let Some(age) = start.map(|s| step - s) else {
                continue;
            };
            imitated |= w.save.creatures[1].state.action == ActionKind::Greet;
            blinked |= w.save.creatures.iter().any(|c| {
                c.state
                    .attention
                    .is_some_and(|p| p.emotion == AttentionEmotion::Averting)
            });
            if [2, 19, 48, if case == 0 { 83 } else { 100 }][col] == age {
                paint_scene(&mut sheet, &w, &d, col as i32, case, 220.0);
                col += 1;
                if col == 4 {
                    break;
                }
            }
        }
        assert_eq!(col, 4);
        if case == 0 {
            assert!(imitated);
        } else {
            assert!(blinked);
        }
    }
    save_review("copycat-and-stare.png", &sheet);
}

/// S10: the audience itself, drawn large enough to read the faces at each stage of one attempt.
#[test]
fn spectator_reactions_read_without_labels() {
    let (mut w, mut d, now) = review_scene();
    w.save.settings.display_scale = 2;
    let mut target = d.windows[0].clone();
    target.key = 702;
    target.bounds.x = 870.0;
    target.bounds.width = 300.0;
    d.windows.push(target);
    for (i, c) in w.save.creatures.iter_mut().enumerate() {
        c.state.position = Point {
            x: [770.0, 660.0, 560.0, 460.0][i],
            y: 600.0,
        };
        c.state.surface = SurfaceAttachment {
            kind: SurfaceKind::WindowLedge,
            monitor_id: 1,
            window_key: Some(701),
            relative_x: (c.state.position.x - 200.0) / 600.0,
        };
        // One jumper, one timid watcher, one playful watcher, one steady watcher.
        c.personality.boldness = [0.65, 0.0, 0.7, 0.5][i];
        c.personality.playfulness = [0.0, 0.0, 1.0, 0.2][i];
        c.personality.curiosity = 0.9;
        c.personality.sociability = 1.0;
        c.state.facing_right = true;
    }
    let mut w = World::from_save(w.save);
    for c in &mut w.save.creatures {
        c.state.action_duration = 100.0;
        c.state.drives.energy = 0.8;
    }
    let mut sheet = Canvas::new(2160, 576);
    sheet.fill_rect(0, 0, 2160, 576, Rgba::new(246, 242, 229, 255));
    let mut start = None;
    let mut col = 0;
    let mut gasped = false;
    let mut staggered = false;
    let mut celebrated = false;
    let mut trace: Vec<Option<AttentionEmotion>> = Vec::new();
    for step in 1..600 {
        d.window_sample.as_mut().unwrap().monotonic_millis = step * 50;
        w.tick(now + Duration::milliseconds(step as i64 * 50), 0.05, &d);
        if start.is_none() && w.save.creatures[1].state.attention.is_some() {
            start = Some(step);
        }
        let Some(age) = start.map(|s| step - s) else {
            continue;
        };
        gasped |= w.save.creatures[1..].iter().any(|c| {
            c.state
                .attention
                .is_some_and(|p| p.emotion == AttentionEmotion::Startled)
        });
        celebrated |= w.save.creatures[2..]
            .iter()
            .any(|c| c.state.action == ActionKind::Greet);
        // Let the whole scene play out once every stage is captured, so each watcher has its
        // own moment before the assertions below.
        if col == 5 {
            if w.save.creatures.iter().all(|c| c.state.attention.is_none()) {
                break;
            }
            continue;
        }
        // Capture the timid watcher's own turning points rather than fixed frames.
        let feeling = w.save.creatures[1].state.attention.map(|p| p.emotion);
        let wanted = [
            AttentionEmotion::Curious,
            AttentionEmotion::Curious,
            AttentionEmotion::Startled,
            AttentionEmotion::Averting,
            AttentionEmotion::Relieved,
        ][col];
        if trace.last() != Some(&feeling) {
            trace.push(feeling);
        }
        if feeling != Some(wanted) || (col == 1 && age < 16) {
            continue;
        }
        // Watchers do not move through the stages together, so record where the others are at
        // each of this one's turning points.
        staggered |= w.save.creatures[2..]
            .iter()
            .any(|c| c.state.attention.map(|p| p.emotion) != Some(wanted));
        // Three watchers at 4x, so the face tells the story without a caption.
        for (index, c) in w.save.creatures.iter().enumerate().skip(1) {
            let face = CreatureRenderer::resolve_face_state(c, d.cursor, false);
            let frame = CreatureRenderer::render_composited_frame(
                &c.appearance,
                c.state.action,
                MotionSignature::for_creature(c).frame(c.state.action, c.state.action_elapsed),
                c.state.facing_right,
                false,
                face,
            );
            let x = col as i32 * 432 + (index as i32 - 1) * 144;
            sheet.fill_rect(x + 4, 4, 136, 568, Rgba::new(222, 234, 213, 255));
            for py in 0..48 {
                for px in 0..48 {
                    let pixel = frame.get(px, py);
                    if pixel.a != 0 {
                        sheet.fill_rect(x + 12 + px * 2, 200 + py * 2, 2, 2, pixel);
                    }
                }
            }
        }
        col += 1;
    }
    assert_eq!(col, 5, "every stage was captured: {trace:?}");
    assert!(gasped, "a sudden catch draws a gasp");
    assert!(staggered, "watchers react in their own time");
    assert!(celebrated, "a playful watcher celebrates the landing");
    save_review("spectator-reactions.png", &sheet);
}

// ---------------------------------------------------------------------------------------------
// R05: the art itself. Every shape the renderer can draw, at every size the tray offers, with the
// contact geometry, the carried prop, and the optional outline arranged the way the overlay
// composes them rather than the way a single sprite happens to look on its own.
// ---------------------------------------------------------------------------------------------

/// Source-over. A soft outline and any other translucent edge have to read against the sheet the
/// way they read against a desktop, not punch a dark hole through it.
fn over(source: Rgba, under: Rgba) -> Rgba {
    let alpha = u32::from(source.a);
    let mix = |s: u8, u: u8| ((u32::from(s) * alpha + u32::from(u) * (255 - alpha)) / 255) as u8;
    Rgba::new(
        mix(source.r, under.r),
        mix(source.g, under.g),
        mix(source.b, under.b),
        under.a.max(source.a),
    )
}

/// Stamp one art frame at an integer zoom the way the overlay samples its atlas: nearest, never
/// filtered. `squeeze` is the horizontal narrowing the overlay applies while a companion slips
/// through a gap, so a review sheet shows the same shape the desktop does.
fn stamp(sheet: &mut Canvas, frame: &Canvas, left: i32, top: i32, zoom: i32, squeeze: f32) {
    let width = (frame.width() as f32 * zoom as f32 * squeeze).round() as i32;
    for y in 0..frame.height() as i32 * zoom {
        for x in 0..width {
            let pixel = frame.get((x as f32 / (zoom as f32 * squeeze)) as i32, y / zoom);
            if pixel.a > 0 {
                sheet.set(left + x, top + y, over(pixel, sheet.get(left + x, top + y)));
            }
        }
    }
}

/// A companion grown to one recipe, so a sheet can walk every body plan in turn. Everything but
/// the plan, the ears, and the size is the colony's own generated appearance.
fn plan_companion(body: BodyPlan, ears: EarStyle, logical_size: u8) -> Creature {
    let mut creature = World::preview_adult(
        [61; 32],
        datetime!(2026-01-01 0:00 UTC),
        &DesktopSnapshot::default(),
    );
    let mut design = creature
        .appearance
        .design
        .expect("a generated companion carries its recipe");
    design.body = body;
    design.ears = ears;
    apply_creature_design(&mut creature, Some(design));
    creature.appearance.logical_size = logical_size;
    creature.state.facing_right = true;
    creature
}

/// A companion from before modular recipes. Without a design the renderer falls back to the older
/// per-family drawings, which a long-lived save still has to draw correctly.
fn legacy_companion(family: BodyFamily, logical_size: u8) -> Creature {
    let mut creature = plan_companion(BodyPlan::Round, EarStyle::Pointed, logical_size);
    apply_creature_design(&mut creature, None);
    creature.appearance.family = family;
    creature.appearance.forelimbs.style = match family {
        BodyFamily::Blob => ForelimbStyle::Pseudopod,
        BodyFamily::Hopper => ForelimbStyle::MittenArm,
        BodyFamily::SoftQuadruped => ForelimbStyle::FrontPaw,
    };
    creature.appearance.forelimbs.tip_style = match family {
        BodyFamily::Blob => LimbTipStyle::Round,
        BodyFamily::Hopper => LimbTipStyle::Mitten,
        BodyFamily::SoftQuadruped => LimbTipStyle::Paw,
    };
    creature
}

/// Every companion shape the art can draw: the five modular body plans the colony grows today,
/// walked against all six ear sets, then the three legacy body families an older save carries.
fn review_subjects(logical_size: u8) -> Vec<(String, Creature)> {
    let mut subjects: Vec<(String, Creature)> = BodyPlan::ALL
        .into_iter()
        .cycle()
        .zip(EarStyle::ALL)
        .map(|(body, ears)| {
            (
                format!("{body:?}/{ears:?}"),
                plan_companion(body, ears, logical_size),
            )
        })
        .collect();
    subjects.extend(
        [
            BodyFamily::Blob,
            BodyFamily::Hopper,
            BodyFamily::SoftQuadruped,
        ]
        .into_iter()
        .map(|family| {
            (
                format!("legacy {family:?}"),
                legacy_companion(family, logical_size),
            )
        }),
    );
    subjects
}

/// The sprite as the overlay assembles it: the body atlas frame, outlined while it is still a
/// body, then the layered face on top. Returns the face anchor, because the prop quad and the
/// face quad are both measured from it.
fn overlay_sprite(
    creature: &Creature,
    action: ActionKind,
    frame: u8,
    outline: bool,
) -> (Canvas, PixelPoint) {
    let mut body = CreatureRenderer::render_body_frame(&creature.appearance, action, frame, false);
    if outline {
        CreatureRenderer::outline_frame(&mut body.canvas);
    }
    if !creature.state.facing_right {
        body.canvas.mirror_horizontal();
        body.face_anchor.x = FRAME_SIZE as i32 - body.face_anchor.x;
    }
    let mut state =
        CreatureRenderer::resolve_face_state(creature, CursorSnapshot::default(), false);
    if !creature.state.facing_right {
        state.gaze.x = -state.gaze.x;
    }
    let mut face = CreatureRenderer::render_face_frame(&creature.appearance, state);
    if !creature.state.facing_right {
        face.mirror_horizontal();
    }
    let half = FACE_FRAME_SIZE as i32 / 2;
    for y in 0..FACE_FRAME_SIZE as i32 {
        for x in 0..FACE_FRAME_SIZE as i32 {
            let pixel = face.get(x, y);
            if pixel.a > 0 {
                body.canvas.set(
                    body.face_anchor.x - half + x,
                    body.face_anchor.y - half + y,
                    pixel,
                );
            }
        }
    }
    (body.canvas, body.face_anchor)
}

/// The lowest row a frame paints, counted from the contact point the overlay seats it on. A
/// companion standing on a surface draws its last pixel on the row above it, so that is -1.
fn lowest_row_from_contact(frame: &Canvas, origin_y: i32) -> Option<i32> {
    frame
        .alpha_bounds()
        .map(|(_, _, _, max_y)| origin_y + max_y as i32)
}

/// How close a whole rest cycle comes to the surface it is seated on. Every resting frame shares
/// one seating offset, so the cycle as a whole is what has to land: -1 is a companion whose
/// lowest resting frame draws its last pixel on the row above the surface.
fn resting_reach(creature: &Creature, origin_y: i32) -> Option<i32> {
    [ActionKind::Idle, ActionKind::Perch, ActionKind::Homebound]
        .into_iter()
        .flat_map(|action| {
            (0..AnimationSpec::for_action(action).frames).map(move |frame| (action, frame))
        })
        .filter_map(|(action, frame)| {
            lowest_row_from_contact(
                &CreatureRenderer::render_body_frame(&creature.appearance, action, frame, false)
                    .canvas,
                origin_y,
            )
        })
        .max()
}

/// R05.1: does a companion still read as itself at Small, and is it seated at every size?
///
/// Adults and third-generation minis, at the three zooms the tray offers, each standing on a
/// drawn surface. A plan that reads only at Large, or a silhouette two plans share, is a defect
/// the sheet has to make obvious.
#[test]
fn every_body_plan_reads_and_seats_at_small_medium_and_large() {
    const CELL_W: i32 = 208;
    const CELL_H: i32 = 248;
    const CONTACT: i32 = 208;
    let rows = review_subjects(40).len() as i32;
    let mut sheet = Canvas::new((CELL_W * 6) as u32, (CELL_H * rows) as u32);
    sheet.fill_rect(0, 0, sheet.width() as i32, sheet.height() as i32, PAPER);
    let mut faults = Vec::new();
    let mut silhouettes = Vec::new();
    for (size_index, logical_size) in [40_u8, 21].into_iter().enumerate() {
        for (row, (label, creature)) in review_subjects(logical_size).into_iter().enumerate() {
            let baseline = CreatureRenderer::resting_baseline(&creature.appearance, false);
            let origin = FramePlacement::for_action(ActionKind::Idle, baseline).origin_y;
            let (frame, anchor) = overlay_sprite(&creature, ActionKind::Idle, 0, false);
            if logical_size == 40 {
                silhouettes.push((label.clone(), frame.clone()));
            }
            // A face is only worth its pixels if it still carries eyes and a mouth apart from the
            // coat, so count what the reserved face square actually contains.
            let mut tones = std::collections::BTreeSet::new();
            let half = FACE_FRAME_SIZE as i32 / 2;
            for y in anchor.y - half..anchor.y + half {
                for x in anchor.x - half..anchor.x + half {
                    let pixel = frame.get(x, y);
                    if pixel.a > 0 {
                        tones.insert((pixel.r, pixel.g, pixel.b));
                    }
                }
            }
            if tones.len() < 4 {
                faults.push(format!(
                    "{label} at size {logical_size} draws only {} tones in its face",
                    tones.len()
                ));
            }
            for (zoom_index, zoom) in [2_i32, 3, 4].into_iter().enumerate() {
                let col = size_index as i32 * 3 + zoom_index as i32;
                let (x, y) = (col * CELL_W, row as i32 * CELL_H);
                sheet.fill_rect(x + 4, y + 4, CELL_W - 8, CELL_H - 8, PANEL);
                sheet.fill_rect(
                    x + 4,
                    y + CONTACT,
                    CELL_W - 8,
                    CELL_H - 4 - CONTACT,
                    SURFACE,
                );
                stamp(
                    &mut sheet,
                    &frame,
                    x + CELL_W / 2 - FRAME_SIZE as i32 * zoom / 2,
                    y + CONTACT + origin * zoom,
                    zoom,
                    1.0,
                );
            }
            // Seating is one number for the whole rest cycle, so the claim is about the cycle:
            // its lowest frame lands on the surface and none of them break through it.
            let settled = resting_reach(&creature, origin);
            if settled != Some(-1) {
                faults.push(format!(
                    "{label} at size {logical_size} rests {settled:?} from its surface"
                ));
            }
        }
    }
    for (index, (label, frame)) in silhouettes.iter().enumerate() {
        for (other_label, other) in silhouettes.iter().skip(index + 1) {
            if frame == other {
                faults.push(format!("{label} and {other_label} draw the same companion"));
            }
        }
    }
    save_review("body-plans-and-scales.png", &sheet);
    assert!(faults.is_empty(), "{faults:#?}");
}

/// R05.2: feet on the surface, hands on the ledge, for every shape the renderer draws.
///
/// The resting poses the baseline is measured from, two poses it is not measured from, and the
/// hanging pose that uses its own -7 anchor instead. A shape whose baseline is wrong sinks or
/// never settles here, in the column where it happens.
#[test]
fn feet_and_hanging_hands_meet_the_ledge_for_every_body_plan() {
    const CELL_W: i32 = 176;
    const CELL_H: i32 = 208;
    const CONTACT: i32 = 168;
    let columns = [
        (ActionKind::Idle, 0_u8),
        (ActionKind::Perch, 0),
        (ActionKind::Traverse, 1),
        (ActionKind::Sleep, 0),
        (ActionKind::Eat, 1),
        (ActionKind::Dangle, 0),
    ];
    let subjects = review_subjects(38);
    let mut sheet = Canvas::new(
        (CELL_W * columns.len() as i32) as u32,
        (CELL_H * subjects.len() as i32) as u32,
    );
    sheet.fill_rect(0, 0, sheet.width() as i32, sheet.height() as i32, PAPER);
    let mut faults = Vec::new();
    for (row, (label, creature)) in subjects.into_iter().enumerate() {
        let baseline = CreatureRenderer::resting_baseline(&creature.appearance, false);
        for (col, (action, frame_index)) in columns.into_iter().enumerate() {
            let (x, y) = (col as i32 * CELL_W, row as i32 * CELL_H);
            sheet.fill_rect(x + 4, y + 4, CELL_W - 8, CELL_H - 8, PANEL);
            let hanging = action == ActionKind::Dangle;
            // A dangling companion hangs off the underside of a window, so its surface is
            // everything above the contact row and the room it needs is below it.
            let contact = if hanging { 48 } else { CONTACT };
            if hanging {
                sheet.fill_rect(x + 4, y + 4, CELL_W - 8, contact - 4, SURFACE);
            } else {
                sheet.fill_rect(
                    x + 4,
                    y + contact,
                    CELL_W - 8,
                    CELL_H - 4 - contact,
                    SURFACE,
                );
            }
            sheet.fill_rect(x + 4, y + contact, CELL_W - 8, 1, GUIDE);
            let origin = FramePlacement::for_action(action, baseline).origin_y;
            let (sprite, _) = overlay_sprite(&creature, action, frame_index, false);
            stamp(
                &mut sheet,
                &sprite,
                x + CELL_W / 2 - FRAME_SIZE as i32 * 3 / 2,
                y + contact + origin * 3,
                3,
                1.0,
            );
            let Some((_, min_y, _, max_y)) = sprite.alpha_bounds() else {
                faults.push(format!("{label} {action:?} draws nothing"));
                continue;
            };
            let (top, bottom) = (origin + min_y as i32, origin + max_y as i32);
            if hanging && creature.appearance.design.is_none() {
                // The -7 hanging anchor is measured against the modular plans, which draw their
                // grip on row 7 on purpose. The three legacy families raise their hands from
                // wherever their shoulders happen to sit instead, and the whole pose lands below
                // the ledge holding nothing. R05 found this and it is recorded rather than
                // papered over, so whoever fixes it is sent straight back to this note.
                if top <= 0 {
                    faults.push(format!(
                        "{label} now reaches its ledge; R05's legacy dangle note is stale"
                    ));
                }
            } else if hanging {
                let gripping =
                    (0..FRAME_SIZE as i32).any(|column| sprite.get(column, -origin).a > 0);
                if !gripping || top >= 0 || bottom <= 0 {
                    faults.push(format!(
                        "{label} hangs {top}..{bottom} from the ledge and grips it: {gripping}"
                    ));
                }
            } else if bottom > -1 {
                faults.push(format!(
                    "{label} {action:?} sinks {} px through its surface",
                    bottom + 1
                ));
            } else if resting_reach(&creature, origin) != Some(-1) {
                faults.push(format!("{label} never settles onto its surface"));
            }
        }
    }
    save_review("anchors-by-body-plan.png", &sheet);
    assert!(faults.is_empty(), "{faults:#?}");
}

/// R05.3: a carried thing, drawn where the overlay draws it.
///
/// The prop is its own quad, placed from `PropAnchor` against the face anchor and drawn after the
/// layered face, so it cannot be reviewed from a sprite alone. This rebuilds the overlay's
/// arrangement — body frame, face anchor, prop quad — for both facings and, in the last column,
/// for two companions reaching toward one another, which is the moment the anchor exists to serve.
#[test]
fn a_carried_prop_is_held_in_front_of_every_body_plan_in_both_facings() {
    const CELL: i32 = 232;
    const PAIR: i32 = 464;
    const ROW: i32 = 232;
    const CONTACT: i32 = 216;
    const ZOOM: i32 = 4;
    let subjects = review_subjects(38);
    let mut sheet = Canvas::new(
        (CELL * 2 + PAIR) as u32,
        (ROW * subjects.len() as i32) as u32,
    );
    sheet.fill_rect(0, 0, sheet.width() as i32, sheet.height() as i32, PAPER);
    let mut faults = Vec::new();
    for (row, (label, subject)) in subjects.into_iter().enumerate() {
        let y = row as i32 * ROW;
        sheet.fill_rect(4, y + 4, sheet.width() as i32 - 8, ROW - 8, PANEL);
        sheet.fill_rect(
            4,
            y + CONTACT,
            sheet.width() as i32 - 8,
            ROW - 4 - CONTACT,
            SURFACE,
        );
        let place = |sheet: &mut Canvas, centre: i32, facing_right: bool| {
            let mut creature = subject.clone();
            creature.state.facing_right = facing_right;
            creature.state.action = ActionKind::PresentDiscovery;
            let baseline = CreatureRenderer::resting_baseline(&creature.appearance, false);
            let origin = FramePlacement::for_creature(&creature, baseline).origin_y;
            let left = centre - FRAME_SIZE as i32 * ZOOM / 2;
            let top = y + CONTACT + origin * ZOOM;
            let (sprite, anchor) =
                overlay_sprite(&creature, ActionKind::PresentDiscovery, 2, false);
            stamp(sheet, &sprite, left, top, ZOOM, 1.0);
            let prop_anchor = PropAnchor::for_creature(&creature);
            let prop_x = left + anchor.x * ZOOM + (prop_anchor.dx * ZOOM as f32) as i32;
            let prop_y = top + anchor.y * ZOOM + (prop_anchor.dy * ZOOM as f32) as i32;
            let prop = CreatureRenderer::render_trinket(
                &creature.appearance,
                creature.state.activity_variant,
            );
            let quad = FACE_FRAME_SIZE as i32 * ZOOM / 2;
            stamp(sheet, &prop, prop_x - quad, prop_y - quad, ZOOM, 1.0);
            // The body frame's own edges, so a prop hanging outside one is visible rather than
            // merely arguable.
            for edge in 0..2 {
                let inset = edge * (FRAME_SIZE as i32 * ZOOM - 1);
                sheet.fill_rect(left + inset, top, 1, FRAME_SIZE as i32 * ZOOM, GUIDE);
            }
            (anchor, prop_anchor, prop)
        };
        for (col, facing_right) in [(0, true), (1, false)] {
            let (anchor, prop_anchor, prop) =
                place(&mut sheet, col * CELL + CELL / 2, facing_right);
            // Everything below is in art pixels measured from the face anchor, because that is
            // the frame the overlay itself places the prop quad in.
            let half = FACE_FRAME_SIZE as i32 / 2;
            let (dx, dy) = (prop_anchor.dx as i32, prop_anchor.dy as i32);
            if (dx > 0) != facing_right {
                faults.push(format!(
                    "{label} facing {facing_right} carries its prop {dx} px behind itself"
                ));
            }
            let (quad_left, quad_top) = (anchor.x + dx - half, anchor.y + dy - half);
            let escape = (-quad_left)
                .max(-quad_top)
                .max(quad_left + FACE_FRAME_SIZE as i32 - FRAME_SIZE as i32)
                .max(quad_top + FACE_FRAME_SIZE as i32 - FRAME_SIZE as i32);
            if escape > 0 {
                faults.push(format!(
                    "{label} facing {facing_right}: the prop quad leaves the body frame by {escape} px"
                ));
            }
            // The prop is the one quad drawn over the layered face, so its own pixels — not its
            // mostly empty quad — are what must stay off the eyes. A prop over the eyes is a prop
            // the companion is wearing, not holding.
            for py in 0..FACE_FRAME_SIZE as i32 {
                for px in 0..FACE_FRAME_SIZE as i32 {
                    let (x, y) = (quad_left + px - anchor.x, quad_top + py - anchor.y);
                    if prop.get(px, py).a > 0 && x.abs() <= 4 && y.abs() <= 3 {
                        faults.push(format!(
                            "{label} facing {facing_right}: the prop {prop_anchor:?} covers the eyes"
                        ));
                    }
                }
            }
        }
        place(&mut sheet, CELL * 2 + PAIR / 2 - 92, true);
        place(&mut sheet, CELL * 2 + PAIR / 2 + 92, false);
    }
    save_review("prop-handoff.png", &sheet);
    faults.dedup();
    assert!(faults.is_empty(), "{faults:#?}");
}

/// R05.4: the optional outline, on wallpaper busy enough to need it.
///
/// Plain and outlined side by side at each zoom, over a checker that alternates far lighter and
/// far darker than the coat. The edge has to stay one pixel and stay soft, and the gaps that
/// carry the shape — between limbs, beside ears — have to stay open rather than close into it.
#[test]
fn the_optional_outline_stays_a_soft_single_pixel_edge_at_every_scale() {
    const CELL_W: i32 = 208;
    const CELL_H: i32 = 216;
    let subjects = review_subjects(38);
    let mut sheet = Canvas::new((CELL_W * 6) as u32, (CELL_H * subjects.len() as i32) as u32);
    let mut faults = Vec::new();
    for y in 0..sheet.height() as i32 {
        for x in 0..sheet.width() as i32 {
            let dark = (x / 16 + y / 16) % 2 == 0;
            let tone = if dark {
                Rgba::new(38, 44, 40, 255)
            } else {
                Rgba::new(238, 232, 214, 255)
            };
            sheet.set(x, y, tone);
        }
    }
    for (row, (label, creature)) in subjects.into_iter().enumerate() {
        let plain = CreatureRenderer::render_body_frame(
            &creature.appearance,
            ActionKind::Traverse,
            1,
            false,
        )
        .canvas;
        let mut edged = plain.clone();
        CreatureRenderer::outline_frame(&mut edged);
        let plain_bounds = plain
            .alpha_bounds()
            .expect("a walking companion is visible");
        let edged_bounds = edged
            .alpha_bounds()
            .expect("an outlined companion is visible");
        for (before, after, edge) in [
            (plain_bounds.0, edged_bounds.0, "left"),
            (plain_bounds.1, edged_bounds.1, "top"),
            (edged_bounds.2, plain_bounds.2, "right"),
            (edged_bounds.3, plain_bounds.3, "bottom"),
        ] {
            if before.abs_diff(after) > 1 {
                faults.push(format!(
                    "{label}: the outline reaches {} px past the {edge} edge",
                    before.abs_diff(after)
                ));
            }
        }
        for py in 0..FRAME_SIZE as i32 {
            for px in 0..FRAME_SIZE as i32 {
                let (was, now) = (plain.get(px, py), edged.get(px, py));
                if was.a > 16 && now != was {
                    faults.push(format!("{label}: the outline repainted the companion"));
                } else if was.a <= 16 && now.a == u8::MAX {
                    faults.push(format!("{label}: the outline is opaque, not a soft edge"));
                }
                // A one-pixel slot — between two limbs, beside an ear — is reached from both
                // sides at once, so the outline does fill it. What matters is that it fills it
                // with the same soft edge it draws everywhere else, which still reads as a seam.
                // Anything heavier would weld the two parts into a single shape.
                let solid = |x: i32, y: i32| plain.get(x, y).a > 16;
                if was.a <= 16
                    && now.a > 150
                    && ((solid(px - 1, py) && solid(px + 1, py))
                        || (solid(px, py - 1) && solid(px, py + 1)))
                {
                    faults.push(format!(
                        "{label}: a one-pixel gap at {px},{py} is welded shut, not seamed"
                    ));
                }
            }
        }
        for (zoom_index, zoom) in [2_i32, 3, 4].into_iter().enumerate() {
            for (variant, frame) in [(0, &plain), (1, &edged)] {
                let col = zoom_index as i32 * 2 + variant;
                let (x, y) = (col * CELL_W, row as i32 * CELL_H);
                stamp(
                    &mut sheet,
                    frame,
                    x + CELL_W / 2 - FRAME_SIZE as i32 * zoom / 2,
                    y + CELL_H / 2 - FRAME_SIZE as i32 * zoom / 2,
                    zoom,
                    1.0,
                );
            }
        }
    }
    save_review("outline-on-busy-wallpaper.png", &sheet);
    faults.dedup();
    assert!(faults.is_empty(), "{faults:#?}");
}

/// R05.5: the tight places. A gap narrow enough that the overlay squeezes the sprite sideways,
/// and a ledge barely longer than the companion standing on it.
#[test]
fn narrow_gaps_and_short_ledges_stage_every_body_plan() {
    const CELL_W: i32 = 224;
    const CELL_H: i32 = 240;
    const CONTACT: i32 = 200;
    const SQUEEZE: f32 = 0.72;
    let subjects = review_subjects(38);
    let mut sheet = Canvas::new((CELL_W * 6) as u32, (CELL_H * subjects.len() as i32) as u32);
    sheet.fill_rect(0, 0, sheet.width() as i32, sheet.height() as i32, PAPER);
    let mut faults = Vec::new();
    for (row, (label, creature)) in subjects.into_iter().enumerate() {
        let baseline = CreatureRenderer::resting_baseline(&creature.appearance, false);
        for (zoom_index, zoom) in [2_i32, 3, 4].into_iter().enumerate() {
            for (situation, action) in [(0, ActionKind::SqueezeWindow), (1, ActionKind::Perch)] {
                let col = situation * 3 + zoom_index as i32;
                let (x, y) = (col * CELL_W, row as i32 * CELL_H);
                sheet.fill_rect(x + 4, y + 4, CELL_W - 8, CELL_H - 8, PANEL);
                let origin = FramePlacement::for_action(action, baseline).origin_y;
                let (sprite, _) = overlay_sprite(&creature, action, 1, false);
                let Some((min_x, _, max_x, max_y)) = sprite.alpha_bounds() else {
                    faults.push(format!("{label} {action:?} draws nothing"));
                    continue;
                };
                let squeeze = if situation == 0 { SQUEEZE } else { 1.0 };
                let drawn = (FRAME_SIZE as f32 * zoom as f32 * squeeze).round() as i32;
                let centre = x + CELL_W / 2;
                if situation == 0 {
                    // Two window edges, opened to exactly the width the narrowed body needs, so
                    // the sheet shows whether it really clears them.
                    let body = (max_x - min_x + 1) as f32 * zoom as f32 * squeeze;
                    let half = (body.round() as i32 + zoom) / 2;
                    sheet.fill_rect(x + 4, y + 4, centre - half - x - 4, CELL_H - 8, SURFACE);
                    sheet.fill_rect(
                        centre + half,
                        y + 4,
                        x + CELL_W - 4 - centre - half,
                        CELL_H - 8,
                        SURFACE,
                    );
                } else {
                    // A ledge only a little longer than the companion is wide.
                    let ledge = FRAME_SIZE as i32 * zoom * 3 / 4;
                    sheet.fill_rect(
                        centre - ledge / 2,
                        y + CONTACT,
                        ledge,
                        CELL_H - 4 - CONTACT,
                        SURFACE,
                    );
                }
                sheet.fill_rect(x + 4, y + CONTACT, CELL_W - 8, 1, GUIDE);
                stamp(
                    &mut sheet,
                    &sprite,
                    centre - drawn / 2,
                    y + CONTACT + origin * zoom,
                    zoom,
                    squeeze,
                );
                // A walk cycle lifts, so the claim here is only that neither pose breaks through
                // the ledge it is standing on, and that the squeeze really narrows the body.
                if origin + max_y as i32 > -1 {
                    faults.push(format!(
                        "{label} {action:?} at {zoom}x sinks {} px through the ledge",
                        origin + max_y as i32 + 1
                    ));
                }
                if situation == 0
                    && (max_x - min_x + 1) as f32 * SQUEEZE >= (max_x - min_x + 1) as f32
                {
                    faults.push(format!("{label} does not narrow to slip through a gap"));
                }
            }
        }
    }
    save_review("narrow-gaps-and-short-ledges.png", &sheet);
    faults.dedup();
    assert!(faults.is_empty(), "{faults:#?}");
}
