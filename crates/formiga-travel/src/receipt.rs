//! What Hill says back: that it has the colony, and later what the colony brings home. And what
//! Desktop says if it takes the colony home first.
//!
//! A receipt is written by Hill's own host, after it has checked whatever its content packages
//! asked for. No package ever writes one, and nothing a package wrote is carried in one as text:
//! a receipt is a short list of [`ReturnEffect`]s from a fixed vocabulary.

use crate::document::{Document, TravelError, header_ok, is_sha256_hex};
use crate::ids::SessionId;
use crate::limits::*;
use crate::snapshot::TravelSnapshot;
use crate::text::is_sanitized;
use crate::{ACK_FORMAT, RECALL_FORMAT, RECEIPT_FORMAT, TRAVEL_FORMAT_VERSION};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

/// What every answer must match: the trip, and the exact bytes of the snapshot Desktop wrote for
/// it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SnapshotSeal {
    pub session_id: SessionId,
    pub snapshot_sha256: String,
    pub created_at_utc: OffsetDateTime,
}

impl SnapshotSeal {
    /// The seal of a snapshot, from the bytes it was written as.
    pub fn of(snapshot: &TravelSnapshot, bytes: &[u8]) -> Self {
        Self {
            session_id: snapshot.session_id.clone(),
            snapshot_sha256: crate::sha256_hex(bytes),
            created_at_utc: snapshot.created_at_utc,
        }
    }
}

/// Why Hill could not take the colony.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AckRefusal {
    /// The snapshot needs a newer travel reader than this Hill has; `reads` is the newest it has.
    UnsupportedVersion { reads: u32 },
    /// The snapshot did not check out.
    Invalid,
    /// Hill is already hosting a colony.
    Busy,
    /// Anything a newer Hill says that this build does not know.
    #[serde(other)]
    Other,
}

/// Hill's answer once it has read the snapshot.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Acknowledgement {
    pub format: String,
    pub version: u32,
    pub min_reader_version: u32,
    pub session_id: SessionId,
    /// The SHA-256 of the snapshot's bytes as Hill read them.
    pub snapshot_sha256: String,
    pub hill_version: String,
    pub accepted: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal: Option<AckRefusal>,
}

impl Acknowledgement {
    pub fn accepted(seal: &SnapshotSeal, hill_version: &str) -> Self {
        Self::new(seal, hill_version, None)
    }

    pub fn refused(seal: &SnapshotSeal, hill_version: &str, refusal: AckRefusal) -> Self {
        Self::new(seal, hill_version, Some(refusal))
    }

    fn new(seal: &SnapshotSeal, hill_version: &str, refusal: Option<AckRefusal>) -> Self {
        Self {
            format: ACK_FORMAT.to_owned(),
            version: TRAVEL_FORMAT_VERSION,
            min_reader_version: 1,
            session_id: seal.session_id.clone(),
            snapshot_sha256: seal.snapshot_sha256.clone(),
            hill_version: crate::sanitize_text(hill_version, MAX_VERSION_CHARS),
            accepted: refusal.is_none(),
            refusal,
        }
    }

    /// Whether this acknowledgement is about exactly the snapshot `seal` names.
    pub fn answers(&self, seal: &SnapshotSeal) -> bool {
        self.session_id == seal.session_id && self.snapshot_sha256 == seal.snapshot_sha256
    }
}

impl Document for Acknowledgement {
    const FORMAT: &'static str = ACK_FORMAT;
    const MAX_BYTES: u64 = MAX_ACK_BYTES;

    fn validate(&self) -> Result<(), TravelError> {
        if !header_ok(
            &self.format,
            self.version,
            self.min_reader_version,
            ACK_FORMAT,
        ) || !is_sha256_hex(&self.snapshot_sha256)
            || !is_sanitized(&self.hill_version, MAX_VERSION_CHARS)
            || self.accepted == self.refusal.is_some()
        {
            return Err(TravelError::invalid(
                "an acknowledgement that does not add up",
            ));
        }
        Ok(())
    }
}

