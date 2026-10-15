//! A whole session in Formiga Battle, end to end, against the stand-in Battle: the roster written
//! as Desktop writes it, the stub started as Desktop starts Battle, and its answer read back. A
//! roster Battle can read is accepted, and every way of turning one away is said in an
//! acknowledgement for that session.

use formiga_battle_contract::{
    ACK_FILE, AckRefusal, BATTLE_FORMAT_VERSION, BattleAck, BattleSnapshot, LAUNCH_ARGUMENT,
    SNAPSHOT_FILE, SessionId, project_roster, sample,
};
use std::path::PathBuf;
use std::process::{Command, Output};
use time::OffsetDateTime;

struct Session {
    root: PathBuf,
    dir: PathBuf,
    snapshot: BattleSnapshot,
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// Write the sample colony's roster into a session directory of its own, as Desktop does.
fn open(name: &str) -> Session {
    let root = std::env::temp_dir().join(format!(
        "formiga-battle-session-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    let session = SessionId::generate().unwrap();
    let dir = root.join(session.as_str());
    std::fs::create_dir_all(&dir).unwrap();
    let snapshot = project_roster(
        &sample::colony(),
        session,
        OffsetDateTime::now_utc(),
        "0.67.6",
    )
    .unwrap();
    formiga_travel::write_atomically(&dir.join(SNAPSHOT_FILE), &snapshot.encode().unwrap())
        .unwrap();
    Session {
        root,
        dir,
        snapshot,
    }
}

/// Run the stub as Desktop starts Battle, to the end.
fn run(session: &Session, behaviour: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_formiga-battle-stub"))
        .arg(LAUNCH_ARGUMENT)
        .arg(&session.dir)
        .env("FORMIGA_BATTLE_STUB", behaviour)
        .output()
        .expect("the stub runs")
}

fn ack(session: &Session) -> BattleAck {
    BattleAck::decode(&std::fs::read(session.dir.join(ACK_FILE)).expect("an answer")).unwrap()
}

#[test]
fn the_whole_colony_is_read_and_accepted() {
    let session = open("accept");
    let out = run(&session, "stay=0");
    assert!(out.status.success());
    let ack = ack(&session);
    assert!(ack.accepted && ack.session_id == session.snapshot.session_id);
    let said = String::from_utf8(out.stdout).unwrap();
    let count = session.snapshot.contenders.len();
    assert!(
        said.starts_with(&format!("{count} companions from Formiga Desktop 0.67.6")),
        "{said}"
    );
    for contender in &session.snapshot.contenders {
        assert!(said.contains(&contender.traveler.name), "{said}");
    }
}

#[test]
fn a_busy_battle_says_so_for_this_session() {
    let session = open("busy");
    assert!(!run(&session, "refuse=busy").status.success());
    let ack = ack(&session);
    assert_eq!(ack.refusal, Some(AckRefusal::Busy));
    assert_eq!(ack.session_id, session.snapshot.session_id);
}

#[test]
fn a_roster_that_does_not_check_out_is_turned_away() {
    let session = open("garbage");
    std::fs::write(
        session.dir.join(SNAPSHOT_FILE),
        b"{\"format\":\"formiga.battle.snapshot\"}",
    )
    .unwrap();
    assert!(!run(&session, "stay=0").status.success());
    assert_eq!(ack(&session).refusal, Some(AckRefusal::Invalid));
}

#[test]
fn a_roster_from_a_newer_desktop_is_turned_away_for_its_version() {
    let session = open("newer");
    let mut value = serde_json::to_value(&session.snapshot).unwrap();
    value["version"] = (BATTLE_FORMAT_VERSION + 1).into();
    value["min_reader_version"] = (BATTLE_FORMAT_VERSION + 1).into();
    std::fs::write(
        session.dir.join(SNAPSHOT_FILE),
        serde_json::to_vec(&value).unwrap(),
    )
    .unwrap();
    assert!(!run(&session, "stay=0").status.success());
    assert_eq!(
        ack(&session).refusal,
        Some(AckRefusal::UnsupportedVersion {
            reads: BATTLE_FORMAT_VERSION
        })
    );
}

#[test]
fn a_roster_copied_into_another_sessions_directory_is_not_mistaken_for_it() {
    let session = open("copied");
    let elsewhere = session.root.join(SessionId::generate().unwrap().as_str());
    std::fs::create_dir_all(&elsewhere).unwrap();
    std::fs::copy(
        session.dir.join(SNAPSHOT_FILE),
        elsewhere.join(SNAPSHOT_FILE),
    )
    .unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_formiga-battle-stub"))
        .arg(LAUNCH_ARGUMENT)
        .arg(&elsewhere)
        .env("FORMIGA_BATTLE_STUB", "stay=0")
        .output()
        .unwrap();
    assert!(!out.status.success());
    let ack = BattleAck::decode(&std::fs::read(elsewhere.join(ACK_FILE)).unwrap()).unwrap();
    assert_eq!(ack.refusal, Some(AckRefusal::Invalid));
}
