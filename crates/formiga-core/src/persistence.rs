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
            1..=9 => migrate_legacy(value, version),
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
    if source_version <= 5 {
        migrate_lived_experience(&mut value);
    }
    if source_version <= 6 {
        migrate_relationships(&mut value);
    }
    value["save_version"] = serde_json::Value::from(crate::SAVE_VERSION);
    let mut save: SaveFile = serde_json::from_value(value)?;
    save.save_version = crate::SAVE_VERSION;
    if save.ritual.next_at_utc == time::OffsetDateTime::UNIX_EPOCH {
        save.ritual.next_at_utc =
            crate::world::scheduled_ritual_at(save.colony_seed, 0, save.maximum_seen_utc);
    }
    save.objects.objects.truncate(crate::MAX_COLONY_OBJECTS);
    if save.objects.next_at_utc == time::OffsetDateTime::UNIX_EPOCH {
        save.objects.next_at_utc = crate::world::scheduled_colony_object_at(
            save.colony_seed,
            save.objects.ordinal,
            save.maximum_seen_utc,
        );
    }
    if source_version == 1 && primary_only {
        save.settings.habitat.preset = crate::HabitatPreset::PrimaryDisplay;
    }
    if source_version <= 3 {
        save.home =
            crate::ColonyHome::from_seed(save.colony_seed, None, None, Some(save.maximum_seen_utc));
    }
    let mut seen_decorations = std::collections::BTreeSet::new();
    save.home
        .decorations
        .decorations
        .retain(|kind| seen_decorations.insert(*kind));
    save.home
        .decorations
        .decorations
        .truncate(crate::MAX_SHELTER_DECORATIONS);
    if save.home.decorations.next_at_utc == time::OffsetDateTime::UNIX_EPOCH {
        save.home.decorations.next_at_utc = crate::world::scheduled_shelter_decoration_at(
            save.colony_seed,
            save.home.decorations.ordinal,
            save.maximum_seen_utc,
        );
    }
    for creature in &mut save.creatures {
        if creature.born_at_utc == time::OffsetDateTime::UNIX_EPOCH {
            creature.born_at_utc = legacy_birth_at(save.created_at_utc, creature.generation);
        }
    }
    Ok(save)
}

fn migrate_relationships(value: &mut serde_json::Value) {
    use serde_json::{Map, Value, json};

    let Some(creatures) = value.get_mut("creatures").and_then(Value::as_array_mut) else {
        value["relationships"] = Value::Array(Vec::new());
        return;
    };
    let creature_ids: Vec<_> = creatures
        .iter()
        .filter_map(|creature| creature.get("id").and_then(Value::as_u64))
        .collect();
    let valid_ids: std::collections::BTreeSet<_> = creature_ids.iter().copied().collect();
    let mut legacy_scores: std::collections::BTreeMap<(u64, u64), Vec<f64>> =
        std::collections::BTreeMap::new();
    for creature in creatures.iter_mut() {
        let Some(source) = creature.get("id").and_then(Value::as_u64) else {
            continue;
        };
        let relationships = creature
            .pointer_mut("/state/relationships")
            .map(Value::take)
            .and_then(|value| value.as_object().cloned())
            .unwrap_or_else(Map::new);
        if let Some(state) = creature.get_mut("state").and_then(Value::as_object_mut) {
            state.remove("relationships");
        }
        for (target, score) in relationships {
            let Ok(target) = target.parse::<u64>() else {
                continue;
            };
            let Some(pair) = crate::canonical_creature_pair(source, target) else {
                continue;
            };
            if valid_ids.contains(&target)
                && let Some(score) = score.as_f64()
            {
                legacy_scores
                    .entry(pair)
                    .or_default()
                    .push(score.clamp(0.0, 1.0));
            }
        }
    }

    let mut relationships = Vec::new();
    for (index, a) in creature_ids.iter().copied().enumerate() {
        for b in creature_ids.iter().copied().skip(index + 1) {
            let Some(pair) = crate::canonical_creature_pair(a, b) else {
                continue;
            };
            let scores = legacy_scores.get(&pair).map(Vec::as_slice).unwrap_or(&[]);
            let legacy_affinity = if scores.is_empty() {
                0.0
            } else {
                scores.iter().sum::<f64>() / scores.len() as f64
            };
            relationships.push(json!({
                "a": pair.0,
                "b": pair.1,
                "affinity": (legacy_affinity * 255.0).round() as u8,
                "familiarity": (legacy_affinity * 64.0).round() as u8,
                "playfulness": 0,
                "avoidance": 0,
            }));
        }
    }
    relationships.truncate(crate::MAX_RELATIONSHIPS);
    value["relationships"] = Value::Array(relationships);
}

