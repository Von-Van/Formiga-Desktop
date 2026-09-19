mod ambience;
mod attention;
mod behavior;
mod bubble;
mod clock;
mod companion;
mod cursor;
mod design;
mod habitat;
mod model;
mod persistence;
mod rng;
mod seed_share;
mod topology;
mod trinkets;
mod visitor;
mod world;

pub use ambience::DesktopAmbience;
pub use attention::{AttentionEmotion, AttentionPose, Gesture, WindowSample};
pub use behavior::{BehaviorContext, BondContext, ObjectUtility, choose_action};
pub use bubble::BubbleIcon;
pub use clock::{Clock, FixedClock, SystemClock};
pub use companion::*;
pub use design::{BodyPlan, CreatureDesign, EarStyle, apply_creature_design};
pub use habitat::{
    CREATURE_FRAME_WIDTH, DWELLING_CELL, DwellingKind, MAX_HABITAT_ZONES, PORCH_WIDTH,
    REST_CLEAR_RATIO, VillageLot, accessible_regions, colony_cottages, habitat_contains,
    home_anchor, home_dwelling_position, home_guest_position, home_object_position,
    home_resting_position, nearest_habitat_point, resolved_colony_object_position,
    resolved_home_anchor, validate_habitat,
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
pub use trinkets::{TrinketCondition, TrinketInfo, all_trinkets, trinket_info, trinkets_for};
pub use visitor::{
    GuestBookEntry, MAX_GUEST_BOOK_ENTRIES, ResidentAnswer, VisitPhase, VisitProgress, Visitor,
    VisitorError, VisitorSource, VisitorState,
};
pub use world::{BubbleGrowth, ThoughtBubble, World};

pub const SAVE_VERSION: u32 = 15;
