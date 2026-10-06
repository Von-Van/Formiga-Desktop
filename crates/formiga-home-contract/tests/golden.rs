//! Every Home version as it was first written. These files are never regenerated to make a test
//! pass: a change that breaks one of them breaks every Home already installed, or every state an
//! older Desktop already keeps. `FORMIGA_HOME_BLESS=1` writes this build's own version's files
//! only, so a new version's fixtures go beside the old ones and never over them.

mod common;

use common::{arranged, closed_at, neighbours_home, session};
use formiga_home_contract::*;
use std::path::PathBuf;
use time::macros::datetime;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn read(name: &str) -> Vec<u8> {
    std::fs::read(fixture(name)).unwrap_or_else(|error| panic!("{name}: {error}"))
}

/// Every version a fixture set has been written for.
const WRITTEN: std::ops::RangeInclusive<u32> = 1..=HOME_FORMAT_VERSION;

/// The fixtures as this build writes them, named for this build's version.
fn written_now() -> Vec<(String, Vec<u8>)> {
    let snapshot = sample::snapshot();
    let mut state = HomeState::new(&snapshot.colony_key);
    state
        .households
        .push(neighbours_home(snapshot.village[1].keeper));
    let snapshot_bytes = encode(&snapshot).unwrap();
    let state_bytes = encode(&state).unwrap();
    let seal = SessionSeal::of(&snapshot, &snapshot_bytes, &state_bytes);
    let ack = HomeAck::accepted(&seal, "0.1.0");
    let mut arranged_state = state.clone();
    let mut home = arranged(&snapshot);
    // Since version 2: the keeper has taken to the armchair.
    for _ in 0..3 {
        home.note_use(snapshot.household.keeper, Liked::Piece { room: 0, uid: 2 });
    }
    // Since version 3: a front door, and a nook behind the room through a doorway.
    home.rooms[0].doors = vec![
        Door {
            side: WallSide::West,
            at: 5,
        },
        Door {
            side: WallSide::North,
            at: 6,
        },
    ];
    home.rooms.push(RoomLayout {
        width: 4,
        depth: 4,
        floor: CatalogId::known("floor.rose"),
        wall: CatalogId::known("wall.stripes"),
        pieces: vec![PlacedPiece {
            uid: 3,
            piece: CatalogId::known("cushion"),
            x: 1,
            y: 1,
            turn: 0,
        }],
        displays: Vec::new(),
        plan: Some(PlanPoint { x: 4, y: -4 }),
        kind: Some(CatalogId::known("room.nook")),
        doors: Vec::new(),
    });
    // Since version 4: a postcard a friend left, pinned up in the nook, and the journal of it.
    let keeper = snapshot.household.keeper;
    let friend = snapshot.visitors[0].id;
    home.mementos.push(Memento {
        serial: 1,
        kind: MementoKind::Postcard,
        by: Some(friend),
        of: Vec::new(),
        inks: Vec::new(),
        made_at_utc: datetime!(2026-11-11 10:12 UTC),
    });
    // And a photo of the keeper and the friend, keeping how each looked.
    home.mementos.push(Memento {
        serial: 2,
        kind: MementoKind::Photo,
        by: None,
        of: vec![keeper, friend],
        inks: vec![
            Ink {
                outline: [74, 58, 52],
                deep: [150, 118, 104],
                body: [222, 196, 178],
                light: [244, 230, 218],
                accent: [214, 120, 132],
            },
            Ink {
                outline: [52, 66, 58],
                deep: [104, 140, 118],
                body: [170, 206, 182],
                light: [214, 236, 220],
                accent: [236, 196, 96],
            },
        ],
        made_at_utc: datetime!(2026-11-11 10:15 UTC),
    });
    home.rooms[1].displays.push(PlacedDisplay {
        item: DisplayId::memento(keeper, 1),
        spot: Spot::Wall {
            side: WallSide::North,
            at: 2,
        },
    });
    home.note(
        datetime!(2026-11-11 9:58 UTC),
        HomeMoment::Visit { visitor: friend },
    );
    home.note(
        datetime!(2026-11-11 10:12 UTC),
        HomeMoment::Memento {
            memento: MementoKind::Postcard,
            by: Some(friend),
            of: Vec::new(),
        },
    );
    // Since version 5: the friend stayed over.
    home.note(
        datetime!(2026-11-11 10:16 UTC),
        HomeMoment::StayedOver { visitor: friend },
    );
    // Since version 6: and was asked to move in.
    home.note(
        datetime!(2026-11-11 10:17 UTC),
        HomeMoment::AskedToMoveIn { visitor: friend },
    );
    arranged_state.set_household(home);
    let result = HomeResult::new(&seal, closed_at(), "0.1.0", arranged_state);
    let receipt = HomeReceipt::new(
        &seal,
        closed_at(),
        "0.1.0",
        vec![
            HomeEffect::HomeVisit {
                household: snapshot.household.keeper,
                arrived_at_utc: datetime!(2026-11-11 9:31 UTC),
                left_at_utc: datetime!(2026-11-11 10:19 UTC),
            },
            // Since version 4.
            HomeEffect::Together {
                a: keeper,
                b: friend,
                together: Together::Play,
                times: 2,
            },
            HomeEffect::Moment {
                moment: HomeMoment::Memento {
                    memento: MementoKind::Postcard,
                    by: Some(friend),
                    of: Vec::new(),
                },
            },
            // Since version 5: and the owner went over to the friend's house.
            HomeEffect::NextDoor { household: friend },
            // Since version 6: having asked the friend to come and live here.
            HomeEffect::MoveIn {
                resident: friend,
                household: keeper,
            },
        ],
    );
    let recall = HomeRecall::new(session(), closed_at(), RecallReason::OwnerAsked);
    let name = |kind: &str| format!("{kind}-v{HOME_FORMAT_VERSION}.json");
    vec![
        (name("snapshot"), snapshot_bytes),
        (name("state"), state_bytes),
        (name("ack"), encode(&ack).unwrap()),
        (name("result"), encode(&result).unwrap()),
        (name("receipt"), encode(&receipt).unwrap()),
        (name("recall"), encode(&recall).unwrap()),
    ]
}

