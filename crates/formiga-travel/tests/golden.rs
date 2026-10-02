//! Version 1 as it shipped. These files are never regenerated to make a test pass: a change that
//! breaks one of them breaks every Hill already installed. Set `FORMIGA_TRAVEL_BLESS=1` only
//! when deliberately writing a new version's fixtures, and add them beside these rather than over
//! them.

mod common;

use common::{MADE, colony};
use formiga_travel::*;
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

fn session() -> SessionId {
    SessionId::parse("5eed5eed5eed5eed5eed5eed5eed5eed").unwrap()
}

/// The fixtures as this build writes them.
fn current() -> Vec<(&'static str, Vec<u8>)> {
    let snapshot = project_colony(&colony(3), session(), MADE, "0.66.1").unwrap();
    let snapshot_bytes = encode(&snapshot).unwrap();
    let seal = SnapshotSeal::of(&snapshot, &snapshot_bytes);
    let ack = Acknowledgement::accepted(&seal, "0.1.0");
    let receipt = ReturnReceipt::new(
        &seal,
        datetime!(2026-10-02 11:05 UTC),
        "0.1.0",
        vec![ReturnEffect::Visit {
            arrived_at_utc: datetime!(2026-10-02 9:31 UTC),
            left_at_utc: datetime!(2026-10-02 11:04 UTC),
        }],
    );
    let recall = Recall::new(
        session(),
        datetime!(2026-10-02 11:00 UTC),
        RecallReason::OwnerAsked,
    );
    vec![
        ("snapshot-v1.json", snapshot_bytes),
        ("ack-v1.json", encode(&ack).unwrap()),
        ("receipt-v1.json", encode(&receipt).unwrap()),
        ("recall-v1.json", encode(&recall).unwrap()),
    ]
}

#[test]
fn version_one_is_still_written_exactly_as_it_shipped() {
    let bless = std::env::var_os("FORMIGA_TRAVEL_BLESS").is_some();
    for (name, bytes) in current() {
        if bless {
            std::fs::write(fixture(name), &bytes).unwrap();
            continue;
        }
        assert!(
            read(name) == bytes,
            "{name} is no longer written the way version 1 shipped"
        );
    }
}

#[test]
fn a_version_one_snapshot_still_imports_and_draws() {
    let snapshot: TravelSnapshot = decode(&read("snapshot-v1.json")).unwrap();
    assert_eq!(snapshot.session_id, session());
    assert_eq!(snapshot.travelers.len(), 6);
    assert!(snapshot.offers(Capability::VisitRecord));
    for traveler in &snapshot.travelers {
        let creature = traveler.to_creature().unwrap();
        let dress = traveler.accessory.map(|accessory| accessory.to_art());
        let frame = formiga_art::CreatureRenderer::render_dressed_body_frame(
            &creature.appearance,
            dress,
            formiga_art::BodyClip::Action(formiga_core::ActionKind::Idle),
            0,
            false,
        );
        assert!(
            frame.canvas.alpha_bounds().is_some(),
            "{} draws as something",
            traveler.name
        );
    }
}

#[test]
fn a_snapshot_from_a_newer_desktop_that_version_one_may_read_is_read() {
    let snapshot: TravelSnapshot = decode(&read("snapshot-v2-readable-by-v1.json")).unwrap();
    assert_eq!(snapshot.version, 2);
    assert_eq!(
        snapshot.capabilities,
        vec![Capability::VisitRecord, Capability::Unknown]
    );
    assert_eq!(snapshot.travelers.len(), 6);
}

#[test]
fn a_snapshot_that_needs_a_newer_reader_says_so() {
    match decode::<TravelSnapshot>(&read("snapshot-v2-needs-v2.json")) {
        Err(TravelError::UnsupportedVersion { needs: 2, reads: 1 }) => {}
        other => panic!("expected a clear version refusal, got {other:?}"),
    }
}

#[test]
fn version_one_answers_still_read() {
    let snapshot_bytes = read("snapshot-v1.json");
    let snapshot: TravelSnapshot = decode(&snapshot_bytes).unwrap();
    let seal = SnapshotSeal::of(&snapshot, &snapshot_bytes);
    let ack: Acknowledgement = decode(&read("ack-v1.json")).unwrap();
    assert!(ack.accepted && ack.answers(&seal));
    let receipt: ReturnReceipt = decode(&read("receipt-v1.json")).unwrap();
    assert!(receipt.answers(&seal));
    assert!(matches!(receipt.effects[..], [ReturnEffect::Visit { .. }]));
    let recall: Recall = decode(&read("recall-v1.json")).unwrap();
    assert_eq!(recall.session_id, session());
}

#[test]
fn a_receipt_from_a_newer_hill_keeps_what_this_build_knows() {
    let receipt: ReturnReceipt = decode(&read("receipt-v2-readable-by-v1.json")).unwrap();
    assert_eq!(
        receipt
            .effects
            .iter()
            .map(ReturnEffect::kind)
            .collect::<Vec<_>>(),
        ["visit", "unsupported", "souvenir"]
    );
}
