//! One household as it is opened in Home: who lives in the house, how each one looks and carries
//! itself, how they get on, who keeps the other houses in the village, and what the colony has
//! that a house can show. Nothing about where anyone was on the desktop, what was open on it, or
//! how Desktop runs them.

use crate::document::{HomeDocument, HomeError, header_ok, is_lower_hex};
use crate::inventory::DisplayItem;
use crate::limits::*;
use crate::{DisplayId, HOME_FORMAT_VERSION, SNAPSHOT_FORMAT};
use formiga_core as core;
use formiga_travel::{
    Document as _, Presentation, SessionId, TravelRelationship, TravelRole, TravelSnapshot,
    Traveler, TravelerId, is_sanitized,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use time::OffsetDateTime;

/// What Desktop will do with Home's answers. Home should only ask for what is listed; anything
/// else in a receipt is set aside unread. Desktop always keeps a result that checks out, whatever
/// is offered here: the layouts are Home's own to keep.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HomeCapability {
    /// A [`crate::HomeEffect::HomeVisit`]: Desktop notes, in its own words, that the household
    /// had company.
    VisitRecord,
    /// [`crate::HomeEffect::Together`]: small nudges to how companions get on, from time they
    /// spent together at home. Since version 4.
    BondNudges,
    /// [`crate::HomeEffect::Moment`]: a few lines for Desktop's journal, in its own words.
    /// Since version 4.
    JournalMoments,
    /// [`crate::HomeEffect::NextDoor`]: Desktop opens the house the owner went over to, next,
    /// if it can. Since version 5.
    NextDoor,
    /// [`crate::HomeEffect::MoveIn`]: Desktop considers, by its own rules, a visitor the owner
    /// asked to come and live here. Since version 6.
    Roommates,
    /// [`crate::HomeIndoors`]: Desktop keeps indoors only whoever Home says is in the house, and
    /// everyone else lent for the visit out on the desktop, as the owner chooses and as visitors
    /// come and go. Since version 8.
    Indoors,
    /// Anything a newer Desktop offers that this build does not know.
    #[serde(other)]
    Unknown,
}

impl HomeCapability {
    /// Every capability this build knows: what a rehearsal offers, so Home can show everything it
    /// would do for a Desktop that took all of it back. Desktop offers only what it applies.
    pub const ALL: [Self; 6] = [
        Self::VisitRecord,
        Self::BondNudges,
        Self::JournalMoments,
        Self::NextDoor,
        Self::Roommates,
        Self::Indoors,
    ];
}

/// The kind of house the household lives in, as the village shows it from outside. A hint for
/// how the inside might first be decorated, and nothing more: a layout belongs to the household,
/// not to the house's outside, and is kept whatever the owner later makes the outside.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HouseStyle {
    Tent,
    Mushroom,
    PillowFort,
    LeafHouse,
    #[serde(other)]
    Unknown,
}

impl From<core::ShelterStyle> for HouseStyle {
    fn from(style: core::ShelterStyle) -> Self {
        match style {
            core::ShelterStyle::Tent => Self::Tent,
            core::ShelterStyle::Mushroom => Self::Mushroom,
            core::ShelterStyle::PillowFort => Self::PillowFort,
            core::ShelterStyle::LeafHouse => Self::LeafHouse,
        }
    }
}

/// The house that was opened.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Household {
    /// The full-size companion who keeps the house: the household's identity, in Home's layouts
    /// as everywhere else.
    pub keeper: TravelerId,
    /// Where the house stands in the village, counting the colony house as 0.
    pub slot: u8,
    pub style: HouseStyle,
}

/// Who keeps one of the village's houses, so Home can say where something is shown.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Neighbour {
    pub keeper: TravelerId,
    pub name: String,
    pub slot: u8,
}

/// One household as it is opened in Home.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HomeSnapshot {
    pub format: String,
    pub version: u32,
    pub min_reader_version: u32,
    pub session_id: SessionId,
    /// The same colony on every visit, as 16 lowercase hex digits: a one-way digest of what makes
    /// it that colony, and not the one a trip carries, so the two apps' records of a colony
    /// cannot be matched up.
    pub colony_key: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at_utc: OffsetDateTime,
    pub desktop_version: String,
    /// The travel version each resident is written in, and the oldest travel reader that can
    /// draw every one of them: a resident is exactly what a trip would carry.
    pub travel_version: u32,
    pub travel_min_reader_version: u32,
    pub capabilities: Vec<HomeCapability>,
    pub household: Household,
    /// The keeper first, then everyone who lives with it, in the order they arrived.
    pub residents: Vec<Traveler>,
    /// Friends from other houses whom Desktop has lent for the visit, since version 2. Desktop
    /// keeps them indoors too while the house is open, or, where it offers
    /// [`HomeCapability::Indoors`], while Home says they are in. Each keeps a house of its own in the
    /// village; none lives here.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub visitors: Vec<Traveler>,
    /// How everyone in the house gets on, residents and visitors alike.
    #[serde(default)]
    pub relationships: Vec<TravelRelationship>,
    /// Every house in the village, in the order they stand, this one included.
    #[serde(default)]
    pub village: Vec<Neighbour>,
    /// Everything the colony has that a house can show, wherever it is shown now.
    #[serde(default)]
    pub inventory: Vec<DisplayItem>,
    /// Whole days since the colony began: what a few pieces of furniture wait for.
    #[serde(default)]
    pub days_lived: u32,
    #[serde(default)]
    pub presentation: Presentation,
}

impl HomeSnapshot {
    pub fn offers(&self, capability: HomeCapability) -> bool {
        self.capabilities.contains(&capability)
    }

