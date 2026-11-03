//! A meal: what a companion is eating, how that food is eaten, and the shape of the meal from the
//! first look at it to the last lick of a paw.
//!
//! A companion that settles down to eat is given a food that fits the moment — something from the
//! village's gardens, a breakfast in the morning, soup in the evening, a treat when the person at
//! the desk offers one — and the meal is laid out as a run of steps: a look it over, bites in the
//! way that food is eaten, chewing between them, one moment that makes this meal its own, and a
//! finish. Runtime only, like a beat: nothing here is saved, journaled or counted, and nothing in
//! the simulation reads it. It is there for the art.

use crate::GardenKind;
use rand::Rng;

/// How a food is eaten, which is mostly how big it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FoodSize {
    /// Popped in whole, a few in a row, and sometimes tossed up and caught.
    Tiny,
    /// Held in one paw and nibbled round.
    Hand,
    /// Held in both paws, with big bites and a long chew after each.
    Big,
    /// Munched from one end to the other.
    Long,
    /// Cracked open first, then nibbled.
    Shelled,
    /// Blown on, then slurped from the bowl.
    Bowl,
}

/// Something to eat. The order is the held sheet's, so a food only ever joins the end.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Food {
    Berries,
    Seeds,
    Peas,
    Strawberry,
    #[default]
    Biscuit,
    Cherries,
    MushroomCap,
    Apple,
    PumpkinSlice,
    MelonWedge,
    CakeSlice,
    Carrot,
    PeaPod,
    Breadstick,
    Walnut,
    Acorn,
    Soup,
    Porridge,
}

impl Food {
    pub const ALL: [Self; 18] = [
        Self::Berries,
        Self::Seeds,
        Self::Peas,
        Self::Strawberry,
        Self::Biscuit,
        Self::Cherries,
        Self::MushroomCap,
        Self::Apple,
        Self::PumpkinSlice,
        Self::MelonWedge,
        Self::CakeSlice,
        Self::Carrot,
        Self::PeaPod,
        Self::Breadstick,
        Self::Walnut,
        Self::Acorn,
        Self::Soup,
        Self::Porridge,
    ];

    pub const fn index(self) -> u8 {
        self as u8
    }

    pub const fn size(self) -> FoodSize {
        match self {
            Self::Berries | Self::Seeds | Self::Peas => FoodSize::Tiny,
            Self::Strawberry | Self::Biscuit | Self::Cherries | Self::MushroomCap => FoodSize::Hand,
            Self::Apple | Self::PumpkinSlice | Self::MelonWedge | Self::CakeSlice => FoodSize::Big,
            Self::Carrot | Self::PeaPod | Self::Breadstick => FoodSize::Long,
            Self::Walnut | Self::Acorn => FoodSize::Shelled,
            Self::Soup | Self::Porridge => FoodSize::Bowl,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Berries => "berries",
            Self::Seeds => "seeds",
            Self::Peas => "peas",
            Self::Strawberry => "a strawberry",
            Self::Biscuit => "a biscuit",
            Self::Cherries => "twin cherries",
            Self::MushroomCap => "a mushroom",
            Self::Apple => "an apple",
            Self::PumpkinSlice => "a slice of pumpkin",
            Self::MelonWedge => "a wedge of melon",
            Self::CakeSlice => "a slice of cake",
            Self::Carrot => "a carrot",
            Self::PeaPod => "a pea pod",
            Self::Breadstick => "a breadstick",
            Self::Walnut => "a walnut",
            Self::Acorn => "an acorn",
            Self::Soup => "a bowl of soup",
            Self::Porridge => "a bowl of porridge",
        }
    }

    /// What a garden of this kind gives to eat, once it has grown.
    pub const fn from_garden(kind: GardenKind) -> Option<Self> {
        match kind {
            GardenKind::Vegetables => Some(Self::Carrot),
            GardenKind::Pumpkins => Some(Self::PumpkinSlice),
            // Tomatoes and herbs go into the soup.
            GardenKind::Tomatoes | GardenKind::Herbs => Some(Self::Soup),
            GardenKind::PeaTrellis => Some(Self::PeaPod),
            GardenKind::Strawberries => Some(Self::Strawberry),
            GardenKind::BerryBush => Some(Self::Berries),
            GardenKind::MushroomRing => Some(Self::MushroomCap),
            GardenKind::Flowers
            | GardenKind::Sunflowers
            | GardenKind::Tulips
            | GardenKind::Cactus => None,
        }
    }

