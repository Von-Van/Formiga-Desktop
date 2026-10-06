//! The design contract between Formiga Desktop and Formiga Farm.
//!
//! Formiga Desktop owns the living colony. Formiga Farm is a workshop for how its creatures look:
//! it can reshape one that already lives in the colony, or draw a new one for Desktop to welcome.
//! Farm never owns a creature. Desktop opens a session with a [`FarmSnapshot`], a narrow
//! projection of what Farm needs to draw and nothing more, and Farm answers with
//! [`FarmProposal`]s: a design, and what it is for. Desktop alone decides whether to keep one,
//! by [`accept_proposal`], and changes only the creature's appearance when it does.
//!
//! This crate is Desktop's, as `formiga-travel` is the contract Formiga Hill builds against and
//! `formiga-home-contract` the one Formiga Home does, and Formiga Farm takes it by Desktop's
//! release tag. A creature's look travels as the very [`formiga_travel::TravelAppearance`] a trip
//! carries, with its sculpt beside it; the design itself is [`formiga_core::forms::Design`]; and
//! everything a session shares with a trip or a visit (the session's identifier, how documents
//! are bounded, version-checked and written whole, how text is made safe, and the reasons for
//! refusing a session) is `formiga-expansion-rulebook`'s.
//!
//! # A session, file by file
//!
//! Everything for one session lives in one directory, named after its [`SessionId`]:
//!
//! | File | Written by | When |
//! |---|---|---|
//! | [`SNAPSHOT_FILE`] | Desktop | Before Farm is started. Never changed afterwards. |
//! | [`ACK_FILE`] | Farm | Once Farm has read the snapshot, saying whether it can open it. |
//! | [`PROPOSAL_FILE`] | Farm | Each time the owner applies a design, whole. Each has a serial one higher than the last; the newest written is the one that counts. |
//! | [`VERDICT_FILE`] | Desktop | When Desktop has decided on a proposal: kept, or why not. |
//! | [`RECALL_FILE`] | Desktop | If Desktop ends the session first. |
//!
//! Farm is started with two arguments, [`LAUNCH_ARGUMENT`] and the session directory's absolute
//! path, and with nothing else. Every file is written whole to a temporary name and then renamed
//! into place, so a reader sees an old file, a new file, or no file, never half of one.
//!
//! Desktop decides on a proposal while Farm is open, if it is watching, and otherwise when Farm
//! exits; either way it decides on each serial once. A creature is never handed over: it goes on
//! living on the desktop while Farm works on a copy of its design, and if anything goes wrong
//! (Farm missing, refusing, crashing, or writing something that does not check out) the creature
//! simply keeps the look it had.
//!
//! # Versions
//!
//! Every document carries its `format`, the `version` it was written as, and the
//! `min_reader_version` a reader must understand to use it, read exactly as `formiga-travel`
//! reads its own. The snapshot also names the design model version
//! ([`formiga_core::forms::DESIGN_VERSION`]) Desktop draws, and which sculpted plans it can keep.
//!
//! | Version | What it added |
//! |---|---|
//! | 1 | Everything |
//!
//! The golden fixtures under `tests/fixtures` are every version as it was first written, and must
//! keep reading.

mod accept;
mod document;
mod projection;
mod replies;
pub mod sample;
mod snapshot;

pub use accept::{Accepted, Current, Rejection, accept_proposal, apply_design};
pub use document::{
    FarmDocument, FarmError, decode, encode, read_bounded, read_document, sha256_hex,
    write_document,
};
pub use formiga_expansion_rulebook::SessionId;
pub use formiga_travel::{Presentation, TravelerId};
pub use projection::{Opening, ProjectionError, project_create, project_edit};
pub use replies::{
    AckRefusal, FarmAck, FarmProposal, FarmRecall, FarmVerdict, Lineage, ProposalKind,
    RecallReason, SessionSeal, Verdict,
};
pub use snapshot::{Availability, FarmCapability, FarmCreature, FarmMode, FarmSnapshot, Renderer};

