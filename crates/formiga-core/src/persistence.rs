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
            1..=3 => migrate_legacy(value, version),
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

fn migrate_legacy(
    mut value: serde_json::Value,
    source_version: u32,
) -> Result<SaveFile, PersistenceError> {
    let primary_only = value
        .pointer("/settings/primary_display_only")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    if source_version <= 2
        && let Some(creatures) = value
            .get_mut("creatures")
            .and_then(serde_json::Value::as_array_mut)
    {
        for creature in creatures {
            let Some(appearance) = creature
                .get_mut("appearance")
                .and_then(serde_json::Value::as_object_mut)
            else {
                continue;
            };
            migrate_appearance(appearance);
        }
    }
    value["save_version"] = serde_json::Value::from(crate::SAVE_VERSION);
    let mut save: SaveFile = serde_json::from_value(value)?;
    save.save_version = crate::SAVE_VERSION;
    if source_version == 1 && primary_only {
        save.settings.habitat.preset = crate::HabitatPreset::PrimaryDisplay;
    }
    save.home =
        crate::ColonyHome::from_seed(save.colony_seed, None, None, Some(save.maximum_seen_utc));
    Ok(save)
}

fn migrate_appearance(appearance: &mut serde_json::Map<String, serde_json::Value>) {
    use serde_json::{Value, json};

    let appendage_style = appearance
        .remove("appendage_style")
        .unwrap_or_else(|| Value::String("None".into()));
    let appendage_size = appearance
        .remove("appendage_size")
        .unwrap_or_else(|| Value::from(3));
    appearance.insert(
        "head_appendages".into(),
        json!({
            "style": appendage_style,
            "size": appendage_size,
        }),
    );

    let eye_size = appearance
        .remove("eye_size")
        .and_then(|value| value.as_u64())
        .unwrap_or(1)
        .clamp(1, 2);
    let eye_spacing = appearance
        .remove("eye_spacing")
        .and_then(|value| value.as_u64())
        .unwrap_or(5)
        .clamp(3, 7);
    let vertical_offset = appearance
        .remove("eye_height")
        .and_then(|value| value.as_i64())
        .unwrap_or(0)
        .clamp(-2, 2);
    let signature = appearance
        .get("face_signature")
        .and_then(Value::as_u64)
        .unwrap_or_default();
    let family = appearance
        .get("family")
        .and_then(Value::as_str)
        .unwrap_or("Blob")
        .to_owned();

    appearance.insert(
        "face".into(),
        json!({
            "eye_shape": select_gene(signature, 0, &["Round", "Tall", "SoftSquare"]),
            "eye_size": eye_size,
            "eye_spacing": eye_spacing,
            "vertical_offset": vertical_offset,
            "pupil_style": select_gene(signature, 2, &["Dot", "Wide", "Spark"]),
            "highlight_style": select_gene(signature, 4, &["Single", "Double", "Diagonal"]),
            "brow_style": select_gene(signature, 6, &["None", "Soft", "Bold"]),
            "mouth_style": select_gene(signature, 8, &["Tiny", "Smile", "Cat", "Beak"]),
            "cheek_style": select_gene(signature, 10, &["None", "Dots", "Blush"]),
        }),
    );
    let (style, tip_style) = match family.as_str() {
        "Hopper" => ("MittenArm", "Mitten"),
        "SoftQuadruped" => ("FrontPaw", "Paw"),
        _ if signature & 1 == 0 => ("SoftNub", "Round"),
        _ => ("Pseudopod", "Round"),
    };
    appearance.insert(
        "forelimbs".into(),
        json!({
            "style": style,
            "length": 3 + (signature >> 12) % 5,
            "thickness": 1 + (signature >> 15) % 2,
            "tip_style": tip_style,
            "rest_pose": select_gene(signature, 16, &["AtSides", "Folded", "Together"]),
        }),
    );
    appearance.insert(
        "effect_motif".into(),
        Value::String(
            select_gene(
                signature,
                18,
                &["None", "Dot", "Star", "Heart", "Leaf", "Spark"],
            )
            .into(),
        ),
    );
}

