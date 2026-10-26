//! Migrations: a colony written by each earlier save version opens as the current one, keeping
//! everything it had and inventing nothing it did not.
use super::*;

/// Bring an older file forward and validate it, exactly as reading it from disk would.
fn migrate(value: serde_json::Value, version: u32) -> Result<SaveFile, PersistenceError> {
    assert_eq!(
        migrations::version_of(&value),
        version,
        "the file says it is {version}"
    );
    migrations::upgrade(value).map(|save| ValidatedSave::from(save).into_inner())
}

#[test]
fn v12_migration_preserves_identity_history_and_defaults_new_features() {
    let desktop = crate::DesktopSnapshot::default();
    let original = crate::World::new([55; 32], datetime!(2026-09-14 12:00 UTC), &desktop).save;
    let mut json = serde_json::to_value(&original).unwrap();
    json["save_version"] = 12.into();
    json.as_object_mut().unwrap().remove("companion");
    json["home"]
        .as_object_mut()
        .unwrap()
        .remove("hidden_decorations");
    let migrated = migrate(json, 12).unwrap();
    assert_eq!(migrated.save_version, crate::SAVE_VERSION);
    assert_eq!(migrated.creatures, original.creatures);
    assert_eq!(migrated.home, original.home);
    assert_eq!(migrated.settings, original.settings);
    assert!(migrated.companion.onboarding_complete);
    assert!(migrated.companion.journal.is_empty());
}

/// A colony written by 0.64, before pair tallies and the journal's read marker: every
/// companion, bond score and journal moment comes through exactly as it was, no pair is given
/// a single moment it was not seen sharing, and the journal reads as already read.
#[test]
fn a_v22_colony_keeps_its_bonds_and_journal_and_invents_no_shared_history() {
    let desktop = crate::DesktopSnapshot::default();
    let now = datetime!(2026-09-30 12:00 UTC);
    let mut world = crate::World::new([61; 32], now, &desktop);
    world.tick(now + time::Duration::days(12), 0.05, &desktop);
    for (index, relationship) in world.save.relationships.iter_mut().enumerate() {
        relationship.affinity = 200 - index as u8 * 9;
        relationship.familiarity = 180;
    }
    world.save.companion.remember(
        Some(world.save.creatures[0].id),
        crate::JournalMoment::Discovery,
        now + time::Duration::days(2),
    );
    let original = world.save;
    assert!(original.relationships.len() >= 3);
    let mut json = serde_json::to_value(&original).unwrap();
    json["save_version"] = 22.into();
    let text = json.to_string();
    assert!(!text.contains("tally") && !text.contains("journal_seen_until"));
    let migrated = migrate(json, 22).unwrap();
    assert_eq!(migrated.save_version, crate::SAVE_VERSION);
    assert_eq!(migrated.creatures, original.creatures);
    assert_eq!(migrated.companion.journal, original.companion.journal);
    assert_eq!(migrated.relationships, original.relationships);
    assert!(migrated.tallies.is_empty());
    assert_eq!(
        migrated.companion.journal_seen_until,
        original.companion.journal.iter().map(|e| e.at).max()
    );
    assert_eq!(migrated.companion.unread().count(), 0);
    // And back out again without gaining anything: an empty tally is not written at all.
    let written = serde_json::to_string(&migrated).unwrap();
    assert!(!written.contains("\"tallies\""), "{written}");
    let reread: SaveFile = serde_json::from_str(&written).unwrap();
    assert_eq!(reread, migrated);
}

