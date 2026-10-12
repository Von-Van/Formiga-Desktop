//! Every Farm version as it was first written. These files are never regenerated to make a test
//! pass: a change that breaks one of them breaks every Farm already installed. `FORMIGA_FARM_BLESS=1`
//! writes this build's own version's files only, so a new version's fixtures go beside the old
//! ones and never over them.

use formiga_core::forms::{Design, Form, Plan, Sculpt};
use formiga_farm_contract::*;
use std::path::PathBuf;
use time::Duration;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn read(name: &str) -> Vec<u8> {
    std::fs::read(fixture(name)).unwrap_or_else(|error| panic!("{name}: {error}"))
}

/// Every version a fixture set has been written for.
const WRITTEN: std::ops::RangeInclusive<u32> = 1..=FARM_FORMAT_VERSION;

fn sculpted() -> Design {
    Design {
        form: Form::Sculpted {
            sculpt: Sculpt::starter(Plan::Floater),
        },
        face: formiga_core::forms::plain_face(),
    }
}

/// The fixtures as this build writes them, named for this build's version.
fn written_now() -> Vec<(String, Vec<u8>)> {
    let v = FARM_FORMAT_VERSION;
    let edit = sample::edit_snapshot(0);
    let create = sample::create_snapshot();
    let edit_bytes = encode(&edit).unwrap();
    let create_bytes = encode(&create).unwrap();
    let edit_seal = SessionSeal::of(&edit, &edit_bytes);
    let create_seal = SessionSeal::of(&create, &create_bytes);
    let creature = edit.creature().unwrap();
    let at = sample::OPENED + Duration::minutes(7);
    let edit_proposal = FarmProposal::new(
        &edit_seal,
        1,
        at,
        "0.1.0",
        ProposalKind::EditExisting {
            target: creature.id,
            expected_revision: creature.revision.clone(),
        },
        sculpted(),
        Lineage::new(Some("animal.humpback_whale"), Some("Big Splash")),
    );
    let create_proposal = FarmProposal::new(
        &create_seal,
        2,
        at,
        "0.1.0",
        ProposalKind::CreateNew,
        sculpted(),
        Lineage::new(Some("form.floater"), None),
    );
    let current = creature.design().unwrap();
    let mut out = vec![
        (format!("snapshot-edit-v{v}.json"), edit_bytes),
        (format!("snapshot-create-v{v}.json"), create_bytes),
        (
            format!("ack-v{v}.json"),
            encode(&FarmAck::accepted(&edit_seal, "0.1.0")).unwrap(),
        ),
        (
            format!("proposal-edit-v{v}.json"),
            encode(&edit_proposal).unwrap(),
        ),
        (
            format!("proposal-create-v{v}.json"),
            encode(&create_proposal).unwrap(),
        ),
        (
            format!("verdict-kept-v{v}.json"),
            encode(&FarmVerdict::new(
                &edit_seal,
                1,
                at,
                Verdict::Kept {
                    revision: sculpted().revision(),
                },
            ))
            .unwrap(),
        ),
        (
            format!("verdict-stale-v{v}.json"),
            encode(&FarmVerdict::new(
                &edit_seal,
                1,
                at,
                Verdict::Stale {
                    revision: current.revision(),
                    current,
                },
            ))
            .unwrap(),
        ),
        (
            format!("recall-v{v}.json"),
            encode(&FarmRecall::new(
                edit.session_id.clone(),
                RecallReason::Closing,
            ))
            .unwrap(),
        ),
    ];
    out.sort();
    out
}

#[test]
fn this_version_is_still_written_exactly_as_it_was_first_written() {
    let bless = std::env::var_os("FORMIGA_FARM_BLESS").is_some();
    for (name, bytes) in written_now() {
        if bless {
            std::fs::write(fixture(&name), &bytes).unwrap();
            continue;
        }
        assert!(
            read(&name) == bytes,
            "{name} is no longer written the way Farm version {FARM_FORMAT_VERSION} first was"
        );
    }
}

#[test]
fn every_version_ever_written_still_reads_and_still_answers_its_own_session() {
    for version in WRITTEN {
        let name = |kind: &str| format!("{kind}-v{version}.json");
        let edit_bytes = read(&name("snapshot-edit"));
        let edit: FarmSnapshot = decode(&edit_bytes).unwrap();
        let create: FarmSnapshot = decode(&read(&name("snapshot-create"))).unwrap();
        assert!(create.creature().is_none());
        let seal = SessionSeal::of(&edit, &edit_bytes);
        let ack: FarmAck = decode(&read(&name("ack"))).unwrap();
        assert!(ack.answers(&seal) && ack.accepted);
        let proposal: FarmProposal = decode(&read(&name("proposal-edit"))).unwrap();
        assert!(proposal.answers(&seal));
        let _: FarmProposal = decode(&read(&name("proposal-create"))).unwrap();
        let kept: FarmVerdict = decode(&read(&name("verdict-kept"))).unwrap();
        assert!(kept.answers(&seal, proposal.serial));
        let stale: FarmVerdict = decode(&read(&name("verdict-stale"))).unwrap();
        assert!(matches!(stale.verdict, Verdict::Stale { .. }));
        let _: FarmRecall = decode(&read(&name("recall"))).unwrap();
        // And the proposal is still one Desktop would keep for the creature as it was.
        let save = sample::colony();
        let creature = save
            .creatures
            .iter()
            .find(|c| c.id == proposal_target(&proposal))
            .unwrap();
        let current = Current::Creature {
            creature,
            available: true,
        };
        assert!(accept_proposal(&seal, &edit, &proposal, current).is_ok());
    }
}

fn proposal_target(proposal: &FarmProposal) -> u64 {
    match &proposal.kind {
        ProposalKind::EditExisting { target, .. } => target.0,
        ProposalKind::CreateNew => panic!("an edit"),
    }
}

#[test]
fn a_document_from_a_newer_farm_is_refused_for_its_version_and_an_older_reader_reads_additions() {
    let mut value: serde_json::Value =
        serde_json::from_slice(&read("proposal-edit-v1.json")).unwrap();
    value["version"] = (FARM_FORMAT_VERSION + 1).into();
    value["something_new"] = "ignored".into();
    assert!(decode::<FarmProposal>(&serde_json::to_vec(&value).unwrap()).is_ok());
    value["min_reader_version"] = (FARM_FORMAT_VERSION + 1).into();
    assert!(matches!(
        decode::<FarmProposal>(&serde_json::to_vec(&value).unwrap()),
        Err(FarmError::UnsupportedVersion { .. })
    ));
}

#[test]
fn a_newer_desktops_plans_and_capabilities_are_set_aside_rather_than_refused() {
    let mut value: serde_json::Value =
        serde_json::from_slice(&read("snapshot-edit-v1.json")).unwrap();
    value["renderer"]["plans"]
        .as_array_mut()
        .unwrap()
        .push("serpent".into());
    value["capabilities"]
        .as_array_mut()
        .unwrap()
        .push("teleport".into());
    let snapshot: FarmSnapshot = decode(&serde_json::to_vec(&value).unwrap()).unwrap();
    assert_eq!(snapshot.renderer.plans, Plan::ALL.to_vec());
    assert!(snapshot.capabilities.contains(&FarmCapability::Unknown));
}
