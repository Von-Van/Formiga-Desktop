mod behavior;
mod clock;
mod habitat;
mod model;
mod persistence;
mod rng;
mod world;

pub use behavior::{BehaviorContext, choose_action};
pub use clock::{Clock, FixedClock, SystemClock};
pub use habitat::{
    MAX_HABITAT_ZONES, accessible_regions, habitat_contains, home_anchor, nearest_habitat_point,
    resolved_home_anchor, validate_habitat,
};
pub use model::*;
pub use persistence::{PersistenceError, SaveStore};
pub use rng::{SeedStream, new_colony_seed};
pub use world::World;

pub const SAVE_VERSION: u32 = 4;
