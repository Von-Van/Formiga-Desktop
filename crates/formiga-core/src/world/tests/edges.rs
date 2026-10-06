//! The sides of a display, and of whatever the habitat allows on it, are walls a companion keeps
//! its whole self behind. Each display's overlay draws only the companions standing on it, so a
//! frame reaching past a seam is cut off there; and past the side of an allowed region it is drawn
//! where the person asked it not to be.
use super::*;

/// Two displays the way Windows usually has them: a 1920 × 1080 laptop panel at 125% with its
/// taskbar along the bottom, and beside it a 1280 × 1024 monitor at 100%, bottom-aligned.
fn side_by_side() -> DesktopSnapshot {
    DesktopSnapshot {
        monitors: vec![
            MonitorInfo {
                id: 1,
                display_key: DisplayKey([1; 16]),
                bounds: DesktopRect {
                    x: 0.0,
                    y: 0.0,
                    width: 1920.0,
                    height: 1080.0,
                },
                usable_bounds: DesktopRect {
                    x: 0.0,
                    y: 0.0,
                    width: 1920.0,
                    height: 1032.0,
                },
                scale_factor: 1.25,
                primary: true,
            },
            MonitorInfo {
                id: 2,
                display_key: DisplayKey([2; 16]),
                bounds: DesktopRect {
                    x: 1920.0,
                    y: 56.0,
                    width: 1280.0,
                    height: 1024.0,
                },
                usable_bounds: DesktopRect {
                    x: 1920.0,
                    y: 56.0,
                    width: 1280.0,
                    height: 976.0,
                },
                scale_factor: 1.0,
                primary: false,
            },
        ],
        ..DesktopSnapshot::default()
    }
}

fn on_floor(creature: &mut Creature, monitor: &MonitorInfo, x: f32) {
    creature.state.position = Point {
        x,
        y: monitor.usable_bounds.bottom() - 4.0,
    };
    creature.state.surface = SurfaceAttachment {
        kind: SurfaceKind::ScreenFloor,
        monitor_id: monitor.id,
        window_key: None,
        relative_x: 0.5,
    };
}

#[test]
fn half_a_body_is_measured_in_each_displays_own_points() {
    let desktop = side_by_side();
    // 48 pixels at 3×, drawn in physical pixels: 115.2 points across at 125%, 144 at 100%.
    assert_eq!(body_half_width(&desktop.monitors[0], 3), 57.6);
    assert_eq!(body_half_width(&desktop.monitors[1], 3), 72.0);
    // However small a companion is drawn, a wall still leaves its feet the old eight points.
    assert_eq!(body_half_width(&desktop.monitors[1], 0), 8.0);
}

#[test]
fn a_run_narrower_than_a_body_has_one_place_to_stand_in_its_middle() {
    assert_eq!(standing_span(100.0, 400.0, 50.0), (150.0, 350.0));
    assert_eq!(standing_span(100.0, 160.0, 50.0), (130.0, 130.0));
    let region = DesktopRect {
        x: 100.0,
        y: 0.0,
        width: 300.0,
        height: 100.0,
    };
    assert_eq!(clamp_to_standing(110.0, region, 50.0), 150.0);
    assert_eq!(clamp_to_standing(250.0, region, 50.0), 250.0);
}

#[test]
fn regions_cut_around_an_exclusion_keep_a_point_any_of_them_already_holds_whole() {
    // An exclusion that does not reach the floor leaves the floor whole under it, as the strip
    // below the cut, beside the strips either side of it.
    let below = DesktopRect {
        x: 0.0,
        y: 500.0,
        width: 1000.0,
        height: 500.0,
    };
    let left = DesktopRect {
        x: 0.0,
        y: 0.0,
        width: 300.0,
        height: 1000.0,
    };
    let regions = [left, below];
    let point = Point { x: 290.0, y: 900.0 };
    assert_eq!(keep_whole_in(point, &regions, 50.0), point);
    // Above the cut only the left strip holds it, and its side is a wall.
    let above = Point { x: 290.0, y: 100.0 };
    assert_eq!(keep_whole_in(above, &regions, 50.0).x, 250.0);
    // A point in none of them is the caller's to deal with.
    let outside = Point {
        x: 2_000.0,
        y: 100.0,
    };
    assert_eq!(keep_whole_in(outside, &regions, 50.0), outside);
}

