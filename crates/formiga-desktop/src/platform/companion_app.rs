//! Finding one of Formiga's companion apps, Formiga Hill or Formiga Home, and starting it.
//!
//! Nothing here runs on its own. Desktop asks whether an app is installed when it starts and then
//! at most every ten minutes, from inside a tick it was taking anyway, and starts it only when its
//! owner asks. An app is given two arguments, its contract's flag and the visit's own session
//! directory, and nothing else.
//!
//! An app is started so that Desktop learns when it exits: the process handle is waited on by a
//! thread that sleeps until then. On macOS an app bundle is opened through LaunchServices with
//! `open -W`, which returns when the app does; anything else is started directly.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

/// The names a companion app is found by, as its contract's `discovery` module gives them.
#[derive(Clone, Copy, Debug)]
pub struct AppNames {
    /// For development: the path of the app (or, on macOS, an `.app`) to use instead of an
    /// installed one.
    pub path_override_env: &'static str,
    /// The macOS bundle identifier LaunchServices is asked for.
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    pub macos_bundle_id: &'static str,
    /// An integer in the app's `Info.plist`: the newest version of its contract it reads.
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    pub macos_reads_key: &'static str,
    /// The registry key its Windows installer writes, under `HKEY_CURRENT_USER` or, for a
    /// machine-wide install, `HKEY_LOCAL_MACHINE`.
    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    pub windows_registry_key: &'static str,
    /// `REG_SZ`: the full path of its executable.
    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    pub windows_path_value: &'static str,
    /// `REG_SZ`: its version, for messages.
    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    pub windows_version_value: &'static str,
    /// `REG_DWORD`: the newest version of its contract it reads.
    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    pub windows_reads_value: &'static str,
}

/// An installed companion app, as far as Desktop can tell without starting it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppInstall {
    /// An app bundle on macOS, an executable anywhere else.
    pub path: PathBuf,
    pub version: Option<String>,
    /// The newest version of its contract it reads, when it says.
    pub reads: Option<u32>,
}

/// The app, if it is installed: the development override first, then wherever the platform says
/// it is.
pub fn find(names: &AppNames) -> Option<AppInstall> {
    if let Some(path) = std::env::var_os(names.path_override_env) {
        let path = PathBuf::from(path);
        return path.exists().then_some(AppInstall {
            path,
            version: None,
            reads: None,
        });
    }
    installed(names)
}

/// Start the app at `path` for the visit whose session directory is `session`.
pub fn launch(path: &Path, argument: &str, session: &Path) -> std::io::Result<Child> {
    let mut command = launcher(path);
    command.arg(argument).arg(session).stdin(Stdio::null());
    command.spawn()
}

#[cfg(target_os = "macos")]
fn launcher(path: &Path) -> Command {
    if path.extension().is_some_and(|extension| extension == "app") {
        // A new instance, so a copy already open for something else is not handed this visit,
        // and `-W` so the process ends when the app does.
        let mut command = Command::new("/usr/bin/open");
        command.args(["-W", "-n", "-a"]).arg(path).arg("--args");
        command
    } else {
        Command::new(path)
    }
}

#[cfg(target_os = "windows")]
fn launcher(path: &Path) -> Command {
    Command::new(path)
}

/// The app as LaunchServices knows it, by its bundle identifier, with the version and the
/// contract version its `Info.plist` gives.
#[cfg(target_os = "macos")]
fn installed(names: &AppNames) -> Option<AppInstall> {
    use objc2_app_kit::NSWorkspace;
    use objc2_foundation::{NSBundle, NSNumber, NSString};

    let workspace = NSWorkspace::sharedWorkspace();
    let url = workspace
        .URLForApplicationWithBundleIdentifier(&NSString::from_str(names.macos_bundle_id))?;
    let path = PathBuf::from(url.path()?.to_string());
    let bundle = NSBundle::bundleWithURL(&url);
    let info = |key: &str| {
        bundle
            .as_ref()?
            .objectForInfoDictionaryKey(&NSString::from_str(key))
    };
    let version = info("CFBundleShortVersionString")
        .and_then(|value| value.downcast::<NSString>().ok())
        .map(|value| formiga_travel::sanitize_text(&value.to_string(), 32))
        .filter(|value| !value.is_empty());
    let reads = info(names.macos_reads_key).and_then(|value| match value.downcast::<NSNumber>() {
        Ok(number) => u32::try_from(number.integerValue()).ok(),
        Err(value) => value
            .downcast::<NSString>()
            .ok()?
            .to_string()
            .trim()
            .parse()
            .ok(),
    });
    Some(AppInstall {
        path,
        version,
        reads,
    })
}

/// The app as its installer registered it: for this user first, then for the machine.
#[cfg(target_os = "windows")]
fn installed(names: &AppNames) -> Option<AppInstall> {
    use windows::Win32::System::Registry::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};

    let key = names.windows_registry_key;
    [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE]
        .into_iter()
        .find_map(|root| {
            let path = PathBuf::from(registry::string(root, key, names.windows_path_value)?);
            path.is_file().then(|| AppInstall {
                path,
                version: registry::string(root, key, names.windows_version_value)
                    .map(|value| formiga_travel::sanitize_text(&value, 32))
                    .filter(|value| !value.is_empty()),
                reads: registry::dword(root, key, names.windows_reads_value),
            })
        })
}

#[cfg(target_os = "windows")]
mod registry {
    use windows::Win32::System::Registry::{HKEY, RRF_RT_REG_DWORD, RRF_RT_REG_SZ, RegGetValueW};
    use windows::core::PCWSTR;

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(Some(0)).collect()
    }

    /// A `REG_SZ` value under `key`, at most a path's length.
    pub(super) fn string(root: HKEY, key: &str, value: &str) -> Option<String> {
        let (key, value) = (wide(key), wide(value));
        let mut buffer = vec![0_u16; 1024];
        let mut bytes = (buffer.len() * 2) as u32;
        unsafe {
            RegGetValueW(
                root,
                PCWSTR(key.as_ptr()),
                PCWSTR(value.as_ptr()),
                RRF_RT_REG_SZ,
                None,
                Some(buffer.as_mut_ptr().cast()),
                Some(&mut bytes),
            )
        }
        .ok()
        .ok()?;
        let written = (bytes as usize / 2).min(buffer.len());
        let end = buffer[..written]
            .iter()
            .position(|unit| *unit == 0)
            .unwrap_or(written);
        Some(String::from_utf16_lossy(&buffer[..end]))
    }

    /// A `REG_DWORD` value under `key`.
    pub(super) fn dword(root: HKEY, key: &str, value: &str) -> Option<u32> {
        let (key, value) = (wide(key), wide(value));
        let mut data = 0_u32;
        let mut bytes = 4_u32;
        unsafe {
            RegGetValueW(
                root,
                PCWSTR(key.as_ptr()),
                PCWSTR(value.as_ptr()),
                RRF_RT_REG_DWORD,
                None,
                Some((&mut data as *mut u32).cast()),
                Some(&mut bytes),
            )
        }
        .ok()
        .ok()?;
        Some(data)
    }
}
