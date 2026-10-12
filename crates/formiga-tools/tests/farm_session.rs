//! A whole session in Formiga Farm, end to end, against the stand-in Farm: the snapshot written as
//! Desktop writes it, the stub started as Desktop starts Farm, and its proposal decided by the
//! contract's own rule and answered. A kept design changes a companion's look and nothing else,
//! a new companion is only ever a design, and a proposal that does not check out changes nothing.

use formiga_core::forms::{Form, Plan};
use formiga_core::{Creature, SaveFile};
use formiga_farm_contract::{
    ACK_FILE, Accepted, AckRefusal, Current, FarmAck, FarmProposal, FarmRecall, FarmSnapshot,
    FarmVerdict, LAUNCH_ARGUMENT, PROPOSAL_FILE, RECALL_FILE, RecallReason, Rejection, Renderer,
    SNAPSHOT_FILE, SessionId, SessionSeal, VERDICT_FILE, Verdict, accept_proposal, apply_design,
    project_create, project_edit, read_document, sample, write_document,
};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};
use time::OffsetDateTime;

struct Session {
    root: PathBuf,
    dir: PathBuf,
    seal: SessionSeal,
    snapshot: FarmSnapshot,
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// Write a session into a directory of its own, as Desktop does: reshaping the sample colony's
/// first companion, or drawing a new one over the sample stand-in.
fn open(save: &SaveFile, name: &str, reshape: bool) -> Session {
    let root = std::env::temp_dir().join(format!(
        "formiga-farm-session-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    let session = SessionId::generate().unwrap();
    let dir = root.join(session.as_str());
    std::fs::create_dir_all(&dir).unwrap();
    let now = OffsetDateTime::now_utc();
    let snapshot = if reshape {
        project_edit(
            save,
            save.creatures[0].id,
            true,
            session,
            now,
            "0.67.3",
            sample::capabilities(),
            Renderer::this_build(),
        )
    } else {
        project_create(
            save,
            &sample::stand_in(),
            session,
            now,
            "0.67.3",
            sample::capabilities(),
            Renderer::this_build(),
        )
    }
    .unwrap();
    let bytes = write_document(&dir.join(SNAPSHOT_FILE), &snapshot).unwrap();
    Session {
        seal: SessionSeal::of(&snapshot, &bytes),
        root,
        dir,
        snapshot,
    }
}

/// Start the stub as Desktop starts Farm.
fn start(session: &Session, behaviour: &str) -> Child {
    Command::new(env!("CARGO_BIN_EXE_formiga-farm-stub"))
        .arg(LAUNCH_ARGUMENT)
        .arg(&session.dir)
        .env("FORMIGA_FARM_STUB", behaviour)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("the stub starts")
}

fn finish(mut stub: Child) -> Option<i32> {
    stub.wait().expect("the stub ends").code()
}

/// The proposal the stub writes, once it has written one that reads, or `None` if it exits first.
fn proposal(session: &Session, stub: &mut Child) -> Option<FarmProposal> {
    let since = Instant::now();
    while since.elapsed() < Duration::from_secs(20) {
        if let Ok((proposal, _)) = read_document::<FarmProposal>(&session.dir.join(PROPOSAL_FILE)) {
            return Some(proposal);
        }
        if stub.try_wait().unwrap().is_some() {
            return read_document::<FarmProposal>(&session.dir.join(PROPOSAL_FILE))
                .ok()
                .map(|(proposal, _)| proposal);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    None
}

fn answer(session: &Session, serial: u32, verdict: Verdict) {
    let verdict = FarmVerdict::new(&session.seal, serial, OffsetDateTime::now_utc(), verdict);
    write_document(&session.dir.join(VERDICT_FILE), &verdict).unwrap();
}

/// Everything about a companion but its look, and the recipe its origin keeps beside it.
fn who(creature: &Creature) -> serde_json::Value {
    let mut creature = serde_json::to_value(creature).unwrap();
    let fields = creature.as_object_mut().unwrap();
    fields.remove("appearance");
    fields["origin"].as_object_mut().unwrap().remove("design");
    creature
}

#[test]
fn a_reshaped_companion_keeps_everything_but_its_look() {
    let save = sample::colony();
    let session = open(&save, "reshape", true);
    let mut stub = start(&session, "after=0,stay=0,plan=percher");
    let proposal = proposal(&session, &mut stub).expect("a proposal");
    let ack: FarmAck = read_document(&session.dir.join(ACK_FILE)).unwrap().0;
    assert!(ack.accepted && ack.answers(&session.seal));
    let before = save.creatures[0].clone();
    let current = Current::Creature {
        creature: &before,
        available: true,
    };
    let Ok(Accepted::Edit { target, design }) =
        accept_proposal(&session.seal, &session.snapshot, &proposal, current)
    else {
        panic!("an edit Desktop keeps");
    };
    assert_eq!(target, before.id);
    let mut after = before.clone();
    apply_design(&mut after, &design);
    let revision = formiga_core::forms::Design::of(&after.appearance).revision();
    answer(&session, proposal.serial, Verdict::Kept { revision });
    assert_eq!(finish(stub), Some(0));
    assert_eq!(
        after.appearance.sculpt.as_ref().map(|s| s.plan),
        Some(Plan::Percher)
    );
    assert_eq!(
        after.appearance.face, before.appearance.face,
        "the face it wore"
    );
    assert_ne!(after.appearance, before.appearance);
    assert_eq!(who(&after), who(&before), "nobody else");
}

#[test]
fn a_new_companion_drawn_in_farm_is_only_a_design() {
    let save = sample::colony();
    let session = open(&save, "draw", false);
    let mut stub = start(&session, "after=0,stay=0");
    let proposal = proposal(&session, &mut stub).expect("a proposal");
    let Ok(Accepted::Create { design, .. }) =
        accept_proposal(&session.seal, &session.snapshot, &proposal, Current::Nobody)
    else {
        panic!("a design for Desktop's welcome");
    };
    assert!(matches!(design.form, Form::Sculpted { .. }));
    answer(&session, proposal.serial, Verdict::Welcomed);
    assert_eq!(finish(stub), Some(0));
}

#[test]
fn a_proposal_that_does_not_check_out_changes_nothing() {
    let save = sample::colony();
    let creature = &save.creatures[0];
    let current = Current::Creature {
        creature,
        available: true,
    };

    let session = open(&save, "stale", true);
    let mut stub = start(&session, "after=0,stay=0,stale");
    let stale = proposal(&session, &mut stub).expect("a proposal");
    let rejected = accept_proposal(&session.seal, &session.snapshot, &stale, current);
    assert!(
        matches!(rejected, Err(Rejection::Stale { .. })),
        "{rejected:?}"
    );
    answer(&session, stale.serial, rejected.unwrap_err().verdict());
    assert_eq!(finish(stub), Some(0));

    let session = open(&save, "stranger", true);
    let mut stub = start(&session, "after=0,stay=0,stranger");
    let stranger = proposal(&session, &mut stub).expect("a proposal");
    assert!(!stranger.answers(&session.seal));
    assert_eq!(
        accept_proposal(&session.seal, &session.snapshot, &stranger, current),
        Err(Rejection::WrongSession)
    );
    assert_eq!(finish(stub), Some(0));

    let session = open(&save, "garbage", true);
    let mut stub = start(&session, "after=0,stay=0,garbage");
    assert_eq!(proposal(&session, &mut stub), None, "nothing that reads");
    assert_eq!(finish(stub), Some(0));
}

#[test]
fn a_refusal_or_a_recall_ends_a_session_with_nothing_proposed() {
    let save = sample::colony();
    let session = open(&save, "refused", true);
    let stub = start(&session, "refuse=busy");
    assert_eq!(finish(stub), Some(2));
    let ack: FarmAck = read_document(&session.dir.join(ACK_FILE)).unwrap().0;
    assert!(ack.answers(&session.seal));
    assert_eq!(ack.refusal, Some(AckRefusal::Busy));
    assert!(!session.dir.join(PROPOSAL_FILE).exists());

    let session = open(&save, "recalled", true);
    let stub = start(&session, "after=30");
    let since = Instant::now();
    while !session.dir.join(ACK_FILE).exists() && since.elapsed() < Duration::from_secs(20) {
        std::thread::sleep(Duration::from_millis(50));
    }
    let recall = FarmRecall::new(session.seal.session_id.clone(), RecallReason::Closing);
    write_document(&session.dir.join(RECALL_FILE), &recall).unwrap();
    assert_eq!(finish(stub), Some(0));
    assert!(!session.dir.join(PROPOSAL_FILE).exists());
}

#[test]
fn a_farm_that_crashes_after_applying_leaves_a_whole_proposal() {
    let save = sample::colony();
    let session = open(&save, "crash", true);
    let mut stub = start(&session, "after=0,crash");
    let proposal = proposal(&session, &mut stub).expect("a proposal");
    assert_eq!(finish(stub), Some(101));
    assert!(proposal.answers(&session.seal));
}
