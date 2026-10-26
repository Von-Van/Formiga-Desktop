//! The frozen colony files in `persistence/fixtures/`, one per save version since 14, as a build of
//! that version wrote them: each opens as the current version.
use super::*;

/// The oldest save version kept as a frozen file. The versions before it are covered by the
/// hand-downgraded documents in `versions.rs`, and no colony older than 0.57 has been seen since.
const FIRST_FROZEN_VERSION: u32 = 14;

/// The frozen colony file of one version: a small colony as a build of that version wrote it,
/// kept in `fixtures/` beside these tests and never regenerated to make a test pass. A new
/// version freezes a new file beside the old ones rather than over any of them.
fn frozen_colony(version: u32) -> (String, Vec<u8>) {
    let name = format!("colony-v{version}.json");
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/persistence/fixtures")
        .join(&name);
    let bytes = fs::read(&path).unwrap_or_else(|error| {
        panic!(
            "{name}: {error}; every save version from {FIRST_FROZEN_VERSION} on keeps a frozen \
             colony file beside these tests"
        )
    });
    (name, bytes)
}

/// Every colony file since version 14, as a build of that version wrote it, opens through the
/// same path a file on disk takes and comes out a colony of the current version, with every
/// companion it had and nothing a validated colony never holds. The current version's file,
/// opened and written again, is the same bytes, so the shape this build writes is the shape its
/// own frozen file says it is; moving `SAVE_VERSION` without freezing the new version fails here.
#[test]
fn every_frozen_colony_file_opens_and_the_current_one_is_written_back_byte_for_byte() {
    let directory = std::env::temp_dir().join(format!("formiga-frozen-{}", std::process::id()));
    let store = SaveStore::new(directory.join("colony.json"));
    for version in FIRST_FROZEN_VERSION..=crate::SAVE_VERSION {
        let (name, bytes) = frozen_colony(version);
        assert_eq!(
            named_version(&bytes),
            version,
            "{name} names another version"
        );
        let raw: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let companions = raw["creatures"].as_array().map_or(0, Vec::len);
        let loaded = decode(&bytes).unwrap_or_else(|error| panic!("{name} does not open: {error}"));
        assert_eq!(loaded.save_version, crate::SAVE_VERSION, "{name}");
        assert_eq!(
            loaded.creatures.len(),
            companions,
            "{name} did not keep every companion"
        );
        let found = violations(&loaded);
        assert!(
            found.is_empty(),
            "{name} opens holding what a validated colony never does: {found:?}"
        );
        if version == crate::SAVE_VERSION {
            store.save(&loaded).unwrap();
            let written = fs::read(store.path()).unwrap();
            assert!(
                written == bytes,
                "{name} is not written back as it was read; this build writes:\n{}",
                String::from_utf8_lossy(&written)
            );
        }
    }
    let _ = fs::remove_dir_all(directory);
}
