//! How a companion looks through a meal (see [`formiga_core::meals`]): which pose each step
//! strikes for the food it is eating, the face it wears, and what is in its paws.
//!
//! The poses are the four eating gestures, the drink's tip for a bowl, and a few the atlas already
//! bakes for the moments between bites; the food itself is a held picture drawn over the body.

use super::*;
use formiga_core::meals::{FoodSize, Meal, MealStep};

/// How much faster cheeks being stuffed go than an ordinary bite.
const STUFFING_RATE: f32 = 1.6;

/// The meal a creature is having, if it is eating and not doing something bigger over the top.
pub(super) fn meal_shown(creature: &Creature) -> Option<&Meal> {
    let state = &creature.state;
    (state.action == ActionKind::Eat && state.attention.is_none() && state.beat.is_none())
        .then_some(state.meal.as_ref())
        .flatten()
}

/// The clip a step of a meal shows, and either how far into it the clip is, or a frame held.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MealPose {
    Playing(BodyClip, f32),
    Held(BodyClip, u8),
}

pub fn meal_pose(meal: &Meal) -> MealPose {
    let (step, into, _) = meal.step();
    let size = meal.food.size();
    let gesture = |gesture: Gesture, into: f32| MealPose::Playing(BodyClip::Gesture(gesture), into);
    let bowl = size == FoodSize::Bowl;
    match step {
        // Held up in front, looked over: the meal's opening frame.
        MealStep::LookOver => MealPose::Held(BodyClip::Action(ActionKind::Eat), 0),
        // Bitten hard to crack it open.
        MealStep::Crack => gesture(Gesture::Chomp, into),
        // A bowl held low in both paws and blown on.
        MealStep::Blow => MealPose::Held(BodyClip::Action(ActionKind::Drink), 0),
        MealStep::Bite if bowl => MealPose::Playing(BodyClip::Action(ActionKind::Drink), into),
        MealStep::Bite => match size {
            FoodSize::Tiny => gesture(Gesture::Gulp, into),
            FoodSize::Big => gesture(Gesture::Chomp, into),
            _ => gesture(Gesture::Nibble, into),
        },
        MealStep::TossCatch => gesture(Gesture::Gulp, into),
        MealStep::Chew if bowl => MealPose::Held(BodyClip::Action(ActionKind::Drink), 3),
        MealStep::Chew => gesture(Gesture::Chew, into),
        MealStep::StuffCheeks => gesture(Gesture::Chomp, into * STUFFING_RATE),
        MealStep::Hiccup => gesture(Gesture::Gasp, into),
        MealStep::Doze => gesture(Gesture::Loaf, into),
        MealStep::WipeCheek | MealStep::LickPaws => gesture(Gesture::Groom, into),
        MealStep::PatTummy => gesture(Gesture::Worry, into),
    }
}

/// The face each step of a meal wears.
pub(super) fn meal_expression(creature: &Creature, meal: &Meal) -> ExpressionKind {
    let traits = creature.traits();
    let loves = traits.contains(&formiga_core::Trait::FoodMotivated);
    let picky = traits.iter().any(|t| {
        matches!(
            t,
            formiga_core::Trait::Picky | formiga_core::Trait::Particular
        )
    });
    meal_face(meal, picky, loves)
}

/// The face a step of a meal wears for one that is `picky`, or that `loves` its food: for a
/// companion app with its own idea of who is which.
pub fn meal_face(meal: &Meal, picky: bool, loves: bool) -> ExpressionKind {
    let (step, _, progress) = meal.step();
    match step {
        MealStep::LookOver if picky => ExpressionKind::Focused,
        MealStep::LookOver | MealStep::Crack => ExpressionKind::Curious,
        MealStep::Blow => ExpressionKind::Focused,
        MealStep::Bite | MealStep::TossCatch | MealStep::StuffCheeks if loves => {
            ExpressionKind::Joy
        }
        MealStep::TossCatch => ExpressionKind::Joy,
        MealStep::Bite | MealStep::StuffCheeks => ExpressionKind::Content,
        MealStep::Chew if picky => ExpressionKind::Neutral,
        MealStep::Chew => ExpressionKind::Content,
        MealStep::Hiccup if progress < 0.4 => ExpressionKind::Startled,
        MealStep::Hiccup => ExpressionKind::Smug,
        MealStep::Doze if progress > 0.85 => ExpressionKind::Startled,
        MealStep::Doze => ExpressionKind::Sleepy,
        MealStep::WipeCheek => ExpressionKind::Focused,
        MealStep::LickPaws | MealStep::PatTummy => ExpressionKind::Content,
    }
}

/// The eyelids a step holds, where it holds any.
pub fn meal_eyelids(meal: &Meal) -> Option<EyelidPose> {
    let (step, _, progress) = meal.step();
    match step {
        MealStep::Doze if progress <= 0.85 => Some(EyelidPose::Closed),
        MealStep::LickPaws | MealStep::PatTummy | MealStep::StuffCheeks => Some(EyelidPose::Closed),
        MealStep::Chew => Some(EyelidPose::Half),
        _ => None,
    }
}

/// Where the eyes go: down at the food while it is looked over, up after something tossed.
pub(super) fn meal_gaze(creature: &Creature, meal: &Meal) -> Option<GazeDirection> {
    let forward = if creature.state.facing_right { 1 } else { -1 };
    match meal.step().0 {
        MealStep::LookOver | MealStep::Blow | MealStep::Crack => {
            Some(GazeDirection::new(forward, 1))
        }
        MealStep::TossCatch => Some(GazeDirection::new(0, -1)),
        _ => None,
    }
}