fn migrate_lived_experience(value: &mut serde_json::Value) {
    use serde_json::{Value, json};

    let colony_seed: [u8; 32] = value
        .get("colony_seed")
        .and_then(Value::as_array)
        .and_then(|bytes| {
            let bytes: Vec<u8> = bytes
                .iter()
                .map(Value::as_u64)
                .collect::<Option<Vec<_>>>()?
                .into_iter()
                .map(|byte| u8::try_from(byte).ok())
                .collect::<Option<Vec<_>>>()?;
            bytes.try_into().ok()
        })
        .unwrap_or_default();
    let Some(creatures) = value.get_mut("creatures").and_then(Value::as_array_mut) else {
        return;
    };
    let mut names = Vec::with_capacity(creatures.len());
    for (colony_order, creature) in creatures.iter_mut().enumerate() {
        let generation = creature
            .get("generation")
            .and_then(Value::as_u64)
            .and_then(|value| u8::try_from(value).ok())
            .unwrap_or(colony_order as u8);
        let name = crate::default_creature_name(colony_seed, generation, &names);
        names.push(name.clone());
        let routines = creature
            .pointer_mut("/state/habits")
            .map(Value::take)
            .and_then(|habits| migrate_habits(&habits))
            .unwrap_or_default();
        if let Some(state) = creature.get_mut("state").and_then(Value::as_object_mut) {
            state.remove("habits");
        }
        let Some(creature) = creature.as_object_mut() else {
            continue;
        };
        creature.insert(
            "origin".into(),
            json!({
                "source_colony_seed": colony_seed,
                "source_generation": generation,
            }),
        );
        creature.insert("colony_order".into(), Value::from(colony_order as u64));
        creature.insert("name".into(), Value::String(name));
        creature.insert("memory".into(), json!(crate::CreatureMemory::default()));
        creature.insert(
            "tendencies".into(),
            json!(crate::LearnedTendencies::default()),
        );
        creature.insert("routines".into(), json!(routines));
    }
}

fn migrate_habits(value: &serde_json::Value) -> Option<crate::RoutineTable> {
    let habits = value.as_object()?;
    let entries = habits
        .iter()
        .filter_map(|(legacy_key, strength)| {
            let mut parts = legacy_key.split(':');
            let time_bucket = parts.next()?.parse::<u8>().ok()?.min(3);
            let region = parts.next()?.parse::<u8>().ok()?.min(2);
            let surface = match parts.next()? {
                "ScreenFloor" => crate::SurfaceKind::ScreenFloor,
                "WindowLedge" => crate::SurfaceKind::WindowLedge,
                _ => return None,
            };
            let action = crate::ActionKind::from_legacy_name(parts.next()?)?;
            let relative_x = (f32::from(region) + 0.5) / 3.0;
            let key = crate::routine_key(surface, relative_x, action, time_bucket * 6);
            Some((key, strength.as_f64()? as f32))
        })
        .collect();
    Some(crate::RoutineTable::from_ranked(entries))
}

