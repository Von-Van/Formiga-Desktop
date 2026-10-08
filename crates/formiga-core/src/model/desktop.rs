//! Identifiers, rectangles and the snapshot of the desktop the world is handed each tick.

use serde::{Deserialize, Serialize};
use std::time::Duration;

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

    pub(crate) fn overlaps(self, other: Self) -> bool {
        self.x < other.right()
            && self.right() > other.x
            && self.y < other.bottom()
            && self.bottom() > other.y
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
