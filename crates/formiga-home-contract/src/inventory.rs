//! The things a colony has that can be shown in a house: every find in its scrapbook and every
//! souvenir it has brought home from Formiga Hill, by stable identifier, with only what Home
//! needs to show one. Never the scrapbook itself, and never a trip's receipt.
//!
//! Each thing says how it may be shown rather than leaving Home to guess from its picture, so
//! where it can go is the same on every visit and in every version of Home. Every thing can be
//! shown on a card ([`DisplayMode::FallbackCard`]): a find never goes missing from a house
//! because nobody has drawn a piece of furniture for it yet.

use crate::document::HomeError;
use crate::ids::DisplayId;
use crate::limits::*;
use formiga_core::Souvenir;
use formiga_expansion_rulebook::is_sanitized;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use time::OffsetDateTime;

/// One way a thing may be shown, in the order it would rather be.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DisplayMode {
    /// On a table, a sill or a shelf, as itself: a shell, a key, a button.
    SurfaceSmall,
    /// In a shelf or a glass case, kept a little apart: something rare, or delicate.
    Case,
    /// Hung on a wall, as itself: a ribbon, a postcard, a pressed leaf.
    Wall,
    /// Standing on the floor, as itself: a toy sheep, a firefly jar, a little broom.
    Floor,
    /// Draped or tied: a ribbon, a scrap of wool, a streamer.
    Textile,
    /// On a card, a plaque or in a frame, wherever a small thing or a picture can go. Every thing
    /// may be shown so.
    FallbackCard,
    /// A way a newer Desktop has that this build does not know. It allows nothing.
    #[serde(other)]
    Unknown,
}

/// Where a thing came from, and the key its source knows it by.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DisplaySource {
    /// A trinket in the scrapbook, by its catalogue variant: what `formiga_core::trinket_info`
    /// describes and `formiga_art::draw_trinket` draws.
    DesktopFind { variant: u8 },
    /// One of Formiga Hill's souvenirs that Desktop has kept, by Hill's own identifier.
    HillSouvenir { id: String },
    /// One of the household's own keepsakes, made at home. Never sent by Desktop: Home makes
    /// these for itself, from the household's home. Since version 4.
    HomeMemento { memento: crate::state::MementoKind },
    /// A source a newer Desktop has. Shown on its card, by its name.
    #[serde(other)]
    Unknown,
}

/// The five colours a find is drawn in, resolved by Desktop from the colony it belongs to, so
/// Home draws it the same without knowing anything about that colony.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ink {
    pub outline: [u8; 3],
    pub deep: [u8; 3],
    pub body: [u8; 3],
    pub light: [u8; 3],
    pub accent: [u8; 3],
}

impl Ink {
    pub fn of(ink: formiga_art::TrinketInk) -> Self {
        let rgb = |color: formiga_art::Rgba| [color.r, color.g, color.b];
        Self {
            outline: rgb(ink.outline),
            deep: rgb(ink.deep),
            body: rgb(ink.body),
            light: rgb(ink.light),
            accent: rgb(ink.accent),
        }
    }

    pub fn to_art(self) -> formiga_art::TrinketInk {
        let rgba = |[r, g, b]: [u8; 3]| formiga_art::Rgba::new(r, g, b, 255);
        formiga_art::TrinketInk {
            outline: rgba(self.outline),
            deep: rgba(self.deep),
            body: rgba(self.body),
            light: rgba(self.light),
            accent: rgba(self.accent),
        }
    }
}

/// One thing the colony has that a house can show.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DisplayItem {
    pub id: DisplayId,
    pub source: DisplaySource,
    /// Desktop's name for it: "Shell", "Gingham ribbon". For showing only.
    pub name: String,
    /// The colours a find is drawn in. A souvenir has its own fixed colours, and none here.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ink: Option<Ink>,
    /// How it may be shown, best first. Always includes [`DisplayMode::FallbackCard`].
    pub modes: Vec<DisplayMode>,
    /// When the colony first found it, or brought it home.
    #[serde(with = "time::serde::rfc3339")]
    pub found_at_utc: OffsetDateTime,
    /// Who found it, by name, if anyone in particular did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finder_name: Option<String>,
}

impl DisplayItem {
    pub fn allows(&self, mode: DisplayMode) -> bool {
        mode != DisplayMode::Unknown && self.modes.contains(&mode)
    }

    pub(crate) fn validate(&self) -> Result<(), HomeError> {
        let invalid = HomeError::invalid;
        let names_itself = match &self.source {
            DisplaySource::DesktopFind { variant } => {
                self.id.source() == DisplayId::FIND
                    && self.id.key() == variant.to_string()
                    && self.ink.is_some()
            }
            DisplaySource::HillSouvenir { id } => {
                self.id.source() == DisplayId::SOUVENIR && self.id.key() == id
            }
            // Desktop never sends a keepsake: Home makes those for itself.
            DisplaySource::HomeMemento { .. } => false,
            DisplaySource::Unknown => true,
        };
        if !names_itself {
            return Err(invalid("a thing's id does not say where it came from"));
        }
        if !is_sanitized(&self.name, MAX_ITEM_NAME_CHARS) {
            return Err(invalid("a thing's name is not plain text"));
        }
        if self
            .finder_name
            .as_deref()
            .is_some_and(|name| !is_sanitized(name, MAX_NAME_CHARS))
        {
            return Err(invalid("a finder's name is not plain text"));
        }
        let modes: BTreeSet<_> = self.modes.iter().collect();
        if self.modes.len() > MAX_DISPLAY_MODES
            || modes.len() != self.modes.len()
            || !self.modes.contains(&DisplayMode::FallbackCard)
        {
            return Err(invalid(
                "a thing's ways of being shown are not a short list that includes a card",
            ));
        }
        Ok(())
    }
}

