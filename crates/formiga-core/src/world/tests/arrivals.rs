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

/// A colony out on the desktop with nobody due to turn up by themselves, so the only arrival is
/// the one a test makes.
fn roaming_colony(seed: u8, desktop: &DesktopSnapshot) -> (World, OffsetDateTime) {
    let created = datetime!(2026-01-01 9:00 UTC);
    let mut world = World::new([seed; 32], created, desktop);
    world.tick(created, 0.05, desktop);
    let now = created + time::Duration::minutes(20);
    let_colony_wander(&mut world, now);
    world.save.arrival_state.arrived = [true; 3];
    for creature in &mut world.save.creatures {
        creature.mini_arrivals.arrived = [true; 2];
    }
    world.tick(now, 0.05, desktop);
    world.drain_events().for_each(drop);
    assert!(!world.save.home.is_active());
    (world, now)
}

/// Half a creature's drawn width on the test display, which is how far beyond the edge it has
/// to stand to be out of sight.
fn half_width(world: &World, desktop: &DesktopSnapshot) -> f32 {
    body_half_width(&desktop.monitors[0], world.save.settings.display_scale)
}

/// A companion adopted while the colony is out on the desktop does not appear where it is to
/// stand: it walks in along the floor from beyond the nearer edge of the display, wholly out of
/// sight to begin with, at its own walking pace, and stops a few seconds in, inside the ground it
/// may use, ready to get on with its day.
#[test]
fn a_new_companion_walks_in_from_the_nearer_edge_instead_of_appearing() {
    let desktop = desktop();
    let (mut world, now) = roaming_colony(41, &desktop);
    let id = world
        .add_designed_adult([42; 32], None, now, &desktop)
        .unwrap();
    let half = half_width(&world, &desktop);
    let newcomer = world.save.creatures.iter().find(|c| c.id == id).unwrap();
    // Due to stand at a third of the way along, so it comes in from the left.
    assert!(
        newcomer.state.position.x + half <= desktop.monitors[0].bounds.x + 0.01,
        "it starts in sight, at {}",
        newcomer.state.position.x
    );
    assert!(newcomer.state.facing_right, "it faces the way it walks in");
    assert!(
        world
            .window_journeys
            .get(&id)
            .is_some_and(WindowJourney::entering)
    );
    let pace = 24.0 + newcomer.personality.activity * 34.0;
    let mut x = newcomer.state.position.x;
    let mut walked_for = 0.0;
    for step in 1..=200 {
        world.tick(
            now + time::Duration::milliseconds(step * 50),
            0.05,
            &desktop,
        );
        world.drain_events().for_each(drop);
        let newcomer = world.save.creatures.iter().find(|c| c.id == id).unwrap();
        let moved = newcomer.state.position.x - x;
        x = newcomer.state.position.x;
        if !world.window_journeys.contains_key(&id) {
            break;
        }
        walked_for += 0.05;
        assert_eq!(newcomer.state.action, ActionKind::Traverse, "at {step}");
        assert!(
            moved > 0.0 && moved <= pace * 0.05 + 0.01,
            "it moved {moved} in a tick at {step}"
        );
    }
    assert!(
        !world.window_journeys.contains_key(&id),
        "it never finished walking in"
    );
    assert!(
        walked_for <= ENTRANCE_SECS + 0.1,
        "the walk in took {walked_for}s"
    );
    let newcomer = world.save.creatures.iter().find(|c| c.id == id).unwrap();
    let bounds = desktop.monitors[0].usable_bounds;
    assert!(
        newcomer.state.position.x - half >= bounds.x - 0.01
            && newcomer.state.position.x + half <= bounds.right() + 0.01,
        "it stopped with its body off the display, at {}",
        newcomer.state.position.x
    );
    assert_eq!(newcomer.state.surface.kind, SurfaceKind::ScreenFloor);
}

const ENTRANCE_SECS: f32 = crate::tuning::ENTRANCES.edge_walk_secs;

/// A newcomer due to turn up nearer the right-hand edge walks in from the right.
#[test]
fn a_new_companion_comes_in_by_whichever_edge_is_nearer() {
    let desktop = desktop();
    let (mut world, now) = roaming_colony(43, &desktop);
    let id = world
        .add_designed_adult([44; 32], None, now, &desktop)
        .unwrap();
    // Put it back where it would have appeared, nearer the right, and let it arrive again.
    world.window_journeys.clear();
    let index = world
        .save
        .creatures
        .iter()
        .position(|c| c.id == id)
        .unwrap();
    world.save.creatures[index].state.position.x = 1200.0;
    world.begin_entrance(id, &desktop);
    let half = half_width(&world, &desktop);
    let newcomer = &world.save.creatures[index];
    assert!(newcomer.state.position.x - half >= desktop.monitors[0].bounds.right() - 0.01);
    assert!(!newcomer.state.facing_right);
}

