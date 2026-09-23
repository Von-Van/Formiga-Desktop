use serde::{Deserialize, Serialize};
use std::time::Duration;
use time::OffsetDateTime;

pub type CreatureId = u64;
pub type WindowKey = u64;
pub type MonitorId = u64;

#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub struct DisplayKey(pub [u8; 16]);

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ApplicationKey {
    MacBundleId(String),
    WindowsAumid(String),
    WindowsExecutableHash([u8; 32]),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub fn distance(self, other: Self) -> f32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        (dx * dx + dy * dy).sqrt()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DesktopRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl DesktopRect {
    pub fn right(self) -> f32 {
        self.x + self.width
    }

    pub fn bottom(self) -> f32 {
        self.y + self.height
    }

    pub fn contains(self, point: Point) -> bool {
        point.x >= self.x
            && point.x <= self.right()
            && point.y >= self.y
            && point.y <= self.bottom()
    }

    pub fn clamp(self, point: Point) -> Point {
        Point {
            x: point.x.clamp(self.x, self.right()),
            y: point.y.clamp(self.y, self.bottom()),
        }
    }

    pub fn intersection(self, other: Self) -> Option<Self> {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let right = self.right().min(other.right());
        let bottom = self.bottom().min(other.bottom());
        (right > x && bottom > y).then_some(Self {
            x,
            y,
            width: right - x,
            height: bottom - y,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MonitorInfo {
    pub id: MonitorId,
    #[serde(default)]
    pub display_key: DisplayKey,
    pub bounds: DesktopRect,
    pub usable_bounds: DesktopRect,
    pub scale_factor: f32,
    pub primary: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DesktopWindow {
    pub key: WindowKey,
    pub bounds: DesktopRect,
    pub z_order: u32,
    pub visible: bool,
    pub minimized: bool,
    #[serde(default)]
    pub application: Option<ApplicationKey>,
    #[serde(default)]
    pub application_name: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CursorSnapshot {
    pub position: Point,
    pub velocity: Point,
    pub available: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DesktopSnapshot {
    pub monitors: Vec<MonitorInfo>,
    pub windows: Vec<DesktopWindow>,
    pub cursor: CursorSnapshot,
    #[serde(with = "duration_millis")]
    pub idle_duration: Duration,
    /// Actual scan provenance; omitted by synthetic fixtures and never serialized.
    #[serde(skip)]
    pub window_sample: Option<crate::WindowSample>,
    /// Monotonic time of the native cursor sample; never persisted.
    #[serde(skip)]
    pub cursor_sample_millis: Option<u64>,
}

pub trait PlatformDesktop {
    type Error;

    fn snapshot(&self) -> DesktopSnapshot;
    fn set_overlays_visible(&mut self, visible: bool);
    fn set_launch_at_login(&mut self, enabled: bool) -> Result<(), Self::Error>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BodyFamily {
    Blob,
    Hopper,
    SoftQuadruped,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PatternKind {
    Solid,
    Patches,
    Spots,
    Stripes,
    Mask,
    Socks,
    Tips,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HeadAppendageStyle {
    None,
    Round,
    Pointed,
    Leaf,
    Droop,
    Antenna,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct HeadAppendageGenome {
    pub style: HeadAppendageStyle,
    pub size: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EyeShape {
    Round,
    Tall,
    SoftSquare,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PupilStyle {
    Dot,
    Wide,
    Spark,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HighlightStyle {
    Single,
    Double,
    Diagonal,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BrowStyle {
    None,
    Soft,
    Bold,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MouthStyle {
    Tiny,
    Smile,
    Cat,
    Beak,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CheekStyle {
    None,
    Dots,
    Blush,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FaceGenome {
    pub eye_shape: EyeShape,
    pub eye_size: u8,
    pub eye_spacing: u8,
    pub vertical_offset: i8,
    pub pupil_style: PupilStyle,
    pub highlight_style: HighlightStyle,
    pub brow_style: BrowStyle,
    pub mouth_style: MouthStyle,
    pub cheek_style: CheekStyle,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ForelimbStyle {
    SoftNub,
    Pseudopod,
    MittenArm,
    FrontPaw,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LimbTipStyle {
    Round,
    Mitten,
    Paw,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RestPose {
    AtSides,
    Folded,
    Together,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ForelimbGenome {
    pub style: ForelimbStyle,
    pub length: u8,
    pub thickness: u8,
    pub tip_style: LimbTipStyle,
    pub rest_pose: RestPose,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EffectMotif {
    None,
    Dot,
    Star,
    Heart,
    Leaf,
    Spark,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TailStyle {
    None,
    Stub,
    Taper,
    Tuft,
    Curl,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AppearanceGenome {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub design: Option<crate::CreatureDesign>,
    pub family: BodyFamily,
    pub logical_size: u8,
    pub body_width: u8,
    pub body_height: u8,
    pub head_ratio: f32,
    pub roundness: f32,
    pub leg_length: u8,
    pub foot_size: u8,
    pub head_appendages: HeadAppendageGenome,
    pub tail_style: TailStyle,
    pub tail_length: u8,
    pub face: FaceGenome,
    pub forelimbs: ForelimbGenome,
    pub effect_motif: EffectMotif,
    pub palette_index: u8,
    pub pattern: PatternKind,
    pub pattern_density: f32,
    pub marking_seed: u64,
    pub gait_bob: f32,
    pub face_signature: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PersonalityGenome {
    pub activity: f32,
    pub curiosity: f32,
    pub boldness: f32,
    pub playfulness: f32,
    pub sociability: f32,
    pub routine_affinity: f32,
    pub sleep_timing: f32,
    pub window_tolerance: f32,
    pub cursor_interest: f32,
    pub decision_temperature: f32,
}

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

pub const MAX_RELATIONSHIPS: usize = MAX_COLONY_CREATURES * (MAX_COLONY_CREATURES - 1) / 2;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreatureRelationship {
    pub a: CreatureId,
    pub b: CreatureId,
    pub affinity: u8,
    pub familiarity: u8,
    pub playfulness: u8,
    pub avoidance: u8,
}

impl CreatureRelationship {
    pub fn new(a: CreatureId, b: CreatureId) -> Option<Self> {
        let (a, b) = canonical_creature_pair(a, b)?;
        Some(Self {
            a,
            b,
            ..Self::default()
        })
    }

    pub fn contains(self, creature_id: CreatureId) -> bool {
        self.a == creature_id || self.b == creature_id
    }

    pub fn other(self, creature_id: CreatureId) -> Option<CreatureId> {
        if self.a == creature_id {
            Some(self.b)
        } else if self.b == creature_id {
            Some(self.a)
        } else {
            None
        }
    }

    pub fn closeness(self) -> i16 {
        i16::from(self.affinity) * 2 + i16::from(self.familiarity) - i16::from(self.avoidance) * 2
    }

    pub fn apply(&mut self, experience: RelationshipExperience) {
        let (affinity, familiarity, playfulness, avoidance) = match experience {
            RelationshipExperience::CalmProximity => (0, 1, 0, -1),
            RelationshipExperience::Followed => (0, 1, 0, -1),
            RelationshipExperience::Greeting => (2, 1, 0, -1),
            RelationshipExperience::SharedRest => (2, 2, 0, -2),
            RelationshipExperience::PositivePlay => (1, 1, 3, -1),
            RelationshipExperience::BroughtDiscovery => (3, 1, 1, -1),
            RelationshipExperience::StoleToy => (-1, 1, 3, 2),
            RelationshipExperience::HomecomingGreeting => (3, 2, 0, -2),
            RelationshipExperience::WatchedClimb => (1, 1, 0, -1),
            RelationshipExperience::ConcernedAfterToss => (2, 1, 0, 0),
            RelationshipExperience::Squabble => (-2, 1, 1, 5),
        };
        adjust_relationship_score(&mut self.affinity, affinity);
        adjust_relationship_score(&mut self.familiarity, familiarity);
        adjust_relationship_score(&mut self.playfulness, playfulness);
        adjust_relationship_score(&mut self.avoidance, avoidance);
    }
}

fn adjust_relationship_score(score: &mut u8, delta: i16) {
    *score = (i16::from(*score) + delta).clamp(0, i16::from(u8::MAX)) as u8;
}

pub fn canonical_creature_pair(a: CreatureId, b: CreatureId) -> Option<(CreatureId, CreatureId)> {
    (a != b).then_some(if a < b { (a, b) } else { (b, a) })
}

pub fn relationship_between(
    relationships: &[CreatureRelationship],
    a: CreatureId,
    b: CreatureId,
) -> Option<&CreatureRelationship> {
    let (a, b) = canonical_creature_pair(a, b)?;
    relationships
        .iter()
        .find(|relationship| relationship.a == a && relationship.b == b)
}

pub fn closest_companion(
    relationships: &[CreatureRelationship],
    creature_id: CreatureId,
) -> Option<CreatureId> {
    relationships
        .iter()
        .copied()
        .filter(|relationship| relationship.contains(creature_id))
        .max_by_key(|relationship| {
            (
                relationship.closeness(),
                std::cmp::Reverse(relationship.other(creature_id)),
            )
        })
        .and_then(|relationship| relationship.other(creature_id))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RelationshipExperience {
    CalmProximity,
    Followed,
    Greeting,
    SharedRest,
    PositivePlay,
    BroughtDiscovery,
    StoleToy,
    HomecomingGreeting,
    WatchedClimb,
    ConcernedAfterToss,
    Squabble,
}

pub const MAX_ROUTINES: usize = 12;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoutineSlot {
    pub key: u16,
    pub strength: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoutineTable {
    pub slots: [RoutineSlot; MAX_ROUTINES],
    pub len: u8,
}

impl Default for RoutineTable {
    fn default() -> Self {
        Self {
            slots: [RoutineSlot::default(); MAX_ROUTINES],
            len: 0,
        }
    }
}

impl RoutineTable {
    pub fn strength(&self, key: u16) -> f32 {
        self.slots[..usize::from(self.len.min(MAX_ROUTINES as u8))]
            .iter()
            .find(|slot| slot.key == key)
            .map_or(0.0, |slot| f32::from(slot.strength) / 255.0)
    }

    pub fn reinforce(&mut self, key: u16) {
        let len = usize::from(self.len.min(MAX_ROUTINES as u8));
        for slot in &mut self.slots[..len] {
            slot.strength = slot.strength.saturating_sub(1);
        }
        if let Some(slot) = self.slots[..len].iter_mut().find(|slot| slot.key == key) {
            slot.strength = slot.strength.saturating_add(5);
            return;
        }
        if len < MAX_ROUTINES {
            self.slots[len] = RoutineSlot { key, strength: 5 };
            self.len += 1;
            return;
        }
        let weakest = self.slots[..len]
            .iter()
            .enumerate()
            .min_by_key(|(_, slot)| (slot.strength, slot.key))
            .map(|(index, _)| index)
            .unwrap_or_default();
        if self.slots[weakest].strength <= 5 {
            self.slots[weakest] = RoutineSlot { key, strength: 5 };
        }
    }

    pub fn from_ranked(mut entries: Vec<(u16, f32)>) -> Self {
        entries.sort_by(|(key_a, value_a), (key_b, value_b)| {
            value_b.total_cmp(value_a).then_with(|| key_a.cmp(key_b))
        });
        let mut table = Self::default();
        for (index, (key, strength)) in entries.into_iter().take(MAX_ROUTINES).enumerate() {
            table.slots[index] = RoutineSlot {
                key,
                strength: (strength.clamp(0.0, 1.0) * 255.0).round() as u8,
            };
            table.len += 1;
        }
        table
    }
}

pub fn routine_key(surface: SurfaceKind, relative_x: f32, action: ActionKind, hour_utc: u8) -> u16 {
    let time_bucket = u16::from((hour_utc / 6).min(3));
    let region = u16::from((relative_x.clamp(0.0, 0.999) * 3.0) as u8);
    let surface = match surface {
        SurfaceKind::ScreenFloor => 0,
        SurfaceKind::WindowLedge => 1,
    };
    time_bucket | (region << 2) | (surface << 4) | (u16::from(action.routine_code()) << 5)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreatureOrigin {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub design: Option<crate::CreatureDesign>,
    pub source_colony_seed: [u8; 32],
    pub source_generation: u8,
}

pub const MAX_COLONY_CREATURES: usize = 6;
/// Every full-size companion has a house of its own and the village has room for six, so this is
/// the colony cap rather than a smaller one inside it. `AdultLimit` therefore cannot be the limit
/// that fires while the two agree: a colony with room left over has room for an adult.
pub const MAX_ADULT_CREATURES: usize = MAX_COLONY_CREATURES;
pub const MAX_MINIS_PER_ADULT: usize = 3;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CreatureRole {
    #[default]
    Adult,
    Mini {
        parent_id: CreatureId,
    },
}

impl CreatureRole {
    pub const fn is_adult(self) -> bool {
        matches!(self, Self::Adult)
    }

    pub const fn parent_id(self) -> Option<CreatureId> {
        match self {
            Self::Adult => None,
            Self::Mini { parent_id } => Some(parent_id),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MiniArrivalState {
    pub enabled: bool,
    pub arrived: [bool; 2],
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LearnedTendencies {
    pub cursor_trust: i8,
    pub sociability: i8,
    pub climbing: i8,
    pub sleep_security: i8,
    pub exploration: i8,
    pub play: i8,
    pub home_affinity: i8,
    pub routine: i8,
}

impl LearnedTendencies {
    pub fn adjust(value: &mut i8, delta: i8) {
        *value = i16::from(*value)
            .saturating_add(i16::from(delta))
            .clamp(-100, 100) as i8;
    }

    pub fn utility(value: i8) -> f32 {
        f32::from(value) / 100.0 * 0.35
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FavoriteDisplayMemory {
    pub display: DisplayKey,
    pub confidence: u8,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreferredRegionMemory {
    pub display: DisplayKey,
    /// Row-major index into a 3×3 display grid.
    pub cell: u8,
    pub confidence: u8,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreatureMemory {
    pub times_petted: u32,
    pub times_tossed: u32,
    pub placements: u32,
    pub sleep_interruptions: u32,
    pub window_climbs: u32,
    pub discoveries_found: u32,
    pub play_sessions: u32,
    pub home_visits: u32,
    pub ledge_seconds: u32,
    pub window_ride_seconds: u32,
    pub longest_sleep_seconds: u32,
    pub favorite_display: Option<FavoriteDisplayMemory>,
    pub preferred_region: Option<PreferredRegionMemory>,
    pub descriptor_flags: u16,
    pub profile_revision: u16,
    pub viewed_profile_revision: u16,
    pub milestone_cooldown_active_seconds: u32,
    pub milestone_bubble_shown: bool,
    /// The little habits it has picked up, in the order it picked them up: at most
    /// [`crate::MAX_HABITS`], one per kind of moment. Absent from the file while there are none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub habits: Vec<crate::Habit>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProfileDescriptor {
    Trusting,
    Wary,
    Social,
    Independent,
    LovesHighPlaces,
    Grounded,
    SoundSleeper,
    RestlessSleeper,
    Adventurous,
    Cautious,
    Playful,
    Calm,
    Homebody,
    Wanderer,
    CreatureOfHabit,
    Spontaneous,
}

impl ProfileDescriptor {
    pub const ALL: [Self; 16] = [
        Self::Trusting,
        Self::Wary,
        Self::Social,
        Self::Independent,
        Self::LovesHighPlaces,
        Self::Grounded,
        Self::SoundSleeper,
        Self::RestlessSleeper,
        Self::Adventurous,
        Self::Cautious,
        Self::Playful,
        Self::Calm,
        Self::Homebody,
        Self::Wanderer,
        Self::CreatureOfHabit,
        Self::Spontaneous,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Trusting => "Trusting",
            Self::Wary => "Wary of the cursor",
            Self::Social => "Social",
            Self::Independent => "Independent",
            Self::LovesHighPlaces => "Loves high places",
            Self::Grounded => "Keeps to the ground",
            Self::SoundSleeper => "Sound sleeper",
            Self::RestlessSleeper => "Restless sleeper",
            Self::Adventurous => "Adventurous",
            Self::Cautious => "Cautious explorer",
            Self::Playful => "Playful",
            Self::Calm => "Calm",
            Self::Homebody => "Loves home",
            Self::Wanderer => "Wanderer",
            Self::CreatureOfHabit => "Creature of habit",
            Self::Spontaneous => "Spontaneous",
        }
    }

    pub const fn flag(self) -> u16 {
        1 << self as u16
    }
}

fn descriptor_value(tendencies: LearnedTendencies, descriptor: ProfileDescriptor) -> i8 {
    match descriptor {
        ProfileDescriptor::Trusting => tendencies.cursor_trust,
        ProfileDescriptor::Wary => -tendencies.cursor_trust,
        ProfileDescriptor::Social => tendencies.sociability,
        ProfileDescriptor::Independent => -tendencies.sociability,
        ProfileDescriptor::LovesHighPlaces => tendencies.climbing,
        ProfileDescriptor::Grounded => -tendencies.climbing,
        ProfileDescriptor::SoundSleeper => tendencies.sleep_security,
        ProfileDescriptor::RestlessSleeper => -tendencies.sleep_security,
        ProfileDescriptor::Adventurous => tendencies.exploration,
        ProfileDescriptor::Cautious => -tendencies.exploration,
        ProfileDescriptor::Playful => tendencies.play,
        ProfileDescriptor::Calm => -tendencies.play,
        ProfileDescriptor::Homebody => tendencies.home_affinity,
        ProfileDescriptor::Wanderer => -tendencies.home_affinity,
        ProfileDescriptor::CreatureOfHabit => tendencies.routine,
        ProfileDescriptor::Spontaneous => -tendencies.routine,
    }
}

pub fn update_descriptor_flags(memory: &mut CreatureMemory, tendencies: LearnedTendencies) -> bool {
    let previous = memory.descriptor_flags;
    for descriptor in ProfileDescriptor::ALL {
        let bit = descriptor.flag();
        let threshold = if previous & bit == 0 { 35 } else { 25 };
        if descriptor_value(tendencies, descriptor) >= threshold {
            memory.descriptor_flags |= bit;
        } else {
            memory.descriptor_flags &= !bit;
        }
    }
    if memory.descriptor_flags != previous {
        memory.profile_revision = memory.profile_revision.saturating_add(1);
        true
    } else {
        false
    }
}

pub fn profile_descriptors(creature: &Creature) -> Vec<ProfileDescriptor> {
    let mut descriptors: Vec<_> = ProfileDescriptor::ALL
        .into_iter()
        .filter(|descriptor| creature.memory.descriptor_flags & descriptor.flag() != 0)
        .collect();
    descriptors.sort_by_key(|descriptor| {
        std::cmp::Reverse(descriptor_value(creature.tendencies, *descriptor))
    });
    descriptors.truncate(3);
    descriptors
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum CreatureNameError {
    #[error("a creature name cannot be empty")]
    Empty,
    #[error("a creature name can contain at most 24 characters")]
    TooLong,
    #[error("a creature name cannot contain control characters or line breaks")]
    ControlCharacter,
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ColonyManagementError {
    #[error("the colony already has {} creatures", MAX_COLONY_CREATURES)]
    ColonyFull,
    #[error("the colony already has {} full-size creatures", MAX_ADULT_CREATURES)]
    AdultLimit,
    #[error("creature not found")]
    CreatureNotFound,
    #[error("the last full-size creature cannot be removed")]
    LastAdult,
    #[error("this creature is marked to keep")]
    CreatureKept,
    #[error("generated creature identity conflicts with an existing colony member")]
    DuplicateIdentity,
}

pub fn validate_creature_name(value: &str) -> Result<String, CreatureNameError> {
    if value.chars().any(char::is_control) {
        return Err(CreatureNameError::ControlCharacter);
    }
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(CreatureNameError::Empty);
    }
    if trimmed.chars().count() > 24 {
        return Err(CreatureNameError::TooLong);
    }
    Ok(trimmed.to_owned())
}

pub fn default_creature_name(
    colony_seed: [u8; 32],
    generation: u8,
    existing_names: &[String],
) -> String {
    const NAMES: [&str; 32] = [
        "Pip", "Mallow", "Clover", "Mochi", "Pebble", "Noodle", "Sprig", "Biscuit", "Fig", "Tansy",
        "Button", "Puddle", "Maple", "Wren", "Dumpling", "Tofu", "Bean", "Miso", "Poppy",
        "Cricket", "Moss", "Pecan", "Lumi", "Tumble", "Juniper", "Dottie", "Sundae", "Nori",
        "Pocket", "Bramble", "Taffy", "Sage",
    ];
    let offset = usize::from(generation).wrapping_mul(7) % colony_seed.len();
    let start = (usize::from(colony_seed[offset]) + usize::from(generation) * 11) % NAMES.len();
    (0..NAMES.len())
        .map(|step| NAMES[(start + step) % NAMES.len()])
        .find(|candidate| !existing_names.iter().any(|name| name == candidate))
        .unwrap_or(NAMES[start])
        .to_owned()
}

/// Where the person at the desk would like a companion to spend its time. It is theirs, not the
/// creature's: kept apart from the innate personality and from what it has learned, it only nudges
/// the companion's own choices, and the habitat, hidden and paused states, and every safety check
/// still decide what it can do. Local to this colony, so it never travels in a share code.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RoamingLeaning {
    /// Wherever it likes: temperament and what it has learned decide.
    #[default]
    Anywhere,
    /// Keeps close to the village and to the floor.
    Homebody,
    /// Rarely climbs, and comes down from a ledge sooner.
    FloorDweller,
    /// Seeks out ledges, and stays up on them longer.
    Climber,
}

impl RoamingLeaning {
    pub const ALL: [Self; 4] = [
        Self::Anywhere,
        Self::Homebody,
        Self::FloorDweller,
        Self::Climber,
    ];

    pub fn is_anywhere(&self) -> bool {
        *self == Self::Anywhere
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Anywhere => "Wherever they like",
            Self::Homebody => "Homebody",
            Self::FloorDweller => "Floor-dweller",
            Self::Climber => "Climber",
        }
    }

    pub const fn description(self) -> &'static str {
        match self {
            Self::Anywhere => "Their own nature and what they have learned decide.",
            Self::Homebody => "Stays close to the village and the floor.",
            Self::FloorDweller => "Rarely climbs, and comes down from a ledge sooner.",
            Self::Climber => "Seeks out ledges and stays up on them longer.",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Creature {
    pub id: CreatureId,
    pub generation: u8,
    pub origin: CreatureOrigin,
    pub colony_order: u8,
    #[serde(default)]
    pub role: CreatureRole,
    #[serde(default = "default_true")]
    pub kept: bool,
    #[serde(default)]
    pub mini_arrivals: MiniArrivalState,
    pub name: String,
    #[serde(default = "default_born_at_utc", with = "time::serde::rfc3339")]
    pub born_at_utc: OffsetDateTime,
    pub display_scale_percent: u8,
    pub appearance: AppearanceGenome,
    pub personality: PersonalityGenome,
    pub behavior_seed: [u8; 32],
    pub memory: CreatureMemory,
    pub tendencies: LearnedTendencies,
    pub routines: RoutineTable,
    pub state: CreatureState,
    /// Where its owner would like it to roam. Absent from the file while it is `Anywhere`.
    #[serde(default, skip_serializing_if = "RoamingLeaning::is_anywhere")]
    pub leaning: RoamingLeaning,
    /// What it is wearing, chosen by its owner. Absent from the file while it wears nothing.
    /// Never carried in a share code: a companion shared with a friend arrives as itself.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accessory: Option<crate::Accessory>,
}

const fn default_true() -> bool {
    true
}

fn default_born_at_utc() -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ArrivalState {
    pub arrived: [bool; 3],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RitualKind {
    Picnic,
    GroupNap,
    FloorRace,
    ShelterGathering,
    Catch,
    GroupPresentation,
    HatchDay,
    QuietDayHuddle,
    LateNightSleepPile,
    /// A dance on the ground between the houses. Only ever invited, never scheduled.
    Dance,
}

impl RitualKind {
    pub const ALL: [Self; 10] = [
        Self::Picnic,
        Self::GroupNap,
        Self::FloorRace,
        Self::ShelterGathering,
        Self::Catch,
        Self::GroupPresentation,
        Self::HatchDay,
        Self::QuietDayHuddle,
        Self::LateNightSleepPile,
        Self::Dance,
    ];
}

/// Something the person at the desk can invite the whole village to share while the houses are
/// out, on the ground between them. Runtime only: the journal records one that was shared as the
/// shared moment it is, and nothing else about it is kept.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum VillageMoment {
    Picnic,
    Dance,
    Nap,
}

impl VillageMoment {
    pub const ALL: [Self; 3] = [Self::Picnic, Self::Dance, Self::Nap];

    /// The shared moment the journal writes it down as.
    pub const fn ritual(self) -> RitualKind {
        match self {
            Self::Picnic => RitualKind::Picnic,
            Self::Dance => RitualKind::Dance,
            Self::Nap => RitualKind::GroupNap,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RitualState {
    #[serde(with = "time::serde::rfc3339")]
    pub next_at_utc: OffsetDateTime,
    pub last_kind: Option<RitualKind>,
    pub ordinal: u32,
    pub hatch_day_acknowledged_year: Option<i32>,
}

impl Default for RitualState {
    fn default() -> Self {
        Self {
            next_at_utc: OffsetDateTime::UNIX_EPOCH,
            last_kind: None,
            ordinal: 0,
            hatch_day_acknowledged_year: None,
        }
    }
}

pub const MAX_COLONY_OBJECTS: usize = 8;

/// A belonging the colony keeps in the two trees' yards. The first eight are the ones colonies
/// have always been given; the rest joined them in 0.60.0.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ColonyObjectKind {
    #[default]
    Pillow,
    Toy,
    Plant,
    Blanket,
    Paper,
    Pebble,
    Lamp,
    Cup,
    Kite,
    Teapot,
    Book,
    Basket,
    YarnBall,
    Drum,
    Umbrella,
    Bucket,
    Candle,
    MusicBox,
    SpinningTop,
    Jar,
}

impl ColonyObjectKind {
    pub const ALL: [Self; 20] = [
        Self::Pillow,
        Self::Toy,
        Self::Plant,
        Self::Blanket,
        Self::Paper,
        Self::Pebble,
        Self::Lamp,
        Self::Cup,
        Self::Kite,
        Self::Teapot,
        Self::Book,
        Self::Basket,
        Self::YarnBall,
        Self::Drum,
        Self::Umbrella,
        Self::Bucket,
        Self::Candle,
        Self::MusicBox,
        Self::SpinningTop,
        Self::Jar,
    ];

    pub const fn index(self) -> u8 {
        self as u8
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Pillow => "Pillow",
            Self::Toy => "Toy",
            Self::Plant => "Plant",
            Self::Blanket => "Blanket",
            Self::Paper => "Paper",
            Self::Pebble => "Pebble",
            Self::Lamp => "Lamp",
            Self::Cup => "Cup",
            Self::Kite => "Kite",
            Self::Teapot => "Teapot",
            Self::Book => "Book",
            Self::Basket => "Basket",
            Self::YarnBall => "Ball of yarn",
            Self::Drum => "Drum",
            Self::Umbrella => "Umbrella",
            Self::Bucket => "Bucket",
            Self::Candle => "Candle",
            Self::MusicBox => "Music box",
            Self::SpinningTop => "Spinning top",
            Self::Jar => "Jar",
        }
    }

    pub const fn default_role(self) -> ColonyObjectRole {
        match self {
            Self::Pillow | Self::Blanket | Self::MusicBox => ColonyObjectRole::Sleep,
            Self::Toy | Self::Kite | Self::YarnBall | Self::Drum | Self::SpinningTop => {
                ColonyObjectRole::Play
            }
            Self::Plant | Self::Lamp | Self::Basket | Self::Umbrella | Self::Candle => {
                ColonyObjectRole::Comfort
            }
            Self::Paper | Self::Pebble | Self::Book | Self::Bucket | Self::Jar => {
                ColonyObjectRole::Curiosity
            }
            Self::Cup | Self::Teapot => ColonyObjectRole::Social,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ColonyObjectRole {
    #[default]
    Comfort,
    Sleep,
    Play,
    Social,
    Curiosity,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ColonyObject {
    pub id: u64,
    pub kind: ColonyObjectKind,
    pub display: DisplayKey,
    pub normalized_position: Point,
    pub role: ColonyObjectRole,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ColonyObjectState {
    pub objects: Vec<ColonyObject>,
    #[serde(with = "time::serde::rfc3339")]
    pub next_at_utc: OffsetDateTime,
    pub ordinal: u32,
}

impl Default for ColonyObjectState {
    fn default() -> Self {
        Self {
            objects: Vec::new(),
            next_at_utc: OffsetDateTime::UNIX_EPOCH,
            ordinal: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum HomeCorner {
    #[default]
    BottomLeft,
    BottomRight,
}

/// The four kinds of house, each built from a shape of its own. A colony file keeps the names
/// they had before they were drawn this way, so every existing colony keeps its houses.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ShelterStyle {
    /// A tent pitched from triangles.
    #[default]
    #[serde(rename = "LeafTent")]
    Tent,
    /// A mushroom made of circles.
    #[serde(rename = "MushroomHut")]
    Mushroom,
    /// A pillow fort stacked from squares.
    #[serde(rename = "CushionDen")]
    PillowFort,
    /// A cottage roofed and trimmed in leaves.
    #[serde(rename = "PaperHouse")]
    LeafHouse,
}

impl ShelterStyle {
    pub const ALL: [Self; 4] = [
        Self::Tent,
        Self::Mushroom,
        Self::PillowFort,
        Self::LeafHouse,
    ];

    /// What the Home page calls it.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Tent => "Tent",
            Self::Mushroom => "Mushroom",
            Self::PillowFort => "Pillow fort",
            Self::LeafHouse => "Leaf house",
        }
    }

    /// The house a companion would build for itself, from its own seed: the same one every time,
    /// and a different one from companion to companion often enough that a village mixes.
    pub fn for_keeper(creature: &Creature) -> Self {
        let pick = creature.behavior_seed[13] ^ creature.behavior_seed[29].rotate_left(3);
        Self::ALL[usize::from(pick) % Self::ALL.len()]
    }
}

/// A house type the person at the desk chose for one house, by the companion who keeps it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HouseStyleChoice {
    pub keeper: CreatureId,
    pub style: ShelterStyle,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShelterGenome {
    pub style: ShelterStyle,
    pub palette_index: u8,
    pub accent_index: u8,
    pub width: u8,
    pub height: u8,
    pub detail_seed: u64,
}

/// Where on a house a decoration hangs. Every house has one of each, and each takes one
/// decoration at a time.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum DecorationSlot {
    /// The peak of the roof.
    Roof,
    /// Strung along under the eaves.
    Eaves,
    /// On the front wall, left of the door.
    WallLeft,
    /// On the front wall, right of the door.
    WallRight,
    /// Set on the ground beside the left wall.
    GroundLeft,
    /// Set on the ground beside the right wall.
    GroundRight,
}

impl DecorationSlot {
    pub const ALL: [Self; 6] = [
        Self::Roof,
        Self::Eaves,
        Self::WallLeft,
        Self::WallRight,
        Self::GroundLeft,
        Self::GroundRight,
    ];

    pub const fn index(self) -> usize {
        match self {
            Self::Roof => 0,
            Self::Eaves => 1,
            Self::WallLeft => 2,
            Self::WallRight => 3,
            Self::GroundLeft => 4,
            Self::GroundRight => 5,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Roof => "Roof",
            Self::Eaves => "Eaves",
            Self::WallLeft => "Left wall",
            Self::WallRight => "Right wall",
            Self::GroundLeft => "Left of the house",
            Self::GroundRight => "Right of the house",
        }
    }
}

/// The most decorations one house wears: one in each slot.
pub const MAX_HOUSE_DECORATIONS: usize = DecorationSlot::ALL.len();

/// Something a house can be decorated with. The first six are the ones a colony earned before
/// 0.60.0, under the names its file keeps them by; each kind belongs to one slot on the house.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub enum ShelterDecorationKind {
    #[default]
    Leaf,
    Banner,
    Stone,
    Flower,
    Lamp,
    RoofOrnament,
    WeatherVane,
    Pennant,
    PerchedBird,
    Pinwheel,
    FairyLights,
    WindChime,
    LeafGarland,
    PaperLanterns,
    Wreath,
    WindowBox,
    Ivy,
    HouseSign,
    Clock,
    Birdhouse,
    Horseshoe,
    Mailbox,
    Woodpile,
    WateringCan,
    Boots,
    Barrel,
    PottedPlant,
    Pumpkin,
    Mushrooms,
    Lantern,
}

impl ShelterDecorationKind {
    pub const ALL: [Self; 30] = [
        Self::Leaf,
        Self::Banner,
        Self::Stone,
        Self::Flower,
        Self::Lamp,
        Self::RoofOrnament,
        Self::WeatherVane,
        Self::Pennant,
        Self::PerchedBird,
        Self::Pinwheel,
        Self::FairyLights,
        Self::WindChime,
        Self::LeafGarland,
        Self::PaperLanterns,
        Self::Wreath,
        Self::WindowBox,
        Self::Ivy,
        Self::HouseSign,
        Self::Clock,
        Self::Birdhouse,
        Self::Horseshoe,
        Self::Mailbox,
        Self::Woodpile,
        Self::WateringCan,
        Self::Boots,
        Self::Barrel,
        Self::PottedPlant,
        Self::Pumpkin,
        Self::Mushrooms,
        Self::Lantern,
    ];

    /// The decorations a new colony can put up from its first day.
    pub const STARTING: [Self; 3] = [Self::Banner, Self::Flower, Self::Lamp];

    pub const fn index(self) -> usize {
        self as usize
    }

    /// Where on a house it hangs.
    pub const fn slot(self) -> DecorationSlot {
        match self {
            Self::RoofOrnament
            | Self::WeatherVane
            | Self::Pennant
            | Self::PerchedBird
            | Self::Pinwheel => DecorationSlot::Roof,
            Self::Banner
            | Self::FairyLights
            | Self::WindChime
            | Self::LeafGarland
            | Self::PaperLanterns => DecorationSlot::Eaves,
            Self::Leaf | Self::Wreath | Self::WindowBox | Self::Ivy | Self::HouseSign => {
                DecorationSlot::WallLeft
            }
            Self::Lamp | Self::Clock | Self::Birdhouse | Self::Horseshoe | Self::Mailbox => {
                DecorationSlot::WallRight
            }
            Self::Stone | Self::Woodpile | Self::WateringCan | Self::Boots | Self::Barrel => {
                DecorationSlot::GroundLeft
            }
            Self::Flower | Self::PottedPlant | Self::Pumpkin | Self::Mushrooms | Self::Lantern => {
                DecorationSlot::GroundRight
            }
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Leaf => "Leaf sprig",
            Self::Banner => "Bunting",
            Self::Stone => "Doorstone",
            Self::Flower => "Flower",
            Self::Lamp => "Wall lamp",
            Self::RoofOrnament => "Roof star",
            Self::WeatherVane => "Weather vane",
            Self::Pennant => "Pennant",
            Self::PerchedBird => "Perched bird",
            Self::Pinwheel => "Pinwheel",
            Self::FairyLights => "Fairy lights",
            Self::WindChime => "Wind chime",
            Self::LeafGarland => "Leaf garland",
            Self::PaperLanterns => "Paper lanterns",
            Self::Wreath => "Wreath",
            Self::WindowBox => "Window box",
            Self::Ivy => "Ivy",
            Self::HouseSign => "House sign",
            Self::Clock => "Clock",
            Self::Birdhouse => "Birdhouse",
            Self::Horseshoe => "Horseshoe",
            Self::Mailbox => "Letterbox",
            Self::Woodpile => "Woodpile",
            Self::WateringCan => "Watering can",
            Self::Boots => "Boots",
            Self::Barrel => "Rain barrel",
            Self::PottedPlant => "Potted plant",
            Self::Pumpkin => "Pumpkin",
            Self::Mushrooms => "Mushrooms",
            Self::Lantern => "Lantern",
        }
    }
}

/// Which decorations one house wears, by the companion who keeps it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HouseDressing {
    pub keeper: CreatureId,
    /// At most one for each slot, in slot order.
    pub decorations: Vec<ShelterDecorationKind>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ColonyHome {
    pub display: Option<DisplayKey>,
    pub corner: HomeCorner,
    pub shelter: ShelterGenome,
    #[serde(with = "time::serde::rfc3339::option")]
    pub active_since_utc: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub last_disappeared_utc: Option<OffsetDateTime>,
    /// The spots the person at the desk has put down on the ground between the houses. Absent
    /// from the file while there are none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hangouts: Vec<HangoutSpot>,
    /// The order the cottages stand in, as the person at the desk arranged them: companions by
    /// id, the founder's colony house always first. Absent while they stand as they arrived.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cottage_order: Vec<CreatureId>,
    /// A named palette the village is painted in, in place of the colours it was generated with.
    /// Absent while it keeps its own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub palette: Option<VillagePalette>,
    /// Little garden patches planted along the ground. Absent while there are none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gardens: Vec<GardenPatch>,
    /// House types chosen by hand, one at most for each companion who keeps a house. A house
    /// with none is the colony's own type for the colony house and its keeper's own for a
    /// cottage. Absent while none has been chosen.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub house_styles: Vec<HouseStyleChoice>,
    /// What the village has to choose from, and when the next thing arrives.
    #[serde(default)]
    pub unlocks: VillageUnlocks,
    /// The decorations each house wears, by the companion who keeps it. A house with no entry
    /// wears none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dressing: Vec<HouseDressing>,
    /// Ornaments set out on the village ground. Absent while there are none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ornaments: Vec<OrnamentSpot>,
    /// The keepsakes chosen to hang in the two trees. Absent while the trees fill themselves.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tree_keepsakes: Option<TreeKeepsakes>,
}

/// A palette the village can be painted in: a hand-made pairing of a main colour for roofs,
/// caps, leaves and fabric with an accent for the trim. Choosing none keeps the colours the
/// colony's own seed gave it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VillagePalette {
    Meadow,
    Blossom,
    Harbour,
    Autumn,
    Twilight,
    Pebble,
}

impl VillagePalette {
    pub const ALL: [Self; 6] = [
        Self::Meadow,
        Self::Blossom,
        Self::Harbour,
        Self::Autumn,
        Self::Twilight,
        Self::Pebble,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Meadow => "Meadow",
            Self::Blossom => "Blossom",
            Self::Harbour => "Harbour",
            Self::Autumn => "Autumn",
            Self::Twilight => "Twilight",
            Self::Pebble => "Pebble",
        }
    }

    /// The main and accent palettes it paints the village with, from the same twelve hand-made
    /// palettes the colony's own colours are drawn from. Some houses and the tree wear the main
    /// colour most and some the accent, so both halves of a pair belong to its name: two greens
    /// for a meadow, two pinks for blossom, sea blues for a harbour, warm oranges for autumn,
    /// dusky purples for twilight, and stone grey with moss for pebbles.
    pub const fn palettes(self) -> (u8, u8) {
        match self {
            Self::Meadow => (4, 8),
            Self::Blossom => (0, 10),
            Self::Harbour => (6, 1),
            Self::Autumn => (2, 5),
            Self::Twilight => (7, 3),
            Self::Pebble => (9, 11),
        }
    }
}

/// The most garden patches planted at once, each of a different kind.
pub const MAX_GARDENS: usize = 4;

/// A little patch planted on the village ground. It grows by itself through four stages and back
/// round again, and the colony tends it now and then.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GardenKind {
    Flowers,
    Vegetables,
    Herbs,
    Sunflowers,
    Pumpkins,
    Strawberries,
    MushroomRing,
    BerryBush,
    Tulips,
    Cactus,
    PeaTrellis,
    Tomatoes,
}

impl GardenKind {
    pub const ALL: [Self; 12] = [
        Self::Flowers,
        Self::Vegetables,
        Self::Herbs,
        Self::Sunflowers,
        Self::Pumpkins,
        Self::Strawberries,
        Self::MushroomRing,
        Self::BerryBush,
        Self::Tulips,
        Self::Cactus,
        Self::PeaTrellis,
        Self::Tomatoes,
    ];

    /// The gardens a new colony can plant from its first day.
    pub const STARTING: [Self; 3] = [Self::Flowers, Self::Vegetables, Self::Herbs];

    pub const fn index(self) -> u8 {
        self as u8
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Flowers => "Flower bed",
            Self::Vegetables => "Vegetable patch",
            Self::Herbs => "Herb box",
            Self::Sunflowers => "Sunflowers",
            Self::Pumpkins => "Pumpkin patch",
            Self::Strawberries => "Strawberry planter",
            Self::MushroomRing => "Mushroom ring",
            Self::BerryBush => "Berry bush",
            Self::Tulips => "Tulip row",
            Self::Cactus => "Cactus pots",
            Self::PeaTrellis => "Pea trellis",
            Self::Tomatoes => "Tomato cane",
        }
    }

    pub const fn description(self) -> &'static str {
        match self {
            Self::Flowers => "Three flowers in the colony's own colours.",
            Self::Vegetables => "A cabbage, a carrot and a pumpkin coming along.",
            Self::Herbs => "Rosemary, basil and lavender in a planter box.",
            Self::Sunflowers => "Two tall sunflowers that turn to follow the light.",
            Self::Pumpkins => "A trailing vine with one pumpkin growing fat on it.",
            Self::Strawberries => "A strawberry pot with runners over the rim.",
            Self::MushroomRing => "A little ring of mushrooms that came up by itself.",
            Self::BerryBush => "A round bush that fills with berries.",
            Self::Tulips => "A row of tulips standing to attention.",
            Self::Cactus => "Two small cacti in clay pots, one of them flowering.",
            Self::PeaTrellis => "Peas climbing a little lattice of sticks.",
            Self::Tomatoes => "A tomato plant tied to a cane.",
        }
    }

    /// Whether what grows here is something a companion might pick and eat.
    pub const fn edible(self) -> bool {
        matches!(
            self,
            Self::Vegetables
                | Self::Herbs
                | Self::Pumpkins
                | Self::Strawberries
                | Self::BerryBush
                | Self::PeaTrellis
                | Self::Tomatoes
        )
    }

    /// Whether this is a patch someone would proudly hold something up from.
    pub const fn harvest(self) -> bool {
        matches!(
            self,
            Self::Vegetables | Self::Pumpkins | Self::PeaTrellis | Self::Tomatoes
        )
    }

    /// How long each stage of growing lasts, in hours: quick herbs, slow pumpkins.
    pub const fn stage_hours(self) -> i64 {
        match self {
            Self::Herbs | Self::MushroomRing => 5,
            Self::Flowers | Self::Tulips | Self::Strawberries => 6,
            Self::Vegetables | Self::PeaTrellis | Self::Tomatoes | Self::BerryBush => 8,
            Self::Sunflowers | Self::Cactus => 10,
            Self::Pumpkins => 12,
        }
    }
}

/// How far along a garden patch is. A patch goes round these by itself — no watering needed, and
/// nothing wilts for want of it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GardenStage {
    /// Just a few green shoots.
    Sprout,
    /// Leafy, with nothing on it yet.
    Growing,
    /// Flowering or fruiting: how a patch looked before it grew.
    Grown,
    /// At its fullest, before it goes back to seed and starts over.
    Bounty,
}

impl GardenStage {
    pub const ALL: [Self; 4] = [Self::Sprout, Self::Growing, Self::Grown, Self::Bounty];

    pub const fn index(self) -> u8 {
        self as u8
    }
}

/// One garden patch: what is growing in it, how far along the village ground it is, as a
/// fraction from its left end to its right, and when it was planted.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct GardenPatch {
    pub kind: GardenKind,
    pub along: f32,
    /// Absent for a patch planted before gardens grew; such a patch is taken to have been planted
    /// long ago, part way round its cycle.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "time::serde::rfc3339::option"
    )]
    pub planted_at_utc: Option<OffsetDateTime>,
}

impl GardenPatch {
    /// How far along this patch is at `now`. It starts as a sprout when it is planted, and then
    /// goes round sprout, growing, grown and bounty for as long as it stays in the ground. A clock
    /// set back never un-plants it.
    pub fn stage(&self, now: OffsetDateTime) -> GardenStage {
        let hours = match self.planted_at_utc {
            Some(planted) => (now - planted).whole_hours().max(0),
            // An old patch: somewhere round its cycle, the same place for the same kind, and
            // moving on from there with the clock like any other.
            None => now.unix_timestamp() / 3600 + i64::from(self.kind.index()) * 7,
        };
        let stage = (hours / self.kind.stage_hours()).rem_euclid(4);
        GardenStage::ALL[stage as usize]
    }
}

/// The most hangout spots put down at once, each of a different kind.
pub const MAX_HANGOUTS: usize = 4;

/// Something the person at the desk can put down on the village ground for the colony to gather
/// at. Each gently draws one kind of quiet moment at home to it; a companion is free to do
/// something else instead.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HangoutKind {
    /// A plump floor cushion: somewhere to nap.
    Cushion,
    /// A picnic blanket spread out: somewhere to snack and sip.
    Blanket,
    /// A little spyglass on a stand: somewhere to stand and look out.
    Lookout,
    Hammock,
    Swing,
    TeaTable,
    BookNook,
    Campfire,
    Sandbox,
    Puddle,
    Bench,
    DrumStump,
    BirdFeeder,
    StargazingMat,
    SunnyRock,
}

impl HangoutKind {
    pub const ALL: [Self; 15] = [
        Self::Cushion,
        Self::Blanket,
        Self::Lookout,
        Self::Hammock,
        Self::Swing,
        Self::TeaTable,
        Self::BookNook,
        Self::Campfire,
        Self::Sandbox,
        Self::Puddle,
        Self::Bench,
        Self::DrumStump,
        Self::BirdFeeder,
        Self::StargazingMat,
        Self::SunnyRock,
    ];

    /// The spots a new colony can put down from its first day.
    pub const STARTING: [Self; 3] = [Self::Cushion, Self::Blanket, Self::Lookout];

    pub const fn index(self) -> u8 {
        self as u8
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Cushion => "Nap cushion",
            Self::Blanket => "Picnic blanket",
            Self::Lookout => "Lookout",
            Self::Hammock => "Hammock",
            Self::Swing => "Swing",
            Self::TeaTable => "Tea table",
            Self::BookNook => "Book nook",
            Self::Campfire => "Campfire",
            Self::Sandbox => "Sandbox",
            Self::Puddle => "Splash puddle",
            Self::Bench => "Bench",
            Self::DrumStump => "Drum stump",
            Self::BirdFeeder => "Bird feeder",
            Self::StargazingMat => "Stargazing mat",
            Self::SunnyRock => "Sunny rock",
        }
    }

    pub const fn description(self) -> &'static str {
        match self {
            Self::Cushion => "Somewhere soft for a nap at home.",
            Self::Blanket => "Somewhere to snack and sip, and where a picnic gathers.",
            Self::Lookout => "Somewhere to stand and look out over the desktop.",
            Self::Hammock => "Slung between two posts, for a swaying nap.",
            Self::Swing => "A plank on two ropes, for swinging on.",
            Self::TeaTable => "A little table set for tea, for a sip and a sit.",
            Self::BookNook => "A stack of books to sit by and read.",
            Self::Campfire => "A ring of stones and a small fire to warm paws at.",
            Self::Sandbox => "A box of sand, for digging in.",
            Self::Puddle => "A puddle kept on purpose, for splashing.",
            Self::Bench => "A bench for sitting and watching the village.",
            Self::DrumStump => "A hollow stump that makes a good drum.",
            Self::BirdFeeder => "A feeder on a pole, and birds to watch at it.",
            Self::StargazingMat => "A mat to lie back on and look up from.",
            Self::SunnyRock => "A flat rock that holds the warmth, for basking.",
        }
    }
}

/// The most ornaments set out at once, each of a different kind.
pub const MAX_ORNAMENTS: usize = 4;

/// A standing ornament for the village ground: something to look at and to wander over and
/// inspect, rather than somewhere to spend a quiet moment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OrnamentKind {
    LampPost,
    BirdBath,
    Signpost,
    WishingWell,
    PicketFence,
    SteppingStones,
    Scarecrow,
    WindSpinner,
    Wheelbarrow,
    Beehive,
    StoneCairn,
    LilyPond,
    MailboxPost,
    FlagPole,
    LogStool,
}

impl OrnamentKind {
    pub const ALL: [Self; 15] = [
        Self::LampPost,
        Self::BirdBath,
        Self::Signpost,
        Self::WishingWell,
        Self::PicketFence,
        Self::SteppingStones,
        Self::Scarecrow,
        Self::WindSpinner,
        Self::Wheelbarrow,
        Self::Beehive,
        Self::StoneCairn,
        Self::LilyPond,
        Self::MailboxPost,
        Self::FlagPole,
        Self::LogStool,
    ];

    /// The ornaments a new colony can set out from its first day.
    pub const STARTING: [Self; 3] = [Self::LampPost, Self::BirdBath, Self::Signpost];

    pub const fn index(self) -> u8 {
        self as u8
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::LampPost => "Lamp post",
            Self::BirdBath => "Bird bath",
            Self::Signpost => "Signpost",
            Self::WishingWell => "Wishing well",
            Self::PicketFence => "Picket fence",
            Self::SteppingStones => "Stepping stones",
            Self::Scarecrow => "Scarecrow",
            Self::WindSpinner => "Wind spinner",
            Self::Wheelbarrow => "Wheelbarrow",
            Self::Beehive => "Beehive",
            Self::StoneCairn => "Stone stack",
            Self::LilyPond => "Lily pond",
            Self::MailboxPost => "Post box",
            Self::FlagPole => "Flag pole",
            Self::LogStool => "Log stool",
        }
    }

    pub const fn description(self) -> &'static str {
        match self {
            Self::LampPost => "A lamp on a post that lights up after dark.",
            Self::BirdBath => "A stone bowl of water for passing birds.",
            Self::Signpost => "Two arrows pointing to places nobody has been.",
            Self::WishingWell => "A little well with a roof and a bucket.",
            Self::PicketFence => "A short run of white fence.",
            Self::SteppingStones => "Flat stones set in the grass.",
            Self::Scarecrow => "A scarecrow that scares nothing at all.",
            Self::WindSpinner => "A spinner on a pole that turns in the slightest breeze.",
            Self::Wheelbarrow => "A wheelbarrow, parked with a few things in it.",
            Self::Beehive => "A round straw hive and its busy bees.",
            Self::StoneCairn => "Stones balanced one on another.",
            Self::LilyPond => "A tiny pond with a lily pad on it.",
            Self::MailboxPost => "A post box on a pole, waiting for letters.",
            Self::FlagPole => "A tall pole with the colony's own flag.",
            Self::LogStool => "A round of log to sit on.",
        }
    }
}

/// One ornament: what it is, and how far along the village ground it stands.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct OrnamentSpot {
    pub kind: OrnamentKind,
    pub along: f32,
}

/// One hangout spot: what it is, and how far along the village ground it stands, as a fraction
/// from its left end to its right, so it keeps its place on the ground as the village grows,
/// shrinks, or moves.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct HangoutSpot {
    pub kind: HangoutKind,
    pub along: f32,
}

/// Something the village can gain over time: a decoration for its houses, a hangout spot, a
/// garden, or an ornament for its ground.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VillageItem {
    Decoration(ShelterDecorationKind),
    Hangout(HangoutKind),
    Garden(GardenKind),
    Ornament(OrnamentKind),
}

impl VillageItem {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Decoration(kind) => kind.label(),
            Self::Hangout(kind) => kind.label(),
            Self::Garden(kind) => kind.label(),
            Self::Ornament(kind) => kind.label(),
        }
    }

    /// Everything the village can gain, in the order the catalogues list them.
    pub fn all() -> impl Iterator<Item = Self> {
        ShelterDecorationKind::ALL
            .into_iter()
            .map(Self::Decoration)
            .chain(HangoutKind::ALL.into_iter().map(Self::Hangout))
            .chain(GardenKind::ALL.into_iter().map(Self::Garden))
            .chain(OrnamentKind::ALL.into_iter().map(Self::Ornament))
    }
}

/// What the village has to choose from so far, and when the next thing arrives. Every category
/// starts with three, and one more arrives every day or two for as long as there is anything left.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct VillageUnlocks {
    pub decorations: Vec<ShelterDecorationKind>,
    pub hangouts: Vec<HangoutKind>,
    pub gardens: Vec<GardenKind>,
    pub ornaments: Vec<OrnamentKind>,
    #[serde(with = "time::serde::rfc3339")]
    pub next_at_utc: OffsetDateTime,
    pub ordinal: u32,
}

impl Default for VillageUnlocks {
    fn default() -> Self {
        Self::starting()
    }
}

impl VillageUnlocks {
    /// A new colony's village: three of everything.
    pub fn starting() -> Self {
        Self {
            decorations: ShelterDecorationKind::STARTING.to_vec(),
            hangouts: HangoutKind::STARTING.to_vec(),
            gardens: GardenKind::STARTING.to_vec(),
            ornaments: OrnamentKind::STARTING.to_vec(),
            next_at_utc: OffsetDateTime::UNIX_EPOCH,
            ordinal: 0,
        }
    }

    /// Whether the village has this to choose from yet.
    pub fn has(&self, item: VillageItem) -> bool {
        match item {
            VillageItem::Decoration(kind) => self.decorations.contains(&kind),
            VillageItem::Hangout(kind) => self.hangouts.contains(&kind),
            VillageItem::Garden(kind) => self.gardens.contains(&kind),
            VillageItem::Ornament(kind) => self.ornaments.contains(&kind),
        }
    }

    /// Add something to choose from. Returns whether it was new.
    pub fn grant(&mut self, item: VillageItem) -> bool {
        if self.has(item) {
            return false;
        }
        match item {
            VillageItem::Decoration(kind) => self.decorations.push(kind),
            VillageItem::Hangout(kind) => self.hangouts.push(kind),
            VillageItem::Garden(kind) => self.gardens.push(kind),
            VillageItem::Ornament(kind) => self.ornaments.push(kind),
        }
        true
    }

    /// Everything still to come.
    pub fn remaining(&self) -> impl Iterator<Item = VillageItem> + '_ {
        VillageItem::all().filter(|item| !self.has(*item))
    }

    /// Each kind once, in the order it arrived, and every category topped up to its first three.
    pub fn normalize(&mut self) {
        fn dedup<T: PartialEq + Copy>(list: &mut Vec<T>, starting: &[T]) {
            let mut seen: Vec<T> = Vec::with_capacity(list.len());
            list.retain(|item| {
                let fresh = !seen.contains(item);
                seen.push(*item);
                fresh
            });
            for item in starting {
                if !list.contains(item) {
                    list.push(*item);
                }
            }
        }
        dedup(&mut self.decorations, &ShelterDecorationKind::STARTING);
        dedup(&mut self.hangouts, &HangoutKind::STARTING);
        dedup(&mut self.gardens, &GardenKind::STARTING);
        dedup(&mut self.ornaments, &OrnamentKind::STARTING);
    }
}

/// The number of hooks across the two keepsake trees: eight on each.
pub const TREE_HOOKS: usize = 16;

/// Which keepsakes hang on the trees' hooks, as the person at the desk chose them: a variant or
/// nothing for each hook, the outward tree's eight first. Absent until anything is chosen, in
/// which case the trees fill themselves as finds come in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TreeKeepsakes {
    pub hooks: [Option<u8>; TREE_HOOKS],
}

impl TreeKeepsakes {
    /// How many hooks hold something.
    pub fn hung(&self) -> usize {
        self.hooks.iter().flatten().count()
    }
}

/// What hangs on each of the sixteen hooks. With nothing chosen, the original sixteen finds hang
/// on the hooks they have always hung on — a hook for each of them — and anything found since
/// fills the hooks still empty, in the order it was found. With a choice, exactly what was chosen,
/// less anything the scrapbook does not hold.
pub fn hung_keepsakes(
    chosen: Option<&TreeKeepsakes>,
    scrapbook: &[crate::ScrapbookRecord],
) -> [Option<u8>; TREE_HOOKS] {
    let found = |variant: u8| {
        variant < crate::TRINKET_VARIANTS
            && scrapbook.iter().any(|record| record.variant == variant)
    };
    if let Some(chosen) = chosen {
        return chosen
            .hooks
            .map(|hook| hook.filter(|variant| found(*variant)));
    }
    let mut hooks = [None; TREE_HOOKS];
    for record in scrapbook {
        if usize::from(record.variant) < TREE_HOOKS {
            hooks[usize::from(record.variant)] = Some(record.variant);
        }
    }
    let mut later: Vec<&crate::ScrapbookRecord> = scrapbook
        .iter()
        .filter(|record| usize::from(record.variant) >= TREE_HOOKS && found(record.variant))
        .collect();
    later.sort_by_key(|record| (record.first_at, record.variant));
    let mut later = later.into_iter();
    for hook in &mut hooks {
        if hook.is_none() {
            *hook = later.next().map(|record| record.variant);
        }
    }
    hooks
}

impl ColonyHome {
    pub fn from_seed(
        seed: [u8; 32],
        display: Option<DisplayKey>,
        active_since_utc: Option<OffsetDateTime>,
        last_disappeared_utc: Option<OffsetDateTime>,
    ) -> Self {
        let detail_seed = u64::from_le_bytes(seed[8..16].try_into().unwrap());
        Self {
            display,
            corner: if seed[0] & 1 == 0 {
                HomeCorner::BottomLeft
            } else {
                HomeCorner::BottomRight
            },
            shelter: ShelterGenome {
                style: ShelterStyle::ALL[usize::from(seed[1] % 4)],
                palette_index: seed[2] % 12,
                accent_index: seed[3] % 12,
                width: 34 + seed[4] % 9,
                height: 27 + seed[5] % 10,
                detail_seed,
            },
            active_since_utc,
            last_disappeared_utc,
            hangouts: Vec::new(),
            cottage_order: Vec::new(),
            palette: None,
            gardens: Vec::new(),
            house_styles: Vec::new(),
            unlocks: VillageUnlocks::starting(),
            dressing: Vec::new(),
            ornaments: Vec::new(),
            tree_keepsakes: None,
        }
    }

    /// The type of every house in the village, in the order they stand: whatever was chosen for
    /// it, or else the colony's own type for the colony house and the keeper's own for a
    /// cottage. Slots past the last house are the colony's own type.
    pub fn house_style_list(&self, creatures: &[Creature]) -> [ShelterStyle; MAX_COLONY_CREATURES] {
        let mut styles = [self.shelter.style; MAX_COLONY_CREATURES];
        let owners = crate::house_owners(creatures, &self.cottage_order);
        for (slot, keeper) in owners.as_slice().iter().enumerate() {
            styles[slot] = match self.house_style(*keeper) {
                Some(chosen) => chosen,
                None if slot == 0 => self.shelter.style,
                None => creatures
                    .iter()
                    .find(|creature| creature.id == *keeper)
                    .map_or(self.shelter.style, ShelterStyle::for_keeper),
            };
        }
        styles
    }

    /// The house type chosen by hand for the house this companion keeps, if one was.
    pub fn house_style(&self, keeper: CreatureId) -> Option<ShelterStyle> {
        self.house_styles
            .iter()
            .find(|choice| choice.keeper == keeper)
            .map(|choice| choice.style)
    }

    /// Choose a type for the house this companion keeps, or with `None` give it back its own.
    pub fn set_house_style(&mut self, keeper: CreatureId, style: Option<ShelterStyle>) {
        self.house_styles.retain(|choice| choice.keeper != keeper);
        if let Some(style) = style {
            self.house_styles.push(HouseStyleChoice { keeper, style });
        }
        self.normalize_village();
    }

    /// The decorations the house this companion keeps wears, one per slot at most, in slot order.
    pub fn decorations_of(&self, keeper: CreatureId) -> &[ShelterDecorationKind] {
        self.dressing
            .iter()
            .find(|dressing| dressing.keeper == keeper)
            .map_or(&[], |dressing| dressing.decorations.as_slice())
    }

    /// The decoration in one slot of the house this companion keeps.
    pub fn decoration_in(
        &self,
        keeper: CreatureId,
        slot: DecorationSlot,
    ) -> Option<ShelterDecorationKind> {
        self.decorations_of(keeper)
            .iter()
            .copied()
            .find(|kind| kind.slot() == slot)
    }

    /// Hang a decoration on the house this companion keeps, in the slot it belongs to, replacing
    /// whatever was there; or with `None`, take down whatever is in `slot`. Refused for a
    /// decoration the village has not got yet, or one that does not belong in `slot`.
    pub fn set_decoration(
        &mut self,
        keeper: CreatureId,
        slot: DecorationSlot,
        kind: Option<ShelterDecorationKind>,
    ) -> bool {
        if let Some(kind) = kind
            && (kind.slot() != slot || !self.unlocks.decorations.contains(&kind))
        {
            return false;
        }
        let index = match self.dressing.iter().position(|d| d.keeper == keeper) {
            Some(index) => index,
            None => {
                self.dressing.push(HouseDressing {
                    keeper,
                    decorations: Vec::new(),
                });
                self.dressing.len() - 1
            }
        };
        let decorations = &mut self.dressing[index].decorations;
        decorations.retain(|existing| existing.slot() != slot);
        if let Some(kind) = kind {
            decorations.push(kind);
        }
        self.normalize_village();
        true
    }

    /// The decorations every house in the village wears, in the order the houses stand.
    pub fn house_decoration_list(
        &self,
        creatures: &[Creature],
    ) -> [Vec<ShelterDecorationKind>; MAX_COLONY_CREATURES] {
        let mut lists: [Vec<ShelterDecorationKind>; MAX_COLONY_CREATURES] = Default::default();
        let owners = crate::house_owners(creatures, &self.cottage_order);
        for (slot, keeper) in owners.as_slice().iter().enumerate() {
            lists[slot] = self.decorations_of(*keeper).to_vec();
        }
        lists
    }

    /// The shelter as it is drawn: the colony's own, repainted in the palette chosen for the
    /// village if one was. Its style, size and details are never touched.
    pub fn drawn_shelter(&self) -> ShelterGenome {
        let mut shelter = self.shelter;
        if let Some(palette) = self.palette {
            (shelter.palette_index, shelter.accent_index) = palette.palettes();
        }
        shelter
    }

    /// Stand the cottages in a new order, given as the companions who keep them. The founder's
    /// colony house stays first whatever the order says, a cottage left out keeps its place after
    /// the ones given, and an order that is just the order everyone arrived in is not written
    /// down at all.
    pub fn arrange_cottages(&mut self, order: Vec<CreatureId>, creatures: &[Creature]) {
        self.cottage_order = order;
        self.normalize_village();
        let arranged = crate::house_owners(creatures, &self.cottage_order);
        self.cottage_order = if arranged == crate::house_owners(creatures, &[]) {
            Vec::new()
        } else {
            arranged.as_slice()[1..].to_vec()
        };
    }

    /// Plant a patch, move it, or with `None` dig it up. A fraction outside the ground is brought
    /// back onto it; one that is not a number is refused, and so is a garden the village has not
    /// got yet or one more than the ground holds. A new patch is planted `now` and starts as a
    /// sprout; moving one keeps it growing where it is.
    pub fn set_garden(
        &mut self,
        kind: GardenKind,
        along: Option<f32>,
        now: OffsetDateTime,
    ) -> bool {
        match along {
            Some(along) if !along.is_finite() => false,
            Some(along) => {
                let along = along.clamp(0.0, 1.0);
                match self.gardens.iter_mut().find(|patch| patch.kind == kind) {
                    Some(patch) => patch.along = along,
                    None => {
                        if !self.unlocks.gardens.contains(&kind)
                            || self.gardens.len() >= MAX_GARDENS
                        {
                            return false;
                        }
                        self.gardens.push(GardenPatch {
                            kind,
                            along,
                            planted_at_utc: Some(now),
                        });
                    }
                }
                self.normalize_village();
                true
            }
            None => {
                self.gardens.retain(|patch| patch.kind != kind);
                true
            }
        }
    }

    /// Set an ornament out, move it, or with `None` take it in again, on the same terms as a
    /// garden.
    pub fn set_ornament(&mut self, kind: OrnamentKind, along: Option<f32>) -> bool {
        match along {
            Some(along) if !along.is_finite() => false,
            Some(along) => {
                let along = along.clamp(0.0, 1.0);
                match self.ornaments.iter_mut().find(|spot| spot.kind == kind) {
                    Some(spot) => spot.along = along,
                    None => {
                        if !self.unlocks.ornaments.contains(&kind)
                            || self.ornaments.len() >= MAX_ORNAMENTS
                        {
                            return false;
                        }
                        self.ornaments.push(OrnamentSpot { kind, along });
                    }
                }
                self.normalize_village();
                true
            }
            None => {
                self.ornaments.retain(|spot| spot.kind != kind);
                true
            }
        }
    }

    /// Everything the person at the desk arranged about the village put back as it was
    /// generated: the cottages in the order their keepers arrived, the colony's own colours and
    /// house types, and nothing planted or set out on the ground. Hangout spots, decorations and
    /// the keepsakes in the trees are left as they were.
    pub fn reset_arrangement(&mut self) {
        self.cottage_order.clear();
        self.palette = None;
        self.gardens.clear();
        self.ornaments.clear();
        self.house_styles.clear();
    }

    /// One of each kind at most, each somewhere on the ground and each something the village
    /// has, and no more than the ground holds; one dressing per house, one decoration per slot.
    pub fn normalize_village(&mut self) {
        self.unlocks.normalize();
        self.normalize_hangouts();
        let mut seen = Vec::new();
        let gardens = &self.unlocks.gardens;
        self.gardens.retain(|patch| {
            let fresh = patch.along.is_finite()
                && !seen.contains(&patch.kind)
                && gardens.contains(&patch.kind);
            seen.push(patch.kind);
            fresh
        });
        for patch in &mut self.gardens {
            patch.along = patch.along.clamp(0.0, 1.0);
        }
        self.gardens.sort_by_key(|patch| patch.kind.index());
        self.gardens.truncate(MAX_GARDENS);
        let mut seen = Vec::new();
        let ornaments = &self.unlocks.ornaments;
        self.ornaments.retain(|spot| {
            let fresh = spot.along.is_finite()
                && !seen.contains(&spot.kind)
                && ornaments.contains(&spot.kind);
            seen.push(spot.kind);
            fresh
        });
        for spot in &mut self.ornaments {
            spot.along = spot.along.clamp(0.0, 1.0);
        }
        self.ornaments.sort_by_key(|spot| spot.kind.index());
        self.ornaments.truncate(MAX_ORNAMENTS);
        let mut seen = Vec::new();
        self.cottage_order.retain(|id| {
            let fresh = !seen.contains(id);
            seen.push(*id);
            fresh
        });
        self.cottage_order.truncate(MAX_COLONY_CREATURES);
        let mut seen = Vec::new();
        self.house_styles.retain(|choice| {
            let fresh = !seen.contains(&choice.keeper);
            seen.push(choice.keeper);
            fresh
        });
        self.house_styles.truncate(MAX_COLONY_CREATURES);
        let mut seen = Vec::new();
        let decorations = &self.unlocks.decorations;
        self.dressing.retain(|dressing| {
            let fresh = !seen.contains(&dressing.keeper);
            seen.push(dressing.keeper);
            fresh
        });
        for dressing in &mut self.dressing {
            let mut slots = Vec::with_capacity(MAX_HOUSE_DECORATIONS);
            dressing.decorations.retain(|kind| {
                let fresh = !slots.contains(&kind.slot()) && decorations.contains(kind);
                slots.push(kind.slot());
                fresh
            });
            dressing.decorations.sort_by_key(|kind| kind.slot());
        }
        self.dressing
            .retain(|dressing| !dressing.decorations.is_empty());
        self.dressing.truncate(MAX_COLONY_CREATURES);
        if let Some(chosen) = &mut self.tree_keepsakes {
            let mut seen = Vec::new();
            for hook in &mut chosen.hooks {
                if let Some(variant) = *hook
                    && (variant >= crate::TRINKET_VARIANTS || seen.contains(&variant))
                {
                    *hook = None;
                }
                if let Some(variant) = *hook {
                    seen.push(variant);
                }
            }
        }
    }

    pub fn is_active(&self) -> bool {
        self.active_since_utc.is_some()
    }

    /// The spot of this kind, if one has been put down.
    pub fn hangout(&self, kind: HangoutKind) -> Option<HangoutSpot> {
        self.hangouts.iter().copied().find(|spot| spot.kind == kind)
    }

    /// The patch of this kind, if one has been planted.
    pub fn garden(&self, kind: GardenKind) -> Option<GardenPatch> {
        self.gardens
            .iter()
            .copied()
            .find(|patch| patch.kind == kind)
    }

    /// The ornament of this kind, if one has been set out.
    pub fn ornament(&self, kind: OrnamentKind) -> Option<OrnamentSpot> {
        self.ornaments
            .iter()
            .copied()
            .find(|spot| spot.kind == kind)
    }

    /// Whether anything has been put down, planted or set out on the village ground.
    pub fn has_ground_items(&self) -> bool {
        !self.hangouts.is_empty() || !self.gardens.is_empty() || !self.ornaments.is_empty()
    }

    /// Put a spot down, move it, or with `None` pick it up again. A fraction outside the ground
    /// is brought back onto it; one that is not a number is refused, and so is a spot the village
    /// has not got yet or one more than the ground holds.
    pub fn set_hangout(&mut self, kind: HangoutKind, along: Option<f32>) -> bool {
        match along {
            Some(along) if !along.is_finite() => false,
            Some(along) => {
                let along = along.clamp(0.0, 1.0);
                match self.hangouts.iter_mut().find(|spot| spot.kind == kind) {
                    Some(spot) => spot.along = along,
                    None => {
                        if !self.unlocks.hangouts.contains(&kind)
                            || self.hangouts.len() >= MAX_HANGOUTS
                        {
                            return false;
                        }
                        self.hangouts.push(HangoutSpot { kind, along });
                    }
                }
                self.normalize_hangouts();
                true
            }
            None => {
                self.hangouts.retain(|spot| spot.kind != kind);
                true
            }
        }
    }

    /// One spot of each kind at most, each somewhere on the ground, in a stable order.
    pub fn normalize_hangouts(&mut self) {
        let mut seen = Vec::new();
        let hangouts = &self.unlocks.hangouts;
        self.hangouts.retain(|spot| {
            let fresh = spot.along.is_finite()
                && !seen.contains(&spot.kind)
                && hangouts.contains(&spot.kind);
            seen.push(spot.kind);
            fresh
        });
        for spot in &mut self.hangouts {
            spot.along = spot.along.clamp(0.0, 1.0);
        }
        self.hangouts.sort_by_key(|spot| spot.kind.index());
        self.hangouts.truncate(MAX_HANGOUTS);
    }

    /// Hang the keepsakes in the trees by hand: a variant or nothing for each hook, or with
    /// `None` let the trees fill themselves again.
    pub fn set_tree_keepsakes(&mut self, hooks: Option<[Option<u8>; TREE_HOOKS]>) {
        self.tree_keepsakes = hooks.map(|hooks| TreeKeepsakes { hooks });
        self.normalize_village();
    }
}

impl Default for ColonyHome {
    fn default() -> Self {
        Self::from_seed([0; 32], None, None, None)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum HabitatZoneKind {
    #[default]
    Allowed,
    Excluded,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HabitatZone {
    pub id: u64,
    pub display: DisplayKey,
    pub normalized_bounds: DesktopRect,
    pub kind: HabitatZoneKind,
    pub enabled: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum HabitatPreset {
    #[default]
    EntireDesktop,
    PrimaryDisplay,
    BottomEdge,
    BottomCorners,
    LowerHalf,
    Custom,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct HabitatPolicy {
    pub preset: HabitatPreset,
    pub zones: Vec<HabitatZone>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplicationOcclusionRule {
    pub application: ApplicationKey,
    pub display_name: String,
    pub enabled: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub visible: bool,
    pub paused: bool,
    pub display_scale: u8,
    pub window_ledges: bool,
    pub cursor_reactions: bool,
    pub reduce_motion: bool,
    pub launch_at_login: bool,
    pub direct_manipulation: bool,
    pub fullscreen_app_occlusion: bool,
    pub habitat: HabitatPolicy,
    pub application_occlusion_rules: Vec<ApplicationOcclusionRule>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            visible: true,
            paused: false,
            display_scale: 3,
            window_ledges: true,
            cursor_reactions: true,
            reduce_motion: false,
            launch_at_login: false,
            direct_manipulation: true,
            fullscreen_app_occlusion: true,
            habitat: HabitatPolicy::default(),
            application_occlusion_rules: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SaveFile {
    #[serde(default)]
    pub companion: crate::CompanionState,
    pub save_version: u32,
    pub colony_seed: [u8; 32],
    #[serde(with = "time::serde::rfc3339")]
    pub created_at_utc: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub maximum_seen_utc: OffsetDateTime,
    pub arrival_state: ArrivalState,
    #[serde(default)]
    pub home: ColonyHome,
    pub settings: Settings,
    pub creatures: Vec<Creature>,
    #[serde(default)]
    pub relationships: Vec<CreatureRelationship>,
    #[serde(default)]
    pub ritual: RitualState,
    #[serde(default)]
    pub objects: ColonyObjectState,
    #[serde(default)]
    pub visitors: crate::VisitorState,
}

/// What the person at the desk is holding out. Runtime-only: an offer is a moment, not a record.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OfferKind {
    Snack,
    Toy,
}

#[derive(Clone, Debug, PartialEq)]
pub enum WorldEvent {
    CreatureSpawned {
        creature_id: CreatureId,
    },
    ActionStarted {
        creature_id: CreatureId,
        action: ActionKind,
    },
    ActionCompleted {
        creature_id: CreatureId,
        action: ActionKind,
    },
    SurfaceChanged {
        creature_id: CreatureId,
        kind: SurfaceKind,
    },
    CursorReaction {
        creature_id: CreatureId,
        action: ActionKind,
    },
    WindowReaction {
        creature_id: CreatureId,
        action: ActionKind,
    },
    BondInteraction {
        a: CreatureId,
        b: CreatureId,
        experience: RelationshipExperience,
    },
    CreatureSlept {
        creature_id: CreatureId,
    },
    CreatureWoke {
        creature_id: CreatureId,
    },
    CreatureRested {
        creature_id: CreatureId,
        uninterrupted_seconds: u32,
    },
    SleepInterrupted {
        creature_id: CreatureId,
        elapsed_seconds: u32,
    },
    CreaturePetted {
        creature_id: CreatureId,
    },
    CreaturePlaced {
        creature_id: CreatureId,
        display: DisplayKey,
        region: u8,
    },
    ObservationElapsed {
        creature_id: CreatureId,
        display: DisplayKey,
        region: u8,
        on_ledge: bool,
        riding_window: bool,
        nearby_creature: Option<CreatureId>,
        active_seconds: u8,
    },
    ProfileChanged {
        creature_id: CreatureId,
        new_descriptor: Option<ProfileDescriptor>,
        show_milestone: bool,
    },
    DragStarted {
        creature_id: CreatureId,
    },
    DragEnded {
        creature_id: CreatureId,
        outcome: DragReleaseKind,
    },
    TossLanded {
        creature_id: CreatureId,
        surface: SurfaceKind,
        bounced: bool,
    },
    /// Something was held out to one creature and it gave its answer. Being asked kindly is a
    /// good thing to have happen whichever way the answer went; the meal or the game that may
    /// follow is counted by its own completed action, not by this.
    OfferAnswered {
        creature_id: CreatureId,
        kind: OfferKind,
        accepted: bool,
    },
    HomeAppeared,
    HomeDisappeared {
        interrupted: bool,
    },
    RitualStarted {
        kind: RitualKind,
    },
    RitualCompleted {
        kind: RitualKind,
    },
    RitualInterrupted {
        kind: RitualKind,
    },
    ColonyObjectAdded {
        object_id: u64,
        kind: ColonyObjectKind,
    },
    /// Something new for the village to choose from: a decoration, a hangout spot, a garden or
    /// an ornament.
    VillageUnlocked {
        item: VillageItem,
    },
    /// A companion picked up a little habit of its own.
    HabitLearned {
        creature_id: CreatureId,
        habit: crate::Habit,
    },
}

impl WorldEvent {
    /// How soon this event needs the colony written to disk. Arrivals, the houses coming and going,
    /// rituals, new belongings and a newly picked-up habit are kept at once; a creature starting an
    /// action or stepping onto another surface is everyday movement, gathered into the next
    /// routine checkpoint.
    pub fn save_urgency(&self) -> crate::SaveUrgency {
        match self {
            Self::CreatureSpawned { .. }
            | Self::HomeAppeared
            | Self::HomeDisappeared { .. }
            | Self::RitualStarted { .. }
            | Self::RitualInterrupted { .. }
            | Self::ColonyObjectAdded { .. }
            | Self::VillageUnlocked { .. }
            | Self::HabitLearned { .. } => crate::SaveUrgency::Prompt,
            Self::ActionStarted { .. } | Self::SurfaceChanged { .. } => crate::SaveUrgency::Routine,
            _ => crate::SaveUrgency::None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WorldCommand {
    BeginInteraction {
        creature_id: CreatureId,
        cursor: Point,
    },
    UpdateInteraction {
        cursor: Point,
        velocity: Point,
    },
    EndInteraction {
        cursor: Point,
        velocity: Point,
    },
    CancelInteraction,
    GatherCreatures,
    /// Hold out a snack. The creature decides for itself whether it wants one.
    OfferSnack {
        creature_id: CreatureId,
    },
    /// Hold out a toy. The creature decides for itself whether it feels like playing.
    OfferToy {
        creature_id: CreatureId,
    },
    /// Call the whole colony home now, exactly as the ordinary home visit would.
    SendHome,
    /// Ask the village to share a moment while the houses are out. The companion the menu was
    /// opened on is the one asking; everyone who is home decides for themselves.
    InviteVillageMoment {
        creature_id: CreatureId,
        moment: VillageMoment,
    },
    /// Bring the moment under way to an end.
    StopVillageMoment,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DragReleaseKind {
    Placed(SurfaceKind),
    Tossed { velocity: Point },
}

mod duration_millis {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::time::Duration;

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u64(duration.as_millis().min(u64::MAX as u128) as u64)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Duration::from_millis(u64::deserialize(deserializer)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compact_lived_experience_state_stays_within_budget() {
        assert!(std::mem::size_of::<CreatureMemory>() < 192);
        assert!(std::mem::size_of::<LearnedTendencies>() <= 8);
        let memory = CreatureMemory {
            times_petted: u32::MAX,
            times_tossed: u32::MAX,
            placements: u32::MAX,
            sleep_interruptions: u32::MAX,
            window_climbs: u32::MAX,
            discoveries_found: u32::MAX,
            play_sessions: u32::MAX,
            home_visits: u32::MAX,
            ledge_seconds: u32::MAX,
            window_ride_seconds: u32::MAX,
            longest_sleep_seconds: u32::MAX,
            favorite_display: Some(FavoriteDisplayMemory {
                display: DisplayKey([u8::MAX; 16]),
                confidence: u8::MAX,
            }),
            preferred_region: Some(PreferredRegionMemory {
                display: DisplayKey([u8::MAX; 16]),
                cell: 8,
                confidence: u8::MAX,
            }),
            descriptor_flags: u16::MAX,
            profile_revision: u16::MAX,
            viewed_profile_revision: u16::MAX,
            milestone_cooldown_active_seconds: u32::MAX,
            milestone_bubble_shown: true,
            habits: vec![crate::Habit::CirclesBeforeNaps, crate::Habit::LooksFoodOver],
        };
        let routines = RoutineTable {
            slots: [RoutineSlot {
                key: u16::MAX,
                strength: u8::MAX,
            }; MAX_ROUTINES],
            len: MAX_ROUTINES as u8,
        };
        let payload = serde_json::to_vec(&(
            CreatureOrigin::default(),
            "Mallow the Magnificent",
            memory,
            LearnedTendencies {
                cursor_trust: 100,
                sociability: 100,
                climbing: 100,
                sleep_security: 100,
                exploration: 100,
                play: 100,
                home_affinity: 100,
                routine: 100,
            },
            routines,
        ))
        .unwrap();
        assert!(
            payload.len() < 2 * 1024,
            "payload used {} bytes",
            payload.len()
        );
    }

    #[test]
    fn ritual_persistence_contains_only_the_bounded_schedule_projection() {
        let value = serde_json::to_value(RitualState {
            next_at_utc: OffsetDateTime::UNIX_EPOCH,
            last_kind: Some(RitualKind::Picnic),
            ordinal: 17,
            hatch_day_acknowledged_year: Some(2026),
        })
        .unwrap();
        let fields = value.as_object().unwrap();
        assert_eq!(fields.len(), 4);
        assert!(fields.contains_key("next_at_utc"));
        assert!(fields.contains_key("last_kind"));
        assert!(fields.contains_key("ordinal"));
        assert!(fields.contains_key("hatch_day_acknowledged_year"));
        assert_eq!(RitualKind::ALL.len(), 10);
    }

    #[test]
    fn colony_object_projection_is_typed_and_bounded() {
        assert_eq!(ColonyObjectKind::ALL.len(), 20);
        for (index, kind) in ColonyObjectKind::ALL.into_iter().enumerate() {
            assert_eq!(usize::from(kind.index()), index);
        }
        assert_eq!(MAX_COLONY_OBJECTS, 8);
        assert_eq!(
            ColonyObjectKind::Pillow.default_role(),
            ColonyObjectRole::Sleep
        );
        assert_eq!(ColonyObjectKind::Toy.default_role(), ColonyObjectRole::Play);
        assert_eq!(
            ColonyObjectKind::Cup.default_role(),
            ColonyObjectRole::Social
        );
        let object = ColonyObject {
            id: 7,
            kind: ColonyObjectKind::Plant,
            display: DisplayKey([3; 16]),
            normalized_position: Point { x: 0.4, y: 0.8 },
            role: ColonyObjectRole::Comfort,
        };
        let value = serde_json::to_value(object).unwrap();
        assert_eq!(value.as_object().unwrap().len(), 5);
    }

    #[test]
    fn every_decoration_belongs_to_one_slot_and_every_slot_has_five_to_choose_from() {
        for (index, kind) in ShelterDecorationKind::ALL.into_iter().enumerate() {
            assert_eq!(kind.index(), index);
        }
        // The six a colony could earn before keep their names in the file.
        for (kind, name) in [
            (ShelterDecorationKind::Leaf, "Leaf"),
            (ShelterDecorationKind::Banner, "Banner"),
            (ShelterDecorationKind::Stone, "Stone"),
            (ShelterDecorationKind::Flower, "Flower"),
            (ShelterDecorationKind::Lamp, "Lamp"),
            (ShelterDecorationKind::RoofOrnament, "RoofOrnament"),
        ] {
            assert_eq!(serde_json::to_value(kind).unwrap(), name);
        }
        // Each of the six earned before hangs in a slot of its own, so a colony house that wore
        // all six still can.
        let earned: std::collections::BTreeSet<_> = ShelterDecorationKind::ALL[..6]
            .iter()
            .map(|kind| kind.slot())
            .collect();
        assert_eq!(earned.len(), DecorationSlot::ALL.len());
        for slot in DecorationSlot::ALL {
            let choices = ShelterDecorationKind::ALL
                .into_iter()
                .filter(|kind| kind.slot() == slot)
                .count();
            assert_eq!(choices, 5, "{slot:?}");
        }
        let labels: std::collections::BTreeSet<_> = ShelterDecorationKind::ALL
            .iter()
            .map(|k| k.label())
            .collect();
        assert_eq!(labels.len(), ShelterDecorationKind::ALL.len());
    }

    #[test]
    fn a_house_wears_one_decoration_per_slot_and_only_what_the_village_has() {
        let mut home = ColonyHome::default();
        let keeper = 7;
        assert!(home.set_decoration(
            keeper,
            DecorationSlot::Eaves,
            Some(ShelterDecorationKind::Banner)
        ));
        // Not yet the village's to hang.
        assert!(!home.set_decoration(
            keeper,
            DecorationSlot::Eaves,
            Some(ShelterDecorationKind::FairyLights)
        ));
        // In the wrong slot.
        assert!(!home.set_decoration(
            keeper,
            DecorationSlot::Roof,
            Some(ShelterDecorationKind::Banner)
        ));
        home.unlocks
            .grant(VillageItem::Decoration(ShelterDecorationKind::FairyLights));
        assert!(home.set_decoration(
            keeper,
            DecorationSlot::Eaves,
            Some(ShelterDecorationKind::FairyLights)
        ));
        assert!(home.set_decoration(
            keeper,
            DecorationSlot::WallRight,
            Some(ShelterDecorationKind::Lamp)
        ));
        assert_eq!(
            home.decorations_of(keeper),
            &[
                ShelterDecorationKind::FairyLights,
                ShelterDecorationKind::Lamp
            ]
        );
        assert!(home.set_decoration(keeper, DecorationSlot::Eaves, None));
        assert_eq!(home.decorations_of(keeper), &[ShelterDecorationKind::Lamp]);
        assert!(home.set_decoration(keeper, DecorationSlot::WallRight, None));
        assert!(home.dressing.is_empty(), "a bare house has no entry at all");
    }

    #[test]
    fn a_village_starts_with_three_of_everything_and_grows_one_at_a_time() {
        let mut unlocks = VillageUnlocks::starting();
        assert_eq!(unlocks.decorations.len(), 3);
        assert_eq!(unlocks.hangouts.len(), 3);
        assert_eq!(unlocks.gardens.len(), 3);
        assert_eq!(unlocks.ornaments.len(), 3);
        let total = VillageItem::all().count();
        assert_eq!(total, 30 + 15 + 12 + 15);
        assert_eq!(unlocks.remaining().count(), total - 12);
        let next = unlocks.remaining().next().unwrap();
        assert!(unlocks.grant(next));
        assert!(!unlocks.grant(next), "granted once");
        assert_eq!(unlocks.remaining().count(), total - 13);
        // Normalising tops a category back up and drops a repeat.
        unlocks.hangouts = vec![HangoutKind::Swing, HangoutKind::Swing];
        unlocks.normalize();
        assert_eq!(
            unlocks.hangouts,
            vec![
                HangoutKind::Swing,
                HangoutKind::Cushion,
                HangoutKind::Blanket,
                HangoutKind::Lookout
            ]
        );
    }

    #[test]
    fn a_garden_grows_round_its_stages_by_itself() {
        let planted = OffsetDateTime::UNIX_EPOCH + time::Duration::days(20_000);
        let patch = GardenPatch {
            kind: GardenKind::Herbs,
            along: 0.5,
            planted_at_utc: Some(planted),
        };
        let hours = GardenKind::Herbs.stage_hours();
        let at = |h: i64| planted + time::Duration::hours(h);
        assert_eq!(patch.stage(planted), GardenStage::Sprout);
        assert_eq!(patch.stage(at(hours)), GardenStage::Growing);
        assert_eq!(patch.stage(at(hours * 2)), GardenStage::Grown);
        assert_eq!(patch.stage(at(hours * 3)), GardenStage::Bounty);
        assert_eq!(patch.stage(at(hours * 4)), GardenStage::Sprout);
        // A clock set back never un-plants it.
        assert_eq!(patch.stage(at(-50)), GardenStage::Sprout);
        // A patch from before gardens grew is somewhere round its cycle, and moves on with time.
        let old = GardenPatch {
            planted_at_utc: None,
            ..patch
        };
        let seen: std::collections::BTreeSet<_> =
            (0..4).map(|step| old.stage(at(step * hours))).collect();
        assert_eq!(seen.len(), 4);
    }

    #[test]
    fn the_trees_fill_themselves_until_chosen_and_the_first_sixteen_keep_their_hooks() {
        let record = |variant: u8, day: i64| crate::ScrapbookRecord {
            variant,
            first_at: OffsetDateTime::UNIX_EPOCH + time::Duration::days(day),
            finder: None,
            finder_name: String::new(),
        };
        // An older colony's finds hang where they always did.
        let old = [record(3, 1), record(12, 2), record(0, 3)];
        let hooks = hung_keepsakes(None, &old);
        assert_eq!(hooks[3], Some(3));
        assert_eq!(hooks[12], Some(12));
        assert_eq!(hooks[0], Some(0));
        assert_eq!(hooks.iter().flatten().count(), 3);
        // Later finds fill the empty hooks in the order they were found.
        let mut grown = old.to_vec();
        grown.extend([record(40, 9), record(20, 5), record(150, 7)]);
        let hooks = hung_keepsakes(None, &grown);
        assert_eq!(hooks[1], Some(20));
        assert_eq!(hooks[2], Some(150));
        assert_eq!(hooks[4], Some(40));
        // Chosen by hand: exactly the choice, less anything never found.
        let mut chosen = [None; TREE_HOOKS];
        chosen[5] = Some(40);
        chosen[6] = Some(99);
        let hooks = hung_keepsakes(Some(&TreeKeepsakes { hooks: chosen }), &grown);
        assert_eq!(hooks[5], Some(40));
        assert_eq!(hooks.iter().flatten().count(), 1);
        // A repeat or a variant past the catalogue is dropped when the village is tidied.
        let mut home = ColonyHome::default();
        let mut hooks = [None; TREE_HOOKS];
        hooks[0] = Some(2);
        hooks[1] = Some(2);
        hooks[2] = Some(250);
        home.set_tree_keepsakes(Some(hooks));
        assert_eq!(home.tree_keepsakes.unwrap().hung(), 1);
    }

    #[test]
    fn routine_table_is_bounded_and_keeps_the_strongest_legacy_entries() {
        let entries = (0..24).map(|key| (key, f32::from(key) / 24.0)).collect();
        let table = RoutineTable::from_ranked(entries);
        assert_eq!(table.len, MAX_ROUTINES as u8);
        assert!(
            table.slots[..MAX_ROUTINES]
                .iter()
                .all(|slot| slot.key >= 12)
        );
    }

    #[test]
    fn squeeze_action_appends_without_renumbering_persisted_routine_codes() {
        assert_eq!(ActionKind::Idle.routine_code(), 0);
        assert_eq!(ActionKind::Traverse.routine_code(), 1);
        assert_eq!(ActionKind::PetReaction.routine_code(), 23);
        assert_eq!(ActionKind::SqueezeWindow.routine_code(), 24);
    }

    #[test]
    fn learned_tendencies_saturate_and_descriptors_use_hysteresis() {
        let mut tendencies = LearnedTendencies::default();
        LearnedTendencies::adjust(&mut tendencies.climbing, 120);
        assert_eq!(tendencies.climbing, 100);
        let mut memory = CreatureMemory::default();
        assert!(update_descriptor_flags(&mut memory, tendencies));
        let high_places = ProfileDescriptor::LovesHighPlaces.flag();
        assert_ne!(memory.descriptor_flags & high_places, 0);
        tendencies.climbing = 30;
        assert!(!update_descriptor_flags(&mut memory, tendencies));
        tendencies.climbing = 24;
        assert!(update_descriptor_flags(&mut memory, tendencies));
        assert_eq!(memory.descriptor_flags & high_places, 0);
    }

    #[test]
    fn creature_names_are_trimmed_unicode_and_reject_controls() {
        assert_eq!(validate_creature_name("  Möchi  ").unwrap(), "Möchi");
        assert_eq!(
            validate_creature_name("\nPip"),
            Err(CreatureNameError::ControlCharacter)
        );
        assert_eq!(validate_creature_name("   "), Err(CreatureNameError::Empty));
        assert_eq!(
            validate_creature_name("abcdefghijklmnopqrstuvwxyz"),
            Err(CreatureNameError::TooLong)
        );
    }

    #[test]
    fn default_names_avoid_initial_duplicates() {
        let first = default_creature_name([7; 32], 0, &[]);
        let second = default_creature_name([7; 32], 1, std::slice::from_ref(&first));
        assert_ne!(first, second);
    }

    #[test]
    fn relationship_scores_are_exactly_four_bytes_and_saturate() {
        assert_eq!(
            std::mem::size_of::<u8>() * 4,
            std::mem::size_of_val(&[
                CreatureRelationship::default().affinity,
                CreatureRelationship::default().familiarity,
                CreatureRelationship::default().playfulness,
                CreatureRelationship::default().avoidance,
            ])
        );
        let mut relationship = CreatureRelationship::new(9, 3).unwrap();
        assert_eq!((relationship.a, relationship.b), (3, 9));
        for _ in 0..300 {
            relationship.apply(RelationshipExperience::PositivePlay);
        }
        assert_eq!(relationship.affinity, u8::MAX);
        assert_eq!(relationship.familiarity, u8::MAX);
        assert_eq!(relationship.playfulness, u8::MAX);
        assert_eq!(relationship.avoidance, 0);
        for _ in 0..300 {
            relationship.apply(RelationshipExperience::Squabble);
        }
        assert_eq!(relationship.affinity, 0);
        assert_eq!(relationship.avoidance, u8::MAX);
    }

    #[test]
    fn closest_companion_uses_canonical_shared_records() {
        let relationships = [
            CreatureRelationship {
                a: 1,
                b: 2,
                affinity: 90,
                familiarity: 20,
                playfulness: 0,
                avoidance: 0,
            },
            CreatureRelationship {
                a: 1,
                b: 3,
                affinity: 120,
                familiarity: 80,
                playfulness: 0,
                avoidance: 100,
            },
        ];
        assert_eq!(closest_companion(&relationships, 1), Some(2));
        assert_eq!(closest_companion(&relationships, 2), Some(1));
    }
}
