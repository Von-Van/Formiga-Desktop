//! The travel contract between Formiga Desktop and Formiga Hill.
//!
//! Formiga Desktop owns the living colony. When its owner sends the colony to Formiga Hill,
//! Desktop writes a [`TravelSnapshot`] into a fresh session directory and starts Hill with that
//! directory's path. The snapshot is a deliberate projection of the colony, written for this
//! purpose alone: it is never the colony file, and nothing Desktop adds to the colony file
//! reaches Hill unless it is added here first.
//!
//! # A trip, file by file
//!
//! Everything for one trip lives in one session directory, which Desktop creates inside its own
//! data directory and names after the [`SessionId`]:
//!
//! | File | Written by | When |
//! |---|---|---|
//! | [`SNAPSHOT_FILE`] | Desktop | Before Hill is started. Never changed afterwards. |
//! | [`ACK_FILE`] | Hill | Once Hill has read the snapshot, saying whether it can host it. |
//! | [`RECEIPT_FILE`] | Hill's own host | When the colony comes home. |
//! | [`RECALL_FILE`] | Desktop | If Desktop takes the colony home without a receipt. |
//!
//! Hill is started with two arguments, [`LAUNCH_ARGUMENT`] and the session directory's path,
//! and with nothing else: no other path, no capability, no access to the rest of Desktop's data.
//! Every file is written whole to a temporary name and then renamed into place
//! ([`write_document`]), so a reader sees an old file, a new file, or no file, never half of one.
//!
//! Desktop reads the acknowledgement and the receipt only after Hill has exited, or when it
//! starts up again and finds a trip still open. Whatever goes wrong — Hill missing, refusing the
//! snapshot, crashing, writing a receipt that does not check out — Desktop brings the colony home
//! exactly as it left. A receipt can only ever add the small, fixed set of things listed in
//! [`ReturnEffect`], and Desktop applies only those it names in the snapshot's capabilities.
//!
//! # Versions
//!
//! Every document carries its `format`, the `version` it was written as, and the
//! `min_reader_version` a reader must understand to use it. A reader accepts a document whose
//! `min_reader_version` is at most [`TRAVEL_FORMAT_VERSION`], ignores fields it does not know, and
//! refuses anything else with [`TravelError::UnsupportedVersion`] rather than guessing. A writer
//! that only adds optional fields bumps `version` and leaves `min_reader_version` alone; one that
//! changes or removes anything, or adds a variant to any enum here, bumps both. An enum whose
//! values are only ever matched against, such as [`Trait`], reads a value it does not know as
//! `Unknown` instead, so adding to it needs no bump. A snapshot's `min_reader_version` also rises
//! by itself when a companion is drawn in an edition an older reader cannot draw
//! ([`reader_for_colony`]), so an older Hill turns such a colony away as too new rather than as
//! damaged.
//!
//! | Version | Shipped with | What it added |
//! |---|---|---|
//! | 1 | Desktop 0.66.4 | Everything |
//! | 2 | Desktop 0.66.5 | `trait_ids`: each trait by an identifier as well as in Desktop's words |
//! | 3 | Desktop 0.66.6 | `accepts_souvenirs` and [`Capability::Souvenirs`]: the souvenirs Desktop keeps, by Formiga Hill's own identifiers |
//! | 4 | Desktop 0.67.0 | No new fields: `accepts_souvenirs` also lists the Fairground's four souvenirs |
//!
//! The golden fixtures under `tests/fixtures` are every version as it shipped, and must keep
//! reading.
//!
//! # Bounds
//!
//! Every document is size-limited before it is parsed, every list and string is bounded, and
//! every string is sanitized when it is written and checked again when it is read. See
//! [`limits`].

mod appearance;
mod document;
mod ids;
mod projection;
mod receipt;
pub mod sample;
mod snapshot;
mod text;

pub use appearance::{
    BodyFamily, BrowStyle, CheekStyle, DesignRecipe, EffectMotif, EyeShape, ForelimbStyle,
    HeadAppendageStyle, HighlightStyle, LimbTipStyle, MouthStyle, PatternKind, PupilStyle,
    RestPose, TailStyle, TravelAppearance, TravelFace, TravelForelimbs, TravelHeadAppendages,
    TravelTail,
};
pub use document::{
    Document, TravelError, decode, encode, read_bounded, read_document, sha256_hex,
    write_atomically, write_document,
};
pub use ids::{SessionId, TravelerId};
pub use projection::{ProjectionError, project_colony, reader_for_colony};
pub use receipt::{
    AckRefusal, Acknowledgement, Recall, RecallReason, ReturnEffect, ReturnReceipt, SnapshotSeal,
};
pub use snapshot::{
    AccessoryColors, AccessoryItem, AccessoryKind, Band, Capability, Celebration, Habit,
    Presentation, TemperamentKind, Tension, Theme, Trait, TravelAccessory, TravelAxes,
    TravelCharacter, TravelMotion, TravelRelationship, TravelRole, TravelSnapshot, Traveler,
};
pub use text::{is_sanitized, sanitize_text};

