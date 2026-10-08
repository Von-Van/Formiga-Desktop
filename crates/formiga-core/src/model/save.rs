//! The colony file, the trips it keeps a log of, and the day's finds.

use super::{
    ArrivalState, ColonyHome, ColonyObjectState, Creature, CreatureRelationship, PairTally,
    RitualState, Settings,
};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SaveFile {
    #[serde(default)]
    pub companion: crate::CompanionState,
    pub save_version: u32,
    pub colony_seed: [u8; 32],
    #[serde(with = "time::serde::rfc3339")]
    pub created_at_utc: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub maximum_seen_utc: OffsetDateTime,
    pub arrival_state: ArrivalState,
    #[serde(default)]
    pub home: ColonyHome,
    pub settings: Settings,
    pub creatures: Vec<Creature>,
    #[serde(default)]
    pub relationships: Vec<CreatureRelationship>,
    /// What each pair has been seen doing together, one record per pair that has done anything.
    /// Absent from the file while nothing has been counted.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tallies: Vec<PairTally>,
    #[serde(default)]
    pub ritual: RitualState,
    #[serde(default)]
    pub objects: ColonyObjectState,
    #[serde(default)]
    pub visitors: crate::VisitorState,
    /// How many trinkets have turned up on the local day it names. A colony finds only a few a
    /// day, however many companions it has: see `daily_trinket_target`.
    #[serde(default, skip_serializing_if = "FindsToday::is_empty")]
    pub finds_today: FindsToday,
    /// A few counts for each of the last week's days, for the Today page to compare. Absent from
    /// the file until anything is counted.
    #[serde(default, skip_serializing_if = "crate::DayBook::is_empty")]
    pub day_book: crate::DayBook,
    /// The colony's trips away on the train, and the last of them. Absent from the file until the
    /// colony has been anywhere.
    #[serde(default, skip_serializing_if = "TripLog::is_empty")]
    pub trips: TripLog,
}

/// How often the colony has been away on the train and come home, and the last time. Only what
/// Desktop itself writes from a trip it checked is ever kept here: nothing another app sent is
/// copied in as it was sent.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TripLog {
    pub count: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last: Option<Trip>,
    /// The souvenirs the colony has brought home from Formiga Hill, each once, in the order they
    /// came.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub souvenirs: Vec<crate::SouvenirRecord>,
}

impl TripLog {
    pub fn is_empty(&self) -> bool {
        self.count == 0 && self.last.is_none() && self.souvenirs.is_empty()
    }

    /// Whether the colony has brought this souvenir home.
    pub fn has_souvenir(&self, souvenir: crate::Souvenir) -> bool {
        self.souvenirs
            .iter()
            .any(|record| record.souvenir == souvenir)
    }
}

/// One trip away and home again.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Trip {
    /// The trip's own identifier, 32 lowercase hex digits, kept so the same trip is never counted
    /// twice.
    pub session: String,
    #[serde(with = "time::serde::rfc3339")]
    pub arrived_at_utc: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub left_at_utc: OffsetDateTime,
}

impl Trip {
    /// Whether `session` is written the one way a trip's identifier is.
    pub fn is_session(session: &str) -> bool {
        session.len() == 32
            && session
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    }
}

/// The trinkets found so far on one local day.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FindsToday {
    /// The local day, as a Julian day number.
    pub day: i32,
    pub count: u8,
}

impl FindsToday {
    pub const fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// How many have been found on `day`.
    pub const fn on(&self, day: i32) -> u8 {
        if self.day == day { self.count } else { 0 }
    }
}

/// How many trinkets a colony finds on one local day: one to five, three on average, drawn
/// from the colony's seed and the date, so the same colony always has the same day.
pub fn daily_trinket_target(colony_seed: [u8; 32], day: i32) -> u8 {
    use rand::Rng;
    crate::SeedStream::new(colony_seed)
        .rng("daily-finds", u64::from(day.unsigned_abs()))
        .random_range(crate::tuning::FINDS.per_day)
}
