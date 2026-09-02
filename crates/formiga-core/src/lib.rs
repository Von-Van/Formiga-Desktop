mod behavior;
mod clock;
mod habitat;
mod model;
mod persistence;
mod rng;
mod seed_share;
mod topology;
mod world;

pub use behavior::{BehaviorContext, BondContext, ObjectUtility, choose_action};
pub use clock::{Clock, FixedClock, SystemClock};
pub use habitat::{
    MAX_HABITAT_ZONES, accessible_regions, habitat_contains, home_anchor, nearest_habitat_point,
    resolved_colony_object_position, resolved_home_anchor, validate_habitat,
};
pub use model::*;
pub use persistence::{PersistenceError, SaveStore};
pub use rng::{SeedStream, new_colony_seed};
pub use seed_share::{
    SeedCodeError, SharedCreatureSeed, decode_creature_seed, derive_imported_colony_seed,
    encode_creature_seed,
};
pub use topology::{
    CursorInvitation, DesktopTopology, MAX_TOPOLOGY_LANDMARKS, MAX_TOPOLOGY_WINDOWS,
    MAX_WINDOW_ROUTE_HOPS, RouteHopKind, RoutePreferences, TopologyLandmark, TopologyLandmarkKind,
    TopologyRouteHop, TopologyWindow,
};
pub use world::World;

pub const SAVE_VERSION: u32 = 11;
