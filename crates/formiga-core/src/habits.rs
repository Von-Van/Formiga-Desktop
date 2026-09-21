//! Signature little habits: the small, steady ways one companion does ordinary things.
//!
//! Every companion celebrates in its own way from the day it arrives. That is read from what it
//! was born with, so it is never stored, never changes, and is the same for the same creature in
//! any colony. On top of it a companion picks up at most [`MAX_HABITS`] habits from living here —
//! stretching before a nap, looking a snack over before the first bite — each belonging to one
//! kind of everyday moment. Which ones depends on its temperament and on what it has learned, and
//! once it has a habit it keeps it.
//!
//! A habit only changes how the first second or so of an action looks. Where the creature goes,
//! how long the action lasts and what it does to drives, bonds and tendencies are exactly what
//! they would have been without it.

use crate::{ActionKind, Creature, CreatureState, Gesture};
use serde::{Deserialize, Serialize};

/// A companion keeps at most this many habits, each for a different kind of moment.
pub const MAX_HABITS: usize = 2;

/// Once a companion has a habit, how often it does it when the moment comes round: most of the
/// time, so it reads as that creature's way of doing things, but not so reliably that it looks
/// wound up.
pub const HABIT_PERFORMANCE_CHANCE: f64 = 0.8;

/// How long a companion that has stopped short of where it will do its habit waits to arrive
/// before it lets the habit go for this time.
pub const FLOURISH_WAIT_SECS: f32 = 8.0;

/// How a companion celebrates when something goes its way.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Celebration {
    /// Both paws thrown up in a bounce.
    Hop,
    /// A little side-to-side dance.
    Dance,
    /// A spin on the spot, turning with each bounce.
    Twirl,
}

impl Celebration {
    pub const ALL: [Self; 3] = [Self::Hop, Self::Dance, Self::Twirl];

    /// This companion's own celebration. Energetic creatures lean toward a hop, playful ones
    /// toward a dance and bold ones toward a twirl, and the creature's own seed makes the call,
    /// so two companions with the same temperament can still celebrate differently.
    pub fn for_creature(creature: &Creature) -> Self {
        let p = &creature.personality;
        let weights = [
            0.6 + p.activity,
            0.3 + p.playfulness * 1.4,
            0.2 + p.boldness * 0.9 + p.activity * 0.4,
        ];
        let total: f32 = weights.iter().sum();
        let mut roll = unit(seed_mix(creature, 0x6365_6c65_6272_6174)) * total;
        for (celebration, weight) in Self::ALL.into_iter().zip(weights) {
            if roll < weight {
                return celebration;
            }
            roll -= weight;
        }
        Self::Twirl
    }

    /// The body pose the celebration is drawn with. A twirl is the hop, turning.
    pub const fn gesture(self) -> Gesture {
        match self {
            Self::Hop | Self::Twirl => Gesture::Cheer,
            Self::Dance => Gesture::Bop,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Hop => "Celebrates with a hop",
            Self::Dance => "Celebrates with a little dance",
            Self::Twirl => "Celebrates with a twirl",
        }
    }
}

/// The kinds of everyday moment a habit can belong to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HabitCue {
    /// A snack or a drink.
    Meal,
    /// Settling down to sleep.
    Nap,
    /// Greeting a friend, or going over to see the pointer.
    Hello,
    /// A game, alone or with a friend.
    Play,
}

impl HabitCue {
    pub const fn for_action(action: ActionKind) -> Option<Self> {
        match action {
            ActionKind::Eat | ActionKind::Drink => Some(Self::Meal),
            ActionKind::Sleep => Some(Self::Nap),
            ActionKind::Greet | ActionKind::InvestigateCursor => Some(Self::Hello),
            ActionKind::SoloPlay | ActionKind::SocialPlay => Some(Self::Play),
            _ => None,
        }
    }

