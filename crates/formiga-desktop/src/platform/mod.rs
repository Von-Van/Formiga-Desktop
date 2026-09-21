#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

/// How far the system's own bars — the menu bar, the Dock, the taskbar — reach into a display from
/// each edge, in points.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Insets {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

#[cfg(target_os = "macos")]
pub use macos::{
    BOTTOM_RESERVED, OVERLAY_HALF_RESOLUTION, begin_interaction_capture, browse_application,
    canonical_monitor_bounds, configure_interaction_proxy, configure_native_overlay,
    cursor_and_idle, display_key, end_interaction_capture, launch_update, left_button_down,
    normalize_cursor, normalize_windows, open_directory, raise_above_overlays,
    secondary_click_modifier, set_interaction_hittest, set_interaction_shape, set_launch_at_login,
    set_menu_proxy_shape, set_overlay_hittest, use_nearest_overlay_filter, visible_windows,
    work_area_insets,
};
#[cfg(target_os = "windows")]
pub use windows::{
    BOTTOM_RESERVED, OVERLAY_HALF_RESOLUTION, begin_interaction_capture, browse_application,
    canonical_monitor_bounds, configure_interaction_proxy, configure_native_overlay,
    cursor_and_idle, display_key, end_interaction_capture, launch_update, left_button_down,
    normalize_cursor, normalize_windows, open_directory, raise_above_overlays,
    secondary_click_modifier, set_interaction_hittest, set_interaction_shape, set_launch_at_login,
    set_menu_proxy_shape, set_overlay_hittest, use_nearest_overlay_filter, visible_windows,
    work_area_insets,
};

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
compile_error!("Formiga v0.1 supports macOS and Windows only");
