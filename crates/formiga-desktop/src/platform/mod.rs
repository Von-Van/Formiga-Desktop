#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "macos")]
pub use macos::{
    canonical_monitor_bounds, configure_native_overlay, cursor_and_idle, normalize_cursor,
    normalize_windows, open_directory, set_launch_at_login, visible_windows,
};
#[cfg(target_os = "windows")]
pub use windows::{
    canonical_monitor_bounds, configure_native_overlay, cursor_and_idle, normalize_cursor,
    normalize_windows, open_directory, set_launch_at_login, visible_windows,
};

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
compile_error!("Formiga v0.1 supports macOS and Windows only");
