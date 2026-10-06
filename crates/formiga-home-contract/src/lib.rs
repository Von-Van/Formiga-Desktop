//! The household contract between Formiga Desktop and Formiga Home.
//!
//! Formiga Desktop owns the living colony. When its owner clicks a house in the village, Desktop
//! writes a [`HomeSnapshot`] of the household that lives there, and a copy of the Home layouts it
//! keeps, into a fresh session directory, and starts Home with that directory's path. The snapshot
//! is a deliberate projection of one household, written for this purpose alone: it is never the
//! colony file, and nothing Desktop adds to the colony file reaches Home unless it is added here
//! first. Home never reads the colony file, and never writes anything but its answers.
//!
//! This crate is Desktop's, as `formiga-travel` is the contract Formiga Hill builds against, and
//! Formiga Home takes it by Desktop's release tag. A resident is drawn from the very
//! [`formiga_travel::Traveler`] a trip would carry, and everything every visit shares with a trip
//! (the session's identifier, how documents are bounded, version-checked and written whole, how
//! text is made safe, and the reasons for refusing a visit) is `formiga-expansion-rulebook`'s.
//!
//! # A visit, file by file
//!
//! Everything for one visit lives in one session directory, named after its [`SessionId`]:
//!
//! | File | Written by | When |
//! |---|---|---|
//! | [`SNAPSHOT_FILE`] | Desktop | Before Home is started. Never changed afterwards. |
//! | [`STATE_FILE`] | Desktop | Before Home is started: the Home layouts Desktop last accepted. Never changed afterwards. |
//! | [`ACK_FILE`] | Home | Once Home has read both, saying whether it can host the household. |
//! | [`RESULT_FILE`] | Home | Whenever the owner finishes arranging, and once more on leaving: every layout as Home would have them, whole. The last one written is Home's answer. |
//! | [`RECEIPT_FILE`] | Home | When the owner leaves the house. |
//! | [`RECALL_FILE`] | Desktop | If Desktop ends the visit first. |
//!
//! Home is started with two arguments, [`LAUNCH_ARGUMENT`] and the session directory's absolute
//! path, and with nothing else. Every file is written whole to a temporary name and then renamed
//! into place, so a reader sees an old file, a new file, or no file, never half of one.
//!
//! Desktop reads Home's answers once Home has exited, or when it starts again and finds a visit
//! still open. Whatever goes wrong — Home missing, refusing, crashing, or writing something that
//! does not check out — the household goes back to living on the desktop, and the layouts Desktop
//! last accepted stay as they were. A result is applied only through [`accept_result`], which
//! keeps nothing Home may not change; a receipt can only add the small, fixed set of things in
//! [`HomeEffect`], and only those the snapshot's capabilities offer.
//!
//! # Versions
//!
//! Every document carries its `format`, the `version` it was written as, and the
//! `min_reader_version` a reader must understand to use it, read exactly as `formiga-travel`
//! reads its own: a reader accepts a document whose `min_reader_version` is at most
//! [`HOME_FORMAT_VERSION`], ignores fields it does not know, and refuses anything else with
//! [`HomeError::UnsupportedVersion`] rather than guessing. A snapshot also names the travel
//! version its residents are written in, and asks for a travel reader that can draw them.
//!
//! | Version | What it added |
//! |---|---|
//! | 1 | Everything |
//! | 2 | `visitors`: friends Desktop lends for the visit; `likings`: what each resident has come to like in its home |
//! | 3 | A room's `plan`, `kind` and `doors`: where each room stands in its house, what kind of room it is, and its doorways |
//! | 4 | A home's `mementos` and `journal`; the receipt's `together` and `moment` effects, and the capabilities that offer them |
//! | 5 | The receipt's `next_door` effect and the capability that offers it: the owner going over to another house, which Desktop opens next if it will; a `stayed_over` moment |
//! | 6 | The receipt's `move_in` effect and the `roommates` capability that offers it: a visitor asked to come and live here, which Desktop decides on; an `asked_to_move_in` moment |
//! | 7 | Desktop 0.67.0, which adopted the contract. No new fields: residents are written in travel version 4, and the Fairground's four souvenirs may be shown |
//!
//! The golden fixtures under `tests/fixtures` are every version as it was first written, and must
//! keep reading.
//!
//! # Bounds
//!
//! Every document is size-limited before it is parsed, every list and string is bounded, and
//! every string is made safe when it is written and checked again when it is read. See
//! [`limits`].

