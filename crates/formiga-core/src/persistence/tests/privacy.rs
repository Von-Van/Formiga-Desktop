//! Privacy: a colony file holds only the field names a colony keeps, and nothing of the desktop
//! it was watching or a scene or visit under way when it was written.
use super::*;

/// Every field name a version-17 colony file is allowed to use, gathered from a colony that
/// has one of everything. The list is long on purpose: an observation that reached the save
/// would have to bring a name with it, and this is what notices.
const SAVED_FIELDS: [&str; 331] = [
    "Battle",
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
    "battle",
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
    "circuit_points",
    "classic",
    "climbing",
    "club",
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
    "creature_id",
    "creatures",
    "crown",
    "cup",
    "cups",
    "curiosity",
    "curiosity_satisfaction",
    "cursor_cooldown",
    "cursor_interest",
    "cursor_reactions",
    "cursor_trust",
    "day",
    "day_book",
    "days",
    "decision_temperature",
    "decorations",
    "depth",
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
    "drawn",
    "dressing",
    "drives",
    "ear_size",
    "earned_at_utc",
    "ears",
    "effect_motif",
    "enabled",
    "ended_by",
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
    "field",
    "finder",
    "finder_name",
    "finished_at_utc",
    "first_at",
    "foot_size",
    "forelimbs",
    "formed_at_utc",
    "friends",
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
    "handler",
    "handler_key",
    "hangouts",
    "hatch_day_acknowledged_year",
    "head",
    "head_appendages",
    "head_ratio",
    "height",
    "highlight_style",
    "history",
    "home",
    "home_affinity",
    "home_seconds",
    "home_visits",
    "hooks",
    "horns",
    "house_depths",
    "house_styles",
    "id",
    "impulsiveness",
    "journal",
    "journal_seen_until",
    "keeper",
    "keepsake",
    "keepsakes",
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
    "lineups",
    "logical_size",
    "longest_sleep_seconds",
    "marking",
    "marking_seed",
    "matches",
    "maximum_seen_utc",
    "members",
    "members_since_utc",
    "memory",
    "milestone_bubble_shown",
    "milestone_cooldown_active_seconds",
    "mini_arrivals",
    "minute",
    "mode",
    "modes",
    "moment",
    "moments",
    "monitor_id",
    "mouth_style",
    "muzzle",
    "name",
    "next_at_utc",
    "normalized_bounds",
    "normalized_position",
    "notebook",
    "objects",
    "on_stage",
    "onboarding_complete",
    "opponent",
    "opponent_pennant",
    "opponent_trio",
    "ordinal",
    "origin",
    "ornaments",
    "overridden",
    "pairs",
    "palette",
    "palette_index",
    "parent_id",
    "pattern",
    "pattern_density",
    "paused",
    "pennant",
    "personality",
    "pins",
    "placements",
    "planted_at_utc",
    "play",
    "play_sessions",
    "played",
    "playfulness",
    "plays",
    "position",
    "preferred_region",
    "preset",
    "profile_revision",
    "pupil_style",
    "quiet_until",
    "recent",
    "record",
    "reduce_motion",
    "relationships",
    "relative_x",
    "rest_pose",
    "rests",
    "retired",
    "ritual",
    "role",
    "roof",
    "roundness",
    "routine",
    "routine_affinity",
    "routines",
    "save_version",
    "schedule",
    "scrapbook",
    "sculpt",
    "settings",
    "shape",
    "shared_rests",
    "shelter",
    "signed",
    "since",
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
    "steps",
    "strength",
    "style",
    "surface",
    "suspicion",
    "symbol",
    "tail",
    "tail_length",
    "tail_style",
    "tallies",
    "tally",
    "team",
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
    "trim",
    "trio",
    "trios",
    "turns",
    "unlocked",
    "unlocks",
    "until_utc",
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
    "wins",
    "won",
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
        "house_depths",
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

fn collect_field_names(value: &serde_json::Value, names: &mut std::collections::BTreeSet<String>) {
    match value {
        serde_json::Value::Object(fields) => {
            for (name, child) in fields {
                names.insert(name.clone());
                // A sculpted form is the design model's own vocabulary, held to it by the model's
                // own tests: its `plan` is a body plan, not a plan under way.
                if name != "sculpt" {
                    collect_field_names(child, names);
                }
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
