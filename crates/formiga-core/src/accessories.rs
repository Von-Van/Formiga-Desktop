//! Small things a companion can wear: one at a time, chosen by the person at the desk.
//!
//! Every accessory is made from something the colony found. A leaf becomes a leaf hat, a pressed
//! daisy a flower crown, a scrap of wool a scarf; and any keepsake at all can be worn as a pin. So
//! what the colony has brought back is what its companions have to choose from, and the scrapbook
//! is the only record of what is available — nothing about an accessory is unlocked separately.

use crate::{CreatureId, ScrapbookRecord, TRINKET_VARIANTS, trinket_info};
use serde::{Deserialize, Serialize};

/// Where on a companion an accessory is worn.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AccessoryPlace {
    /// On top of the head: hats, crowns and bows.
    Head,
    /// Round the neck: scarves, collars and necklaces.
    Neck,
    /// Across the body: a bag on a strap, or something carried on the back.
    Body,
    /// Pinned on the chest.
    Pin,
}

/// Something a companion can wear, made from a find.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum AccessoryKind {
    LeafHat,
    FlowerCrown,
    AcornCap,
    FeatherCap,
    MushroomCap,
    RibbonBow,
    PartyHat,
    Sprout,
    StarClip,
    MoonClip,
    CloudEarmuffs,
    Nightcap,
    KnittedScarf,
    Bandana,
    BellCollar,
    ShellNecklace,
    PetalRuff,
    FriendshipCord,
    TinySatchel,
    SnailPack,
}

impl AccessoryKind {
    pub const ALL: [Self; 20] = [
        Self::LeafHat,
        Self::FlowerCrown,
        Self::AcornCap,
        Self::FeatherCap,
        Self::MushroomCap,
        Self::RibbonBow,
        Self::PartyHat,
        Self::Sprout,
        Self::StarClip,
        Self::MoonClip,
        Self::CloudEarmuffs,
        Self::Nightcap,
        Self::KnittedScarf,
        Self::Bandana,
        Self::BellCollar,
        Self::ShellNecklace,
        Self::PetalRuff,
        Self::FriendshipCord,
        Self::TinySatchel,
        Self::SnailPack,
    ];

    pub const fn index(self) -> u8 {
        self as u8
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::LeafHat => "Leaf hat",
            Self::FlowerCrown => "Flower crown",
            Self::AcornCap => "Acorn cap",
            Self::FeatherCap => "Feather cap",
            Self::MushroomCap => "Toadstool cap",
            Self::RibbonBow => "Ribbon bow",
            Self::PartyHat => "Party hat",
            Self::Sprout => "Sprout",
            Self::StarClip => "Star clip",
            Self::MoonClip => "Moon clip",
            Self::CloudEarmuffs => "Cloud earmuffs",
            Self::Nightcap => "Nightcap",
            Self::KnittedScarf => "Knitted scarf",
            Self::Bandana => "Bandana",
            Self::BellCollar => "Bell collar",
            Self::ShellNecklace => "Shell necklace",
            Self::PetalRuff => "Petal ruff",
            Self::FriendshipCord => "Friendship cord",
            Self::TinySatchel => "Tiny satchel",
            Self::SnailPack => "Snail-shell pack",
        }
    }

    pub const fn place(self) -> AccessoryPlace {
        match self {
            Self::LeafHat
            | Self::FlowerCrown
            | Self::AcornCap
            | Self::FeatherCap
            | Self::MushroomCap
            | Self::RibbonBow
            | Self::PartyHat
            | Self::Sprout
            | Self::StarClip
            | Self::MoonClip
            | Self::CloudEarmuffs
            | Self::Nightcap => AccessoryPlace::Head,
            Self::KnittedScarf
            | Self::Bandana
            | Self::BellCollar
            | Self::ShellNecklace
            | Self::PetalRuff
            | Self::FriendshipCord => AccessoryPlace::Neck,
            Self::TinySatchel | Self::SnailPack => AccessoryPlace::Body,
        }
    }

    /// The find it is made from. Finding it once is enough for every companion to wear one.
    pub const fn made_from(self) -> u8 {
        match self {
            Self::LeafHat => 2,
            Self::FlowerCrown => 26,
            Self::AcornCap => 16,
            Self::FeatherCap => 10,
            Self::MushroomCap => 108,
            Self::RibbonBow => 23,
            Self::PartyHat => 152,
            Self::Sprout => 104,
            Self::StarClip => 6,
            Self::MoonClip => 8,
            Self::CloudEarmuffs => 11,
            Self::Nightcap => 63,
            Self::KnittedScarf => 24,
            Self::Bandana => 28,
            Self::BellCollar => 25,
            Self::ShellNecklace => 3,
            Self::PetalRuff => 107,
            Self::FriendshipCord => 14,
            Self::TinySatchel => 27,
            Self::SnailPack => 105,
        }
    }

    /// The accessory a find makes, if it makes one.
    pub fn made_by(variant: u8) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|kind| kind.made_from() == variant)
    }
}

