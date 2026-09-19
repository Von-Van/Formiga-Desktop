use super::*;

#[test]
fn drag_release_lands_inside_the_desktop_habitat() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    let mut world = World::new([21; 32], created, &desktop);
    let creature_id = world.save.creatures[0].id;
    let original = world.save.creatures[0].state.position;
    assert!(world.handle_command(
        WorldCommand::BeginInteraction {
            creature_id,
            cursor: original,
        },
        &desktop,
    ));
    assert_eq!(world.save.creatures[0].state.action, ActionKind::Idle);
    assert!(world.handle_command(
        WorldCommand::UpdateInteraction {
            cursor: Point { x: 900.0, y: 300.0 },
            velocity: Point::default(),
        },
        &desktop,
    ));
    assert_eq!(world.save.creatures[0].state.action, ActionKind::Dragged);
    assert!(world.handle_command(
        WorldCommand::EndInteraction {
            cursor: Point { x: 900.0, y: 300.0 },
            velocity: Point::default(),
        },
        &desktop,
    ));
    let creature = &world.save.creatures[0];
    assert_eq!(creature.state.action, ActionKind::Landing);
    assert!(
        desktop.monitors[0]
            .usable_bounds
            .contains(creature.state.position)
    );
    assert_eq!(creature.state.surface.kind, SurfaceKind::ScreenFloor);
}

#[test]
fn click_without_drag_pets_and_does_not_dismiss_the_shelter() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    let mut world = World::new([81; 32], created, &desktop);
    world.tick(created, 0.05, &desktop);
    finish_home_approach(&mut world, created, &desktop);
    let creature_id = world.save.creatures[0].id;
    let position = world.save.creatures[0].state.position;
    assert!(world.save.home.is_active());
    world.handle_command(
        WorldCommand::BeginInteraction {
            creature_id,
            cursor: position,
        },
        &desktop,
    );
    world.handle_command(
        WorldCommand::EndInteraction {
            cursor: Point {
                x: position.x + DRAG_THRESHOLD,
                y: position.y,
            },
            velocity: Point::default(),
        },
        &desktop,
    );
    let creature = &world.save.creatures[0];
    assert!(world.save.home.is_active());
    assert_eq!(creature.state.action, ActionKind::PetReaction);
    assert_eq!(creature.memory.times_petted, 1);
    assert_eq!(creature.tendencies.cursor_trust, 3);
    assert_eq!(creature.tendencies.sociability, 2);
}

#[test]
fn drag_out_and_back_uses_maximum_excursion_instead_of_release_position() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    let mut world = World::new([82; 32], created, &desktop);
    let creature_id = world.save.creatures[0].id;
    let start = world.save.creatures[0].state.position;
    world.handle_command(
        WorldCommand::BeginInteraction {
            creature_id,
            cursor: start,
        },
        &desktop,
    );
    world.handle_command(
        WorldCommand::UpdateInteraction {
            cursor: Point {
                x: start.x + DRAG_THRESHOLD + 0.1,
                y: start.y,
            },
            velocity: Point::default(),
        },
        &desktop,
    );
    world.handle_command(
        WorldCommand::UpdateInteraction {
            cursor: start,
            velocity: Point::default(),
        },
        &desktop,
    );
    world.handle_command(
        WorldCommand::EndInteraction {
            cursor: start,
            velocity: Point::default(),
        },
        &desktop,
    );
    assert_eq!(world.save.creatures[0].memory.times_petted, 0);
    assert_eq!(world.save.creatures[0].state.action, ActionKind::Landing);
}

#[test]
fn cancelled_drag_restores_the_last_safe_state() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    let mut world = World::new([22; 32], created, &desktop);
    let creature_id = world.save.creatures[0].id;
    let original_position = world.save.creatures[0].state.position;
    let original_surface = world.save.creatures[0].state.surface.clone();
    let original_action = world.save.creatures[0].state.action;
    assert!(world.handle_command(
        WorldCommand::BeginInteraction {
            creature_id,
            cursor: original_position,
        },
        &desktop,
    ));
    world.handle_command(
        WorldCommand::UpdateInteraction {
            cursor: Point {
                x: -500.0,
                y: -500.0,
            },
            velocity: Point::default(),
        },
        &desktop,
    );
    assert!(world.handle_command(WorldCommand::CancelInteraction, &desktop));
    let creature = &world.save.creatures[0];
    assert_eq!(creature.state.position, original_position);
    assert_eq!(creature.state.surface, original_surface);
    assert_eq!(creature.state.action, original_action);
}

