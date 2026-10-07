//! What every visit between Formiga Desktop and one of its companion apps is made of.
//!
//! Formiga Hill, Formiga Home and the apps after them are each visited the same way: Desktop
//! writes a few documents into a fresh session directory named after the visit's [`SessionId`],
//! starts the app with that directory's path, and reads back what the app answers. What each
//! visit carries, and what may come back from it, is the app's own contract (`formiga-travel` for
//! Hill, `formiga-home-contract` for Home, `formiga-farm-contract` for Formiga Farm). How it is
//! carried is the same for all of them, and lives here once:
//!
//! - the visit's identifier, which is also its directory's name and so is only ever hex;
//! - every document's header (`format`, `version`, `min_reader_version`), checked on the raw JSON
//!   before the document is shaped, so a newer file is refused for its version rather than its
//!   shape ([`check_header`], [`decode`]);
//! - every document bounded before it is parsed and before it is written ([`read_bounded`],
//!   [`encode`]), and written whole to a temporary name and renamed into place
//!   ([`write_atomically`]), so a reader sees an old file, a new file, or no file, never half of
//!   one;
//! - the SHA-256 that ties an answer to the exact bytes it answers ([`sha256_hex`]);
//! - every string made safe when it is written and checked again when it is read
//!   ([`sanitize_text`], [`is_sanitized`]);
//! - an app's reasons for turning a visit away ([`AckRefusal`]);
//! - the error each contract reads and writes its documents with, in its own words
//!   ([`contract_error!`]).
//!
//! A contract keeps its own version, its own documents and its own error, and says whose file a
//! problem was in. Nothing here changes a byte of what any contract writes: each contract's golden
//! fixtures are the proof.

#[doc(hidden)]
pub mod contract_error;
mod document;
mod ids;
mod refusal;
mod text;

pub use document::{
    Kind, RulebookError, check_header, decode, encode, header_ok, hex, is_lower_hex, is_sha256_hex,
    read_bounded, sha256_hex, unhex, write_atomically,
};
pub use ids::SessionId;
pub use refusal::AckRefusal;
pub use text::{is_sanitized, sanitize_text};
