//! Reading and writing the colony file.
//!
//! A file on disk becomes a running colony in four steps, each with one job and each in one
//! place:
//!
//! 1. **Raw JSON.** At most [`MAX_SAVE_BYTES`] are read and parsed as plain JSON. Nothing is
//!    assumed about the file yet except the `save_version` it names.
//! 2. **One migration per version** ([`migrations`]). A file from an older version is brought
//!    forward a version at a time, and each step knows only the version it reads and the one it
//!    leaves behind. Adding a version is adding a step, never editing an earlier one.
//! 3. **The persisted shape.** Only a file in the current version's shape is parsed into
//!    [`SaveFile`], which is also exactly what is written back out.
//! 4. **A validated colony** ([`validation`]). Every limit, reference and schedule the running
//!    colony relies on is enforced in one place, and the result is a [`ValidatedSave`]: the only
//!    thing a [`World`](crate::World) is ever built from.
//!
//! Writing goes the other way and needs none of this. The current shape is written as it stands
//! to a temporary file, which is synced to disk and then moved over the old one once the old one
//! has been kept as the backup.

use crate::SaveFile;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

mod damage;
mod migrations;
mod validation;

pub use damage::damage;
pub use validation::{ImportRefusal, ValidatedSave, violations};

/// The largest colony file read. A full colony of six with a full journal is well under half of
/// this; anything larger is not a colony file.
pub const MAX_SAVE_BYTES: u64 = 2 * 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum PersistenceError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("invalid save file: {0}")]
    Json(#[from] serde_json::Error),
    #[error("unsupported save version {0}")]
    UnsupportedVersion(u32),
    #[error("the file is larger than any colony file can be ({0} bytes or more)")]
    TooLarge(u64),
    #[error("not a whole colony: {0}")]
    NotAColony(#[from] ImportRefusal),
}

/// How soon something needs the colony written to disk.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum SaveUrgency {
    /// Nothing is waiting; the periodic save still runs.
    #[default]
    None,
    /// Everyday movement, written with the next routine checkpoint.
    Routine,
    /// A change worth keeping at once.
    Prompt,
}

/// Routine changes are written together at most this often. It is also the most everyday
/// progress — who was doing what, and where — that a crash can cost.
pub const ROUTINE_CHECKPOINT: Duration = Duration::from_secs(15);

/// A colony with nothing waiting is still written this often, since drives, positions, and learned
/// tendencies change without raising any event of their own.
pub const PERIODIC_SAVE: Duration = Duration::from_secs(30);

/// Whether the colony should be written now, given the most urgent thing waiting since the last
/// write and how long ago that write was. A busy colony starts dozens of actions a minute; writing
/// the whole file for each one used to rewrite it more than once a second.
pub fn save_due(waiting: SaveUrgency, since_last_save: Duration) -> bool {
    match waiting {
        SaveUrgency::Prompt => true,
        SaveUrgency::Routine => since_last_save >= ROUTINE_CHECKPOINT,
        SaveUrgency::None => since_last_save >= PERIODIC_SAVE,
    }
}

/// Read a colony from the bytes of a file: parse them as JSON, bring them forward to the current
/// version, parse that into the current shape, and validate the result.
pub fn decode(bytes: &[u8]) -> Result<ValidatedSave, PersistenceError> {
    if bytes.len() as u64 > MAX_SAVE_BYTES {
        return Err(PersistenceError::TooLarge(MAX_SAVE_BYTES));
    }
    let raw = parse_raw(bytes)?;
    let persisted = migrations::upgrade(raw)?;
    Ok(ValidatedSave::from(persisted))
}

/// The first step: plain JSON, holding no number too large for the fields a colony keeps. Every
/// fraction a colony keeps is an `f32`, and a number past its range would read as infinite. No
/// build writes one — an infinite `f32` is written as `null` — so a file holding one has been
/// damaged. (`f32::MAX` itself, which a held action uses, reads back as itself.)
fn parse_raw(bytes: &[u8]) -> Result<serde_json::Value, PersistenceError> {
    fn in_range(value: &serde_json::Value) -> bool {
        match value {
            serde_json::Value::Number(number) => number
                .as_f64()
                .is_some_and(|number| (number as f32).is_finite()),
            serde_json::Value::Array(items) => items.iter().all(in_range),
            serde_json::Value::Object(map) => map.values().all(in_range),
            _ => true,
        }
    }
    let raw: serde_json::Value = serde_json::from_slice(bytes)?;
    if !in_range(&raw) {
        return Err(PersistenceError::Json(serde::de::Error::custom(
            "a number in the file is out of range",
        )));
    }
    Ok(raw)
}

