use super::*;
use formiga_core::{DesktopRect, DesktopSnapshot, DisplayKey, MonitorInfo, World};
use formiga_travel::{TRAVEL_FORMAT_VERSION, encode, project_colony};
use time::macros::datetime;

const LEFT: OffsetDateTime = datetime!(2026-10-02 9:30 UTC);

/// A directory of its own under the system's temporary directory, removed when dropped.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "formiga-hill-session-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn snapshot() -> TravelSnapshot {
    let desktop = DesktopSnapshot {
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
    };
    let world = World::new([4; 32], LEFT, &desktop);
    project_colony(&world.save, SessionId::generate().unwrap(), LEFT, "0.66.1").unwrap()
}

fn visit(arrived: OffsetDateTime, left: OffsetDateTime) -> ReturnEffect {
    ReturnEffect::Visit {
        arrived_at_utc: arrived,
        left_at_utc: left,
    }
}

fn receipt(trip: &OpenTrip, effects: Vec<ReturnEffect>) -> ReturnReceipt {
    ReturnReceipt::new(&trip.seal, LEFT + Duration::hours(1), "0.1.0", effects)
}

fn write_receipt(trip: &OpenTrip, receipt: &ReturnReceipt) {
    write_document(&trip.dir.join(RECEIPT_FILE), receipt).unwrap();
}

#[test]
fn a_trip_is_written_whole_and_found_again_after_a_restart() {
    let scratch = Scratch::new("open");
    let files = TravelFiles::new(&scratch.0);
    assert_eq!(files.open_trip(), None);
    let snapshot = snapshot();
    let trip = files.open(&snapshot).unwrap();
    assert_eq!(
        trip.dir,
        scratch.0.join("travel").join(snapshot.session_id.as_str())
    );
    let written = fs::read(trip.dir.join(SNAPSHOT_FILE)).unwrap();
    assert_eq!(written, encode(&snapshot).unwrap());
    assert_eq!(trip.seal, SnapshotSeal::of(&snapshot, &written));
    assert_eq!(TravelFiles::new(&scratch.0).open_trip(), Some(trip.clone()));
    // A session directory is never reused.
    assert!(files.open(&snapshot).is_err());
    assert_eq!(fs::read(trip.dir.join(SNAPSHOT_FILE)).unwrap(), written);
}

#[test]
fn a_trip_with_no_answer_is_silent() {
    let scratch = Scratch::new("silent");
    let files = TravelFiles::new(&scratch.0);
    let trip = files.open(&snapshot()).unwrap();
    assert_eq!(
        files.answer(&trip),
        Answer::Silent {
            refusal: None,
            problem: None
        }
    );
}

#[test]
fn only_a_receipt_for_exactly_this_trip_is_a_receipt() {
    let scratch = Scratch::new("receipts");
    let files = TravelFiles::new(&scratch.0);
    let trip = files.open(&snapshot()).unwrap();
    let good = receipt(&trip, vec![visit(LEFT, LEFT + Duration::minutes(50))]);
    write_receipt(&trip, &good);
    assert_eq!(files.answer(&trip), Answer::Receipt(good.clone()));

    let path = trip.dir.join(RECEIPT_FILE);
    let full = encode(&good).unwrap();
    let mut other_session = good.clone();
    other_session.session_id = SessionId::generate().unwrap();
    let mut other_snapshot = good.clone();
    other_snapshot.snapshot_sha256 = "00".repeat(32);
    let mut newer = serde_json::to_value(&good).unwrap();
    newer["version"] = (TRAVEL_FORMAT_VERSION + 1).into();
    newer["min_reader_version"] = (TRAVEL_FORMAT_VERSION + 1).into();
    let cases: Vec<(&str, Vec<u8>)> = vec![
        ("half-written", full[..full.len() / 2].to_vec()),
        ("garbage", b"\x00\xff not json".to_vec()),
        ("empty", Vec::new()),
        ("oversized", vec![b' '; 64 * 1024]),
        ("another session", encode(&other_session).unwrap()),
        ("another snapshot", encode(&other_snapshot).unwrap()),
        ("a newer version", serde_json::to_vec(&newer).unwrap()),
        (
            "an acknowledgement",
            fs::read(trip.dir.join(SNAPSHOT_FILE)).unwrap(),
        ),
    ];
    for (what, bytes) in cases {
        fs::write(&path, bytes).unwrap();
        match files.answer(&trip) {
            Answer::Silent {
                problem: Some(_), ..
            } => {}
            other => panic!("{what} was taken as {other:?}"),
        }
    }
}