#[test]
fn a_companion_by_a_seam_is_brought_back_onto_its_own_display_whole() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = side_by_side();
    let mut world = two_creature_world([97; 32], created);
    on_floor(&mut world.save.creatures[0], &desktop.monitors[0], 1_915.0);
    world.save.creatures[0].state.facing_right = true;
    on_floor(&mut world.save.creatures[1], &desktop.monitors[1], 1_925.0);
    world.save.creatures[1].state.facing_right = false;
    keep_creatures_in_habitat(
        &mut world.save.creatures,
        &desktop,
        &world.save.settings.habitat,
        &[],
        3,
    );
    let [left, right] = [&world.save.creatures[0], &world.save.creatures[1]];
    assert_eq!(left.state.position.x, 1_920.0 - 57.6);
    assert!(!left.state.facing_right, "turned back from the wall");
    assert_eq!(left.state.surface.monitor_id, 1);
    assert_eq!(right.state.position.x, 1_920.0 + 72.0);
    assert!(right.state.facing_right);
    assert_eq!(right.state.surface.monitor_id, 2);
}

#[test]
fn walking_into_a_seam_or_a_region_side_is_arriving_at_a_wall_half_a_body_in() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = side_by_side();
    let mut world = two_creature_world([97; 32], created);
    let creature = &mut world.save.creatures[0];
    on_floor(creature, &desktop.monitors[0], 1_900.0);
    creature.state.velocity.x = 40.0;
    assert!(constrain_to_surface(
        creature,
        &desktop,
        &HabitatPolicy::default(),
        3
    ));
    assert_eq!(creature.state.position.x, 1_920.0 - 57.6);
    assert_eq!(creature.state.velocity.x, 0.0);

    // The bottom corners of the second display: the left corner's inner side is a wall too.
    let corners = HabitatPolicy {
        preset: HabitatPreset::BottomCorners,
        zones: Vec::new(),
    };
    let corner = accessible_regions(&corners, &desktop.monitors[1])[0];
    on_floor(creature, &desktop.monitors[1], corner.right() - 4.0);
    assert!(constrain_to_surface(creature, &desktop, &corners, 3));
    assert_eq!(creature.state.position.x, corner.right() - 72.0);
    assert!(!creature.state.facing_right);
}

#[test]
fn a_ledge_running_past_the_side_of_its_display_ends_where_the_whole_companion_still_shows() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let mut desktop = side_by_side();
    // A window dragged half onto the second display: its top edge runs straight across the seam.
    desktop.windows.push(DesktopWindow {
        key: 70,
        bounds: DesktopRect {
            x: 1_500.0,
            y: 600.0,
            width: 800.0,
            height: 300.0,
        },
        z_order: 0,
        visible: true,
        minimized: false,
        application: None,
        application_name: None,
    });
    let mut world = two_creature_world([97; 32], created);
    let creature = &mut world.save.creatures[0];
    creature.state.position = Point {
        x: 1_918.0,
        y: 600.0,
    };
    creature.state.surface = SurfaceAttachment {
        kind: SurfaceKind::WindowLedge,
        monitor_id: 1,
        window_key: Some(70),
        relative_x: 0.5,
    };
    assert!(constrain_to_surface(
        creature,
        &desktop,
        &HabitatPolicy::default(),
        3
    ));
    assert_eq!(creature.state.position.x, 1_920.0 - 57.6);
    assert_eq!(creature.state.position.y, 600.0);
}

#[test]
fn a_companion_let_go_by_a_seam_lands_whole_on_the_display_under_the_cursor() {
    let desktop = side_by_side();
    let policy = HabitatPolicy::default();
    let (point, surface) = find_drop_support(
        Point {
            x: 1_910.0,
            y: 400.0,
        },
        &desktop,
        &policy,
        true,
        3,
    )
    .expect("the floor catches it");
    assert_eq!(surface.monitor_id, 1);
    assert_eq!(point.x, 1_920.0 - 57.6);
    let (point, surface) = find_drop_support(
        Point {
            x: 1_925.0,
            y: 400.0,
        },
        &desktop,
        &policy,
        true,
        3,
    )
    .expect("the floor catches it");
    assert_eq!(surface.monitor_id, 2);
    assert_eq!(point.x, 1_920.0 + 72.0);
}

