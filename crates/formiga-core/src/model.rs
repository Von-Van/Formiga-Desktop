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
}

impl ActionKind {
    pub const ALL: [Self; 18] = [
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
    pub habits: BTreeMap<String, f32>,
    pub relationships: BTreeMap<CreatureId, f32>,
    pub cursor_cooldown: f32,
    /// Runtime-visible countdown used to stage several earned arrivals after a long absence.
    /// It is persisted so quitting during the reveal sequence cannot skip or duplicate a mini.
    #[serde(default)]
    pub arrival_delay_secs: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Creature {
    pub id: CreatureId,
    pub generation: u8,
    pub display_scale_percent: u8,
    pub appearance: AppearanceGenome,
    pub personality: PersonalityGenome,
    pub behavior_seed: [u8; 32],
    pub state: CreatureState,
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
    DragStarted {
        creature_id: CreatureId,
    },
    DragEnded {
        creature_id: CreatureId,
        surface: SurfaceKind,
    },
    HomeAppeared,
    HomeDisappeared {
        interrupted: bool,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WorldCommand {
    BeginDrag {
        creature_id: CreatureId,
        cursor: Point,
    },
    UpdateDrag {
        cursor: Point,
    },
    EndDrag {
        cursor: Point,
    },
    CancelDrag,
    GatherCreatures,
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
