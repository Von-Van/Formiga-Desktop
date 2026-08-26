use core_foundation::base::{CFType, TCFType};
use core_foundation::dictionary::CFDictionary;
use core_foundation::number::CFNumber;
use core_foundation::string::CFString;
use core_graphics::display::{
    CGDisplay, kCGNullWindowID, kCGWindowListExcludeDesktopElements,
    kCGWindowListOptionOnScreenOnly,
};
use core_graphics::event::CGEvent;
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use core_graphics::geometry::CGRect;
use formiga_core::{CursorSnapshot, DesktopRect, DesktopWindow, MonitorInfo, Point};
use objc2_app_kit::{NSView, NSWindowCollectionBehavior};
use std::fs;
use std::process::Command;
use std::time::Duration;
use winit::platform::macos::WindowExtMacOS;
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::Window;

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGEventSourceSecondsSinceLastEventType(
        state_id: CGEventSourceStateID,
        event_type: u32,
    ) -> f64;
}

pub fn configure_native_overlay(window: &Window) {
    let _ = window.set_cursor_hittest(false);
    #[allow(deprecated)]
    window.set_has_shadow(false);
    let Ok(handle) = window.window_handle() else {
        return;
    };
    let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
        return;
    };
    let view = unsafe { handle.ns_view.cast::<NSView>().as_ref() };
    if let Some(ns_window) = view.window() {
        ns_window.setIgnoresMouseEvents(true);
        ns_window.setOpaque(false);
        ns_window.setHasShadow(false);
        let behavior = unsafe { ns_window.collectionBehavior() }
            | NSWindowCollectionBehavior::CanJoinAllSpaces
            | NSWindowCollectionBehavior::Stationary
            | NSWindowCollectionBehavior::IgnoresCycle
            | NSWindowCollectionBehavior::FullScreenAuxiliary;
        unsafe { ns_window.setCollectionBehavior(behavior) };
    }
}

pub fn canonical_monitor_bounds(
    physical_x: i32,
    physical_y: i32,
    physical_width: u32,
    physical_height: u32,
    scale: f32,
) -> DesktopRect {
    DesktopRect {
        x: physical_x as f32 / scale,
        y: physical_y as f32 / scale,
        width: physical_width as f32 / scale,
        height: physical_height as f32 / scale,
    }
}

// Quartz cursor and window APIs already use the same global point space as the simulation.
pub fn normalize_cursor(_cursor: &mut CursorSnapshot, _monitors: &[MonitorInfo]) {}

pub fn normalize_windows(_windows: &mut [DesktopWindow], _monitors: &[MonitorInfo]) {}

pub fn cursor_and_idle(
    previous: Option<(Point, std::time::Instant)>,
) -> (CursorSnapshot, Duration) {
    let position = CGEventSource::new(CGEventSourceStateID::CombinedSessionState)
        .and_then(CGEvent::new)
        .map(|event| {
            let point = event.location();
            Point {
                x: point.x as f32,
                y: point.y as f32,
            }
        })
        .ok();
    let now = std::time::Instant::now();
    let velocity = match (position, previous) {
        (Some(current), Some((old, instant))) => {
            let seconds = now.duration_since(instant).as_secs_f32().max(0.001);
            Point {
                x: (current.x - old.x) / seconds,
                y: (current.y - old.y) / seconds,
            }
        }
        _ => Point::default(),
    };
    let idle_seconds = unsafe {
        CGEventSourceSecondsSinceLastEventType(CGEventSourceStateID::CombinedSessionState, u32::MAX)
    };
    let idle = if idle_seconds.is_finite() && idle_seconds >= 0.0 {
        Duration::from_secs_f64(idle_seconds)
    } else {
        Duration::ZERO
    };
    (
        CursorSnapshot {
            position: position.unwrap_or_default(),
            velocity,
            available: position.is_some(),
        },
        idle,
    )
}

pub fn visible_windows() -> Vec<DesktopWindow> {
    let options = kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements;
    let Some(array) = CGDisplay::window_list_info(options, Some(kCGNullWindowID)) else {
        return Vec::new();
    };
    let mut windows = Vec::new();
    for (z_order, raw) in array.iter().enumerate() {
        let raw = *raw;
        let dictionary =
            unsafe { CFDictionary::<CFString, CFType>::wrap_under_get_rule(raw.cast_mut().cast()) };
        let layer = number(&dictionary, unsafe {
            core_graphics::window::kCGWindowLayer
        })
        .unwrap_or(1);
        let key = number(&dictionary, unsafe {
            core_graphics::window::kCGWindowNumber
        })
        .unwrap_or(0);
        if layer != 0 || key <= 0 {
            continue;
        }
        let bounds_key =
            unsafe { CFString::wrap_under_get_rule(core_graphics::window::kCGWindowBounds) };
        let Some(bounds_value) = dictionary.find(&bounds_key) else {
            continue;
        };
        let Some(bounds_dictionary) = bounds_value.downcast::<CFDictionary>() else {
            continue;
        };
        let Some(bounds) = CGRect::from_dict_representation(&bounds_dictionary) else {
            continue;
        };
        if bounds.size.width < 120.0 || bounds.size.height < 80.0 {
            continue;
        }
        windows.push(DesktopWindow {
            key: key as u64,
            bounds: DesktopRect {
                x: bounds.origin.x as f32,
                y: bounds.origin.y as f32,
                width: bounds.size.width as f32,
                height: bounds.size.height as f32,
            },
            z_order: z_order as u32,
            visible: true,
            minimized: false,
        });
    }
    windows
}

fn number(
    dictionary: &CFDictionary<CFString, CFType>,
    key: core_foundation::string::CFStringRef,
) -> Option<i32> {
    let key = unsafe { CFString::wrap_under_get_rule(key) };
    dictionary.find(&key)?.downcast::<CFNumber>()?.to_i32()
}

pub fn set_launch_at_login(enabled: bool) -> anyhow::Result<()> {
    // A per-user launch agent is used for development and unsigned builds. It requires neither
    // Accessibility nor Apple Events permission. A signed distribution can replace this adapter
    // with SMAppService without changing the application boundary.
    let executable = std::env::current_exe()?;
    let home = std::env::var_os("HOME").ok_or_else(|| anyhow::anyhow!("HOME is unavailable"))?;
    let launch_agents = std::path::PathBuf::from(home).join("Library/LaunchAgents");
    let plist = launch_agents.join("com.formiga.desktop.plist");
    let domain = format!("gui/{}", unsafe { libc::getuid() });

    if enabled {
        fs::create_dir_all(&launch_agents)?;
        let executable = xml_escape(&executable.to_string_lossy());
        let contents = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>Label</key><string>com.formiga.desktop</string>
<key>ProgramArguments</key><array><string>{executable}</string></array>
<key>RunAtLoad</key><true/>
</dict></plist>
"#
        );
        fs::write(&plist, contents)?;
        let _ = Command::new("launchctl")
            .args(["bootout", &domain])
            .arg(&plist)
            .status();
        let status = Command::new("launchctl")
            .args(["bootstrap", &domain])
            .arg(&plist)
            .status()?;
        anyhow::ensure!(
            status.success(),
            "could not register the Formiga launch agent"
        );
    } else {
        if plist.exists() {
            let _ = Command::new("launchctl")
                .args(["bootout", &domain])
                .arg(&plist)
                .status();
            fs::remove_file(plist)?;
        }
    }
    Ok(())
}

pub fn open_directory(path: &std::path::Path) -> anyhow::Result<()> {
    let status = Command::new("open").arg(path).status()?;
    anyhow::ensure!(status.success(), "macOS could not open the directory");
    Ok(())
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
