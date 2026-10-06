//! Opening a house in Formiga Home, and letting its household back out.
//!
//! Formiga Home is a separate app. Desktop never needs it: without it installed nothing here runs,
//! no house can be clicked, and nothing in the tray or the notebook mentions it. With it installed,
//! a house offers to be opened from its menu, or from the notebook's Home page, and a visit goes
//! like this:
//!
//! | State | What Desktop does |
//! |---|---|
//! | Idle | Lives as always. |
//! | Opening | The household, and any close friend from another house lent for the visit, let go of whatever they were doing and go indoors; the colony is saved and the visit's snapshot and the homes Desktop keeps are written. Any failure brings them straight back out, untouched. |
//! | Open | Home has the house. Those who went in stay indoors and the world leaves them be; everyone else lives as always. The tray offers to bring them back out. |
//! | Closing | Home has gone, or the owner asked for the household back: the homes Home left are kept as far as the contract allows, a line for the visit goes in the journal if Home sent one, the visit's files are cleared, and everyone comes back out where they went in. |
//!
//! It always fails toward home. Desktop never waits on Home to let the household out: if Home is
//! missing, refuses, crashes or is closed early, everyone comes back out and the homes Desktop
//! last kept still stand; if Desktop itself stops while a house is open, the next start finishes
//! the visit from the marker it left, with Home's answers if there are any and without them
//! otherwise.
//!
//! Nothing here polls. Home is found, started and waited on through its slot ([`HOME`], in
//! [`crate::expansion`]): its process is waited on by one sleeping thread, which reports through
//! the event loop when it exits, and Desktop checks whether Home is installed only when it starts
//! and then at most every ten minutes, from a tick it was taking anyway.

pub mod session;

use crate::expansion::files::Folder;
use crate::expansion::{Expansion, Words};
use crate::platform::companion_app::AppNames;
use formiga_home_contract::{SessionId, discovery};
use session::OpenVisit;

/// Formiga Home, as Desktop finds it, starts it, keeps its visits' files and speaks of it.
pub static HOME: Expansion = Expansion {
    name: "Formiga Home",
    app: AppNames {
        path_override_env: discovery::PATH_OVERRIDE_ENV,
        macos_bundle_id: discovery::MACOS_BUNDLE_ID,
        macos_reads_key: discovery::MACOS_HOME_VERSION_KEY,
        windows_registry_key: discovery::WINDOWS_REGISTRY_KEY,
        windows_path_value: discovery::WINDOWS_PATH_VALUE,
        windows_version_value: discovery::WINDOWS_VERSION_VALUE,
        windows_reads_value: discovery::WINDOWS_HOME_VERSION_VALUE,
    },
    launch_argument: formiga_home_contract::LAUNCH_ARGUMENT,
    start_after_env: "FORMIGA_HOME_VISIT_AFTER",
    folder: Folder {
        directory: session::HOME_DIRECTORY,
        marker_file: session::MARKER_FILE,
        marker_format: session::MARKER_FORMAT,
        kept: &[session::KEPT_FILE],
    },
    words: Words {
        contract: "household",
        update_to: "open its houses",
        busy: "already has a house open",
        unreadable: "could not read the house it was given",
        declined: "could not open the house this time",
    },
};

/// What the thread watching Home reports.
#[derive(Debug)]
pub enum HouseEvent {
    /// Home's process has ended, however it ended.
    Exited { session: SessionId },
}

/// Where a visit is. Runtime only: across a restart only the marker in [`session`] survives.
#[derive(Default)]
pub enum VisitState {
    #[default]
    Idle,
    /// Home has the house.
    Open { open: OpenVisit },
}

impl VisitState {
    /// The visit still open, while there is one.
    pub fn open(&self) -> Option<&OpenVisit> {
        match self {
            Self::Open { open } => Some(open),
            Self::Idle => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expansion::Refusal;
    use crate::platform::companion_app::AppInstall;
    use formiga_home_contract::AckRefusal;
    use std::path::PathBuf;

    #[test]
    fn an_older_home_is_told_exactly_what_it_lacks() {
        let install = AppInstall {
            path: PathBuf::from("Formiga Home.app"),
            version: Some("0.1.0".to_owned()),
            reads: Some(1),
        };
        assert_eq!(HOME.incompatibility(&install, 1), None);
        assert_eq!(
            HOME.incompatibility(&install, 2).as_deref(),
            Some(
                "Formiga Home 0.1.0 reads household version 1, and this colony needs version 2. \
                 Update Formiga Home to open its houses."
            )
        );
        let silent = AppInstall {
            reads: None,
            ..install
        };
        assert_eq!(
            HOME.incompatibility(&silent, 9),
            None,
            "Home answers for itself"
        );
    }

    #[test]
    fn home_is_spoken_of_as_it_always_was() {
        let said = |reason, version: &str| {
            HOME.refusal_text(&Refusal {
                reason,
                version: version.to_owned(),
            })
        };
        assert_eq!(
            said(AckRefusal::Busy, ""),
            "Formiga Home already has a house open."
        );
        assert_eq!(
            said(AckRefusal::Other, "0.1.0"),
            "Formiga Home 0.1.0 could not open the house this time."
        );
    }
}
