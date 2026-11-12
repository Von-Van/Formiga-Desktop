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
    /// Start, carry on or clear everyone's meal, the guest on stage included. Called once, at the
    /// end of every tick.
    pub(super) fn carry_meals(&mut self, now: OffsetDateTime, dt: f32) {
        // Paused, the colony holds still, and so does every meal: the bite it is on now is the
        // bite it takes up again when it carries on. A meal under way moves on with the eating
        // action's own clock, so it also waits through a pause, a yawn or anything else that
        // holds that clock.
        let dt = if self.save.settings.paused { 0.0 } else { dt };
        // A guest's calm moments run by the visit's beat, not an action length of their own.
        let guest_beat = guest_beat_left(&self.save.visitors);
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
        let reduce_motion = self.save.settings.reduce_motion;
        // A snack held out is a treat whether it is eaten where it was offered or, while the home
        // is out, on the doorstep a moment later.
        let offered: Vec<CreatureId> = self
            .save
            .creatures
            .iter()
            .chain(self.save.visitors.on_stage())
            .map(|creature| creature.id)
            .filter(|id| {
                offers::enjoying(&self.offers, *id) == Some(ActionKind::Eat)
                    || self.offered_home_moment(*id) == Some(ActionKind::Eat)
            })
            .collect();
        let guest = self
            .save
            .visitors
            .guest
            .as_mut()
            .filter(|guest| guest.on_stage)
            .map(|guest| &mut guest.creature);
        for creature in self.save.creatures.iter_mut().chain(guest) {
            let state = &mut creature.state;
            if state.action != ActionKind::Eat {
                state.meal = None;
                continue;
            }
            // Eating chosen again straight after eating starts its action clock again: that is a
            // new meal, not the end of the last one held for the whole of it.
            if state
                .meal
                .is_some_and(|meal| state.action_elapsed + 1e-3 < meal.action_mark)
            {
                state.meal = None;
            }
            if let Some(meal) = &mut state.meal {
                meal.elapsed += (state.action_elapsed - meal.action_mark).max(0.0);
                meal.action_mark = state.action_elapsed;
            } else {
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
                        offered: offered.contains(&creature.id),
                    },
                    &mut rng,
                );
                let left = action_left(&creature.state, creature.id, guest_beat).max(1.0);
                let mut meal = Meal::plan(food, left.min(30.0), appetite, &mut rng);
                meal.elapsed = dt.min(left);
                meal.action_mark = creature.state.action_elapsed;
                creature.state.meal = Some(meal);
            }
            let state = &mut creature.state;
            let Some(meal) = &mut state.meal else {
                continue;
            };
            // Looking its food over as a habit is the meal's own look, held for as long as the
            // habit takes, with the cracking and the bites after it.
            if let Some(flourish) = state.flourish
                && flourish.habit == Habit::LooksFoodOver
                && flourish.action == ActionKind::Eat
                && let Some(began) = flourish.started_at
            {
                let behind = state.action_elapsed - meal.elapsed;
                meal.look_over_until(began + flourish.habit.seconds() - behind);
            }
            if reduce_motion {
                meal.without_tosses();
            }
        }
    }
}

/// The guest on stage, and how long its calm moment has left to run, if it is having one.
pub(super) fn guest_beat_left(visitors: &VisitorState) -> Option<(CreatureId, f32)> {
    visitors
        .guest
        .as_ref()
        .filter(|guest| guest.on_stage)
        .map(|guest| (guest.creature.id, guest.visit.beat_remaining.max(0.0)))
}

/// How long the action under way has left: its own length less how far in it is, or, for a guest
/// whose calm moment has no length of its own, what is left of the moment.
pub(super) fn action_left(
    state: &CreatureState,
    id: CreatureId,
    guest_beat: Option<(CreatureId, f32)>,
) -> f32 {
    match guest_beat {
        Some((guest, left)) if guest == id && state.action_duration == f32::MAX => left,
        _ => state.action_duration - state.action_elapsed,
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
