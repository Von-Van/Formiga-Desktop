use formiga_core::{CursorSnapshot, DesktopRect, DesktopWindow, MonitorInfo, Point};
use std::process::Command;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use windows::Win32::Foundation::{HWND, LPARAM, POINT, RECT};
use windows::Win32::Graphics::Dwm::{
    DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS, DwmGetWindowAttribute,
};
use windows::Win32::System::Registry::{
    HKEY_CURRENT_USER, KEY_SET_VALUE, REG_SZ, RegCloseKey, RegDeleteValueW, RegOpenKeyExW,
    RegSetValueExW,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GWL_EXSTYLE, GWL_STYLE, GetCursorPos, GetWindowLongW, GetWindowRect,
    GetWindowThreadProcessId, HWND_TOPMOST, IsWindowVisible, SWP_NOACTIVATE, SWP_NOMOVE,
    SWP_NOSIZE, SetWindowLongW, SetWindowPos, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    WS_EX_TRANSPARENT, WS_POPUP,
};
use windows::core::BOOL;
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::Window;

static ENUMERATED: Mutex<Vec<DesktopWindow>> = Mutex::new(Vec::new());

pub fn configure_native_overlay(window: &Window) {
    let _ = window.set_cursor_hittest(false);
    let Ok(handle) = window.window_handle() else {
        return;
    };
    let RawWindowHandle::Win32(handle) = handle.as_raw() else {
        return;
    };
    let hwnd = HWND(handle.hwnd.get() as *mut _);
    unsafe {
        let style = GetWindowLongW(hwnd, GWL_STYLE) as u32 | WS_POPUP.0;
        SetWindowLongW(hwnd, GWL_STYLE, style as i32);
        let extended = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32
            | WS_EX_LAYERED.0
            | WS_EX_TRANSPARENT.0
            | WS_EX_TOOLWINDOW.0
            | WS_EX_NOACTIVATE.0;
        SetWindowLongW(hwnd, GWL_EXSTYLE, extended as i32);
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );
    }
}

pub fn canonical_monitor_bounds(
    physical_x: i32,
    physical_y: i32,
    physical_width: u32,
    physical_height: u32,
    scale: f32,
) -> DesktopRect {
    // Windows' virtual desktop origins remain in the DPI-aware Win32 coordinate space. Local
    // dimensions are converted to points, keeping disjoint displays as independent graphs.
    DesktopRect {
        x: physical_x as f32,
        y: physical_y as f32,
        width: physical_width as f32 / scale,
        height: physical_height as f32 / scale,
    }
}

pub fn normalize_cursor(cursor: &mut CursorSnapshot, monitors: &[MonitorInfo]) {
    let scale = physical_monitor(cursor.position, monitors)
        .map(|monitor| monitor.scale_factor)
        .unwrap_or(1.0);
    cursor.position = physical_point_to_logical(cursor.position, monitors);
    cursor.velocity.x /= scale;
    cursor.velocity.y /= scale;
}

pub fn normalize_windows(windows: &mut [DesktopWindow], monitors: &[MonitorInfo]) {
    for window in windows {
        let top_left = physical_point_to_logical(
            Point {
                x: window.bounds.x,
                y: window.bounds.y,
            },
            monitors,
        );
        let bottom_right = physical_point_to_logical(
            Point {
                x: window.bounds.right(),
                y: window.bounds.bottom(),
            },
            monitors,
        );
        window.bounds = DesktopRect {
            x: top_left.x,
            y: top_left.y,
            width: (bottom_right.x - top_left.x).max(1.0),
            height: (bottom_right.y - top_left.y).max(1.0),
        };
    }
}

fn physical_point_to_logical(point: Point, monitors: &[MonitorInfo]) -> Point {
    let Some(monitor) = physical_monitor(point, monitors)
        .or_else(|| monitors.iter().find(|monitor| monitor.primary))
        .or_else(|| monitors.first())
    else {
        return point;
    };
    Point {
        x: monitor.bounds.x + (point.x - monitor.bounds.x) / monitor.scale_factor,
        y: monitor.bounds.y + (point.y - monitor.bounds.y) / monitor.scale_factor,
    }
}

fn physical_monitor(point: Point, monitors: &[MonitorInfo]) -> Option<&MonitorInfo> {
    monitors.iter().find(|monitor| {
        let physical = DesktopRect {
            x: monitor.bounds.x,
            y: monitor.bounds.y,
            width: monitor.bounds.width * monitor.scale_factor,
            height: monitor.bounds.height * monitor.scale_factor,
        };
        physical.contains(point)
    })
}