/// What one companion is wearing: an accessory made from a find, or a find itself worn as a pin.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Accessory {
    Worn(AccessoryKind),
    /// A trinket, by catalogue variant, pinned on the chest.
    Pin(u8),
}

impl Accessory {
    pub const fn place(self) -> AccessoryPlace {
        match self {
            Self::Worn(kind) => kind.place(),
            Self::Pin(_) => AccessoryPlace::Pin,
        }
    }

    /// What the colony page calls it.
    pub fn label(self) -> String {
        match self {
            Self::Worn(kind) => kind.label().to_owned(),
            Self::Pin(variant) => format!(
                "{} pin",
                trinket_info(variant).map_or("Keepsake", |info| info.name)
            ),
        }
    }

    /// Whether the colony has found what this is made from.
    pub fn available(self, scrapbook: &[ScrapbookRecord]) -> bool {
        let found = |variant: u8| scrapbook.iter().any(|record| record.variant == variant);
        match self {
            Self::Worn(kind) => found(kind.made_from()),
            Self::Pin(variant) => variant < TRINKET_VARIANTS && found(variant),
        }
    }
}

/// Everything a companion in this colony could wear right now: the accessories made from what it
/// has found, in catalogue order, then every find as a pin, in the order of the catalogue.
pub fn available_accessories(scrapbook: &[ScrapbookRecord]) -> Vec<Accessory> {
    let mut available: Vec<Accessory> = AccessoryKind::ALL
        .into_iter()
        .map(Accessory::Worn)
        .filter(|accessory| accessory.available(scrapbook))
        .collect();
    let mut pins: Vec<u8> = scrapbook
        .iter()
        .map(|record| record.variant)
        .filter(|variant| *variant < TRINKET_VARIANTS)
        .collect();
    pins.sort_unstable();
    pins.dedup();
    available.extend(pins.into_iter().map(Accessory::Pin));
    available
}

/// Why a companion could not be dressed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum AccessoryError {
    #[error("no companion by that name lives here")]
    UnknownCreature(CreatureId),
    #[error("the colony has not found what that is made from")]
    NotFound,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn every_accessory_is_made_from_a_different_find_that_exists() {
        let mut sources = BTreeSet::new();
        for kind in AccessoryKind::ALL {
            let source = kind.made_from();
            assert!(
                trinket_info(source).is_some(),
                "{kind:?} is made from a find that is not in the catalogue"
            );
            assert!(sources.insert(source), "{kind:?} shares its find");
            assert_eq!(AccessoryKind::made_by(source), Some(kind));
            assert!(kind.label().len() <= 20);
        }
        let labels: BTreeSet<&str> = AccessoryKind::ALL.iter().map(|k| k.label()).collect();
        assert_eq!(labels.len(), AccessoryKind::ALL.len());
        assert_eq!(AccessoryKind::made_by(0), None, "a gem is only ever a pin");
    }

    #[test]
    fn only_what_the_colony_has_found_can_be_worn() {
        let record = |variant: u8| ScrapbookRecord {
            variant,
            first_at: time::OffsetDateTime::UNIX_EPOCH,
            finder: None,
            finder_name: String::new(),
        };
        assert!(available_accessories(&[]).is_empty());
        let scrapbook = [record(2), record(0), record(200)];
        let available = available_accessories(&scrapbook);
        assert_eq!(
            available,
            vec![
                Accessory::Worn(AccessoryKind::LeafHat),
                Accessory::Pin(0),
                Accessory::Pin(2),
            ]
        );
        assert!(!Accessory::Worn(AccessoryKind::FlowerCrown).available(&scrapbook));
        assert!(!Accessory::Pin(200).available(&scrapbook));
        assert_eq!(Accessory::Pin(2).label(), "Leaf pin");
        assert_eq!(
            Accessory::Worn(AccessoryKind::LeafHat).place(),
            AccessoryPlace::Head
        );
    }
}
