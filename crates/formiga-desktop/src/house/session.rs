//! The files of a visit to a house, on Desktop's side.
//!
//! Everything lives under `home/` in the data directory, beside the colony and never inside it:
//!
//! - `home/state.json`: every household's home as Desktop last kept it. Written whole, and only
//!   ever as [`formiga_home_contract::accept_result`] leaves it.
//! - `home/visit.json`, while a visit is open: which session it is, the seal every answer must
//!   match, whose house it is and who went in. It is what lets Desktop finish a visit cleanly
//!   after it has itself been restarted, and it holds nothing else about the colony.
//! - `home/<session>/`, the session directory Home is given: the snapshot and the homes Desktop
//!   wrote, and whatever Home writes back.
//!
//! A visit is closed as a trip is: once Home has gone, the marker and the session directory are
//! deleted; while Home may still be running, only the marker, leaving the recall where Home can see
//! it. Anything under `home/` that is neither the kept homes nor the open visit is swept away when
//! Desktop starts and before each new visit. All of that is done as for any companion app, by
//! [`crate::expansion::files`]; what is written, read and kept is Home's.

pub use crate::expansion::Refusal;
use crate::expansion::files::VisitFiles;
use formiga_core::CreatureId;
use formiga_home_contract::{
    ACK_FILE, HomeAck, HomeCapability, HomeEffect, HomeError, HomeIndoors, HomeRecall, HomeReceipt,
    HomeResult, HomeSnapshot, HomeState, INDOORS_FILE, RECALL_FILE, RECEIPT_FILE, RESULT_FILE,
    RecallReason, SNAPSHOT_FILE, STATE_FILE, SessionSeal, accept_result, read_document,
    write_document,
};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use time::{Duration, OffsetDateTime};

pub const HOME_DIRECTORY: &str = "home";
/// The homes Desktop keeps, under `home/`. The same name as the copy a session directory holds,
/// in a directory of its own.
pub const KEPT_FILE: &str = "state.json";
pub const MARKER_FILE: &str = "visit.json";
pub const MARKER_FORMAT: &str = "formiga.desktop.house-visit";

/// How far a visit's times may sit outside the visit, for two clocks that disagree a little.
const CLOCK_SLACK: Duration = Duration::minutes(5);

/// What Desktop remembers of an open visit across a restart, beside its session.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Marker {
    snapshot_sha256: String,
    state_sha256: String,
    #[serde(with = "time::serde::rfc3339")]
    created_at_utc: OffsetDateTime,
    keeper: CreatureId,
    away: Vec<CreatureId>,
    capabilities: Vec<HomeCapability>,
}

/// A visit that has been written and not yet closed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenVisit {
    pub seal: SessionSeal,
    pub dir: PathBuf,
    /// Who keeps the house.
    pub keeper: CreatureId,
    /// Everyone lent for the visit: the household, and any friend from another house. All of
    /// them go in, unless the visit follows Home's word on who is indoors.
    pub away: Vec<CreatureId>,
    /// What this visit's snapshot told Home Desktop would take back.
    pub capabilities: Vec<HomeCapability>,
}

/// What Home left for a visit by the time it had gone: its last result and its receipt, each only
/// if it answers exactly this visit, and its refusal if it said no.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Answer {
    pub result: Option<HomeResult>,
    pub receipt: Option<HomeReceipt>,
    pub refusal: Option<Refusal>,
    /// Anything found that does not check out, for the log.
    pub problems: Vec<String>,
}

/// What Desktop takes from a receipt: at most a line for the visit, and the kinds of everything
/// else it set aside unread.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Welcome {
    pub visited: bool,
    pub set_aside: Vec<&'static str>,
}

/// The visits' directory.
pub struct HouseFiles {
    visits: VisitFiles<Marker>,
}

impl HouseFiles {
    pub fn new(data_dir: &Path) -> Self {
        Self {
            visits: VisitFiles::new(data_dir, &super::HOME.folder),
        }
    }

    fn kept_path(&self) -> PathBuf {
        self.visits.directory().join(KEPT_FILE)
    }

    /// The homes as Desktop last kept them for the colony `colony_key` names: none, if it has
    /// kept none for this colony or the file does not check out.
    pub fn kept(&self, colony_key: &str) -> HomeState {
        match read_document::<HomeState>(&self.kept_path()) {
            Ok((state, _)) if state.colony_key == colony_key => state,
            Ok(_) => HomeState::new(colony_key),
            Err(error) => {
                if !error.is_missing() {
                    tracing::warn!(%error, "the homes Desktop kept could not be read");
                }
                HomeState::new(colony_key)
            }
        }
    }

