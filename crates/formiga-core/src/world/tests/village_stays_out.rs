use super::*;

fn stays_out_world(seed: u8, created: OffsetDateTime, desktop: &DesktopSnapshot) -> World {
    let mut world = World::new([seed; 32], created, desktop);
    world.save.home.stays_out = true;
    world.dark_at_home_for_tests = Some(false);
    world
}

/// A village that stays out is on the desktop between visits as well as during them, while the
/// colony keeps the same rhythm of fifteen minutes home and fifteen out.
#[test]
fn a_village_that_stays_out_keeps_the_rhythm_and_never_leaves() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    let mut world = stays_out_world(52, created, &desktop);
    world.tick(created, 0.05, &desktop);
    assert!(world.save.home.is_active());
    assert!(world.save.home.shown());

    world.tick(created + Duration::minutes(15), 0.05, &desktop);
    assert!(!world.save.home.is_active(), "out again after a visit");
    assert!(world.save.home.shown(), "the houses stay where they are");
    assert_eq!(world.save.home.display, Some(DisplayKey([1; 16])));

    world.tick(
        created + Duration::minutes(29) + Duration::seconds(59),
        0.05,
        &desktop,
    );
    assert!(!world.save.home.is_active());
    world.tick(created + Duration::minutes(30), 0.05, &desktop);
    assert!(
        world.save.home.is_active(),
        "home again on the usual rhythm"
    );
}

/// After dark a visit lasts three times as long, and the colony still goes out for a while.
#[test]
fn after_dark_a_visit_home_lasts_longer() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    let mut world = stays_out_world(52, created, &desktop);
    world.dark_at_home_for_tests = Some(true);
    world.tick(created, 0.05, &desktop);
    assert!(world.save.home.is_active());
    world.tick(created + Duration::minutes(30), 0.05, &desktop);
    assert!(world.save.home.is_active(), "still home half an hour in");
    world.tick(created + Duration::minutes(45), 0.05, &desktop);
    assert!(!world.save.home.is_active());
    assert!(world.save.home.shown());
}

/// With nobody at the desk the colony heads home straight away, whatever the rhythm says, and
/// stays home until somebody is back; a village that comes and goes takes no notice.
#[test]
fn everyone_goes_home_while_nobody_is_at_the_desk() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let mut desktop = desktop();
    let mut world = stays_out_world(52, created, &desktop);
    world.tick(created, 0.05, &desktop);
    world.tick(created + Duration::minutes(15), 0.05, &desktop);
    assert!(!world.save.home.is_active());

    desktop.idle_duration = std::time::Duration::from_secs(6 * 60);
    world.tick(created + Duration::minutes(17), 0.05, &desktop);
    assert!(world.save.home.is_active(), "home well inside the cooldown");
    world.tick(created + Duration::minutes(50), 0.05, &desktop);
    assert!(
        world.save.home.is_active(),
        "still home while nobody is back"
    );

    desktop.idle_duration = std::time::Duration::ZERO;
    world.tick(created + Duration::minutes(50), 0.05, &desktop);
    assert!(!world.save.home.is_active(), "out again once somebody is");

    let mut comes_and_goes = World::new([52; 32], created, &desktop);
    comes_and_goes.tick(created, 0.05, &desktop);
    comes_and_goes.tick(created + Duration::minutes(15), 0.05, &desktop);
    desktop.idle_duration = std::time::Duration::from_secs(6 * 60);
    comes_and_goes.tick(created + Duration::minutes(17), 0.05, &desktop);
    assert!(!comes_and_goes.save.home.is_active());
    assert!(!comes_and_goes.save.home.shown());
}

/// Sending everyone out ends a visit early and starts the usual while out, and does nothing when
/// nobody is home.
#[test]
fn sending_everyone_out_ends_a_visit_early() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    let mut world = stays_out_world(52, created, &desktop);
    world.tick(created, 0.05, &desktop);
    let now = created + Duration::minutes(3);
    world.tick(now, 0.05, &desktop);
    assert!(world.save.home.is_active());
    assert!(world.handle_command(WorldCommand::SendOut, &desktop));
    assert!(!world.save.home.is_active());
    assert!(world.save.home.shown());
    assert_eq!(world.save.home.last_disappeared_utc, Some(now));
    assert!(!world.handle_command(WorldCommand::SendOut, &desktop));
    world.tick(now + Duration::minutes(14), 0.05, &desktop);
    assert!(!world.save.home.is_active(), "out for the usual while");
    // And the person at the desk can call them straight back.
    assert!(world.handle_command(WorldCommand::SendHome, &desktop));
    assert!(world.save.home.is_active());
}

/// Between visits a wonder that turns up on the floor never stands in among the houses.
#[test]
fn a_wonder_between_visits_stands_clear_of_the_village() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    let mut world = stays_out_world(52, created, &desktop);
    world.tick(created, 0.05, &desktop);
    let now = created + Duration::minutes(15);
    world.tick(now, 0.05, &desktop);
    assert!(!world.save.home.is_active());
    let (low, high) = world
        .village_floor_span(1, &desktop)
        .expect("the village stands on the only display");
    for creature in &mut world.save.creatures {
        creature.state.position = Point {
            x: (low + high) / 2.0,
            y: 846.0,
        };
        creature.state.surface = SurfaceAttachment {
            kind: SurfaceKind::ScreenFloor,
            monitor_id: 1,
            window_key: None,
            relative_x: 0.5,
        };
        creature.state.action = ActionKind::Idle;
        creature.state.drives.energy = 0.9;
        creature.state.drives.sleep_pressure = 0.1;
    }
    world.wonders.due_now(Some(crate::WonderKind::Chair));
    let mut seen = None;
    for _ in 0..400 {
        world.tick(now, 0.05, &desktop);
        if let Some(view) = world.wonder() {
            seen = Some(view);
            break;
        }
    }
    let view = seen.expect("a chair turned up");
    // Art pixels are a point and a half each on this display.
    let half = crate::WonderKind::Chair.span() * 1.5 / 2.0;
    assert!(
        view.at.x + half <= low || view.at.x - half >= high,
        "{view:?} stands among the houses {low}..{high}"
    );
}
