//! Every travel version as it shipped. These files are never regenerated to make a test pass: a
//! change that breaks one of them breaks every Hill already installed, or every snapshot an older
//! Desktop already wrote. Version 1 shipped with Desktop 0.66.4, version 2 added each trait's
//! identifier, and version 3 the souvenirs Desktop keeps. `FORMIGA_TRAVEL_BLESS=1` writes this
//! build's own version's files only, so a new version's fixtures go beside the old ones and never
//! over them.

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

/// Every version a fixture set has been written for.
const SHIPPED: std::ops::RangeInclusive<u32> = 1..=TRAVEL_FORMAT_VERSION;

/// The fixtures as this build writes them, named for this build's version.
fn written_now() -> Vec<(String, Vec<u8>)> {
    let snapshot = project_colony(&colony(3), session(), MADE, "0.66.1").unwrap();
    let snapshot_bytes = encode(&snapshot).unwrap();
    let seal = SnapshotSeal::of(&snapshot, &snapshot_bytes);
    let ack = Acknowledgement::accepted(&seal, "0.1.0");
    let receipt = ReturnReceipt::new(
        &seal,
        datetime!(2026-10-02 11:05 UTC),
        "0.1.0",
        vec![
            ReturnEffect::Visit {
                arrived_at_utc: datetime!(2026-10-02 9:31 UTC),
                left_at_utc: datetime!(2026-10-02 11:04 UTC),
            },
            ReturnEffect::Souvenir {
                id: "picnic_ribbon".to_owned(),
            },
        ],
    );
    let recall = Recall::new(
        session(),
        datetime!(2026-10-02 11:00 UTC),
        RecallReason::OwnerAsked,
    );
    let name = |kind: &str| format!("{kind}-v{TRAVEL_FORMAT_VERSION}.json");
    vec![
        (name("snapshot"), snapshot_bytes),
        (name("ack"), encode(&ack).unwrap()),
        (name("receipt"), encode(&receipt).unwrap()),
        (name("recall"), encode(&recall).unwrap()),
    ]
}

#[test]
fn this_version_is_still_written_exactly_as_it_shipped() {
    let bless = std::env::var_os("FORMIGA_TRAVEL_BLESS").is_some();
    for (name, bytes) in written_now() {
        if bless {
            std::fs::write(fixture(&name), &bytes).unwrap();
            continue;
        }
        assert!(
            read(&name) == bytes,
            "{name} is no longer written the way travel version {TRAVEL_FORMAT_VERSION} shipped"
        );
    }
}

#[test]
fn every_shipped_snapshot_still_imports_and_draws() {
    for version in SHIPPED {
        let snapshot: TravelSnapshot = decode(&read(&format!("snapshot-v{version}.json"))).unwrap();
        assert_eq!(snapshot.version, version);
        assert_eq!(snapshot.min_reader_version, 1);
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
                "{} in version {version} draws as something",
                traveler.name
            );
        }
    }
}

#[test]
fn every_shipped_answer_still_reads() {
    for version in SHIPPED {
        let snapshot_bytes = read(&format!("snapshot-v{version}.json"));
        let snapshot: TravelSnapshot = decode(&snapshot_bytes).unwrap();
        let seal = SnapshotSeal::of(&snapshot, &snapshot_bytes);
        let ack: Acknowledgement = decode(&read(&format!("ack-v{version}.json"))).unwrap();
        assert!(ack.accepted && ack.answers(&seal), "version {version}");
        let receipt: ReturnReceipt = decode(&read(&format!("receipt-v{version}.json"))).unwrap();
        assert!(receipt.answers(&seal), "version {version}");
        assert!(matches!(
            receipt.effects.first(),
            Some(ReturnEffect::Visit { .. })
        ));
        let recall: Recall = decode(&read(&format!("recall-v{version}.json"))).unwrap();
        assert_eq!(recall.session_id, session());
    }
}

#[test]
fn version_two_names_every_trait_by_identifier_and_version_one_only_in_words() {
    let old: TravelSnapshot = decode(&read("snapshot-v1.json")).unwrap();
    assert!(
        old.travelers
            .iter()
            .all(|traveler| traveler.character.trait_ids.is_empty()),
        "version 1 never had identifiers, and still reads without them"
    );
    let new: TravelSnapshot = decode(&read("snapshot-v2.json")).unwrap();
    for (before, after) in old.travelers.iter().zip(&new.travelers) {
        assert_eq!(after.character.traits, before.character.traits);
        assert_eq!(
            after.character.trait_ids.len(),
            after.character.traits.len()
        );
        assert!(!after.character.trait_ids.contains(&Trait::Unknown));
    }
}

#[test]
fn version_three_lists_the_souvenirs_desktop_keeps_and_the_older_ones_none() {
    for version in [1, 2] {
        let old: TravelSnapshot = decode(&read(&format!("snapshot-v{version}.json"))).unwrap();
        assert!(old.accepts_souvenirs.is_empty() && !old.offers(Capability::Souvenirs));
        assert!(!old.accepts_souvenir("picnic_ribbon"), "version {version}");
    }
    let new: TravelSnapshot = decode(&read("snapshot-v3.json")).unwrap();
    assert!(new.offers(Capability::Souvenirs));
    assert_eq!(
        new.accepts_souvenirs,
        formiga_core::Souvenir::ALL.map(|souvenir| souvenir.id().to_owned())
    );
    let receipt: ReturnReceipt = decode(&read("receipt-v3.json")).unwrap();
    assert_eq!(
        receipt.effects[1],
        ReturnEffect::Souvenir {
            id: "picnic_ribbon".to_owned()
        }
    );
}

#[test]
fn a_snapshot_from_a_newer_desktop_that_this_build_may_read_is_read() {
    let snapshot: TravelSnapshot = decode(&read("snapshot-future-readable.json")).unwrap();
    assert!(snapshot.version > TRAVEL_FORMAT_VERSION);
    assert_eq!(
        snapshot.capabilities,
        vec![Capability::VisitRecord, Capability::Unknown]
    );
    assert_eq!(snapshot.travelers.len(), 6);
    assert_eq!(
        snapshot.travelers[0].character.trait_ids.last(),
        Some(&Trait::Unknown),
        "a trait this build does not know is read as unknown, not refused"
    );
}

#[test]
fn a_snapshot_that_needs_a_newer_reader_says_so() {
    match decode::<TravelSnapshot>(&read("snapshot-future-needs-newer.json")) {
        Err(TravelError::UnsupportedVersion { needs: 99, reads }) => {
            assert_eq!(reads, TRAVEL_FORMAT_VERSION)
        }
        other => panic!("expected a clear version refusal, got {other:?}"),
    }
}

#[test]
fn a_receipt_from_a_newer_hill_keeps_what_this_build_knows() {
    let receipt: ReturnReceipt = decode(&read("receipt-future-readable.json")).unwrap();
    assert_eq!(
        receipt
            .effects
            .iter()
            .map(ReturnEffect::kind)
            .collect::<Vec<_>>(),
        ["visit", "unsupported", "souvenir"]
    );
}
