//! A companion as the colony keeps it: who it is, where it came from, its name and its role,
//! and the colony's limits on how many there are.

use super::{
    AppearanceGenome, CreatureMemory, CreatureState, LearnedTendencies, PersonalityGenome,
    RoutineTable,
};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

pub type CreatureId = u64;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreatureOrigin {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub design: Option<crate::CreatureDesign>,
    pub source_colony_seed: [u8; 32],
    pub source_generation: u8,
}

pub const MAX_COLONY_CREATURES: usize = 6;
/// Every full-size companion has a house of its own and the village has room for six, so this is
/// the colony cap rather than a smaller one inside it. `AdultLimit` therefore cannot be the limit
/// that fires while the two agree: a colony with room left over has room for an adult.
pub const MAX_ADULT_CREATURES: usize = MAX_COLONY_CREATURES;
pub const MAX_MINIS_PER_ADULT: usize = 3;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CreatureRole {
    #[default]
    Adult,
    Mini {
        parent_id: CreatureId,
    },
}

impl CreatureRole {
    pub const fn is_adult(self) -> bool {
        matches!(self, Self::Adult)
    }

    pub const fn parent_id(self) -> Option<CreatureId> {
        match self {
            Self::Adult => None,
            Self::Mini { parent_id } => Some(parent_id),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MiniArrivalState {
    pub enabled: bool,
    pub arrived: [bool; 2],
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum CreatureNameError {
    #[error("a creature name cannot be empty")]
    Empty,
    #[error("a creature name can contain at most 24 characters")]
    TooLong,
    #[error("a creature name cannot contain control characters or line breaks")]
    ControlCharacter,
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ColonyManagementError {
    #[error("the colony already has {} creatures", MAX_COLONY_CREATURES)]
    ColonyFull,
    #[error("the colony already has {} full-size creatures", MAX_ADULT_CREATURES)]
    AdultLimit,
    #[error("creature not found")]
    CreatureNotFound,
    #[error("the last full-size creature cannot be removed")]
    LastAdult,
    #[error("this creature is marked to keep")]
    CreatureKept,
    #[error("generated creature identity conflicts with an existing colony member")]
    DuplicateIdentity,
}

/// The longest a companion's name can be, in characters.
pub const MAX_NAME_CHARACTERS: usize = 24;

pub fn validate_creature_name(value: &str) -> Result<String, CreatureNameError> {
    if value.chars().any(char::is_control) {
        return Err(CreatureNameError::ControlCharacter);
    }
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(CreatureNameError::Empty);
    }
    if trimmed.chars().count() > MAX_NAME_CHARACTERS {
        return Err(CreatureNameError::TooLong);
    }
    Ok(trimmed.to_owned())
}

pub fn default_creature_name(
    colony_seed: [u8; 32],
    generation: u8,
    existing_names: &[String],
) -> String {
    const NAMES: [&str; 32] = [
        "Pip", "Mallow", "Clover", "Mochi", "Pebble", "Noodle", "Sprig", "Biscuit", "Fig", "Tansy",
        "Button", "Puddle", "Maple", "Wren", "Dumpling", "Tofu", "Bean", "Miso", "Poppy",
        "Cricket", "Moss", "Pecan", "Lumi", "Tumble", "Juniper", "Dottie", "Sundae", "Nori",
        "Pocket", "Bramble", "Taffy", "Sage",
    ];
    let offset = usize::from(generation).wrapping_mul(7) % colony_seed.len();
    let start = (usize::from(colony_seed[offset]) + usize::from(generation) * 11) % NAMES.len();
    (0..NAMES.len())
        .map(|step| NAMES[(start + step) % NAMES.len()])
        .find(|candidate| !existing_names.iter().any(|name| name == candidate))
        .unwrap_or(NAMES[start])
        .to_owned()
}

/// Where the person at the desk would like a companion to spend its time. It is theirs, not the
/// creature's: kept apart from the innate personality and from what it has learned, it only nudges
/// the companion's own choices, and the habitat, hidden and paused states, and every safety check
/// still decide what it can do. Local to this colony, so it never travels in a share code.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RoamingLeaning {
    /// Wherever it likes: temperament and what it has learned decide.
    #[default]
    Anywhere,
    /// Keeps close to the village and to the floor.
    Homebody,
    /// Rarely climbs, and comes down from a ledge sooner.
    FloorDweller,
    /// Seeks out ledges, and stays up on them longer.
    Climber,
}

impl RoamingLeaning {
    pub const ALL: [Self; 4] = [
        Self::Anywhere,
        Self::Homebody,
        Self::FloorDweller,
        Self::Climber,
    ];

    pub fn is_anywhere(&self) -> bool {
        *self == Self::Anywhere
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Anywhere => "Wherever they like",
            Self::Homebody => "Homebody",
            Self::FloorDweller => "Floor-dweller",
            Self::Climber => "Climber",
        }
    }

    pub const fn description(self) -> &'static str {
        match self {
            Self::Anywhere => "Their own nature and what they have learned decide.",
            Self::Homebody => "Stays close to the village and the floor.",
            Self::FloorDweller => "Rarely climbs, and comes down from a ledge sooner.",
            Self::Climber => "Seeks out ledges and stays up on them longer.",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Creature {
    pub id: CreatureId,
    pub generation: u8,
    pub origin: CreatureOrigin,
    pub colony_order: u8,
    #[serde(default)]
    pub role: CreatureRole,
    #[serde(default = "default_true")]
    pub kept: bool,
    #[serde(default)]
    pub mini_arrivals: MiniArrivalState,
    pub name: String,
    #[serde(default = "default_born_at_utc", with = "time::serde::rfc3339")]
    pub born_at_utc: OffsetDateTime,
    pub display_scale_percent: u8,
    pub appearance: AppearanceGenome,
    pub personality: PersonalityGenome,
    /// Who it is, for a companion made since 0.62.0. Absent for everyone older, whose
    /// temperament is read from the values they already have.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperament: Option<crate::Temperament>,
    pub behavior_seed: [u8; 32],
    pub memory: CreatureMemory,
    pub tendencies: LearnedTendencies,
    pub routines: RoutineTable,
    pub state: CreatureState,
    /// Where its owner would like it to roam. Absent from the file while it is `Anywhere`.
    #[serde(default, skip_serializing_if = "RoamingLeaning::is_anywhere")]
    pub leaning: RoamingLeaning,
    /// What it is wearing, chosen by its owner. Absent from the file while it wears nothing.
    /// Never carried in a share code: a companion shared with a friend arrives as itself.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accessory: Option<crate::Accessory>,
}

const fn default_true() -> bool {
    true
}

fn default_born_at_utc() -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ArrivalState {
    pub arrived: [bool; 3],
}
