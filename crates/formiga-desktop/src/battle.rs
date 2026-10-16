//! Playing a match in Formiga Battle with the colony's companions.
//!
//! Formiga Battle is a separate app. Desktop never needs it: without it installed nothing here
//! runs and nothing in the tray or the notebook mentions it. With it installed, the tray offers
//! to play, and a session goes like this:
//!
//! | State | What Desktop does |
//! |---|---|
//! | Idle | Lives as always. |
//! | Departing | Battle has been written a copy of the colony's roster: every companion as a trip to Formiga Hill would carry it, with the leanings it has learned. The train that takes the colony to Hill pulls in and everyone gets on; the world does not tick. As it pulls away Battle is started, so its first menu opens with everyone already riding; if it cannot be, the train stops and everyone gets off again. |
//! | Away | Nothing of the colony is on the desktop and the world does not tick. The tray says Battle is open. |
//! | Returning | Battle has gone: if it turned the roster away, the owner is told why in its own words once everyone is home; otherwise each match its result names is kept once, by its id ([`record`]). The session's files are cleared and the train brings everyone home. |
//!
//! The whole colony rides to Battle, as to Hill, so no trip or visit starts while a match is open
//! and no match while one is under way ([`crate::trip`] is the train both share). The roster
//! carries the colony's Battle record, and what comes back is only that record, the journal's
//! lines for each match and the keepsakes Desktop knows; a result counts only if it names the
//! session and the checksum of the roster Desktop wrote. If Desktop itself stops while Battle is
//! open, Battle keeps the roster it already read, and the next start keeps what the result says so
//! far, clears the session's files and brings the colony home by train.
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
        occupied: "The colony is away at Formiga Battle just now.",
    },
};

/// What the thread waiting on Battle reports.
#[derive(Debug)]
pub enum BattleEvent {
    /// Battle's process has ended, however it ended.
    Exited { session: SessionId },
}

/// Where a session is. Runtime only: across a restart only the marker in [`session`] survives.
pub type SessionState = crate::trip::TripState<OpenSession>;

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

    /// The whole colony rides to a match, so a trip or a visit waits for it to come home, and a
    /// match waits for every trip and visit.
    #[test]
    fn a_match_takes_the_whole_colony_so_trips_and_visits_wait_for_it() {
        let open = [(&BATTLE, Holding::Everyone)];
        assert_eq!(
            busy_elsewhere(&crate::hill::HILL, None, &open),
            Some("The colony is away at Formiga Battle just now.")
        );
        assert!(busy_elsewhere(&crate::house::HOME, Some(&[1]), &open).is_some());
        let indoors = [(&crate::house::HOME, Holding::These(vec![1]))];
        assert!(busy_elsewhere(&BATTLE, None, &indoors).is_some());
        let at_hill = [(&crate::hill::HILL, Holding::Everyone)];
        assert!(busy_elsewhere(&BATTLE, None, &at_hill).is_some());
    }
}
