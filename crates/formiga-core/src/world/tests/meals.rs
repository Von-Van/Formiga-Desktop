use super::*;

const TICK: f32 = 0.05;

/// Nothing a colony does depends on what it is eating: the same seed ticked the same way does
/// the same things whatever its meals say.
#[test]
fn meals_change_nothing_the_colony_does() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    let run = |wipe: bool| {
        let mut world = World::new([61; 32], created, &desktop);
        let now = created + Duration::hours(1);
        let_colony_wander(&mut world, now);
        let mut meals = 0;
        for tick in 0..4_000 {
            if wipe {
                for creature in &mut world.save.creatures {
                    creature.state.meal = None;
                }
            }
            world.tick(now + Duration::milliseconds(50 * tick), TICK, &desktop);
            meals += world
                .save
                .creatures
                .iter()
                .filter(|creature| creature.state.meal.is_some())
                .count();
        }
        let trace = world
            .save
            .creatures
            .iter()
            .map(|creature| (creature.state.position, creature.state.action))
            .collect::<Vec<_>>();
        (trace, meals)
    };
    let (kept, eaten) = run(false);
    let (wiped, _) = run(true);
    assert_eq!(kept, wiped);
    assert!(eaten > 0, "nobody ate in over three minutes of wandering");
}

/// A companion that settles down to eat is given a meal at once, carries it along while it eats,
/// and has it cleared the moment it does something else; the next meal is laid out afresh.
#[test]
fn a_meal_starts_with_eating_and_ends_with_it() {
    let created = datetime!(2026-01-01 12:00 UTC);
    let desktop = desktop();
    let mut world = World::new([62; 32], created, &desktop);
    let id = world.save.creatures[0].id;
    let mut foods = std::collections::BTreeSet::new();
    let mut now = created;
    for round in 0..12 {
        let creature = &mut world.save.creatures[0];
        creature.state.action = ActionKind::Eat;
        creature.state.action_elapsed = 0.0;
        creature.state.action_duration = 8.0;
        creature.state.meal = None;
        now += Duration::minutes(7 + round);
        world.carry_meals(now, TICK);
        let meal = world.save.creatures[0]
            .state
            .meal
            .expect("eating starts a meal");
        assert!(meal.elapsed <= TICK);
        foods.insert(meal.food.index());
        world.save.creatures[0].state.action_elapsed += TICK;
        world.carry_meals(now, TICK);
        let carried = world.save.creatures[0].state.meal.expect("still eating");
        assert!(carried.elapsed > meal.elapsed, "the meal goes on");
        assert_eq!(carried.food, meal.food, "and keeps its food");
        world.save.creatures[0].state.action = ActionKind::Idle;
        world.carry_meals(now, TICK);
        assert!(
            world.save.creatures[0].state.meal.is_none(),
            "a meal ends with eating"
        );
    }
    assert_eq!(world.save.creatures[0].id, id);
    assert!(foods.len() >= 4, "meals keep to {} foods", foods.len());
}

/// A snack held out while the home is out is taken on the doorstep a moment later, and is still
/// one of the treats offered anywhere else: never an ordinary meal of the hour.
#[test]
fn a_snack_taken_on_the_doorstep_is_still_a_treat() {
    let desktop = desktop();
    let mut world = two_creature_world([9; 32], datetime!(2026-09-18 8:00 UTC));
    let mut now = world.save.maximum_seen_utc;
    let id = world.save.creatures[0].id;
    assert!(world.handle_command(WorldCommand::SendHome, &desktop));
    finish_home_approach(&mut world, now, &desktop);
    let mut treats = 0;
    for round in 0..12 {
        now += Duration::seconds(1);
        assert!(
            world.begin_home_moment(id, ActionKind::Eat, 8.0),
            "round {round}"
        );
        world.tick(now, TICK, &desktop);
        let creature = &world.save.creatures[0];
        assert_eq!(creature.state.action, ActionKind::Eat);
        let food = creature
            .state
            .meal
            .expect("a doorstep snack is a meal")
            .food;
        assert!(
            crate::meals::Food::TREATS.contains(&food),
            "round {round}: a doorstep snack was {food:?}"
        );
        treats += 1;
        world.cancel_home_moments();
        world.tick(now, TICK, &desktop);
    }
    assert_eq!(treats, 12);
}

/// A companion that looks its food over as a habit holds the meal's own look for as long as the
/// habit takes; the cracking and the bites come after it, not hidden under it.
#[test]
fn a_habit_of_looking_food_over_opens_the_meal_rather_than_covering_it() {
    let created = datetime!(2026-01-01 16:00 UTC);
    let mut world = World::new([62; 32], created, &desktop());
    let creature = &mut world.save.creatures[0];
    creature.state.action = ActionKind::Eat;
    creature.state.action_elapsed = 0.0;
    creature.state.action_duration = 8.0;
    creature.state.flourish = Some(crate::habits::Flourish {
        habit: Habit::LooksFoodOver,
        action: ActionKind::Eat,
        started_at: Some(0.0),
    });
    world.carry_meals(created, TICK);
    let meal = world.save.creatures[0].state.meal.expect("a meal");
    let (first, second) = (meal.steps()[0], meal.steps()[1]);
    assert_eq!(first.0, crate::meals::MealStep::LookOver);
    assert!(
        second.1 >= Habit::LooksFoodOver.seconds() - TICK,
        "the meal moves on at {} while the habit holds the look",
        second.1
    );
    assert!(meal.steps().last().unwrap().1 < meal.length);
}