    /// What is eaten at this hour of the day, by the desk's clock.
    fn of_the_hour(hour: u8) -> &'static [Self] {
        match hour {
            5..=10 => &[
                Self::Porridge,
                Self::Biscuit,
                Self::Berries,
                Self::Breadstick,
            ],
            11..=14 => &[
                Self::Apple,
                Self::Carrot,
                Self::PeaPod,
                Self::Peas,
                Self::Cherries,
            ],
            15..=17 => &[
                Self::Biscuit,
                Self::MelonWedge,
                Self::Strawberry,
                Self::Walnut,
            ],
            18..=21 => &[
                Self::Soup,
                Self::MushroomCap,
                Self::PumpkinSlice,
                Self::Breadstick,
            ],
            // Something small found in the night.
            _ => &[Self::Seeds, Self::Acorn, Self::Biscuit, Self::Berries],
        }
    }

    /// What is offered as a treat.
    const TREATS: [Self; 5] = [
        Self::CakeSlice,
        Self::Strawberry,
        Self::Cherries,
        Self::MelonWedge,
        Self::Biscuit,
    ];
}

/// What a companion is doing at one point in its meal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MealStep {
    /// Holding it up and looking it over, with a sniff.
    LookOver,
    /// Knocking a nut on the ground to crack it.
    Crack,
    /// Blowing on a bowl that is too hot.
    Blow,
    /// A bite, taken the way this food is eaten.
    Bite,
    /// Tossing something tiny up and catching it in its mouth.
    TossCatch,
    /// Chewing, the food lowered for a moment.
    Chew,
    /// A crumb on its cheek, wiped off with a paw.
    WipeCheek,
    /// Cheeks stuffed full, bite after bite.
    StuffCheeks,
    /// A hiccup.
    Hiccup,
    /// Nodding off over its food, and waking again.
    Doze,
    /// The paws licked clean.
    LickPaws,
    /// A pat of a full tummy.
    PatTummy,
}

impl MealStep {
    /// Whether the food is in its paws through this step.
    pub const fn holds_food(self) -> bool {
        !matches!(self, Self::WipeCheek | Self::LickPaws | Self::PatTummy)
    }
}

/// What the companion itself brings to the table.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Appetite {
    /// Looks a food over a long while, and nibbles.
    pub picky: bool,
    /// Loves its food: big mouthfuls, cheeks stuffed.
    pub greedy: bool,
    /// Tired enough to nod off over it.
    pub sleepy: bool,
    /// Its own favourite food.
    pub favourite: Food,
}

/// Where the meal is had, as far as choosing what is eaten goes.
#[derive(Clone, Copy, Debug, Default)]
pub struct MealSetting<'a> {
    /// The hour of the day by the desk's clock.
    pub hour: u8,
    /// Gardens at the village that have grown far enough to give something.
    pub ripe: &'a [GardenKind],
    /// Offered by the person at the desk.
    pub offered: bool,
}

const MAX_STEPS: usize = 16;

/// A meal under way.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Meal {
    pub food: Food,
    /// Seconds since it began.
    pub elapsed: f32,
    /// Seconds it lasts.
    pub length: f32,
    steps: [(MealStep, f32); MAX_STEPS],
    count: u8,
}

impl Meal {
    /// Chooses what is eaten.
    pub fn choose_food<R: Rng + ?Sized>(
        appetite: Appetite,
        setting: MealSetting<'_>,
        rng: &mut R,
    ) -> Food {
        if setting.offered {
            return Food::TREATS[rng.random_range(0..Food::TREATS.len())];
        }
        // A favourite now and then, more often for one that loves its food.
        let favourite_chance = if appetite.greedy { 0.3 } else { 0.18 };
        if rng.random_bool(favourite_chance) {
            return appetite.favourite;
        }
        let garden: Vec<Food> = setting
            .ripe
            .iter()
            .filter_map(|kind| Food::from_garden(*kind))
            .collect();
        if !garden.is_empty() && rng.random_bool(0.45) {
            return garden[rng.random_range(0..garden.len())];
        }
        let hour = Food::of_the_hour(setting.hour);
        hour[rng.random_range(0..hour.len())]
    }

