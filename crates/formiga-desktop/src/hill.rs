//! Sending the colony to Formiga Hill, and bringing it home.
//!
//! Formiga Hill is a separate app. Desktop never needs it: without it installed nothing here runs
//! and nothing in the tray mentions it. With it installed, the tray offers to send the colony
//! there, and a trip goes like this:
//!
//! | State | What Desktop does |
//! |---|---|
//! | Idle | Lives as always. |
//! | Preparing | Lets go of whatever anyone was in the middle of, saves the colony and writes the travel snapshot. Any failure leaves the colony home, untouched. |
//! | Departing | The train pulls in and everyone gets on. The world does not tick. As the train pulls away Hill is started, so its own arrival follows on; if it cannot be, the train stops and everyone gets off again. |
//! | Away | Nothing of the colony is on the desktop and the world does not tick. The tray offers to bring it home. |
//! | Returning | Hill has gone, or the owner asked for the colony back: what Hill sent is checked, the little that may be kept is, the trip's files are cleared, and the train brings everyone home. |
//!
//! It always fails toward home. Desktop never waits on Hill to make the colony visible again: if
//! Hill is missing, refuses the colony, crashes or is closed early, the colony comes home exactly
//! as it left; if Desktop itself stops while the colony is away, the next start finishes the trip
//! from the marker it left, with Hill's receipt if there is one and without it otherwise.
//!
//! Nothing here polls. Hill is found, started and waited on through its slot ([`HILL`], in
//! [`crate::expansion`]): its process is waited on by one sleeping thread, which reports through
//! the event loop when it exits, and Desktop checks whether Hill is installed only when it starts
//! and then at most every ten minutes, from a tick it was taking anyway.

pub mod scene;
pub mod session;

use crate::expansion::files::Folder;
use crate::expansion::{Expansion, Words};
use crate::platform::companion_app::{AppInstall, AppNames};
use formiga_travel::{SessionId, discovery};
use scene::TrainScene;
use session::OpenTrip;

/// Formiga Hill, as Desktop finds it, starts it, keeps its trips' files and speaks of it.
pub static HILL: Expansion = Expansion {
    name: "Formiga Hill",
    app: AppNames {
        path_override_env: discovery::PATH_OVERRIDE_ENV,
        macos_bundle_id: discovery::MACOS_BUNDLE_ID,
        macos_reads_key: discovery::MACOS_TRAVEL_VERSION_KEY,
        windows_registry_key: discovery::WINDOWS_REGISTRY_KEY,
        windows_path_value: discovery::WINDOWS_PATH_VALUE,
        windows_version_value: discovery::WINDOWS_VERSION_VALUE,
        windows_reads_value: discovery::WINDOWS_TRAVEL_VERSION_VALUE,
    },
    launch_argument: formiga_travel::LAUNCH_ARGUMENT,
    start_after_env: "FORMIGA_HILL_TRIP_AFTER",
    folder: Folder {
        directory: session::TRAVEL_DIRECTORY,
        marker_file: session::MARKER_FILE,
        marker_format: session::MARKER_FORMAT,
        kept: &[],
    },
    words: Words {
        contract: "travel",
        update_to: "take the colony there",
        busy: "is already hosting a colony",
        unreadable: "could not read the colony's ticket",
        declined: "could not take the colony this time",
        occupied: "The colony is away at Formiga Hill just now.",
    },
};

/// What the thread watching Hill reports.
#[derive(Debug)]
pub enum HillEvent {
    /// Hill's process has ended, however it ended.
    Exited { session: SessionId },
}

/// Something to tell the owner once the colony is home.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HomecomingNote {
    pub text: String,
    /// It answers something they just did, so it is said in a dialog rather than only noted.
    pub asked_for: bool,
}

/// Where a trip is. Runtime only: across a restart only the marker in [`session`] survives.
#[derive(Default)]
pub enum TripState {
    #[default]
    Idle,
    /// The train is in, and the colony is getting on. `hill` is the Hill to start as the train
    /// pulls away, until it has been.
    Departing {
        open: OpenTrip,
        scene: TrainScene,
        hill: Option<AppInstall>,
    },
    /// The colony is at Hill.
    Away { open: OpenTrip },
    /// The colony is home again and getting off the train. The trip is already closed.
    Returning {
        scene: TrainScene,
        note: Option<HomecomingNote>,
    },
}

impl TripState {
    /// Whether the colony is not simply living on the desktop: the world is held still and the
    /// overlay shows only the trip.
    pub fn holds_world(&self) -> bool {
        !matches!(self, Self::Idle)
    }

    /// The trip still open, while there is one.
    pub fn open(&self) -> Option<&OpenTrip> {
        match self {
            Self::Departing { open, .. } | Self::Away { open } => Some(open),
            _ => None,
        }
    }

    pub fn scene(&self) -> Option<&TrainScene> {
        match self {
            Self::Departing { scene, .. } | Self::Returning { scene, .. } => Some(scene),
            _ => None,
        }
    }

    pub fn scene_mut(&mut self) -> Option<&mut TrainScene> {
        match self {
            Self::Departing { scene, .. } | Self::Returning { scene, .. } => Some(scene),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expansion::Refusal;
    use formiga_travel::AckRefusal;
    use std::path::PathBuf;

    #[test]
    fn an_older_hill_is_told_exactly_what_it_lacks() {
        let install = AppInstall {
            path: PathBuf::from("Formiga Hill.app"),
            version: Some("0.1.0".to_owned()),
            reads: Some(1),
        };
        assert_eq!(HILL.incompatibility(&install, 1), None);
        assert_eq!(
            HILL.incompatibility(&install, 2).as_deref(),
            Some(
                "Formiga Hill 0.1.0 reads travel version 1, and this colony needs version 2. \
                 Update Formiga Hill to take the colony there."
            )
        );
        let silent = AppInstall {
            reads: None,
            ..install
        };
        assert_eq!(
            HILL.incompatibility(&silent, 9),
            None,
            "Hill answers for itself"
        );
    }

    #[test]
    fn hill_is_spoken_of_as_it_always_was() {
        let said = |reason, version: &str| {
            HILL.refusal_text(&Refusal {
                reason,
                version: version.to_owned(),
            })
        };
        assert_eq!(
            said(AckRefusal::UnsupportedVersion { reads: 0 }, "0.1.0"),
            "Formiga Hill 0.1.0 reads travel version 0, which is too old for this colony. Update \
             Formiga Hill to take the colony there."
        );
        assert_eq!(
            said(AckRefusal::Busy, "0.1.0"),
            "Formiga Hill 0.1.0 is already hosting a colony."
        );
        assert_eq!(
            said(AckRefusal::Invalid, ""),
            "Formiga Hill could not read the colony's ticket."
        );
        assert_eq!(
            said(AckRefusal::Other, ""),
            "Formiga Hill could not take the colony this time."
        );
    }
}