    /// Keep `state` as the homes, written whole.
    pub fn keep(&self, state: &HomeState) -> Result<(), HomeError> {
        fs::create_dir_all(self.visits.directory())?;
        write_document(&self.kept_path(), state).map(|_| ())
    }

    /// Write a visit: a new session directory holding the snapshot and the homes as Desktop keeps
    /// them, tidied for the colony as it is now, then the marker that says a visit is open. Either
    /// everything is written or nothing is left behind.
    pub fn open(
        &self,
        snapshot: &HomeSnapshot,
        away: &[CreatureId],
    ) -> Result<OpenVisit, HomeError> {
        self.visits.open(&snapshot.session_id, |dir| {
            let homes = self.kept(&snapshot.colony_key).settled_for(snapshot);
            let snapshot_bytes = write_document(&dir.join(SNAPSHOT_FILE), snapshot)?;
            let state_bytes = write_document(&dir.join(STATE_FILE), &homes)?;
            let visit = OpenVisit {
                seal: SessionSeal::of(snapshot, &snapshot_bytes, &state_bytes),
                dir: dir.to_owned(),
                keeper: snapshot.household.keeper.0,
                away: away.to_vec(),
                capabilities: snapshot.capabilities.clone(),
            };
            let marker = Marker {
                snapshot_sha256: visit.seal.snapshot_sha256.clone(),
                state_sha256: visit.seal.state_sha256.clone(),
                created_at_utc: visit.seal.created_at_utc,
                keeper: visit.keeper,
                away: visit.away.clone(),
                capabilities: visit.capabilities.clone(),
            };
            Ok((visit, marker))
        })
    }

    /// The visit left open, if Desktop stopped before closing one. A marker that cannot be read
    /// is no visit at all.
    pub fn open_visit(&self) -> Option<OpenVisit> {
        let (session_id, marker) = self.visits.open_visit()?;
        Some(OpenVisit {
            dir: self.visits.session_dir(&session_id),
            seal: SessionSeal {
                session_id,
                snapshot_sha256: marker.snapshot_sha256,
                state_sha256: marker.state_sha256,
                created_at_utc: marker.created_at_utc,
            },
            keeper: marker.keeper,
            away: marker.away,
            capabilities: marker.capabilities,
        })
    }

    /// What Home left for this visit. Only an answer for exactly this snapshot and these homes is
    /// ever an answer; one that is damaged, half-written, oversized, from a version Desktop cannot
    /// read, or for another visit is none at all.
    pub fn answer(&self, visit: &OpenVisit) -> Answer {
        let mut answer = Answer::default();
        match read_document::<HomeResult>(&visit.dir.join(RESULT_FILE)) {
            Ok((result, _)) if result.answers(&visit.seal) => answer.result = Some(result),
            Ok(_) => answer
                .problems
                .push("a result for another visit".to_owned()),
            Err(error) if error.is_missing() => {}
            Err(error) => answer.problems.push(format!("the result: {error}")),
        }
        match read_document::<HomeReceipt>(&visit.dir.join(RECEIPT_FILE)) {
            Ok((receipt, _)) if receipt.answers(&visit.seal) => answer.receipt = Some(receipt),
            Ok(_) => answer
                .problems
                .push("a receipt for another visit".to_owned()),
            Err(error) if error.is_missing() => {}
            Err(error) => answer.problems.push(format!("the receipt: {error}")),
        }
        answer.refusal = read_document::<HomeAck>(&visit.dir.join(ACK_FILE))
            .ok()
            .filter(|(ack, _)| ack.answers(&visit.seal))
            .and_then(|(ack, _)| {
                Some(Refusal {
                    reason: ack.refusal?,
                    version: ack.home_version,
                })
            });
        answer
    }

    /// Who Home says is in the house just now, if it has said since `seen`, which this returns
    /// moved on: only for a visit that offered to follow it, only those lent for the visit, and
    /// only from a word for exactly this visit. Nothing new, or nothing that checks out, is
    /// `None`, and the house keeps who it has.
    pub fn indoors(
        &self,
        visit: &OpenVisit,
        seen: &mut Option<SystemTime>,
    ) -> Option<Vec<CreatureId>> {
        if !visit.capabilities.contains(&HomeCapability::Indoors) {
            return None;
        }
        let path = visit.dir.join(INDOORS_FILE);
        let modified = fs::metadata(&path).and_then(|meta| meta.modified()).ok()?;
        if *seen == Some(modified) {
            return None;
        }
        *seen = Some(modified);
        match read_document::<HomeIndoors>(&path) {
            Ok((indoors, _)) if indoors.answers(&visit.seal) => Some(
                indoors
                    .indoors
                    .iter()
                    .map(|id| id.0)
                    .filter(|id| visit.away.contains(id))
                    .collect(),
            ),
            Ok(_) => {
                tracing::warn!("Formiga Home said who is indoors for another visit");
                None
            }
            Err(error) => {
                tracing::warn!(%error, "could not read who Formiga Home has indoors");
                None
            }
        }
    }

