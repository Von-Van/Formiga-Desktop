use super::*;

#[test]
fn ritual_schedule_is_deterministic_and_stays_between_twelve_and_forty_eight_hours() {
    let now = datetime!(2026-01-01 0:00 UTC);
    for ordinal in 0..64 {
        let first = scheduled_ritual_at([91; 32], ordinal, now);
        let second = scheduled_ritual_at([91; 32], ordinal, now);
        assert_eq!(first, second);
        assert!(first - now >= Duration::hours(12));
        assert!(first - now <= Duration::hours(48));
    }
}

#[test]
fn every_ritual_uses_a_bounded_runtime_plan_and_existing_actions() {
    use std::collections::HashSet;

    let created = datetime!(2026-01-01 12:00 UTC);
    let mut now = created + Duration::days(2);
    while !(local_time_or_utc(now).hour() >= 22 || local_time_or_utc(now).hour() < 5) {
        now += Duration::hours(1);
    }
    let mut quiet_desktop = desktop();
    quiet_desktop.idle_duration = std::time::Duration::from_secs(20 * 60);
    let mut seen = HashSet::new();
    for ordinal in 0..512 {
        let mut world = two_creature_world([92; 32], created);
        world.save.ritual.ordinal = ordinal;
        assert!(world.try_start_colony_plan(now, &quiet_desktop));
        let plan = world.colony_plan.as_ref().expect("ritual plan starts");
        assert!(plan.participants.len() <= 4);
        if plan.kind == RitualKind::Catch {
            assert_eq!(plan.participants.len(), 2);
        }
        assert!(
            plan.participants
                .iter()
                .all(|participant| ActionKind::ALL.contains(&participant.ceremony_action))
        );
        seen.insert(plan.kind);
        if seen.len() == RitualKind::ALL.len() - 1 {
            break;
        }
    }
    for expected in RitualKind::ALL {
        if expected != RitualKind::HatchDay {
            assert!(seen.contains(&expected), "did not schedule {expected:?}");
        }
    }
}

#[test]
fn reduced_motion_excludes_races_and_interruption_reschedules_without_replay() {
    let created = datetime!(2026-01-01 12:00 UTC);
    let now = created + Duration::days(2);
    let mut desktop = desktop();
    desktop.idle_duration = std::time::Duration::from_secs(20 * 60);
    for ordinal in 0..128 {
        let mut world = two_creature_world([93; 32], created);
        world.save.settings.reduce_motion = true;
        world.save.ritual.ordinal = ordinal;
        assert!(world.try_start_colony_plan(now, &desktop));
        assert_ne!(
            world.colony_plan.as_ref().unwrap().kind,
            RitualKind::FloorRace
        );
    }

    let mut world = two_creature_world([94; 32], created);
    assert!(world.try_start_colony_plan(now, &desktop));
    let kind = world.colony_plan.as_ref().unwrap().kind;
    let ordinal = world.save.ritual.ordinal;
    world.interrupt_colony_plan(now);
    assert!(world.colony_plan.is_none());
    assert_eq!(world.save.ritual.ordinal, ordinal);
    assert!(world.save.ritual.next_at_utc - now >= Duration::hours(2));
    assert!(world.save.ritual.next_at_utc - now <= Duration::hours(6));
    assert!(world.drain_events().any(|event| matches!(
        event,
        WorldEvent::RitualInterrupted { kind: interrupted } if interrupted == kind
    )));
}

#[test]
fn overdue_downtime_runs_at_most_one_ritual_and_schedules_from_now() {
    let created = datetime!(2026-01-01 12:00 UTC);
    let now = created + Duration::days(10);
    let desktop = desktop();
    let mut world = two_creature_world([95; 32], created);
    world.save.home.last_disappeared_utc = Some(now);
    world.save.ritual.next_at_utc = created + Duration::hours(12);
    for creature in &mut world.save.creatures {
        creature.state.action_duration = 0.0;
    }
    world.tick(now, 0.05, &desktop);
    assert_eq!(world.save.ritual.ordinal, 1);
    assert!(world.colony_plan.is_some());
    assert!(world.save.ritual.next_at_utc >= now + Duration::hours(12));
    world.advance_colony_plan(now, RITUAL_APPROACH_SECS + 0.1, &desktop);
    world.advance_colony_plan(now, 60.0, &desktop);
    assert!(world.colony_plan.is_none());
    world.tick(now, 0.05, &desktop);
    assert_eq!(
        world.save.ritual.ordinal, 1,
        "missed rituals must not replay"
    );
}

#[test]
fn hatch_day_is_local_deduplicated_and_reduced_motion_safe() {
    let created = datetime!(2025-06-15 16:00 UTC);
    let now = datetime!(2026-06-15 16:00 UTC);
    let desktop = desktop();
    let mut world = two_creature_world([96; 32], created);
    world.save.settings.reduce_motion = true;
    assert!(world.try_start_colony_plan(now, &desktop));
    assert_eq!(
        world.colony_plan.as_ref().unwrap().kind,
        RitualKind::HatchDay
    );
    assert_eq!(
        world.save.ritual.hatch_day_acknowledged_year,
        Some(local_time_or_utc(now).year())
    );
    world.interrupt_colony_plan(now);
    assert!(
        !world
            .eligible_ritual_kinds(now, &desktop, true)
            .contains(&RitualKind::HatchDay)
    );
}

#[test]
fn rituals_cancel_safely_when_hidden_paused_dragged_or_geometry_changes() {
    let created = datetime!(2026-01-01 12:00 UTC);
    let now = created + Duration::days(2);
    let desktop = desktop();

    for pause_instead_of_hide in [false, true] {
        let mut world = two_creature_world([98; 32], created);
        assert!(world.try_start_colony_plan(now, &desktop));
        if pause_instead_of_hide {
            world.save.settings.paused = true;
        } else {
            world.save.settings.visible = false;
        }
        world.tick(now, 0.05, &desktop);
        assert!(world.colony_plan.is_none());
    }

    let mut changed = two_creature_world([99; 32], created);
    assert!(changed.try_start_colony_plan(now, &desktop));
    let mut changed_desktop = desktop.clone();
    changed_desktop.monitors[0].usable_bounds.width -= 40.0;
    changed.tick(now, 0.05, &changed_desktop);
    assert!(changed.colony_plan.is_none());

    let mut dragged = two_creature_world([100; 32], created);
    assert!(dragged.try_start_colony_plan(now, &desktop));
    let creature_id = dragged.save.creatures[0].id;
    let cursor = dragged.save.creatures[0].state.position;
    assert!(dragged.handle_command(
        WorldCommand::BeginInteraction {
            creature_id,
            cursor,
        },
        &desktop,
    ));
    assert!(dragged.handle_command(
        WorldCommand::UpdateInteraction {
            cursor: Point {
                x: cursor.x + DRAG_THRESHOLD + 1.0,
                y: cursor.y,
            },
            velocity: Point::default(),
        },
        &desktop,
    ));
    assert!(dragged.colony_plan.is_none());
    assert!(dragged.is_dragging());
}
