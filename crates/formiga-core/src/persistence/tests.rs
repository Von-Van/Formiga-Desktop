use super::*;
use crate::{ArrivalState, Settings};
use time::macros::datetime;

/// Bring an older file forward and validate it, exactly as reading it from disk would.
fn migrate(value: serde_json::Value, version: u32) -> Result<SaveFile, PersistenceError> {
    assert_eq!(
        migrations::version_of(&value),
        version,
        "the file says it is {version}"
    );
    migrations::upgrade(value).map(|save| ValidatedSave::from(save).into_inner())
}

/// Every field name a version-17 colony file is allowed to use, gathered from a colony that
/// has one of everything. The list is long on purpose: an observation that reached the save
/// would have to bring a name with it, and this is what notices.
const SAVED_FIELDS: [&str; 275] = [
    "Decoration",
    "Friendship",
    "Garden",
    "Habit",
    "Hangout",
    "MacBundleId",
    "Object",
    "Ornament",
    "Pin",
    "Preference",
    "Revisit",
    "Ritual",
    "Unlocked",
    "Visit",
    "Worn",
    "a",
    "accent",
    "accent_index",
    "accessory",
    "action",
    "action_duration",
    "action_elapsed",
    "active_since_utc",
    "activity",
    "activity_variant",
    "affection",
    "affinity",
    "along",
    "appearance",
    "application",
    "application_occlusion_rules",
    "applied",
    "archetype",
    "arousal",
    "arrival_delay_secs",
    "arrival_state",
    "arrived",
    "at",
    "avoidance",
    "axes",
    "b",
    "behavior_seed",
    "belly",
    "belly_color",
    "body",
    "body_height",
    "body_width",
    "boldness",
    "boredom",
    "born_at_utc",
    "brow_style",
    "calm_spells",
    "cell",
    "cheek_style",
    "classic",
    "climbing",
    "coat",
    "colony_order",
    "colony_seed",
    "comfort",
    "companion",
    "confidence",
    "corner",
    "cottage_order",
    "created_at_utc",
    "creature",
    "creatures",
    "crown",
    "curiosity",
    "curiosity_satisfaction",
    "cursor_cooldown",
    "cursor_interest",
    "cursor_reactions",
    "cursor_trust",
    "days",
    "decision_temperature",
    "decorations",
    "descriptor_flags",
    "design",
    "detail_seed",
    "details",
    "direct_manipulation",
    "discoveries_found",
    "display",
    "display_name",
    "display_scale",
    "display_scale_percent",
    "dressing",
    "drives",
    "ear_size",
    "ears",
    "effect_motif",
    "enabled",
    "energy",
    "exploration",
    "eye_shape",
    "eye_size",
    "eye_spacing",
    "face",
    "face_signature",
    "face_template",
    "facing_right",
    "familiarity",
    "family",
    "favorite_display",
    "favorites",
    "feistiness",
    "finder",
    "finder_name",
    "first_at",
    "foot_size",
    "forelimbs",
    "fullscreen_app_occlusion",
    "gait_bob",
    "gardens",
    "gatherings",
    "generation",
    "gifts",
    "greetings",
    "guest",
    "guest_book",
    "habitat",
    "habits",
    "hangouts",
    "hatch_day_acknowledged_year",
    "head",
    "head_appendages",
    "head_ratio",
    "height",
    "highlight_style",
    "home",
    "home_affinity",
    "home_visits",
    "hooks",
    "horns",
    "house_styles",
    "id",
    "impulsiveness",
    "journal",
    "journal_seen_until",
    "keeper",
    "kept",
    "kept_at_utc",
    "key",
    "kind",
    "last_disappeared_utc",
    "last_kind",
    "launch_at_login",
    "leaning",
    "ledge_seconds",
    "leg_length",
    "legs",
    "len",
    "length",
    "limbs",
    "logical_size",
    "longest_sleep_seconds",
    "marking",
    "marking_seed",
    "maximum_seen_utc",
    "memory",
    "milestone_bubble_shown",
    "milestone_cooldown_active_seconds",
    "mini_arrivals",
    "minute",
    "modes",
    "moment",
    "monitor_id",
    "mouth_style",
    "muzzle",
    "name",
    "next_at_utc",
    "normalized_bounds",
    "normalized_position",
    "objects",
    "on_stage",
    "onboarding_complete",
    "ordinal",
    "origin",
    "ornaments",
    "overridden",
    "palette",
    "palette_index",
    "parent_id",
    "pattern",
    "pattern_density",
    "paused",
    "personality",
    "pins",
    "placements",
    "planted_at_utc",
    "play",
    "play_sessions",
    "playfulness",
    "plays",
    "position",
    "preferred_region",
    "preset",
    "profile_revision",
    "pupil_style",
    "quiet_until",
    "reduce_motion",
    "relationships",
    "relative_x",
    "rest_pose",
    "ritual",
    "role",
    "roundness",
    "routine",
    "routine_affinity",
    "routines",
    "save_version",
    "schedule",
    "scrapbook",
    "settings",
    "shared_rests",
    "shelter",
    "signed",
    "size",
    "sleep_interruptions",
    "sleep_pressure",
    "sleep_security",
    "sleep_timing",
    "slots",
    "sociability",
    "social",
    "social_need",
    "sought",
    "source",
    "source_colony_seed",
    "source_generation",
    "sprite_outline",
    "squabbles",
    "state",
    "stays_until_utc",
    "strength",
    "style",
    "surface",
    "suspicion",
    "tail",
    "tail_length",
    "tail_style",
    "tallies",
    "tally",
    "temperament",
    "tendencies",
    "tension",
    "text_scale",
    "theme",
    "thickness",
    "times_petted",
    "times_tossed",
    "tip",
    "tip_color",
    "tip_style",
    "transitions",
    "tree_keepsakes",
    "unlocks",
    "variant",
    "velocity",
    "vertical_offset",
    "viewed_profile_revision",
    "visible",
    "visited_at_utc",
    "visitors",
    "width",
    "window_climbs",
    "window_key",
    "window_ledges",
    "window_ride_seconds",
    "window_tolerance",
    "wings",
    "x",
    "y",
    "zones",
];

