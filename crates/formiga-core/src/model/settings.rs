//! The person's settings, and the habitat they keep the colony to.

use super::{ApplicationKey, DesktopRect, DisplayKey};
use serde::{Deserialize, Serialize};

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
