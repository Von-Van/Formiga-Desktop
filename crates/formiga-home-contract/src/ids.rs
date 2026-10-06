//! Identifiers, each written one way only: lowercase letters, digits, `-`, `_` and `.`, so none is
//! ever a path, a script, or prose.

use crate::limits::MAX_ID_CHARS;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Whether `text` is an identifier as every one in this contract is written.
pub(crate) fn is_id(text: &str) -> bool {
    (1..=MAX_ID_CHARS).contains(&text.len())
        && text
            .bytes()
            .all(|byte| matches!(byte, b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.'))
}

/// Something from Home's own catalogue, by the identifier Home gave it: a piece of furniture
/// (`armchair`), a floor (`floor.boards`) or a wall (`wall.plaster`). Desktop checks only that it
/// is written as an identifier, never what it names: Home's catalogue can grow without Desktop.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct CatalogId(String);

impl CatalogId {
    pub fn parse(text: &str) -> Option<Self> {
        is_id(text).then(|| Self(text.to_owned()))
    }

    /// For identifiers written into the source, which are checked by the tests that use them.
    pub fn known(text: &'static str) -> Self {
        debug_assert!(is_id(text), "{text:?} is not an identifier");
        Self(text.to_owned())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CatalogId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for CatalogId {
    type Error = String;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        if is_id(&text) {
            Ok(Self(text))
        } else {
            Err("a catalogue id is a short lowercase identifier".to_owned())
        }
    }
}

impl From<CatalogId> for String {
    fn from(id: CatalogId) -> Self {
        id.0
    }
}

/// One thing the colony has that can be shown in a house, the same in every house and on every
/// visit: `find.3` for the scrapbook's shell, `souvenir.picnic_ribbon` for Formiga Hill's ribbon.
/// What it names is never worked out from the words: the source part says where it came from and
/// the rest is that source's own stable key.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct DisplayId(String);

impl DisplayId {
    pub const FIND: &'static str = "find";
    pub const SOUVENIR: &'static str = "souvenir";
    pub const MEMENTO: &'static str = "memento";

    /// A trinket from the scrapbook, by its catalogue variant.
    pub fn find(variant: u8) -> Self {
        Self(format!("{}.{variant}", Self::FIND))
    }

    /// One of a household's own keepsakes: the keeper of its house, and its number there, so no
    /// two houses' keepsakes are ever taken for one another.
    pub fn memento(keeper: formiga_travel::TravelerId, serial: u16) -> Self {
        Self(format!("{}.{}-{serial}", Self::MEMENTO, keeper.0))
    }

    /// One of Formiga Hill's souvenirs, by Hill's own identifier for it.
    pub fn souvenir(id: &str) -> Option<Self> {
        Self::parse(&format!("{}.{id}", Self::SOUVENIR))
    }

    pub fn parse(text: &str) -> Option<Self> {
        let (source, key) = text.split_once('.')?;
        (is_id(text) && !source.is_empty() && !key.is_empty()).then(|| Self(text.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Where it came from: `find`, `souvenir`, `memento`, or a source a newer Desktop has.
    pub fn source(&self) -> &str {
        self.0.split_once('.').map_or("", |(source, _)| source)
    }

    /// The source's own key for it.
    pub fn key(&self) -> &str {
        self.0.split_once('.').map_or("", |(_, key)| key)
    }
}

impl fmt::Display for DisplayId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for DisplayId {
    type Error = String;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        Self::parse(&text).ok_or_else(|| "a display id is a source and a key: find.3".to_owned())
    }
}

impl From<DisplayId> for String {
    fn from(id: DisplayId) -> Self {
        id.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifiers_are_only_ever_short_lowercase_words() {
        for fine in [
            "armchair",
            "floor.boards",
            "wall.leafy-paper",
            "toy_box",
            "a",
        ] {
            assert!(CatalogId::parse(fine).is_some(), "{fine:?}");
        }
        for hostile in [
            "",
            "../state.json",
            "Armchair",
            "arm chair",
            "armchair\n",
            "c:\\home",
            &"x".repeat(MAX_ID_CHARS + 1),
        ] {
            assert!(
                CatalogId::parse(hostile).is_none(),
                "{hostile:?} was accepted"
            );
            assert!(
                serde_json::from_value::<CatalogId>(serde_json::json!(hostile)).is_err(),
                "{hostile:?} was read"
            );
        }
    }

    #[test]
    fn a_display_id_names_its_source_and_its_key() {
        let shell = DisplayId::find(3);
        assert_eq!(shell.as_str(), "find.3");
        assert_eq!((shell.source(), shell.key()), ("find", "3"));
        let ribbon = DisplayId::souvenir("picnic_ribbon").unwrap();
        assert_eq!(ribbon.as_str(), "souvenir.picnic_ribbon");
        assert_eq!(ribbon.key(), "picnic_ribbon");
        for hostile in [
            "find",
            "find.",
            ".3",
            "find/3",
            "souvenir.Picnic",
            "find..3/",
        ] {
            assert!(
                DisplayId::parse(hostile).is_none(),
                "{hostile:?} was accepted"
            );
        }
        assert_eq!(DisplayId::souvenir("../colony"), None);
    }
}
