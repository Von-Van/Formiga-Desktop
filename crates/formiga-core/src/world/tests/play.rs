use super::*;

const TICK: f32 = 0.05;

/// Nothing a colony does depends on what it plays with.
#[test]
fn play_changes_nothing_the_colony_does() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    let run = |wipe: bool| {
        let mut world = World::new([63; 32], created, &desktop);
        let now = created + Duration::hours(1);
        let_colony_wander(&mut world, now);
        let mut played = 0;
        for tick in 0..4_000 {
            if wipe {
                for creature in &mut world.save.creatures {
                    creature.state.play = None;
                }
            }
            world.tick(now + Duration::milliseconds(50 * tick), TICK, &desktop);
            played += world
                .save
                .creatures
                .iter()
                .filter(|creature| creature.state.play.is_some())
                .count();
        }
        let trace = world
            .save
            .creatures
            .iter()
            .map(|creature| (creature.state.position, creature.state.action))
            .collect::<Vec<_>>();
        (trace, played)
    };
    let (kept, played) = run(false);
    let (wiped, _) = run(true);
    assert_eq!(kept, wiped);
    assert!(
        played > 0,
        "nobody played in over three minutes of wandering"
    );
}

/// A companion that settles down to play picks something up at once, plays with it while it
/// plays, and puts it down when it stops; with an umbrella in the colony, a drop off a ledge is
/// sometimes ridden down under it.
#[test]
fn play_starts_with_playing_and_an_umbrella_is_ridden_down() {
    let created = datetime!(2026-01-01 12:00 UTC);
    let desktop = desktop();
    let mut world = World::new([64; 32], created, &desktop);
    let mut now = created;
    let begin = |world: &mut World, action: ActionKind| {
        let creature = &mut world.save.creatures[0];
        creature.state.action = action;
        creature.state.action_elapsed = 0.0;
        creature.state.action_duration = 6.0;
        creature.state.play = None;
    };
    begin(&mut world, ActionKind::SoloPlay);
    world.carry_play(now, TICK);
    let play = world.save.creatures[0]
        .state
        .play
        .expect("playing starts play");
    world.carry_play(now, TICK);
    assert!(world.save.creatures[0].state.play.unwrap().elapsed > play.elapsed);
    world.save.creatures[0].state.action = ActionKind::Idle;
    world.carry_play(now, TICK);
    assert!(world.save.creatures[0].state.play.is_none());
    // No umbrella, no ride.
    begin(&mut world, ActionKind::Landing);
    world.carry_play(now, TICK);
    assert!(world.save.creatures[0].state.play.is_none());
    let mut umbrella = world
        .save
        .objects
        .objects
        .first()
        .cloned()
        .unwrap_or(ColonyObject {
            id: 99,
            kind: ColonyObjectKind::Umbrella,
            display: desktop.monitors[0].display_key,
            normalized_position: Point { x: 0.5, y: 0.5 },
            role: ColonyObjectRole::default(),
        });
    umbrella.kind = ColonyObjectKind::Umbrella;
    world.save.objects.objects.push(umbrella);
    let mut rides = 0;
    for _ in 0..20 {
        now += Duration::seconds(13);
        begin(&mut world, ActionKind::Landing);
        world.carry_play(now, TICK);
        if let Some(play) = world.save.creatures[0].state.play {
            assert_eq!(play.now().0, formiga_core_play_ride());
            rides += 1;
        }
    }
    assert!((3..20).contains(&rides), "{rides} of 20 drops were ridden");
}

fn formiga_core_play_ride() -> crate::play::PlayMove {
    crate::play::PlayMove::Ride
}

/// Paused, a game and a meal hold where they are along with the action they belong to, so the
/// first frame after carrying on is the frame from just before the pause, not a skip ahead.
#[test]
fn a_pause_holds_a_game_and_a_meal_where_they_are() {
    let created = datetime!(2026-01-01 12:00 UTC);
    let desktop = desktop();
    let now = created + Duration::hours(1);
    for action in [ActionKind::SoloPlay, ActionKind::Eat] {
        let mut world = World::new([65; 32], created, &desktop);
        let state = &mut world.save.creatures[0].state;
        state.action = action;
        state.action_elapsed = 0.0;
        state.action_duration = 20.0;
        let clocks = |world: &World| {
            let state = &world.save.creatures[0].state;
            (
                state.action,
                state.action_elapsed,
                state.play.map(|play| play.elapsed),
                state.meal.as_ref().map(|meal| meal.elapsed),
            )
        };
        for tick in 0..6 {
            world.tick(now + Duration::milliseconds(50 * tick), TICK, &desktop);
        }
        let before = clocks(&world);
        assert_eq!(before.0, action);
        assert!(before.2.is_some() || before.3.is_some(), "{before:?}");
        world.save.settings.paused = true;
        for tick in 6..106 {
            world.tick(now + Duration::milliseconds(50 * tick), TICK, &desktop);
        }
        assert_eq!(clocks(&world), before, "five seconds paused moved a clock");
    }
}
