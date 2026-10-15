//! The files of a session in Formiga Battle, on Desktop's side.
//!
//! Everything lives under `battle/` in the data directory, beside the colony and never inside it:
//!
//! - `battle/session.json`, while a session is open: which session it is, and nothing else about
//!   the colony. It is what lets the next start clear a session Desktop stopped in the middle of.
//! - `battle/<session>/`, the session directory Battle is given: the roster Desktop wrote, and
//!   Battle's acknowledgement.
//!
//! Once Battle has gone, the marker and the session directory are deleted. Anything else under
//! `battle/` is swept away when Desktop starts and before each new session. All of that is done as
//! for any companion app, by [`crate::expansion::files`]; what is written and read is Battle's.

pub use crate::expansion::Refusal;
use crate::expansion::files::VisitFiles;
use formiga_battle_contract::{
    ACK_FILE, BattleAck, BattleError, BattleSnapshot, SNAPSHOT_FILE, SessionId, limits,
};
use formiga_expansion_rulebook::{read_bounded, write_atomically};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const BATTLE_DIRECTORY: &str = "battle";
pub const MARKER_FILE: &str = "session.json";
pub const MARKER_FORMAT: &str = "formiga.desktop.battle-session";

/// What Desktop remembers of an open session across a restart, beside its session: nothing, for
/// nothing comes back from Battle that would need it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
struct Marker {}

/// A session that has been written and not yet closed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenSession {
    pub session_id: SessionId,
    pub dir: PathBuf,
}

/// The sessions' directory.
pub struct BattleFiles {
    sessions: VisitFiles<Marker>,
}

impl BattleFiles {
    pub fn new(data_dir: &Path) -> Self {
        Self {
            sessions: VisitFiles::new(data_dir, &super::BATTLE.folder),
        }
    }

    /// Write a session: a new session directory holding the roster, then the marker that says a
    /// session is open. Either everything is written or nothing is left behind.
    pub fn open(&self, snapshot: &BattleSnapshot) -> Result<OpenSession, BattleError> {
        self.sessions.open(&snapshot.session_id, |dir| {
            write_atomically(&dir.join(SNAPSHOT_FILE), &snapshot.encode()?)?;
            let open = OpenSession {
                session_id: snapshot.session_id.clone(),
                dir: dir.to_owned(),
            };
            Ok((open, Marker {}))
        })
    }

    /// The session left open, if Desktop stopped before closing one. A marker that cannot be
    /// read is no session at all.
    pub fn open_session(&self) -> Option<OpenSession> {
        let (session_id, Marker {}) = self.sessions.open_visit()?;
        Some(OpenSession {
            dir: self.sessions.session_dir(&session_id),
            session_id,
        })
    }

    /// Battle's refusal of the session, if it said no. An acknowledgement that does not read, or
    /// answers another session, says nothing.
    pub fn refusal(&self, open: &OpenSession) -> Option<Refusal> {
        let bytes = read_bounded(&open.dir.join(ACK_FILE), limits::MAX_ACK_BYTES).ok()?;
        let ack = BattleAck::decode(&bytes).ok()?;
        if ack.session_id != open.session_id {
            return None;
        }
        Some(Refusal {
            reason: ack.refusal?,
            version: ack.battle_version,
        })
    }

    /// Close a session once Battle has gone, or was never started: the marker goes first, then
    /// everything written for it.
    pub fn close(&self, open: &OpenSession) {
        self.sessions.close(&open.dir);
    }

    /// Remove everything under `battle/` that is not the open session.
    pub fn sweep(&self) {
        self.sessions.sweep();
    }
}

/// Battle's refusal, said so the owner can do something about it.
pub fn refusal_text(refusal: &Refusal) -> String {
    super::BATTLE.refusal_text(refusal)
}

#[cfg(test)]
mod tests {
    use super::*;
    use formiga_battle_contract::{AckRefusal, sample};

    /// A data directory of its own under the system's temporary directory, removed when dropped.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "formiga-battle-session-{name}-{}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn roster() -> BattleSnapshot {
        let mut snapshot = sample::snapshot();
        snapshot.session_id = SessionId::generate().unwrap();
        snapshot
    }

    #[test]
    fn a_session_is_written_whole_and_found_again_after_a_restart() {
        let scratch = Scratch::new("open");
        let files = BattleFiles::new(&scratch.0);
        assert_eq!(files.open_session(), None);
        let snapshot = roster();
        let open = files.open(&snapshot).unwrap();
        assert_eq!(
            open.dir,
            scratch.0.join("battle").join(snapshot.session_id.as_str())
        );
        let written = BattleSnapshot::read(&open.dir.join(SNAPSHOT_FILE)).unwrap();
        assert_eq!(written, snapshot);
        assert_eq!(
            BattleFiles::new(&scratch.0).open_session(),
            Some(open.clone())
        );
        files.close(&open);
        assert_eq!(files.open_session(), None);
        assert!(!open.dir.exists());
    }

    #[test]
    fn battles_answer_is_read_only_for_its_own_session() {
        let scratch = Scratch::new("ack");
        let files = BattleFiles::new(&scratch.0);
        let open = files.open(&roster()).unwrap();
        assert_eq!(files.refusal(&open), None, "no answer is no refusal");
        let write = |ack: BattleAck| {
            write_atomically(&open.dir.join(ACK_FILE), &ack.encode().unwrap()).unwrap();
        };
        write(BattleAck::new(open.session_id.clone(), "0.1.0", None));
        assert_eq!(files.refusal(&open), None);
        write(BattleAck::new(
            open.session_id.clone(),
            "0.1.0",
            Some(AckRefusal::Busy),
        ));
        let refusal = files.refusal(&open).unwrap();
        assert_eq!(
            refusal_text(&refusal),
            "Formiga Battle 0.1.0 already has a match open."
        );
        write(BattleAck::new(
            SessionId::generate().unwrap(),
            "0.1.0",
            Some(AckRefusal::Invalid),
        ));
        assert_eq!(files.refusal(&open), None, "another session's answer");
        std::fs::write(open.dir.join(ACK_FILE), b"{\"accepted\":false}").unwrap();
        assert_eq!(files.refusal(&open), None, "an answer that does not read");
    }

    #[test]
    fn an_old_session_is_swept_away_before_a_new_one() {
        let scratch = Scratch::new("sweep");
        let files = BattleFiles::new(&scratch.0);
        let first = files.open(&roster()).unwrap();
        files.sessions.forget();
        let second = files.open(&roster()).unwrap();
        assert!(!first.dir.exists());
        assert!(second.dir.join(SNAPSHOT_FILE).is_file());
    }
}
