//! The one identifier every visit has, written one way only, so a path built from it can never
//! leave the directory it names.

use crate::document::{hex, unhex};
use serde::{Deserialize, Serialize};
use std::fmt;

/// One visit: 128 random bits, written as 32 lowercase hex digits. It is also the name of the
/// visit's session directory, which is safe only because nothing else is ever accepted as one.
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
}
