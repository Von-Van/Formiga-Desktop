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
//! Desktop starts and before each new visit.

use formiga_core::CreatureId;
use formiga_home_contract::{
    ACK_FILE, AckRefusal, HomeAck, HomeCapability, HomeEffect, HomeError, HomeRecall, HomeReceipt,
    HomeResult, HomeSnapshot, HomeState, RECALL_FILE, RECEIPT_FILE, RESULT_FILE, RecallReason,
    SNAPSHOT_FILE, STATE_FILE, SessionId, SessionSeal, accept_result, read_document,
    write_document,
};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use time::{Duration, OffsetDateTime};

pub const HOME_DIRECTORY: &str = "home";
/// The homes Desktop keeps, under `home/`. The same name as the copy a session directory holds,
/// in a directory of its own.
const KEPT_FILE: &str = "state.json";
const MARKER_FILE: &str = "visit.json";
const MARKER_FORMAT: &str = "formiga.desktop.house-visit";
const MAX_MARKER_BYTES: u64 = 4 * 1024;

/// How far a visit's times may sit outside the visit, for two clocks that disagree a little.
const CLOCK_SLACK: Duration = Duration::minutes(5);

/// What Desktop remembers of an open visit across a restart.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Marker {
    format: String,
    session_id: SessionId,
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
    /// Everyone who went in: the household, and any friend lent for the visit.
    pub away: Vec<CreatureId>,
    /// What this visit's snapshot told Home Desktop would take back.
    pub capabilities: Vec<HomeCapability>,
}

/// Home's own reason for not opening the house.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refusal {
    pub reason: AckRefusal,
    pub home_version: String,
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
    root: PathBuf,
}

impl HouseFiles {
    pub fn new(data_dir: &Path) -> Self {
        Self {
            root: data_dir.join(HOME_DIRECTORY),
        }
    }

    fn marker_path(&self) -> PathBuf {
        self.root.join(MARKER_FILE)
    }

    fn kept_path(&self) -> PathBuf {
        self.root.join(KEPT_FILE)
    }

    /// A session's directory. Safe to build from the identifier, since a [`SessionId`] is never
    /// anything but 32 hex digits.
    pub fn session_dir(&self, session: &SessionId) -> PathBuf {
        self.root.join(session.as_str())
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
        fs::create_dir_all(&self.root)?;
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
        fs::create_dir_all(&self.root)?;
        self.sweep();
        let homes = self.kept(&snapshot.colony_key).settled_for(snapshot);
        let dir = self.session_dir(&snapshot.session_id);
        // A fresh directory every time: an existing one is never reused or trusted.
        fs::create_dir(&dir)?;
        let written = (|| {
            let snapshot_bytes = write_document(&dir.join(SNAPSHOT_FILE), snapshot)?;
            let state_bytes = write_document(&dir.join(STATE_FILE), &homes)?;
            let visit = OpenVisit {
                seal: SessionSeal::of(snapshot, &snapshot_bytes, &state_bytes),
                dir: dir.clone(),
                keeper: snapshot.household.keeper.0,
                away: away.to_vec(),
                capabilities: snapshot.capabilities.clone(),
            };
            let marker = Marker {
                format: MARKER_FORMAT.to_owned(),
                session_id: visit.seal.session_id.clone(),
                snapshot_sha256: visit.seal.snapshot_sha256.clone(),
                state_sha256: visit.seal.state_sha256.clone(),
                created_at_utc: visit.seal.created_at_utc,
                keeper: visit.keeper,
                away: visit.away.clone(),
                capabilities: visit.capabilities.clone(),
            };
            let mut bytes = serde_json::to_vec_pretty(&marker)?;
            bytes.push(b'\n');
            formiga_travel::write_atomically(&self.marker_path(), &bytes)
                .map_err(|error| HomeError::Io(io::Error::other(error.to_string())))?;
            Ok(visit)
        })();
        if written.is_err() {
            let _ = fs::remove_dir_all(&dir);
        }
        written
    }

    /// The visit left open, if Desktop stopped before closing one. A marker that cannot be read
    /// is no visit at all.
    pub fn open_visit(&self) -> Option<OpenVisit> {
        let bytes =
            formiga_home_contract::read_bounded(&self.marker_path(), MAX_MARKER_BYTES).ok()?;
        let marker: Marker = serde_json::from_slice(&bytes).ok()?;
        (marker.format == MARKER_FORMAT).then(|| OpenVisit {
            dir: self.session_dir(&marker.session_id),
            seal: SessionSeal {
                session_id: marker.session_id,
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
                    home_version: ack.home_version,
                })
            });
        answer
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
        self.forget();
    }

    fn forget(&self) {
        if let Err(error) = fs::remove_file(self.marker_path())
            && error.kind() != io::ErrorKind::NotFound
        {
            tracing::warn!(%error, "could not close the visit's marker");
        }
    }

    /// Close a visit once Home has gone, or was never started: the marker goes first, so a visit
    /// is never half-closed and then reopened, and then everything that was written for it.
    pub fn close(&self, visit: &OpenVisit) {
        self.forget();
        if let Err(error) = fs::remove_dir_all(&visit.dir)
            && error.kind() != io::ErrorKind::NotFound
        {
            tracing::warn!(%error, "could not clear the visit's files");
        }
    }

    /// Remove everything under `home/` that is neither the kept homes nor the open visit.
    /// Directories are removed without following links.
    pub fn sweep(&self) {
        let open = self.open_visit();
        let Ok(entries) = fs::read_dir(&self.root) else {
            return;
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let keep = name == MARKER_FILE
                || name == KEPT_FILE
                || open
                    .as_ref()
                    .is_some_and(|visit| name == visit.seal.session_id.as_str());
            if keep {
                continue;
            }
            let path = entry.path();
            let removed = match entry.file_type() {
                Ok(kind) if kind.is_dir() => fs::remove_dir_all(&path),
                _ => fs::remove_file(&path),
            };
            if let Err(error) = removed {
                tracing::warn!(%error, "could not sweep an old visit's file");
            }
        }
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
    let home = if refusal.home_version.is_empty() {
        "Formiga Home".to_owned()
    } else {
        format!("Formiga Home {}", refusal.home_version)
    };
    match refusal.reason {
        AckRefusal::UnsupportedVersion { reads } => format!(
            "{home} reads household version {reads}, which is too old for this colony. Update \
             Formiga Home to open its houses."
        ),
        AckRefusal::Busy => format!("{home} already has a house open."),
        AckRefusal::Invalid => format!("{home} could not read the house it was given."),
        AckRefusal::Other => format!("{home} could not open the house this time."),
    }
}

#[cfg(test)]
mod tests;
