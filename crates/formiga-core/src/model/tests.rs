use super::*;
use time::OffsetDateTime;

#[test]
fn compact_lived_experience_state_stays_within_budget() {
    assert!(std::mem::size_of::<CreatureMemory>() < 192);
    assert!(std::mem::size_of::<LearnedTendencies>() <= 32);
    let memory = CreatureMemory {
        times_petted: u32::MAX,
        times_tossed: u32::MAX,
        placements: u32::MAX,
        sleep_interruptions: u32::MAX,
        window_climbs: u32::MAX,
        discoveries_found: u32::MAX,
        play_sessions: u32::MAX,
        home_visits: u32::MAX,
        ledge_seconds: u32::MAX,
        window_ride_seconds: u32::MAX,
        longest_sleep_seconds: u32::MAX,
        favorite_display: Some(FavoriteDisplayMemory {
            display: DisplayKey([u8::MAX; 16]),
            confidence: u8::MAX,
        }),
        preferred_region: Some(PreferredRegionMemory {
            display: DisplayKey([u8::MAX; 16]),
            cell: 8,
            confidence: u8::MAX,
        }),
        descriptor_flags: u16::MAX,
        profile_revision: u16::MAX,
        viewed_profile_revision: u16::MAX,
        milestone_cooldown_active_seconds: u32::MAX,
        milestone_bubble_shown: true,
        habits: vec![crate::Habit::CirclesBeforeNaps, crate::Habit::LooksFoodOver],
    };
    let routines = RoutineTable {
        slots: [RoutineSlot {
            key: u16::MAX,
            strength: u8::MAX,
        }; MAX_ROUTINES],
        len: MAX_ROUTINES as u8,
    };
    let payload = serde_json::to_vec(&(
        CreatureOrigin::default(),
        "Mallow the Magnificent",
        memory,
        LearnedTendencies {
            cursor_trust: 100.0,
            sociability: 100.0,
            climbing: 100.0,
            sleep_security: 100.0,
            exploration: 100.0,
            play: 100.0,
            home_affinity: 100.0,
            routine: 100.0,
        },
        routines,
    ))
    .unwrap();
    assert!(
        payload.len() < 2 * 1024,
        "payload used {} bytes",
        payload.len()
    );
}

#[test]
fn ritual_persistence_contains_only_the_bounded_schedule_projection() {
    let value = serde_json::to_value(RitualState {
        next_at_utc: OffsetDateTime::UNIX_EPOCH,
        last_kind: Some(RitualKind::Picnic),
        ordinal: 17,
        hatch_day_acknowledged_year: Some(2026),
    })
    .unwrap();
    let fields = value.as_object().unwrap();
    assert_eq!(fields.len(), 4);
    assert!(fields.contains_key("next_at_utc"));
    assert!(fields.contains_key("last_kind"));
    assert!(fields.contains_key("ordinal"));
    assert!(fields.contains_key("hatch_day_acknowledged_year"));
    assert_eq!(RitualKind::ALL.len(), 10);
}

#[test]
fn colony_object_projection_is_typed_and_bounded() {
    assert_eq!(ColonyObjectKind::ALL.len(), 20);
    for (index, kind) in ColonyObjectKind::ALL.into_iter().enumerate() {
        assert_eq!(usize::from(kind.index()), index);
    }
    assert_eq!(MAX_COLONY_OBJECTS, 8);
    assert_eq!(
        ColonyObjectKind::Pillow.default_role(),
        ColonyObjectRole::Sleep
    );
    assert_eq!(ColonyObjectKind::Toy.default_role(), ColonyObjectRole::Play);
    assert_eq!(
        ColonyObjectKind::Cup.default_role(),
        ColonyObjectRole::Social
    );
    let object = ColonyObject {
        id: 7,
        kind: ColonyObjectKind::Plant,
        display: DisplayKey([3; 16]),
        normalized_position: Point { x: 0.4, y: 0.8 },
        role: ColonyObjectRole::Comfort,
    };
    let value = serde_json::to_value(object).unwrap();
    assert_eq!(value.as_object().unwrap().len(), 5);
}

