//! Saving: a colony written and read back is the colony that was written, and what a colony does
//! decides how soon it is written.
use super::*;

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
