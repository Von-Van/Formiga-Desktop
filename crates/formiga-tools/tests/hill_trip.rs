//! A whole trip, end to end, against the stand-in Hill: the snapshot written as Desktop writes it,
//! the stub started as Desktop starts Hill, and its answers read back. The colony itself only
//! changes by what Desktop applies from a receipt — and a trip that brings nothing back leaves the
//! colony file exactly as it was.

use formiga_core::*;
use formiga_travel::*;
use std::path::{Path, PathBuf};
use std::process::Command;
use time::OffsetDateTime;
use time::macros::datetime;

struct Session {
    root: PathBuf,
    dir: PathBuf,
    seal: SnapshotSeal,
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn desktop() -> DesktopSnapshot {
    DesktopSnapshot {
        monitors: vec![MonitorInfo {
            id: 1,
            display_key: DisplayKey([1; 16]),
            bounds: DesktopRect {
                x: 0.0,
                y: 0.0,
                width: 1440.0,
                height: 900.0,
            },
            usable_bounds: DesktopRect {
                x: 0.0,
                y: 24.0,
                width: 1440.0,
                height: 826.0,
            },
            scale_factor: 2.0,
            primary: true,
        }],
        ..DesktopSnapshot::default()
    }
}

fn colony() -> World {
    let now = datetime!(2026-10-02 9:00 UTC);
    let desktop = desktop();
    let mut world = World::new([21; 32], now, &desktop);
    for seed in 22..25 {
        world
            .add_designed_adult([seed; 32], None, now, &desktop)
            .unwrap();
    }
    world.prepare_for_trip(now);
    world
}

/// Write a trip's snapshot into a session directory of its own, as Desktop does.
fn pack(world: &World, name: &str) -> Session {
    let root =
        std::env::temp_dir().join(format!("formiga-hill-trip-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let session = SessionId::generate().unwrap();
    let dir = root.join(session.as_str());
    std::fs::create_dir_all(&dir).unwrap();
    let snapshot =
        project_colony(&world.save, session, OffsetDateTime::now_utc(), "0.66.1").unwrap();
    let bytes = write_document(&dir.join(SNAPSHOT_FILE), &snapshot).unwrap();
    Session {
        seal: SnapshotSeal::of(&snapshot, &bytes),
        root,
        dir,
    }
}

/// Start the stub as Desktop starts Hill, and wait for it.
fn visit(dir: &Path, behaviour: &str) -> Option<i32> {
    Command::new(env!("CARGO_BIN_EXE_formiga-hill-stub"))
        .arg(LAUNCH_ARGUMENT)
        .arg(dir)
        .env("FORMIGA_HILL_STUB", behaviour)
        .output()
        .expect("the stub runs")
        .status
        .code()
}

fn receipt(session: &Session) -> Result<ReturnReceipt, TravelError> {
    read_document(&session.dir.join(RECEIPT_FILE))
}

fn ack(session: &Session) -> Acknowledgement {
    read_document(&session.dir.join(ACK_FILE)).expect("an acknowledgement")
}

/// The colony file without what a trip home may add: the trip itself, the souvenirs it brought,
/// and its line in the journal.
fn without_the_trip(save: &SaveFile) -> Vec<u8> {
    let mut save = save.clone();
    save.trips = TripLog::default();
    save.companion
        .journal
        .retain(|entry| entry.moment != JournalMoment::Trip);
    serde_json::to_vec_pretty(&save).unwrap()
}

#[test]
fn an_ordinary_visit_brings_home_only_the_visit() {
    let mut world = colony();
    let before = serde_json::to_vec_pretty(&world.save).unwrap();
    let session = pack(&world, "ordinary");
    assert_eq!(visit(&session.dir, "stay=0"), Some(0));
    let ack = ack(&session);
    assert!(ack.accepted && ack.answers(&session.seal));
    let receipt = receipt(&session).unwrap();
    assert!(receipt.answers(&session.seal));
    let [
        ReturnEffect::Visit {
            arrived_at_utc,
            left_at_utc,
        },
    ] = receipt.effects[..]
    else {
        panic!("only a visit: {:?}", receipt.effects);
    };
    // What Desktop keeps: the visit, once.
    assert!(world.welcome_home(
        Trip {
            session: session.seal.session_id.to_string(),
            arrived_at_utc,
            left_at_utc,
        },
        OffsetDateTime::now_utc(),
    ));
    assert_eq!(world.save.trips.count, 1);
    assert_eq!(
        without_the_trip(&world.save),
        before,
        "nothing else in the colony changed"
    );
}

/// What Desktop does with a receipt's souvenirs, short of the desktop app itself: each one this
/// build keeps, once.
fn keep(world: &mut World, receipt: &ReturnReceipt) -> bool {
    let souvenirs: Vec<Souvenir> = receipt
        .effects
        .iter()
        .filter_map(|effect| match effect {
            ReturnEffect::Souvenir { id } => Souvenir::from_id(id),
            _ => None,
        })
        .collect();
    world.keep_souvenirs(&souvenirs, OffsetDateTime::now_utc())
}

#[test]
fn a_visit_brings_home_each_souvenir_desktop_keeps_once() {
    let mut world = colony();
    let before = serde_json::to_vec_pretty(&world.save).unwrap();
    let session = pack(&world, "souvenirs");
    assert_eq!(visit(&session.dir, "stay=0,souvenir"), Some(0));
    let receipt = receipt(&session).unwrap();
    let ids: Vec<&str> = receipt
        .effects
        .iter()
        .filter_map(|effect| match effect {
            ReturnEffect::Souvenir { id } => Some(id.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(ids, Souvenir::ALL.map(Souvenir::id), "every one listed");
    assert!(keep(&mut world, &receipt));
    let kept: Vec<Souvenir> = world
        .save
        .trips
        .souvenirs
        .iter()
        .map(|record| record.souvenir)
        .collect();
    assert_eq!(kept, Souvenir::ALL);
    // Brought home again on the next trip, or named by an identifier Desktop does not keep,
    // nothing changes.
    for behaviour in [
        "stay=0,souvenir=picnic_ribbon",
        "stay=0,souvenir=acorn-badge",
    ] {
        let again = pack(&world, &behaviour.replace([',', '='], "-"));
        assert_eq!(visit(&again.dir, behaviour), Some(0));
        let receipt = self::receipt(&again).unwrap();
        assert!(
            matches!(receipt.effects.last(), Some(ReturnEffect::Souvenir { .. })),
            "{behaviour}"
        );
        assert!(!keep(&mut world, &receipt), "{behaviour}");
    }
    assert_eq!(world.save.trips.souvenirs.len(), Souvenir::ALL.len());
    assert_eq!(
        without_the_trip(&world.save),
        before,
        "a souvenir changes nothing else in the colony"
    );
}

#[test]
fn a_trip_that_brings_nothing_back_leaves_the_colony_file_as_it_was() {
    for behaviour in [
        "stay=0,silent",
        "stay=0,garbage",
        "stay=0,stranger",
        "stay=0,crash",
    ] {
        let world = colony();
        let before = serde_json::to_vec_pretty(&world.save).unwrap();
        let session = pack(&world, &behaviour.replace([',', '='], "-"));
        let code = visit(&session.dir, behaviour);
        let usable = receipt(&session)
            .ok()
            .filter(|receipt| receipt.answers(&session.seal));
        assert!(
            usable.is_none(),
            "{behaviour} left a receipt Desktop would use"
        );
        if behaviour.ends_with("crash") {
            assert_eq!(code, Some(101));
            assert!(ack(&session).accepted, "it got as far as taking the colony");
        }
        assert_eq!(serde_json::to_vec_pretty(&world.save).unwrap(), before);
    }
}

#[test]
fn hill_refusing_the_colony_says_why() {
    let world = colony();
    for (behaviour, expected) in [
        (
            "refuse=version",
            AckRefusal::UnsupportedVersion { reads: 0 },
        ),
        ("refuse=busy", AckRefusal::Busy),
    ] {
        let session = pack(&world, behaviour.replace('=', "-").as_str());
        assert_eq!(visit(&session.dir, behaviour), Some(2));
        let ack = ack(&session);
        assert!(!ack.accepted);
        assert_eq!(ack.refusal, Some(expected));
        assert!(receipt(&session).is_err());
    }
}

#[test]
fn a_snapshot_from_a_newer_desktop_is_refused_for_its_version() {
    let world = colony();
    let session = pack(&world, "newer");
    let path = session.dir.join(SNAPSHOT_FILE);
    let mut value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    value["version"] = (TRAVEL_FORMAT_VERSION + 1).into();
    value["min_reader_version"] = (TRAVEL_FORMAT_VERSION + 1).into();
    std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
    assert_eq!(visit(&session.dir, "stay=0"), Some(2));
    assert_eq!(
        ack(&session).refusal,
        Some(AckRefusal::UnsupportedVersion {
            reads: TRAVEL_FORMAT_VERSION
        })
    );
}

#[test]
fn a_recall_ends_the_visit_without_a_receipt() {
    let world = colony();
    let session = pack(&world, "recall");
    let recall = Recall::new(
        session.seal.session_id.clone(),
        OffsetDateTime::now_utc(),
        RecallReason::OwnerAsked,
    );
    write_document(&session.dir.join(RECALL_FILE), &recall).unwrap();
    assert_eq!(visit(&session.dir, "stay=30"), Some(0));
    assert!(receipt(&session).is_err());
}

#[test]
fn a_directory_that_is_not_a_session_is_not_used() {
    let world = colony();
    let session = pack(&world, "misnamed");
    let misnamed = session.root.join("not-a-session");
    std::fs::rename(&session.dir, &misnamed).unwrap();
    assert_ne!(visit(&misnamed, "stay=0"), Some(0));
    assert!(!misnamed.join(RECEIPT_FILE).exists());
}

#[test]
fn a_session_cleared_away_ends_the_visit() {
    let world = colony();
    let session = pack(&world, "cleared");
    let mut stub = Command::new(env!("CARGO_BIN_EXE_formiga-hill-stub"))
        .arg(LAUNCH_ARGUMENT)
        .arg(&session.dir)
        .env("FORMIGA_HILL_STUB", "stay=30")
        .spawn()
        .unwrap();
    // Once it has read the snapshot, take the whole session away, as a sweep would.
    let started = std::time::Instant::now();
    while !session.dir.join(ACK_FILE).exists() {
        assert!(
            started.elapsed().as_secs() < 10,
            "the stub never acknowledged"
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    std::fs::remove_dir_all(&session.dir).unwrap();
    let cleared = std::time::Instant::now();
    loop {
        if let Some(status) = stub.try_wait().unwrap() {
            assert!(status.success());
            break;
        }
        assert!(cleared.elapsed().as_secs() < 5, "the stub stayed on");
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}