/// A colony written by 0.61, before temperaments, archetypes and fractional leanings: every
/// companion comes through exactly as it was, whole-number leanings read as the same values,
/// nobody is given a temperament it did not have, and each reads one from its own values.
#[test]
fn a_v19_colony_keeps_every_companion_exactly_as_it_was() {
    let desktop = crate::DesktopSnapshot::default();
    let now = datetime!(2026-09-24 12:00 UTC);
    let mut world = crate::World::new_original([59; 32], now, &desktop);
    world.tick(now + time::Duration::days(40), 0.05, &desktop);
    for (index, creature) in world.save.creatures.iter_mut().enumerate() {
        creature.tendencies.play = 100.0;
        creature.tendencies.cursor_trust = -(index as f32) * 7.0;
    }
    let original = world.save;
    assert!(original.creatures.len() >= 3);
    assert!(original.creatures.iter().all(|c| c.temperament.is_none()));
    let mut json = serde_json::to_value(&original).unwrap();
    json["save_version"] = 19.into();
    // 0.61 wrote leanings as whole numbers.
    for creature in json["creatures"].as_array_mut().unwrap() {
        let tendencies = creature["tendencies"].as_object_mut().unwrap();
        for value in tendencies.values_mut() {
            *value = serde_json::Value::from(value.as_f64().unwrap().round() as i64);
        }
    }
    let text = json.to_string();
    assert!(!text.contains("temperament") && !text.contains("archetype"));
    let migrated = migrate(json, 19).unwrap();
    assert_eq!(migrated.save_version, crate::SAVE_VERSION);
    assert_eq!(migrated.creatures, original.creatures);
    for creature in &migrated.creatures {
        assert!(creature.temperament.is_none());
        assert_eq!(
            creature.temperament(),
            crate::Temperament::read(&creature.personality)
        );
        assert!(creature.temperament_phrase().starts_with('A'));
    }
    // And back out again without gaining anything.
    let written = serde_json::to_string(&migrated).unwrap();
    assert!(!written.contains("\"temperament\""), "{written}");
}

/// A colony from before 0.60.0 keeps everything it earned: its earned decorations are the
/// village's to choose from, the ones that were showing go on hanging on the colony house in
/// their places and the hidden ones are taken down, and every category is topped up to its
/// first three.
#[test]
fn a_v18_colony_keeps_what_it_earned_and_hangs_what_was_showing() {
    use crate::{DecorationSlot, GardenKind, HangoutKind, OrnamentKind, ShelterDecorationKind};
    let desktop = crate::DesktopSnapshot::default();
    let now = datetime!(2026-09-14 12:00 UTC);
    let original = crate::World::new([57; 32], now, &desktop).save;
    let founder = original.creatures[0].id;
    let mut json = serde_json::to_value(&original).unwrap();
    json["save_version"] = 18.into();
    let home = json["home"].as_object_mut().unwrap();
    home.remove("unlocks");
    home.remove("dressing");
    home.insert(
        "decorations".into(),
        serde_json::json!({
            "decorations": ["Leaf", "Banner", "Lamp", "RoofOrnament"],
            "next_at_utc": "2026-09-20T00:00:00Z",
            "ordinal": 4,
        }),
    );
    // The banner was hidden.
    home.insert("hidden_decorations".into(), 2.into());
    let migrated = migrate(json, 18).unwrap();
    assert_eq!(migrated.save_version, crate::SAVE_VERSION);
    let unlocks = &migrated.home.unlocks;
    for kind in [
        ShelterDecorationKind::Leaf,
        ShelterDecorationKind::Banner,
        ShelterDecorationKind::Lamp,
        ShelterDecorationKind::RoofOrnament,
    ]
    .into_iter()
    .chain(ShelterDecorationKind::STARTING)
    {
        assert!(unlocks.decorations.contains(&kind), "{kind:?} was earned");
    }
    assert_eq!(unlocks.hangouts, HangoutKind::STARTING);
    assert_eq!(unlocks.gardens, GardenKind::STARTING);
    assert_eq!(unlocks.ornaments, OrnamentKind::STARTING);
    let wears = |place| migrated.home.decoration_in(founder, place);
    assert_eq!(
        wears(DecorationSlot::WallLeft),
        Some(ShelterDecorationKind::Leaf)
    );
    assert_eq!(
        wears(DecorationSlot::WallRight),
        Some(ShelterDecorationKind::Lamp)
    );
    assert_eq!(
        wears(DecorationSlot::Roof),
        Some(ShelterDecorationKind::RoofOrnament)
    );
    assert_eq!(wears(DecorationSlot::Eaves), None, "the banner was hidden");
    assert_eq!(migrated.creatures, original.creatures);
}

