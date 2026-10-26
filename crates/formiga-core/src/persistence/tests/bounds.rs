//! Bounds: whatever a file holds, a colony read from it is back inside every cap and holds
//! nothing it cannot draw.
use super::*;

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

/// A sculpt the file holds out of bounds is brought inside them, and one with no recipe beside
/// it is given the nearest, as a recipe out of bounds is: validation keeps what a colony holds
/// drawable, whatever wrote it.
#[test]
fn a_sculpt_out_of_bounds_is_brought_inside_them_and_given_a_recipe() {
    let mut save = example_save();
    let mut creature = crate::World::preview_adult(
        [52; 32],
        save.created_at_utc,
        &crate::DesktopSnapshot::default(),
    );
    creature.appearance = sculpted(&creature, crate::forms::Plan::Crawler);
    let mut sculpt = creature.appearance.sculpt.clone().unwrap();
    sculpt.shape.girth = 200;
    creature.appearance.sculpt = Some(sculpt.clone());
    creature.appearance.design = None;
    save.creatures.push(creature);
    assert!(
        violations(&save)
            .iter()
            .any(|violation| violation.contains("sculpted form")),
        "{:?}",
        violations(&save)
    );
    let kept = ValidatedSave::from(save).into_inner();
    let appearance = &kept.creatures.last().unwrap().appearance;
    assert_eq!(appearance.sculpt, Some(sculpt.normalized()));
    assert_eq!(
        appearance.design.map(|recipe| recipe.body),
        Some(crate::BodyPlan::Round)
    );
    assert!(violations(&kept).is_empty(), "{:?}", violations(&kept));
}
