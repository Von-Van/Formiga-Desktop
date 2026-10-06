//! Reading and writing Home documents: bounded before parsing, version-checked before shaping,
//! validated both ways, and written whole or not at all. The same discipline as a trip's
//! documents, under Home's own version.

use crate::HOME_FORMAT_VERSION;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

pub use formiga_travel::sha256_hex;

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

/// The three fields every document starts with, checked on the raw JSON before it is shaped, so a
/// document from a newer writer is refused for its version rather than for whatever shape it
/// happens to have.
fn check_header(value: &Value, expected: &'static str) -> Result<(), HomeError> {
    let format = value
        .get("format")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if format != expected {
        return Err(HomeError::WrongFormat {
            expected,
            found: format.chars().take(64).collect(),
        });
    }
    let version = value
        .get("version")
        .and_then(Value::as_u64)
        .filter(|version| *version >= 1)
        .ok_or_else(|| HomeError::invalid("the file names no version"))?;
    let needs = value
        .get("min_reader_version")
        .map(|needs| {
            needs
                .as_u64()
                .ok_or_else(|| HomeError::invalid("min_reader_version is not a number"))
        })
        .transpose()?
        .unwrap_or(version);
    if needs > version {
        return Err(HomeError::invalid(
            "min_reader_version is newer than the version it was written as",
        ));
    }
    if needs > u64::from(HOME_FORMAT_VERSION) {
        return Err(HomeError::UnsupportedVersion {
            needs: u32::try_from(needs).unwrap_or(u32::MAX),
            reads: HOME_FORMAT_VERSION,
        });
    }
    Ok(())
}

/// The three header fields every document is written with: what it is, the version this build
/// writes, and the oldest reader that may use it.
pub(crate) fn header_ok(format: &str, version: u32, min_reader: u32, expected: &str) -> bool {
    format == expected && version >= 1 && (1..=version).contains(&min_reader)
}

/// Read a document from its bytes.
pub fn decode<T: HomeDocument>(bytes: &[u8]) -> Result<T, HomeError> {
    if bytes.len() as u64 > T::MAX_BYTES {
        return Err(HomeError::TooLarge {
            limit: T::MAX_BYTES,
        });
    }
    let value: Value = serde_json::from_slice(bytes)?;
    check_header(&value, T::FORMAT)?;
    let document: T = serde_json::from_value(value)?;
    document.validate()?;
    Ok(document)
}

/// A document's bytes, once it has been validated. The same document always gives the same bytes.
pub fn encode<T: HomeDocument>(document: &T) -> Result<Vec<u8>, HomeError> {
    document.validate()?;
    let mut bytes = serde_json::to_vec_pretty(document)?;
    bytes.push(b'\n');
    if bytes.len() as u64 > T::MAX_BYTES {
        return Err(HomeError::TooLarge {
            limit: T::MAX_BYTES,
        });
    }
    Ok(bytes)
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
    formiga_travel::write_atomically(path, &bytes)?;
    Ok(bytes)
}

/// At most `limit` bytes of a file, or [`HomeError::TooLarge`] if there are more.
pub fn read_bounded(path: &Path, limit: u64) -> Result<Vec<u8>, HomeError> {
    let mut bytes = Vec::new();
    File::open(path)?.take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(HomeError::TooLarge { limit });
    }
    Ok(bytes)
}

/// Whether `text` is 64 lowercase hex digits: how a seal names the bytes it answers.
pub(crate) fn is_sha256_hex(text: &str) -> bool {
    is_lower_hex(text, 64)
}

pub(crate) fn is_lower_hex(text: &str, digits: usize) -> bool {
    text.len() == digits
        && text
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_header_from_a_newer_reader_is_refused_for_its_version() {
        let newer = HOME_FORMAT_VERSION + 1;
        let value = serde_json::json!({
            "format": "formiga.home.snapshot",
            "version": newer + 4,
            "min_reader_version": newer,
            "anything": ["shaped", "differently"],
        });
        match check_header(&value, "formiga.home.snapshot") {
            Err(HomeError::UnsupportedVersion { needs, reads }) => {
                assert_eq!((needs, reads), (newer, HOME_FORMAT_VERSION))
            }
            other => panic!("expected a version refusal, got {other:?}"),
        }
    }

    #[test]
    fn a_newer_writer_that_older_readers_may_use_is_accepted() {
        let value = serde_json::json!({
            "format": "formiga.home.state",
            "version": 4,
            "min_reader_version": 1,
        });
        assert!(check_header(&value, "formiga.home.state").is_ok());
    }

    #[test]
    fn headers_that_cannot_be_trusted_are_refused() {
        for value in [
            serde_json::json!({ "format": "formiga.home.receipt", "version": 1 }),
            serde_json::json!({ "version": 1 }),
            serde_json::json!({ "format": "formiga.home.state" }),
            serde_json::json!({ "format": "formiga.home.state", "version": 0 }),
            serde_json::json!({ "format": "formiga.home.state", "version": 1, "min_reader_version": 2 }),
            serde_json::json!({ "format": "formiga.home.state", "version": 1, "min_reader_version": "1" }),
            serde_json::json!({ "format": "formiga.travel.snapshot", "version": 1 }),
            serde_json::json!(["formiga.home.state", 1]),
        ] {
            assert!(
                check_header(&value, "formiga.home.state").is_err(),
                "{value} was accepted"
            );
        }
    }

    #[test]
    fn a_file_larger_than_its_limit_is_refused_without_being_read_whole() {
        let directory =
            std::env::temp_dir().join(format!("formiga-home-bounded-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("big.json");
        std::fs::write(&path, vec![b' '; 2048]).unwrap();
        assert!(matches!(
            read_bounded(&path, 1024),
            Err(HomeError::TooLarge { limit: 1024 })
        ));
        assert_eq!(read_bounded(&path, 2048).unwrap().len(), 2048);
        std::fs::remove_dir_all(&directory).unwrap();
    }
}