#[test]
fn v13_migration_adds_empty_keepsakes_and_keeps_everything_that_was_already_there() {
    let mut save = example_save();
    save.companion.journal.push(crate::JournalEntry {
        at: save.created_at_utc,
        creature: None,
        moment: crate::JournalMoment::Ritual(crate::RitualKind::Picnic),
    });
    save.companion.modes[0] = Some(crate::BehaviorPreset::capture(&save.settings));
    save.companion.quiet_until = Some(save.created_at_utc);
    save.companion.onboarding_complete = false;
    save.creatures.push(crate::World::preview_adult(
        [77; 32],
        save.created_at_utc,
        &crate::DesktopSnapshot::default(),
    ));
    save.creatures[0].memory.discoveries_found = 40;
    let mut value = serde_json::to_value(&save).unwrap();
    value["save_version"] = 13.into();
    // A v13 file has none of the new keepsakes at all.
    let companion = value["companion"].as_object_mut().unwrap();
    for field in ["pins", "scrapbook", "appearance", "schedule"] {
        companion.remove(field);
    }
    let migrated = migrate(value, 13).unwrap();
    assert_eq!(migrated.save_version, crate::SAVE_VERSION);
    assert_eq!(migrated.companion.journal, save.companion.journal);
    assert_eq!(migrated.companion.modes, save.companion.modes);
    assert_eq!(migrated.companion.quiet_until, save.companion.quiet_until);
    assert!(!migrated.companion.onboarding_complete);
    assert_eq!(migrated.creatures, save.creatures);
    assert_eq!(migrated.settings, save.settings);
    assert_eq!(migrated.relationships, save.relationships);
    // Nothing is invented from a legacy discovery count, and the defaults are the quiet ones.
    assert!(migrated.companion.pins.is_empty());
    assert!(migrated.companion.scrapbook.is_empty());
    assert!(!migrated.companion.schedule.enabled);
    assert!(migrated.companion.schedule.transitions.is_empty());
    assert_eq!(
        migrated.companion.appearance,
        crate::AppearancePreferences::default()
    );
    assert_eq!(migrated.companion.appearance.text_scale, 100);
    assert_eq!(
        migrated.companion.appearance.theme,
        crate::ThemeChoice::System
    );
}

#[test]
fn v14_migration_leaves_a_colony_with_an_empty_guest_book_and_nobody_visiting() {
    let desktop = crate::DesktopSnapshot::default();
    let now = datetime!(2026-09-14 12:00 UTC);
    let mut world = crate::World::new([65; 32], now, &desktop);
    world
        .invite_visitor(a_friends_code(), now, &desktop)
        .unwrap();
    world.save.visitors.gatherings = 9;
    world.save.visitors.guest_book.push(crate::GuestBookEntry {
        visited_at_utc: now,
        name: "Someone".into(),
        origin: a_friends_code().into(),
        source: crate::VisitorSource::Invited,
    });
    let save = world.save.clone();
    let mut value = serde_json::to_value(&save).unwrap();
    value["save_version"] = 14.into();
    let migrated = migrate(value, 14).unwrap();
    assert_eq!(migrated.save_version, crate::SAVE_VERSION);
    // A version-14 colony has never had a visitor, whatever a field of the same name held.
    assert_eq!(migrated.visitors, crate::VisitorState::default());
    assert!(migrated.visitors.guest.is_none());
    assert!(migrated.visitors.guest_book.is_empty());
    assert_eq!(migrated.visitors.gatherings, 0);
    // Everything the colony really did have is exactly as it was, its journal counting as
    // read up to its newest moment.
    assert_eq!(migrated.creatures, save.creatures);
    assert_eq!(
        migrated.companion,
        crate::CompanionState {
            journal_seen_until: save.companion.journal.iter().map(|e| e.at).max(),
            ..save.companion.clone()
        }
    );
    assert_eq!(migrated.home, save.home);
    assert_eq!(migrated.settings, save.settings);
    assert_eq!(migrated.relationships, save.relationships);
}

