//! The projection from Desktop's colony to a travel snapshot: deterministic, complete enough to
//! draw every traveler exactly, and holding nothing about the desktop the colony lives on.

mod common;

use common::{MADE, colony};
use formiga_art::{AccessoryArt, BodyClip, CreatureRenderer, MotionSignature, palette_for};
use formiga_core::{ActionKind, Edition, Gesture};
use formiga_travel::*;
use serde_json::Value;

fn session() -> SessionId {
    SessionId::parse("0f1e2d3c4b5a69788796a5b4c3d2e1f0").unwrap()
}

fn snapshot_of(save: &formiga_core::SaveFile) -> TravelSnapshot {
    project_colony(save, session(), MADE, "0.66.1").expect("a colony can be described")
}

#[test]
fn the_same_colony_gives_the_same_snapshot_byte_for_byte() {
    for seed in [3, 41, 200] {
        let save = colony(seed);
        let first = encode(&snapshot_of(&save)).unwrap();
        let second = encode(&snapshot_of(&save.clone())).unwrap();
        assert_eq!(first, second, "colony {seed}");
        let read: TravelSnapshot = decode(&first).unwrap();
        assert_eq!(
            read,
            snapshot_of(&save),
            "colony {seed} reads back as itself"
        );
    }
}

#[test]
fn the_test_colony_has_one_of_everything() {
    let save = colony(3);
    let editions: Vec<_> = save
        .creatures
        .iter()
        .map(|creature| creature.appearance.design.map(|design| design.edition()))
        .collect();
    for wanted in [
        None,
        Some(Edition::Original),
        Some(Edition::Archetypes),
        Some(Edition::Details),
    ] {
        assert!(
            editions.contains(&wanted),
            "{wanted:?} missing from {editions:?}"
        );
    }
    assert!(
        save.creatures.iter().any(|creature| creature
            .appearance
            .design
            .is_some_and(|d| !d.classic.is_modular())),
        "a classic-parts recipe"
    );
    assert!(
        save.creatures
            .iter()
            .any(|creature| !creature.role.is_adult())
    );
    assert_eq!(save.creatures.len(), formiga_core::MAX_COLONY_CREATURES);
}

#[test]
fn every_look_crosses_exactly_and_draws_the_same() {
    for seed in [3, 41, 200] {
        let save = colony(seed);
        let snapshot: TravelSnapshot = decode(&encode(&snapshot_of(&save)).unwrap()).unwrap();
        assert_eq!(snapshot.travelers.len(), save.creatures.len());
        let members: Vec<_> = save
            .creatures
            .iter()
            .map(|creature| palette_for(&creature.appearance))
            .collect();
        for (creature, traveler) in save.creatures.iter().zip(&snapshot.travelers) {
            assert_eq!(traveler.id, TravelerId(creature.id));
            let genome = traveler.appearance.to_genome().unwrap();
            assert_eq!(genome, creature.appearance, "{}'s look", creature.name);
            let stand_in = traveler.to_creature().unwrap();
            assert_eq!(stand_in.appearance, creature.appearance);
            assert_eq!(
                stand_in.display_scale_percent,
                creature.display_scale_percent
            );
            assert_eq!(stand_in.role, creature.role);
            assert_eq!(stand_in.accessory, creature.accessory);
            assert_eq!(stand_in.memory.habits, creature.memory.habits);
            assert_eq!(stand_in.temperament().kind, creature.temperament().kind);
            assert_eq!(
                stand_in.traits(),
                creature.traits(),
                "{}'s traits",
                creature.name
            );
            assert_eq!(
                MotionSignature::for_creature(&stand_in),
                MotionSignature::for_creature(creature),
                "{} keeps its own pace",
                creature.name
            );
            assert_eq!(
                traveler
                    .motion
                    .celebration
                    .map(formiga_core::Celebration::from),
                Some(formiga_core::Celebration::for_creature(creature)),
                "{} celebrates its own way",
                creature.name
            );
            let home_dress = creature
                .accessory
                .map(|accessory| AccessoryArt::resolve(accessory, save.colony_seed, &members));
            let travel_dress = traveler.accessory.map(|accessory| accessory.to_art());
            assert_eq!(travel_dress, home_dress, "{}'s accessory", creature.name);
            for (clip, frame) in [
                (BodyClip::Action(ActionKind::Idle), 0),
                (BodyClip::Action(ActionKind::Traverse), 3),
                (BodyClip::Gesture(Gesture::Reach), 1),
            ] {
                let at_home = CreatureRenderer::render_dressed_body_frame(
                    &creature.appearance,
                    home_dress,
                    clip,
                    frame,
                    false,
                );
                let away = CreatureRenderer::render_dressed_body_frame(
                    &genome,
                    travel_dress,
                    clip,
                    frame,
                    false,
                );
                assert!(
                    at_home.canvas.pixels() == away.canvas.pixels(),
                    "{} is drawn differently away from home",
                    creature.name
                );
            }
        }
    }
}

