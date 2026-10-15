//! Playing a match in Formiga Battle with the colony's companions.
//!
//! Formiga Battle is a separate app. Desktop never needs it: without it installed nothing here
//! runs and nothing in the tray or the notebook mentions it. With it installed, the tray offers
//! to play, and a session goes like this:
//!
//! | State | What Desktop does |
//! |---|---|
//! | Idle | Lives as always. |
//! | Open | Battle has a copy of the colony's roster: every companion as a trip to Formiga Hill would carry it, with the leanings it has learned. Everyone goes on living on the desktop the whole time, and the tray says Battle is open. |
//! | Closing | Battle has gone: if it turned the roster away, the owner is told why in its own words, and the session's files are cleared. |
//!
//! Nobody is lent to Battle, so nothing here keeps a companion off the desktop or out of another
//! app, and no trip or visit keeps Battle from being played. Nothing comes back from a match yet:
//! Battle only says whether it could read the roster, and the colony is never changed by it, so
//! nothing about Battle is saved. If Desktop itself stops while Battle is open, Battle keeps the
//! roster it already read, and the next start clears the session's files.
//!
//! Battle is found, started and waited on through its slot ([`BATTLE`], in [`crate::expansion`]),
//! and Desktop checks whether it is installed only when it starts and then at most every ten
//! minutes, from a tick it was taking anyway.

pub mod record;
pub mod session;

use crate::expansion::files::Folder;
use crate::expansion::{Expansion, Words};
use crate::platform::companion_app::AppNames;
use formiga_battle_contract::{SessionId, discovery};
use session::OpenSession;

/// Formiga Battle, as Desktop finds it, starts it, keeps its sessions' files and speaks of it.
pub static BATTLE: Expansion = Expansion {
    name: "Formiga Battle",
    app: AppNames {
        path_override_env: discovery::PATH_OVERRIDE_ENV,
        macos_bundle_id: discovery::MACOS_BUNDLE_ID,
        macos_reads_key: discovery::MACOS_BATTLE_VERSION_KEY,
        windows_registry_key: discovery::WINDOWS_REGISTRY_KEY,
        windows_path_value: discovery::WINDOWS_PATH_VALUE,
        windows_version_value: discovery::WINDOWS_VERSION_VALUE,
        windows_reads_value: discovery::WINDOWS_BATTLE_VERSION_VALUE,
    },
    launch_argument: formiga_battle_contract::LAUNCH_ARGUMENT,
    start_after_env: "FORMIGA_BATTLE_SESSION_AFTER",
    folder: Folder {
        directory: session::BATTLE_DIRECTORY,
        marker_file: session::MARKER_FILE,
        marker_format: session::MARKER_FORMAT,
        kept: &[],
    },
    words: Words {
        contract: "battle",
        update_to: "play with the colony's companions",
        busy: "already has a match open",
        unreadable: "could not read the companions it was given",
        declined: "could not open a match this time",
        // Battle is never lent anyone, so no visit is ever turned away in these words.
        occupied: "Formiga Battle is open just now.",
    },
};

/// What the thread waiting on Battle reports.
#[derive(Debug)]
pub enum BattleEvent {
    /// Battle's process has ended, however it ended.
    Exited { session: SessionId },
}

/// Where a session is. Runtime only: across a restart only the marker in [`session`] survives.
#[derive(Default)]
pub enum SessionState {
    #[default]
    Idle,
    Open(OpenSession),
}

impl SessionState {
    /// The session still open, while there is one.
    pub fn open(&self) -> Option<&OpenSession> {
        match self {
            Self::Open(open) => Some(open),
            Self::Idle => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expansion::{Holding, Refusal, busy_elsewhere};
    use crate::platform::companion_app::AppInstall;
    use formiga_battle_contract::AckRefusal;
    use std::path::PathBuf;

    #[test]
    fn an_older_battle_is_told_exactly_what_it_lacks() {
        let install = AppInstall {
            path: PathBuf::from("Formiga Battle.app"),
            version: Some("0.1.0".to_owned()),
            reads: Some(1),
        };
        assert_eq!(BATTLE.incompatibility(&install, 1), None);
        assert_eq!(
            BATTLE.incompatibility(&install, 2).as_deref(),
            Some(
                "Formiga Battle 0.1.0 reads battle version 1, and this colony needs version 2. \
                 Update Formiga Battle to play with the colony's companions."
            )
        );
    }

    #[test]
    fn battle_is_spoken_of_in_its_own_words() {
        let said = |reason, version: &str| {
            BATTLE.refusal_text(&Refusal {
                reason,
                version: version.to_owned(),
            })
        };
        assert_eq!(
            said(AckRefusal::Busy, ""),
            "Formiga Battle already has a match open."
        );
        assert_eq!(
            said(AckRefusal::Invalid, "0.1.0"),
            "Formiga Battle 0.1.0 could not read the companions it was given."
        );
    }

    #[test]
    fn a_match_keeps_nobody_from_a_trip_or_a_visit() {
        let open = [(&BATTLE, Holding::Nobody)];
        assert_eq!(busy_elsewhere(&crate::hill::HILL, None, &open), None);
        assert_eq!(busy_elsewhere(&crate::house::HOME, Some(&[1]), &open), None);
    }
}