/// A version-16 colony opens unchanged: its recipes gain no classic parts, and the one
/// companion from before recipes keeps having none.
#[test]
fn v16_migration_keeps_every_recipe_exactly_as_it_was() {
    let desktop = crate::DesktopSnapshot::default();
    let now = datetime!(2026-09-21 12:00 UTC);
    let mut world = crate::World::new([67; 32], now, &desktop);
    world.tick(now + time::Duration::days(8), 0.05, &desktop);
    for creature in &mut world.save.creatures {
        let design = creature
            .appearance
            .design
            .map(|design| crate::CreatureDesign {
                classic: crate::ClassicParts::default(),
                ..design
            });
        crate::apply_creature_design(creature, design);
    }
    crate::apply_creature_design(&mut world.save.creatures[0], None);
    let save = world.save.clone();
    let mut value = serde_json::to_value(&save).unwrap();
    assert!(
        !value.to_string().contains("classic"),
        "a v16 file has none"
    );
    value["save_version"] = 16.into();
    let migrated = migrate(value, 16).unwrap();
    assert_eq!(migrated.save_version, crate::SAVE_VERSION);
    assert_eq!(migrated.creatures, save.creatures);
    assert_eq!(migrated.creatures[0].appearance.design, None);
    assert!(migrated.creatures.len() > 1);
    assert!(
        migrated
            .creatures
            .iter()
            .filter_map(|creature| creature.appearance.design)
            .all(|design| design.classic.is_modular())
    );
}