/// The version of every Farm document this build writes, and the newest it reads.
pub const FARM_FORMAT_VERSION: u32 = 1;

/// The `format` of each document.
pub const SNAPSHOT_FORMAT: &str = "formiga.farm.snapshot";
pub const ACK_FORMAT: &str = "formiga.farm.ack";
pub const PROPOSAL_FORMAT: &str = "formiga.farm.proposal";
pub const VERDICT_FORMAT: &str = "formiga.farm.verdict";
pub const RECALL_FORMAT: &str = "formiga.farm.recall";

/// The files of one session directory.
pub const SNAPSHOT_FILE: &str = "snapshot.json";
pub const ACK_FILE: &str = "ack.json";
pub const PROPOSAL_FILE: &str = "proposal.json";
pub const VERDICT_FILE: &str = "verdict.json";
pub const RECALL_FILE: &str = "recall.json";

/// The argument Farm is started with, followed by the session directory's absolute path.
pub const LAUNCH_ARGUMENT: &str = "--formiga-farm";

/// How Desktop finds an installed Farm, and learns which Farm version it reads, without starting
/// it: the same arrangement as Formiga Hill's and Formiga Home's, under Farm's own names.
pub mod discovery {
    /// The macOS bundle identifier Desktop asks LaunchServices for.
    pub const MACOS_BUNDLE_ID: &str = "com.formiga.farm";
    /// An integer in Farm's `Info.plist`: the newest Farm version it reads.
    pub const MACOS_FARM_VERSION_KEY: &str = "FormigaFarmVersion";
    /// The per-user registry key Farm's Windows installer writes, under `HKEY_CURRENT_USER`, with
    /// the same key under `HKEY_LOCAL_MACHINE` for a machine-wide install.
    pub const WINDOWS_REGISTRY_KEY: &str = r"Software\Formiga\Farm";
    /// `REG_SZ`: the full path of Farm's executable.
    pub const WINDOWS_PATH_VALUE: &str = "Path";
    /// `REG_SZ`: Farm's version, for messages.
    pub const WINDOWS_VERSION_VALUE: &str = "Version";
    /// `REG_DWORD`: the newest Farm version it reads.
    pub const WINDOWS_FARM_VERSION_VALUE: &str = "FarmVersion";
    /// For development: the path of a Farm executable (or, on macOS, an `.app`) to use instead of
    /// an installed one. Its Farm version is not checked before it is started.
    pub const PATH_OVERRIDE_ENV: &str = "FORMIGA_FARM_PATH";
}

/// The upper bounds every document is held to, on both sides.
pub mod limits {
    /// One creature's look, and the lists of what Desktop can draw, are a few KiB.
    pub const MAX_SNAPSHOT_BYTES: u64 = 64 * 1024;
    pub const MAX_ACK_BYTES: u64 = 4 * 1024;
    /// One design, and the few lines that say what it is for.
    pub const MAX_PROPOSAL_BYTES: u64 = 32 * 1024;
    /// A verdict may carry the creature's newer design back, to compare against.
    pub const MAX_VERDICT_BYTES: u64 = 32 * 1024;
    pub const MAX_RECALL_BYTES: u64 = 4 * 1024;
    /// A companion's name, as a trip carries it.
    pub const MAX_NAME_CHARS: usize = formiga_travel::limits::MAX_NAME_CHARS;
    /// A draft's name, as the owner gives it.
    pub const MAX_DRAFT_NAME_CHARS: usize = 32;
    /// A preset's identifier: lowercase letters, digits, `_` and `.`.
    pub const MAX_PRESET_ID_CHARS: usize = 48;
    pub const MAX_VERSION_CHARS: usize = 32;
    pub const MAX_CAPABILITIES: usize = 16;
    /// A reason given in words, for messages only.
    pub const MAX_REASON_CHARS: usize = 120;
    /// How many proposals one session may make.
    pub const MAX_SERIAL: u32 = 10_000;
}