pub fn cursor_and_idle(previous: Option<(Point, Instant)>) -> (CursorSnapshot, Duration) {
    let mut point = POINT::default();
    let available = unsafe { GetCursorPos(&mut point).is_ok() };
    let position = Point {
        x: point.x as f32,
        y: point.y as f32,
    };
    let velocity = previous
        .map(|(old, instant)| {
            let seconds = instant.elapsed().as_secs_f32().max(0.001);
            Point {
                x: (position.x - old.x) / seconds,
                y: (position.y - old.y) / seconds,
            }
        })
        .unwrap_or_default();
    let mut info = LASTINPUTINFO {
        cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
        dwTime: 0,
    };
    let idle = if unsafe { GetLastInputInfo(&mut info).as_bool() } {
        Duration::from_millis(
            unsafe { windows::Win32::System::SystemInformation::GetTickCount() }
                .wrapping_sub(info.dwTime) as u64,
        )
    } else {
        Duration::ZERO
    };
    (
        CursorSnapshot {
            position,
            velocity,
            available,
        },
        idle,
    )
}

pub fn visible_windows() -> Vec<DesktopWindow> {
    ENUMERATED.lock().expect("window list poisoned").clear();
    unsafe {
        let _ = EnumWindows(Some(enum_window), LPARAM(0));
    }
    std::mem::take(&mut *ENUMERATED.lock().expect("window list poisoned"))
}

unsafe extern "system" fn enum_window(hwnd: HWND, _: LPARAM) -> BOOL {
    if !unsafe { IsWindowVisible(hwnd) }.as_bool() {
        return BOOL(1);
    }
    let mut process_id = 0;
    unsafe {
        GetWindowThreadProcessId(hwnd, Some(&mut process_id));
    }
    if process_id == std::process::id() {
        return BOOL(1);
    }
    let mut cloaked = 0_u32;
    let _ = unsafe {
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED,
            (&mut cloaked as *mut u32).cast(),
            std::mem::size_of::<u32>() as u32,
        )
    };
    if cloaked != 0 {
        return BOOL(1);
    }
    let mut rect = RECT::default();
    if unsafe {
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            (&mut rect as *mut RECT).cast(),
            std::mem::size_of::<RECT>() as u32,
        )
    }
    .is_err()
    {
        let _ = unsafe { GetWindowRect(hwnd, &mut rect) };
    }
    let width = rect.right - rect.left;
    let height = rect.bottom - rect.top;
    if width >= 120 && height >= 80 {
        ENUMERATED
            .lock()
            .expect("window list poisoned")
            .push(DesktopWindow {
                key: hwnd.0 as usize as u64,
                bounds: DesktopRect {
                    x: rect.left as f32,
                    y: rect.top as f32,
                    width: width as f32,
                    height: height as f32,
                },
                z_order: 0,
                visible: true,
                minimized: false,
            });
    }
    BOOL(1)
}

pub fn set_launch_at_login(enabled: bool) -> anyhow::Result<()> {
    use windows::core::PCWSTR;
    let key_path: Vec<u16> = "Software\\Microsoft\\Windows\\CurrentVersion\\Run\0"
        .encode_utf16()
        .collect();
    let value_name: Vec<u16> = "Formiga\0".encode_utf16().collect();
    let mut key = Default::default();
    unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(key_path.as_ptr()),
            Some(0),
            KEY_SET_VALUE,
            &mut key,
        )
        .ok()?;
    }
    if enabled {
        let value: Vec<u16> = format!("\"{}\"\0", std::env::current_exe()?.display())
            .encode_utf16()
            .collect();
        unsafe {
            RegSetValueExW(
                key,
                PCWSTR(value_name.as_ptr()),
                None,
                REG_SZ,
                Some(std::slice::from_raw_parts(
                    value.as_ptr().cast(),
                    value.len() * 2,
                )),
            )
            .ok()?;
        }
    } else {
        unsafe {
            let _ = RegDeleteValueW(key, PCWSTR(value_name.as_ptr()));
        }
    }
    unsafe {
        let _ = RegCloseKey(key);
    }
    Ok(())
}

pub fn open_directory(path: &std::path::Path) -> anyhow::Result<()> {
    let status = Command::new("explorer.exe").arg(path).status()?;
    anyhow::ensure!(status.success(), "Windows could not open the directory");
    Ok(())
}
