use super::*;

/// Two sleepers, the later-drawn one settled right on top of the earlier, and a friend of theirs
/// awake and standing about a few steps off, all on the floor.
fn two_sleepers_and_a_friend(seed: [u8; 32]) -> (World, DesktopSnapshot, OffsetDateTime) {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    let mut world = two_creature_world(seed, created);
    world.save.settings.display_scale = 2;
    let now = created + Duration::hours(2);
    world.save.ritual.next_at_utc = now + Duration::days(30);
    world.save.arrival_state.arrived = [true; 3];
    let_colony_wander(&mut world, now);
    let mut friend = world.save.creatures[0].clone();
    friend.id = 900;
    friend.colony_order = 9;
    friend.role = CreatureRole::Adult;
    world.save.creatures.push(friend);
    world.register_creature_runtime(&world.save.creatures[2].clone());
    for creature in &mut world.save.creatures {
        creature.mini_arrivals.arrived = [true; 2];
        creature.state.action = ActionKind::Sleep;
        creature.state.action_elapsed = 0.0;
        creature.state.action_duration = 600.0;
        creature.state.drives.sleep_pressure = 1.0;
        creature.personality.sociability = 0.2;
        creature.personality.playfulness = 0.0;
    }
    let spot = world.save.creatures[0].state.position;
    world.save.creatures[1].state.position = spot;
    let friend = &mut world.save.creatures[2];
    friend.state.position = Point {
        x: spot.x + 120.0,
        y: spot.y,
    };
    friend.state.action = ActionKind::Idle;
    friend.state.action_duration = 600.0;
    friend.state.drives.sleep_pressure = 0.0;
    world.sleep_elapsed.insert(world.save.creatures[0].id, 30.0);
    world.sleep_elapsed.insert(world.save.creatures[1].id, 4.0);
    (world, desktop, now)
}

/// A sleeper that has to make room is towed clear by a friend who is awake: the friend walks
/// over, takes up the rope, pulls the sleeper out at an unhurried pace, lets go, and goes back to
/// standing about. Nobody wakes, nothing records a broken night, and the two sleepers end up
/// shoulder to shoulder rather than one on top of the other.
#[test]
fn a_friend_tows_a_sleeper_clear_on_a_rope_without_waking_it() {
    let (mut world, desktop, now) = two_sleepers_and_a_friend([103; 32]);
    let (sounder, lighter, friend) = (
        world.save.creatures[0].id,
        world.save.creatures[1].id,
        world.save.creatures[2].id,
    );
    let mut towed_for = 0;
    let mut disturbed = Vec::new();
    let mut fastest: f32 = 0.0;
    let mut previous = world.save.creatures[1].state.position.x;
    for step in 1..=400 {
        world.tick(now + Duration::milliseconds(step * 50), 0.05, &desktop);
        disturbed.extend(world.drain_events().filter(|event| {
            matches!(
                event,
                WorldEvent::SleepInterrupted { .. }
                    | WorldEvent::CreatureWoke { .. }
                    | WorldEvent::CreatureRested { .. }
            )
        }));
        let sleeper = world
            .save
            .creatures
            .iter()
            .find(|c| c.id == lighter)
            .unwrap();
        if sleeper.state.nudge == Some(SleepNudge::Towed { by: friend }) {
            towed_for += 1;
            fastest = fastest.max((sleeper.state.position.x - previous).abs() / 0.05);
        }
        previous = sleeper.state.position.x;
    }
    assert!(towed_for > 10, "the sleeper was never towed");
    assert!(
        fastest <= super::super::tows::TOW_SPEED + 0.5,
        "the tow went at {fastest} points a second"
    );
    assert!(disturbed.is_empty(), "the tow woke somebody: {disturbed:?}");
    assert!(
        world.covered_faces(&desktop).is_empty(),
        "one sleeper is still on top of the other"
    );
    for id in [sounder, lighter] {
        let creature = world.save.creatures.iter().find(|c| c.id == id).unwrap();
        assert_eq!(
            creature.state.action,
            ActionKind::Sleep,
            "a sleeper was woken"
        );
        assert_eq!(creature.state.nudge, None, "the rope was never let go of");
    }
    assert!(
        !world.tows.involves(friend),
        "the friend is still holding the rope"
    );
    // The sounder sleeper was left where it lay.
    assert_eq!(world.save.creatures[0].state.position.x, 500.0);
}

