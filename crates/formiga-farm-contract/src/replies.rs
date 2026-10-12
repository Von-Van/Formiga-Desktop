//! What passes between the two apps once a session is open: Farm's acknowledgement and its
//! proposals, Desktop's verdict on each proposal, and Desktop's recall if it ends the session.
//!
//! Every one of them is sealed to the session it belongs to: its id, and the exact bytes of the
//! snapshot Desktop wrote for it, so an answer can only ever be about one session and one
//! starting point.

use crate::document::{FarmDocument, FarmError, header_ok, is_lower_hex};
use crate::limits::*;
use crate::{
    ACK_FORMAT, FARM_FORMAT_VERSION, FarmSnapshot, PROPOSAL_FORMAT, RECALL_FORMAT, VERDICT_FORMAT,
};
use formiga_core::forms::Design;
use formiga_expansion_rulebook::{SessionId, is_sanitized, sanitize_text};
use formiga_travel::TravelerId;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

/// What every answer must match: the session, and the exact bytes of the snapshot that opened it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionSeal {
    pub session_id: SessionId,
    pub snapshot_sha256: String,
}

impl SessionSeal {
    /// The seal of a session, from the bytes its snapshot was written as.
    pub fn of(snapshot: &FarmSnapshot, snapshot_bytes: &[u8]) -> Self {
        Self {
            session_id: snapshot.session_id.clone(),
            snapshot_sha256: crate::sha256_hex(snapshot_bytes),
        }
    }

    fn matches(&self, session_id: &SessionId, snapshot: &str) -> bool {
        &self.session_id == session_id && self.snapshot_sha256 == snapshot
    }
}

fn version_text(version: &str) -> String {
    Some(sanitize_text(version, MAX_VERSION_CHARS))
        .filter(|text| !text.is_empty())
        .unwrap_or_else(|| "unknown".to_owned())
}

/// Why Farm could not open the session: `unsupported_version` with the newest Farm version it
/// reads, `invalid` when the snapshot did not check out, `busy` while it has a session open
/// already, and `other` for anything a newer Farm says. Every companion app refuses in the same
/// words, so this is the rulebook's.
pub use formiga_expansion_rulebook::AckRefusal;

/// Farm's answer once it has read the snapshot.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FarmAck {
    pub format: String,
    pub version: u32,
    pub min_reader_version: u32,
    pub session_id: SessionId,
    pub snapshot_sha256: String,
    pub farm_version: String,
    pub accepted: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal: Option<AckRefusal>,
}

impl FarmAck {
    pub fn accepted(seal: &SessionSeal, farm_version: &str) -> Self {
        Self::new(seal, farm_version, None)
    }

    pub fn refused(seal: &SessionSeal, farm_version: &str, refusal: AckRefusal) -> Self {
        Self::new(seal, farm_version, Some(refusal))
    }

    fn new(seal: &SessionSeal, farm_version: &str, refusal: Option<AckRefusal>) -> Self {
        Self {
            format: ACK_FORMAT.to_owned(),
            version: FARM_FORMAT_VERSION,
            min_reader_version: 1,
            session_id: seal.session_id.clone(),
            snapshot_sha256: seal.snapshot_sha256.clone(),
            farm_version: version_text(farm_version),
            accepted: refusal.is_none(),
            refusal,
        }
    }

    /// Whether this acknowledgement is about exactly the session `seal` names.
    pub fn answers(&self, seal: &SessionSeal) -> bool {
        seal.matches(&self.session_id, &self.snapshot_sha256)
    }
}

impl FarmDocument for FarmAck {
    const FORMAT: &'static str = ACK_FORMAT;
    const MAX_BYTES: u64 = MAX_ACK_BYTES;

    fn validate(&self) -> Result<(), FarmError> {
        if !header_ok(
            &self.format,
            self.version,
            self.min_reader_version,
            ACK_FORMAT,
        ) || !is_lower_hex(&self.snapshot_sha256, 64)
            || !is_sanitized(&self.farm_version, MAX_VERSION_CHARS)
            || self.accepted == self.refusal.is_some()
        {
            return Err(FarmError::invalid(
                "an acknowledgement that does not add up",
            ));
        }
        Ok(())
    }
}

/// What a proposed design is for.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProposalKind {
    /// A new look for the creature the session opened on, made from its design as it was at
    /// `expected_revision`.
    EditExisting {
        target: TravelerId,
        expected_revision: String,
    },
    /// A design for a new companion, for Desktop's own welcome to give a name and a life to.
    /// It claims no id, no family and no history: Desktop gives it all of those itself.
    CreateNew,
}