/// Whether a clip shows what it holds when no meal says otherwise: eating, playing and drinking,
/// and the eating gestures. The hold every other pose bakes is only for meals.
pub fn shows_held_by_default(clip: BodyClip) -> bool {
    matches!(
        clip,
        BodyClip::Action(ActionKind::Eat | ActionKind::SoloPlay | ActionKind::Drink)
            | BodyClip::Gesture(Gesture::Nibble | Gesture::Chomp | Gesture::Chew | Gesture::Gulp)
    )
}

/// What a meal has in its paws just now, if anything: its food, eaten down as far as it has got.
pub fn meal_held(meal: &Meal) -> Option<HeldThing> {
    meal.bite().map(|bite| HeldThing::Food {
        food: meal.food,
        bite,
    })
}

/// What a creature has in its paws in a frame of `clip`, given the hold that frame baked: the
/// food of its meal, eaten down as far as the meal has got, or what the clip holds by default.
pub fn held_now(
    creature: &Creature,
    clip: BodyClip,
    baked: Option<HeldPlacement>,
) -> Option<HeldPlacement> {
    let baked = baked?;
    if let Some(meal) = meal_shown(creature) {
        return Some(baked.holding(meal_held(meal)?));
    }
    if let Some(play) = super::play::play_shown(creature) {
        return Some(super::play::play_placement(creature, play, clip, baked));
    }
    shows_held_by_default(clip).then_some(baked)
}

#[cfg(test)]
mod tests {
    use super::*;
    use formiga_core::meals::{Appetite, Food};
    use formiga_core::{DesktopSnapshot, World};
    use rand::SeedableRng;

    fn eater(food: Food) -> Creature {
        let mut creature = World::preview_adult(
            [31; 32],
            time::OffsetDateTime::UNIX_EPOCH,
            &DesktopSnapshot::default(),
        );
        creature.state.action = ActionKind::Eat;
        creature.state.action_duration = 9.0;
        let mut rng = ChaCha12Rng::seed_from_u64(5);
        creature.state.meal = Some(Meal::plan(food, 9.0, Appetite::default(), &mut rng));
        creature
    }

    /// A food is eaten the way it is eaten: something tiny is gulped, something big chomped,
    /// a bowl slurped, and anything in the hand nibbled.
    #[test]
    fn each_size_of_food_is_bitten_its_own_way() {
        let bite_clip = |food: Food| {
            let mut creature = eater(food);
            let meal = creature.state.meal.as_mut().unwrap();
            let start = meal
                .steps()
                .iter()
                .find(|(step, _)| *step == MealStep::Bite)
                .map(|(_, start)| *start)
                .expect("a meal has a bite");
            meal.elapsed = start + 0.05;
            match meal_pose(meal) {
                MealPose::Playing(clip, _) | MealPose::Held(clip, _) => clip,
            }
        };
        assert_eq!(bite_clip(Food::Apple), BodyClip::Gesture(Gesture::Chomp));
        assert_eq!(
            bite_clip(Food::Strawberry),
            BodyClip::Gesture(Gesture::Nibble)
        );
        assert_eq!(bite_clip(Food::Soup), BodyClip::Action(ActionKind::Drink));
        assert!(matches!(
            bite_clip(Food::Berries),
            BodyClip::Gesture(Gesture::Gulp)
        ));
    }

    /// Through a meal, the paws hold the meal's own food, eaten down; out of one, a pose borrowed
    /// for a meal holds nothing, and eating holds the snack it always has.
    #[test]
    fn the_paws_hold_the_meals_food_and_nothing_borrowed_otherwise() {
        let mut creature = eater(Food::MelonWedge);
        let clip = BodyClip::Gesture(Gesture::Chomp);
        let baked = CreatureRenderer::render_body_frame(&creature.appearance, clip, 1, false).held;
        assert!(baked.is_some(), "a chomp holds something");
        let mut seen = Vec::new();
        let length = creature.state.meal.unwrap().length;
        let mut t = 0.0;
        while t < length {
            creature.state.meal.as_mut().unwrap().elapsed = t;
            if let Some(held) = held_now(&creature, clip, baked) {
                match held.thing {
                    HeldThing::Food { food, bite } => {
                        assert_eq!(food, Food::MelonWedge);
                        seen.push(bite);
                    }
                    other => panic!("a meal holds {other:?}"),
                }
            }
            t += 0.1;
        }
        assert!(seen.windows(2).all(|pair| pair[0] <= pair[1]));
        assert_eq!(seen.first(), Some(&0));
        assert_eq!(seen.last(), Some(&3));
        creature.state.meal = None;
        let gasp = BodyClip::Gesture(Gesture::Gasp);
        let baked = CreatureRenderer::render_body_frame(&creature.appearance, gasp, 0, false).held;
        assert_eq!(held_now(&creature, gasp, baked), None);
        let eat = BodyClip::Action(ActionKind::Eat);
        let baked = CreatureRenderer::render_body_frame(&creature.appearance, eat, 0, false).held;
        assert!(matches!(
            held_now(&creature, eat, baked).map(|held| held.thing),
            Some(HeldThing::Snack { .. })
        ));
    }
}