/// Every key anywhere in a JSON document.
fn keys(value: &Value, into: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            for (key, inner) in map {
                into.push(key.clone());
                keys(inner, into);
            }
        }
        Value::Array(items) => items.iter().for_each(|item| keys(item, into)),
        _ => {}
    }
}

#[test]
fn the_snapshot_holds_nothing_about_the_desktop_or_the_colony_file() {
    let save = colony(41);
    let bytes = encode(&snapshot_of(&save)).unwrap();
    let value: Value = serde_json::from_slice(&bytes).unwrap();
    let mut found = Vec::new();
    keys(&value, &mut found);
    for desktop_only in [
        "position",
        "velocity",
        "surface",
        "monitor",
        "monitor_id",
        "display",
        "display_key",
        "window",
        "window_key",
        "windows",
        "cursor",
        "bounds",
        "usable_bounds",
        "drives",
        "routines",
        "memory",
        "tendencies",
        "behavior_seed",
        "colony_seed",
        "settings",
        "habitat",
        "journal",
        "home",
        "visitors",
        "state",
        "leaning",
        "save_version",
        "application_occlusion_rules",
        "kept",
    ] {
        assert!(
            !found.iter().any(|key| key == desktop_only),
            "the snapshot carries {desktop_only:?}"
        );
    }
    let top: std::collections::BTreeSet<_> = value
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        top,
        std::collections::BTreeSet::from([
            "format",
            "version",
            "min_reader_version",
            "session_id",
            "colony_id",
            "created_at_utc",
            "desktop_version",
            "capabilities",
            "travelers",
            "relationships",
            "presentation",
        ])
    );
    // The colony's own seed is never written, in any spelling.
    let text = String::from_utf8(bytes).unwrap();
    let seed_hex: String = save
        .colony_seed
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    assert!(!text.contains(&seed_hex));
}

#[test]
fn bonds_cross_as_bands_between_travelers_only() {
    let mut save = colony(200);
    let first = save.creatures[0].id;
    // A bond with somebody no longer in the colony is not carried.
    let mut stray = formiga_core::CreatureRelationship::new(first, 0xdead_beef).unwrap();
    stray.affinity = 200;
    save.relationships.push(stray);
    let snapshot = snapshot_of(&save);
    let pairs = save.creatures.len() * (save.creatures.len() - 1) / 2;
    assert_eq!(snapshot.relationships.len(), pairs);
    assert!(
        snapshot
            .relationships
            .windows(2)
            .all(|w| (w[0].a, w[0].b) < (w[1].a, w[1].b))
    );
    for pair in &snapshot.relationships {
        let bond = formiga_core::relationship_between(&save.relationships, pair.a.0, pair.b.0)
            .expect("the pair is in the colony");
        assert_eq!(pair.affinity, Band::of(bond.affinity));
        assert_eq!(pair.familiarity, Band::of(bond.familiarity));
        assert_eq!(pair.playfulness, Band::of(bond.playfulness));
        assert_eq!(pair.avoidance, Band::of(bond.avoidance));
    }
    let bands: std::collections::BTreeSet<_> = snapshot
        .relationships
        .iter()
        .map(|pair| pair.affinity)
        .collect();
    assert!(bands.len() >= 3, "the colony spans the bands: {bands:?}");
}

