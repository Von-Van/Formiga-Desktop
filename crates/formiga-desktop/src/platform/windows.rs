use formiga_core::{
    ApplicationKey, CursorSnapshot, DesktopRect, DesktopWindow, DisplayKey, MonitorInfo, Point,
};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::process::Command;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM, POINT, RECT};
use windows::Win32::Graphics::Dwm::{
    DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS, DwmGetWindowAttribute,
};
use windows::Win32::Graphics::Gdi::{CombineRgn, CreateRectRgn, DeleteObject, RGN_OR};
use windows::Win32::Storage::Packaging::Appx::GetApplicationUserModelId;
use windows::Win32::System::Registry::{
    HKEY_CURRENT_USER, KEY_SET_VALUE, REG_SZ, RegCloseKey, RegDeleteValueW, RegOpenKeyExW,
    RegSetValueExW,
};
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetLastInputInfo, LASTINPUTINFO, ReleaseCapture, SetCapture,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GWL_EXSTYLE, GWL_STYLE, GetClassNameW, GetCursorPos, GetWindowLongW,
    GetWindowRect, GetWindowThreadProcessId, HWND_TOPMOST, IsIconic, IsWindowVisible,
    SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SetWindowLongW, SetWindowPos, WS_EX_LAYERED,
    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT, WS_POPUP,
};
use windows::core::{BOOL, PWSTR};
use winit::monitor::MonitorHandle;
use winit::platform::windows::MonitorHandleExtWindows;
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::Window;

static ENUMERATED: Mutex<Vec<DesktopWindow>> = Mutex::new(Vec::new());
static ENUMERATED_OWNERS: Mutex<BTreeMap<u32, Option<(ApplicationKey, String)>>> =
    Mutex::new(BTreeMap::new());

pub fn display_key(monitor: &MonitorHandle) -> DisplayKey {
    let digest: [u8; 32] = Sha256::digest(monitor.native_id().as_bytes()).into();
    let mut key = [0_u8; 16];
    key.copy_from_slice(&digest[..16]);
    DisplayKey(key)
}

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

