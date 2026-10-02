//! The files of a trip, on Desktop's side.
//!
//! Everything lives under `travel/` in the data directory, beside the colony and never inside it:
//!
//! - `travel/trip.json`, while a trip is open: which session it is and the seal every answer must
//!   match. It is what lets Desktop finish a trip cleanly after it has itself been restarted, and
//!   it holds nothing about the colony.
//! - `travel/<session>/`, the session directory Hill is given: the snapshot Desktop wrote, and
//!   whatever Hill writes back.
//!
//! Once Hill has gone, a trip is closed by deleting both. A trip Desktop calls home while Hill may
//! still be running is closed by deleting only the marker: its session directory stays, holding the
//! recall, for Hill to find. Anything under `travel/` that is not the open trip — a called-home
//! session, or one left by a trip that could not be closed — is swept away when Desktop starts and
//! before each new trip, so at most one such directory is ever kept.

use formiga_travel::{
    ACK_FILE, AckRefusal, Acknowledgement, Capability, RECALL_FILE, RECEIPT_FILE, Recall,
    RecallReason, ReturnEffect, ReturnReceipt, SNAPSHOT_FILE, SessionId, SnapshotSeal, TravelError,
    TravelSnapshot, read_document, write_atomically, write_document,
};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use time::{Duration, OffsetDateTime};

pub const TRAVEL_DIRECTORY: &str = "travel";
const MARKER_FILE: &str = "trip.json";
const MARKER_FORMAT: &str = "formiga.desktop.trip";
const MAX_MARKER_BYTES: u64 = 4 * 1024;

/// How far a visit's times may sit outside the trip, for two clocks that disagree a little.
const CLOCK_SLACK: Duration = Duration::minutes(5);

/// What Desktop remembers of an open trip across a restart.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Marker {
    format: String,
    session_id: SessionId,
    snapshot_sha256: String,
    #[serde(with = "time::serde::rfc3339")]
    created_at_utc: OffsetDateTime,
    capabilities: Vec<Capability>,
}

/// A trip that has been written and not yet closed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenTrip {
    pub seal: SnapshotSeal,
    pub dir: PathBuf,
    /// What this trip's snapshot told Hill Desktop would do with a receipt.
    pub capabilities: Vec<Capability>,
}

/// What Hill left for Desktop by the time it had gone.
#[derive(Clone, Debug, PartialEq)]
pub enum Answer {
    /// A receipt that answers this trip exactly.
    Receipt(ReturnReceipt),
    /// No receipt Desktop can use: perhaps Hill said why, perhaps something was found that does
    /// not check out.
    Silent {
        refusal: Option<Refusal>,
        problem: Option<String>,
    },
}

/// Hill's own reason for not taking the colony.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refusal {
    pub reason: AckRefusal,
    pub hill_version: String,
}

/// What Desktop will do with a receipt: at most one trip to count, and the kinds of everything
/// else it set aside unread.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Welcome {
    pub trip: Option<formiga_core::Trip>,
    pub set_aside: Vec<&'static str>,
}

/// The travel directory.
pub struct TravelFiles {
    root: PathBuf,
}

impl TravelFiles {
    pub fn new(data_dir: &Path) -> Self {
        Self {
            root: data_dir.join(TRAVEL_DIRECTORY),
        }
    }

    fn marker_path(&self) -> PathBuf {
        self.root.join(MARKER_FILE)
    }

    /// A session's directory. Safe to build from the identifier, since a [`SessionId`] is never
    /// anything but 32 hex digits.
    pub fn session_dir(&self, session: &SessionId) -> PathBuf {
        self.root.join(session.as_str())
    }

    /// Write a trip: a new session directory holding its snapshot, then the marker that says a
    /// trip is open. Either everything is written or nothing is left behind.
    pub fn open(&self, snapshot: &TravelSnapshot) -> Result<OpenTrip, TravelError> {
        fs::create_dir_all(&self.root)?;
        self.sweep();
        let dir = self.session_dir(&snapshot.session_id);
        // A fresh directory every time: an existing one is never reused or trusted.
        fs::create_dir(&dir)?;
        let written = (|| {
            let bytes = write_document(&dir.join(SNAPSHOT_FILE), snapshot)?;
            let trip = OpenTrip {
                seal: SnapshotSeal::of(snapshot, &bytes),
                dir: dir.clone(),
                capabilities: snapshot.capabilities.clone(),
            };
            let marker = Marker {
                format: MARKER_FORMAT.to_owned(),
                session_id: trip.seal.session_id.clone(),
                snapshot_sha256: trip.seal.snapshot_sha256.clone(),
                created_at_utc: trip.seal.created_at_utc,
                capabilities: trip.capabilities.clone(),
            };
            let mut bytes = serde_json::to_vec_pretty(&marker)?;
            bytes.push(b'\n');
            write_atomically(&self.marker_path(), &bytes)?;
            Ok(trip)
        })();
        if written.is_err() {
            let _ = fs::remove_dir_all(&dir);
        }
        written
    }

    /// The trip left open, if Desktop stopped before closing one. A marker that cannot be read
    /// is no trip at all.
    pub fn open_trip(&self) -> Option<OpenTrip> {
        let bytes = formiga_travel::read_bounded(&self.marker_path(), MAX_MARKER_BYTES).ok()?;
        let marker: Marker = serde_json::from_slice(&bytes).ok()?;
        (marker.format == MARKER_FORMAT).then(|| OpenTrip {
            dir: self.session_dir(&marker.session_id),
            seal: SnapshotSeal {
                session_id: marker.session_id,
                snapshot_sha256: marker.snapshot_sha256,
                created_at_utc: marker.created_at_utc,
            },
            capabilities: marker.capabilities,
        })
    }

