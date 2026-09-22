mod ambience;
mod attention;
mod behavior;
mod bubble;
mod clock;
mod companion;
mod cursor;
mod design;
mod habitat;
mod habits;
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
pub use design::{BodyPlan, ClassicParts, CreatureDesign, EarStyle, apply_creature_design};
pub use habitat::{
    BELONGING_CLEARANCE, BELONGING_DEPTH, CREATURE_FRAME_WIDTH, Cottages, DWELLING_CELL,
    DwellingKind, GroundItem, HANGOUT_WIDTH, HomeCommons, HouseOwners, MAX_HABITAT_ZONES,
    OBJECT_WIDTH, REST_CLEAR_RATIO, REST_WALL_SLIVER, RESTING_WIDTH, TREE_WIDTH, TRINKETS_PER_TREE,
    TreeEnd, VILLAGE_SPAN_LIMIT, VillageLot, accessible_regions, colony_cottage_list,
    colony_cottages, habitat_contains, home_anchor, home_commons, home_dwelling_position,
    home_ground_positions, home_guest_position, home_hangout_positions, home_object_position,
    home_object_positions, home_resting_position, home_tree_position, house_owners, house_slot_for,
    nearest_habitat_point, resolved_colony_object_position, resolved_home_anchor, validate_habitat,
    village_span,
};
pub use habits::{
    Celebration, FLOURISH_WAIT_SECS, Flourish, HABIT_PERFORMANCE_CHANCE, Habit, HabitCue,
    MAX_HABITS, habit_for, habit_to_learn, learning_chance,
};
pub use model::*;
pub use persistence::{
    PERIODIC_SAVE, PersistenceError, ROUTINE_CHECKPOINT, SaveStore, SaveUrgency, save_due,
};
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
    FavoriteError, FavoriteVisitor, GuestBookEntry, MAX_FAVORITE_VISITORS, MAX_GUEST_BOOK_ENTRIES,
    MAX_TOUR_STOPS, ResidentAnswer, TourInterest, TourMoment, TourStop, VisitPhase, VisitProgress,
    Visitor, VisitorError, VisitorSource, VisitorState,
};
pub use world::{BubbleGrowth, ColonyEdit, ThoughtBubble, UndoError, World};

pub const SAVE_VERSION: u32 = 18;