fn select_gene(signature: u64, shift: u32, values: &'static [&'static str]) -> &'static str {
    values[((signature >> shift) as usize) % values.len()]
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
            home: crate::ColonyHome::default(),
            settings: Settings::default(),
            creatures: Vec::new(),
        }
    }

    fn downgrade_appearances_to_v2(value: &mut serde_json::Value) {
        let creatures = value["creatures"].as_array_mut().unwrap();
        for creature in creatures {
            let appearance = creature["appearance"].as_object_mut().unwrap();
            let face = appearance.remove("face").unwrap();
            appearance.insert("eye_size".into(), face["eye_size"].clone());
            appearance.insert("eye_spacing".into(), face["eye_spacing"].clone());
            appearance.insert("eye_height".into(), face["vertical_offset"].clone());
            let head_appendages = appearance.remove("head_appendages").unwrap();
            let style = head_appendages["style"].clone();
            let size = head_appendages["size"].clone();
            appearance.insert("appendage_style".into(), style);
            appearance.insert("appendage_size".into(), size);
            appearance.remove("forelimbs");
            appearance.remove("effect_motif");
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
    fn current_save_without_fullscreen_preference_defaults_to_occlusion() {
        let mut value = serde_json::to_value(example_save()).unwrap();
        value["settings"]
            .as_object_mut()
            .unwrap()
            .remove("fullscreen_app_occlusion");
        let directory =
            std::env::temp_dir().join(format!("formiga-fullscreen-default-{}", std::process::id()));
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join("colony.json");
        fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        let loaded = SaveStore::new(&path).load().unwrap().unwrap();
        assert!(loaded.settings.fullscreen_app_occlusion);
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

    #[test]
    fn migrates_v2_creature_identity_and_resolves_art_genes_deterministically() {
        let desktop = crate::DesktopSnapshot {
            monitors: vec![crate::MonitorInfo {
                id: 1,
                display_key: crate::DisplayKey([9; 16]),
                bounds: crate::DesktopRect {
                    x: 0.0,
                    y: 0.0,
                    width: 1280.0,
                    height: 800.0,
                },
                usable_bounds: crate::DesktopRect {
                    x: 0.0,
                    y: 24.0,
                    width: 1280.0,
                    height: 736.0,
                },
                scale_factor: 2.0,
                primary: true,
            }],
            ..Default::default()
        };
        let original = crate::World::new([11; 32], time::OffsetDateTime::UNIX_EPOCH, &desktop).save;
        let creature = &original.creatures[0];
        let legacy_eye_size = creature.appearance.face.eye_size;
        let legacy_eye_spacing = creature.appearance.face.eye_spacing;
        let legacy_eye_height = creature.appearance.face.vertical_offset;
        let legacy_appendage = creature.appearance.head_appendages.style;
        let mut value = serde_json::to_value(&original).unwrap();
        value["save_version"] = serde_json::Value::from(2);
        downgrade_appearances_to_v2(&mut value);

        let directory =
            std::env::temp_dir().join(format!("formiga-v2-save-{}", std::process::id()));
        fs::create_dir_all(&directory).unwrap();
        let first_path = directory.join("first.json");
        let second_path = directory.join("second.json");
        let bytes = serde_json::to_vec(&value).unwrap();
        fs::write(&first_path, &bytes).unwrap();
        fs::write(&second_path, &bytes).unwrap();
        let first = SaveStore::new(&first_path).load().unwrap().unwrap();
        let second = SaveStore::new(&second_path).load().unwrap().unwrap();
        let migrated = &first.creatures[0];

        assert_eq!(first, second);
        assert_eq!(first.save_version, crate::SAVE_VERSION);
        assert_eq!(first.colony_seed, original.colony_seed);
        assert_eq!(migrated.id, creature.id);
        assert_eq!(migrated.generation, creature.generation);
        assert_eq!(migrated.personality, creature.personality);
        assert_eq!(migrated.state.relationships, creature.state.relationships);
        assert_eq!(migrated.appearance.family, creature.appearance.family);
        assert_eq!(
            migrated.appearance.palette_index,
            creature.appearance.palette_index
        );
        assert_eq!(
            migrated.appearance.marking_seed,
            creature.appearance.marking_seed
        );
        assert_eq!(
            migrated.appearance.face_signature,
            creature.appearance.face_signature
        );
        assert_eq!(migrated.appearance.face.eye_size, legacy_eye_size);
        assert_eq!(migrated.appearance.face.eye_spacing, legacy_eye_spacing);
        assert_eq!(migrated.appearance.face.vertical_offset, legacy_eye_height);
        assert_eq!(migrated.appearance.head_appendages.style, legacy_appendage);
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn migrates_v3_with_a_deterministic_home_and_cooldown() {
        let mut original = example_save();
        original.colony_seed = [37; 32];
        let mut value = serde_json::to_value(&original).unwrap();
        value["save_version"] = serde_json::Value::from(3);
        value.as_object_mut().unwrap().remove("home");
        let directory =
            std::env::temp_dir().join(format!("formiga-v3-save-{}", std::process::id()));
        fs::create_dir_all(&directory).unwrap();
        let first_path = directory.join("first.json");
        let second_path = directory.join("second.json");
        let bytes = serde_json::to_vec(&value).unwrap();
        fs::write(&first_path, &bytes).unwrap();
        fs::write(&second_path, &bytes).unwrap();

        let first = SaveStore::new(&first_path).load().unwrap().unwrap();
        let second = SaveStore::new(&second_path).load().unwrap().unwrap();
        assert_eq!(first, second);
        assert_eq!(first.save_version, crate::SAVE_VERSION);
        assert_eq!(first.home.corner, crate::HomeCorner::BottomRight);
        assert_eq!(first.home.shelter, second.home.shelter);
        assert_eq!(first.home.active_since_utc, None);
        assert_eq!(
            first.home.last_disappeared_utc,
            Some(first.maximum_seen_utc)
        );
        let _ = fs::remove_dir_all(directory);
    }
}
