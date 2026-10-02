//! Identifiers, each written one way only, so a path built from one can never leave the
//! directory it names.

use crate::document::{hex, unhex};
use serde::{Deserialize, Serialize};
use std::fmt;

/// One trip: 128 random bits, written as 32 lowercase hex digits. It is also the name of the
/// trip's session directory, which is safe only because nothing else is ever accepted as one.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct SessionId(String);

impl SessionId {
    pub fn generate() -> Result<Self, getrandom::Error> {
        let mut bytes = [0_u8; 16];
        getrandom::fill(&mut bytes)?;
        Ok(Self(hex(&bytes)))
    }

    pub fn parse(text: &str) -> Option<Self> {
        unhex::<16>(text).map(|_| Self(text.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for SessionId {
    type Error = String;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        Self::parse(&text).ok_or_else(|| "a session id is 32 lowercase hex digits".to_owned())
    }
}

impl From<SessionId> for String {
    fn from(id: SessionId) -> Self {
        id.0
    }
}

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
    fn session_ids_are_fresh_and_only_ever_hex() {
        let a = SessionId::generate().unwrap();
        let b = SessionId::generate().unwrap();
        assert_ne!(a, b);
        assert_eq!(a.as_str().len(), 32);
        assert_eq!(SessionId::parse(a.as_str()), Some(a));
        for hostile in [
            "../../../../etc/passwd",
            "..",
            "",
            "0123456789abcdef0123456789abcde/",
            "0123456789ABCDEF0123456789ABCDEF",
            "0123456789abcdef0123456789abcdef0",
            "0123456789abcdef\\123456789abcdef",
        ] {
            assert!(
                SessionId::parse(hostile).is_none(),
                "{hostile:?} was accepted"
            );
        }
    }

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