#[test]
fn every_decoration_belongs_to_one_slot_and_every_slot_has_five_to_choose_from() {
    for (index, kind) in ShelterDecorationKind::ALL.into_iter().enumerate() {
        assert_eq!(kind.index(), index);
    }
    // The six a colony could earn before keep their names in the file.
    for (kind, name) in [
        (ShelterDecorationKind::Leaf, "Leaf"),
        (ShelterDecorationKind::Banner, "Banner"),
        (ShelterDecorationKind::Stone, "Stone"),
        (ShelterDecorationKind::Flower, "Flower"),
        (ShelterDecorationKind::Lamp, "Lamp"),
        (ShelterDecorationKind::RoofOrnament, "RoofOrnament"),
    ] {
        assert_eq!(serde_json::to_value(kind).unwrap(), name);
    }
    // Each of the six earned before hangs in a slot of its own, so a colony house that wore
    // all six still can.
    let earned: std::collections::BTreeSet<_> = ShelterDecorationKind::ALL[..6]
        .iter()
        .map(|kind| kind.slot())
        .collect();
    assert_eq!(earned.len(), DecorationSlot::ALL.len());
    for slot in DecorationSlot::ALL {
        let choices = ShelterDecorationKind::ALL
            .into_iter()
            .filter(|kind| kind.slot() == slot)
            .count();
        assert_eq!(choices, 5, "{slot:?}");
    }
    let labels: std::collections::BTreeSet<_> = ShelterDecorationKind::ALL
        .iter()
        .map(|k| k.label())
        .collect();
    assert_eq!(labels.len(), ShelterDecorationKind::ALL.len());
}

#[test]
fn a_house_wears_one_decoration_per_slot_and_only_what_the_village_has() {
    let mut home = ColonyHome::default();
    let keeper = 7;
    assert!(home.set_decoration(
        keeper,
        DecorationSlot::Eaves,
        Some(ShelterDecorationKind::Banner)
    ));
    // Not yet the village's to hang.
    assert!(!home.set_decoration(
        keeper,
        DecorationSlot::Eaves,
        Some(ShelterDecorationKind::FairyLights)
    ));
    // In the wrong slot.
    assert!(!home.set_decoration(
        keeper,
        DecorationSlot::Roof,
        Some(ShelterDecorationKind::Banner)
    ));
    home.unlocks
        .grant(VillageItem::Decoration(ShelterDecorationKind::FairyLights));
    assert!(home.set_decoration(
        keeper,
        DecorationSlot::Eaves,
        Some(ShelterDecorationKind::FairyLights)
    ));
    assert!(home.set_decoration(
        keeper,
        DecorationSlot::WallRight,
        Some(ShelterDecorationKind::Lamp)
    ));
    assert_eq!(
        home.decorations_of(keeper),
        &[
            ShelterDecorationKind::FairyLights,
            ShelterDecorationKind::Lamp
        ]
    );
    assert!(home.set_decoration(keeper, DecorationSlot::Eaves, None));
    assert_eq!(home.decorations_of(keeper), &[ShelterDecorationKind::Lamp]);
    assert!(home.set_decoration(keeper, DecorationSlot::WallRight, None));
    assert!(home.dressing.is_empty(), "a bare house has no entry at all");
}

#[test]
fn a_village_starts_with_three_of_everything_and_grows_one_at_a_time() {
    let mut unlocks = VillageUnlocks::starting();
    assert_eq!(unlocks.decorations.len(), 3);
    assert_eq!(unlocks.hangouts.len(), 3);
    assert_eq!(unlocks.gardens.len(), 3);
    assert_eq!(unlocks.ornaments.len(), 3);
    let total = VillageItem::all().count();
    assert_eq!(total, 30 + 15 + 12 + 15);
    assert_eq!(unlocks.remaining().count(), total - 12);
    let next = unlocks.remaining().next().unwrap();
    assert!(unlocks.grant(next));
    assert!(!unlocks.grant(next), "granted once");
    assert_eq!(unlocks.remaining().count(), total - 13);
    // Normalising tops a category back up and drops a repeat.
    unlocks.hangouts = vec![HangoutKind::Swing, HangoutKind::Swing];
    unlocks.normalize();
    assert_eq!(
        unlocks.hangouts,
        vec![
            HangoutKind::Swing,
            HangoutKind::Cushion,
            HangoutKind::Blanket,
            HangoutKind::Lookout
        ]
    );
}