    /// Lays out a meal of `food` that fills `length` seconds.
    pub fn plan<R: Rng + ?Sized>(food: Food, length: f32, appetite: Appetite, rng: &mut R) -> Self {
        let size = food.size();
        let mut steps: Vec<(MealStep, f32)> = Vec::with_capacity(MAX_STEPS);
        let look = if appetite.picky { 1.6 } else { 0.9 };
        steps.push((MealStep::LookOver, look + rng.random_range(0.0..0.4)));
        match size {
            FoodSize::Shelled => steps.push((MealStep::Crack, 1.2)),
            FoodSize::Bowl if rng.random_bool(0.6) => steps.push((MealStep::Blow, 1.0)),
            _ => {}
        }
        let (bite, chew) = match size {
            FoodSize::Tiny => (0.6, 0.5),
            FoodSize::Hand => (0.7, 0.8),
            FoodSize::Big => (1.0, 1.4),
            FoodSize::Long => (0.5, 0.6),
            FoodSize::Shelled => (0.7, 0.8),
            FoodSize::Bowl => (1.1, 0.5),
        };
        let (fewest, most) = match size {
            FoodSize::Tiny | FoodSize::Long => (3, 5),
            FoodSize::Big => (2, 3),
            _ => (3, 4),
        };
        let mut bites: u32 = rng.random_range(fewest..=most);
        if appetite.picky {
            bites += 1;
        }
        let finish = if rng.random_bool(0.5) {
            vec![(MealStep::LickPaws, 1.0), (MealStep::PatTummy, 0.8)]
        } else if rng.random_bool(0.5) {
            vec![(MealStep::LickPaws, 1.1)]
        } else {
            vec![(MealStep::PatTummy, 1.0)]
        };
        let moment = Self::moment(size, appetite, rng);
        // Fewer bites until the meal at its natural pace fits; then the pace takes up the rest.
        let natural = |bites: u32| {
            let fixed: f32 = steps.iter().map(|(_, secs)| secs).sum::<f32>()
                + finish.iter().map(|(_, secs)| secs).sum::<f32>()
                + moment.map_or(0.0, |(_, secs)| secs);
            fixed + bites as f32 * (bite + chew)
        };
        while bites > 1 && natural(bites) > length * 1.3 {
            bites -= 1;
        }
        let middle = bites.div_ceil(2);
        for taken in 0..bites {
            let step = if size == FoodSize::Tiny && rng.random_bool(0.35) {
                MealStep::TossCatch
            } else {
                MealStep::Bite
            };
            steps.push((step, bite * rng.random_range(0.85..1.15)));
            steps.push((MealStep::Chew, chew * rng.random_range(0.8..1.3)));
            if taken + 1 == middle
                && let Some(moment) = moment
            {
                steps.push(moment);
            }
        }
        steps.extend(finish);
        let total: f32 = steps.iter().map(|(_, secs)| secs).sum();
        let scale = length.max(0.5) / total.max(0.01);
        let mut laid = [(MealStep::LookOver, 0.0); MAX_STEPS];
        let mut start = 0.0;
        let count = steps.len().min(MAX_STEPS);
        // A meal too long for the table keeps its finish rather than its last bites.
        let kept: Vec<(MealStep, f32)> = if steps.len() > MAX_STEPS {
            let tail = steps.len() - 2;
            steps[..MAX_STEPS - 2]
                .iter()
                .chain(&steps[tail..])
                .copied()
                .collect()
        } else {
            steps
        };
        for (slot, (step, secs)) in laid.iter_mut().zip(kept) {
            *slot = (step, start);
            start += secs * scale;
        }
        Self {
            food,
            elapsed: 0.0,
            length: length.max(0.5),
            steps: laid,
            count: count as u8,
        }
    }

    /// The one moment that makes this meal its own, if it has one.
    fn moment<R: Rng + ?Sized>(
        size: FoodSize,
        appetite: Appetite,
        rng: &mut R,
    ) -> Option<(MealStep, f32)> {
        let mut choices: Vec<(MealStep, f32, u32)> = vec![(MealStep::Hiccup, 0.9, 1)];
        if size != FoodSize::Bowl {
            choices.push((MealStep::WipeCheek, 1.1, 3));
        }
        if matches!(size, FoodSize::Big | FoodSize::Hand | FoodSize::Long) {
            choices.push((
                MealStep::StuffCheeks,
                1.4,
                if appetite.greedy { 6 } else { 2 },
            ));
        }
        if appetite.sleepy {
            choices.push((MealStep::Doze, 2.0, 5));
        }
        // Some meals are just a meal.
        let quiet = 3;
        let total: u32 = choices.iter().map(|(_, _, weight)| weight).sum::<u32>() + quiet;
        let mut roll = rng.random_range(0..total);
        for (step, secs, weight) in choices {
            if roll < weight {
                return Some((step, secs));
            }
            roll -= weight;
        }
        None
    }