#[test]
fn v11_migration_keeps_legacy_appearance_and_does_not_assign_a_design() {
    let mut save = example_save();
    let mut creature = crate::World::preview_adult(
        [51; 32],
        save.created_at_utc,
        &crate::DesktopSnapshot::default(),
    );
    crate::apply_creature_design(&mut creature, None);
    save.creatures.push(creature.clone());
    let mut value = serde_json::to_value(&save).unwrap();
    value["save_version"] = 11.into();
    assert!(value["creatures"][0]["appearance"].get("design").is_none());
    let migrated = migrate(value, 11).unwrap();
    assert_eq!(migrated.save_version, crate::SAVE_VERSION);
    assert_eq!(migrated.creatures[0], creature);
    let resumed = crate::World::from_save(migrated);
    assert_eq!(resumed.save.creatures[0].appearance, creature.appearance);
    assert!(resumed.save.creatures[0].origin.design.is_none());
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
    let directory = std::env::temp_dir().join(format!("formiga-v1-save-{}", std::process::id()));
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

    let directory = std::env::temp_dir().join(format!("formiga-v2-save-{}", std::process::id()));
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
    let directory = std::env::temp_dir().join(format!("formiga-v3-save-{}", std::process::id()));
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
    for (generation, creature) in value["creatures"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .enumerate()
    {
        creature["generation"] = serde_json::Value::from(generation as u64);
        creature.as_object_mut().unwrap().remove("born_at_utc");
    }

    let directory = std::env::temp_dir().join(format!("formiga-v4-save-{}", std::process::id()));
    fs::create_dir_all(&directory).unwrap();
    let path = directory.join("colony.json");
    fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
    let migrated = SaveStore::new(&path).load().unwrap().unwrap();

    assert_eq!(migrated.save_version, crate::SAVE_VERSION);
    // Everything about the home a version-4 file could hold comes through untouched. What the
    // village can choose from is decided afresh for any file older than 19, which no version-4
    // colony could have held in the first place.
    let mut home = migrated.home.clone();
    home.unlocks = expected_home.unlocks.clone();
    home.dressing = expected_home.dressing.clone();
    assert_eq!(home, expected_home);
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

    let directory = std::env::temp_dir().join(format!("formiga-v5-save-{}", std::process::id()));
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
    original.save.creatures[0].tendencies.cursor_trust = 33.0;
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

    let directory = std::env::temp_dir().join(format!("formiga-v6-save-{}", std::process::id()));
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
    let directory = std::env::temp_dir().join(format!("formiga-v7-save-{}", std::process::id()));
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
    let directory = std::env::temp_dir().join(format!("formiga-v8-save-{}", std::process::id()));
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
    let directory = std::env::temp_dir().join(format!("formiga-v9-save-{}", std::process::id()));
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
    // A colony that never earned a decoration hangs none, and has the village's first three
    // of everything to choose from, with the next thing a day or two away.
    assert!(migrated.home.dressing.is_empty());
    assert_eq!(migrated.home.unlocks.decorations.len(), 3);
    assert_eq!(migrated.home.unlocks.ornaments.len(), 3);
    let next = migrated.home.unlocks.next_at_utc - maximum_seen;
    assert!(next >= time::Duration::hours(24) && next <= time::Duration::hours(48));

    let round_trip = directory.join("round-trip.json");
    let store = SaveStore::new(&round_trip);
    store.save(&migrated).unwrap();
    assert_eq!(store.load().unwrap(), Some(migrated));
    let _ = fs::remove_dir_all(directory);
}

#[test]
fn migrates_v10_colonies_without_losing_a_creature_or_its_history() {
    let created = datetime!(2026-01-31 8:30 UTC);
    let desktop = crate::DesktopSnapshot {
        monitors: vec![crate::MonitorInfo {
            id: 1,
            display_key: crate::DisplayKey([10; 16]),
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
    let mut original = crate::World::new([110; 32], created, &desktop);
    original.tick(created + time::Duration::days(32), 0.05, &desktop);
    assert_eq!(original.save.creatures.len(), 4);
    for (index, creature) in original.save.creatures.iter_mut().enumerate() {
        creature.name = format!("Legacy {index}");
        creature.memory.times_petted = index as u32 + 10;
        creature.tendencies.sociability = (index * 7) as f32;
    }
    let expected: Vec<_> = original
        .save
        .creatures
        .iter()
        .map(|creature| {
            (
                creature.id,
                creature.name.clone(),
                creature.memory.clone(),
                creature.tendencies,
                creature.appearance.clone(),
            )
        })
        .collect();

    let mut value = serde_json::to_value(&original.save).unwrap();
    value["save_version"] = serde_json::Value::from(10);
    for creature in value["creatures"].as_array_mut().unwrap() {
        let object = creature.as_object_mut().unwrap();
        object.remove("role");
        object.remove("kept");
        object.remove("mini_arrivals");
    }
    let directory = std::env::temp_dir().join(format!("formiga-v10-save-{}", std::process::id()));
    fs::create_dir_all(&directory).unwrap();
    let path = directory.join("colony.json");
    fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();

    let migrated = SaveStore::new(&path).load().unwrap().unwrap();
    assert_eq!(migrated.creatures.len(), expected.len());
    for (index, (actual, expected)) in migrated.creatures.iter().zip(expected).enumerate() {
        assert_eq!(actual.id, expected.0);
        assert_eq!(actual.name, expected.1);
        assert_eq!(actual.memory, expected.2);
        assert_eq!(actual.tendencies, expected.3);
        // Each takes the size its own seed gives it, and its appearance is otherwise as it was.
        let mut appearance = expected.4.clone();
        appearance.logical_size = crate::size_for(
            crate::stature_percent(actual.origin.source_colony_seed),
            actual.display_scale_percent,
        );
        assert_eq!(actual.appearance, appearance);
        assert!(actual.kept);
        assert!(!actual.mini_arrivals.enabled);
        assert_eq!(actual.role.is_adult(), index == 0);
    }

    let round_trip = directory.join("round-trip.json");
    let store = SaveStore::new(&round_trip);
    store.save(&migrated).unwrap();
    assert_eq!(store.load().unwrap(), Some(migrated));
    let _ = fs::remove_dir_all(directory);
}

/// A version-28 colony opens with nobody sculpted and writes exactly what it read, and a
/// sculpted form comes back from the file exactly as it was kept, beside the recipe anything
/// that cannot draw sculpts draws instead.
#[test]
fn v29_keeps_a_sculpted_form_beside_its_recipe_and_older_colonies_have_none() {
    let desktop = crate::DesktopSnapshot::default();
    let now = datetime!(2026-10-06 12:00 UTC);
    let mut world = crate::World::new([29; 32], now, &desktop);
    world.tick(now + time::Duration::days(3), 0.05, &desktop);
    let save = world.save.clone();
    let mut value = serde_json::to_value(&save).unwrap();
    assert!(!value.to_string().contains("sculpt"), "a v28 file has none");
    value["save_version"] = 28.into();
    let migrated = migrate(value, 28).unwrap();
    assert_eq!(migrated.save_version, crate::SAVE_VERSION);
    assert_eq!(migrated.creatures, save.creatures);

    let mut save = migrated;
    let creature = &mut save.creatures[0];
    creature.appearance = sculpted(creature, crate::forms::Plan::Percher);
    creature.origin.design = creature.appearance.design;
    let recipe = creature.appearance.design.expect("a sculpt keeps a recipe");
    assert_eq!(
        recipe.body,
        crate::BodyPlan::Winged,
        "the nearest companion plan"
    );
    let directory = std::env::temp_dir().join(format!("formiga-sculpt-{}", std::process::id()));
    let store = SaveStore::new(directory.join("colony.json"));
    store.save(&save).unwrap();
    let loaded = store.load().unwrap().unwrap();
    assert_eq!(loaded.creatures, save.creatures);
    assert_eq!(
        crate::forms::Design::of(&loaded.creatures[0].appearance).form,
        crate::forms::Form::Sculpted {
            sculpt: crate::forms::Sculpt::starter(crate::forms::Plan::Percher)
        }
    );
    let _ = fs::remove_dir_all(directory);
}

/// A version-30 colony opens having never played Formiga Battle and writes exactly what it read,
/// and what a colony keeps of Battle comes back from the file exactly as it was kept.
#[test]
fn v31_keeps_a_battle_record_and_keepsakes_and_older_colonies_have_none() {
    let desktop = crate::DesktopSnapshot::default();
    let now = datetime!(2026-10-15 12:00 UTC);
    let mut world = crate::World::new([30; 32], now, &desktop);
    world.tick(now + time::Duration::days(3), 0.05, &desktop);
    let save = world.save.clone();
    let mut value = serde_json::to_value(&save).unwrap();
    assert!(
        value.get("battle").is_none(),
        "a colony that never played writes nothing of it"
    );
    value["save_version"] = 30.into();
    let migrated = migrate(value, 30).unwrap();
    assert_eq!(migrated.save_version, crate::SAVE_VERSION);
    assert!(migrated.battle.is_empty());
    assert_eq!(migrated.creatures, save.creatures);

    let mut world = crate::World::from_save(migrated);
    world.save.battle.record.circuit_points = 15;
    world.save.battle.record.applied = vec!["fedcba9876543210fedcba9876543210".to_owned()];
    let at = world.save.maximum_seen_utc;
    let line = "Mochi, Biscuit and Fig won a Backyard Cup match.".to_owned();
    let earned = world.keep_battle_match(
        std::slice::from_ref(&line),
        &[
            "battle.trophy.backyard_cup".to_owned(),
            "battle.trophy.backyard_cup".to_owned(),
            "battle.ribbon.someday".to_owned(),
        ],
        at,
    );
    assert_eq!(earned, 1, "each keepsake Desktop knows, once");
    let directory = std::env::temp_dir().join(format!("formiga-battle-{}", std::process::id()));
    let store = SaveStore::new(directory.join("colony.json"));
    store.save(&world.save).unwrap();
    let loaded = store.load().unwrap().unwrap();
    assert_eq!(loaded.battle, world.save.battle);
    assert!(
        loaded
            .battle
            .has_keepsake(crate::BattleKeepsake::BackyardCupTrophy)
    );
    assert_eq!(
        loaded.companion.journal.last().map(|entry| &entry.moment),
        Some(&crate::JournalMoment::Battle(line))
    );
    let _ = fs::remove_dir_all(directory);
}

/// A version-31 colony that has played Battle opens with its record as it was and no Handler or
/// trios, and a Handler card and trio come back from the file exactly as they were kept.
#[test]
fn v32_keeps_a_handler_and_trios_and_older_colonies_have_named_none() {
    let desktop = crate::DesktopSnapshot::default();
    let now = datetime!(2026-10-16 12:00 UTC);
    let mut world = crate::World::new([32; 32], now, &desktop);
    world.save.battle.record.circuit_points = 15;
    let save = world.save.clone();
    let mut value = serde_json::to_value(&save).unwrap();
    assert!(value["battle"]["record"].get("handler").is_none());
    assert!(value["battle"]["record"].get("trios").is_none());
    value["save_version"] = 31.into();
    let migrated = migrate(value, 31).unwrap();
    assert_eq!(migrated.save_version, crate::SAVE_VERSION);
    assert_eq!(migrated.battle, save.battle);
    assert!(migrated.battle.record.handler.is_none() && migrated.battle.record.trios.is_empty());

    let mut world = crate::World::from_save(migrated);
    let pennant = crate::BattlePennant {
        shape: "swallowtail".to_owned(),
        field: "forest".to_owned(),
        symbol: "acorn".to_owned(),
        trim: "stitched".to_owned(),
    };
    let ids: Vec<u64> = {
        let mut ids: Vec<u64> = world.save.creatures.iter().map(|c| c.id).take(3).collect();
        ids.extend([900_001, 900_002, 900_003]);
        ids.truncate(3);
        ids.sort_unstable();
        ids
    };
    world.save.battle.record.handler = Some(crate::BattleHandlerCard {
        name: "Jo".to_owned(),
        club: Some("Hedge End Club".to_owned()),
        pennant: pennant.clone(),
    });
    world.save.battle.record.trios = vec![crate::BattleTrio {
        id: "7a1e5c3b9d2f4068".to_owned(),
        name: "The Thistle Three".to_owned(),
        members: ids.clone(),
        pennant: Some(pennant),
        retired: false,
        formed_at_utc: now,
        members_since_utc: now,
        friends: crate::BattleTally {
            played: 3,
            won: 2,
            drawn: 1,
        },
        visitors: crate::BattleTally::default(),
        cups: vec![crate::BattleCupWins {
            cup: "cup.backyard".to_owned(),
            wins: 1,
        }],
        lineups: vec![crate::BattleLineup {
            members: vec![1, 2, 3],
            until_utc: now,
        }],
    }];
    assert!(world.save.battle.holds());
    let directory =
        std::env::temp_dir().join(format!("formiga-battle-handler-{}", std::process::id()));
    let store = SaveStore::new(directory.join("colony.json"));
    store.save(&world.save).unwrap();
    let loaded = store.load().unwrap().unwrap();
    assert_eq!(loaded.battle, world.save.battle);
    let _ = fs::remove_dir_all(directory);
}

#[test]
fn v33_keeps_the_handler_key_and_older_colonies_are_given_one_at_battle() {
    let desktop = crate::DesktopSnapshot::default();
    let now = datetime!(2026-10-17 12:00 UTC);
    let mut world = crate::World::new([33; 32], now, &desktop);
    world.save.battle.record.circuit_points = 15;
    let save = world.save.clone();
    let mut value = serde_json::to_value(&save).unwrap();
    assert!(value["battle"]["record"].get("handler_key").is_none());
    value["save_version"] = 32.into();
    let migrated = migrate(value, 32).unwrap();
    assert_eq!(migrated.save_version, crate::SAVE_VERSION);
    assert_eq!(migrated.battle, save.battle);
    assert!(migrated.battle.record.handler_key.is_none());

    let mut world = crate::World::from_save(migrated);
    assert!(world.save.battle.record.give_handler_key().unwrap());
    let key = world.save.battle.record.handler_key.clone().unwrap();
    assert_eq!(key.len(), 32);
    assert!(
        key.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    );
    // Made once: asking again keeps the same key.
    assert!(!world.save.battle.record.give_handler_key().unwrap());
    assert_eq!(
        world.save.battle.record.handler_key.as_deref(),
        Some(key.as_str())
    );
    assert!(world.save.battle.holds());

    let directory =
        std::env::temp_dir().join(format!("formiga-battle-handler-key-{}", std::process::id()));
    let store = SaveStore::new(directory.join("colony.json"));
    store.save(&world.save).unwrap();
    let loaded = store.load().unwrap().unwrap();
    assert_eq!(loaded.battle.record.handler_key, Some(key));
    let _ = fs::remove_dir_all(directory);
}