pub fn configure_interaction_proxy(window: &Window) {
    let Some(hwnd) = window_hwnd(window) else {
        return;
    };
    unsafe {
        let style = GetWindowLongW(hwnd, GWL_STYLE) as u32 | WS_POPUP.0;
        SetWindowLongW(hwnd, GWL_STYLE, style as i32);
        let extended = (GetWindowLongW(hwnd, GWL_EXSTYLE) as u32
            | WS_EX_LAYERED.0
            | WS_EX_TOOLWINDOW.0
            | WS_EX_NOACTIVATE.0)
            & !WS_EX_TRANSPARENT.0;
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

pub fn set_interaction_hittest(_window: &Window, _enabled: bool) {
    // The window region contains only opaque sprite pixels, so Windows performs exact native
    // hit testing without toggling the entire proxy window in response to cursor movement.
}

pub fn set_interaction_shape(window: &Window, mask: &[bool], scale: u8) {
    let Some(hwnd) = window_hwnd(window) else {
        return;
    };
    let scale = i32::from(scale.max(1));
    let combined = unsafe { CreateRectRgn(0, 0, 0, 0) };
    for y in 0..formiga_art::FRAME_SIZE as usize {
        let mut x = 0;
        while x < formiga_art::FRAME_SIZE as usize {
            while x < formiga_art::FRAME_SIZE as usize
                && !mask[y * formiga_art::FRAME_SIZE as usize + x]
            {
                x += 1;
            }
            let start = x;
            while x < formiga_art::FRAME_SIZE as usize
                && mask[y * formiga_art::FRAME_SIZE as usize + x]
            {
                x += 1;
            }
            if start == x {
                continue;
            }
            let row = unsafe {
                CreateRectRgn(
                    start as i32 * scale,
                    y as i32 * scale,
                    x as i32 * scale,
                    (y as i32 + 1) * scale,
                )
            };
            unsafe {
                CombineRgn(Some(combined), Some(combined), Some(row), RGN_OR);
                let _ = DeleteObject(row.into());
            }
        }
    }
    let accepted =
        unsafe { windows::Win32::Graphics::Gdi::SetWindowRgn(hwnd, Some(combined), true) };
    if accepted == 0 {
        unsafe {
            let _ = DeleteObject(combined.into());
        }
    }
}

pub fn begin_interaction_capture(window: &Window) {
    if let Some(hwnd) = window_hwnd(window) {
        unsafe {
            SetCapture(hwnd);
        }
    }
}

pub fn end_interaction_capture() {
    unsafe {
        let _ = ReleaseCapture();
    }
}

fn window_hwnd(window: &Window) -> Option<HWND> {
    let handle = window.window_handle().ok()?;
    let RawWindowHandle::Win32(handle) = handle.as_raw() else {
        return None;
    };
    Some(HWND(handle.hwnd.get() as *mut _))
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
    ENUMERATED_OWNERS
        .lock()
        .expect("window owner cache poisoned")
        .clear();
    unsafe {
        let _ = EnumWindows(Some(enum_window), LPARAM(0));
    }
    std::mem::take(&mut *ENUMERATED.lock().expect("window list poisoned"))
}

unsafe extern "system" fn enum_window(hwnd: HWND, _: LPARAM) -> BOOL {
    if !unsafe { IsWindowVisible(hwnd) }.as_bool()
        || unsafe { IsIconic(hwnd) }.as_bool()
        || is_desktop_chrome(hwnd)
    {
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
        let owner = ENUMERATED_OWNERS
            .lock()
            .expect("window owner cache poisoned")
            .entry(process_id)
            .or_insert_with(|| application_for_pid(process_id))
            .clone();
        let mut windows = ENUMERATED.lock().expect("window list poisoned");
        let z_order = windows.len() as u32;
        windows.push(DesktopWindow {
            key: hwnd.0 as usize as u64,
            bounds: DesktopRect {
                x: rect.left as f32,
                y: rect.top as f32,
                width: width as f32,
                height: height as f32,
            },
            z_order,
            visible: true,
            minimized: false,
            application: owner.as_ref().map(|(key, _)| key.clone()),
            application_name: owner.map(|(_, name)| name),
        });
    }
    BOOL(1)
}

fn is_desktop_chrome(hwnd: HWND) -> bool {
    let mut buffer = [0_u16; 128];
    let length = unsafe { GetClassNameW(hwnd, &mut buffer) };
    if length <= 0 {
        return false;
    }
    matches!(
        String::from_utf16_lossy(&buffer[..length as usize]).as_str(),
        "Shell_TrayWnd" | "Shell_SecondaryTrayWnd" | "Progman" | "WorkerW"
    )
}

fn application_for_pid(process_id: u32) -> Option<(ApplicationKey, String)> {
    let process =
        unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, process_id).ok()? };
    let aumid = application_user_model_id(process);
    let mut buffer = vec![0_u16; 32_768];
    let mut length = buffer.len() as u32;
    let result = unsafe {
        QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_WIN32,
            PWSTR(buffer.as_mut_ptr()),
            &mut length,
        )
    };
    let _ = unsafe { CloseHandle(process) };
    result.ok()?;
    buffer.truncate(length as usize);
    let path = String::from_utf16(&buffer).ok()?;
    let (_, name) = application_from_path(std::path::Path::new(&path))?;
    Some((
        aumid
            .map(ApplicationKey::WindowsAumid)
            .unwrap_or_else(|| application_key_from_path(std::path::Path::new(&path))),
        name,
    ))
}

fn application_user_model_id(process: windows::Win32::Foundation::HANDLE) -> Option<String> {
    let mut length = 0_u32;
    let first = unsafe { GetApplicationUserModelId(process, &mut length, None) };
    if length == 0 || (first.0 != 0 && first.0 != 122) {
        return None;
    }
    let mut buffer = vec![0_u16; length as usize];
    unsafe { GetApplicationUserModelId(process, &mut length, Some(PWSTR(buffer.as_mut_ptr()))) }
        .ok()
        .ok()?;
    let used = buffer
        .iter()
        .position(|unit| *unit == 0)
        .unwrap_or(buffer.len());
    let value = String::from_utf16(&buffer[..used]).ok()?;
    (!value.is_empty()).then_some(value)
}

fn application_key_from_path(path: &std::path::Path) -> ApplicationKey {
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let normalized = canonical.to_string_lossy().to_lowercase();
    ApplicationKey::WindowsExecutableHash(Sha256::digest(normalized.as_bytes()).into())
}

fn application_from_path(path: &std::path::Path) -> Option<(ApplicationKey, String)> {
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let name = canonical
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("Windows application")
        .to_owned();
    Some((application_key_from_path(&canonical), name))
}

pub fn browse_application() -> Option<(ApplicationKey, String)> {
    let path = rfd::FileDialog::new()
        .set_title("Choose a Windows application")
        .add_filter("Windows applications", &["exe"])
        .pick_file()?;
    application_from_path(&path)
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
