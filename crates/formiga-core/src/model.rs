use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
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
    pub const ALL: [Self; 24] = [
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
    pub position: Point,
    pub velocity: Point,
    pub facing_right: bool,
    pub action: ActionKind,
    pub action_elapsed: f32,
    pub action_duration: f32,
    pub drives: Drives,
    pub surface: SurfaceAttachment,
    pub relationships: BTreeMap<CreatureId, f32>,
    pub cursor_cooldown: f32,
    /// Runtime presentation selection for generated activity art. It is defaulted for v4 saves
    /// and reset with interrupted actions, so it does not form a discovery collection.
    #[serde(default)]
    pub activity_variant: u8,
    /// Runtime-visible countdown used to stage several earned arrivals after a long absence.
    /// It is persisted so quitting during the reveal sequence cannot skip or duplicate a mini.
    #[serde(default)]
    pub arrival_delay_secs: f32,
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
    pub source_colony_seed: [u8; 32],
    pub source_generation: u8,
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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Creature {
    pub id: CreatureId,
    pub generation: u8,
    pub origin: CreatureOrigin,
    pub colony_order: u8,
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
}

fn default_born_at_utc() -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ArrivalState {
    pub arrived: [bool; 3],
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum HomeCorner {
    #[default]
    BottomLeft,
    BottomRight,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShelterStyle {
    #[default]
    LeafTent,
    MushroomHut,
    CushionDen,
    PaperHouse,
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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ColonyHome {
    pub display: Option<DisplayKey>,
    pub corner: HomeCorner,
    pub shelter: ShelterGenome,
    #[serde(with = "time::serde::rfc3339::option")]
    pub active_since_utc: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub last_disappeared_utc: Option<OffsetDateTime>,
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
                style: match seed[1] % 4 {
                    0 => ShelterStyle::LeafTent,
                    1 => ShelterStyle::MushroomHut,
                    2 => ShelterStyle::CushionDen,
                    _ => ShelterStyle::PaperHouse,
                },
                palette_index: seed[2] % 12,
                accent_index: seed[3] % 12,
                width: 34 + seed[4] % 9,
                height: 27 + seed[5] % 10,
                detail_seed,
            },
            active_since_utc,
            last_disappeared_utc,
        }
    }

    pub fn is_active(&self) -> bool {
        self.active_since_utc.is_some()
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
    SocialInteraction {
        a: CreatureId,
        b: CreatureId,
        action: ActionKind,
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
    HomeAppeared,
    HomeDisappeared {
        interrupted: bool,
    },
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
}
