//! Recovery: a damaged colony file is refused or repaired, never half read, and a good backup is
//! never written over by a bad file.
use super::*;

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
            super::damage(&mut damaged, &mut rng);
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
