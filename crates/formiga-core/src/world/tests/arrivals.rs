use super::*;

#[test]
fn colony_arrives_on_calendar_thresholds() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let mut world = World::new([9; 32], created, &desktop());
    world.tick(
        created + time::Duration::hours(1) - time::Duration::seconds(1),
        0.05,
        &desktop(),
    );
    assert_eq!(world.save.creatures.len(), 1);
    world.tick(created + time::Duration::hours(1), 0.05, &desktop());
    assert_eq!(world.save.creatures.len(), 2);
    assert_eq!(
        world.save.creatures[1].born_at_utc,
        created + time::Duration::hours(1)
    );
    world.tick(
        created + time::Duration::days(7) - time::Duration::seconds(1),
        0.05,
        &desktop(),
    );
    assert_eq!(world.save.creatures.len(), 2);
    world.tick(created + time::Duration::days(7), 0.05, &desktop());
    assert_eq!(world.save.creatures.len(), 3);
    world.tick(datetime!(2026-01-31 23:59:59 UTC), 0.05, &desktop());
    assert_eq!(world.save.creatures.len(), 3);
    world.tick(datetime!(2026-02-01 0:00 UTC), 0.05, &desktop());
    assert_eq!(world.save.creatures.len(), 4);
}

#[test]
fn calendar_month_arrival_clamps_to_the_destination_month() {
    assert_eq!(
        add_calendar_months_utc(datetime!(2026-01-31 14:05:06 UTC), 1),
        datetime!(2026-02-28 14:05:06 UTC)
    );
    assert_eq!(
        add_calendar_months_utc(datetime!(2028-01-31 14:05:06 UTC), 1),
        datetime!(2028-02-29 14:05:06 UTC)
    );
    assert_eq!(
        add_calendar_months_utc(datetime!(2026-12-31 14:05:06 UTC), 1),
        datetime!(2027-01-31 14:05:06 UTC)
    );
}

#[test]
fn clock_rollback_does_not_remove_arrivals() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let mut world = World::new([2; 32], created, &desktop());
    world.tick(created + time::Duration::days(180), 0.05, &desktop());
    world.tick(created + time::Duration::days(3), 0.05, &desktop());
    assert_eq!(world.save.creatures.len(), 4);
}

#[test]
fn overdue_arrivals_are_present_but_revealed_fifteen_seconds_apart() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let mut world = World::new([12; 32], created, &desktop());
    world.tick(created + time::Duration::days(181), 0.05, &desktop());
    assert_eq!(world.save.creatures.len(), 4);
    assert_eq!(world.save.creatures[1].state.arrival_delay_secs, 0.0);
    assert!(world.save.creatures[2].state.arrival_delay_secs > 14.0);
    assert!(world.save.creatures[3].state.arrival_delay_secs > 29.0);
    world.tick(created + time::Duration::days(181), 15.0, &desktop());
    assert_eq!(world.save.creatures[2].state.arrival_delay_secs, 0.0);
    assert!(world.save.creatures[3].state.arrival_delay_secs > 14.0);
}

/// A calendar arrival and an adult's own mini can fall due in the same tick. They queue up
/// behind one another like any other pair of arrivals, instead of both revealing at once on
/// top of each other.
#[test]
fn a_calendar_arrival_and_an_adult_mini_due_together_still_queue_up() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    let mut world = World::new([137; 32], created, &desktop);
    world
        .add_designed_adult([138; 32], None, created, &desktop)
        .unwrap();
    assert_eq!(world.save.creatures.len(), 2);
    world.drain_events().for_each(drop);
    world.tick(created + Duration::hours(1), 0.05, &desktop);
    assert_eq!(world.save.creatures.len(), 4, "both arrivals came due");
    assert_eq!(world.save.creatures[2].state.arrival_delay_secs, 0.0);
    assert!(
        world.save.creatures[3].state.arrival_delay_secs > 14.0,
        "the second one waits its turn"
    );
    assert_eq!(
        world
            .drain_events()
            .filter(|event| matches!(event, WorldEvent::CreatureSpawned { .. }))
            .count(),
        1,
        "only the one that is actually here is announced"
    );
}

/// A companion from before recipes has none to pass on, so its mini is one of the originals too:
/// its parent's family and palette, and a version 1 code that replays it exactly.
#[test]
fn an_original_companions_mini_is_drawn_the_original_way() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let mut world = World::new([4; 32], created, &desktop());
    crate::apply_creature_design(&mut world.save.creatures[0], None);
    world.tick(created + time::Duration::hours(1), 0.05, &desktop());
    let parent = &world.save.creatures[0];
    let mini = &world.save.creatures[1];
    assert_eq!(mini.role.parent_id(), Some(parent.id));
    assert_eq!(mini.appearance.design, None);
    assert_eq!(mini.origin.design, None);
    assert_eq!(parent.appearance.family, mini.appearance.family);
    assert_eq!(
        parent.appearance.palette_index,
        mini.appearance.palette_index
    );
    let code = crate::encode_creature_seed(mini.origin);
    let imported = World::from_shared_creature(
        crate::decode_creature_seed(&code).unwrap(),
        created,
        &desktop(),
    );
    assert_eq!(imported.save.creatures[0].appearance, mini.appearance);
}

#[test]
fn mini_is_related_but_not_identical() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let mut world = World::new([4; 32], created, &desktop());
    world.tick(created + time::Duration::days(30), 0.05, &desktop());
    let parent = &world.save.creatures[0];
    let mini = &world.save.creatures[1];
    assert_eq!(parent.appearance.family, mini.appearance.family);
    assert_eq!(
        parent.appearance.face_signature,
        mini.appearance.face_signature
    );
    assert_eq!(
        parent.appearance.palette_index,
        mini.appearance.palette_index
    );
    assert_eq!(
        parent.appearance.face.eye_shape,
        mini.appearance.face.eye_shape
    );
    assert_eq!(
        parent.appearance.face.eye_size,
        mini.appearance.face.eye_size
    );
    assert_eq!(
        parent.appearance.face.eye_spacing,
        mini.appearance.face.eye_spacing
    );
    assert_eq!(
        parent.appearance.face.pupil_style,
        mini.appearance.face.pupil_style
    );
    assert_eq!(
        parent.appearance.face.highlight_style,
        mini.appearance.face.highlight_style
    );
    assert_ne!(parent.appearance.marking_seed, mini.appearance.marking_seed);
    assert_ne!(parent.personality, mini.personality);
}

#[test]
fn one_calendar_month_adds_a_full_size_adult() {
    let created = datetime!(2026-01-31 8:30 UTC);
    let desktop = desktop();
    let mut world = World::new([130; 32], created, &desktop);
    world.tick(created + Duration::hours(1), 0.05, &desktop);
    world.tick(created + Duration::days(7), 0.05, &desktop);
    world.tick(datetime!(2026-02-28 8:30 UTC), 0.05, &desktop);
    assert_eq!(world.save.creatures.len(), 4);
    assert_eq!(adult_count(&world.save.creatures), 2);
    let newest = world.save.creatures.last().unwrap();
    assert!(newest.role.is_adult());
    assert_eq!(newest.display_scale_percent, 100);
}