/// The version of every travel document this build writes, and the newest it reads.
pub const TRAVEL_FORMAT_VERSION: u32 = 4;

/// The `format` of each document.
pub const SNAPSHOT_FORMAT: &str = "formiga.travel.snapshot";
pub const ACK_FORMAT: &str = "formiga.travel.ack";
pub const RECEIPT_FORMAT: &str = "formiga.travel.receipt";
pub const RECALL_FORMAT: &str = "formiga.travel.recall";

/// The files of one session directory.
pub const SNAPSHOT_FILE: &str = "snapshot.json";
pub const ACK_FILE: &str = "ack.json";
pub const RECEIPT_FILE: &str = "receipt.json";
pub const RECALL_FILE: &str = "recall.json";

/// The argument Hill is started with, followed by the session directory's absolute path.
pub const LAUNCH_ARGUMENT: &str = "--formiga-travel";

/// How Desktop finds an installed Hill, and learns which travel version it reads, without
/// starting it.
pub mod discovery {
    /// The macOS bundle identifier Desktop asks LaunchServices for.
    pub const MACOS_BUNDLE_ID: &str = "com.formiga.hill";
    /// An integer in Hill's `Info.plist`: the newest travel version it reads.
    pub const MACOS_TRAVEL_VERSION_KEY: &str = "FormigaTravelVersion";
    /// The per-user registry key Hill's Windows installer writes, under `HKEY_CURRENT_USER`, with
    /// the same key under `HKEY_LOCAL_MACHINE` for a machine-wide install.
    pub const WINDOWS_REGISTRY_KEY: &str = r"Software\Formiga\Hill";
    /// `REG_SZ`: the full path of Hill's executable.
    pub const WINDOWS_PATH_VALUE: &str = "Path";
    /// `REG_SZ`: Hill's version, for messages.
    pub const WINDOWS_VERSION_VALUE: &str = "Version";
    /// `REG_DWORD`: the newest travel version it reads.
    pub const WINDOWS_TRAVEL_VERSION_VALUE: &str = "TravelVersion";
    /// For development: the path of a Hill executable (or, on macOS, an `.app`) to use instead
    /// of an installed one. Its travel version is not checked before it is started.
    pub const PATH_OVERRIDE_ENV: &str = "FORMIGA_HILL_PATH";
}

/// The upper bounds every document is held to, on both sides.
pub mod limits {
    /// A whole colony of six with every field filled is under 40 KiB.
    pub const MAX_SNAPSHOT_BYTES: u64 = 256 * 1024;
    pub const MAX_ACK_BYTES: u64 = 4 * 1024;
    pub const MAX_RECEIPT_BYTES: u64 = 32 * 1024;
    pub const MAX_RECALL_BYTES: u64 = 4 * 1024;
    /// Twice Desktop's own colony cap, so a larger colony one day is a version bump on Desktop's
    /// side, not a refusal on Hill's.
    pub const MAX_TRAVELERS: usize = 12;
    pub const MAX_RELATIONSHIPS: usize = MAX_TRAVELERS * (MAX_TRAVELERS - 1) / 2;
    pub const MAX_NAME_CHARS: usize = 24;
    /// A temperament phrase such as "A suspicious but loyal guardian".
    pub const MAX_PHRASE_CHARS: usize = 64;
    pub const MAX_TRAIT_CHARS: usize = 24;
    pub const MAX_TRAITS: usize = 3;
    pub const MAX_HABITS: usize = 8;
    pub const MAX_CAPABILITIES: usize = 16;
    pub const MAX_VERSION_CHARS: usize = 32;
    pub const MAX_EFFECTS: usize = 16;
    /// The souvenirs a snapshot can say Desktop keeps. Desktop keeps seven; the room is for
    /// Formiga Hill's catalogue to grow without a refusal on Hill's side.
    pub const MAX_SOUVENIRS: usize = 64;
    /// A souvenir or keepsake identifier: lowercase letters, digits, `-`, `_` and `.`.
    pub const MAX_REWARD_ID_CHARS: usize = 48;
}