/// A guest on stage that sits down to eat has a meal laid out like anyone else's, and one that
/// plays has a game, with the things it brought rather than the colony's.
#[test]
fn a_guest_eats_and_plays_the_way_residents_do() {
    let desktop = desktop();
    let now = datetime!(2026-09-18 12:00 UTC);
    let mut world = two_creature_world([5; 32], now);
    world.save.objects.objects.push(ColonyObject {
        id: 51,
        kind: ColonyObjectKind::YarnBall,
        display: desktop.monitors[0].display_key,
        normalized_position: Point { x: 0.5, y: 0.5 },
        role: ColonyObjectRole::default(),
    });
    let mut creature = world.save.creatures[0].clone();
    creature.id = 77;
    let mut guest = Visitor::new(creature, VisitorSource::Wanderer, None);
    guest.on_stage = true;
    world.save.visitors.guest = Some(guest);
    let set = |world: &mut World, action: ActionKind| {
        let guest = &mut world.save.visitors.guest.as_mut().unwrap().creature;
        guest.state.action = action;
        guest.state.action_elapsed = 0.0;
        guest.state.action_duration = 8.0;
        guest.state.meal = None;
        guest.state.play = None;
    };
    set(&mut world, ActionKind::Eat);
    world.carry_meals(now, TICK);
    let guest = world.save.visitors.on_stage().unwrap();
    assert!(guest.state.meal.is_some(), "a guest eats a real meal");
    for round in 0..20 {
        set(&mut world, ActionKind::SoloPlay);
        world.carry_play(now + Duration::seconds(round * 11), TICK);
        let play = world
            .save
            .visitors
            .on_stage()
            .unwrap()
            .state
            .play
            .expect("a guest plays a real game");
        assert!(matches!(play.thing, crate::play::Plaything::Toy(_)));
        assert_eq!(play.borrowed, None);
    }
}

/// A guest's calm moment has no length of its own, only what is left of the visit's beat: its
/// meal and its game are laid out to fit that, not half a minute nobody stays for. And its game
/// is shown, with no pose of the moment's own over the top of it.
#[test]
fn a_guest_s_calm_meal_and_game_fit_what_is_left_of_its_moment() {
    let now = datetime!(2026-09-18 12:00 UTC);
    let mut world = two_creature_world([5; 32], now);
    let mut creature = world.save.creatures[0].clone();
    creature.id = 77;
    let mut guest = Visitor::new(creature, VisitorSource::Wanderer, None);
    guest.on_stage = true;
    guest.visit.beat_remaining = 6.5;
    world.save.visitors.guest = Some(guest);
    let set = |world: &mut World, action: ActionKind| {
        let guest = &mut world.save.visitors.guest.as_mut().unwrap().creature;
        guest.state.action = action;
        guest.state.action_elapsed = 0.0;
        guest.state.action_duration = f32::MAX;
        guest.state.attention = None;
        guest.state.meal = None;
        guest.state.play = None;
    };
    set(&mut world, ActionKind::Eat);
    world.carry_meals(now, TICK);
    let guest = world.save.visitors.on_stage().unwrap();
    let meal = guest.state.meal.expect("a guest eats a real meal");
    assert!(
        (meal.length - 6.5).abs() < 0.01,
        "the meal fits the moment: {}",
        meal.length
    );
    set(&mut world, ActionKind::SoloPlay);
    world.carry_play(now, TICK);
    let guest = world.save.visitors.on_stage().unwrap();
    let play = guest.state.play.expect("a guest plays a real game");
    assert!(
        (play.length - 6.5).abs() < 0.01,
        "the game fits the moment: {}",
        play.length
    );
    assert!(guest.state.play_shown().is_some(), "and it is shown");
}

/// A meal and a game move on with their action's own clock: not while a yawn or anything else
/// holds it, and from the start again when the same action is chosen straight after itself.
#[test]
fn meals_and_games_keep_to_their_action_s_clock() {
    let created = datetime!(2026-01-01 12:00 UTC);
    let desktop = desktop();
    let mut world = World::new([62; 32], created, &desktop);
    for action in [ActionKind::Eat, ActionKind::SoloPlay] {
        let shown = |world: &World| {
            let state = &world.save.creatures[0].state;
            match action {
                ActionKind::Eat => state.meal.map(|meal| (meal.elapsed, meal.length)),
                _ => state.play.map(|play| (play.elapsed, play.length)),
            }
        };
        let carry = |world: &mut World| {
            world.carry_meals(created, TICK);
            world.carry_play(created, TICK);
        };
        let creature = &mut world.save.creatures[0];
        creature.state.action = action;
        creature.state.action_elapsed = TICK;
        creature.state.action_duration = 8.0;
        creature.state.attention = None;
        creature.state.meal = None;
        creature.state.play = None;
        carry(&mut world);
        let (began, _) = shown(&world).expect("it starts");
        // Held by a yawn: the action's clock waits, and so does the meal or game.
        for _ in 0..40 {
            carry(&mut world);
        }
        assert_eq!(
            shown(&world).unwrap().0,
            began,
            "{action:?} waits with its clock"
        );
        // Carried right through to its end.
        for _ in 0..200 {
            world.save.creatures[0].state.action_elapsed += TICK;
            carry(&mut world);
        }
        let (late, length) = shown(&world).unwrap();
        assert!(late >= length - TICK, "{action:?} has run its course");
        // Chosen again straight after itself: the clock starts again, and so does a new one.
        world.save.creatures[0].state.action_elapsed = TICK;
        carry(&mut world);
        let (again, _) = shown(&world).expect("a new one starts");
        assert!(again <= TICK, "{action:?} starts afresh, not at {again}");
    }
}