    /// The chance that one of these moments is the one a companion makes its own, before its
    /// temperament is taken into account. Naps come round several times an hour and games far
    /// less often, so each is scaled by how often it happens, which leaves every kind of moment
    /// about as likely as another to become a habit: in a simulated day each comes up at a rate
    /// that makes a first habit likely within a few hours of company.
    const fn base_chance(self) -> f32 {
        match self {
            Self::Meal => 0.035,
            Self::Nap => 0.012,
            Self::Hello => 0.018,
            Self::Play => 0.03,
        }
    }
}

/// Something small a companion has picked up and now does its own way, nearly every time.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Habit {
    /// Holds a snack or a drink up and looks it over before the first bite or sip.
    LooksFoodOver,
    /// Stretches up tall on its toes before settling down for a nap.
    StretchesBeforeNaps,
    /// Turns round and round on the spot before settling down for a nap.
    CirclesBeforeNaps,
    /// Waves hello: to a friend it is greeting, or to the pointer it has come to see.
    WavesHello,
    /// Drops into a play bow before a game.
    PlayBows,
}

impl Habit {
    pub const ALL: [Self; 5] = [
        Self::LooksFoodOver,
        Self::StretchesBeforeNaps,
        Self::CirclesBeforeNaps,
        Self::WavesHello,
        Self::PlayBows,
    ];

    pub const fn cue(self) -> HabitCue {
        match self {
            Self::LooksFoodOver => HabitCue::Meal,
            Self::StretchesBeforeNaps | Self::CirclesBeforeNaps => HabitCue::Nap,
            Self::WavesHello => HabitCue::Hello,
            Self::PlayBows => HabitCue::Play,
        }
    }

    /// How the colony page and the journal name it.
    pub const fn label(self) -> &'static str {
        match self {
            Self::LooksFoodOver => "Looks a snack over before eating",
            Self::StretchesBeforeNaps => "Stretches before a nap",
            Self::CirclesBeforeNaps => "Turns in circles before a nap",
            Self::WavesHello => "Waves hello",
            Self::PlayBows => "Play-bows before a game",
        }
    }

    /// How long doing it takes, in seconds of the action it belongs to.
    pub const fn seconds(self) -> f32 {
        match self {
            Self::LooksFoodOver => 1.6,
            Self::StretchesBeforeNaps => 1.5,
            Self::CirclesBeforeNaps => 1.6,
            Self::WavesHello => 1.2,
            Self::PlayBows => 1.0,
        }
    }

    /// How well a habit suits a companion, from what it was born with and, on top of that, from
    /// what it has come to like: roughly one half for a poor fit to two for a strong one.
    fn affinity(self, creature: &Creature) -> f32 {
        let p = &creature.personality;
        let t = &creature.tendencies;
        let learned = |value: i8| f32::from(value.max(0)) / 100.0 * 0.5;
        match self {
            // Curious and careful creatures check what they have been given.
            Self::LooksFoodOver => {
                0.4 + p.curiosity * 0.6
                    + (1.0 - p.boldness) * 0.5
                    + learned(t.exploration.saturating_neg())
            }
            Self::StretchesBeforeNaps => 0.4 + p.activity * 0.9 + learned(t.sleep_security),
            Self::CirclesBeforeNaps => 0.4 + p.routine_affinity * 0.9 + learned(t.routine),
            Self::WavesHello => {
                0.4 + p.sociability * 0.5 + p.cursor_interest * 0.4 + learned(t.sociability)
            }
            Self::PlayBows => 0.4 + p.playfulness * 0.9 + learned(t.play),
        }
    }
}

/// The habit a companion could pick up at a moment like this one: none if it already has one for
/// this kind of moment or has no room for another, otherwise whichever habit for the moment suits
/// it best.
pub fn habit_to_learn(creature: &Creature, cue: HabitCue) -> Option<Habit> {
    let habits = &creature.memory.habits;
    if habits.len() >= MAX_HABITS || habits.iter().any(|habit| habit.cue() == cue) {
        return None;
    }
    Habit::ALL
        .into_iter()
        .filter(|habit| habit.cue() == cue)
        .max_by(|a, b| a.affinity(creature).total_cmp(&b.affinity(creature)))
}

