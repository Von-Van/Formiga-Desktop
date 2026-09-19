use super::*;

#[test]
fn ambient_cadence_is_deterministic_bounded_and_suspended() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    let mut first = World::new([35; 32], created, &desktop);
    let second = World::new([35; 32], created, &desktop);
    let id = first.save.creatures[0].id;
    let first_timers = first.ambient_timers[&id];
    let second_timers = second.ambient_timers[&id];
    assert_eq!(
        first_timers.inspect_remaining,
        second_timers.inspect_remaining
    );
    assert_eq!(
        first_timers.dangle_remaining,
        second_timers.dangle_remaining
    );
    assert_eq!(first.discovery_remaining, second.discovery_remaining);
    assert!((120.0..240.0).contains(&first_timers.inspect_remaining));
    assert!((240.0..480.0).contains(&first_timers.dangle_remaining));
    assert!((600.0..1_200.0).contains(&first.discovery_remaining));

    let_colony_wander(&mut first, created);
    first.save.settings.visible = false;
    first.ambient_timers.get_mut(&id).unwrap().inspect_remaining = 0.0;
    first.ambient_timers.get_mut(&id).unwrap().dangle_remaining = 0.0;
    first.discovery_remaining = 0.0;
    first.save.creatures[0].state.action = ActionKind::Idle;
    first.save.creatures[0].state.action_elapsed = 4.0;
    first.save.creatures[0].state.action_duration = 3.0;
    first.tick(created, 1.0, &desktop);
    assert_eq!(first.ambient_timers[&id].inspect_remaining, 0.0);
    assert_eq!(first.ambient_timers[&id].dangle_remaining, 0.0);
    assert_eq!(first.discovery_remaining, 0.0);
    assert!(!matches!(
        first.save.creatures[0].state.action,
        ActionKind::InspectScreen | ActionKind::Dangle | ActionKind::PresentDiscovery
    ));

    first.save.settings.paused = true;
    first.save.settings.visible = true;
    first.tick(created, 1.0, &desktop);
    assert_eq!(first.discovery_remaining, 0.0);
    assert!(!matches!(
        first.save.creatures[0].state.action,
        ActionKind::InspectScreen | ActionKind::Dangle | ActionKind::PresentDiscovery
    ));
}

#[test]
fn only_one_creature_begins_a_colony_discovery() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    let now = created + time::Duration::days(180);
    let mut world = World::new([43; 32], created, &desktop);
    world.tick(now, 0.05, &desktop);
    for creature in &mut world.save.creatures {
        creature.state.arrival_delay_secs = 0.0;
        creature.state.action = ActionKind::Idle;
        creature.state.action_elapsed = 4.0;
        creature.state.action_duration = 3.0;
    }
    world.save.ritual.next_at_utc = now + Duration::hours(12);
    world.discovery_remaining = 0.0;
    world.tick(now, 0.05, &desktop);
    assert_eq!(
        world
            .save
            .creatures
            .iter()
            .filter(|creature| creature.state.action == ActionKind::PresentDiscovery)
            .count(),
        1
    );
    assert!((600.0..1_200.0).contains(&world.discovery_remaining));
}

#[test]
fn inspection_landmarks_are_geometry_only_screen_thirds() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    let mut world = World::new([36; 32], created, &desktop);
    let creature = &mut world.save.creatures[0];
    creature.state.surface = SurfaceAttachment {
        kind: SurfaceKind::ScreenFloor,
        monitor_id: 1,
        window_key: None,
        relative_x: 1.0 / 3.0,
    };
    creature.state.position = Point {
        x: 490.0,
        y: desktop.monitors[0].usable_bounds.bottom() - 4.0,
    };
    assert!(crossed_inspection_anchor(
        creature,
        470.0,
        &desktop,
        &world.save.settings.habitat,
    ));
    creature.state.position.x = 760.0;
    assert!(!crossed_inspection_anchor(
        creature,
        730.0,
        &desktop,
        &world.save.settings.habitat,
    ));
}