mod accept;
mod document;
mod ids;
mod inventory;
mod projection;
mod replies;
pub mod sample;
mod snapshot;
mod state;

pub use accept::{Accepted, accept_result};
pub use document::{
    HomeDocument, HomeError, decode, encode, read_bounded, read_document, sha256_hex,
    write_document,
};
pub use formiga_expansion_rulebook::SessionId;
pub use formiga_travel::TravelerId;
pub use ids::{CatalogId, DisplayId};
pub use inventory::{DisplayItem, DisplayMode, DisplaySource, Ink, find_modes, souvenir_modes};
pub use projection::{ProjectionError, colony_key, likely_visitors, project_household};
pub use replies::{
    AckRefusal, HomeAck, HomeEffect, HomeRecall, HomeReceipt, HomeResult, RecallReason,
    SessionSeal, Together,
};
pub use snapshot::{HomeCapability, HomeSnapshot, HouseStyle, Household, Neighbour};
pub use state::{
    Door, FavouriteKind, HomeMoment, HomeState, HouseholdHome, JournalEntry, Liked, Liking,
    Memento, MementoKind, PlacedDisplay, PlacedPiece, PlanPoint, RoomLayout, Spot, WallSide,
};

/// The version of every Home document this build writes, and the newest it reads.
pub const HOME_FORMAT_VERSION: u32 = 7;

/// The `format` of each document.
pub const SNAPSHOT_FORMAT: &str = "formiga.home.snapshot";
pub const STATE_FORMAT: &str = "formiga.home.state";
pub const ACK_FORMAT: &str = "formiga.home.ack";
pub const RESULT_FORMAT: &str = "formiga.home.result";
pub const RECEIPT_FORMAT: &str = "formiga.home.receipt";
pub const RECALL_FORMAT: &str = "formiga.home.recall";

/// The files of one session directory.
pub const SNAPSHOT_FILE: &str = "snapshot.json";
pub const STATE_FILE: &str = "state.json";
pub const ACK_FILE: &str = "ack.json";
pub const RESULT_FILE: &str = "result-state.json";
pub const RECEIPT_FILE: &str = "receipt.json";
pub const RECALL_FILE: &str = "recall.json";

/// The argument Home is started with, followed by the session directory's absolute path.
pub const LAUNCH_ARGUMENT: &str = "--formiga-home";

/// How Desktop finds an installed Home, and learns which Home version it reads, without starting
/// it: the same arrangement as Formiga Hill's, under Home's own names.
pub mod discovery {
    /// The macOS bundle identifier Desktop asks LaunchServices for.
    pub const MACOS_BUNDLE_ID: &str = "com.formiga.home";
    /// An integer in Home's `Info.plist`: the newest Home version it reads.
    pub const MACOS_HOME_VERSION_KEY: &str = "FormigaHomeVersion";
    /// The per-user registry key Home's Windows installer writes, under `HKEY_CURRENT_USER`, with
    /// the same key under `HKEY_LOCAL_MACHINE` for a machine-wide install.
    pub const WINDOWS_REGISTRY_KEY: &str = r"Software\Formiga\Home";
    /// `REG_SZ`: the full path of Home's executable.
    pub const WINDOWS_PATH_VALUE: &str = "Path";
    /// `REG_SZ`: Home's version, for messages.
    pub const WINDOWS_VERSION_VALUE: &str = "Version";
    /// `REG_DWORD`: the newest Home version it reads.
    pub const WINDOWS_HOME_VERSION_VALUE: &str = "HomeVersion";
    /// For development: the path of a Home executable (or, on macOS, an `.app`) to use instead of
    /// an installed one. Its Home version is not checked before it is started.
    pub const PATH_OVERRIDE_ENV: &str = "FORMIGA_HOME_PATH";
}