/// The chance that this moment is the one where the companion picks the habit up. A second habit
/// comes more slowly than the first, so a new companion shows one little way of its own within
/// its first hours and the other some time after.
pub fn learning_chance(creature: &Creature, habit: Habit) -> f64 {
    let later = if creature.memory.habits.is_empty() {
        1.0
    } else {
        0.3
    };
    f64::from((habit.cue().base_chance() * habit.affinity(creature) * later).clamp(0.0, 1.0))
}

/// The habit a companion already has for moments like this one, if any.
pub fn habit_for(creature: &Creature, cue: HabitCue) -> Option<Habit> {
    creature
        .memory
        .habits
        .iter()
        .copied()
        .find(|habit| habit.cue() == cue)
}

/// A habit being done right now: the opening second or so of an action, drawn the companion's own
/// way. Runtime only, like the attention pose, so a save never records one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Flourish {
    pub habit: Habit,
    /// The action it opens. Anything taking over from that action ends it.
    pub action: ActionKind,
    /// When it began, in the action's own elapsed seconds; `None` while the companion is still on
    /// its way to wherever it is going to do the thing.
    pub started_at: Option<f32>,
}

impl Flourish {
    /// Seconds into the flourish while it is showing, and `None` otherwise.
    pub fn progress(&self, state: &CreatureState) -> Option<f32> {
        let into = state.action_elapsed - self.started_at?;
        (state.action == self.action && (0.0..self.habit.seconds()).contains(&into)).then_some(into)
    }
}

/// A stable value from the creature's own behaviour seed, separate from every other use of it.
fn seed_mix(creature: &Creature, salt: u64) -> u64 {
    let seed = &creature.behavior_seed;
    let mut value = salt;
    for chunk in seed.chunks_exact(8) {
        value ^= u64::from_le_bytes(chunk.try_into().expect("eight bytes"));
        value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        value ^= value >> 31;
    }
    value
}

