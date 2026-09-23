use super::*;

/// Turning window ledges off is a request, not a hint. A colony already on the floor stays
/// there, however inviting the window above it looks.
#[test]
fn turning_window_ledges_off_keeps_the_colony_on_the_floor() {
    let created = datetime!(2026-04-01 0:00 UTC);
    let mut desktop = desktop();
    desktop.window_sample = Some(WindowSample {
        monotonic_millis: 0,
        reliable: true,
    });
    desktop.windows.push(DesktopWindow {
        key: 901,
        bounds: DesktopRect {
            x: 260.0,
            y: 600.0,
            width: 560.0,
            height: 230.0,
        },
        z_order: 0,
        visible: true,
        minimized: false,
        application: None,
        application_name: None,
    });
    let mut world = two_creature_world([64; 32], created);
    let now = created + Duration::hours(1);
    let tick = |world: &mut World, desktop: &mut DesktopSnapshot, step: i64| {
        desktop.window_sample.as_mut().unwrap().monotonic_millis = step as u64 * 50;
        world.tick(now + Duration::milliseconds(step * 50), 0.05, desktop);
    };
    // With ledges allowed, this desktop is one a creature does climb onto.
    let mut climbed = false;
    for step in 1..=3_000 {
        tick(&mut world, &mut desktop, step);
        climbed |= world
            .save
            .creatures
            .iter()
            .any(|c| c.state.surface.kind == SurfaceKind::WindowLedge);
        if climbed {
            break;
        }
    }
    assert!(
        climbed,
        "the fixture has to be a desktop worth climbing, or the preference proves nothing"
    );
    // Turn the preference off and put everyone back on the floor.
    let mut world = two_creature_world([64; 32], created);
    world.save.settings.window_ledges = false;
    for step in 1..=1_200 {
        tick(&mut world, &mut desktop, step);
        assert!(
            world
                .save
                .creatures
                .iter()
                .all(|c| c.state.surface.kind != SurfaceKind::WindowLedge),
            "a creature climbed a ledge at step {step} with the preference off"
        );
        assert!(world.window_journeys.is_empty(), "and none set off for one");
    }
}

#[test]
fn perched_creature_rides_and_falls_from_window() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let mut desktop = desktop();
    desktop.windows.push(DesktopWindow {
        key: 44,
        bounds: DesktopRect {
            x: 200.0,
            y: 300.0,
            width: 600.0,
            height: 400.0,
        },
        z_order: 0,
        visible: true,
        minimized: false,
        application: None,
        application_name: None,
    });
    let mut world = World::new([8; 32], created, &desktop);
    let_colony_wander(&mut world, created);
    let creature = &mut world.save.creatures[0];
    creature.state.surface = SurfaceAttachment {
        kind: SurfaceKind::WindowLedge,
        monitor_id: 1,
        window_key: Some(44),
        relative_x: 0.5,
    };
    world.tick(created, 0.05, &desktop);
    assert_eq!(
        world.save.creatures[0].state.position,
        Point { x: 500.0, y: 300.0 }
    );
    desktop.windows[0].bounds.x = 240.0;
    desktop.windows[0].bounds.y = 280.0;
    world.tick(created, 0.05, &desktop);
    assert_eq!(world.save.creatures[0].state.position.y, 280.0);
    assert_eq!(world.save.creatures[0].state.action, ActionKind::RideWindow);
    world.save.creatures[0].personality.window_tolerance = 0.0;
    desktop.windows[0].bounds.x = 500.0;
    world.tick(created, 0.05, &desktop);
    assert_eq!(
        world.save.creatures[0].state.action,
        ActionKind::ReactToWindow
    );
    desktop.windows.clear();
    world.tick(created, 0.05, &desktop);
    assert_eq!(
        world.save.creatures[0].state.surface.kind,
        SurfaceKind::ScreenFloor
    );
    assert_eq!(
        world.save.creatures[0].state.action,
        ActionKind::ReactToWindow
    );
}

#[test]
fn a_native_identifier_change_with_the_same_frame_keeps_the_perch() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let mut desktop = desktop();
    let window = DesktopWindow {
        key: 44,
        bounds: DesktopRect {
            x: 200.0,
            y: 300.0,
            width: 600.0,
            height: 400.0,
        },
        z_order: 0,
        visible: true,
        minimized: false,
        application: None,
        application_name: None,
    };
    desktop.windows.push(window.clone());
    let mut world = World::new([8; 32], created, &desktop);
    let_colony_wander(&mut world, created);
    world.save.creatures[0].state.surface = SurfaceAttachment {
        kind: SurfaceKind::WindowLedge,
        monitor_id: 1,
        window_key: Some(44),
        relative_x: 0.5,
    };
    world.tick(created, 0.05, &desktop);
    desktop.windows[0].key = 45;
    world.tick(created, 0.05, &desktop);
    let c = &world.save.creatures[0];
    assert_eq!(c.state.surface.window_key, Some(45));
    assert_eq!(c.state.position, Point { x: 500.0, y: 300.0 });
    assert_ne!(c.state.action, ActionKind::ReactToWindow);
    // Two new windows with that frame are ambiguous: fall back to ordinary support recovery.
    let mut twin = window;
    twin.key = 47;
    desktop.windows[0].key = 46;
    desktop.windows.push(twin);
    world.tick(created, 0.05, &desktop);
    assert_eq!(
        world.save.creatures[0].state.surface.kind,
        SurfaceKind::ScreenFloor
    );
}

