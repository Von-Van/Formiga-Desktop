//! Reading and writing travel documents: bounded before parsing, version-checked before shaping,
//! validated both ways, and written whole or not at all.

use crate::TRAVEL_FORMAT_VERSION;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::Path;

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

/// The three fields every document starts with, checked on the raw JSON before it is shaped, so a
/// document from a newer writer is refused for its version rather than for whatever shape it
/// happens to have.
fn check_header(value: &Value, expected: &'static str) -> Result<(), TravelError> {
    let format = value
        .get("format")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if format != expected {
        return Err(TravelError::WrongFormat {
            expected,
            found: format.chars().take(64).collect(),
        });
    }
    let version = value
        .get("version")
        .and_then(Value::as_u64)
        .filter(|version| *version >= 1)
        .ok_or_else(|| TravelError::invalid("the file names no version"))?;
    let needs = value
        .get("min_reader_version")
        .map(|needs| {
            needs
                .as_u64()
                .ok_or_else(|| TravelError::invalid("min_reader_version is not a number"))
        })
        .transpose()?
        .unwrap_or(version);
    if needs > version {
        return Err(TravelError::invalid(
            "min_reader_version is newer than the version it was written as",
        ));
    }
    if needs > u64::from(TRAVEL_FORMAT_VERSION) {
        return Err(TravelError::UnsupportedVersion {
            needs: u32::try_from(needs).unwrap_or(u32::MAX),
            reads: TRAVEL_FORMAT_VERSION,
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
pub fn decode<T: Document>(bytes: &[u8]) -> Result<T, TravelError> {
    if bytes.len() as u64 > T::MAX_BYTES {
        return Err(TravelError::TooLarge {
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
pub fn encode<T: Document>(document: &T) -> Result<Vec<u8>, TravelError> {
    document.validate()?;
    let mut bytes = serde_json::to_vec_pretty(document)?;
    bytes.push(b'\n');
    if bytes.len() as u64 > T::MAX_BYTES {
        return Err(TravelError::TooLarge {
            limit: T::MAX_BYTES,
        });
    }
    Ok(bytes)
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
    let mut bytes = Vec::new();
    File::open(path)?.take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(TravelError::TooLarge { limit });
    }
    Ok(bytes)
}

/// Write a file whole: a reader at any moment sees the old file, the new one, or none, never part
/// of one.
pub fn write_atomically(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let file_name = path
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "no file name"))?;
    let mut temporary_name = file_name.to_owned();
    temporary_name.push(".tmp");
    let temporary = path.with_file_name(temporary_name);
    {
        let mut file = File::create(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }
    if let Err(error) = fs::rename(&temporary, path) {
        let _ = fs::remove_file(&temporary);
        return Err(error);
    }
    // The rename itself is only durable once the directory is. Windows has no directory handle to
    // sync, and its rename is already written through.
    #[cfg(unix)]
    if let Some(parent) = path.parent() {
        File::open(parent)?.sync_all()?;
    }
    Ok(())
}

/// The SHA-256 of some bytes, in lowercase hex: how a receipt names the snapshot it answers.
pub fn sha256_hex(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(DIGITS[usize::from(byte >> 4)] as char);
        out.push(DIGITS[usize::from(byte & 0x0f)] as char);
    }
    out
}

/// Lowercase hex of exactly `N` bytes.
pub(crate) fn unhex<const N: usize>(text: &str) -> Option<[u8; N]> {
    if text.len() != N * 2 {
        return None;
    }
    let digit = |byte: u8| match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    };
    let mut out = [0_u8; N];
    for (index, pair) in text.as_bytes().chunks_exact(2).enumerate() {
        out[index] = (digit(pair[0])? << 4) | digit(pair[1])?;
    }
    Some(out)
}

pub(crate) fn is_sha256_hex(text: &str) -> bool {
    unhex::<32>(text).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_round_trips_and_refuses_anything_else() {
        let bytes = [0x00, 0x7f, 0x80, 0xff];
        assert_eq!(hex(&bytes), "007f80ff");
        assert_eq!(unhex::<4>("007f80ff"), Some(bytes));
        assert_eq!(
            unhex::<4>("007F80FF"),
            None,
            "uppercase is not the written form"
        );
        assert_eq!(unhex::<4>("007f80f"), None);
        assert_eq!(unhex::<4>("007f80fg"), None);
    }

    #[test]
    fn a_header_from_a_newer_reader_is_refused_for_its_version() {
        let newer = TRAVEL_FORMAT_VERSION + 1;
        let value = serde_json::json!({
            "format": "formiga.travel.snapshot",
            "version": newer + 4,
            "min_reader_version": newer,
            "anything": ["shaped", "differently"],
        });
        match check_header(&value, "formiga.travel.snapshot") {
            Err(TravelError::UnsupportedVersion { needs, reads }) => {
                assert_eq!((needs, reads), (newer, TRAVEL_FORMAT_VERSION))
            }
            other => panic!("expected a version refusal, got {other:?}"),
        }
    }

    #[test]
    fn a_newer_writer_that_older_readers_may_use_is_accepted() {
        let value = serde_json::json!({
            "format": "formiga.travel.snapshot",
            "version": 4,
            "min_reader_version": 1,
        });
        assert!(check_header(&value, "formiga.travel.snapshot").is_ok());
    }

    #[test]
    fn headers_that_cannot_be_trusted_are_refused() {
        for value in [
            serde_json::json!({ "format": "formiga.travel.receipt", "version": 1 }),
            serde_json::json!({ "version": 1 }),
            serde_json::json!({ "format": "formiga.travel.snapshot" }),
            serde_json::json!({ "format": "formiga.travel.snapshot", "version": 0 }),
            serde_json::json!({ "format": "formiga.travel.snapshot", "version": 1, "min_reader_version": 2 }),
            serde_json::json!({ "format": "formiga.travel.snapshot", "version": 1, "min_reader_version": "1" }),
            serde_json::json!(["formiga.travel.snapshot", 1]),
        ] {
            assert!(
                check_header(&value, "formiga.travel.snapshot").is_err(),
                "{value} was accepted"
            );
        }
    }

    #[test]
    fn a_file_larger_than_its_limit_is_refused_without_being_read_whole() {
        let directory =
            std::env::temp_dir().join(format!("formiga-travel-bounded-{}", std::process::id()));
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join("big.json");
        fs::write(&path, vec![b' '; 2048]).unwrap();
        assert!(matches!(
            read_bounded(&path, 1024),
            Err(TravelError::TooLarge { limit: 1024 })
        ));
        assert_eq!(read_bounded(&path, 2048).unwrap().len(), 2048);
        fs::remove_dir_all(&directory).unwrap();
    }

    #[test]
    fn an_atomic_write_leaves_no_temporary_file_behind() {
        let directory =
            std::env::temp_dir().join(format!("formiga-travel-atomic-{}", std::process::id()));
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join("receipt.json");
        write_atomically(&path, b"first").unwrap();
        write_atomically(&path, b"second").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"second");
        let names: Vec<_> = fs::read_dir(&directory)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(names, vec![std::ffi::OsString::from("receipt.json")]);
        fs::remove_dir_all(&directory).unwrap();
    }
}