    /// Keep what `result` may change of the homes, by the contract's own rule, against exactly
    /// the snapshot and homes this visit sent. Returns the kinds of what was set aside, or why
    /// nothing could be kept, in which case the homes stay exactly as they were.
    pub fn keep_result(
        &self,
        visit: &OpenVisit,
        result: &HomeResult,
    ) -> Result<Vec<&'static str>, String> {
        let (snapshot, snapshot_bytes) =
            read_document::<HomeSnapshot>(&visit.dir.join(SNAPSHOT_FILE))
                .map_err(|error| format!("the visit's snapshot: {error}"))?;
        let (sent, state_bytes) = read_document::<HomeState>(&visit.dir.join(STATE_FILE))
            .map_err(|error| format!("the homes the visit was sent: {error}"))?;
        // Desktop's own files, checked again: an answer is only ever weighed against exactly what
        // was sent.
        if SessionSeal::of(&snapshot, &snapshot_bytes, &state_bytes) != visit.seal {
            return Err("the visit's files are not the ones Desktop wrote".to_owned());
        }
        let accepted = accept_result(&visit.seal, &snapshot, &sent, result);
        self.keep(&accepted.state)
            .map_err(|error| format!("the homes could not be kept: {error}"))?;
        Ok(accepted.set_aside)
    }

    /// End a visit while Home may still be running: tell Home it is over, and forget the visit,
    /// but leave its directory where Home can see the recall. Best effort: the household is back
    /// out either way, and Home also takes its snapshot disappearing as a recall.
    pub fn call_home(&self, visit: &OpenVisit, reason: RecallReason, now: OffsetDateTime) {
        let recall = HomeRecall::new(visit.seal.session_id.clone(), now, reason);
        if let Err(error) = write_document(&visit.dir.join(RECALL_FILE), &recall) {
            tracing::warn!(%error, "could not leave Formiga Home a recall");
        }
        self.visits.forget();
    }

    /// Close a visit once Home has gone, or was never started: the marker goes first, so a visit
    /// is never half-closed and then reopened, and then everything that was written for it.
    pub fn close(&self, visit: &OpenVisit) {
        self.visits.close(&visit.dir);
    }

    /// Remove everything under `home/` that is neither the kept homes nor the open visit.
    /// Directories are removed without following links.
    pub fn sweep(&self) {
        self.visits.sweep();
    }
}

/// The receipt's effects that Desktop applies, which are only those this visit's snapshot offered
/// and only as Desktop checks them: one visit, to this house, inside the visit's own time.
/// Everything else is set aside unread.
pub fn welcome(visit: &OpenVisit, receipt: &HomeReceipt, now: OffsetDateTime) -> Welcome {
    let mut welcome = Welcome::default();
    let records_visits = visit.capabilities.contains(&HomeCapability::VisitRecord);
    let earliest = visit.seal.created_at_utc - CLOCK_SLACK;
    let latest = now + CLOCK_SLACK;
    for effect in &receipt.effects {
        match *effect {
            HomeEffect::HomeVisit {
                household,
                arrived_at_utc,
                left_at_utc,
            } if records_visits
                && !welcome.visited
                && household.0 == visit.keeper
                && earliest <= arrived_at_utc
                && arrived_at_utc <= left_at_utc
                && left_at_utc <= latest =>
            {
                welcome.visited = true;
            }
            ref other => welcome.set_aside.push(effect_kind(other)),
        }
    }
    welcome
}

/// A short, fixed name for an effect, for the log.
fn effect_kind(effect: &HomeEffect) -> &'static str {
    match effect {
        HomeEffect::HomeVisit { .. } => "home_visit",
        HomeEffect::Together { .. } => "together",
        HomeEffect::Moment { .. } => "moment",
        HomeEffect::NextDoor { .. } => "next_door",
        HomeEffect::MoveIn { .. } => "move_in",
        HomeEffect::Unsupported => "unsupported",
    }
}

/// Home's refusal, said so the owner can do something about it.
pub fn refusal_text(refusal: &Refusal) -> String {
    super::HOME.refusal_text(refusal)
}

#[cfg(test)]
mod tests;