#[test]
fn nearby_window_novelty_preserves_sleep_but_attracts_an_awake_creature() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let mut desktop = desktop();
    let mut world = World::new([31; 32], created, &desktop);
    let_colony_wander(&mut world, created);
    world.tick(created, 0.05, &desktop);
    let position = world.save.creatures[0].state.position;
    world.save.creatures[0].state.action = ActionKind::Sleep;
    world.save.creatures[0].state.action_elapsed = 5.0;
    world.save.creatures[0].state.action_duration = 100.0;
    desktop.windows.push(DesktopWindow {
        key: 91,
        bounds: DesktopRect {
            x: position.x - 180.0,
            y: position.y - 260.0,
            width: 360.0,
            height: 120.0,
        },
        z_order: 0,
        visible: true,
        minimized: false,
        application: None,
        application_name: None,
    });
    world.tick(created, 0.05, &desktop);
    assert_eq!(world.save.creatures[0].state.action, ActionKind::Sleep);
    world.save.creatures[0].state.action = ActionKind::Idle;
    world.save.creatures[0].personality.curiosity = 1.0;
    world.tick(created, 0.05, &desktop);
    assert_eq!(
        world.save.creatures[0].state.action,
        ActionKind::InspectScreen
    );
    assert_eq!(
        world.save.creatures[0].state.attention.unwrap().emotion,
        AttentionEmotion::Curious
    );
}

#[test]
fn most_creatures_discover_a_reachable_window_within_one_minute() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let mut discovered = 0;
    for seed_byte in 0_u8..20 {
        let mut desktop = desktop();
        let mut world = World::new([seed_byte; 32], created, &desktop);
        let_colony_wander(&mut world, created);
        let start = world.save.creatures[0].state.position;
        desktop.windows.push(DesktopWindow {
            key: 100 + u64::from(seed_byte),
            bounds: DesktopRect {
                x: start.x - 180.0,
                y: start.y - 320.0,
                width: 420.0,
                height: 250.0,
            },
            z_order: 0,
            visible: true,
            minimized: false,
            application: None,
            application_name: None,
        });
        for _ in 0..1_200 {
            world.tick(created, 0.05, &desktop);
            if world.save.creatures[0].state.surface.kind == SurfaceKind::WindowLedge {
                discovered += 1;
                break;
            }
        }
    }
    assert!(
        discovered >= 16,
        "only {discovered}/20 creatures found the ledge"
    );
}

#[test]
fn perched_creature_can_choose_a_different_window_height() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let mut desktop = desktop();
    desktop.windows.extend([
        DesktopWindow {
            key: 201,
            bounds: DesktopRect {
                x: 360.0,
                y: 610.0,
                width: 460.0,
                height: 210.0,
            },
            z_order: 1,
            visible: true,
            minimized: false,
            application: None,
            application_name: None,
        },
        DesktopWindow {
            key: 202,
            bounds: DesktopRect {
                x: 470.0,
                y: 330.0,
                width: 520.0,
                height: 300.0,
            },
            z_order: 0,
            visible: true,
            minimized: false,
            application: None,
            application_name: None,
        },
    ]);
    let mut world = World::new([41; 32], created, &desktop);
    let_colony_wander(&mut world, created);
    let creature = &mut world.save.creatures[0];
    creature.state.position = Point { x: 590.0, y: 610.0 };
    creature.state.surface = SurfaceAttachment {
        kind: SurfaceKind::WindowLedge,
        monitor_id: 1,
        window_key: Some(201),
        relative_x: 0.5,
    };

    let mut topology = DesktopTopology::default();
    topology.rebuild_if_changed(&desktop, &BTreeMap::new());
    let (target, surface) = find_nearby_ledge(
        creature,
        &desktop,
        &world.save.settings.habitat,
        &topology,
        world.save.settings.display_scale,
    )
    .expect("the upper window should be a reachable transfer");
    assert_eq!(surface.window_key, Some(202));
    assert_eq!(target.y, 330.0);
}

#[test]
fn creatures_explore_multiple_window_levels() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let mut explorers = 0;
    for seed_byte in 0_u8..20 {
        let mut desktop = desktop();
        let mut world = World::new([seed_byte; 32], created, &desktop);
        let_colony_wander(&mut world, created);
        let start = world.save.creatures[0].state.position;
        desktop.windows.extend([
            DesktopWindow {
                key: 300 + u64::from(seed_byte) * 3,
                bounds: DesktopRect {
                    x: start.x - 180.0,
                    y: start.y - 190.0,
                    width: 410.0,
                    height: 150.0,
                },
                z_order: 2,
                visible: true,
                minimized: false,
                application: None,
                application_name: None,
            },
            DesktopWindow {
                key: 301 + u64::from(seed_byte) * 3,
                bounds: DesktopRect {
                    x: start.x - 90.0,
                    y: start.y - 390.0,
                    width: 440.0,
                    height: 180.0,
                },
                z_order: 1,
                visible: true,
                minimized: false,
                application: None,
                application_name: None,
            },
            DesktopWindow {
                key: 302 + u64::from(seed_byte) * 3,
                bounds: DesktopRect {
                    x: start.x - 210.0,
                    y: start.y - 590.0,
                    width: 520.0,
                    height: 190.0,
                },
                z_order: 0,
                visible: true,
                minimized: false,
                application: None,
                application_name: None,
            },
        ]);
        // Four minutes: long enough for a spell back down on the floor between climbs, which
        // every companion that likes to be anywhere now takes.
        let mut visited = std::collections::BTreeSet::new();
        for _ in 0..4_800 {
            world.tick(created, 0.05, &desktop);
            if let Some(key) = world.save.creatures[0].state.surface.window_key {
                visited.insert(key);
            }
            if visited.len() >= 2 {
                explorers += 1;
                break;
            }
        }
    }
    assert!(
        explorers >= 16,
        "only {explorers}/20 creatures explored more than one window level"
    );
}
