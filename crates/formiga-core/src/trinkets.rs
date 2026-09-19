//! What a companion can bring back, and when.
//!
//! One table, read by the artwork, the scrapbook, and the simulation alike. The variant number is
//! the identifier a save keeps, so variants 0..8 mean exactly what they have always meant: a
//! colony that found a shell in an older version still has a shell here. Variants 8..16 are the
//! conditional ones, two for each circumstance, and they are new only in the sense that nothing
//! has selected them yet.
//!
//! A `hint` is what an undiscovered slot says. It describes the trinket's own world — after dark,
//! high up, partway along a ride, beside a friend — and never asks the reader to go and do
//! anything. Nothing here is a task list.

use crate::TRINKET_VARIANTS;

/// The circumstance a trinket turns up in. `Anywhere` is the original eight, which have no
/// condition at all and can be found on any ordinary day.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TrinketCondition {
    Anywhere,
    Night,
    HighTier,
    MidRide,
    BesideCloseFriend,
}

impl TrinketCondition {
    /// Every condition, in catalogue order.
    pub const ALL: [Self; 5] = [
        Self::Anywhere,
        Self::Night,
        Self::HighTier,
        Self::MidRide,
        Self::BesideCloseFriend,
    ];
}

/// One trinket, as the scrapbook and the artwork both read it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TrinketInfo {
    /// The stable identifier a save keeps. Equal to this entry's index in the catalogue.
    pub variant: u8,
    pub name: &'static str,
    /// Shown once the trinket has been found.
    pub description: &'static str,
    /// Shown while the slot is still empty.
    pub hint: &'static str,
    pub condition: TrinketCondition,
}

/// Ordinary finds share one honest line: there is nothing to wait for.
const ANYWHERE_HINT: &str = "Could turn up on any ordinary day.";

/// The catalogue. Variants 0..8 keep the names and descriptions the scrapbook has always shown.
static TRINKETS: [TrinketInfo; TRINKET_VARIANTS as usize] = [
    TrinketInfo {
        variant: 0,
        name: "Gem",
        description: "A cut stone that throws a little colour when the light moves.",
        hint: ANYWHERE_HINT,
        condition: TrinketCondition::Anywhere,
    },
    TrinketInfo {
        variant: 1,
        name: "Key",
        description: "A small key. Nobody has found the lock it belongs to.",
        hint: ANYWHERE_HINT,
        condition: TrinketCondition::Anywhere,
    },
    TrinketInfo {
        variant: 2,
        name: "Leaf",
        description: "A leaf pressed flat, kept for the shape of it.",
        hint: ANYWHERE_HINT,
        condition: TrinketCondition::Anywhere,
    },
    TrinketInfo {
        variant: 3,
        name: "Shell",
        description: "A spiral shell, carried a long way from any sea.",
        hint: ANYWHERE_HINT,
        condition: TrinketCondition::Anywhere,
    },
    TrinketInfo {
        variant: 4,
        name: "Ring charm",
        description: "A ring far too small for anyone here to wear.",
        hint: ANYWHERE_HINT,
        condition: TrinketCondition::Anywhere,
    },
    TrinketInfo {
        variant: 5,
        name: "Tiny bottle",
        description: "A stoppered bottle with something cloudy inside.",
        hint: ANYWHERE_HINT,
        condition: TrinketCondition::Anywhere,
    },
    TrinketInfo {
        variant: 6,
        name: "Star relic",
        description: "A little star of worn metal, its edges gone soft.",
        hint: ANYWHERE_HINT,
        condition: TrinketCondition::Anywhere,
    },
    TrinketInfo {
        variant: 7,
        name: "Odd little tablet",
        description: "A flat tablet marked with lines nobody can read.",
        hint: ANYWHERE_HINT,
        condition: TrinketCondition::Anywhere,
    },
    TrinketInfo {
        variant: 8,
        name: "Moon shard",
        description: "A sliver of pale stone that keeps a little light of its own.",
        hint: "Only turns up after dark.",
        condition: TrinketCondition::Night,
    },
    TrinketInfo {
        variant: 9,
        name: "Firefly jar",
        description: "A small jar, still faintly warm, with nothing inside it now.",
        hint: "Something from the late side of the evening.",
        condition: TrinketCondition::Night,
    },
    TrinketInfo {
        variant: 10,
        name: "Long feather",
        description: "A feather from something that passed by far overhead.",
        hint: "Comes from somewhere high up.",
        condition: TrinketCondition::HighTier,
    },
    TrinketInfo {
        variant: 11,
        name: "Cloud puff",
        description: "A tuft of something soft that will not quite settle.",
        hint: "Only found well above the ground.",
        condition: TrinketCondition::HighTier,
    },
    TrinketInfo {
        variant: 12,
        name: "Ticket stub",
        description: "Half a ticket, torn along the top, for a ride nobody remembers.",
        hint: "Picked up partway along a ride.",
        condition: TrinketCondition::MidRide,
    },
    TrinketInfo {
        variant: 13,
        name: "Little pinwheel",
        description: "A paper wheel that spins whenever anything moves past it.",
        hint: "Found mid-journey, never at either end.",
        condition: TrinketCondition::MidRide,
    },
    TrinketInfo {
        variant: 14,
        name: "Friendship knot",
        description: "A cord tied in a loop, with two ends that never come apart.",
        hint: "Only found beside a close friend.",
        condition: TrinketCondition::BesideCloseFriend,
    },
    TrinketInfo {
        variant: 15,
        name: "Matching charms",
        description: "Two small charms cut from the same piece, kept together.",
        hint: "Turns up when a dear friend is near.",
        condition: TrinketCondition::BesideCloseFriend,
    },
];