#[test]
fn toss_release_threshold_and_reduced_motion_preserve_precise_placement() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    let mut world = World::new([37; 32], created, &desktop);
    let creature_id = world.save.creatures[0].id;
    let start = world.save.creatures[0].state.position;
    assert!(world.handle_command(
        WorldCommand::BeginInteraction {
            creature_id,
            cursor: start,
        },
        &desktop,
    ));
    assert!(world.handle_command(
        WorldCommand::UpdateInteraction {
            cursor: Point {
                x: start.x + 12.0,
                y: start.y,
            },
            velocity: Point { x: 300.0, y: 0.0 },
        },
        &desktop,
    ));
    assert!(world.handle_command(
        WorldCommand::EndInteraction {
            cursor: start,
            velocity: Point { x: 300.0, y: 0.0 },
        },
        &desktop,
    ));
    assert_eq!(world.save.creatures[0].state.action, ActionKind::Tossed);
    assert!(world.tosses.contains_key(&creature_id));
    assert_eq!(world.save.creatures[0].state.velocity.x, 195.0);
    assert!(world.drain_events().any(|event| matches!(
        event,
        WorldEvent::DragEnded {
            outcome: DragReleaseKind::Tossed { .. },
            ..
        }
    )));

    world.save.settings.paused = true;
    world.tick(created, 0.05, &desktop);
    assert!(world.tosses.is_empty());
    assert_eq!(world.save.creatures[0].state.action, ActionKind::Landing);
    assert!(world.drain_events().any(|event| matches!(
        event,
        WorldEvent::TossLanded {
            creature_id: landed_id,
            surface: SurfaceKind::ScreenFloor,
            ..
        } if landed_id == creature_id
    )));

    let mut below_threshold = World::new([45; 32], created, &desktop);
    let creature_id = below_threshold.save.creatures[0].id;
    let start = below_threshold.save.creatures[0].state.position;
    below_threshold.handle_command(
        WorldCommand::BeginInteraction {
            creature_id,
            cursor: start,
        },
        &desktop,
    );
    below_threshold.handle_command(
        WorldCommand::EndInteraction {
            cursor: start,
            velocity: Point { x: 219.0, y: 0.0 },
        },
        &desktop,
    );
    assert_eq!(
        below_threshold.save.creatures[0].state.action,
        ActionKind::PetReaction
    );
    assert_eq!(below_threshold.save.creatures[0].memory.times_petted, 1);
    assert!(below_threshold.tosses.is_empty());

    let mut reduced = World::new([38; 32], created, &desktop);
    reduced.save.settings.reduce_motion = true;
    let creature_id = reduced.save.creatures[0].id;
    let start = reduced.save.creatures[0].state.position;
    reduced.handle_command(
        WorldCommand::BeginInteraction {
            creature_id,
            cursor: start,
        },
        &desktop,
    );
    reduced.handle_command(
        WorldCommand::UpdateInteraction {
            cursor: Point {
                x: start.x + 12.0,
                y: start.y,
            },
            velocity: Point {
                x: 900.0,
                y: -100.0,
            },
        },
        &desktop,
    );
    reduced.handle_command(
        WorldCommand::EndInteraction {
            cursor: start,
            velocity: Point {
                x: 900.0,
                y: -100.0,
            },
        },
        &desktop,
    );
    assert_eq!(reduced.save.creatures[0].state.action, ActionKind::Idle);
    assert!(reduced.tosses.is_empty());
}

#[test]
fn drag_velocity_history_is_fixed_capacity_and_caps_launch_speed() {
    let mut drag = InteractionSession {
        press_cursor: Point::default(),
        max_excursion: 0.0,
        dragging: true,
        creature_id: 1,
        guest: false,
        grab_offset: Point::default(),
        original_position: Point::default(),
        original_surface: SurfaceAttachment {
            kind: SurfaceKind::ScreenFloor,
            monitor_id: 1,
            window_key: None,
            relative_x: 0.5,
        },
        original_action: ActionKind::Idle,
        velocity_samples: [Point::default(); 3],
        velocity_sample_count: 0,
        next_velocity_sample: 0,
    };
    for x in [100.0, 200.0, 300.0, 400.0] {
        drag.record_velocity(Point { x, y: 0.0 });
    }
    assert_eq!(drag.velocity_sample_count, 3);
    assert_eq!(drag.release_velocity(), Point { x: 300.0, y: 0.0 });

    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    let mut world = World::new([44; 32], created, &desktop);
    let creature_id = world.save.creatures[0].id;
    let start = world.save.creatures[0].state.position;
    world.handle_command(
        WorldCommand::BeginInteraction {
            creature_id,
            cursor: start,
        },
        &desktop,
    );
    world.handle_command(
        WorldCommand::UpdateInteraction {
            cursor: Point {
                x: start.x + 12.0,
                y: start.y,
            },
            velocity: Point { x: 2_000.0, y: 0.0 },
        },
        &desktop,
    );
    world.handle_command(
        WorldCommand::EndInteraction {
            cursor: start,
            velocity: Point { x: 2_000.0, y: 0.0 },
        },
        &desktop,
    );
    assert_eq!(world.save.creatures[0].state.velocity.x, TOSS_MAX_SPEED);
}

