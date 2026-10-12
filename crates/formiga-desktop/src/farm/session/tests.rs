use super::*;
use formiga_farm_contract::{
    AckRefusal, FarmAck, FarmMode, Lineage, ProposalKind, SessionId, sample,
};
use std::fs;
use time::macros::datetime;

const WRITTEN: OffsetDateTime = datetime!(2026-11-12 10:05 UTC);

/// A directory of its own under the system's temporary directory, removed when dropped.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "formiga-farm-session-{name}-{}",
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

/// The sample session reshaping the sample colony's first companion, written under `scratch`.
fn opened(scratch: &Scratch) -> (FarmFiles, OpenSession, FarmSnapshot) {
    let files = FarmFiles::new(&scratch.0);
    let snapshot = sample::edit_snapshot(0);
    let FarmMode::EditExisting { creature } = &snapshot.mode else {
        unreachable!("an edit session")
    };
    let purpose = Purpose::Reshape {
        creature: creature.id.0,
    };
    let open = files.open(&snapshot, purpose).unwrap();
    (files, open, snapshot)
}

/// What Farm would write as its `serial`th proposal: the companion's own look, made from the
/// revision it was opened at.
fn proposal(snapshot: &FarmSnapshot, seal: &SessionSeal, serial: u32) -> FarmProposal {
    let creature = snapshot.creature().unwrap();
    FarmProposal::new(
        seal,
        serial,
        WRITTEN,
        "0.1.0",
        ProposalKind::EditExisting {
            target: creature.id,
            expected_revision: creature.revision.clone(),
        },
        creature.design().unwrap(),
        Lineage::default(),
    )
}

#[test]
fn a_session_is_written_whole_and_found_again_after_a_restart() {
    let scratch = Scratch::new("open");
    let (files, open, snapshot) = opened(&scratch);
    assert_eq!(
        open.dir,
        scratch.0.join("farm").join(snapshot.session_id.as_str())
    );
    assert_eq!(files.open_session(), Some(open.clone()));
    assert_eq!(
        FarmFiles::new(&scratch.0).open_session(),
        Some(open.clone()),
        "found again by a Desktop that has just started"
    );
    assert_eq!(files.snapshot(&open), Ok(snapshot));
    let marker = fs::read_to_string(scratch.0.join("farm").join(MARKER_FILE)).unwrap();
    assert!(marker.contains("\"for\": \"reshape\""), "{marker}");
}

#[test]
fn a_new_companion_remembers_the_seed_it_will_be_made_from() {
    let scratch = Scratch::new("draw");
    let files = FarmFiles::new(&scratch.0);
    let purpose = Purpose::Draw {
        stand_in_seed: [7; 32],
    };
    files.open(&sample::create_snapshot(), purpose).unwrap();
    assert_eq!(
        FarmFiles::new(&scratch.0).open_session().unwrap().purpose,
        purpose
    );
}

#[test]
fn each_proposal_is_answered_once() {
    let scratch = Scratch::new("serials");
    let (files, open, snapshot) = opened(&scratch);
    assert_eq!(files.unanswered(&open), Ok(None), "nothing proposed yet");
    let first = proposal(&snapshot, &open.seal, 1);
    write_document(&open.dir.join(PROPOSAL_FILE), &first).unwrap();
    assert_eq!(files.unanswered(&open), Ok(Some(first)));
    let revision = snapshot.creature().unwrap().revision.clone();
    files
        .answer(&open, 1, Verdict::Kept { revision }, WRITTEN)
        .unwrap();
    assert_eq!(files.unanswered(&open), Ok(None), "answered already");
    let second = proposal(&snapshot, &open.seal, 2);
    write_document(&open.dir.join(PROPOSAL_FILE), &second).unwrap();
    assert_eq!(files.unanswered(&open), Ok(Some(second)));
}

#[test]
fn a_proposal_that_does_not_check_out_is_no_proposal() {
    let scratch = Scratch::new("strangers");
    let (files, open, snapshot) = opened(&scratch);
    let stranger = SessionSeal {
        session_id: SessionId::generate().unwrap(),
        snapshot_sha256: open.seal.snapshot_sha256.clone(),
    };
    write_document(
        &open.dir.join(PROPOSAL_FILE),
        &proposal(&snapshot, &stranger, 1),
    )
    .unwrap();
    assert!(files.unanswered(&open).is_err(), "for another session");
    fs::write(open.dir.join(PROPOSAL_FILE), b"{\"format\": \"formiga").unwrap();
    assert!(files.unanswered(&open).is_err(), "half-written");
}

#[test]
fn a_snapshot_changed_after_it_was_written_is_not_weighed_against() {
    let scratch = Scratch::new("tampered");
    let (files, open, _) = opened(&scratch);
    let path = open.dir.join(SNAPSHOT_FILE);
    let text = fs::read_to_string(&path).unwrap();
    assert!(text.contains("\"0.67.0\""));
    fs::write(&path, text.replace("\"0.67.0\"", "\"0.67.9\"")).unwrap();
    assert!(files.snapshot(&open).is_err());
}

#[test]
fn a_refusal_is_read_only_for_this_session() {
    let scratch = Scratch::new("refusal");
    let (files, open, _) = opened(&scratch);
    assert_eq!(files.refusal(&open), None);
    let refused = FarmAck::refused(&open.seal, "0.1.0", AckRefusal::Busy);
    write_document(&open.dir.join(ACK_FILE), &refused).unwrap();
    assert_eq!(
        files.refusal(&open),
        Some(Refusal {
            reason: AckRefusal::Busy,
            version: "0.1.0".to_owned(),
        })
    );
    assert_eq!(
        refusal_text(&files.refusal(&open).unwrap()),
        "Formiga Farm 0.1.0 already has a companion open."
    );
}

#[test]
fn a_recall_is_left_where_farm_can_see_it_and_closing_clears_everything() {
    let scratch = Scratch::new("recall");
    let (files, open, _) = opened(&scratch);
    files.call_home(&open, RecallReason::Closing);
    assert!(open.dir.join(RECALL_FILE).is_file());
    assert_eq!(files.open_session(), None, "the marker is gone");
    files.sweep();
    assert!(!open.dir.exists(), "swept before the next session");
    let scratch = Scratch::new("close");
    let (files, open, _) = opened(&scratch);
    files.close(&open);
    assert!(!open.dir.exists());
    assert_eq!(files.open_session(), None);
}