/// The vocabulary of watching a desktop and of a scene under way. None of it belongs in a file.
const RUNTIME_ONLY_FIELDS: [&str; 42] = [
    "answers",
    "attention",
    "beat",
    "bounds",
    "cooldowns",
    "cursor",
    "doorway",
    "elapsed",
    "emotion",
    "flourish",
    "hanging",
    "holder",
    "hop",
    "hops",
    "idle_duration",
    "phase",
    "journey",
    "journeys",
    "landing",
    "landmark",
    "landmarks",
    "last_seen",
    "minimized",
    "monotonic_millis",
    "nudge",
    "observer",
    "path",
    "plan",
    "plans",
    "plaything",
    "pose",
    "refusals",
    "reservation",
    "route",
    "routes",
    "scene",
    "session",
    "setbacks",
    "signals",
    "started_at",
    "trail",
    "z_order",
];

#[test]
fn a_file_written_mid_scene_holds_no_window_cursor_or_play_in_progress() {
    let (world, captured) = colony_in_the_middle_of_everything();
    let directory = std::env::temp_dir().join(format!("formiga-privacy-{}", std::process::id()));
    let store = SaveStore::new(directory.join("colony.json"));
    store.save(&world.save).unwrap();
    let text = fs::read_to_string(store.path()).unwrap();
    eprintln!(
        "a four-companion colony mid-scene writes {} bytes",
        text.len()
    );
    let value: serde_json::Value = serde_json::from_str(&text).unwrap();

    // Whatever the file turns out to hold, it was written while all of this was happening.
    let mut names = std::collections::BTreeSet::new();
    collect_field_names(&value, &mut names);
    for name in &names {
        assert!(
            SAVED_FIELDS.contains(&name.as_str()),
            "the file names {name:?}, which no colony file is meant to hold"
        );
    }
    for absent in RUNTIME_ONLY_FIELDS {
        assert!(!names.contains(absent), "the file names {absent:?}");
    }
    // The sweep reaches what the person at the desk arranged, not only what the colony did.
    for arranged in [
        "habits",
        "hangouts",
        "cottage_order",
        "palette",
        "gardens",
        "house_styles",
    ] {
        assert!(
            names.contains(arranged),
            "the file should hold {arranged:?}"
        );
    }

    // The only rectangle a colony keeps is a habitat zone the user drew, in fractions of a
    // display. A window's frame cannot arrive dressed as one of those.
    let zones = collect_fields(&value, "normalized_bounds");
    assert!(!zones.is_empty(), "the colony has a zone to check");
    for zone in zones {
        for edge in ["x", "y", "width", "height"] {
            let value = zone[edge].as_f64().unwrap();
            assert!((0.0..=1.0).contains(&value), "{edge} of a zone is {value}");
        }
    }

    // There are exactly as many positions in the file as there are companions, and each one is
    // that companion's own. Nobody's last-seen spot, viewing place, or route hop is written.
    let positions = collect_fields(&value, "position");
    assert_eq!(positions.len(), world.save.creatures.len());
    for creature in &world.save.creatures {
        assert!(positions.iter().any(|written| {
            written["x"].as_f64().map(|x| x as f32) == Some(creature.state.position.x)
                && written["y"].as_f64().map(|y| y as f32) == Some(creature.state.position.y)
        }));
    }

    // Set a companion's own position and speed aside, and no number left in the file came off
    // the desktop: not an edge of either window, and not where the cursor was.
    let mut without_creature_motion = value.clone();
    for creature in without_creature_motion["creatures"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
    {
        let state = creature["state"].as_object_mut().unwrap();
        state.remove("position");
        state.remove("velocity");
    }
    let mut numbers = Vec::new();
    collect_numbers(&without_creature_motion, &mut numbers);
    assert!(
        numbers.contains(&(crate::SAVE_VERSION as f32)),
        "the sweep is reading the file"
    );
    let mut observed = Vec::new();
    for step in 0..=captured {
        let desktop = observed_desktop(step);
        observed.extend([
            desktop.cursor.position.x,
            desktop.cursor.position.y,
            desktop.cursor.velocity.x,
            desktop.cursor.velocity.y,
        ]);
        for window in desktop.windows {
            observed.extend([
                window.bounds.x,
                window.bounds.y,
                window.bounds.width,
                window.bounds.height,
                window.bounds.right(),
                window.bounds.bottom(),
            ]);
        }
    }
    for value in observed {
        assert!(
            !numbers.contains(&value),
            "the file holds {value}, which it could only have read off the desktop"
        );
    }

    // Reopening the colony finds nobody mid-scene: no gaze, no plan, no leftover motion.
    let reopened = crate::World::from_save(store.load().unwrap().unwrap());
    assert!(reopened.save.creatures.iter().all(|creature| {
        creature.state.attention.is_none()
            && creature.state.action == crate::ActionKind::Idle
            && creature.state.velocity == crate::Point::default()
    }));
    let _ = fs::remove_dir_all(directory);
}

#[test]
fn an_oversized_file_loads_with_every_collection_back_inside_its_cap() {
    let now = datetime!(2026-02-01 9:00 UTC);
    let mut save = example_save();
    save.creatures.push(crate::World::preview_adult(
        [23; 32],
        now,
        &crate::DesktopSnapshot::default(),
    ));
    for index in 0..200u16 {
        save.companion.journal.push(crate::JournalEntry {
            at: now + time::Duration::minutes(i64::from(index)),
            creature: None,
            moment: crate::JournalMoment::Arrival,
        });
    }
    // Kept from the recent end of the journal, which is the part a trim leaves behind.
    save.companion.pins = save.companion.journal[160..]
        .iter()
        .map(crate::PinnedMoment::of)
        .collect();
    save.companion.scrapbook = (0..=u8::MAX)
        .chain(0..40)
        .map(|variant| crate::ScrapbookRecord {
            variant,
            first_at: now,
            finder: None,
            finder_name: format!("Finder {variant}"),
        })
        .collect();
    save.companion.schedule.transitions = (0..60u16)
        .map(|index| crate::ScheduledTransition {
            days: 0b111_1111,
            minute: index * 40,
            preset: (index % 3) as u8,
        })
        .collect();
    save.companion.appearance.text_scale = 240;
    save.companion.modes[0] = Some(crate::BehaviorPreset {
        habitat: over_full_habitat(),
        window_ledges: true,
        cursor_reactions: true,
        reduce_motion: false,
    });
    save.objects.objects = (0..40)
        .map(|index| crate::ColonyObject {
            id: index,
            kind: crate::ColonyObjectKind::ALL[index as usize % 8],
            role: crate::ColonyObjectKind::ALL[index as usize % 8].default_role(),
            ..Default::default()
        })
        .collect();
    save.home.unlocks.decorations = crate::ShelterDecorationKind::ALL
        .iter()
        .copied()
        .cycle()
        .take(90)
        .collect();
    save.home.dressing = (0..40)
        .map(|keeper| crate::HouseDressing {
            keeper: keeper % 9,
            decorations: crate::ShelterDecorationKind::ALL.to_vec(),
        })
        .collect();

    // The same oversized collections, arriving as a current file and as a version-13 one.
    for version in [crate::SAVE_VERSION, 13] {
        let directory =
            std::env::temp_dir().join(format!("formiga-caps-{version}-{}", std::process::id()));
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join("colony.json");
        let mut value = serde_json::to_value(&save).unwrap();
        value["save_version"] = version.into();
        fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        let loaded = crate::World::from_save(SaveStore::new(&path).load().unwrap().unwrap());
        let companion = &loaded.save.companion;
        assert_eq!(companion.journal.len(), crate::MAX_JOURNAL_ENTRIES);
        // A version-13 colony has no keepsakes to cap: it is given empty ones.
        let keepsakes = version == crate::SAVE_VERSION;
        assert_eq!(
            companion.pins.len(),
            if keepsakes {
                crate::MAX_PINNED_ENTRIES
            } else {
                0
            }
        );
        assert!(
            companion
                .pins
                .iter()
                .all(|pin| companion.journal.iter().any(|entry| pin.names(entry))),
            "every pin still names a moment the journal holds"
        );
        assert_eq!(
            companion.scrapbook.len(),
            if keepsakes {
                usize::from(crate::TRINKET_VARIANTS)
            } else {
                0
            }
        );
        assert!(
            companion
                .scrapbook
                .windows(2)
                .all(|pair| pair[0].variant < pair[1].variant),
            "one record per variant, in order"
        );
        assert_eq!(
            companion.schedule.transitions.len(),
            if keepsakes {
                crate::MAX_SCHEDULED_TRANSITIONS
            } else {
                0
            }
        );
        assert!(
            companion
                .schedule
                .transitions
                .iter()
                .all(|row| row.minute < 1440 && row.preset < 2 && row.days != 0)
        );
        assert_eq!(
            companion.appearance.text_scale,
            if keepsakes { 150 } else { 100 }
        );
        for mode in companion.modes.iter().flatten() {
            assert!(mode.habitat.zones.len() <= crate::MAX_HABITAT_ZONES);
        }
        assert!(loaded.save.objects.objects.len() <= crate::MAX_COLONY_OBJECTS);
        assert!(
            loaded.save.home.unlocks.decorations.len() <= crate::ShelterDecorationKind::ALL.len()
        );
        assert!(loaded.save.home.dressing.len() <= crate::MAX_COLONY_CREATURES);
        for dressing in &loaded.save.home.dressing {
            assert!(dressing.decorations.len() <= crate::MAX_HOUSE_DECORATIONS);
        }
        assert!(loaded.save.relationships.len() <= crate::MAX_RELATIONSHIPS);
        for creature in &loaded.save.creatures {
            assert!(creature.routines.len as usize <= crate::MAX_ROUTINES);
        }

        // What came back fits, and writing it out and reading it again changes nothing more.
        let round_trip = SaveStore::new(directory.join("round-trip.json"));
        round_trip.save(&loaded.save).unwrap();
        assert_eq!(
            round_trip.load().unwrap(),
            Some(ValidatedSave::from(loaded.save.clone()))
        );
        // The backup the second write leaves behind is a colony too, not a broken file.
        round_trip.save(&loaded.save).unwrap();
        assert_eq!(
            round_trip
                .load_path(&round_trip.path().with_extension("json.bak"))
                .unwrap(),
            loaded.save
        );
        let _ = fs::remove_dir_all(directory);
    }
}

fn over_full_habitat() -> crate::HabitatPolicy {
    crate::HabitatPolicy {
        preset: crate::HabitatPreset::Custom,
        zones: (0..80)
            .map(|id| crate::HabitatZone {
                id,
                display: crate::DisplayKey([1; 16]),
                normalized_bounds: crate::DesktopRect {
                    x: 0.0,
                    y: 0.0,
                    width: 1.0,
                    height: 1.0,
                },
                kind: crate::HabitatZoneKind::Allowed,
                enabled: true,
            })
            .collect(),
    }
}

fn collect_field_names(value: &serde_json::Value, names: &mut std::collections::BTreeSet<String>) {
    match value {
        serde_json::Value::Object(fields) => {
            for (name, child) in fields {
                names.insert(name.clone());
                collect_field_names(child, names);
            }
        }
        serde_json::Value::Array(items) => {
            items
                .iter()
                .for_each(|item| collect_field_names(item, names));
        }
        _ => {}
    }
}

fn collect_fields<'a>(value: &'a serde_json::Value, name: &str) -> Vec<&'a serde_json::Value> {
    let mut found = Vec::new();
    match value {
        serde_json::Value::Object(fields) => {
            for (field, child) in fields {
                if field == name {
                    found.push(child);
                }
                found.extend(collect_fields(child, name));
            }
        }
        serde_json::Value::Array(items) => {
            items
                .iter()
                .for_each(|item| found.extend(collect_fields(item, name)));
        }
        _ => {}
    }
    found
}

