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
//! Nothing here polls. Home's process is waited on by one sleeping thread, which reports through
//! the event loop when it exits, and Desktop checks whether Home is installed only when it starts
//! and then at most every ten minutes, from a tick it was taking anyway.

pub mod session;

use crate::app::UserEvent;
use formiga_home_contract::SessionId;
use session::OpenVisit;
use std::process::Child;
use winit::event_loop::EventLoopProxy;

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

/// Wait for Home to exit on a thread of its own, which sleeps until then, and say so through the
/// event loop.
pub fn watch(
    mut child: Child,
    session: SessionId,
    proxy: EventLoopProxy<UserEvent>,
) -> std::io::Result<()> {
    std::thread::Builder::new()
        .name("formiga-home-watch".to_owned())
        .spawn(move || {
            if let Err(error) = child.wait() {
                tracing::warn!(%error, "lost track of Formiga Home");
            }
            let _ = proxy.send_event(UserEvent::House(HouseEvent::Exited { session }));
        })?;
    Ok(())
}