/// Finds flat or light enough to hang on a wall.
const WALL_FINDS: [u8; 33] = [
    2, 10, 12, 14, 23, 26, 28, 49, 57, 59, 61, 66, 70, 74, 76, 77, 84, 98, 99, 101, 104, 107, 115,
    124, 125, 127, 136, 137, 142, 144, 145, 152, 158,
];
/// Finds that drape or tie.
const TEXTILE_FINDS: [u8; 15] = [
    14, 23, 24, 28, 41, 63, 65, 66, 84, 87, 111, 125, 133, 142, 152,
];
/// Finds that stand on the floor: the bigger, the odder, and the ones that grow.
const FLOOR_FINDS: [u8; 21] = [
    9, 13, 20, 22, 44, 62, 78, 91, 96, 103, 108, 109, 113, 117, 120, 132, 134, 138, 141, 146, 150,
];
/// Finds rare or delicate enough to keep in a case first.
const PRECIOUS_FINDS: [u8; 22] = [
    0, 6, 7, 8, 15, 58, 60, 64, 83, 97, 116, 123, 135, 140, 147, 148, 149, 151, 156, 157, 158, 159,
];

/// How a find from the scrapbook may be shown, best first. The same for every colony.
pub fn find_modes(variant: u8) -> Vec<DisplayMode> {
    use DisplayMode::*;
    let mut modes = if FLOOR_FINDS.contains(&variant) {
        vec![Floor, SurfaceSmall]
    } else if PRECIOUS_FINDS.contains(&variant) {
        vec![Case, SurfaceSmall]
    } else {
        vec![SurfaceSmall, Case]
    };
    // A flat thing would rather hang, unless it is precious enough to keep behind glass first.
    if WALL_FINDS.contains(&variant) {
        let at = usize::from(PRECIOUS_FINDS.contains(&variant));
        modes.insert(at, Wall);
    }
    if TEXTILE_FINDS.contains(&variant) {
        modes.push(Textile);
    }
    modes.push(FallbackCard);
    modes
}

/// How one of Formiga Hill's souvenirs may be shown, best first.
pub fn souvenir_modes(souvenir: Souvenir) -> Vec<DisplayMode> {
    use DisplayMode::*;
    match souvenir {
        Souvenir::PicnicRibbon => vec![Wall, Textile, SurfaceSmall, Case, FallbackCard],
        Souvenir::PressedDaisy => vec![Wall, Case, SurfaceSmall, FallbackCard],
        Souvenir::SwingFeather | Souvenir::FairTicket => {
            vec![Wall, SurfaceSmall, Case, FallbackCard]
        }
        Souvenir::WellPenny | Souvenir::ChestMarble => vec![Case, SurfaceSmall, FallbackCard],
        Souvenir::OakAcorn | Souvenir::StrikerBell => vec![SurfaceSmall, Case, FallbackCard],
        Souvenir::RaceRosette => vec![Wall, Textile, SurfaceSmall, Case, FallbackCard],
        Souvenir::HooplaTeddy => vec![SurfaceSmall, Floor, Case, FallbackCard],
        Souvenir::TugRope => vec![Wall, Textile, SurfaceSmall, FallbackCard],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_find_and_souvenir_can_be_shown_on_a_card() {
        for variant in 0..formiga_core::TRINKET_VARIANTS {
            let modes = find_modes(variant);
            assert!(modes.contains(&DisplayMode::FallbackCard), "find {variant}");
            assert!(modes.len() <= MAX_DISPLAY_MODES);
            let unique: BTreeSet<_> = modes.iter().collect();
            assert_eq!(unique.len(), modes.len(), "find {variant} repeats a mode");
        }
        for souvenir in Souvenir::ALL {
            assert_eq!(
                souvenir_modes(souvenir).last(),
                Some(&DisplayMode::FallbackCard)
            );
        }
    }

    #[test]
    fn the_lists_name_only_finds_the_catalogue_has() {
        for list in [
            &WALL_FINDS[..],
            &TEXTILE_FINDS[..],
            &FLOOR_FINDS[..],
            &PRECIOUS_FINDS[..],
        ] {
            assert!(list.iter().all(|v| *v < formiga_core::TRINKET_VARIANTS));
            assert!(
                list.windows(2).all(|pair| pair[0] < pair[1]),
                "kept in order"
            );
        }
        assert!(
            FLOOR_FINDS.iter().all(|v| !PRECIOUS_FINDS.contains(v)),
            "a find stands on the floor or waits in a case, not both"
        );
    }

    #[test]
    fn ways_a_newer_desktop_has_read_as_unknown_and_allow_nothing() {
        let modes: Vec<DisplayMode> =
            serde_json::from_str(r#"["wall", "hologram", "fallback_card"]"#).unwrap();
        assert_eq!(
            modes,
            vec![
                DisplayMode::Wall,
                DisplayMode::Unknown,
                DisplayMode::FallbackCard
            ]
        );
        let source: DisplaySource =
            serde_json::from_str(r#"{"kind": "postcard", "from": "Lisbon"}"#).unwrap();
        assert_eq!(source, DisplaySource::Unknown);
    }
}
