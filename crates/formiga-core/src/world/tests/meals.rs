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
