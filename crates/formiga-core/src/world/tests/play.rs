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
/// plays, and puts it down when it stops; with an umbrella in the colony, a real drop off a window
/// is sometimes ridden down under it, and nothing else is: not a step down, not a landing with no
/// hop at all.
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
    world.save.creatures[0].state.action_elapsed += TICK;
    world.carry_play(now, TICK);
    assert!(world.save.creatures[0].state.play.unwrap().elapsed > play.elapsed);
    world.save.creatures[0].state.action = ActionKind::Idle;
    world.carry_play(now, TICK);
    assert!(world.save.creatures[0].state.play.is_none());
    let id = world.save.creatures[0].id;
    let start = world.save.creatures[0].state.position;
    let hop = |drop: f32| {
        WindowJourney::Hop(HopJourney {
            start,
            target: Point {
                x: start.x + 30.0,
                y: start.y + drop,
            },
            surface: world_surface(),
            elapsed: 0.0,
            duration: 1.2,
            lift: 0.0,
            ballistic: false,
        })
    };
    // No umbrella, no ride.
    world.window_journeys.insert(id, hop(200.0));
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
    let umbrella_id = umbrella.id;
    world.save.objects.objects.push(umbrella);
    let mut rides = 0;
    for _ in 0..20 {
        now += Duration::seconds(13);
        world.window_journeys.insert(id, hop(200.0));
        begin(&mut world, ActionKind::Landing);
        world.carry_play(now, TICK);
        if let Some(play) = world.save.creatures[0].state.play {
            assert_eq!(play.now().0, formiga_core_play_ride());
            assert_eq!(play.borrowed, Some(umbrella_id));
            assert!(play.length <= 1.2);
            rides += 1;
        }
    }
    assert!((3..20).contains(&rides), "{rides} of 20 drops were ridden");
    // A step down, a landing with no hop, and a hop already under way are never ridden.
    for setup in 0..3 {
        for _ in 0..20 {
            now += Duration::seconds(13);
            world.window_journeys.remove(&id);
            match setup {
                0 => {
                    world.window_journeys.insert(id, hop(20.0));
                }
                1 => {}
                _ => {
                    let mut late = hop(200.0);
                    if let WindowJourney::Hop(journey) = &mut late {
                        journey.elapsed = 0.6;
                    }
                    world.window_journeys.insert(id, late);
                }
            }
            begin(&mut world, ActionKind::Landing);
            world.carry_play(now, TICK);
            assert!(
                world.save.creatures[0].state.play.is_none(),
                "setup {setup} rode an umbrella"
            );
        }
    }
}

/// While a companion plays with one of the colony's belongings, that belonging is borrowed by
/// its id, so the yard can leave it out; nobody else picks up the same one at the same time.
#[test]
fn a_belonging_played_with_is_borrowed_by_one_companion_at_a_time() {
    let created = datetime!(2026-01-01 12:00 UTC);
    let desktop = desktop();
    let mut world = World::new([65; 32], created, &desktop);
    world.save.objects.objects.clear();
    world.save.objects.objects.push(ColonyObject {
        id: 41,
        kind: ColonyObjectKind::YarnBall,
        display: desktop.monitors[0].display_key,
        normalized_position: Point { x: 0.5, y: 0.5 },
        role: ColonyObjectRole::default(),
    });
    let mut now = created;
    let mut borrowed = 0;
    for _ in 0..40 {
        now += Duration::seconds(7);
        for creature in &mut world.save.creatures {
            creature.state.action = ActionKind::SoloPlay;
            creature.state.action_elapsed = 0.0;
            creature.state.action_duration = 6.0;
            creature.state.play = None;
        }
        world.carry_play(now, TICK);
        let holders: Vec<_> = world
            .save
            .creatures
            .iter()
            .filter_map(|creature| creature.state.play)
            .filter(|play| play.borrowed.is_some())
            .collect();
        assert!(holders.len() <= 1, "the yarn ball is in two places");
        for play in holders {
            assert_eq!(play.borrowed, Some(41));
            assert_eq!(
                play.thing,
                crate::play::Plaything::Belonging(ColonyObjectKind::YarnBall)
            );
            borrowed += 1;
        }
    }
    assert!(borrowed > 0, "the yarn ball was never played with");
    // Put back when the game is over.
    for creature in &mut world.save.creatures {
        creature.state.action = ActionKind::Idle;
    }
    world.carry_play(now, TICK);
    assert!(
        world
            .save
            .creatures
            .iter()
            .all(|creature| creature.state.play.is_none())
    );
}

fn world_surface() -> SurfaceAttachment {
    SurfaceAttachment {
        kind: SurfaceKind::ScreenFloor,
        monitor_id: 1,
        window_key: None,
        relative_x: 0.5,
    }
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
