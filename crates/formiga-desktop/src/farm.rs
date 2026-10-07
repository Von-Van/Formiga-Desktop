//! Reshaping a companion in Formiga Farm, or drawing a new one there.
//!
//! Formiga Farm is a separate app. Desktop never needs it: without it installed nothing here runs
//! and the notebook says nothing about it. With it installed, a companion's page in the notebook
//! offers to reshape it in Farm, and the studio offers to draw a new companion there. A session
//! goes like this:
//!
//! | State | What Desktop does |
//! |---|---|
//! | Idle | Lives as always. |
//! | Open | Farm has a copy of one companion's look, or a stand-in for a new one, and the companion goes on living on the desktop. Each time the owner applies a design in Farm, Desktop decides on it at once by the contract's own rule: a companion's new look is kept and noted in the journal, and a new companion's design goes into the studio, where the owner may welcome it. Farm is told each answer. |
//! | Closing | Farm has gone: whatever it proposed last and was not yet answered is decided on, and the session's files are cleared. |
//!
//! Nobody is lent to Farm, so nothing here keeps a companion off the desktop or out of another
//! app. A companion out with Formiga Hill or Formiga Home is not reshaped until it is back, and
//! one that has left the colony is not reshaped at all. It always fails toward the look a
//! companion has: if Farm is missing, refuses, crashes or writes something that does not check
//! out, nothing changes. If Desktop itself stops while Farm is open, the next start answers what
//! Farm last proposed and tells Farm the session is over.
//!
//! Farm is found, started and waited on through its slot ([`FARM`], in [`crate::expansion`]),
//! and Desktop checks whether it is installed only when it starts and then at most every ten
//! minutes, from a tick it was taking anyway. While Farm runs, and only then, one more thread
//! ([`Watcher`]) looks at its proposal a few times a second and says through the event loop when
//! there is a new one, so a look the owner applies shows on the desktop while Farm is still open.

pub mod session;

use crate::app::UserEvent;
use crate::expansion::files::Folder;
use crate::expansion::{Expansion, Words};
use crate::platform::companion_app::AppNames;
use formiga_core::CreatureId;
use formiga_farm_contract::{FarmProposal, PROPOSAL_FILE, SessionId, discovery, read_document};
use session::OpenSession;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use winit::event_loop::EventLoopProxy;

/// How often the watcher looks at Farm's proposal while Farm runs.
const WATCH_EVERY: Duration = Duration::from_millis(250);

/// Formiga Farm, as Desktop finds it, starts it, keeps its sessions' files and speaks of it.
pub static FARM: Expansion = Expansion {
    name: "Formiga Farm",
    app: AppNames {
        path_override_env: discovery::PATH_OVERRIDE_ENV,
        macos_bundle_id: discovery::MACOS_BUNDLE_ID,
        macos_reads_key: discovery::MACOS_FARM_VERSION_KEY,
        windows_registry_key: discovery::WINDOWS_REGISTRY_KEY,
        windows_path_value: discovery::WINDOWS_PATH_VALUE,
        windows_version_value: discovery::WINDOWS_VERSION_VALUE,
        windows_reads_value: discovery::WINDOWS_FARM_VERSION_VALUE,
    },
    launch_argument: formiga_farm_contract::LAUNCH_ARGUMENT,
    start_after_env: "FORMIGA_FARM_SESSION_AFTER",
    folder: Folder {
        directory: session::FARM_DIRECTORY,
        marker_file: session::MARKER_FILE,
        marker_format: session::MARKER_FORMAT,
        kept: &[],
    },
    words: Words {
        contract: "design",
        update_to: "reshape the colony's companions",
        busy: "already has a companion open",
        unreadable: "could not read the companion it was given",
        declined: "could not open the companion this time",
        // Farm is never lent anyone, so no visit is ever turned away in these words.
        occupied: "Formiga Farm is open just now.",
    },
};

/// What the notebook asks of Farm.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FarmRequest {
    /// Reshape this companion.
    Reshape(CreatureId),
    /// Draw a new companion, for the studio to welcome.
    Draw,
}

/// What the threads watching Farm report.
#[derive(Debug)]
pub enum FarmEvent {
    /// Farm has written a proposal Desktop has not seen yet.
    Proposed { session: SessionId },
    /// Farm's process has ended, however it ended.
    Exited { session: SessionId },
}

/// Where a session is. Runtime only: across a restart only the marker in [`session`] survives.
#[derive(Default)]
pub enum SessionState {
    #[default]
    Idle,
    /// Farm has the session, watched while it runs: the watcher is only held, and stops when
    /// the session ends and it is dropped.
    Open {
        open: OpenSession,
        _watcher: Option<Watcher>,
    },
}

impl SessionState {
    /// The session still open, while there is one.
    pub fn open(&self) -> Option<&OpenSession> {
        match self {
            Self::Open { open, .. } => Some(open),
            Self::Idle => None,
        }
    }
}

/// A thread that looks at a session's proposal a few times a second, and says through the event
/// loop whenever it holds a serial it has not seen. It stops when it is dropped, at its next look.
pub struct Watcher {
    stop: Arc<AtomicBool>,
}

impl Watcher {
    pub fn start(
        dir: &Path,
        session: SessionId,
        proxy: EventLoopProxy<UserEvent>,
    ) -> std::io::Result<Self> {
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = Arc::clone(&stop);
        let path = dir.join(PROPOSAL_FILE);
        std::thread::Builder::new()
            .name("formiga-farm-proposals".to_owned())
            .spawn(move || {
                let mut seen = 0;
                loop {
                    std::thread::sleep(WATCH_EVERY);
                    if stopped.load(Ordering::Relaxed) {
                        return;
                    }
                    // Only the serial is looked at here: whether the proposal counts for
                    // anything is decided on the app's side, by the contract.
                    let Ok((proposal, _)) = read_document::<FarmProposal>(&path) else {
                        continue;
                    };
                    if proposal.serial != seen {
                        seen = proposal.serial;
                        let event = FarmEvent::Proposed {
                            session: session.clone(),
                        };
                        if proxy.send_event(UserEvent::Farm(event)).is_err() {
                            return;
                        }
                    }
                }
            })?;
        Ok(Self { stop })
    }
}

impl Drop for Watcher {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expansion::Refusal;
    use crate::platform::companion_app::AppInstall;
    use formiga_farm_contract::AckRefusal;
    use std::path::PathBuf;

    #[test]
    fn an_older_farm_is_told_exactly_what_it_lacks() {
        let install = AppInstall {
            path: PathBuf::from("Formiga Farm.app"),
            version: Some("0.1.0".to_owned()),
            reads: Some(1),
        };
        assert_eq!(FARM.incompatibility(&install, 1), None);
        assert_eq!(
            FARM.incompatibility(&install, 2).as_deref(),
            Some(
                "Formiga Farm 0.1.0 reads design version 1, and this colony needs version 2. \
                 Update Formiga Farm to reshape the colony's companions."
            )
        );
    }

    #[test]
    fn farm_is_spoken_of_in_its_own_words() {
        let said = |reason, version: &str| {
            FARM.refusal_text(&Refusal {
                reason,
                version: version.to_owned(),
            })
        };
        assert_eq!(
            said(AckRefusal::Busy, ""),
            "Formiga Farm already has a companion open."
        );
        assert_eq!(
            said(AckRefusal::Invalid, "0.1.0"),
            "Formiga Farm 0.1.0 could not read the companion it was given."
        );
    }
}
