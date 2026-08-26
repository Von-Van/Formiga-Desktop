use crate::SaveFile;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum PersistenceError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("invalid save file: {0}")]
    Json(#[from] serde_json::Error),
    #[error("unsupported save version {0}")]
    UnsupportedVersion(u32),
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

    pub fn load(&self) -> Result<Option<SaveFile>, PersistenceError> {
        match self.load_path(&self.path) {
            Ok(save) => Ok(Some(save)),
            Err(PersistenceError::Io(error)) if error.kind() == io::ErrorKind::NotFound => Ok(None),
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
            let _ = fs::copy(&self.path, &backup);
        }
        atomic_replace(&temporary, &self.path)?;
        Ok(())
    }

    fn load_path(&self, path: &Path) -> Result<SaveFile, PersistenceError> {
        let bytes = fs::read(path)?;
        let value: serde_json::Value = serde_json::from_slice(&bytes)?;
        let version = value
            .get("save_version")
            .and_then(serde_json::Value::as_u64)
            .and_then(|value| u32::try_from(value).ok())
            .unwrap_or_default();
        match version {
            crate::SAVE_VERSION => Ok(serde_json::from_value(value)?),
            1 => migrate_v1(value),
            unsupported => Err(PersistenceError::UnsupportedVersion(unsupported)),
        }
    }

    fn temporary_path(&self) -> PathBuf {
        self.path.with_extension("json.tmp")
    }

    fn backup_path(&self) -> PathBuf {
        self.path.with_extension("json.bak")
    }
}

fn migrate_v1(mut value: serde_json::Value) -> Result<SaveFile, PersistenceError> {
    let primary_only = value
        .pointer("/settings/primary_display_only")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    value["save_version"] = serde_json::Value::from(crate::SAVE_VERSION);
    let mut save: SaveFile = serde_json::from_value(value)?;
    save.save_version = crate::SAVE_VERSION;
    if primary_only {
        save.settings.habitat.preset = crate::HabitatPreset::PrimaryDisplay;
    }
    Ok(save)
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
mod tests {
    use super::*;
    use crate::{ArrivalState, Settings};
    use time::macros::datetime;

    fn example_save() -> SaveFile {
        SaveFile {
            save_version: crate::SAVE_VERSION,
            colony_seed: [1; 32],
            created_at_utc: datetime!(2026-01-01 0:00 UTC),
            maximum_seen_utc: datetime!(2026-01-01 0:00 UTC),
            arrival_state: ArrivalState::default(),
            settings: Settings::default(),
            creatures: Vec::new(),
        }
    }

    #[test]
    fn round_trips_atomically() {
        let directory = std::env::temp_dir().join(format!("formiga-save-{}", std::process::id()));
        let path = directory.join("colony.json");
        let store = SaveStore::new(&path);
        store.save(&example_save()).unwrap();
        assert_eq!(store.load().unwrap(), Some(example_save()));
        let mut replacement = example_save();
        replacement.settings.reduce_motion = true;
        store.save(&replacement).unwrap();
        assert_eq!(store.load().unwrap(), Some(replacement));
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn migrates_v1_primary_display_setting() {
        let mut value = serde_json::to_value(example_save()).unwrap();
        value["save_version"] = serde_json::Value::from(1);
        let settings = value["settings"].as_object_mut().unwrap();
        settings.remove("direct_manipulation");
        settings.remove("habitat");
        settings.remove("application_occlusion_rules");
        settings.insert("primary_display_only".into(), serde_json::Value::Bool(true));
        let directory =
            std::env::temp_dir().join(format!("formiga-v1-save-{}", std::process::id()));
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join("colony.json");
        fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        let migrated = SaveStore::new(&path).load().unwrap().unwrap();
        assert_eq!(migrated.save_version, crate::SAVE_VERSION);
        assert_eq!(
            migrated.settings.habitat.preset,
            crate::HabitatPreset::PrimaryDisplay
        );
        assert!(migrated.settings.direct_manipulation);
        let _ = fs::remove_dir_all(directory);
    }
}
