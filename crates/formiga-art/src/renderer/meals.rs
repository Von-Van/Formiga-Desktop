//! How a companion looks through a meal (see [`formiga_core::meals`]): which pose each step
//! strikes for the food it is eating, the face it wears, and what is in its paws.
//!
//! The poses are the four eating gestures, the drink's tip for a bowl, and a few the atlas already
//! bakes for the moments between bites; the food itself is a held picture drawn over the body.

use super::*;
use formiga_core::meals::{FoodSize, Meal, MealStep};

/// How much faster cheeks being stuffed go than an ordinary bite.
const STUFFING_RATE: f32 = 1.6;
/// How much of a toss-and-catch is the toss: the rest is the catch, at the mouth.
const TOSSED_FOR: f32 = 0.7;
/// How high something tiny is tossed above where it is held, in frame pixels.
const TOSS_HEIGHT: f32 = 12.0;

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
        // Looking up after it as it goes up, and a gulp as it comes down into the mouth.
        MealStep::TossCatch if meal.step().2 < TOSSED_FOR => gesture(Gesture::Watch, into),
        MealStep::TossCatch => gesture(Gesture::Gulp, into),
        MealStep::Chew if bowl => MealPose::Held(BodyClip::Action(ActionKind::Drink), 3),
        MealStep::Chew => gesture(Gesture::Chew, into),
        MealStep::StuffCheeks => gesture(Gesture::Chomp, into * STUFFING_RATE),
        MealStep::Hiccup => gesture(Gesture::Gasp, into),
        MealStep::Doze => gesture(Gesture::Loaf, into),
        MealStep::WipeCheek | MealStep::LickPaws => gesture(Gesture::Groom, into),
        MealStep::PatTummy => gesture(Gesture::Worry, into),
        // Bent to put the empty bowl down on the ground.
        MealStep::SetDown => gesture(Gesture::Pluck, into),
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
        MealStep::LickPaws | MealStep::PatTummy | MealStep::SetDown => ExpressionKind::Content,
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
        MealStep::SetDown => Some(GazeDirection::new(forward, 1)),
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

/// What a meal has in its paws just now, if anything: its food, eaten down as far as it has got,
/// or the empty bowl on its way down to the ground.
pub fn meal_held(meal: &Meal) -> Option<HeldThing> {
    if meal.step().0 == MealStep::SetDown {
        return Some(HeldThing::EmptyBowl);
    }
    meal.bite().map(|bite| HeldThing::Food {
        food: meal.food,
        bite,
    })
}

