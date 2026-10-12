//! What a companion is doing: its action, the surface it is on, its drives and runtime state,
//! and the short beats it has between everything else.

use super::{CreatureId, GardenKind, MonitorId, Point, WindowKey};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ActionKind {
    Idle,
    Traverse,
    SqueezeWindow,
    Perch,
    Sleep,
    InvestigateCursor,
    AvoidCursor,
    ReactToWindow,
    RideWindow,
    SoloPlay,
    Eat,
    Drink,
    Sprint,
    Greet,
    Follow,
    SocialPlay,
    Dragged,
    Landing,
    Homebound,
    ClimbWindow,
    Dangle,
    InspectScreen,
    PresentDiscovery,
    Tossed,
    PetReaction,
}

impl ActionKind {
    pub const ALL: [Self; 25] = [
        Self::Idle,
        Self::Traverse,
        Self::SqueezeWindow,
        Self::Perch,
        Self::Sleep,
        Self::InvestigateCursor,
        Self::AvoidCursor,
        Self::ReactToWindow,
        Self::RideWindow,
        Self::SoloPlay,
        Self::Eat,
        Self::Drink,
        Self::Sprint,
        Self::Greet,
        Self::Follow,
        Self::SocialPlay,
        Self::Dragged,
        Self::Landing,
        Self::Homebound,
        Self::ClimbWindow,
        Self::Dangle,
        Self::InspectScreen,
        Self::PresentDiscovery,
        Self::Tossed,
        Self::PetReaction,
    ];

    /// Unique body clips baked into the creature atlas. Tossing deliberately reuses the dragged
    /// body clip, so it remains expressive without consuming another four 48x48 texture slots.
    pub const BODY_CLIPS: [Self; 22] = [
        Self::Idle,
        Self::Traverse,
        Self::Perch,
        Self::Sleep,
        Self::InvestigateCursor,
        Self::AvoidCursor,
        Self::ReactToWindow,
        Self::RideWindow,
        Self::SoloPlay,
        Self::Eat,
        Self::Drink,
        Self::Sprint,
        Self::Greet,
        Self::Follow,
        Self::SocialPlay,
        Self::Dragged,
        Self::Landing,
        Self::Homebound,
        Self::ClimbWindow,
        Self::Dangle,
        Self::InspectScreen,
        Self::PresentDiscovery,
    ];

    pub const AUTONOMOUS: [Self; 15] = [
        Self::Idle,
        Self::Traverse,
        Self::Perch,
        Self::Sleep,
        Self::InvestigateCursor,
        Self::AvoidCursor,
        Self::ReactToWindow,
        Self::RideWindow,
        Self::SoloPlay,
        Self::Eat,
        Self::Drink,
        Self::Sprint,
        Self::Greet,
        Self::Follow,
        Self::SocialPlay,
    ];

    pub const fn routine_code(self) -> u8 {
        match self {
            Self::Idle => 0,
            Self::Traverse => 1,
            Self::Perch => 2,
            Self::Sleep => 3,
            Self::InvestigateCursor => 4,
            Self::AvoidCursor => 5,
            Self::ReactToWindow => 6,
            Self::RideWindow => 7,
            Self::SoloPlay => 8,
            Self::Eat => 9,
            Self::Drink => 10,
            Self::Sprint => 11,
            Self::Greet => 12,
            Self::Follow => 13,
            Self::SocialPlay => 14,
            Self::Dragged => 15,
            Self::Landing => 16,
            Self::Homebound => 17,
            Self::ClimbWindow => 18,
            Self::Dangle => 19,
            Self::InspectScreen => 20,
            Self::PresentDiscovery => 21,
            Self::Tossed => 22,
            Self::PetReaction => 23,
            // Appended so existing fixed routine keys remain byte-for-byte stable.
            Self::SqueezeWindow => 24,
        }
    }