    pub fn resident(&self, id: TravelerId) -> Option<&Traveler> {
        self.residents.iter().find(|resident| resident.id == id)
    }

    pub fn visitor(&self, id: TravelerId) -> Option<&Traveler> {
        self.visitors.iter().find(|visitor| visitor.id == id)
    }

    pub fn item(&self, id: &DisplayId) -> Option<&DisplayItem> {
        self.inventory.iter().find(|item| &item.id == id)
    }

    pub fn neighbour(&self, keeper: TravelerId) -> Option<&Neighbour> {
        self.village.iter().find(|house| house.keeper == keeper)
    }

    /// Whether this build of `formiga-travel` can draw every resident.
    pub fn residents_readable(&self) -> bool {
        self.travel_min_reader_version <= formiga_travel::TRAVEL_FORMAT_VERSION
    }

    /// Everyone in the house as a trip would carry them, for anything that reads a trip's
    /// snapshot: the residents and any visitors, with their bonds and the owner's preferences,
    /// and nothing else of a trip.
    pub fn as_travel(&self) -> TravelSnapshot {
        TravelSnapshot {
            format: formiga_travel::SNAPSHOT_FORMAT.to_owned(),
            version: formiga_travel::TRAVEL_FORMAT_VERSION,
            min_reader_version: self
                .travel_min_reader_version
                .clamp(1, formiga_travel::TRAVEL_FORMAT_VERSION),
            session_id: self.session_id.clone(),
            colony_id: self.colony_key.clone(),
            created_at_utc: self.created_at_utc,
            desktop_version: self.desktop_version.clone(),
            capabilities: Vec::new(),
            accepts_souvenirs: Vec::new(),
            travelers: self
                .residents
                .iter()
                .chain(&self.visitors)
                .cloned()
                .collect(),
            relationships: self.relationships.clone(),
            presentation: self.presentation,
        }
    }
}

impl HomeDocument for HomeSnapshot {
    const FORMAT: &'static str = SNAPSHOT_FORMAT;
    const MAX_BYTES: u64 = MAX_SNAPSHOT_BYTES;

    fn validate(&self) -> Result<(), HomeError> {
        let invalid = HomeError::invalid;
        if !header_ok(
            &self.format,
            self.version,
            self.min_reader_version,
            SNAPSHOT_FORMAT,
        ) {
            return Err(invalid(
                "the snapshot's header is not one this build writes",
            ));
        }
        if !is_sanitized(&self.desktop_version, MAX_VERSION_CHARS) {
            return Err(invalid("the Desktop version is not plain text"));
        }
        if !is_lower_hex(&self.colony_key, 16) {
            return Err(invalid("a colony key is 16 lowercase hex digits"));
        }
        if !(1..=self.travel_version).contains(&self.travel_min_reader_version) {
            return Err(invalid("the residents' travel version does not add up"));
        }
        if self.capabilities.len() > MAX_CAPABILITIES {
            return Err(invalid("too many capabilities"));
        }
        if self.residents.is_empty() || self.residents.len() > MAX_RESIDENTS {
            return Err(invalid("a household is one to twelve companions"));
        }
        if self.visitors.len() > MAX_VISITORS
            || self.residents.len() + self.visitors.len() > MAX_RESIDENTS
        {
            return Err(invalid("too many visitors"));
        }
        // Each resident, and the bonds between them, are held to everything a trip's travelers
        // are: names, looks, ranges, little ones with their adults, pairs listed once.
        self.as_travel().validate().map_err(|error| {
            HomeError::invalid(format!("a resident does not check out: {error}"))
        })?;
        let keeper = &self.residents[0];
        if keeper.id != self.household.keeper || keeper.role != TravelRole::Adult {
            return Err(invalid(
                "the household's keeper is not its first, full-size resident",
            ));
        }
        if self.village.len() > MAX_HOUSEHOLDS {
            return Err(invalid("too many houses in the village"));
        }
        let keepers: BTreeSet<_> = self.village.iter().map(|house| house.keeper).collect();
        let slots: BTreeSet<_> = self.village.iter().map(|house| house.slot).collect();
        if keepers.len() != self.village.len()
            || slots.len() != self.village.len()
            || !self
                .village
                .iter()
                .all(|house| is_sanitized(&house.name, MAX_NAME_CHARS))
        {
            return Err(invalid(
                "the village's houses are not each kept by a different companion",
            ));
        }
        let home = self.neighbour(self.household.keeper);
        if home.is_none_or(|house| house.slot != self.household.slot) {
            return Err(invalid("the household's own house is not in the village"));
        }
        // A visitor is a full-size companion who keeps a house of its own, and not this one.
        let lent = self.visitors.iter().all(|visitor| {
            visitor.role == TravelRole::Adult
                && visitor.id != self.household.keeper
                && self.neighbour(visitor.id).is_some()
        });
        if !lent {
            return Err(invalid("a visitor does not keep a house of its own"));
        }
        if self.inventory.len() > MAX_INVENTORY {
            return Err(invalid("too many things to show"));
        }
        let mut ids = BTreeSet::new();
        for item in &self.inventory {
            item.validate()?;
            if !ids.insert(&item.id) {
                return Err(invalid("a thing to show is listed twice"));
            }
        }
        if !(100..=150).contains(&self.presentation.text_scale_percent) {
            return Err(invalid("a text size out of range"));
        }
        Ok(())
    }
}

/// A snapshot with this build's header, for the projection to fill.
pub(crate) fn header() -> (String, u32, u32) {
    (SNAPSHOT_FORMAT.to_owned(), HOME_FORMAT_VERSION, 1)
}
