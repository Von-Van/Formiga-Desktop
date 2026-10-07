//! The files of a session in Formiga Farm, on Desktop's side.
//!
//! Everything lives under `farm/` in the data directory, beside the colony and never inside it:
//!
//! - `farm/session.json`, while a session is open: which session it is, the seal every answer
//!   must match, and what it was opened for (the companion being reshaped, or the seed of the
//!   stand-in a new companion is drawn over). It is what lets Desktop answer Farm's last proposal
//!   after it has itself been restarted, and it holds nothing else about the colony.
//! - `farm/<session>/`, the session directory Farm is given: the snapshot Desktop wrote, and
//!   Farm's acknowledgement and proposals, and Desktop's verdicts.
//!
//! A session is closed as a visit is: once Farm has gone, the marker and the session directory are
//! deleted; while Farm may still be running, only the marker, leaving the recall where Farm can
//! see it. Anything else under `farm/` is swept away when Desktop starts and before each new
//! session. All of that is done as for any companion app, by [`crate::expansion::files`]; what is
//! written, read and decided is Farm's.

pub use crate::expansion::Refusal;
use crate::expansion::files::VisitFiles;
use formiga_core::CreatureId;
use formiga_farm_contract::{
    ACK_FILE, FarmAck, FarmError, FarmProposal, FarmRecall, FarmSnapshot, FarmVerdict,
    PROPOSAL_FILE, RECALL_FILE, RecallReason, SNAPSHOT_FILE, SessionSeal, VERDICT_FILE, Verdict,
    read_document, write_document,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use time::OffsetDateTime;

pub const FARM_DIRECTORY: &str = "farm";
pub const MARKER_FILE: &str = "session.json";
pub const MARKER_FORMAT: &str = "formiga.desktop.farm-session";

/// What a session was opened for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "for", rename_all = "snake_case")]
pub enum Purpose {
    /// Reshaping this companion.
    Reshape { creature: CreatureId },
    /// Drawing a new companion over the stand-in this seed makes, which is also the seed the
    /// newcomer is made from if the owner welcomes it.
    Draw { stand_in_seed: [u8; 32] },
}

/// What Desktop remembers of an open session across a restart, beside its session.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Marker {
    snapshot_sha256: String,
    purpose: Purpose,
}

/// A session that has been written and not yet closed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenSession {
    pub seal: SessionSeal,
    pub dir: PathBuf,
    pub purpose: Purpose,
}

/// The sessions' directory.
pub struct FarmFiles {
    sessions: VisitFiles<Marker>,
}

impl FarmFiles {
    pub fn new(data_dir: &Path) -> Self {
        Self {
            sessions: VisitFiles::new(data_dir, &super::FARM.folder),
        }
    }

    /// Write a session: a new session directory holding the snapshot, then the marker that says a
    /// session is open. Either everything is written or nothing is left behind.
    pub fn open(
        &self,
        snapshot: &FarmSnapshot,
        purpose: Purpose,
    ) -> Result<OpenSession, FarmError> {
        self.sessions.open(&snapshot.session_id, |dir| {
            let bytes = write_document(&dir.join(SNAPSHOT_FILE), snapshot)?;
            let open = OpenSession {
                seal: SessionSeal::of(snapshot, &bytes),
                dir: dir.to_owned(),
                purpose,
            };
            let marker = Marker {
                snapshot_sha256: open.seal.snapshot_sha256.clone(),
                purpose,
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
            seal: SessionSeal {
                session_id,
                snapshot_sha256: marker.snapshot_sha256,
            },
            purpose: marker.purpose,
        })
    }

    /// The snapshot this session was opened with, exactly as Desktop wrote it: a proposal is only
    /// ever weighed against what was sent.
    pub fn snapshot(&self, open: &OpenSession) -> Result<FarmSnapshot, String> {
        let (snapshot, bytes) = read_document::<FarmSnapshot>(&open.dir.join(SNAPSHOT_FILE))
            .map_err(|error| format!("the session's snapshot: {error}"))?;
        if SessionSeal::of(&snapshot, &bytes) != open.seal {
            return Err("the session's snapshot is not the one Desktop wrote".to_owned());
        }
        Ok(snapshot)
    }

    /// The proposal Farm wrote last and Desktop has not answered yet, if there is one. Each
    /// serial is answered once: a proposal no newer than the last verdict is no news. One that
    /// is damaged, half-written, oversized, from a version Desktop cannot read, or for another
    /// session is none at all, and why goes to the log.
    pub fn unanswered(&self, open: &OpenSession) -> Result<Option<FarmProposal>, String> {
        let proposal = match read_document::<FarmProposal>(&open.dir.join(PROPOSAL_FILE)) {
            Ok((proposal, _)) => proposal,
            Err(error) if error.is_missing() => return Ok(None),
            Err(error) => return Err(format!("the proposal: {error}")),
        };
        if !proposal.answers(&open.seal) {
            return Err("a proposal for another session".to_owned());
        }
        Ok((proposal.serial > self.answered(open)).then_some(proposal))
    }

    /// The newest serial Desktop has answered in this session; none before the first.
    fn answered(&self, open: &OpenSession) -> u32 {
        match read_document::<FarmVerdict>(&open.dir.join(VERDICT_FILE)) {
            Ok((verdict, _)) if verdict.answers(&open.seal, verdict.serial) => verdict.serial,
            _ => 0,
        }
    }

    /// Answer the proposal numbered `serial`, written whole for Farm to find.
    pub fn answer(
        &self,
        open: &OpenSession,
        serial: u32,
        verdict: Verdict,
        now: OffsetDateTime,
    ) -> Result<(), FarmError> {
        let verdict = FarmVerdict::new(&open.seal, serial, now, verdict);
        write_document(&open.dir.join(VERDICT_FILE), &verdict).map(|_| ())
    }

    /// Farm's refusal of the session, if it said no.
    pub fn refusal(&self, open: &OpenSession) -> Option<Refusal> {
        read_document::<FarmAck>(&open.dir.join(ACK_FILE))
            .ok()
            .filter(|(ack, _)| ack.answers(&open.seal))
            .and_then(|(ack, _)| {
                Some(Refusal {
                    reason: ack.refusal?,
                    version: ack.farm_version,
                })
            })
    }

    /// End a session while Farm may still be running: tell Farm it is over, and forget the
    /// session, but leave its directory where Farm can see the recall. Best effort: Farm also
    /// takes its snapshot disappearing as a recall.
    pub fn call_home(&self, open: &OpenSession, reason: RecallReason) {
        let recall = FarmRecall::new(open.seal.session_id.clone(), reason);
        if let Err(error) = write_document(&open.dir.join(RECALL_FILE), &recall) {
            tracing::warn!(%error, "could not leave Formiga Farm a recall");
        }
        self.sessions.forget();
    }

    /// Close a session once Farm has gone, or was never started: the marker goes first, so a
    /// session is never half-closed and then reopened, and then everything written for it.
    pub fn close(&self, open: &OpenSession) {
        self.sessions.close(&open.dir);
    }

    /// Remove everything under `farm/` that is not the open session.
    pub fn sweep(&self) {
        self.sessions.sweep();
    }
}

/// Farm's refusal, said so the owner can do something about it.
pub fn refusal_text(refusal: &Refusal) -> String {
    super::FARM.refusal_text(refusal)
}

#[cfg(test)]
mod tests;
