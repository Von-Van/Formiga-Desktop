//! What a companion has learned from living here, what it remembers, and the words its profile
//! uses for it.

use super::{Creature, DisplayKey};
use serde::{Deserialize, Serialize};

/// What a companion has come to lean toward from living here, each from -100 to 100. A leaning is
/// learned a little at a time, less the nearer it already is to the end an experience pushes it
/// toward, and drifts back toward the companion's own nature while nothing pushes it, so no
/// companion ends at the top of all of them. Whole numbers in older files read as the same values.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct LearnedTendencies {
    pub cursor_trust: f32,
    pub sociability: f32,
    pub climbing: f32,
    pub sleep_security: f32,
    pub exploration: f32,
    pub play: f32,
    pub home_affinity: f32,
    pub routine: f32,
}

impl LearnedTendencies {
    /// A plain nudge, kept on the scale.
    pub fn adjust(value: &mut f32, delta: f32) {
        *value = (*value + delta).clamp(-100.0, 100.0);
    }

    /// What an experience teaches: `delta`, less the nearer the leaning already is to the end it
    /// pushes toward, so however often something happens a leaning only approaches an end.
    pub fn learn(value: &mut f32, delta: f32) {
        let room = if delta >= 0.0 {
            (100.0 - value.max(0.0)) / 100.0
        } else {
            (100.0 + value.min(0.0)) / 100.0
        };
        Self::adjust(value, delta * room);
    }

    pub fn utility(value: f32) -> f32 {
        value.clamp(-100.0, 100.0) / 100.0 * 0.35
    }

    /// Every leaning, in field order.
    pub fn values(&self) -> [f32; 8] {
        [
            self.cursor_trust,
            self.sociability,
            self.climbing,
            self.sleep_security,
            self.exploration,
            self.play,
            self.home_affinity,
            self.routine,
        ]
    }

    fn values_mut(&mut self) -> [&mut f32; 8] {
        [
            &mut self.cursor_trust,
            &mut self.sociability,
            &mut self.climbing,
            &mut self.sleep_security,
            &mut self.exploration,
            &mut self.play,
            &mut self.home_affinity,
            &mut self.routine,
        ]
    }

    /// Where each leaning rests for a companion of this temperament when nothing is teaching it
    /// anything: a bold one sits a little toward high places, a suspicious one a little away from
    /// the cursor. A middling side rests at 0, which is where every leaning of a companion older
    /// than temperaments rests for the sides it never had.
    pub fn baseline(temperament: &crate::Temperament) -> Self {
        let a = temperament.axes.bounded();
        let lean = |value: f32| (value - 0.5) * 2.0;
        Self {
            cursor_trust: 30.0 * -lean(a.suspicion) + 15.0 * lean(a.affection),
            sociability: 35.0 * lean(a.social),
            climbing: 20.0 * lean(a.boldness) + 15.0 * lean(a.energy),
            sleep_security: 20.0 * -lean(a.suspicion) + 15.0 * -lean(a.energy),
            exploration: 35.0 * lean(a.curiosity),
            play: 35.0 * lean(a.playfulness),
            home_affinity: 20.0 * lean(a.affection) + 15.0 * -lean(a.curiosity),
            routine: 35.0 * -lean(a.impulsiveness),
        }
    }

