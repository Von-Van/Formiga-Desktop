//! Sending the colony to Formiga Hill, and bringing it home.
//!
//! Formiga Hill is a separate app. Desktop never needs it: without it installed nothing here runs
//! and nothing in the tray mentions it. With it installed, the tray offers to send the colony
//! there, and a trip goes like this:
//!
//! | State | What Desktop does |
//! |---|---|
//! | Idle | Lives as always. |
//! | Preparing | Lets go of whatever anyone was in the middle of, saves the colony, writes the travel snapshot, and starts Hill. Any failure leaves the colony home, untouched. |
//! | Departing | The train pulls in and everyone gets on. The world does not tick. |
//! | Away | Nothing of the colony is on the desktop and the world does not tick. The tray offers to bring it home. |
//! | Returning | Hill has gone, or the owner asked for the colony back: what Hill sent is checked, the little that may be kept is, the trip's files are cleared, and the train brings everyone home. |
//!
//! It always fails toward home. Desktop never waits on Hill to make the colony visible again: if
//! Hill is missing, refuses the colony, crashes or is closed early, the colony comes home exactly
//! as it left; if Desktop itself stops while the colony is away, the next start finishes the trip
//! from the marker it left, with Hill's receipt if there is one and without it otherwise.
//!
//! Nothing here polls. Hill's process is waited on by one sleeping thread, which reports through
//! the event loop when it exits, and Desktop checks whether Hill is installed only when it starts
//! and then at most every ten minutes, from a tick it was taking anyway.

pub mod scene;
pub mod session;

use crate::app::UserEvent;
use formiga_travel::SessionId;
use scene::TrainScene;
use session::OpenTrip;
use std::process::Child;
use winit::event_loop::EventLoopProxy;

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
    /// The train is in, and the colony is getting on. Hill has been started.
    Departing { open: OpenTrip, scene: TrainScene },
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

/// Wait for Hill to exit on a thread of its own, which sleeps until then, and say so through the
/// event loop.
pub fn watch(
    mut child: Child,
    session: SessionId,
    proxy: EventLoopProxy<UserEvent>,
) -> std::io::Result<()> {
    std::thread::Builder::new()
        .name("formiga-hill-watch".to_owned())
        .spawn(move || {
            if let Err(error) = child.wait() {
                tracing::warn!(%error, "lost track of Formiga Hill");
            }
            let _ = proxy.send_event(UserEvent::Hill(HillEvent::Exited { session }));
        })?;
    Ok(())
}