#[test]
fn a_garden_grows_round_its_stages_by_itself() {
    let planted = OffsetDateTime::UNIX_EPOCH + time::Duration::days(20_000);
    let patch = GardenPatch {
        kind: GardenKind::Herbs,
        along: 0.5,
        planted_at_utc: Some(planted),
    };
    let hours = GardenKind::Herbs.stage_hours();
    let at = |h: i64| planted + time::Duration::hours(h);
    assert_eq!(patch.stage(planted), GardenStage::Sprout);
    assert_eq!(patch.stage(at(hours)), GardenStage::Growing);
    assert_eq!(patch.stage(at(hours * 2)), GardenStage::Grown);
    assert_eq!(patch.stage(at(hours * 3)), GardenStage::Bounty);
    assert_eq!(patch.stage(at(hours * 4)), GardenStage::Sprout);
    // A clock set back never un-plants it.
    assert_eq!(patch.stage(at(-50)), GardenStage::Sprout);
    // A patch from before gardens grew is somewhere round its cycle, and moves on with time.
    let old = GardenPatch {
        planted_at_utc: None,
        ..patch
    };
    let seen: std::collections::BTreeSet<_> =
        (0..4).map(|step| old.stage(at(step * hours))).collect();
    assert_eq!(seen.len(), 4);
}

#[test]
fn the_trees_fill_themselves_until_chosen_and_the_first_sixteen_keep_their_hooks() {
    let record = |variant: u8, day: i64| crate::ScrapbookRecord {
        variant,
        first_at: OffsetDateTime::UNIX_EPOCH + time::Duration::days(day),
        finder: None,
        finder_name: String::new(),
    };
    // An older colony's finds hang where they always did.
    let old = [record(3, 1), record(12, 2), record(0, 3)];
    let hooks = hung_keepsakes(None, &old);
    assert_eq!(hooks[3], Some(3));
    assert_eq!(hooks[12], Some(12));
    assert_eq!(hooks[0], Some(0));
    assert_eq!(hooks.iter().flatten().count(), 3);
    // Later finds fill the empty hooks in the order they were found.
    let mut grown = old.to_vec();
    grown.extend([record(40, 9), record(20, 5), record(150, 7)]);
    let hooks = hung_keepsakes(None, &grown);
    assert_eq!(hooks[1], Some(20));
    assert_eq!(hooks[2], Some(150));
    assert_eq!(hooks[4], Some(40));
    // Chosen by hand: exactly the choice, less anything never found.
    let mut chosen = [None; TREE_HOOKS];
    chosen[5] = Some(40);
    chosen[6] = Some(99);
    let hooks = hung_keepsakes(Some(&TreeKeepsakes { hooks: chosen }), &grown);
    assert_eq!(hooks[5], Some(40));
    assert_eq!(hooks.iter().flatten().count(), 1);
    // A repeat or a variant past the catalogue is dropped when the village is tidied.
    let mut home = ColonyHome::default();
    let mut hooks = [None; TREE_HOOKS];
    hooks[0] = Some(2);
    hooks[1] = Some(2);
    hooks[2] = Some(250);
    home.set_tree_keepsakes(Some(hooks));
    assert_eq!(home.tree_keepsakes.unwrap().hung(), 1);
}

#[test]
fn routine_table_is_bounded_and_keeps_the_strongest_legacy_entries() {
    let entries = (0..24).map(|key| (key, f32::from(key) / 24.0)).collect();
    let table = RoutineTable::from_ranked(entries);
    assert_eq!(table.len, MAX_ROUTINES as u8);
    assert!(
        table.slots[..MAX_ROUTINES]
            .iter()
            .all(|slot| slot.key >= 12)
    );
}

#[test]
fn squeeze_action_appends_without_renumbering_persisted_routine_codes() {
    assert_eq!(ActionKind::Idle.routine_code(), 0);
    assert_eq!(ActionKind::Traverse.routine_code(), 1);
    assert_eq!(ActionKind::PetReaction.routine_code(), 23);
    assert_eq!(ActionKind::SqueezeWindow.routine_code(), 24);
}

#[test]
fn learned_tendencies_saturate_and_descriptors_use_hysteresis() {
    let mut tendencies = LearnedTendencies::default();
    LearnedTendencies::adjust(&mut tendencies.climbing, 120.0);
    assert_eq!(tendencies.climbing, 100.0);
    let mut memory = CreatureMemory::default();
    assert!(update_descriptor_flags(&mut memory, tendencies));
    let high_places = ProfileDescriptor::LovesHighPlaces.flag();
    assert_ne!(memory.descriptor_flags & high_places, 0);
    tendencies.climbing = 30.0;
    assert!(!update_descriptor_flags(&mut memory, tendencies));
    tendencies.climbing = 24.0;
    assert!(update_descriptor_flags(&mut memory, tendencies));
    assert_eq!(memory.descriptor_flags & high_places, 0);
}