    pub fn from_legacy_name(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|action| format!("{action:?}") == name)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ActionChoice {
    pub action: ActionKind,
    pub target_creature: Option<CreatureId>,
    pub target_point: Option<Point>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SurfaceKind {
    ScreenFloor,
    WindowLedge,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SurfaceAttachment {
    pub kind: SurfaceKind,
    pub monitor_id: MonitorId,
    pub window_key: Option<WindowKey>,
    pub relative_x: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Drives {
    pub energy: f32,
    pub sleep_pressure: f32,
    pub curiosity_satisfaction: f32,
    pub boredom: f32,
    pub comfort: f32,
    pub arousal: f32,
    pub social_need: f32,
}

impl Default for Drives {
    fn default() -> Self {
        Self {
            energy: 0.82,
            sleep_pressure: 0.15,
            curiosity_satisfaction: 0.45,
            boredom: 0.2,
            comfort: 0.65,
            arousal: 0.15,
            social_need: 0.3,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CreatureState {
    #[serde(skip)]
    pub attention: Option<crate::AttentionPose>,
    pub position: Point,
    pub velocity: Point,
    pub facing_right: bool,
    pub action: ActionKind,
    pub action_elapsed: f32,
    pub action_duration: f32,
    pub drives: Drives,
    pub surface: SurfaceAttachment,
    pub cursor_cooldown: f32,
    /// Runtime presentation selection for generated activity art. It is defaulted for v4 saves
    /// and reset with interrupted actions, so it does not form a discovery collection.
    #[serde(default)]
    pub activity_variant: u8,
    /// Runtime-visible countdown used to stage several earned arrivals after a long absence.
    /// It is persisted so quitting during the reveal sequence cannot skip or duplicate a mini.
    #[serde(default)]
    pub arrival_delay_secs: f32,
    /// A habit being done at the start of the current action. Runtime only, like `attention`.
    #[serde(skip)]
    pub flourish: Option<crate::Flourish>,
    /// Being moved over while asleep: towed on a rope by a friend, or wriggling over by itself.
    /// Runtime only, like `attention`.
    #[serde(skip)]
    pub nudge: Option<SleepNudge>,
    /// A small moment it is having by itself: a yawn, a leaf on its face, a turn at the garden or
    /// at its own door. Runtime only, like `attention`.
    #[serde(skip)]
    pub beat: Option<Beat>,
    /// Inside its own house, out of sight behind the drawn curtain. Runtime only: a colony
    /// always opens with everybody outside.
    #[serde(skip)]
    pub indoors: bool,
}

/// A short moment a companion has between everything else it does: a yawn it caught from a
/// friend, a leaf landing on its face, watering a garden, fluffing the cushion its house is made
/// of. Each is a few seconds long and plays from start to finish unless something bigger comes
/// along; the art reads how far through it is to choose a pose, a face, and anything it holds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Beat {
    pub kind: BeatKind,
    /// Seconds since it began.
    pub elapsed: f32,
    /// Seconds it lasts.
    pub length: f32,
    /// Where it is looking, if anywhere in particular.
    pub look: Option<Point>,
    /// What it is holding up, if anything.
    pub held: Option<VillageProp>,
}

impl Beat {
    pub fn new(kind: BeatKind, length: f32) -> Self {
        Self {
            kind,
            elapsed: 0.0,
            length: length.max(0.1),
            look: None,
            held: None,
        }
    }

    /// How far through it is, from 0 to 1.
    pub fn progress(&self) -> f32 {
        (self.elapsed / self.length).clamp(0.0, 1.0)
    }

    pub fn finished(&self) -> bool {
        self.elapsed >= self.length
    }
}

/// What a beat is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BeatKind {
    /// Breathing in, a yawn, and settling again.
    Yawn,
    /// Lips pressed together and a little shake of the head: trying not to yawn, for now.
    ResistYawn,
    /// Stopped to look at something a companion nearby is doing.
    Notice,
    /// A leaf landed on its face: a start, a shake to get it off, and back to what it was doing.
    LeafOnFace,
    /// What it was eating got away from it: a start, then off after it.
    DroppedSnack,
    /// Picking up what rolled away.
    Retrieve,
    /// Sat down beside the cushion rather than on it: a start, and a shuffle across.
    MissedCushion,
    /// Watering a garden patch.
    Watering,
    /// Crouched over a patch, looking closely at what is coming up.
    InspectSprout,
    /// Picking something from a patch.
    Picking,
    /// Carrying something it grew over to a friend. Strikes no pose of its own: the walk shows,
    /// with the thing held out in front.
    Carrying,
    /// Holding up something it grew for a friend to see.
    ShowingOff,
    /// Looking on, pleased, at something a friend is showing it.
    Admiring,
    /// Retying the flap of a tent.
    AdjustFlap,
    /// Plumping up the cushion a pillow fort is made of.
    FluffCushion,
    /// Looking up at the cap of a mushroom house and giving it a pat.
    InspectCap,
    /// Tidying the leaves of a leaf house.
    TidyLeaves,
    /// Sitting up on top of its house, looking out.
    RoofSit,
    /// Puffed up with its paws folded and its nose in the air: jealous of a pet somebody else
    /// got, or grumbling at one it got itself. This and the rest below are a companion's
    /// temperament showing.
    Huff,
    /// A start, then a paw to the brow and down it goes: a dramatic companion at a fright.
    Swoon,
    /// Sat up with its paws together, looking at somebody else's snack.
    Beg,
    /// A pose struck for whoever is watching, after a climb or a find.
    Strut,
    /// Hiding behind its paws and peeking out: shy, or embarrassed.
    Peek,
    /// A foot stamped: it will not, or it is tired of waiting.
    Stomp,
    /// Crouched and wound up, then a pounce: a troublemaker springing on a dozing friend.
    Pounce,
    /// Jumping at something: the friend a troublemaker has just pounced on.
    Startle,
}

/// Small things the village shows in someone's hands or on the ground.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum VillageProp {
    WateringCan,
    /// Something picked from a garden of this kind.
    Produce(GardenKind),
    /// A snack that rolled away.
    Apple,
    /// A leaf drifting down, or sitting on somebody's face.
    Leaf,
}

/// Somebody inside one of the village's houses, behind its drawn curtain. `slot` counts the
/// colony house as zero, the way the village lays its houses out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HouseOccupancy {
    pub slot: usize,
    /// Asleep in there, so the house breathes out a Z now and then.
    pub napping: bool,
}

/// A house being seen to by its keeper just now: the chore, and how far through it they are, so
/// the house can answer — a cushion plumping up, a cap wobbling, a flap swinging, a leaf coming
/// loose.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HouseMotion {
    pub slot: usize,
    pub chore: BeatKind,
    pub progress: f32,
}

/// How a sleeper that has to make room is moved without being woken.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SleepNudge {
    /// Pulled along on a little rope by the companion holding the other end.
    Towed { by: CreatureId },
    /// Wriggling over in its sleep, with nobody free to tow it.
    Wriggling,
}

impl CreatureState {
    /// Still on its way to where it is going to sleep. A nap begins with the walk to the pillow, a
    /// friend, or the spot the colony gathers at, and until it gets there the companion is walking
    /// to bed, drowsy, not already asleep and gliding across the floor.
    pub fn walking_to_sleep(&self) -> bool {
        self.action == ActionKind::Sleep && self.velocity.x.abs() > 1.0
    }
}