#[test]
fn this_version_is_still_written_exactly_as_it_was_first_written() {
    let bless = std::env::var_os("FORMIGA_HOME_BLESS").is_some();
    for (name, bytes) in written_now() {
        if bless {
            std::fs::write(fixture(&name), &bytes).unwrap();
            continue;
        }
        assert!(
            read(&name) == bytes,
            "{name} is no longer written the way Home version {HOME_FORMAT_VERSION} first was"
        );
    }
}

#[test]
fn every_version_ever_written_still_reads_and_still_answers_its_own_visit() {
    for version in WRITTEN {
        let name = |kind: &str| format!("{kind}-v{version}.json");
        let snapshot_bytes = read(&name("snapshot"));
        let state_bytes = read(&name("state"));
        let snapshot: HomeSnapshot = decode(&snapshot_bytes).unwrap();
        let state: HomeState = decode(&state_bytes).unwrap();
        let seal = SessionSeal::of(&snapshot, &snapshot_bytes, &state_bytes);
        let ack: HomeAck = decode(&read(&name("ack"))).unwrap();
        let result: HomeResult = decode(&read(&name("result"))).unwrap();
        let receipt: HomeReceipt = decode(&read(&name("receipt"))).unwrap();
        let _: HomeRecall = decode(&read(&name("recall"))).unwrap();
        assert!(ack.answers(&seal) && result.answers(&seal) && receipt.answers(&seal));
        let accepted = accept_result(&seal, &snapshot, &state, &result);
        assert!(
            accepted.set_aside.is_empty(),
            "version {version}: {:?}",
            accepted.set_aside
        );
    }
}

#[test]
fn a_document_from_a_newer_home_that_older_readers_may_use_still_reads() {
    let mut value: serde_json::Value = serde_json::from_slice(&read("receipt-v1.json")).unwrap();
    value["version"] = (HOME_FORMAT_VERSION + 3).into();
    value["photos"] = serde_json::json!(["a fine afternoon"]);
    value["effects"][0]["kind"] = "bond_nudge".into();
    let receipt: HomeReceipt = decode(&serde_json::to_vec(&value).unwrap()).unwrap();
    assert_eq!(receipt.effects, vec![HomeEffect::Unsupported]);
}

#[test]
fn a_document_that_needs_a_newer_reader_is_refused_for_its_version() {
    let mut value: serde_json::Value = serde_json::from_slice(&read("state-v1.json")).unwrap();
    value["version"] = (HOME_FORMAT_VERSION + 1).into();
    value["min_reader_version"] = (HOME_FORMAT_VERSION + 1).into();
    value["households"] = "shaped some new way".into();
    assert!(matches!(
        decode::<HomeState>(&serde_json::to_vec(&value).unwrap()),
        Err(HomeError::UnsupportedVersion { .. })
    ));
}