/// Something that keeps happening only ever brings a leaning toward an end, and when it stops,
/// the leaning drifts back to where the companion's nature rests it, from either side.
#[test]
fn leanings_approach_an_end_and_drift_back_both_ways() {
    let mut value = 0.0;
    for _ in 0..2_000 {
        LearnedTendencies::learn(&mut value, 3.0);
    }
    assert!(value > 99.0 && value <= 100.0, "{value}");
    let before = value;
    LearnedTendencies::learn(&mut value, 3.0);
    assert!(value - before < 0.01, "near the end, it barely moves");
    for _ in 0..2_000 {
        LearnedTendencies::learn(&mut value, -3.0);
    }
    assert!((-100.0..-99.0).contains(&value), "{value}");

    let temperament = crate::Temperament {
        kind: crate::TemperamentKind::Grump,
        axes: crate::Axes {
            social: 0.1,
            suspicion: 0.9,
            ..crate::Axes::MIDDLING
        },
        tension: None,
    };
    let rest = LearnedTendencies::baseline(&temperament);
    assert!(
        rest.sociability < -20.0 && rest.cursor_trust < -20.0,
        "{rest:?}"
    );
    assert_eq!(rest.play, 0.0, "a middling side rests at the middle");
    let mut high = LearnedTendencies {
        sociability: 100.0,
        cursor_trust: 100.0,
        ..LearnedTendencies::default()
    };
    let mut low = LearnedTendencies {
        sociability: -100.0,
        ..LearnedTendencies::default()
    };
    for _ in 0..20_000 {
        high.fade_toward(rest, 1.0 / 2000.0);
        low.fade_toward(rest, 1.0 / 2000.0);
    }
    assert!(
        (high.sociability - rest.sociability).abs() < 1.0,
        "{high:?}"
    );
    assert!((low.sociability - rest.sociability).abs() < 1.0, "{low:?}");
    assert!(
        (high.cursor_trust - rest.cursor_trust).abs() < 1.0,
        "{high:?}"
    );
    // An older companion's middling sides rest its leanings at nothing at all.
    let read = crate::Temperament {
        kind: crate::TemperamentKind::Explorer,
        axes: crate::Axes::MIDDLING,
        tension: None,
    };
    assert_eq!(
        LearnedTendencies::baseline(&read),
        LearnedTendencies::default()
    );
}

#[test]
fn creature_names_are_trimmed_unicode_and_reject_controls() {
    assert_eq!(validate_creature_name("  Möchi  ").unwrap(), "Möchi");
    assert_eq!(
        validate_creature_name("\nPip"),
        Err(CreatureNameError::ControlCharacter)
    );
    assert_eq!(validate_creature_name("   "), Err(CreatureNameError::Empty));
    assert_eq!(
        validate_creature_name("abcdefghijklmnopqrstuvwxyz"),
        Err(CreatureNameError::TooLong)
    );
}

#[test]
fn default_names_avoid_initial_duplicates() {
    let first = default_creature_name([7; 32], 0, &[]);
    let second = default_creature_name([7; 32], 1, std::slice::from_ref(&first));
    assert_ne!(first, second);
}

#[test]
fn relationship_scores_are_exactly_four_bytes_and_saturate() {
    assert_eq!(
        std::mem::size_of::<u8>() * 4,
        std::mem::size_of_val(&[
            CreatureRelationship::default().affinity,
            CreatureRelationship::default().familiarity,
            CreatureRelationship::default().playfulness,
            CreatureRelationship::default().avoidance,
        ])
    );
    let mut relationship = CreatureRelationship::new(9, 3).unwrap();
    assert_eq!((relationship.a, relationship.b), (3, 9));
    for _ in 0..300 {
        relationship.apply(RelationshipExperience::PositivePlay);
    }
    assert_eq!(relationship.affinity, u8::MAX);
    assert_eq!(relationship.familiarity, u8::MAX);
    assert_eq!(relationship.playfulness, u8::MAX);
    assert_eq!(relationship.avoidance, 0);
    for _ in 0..300 {
        relationship.apply(RelationshipExperience::Squabble);
    }
    assert_eq!(relationship.affinity, 0);
    assert_eq!(relationship.avoidance, u8::MAX);
}

#[test]
fn closest_companion_uses_canonical_shared_records() {
    let relationships = [
        CreatureRelationship {
            a: 1,
            b: 2,
            affinity: 90,
            familiarity: 20,
            playfulness: 0,
            avoidance: 0,
        },
        CreatureRelationship {
            a: 1,
            b: 3,
            affinity: 120,
            familiarity: 80,
            playfulness: 0,
            avoidance: 100,
        },
    ];
    assert_eq!(closest_companion(&relationships, 1), Some(2));
    assert_eq!(closest_companion(&relationships, 2), Some(1));
}