    /// The step under way, how many seconds into it, and how far through it from 0 to 1.
    pub fn step(&self) -> (MealStep, f32, f32) {
        let steps = &self.steps[..usize::from(self.count.max(1))];
        let index = steps
            .iter()
            .rposition(|(_, start)| *start <= self.elapsed)
            .unwrap_or(0);
        let (step, start) = steps[index];
        let end = steps.get(index + 1).map_or(self.length, |(_, next)| *next);
        let into = (self.elapsed - start).max(0.0);
        (step, into, (into / (end - start).max(0.01)).clamp(0.0, 1.0))
    }

    /// Every step, in order, with the second it starts.
    pub fn steps(&self) -> &[(MealStep, f32)] {
        &self.steps[..usize::from(self.count)]
    }

    /// How much has been eaten, from 0 (none) to 3 (down to the last of it), or `None` once
    /// it is all gone or put down.
    pub fn bite(&self) -> Option<u8> {
        let (step, _, progress) = self.step();
        if !step.holds_food() {
            return None;
        }
        let steps = self.steps();
        let eats = |step: &MealStep| {
            matches!(
                step,
                MealStep::Bite | MealStep::TossCatch | MealStep::StuffCheeks
            )
        };
        let total = steps.iter().filter(|(step, _)| eats(step)).count().max(1);
        let mut taken = steps
            .iter()
            .filter(|(step, start)| eats(step) && *start < self.elapsed)
            .count();
        // A bite is not out of it until the mouth has closed on it.
        if eats(&step) && progress < 0.5 {
            taken = taken.saturating_sub(1);
        }
        Some(((taken * 3).div_ceil(total)).min(3) as u8)
    }

    pub fn finished(&self) -> bool {
        self.elapsed >= self.length
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand_chacha::ChaCha12Rng;

    #[test]
    fn a_meal_fills_its_time_and_runs_from_a_look_to_a_finish() {
        for seed in 0..200_u64 {
            let mut rng = ChaCha12Rng::seed_from_u64(seed);
            let food = Food::ALL[seed as usize % Food::ALL.len()];
            let length = rng.random_range(5.0..9.0);
            let appetite = Appetite {
                picky: seed % 3 == 0,
                greedy: seed % 4 == 0,
                sleepy: seed % 5 == 0,
                favourite: Food::Apple,
            };
            let meal = Meal::plan(food, length, appetite, &mut rng);
            let steps = meal.steps();
            assert_eq!(steps[0], (MealStep::LookOver, 0.0));
            assert!(steps.windows(2).all(|pair| pair[0].1 < pair[1].1));
            assert!(steps.last().expect("a meal has steps").1 < length);
            assert!(matches!(
                steps.last().unwrap().0,
                MealStep::LickPaws | MealStep::PatTummy
            ));
            assert!(
                steps
                    .iter()
                    .any(|(step, _)| matches!(step, MealStep::Bite | MealStep::TossCatch))
            );
            if food.size() == FoodSize::Shelled {
                assert_eq!(steps[1].0, MealStep::Crack);
            }
        }
    }

    #[test]
    fn the_food_is_eaten_down_and_put_away_at_the_end() {
        let mut rng = ChaCha12Rng::seed_from_u64(7);
        let mut meal = Meal::plan(Food::Apple, 8.0, Appetite::default(), &mut rng);
        assert_eq!(meal.bite(), Some(0));
        let mut seen = Vec::new();
        while !meal.finished() {
            if let Some(bite) = meal.bite() {
                assert!(seen.last().is_none_or(|last| *last <= bite));
                seen.push(bite);
            }
            meal.elapsed += 0.05;
        }
        assert_eq!(seen.last(), Some(&3));
        meal.elapsed = meal.length - 0.01;
        assert_eq!(meal.bite(), None);
    }

    #[test]
    fn meals_change_with_the_hour_the_garden_and_an_offer() {
        let appetite = Appetite::default();
        let mut rng = ChaCha12Rng::seed_from_u64(3);
        let mut kinds = std::collections::BTreeSet::new();
        for hour in [7_u8, 12, 16, 20, 2] {
            for _ in 0..40 {
                let setting = MealSetting {
                    hour,
                    ripe: &[],
                    offered: false,
                };
                kinds.insert(Meal::choose_food(appetite, setting, &mut rng).index());
            }
        }
        assert!(kinds.len() >= 12, "only {} foods turn up", kinds.len());
        let setting = MealSetting {
            hour: 12,
            ripe: &[GardenKind::Strawberries],
            offered: false,
        };
        assert!(
            (0..60).any(|_| Meal::choose_food(appetite, setting, &mut rng) == Food::Strawberry)
        );
        let offered = MealSetting {
            offered: true,
            ..setting
        };
        for _ in 0..20 {
            assert!(Food::TREATS.contains(&Meal::choose_food(appetite, offered, &mut rng)));
        }
    }
}