fn collect_numbers(value: &serde_json::Value, numbers: &mut Vec<f32>) {
    match value {
        serde_json::Value::Number(number) => {
            numbers.extend(number.as_f64().map(|number| number as f32));
        }
        serde_json::Value::Object(fields) => {
            fields
                .values()
                .for_each(|child| collect_numbers(child, numbers));
        }
        serde_json::Value::Array(items) => {
            items.iter().for_each(|item| collect_numbers(item, numbers));
        }
        _ => {}
    }
}

/// How long the busy colony is given to have everything happening at once. The scene is
/// deterministic, so it always reaches that moment at the same tick.
const BUSY_STEPS: u64 = 4_000;
const LEDGE_WINDOW: crate::WindowKey = 0x00C0_FFEE;
const DRIFTING_WINDOW: crate::WindowKey = 0x00BA_DBED;

/// A colony forty days old with a shared ledge, a companion crossing the floor, a window
/// sliding about, and a cursor sweeping past. It is driven until all of that is true at once,
/// and returns the tick it stopped on so the desktop it saw can be reconstructed.
fn colony_in_the_middle_of_everything() -> (crate::World, u64) {
    let created = datetime!(2026-01-01 0:00 UTC);
    let now = created + time::Duration::days(40);
    let mut desktop = observed_desktop(0);
    let mut world = crate::World::new([92; 32], created, &desktop);
    world.tick(now, 0.05, &desktop);
    // Out of the house, so the colony is living on the desktop rather than walking home.
    world.save.home.active_since_utc = None;
    world.save.home.last_disappeared_utc = Some(now);
    world.save.ritual.next_at_utc = now + time::Duration::days(1);
    // A companion its owner would rather keep on the floor.
    world.save.creatures[0].leaning = crate::RoamingLeaning::FloorDweller;
    world.save.settings.habitat = crate::HabitatPolicy {
        preset: crate::HabitatPreset::Custom,
        zones: vec![crate::HabitatZone {
            id: 1,
            display: crate::DisplayKey([1; 16]),
            normalized_bounds: crate::DesktopRect {
                x: 0.0,
                y: 0.05,
                width: 1.0,
                height: 0.95,
            },
            kind: crate::HabitatZoneKind::Allowed,
            enabled: true,
        }],
    };
    for (index, creature) in world.save.creatures.iter_mut().enumerate() {
        creature.state.arrival_delay_secs = 0.0;
        creature.state.action = crate::ActionKind::Idle;
        creature.state.action_elapsed = 0.0;
        creature.state.action_duration = 100.0;
        creature.state.drives = crate::Drives::default();
        creature.state.facing_right = index % 2 == 0;
        creature.personality.curiosity = 1.0;
        creature.personality.sociability = 1.0;
        creature.personality.playfulness = 1.0;
        creature.personality.cursor_interest = 1.0;
        creature.personality.boldness = if index == 1 { 1.0 } else { 0.4 };
        creature.personality.window_tolerance = creature.personality.boldness;
        if index < 2 {
            creature.state.surface = crate::SurfaceAttachment {
                kind: crate::SurfaceKind::WindowLedge,
                monitor_id: 1,
                window_key: Some(LEDGE_WINDOW),
                relative_x: (150.0 + index as f32 * 80.0) / 517.3125,
            };
            creature.state.position = crate::Point {
                x: 300.3125 + 150.0 + index as f32 * 80.0,
                y: 600.3125,
            };
        } else {
            creature.state.surface = crate::SurfaceAttachment {
                kind: crate::SurfaceKind::ScreenFloor,
                monitor_id: 1,
                window_key: None,
                relative_x: 0.5,
            };
            creature.state.position = crate::Point {
                x: 980.0 + (index - 2) as f32 * 90.0,
                y: 846.0,
            };
        }
    }
    world.tick(now, 0.05, &desktop);
    world.drain_events().for_each(drop);
    // Two observers fed the same scans, so the test can say what the colony is looking at
    // without reaching into it.
    let mut geometry = crate::attention::GeometryObserver::default();
    let mut cursor = crate::cursor::CursorObserver::default();
    geometry.update(&desktop, 0.05, true);
    cursor.update(&desktop, 0.05, true);
    for step in 1..=BUSY_STEPS {
        desktop = observed_desktop(step);
        world.tick(
            now + time::Duration::milliseconds(step as i64 * 50),
            0.05,
            &desktop,
        );
        world.drain_events().for_each(drop);
        geometry.update(&desktop, 0.05, true);
        cursor.update(&desktop, 0.05, true);
        let together = world.save.creatures.iter().any(|creature| {
            matches!(
                creature.state.action,
                crate::ActionKind::Greet
                    | crate::ActionKind::Follow
                    | crate::ActionKind::SocialPlay
                    | crate::ActionKind::SoloPlay
                    | crate::ActionKind::Sprint
            )
        });
        let travelling = world.save.creatures.iter().any(|creature| {
            matches!(
                creature.state.action,
                crate::ActionKind::Traverse
                    | crate::ActionKind::ClimbWindow
                    | crate::ActionKind::RideWindow
                    | crate::ActionKind::Dangle
            )
        });
        let watching = world
            .save
            .creatures
            .iter()
            .any(|creature| creature.state.attention.is_some());
        if together
            && travelling
            && watching
            && geometry.signals().next().is_some()
            && cursor.cue().is_some()
        {
            // A companion with two little habits of its own, one of them in the journal,
            // and another in the middle of doing one.
            world.save.creatures[0].memory.habits =
                vec![crate::Habit::StretchesBeforeNaps, crate::Habit::WavesHello];
            let first = world.save.creatures[0].id;
            world.save.companion.remember(
                Some(first),
                crate::JournalMoment::Habit(crate::Habit::WavesHello),
                now,
            );
            // Two spots put down on the village ground.
            world
                .save
                .home
                .set_hangout(crate::HangoutKind::Blanket, Some(0.3125));
            world
                .save
                .home
                .set_hangout(crate::HangoutKind::Lookout, Some(0.8125));
            // A village arranged by hand: the cottage order written down when there were more
            // cottages to order, painted in a named palette, with a herb box planted.
            world.save.home.cottage_order =
                crate::house_owners(&world.save.creatures, &[]).as_slice()[1..].to_vec();
            world.save.home.palette = Some(crate::VillagePalette::Harbour);
            world
                .save
                .home
                .set_garden(crate::GardenKind::Herbs, Some(0.5625), now);
            // An ornament set out, a decoration hung, a keepsake chosen for the trees, and a
            // companion wearing a pin.
            world
                .save
                .home
                .set_ornament(crate::OrnamentKind::BirdBath, Some(0.6875));
            world
                .save
                .companion
                .remember_discovery(2, first, "Finder".into(), now);
            let mut hooks = [None; crate::TREE_HOOKS];
            hooks[3] = Some(2);
            world.save.home.set_tree_keepsakes(Some(hooks));
            world.save.creatures[0].accessory = Some(crate::Accessory::Pin(2));
            // And the colony house built as a pillow fort.
            let founder = world.save.creatures[0].id;
            world
                .save
                .home
                .set_house_style(founder, Some(crate::ShelterStyle::PillowFort));
            world.save.home.set_decoration(
                founder,
                crate::DecorationSlot::Eaves,
                Some(crate::ShelterDecorationKind::Banner),
            );
            // Another being towed out of the way in its sleep.
            world.save.creatures[0].state.nudge = Some(crate::SleepNudge::Towed {
                by: world.save.creatures[1].id,
            });
            let last = world.save.creatures.last_mut().expect("a colony");
            last.state.flourish = Some(crate::Flourish {
                habit: crate::Habit::LooksFoodOver,
                action: last.state.action,
                started_at: Some(0.25),
            });
            return (world, step);
        }
    }
    panic!("the colony never had a scene, a journey, a cursor, and a cue all at once");
}

