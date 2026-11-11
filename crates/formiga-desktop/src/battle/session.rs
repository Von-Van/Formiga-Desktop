//! The files of a session in Formiga Battle, on Desktop's side.
//!
//! Everything lives under `battle/` in the data directory, beside the colony and never inside it:
//!
//! - `battle/session.json`, while a session is open: which session it is, and the checksum of the
//!   snapshot Desktop wrote for it, which every result must name, and once Battle has been
//!   started, its process and program. It holds nothing else about the colony, and it is what
//!   lets the next start keep what a session Desktop stopped in the middle of earned, and find
//!   Battle again if it is still open.
//! - `battle/<session>/`, the session directory Battle is given: the roster Desktop wrote, and
//!   Battle's acknowledgement and result.
//!
//! Once Battle has gone, the marker and the session directory are deleted. Anything else under
//! `battle/` is swept away when Desktop starts and before each new session. All of that is done as
//! for any companion app, by [`crate::expansion::files`]; what is written and read is Battle's.

pub use crate::expansion::Refusal;
use crate::expansion::files::VisitFiles;
use crate::platform::companion_app::Started;
use formiga_battle_contract::result::result_limits::MAX_RESULT_BYTES;
use formiga_battle_contract::{
    ACK_FILE, BattleAck, BattleError, BattleResult, BattleSnapshot, RESULT_FILE, SNAPSHOT_FILE,
    SessionId, limits,
};
use formiga_expansion_rulebook::{read_bounded, sha256_hex, write_atomically};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const BATTLE_DIRECTORY: &str = "battle";
pub const MARKER_FILE: &str = "session.json";
pub const MARKER_FORMAT: &str = "formiga.desktop.battle-session";

/// What Desktop remembers of an open session across a restart, beside its session.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
struct Marker {
    snapshot_sha256: String,
    /// Battle's process, once it has been started for the session.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    started: Option<Started>,
}

/// A session that has been written and not yet closed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenSession {
    pub session_id: SessionId,
    /// The checksum of the snapshot Desktop wrote, which a result must name to be applied.
    pub snapshot_sha256: String,
    pub dir: PathBuf,
    /// Battle's process, once it has been started for the session.
    pub started: Option<Started>,
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
            let bytes = snapshot.encode()?;
            write_atomically(&dir.join(SNAPSHOT_FILE), &bytes)?;
            let open = OpenSession {
                session_id: snapshot.session_id.clone(),
                snapshot_sha256: sha256_hex(&bytes),
                dir: dir.to_owned(),
                started: None,
            };
            let marker = Marker {
                snapshot_sha256: open.snapshot_sha256.clone(),
                started: None,
            };
            Ok((open, marker))
        })
    }

    /// The session left open, if Desktop stopped before closing one. A marker that cannot be
    /// read is no session at all.
    pub fn open_session(&self) -> Option<OpenSession> {
        let (session_id, marker) = self.sessions.open_visit()?;
        Some(OpenSession {
            dir: self.sessions.session_dir(&session_id),
            session_id,
            snapshot_sha256: marker.snapshot_sha256,
            started: marker.started,
        })
    }

    /// Remember the process Battle was started as for the open session, so a Desktop restarted
    /// while it is still open can wait for it again rather than close the session under it.
    pub fn remember_start(&self, open: &OpenSession, started: &Started) {
        let marker = Marker {
            snapshot_sha256: open.snapshot_sha256.clone(),
            started: Some(started.clone()),
        };
        if let Err(error) = self.sessions.remember(&open.session_id, marker) {
            tracing::warn!(%error, "could not remember Formiga Battle's process");
        }
    }

    /// What Battle says the session's matches earned, if it has said anything yet. A result that
    /// is damaged, oversized, from a version Desktop cannot read, or for another session or
    /// snapshot is none at all, and why is said.
    pub fn result(&self, open: &OpenSession) -> Result<Option<BattleResult>, String> {
        let bytes = match read_bounded(&open.dir.join(RESULT_FILE), MAX_RESULT_BYTES) {
            Ok(bytes) => bytes,
            Err(formiga_expansion_rulebook::RulebookError::Io(error))
                if error.kind() == std::io::ErrorKind::NotFound =>
            {
                return Ok(None);
            }
            Err(error) => return Err(format!("the result: {error}")),
        };
        let result =
            BattleResult::decode(&bytes).map_err(|error| format!("the result: {error}"))?;
        if result.session_id != open.session_id || result.snapshot_sha256 != open.snapshot_sha256 {
            return Err("a result for another session".to_owned());
        }
        Ok(Some(result))
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
    use crate::scratch::Scratch;
    use formiga_battle_contract::{AckRefusal, sample};

    fn roster() -> BattleSnapshot {
        let mut snapshot = sample::snapshot();
        snapshot.session_id = SessionId::generate().unwrap();
        snapshot
    }

    #[test]
    fn a_session_is_written_whole_and_found_again_after_a_restart() {
        let scratch = Scratch::new("battle-session", "open");
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

    /// Battle's process, once remembered, is found again after a restart with the session, and a
    /// marker written before there was one to remember still reads.
    #[test]
    fn battles_process_is_remembered_with_its_session() {
        let scratch = Scratch::new("battle-session", "started");
        let files = BattleFiles::new(&scratch.0);
        let open = files.open(&roster()).unwrap();
        assert_eq!(files.open_session().unwrap().started, None);
        let started = Started {
            process: 4242,
            program: PathBuf::from("/usr/bin/open"),
        };
        files.remember_start(&open, &started);
        let found = BattleFiles::new(&scratch.0).open_session().unwrap();
        assert_eq!(found.started, Some(started.clone()));
        assert_eq!(found.snapshot_sha256, open.snapshot_sha256);
        // Once the session is closed, nothing is written for it again.
        files.close(&open);
        files.remember_start(&open, &started);
        assert_eq!(files.open_session(), None);
    }

    /// A process that has ended, or one now running another program under the same number, is
    /// not Battle still open.
    #[test]
    fn only_the_process_battle_was_started_as_counts_as_still_open() {
        let mut child = std::process::Command::new(if cfg!(windows) { "cmd" } else { "sleep" })
            .args(if cfg!(windows) {
                &["/C", "ping -n 30 127.0.0.1 >NUL"][..]
            } else {
                &["30"][..]
            })
            .spawn()
            .unwrap();
        let program = which(if cfg!(windows) { "cmd.exe" } else { "sleep" });
        let started = Started {
            process: child.id(),
            program: program.clone(),
        };
        assert!(started.still_running(), "{program:?}");
        let impostor = Started {
            process: child.id(),
            program: PathBuf::from("/somewhere/else"),
        };
        assert!(!impostor.still_running());
        child.kill().unwrap();
        child.wait().unwrap();
        assert!(!started.still_running());
    }

    /// Where a program on the search path is, as the system names a running process.
    fn which(name: &str) -> PathBuf {
        std::env::split_paths(&std::env::var_os("PATH").unwrap())
            .map(|dir| dir.join(name))
            .find(|path| path.is_file())
            .map(|path| std::fs::canonicalize(&path).unwrap_or(path))
            .unwrap()
    }

    #[test]
    fn battles_answer_is_read_only_for_its_own_session() {
        let scratch = Scratch::new("battle-session", "ack");
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
        let scratch = Scratch::new("battle-session", "sweep");
        let files = BattleFiles::new(&scratch.0);
        let first = files.open(&roster()).unwrap();
        files.sessions.forget();
        let second = files.open(&roster()).unwrap();
        assert!(!first.dir.exists());
        assert!(second.dir.join(SNAPSHOT_FILE).is_file());
    }
}
