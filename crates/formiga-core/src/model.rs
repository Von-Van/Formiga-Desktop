//! The colony's saved types, the desktop snapshot the world is handed, and the commands and
//! events that pass between `World` and the desktop, one topic per module. Everything public is
//! re-exported here, and from here at the crate root, under the name it has always had.

mod actions;
mod appearance;
mod creature;
mod desktop;
mod events;
mod home;
mod memory;
mod objects;
mod relationships;
mod rituals;
mod routines;
mod save;
mod settings;
mod shelter;
mod village;

#[cfg(test)]
mod tests;

pub use actions::*;
pub use appearance::*;
pub use creature::*;
pub use desktop::*;
pub use events::*;
pub use home::*;
pub use memory::*;
pub use objects::*;
pub use relationships::*;
pub use rituals::*;
pub use routines::*;
pub use save::*;
pub use settings::*;
pub use shelter::*;
pub use village::*;
