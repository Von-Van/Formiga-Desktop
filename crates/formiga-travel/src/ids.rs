//! A companion's identifier, written one way only. A trip's own identifier, the [`SessionId`]
//! every visit has, is the rulebook's.
//!
//! [`SessionId`]: formiga_expansion_rulebook::SessionId

use crate::document::{hex, unhex};
use serde::{Deserialize, Serialize};
use std::fmt;

/// A companion, by the identifier Desktop knows it by, written as 16 lowercase hex digits so that
/// no reader has to hold it in a floating-point number.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct TravelerId(pub u64);

impl fmt::Display for TravelerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&hex(&self.0.to_be_bytes()))
    }
}

impl TryFrom<String> for TravelerId {
    type Error = String;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        unhex::<8>(&text)
            .map(|bytes| Self(u64::from_be_bytes(bytes)))
            .ok_or_else(|| "a traveler id is 16 lowercase hex digits".to_owned())
    }
}

impl From<TravelerId> for String {
    fn from(id: TravelerId) -> Self {
        id.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn traveler_ids_survive_values_a_double_cannot_hold() {
        let id = TravelerId(u64::MAX - 1);
        let text = serde_json::to_string(&id).unwrap();
        assert_eq!(text, "\"fffffffffffffffe\"");
        assert_eq!(serde_json::from_str::<TravelerId>(&text).unwrap(), id);
        assert!(serde_json::from_str::<TravelerId>("18446744073709551614").is_err());
        assert!(serde_json::from_str::<TravelerId>("\"fffe\"").is_err());
    }
}
