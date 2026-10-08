//! The houses: their styles and genomes, and the decorations they wear.

use super::{Creature, CreatureId};
use serde::{Deserialize, Serialize};

/// The four kinds of house, each built from a shape of its own. A colony file keeps the names
/// they had before they were drawn this way, so every existing colony keeps its houses.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ShelterStyle {
    /// A tent pitched from triangles.
    #[default]
    #[serde(rename = "LeafTent")]
    Tent,
    /// A mushroom made of circles.
    #[serde(rename = "MushroomHut")]
    Mushroom,
    /// A pillow fort stacked from squares.
    #[serde(rename = "CushionDen")]
    PillowFort,
    /// A cottage roofed and trimmed in leaves.
    #[serde(rename = "PaperHouse")]
    LeafHouse,
}

impl ShelterStyle {
    pub const ALL: [Self; 4] = [
        Self::Tent,
        Self::Mushroom,
        Self::PillowFort,
        Self::LeafHouse,
    ];

    /// What the Home page calls it.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Tent => "Tent",
            Self::Mushroom => "Mushroom",
            Self::PillowFort => "Pillow fort",
            Self::LeafHouse => "Leaf house",
        }
    }

    /// The house a companion would build for itself, from its own seed: the same one every time,
    /// and a different one from companion to companion often enough that a village mixes.
    pub fn for_keeper(creature: &Creature) -> Self {
        let pick = creature.behavior_seed[13] ^ creature.behavior_seed[29].rotate_left(3);
        Self::ALL[usize::from(pick) % Self::ALL.len()]
    }
}

/// A house type the person at the desk chose for one house, by the companion who keeps it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HouseStyleChoice {
    pub keeper: CreatureId,
    pub style: ShelterStyle,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShelterGenome {
    pub style: ShelterStyle,
    pub palette_index: u8,
    pub accent_index: u8,
    pub width: u8,
    pub height: u8,
    pub detail_seed: u64,
}

/// Where on a house a decoration hangs. Every house has one of each, and each takes one
/// decoration at a time.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum DecorationSlot {
    /// The peak of the roof.
    Roof,
    /// Strung along under the eaves.
    Eaves,
    /// On the front wall, left of the door.
    WallLeft,
    /// On the front wall, right of the door.
    WallRight,
    /// Set on the ground beside the left wall.
    GroundLeft,
    /// Set on the ground beside the right wall.
    GroundRight,
}

impl DecorationSlot {
    pub const ALL: [Self; 6] = [
        Self::Roof,
        Self::Eaves,
        Self::WallLeft,
        Self::WallRight,
        Self::GroundLeft,
        Self::GroundRight,
    ];

    pub const fn index(self) -> usize {
        match self {
            Self::Roof => 0,
            Self::Eaves => 1,
            Self::WallLeft => 2,
            Self::WallRight => 3,
            Self::GroundLeft => 4,
            Self::GroundRight => 5,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Roof => "Roof",
            Self::Eaves => "Eaves",
            Self::WallLeft => "Left wall",
            Self::WallRight => "Right wall",
            Self::GroundLeft => "Left of the house",
            Self::GroundRight => "Right of the house",
        }
    }
}

/// The most decorations one house wears: one in each slot.
pub const MAX_HOUSE_DECORATIONS: usize = DecorationSlot::ALL.len();

/// Something a house can be decorated with. The first six are the ones a colony earned before
/// 0.60.0, under the names its file keeps them by; each kind belongs to one slot on the house.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub enum ShelterDecorationKind {
    #[default]
    Leaf,
    Banner,
    Stone,
    Flower,
    Lamp,
    RoofOrnament,
    WeatherVane,
    Pennant,
    PerchedBird,
    Pinwheel,
    FairyLights,
    WindChime,
    LeafGarland,
    PaperLanterns,
    Wreath,
    WindowBox,
    Ivy,
    HouseSign,
    Clock,
    Birdhouse,
    Horseshoe,
    Mailbox,
    Woodpile,
    WateringCan,
    Boots,
    Barrel,
    PottedPlant,
    Pumpkin,
    Mushrooms,
    Lantern,
}