#[test]
fn names_and_profiles_are_plain_text_when_they_leave() {
    let mut save = colony(3);
    save.creatures[0].name = "\u{202E}wollaM".to_owned();
    save.creatures[1].name = "  Biscuit\u{2066}  ".to_owned();
    save.creatures[2].name = "\u{200B}".to_owned();
    let snapshot = snapshot_of(&save);
    assert_eq!(snapshot.travelers[0].name, "wollaM");
    assert_eq!(snapshot.travelers[1].name, "Biscuit");
    assert_eq!(snapshot.travelers[2].name, "Companion");
    for traveler in &snapshot.travelers {
        assert!(is_sanitized(
            &traveler.character.phrase,
            limits::MAX_PHRASE_CHARS
        ));
        assert_eq!(traveler.character.traits.len(), 3);
    }
}

#[test]
fn the_little_one_travels_with_its_adult() {
    let save = colony(3);
    let snapshot = snapshot_of(&save);
    let mini = snapshot
        .travelers
        .iter()
        .find(|traveler| matches!(traveler.role, TravelRole::Mini { .. }))
        .unwrap();
    let TravelRole::Mini { parent_id } = mini.role else {
        unreachable!()
    };
    assert_eq!(
        snapshot.traveler(parent_id).unwrap().role,
        TravelRole::Adult
    );
    assert!(mini.scale_percent < 100);
    assert!(
        mini.appearance.logical_size
            < snapshot
                .traveler(parent_id)
                .unwrap()
                .appearance
                .logical_size
    );
}

#[test]
fn the_owners_reduced_motion_goes_with_them() {
    let mut save = colony(3);
    assert!(!snapshot_of(&save).presentation.reduce_motion);
    save.settings.reduce_motion = true;
    assert!(snapshot_of(&save).presentation.reduce_motion);
}

#[test]
fn a_colony_with_nobody_in_it_is_not_sent() {
    let mut save = colony(3);
    save.creatures.clear();
    assert!(matches!(
        project_colony(&save, session(), MADE, "0.66.1"),
        Err(ProjectionError::Empty)
    ));
}

#[test]
fn a_snapshot_that_does_not_add_up_is_refused_both_ways() {
    let snapshot = snapshot_of(&colony(3));
    let mut cases: Vec<(&str, TravelSnapshot)> = Vec::new();
    let mut twin = snapshot.clone();
    twin.travelers[1].id = twin.travelers[0].id;
    cases.push(("twins", twin));
    let mut orphan = snapshot.clone();
    let mini = orphan
        .travelers
        .iter()
        .position(|t| matches!(t.role, TravelRole::Mini { .. }))
        .unwrap();
    orphan.travelers[mini].role = TravelRole::Mini {
        parent_id: TravelerId(1),
    };
    cases.push(("orphan", orphan));
    let mut crowd = snapshot.clone();
    while crowd.travelers.len() <= limits::MAX_TRAVELERS {
        let mut extra = crowd.travelers[0].clone();
        extra.id = TravelerId(crowd.travelers.len() as u64 + 1000);
        extra.role = TravelRole::Adult;
        crowd.travelers.push(extra);
    }
    cases.push(("crowd", crowd));
    let mut hostile = snapshot.clone();
    hostile.travelers[0].name = "Mallow\u{202E}".to_owned();
    cases.push(("hostile name", hostile));
    let mut giant = snapshot.clone();
    giant.travelers[0].appearance.logical_size = 200;
    cases.push(("giant", giant));
    let mut stray = snapshot.clone();
    stray.relationships[0].b = TravelerId(99);
    cases.push(("stray bond", stray));
    let mut nonsense = snapshot.clone();
    nonsense.travelers[0].character.axes.social = f32::NAN;
    cases.push(("nonsense", nonsense));
    let mut unknown = snapshot;
    unknown.travelers[0].appearance.design = Some(DesignRecipe {
        parts: "ff".repeat(20),
        details: None,
    });
    cases.push(("unknown recipe", unknown));
    for (what, case) in cases {
        assert!(encode(&case).is_err(), "{what} was written");
        let bytes = serde_json::to_vec(&case).unwrap();
        assert!(decode::<TravelSnapshot>(&bytes).is_err(), "{what} was read");
    }
}