fn unit(value: u64) -> f32 {
    (value >> 40) as f32 / (1_u64 << 24) as f32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DesktopRect, DesktopSnapshot, DisplayKey, MonitorInfo, World};
    use time::macros::datetime;

    fn creature(seed: u8) -> Creature {
        let desktop = DesktopSnapshot {
            monitors: vec![MonitorInfo {
                id: 1,
                display_key: DisplayKey([1; 16]),
                bounds: DesktopRect {
                    x: 0.0,
                    y: 0.0,
                    width: 1440.0,
                    height: 900.0,
                },
                usable_bounds: DesktopRect {
                    x: 0.0,
                    y: 24.0,
                    width: 1440.0,
                    height: 826.0,
                },
                scale_factor: 2.0,
                primary: true,
            }],
            ..DesktopSnapshot::default()
        };
        let mut seed_bytes = [seed; 32];
        seed_bytes[0] = seed.wrapping_mul(31);
        World::new(seed_bytes, datetime!(2026-01-01 0:00 UTC), &desktop)
            .save
            .creatures
            .remove(0)
    }

    /// Every way of celebrating turns up in a colony's worth of creatures, and each creature's
    /// is its own for good: the same whatever it is called, wherever it lives, and however much
    /// it has learned since.
    #[test]
    fn celebrations_are_spread_about_and_fixed_for_each_creature() {
        let mut counts = [0_usize; 3];
        for seed in 0..=255_u8 {
            let mut creature = creature(seed);
            let celebration = Celebration::for_creature(&creature);
            counts[Celebration::ALL
                .iter()
                .position(|style| *style == celebration)
                .unwrap()] += 1;
            creature.id ^= 0xffff;
            creature.name = "Renamed".into();
            creature.tendencies.play = 100;
            creature.memory.habits = vec![Habit::PlayBows];
            assert_eq!(Celebration::for_creature(&creature), celebration);
        }
        eprintln!("celebrations over 256 creatures (hop, dance, twirl): {counts:?}");
        for (style, count) in Celebration::ALL.into_iter().zip(counts) {
            assert!(
                (40..=130).contains(&count),
                "{style:?} came up {count} times in 256"
            );
        }
    }

    /// A habit belongs to one kind of moment, and the habit a creature picks up for a moment
    /// is the one that suits its temperament.
    #[test]
    fn the_habit_picked_up_suits_the_creature() {
        let mut restless = creature(3);
        restless.personality.activity = 0.95;
        restless.personality.routine_affinity = 0.1;
        assert_eq!(
            habit_to_learn(&restless, HabitCue::Nap),
            Some(Habit::StretchesBeforeNaps)
        );
        let mut settled = creature(3);
        settled.personality.activity = 0.2;
        settled.personality.routine_affinity = 0.9;
        assert_eq!(
            habit_to_learn(&settled, HabitCue::Nap),
            Some(Habit::CirclesBeforeNaps)
        );
        // Every habit is a short opening, never most of the action it opens.
        for habit in Habit::ALL {
            assert!((1.0..=2.0).contains(&habit.seconds()), "{habit:?}");
        }
        for action in crate::ActionKind::ALL {
            let cue = HabitCue::for_action(action);
            assert_eq!(
                cue.is_some(),
                matches!(
                    action,
                    ActionKind::Eat
                        | ActionKind::Drink
                        | ActionKind::Sleep
                        | ActionKind::Greet
                        | ActionKind::InvestigateCursor
                        | ActionKind::SoloPlay
                        | ActionKind::SocialPlay
                ),
                "{action:?}"
            );
        }
    }

    /// Room for two, one per kind of moment, and the second comes more slowly than the first.
    #[test]
    fn a_second_habit_comes_slower_and_a_third_never() {
        let mut creature = creature(9);
        let first = habit_to_learn(&creature, HabitCue::Meal).unwrap();
        let fresh = learning_chance(&creature, first);
        assert!(fresh > 0.0 && fresh < 0.1, "{fresh}");
        creature.memory.habits.push(first);
        assert_eq!(habit_to_learn(&creature, HabitCue::Meal), None);
        let second = habit_to_learn(&creature, HabitCue::Play).unwrap();
        assert!(learning_chance(&creature, second) < fresh);
        creature.memory.habits.push(second);
        for cue in [
            HabitCue::Meal,
            HabitCue::Nap,
            HabitCue::Hello,
            HabitCue::Play,
        ] {
            assert_eq!(habit_to_learn(&creature, cue), None, "{cue:?}");
        }
        assert_eq!(habit_for(&creature, HabitCue::Play), Some(second));
        assert_eq!(habit_for(&creature, HabitCue::Nap), None);
    }

    #[test]
    fn a_flourish_shows_only_during_its_own_opening_seconds() {
        let mut state = creature(12).state;
        state.action = ActionKind::Sleep;
        state.action_elapsed = 3.0;
        let flourish = Flourish {
            habit: Habit::StretchesBeforeNaps,
            action: ActionKind::Sleep,
            started_at: Some(2.5),
        };
        assert_eq!(flourish.progress(&state), Some(0.5));
        state.action_elapsed = 2.5 + Habit::StretchesBeforeNaps.seconds();
        assert_eq!(flourish.progress(&state), None, "finished");
        state.action_elapsed = 2.0;
        assert_eq!(flourish.progress(&state), None, "not begun");
        state.action_elapsed = 3.0;
        state.action = ActionKind::Idle;
        assert_eq!(flourish.progress(&state), None, "another action");
        let waiting = Flourish {
            started_at: None,
            ..flourish
        };
        state.action = ActionKind::Sleep;
        assert_eq!(waiting.progress(&state), None, "still on its way");
    }
}