#[test]
fn hill_saying_no_is_heard() {
    let scratch = Scratch::new("refused");
    let files = TravelFiles::new(&scratch.0);
    let trip = files.open(&snapshot()).unwrap();
    let ack = Acknowledgement::refused(
        &trip.seal,
        "0.1.0",
        AckRefusal::UnsupportedVersion { reads: 0 },
    );
    write_document(&trip.dir.join(ACK_FILE), &ack).unwrap();
    let Answer::Silent {
        refusal: Some(refusal),
        problem: None,
    } = files.answer(&trip)
    else {
        panic!("the refusal was not heard");
    };
    assert_eq!(
        refusal_text(&refusal),
        "Formiga Hill 0.1.0 reads travel version 0, and this colony travels as version 1. \
         Update Formiga Hill to take the colony there."
    );
    // An acknowledgement for another trip says nothing about this one.
    let mut stranger = Acknowledgement::refused(&trip.seal, "0.1.0", AckRefusal::Busy);
    stranger.session_id = SessionId::generate().unwrap();
    write_document(&trip.dir.join(ACK_FILE), &stranger).unwrap();
    assert_eq!(
        files.answer(&trip),
        Answer::Silent {
            refusal: None,
            problem: None
        }
    );
}

#[test]
fn a_receipt_adds_one_visit_and_nothing_else() {
    let scratch = Scratch::new("welcome");
    let files = TravelFiles::new(&scratch.0);
    let trip = files.open(&snapshot()).unwrap();
    let now = LEFT + Duration::hours(1);
    let arrived = LEFT + Duration::minutes(1);
    let left = LEFT + Duration::minutes(55);
    let receipt = receipt(
        &trip,
        vec![
            visit(arrived, left),
            visit(arrived, left),
            ReturnEffect::Souvenir {
                id: "acorn-badge".to_owned(),
            },
            ReturnEffect::Keepsake {
                id: "hill-stone".to_owned(),
            },
            ReturnEffect::Unsupported,
        ],
    );
    let welcome = welcome(&trip, &receipt, now);
    assert_eq!(
        welcome.trip,
        Some(formiga_core::Trip {
            session: trip.seal.session_id.to_string(),
            arrived_at_utc: arrived,
            left_at_utc: left,
        })
    );
    assert_eq!(
        welcome.set_aside,
        ["visit", "souvenir", "keepsake", "unsupported"]
    );
}

#[test]
fn a_visit_outside_the_trip_or_never_offered_is_set_aside() {
    let scratch = Scratch::new("outside");
    let files = TravelFiles::new(&scratch.0);
    let mut trip = files.open(&snapshot()).unwrap();
    let now = LEFT + Duration::hours(1);
    for (arrived, left) in [
        (LEFT - Duration::days(3), LEFT),
        (LEFT, now + Duration::days(1)),
    ] {
        let welcome = welcome(&trip, &receipt(&trip, vec![visit(arrived, left)]), now);
        assert_eq!(welcome.trip, None);
        assert_eq!(welcome.set_aside, ["visit"]);
    }
    trip.capabilities.clear();
    let welcome = welcome(
        &trip,
        &receipt(&trip, vec![visit(LEFT, LEFT + Duration::minutes(5))]),
        now,
    );
    assert_eq!(
        welcome.trip, None,
        "a visit is only kept when it was offered"
    );
}

#[test]
fn closing_a_trip_leaves_nothing_and_sweeping_leaves_only_the_open_one() {
    let scratch = Scratch::new("sweep");
    let files = TravelFiles::new(&scratch.0);
    let old = files.open(&snapshot()).unwrap();
    files.close(&old);
    assert_eq!(files.open_trip(), None);
    assert!(!old.dir.exists());

    let open = files.open(&snapshot()).unwrap();
    let stale = scratch
        .0
        .join("travel")
        .join("0123456789abcdef0123456789abcdef");
    fs::create_dir_all(&stale).unwrap();
    fs::write(stale.join(RECEIPT_FILE), b"{}").unwrap();
    fs::write(scratch.0.join("travel").join("trip.json.tmp"), b"half").unwrap();
    // A link left in the travel directory is removed, and what it points at is not touched.
    let outside = scratch.0.join("colony.json");
    fs::write(&outside, b"the colony").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(&scratch.0, scratch.0.join("travel").join("link")).unwrap();
    files.sweep();
    let mut left: Vec<_> = fs::read_dir(scratch.0.join("travel"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    left.sort();
    let mut expected = vec![open.seal.session_id.to_string(), MARKER_FILE.to_owned()];
    expected.sort();
    assert_eq!(left, expected);
    assert_eq!(fs::read(&outside).unwrap(), b"the colony");
}

#[test]
fn calling_the_colony_home_closes_the_trip_and_leaves_hill_its_recall() {
    let scratch = Scratch::new("recall");
    let files = TravelFiles::new(&scratch.0);
    let trip = files.open(&snapshot()).unwrap();
    files.call_home(&trip, RecallReason::OwnerAsked, LEFT + Duration::minutes(3));
    assert_eq!(files.open_trip(), None, "the trip is over for Desktop");
    let recall: Recall = read_document(&trip.dir.join(RECALL_FILE)).unwrap();
    assert_eq!(recall.session_id, trip.seal.session_id);
    assert_eq!(recall.reason, RecallReason::OwnerAsked);
    // The next trip, or the next start, sweeps it away.
    let next = files.open(&snapshot()).unwrap();
    assert!(!trip.dir.exists());
    assert!(next.dir.exists());
}