/// Over a long session on two displays, with the habitat cut down to the bottom corners of each
/// and windows sliding across the seam, no companion that is simply standing or walking about is
/// ever drawn past the side of the display it is on or of the corner it is allowed — starting
/// with two who begin the session pressed up against exactly those walls.
#[test]
fn over_a_long_session_nobody_settled_reaches_past_a_seam_or_a_corner() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let mut desktop = side_by_side();
    desktop.window_sample = Some(WindowSample {
        monotonic_millis: 0,
        reliable: true,
    });
    for (key, x, y) in [
        (81_u64, 1_500.0, 700.0),
        (82, 200.0, 760.0),
        (83, 2_900.0, 720.0),
    ] {
        desktop.windows.push(DesktopWindow {
            key,
            bounds: DesktopRect {
                x,
                y,
                width: 640.0,
                height: 260.0,
            },
            z_order: key as u32,
            visible: true,
            minimized: false,
            application: None,
            application_name: None,
        });
    }
    let mut world = World::new_original([53; 32], created, &desktop);
    world.save.settings.habitat.preset = HabitatPreset::BottomCorners;
    let now = created + Duration::days(2);
    world.tick(now, 0.05, &desktop);
    let_colony_wander(&mut world, now);
    world.pending_home_greetings.clear();
    assert!(world.save.creatures.len() >= 2);
    // One starts against the inner side of the first display's left corner, the other against
    // the seam on the second display, both heading into the wall.
    let first_corner = accessible_regions(&world.save.settings.habitat, &desktop.monitors[0])[0];
    let (first, second) = (desktop.monitors[0].clone(), desktop.monitors[1].clone());
    on_floor(
        &mut world.save.creatures[0],
        &first,
        first_corner.right() - 2.0,
    );
    world.save.creatures[0].state.facing_right = true;
    on_floor(&mut world.save.creatures[1], &second, 1_922.0);
    world.save.creatures[1].state.facing_right = false;
    let mut worst: f32 = 0.0;
    let mut checked = 0_u32;
    for step in 1..=3_000_i64 {
        // The first window slides back and forth across the seam.
        let phase = (step % 200) as f32 / 200.0 * std::f32::consts::TAU;
        desktop.windows[0].bounds.x = 1_500.0 + phase.sin() * 300.0;
        desktop.window_sample.as_mut().unwrap().monotonic_millis = step as u64 * 50;
        world.tick(now + Duration::milliseconds(step * 50), 0.05, &desktop);
        world.drain_events().for_each(drop);
        for creature in &world.save.creatures {
            if creature.state.indoors
                || creature.state.arrival_delay_secs > 0.0
                || world.window_journeys.contains_key(&creature.id)
                || world.tosses.contains_key(&creature.id)
                || world.attention.crossing_ids().any(|id| id == creature.id)
                || world.wonders.players().any(|id| id == creature.id)
            {
                continue;
            }
            let Some(monitor) = desktop
                .monitors
                .iter()
                .find(|m| m.id == creature.state.surface.monitor_id)
            else {
                continue;
            };
            // Measured from the frame the overlay draws, not from the rule under test.
            let half = CREATURE_FRAME_WIDTH * f32::from(world.save.settings.display_scale)
                / monitor.scale_factor
                / 2.0;
            let regions = accessible_regions(&world.save.settings.habitat, monitor);
            let x = creature.state.position.x;
            let overhang = regions
                .iter()
                .filter(|region| region.contains(creature.state.position))
                .map(|region| {
                    (region.x - (x - half))
                        .max((x + half) - region.right())
                        .max(0.0)
                })
                .fold(f32::INFINITY, f32::min);
            if overhang.is_finite() {
                checked += 1;
                worst = worst.max(overhang);
            }
        }
    }
    assert!(checked > 1_000, "hardly anybody was looked at: {checked}");
    assert!(
        worst <= 0.5,
        "a settled companion reached {worst:.1} points past its display or its corner"
    );
}