#[test]
fn swept_toss_lands_on_ledges_bounces_once_and_settles() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let mut desktop = desktop();
    desktop.windows.push(DesktopWindow {
        key: 95,
        bounds: DesktopRect {
            x: 300.0,
            y: 400.0,
            width: 500.0,
            height: 300.0,
        },
        z_order: 0,
        visible: true,
        minimized: false,
        application: None,
        application_name: None,
    });
    let swept = find_swept_support(
        Point { x: 500.0, y: 100.0 },
        Point { x: 500.0, y: 900.0 },
        &desktop,
        &HabitatPolicy::default(),
        true,
    )
    .expect("sweep should find the window before the floor");
    assert_eq!(swept.1.kind, SurfaceKind::WindowLedge);
    assert_eq!(swept.1.window_key, Some(95));
    let floor_only = find_swept_support(
        Point { x: 500.0, y: 100.0 },
        Point { x: 500.0, y: 900.0 },
        &desktop,
        &HabitatPolicy::default(),
        false,
    )
    .expect("disabled ledges should still leave a floor");
    assert_eq!(floor_only.1.kind, SurfaceKind::ScreenFloor);

    desktop.windows.clear();
    let mut creature = World::new([39; 32], created, &desktop)
        .save
        .creatures
        .remove(0);
    creature.state.position = Point { x: 500.0, y: 200.0 };
    creature.state.velocity = Point {
        x: 260.0,
        y: -160.0,
    };
    creature.state.action = ActionKind::Tossed;
    let mut toss = TossState {
        elapsed: 0.0,
        bounces: 0,
        last_safe_position: Point { x: 500.0, y: 846.0 },
        last_safe_surface: SurfaceAttachment {
            kind: SurfaceKind::ScreenFloor,
            monitor_id: 1,
            window_key: None,
            relative_x: 0.5,
        },
    };
    let mut landed = None;
    for _ in 0..60 {
        landed = advance_toss(
            &mut creature,
            &mut toss,
            0.05,
            &desktop,
            &HabitatPolicy::default(),
            false,
            true,
        );
        if landed.is_some() {
            break;
        }
    }
    let (surface, bounced) = landed.expect("toss should settle before its timeout");
    assert!(bounced);
    assert_eq!(toss.bounces, 1);
    assert_eq!(surface.kind, SurfaceKind::ScreenFloor);
    assert_eq!(creature.state.action, ActionKind::Landing);
    assert_eq!(creature.state.velocity, Point::default());

    creature.state.position = Point { x: 500.0, y: 845.0 };
    creature.state.velocity = Point::default();
    creature.state.action = ActionKind::Tossed;
    let mut low_energy = TossState {
        elapsed: 0.0,
        bounces: 0,
        last_safe_position: Point { x: 500.0, y: 846.0 },
        last_safe_surface: toss.last_safe_surface.clone(),
    };
    let first_impact = advance_toss(
        &mut creature,
        &mut low_energy,
        0.05,
        &desktop,
        &HabitatPolicy::default(),
        false,
        true,
    )
    .expect("low-energy impact should settle immediately");
    assert!(!first_impact.1);
    assert_eq!(low_energy.bounces, 0);

    creature.state.position = Point {
        x: 2_000.0,
        y: 200.0,
    };
    creature.state.velocity = Point { x: 400.0, y: 0.0 };
    creature.state.action = ActionKind::Tossed;
    let mut timed_out = TossState {
        elapsed: TOSS_MAX_DURATION - 0.01,
        bounces: 0,
        last_safe_position: Point { x: 500.0, y: 846.0 },
        last_safe_surface: toss.last_safe_surface,
    };
    assert!(
        advance_toss(
            &mut creature,
            &mut timed_out,
            0.05,
            &desktop,
            &HabitatPolicy::default(),
            false,
            true,
        )
        .is_some()
    );
    assert!(
        desktop.monitors[0]
            .usable_bounds
            .contains(creature.state.position)
    );
}

#[test]
fn grabbing_a_toss_in_flight_cancels_back_to_its_last_safe_state() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    let mut world = World::new([40; 32], created, &desktop);
    let creature_id = world.save.creatures[0].id;
    let original = world.save.creatures[0].state.position;
    world.handle_command(
        WorldCommand::BeginInteraction {
            creature_id,
            cursor: original,
        },
        &desktop,
    );
    world.handle_command(
        WorldCommand::EndInteraction {
            cursor: Point {
                x: original.x + 80.0,
                y: original.y - 80.0,
            },
            velocity: Point {
                x: 700.0,
                y: -500.0,
            },
        },
        &desktop,
    );
    let airborne = world.save.creatures[0].state.position;
    assert!(world.handle_command(
        WorldCommand::BeginInteraction {
            creature_id,
            cursor: airborne,
        },
        &desktop,
    ));
    assert!(world.tosses.is_empty());
    assert!(world.handle_command(WorldCommand::CancelInteraction, &desktop));
    assert_eq!(world.save.creatures[0].state.position, original);
    assert_eq!(world.save.creatures[0].state.action, ActionKind::Idle);
}