/// The catalogue entry for one variant, or `None` for a number no artwork exists for.
pub fn trinket_info(variant: u8) -> Option<&'static TrinketInfo> {
    TRINKETS.get(usize::from(variant))
}

/// Every trinket found in one circumstance, in catalogue order.
pub fn trinkets_for(condition: TrinketCondition) -> impl Iterator<Item = &'static TrinketInfo> {
    TRINKETS.iter().filter(move |t| t.condition == condition)
}

/// Every trinket, in catalogue order.
pub fn all_trinkets() -> impl Iterator<Item = &'static TrinketInfo> {
    TRINKETS.iter()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn the_catalogue_covers_every_variant_exactly_once() {
        assert_eq!(TRINKETS.len(), 16);
        assert_eq!(TRINKETS.len(), usize::from(TRINKET_VARIANTS));
        for (index, info) in TRINKETS.iter().enumerate() {
            assert_eq!(
                usize::from(info.variant),
                index,
                "variants are contiguous and equal to their own index"
            );
            assert_eq!(trinket_info(info.variant), Some(info));
        }
        assert_eq!(trinket_info(TRINKET_VARIANTS), None);
        assert_eq!(trinket_info(u8::MAX), None);
    }

    #[test]
    fn the_original_eight_keep_their_numbers_and_the_rest_are_conditional() {
        // A save from before this table still means what it meant.
        for variant in 0..8 {
            let info = trinket_info(variant).expect("the original eight are still here");
            assert_eq!(info.condition, TrinketCondition::Anywhere, "{variant}");
        }
        assert_eq!(trinket_info(0).unwrap().name, "Gem");
        assert_eq!(trinket_info(7).unwrap().name, "Odd little tablet");
        assert_eq!(trinkets_for(TrinketCondition::Anywhere).count(), 8);
        for condition in [
            TrinketCondition::Night,
            TrinketCondition::HighTier,
            TrinketCondition::MidRide,
            TrinketCondition::BesideCloseFriend,
        ] {
            let found: Vec<u8> = trinkets_for(condition).map(|t| t.variant).collect();
            assert_eq!(found.len(), 2, "{condition:?} should have exactly two");
            assert!(
                found.iter().all(|variant| *variant >= 8),
                "{condition:?} uses only new variants: {found:?}"
            );
        }
        assert_eq!(
            TrinketCondition::ALL
                .into_iter()
                .map(|condition| trinkets_for(condition).count())
                .sum::<usize>(),
            TRINKETS.len(),
            "every trinket belongs to exactly one condition"
        );
    }

    #[test]
    fn every_trinket_reads_as_its_own_thing() {
        let names: BTreeSet<&str> = TRINKETS.iter().map(|t| t.name).collect();
        assert_eq!(names.len(), TRINKETS.len(), "names are unique");
        let descriptions: BTreeSet<&str> = TRINKETS.iter().map(|t| t.description).collect();
        assert_eq!(
            descriptions.len(),
            TRINKETS.len(),
            "descriptions are unique"
        );
        for info in &TRINKETS {
            for text in [info.name, info.description, info.hint] {
                assert!(!text.trim().is_empty(), "{info:?} has an empty line");
                assert!(text.len() <= 80, "{text:?} is too long for a card");
            }
            // A hint describes where a thing lives. It never tells the reader to change what they
            // are doing with their own computer.
            let hint = info.hint.to_ascii_lowercase();
            for instruction in [
                "you ",
                "your ",
                "try ",
                "keep ",
                "leave ",
                "wait ",
                "make sure",
            ] {
                assert!(
                    !hint.contains(instruction),
                    "{:?} instructs the reader: {:?}",
                    info.name,
                    info.hint
                );
            }
        }
        // Conditional slots say something about their own circumstance rather than nothing.
        for info in TRINKETS
            .iter()
            .filter(|t| t.condition != TrinketCondition::Anywhere)
        {
            assert_ne!(
                info.hint, ANYWHERE_HINT,
                "{:?} needs its own hint",
                info.name
            );
        }
    }
}
