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
use formiga_core::{
    ApplicationKey, CursorSnapshot, DesktopRect, DesktopWindow, DisplayKey, MonitorInfo, Point,
};
use objc2_app_kit::{NSRunningApplication, NSView, NSWindowCollectionBehavior};
use std::collections::BTreeMap;
use std::fs;
use std::process::Command;
use std::time::Duration;
use winit::monitor::MonitorHandle;
use winit::platform::macos::MonitorHandleExtMacOS;
use winit::platform::macos::WindowExtMacOS;
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::Window;

pub fn display_key(monitor: &MonitorHandle) -> DisplayKey {
    let id = monitor.native_id();
    let mut bytes = [0_u8; 16];
    bytes[..4].copy_from_slice(&id.to_le_bytes());
    bytes[4..8].copy_from_slice(&(!id).to_le_bytes());
    bytes[8..12].copy_from_slice(b"macD");
    bytes[12..].copy_from_slice(b"ispl");
    DisplayKey(bytes)
}

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
        let behavior = ns_window.collectionBehavior()
            | NSWindowCollectionBehavior::CanJoinAllSpaces
            | NSWindowCollectionBehavior::Stationary
            | NSWindowCollectionBehavior::IgnoresCycle
            | NSWindowCollectionBehavior::FullScreenAuxiliary;
        ns_window.setCollectionBehavior(behavior);
    }
}

pub fn configure_interaction_proxy(window: &Window) {
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
        let behavior = ns_window.collectionBehavior()
            | NSWindowCollectionBehavior::CanJoinAllSpaces
            | NSWindowCollectionBehavior::Stationary
            | NSWindowCollectionBehavior::IgnoresCycle
            | NSWindowCollectionBehavior::FullScreenAuxiliary;
        ns_window.setCollectionBehavior(behavior);
    }
}

pub fn set_interaction_hittest(window: &Window, enabled: bool) {
    let _ = window.set_cursor_hittest(enabled);
}

pub fn set_interaction_shape(_window: &Window, _mask: &[bool], _scale: u8) {}

pub fn begin_interaction_capture(_window: &Window) {}

pub fn end_interaction_capture() {}

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
    let mut owners = BTreeMap::new();
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
        let owner_pid = number(&dictionary, unsafe {
            core_graphics::window::kCGWindowOwnerPID
        });
        if owner_pid == Some(std::process::id() as i32) {
            continue;
        }
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
        let owner = owner_pid.and_then(|pid| {
            owners
                .entry(pid)
                .or_insert_with(|| application_for_pid(pid))
                .clone()
        });
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
            application: owner.as_ref().map(|(key, _)| key.clone()),
            application_name: owner.map(|(_, name)| name),
        });
    }
    windows
}

fn application_for_pid(pid: i32) -> Option<(ApplicationKey, String)> {
    let application = NSRunningApplication::runningApplicationWithProcessIdentifier(pid)?;
    let bundle = application.bundleIdentifier()?.to_string();
    let name = application
        .localizedName()
        .map(|name| name.to_string())
        .unwrap_or_else(|| bundle.clone());
    Some((ApplicationKey::MacBundleId(bundle), name))
}

pub fn browse_application() -> Option<(ApplicationKey, String)> {
    let path = rfd::FileDialog::new()
        .set_title("Choose a macOS application")
        .set_directory("/Applications")
        .pick_file()?;
    let output = Command::new("plutil")
        .args(["-extract", "CFBundleIdentifier", "raw", "-o", "-"])
        .arg(path.join("Contents/Info.plist"))
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let bundle = String::from_utf8(output.stdout).ok()?.trim().to_owned();
    if bundle.is_empty() {
        return None;
    }
    let name = path
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or(&bundle)
        .to_owned();
    Some((ApplicationKey::MacBundleId(bundle), name))
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