impl ShelterDecorationKind {
    pub const ALL: [Self; 30] = [
        Self::Leaf,
        Self::Banner,
        Self::Stone,
        Self::Flower,
        Self::Lamp,
        Self::RoofOrnament,
        Self::WeatherVane,
        Self::Pennant,
        Self::PerchedBird,
        Self::Pinwheel,
        Self::FairyLights,
        Self::WindChime,
        Self::LeafGarland,
        Self::PaperLanterns,
        Self::Wreath,
        Self::WindowBox,
        Self::Ivy,
        Self::HouseSign,
        Self::Clock,
        Self::Birdhouse,
        Self::Horseshoe,
        Self::Mailbox,
        Self::Woodpile,
        Self::WateringCan,
        Self::Boots,
        Self::Barrel,
        Self::PottedPlant,
        Self::Pumpkin,
        Self::Mushrooms,
        Self::Lantern,
    ];

    /// The decorations a new colony can put up from its first day.
    pub const STARTING: [Self; 3] = [Self::Banner, Self::Flower, Self::Lamp];

    pub const fn index(self) -> usize {
        self as usize
    }

    /// Where on a house it hangs.
    pub const fn slot(self) -> DecorationSlot {
        match self {
            Self::RoofOrnament
            | Self::WeatherVane
            | Self::Pennant
            | Self::PerchedBird
            | Self::Pinwheel => DecorationSlot::Roof,
            Self::Banner
            | Self::FairyLights
            | Self::WindChime
            | Self::LeafGarland
            | Self::PaperLanterns => DecorationSlot::Eaves,
            Self::Leaf | Self::Wreath | Self::WindowBox | Self::Ivy | Self::HouseSign => {
                DecorationSlot::WallLeft
            }
            Self::Lamp | Self::Clock | Self::Birdhouse | Self::Horseshoe | Self::Mailbox => {
                DecorationSlot::WallRight
            }
            Self::Stone | Self::Woodpile | Self::WateringCan | Self::Boots | Self::Barrel => {
                DecorationSlot::GroundLeft
            }
            Self::Flower | Self::PottedPlant | Self::Pumpkin | Self::Mushrooms | Self::Lantern => {
                DecorationSlot::GroundRight
            }
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Leaf => "Leaf sprig",
            Self::Banner => "Bunting",
            Self::Stone => "Doorstone",
            Self::Flower => "Flower",
            Self::Lamp => "Wall lamp",
            Self::RoofOrnament => "Roof star",
            Self::WeatherVane => "Weather vane",
            Self::Pennant => "Pennant",
            Self::PerchedBird => "Perched bird",
            Self::Pinwheel => "Pinwheel",
            Self::FairyLights => "Fairy lights",
            Self::WindChime => "Wind chime",
            Self::LeafGarland => "Leaf garland",
            Self::PaperLanterns => "Paper lanterns",
            Self::Wreath => "Wreath",
            Self::WindowBox => "Window box",
            Self::Ivy => "Ivy",
            Self::HouseSign => "House sign",
            Self::Clock => "Clock",
            Self::Birdhouse => "Birdhouse",
            Self::Horseshoe => "Horseshoe",
            Self::Mailbox => "Letterbox",
            Self::Woodpile => "Woodpile",
            Self::WateringCan => "Watering can",
            Self::Boots => "Boots",
            Self::Barrel => "Rain barrel",
            Self::PottedPlant => "Potted plant",
            Self::Pumpkin => "Pumpkin",
            Self::Mushrooms => "Mushrooms",
            Self::Lantern => "Lantern",
        }
    }
}

/// Which decorations one house wears, by the companion who keeps it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HouseDressing {
    pub keeper: CreatureId,
    /// At most one for each slot, in slot order.
    pub decorations: Vec<ShelterDecorationKind>,
}
