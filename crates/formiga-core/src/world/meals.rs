//! Giving each companion that settles down to eat a [`Meal`]: a food that fits the moment, laid
//! out from the first look at it to the last lick of a paw, and carried along while it eats.
//!
//! Like [`Presence`], this pass runs at the end of each tick and writes nothing anything else in
//! the simulation reads, so it cannot change what a colony does: only how the art draws it
//! eating. Its dice are its own, drawn from the colony seed, the companion and the second the meal
//! began by the desk's clock, so they take nothing from the streams the simulation rolls.

use super::*;
use crate::meals::{Appetite, Food, Meal, MealSetting};

impl World {
    /// Start, carry on or clear everyone's meal. Called once, at the end of every tick.
    pub(super) fn carry_meals(&mut self, now: OffsetDateTime, dt: f32) {
        // Paused, the colony holds still, and so does every meal: the bite it is on now is the
        // bite it takes up again when it carries on.
        let dt = if self.save.settings.paused { 0.0 } else { dt };
        let hour = local_time_or_utc(now).hour();
        let timeline = self.save.maximum_seen_utc;
        let ripe: Vec<GardenKind> = self
            .save
            .home
            .gardens
            .iter()
            .filter(|patch| {
                matches!(
                    patch.stage(timeline),
                    GardenStage::Grown | GardenStage::Bounty
                )
            })
            .map(|patch| patch.kind)
            .collect();
        let streams = SeedStream::new(self.save.colony_seed);
        for creature in &mut self.save.creatures {
            let state = &mut creature.state;
            if state.action != ActionKind::Eat {
                state.meal = None;
                continue;
            }
            if let Some(meal) = &mut state.meal {
                meal.elapsed += dt;
                continue;
            }
            let offered = offers::enjoying(&self.offers, creature.id) == Some(ActionKind::Eat);
            let mut rng = streams.rng(
                "meal",
                creature.id ^ (now.unix_timestamp() as u64).rotate_left(17),
            );
            let appetite = appetite(creature);
            let food = Meal::choose_food(
                appetite,
                MealSetting {
                    hour,
                    ripe: &ripe,
                    offered,
                },
                &mut rng,
            );
            let left = (creature.state.action_duration - creature.state.action_elapsed).max(1.0);
            let mut meal = Meal::plan(food, left.min(30.0), appetite, &mut rng);
            meal.elapsed = dt.min(left);
            creature.state.meal = Some(meal);
        }
    }
}

/// What a companion brings to a meal: whether it is picky, loves its food, or is tired, and its
/// favourite, which is read from its looks so nothing is saved to say so.
pub(crate) fn appetite(creature: &Creature) -> Appetite {
    let traits = creature.traits();
    let has = |any: &[Trait]| traits.iter().any(|t| any.contains(t));
    Appetite {
        picky: has(&[Trait::Picky, Trait::Particular]),
        greedy: has(&[Trait::FoodMotivated]),
        sleepy: creature.state.drives.sleep_pressure > 0.6,
        favourite: Food::favourite_of(&creature.appearance),
    }
}
