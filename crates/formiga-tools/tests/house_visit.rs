//! A whole visit to a house, end to end, against the stand-in Home: the snapshot and the homes
//! written as Desktop writes them, the stub started as Desktop starts Home, and its answers read
//! back and kept by the contract's own rule. A visit can change only the house it opened, and
//! one that brings nothing back leaves the homes exactly as they were.

use formiga_core::SaveFile;
use formiga_home_contract::{
    ACK_FILE, AckRefusal, HomeAck, HomeCapability, HomeEffect, HomeError, HomeRecall, HomeReceipt,
    HomeResult, HomeSnapshot, HomeState, LAUNCH_ARGUMENT, RECALL_FILE, RECEIPT_FILE, RESULT_FILE,
    RecallReason, SNAPSHOT_FILE, STATE_FILE, SessionId, SessionSeal, accept_result,
    likely_visitors, project_household, read_document, sample, write_document,
};
use std::path::{Path, PathBuf};
use std::process::Command;
use time::OffsetDateTime;

struct Visit {
    root: PathBuf,
    dir: PathBuf,
    seal: SessionSeal,
    snapshot: HomeSnapshot,
    state: HomeState,
}

impl Drop for Visit {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// Write a visit to the sample colony's house into a session directory of its own, as Desktop
/// does, offering `offers`.
fn pack(save: &SaveFile, name: &str, offers: &[HomeCapability]) -> Visit {
    let root =
        std::env::temp_dir().join(format!("formiga-house-visit-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let session = SessionId::generate().unwrap();
    let dir = root.join(session.as_str());
    std::fs::create_dir_all(&dir).unwrap();
    let keeper = sample::keeper(save);
    let snapshot = project_household(
        save,
        keeper,
        &likely_visitors(save, keeper),
        offers,
        session,
        OffsetDateTime::now_utc(),
        "0.67.0",
    )
    .unwrap();
    let state = HomeState::new(&snapshot.colony_key).settled_for(&snapshot);
    let snapshot_bytes = write_document(&dir.join(SNAPSHOT_FILE), &snapshot).unwrap();
    let state_bytes = write_document(&dir.join(STATE_FILE), &state).unwrap();
    Visit {
        seal: SessionSeal::of(&snapshot, &snapshot_bytes, &state_bytes),
        root,
        dir,
        snapshot,
        state,
    }
}

/// Start the stub as Desktop starts Home, and wait for it.
fn run(dir: &Path, behaviour: &str) -> Option<i32> {
    Command::new(env!("CARGO_BIN_EXE_formiga-home-stub"))
        .arg(LAUNCH_ARGUMENT)
        .arg(dir)
        .env("FORMIGA_HOME_STUB", behaviour)
        .output()
        .expect("the stub runs")
        .status
        .code()
}

fn answer<T: formiga_home_contract::HomeDocument>(visit: &Visit, file: &str) -> Option<T> {
    read_document::<T>(&visit.dir.join(file))
        .ok()
        .map(|(document, _)| document)
}

#[test]
fn a_visit_answers_once_and_changes_only_the_house_it_opened() {
    let save = sample::colony();
    let visit = pack(&save, "ordinary", &[HomeCapability::VisitRecord]);
    assert_eq!(run(&visit.dir, "stay=0,arrange"), Some(0));
    let ack: HomeAck = answer(&visit, ACK_FILE).expect("an acknowledgement");
    assert!(ack.accepted && ack.answers(&visit.seal));
    let result: HomeResult = answer(&visit, RESULT_FILE).expect("a result");
    let kept = accept_result(&visit.seal, &visit.snapshot, &visit.state, &result);
    assert!(kept.set_aside.is_empty(), "{:?}", kept.set_aside);
    let keeper = visit.snapshot.household.keeper;
    let home = kept.state.household(keeper).expect("the house it opened");
    assert_eq!(home.rooms[0].pieces.len(), 1, "the shelf");
    assert_eq!(home.rooms[0].displays.len(), 1, "something on it");
    assert_eq!(kept.state.households.len(), 1, "no other house");
    let receipt: HomeReceipt = answer(&visit, RECEIPT_FILE).expect("a receipt");
    assert!(receipt.answers(&visit.seal));
    let [HomeEffect::HomeVisit { household, .. }] = receipt.effects[..] else {
        panic!("only the visit: {:?}", receipt.effects);
    };
    assert_eq!(household, keeper);
}

#[test]
fn a_visit_never_offered_a_line_brings_none() {
    let save = sample::colony();
    let visit = pack(&save, "unoffered", &[]);
    assert_eq!(run(&visit.dir, "stay=0"), Some(0));
    let receipt: HomeReceipt = answer(&visit, RECEIPT_FILE).expect("a receipt");
    assert!(receipt.effects.is_empty());
}

#[test]
fn a_visit_that_brings_nothing_back_leaves_the_homes_as_they_were() {
    let save = sample::colony();
    for behaviour in [
        "stay=0,silent",
        "stay=0,garbage",
        "stay=0,stranger",
        "stay=0,crash",
    ] {
        let visit = pack(
            &save,
            &behaviour.replace([',', '='], "-"),
            &[HomeCapability::VisitRecord],
        );
        let code = run(&visit.dir, behaviour);
        let usable = answer::<HomeReceipt>(&visit, RECEIPT_FILE)
            .filter(|receipt| receipt.answers(&visit.seal));
        assert!(
            usable.is_none(),
            "{behaviour} left a receipt Desktop would use"
        );
        // Whatever homes it left are the homes it was sent.
        if let Some(result) = answer::<HomeResult>(&visit, RESULT_FILE) {
            let kept = accept_result(&visit.seal, &visit.snapshot, &visit.state, &result);
            assert_eq!(
                kept.state,
                visit.state.settled_for(&visit.snapshot),
                "{behaviour} changed the homes"
            );
        }
        if behaviour.ends_with("crash") {
            assert_eq!(code, Some(101));
        }
    }
    // A crash after arranging keeps what was arranged: Home writes the homes as it goes.
    let visit = pack(&save, "crash-arranged", &[HomeCapability::VisitRecord]);
    assert_eq!(run(&visit.dir, "stay=0,arrange,crash"), Some(101));
    assert!(answer::<HomeReceipt>(&visit, RECEIPT_FILE).is_none());
    let result: HomeResult = answer(&visit, RESULT_FILE).expect("what was arranged");
    let kept = accept_result(&visit.seal, &visit.snapshot, &visit.state, &result);
    assert!(kept.set_aside.is_empty());
}

#[test]
fn home_refusing_the_house_says_why() {
    let save = sample::colony();
    for (behaviour, said) in [
        ("refuse=busy", AckRefusal::Busy),
        ("refuse=invalid", AckRefusal::Invalid),
    ] {
        let visit = pack(&save, &behaviour.replace('=', "-"), &[]);
        assert_eq!(run(&visit.dir, behaviour), Some(2));
        let ack: HomeAck = answer(&visit, ACK_FILE).expect("a refusal");
        assert!(!ack.accepted && ack.answers(&visit.seal));
        assert_eq!(ack.refusal, Some(said));
        assert!(answer::<HomeReceipt>(&visit, RECEIPT_FILE).is_none());
    }
}

#[test]
fn a_recall_ends_the_visit_without_a_word() {
    let save = sample::colony();
    let visit = pack(&save, "recalled", &[HomeCapability::VisitRecord]);
    let recall = HomeRecall::new(
        visit.seal.session_id.clone(),
        OffsetDateTime::now_utc(),
        RecallReason::OwnerAsked,
    );
    write_document(&visit.dir.join(RECALL_FILE), &recall).unwrap();
    let started = std::time::Instant::now();
    assert_eq!(run(&visit.dir, "stay=30,arrange"), Some(0));
    assert!(
        started.elapsed().as_secs() < 10,
        "it did not wait out its stay"
    );
    assert!(answer::<HomeReceipt>(&visit, RECEIPT_FILE).is_none());
}

#[test]
fn a_snapshot_from_a_newer_desktop_is_refused_for_its_version() {
    let save = sample::colony();
    let visit = pack(&save, "newer", &[]);
    let mut value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(visit.dir.join(SNAPSHOT_FILE)).unwrap()).unwrap();
    value["version"] = 99.into();
    value["min_reader_version"] = 99.into();
    std::fs::write(
        visit.dir.join(SNAPSHOT_FILE),
        serde_json::to_vec_pretty(&value).unwrap(),
    )
    .unwrap();
    assert!(matches!(
        formiga_home_contract::decode::<HomeSnapshot>(
            &std::fs::read(visit.dir.join(SNAPSHOT_FILE)).unwrap()
        ),
        Err(HomeError::UnsupportedVersion { .. })
    ));
    assert_eq!(run(&visit.dir, "stay=0"), Some(2));
    let ack: HomeAck = answer(&visit, ACK_FILE).expect("a refusal");
    assert!(matches!(
        ack.refusal,
        Some(AckRefusal::UnsupportedVersion { .. })
    ));
}