    /// Drift every leaning a share of the way back toward where it rests.
    pub fn fade_toward(&mut self, baseline: Self, share: f32) {
        let share = share.clamp(0.0, 1.0);
        for (value, rest) in self.values_mut().into_iter().zip(baseline.values()) {
            *value += (rest - *value) * share;
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FavoriteDisplayMemory {
    pub display: DisplayKey,
    pub confidence: u8,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreferredRegionMemory {
    pub display: DisplayKey,
    /// Row-major index into a 3×3 display grid.
    pub cell: u8,
    pub confidence: u8,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreatureMemory {
    pub times_petted: u32,
    pub times_tossed: u32,
    pub placements: u32,
    pub sleep_interruptions: u32,
    pub window_climbs: u32,
    pub discoveries_found: u32,
    pub play_sessions: u32,
    pub home_visits: u32,
    pub ledge_seconds: u32,
    pub window_ride_seconds: u32,
    pub longest_sleep_seconds: u32,
    pub favorite_display: Option<FavoriteDisplayMemory>,
    pub preferred_region: Option<PreferredRegionMemory>,
    pub descriptor_flags: u16,
    pub profile_revision: u16,
    pub viewed_profile_revision: u16,
    pub milestone_cooldown_active_seconds: u32,
    pub milestone_bubble_shown: bool,
    /// The little habits it has picked up, in the order it picked them up: at most
    /// [`crate::MAX_HABITS`], one per kind of moment. Absent from the file while there are none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub habits: Vec<crate::Habit>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProfileDescriptor {
    Trusting,
    Wary,
    Social,
    Independent,
    LovesHighPlaces,
    Grounded,
    SoundSleeper,
    RestlessSleeper,
    Adventurous,
    Cautious,
    Playful,
    Calm,
    Homebody,
    Wanderer,
    CreatureOfHabit,
    Spontaneous,
}

impl ProfileDescriptor {
    pub const ALL: [Self; 16] = [
        Self::Trusting,
        Self::Wary,
        Self::Social,
        Self::Independent,
        Self::LovesHighPlaces,
        Self::Grounded,
        Self::SoundSleeper,
        Self::RestlessSleeper,
        Self::Adventurous,
        Self::Cautious,
        Self::Playful,
        Self::Calm,
        Self::Homebody,
        Self::Wanderer,
        Self::CreatureOfHabit,
        Self::Spontaneous,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Trusting => "Trusting",
            Self::Wary => "Wary of the cursor",
            Self::Social => "Social",
            Self::Independent => "Independent",
            Self::LovesHighPlaces => "Loves high places",
            Self::Grounded => "Keeps to the ground",
            Self::SoundSleeper => "Sound sleeper",
            Self::RestlessSleeper => "Restless sleeper",
            Self::Adventurous => "Adventurous",
            Self::Cautious => "Cautious explorer",
            Self::Playful => "Playful",
            Self::Calm => "Calm",
            Self::Homebody => "Loves home",
            Self::Wanderer => "Wanderer",
            Self::CreatureOfHabit => "Creature of habit",
            Self::Spontaneous => "Spontaneous",
        }
    }

    pub const fn flag(self) -> u16 {
        1 << self as u16
    }
}

fn descriptor_value(tendencies: LearnedTendencies, descriptor: ProfileDescriptor) -> f32 {
    match descriptor {
        ProfileDescriptor::Trusting => tendencies.cursor_trust,
        ProfileDescriptor::Wary => -tendencies.cursor_trust,
        ProfileDescriptor::Social => tendencies.sociability,
        ProfileDescriptor::Independent => -tendencies.sociability,
        ProfileDescriptor::LovesHighPlaces => tendencies.climbing,
        ProfileDescriptor::Grounded => -tendencies.climbing,
        ProfileDescriptor::SoundSleeper => tendencies.sleep_security,
        ProfileDescriptor::RestlessSleeper => -tendencies.sleep_security,
        ProfileDescriptor::Adventurous => tendencies.exploration,
        ProfileDescriptor::Cautious => -tendencies.exploration,
        ProfileDescriptor::Playful => tendencies.play,
        ProfileDescriptor::Calm => -tendencies.play,
        ProfileDescriptor::Homebody => tendencies.home_affinity,
        ProfileDescriptor::Wanderer => -tendencies.home_affinity,
        ProfileDescriptor::CreatureOfHabit => tendencies.routine,
        ProfileDescriptor::Spontaneous => -tendencies.routine,
    }
}

pub fn update_descriptor_flags(memory: &mut CreatureMemory, tendencies: LearnedTendencies) -> bool {
    let previous = memory.descriptor_flags;
    for descriptor in ProfileDescriptor::ALL {
        let bit = descriptor.flag();
        let threshold = if previous & bit == 0 { 35.0 } else { 25.0 };
        if descriptor_value(tendencies, descriptor) >= threshold {
            memory.descriptor_flags |= bit;
        } else {
            memory.descriptor_flags &= !bit;
        }
    }
    if memory.descriptor_flags != previous {
        memory.profile_revision = memory.profile_revision.saturating_add(1);
        true
    } else {
        false
    }
}

/// How far a companion has to lean past the rest of its colony before what it has learned says
/// something about it in particular.
pub const DESCRIPTOR_STANDOUT: f32 = 20.0;

/// What a companion has come to lean toward that sets it apart, at most two, the furthest first:
/// the learned words it has earned, each only while it leans at least [`DESCRIPTOR_STANDOUT`]
/// further that way than the middle of the rest of `colony`. Something the whole colony has
/// learned says nothing about any one of them, so it is not shown at all. A companion on its own
/// has nobody to stand apart from, and shows what it has earned.
pub fn profile_descriptors(creature: &Creature, colony: &[Creature]) -> Vec<ProfileDescriptor> {
    let others: Vec<&Creature> = colony
        .iter()
        .filter(|other| other.id != creature.id)
        .collect();
    let standout = |descriptor: ProfileDescriptor| {
        let own = descriptor_value(creature.tendencies, descriptor);
        if others.is_empty() {
            return own;
        }
        let mut values: Vec<f32> = others
            .iter()
            .map(|other| descriptor_value(other.tendencies, descriptor))
            .collect();
        values.sort_by(f32::total_cmp);
        let middle = values.len() / 2;
        let median = if values.len().is_multiple_of(2) {
            (values[middle - 1] + values[middle]) / 2.0
        } else {
            values[middle]
        };
        own - median
    };
    let mut descriptors: Vec<(ProfileDescriptor, f32)> = ProfileDescriptor::ALL
        .into_iter()
        .filter(|descriptor| creature.memory.descriptor_flags & descriptor.flag() != 0)
        .map(|descriptor| (descriptor, standout(descriptor)))
        .filter(|(_, standout)| *standout >= DESCRIPTOR_STANDOUT)
        .collect();
    descriptors.sort_by(|a, b| b.1.total_cmp(&a.1));
    descriptors.truncate(2);
    descriptors
        .into_iter()
        .map(|(descriptor, _)| descriptor)
        .collect()
}