/// The upper bounds every document is held to, on both sides.
pub mod limits {
    /// A household of six with every find the colony could have is under 100 KiB.
    pub const MAX_SNAPSHOT_BYTES: u64 = 512 * 1024;
    /// Twelve households of three full rooms each is under 300 KiB.
    pub const MAX_STATE_BYTES: u64 = 512 * 1024;
    /// A whole state, and the few lines that tie it to its visit.
    pub const MAX_RESULT_BYTES: u64 = MAX_STATE_BYTES + 4 * 1024;
    pub const MAX_ACK_BYTES: u64 = 4 * 1024;
    pub const MAX_RECEIPT_BYTES: u64 = 16 * 1024;
    pub const MAX_RECALL_BYTES: u64 = 4 * 1024;
    /// Twice Desktop's own colony cap, as for a trip: everyone in a house is in the colony.
    pub const MAX_RESIDENTS: usize = 12;
    /// Friends lent for a visit. Desktop lends two at most; the room is for that to grow.
    pub const MAX_VISITORS: usize = 4;
    /// What a household's residents have come to like, across every room.
    pub const MAX_LIKINGS: usize = 48;
    /// Keepsakes one household keeps, and moments in its journal.
    pub const MAX_MEMENTOS: usize = 24;
    pub const MAX_JOURNAL: usize = 40;
    /// Companions in one keepsake or one moment: who is in a photo.
    pub const MAX_IN_A_MEMENTO: usize = 6;
    /// Moments one visit names for Desktop's journal, and how many times a visit may count for
    /// one pair and one kind of time together.
    pub const MAX_MOMENTS: usize = 3;
    pub const MAX_TOGETHER: u8 = 3;
    pub const MAX_RELATIONSHIPS: usize = MAX_RESIDENTS * (MAX_RESIDENTS - 1) / 2;
    /// Twice the six houses a village has.
    pub const MAX_HOUSEHOLDS: usize = 12;
    /// Rooms one household may have.
    pub const MAX_ROOMS: usize = 3;
    /// Furniture and displays together, across every room of one household.
    pub const MAX_PLACED_PER_HOUSEHOLD: usize = 96;
    /// Every trinket Desktop has, every souvenir it keeps, and room for both to grow.
    pub const MAX_INVENTORY: usize = 512;
    /// The ways one thing may be shown.
    pub const MAX_DISPLAY_MODES: usize = 8;
    /// A room's sides, in floor tiles.
    pub const MIN_ROOM_TILES: u8 = 4;
    pub const MAX_ROOM_TILES: u8 = 16;
    /// Doorways in one room's two far walls.
    pub const MAX_DOORS: usize = 4;
    /// How far from the first room's far corner any room of the house may reach, in tiles.
    pub const MAX_PLAN_REACH: i8 = 32;
    /// The turns a piece of furniture can be set at: a quarter turn each.
    pub const TURNS: u8 = 4;
    /// A companion's name, as a trip carries it.
    pub const MAX_NAME_CHARS: usize = formiga_travel::limits::MAX_NAME_CHARS;
    /// A find's or souvenir's name: "Leaf candle holder", "Fairground ticket".
    pub const MAX_ITEM_NAME_CHARS: usize = 32;
    pub const MAX_VERSION_CHARS: usize = 32;
    pub const MAX_CAPABILITIES: usize = 16;
    pub const MAX_EFFECTS: usize = 16;
    /// A catalogue or display identifier: lowercase letters, digits, `-`, `_` and `.`.
    pub const MAX_ID_CHARS: usize = 48;
}