/// Paused, hidden, or with motion reduced, a newcomer is simply there, where it was always going
/// to appear.
#[test]
fn a_new_companion_is_simply_there_when_motion_is_reduced_or_the_colony_is_still() {
    let desktop = desktop();
    for still in ["reduced", "paused", "hidden"] {
        let (mut world, now) = roaming_colony(45, &desktop);
        match still {
            "reduced" => world.save.settings.reduce_motion = true,
            "paused" => world.save.settings.paused = true,
            _ => world.save.settings.visible = false,
        }
        let id = world
            .add_designed_adult([46; 32], None, now, &desktop)
            .unwrap();
        let newcomer = world.save.creatures.iter().find(|c| c.id == id).unwrap();
        assert!(!world.window_journeys.contains_key(&id), "{still}");
        assert!(
            desktop.monitors[0]
                .usable_bounds
                .contains(newcomer.state.position),
            "{still}: it is not where it appears"
        );
    }
}

/// With the village out on the desktop, a newcomer comes out of its own house door — standing
/// as far back as the house does — and walks on out a couple of steps, coming down to the front
/// of the grass as it goes. During a visit home, the village's own walk takes it from its door.
#[test]
fn with_the_village_out_a_new_companion_comes_out_of_its_door() {
    let desktop = desktop();
    for home in [false, true] {
        let (mut world, now) = roaming_colony(47, &desktop);
        world.save.home.stays_out = true;
        if home {
            world.save.home.active_since_utc = Some(now);
        }
        let id = world
            .add_designed_adult([48; 32], None, now, &desktop)
            .unwrap();
        let index = world
            .save
            .creatures
            .iter()
            .position(|c| c.id == id)
            .unwrap();
        let slot = house_slot_for(
            &world.save.creatures[index],
            &world.save.creatures,
            &world.save.home.cottage_order,
        );
        let door = world.village_paths(&desktop).door(slot).expect("its house");
        let newcomer = &world.save.creatures[index];
        assert_eq!(newcomer.state.position.x, door.x, "home: {home}");
        assert_eq!(newcomer.state.depth, door.depth, "home: {home}");
        assert!(!newcomer.state.indoors);
        let walking = world
            .window_journeys
            .get(&id)
            .is_some_and(WindowJourney::entering);
        assert_eq!(walking, !home, "home: {home}");
        if home {
            continue;
        }
        for step in 1..=200 {
            world.tick(
                now + time::Duration::milliseconds(step * 50),
                0.05,
                &desktop,
            );
            world.drain_events().for_each(drop);
            if !world.window_journeys.contains_key(&id) {
                break;
            }
        }
        let newcomer = world.save.creatures.iter().find(|c| c.id == id).unwrap();
        assert!(!world.window_journeys.contains_key(&id));
        assert_ne!(newcomer.state.position.x, door.x, "it never left its door");
        assert_eq!(newcomer.state.depth, 0.0, "it is still up its path");
    }
}

/// Arrivals that queue up behind one another each walk in when their turn comes, not all at
/// once when they are due.
#[test]
fn queued_arrivals_each_walk_in_when_their_turn_comes() {
    let desktop = desktop();
    let created = datetime!(2026-01-01 0:00 UTC);
    let mut world = World::new([12; 32], created, &desktop);
    world.tick(created, 0.05, &desktop);
    let now = created + time::Duration::days(181);
    let_colony_wander(&mut world, now);
    world.tick(now, 0.05, &desktop);
    assert_eq!(world.save.creatures.len(), 4);
    let (first, second) = (world.save.creatures[1].id, world.save.creatures[2].id);
    assert!(
        world
            .window_journeys
            .get(&first)
            .is_some_and(WindowJourney::entering)
    );
    assert!(
        !world.window_journeys.contains_key(&second),
        "waiting its turn"
    );
    world.tick(now, 15.0, &desktop);
    assert_eq!(world.save.creatures[2].state.arrival_delay_secs, 0.0);
    assert!(
        world
            .window_journeys
            .get(&second)
            .is_some_and(WindowJourney::entering)
    );
    let half = half_width(&world, &desktop);
    let x = world.save.creatures[2].state.position.x;
    assert!(
        x + half <= 0.01 || x - half >= desktop.monitors[0].bounds.right() - 0.01,
        "its turn came and it appeared at {x}"
    );
}
