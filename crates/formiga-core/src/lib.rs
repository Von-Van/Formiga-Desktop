mod accessories;
mod ambience;
mod attention;
mod behavior;
mod bubble;
mod companion;
mod cursor;
mod daybook;
mod design;
mod habitat;
mod habits;
mod model;
mod observations;
mod persistence;
mod rng;
mod seed_share;
mod souvenirs;
mod stature;
mod temperament;
mod topology;
mod trinkets;
pub mod tuning;
mod visitor;
mod wonders;
mod world;

pub use accessories::{
    Accessory, AccessoryError, AccessoryKind, AccessoryPlace, available_accessories,
};
pub use ambience::DesktopAmbience;
pub use attention::{AttentionEmotion, AttentionPose, Gesture, WindowSample};
pub use behavior::{BehaviorContext, BondContext, ObjectUtility, choose_action};
pub use bubble::BubbleIcon;
pub use companion::*;
pub use daybook::{DAYS_KEPT, DayBook, DayNote, DayPair, DayRecord, day_notes};
pub use design::{
    BODY_ARCHETYPES, BodyArchetype, BodyPlan, ClassicParts, CreatureDesign, DetailParts, EarStyle,
    Edition, FACE_TEMPLATES, Strangeness, apply_creature_design, hsl, to_hsl,
};
pub use habitat::{
    BELONGING_CLEARANCE, BELONGING_DEPTH, CREATURE_FRAME_WIDTH, Cottages, DWELLING_CELL,
    DWELLING_DRAWN_HEIGHT, DwellingKind, GroundItem, HANGOUT_WIDTH, HomeCommons, HouseOwners,
    MAX_HABITAT_ZONES, OBJECT_WIDTH, REST_CLEAR_RATIO, REST_WALL_SLIVER, RESTING_WIDTH,
    TREE_OVERLAP, TREE_WIDTH, TRINKETS_PER_TREE, TreeEnd, VILLAGE_SPAN_LIMIT, VillageLot,
    accessible_regions, body_half_width, clamp_to_standing, colony_cottage_list, colony_cottages,
    habitat_contains, home_anchor, home_commons, home_dwelling_position, home_ground_positions,
    home_guest_position, home_hangout_positions, home_lot_widths, home_object_position,
    home_object_positions, home_resting_position, home_tree_position, house_owners,
    house_roof_height, house_slot_for, keep_whole_in, nearest_habitat_point,
    resolved_colony_object_position, resolved_home_anchor, standing_span, validate_habitat,
    village_span,
};
pub use habits::{
    Celebration, FLOURISH_WAIT_SECS, Flourish, HABIT_PERFORMANCE_CHANCE, Habit, HabitCue,
    MAX_HABITS, habit_for, habit_to_learn, learning_chance,
};
pub use model::*;
pub use observations::{Observation, observations_of, observe};
pub use persistence::{
    ImportRefusal, MAX_SAVE_BYTES, PERIODIC_SAVE, PersistenceError, ROUTINE_CHECKPOINT, SaveStore,
    SaveUrgency, ValidatedSave, damage, decode, decode_unvalidated, named_version, save_due,
    violations,
};
pub use rng::{SeedStream, new_colony_seed};
pub use seed_share::{
    SeedCodeError, SharedCreatureSeed, decode_creature_seed, derive_imported_colony_seed,
    encode_creature_seed,
};
pub use souvenirs::{Souvenir, SouvenirRecord};
pub use stature::{
    AVERAGE_SIZE, STATURE_MAX, STATURE_MIN, apply_statures, size_after_parent, size_for,
    stature_percent,
};
pub use temperament::{Axes, Temperament, TemperamentKind, Tension, Trait, Valence};
pub use topology::{
    CursorInvitation, DesktopTopology, MAX_TOPOLOGY_LANDMARKS, MAX_TOPOLOGY_WINDOWS,
    MAX_WINDOW_ROUTE_HOPS, RouteHopKind, RoutePreferences, TopologyLandmark, TopologyLandmarkKind,
    TopologyRouteHop, TopologyWindow,
};
pub use trinkets::{
    TrinketCondition, TrinketInfo, all_trinkets, trinket_count, trinket_info, trinkets_for,
};
pub use visitor::{
    FavoriteError, FavoriteVisitor, GuestBookEntry, MAX_FAVORITE_VISITORS, MAX_GUEST_BOOK_ENTRIES,
    MAX_TOUR_STOPS, ResidentAnswer, TourInterest, TourMoment, TourStop, VisitPhase, VisitProgress,
    Visitor, VisitorError, VisitorSource, VisitorState,
};
pub use wonders::{WonderKind, WonderRecord, WonderSeats, WonderView};
pub use world::{
    BubbleGrowth, ColonyEdit, ThoughtBubble, UndoError, WonderPose, World, wonder_length,
    wonder_motion, wonder_poses,
};

pub const SAVE_VERSION: u32 = 28;