/// Where a design came from, for the owner's own interest. Nothing reads it to decide anything.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lineage {
    /// The preset it was started from, by its identifier: `animal.giant_panda`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preset: Option<String>,
    /// The name the owner gave the draft.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub draft_name: Option<String>,
}

impl Lineage {
    fn is_valid(&self) -> bool {
        self.preset.as_deref().is_none_or(|id| {
            !id.is_empty()
                && id.len() <= MAX_PRESET_ID_CHARS
                && id
                    .bytes()
                    .all(|b| matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'_' | b'.'))
        }) && self
            .draft_name
            .as_deref()
            .is_none_or(|name| !name.is_empty() && is_sanitized(name, MAX_DRAFT_NAME_CHARS))
    }

    /// A lineage with its text made safe, the way it will be written.
    pub fn new(preset: Option<&str>, draft_name: Option<&str>) -> Self {
        let preset = preset.map(str::to_owned);
        let draft_name = draft_name
            .map(|name| sanitize_text(name, MAX_DRAFT_NAME_CHARS))
            .filter(|name| !name.is_empty());
        let lineage = Self { preset, draft_name };
        if lineage.is_valid() {
            lineage
        } else {
            Self {
                preset: None,
                ..lineage
            }
        }
    }
}

/// One design Farm asks Desktop to keep.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FarmProposal {
    pub format: String,
    pub version: u32,
    pub min_reader_version: u32,
    pub session_id: SessionId,
    pub snapshot_sha256: String,
    /// One higher than the session's last proposal: the newest is the one that counts.
    pub serial: u32,
    #[serde(with = "time::serde::rfc3339")]
    pub written_at_utc: OffsetDateTime,
    pub farm_version: String,
    pub kind: ProposalKind,
    pub design: Design,
    #[serde(default)]
    pub lineage: Lineage,
}

impl FarmProposal {
    pub fn new(
        seal: &SessionSeal,
        serial: u32,
        written_at_utc: OffsetDateTime,
        farm_version: &str,
        kind: ProposalKind,
        design: Design,
        lineage: Lineage,
    ) -> Self {
        Self {
            format: PROPOSAL_FORMAT.to_owned(),
            version: FARM_FORMAT_VERSION,
            min_reader_version: 1,
            session_id: seal.session_id.clone(),
            snapshot_sha256: seal.snapshot_sha256.clone(),
            serial,
            written_at_utc,
            farm_version: version_text(farm_version),
            kind,
            design,
            lineage,
        }
    }

    /// Whether this proposal belongs to exactly the session `seal` names.
    pub fn answers(&self, seal: &SessionSeal) -> bool {
        seal.matches(&self.session_id, &self.snapshot_sha256)
    }
}

impl FarmDocument for FarmProposal {
    const FORMAT: &'static str = PROPOSAL_FORMAT;
    const MAX_BYTES: u64 = MAX_PROPOSAL_BYTES;

    fn validate(&self) -> Result<(), FarmError> {
        if !header_ok(
            &self.format,
            self.version,
            self.min_reader_version,
            PROPOSAL_FORMAT,
        ) || !is_lower_hex(&self.snapshot_sha256, 64)
            || !(1..=MAX_SERIAL).contains(&self.serial)
            || !is_sanitized(&self.farm_version, MAX_VERSION_CHARS)
            || !self.lineage.is_valid()
        {
            return Err(FarmError::invalid("a proposal that does not add up"));
        }
        if let ProposalKind::EditExisting {
            expected_revision, ..
        } = &self.kind
            && !is_lower_hex(expected_revision, 32)
        {
            return Err(FarmError::invalid("a proposal names no revision"));
        }
        self.design
            .validate()
            .map_err(|error| FarmError::invalid(format!("the proposed design: {error}")))
    }
}

/// What Desktop decided about one proposal.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Verdict {
    /// The creature has its new look. `revision` is its design's revision now.
    Kept { revision: String },
    /// The design went into Desktop's welcome for a new companion, where the owner names it.
    /// Whether they go on to welcome it is theirs, and Desktop's, to decide.
    Welcomed,
    /// The creature's look changed after the session opened, so the proposal was not made from
    /// it. Its design now, to reopen or compare against.
    Stale { current: Design, revision: String },
    /// The creature is out with another companion app. Nothing changed; it can be proposed again
    /// when it is back.
    Unavailable,
    /// The creature is no longer in the colony.
    Gone,
    /// Desktop cannot draw or keep this design, or did not offer what the proposal asks for.
    Unsupported,
    /// The proposal did not check out.
    Invalid,
    /// Anything a newer Desktop says that this build does not know.
    #[serde(other)]
    Unknown,
}