    /// What Hill left for this trip. Only a receipt for exactly this snapshot is ever a receipt;
    /// one that is damaged, half-written, oversized, from another version Desktop cannot read, or
    /// for another trip is no answer at all.
    pub fn answer(&self, trip: &OpenTrip) -> Answer {
        let mut problem = None;
        match read_document::<ReturnReceipt>(&trip.dir.join(RECEIPT_FILE)) {
            Ok(receipt) if receipt.answers(&trip.seal) => return Answer::Receipt(receipt),
            Ok(_) => problem = Some("the receipt is for another trip".to_owned()),
            Err(TravelError::Io(error)) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => problem = Some(error.to_string()),
        }
        let refusal = read_document::<Acknowledgement>(&trip.dir.join(ACK_FILE))
            .ok()
            .filter(|ack| ack.answers(&trip.seal))
            .and_then(|ack| {
                Some(Refusal {
                    reason: ack.refusal?,
                    hill_version: ack.hill_version,
                })
            });
        Answer::Silent { refusal, problem }
    }

    /// Close a trip while Hill may still be running: tell Hill it is over, and forget the trip, but
    /// leave its directory where Hill can see the recall. Best effort: the colony is home either
    /// way, and Hill also takes its directory disappearing as a recall.
    pub fn call_home(&self, trip: &OpenTrip, reason: RecallReason, now: OffsetDateTime) {
        let recall = Recall::new(trip.seal.session_id.clone(), now, reason);
        if let Err(error) = write_document(&trip.dir.join(RECALL_FILE), &recall) {
            tracing::warn!(%error, "could not leave Formiga Hill a recall");
        }
        self.forget();
    }

    fn forget(&self) {
        if let Err(error) = fs::remove_file(self.marker_path())
            && error.kind() != io::ErrorKind::NotFound
        {
            tracing::warn!(%error, "could not close the trip's marker");
        }
    }

    /// Close a trip once Hill has gone, or was never started: the marker goes first, so a trip is
    /// never half-closed and then reopened, and then everything that was written for it.
    pub fn close(&self, trip: &OpenTrip) {
        self.forget();
        if let Err(error) = fs::remove_dir_all(&trip.dir)
            && error.kind() != io::ErrorKind::NotFound
        {
            tracing::warn!(%error, "could not clear the trip's files");
        }
    }

    /// Remove everything under `travel/` that is not the open trip. Directories are removed
    /// without following links.
    pub fn sweep(&self) {
        let open = self.open_trip();
        let Ok(entries) = fs::read_dir(&self.root) else {
            return;
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let keep = name == MARKER_FILE
                || open
                    .as_ref()
                    .is_some_and(|trip| name == trip.seal.session_id.as_str());
            if keep {
                continue;
            }
            let path = entry.path();
            let removed = match entry.file_type() {
                Ok(kind) if kind.is_dir() => fs::remove_dir_all(&path),
                _ => fs::remove_file(&path),
            };
            if let Err(error) = removed {
                tracing::warn!(%error, "could not sweep an old travel file");
            }
        }
    }
}

/// The receipt's effects that Desktop applies, which are only those this trip's snapshot offered
/// and only as Desktop checks them: one visit, inside the trip's own time. Everything else is set
/// aside unread.
pub fn welcome(trip: &OpenTrip, receipt: &ReturnReceipt, now: OffsetDateTime) -> Welcome {
    let mut welcome = Welcome::default();
    let records_visits = trip.capabilities.contains(&Capability::VisitRecord);
    let earliest = trip.seal.created_at_utc - CLOCK_SLACK;
    let latest = now + CLOCK_SLACK;
    for effect in &receipt.effects {
        match *effect {
            ReturnEffect::Visit {
                arrived_at_utc,
                left_at_utc,
            } if records_visits
                && welcome.trip.is_none()
                && earliest <= arrived_at_utc
                && left_at_utc <= latest =>
            {
                welcome.trip = Some(formiga_core::Trip {
                    session: trip.seal.session_id.to_string(),
                    arrived_at_utc,
                    left_at_utc,
                });
            }
            ref other => welcome.set_aside.push(other.kind()),
        }
    }
    welcome
}

/// Hill's refusal, said so the owner can do something about it.
pub fn refusal_text(refusal: &Refusal) -> String {
    let hill = if refusal.hill_version.is_empty() {
        "Formiga Hill".to_owned()
    } else {
        format!("Formiga Hill {}", refusal.hill_version)
    };
    match refusal.reason {
        AckRefusal::UnsupportedVersion { reads } => format!(
            "{hill} reads travel version {reads}, and this colony travels as version {}. Update \
             Formiga Hill to take the colony there.",
            formiga_travel::TRAVEL_FORMAT_VERSION
        ),
        AckRefusal::Busy => format!("{hill} is already hosting a colony."),
        AckRefusal::Invalid => format!("{hill} could not read the colony's ticket."),
        AckRefusal::Other => format!("{hill} could not take the colony this time."),
    }
}

#[cfg(test)]
mod tests;