/// Something the colony brings home.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ReturnEffect {
    /// The colony spent time at Hill, from when it arrived to when it left. Applied by a Desktop
    /// that offers [`crate::Capability::VisitRecord`].
    Visit {
        #[serde(with = "time::serde::rfc3339")]
        arrived_at_utc: OffsetDateTime,
        #[serde(with = "time::serde::rfc3339")]
        left_at_utc: OffsetDateTime,
    },
    /// One of Formiga Hill's souvenirs, by Formiga Hill's own identifier for it. Desktop keeps
    /// it once, to be looked at, when the trip's snapshot offered [`crate::Capability::Souvenirs`]
    /// and listed it in [`crate::TravelSnapshot::accepts_souvenirs`]. Any other is set aside.
    Souvenir { id: String },
    /// An official keepsake unlocked at Hill, by an identifier Desktop knows. No Desktop knows
    /// any yet, so each is set aside.
    Keepsake { id: String },
    /// Anything a newer Hill sends that this build does not know.
    #[serde(other)]
    Unsupported,
}

impl ReturnEffect {
    /// A short, fixed name for logs.
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Visit { .. } => "visit",
            Self::Souvenir { .. } => "souvenir",
            Self::Keepsake { .. } => "keepsake",
            Self::Unsupported => "unsupported",
        }
    }
}

/// An identifier from a fixed catalogue: lowercase letters, digits, `-`, `_` and `.`.
pub(crate) fn is_reward_id(id: &str) -> bool {
    (1..=MAX_REWARD_ID_CHARS).contains(&id.len())
        && id
            .bytes()
            .all(|byte| matches!(byte, b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.'))
}

/// What Hill's host writes when the colony comes home.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReturnReceipt {
    pub format: String,
    pub version: u32,
    pub min_reader_version: u32,
    pub session_id: SessionId,
    /// The SHA-256 of the snapshot's bytes, so a receipt can only ever answer the one trip.
    pub snapshot_sha256: String,
    #[serde(with = "time::serde::rfc3339")]
    pub returned_at_utc: OffsetDateTime,
    pub hill_version: String,
    #[serde(default)]
    pub effects: Vec<ReturnEffect>,
}

impl ReturnReceipt {
    pub fn new(
        seal: &SnapshotSeal,
        returned_at_utc: OffsetDateTime,
        hill_version: &str,
        effects: Vec<ReturnEffect>,
    ) -> Self {
        Self {
            format: RECEIPT_FORMAT.to_owned(),
            version: TRAVEL_FORMAT_VERSION,
            min_reader_version: 1,
            session_id: seal.session_id.clone(),
            snapshot_sha256: seal.snapshot_sha256.clone(),
            returned_at_utc,
            hill_version: crate::sanitize_text(hill_version, MAX_VERSION_CHARS),
            effects,
        }
    }

    /// Whether this receipt is about exactly the snapshot `seal` names.
    pub fn answers(&self, seal: &SnapshotSeal) -> bool {
        self.session_id == seal.session_id && self.snapshot_sha256 == seal.snapshot_sha256
    }
}

impl Document for ReturnReceipt {
    const FORMAT: &'static str = RECEIPT_FORMAT;
    const MAX_BYTES: u64 = MAX_RECEIPT_BYTES;

    fn validate(&self) -> Result<(), TravelError> {
        let invalid = TravelError::invalid;
        if !header_ok(
            &self.format,
            self.version,
            self.min_reader_version,
            RECEIPT_FORMAT,
        ) || !is_sha256_hex(&self.snapshot_sha256)
            || !is_sanitized(&self.hill_version, MAX_VERSION_CHARS)
        {
            return Err(invalid("a receipt that does not add up"));
        }
        if self.effects.len() > MAX_EFFECTS {
            return Err(invalid("a receipt with too many effects"));
        }
        for effect in &self.effects {
            let fine = match effect {
                ReturnEffect::Visit {
                    arrived_at_utc,
                    left_at_utc,
                } => arrived_at_utc <= left_at_utc,
                ReturnEffect::Souvenir { id } | ReturnEffect::Keepsake { id } => is_reward_id(id),
                ReturnEffect::Unsupported => true,
            };
            if !fine {
                return Err(invalid("a receipt effect that does not add up"));
            }
        }
        Ok(())
    }
}

/// Why Desktop took the colony home without a receipt.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecallReason {
    /// The owner asked for the colony back.
    OwnerAsked,
    /// Desktop started again and found the trip still open.
    DesktopRestarted,
    #[serde(other)]
    Other,
}

/// Desktop's word that the trip is over without a receipt. Hill should close the session; any
/// receipt it writes afterwards is never read.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Recall {
    pub format: String,
    pub version: u32,
    pub min_reader_version: u32,
    pub session_id: SessionId,
    #[serde(with = "time::serde::rfc3339")]
    pub at_utc: OffsetDateTime,
    pub reason: RecallReason,
}

