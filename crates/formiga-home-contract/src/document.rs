//! Reading and writing Home documents: bounded before parsing, version-checked before shaping,
//! validated both ways, and written whole or not at all. How is the rulebook's, the same for a
//! trip's documents and every companion app's; what each document must keep, and the words for
//! what went wrong, are Home's, under Home's own version.

use crate::HOME_FORMAT_VERSION;
use formiga_expansion_rulebook::{self as rulebook, Kind, RulebookError};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::io;
use std::path::Path;

pub use formiga_expansion_rulebook::sha256_hex;
pub(crate) use formiga_expansion_rulebook::{header_ok, is_lower_hex, is_sha256_hex};

#[derive(Debug, thiserror::Error)]
pub enum HomeError {
    #[error("the Home file could not be read or written: {0}")]
    Io(#[from] io::Error),
    #[error("the Home file is larger than any Home file can be ({limit} bytes)")]
    TooLarge { limit: u64 },
    #[error("the Home file could not be read: {0}")]
    Json(#[from] serde_json::Error),
    #[error("expected a {expected} file, found {found:?}")]
    WrongFormat {
        expected: &'static str,
        found: String,
    },
    #[error(
        "the Home file needs a reader of Home version {needs}, and this build reads up to \
         version {reads}"
    )]
    UnsupportedVersion { needs: u32, reads: u32 },
    #[error("the Home file is not usable: {0}")]
    Invalid(String),
}

impl HomeError {
    pub(crate) fn invalid(reason: impl Into<String>) -> Self {
        Self::Invalid(reason.into())
    }

    /// Whether the file was simply not there.
    pub fn is_missing(&self) -> bool {
        matches!(self, Self::Io(error) if error.kind() == io::ErrorKind::NotFound)
    }
}

impl From<RulebookError> for HomeError {
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

/// One kind of Home document.
pub trait HomeDocument: Serialize + DeserializeOwned {
    /// Its `format` field.
    const FORMAT: &'static str;
    /// The largest it may be, checked before it is parsed and before it is written.
    const MAX_BYTES: u64;
    /// Every bound and reference it must keep, checked both when it is written and when it is
    /// read.
    fn validate(&self) -> Result<(), HomeError>;
}

/// How the rulebook holds a document of kind `T`: under its own format and bounds, read as Home
/// version [`HOME_FORMAT_VERSION`].
fn kind<T: HomeDocument>() -> Kind {
    Kind {
        format: T::FORMAT,
        max_bytes: T::MAX_BYTES,
        reads: HOME_FORMAT_VERSION,
    }
}

/// Read a document from its bytes.
pub fn decode<T: HomeDocument>(bytes: &[u8]) -> Result<T, HomeError> {
    let document: T = rulebook::decode(bytes, kind::<T>())?;
    document.validate()?;
    Ok(document)
}

/// A document's bytes, once it has been validated. The same document always gives the same bytes.
pub fn encode<T: HomeDocument>(document: &T) -> Result<Vec<u8>, HomeError> {
    document.validate()?;
    Ok(rulebook::encode(document, kind::<T>())?)
}

/// Read a document from a file, refusing one larger than the document can be without reading the
/// rest of it. Returns the document and the bytes it was read from, which is what a seal is made
/// of.
pub fn read_document<T: HomeDocument>(path: &Path) -> Result<(T, Vec<u8>), HomeError> {
    let bytes = read_bounded(path, T::MAX_BYTES)?;
    Ok((decode(&bytes)?, bytes))
}

/// Validate a document and write it whole: to a temporary file beside it, synced, then renamed
/// into place. Returns the bytes written.
pub fn write_document<T: HomeDocument>(path: &Path, document: &T) -> Result<Vec<u8>, HomeError> {
    let bytes = encode(document)?;
    rulebook::write_atomically(path, &bytes)?;
    Ok(bytes)
}

/// At most `limit` bytes of a file, or [`HomeError::TooLarge`] if there are more.
pub fn read_bounded(path: &Path, limit: u64) -> Result<Vec<u8>, HomeError> {
    Ok(rulebook::read_bounded(path, limit)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::HomeState;

    #[test]
    fn a_state_from_a_newer_reader_is_refused_in_home_words() {
        let newer = HOME_FORMAT_VERSION + 1;
        let bytes = serde_json::to_vec(&serde_json::json!({
            "format": crate::STATE_FORMAT,
            "version": newer + 4,
            "min_reader_version": newer,
            "anything": ["shaped", "differently"],
        }))
        .unwrap();
        let error = decode::<HomeState>(&bytes).unwrap_err();
        assert!(
            matches!(
                error,
                HomeError::UnsupportedVersion { needs, reads }
                    if (needs, reads) == (newer, HOME_FORMAT_VERSION)
            ),
            "{error:?}"
        );
        assert!(error.to_string().contains("Home version"), "{error}");
    }

    #[test]
    fn a_trip_document_is_not_a_home_document() {
        let bytes = serde_json::to_vec(&serde_json::json!({
            "format": "formiga.travel.snapshot",
            "version": 1,
        }))
        .unwrap();
        assert!(matches!(
            decode::<HomeState>(&bytes),
            Err(HomeError::WrongFormat { .. })
        ));
    }

    #[test]
    fn a_file_larger_than_its_limit_is_refused_and_a_missing_one_says_so() {
        let directory =
            std::env::temp_dir().join(format!("formiga-home-bounded-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("big.json");
        std::fs::write(&path, vec![b' '; 2048]).unwrap();
        assert!(matches!(
            read_bounded(&path, 1024),
            Err(HomeError::TooLarge { limit: 1024 })
        ));
        assert!(
            read_bounded(&directory.join("gone.json"), 1024)
                .unwrap_err()
                .is_missing()
        );
        std::fs::remove_dir_all(&directory).unwrap();
    }
}
