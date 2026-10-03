//! Formiga Hill's souvenirs, as Desktop keeps them.
//!
//! Formiga Hill gives these from its own catalogue, and decides when. Desktop keeps the ones it
//! knows once each, for the journal's page of them, and does nothing else with them: a souvenir
//! changes nobody and is never hung, worn or played with. Each is known by Formiga Hill's own
//! identifier for it, which is also how a save writes it; the names and lines are Desktop's.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

/// One of Formiga Hill's souvenirs that Desktop keeps.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Souvenir {
    PicnicRibbon,
    PressedDaisy,
    WellPenny,
    OakAcorn,
    SwingFeather,
    ChestMarble,
    FairTicket,
}

impl Souvenir {
    /// Every souvenir Desktop keeps, in the order Formiga Hill's display case shows them.
    pub const ALL: [Self; 7] = [
        Self::PicnicRibbon,
        Self::PressedDaisy,
        Self::WellPenny,
        Self::OakAcorn,
        Self::SwingFeather,
        Self::ChestMarble,
        Self::FairTicket,
    ];

    /// Formiga Hill's identifier for it: what a trip lists and a receipt names.
    pub const fn id(self) -> &'static str {
        match self {
            Self::PicnicRibbon => "picnic_ribbon",
            Self::PressedDaisy => "pressed_daisy",
            Self::WellPenny => "well_penny",
            Self::OakAcorn => "oak_acorn",
            Self::SwingFeather => "swing_feather",
            Self::ChestMarble => "chest_marble",
            Self::FairTicket => "fair_ticket",
        }
    }

    /// The souvenir Formiga Hill calls `id`, if it is one Desktop keeps.
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|souvenir| souvenir.id() == id)
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::PicnicRibbon => "Gingham ribbon",
            Self::PressedDaisy => "Pressed daisy",
            Self::WellPenny => "Well penny",
            Self::OakAcorn => "Oak acorn",
            Self::SwingFeather => "Swing feather",
            Self::ChestMarble => "Toy-chest marble",
            Self::FairTicket => "Fairground ticket",
        }
    }

    pub const fn description(self) -> &'static str {
        match self {
            Self::PicnicRibbon => "A bow of red gingham, from the colony's first picnic.",
            Self::PressedDaisy => "A daisy, pressed flat so it keeps.",
            Self::WellPenny => "A copper penny from the bottom of the well.",
            Self::OakAcorn => "An acorn from the old oak, still in its cap.",
            Self::SwingFeather => "A soft grey feather that caught on the swing.",
            Self::ChestMarble => "A glass marble with an amber twist, from the toy chest.",
            Self::FairTicket => "A ticket from the Fairground, torn along its perforations.",
        }
    }
}

/// A souvenir the colony brought home, and when it came.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SouvenirRecord {
    pub souvenir: Souvenir,
    #[serde(with = "time::serde::rfc3339")]
    pub brought_home_at_utc: OffsetDateTime,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn a_save_writes_each_souvenir_by_formiga_hills_own_identifier() {
        for souvenir in Souvenir::ALL {
            assert_eq!(serde_json::to_value(souvenir).unwrap(), souvenir.id());
            assert_eq!(Souvenir::from_id(souvenir.id()), Some(souvenir));
        }
        let ids: BTreeSet<_> = Souvenir::ALL.map(Souvenir::id).into();
        assert_eq!(ids.len(), Souvenir::ALL.len());
        for stranger in ["", "Picnic_Ribbon", "acorn-badge", "picnic_ribbon "] {
            assert_eq!(Souvenir::from_id(stranger), None, "{stranger:?}");
        }
    }

    #[test]
    fn every_souvenir_is_named_in_a_line_of_plain_text() {
        let names: BTreeSet<_> = Souvenir::ALL.map(Souvenir::name).into();
        assert_eq!(names.len(), Souvenir::ALL.len());
        for souvenir in Souvenir::ALL {
            for text in [souvenir.name(), souvenir.description()] {
                assert!(!text.is_empty() && text.len() <= 80, "{text}");
                assert!(!text.contains('\n'), "{text}");
            }
            assert!(souvenir.description().ends_with('.'));
        }
    }
}
