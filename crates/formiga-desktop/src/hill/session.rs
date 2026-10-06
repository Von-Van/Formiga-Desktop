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
//! before each new trip, so at most one such directory is ever kept. All of that is done as for
//! any companion app, by [`crate::expansion::files`]; what is written, read and kept is Hill's.

pub use crate::expansion::Refusal;
use crate::expansion::files::VisitFiles;
use formiga_travel::{
    ACK_FILE, Acknowledgement, Capability, RECALL_FILE, RECEIPT_FILE, Recall, RecallReason,
    ReturnEffect, ReturnReceipt, SNAPSHOT_FILE, SnapshotSeal, TravelError, TravelSnapshot,
    read_document, write_document,
};
use serde::{Deserialize, Serialize};
use std::io;
use std::path::{Path, PathBuf};
use time::{Duration, OffsetDateTime};

pub const TRAVEL_DIRECTORY: &str = "travel";
pub const MARKER_FILE: &str = "trip.json";
pub const MARKER_FORMAT: &str = "formiga.desktop.trip";

/// How far a visit's times may sit outside the trip, for two clocks that disagree a little.
const CLOCK_SLACK: Duration = Duration::minutes(5);

/// What Desktop remembers of an open trip across a restart, beside its session.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Marker {
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

/// What Desktop will do with a receipt: at most one trip to count, the souvenirs to keep, and the
/// kinds of everything else it set aside unread.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Welcome {
    pub trip: Option<formiga_core::Trip>,
    /// Each souvenir to keep, once, in the order the receipt named them.
    pub souvenirs: Vec<formiga_core::Souvenir>,
    pub set_aside: Vec<&'static str>,
}

/// The travel directory.
pub struct TravelFiles {
    visits: VisitFiles<Marker>,
}

impl TravelFiles {
    pub fn new(data_dir: &Path) -> Self {
        Self {
            visits: VisitFiles::new(data_dir, &super::HILL.folder),
        }
    }

    /// Write a trip: a new session directory holding its snapshot, then the marker that says a
    /// trip is open. Either everything is written or nothing is left behind.
    pub fn open(&self, snapshot: &TravelSnapshot) -> Result<OpenTrip, TravelError> {
        self.visits.open(&snapshot.session_id, |dir| {
            let bytes = write_document(&dir.join(SNAPSHOT_FILE), snapshot)?;
            let trip = OpenTrip {
                seal: SnapshotSeal::of(snapshot, &bytes),
                dir: dir.to_owned(),
                capabilities: snapshot.capabilities.clone(),
            };
            let marker = Marker {
                snapshot_sha256: trip.seal.snapshot_sha256.clone(),
                created_at_utc: trip.seal.created_at_utc,
                capabilities: trip.capabilities.clone(),
            };
            Ok((trip, marker))
        })
    }

    /// The trip left open, if Desktop stopped before closing one. A marker that cannot be read
    /// is no trip at all.
    pub fn open_trip(&self) -> Option<OpenTrip> {
        let (session_id, marker) = self.visits.open_visit()?;
        Some(OpenTrip {
            dir: self.visits.session_dir(&session_id),
            seal: SnapshotSeal {
                session_id,
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
                    version: ack.hill_version,
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
        self.visits.forget();
    }

    /// Close a trip once Hill has gone, or was never started: the marker goes first, so a trip is
    /// never half-closed and then reopened, and then everything that was written for it.
    pub fn close(&self, trip: &OpenTrip) {
        self.visits.close(&trip.dir);
    }

    /// Remove everything under `travel/` that is not the open trip. Directories are removed
    /// without following links.
    pub fn sweep(&self) {
        self.visits.sweep();
    }
}

/// The receipt's effects that Desktop applies, which are only those this trip's snapshot offered
/// and only as Desktop checks them: one visit, inside the trip's own time, and each souvenir this
/// build keeps, once. Everything else is set aside unread.
pub fn welcome(trip: &OpenTrip, receipt: &ReturnReceipt, now: OffsetDateTime) -> Welcome {
    let mut welcome = Welcome::default();
    let records_visits = trip.capabilities.contains(&Capability::VisitRecord);
    let keeps_souvenirs = trip.capabilities.contains(&Capability::Souvenirs);
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
            ReturnEffect::Souvenir { ref id } if keeps_souvenirs => {
                match formiga_core::Souvenir::from_id(id) {
                    Some(souvenir) if welcome.souvenirs.contains(&souvenir) => {}
                    Some(souvenir) => welcome.souvenirs.push(souvenir),
                    None => welcome.set_aside.push(effect.kind()),
                }
            }
            ref other => welcome.set_aside.push(other.kind()),
        }
    }
    welcome
}

/// Hill's refusal, said so the owner can do something about it.
pub fn refusal_text(refusal: &Refusal) -> String {
    super::HILL.refusal_text(refusal)
}

#[cfg(test)]
mod tests;
