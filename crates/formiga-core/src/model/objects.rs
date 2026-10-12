//! The belongings the colony keeps in the trees' yards.

use super::{DisplayKey, Point};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

pub const MAX_COLONY_OBJECTS: usize = 8;

/// A belonging the colony keeps in the two trees' yards. The first eight are the ones colonies
/// have always been given; the rest joined them in 0.60.0.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ColonyObjectKind {
    #[default]
    Pillow,
    Toy,
    Plant,
    Blanket,
    Paper,
    Pebble,
    Lamp,
    Cup,
    Kite,
    Teapot,
    Book,
    Basket,
    YarnBall,
    Drum,
    Umbrella,
    Bucket,
    Candle,
    MusicBox,
    SpinningTop,
    Jar,
}

impl ColonyObjectKind {
    pub const ALL: [Self; 20] = [
        Self::Pillow,
        Self::Toy,
        Self::Plant,
        Self::Blanket,
        Self::Paper,
        Self::Pebble,
        Self::Lamp,
        Self::Cup,
        Self::Kite,
        Self::Teapot,
        Self::Book,
        Self::Basket,
        Self::YarnBall,
        Self::Drum,
        Self::Umbrella,
        Self::Bucket,
        Self::Candle,
        Self::MusicBox,
        Self::SpinningTop,
        Self::Jar,
    ];

    pub const fn index(self) -> u8 {
        self as u8
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Pillow => "Pillow",
            Self::Toy => "Toy",
            Self::Plant => "Plant",
            Self::Blanket => "Blanket",
            Self::Paper => "Paper",
            Self::Pebble => "Pebble",
            Self::Lamp => "Lamp",
            Self::Cup => "Cup",
            Self::Kite => "Kite",
            Self::Teapot => "Teapot",
            Self::Book => "Book",
            Self::Basket => "Basket",
            Self::YarnBall => "Ball of yarn",
            Self::Drum => "Drum",
            Self::Umbrella => "Umbrella",
            Self::Bucket => "Bucket",
            Self::Candle => "Candle",
            Self::MusicBox => "Music box",
            Self::SpinningTop => "Spinning top",
            Self::Jar => "Jar",
        }
    }

    pub const fn default_role(self) -> ColonyObjectRole {
        match self {
            Self::Pillow | Self::Blanket | Self::MusicBox => ColonyObjectRole::Sleep,
            Self::Toy | Self::Kite | Self::YarnBall | Self::Drum | Self::SpinningTop => {
                ColonyObjectRole::Play
            }
            Self::Plant | Self::Lamp | Self::Basket | Self::Umbrella | Self::Candle => {
                ColonyObjectRole::Comfort
            }
            Self::Paper | Self::Pebble | Self::Book | Self::Bucket | Self::Jar => {
                ColonyObjectRole::Curiosity
            }
            Self::Cup | Self::Teapot => ColonyObjectRole::Social,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ColonyObjectRole {
    #[default]
    Comfort,
    Sleep,
    Play,
    Social,
    Curiosity,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ColonyObject {
    pub id: u64,
    pub kind: ColonyObjectKind,
    pub display: DisplayKey,
    pub normalized_position: Point,
    pub role: ColonyObjectRole,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ColonyObjectState {
    pub objects: Vec<ColonyObject>,
    #[serde(with = "time::serde::rfc3339")]
    pub next_at_utc: OffsetDateTime,
    pub ordinal: u32,
}

impl Default for ColonyObjectState {
    fn default() -> Self {
        Self {
            objects: Vec::new(),
            next_at_utc: OffsetDateTime::UNIX_EPOCH,
            ordinal: 0,
        }
    }
}