/// Desktop's answer to one proposal.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FarmVerdict {
    pub format: String,
    pub version: u32,
    pub min_reader_version: u32,
    pub session_id: SessionId,
    pub snapshot_sha256: String,
    /// The proposal it answers.
    pub serial: u32,
    #[serde(with = "time::serde::rfc3339")]
    pub written_at_utc: OffsetDateTime,
    pub verdict: Verdict,
}

impl FarmVerdict {
    pub fn new(
        seal: &SessionSeal,
        serial: u32,
        written_at_utc: OffsetDateTime,
        verdict: Verdict,
    ) -> Self {
        Self {
            format: VERDICT_FORMAT.to_owned(),
            version: FARM_FORMAT_VERSION,
            min_reader_version: 1,
            session_id: seal.session_id.clone(),
            snapshot_sha256: seal.snapshot_sha256.clone(),
            serial,
            written_at_utc,
            verdict,
        }
    }

    /// Whether this verdict answers exactly proposal `serial` of the session `seal` names.
    pub fn answers(&self, seal: &SessionSeal, serial: u32) -> bool {
        seal.matches(&self.session_id, &self.snapshot_sha256) && self.serial == serial
    }
}

impl FarmDocument for FarmVerdict {
    const FORMAT: &'static str = VERDICT_FORMAT;
    const MAX_BYTES: u64 = MAX_VERDICT_BYTES;

    fn validate(&self) -> Result<(), FarmError> {
        if !header_ok(
            &self.format,
            self.version,
            self.min_reader_version,
            VERDICT_FORMAT,
        ) || !is_lower_hex(&self.snapshot_sha256, 64)
            || !(1..=MAX_SERIAL).contains(&self.serial)
        {
            return Err(FarmError::invalid("a verdict that does not add up"));
        }
        match &self.verdict {
            Verdict::Kept { revision } if !is_lower_hex(revision, 32) => {
                Err(FarmError::invalid("a verdict names no revision"))
            }
            Verdict::Stale { current, revision } => {
                current
                    .validate()
                    .map_err(|error| FarmError::invalid(format!("the current design: {error}")))?;
                if current.revision() != *revision {
                    return Err(FarmError::invalid(
                        "a stale verdict's revision is not its design's",
                    ));
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

/// Why Desktop ended a session.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecallReason {
    /// Desktop is closing, or the owner asked from Desktop.
    Closing,
    /// The creature left the colony.
    CreatureGone,
    #[serde(other)]
    Unknown,
}

/// Desktop ending a session before Farm does.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FarmRecall {
    pub format: String,
    pub version: u32,
    pub min_reader_version: u32,
    pub session_id: SessionId,
    pub reason: RecallReason,
}

impl FarmRecall {
    pub fn new(session_id: SessionId, reason: RecallReason) -> Self {
        Self {
            format: RECALL_FORMAT.to_owned(),
            version: FARM_FORMAT_VERSION,
            min_reader_version: 1,
            session_id,
            reason,
        }
    }
}

impl FarmDocument for FarmRecall {
    const FORMAT: &'static str = RECALL_FORMAT;
    const MAX_BYTES: u64 = MAX_RECALL_BYTES;

    fn validate(&self) -> Result<(), FarmError> {
        if !header_ok(
            &self.format,
            self.version,
            self.min_reader_version,
            RECALL_FORMAT,
        ) {
            return Err(FarmError::invalid("a recall that does not add up"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_lineage_keeps_only_safe_text_and_a_well_formed_preset() {
        let lineage = Lineage::new(
            Some("animal.giant_panda"),
            Some("  Pip\u{202e}  the \n second "),
        );
        assert_eq!(lineage.preset.as_deref(), Some("animal.giant_panda"));
        assert_eq!(lineage.draft_name.as_deref(), Some("Pip the second"));
        assert!(lineage.is_valid());
        let odd = Lineage::new(Some("../../etc/passwd"), None);
        assert_eq!(odd.preset, None);
        let empty = Lineage::new(None, Some("\u{200b}"));
        assert_eq!(empty.draft_name, None);
    }
}