fn observed_desktop(step: u64) -> crate::DesktopSnapshot {
    // Odd fractions, so nothing that reached the file could have come from anywhere else.
    let drift = (step % 8) as f32 * 30.0;
    crate::DesktopSnapshot {
        monitors: vec![crate::MonitorInfo {
            id: 1,
            display_key: crate::DisplayKey([1; 16]),
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
        windows: vec![
            crate::DesktopWindow {
                key: LEDGE_WINDOW,
                bounds: crate::DesktopRect {
                    x: 300.3125,
                    y: 600.3125,
                    width: 517.3125,
                    height: 233.3125,
                },
                z_order: 0,
                visible: true,
                minimized: false,
                application: None,
                application_name: None,
            },
            crate::DesktopWindow {
                key: DRIFTING_WINDOW,
                bounds: crate::DesktopRect {
                    x: 902.3125 - drift,
                    y: 380.3125,
                    width: 421.3125,
                    height: 186.3125,
                },
                z_order: 1,
                visible: true,
                minimized: false,
                application: None,
                application_name: None,
            },
        ],
        cursor: crate::CursorSnapshot {
            position: crate::Point {
                x: 1301.3125 - drift,
                y: 96.8125,
            },
            velocity: crate::Point {
                x: -321.8125,
                y: 123.8125,
            },
            available: true,
        },
        window_sample: Some(crate::WindowSample {
            monotonic_millis: step * 50,
            reliable: true,
        }),
        cursor_sample_millis: Some(step * 50),
        ..Default::default()
    }
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

/// A tally and a read marker survive a round trip exactly.
#[test]
fn a_counted_tally_and_the_read_marker_round_trip() {
    let mut save = example_save();
    let at = datetime!(2026-09-30 9:00 UTC);
    let pair = crate::tally_mut_or_insert(&mut save.tallies, 7, 3).unwrap();
    assert_eq!((pair.a, pair.b), (3, 7));
    pair.tally
        .count(Some(false), crate::RelationshipExperience::Followed, at);
    pair.tally
        .count(None, crate::RelationshipExperience::SharedRest, at);
    save.companion.journal_seen_until = Some(at);
    let text = serde_json::to_string(&save).unwrap();
    let reread: SaveFile = serde_json::from_str(&text).unwrap();
    assert_eq!(reread, save);
    let tally = crate::tally_between(&reread.tallies, 7, 3).unwrap().tally;
    assert_eq!(tally.sought, [0, 1]);
    assert_eq!(tally.shared_rests, 1);
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
fn recovery_preserves_corrupt_files_and_does_not_rotate_over_good_backup() {
    let directory = std::env::temp_dir().join(format!("formiga-recovery-{}", std::process::id()));
    fs::create_dir_all(&directory).unwrap();
    let store = SaveStore::new(directory.join("colony.json"));
    let save = example_save();
    store.save(&save).unwrap();
    store.save(&save).unwrap();
    fs::write(store.path(), b"unreadable original").unwrap();
    let restored = store.load().unwrap().unwrap();
    store.save(&restored).unwrap();
    assert_eq!(store.load_path(&store.backup_path()).unwrap(), save);
    let copies: Vec<_> = fs::read_dir(&directory)
        .unwrap()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().contains("recovery-"))
        .collect();
    assert!(
        copies
            .iter()
            .any(|e| fs::read(e.path()).unwrap() == b"unreadable original")
    );
    fs::remove_file(store.path()).unwrap();
    assert_eq!(store.load().unwrap().unwrap(), save);
    fs::remove_dir_all(directory).unwrap();
}
#[test]
fn full_snapshot_round_trip_retains_keepsakes_journal_modes_and_quiet_time() {
    let path = std::env::temp_dir().join(format!("formiga-snapshot-{}.json", std::process::id()));
    let now = datetime!(2026-09-14 12:00 UTC);
    let mut world = crate::World::new([63; 32], now, &crate::DesktopSnapshot::default());
    world.set_quiet_mode(30, now);
    let founder = world.save.creatures[0].id;
    world.save.home.set_decoration(
        founder,
        crate::DecorationSlot::WallRight,
        Some(crate::ShelterDecorationKind::Lamp),
    );
    world.save.companion.modes[0] = Some(crate::BehaviorPreset::capture(&world.save.settings));
    SaveStore::new(&path).save(&world.save).unwrap();
    assert_eq!(SaveStore::read_snapshot(&path).unwrap(), world.save);
    let before = fs::read(&path).unwrap();
    let _ = SaveStore::read_snapshot(&path).unwrap();
    assert_eq!(fs::read(&path).unwrap(), before);
    fs::remove_file(&path).unwrap();
}

fn example_save() -> SaveFile {
    let mut home = crate::ColonyHome::default();
    home.unlocks.next_at_utc = datetime!(2026-01-05 0:00 UTC);
    SaveFile {
        companion: crate::CompanionState::default(),
        save_version: crate::SAVE_VERSION,
        colony_seed: [1; 32],
        created_at_utc: datetime!(2026-01-01 0:00 UTC),
        maximum_seen_utc: datetime!(2026-01-01 0:00 UTC),
        arrival_state: ArrivalState::default(),
        home,
        settings: Settings::default(),
        creatures: Vec::new(),
        relationships: Vec::new(),
        tallies: Vec::new(),
        ritual: crate::RitualState {
            next_at_utc: datetime!(2026-01-02 0:00 UTC),
            ..crate::RitualState::default()
        },
        objects: crate::ColonyObjectState {
            next_at_utc: datetime!(2026-01-04 0:00 UTC),
            ..crate::ColonyObjectState::default()
        },
        visitors: crate::VisitorState::default(),
        finds_today: crate::FindsToday::default(),
    }
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

/// A shared code for somebody who has never lived here.
fn a_friends_code() -> crate::SharedCreatureSeed {
    crate::SharedCreatureSeed {
        source_colony_seed: [70; 32],
        source_generation: 1,
        design: Some(crate::CreatureDesign::generated([70; 32], 1, None)),
    }
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

#[test]
fn routine_movement_waits_for_a_checkpoint_and_everything_else_is_kept_at_once() {
    let seconds = Duration::from_secs;
    assert!(save_due(SaveUrgency::Prompt, Duration::ZERO));
    assert!(!save_due(SaveUrgency::Routine, seconds(14)));
    assert!(save_due(SaveUrgency::Routine, ROUTINE_CHECKPOINT));
    assert!(!save_due(SaveUrgency::None, seconds(29)));
    assert!(save_due(SaveUrgency::None, PERIODIC_SAVE));
    assert!(ROUTINE_CHECKPOINT < PERIODIC_SAVE);
    let id = 7;
    for (event, urgency) in [
        (
            crate::WorldEvent::CreatureSpawned { creature_id: id },
            SaveUrgency::Prompt,
        ),
        (crate::WorldEvent::HomeAppeared, SaveUrgency::Prompt),
        (
            crate::WorldEvent::HomeDisappeared { interrupted: false },
            SaveUrgency::Prompt,
        ),
        (
            crate::WorldEvent::ActionStarted {
                creature_id: id,
                action: crate::ActionKind::Traverse,
            },
            SaveUrgency::Routine,
        ),
        (
            crate::WorldEvent::SurfaceChanged {
                creature_id: id,
                kind: crate::SurfaceKind::WindowLedge,
            },
            SaveUrgency::Routine,
        ),
        (
            crate::WorldEvent::CreaturePetted { creature_id: id },
            SaveUrgency::None,
        ),
    ] {
        assert_eq!(event.save_urgency(), urgency, "{event:?}");
    }
    // The most urgent thing waiting decides.
    assert_eq!(
        SaveUrgency::Routine.max(SaveUrgency::Prompt),
        SaveUrgency::Prompt
    );
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
fn a_colony_written_mid_visit_holds_the_guest_but_never_the_visit() {
    let desktop = crate::DesktopSnapshot::default();
    let now = datetime!(2026-09-14 12:00 UTC);
    let mut world = crate::World::new([66; 32], now, &desktop);
    world
        .invite_visitor(a_friends_code(), now, &desktop)
        .unwrap();
    // A full book, of both kinds, and a guest out on the desktop mid-scene.
    for index in 0..40u32 {
        world.save.visitors.sign(crate::GuestBookEntry {
            visited_at_utc: now + time::Duration::hours(i64::from(index)),
            name: format!("Guest {index}"),
            origin: crate::CreatureOrigin {
                design: None,
                source_colony_seed: [index as u8; 32],
                source_generation: index as u8 % 4,
            },
            source: if index % 2 == 0 {
                crate::VisitorSource::Wanderer
            } else {
                crate::VisitorSource::Invited
            },
        });
    }
    // Favorites kept to invite again, up to the limit.
    for index in 0..crate::MAX_FAVORITE_VISITORS as u8 {
        let origin = crate::CreatureOrigin {
            design: None,
            source_colony_seed: [100 + index; 32],
            source_generation: index % 4,
        };
        world
            .save
            .visitors
            .keep_favorite(&format!("Friend {index}"), origin, now)
            .unwrap();
    }
    let guest = world
        .save
        .visitors
        .guest
        .as_mut()
        .expect("a friend is here");
    guest.on_stage = true;
    guest.signed = true;
    guest.visit = crate::VisitProgress {
        phase: crate::VisitPhase::Visiting,
        elapsed: 12.5,
        since_home: 40.0,
        greeted: true,
        since_hello: 8.0,
        doorway: Some(crate::Point { x: 640.5, y: 846.0 }),
        beat: 3,
        beat_remaining: 2.25,
        answers: vec![crate::ResidentAnswer {
            creature_id: 7,
            after: 1.0,
            hold: 2.0,
            gesture: Some(crate::Gesture::Bop),
            bubble: None,
        }],
        // The walk round the village is a scene too, and is no more saved than the rest.
        stops: vec![crate::TourStop {
            at: crate::Point { x: 700.0, y: 846.0 },
            look: crate::Point { x: 712.0, y: 846.0 },
            interest: crate::TourInterest::Keepsake,
        }],
        stop: 1,
        moment: crate::TourMoment::Greeting(7),
        stay: 4.5,
        planned: Some(crate::Point { x: 660.0, y: 846.0 }),
        met: vec![7],
    };
    let expected = world.save.visitors.guest.clone().expect("a friend is here");
    world
        .save
        .companion
        .remember(None, crate::JournalMoment::Visit("Wren".into()), now);

    let directory = std::env::temp_dir().join(format!("formiga-visit-{}", std::process::id()));
    let store = SaveStore::new(directory.join("colony.json"));
    store.save(&world.save).unwrap();
    let value: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(store.path()).unwrap()).unwrap();
    let mut names = std::collections::BTreeSet::new();
    collect_field_names(&value, &mut names);
    for name in &names {
        assert!(
            SAVED_FIELDS.contains(&name.as_str()),
            "a colony with a guest names {name:?}, which no colony file is meant to hold"
        );
    }
    for absent in RUNTIME_ONLY_FIELDS {
        assert!(!names.contains(absent), "the file names {absent:?}");
    }
    assert!(names.contains("guest"), "the sweep is reading the file");
    assert!(names.contains("Visit"), "the visit is in the journal");

    // Reopening finds the friend still staying, waiting for the next gathering rather than
    // halfway through the visit it was in the middle of.
    let loaded = store.load().unwrap().unwrap();
    assert_eq!(
        loaded.visitors.guest_book.len(),
        crate::MAX_GUEST_BOOK_ENTRIES
    );
    assert_eq!(loaded.visitors.guest_book, world.save.visitors.guest_book);
    let reopened = loaded.visitors.guest.clone().expect("still staying");
    assert_eq!(reopened.creature, expected.creature);
    assert_eq!(reopened.source, expected.source);
    assert_eq!(reopened.stays_until_utc, expected.stays_until_utc);
    assert!(reopened.signed);
    assert_eq!(reopened.visit, crate::VisitProgress::default());
    let world = crate::World::from_save(loaded);
    assert!(world.save.visitors.on_stage().is_none());
    assert!(world.save.visitors.guest.is_some());
    let _ = fs::remove_dir_all(directory);
}

#[test]
fn keepsakes_stay_bounded_and_a_pin_only_ever_names_a_real_moment() {
    let now = datetime!(2026-02-01 9:00 UTC);
    let mut state = crate::CompanionState::default();
    let entry = crate::JournalEntry {
        at: now,
        creature: None,
        moment: crate::JournalMoment::Arrival,
    };
    for index in 0..20u8 {
        let moment = crate::JournalEntry {
            at: now + time::Duration::minutes(i64::from(index)),
            ..entry.clone()
        };
        state.journal.push(moment.clone());
        state.pin(&moment);
    }
    assert_eq!(state.pins.len(), crate::MAX_PINNED_ENTRIES);
    assert!(
        !state.pin(&state.journal[0].clone()),
        "pinning twice does nothing"
    );
    assert!(state.pinned(&state.journal[0]));
    state.unpin(&state.journal[0].clone());
    assert!(!state.pinned(&state.journal[0]));
    // Every pin still names an entry the journal actually holds.
    assert!(
        state
            .pins
            .iter()
            .all(|pin| state.journal.iter().any(|entry| pin.names(entry)))
    );
    // One record per variant, however often a trinket is found again.
    for round in 0..3 {
        for variant in 0..crate::TRINKET_VARIANTS + 4 {
            state.remember_discovery(
                variant,
                7,
                format!("Finder {round}"),
                now + time::Duration::hours(i64::from(round)),
            );
        }
    }
    assert_eq!(state.scrapbook.len(), usize::from(crate::TRINKET_VARIANTS));
    assert!(
        state
            .scrapbook
            .iter()
            .all(|record| record.finder_name == "Finder 0"),
        "the first finder is the one the scrapbook keeps"
    );
    state.schedule.transitions = vec![
        crate::ScheduledTransition {
            days: 0,
            minute: 10,
            preset: 0,
        },
        crate::ScheduledTransition {
            days: 1,
            minute: 2000,
            preset: 0,
        },
        crate::ScheduledTransition {
            days: 1,
            minute: 10,
            preset: 5,
        },
        crate::ScheduledTransition {
            days: 127,
            minute: 540,
            preset: 1,
        },
    ];
    state.appearance.text_scale = 233;
    state.normalize();
    assert_eq!(
        state.schedule.transitions.len(),
        1,
        "invalid rows are dropped"
    );
    assert_eq!(state.appearance.text_scale, 150);
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
fn round_trips_atomically() {
    let directory = std::env::temp_dir().join(format!("formiga-save-{}", std::process::id()));
    let path = directory.join("colony.json");
    let store = SaveStore::new(&path);
    store.save(&example_save()).unwrap();
    assert_eq!(
        store.load().unwrap(),
        Some(ValidatedSave::from(example_save()))
    );
    let mut replacement = example_save();
    replacement.settings.reduce_motion = true;
    store.save(&replacement).unwrap();
    assert_eq!(
        store.load().unwrap(),
        Some(ValidatedSave::from(replacement))
    );
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

    assert_eq!(store.load().unwrap(), Some(ValidatedSave::from(original)));
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

/// Every place in a JSON document, as pointers.
fn json_pointers(value: &serde_json::Value, at: String, pointers: &mut Vec<String>) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, child) in map {
                json_pointers(child, format!("{at}/{key}"), pointers);
            }
        }
        serde_json::Value::Array(items) => {
            for (index, child) in items.iter().enumerate() {
                json_pointers(child, format!("{at}/{index}"), pointers);
            }
        }
        _ => {}
    }
    pointers.push(at);
}

/// Damage one place in a colony file the way a bad copy, a hand edit or a stray tool might:
/// mostly keeping each value's kind, so the damage reaches what validation repairs, and now and
/// then not, so it reaches what parsing refuses.
fn damage(value: &mut serde_json::Value, rng: &mut impl rand::Rng) {
    use serde_json::{Value, json};
    let mut pointers = Vec::new();
    json_pointers(value, String::new(), &mut pointers);
    let at = &pointers[rng.random_range(0..pointers.len())];
    let Some(place) = value.pointer_mut(at) else {
        return;
    };
    let choice = rng.random_range(0..8);
    *place = match place.take() {
        Value::Array(mut items) => {
            match choice {
                0..=2 if !items.is_empty() => {
                    let item = items[rng.random_range(0..items.len())].clone();
                    items.extend(std::iter::repeat_n(item, rng.random_range(1..40)));
                }
                3 => items.clear(),
                4 => items.reverse(),
                _ => items.truncate(items.len() / 2),
            }
            Value::Array(items)
        }
        Value::Object(mut map) if choice < 6 && !map.is_empty() => {
            let key = map.keys().nth(rng.random_range(0..map.len())).cloned();
            if let Some(key) = key {
                map.remove(&key);
            }
            Value::Object(map)
        }
        Value::Number(number) => match choice {
            0 => json!(0),
            1 => json!(255),
            2 if number.is_f64() => json!(f64::MAX),
            3 if number.is_f64() => json!(-1.5),
            4 => json!(u32::MAX),
            5 => json!(-1),
            _ => json!(number.as_u64().map_or(0, |n| n.wrapping_mul(7) % 256)),
        },
        Value::String(text) => match choice {
            0..=2 => json!(""),
            3 => json!(text.repeat(20)),
            4 => json!("\u{7}\u{0}"),
            _ => json!(format!("  {text}  ")),
        },
        Value::Bool(flag) => json!(!flag),
        _ => json!(null),
    };
}

/// However a colony file is damaged, reading it either refuses it with an error or gives back a
/// colony holding nothing a validated one never does — and that colony opens and runs. Nothing
/// is ever half read.
#[test]
fn a_damaged_file_is_refused_or_repaired_never_half_read() {
    use rand::SeedableRng;
    let (world, _) = colony_in_the_middle_of_everything();
    let value = serde_json::to_value(&world.save).unwrap();
    let mut rng = rand_chacha::ChaCha12Rng::seed_from_u64(0x0663);
    let desktop = observed_desktop(0);
    let (mut refused, mut repaired) = (0, 0);
    for round in 0..600 {
        let mut damaged = value.clone();
        for _ in 0..1 + round % 3 {
            damage(&mut damaged, &mut rng);
        }
        let bytes = serde_json::to_vec(&damaged).unwrap();
        let Ok(save) = decode(&bytes) else {
            refused += 1;
            continue;
        };
        repaired += 1;
        assert_eq!(violations(&save), Vec::<String>::new(), "round {round}");
        let mut opened = crate::World::from_save(save);
        let now = opened.save.maximum_seen_utc;
        for step in 1..=20 {
            opened.tick(
                now + time::Duration::milliseconds(50 * step),
                0.05,
                &desktop,
            );
        }
    }
    eprintln!("{refused} damaged files refused, {repaired} repaired");
    assert!(
        refused > 0 && repaired > 0,
        "the damage reaches both outcomes"
    );
}
