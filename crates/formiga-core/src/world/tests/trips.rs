use super::*;

fn trip(session: &str, arrived: OffsetDateTime) -> Trip {
    Trip {
        session: session.to_owned(),
        arrived_at_utc: arrived,
        left_at_utc: arrived + Duration::minutes(40),
    }
}

const SESSION: &str = "0123456789abcdef0123456789abcdef";

#[test]
fn a_colony_leaves_with_nothing_in_hand() {
    let created = datetime!(2026-10-02 9:00 UTC);
    let desktop = desktop();
    let mut world = two_creature_world([61; 32], created);
    let now = created + Duration::hours(1);
    let held = world.save.creatures[0].id;
    let cursor = world.save.creatures[0].state.position;
    assert!(world.handle_command(
        WorldCommand::BeginInteraction {
            creature_id: held,
            cursor,
        },
        &desktop,
    ));
    world.handle_command(
        WorldCommand::UpdateInteraction {
            cursor: Point {
                x: cursor.x + 80.0,
                y: cursor.y - 200.0,
            },
            velocity: Point::default(),
        },
        &desktop,
    );
    assert!(world.is_interacting());
    world.save.creatures[1].state.indoors = true;
    world.save.creatures[1].state.action = ActionKind::Sleep;
    world.prepare_for_trip(now);
    assert!(!world.is_interacting(), "the drag is let go");
    assert!(world.thought_bubbles().is_empty());
    for creature in &world.save.creatures {
        assert_eq!(creature.state.action, ActionKind::Idle);
        assert!(!creature.state.indoors, "{} is out of doors", creature.name);
        assert!(creature.state.attention.is_none());
        assert_eq!(creature.state.velocity, Point::default());
    }
}

#[test]
fn a_guest_goes_on_its_way_when_the_colony_leaves() {
    let created = datetime!(2026-10-02 9:00 UTC);
    let desktop = desktop();
    let mut world = two_creature_world([62; 32], created);
    let mut now = created + Duration::hours(1);
    world.dismiss_home(now, false);
    world.tick(now, 0.0, &desktop);
    let friend = SharedCreatureSeed {
        source_colony_seed: [77; 32],
        source_generation: 0,
        design: Some(CreatureDesign::generated([77; 32], 0, None)),
    };
    assert_eq!(world.invite_visitor(friend, now, &desktop), Ok(()));
    for _ in 0..800 {
        now += Duration::milliseconds(50);
        world.tick(now, 0.05, &desktop);
    }
    assert!(world.save.visitors.on_stage().is_some(), "the friend came");
    world.prepare_for_trip(now);
    assert!(world.save.visitors.on_stage().is_none());
    assert_eq!(
        world.save.visitors.guest_book.len(),
        1,
        "a visit that happened is written down"
    );
}

#[test]
fn a_trip_home_is_counted_once_and_written_once() {
    let created = datetime!(2026-10-02 9:00 UTC);
    let mut world = two_creature_world([63; 32], created);
    let now = created + Duration::hours(2);
    let journal = world.save.companion.journal.len();
    assert!(world.welcome_home(trip(SESSION, created + Duration::hours(1)), now));
    assert_eq!(world.save.trips.count, 1);
    assert!(world.trip_counted(SESSION));
    let written = world
        .save
        .companion
        .journal
        .iter()
        .filter(|entry| entry.moment == JournalMoment::Trip)
        .count();
    assert_eq!(written, 1);
    assert_eq!(world.save.companion.journal.len(), journal + 1);
    // The same receipt read again, after a restart, changes nothing.
    let before = world.save.clone();
    assert!(!world.welcome_home(trip(SESSION, created + Duration::hours(1)), now));
    assert_eq!(world.save, before);
}

#[test]
fn a_trip_that_cannot_have_happened_changes_nothing() {
    let created = datetime!(2026-10-02 9:00 UTC);
    let mut world = two_creature_world([64; 32], created);
    let before = world.save.clone();
    let mut backwards = trip(SESSION, created);
    std::mem::swap(&mut backwards.arrived_at_utc, &mut backwards.left_at_utc);
    for bad in [
        trip("../../colony.json", created),
        trip("0123456789ABCDEF0123456789ABCDEF", created),
        trip("", created),
        backwards,
    ] {
        assert!(!world.welcome_home(bad, created), "a bad trip was counted");
    }
    assert_eq!(world.save, before);
}

#[test]
fn a_colony_that_has_been_nowhere_writes_no_trips() {
    let created = datetime!(2026-10-02 9:00 UTC);
    let mut world = two_creature_world([65; 32], created);
    let file = serde_json::to_value(&world.save).unwrap();
    assert!(file.get("trips").is_none());
    world.welcome_home(trip(SESSION, created), created + Duration::hours(1));
    let file = serde_json::to_value(&world.save).unwrap();
    assert_eq!(file["trips"]["count"], 1);
    assert_eq!(file["trips"]["last"]["session"], SESSION);
}
