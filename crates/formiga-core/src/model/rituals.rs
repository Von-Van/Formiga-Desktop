//! The colony's shared rituals and the village moments the person at the desk can invite.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RitualKind {
    Picnic,
    GroupNap,
    FloorRace,
    ShelterGathering,
    Catch,
    GroupPresentation,
    HatchDay,
    QuietDayHuddle,
    LateNightSleepPile,
    /// A dance on the ground between the houses. Only ever invited, never scheduled.
    Dance,
}

impl RitualKind {
    pub const ALL: [Self; 10] = [
        Self::Picnic,
        Self::GroupNap,
        Self::FloorRace,
        Self::ShelterGathering,
        Self::Catch,
        Self::GroupPresentation,
        Self::HatchDay,
        Self::QuietDayHuddle,
        Self::LateNightSleepPile,
        Self::Dance,
    ];
}

/// Something the person at the desk can invite the whole village to share while the houses are
/// out, on the ground between them. Runtime only: the journal records one that was shared as the
/// shared moment it is, and nothing else about it is kept.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum VillageMoment {
    Picnic,
    Dance,
    Nap,
}

impl VillageMoment {
    pub const ALL: [Self; 3] = [Self::Picnic, Self::Dance, Self::Nap];

    /// The shared moment the journal writes it down as.
    pub const fn ritual(self) -> RitualKind {
        match self {
            Self::Picnic => RitualKind::Picnic,
            Self::Dance => RitualKind::Dance,
            Self::Nap => RitualKind::GroupNap,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RitualState {
    #[serde(with = "time::serde::rfc3339")]
    pub next_at_utc: OffsetDateTime,
    pub last_kind: Option<RitualKind>,
    pub ordinal: u32,
    pub hatch_day_acknowledged_year: Option<i32>,
}

impl Default for RitualState {
    fn default() -> Self {
        Self {
            next_at_utc: OffsetDateTime::UNIX_EPOCH,
            last_kind: None,
            ordinal: 0,
            hatch_day_acknowledged_year: None,
        }
    }
}
