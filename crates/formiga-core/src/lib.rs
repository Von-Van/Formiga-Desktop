mod behavior;
mod clock;
mod model;
mod persistence;
mod rng;
mod world;

pub use behavior::{BehaviorContext, choose_action};
pub use clock::{Clock, FixedClock, SystemClock};
pub use model::*;
pub use persistence::{PersistenceError, SaveStore};
pub use rng::{SeedStream, new_colony_seed};
pub use world::World;

pub const SAVE_VERSION: u32 = 1;