pub struct SaveStore {
    path: PathBuf,
}

impl SaveStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load(&self) -> Result<Option<ValidatedSave>, PersistenceError> {
        match self.load_path(&self.path) {
            Ok(save) => Ok(Some(save)),
            Err(PersistenceError::Io(error)) if error.kind() == io::ErrorKind::NotFound => {
                match self.load_path(&self.backup_path()) {
                    Ok(save) => Ok(Some(save)),
                    Err(PersistenceError::Io(error)) if error.kind() == io::ErrorKind::NotFound => {
                        Ok(None)
                    }
                    Err(error) => Err(error),
                }
            }
            Err(primary_error) => {
                let backup = self.backup_path();
                match self.load_path(&backup) {
                    Ok(save) => Ok(Some(save)),
                    Err(_) => Err(primary_error),
                }
            }
        }
    }

    pub fn save(&self, save: &SaveFile) -> Result<(), PersistenceError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let bytes = serde_json::to_vec_pretty(save)?;
        let temporary = self.temporary_path();
        let backup = self.backup_path();
        {
            let mut file = File::create(&temporary)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
        }
        if self.path.exists() {
            if self.load_path(&self.path).is_ok() {
                fs::copy(&self.path, &backup)?;
            } else {
                self.preserve_recovery_files()?;
            }
        }
        atomic_replace(&temporary, &self.path)?;
        Ok(())
    }

    /// A user-selected full-colony snapshot, bounded before parsing and never changed on import.
    /// Unlike the colony's own file, which is repaired wherever it can be, a snapshot that is not
    /// a whole colony is refused with the reason.
    pub fn read_snapshot(path: &Path) -> Result<ValidatedSave, PersistenceError> {
        let raw = parse_raw(&read_bounded(path)?)?;
        let persisted = migrations::upgrade(raw)?;
        Ok(ValidatedSave::import(persisted)?)
    }

    /// Preserve both files with unique names before an explicit reset/restore or damaged-save write.
    pub fn preserve_recovery_files(&self) -> Result<Vec<PathBuf>, PersistenceError> {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let mut paths = Vec::new();
        for (index, source) in [self.path.clone(), self.backup_path()].iter().enumerate() {
            match File::open(source) {
                Ok(mut input) => {
                    let target = self
                        .path
                        .with_extension(format!("recovery-{stamp}-{index}.json"));
                    let mut output = fs::OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .open(&target)?;
                    io::copy(&mut input, &mut output)?;
                    output.sync_all()?;
                    paths.push(target);
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
        Ok(paths)
    }

    fn load_path(&self, path: &Path) -> Result<ValidatedSave, PersistenceError> {
        decode(&read_bounded(path)?)
    }

    fn temporary_path(&self) -> PathBuf {
        self.path.with_extension("json.tmp")
    }

    fn backup_path(&self) -> PathBuf {
        self.path.with_extension("json.bak")
    }
}

/// The bytes of a file, refusing one larger than any colony file without reading the rest of it.
fn read_bounded(path: &Path) -> Result<Vec<u8>, PersistenceError> {
    use std::io::Read;
    let mut bytes = Vec::new();
    File::open(path)?
        .take(MAX_SAVE_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_SAVE_BYTES {
        return Err(PersistenceError::TooLarge(MAX_SAVE_BYTES));
    }
    Ok(bytes)
}

#[cfg(not(target_os = "windows"))]
fn atomic_replace(source: &Path, destination: &Path) -> io::Result<()> {
    fs::rename(source, destination)
}

#[cfg(target_os = "windows")]
fn atomic_replace(source: &Path, destination: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
    };

    let source: Vec<u16> = source.as_os_str().encode_wide().chain(Some(0)).collect();
    let destination: Vec<u16> = destination
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    let result = unsafe {
        MoveFileExW(
            source.as_ptr(),
            destination.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if result == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests;