impl Recall {
    pub fn new(session_id: SessionId, at_utc: OffsetDateTime, reason: RecallReason) -> Self {
        Self {
            format: RECALL_FORMAT.to_owned(),
            version: TRAVEL_FORMAT_VERSION,
            min_reader_version: 1,
            session_id,
            at_utc,
            reason,
        }
    }
}

impl Document for Recall {
    const FORMAT: &'static str = RECALL_FORMAT;
    const MAX_BYTES: u64 = MAX_RECALL_BYTES;

    fn validate(&self) -> Result<(), TravelError> {
        if !header_ok(
            &self.format,
            self.version,
            self.min_reader_version,
            RECALL_FORMAT,
        ) {
            return Err(TravelError::invalid("a recall that does not add up"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{decode, encode};
    use time::macros::datetime;

    fn seal() -> SnapshotSeal {
        SnapshotSeal {
            session_id: SessionId::parse("00112233445566778899aabbccddeeff").unwrap(),
            snapshot_sha256: "ab".repeat(32),
            created_at_utc: datetime!(2026-10-02 12:00 UTC),
        }
    }

    #[test]
    fn effects_a_newer_hill_sends_are_read_as_unsupported() {
        let effects: Vec<ReturnEffect> = serde_json::from_str(
            r#"[
                {"kind": "visit", "arrived_at_utc": "2026-10-02T12:00:00Z", "left_at_utc": "2026-10-02T13:00:00Z"},
                {"kind": "photo", "file": "../../colony.json"},
                {"kind": "souvenir", "id": "acorn-badge"}
            ]"#,
        )
        .unwrap();
        assert_eq!(effects[1], ReturnEffect::Unsupported);
        assert_eq!(
            effects[2],
            ReturnEffect::Souvenir {
                id: "acorn-badge".to_owned()
            }
        );
    }

    #[test]
    fn a_receipt_round_trips_and_answers_only_its_own_snapshot() {
        let receipt = ReturnReceipt::new(
            &seal(),
            datetime!(2026-10-02 13:00 UTC),
            "0.1.0",
            vec![ReturnEffect::Visit {
                arrived_at_utc: datetime!(2026-10-02 12:00 UTC),
                left_at_utc: datetime!(2026-10-02 13:00 UTC),
            }],
        );
        let read: ReturnReceipt = decode(&encode(&receipt).unwrap()).unwrap();
        assert_eq!(read, receipt);
        assert!(read.answers(&seal()));
        let mut other = seal();
        other.snapshot_sha256 = "cd".repeat(32);
        assert!(!read.answers(&other));
    }

    #[test]
    fn receipts_that_do_not_add_up_are_refused() {
        let base = ReturnReceipt::new(&seal(), datetime!(2026-10-02 13:00 UTC), "0.1.0", vec![]);
        let mut backwards = base.clone();
        backwards.effects = vec![ReturnEffect::Visit {
            arrived_at_utc: datetime!(2026-10-02 13:00 UTC),
            left_at_utc: datetime!(2026-10-02 12:00 UTC),
        }];
        let mut traversal = base.clone();
        traversal.effects = vec![ReturnEffect::Souvenir {
            id: "../colony".to_owned(),
        }];
        let mut flood = base.clone();
        flood.effects = vec![ReturnEffect::Unsupported; MAX_EFFECTS + 1];
        let mut prose = base.clone();
        prose.hill_version = "0.1.0\u{202E}evil".to_owned();
        let mut wrong = base;
        wrong.format = ACK_FORMAT.to_owned();
        for receipt in [backwards, traversal, flood, prose, wrong] {
            assert!(receipt.validate().is_err(), "{receipt:?} was accepted");
        }
    }

    #[test]
    fn an_acknowledgement_is_accepted_or_says_why_not() {
        let yes = Acknowledgement::accepted(&seal(), "0.1.0");
        let no = Acknowledgement::refused(
            &seal(),
            "0.1.0",
            AckRefusal::UnsupportedVersion { reads: 1 },
        );
        for ack in [&yes, &no] {
            let read: Acknowledgement = decode(&encode(ack).unwrap()).unwrap();
            assert_eq!(&read, ack);
        }
        let mut muddled = yes;
        muddled.refusal = Some(AckRefusal::Busy);
        assert!(muddled.validate().is_err());
    }
}
