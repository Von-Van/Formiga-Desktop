//! Every Battle version as it was first written. These files are never regenerated to make a test
//! pass: a change that breaks one of them breaks every Battle already installed.
//! `FORMIGA_BATTLE_BLESS=1` writes this build's own version's files only, so a new version's
//! fixtures go beside the old ones and never over them.

use formiga_battle_contract::*;
use std::path::PathBuf;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn read(name: &str) -> Vec<u8> {
    std::fs::read(fixture(name)).unwrap_or_else(|error| panic!("{name}: {error}"))
}

/// Every version a fixture set has been written for.
const WRITTEN: std::ops::RangeInclusive<u32> = 1..=BATTLE_FORMAT_VERSION;

/// The fixtures as this build writes them, named for this build's version.
fn written_now() -> Vec<(String, Vec<u8>)> {
    let v = BATTLE_FORMAT_VERSION;
    let snapshot = sample::snapshot();
    let session = snapshot.session_id.clone();
    let mut out = vec![
        (format!("snapshot-v{v}.json"), snapshot.encode().unwrap()),
        (
            format!("ack-v{v}.json"),
            BattleAck::new(session.clone(), "0.1.0", None)
                .encode()
                .unwrap(),
        ),
        (
            format!("ack-refused-v{v}.json"),
            BattleAck::new(session, "0.1.0", Some(AckRefusal::Invalid))
                .encode()
                .unwrap(),
        ),
    ];
    out.sort();
    out
}

#[test]
fn this_version_is_still_written_exactly_as_it_was_first_written() {
    let bless = std::env::var_os("FORMIGA_BATTLE_BLESS").is_some();
    for (name, bytes) in written_now() {
        if bless {
            std::fs::write(fixture(&name), &bytes).unwrap();
            continue;
        }
        assert!(
            read(&name) == bytes,
            "{name} is no longer written the way Battle version {BATTLE_FORMAT_VERSION} first was"
        );
    }
}

#[test]
fn every_version_ever_written_still_reads() {
    for version in WRITTEN {
        let name = |kind: &str| format!("{kind}-v{version}.json");
        let snapshot = BattleSnapshot::decode(&read(&name("snapshot"))).unwrap();
        assert!(!snapshot.contenders.is_empty());
        let ack = BattleAck::decode(&read(&name("ack"))).unwrap();
        assert!(ack.accepted && ack.session_id == snapshot.session_id);
        let refused = BattleAck::decode(&read(&name("ack-refused"))).unwrap();
        assert_eq!(refused.refusal, Some(AckRefusal::Invalid));
    }
}

#[test]
fn a_document_from_a_newer_battle_is_refused_for_its_version_and_an_older_reader_reads_additions() {
    let mut value: serde_json::Value = serde_json::from_slice(&read("ack-v1.json")).unwrap();
    value["version"] = (BATTLE_FORMAT_VERSION + 1).into();
    value["something_new"] = "ignored".into();
    assert!(BattleAck::decode(&serde_json::to_vec(&value).unwrap()).is_ok());
    value["min_reader_version"] = (BATTLE_FORMAT_VERSION + 1).into();
    assert!(matches!(
        BattleAck::decode(&serde_json::to_vec(&value).unwrap()),
        Err(BattleError::UnsupportedVersion { .. })
    ));
}
