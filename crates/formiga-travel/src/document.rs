//! Reading and writing travel documents: bounded before parsing, version-checked before shaping,
//! validated both ways, and written whole or not at all. How is the rulebook's, shared with every
//! companion app; what each document must keep, and the words for what went wrong, are travel's.

use crate::TRAVEL_FORMAT_VERSION;
use formiga_expansion_rulebook::{self as rulebook, Kind, RulebookError};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::io;
use std::path::Path;

pub(crate) use formiga_expansion_rulebook::{header_ok, hex, is_sha256_hex, unhex};
pub use formiga_expansion_rulebook::{sha256_hex, write_atomically};

#[derive(Debug, thiserror::Error)]
pub enum TravelError {
    #[error("the travel file could not be read or written: {0}")]
    Io(#[from] io::Error),
    #[error("the travel file is larger than any travel file can be ({limit} bytes)")]
    TooLarge { limit: u64 },
    #[error("the travel file could not be read: {0}")]
    Json(#[from] serde_json::Error),
    #[error("expected a {expected} file, found {found:?}")]
    WrongFormat {
        expected: &'static str,
        found: String,
    },
    #[error(
        "the travel file needs a reader of travel version {needs}, and this build reads up to \
         version {reads}"
    )]
    UnsupportedVersion { needs: u32, reads: u32 },
    #[error("the travel file is not usable: {0}")]
    Invalid(String),
}

impl TravelError {
    pub(crate) fn invalid(reason: impl Into<String>) -> Self {
        Self::Invalid(reason.into())
    }
}

impl From<RulebookError> for TravelError {
    fn from(error: RulebookError) -> Self {
        match error {
            RulebookError::Io(error) => Self::Io(error),
            RulebookError::TooLarge { limit } => Self::TooLarge { limit },
            RulebookError::Json(error) => Self::Json(error),
            RulebookError::WrongFormat { expected, found } => Self::WrongFormat { expected, found },
            RulebookError::UnsupportedVersion { needs, reads } => {
                Self::UnsupportedVersion { needs, reads }
            }
            RulebookError::Invalid(reason) => Self::Invalid(reason),
        }
    }
}

/// One kind of travel document.
pub trait Document: Serialize + DeserializeOwned {
    /// Its `format` field.
    const FORMAT: &'static str;
    /// The largest it may be, checked before it is parsed and before it is written.
    const MAX_BYTES: u64;
    /// Every bound and reference it must keep, checked both when it is written and when it is
    /// read.
    fn validate(&self) -> Result<(), TravelError>;
}

/// How the rulebook holds a document of kind `T`: under its own format and bounds, read as
/// travel version [`TRAVEL_FORMAT_VERSION`].
fn kind<T: Document>() -> Kind {
    Kind {
        format: T::FORMAT,
        max_bytes: T::MAX_BYTES,
        reads: TRAVEL_FORMAT_VERSION,
    }
}

/// Read a document from its bytes.
pub fn decode<T: Document>(bytes: &[u8]) -> Result<T, TravelError> {
    let document: T = rulebook::decode(bytes, kind::<T>())?;
    document.validate()?;
    Ok(document)
}

/// A document's bytes, once it has been validated. The same document always gives the same bytes.
pub fn encode<T: Document>(document: &T) -> Result<Vec<u8>, TravelError> {
    document.validate()?;
    Ok(rulebook::encode(document, kind::<T>())?)
}

/// Read a document from a file, refusing one larger than the document can be without reading the
/// rest of it.
pub fn read_document<T: Document>(path: &Path) -> Result<T, TravelError> {
    decode(&read_bounded(path, T::MAX_BYTES)?)
}

/// Validate a document and write it whole: to a temporary file beside it, synced, then renamed
/// into place. Returns the bytes written.
pub fn write_document<T: Document>(path: &Path, document: &T) -> Result<Vec<u8>, TravelError> {
    let bytes = encode(document)?;
    write_atomically(path, &bytes)?;
    Ok(bytes)
}

/// At most `limit` bytes of a file, or [`TravelError::TooLarge`] if there are more.
pub fn read_bounded(path: &Path, limit: u64) -> Result<Vec<u8>, TravelError> {
    Ok(rulebook::read_bounded(path, limit)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TravelSnapshot;

    #[test]
    fn a_snapshot_from_a_newer_reader_is_refused_in_travel_words() {
        let newer = TRAVEL_FORMAT_VERSION + 1;
        let bytes = serde_json::to_vec(&serde_json::json!({
            "format": crate::SNAPSHOT_FORMAT,
            "version": newer + 4,
            "min_reader_version": newer,
            "anything": ["shaped", "differently"],
        }))
        .unwrap();
        let error = decode::<TravelSnapshot>(&bytes).unwrap_err();
        assert!(
            matches!(
                error,
                TravelError::UnsupportedVersion { needs, reads }
                    if (needs, reads) == (newer, TRAVEL_FORMAT_VERSION)
            ),
            "{error:?}"
        );
        assert!(error.to_string().contains("travel version"), "{error}");
    }

    #[test]
    fn a_file_larger_than_its_limit_is_refused_in_travel_words() {
        let directory =
            std::env::temp_dir().join(format!("formiga-travel-bounded-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("big.json");
        std::fs::write(&path, vec![b' '; 2048]).unwrap();
        assert!(matches!(
            read_bounded(&path, 1024),
            Err(TravelError::TooLarge { limit: 1024 })
        ));
        std::fs::remove_dir_all(&directory).unwrap();
    }
}