fn legacy_birth_at(created_at_utc: time::OffsetDateTime, generation: u8) -> time::OffsetDateTime {
    const LEGACY_ARRIVAL_DAYS: [i64; 3] = [30, 90, 180];
    generation
        .checked_sub(1)
        .and_then(|index| LEGACY_ARRIVAL_DAYS.get(index as usize))
        .map_or(created_at_utc, |days| {
            created_at_utc + time::Duration::days(*days)
        })
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
        let mut home = crate::ColonyHome::default();
        home.decorations.next_at_utc = datetime!(2026-01-05 0:00 UTC);
        SaveFile {
            save_version: crate::SAVE_VERSION,
            colony_seed: [1; 32],
            created_at_utc: datetime!(2026-01-01 0:00 UTC),
            maximum_seen_utc: datetime!(2026-01-01 0:00 UTC),
            arrival_state: ArrivalState::default(),
            home,
            settings: Settings::default(),
            creatures: Vec::new(),
            relationships: Vec::new(),
            ritual: crate::RitualState {
                next_at_utc: datetime!(2026-01-02 0:00 UTC),
                ..crate::RitualState::default()
            },
            objects: crate::ColonyObjectState {
                next_at_utc: datetime!(2026-01-04 0:00 UTC),
                ..crate::ColonyObjectState::default()
            },
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
    fn corrupt_primary_recovers_the_previous_atomic_save_from_backup() {
        let directory =
            std::env::temp_dir().join(format!("formiga-backup-recovery-{}", std::process::id()));
        let path = directory.join("colony.json");
        let store = SaveStore::new(&path);
        let original = example_save();
        store.save(&original).unwrap();
        let mut replacement = original.clone();
        replacement.settings.reduce_motion = true;
        store.save(&replacement).unwrap();
        fs::write(&path, b"{not valid json").unwrap();

        assert_eq!(store.load().unwrap(), Some(original));
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
        assert_eq!(first.relationships, original.relationships);
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

    #[test]
    fn migrates_v4_birth_times_without_replacing_the_existing_home() {
        let created = datetime!(2026-01-01 8:30 UTC);
        let desktop = crate::DesktopSnapshot {
            monitors: vec![crate::MonitorInfo {
                id: 1,
                display_key: crate::DisplayKey([4; 16]),
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
        let mut world = crate::World::new([44; 32], created, &desktop);
        world.tick(created + time::Duration::days(181), 0.05, &desktop);
        let expected_home = world.save.home.clone();
        let mut value = serde_json::to_value(&world.save).unwrap();
        value["save_version"] = serde_json::Value::from(4);
        for creature in value["creatures"].as_array_mut().unwrap() {
            creature.as_object_mut().unwrap().remove("born_at_utc");
        }

        let directory =
            std::env::temp_dir().join(format!("formiga-v4-save-{}", std::process::id()));
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join("colony.json");
        fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        let migrated = SaveStore::new(&path).load().unwrap().unwrap();

        assert_eq!(migrated.save_version, crate::SAVE_VERSION);
        assert_eq!(migrated.home, expected_home);
        assert_eq!(migrated.creatures[0].born_at_utc, created);
        assert_eq!(
            migrated.creatures[1].born_at_utc,
            created + time::Duration::days(30)
        );
        assert_eq!(
            migrated.creatures[2].born_at_utc,
            created + time::Duration::days(90)
        );
        assert_eq!(
            migrated.creatures[3].born_at_utc,
            created + time::Duration::days(180)
        );
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn migrates_v5_names_origins_and_the_twelve_strongest_routines() {
        let desktop = crate::DesktopSnapshot::default();
        let original = crate::World::new([61; 32], datetime!(2026-02-03 4:05 UTC), &desktop).save;
        let mut value = serde_json::to_value(&original).unwrap();
        value["save_version"] = serde_json::Value::from(5);
        let creature = value["creatures"][0].as_object_mut().unwrap();
        for field in [
            "origin",
            "colony_order",
            "name",
            "memory",
            "tendencies",
            "routines",
        ] {
            creature.remove(field);
        }
        let habits = (0..16)
            .map(|index| {
                (
                    format!("{}:{}:ScreenFloor:Idle", index % 4, index % 3),
                    serde_json::Value::from(f64::from(index) / 16.0),
                )
            })
            .collect();
        creature["state"]["habits"] = serde_json::Value::Object(habits);

        let directory =
            std::env::temp_dir().join(format!("formiga-v5-save-{}", std::process::id()));
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join("colony.json");
        fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        let migrated = SaveStore::new(&path).load().unwrap().unwrap();
        let creature = &migrated.creatures[0];

        assert_eq!(migrated.save_version, crate::SAVE_VERSION);
        assert_eq!(creature.origin.source_colony_seed, [61; 32]);
        assert_eq!(creature.origin.source_generation, 0);
        assert_eq!(creature.colony_order, 0);
        assert!(!creature.name.is_empty());
        assert!(creature.routines.len <= crate::MAX_ROUTINES as u8);
        assert_eq!(creature.memory, crate::CreatureMemory::default());
        assert_eq!(creature.tendencies, crate::LearnedTendencies::default());
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn migrates_v6_relationships_without_changing_creature_identity_or_history() {
        let created = datetime!(2026-02-03 4:05 UTC);
        let desktop = crate::DesktopSnapshot {
            monitors: vec![crate::MonitorInfo {
                id: 1,
                display_key: crate::DisplayKey([6; 16]),
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
        let mut original = crate::World::new([66; 32], created, &desktop);
        original.tick(created + time::Duration::hours(1), 0.05, &desktop);
        original.save.creatures[0].name = "Keepsake".into();
        original.save.creatures[0].memory.times_petted = 47;
        original.save.creatures[0].tendencies.cursor_trust = 33;
        let preserved_creatures = original.save.creatures.clone();
        let first_id = preserved_creatures[0].id;
        let second_id = preserved_creatures[1].id;

        let mut value = serde_json::to_value(&original.save).unwrap();
        value["save_version"] = serde_json::Value::from(6);
        value.as_object_mut().unwrap().remove("relationships");
        value["creatures"][0]["state"]["relationships"] =
            serde_json::json!({ second_id.to_string(): 0.75 });
        value["creatures"][1]["state"]["relationships"] =
            serde_json::json!({ first_id.to_string(): 0.25 });

        let directory =
            std::env::temp_dir().join(format!("formiga-v6-save-{}", std::process::id()));
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
        assert_eq!(first.creatures, preserved_creatures);
        assert_eq!(first.relationships.len(), 1);
        let relationship = first.relationships[0];
        assert_eq!(
            (relationship.a, relationship.b),
            (first_id.min(second_id), first_id.max(second_id))
        );
        assert_eq!(relationship.affinity, 128);
        assert_eq!(relationship.familiarity, 32);
        assert_eq!(relationship.playfulness, 0);
        assert_eq!(relationship.avoidance, 0);

        let round_trip_path = directory.join("round-trip.json");
        let round_trip_store = SaveStore::new(&round_trip_path);
        round_trip_store.save(&first).unwrap();
        assert_eq!(round_trip_store.load().unwrap(), Some(first));
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn migrates_v7_ritual_state_without_changing_any_creature_or_bond() {
        let created = datetime!(2026-02-03 4:05 UTC);
        let desktop = crate::DesktopSnapshot {
            monitors: vec![crate::MonitorInfo {
                id: 1,
                display_key: crate::DisplayKey([7; 16]),
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
        let mut original = crate::World::new([77; 32], created, &desktop);
        original.tick(created + time::Duration::hours(1), 0.05, &desktop);
        original.save.creatures[0].name = "Keepsake".into();
        original.save.creatures[0].memory.times_petted = 19;
        let creatures = original.save.creatures.clone();
        let relationships = original.save.relationships.clone();
        let maximum_seen = original.save.maximum_seen_utc;

        let mut value = serde_json::to_value(&original.save).unwrap();
        value["save_version"] = serde_json::Value::from(7);
        value.as_object_mut().unwrap().remove("ritual");
        let directory =
            std::env::temp_dir().join(format!("formiga-v7-save-{}", std::process::id()));
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join("colony.json");
        fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();

        let migrated = SaveStore::new(&path).load().unwrap().unwrap();
        assert_eq!(migrated.save_version, crate::SAVE_VERSION);
        assert_eq!(migrated.creatures, creatures);
        assert_eq!(migrated.relationships, relationships);
        assert!(migrated.ritual.next_at_utc - maximum_seen >= time::Duration::hours(12));
        assert!(migrated.ritual.next_at_utc - maximum_seen <= time::Duration::hours(48));
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn migrates_v8_objects_without_changing_the_existing_colony() {
        let created = datetime!(2026-02-03 4:05 UTC);
        let desktop = crate::DesktopSnapshot {
            monitors: vec![crate::MonitorInfo {
                id: 1,
                display_key: crate::DisplayKey([8; 16]),
                bounds: crate::DesktopRect {
                    x: -1280.0,
                    y: 0.0,
                    width: 1280.0,
                    height: 800.0,
                },
                usable_bounds: crate::DesktopRect {
                    x: -1280.0,
                    y: 24.0,
                    width: 1280.0,
                    height: 736.0,
                },
                scale_factor: 1.0,
                primary: true,
            }],
            ..Default::default()
        };
        let mut original = crate::World::new([88; 32], created, &desktop);
        original.tick(created + time::Duration::hours(1), 0.05, &desktop);
        original.save.creatures[0].name = "Keepsake".into();
        original.save.creatures[0].memory.times_petted = 23;
        original.save.ritual.ordinal = 4;
        let creatures = original.save.creatures.clone();
        let relationships = original.save.relationships.clone();
        let home = original.save.home.clone();
        let settings = original.save.settings.clone();
        let ritual = original.save.ritual.clone();
        let maximum_seen = original.save.maximum_seen_utc;

        let mut value = serde_json::to_value(&original.save).unwrap();
        value["save_version"] = serde_json::Value::from(8);
        value.as_object_mut().unwrap().remove("objects");
        value["home"].as_object_mut().unwrap().remove("decorations");
        let directory =
            std::env::temp_dir().join(format!("formiga-v8-save-{}", std::process::id()));
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join("colony.json");
        fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();

        let migrated = SaveStore::new(&path).load().unwrap().unwrap();
        assert_eq!(migrated.save_version, crate::SAVE_VERSION);
        assert_eq!(migrated.creatures, creatures);
        assert_eq!(migrated.relationships, relationships);
        assert_eq!(migrated.home.display, home.display);
        assert_eq!(migrated.home.corner, home.corner);
        assert_eq!(migrated.home.shelter, home.shelter);
        assert_eq!(migrated.home.active_since_utc, home.active_since_utc);
        assert_eq!(
            migrated.home.last_disappeared_utc,
            home.last_disappeared_utc
        );
        assert_eq!(migrated.settings, settings);
        assert_eq!(migrated.ritual, ritual);
        assert!(migrated.objects.objects.is_empty());
        assert!(migrated.objects.next_at_utc - maximum_seen >= time::Duration::days(3));
        assert!(migrated.objects.next_at_utc - maximum_seen <= time::Duration::days(7));

        let round_trip = directory.join("round-trip.json");
        let store = SaveStore::new(&round_trip);
        store.save(&migrated).unwrap();
        assert_eq!(store.load().unwrap(), Some(migrated));
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn migrates_v9_decorations_without_changing_objects_or_creatures() {
        let created = datetime!(2026-03-04 5:06 UTC);
        let desktop = crate::DesktopSnapshot {
            monitors: vec![crate::MonitorInfo {
                id: 1,
                display_key: crate::DisplayKey([9; 16]),
                bounds: crate::DesktopRect {
                    x: 0.0,
                    y: 0.0,
                    width: 1440.0,
                    height: 900.0,
                },
                usable_bounds: crate::DesktopRect {
                    x: 0.0,
                    y: 24.0,
                    width: 1440.0,
                    height: 826.0,
                },
                scale_factor: 2.0,
                primary: true,
            }],
            ..Default::default()
        };
        let mut original = crate::World::new([99; 32], created, &desktop);
        original.tick(created + time::Duration::hours(1), 0.05, &desktop);
        original.save.creatures[0].name = "Memento".into();
        original.save.creatures[0].memory.discoveries_found = 12;
        original.save.objects.next_at_utc = created;
        original.tick(created + time::Duration::days(1), 0.05, &desktop);
        let creatures = original.save.creatures.clone();
        let relationships = original.save.relationships.clone();
        let objects = original.save.objects.clone();
        let ritual = original.save.ritual.clone();
        let maximum_seen = original.save.maximum_seen_utc;
        let expected_home = original.save.home.clone();

        let mut value = serde_json::to_value(&original.save).unwrap();
        value["save_version"] = serde_json::Value::from(9);
        value["home"].as_object_mut().unwrap().remove("decorations");
        let directory =
            std::env::temp_dir().join(format!("formiga-v9-save-{}", std::process::id()));
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join("colony.json");
        fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();

        let migrated = SaveStore::new(&path).load().unwrap().unwrap();
        assert_eq!(migrated.save_version, crate::SAVE_VERSION);
        assert_eq!(migrated.creatures, creatures);
        assert_eq!(migrated.relationships, relationships);
        assert_eq!(migrated.objects, objects);
        assert_eq!(migrated.ritual, ritual);
        assert_eq!(migrated.home.display, expected_home.display);
        assert_eq!(migrated.home.corner, expected_home.corner);
        assert_eq!(migrated.home.shelter, expected_home.shelter);
        assert!(migrated.home.decorations.decorations.is_empty());
        assert!(migrated.home.decorations.next_at_utc - maximum_seen >= time::Duration::days(4));
        assert!(migrated.home.decorations.next_at_utc - maximum_seen <= time::Duration::days(9));

        let round_trip = directory.join("round-trip.json");
        let store = SaveStore::new(&round_trip);
        store.save(&migrated).unwrap();
        assert_eq!(store.load().unwrap(), Some(migrated));
        let _ = fs::remove_dir_all(directory);
    }
}