/// How far from where the frame holds things the food is in this step, in frame pixels with the
/// frame facing right: up is negative. Most steps hold it where the frame does; a crumb is wiped
/// off with what is left held low in the other paw, something tiny is tossed up and caught in the
/// mouth, and an empty bowl goes down to the ground.
pub fn meal_offset(meal: &Meal) -> (i32, i32) {
    let (step, _, progress) = meal.step();
    match step {
        MealStep::WipeCheek => (1, 3),
        MealStep::TossCatch if progress < TOSSED_FOR => (
            0,
            -((progress / TOSSED_FOR * std::f32::consts::PI).sin() * TOSS_HEIGHT).round() as i32,
        ),
        MealStep::SetDown => (
            (progress * 3.0).round() as i32,
            (progress * 9.0).round() as i32,
        ),
        _ => (0, 0),
    }
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
        let (dx, dy) = meal_offset(meal);
        return Some(baked.holding(meal_held(meal)?).translated(dx, dy));
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

    fn step_at(meal: &mut Meal, wanted: MealStep, progress: f32) -> bool {
        let steps = meal.steps().to_vec();
        let Some(index) = steps.iter().position(|(step, _)| *step == wanted) else {
            return false;
        };
        let end = steps.get(index + 1).map_or(meal.length, |(_, next)| *next);
        meal.elapsed = steps[index].1 + (end - steps[index].1) * progress;
        true
    }

    /// A crumb is wiped off with what is left of the food still in the other paw: the same bite
    /// as before the wipe, held lower.
    #[test]
    fn a_snack_stays_in_the_paw_through_a_cheek_wipe() {
        let mut wiped = 0;
        for seed in 0..80_u64 {
            let mut creature = eater(Food::Biscuit);
            let mut rng = ChaCha12Rng::seed_from_u64(seed);
            creature.state.meal = Some(Meal::plan(
                Food::Biscuit,
                9.0,
                Appetite::default(),
                &mut rng,
            ));
            let meal = creature.state.meal.as_mut().unwrap();
            if !step_at(meal, MealStep::WipeCheek, 0.0) {
                continue;
            }
            meal.elapsed -= 0.02;
            let before = meal.bite();
            step_at(meal, MealStep::WipeCheek, 0.5);
            assert_eq!(meal.bite(), before, "the wipe keeps the same bite");
            let body = BodyPresentation::for_creature(&creature);
            let baked = CreatureRenderer::render_body_frame(
                &creature.appearance,
                body.clip,
                body.frame,
                false,
            )
            .held;
            let held = held_now(&creature, body.clip, baked).expect("the food is still held");
            assert!(matches!(
                held.thing,
                HeldThing::Food {
                    food: Food::Biscuit,
                    ..
                }
            ));
            assert!(
                held.at.y > baked.unwrap().at.y,
                "held low, out of the way of the wipe"
            );
            wiped += 1;
        }
        assert!(wiped > 0, "no meal had a wipe");
    }

    /// The empty bowl goes down to the ground before the paws are licked or a tummy patted, and
    /// it is empty on its way down.
    #[test]
    fn an_empty_bowl_is_set_down_before_the_paws_are_free() {
        let mut creature = eater(Food::Soup);
        let meal = creature.state.meal.as_mut().unwrap();
        assert!(step_at(meal, MealStep::SetDown, 0.05));
        assert_eq!(meal_held(meal), Some(HeldThing::EmptyBowl));
        let early = meal_offset(meal);
        assert!(step_at(meal, MealStep::SetDown, 0.98));
        let late = meal_offset(meal);
        assert!(
            late.1 > early.1 + 6,
            "the bowl goes down: {early:?} to {late:?}"
        );
        assert!(matches!(
            meal_pose(meal),
            MealPose::Playing(BodyClip::Gesture(Gesture::Pluck), _)
        ));
        let next = meal
            .steps()
            .iter()
            .position(|(step, _)| *step == MealStep::SetDown)
            .unwrap()
            + 1;
        meal.elapsed = meal.steps()[next].1 + 0.01;
        assert_eq!(meal_held(meal), None, "the paws are free once it is down");
    }

    /// Something tiny tossed up rises above the paws, is watched on the way, and is caught in the
    /// mouth with a gulp; an ordinary bite of it is a gulp from start to end, held where it is.
    #[test]
    fn a_berry_tossed_up_reads_apart_from_one_gulped() {
        let mut tossed = None;
        for seed in 0..40_u64 {
            let mut rng = ChaCha12Rng::seed_from_u64(seed);
            let mut meal = Meal::plan(Food::Berries, 9.0, Appetite::default(), &mut rng);
            if step_at(&mut meal, MealStep::TossCatch, 0.0) {
                tossed = Some(meal);
                break;
            }
        }
        let mut meal = tossed.expect("some meal of berries has a toss");
        step_at(&mut meal, MealStep::TossCatch, 0.35);
        assert!(meal_offset(&meal).1 < -8, "tossed up above the paws");
        assert!(matches!(
            meal_pose(&meal),
            MealPose::Playing(BodyClip::Gesture(Gesture::Watch), _)
        ));
        step_at(&mut meal, MealStep::TossCatch, 0.9);
        assert_eq!(meal_offset(&meal), (0, 0));
        assert!(matches!(
            meal_pose(&meal),
            MealPose::Playing(BodyClip::Gesture(Gesture::Gulp), _)
        ));
        step_at(&mut meal, MealStep::Bite, 0.35);
        assert_eq!(meal_offset(&meal), (0, 0));
        // The calm way has no tosses at all.
        meal.without_tosses();
        assert!(
            meal.steps()
                .iter()
                .all(|(step, _)| *step != MealStep::TossCatch)
        );
    }
}