/// Nobody awake to tow it, and the sleeper wriggles over by itself, slowly and still asleep. A
/// friend who is taken off mid-tow — picked up by the person at the desk — lets go at once, and
/// the sleeper wriggles the rest of the way.
#[test]
fn with_nobody_free_the_sleeper_wriggles_over_and_a_tow_cut_short_is_finished_by_wriggling() {
    let (mut world, desktop, now) = two_sleepers_and_a_friend([107; 32]);
    let friend = world.save.creatures[2].id;
    world.save.creatures[2].state.action = ActionKind::Sleep;
    world.save.creatures[2].state.position.x += 400.0;
    let lighter = world.save.creatures[1].id;
    let mut wriggled = false;
    for step in 1..=200 {
        world.tick(now + Duration::milliseconds(step * 50), 0.05, &desktop);
        world.drain_events().for_each(drop);
        let sleeper = world
            .save
            .creatures
            .iter()
            .find(|c| c.id == lighter)
            .unwrap();
        wriggled |= sleeper.state.nudge == Some(SleepNudge::Wriggling);
        assert!(
            !world.tows.involves(friend),
            "an asleep friend was asked to tow"
        );
    }
    assert!(wriggled, "the sleeper never wriggled over");
    assert!(world.covered_faces(&desktop).is_empty());

    let (mut world, desktop, now) = two_sleepers_and_a_friend([109; 32]);
    let (lighter, friend) = (world.save.creatures[1].id, world.save.creatures[2].id);
    let mut step = 0;
    while !world.tows.involves(friend) && step < 200 {
        step += 1;
        world.tick(now + Duration::milliseconds(step * 50), 0.05, &desktop);
        world.drain_events().for_each(drop);
    }
    assert!(world.tows.involves(friend), "no tow started");
    let at = world.save.creatures[2].state.position;
    world.handle_command(
        WorldCommand::BeginInteraction {
            creature_id: friend,
            cursor: at,
        },
        &desktop,
    );
    step += 1;
    world.tick(now + Duration::milliseconds(step * 50), 0.05, &desktop);
    assert!(
        !world.tows.involves(friend),
        "the tow outlived its friend being picked up"
    );
    let sleeper = world
        .save
        .creatures
        .iter()
        .find(|c| c.id == lighter)
        .unwrap();
    assert_ne!(
        sleeper.state.nudge,
        Some(SleepNudge::Towed { by: friend }),
        "the rope is still drawn"
    );
}

/// The rope is only ever drawn in a paw. The friend turns to the sleeper and reaches back for it
/// for a moment before the rope appears, and when the sleeper is clear its paws are down, standing
/// still, on the last tick the rope is drawn, so the rope neither appears in an empty paw nor
/// vanishes out of a full one.
#[test]
fn a_friend_reaches_for_the_rope_before_it_appears_and_puts_its_paws_down_before_it_goes() {
    let (mut world, desktop, now) = two_sleepers_and_a_friend([103; 32]);
    let (lighter, friend) = (world.save.creatures[1].id, world.save.creatures[2].id);
    // Per tick: whether the rope is drawn, whether the friend is reaching, whether it is standing
    // with its paws down, whether it faces right, and whether the sleeper is to its left.
    let mut ticks = Vec::new();
    for step in 1..=400 {
        world.tick(now + Duration::milliseconds(step * 50), 0.05, &desktop);
        world.drain_events().for_each(drop);
        let sleeper = world
            .save
            .creatures
            .iter()
            .find(|c| c.id == lighter)
            .unwrap();
        let tower = world
            .save
            .creatures
            .iter()
            .find(|c| c.id == friend)
            .unwrap();
        let reaching = tower
            .state
            .attention
            .is_some_and(|pose| pose.gesture == Some(Gesture::Reach));
        let paws_down = tower.state.attention.is_none()
            && tower.state.action == ActionKind::Idle
            && tower.state.velocity == Point::default();
        ticks.push((
            sleeper.state.nudge == Some(SleepNudge::Towed { by: friend }),
            reaching,
            paws_down,
            tower.state.facing_right,
            sleeper.state.position.x < tower.state.position.x,
        ));
    }
    let first_rope = ticks.iter().position(|tick| tick.0).expect("no tow");
    let last_rope = ticks.iter().rposition(|tick| tick.0).unwrap();
    assert!(
        ticks[first_rope..=last_rope].iter().all(|tick| tick.0),
        "the rope came and went more than once"
    );
    // Reaching for the rope, turned to the sleeper, for the 0.3 s just before it appears, and
    // never once the rope is there.
    let reach_ticks = (super::super::tows::REACH_SECONDS / 0.05).round() as usize;
    let reach = &ticks[first_rope - reach_ticks..first_rope];
    assert!(
        reach.iter().all(|tick| tick.1),
        "the friend was not reaching for the rope just before it appeared: {reach:?}"
    );
    assert!(
        reach.iter().all(|tick| tick.3 != tick.4 && !tick.0),
        "the friend reached for the rope without facing the sleeper: {reach:?}"
    );
    assert!(
        ticks[first_rope..=last_rope].iter().all(|tick| !tick.1),
        "the friend was still reaching once the rope was in its paw"
    );
    // Paws down on the last tick the rope is drawn, and the rope gone on the next.
    assert!(
        ticks[last_rope].2,
        "the rope was let go of before the friend's paws were down: {:?}",
        ticks[last_rope]
    );
    assert!(
        !ticks[last_rope - 1].2,
        "the friend stood still for more than one tick with the rope drawn"
    );
    assert!(!ticks[last_rope + 1].0, "the rope outlived the tow");
}
