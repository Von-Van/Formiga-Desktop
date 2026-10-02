//! Finding Formiga Hill and starting it for a trip.
//!
//! Nothing here runs on its own. Desktop asks whether Hill is installed when it starts and then at
//! most every ten minutes, from inside a tick it was taking anyway, and starts Hill only when its
//! owner picks the trip from the tray menu. Hill is given two arguments, the travel flag and the
//! trip's own session directory, and nothing else.
//!
//! Hill is started so that Desktop learns when it exits: the process handle is waited on by a
//! thread that sleeps until then. On macOS an app bundle is opened through LaunchServices with
//! `open -W`, which returns when the app does; anything else is started directly.

use formiga_travel::{LAUNCH_ARGUMENT, discovery};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

/// An installed Formiga Hill, as far as Desktop can tell without starting it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HillInstall {
    /// An app bundle on macOS, an executable anywhere else.
    pub path: PathBuf,
    pub version: Option<String>,
    /// The newest travel version it reads, when it says.
    pub travel_version: Option<u32>,
}

impl HillInstall {
    /// Why this Hill cannot take a colony that travels as version `needs`, said so that it can be
    /// acted on. `None` when it can, or when it does not say which versions it reads; Hill then
    /// answers for itself once it has read the snapshot.
    pub fn incompatibility(&self, needs: u32) -> Option<String> {
        let reads = self.travel_version?;
        (reads < needs).then(|| {
            let which = self
                .version
                .as_deref()
                .map(|version| format!("Formiga Hill {version}"))
                .unwrap_or_else(|| "This copy of Formiga Hill".to_owned());
            format!(
                "{which} reads travel version {reads}, and this colony travels as version \
                 {needs}. Update Formiga Hill to take the colony there."
            )
        })
    }
}

/// Formiga Hill, if it is installed: the development override first, then wherever the platform
/// says Hill is.
pub fn find_formiga_hill() -> Option<HillInstall> {
    if let Some(path) = std::env::var_os(discovery::PATH_OVERRIDE_ENV) {
        let path = PathBuf::from(path);
        return path.exists().then_some(HillInstall {
            path,
            version: None,
            travel_version: None,
        });
    }
    installed()
}

/// Start Hill for the trip whose session directory is `session`.
pub fn launch_formiga_hill(install: &HillInstall, session: &Path) -> std::io::Result<Child> {
    let mut command = launcher(&install.path);
    command
        .arg(LAUNCH_ARGUMENT)
        .arg(session)
        .stdin(Stdio::null());
    command.spawn()
}

#[cfg(target_os = "macos")]
fn launcher(path: &Path) -> Command {
    if path.extension().is_some_and(|extension| extension == "app") {
        // A new instance, so a Hill already open for something else is not handed this trip, and
        // `-W` so the process ends when Hill does.
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

/// Hill as LaunchServices knows it, by its bundle identifier, with the version and the travel
/// version its `Info.plist` gives.
#[cfg(target_os = "macos")]
fn installed() -> Option<HillInstall> {
    use objc2_app_kit::NSWorkspace;
    use objc2_foundation::{NSBundle, NSNumber, NSString};

    let workspace = NSWorkspace::sharedWorkspace();
    let url = workspace
        .URLForApplicationWithBundleIdentifier(&NSString::from_str(discovery::MACOS_BUNDLE_ID))?;
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
    let travel_version =
        info(discovery::MACOS_TRAVEL_VERSION_KEY).and_then(|value| match value.downcast::<NSNumber>() {
            Ok(number) => u32::try_from(number.integerValue()).ok(),
            Err(value) => value
                .downcast::<NSString>()
                .ok()?
                .to_string()
                .trim()
                .parse()
                .ok(),
        });
    Some(HillInstall {
        path,
        version,
        travel_version,
    })
}

/// Hill as its installer registered it: for this user first, then for the machine.
#[cfg(target_os = "windows")]
fn installed() -> Option<HillInstall> {
    use windows::Win32::System::Registry::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};

    [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE]
        .into_iter()
        .find_map(|root| {
            let path = PathBuf::from(registry::string(root, discovery::WINDOWS_PATH_VALUE)?);
            path.is_file().then(|| HillInstall {
                path,
                version: registry::string(root, discovery::WINDOWS_VERSION_VALUE)
                    .map(|value| formiga_travel::sanitize_text(&value, 32))
                    .filter(|value| !value.is_empty()),
                travel_version: registry::dword(root, discovery::WINDOWS_TRAVEL_VERSION_VALUE),
            })
        })
}

#[cfg(target_os = "windows")]
mod registry {
    use formiga_travel::discovery::WINDOWS_REGISTRY_KEY;
    use windows::Win32::System::Registry::{HKEY, RRF_RT_REG_DWORD, RRF_RT_REG_SZ, RegGetValueW};
    use windows::core::PCWSTR;

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(Some(0)).collect()
    }

    /// A `REG_SZ` value under Hill's key, at most a path's length.
    pub(super) fn string(root: HKEY, value: &str) -> Option<String> {
        let (key, value) = (wide(WINDOWS_REGISTRY_KEY), wide(value));
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

    /// A `REG_DWORD` value under Hill's key.
    pub(super) fn dword(root: HKEY, value: &str) -> Option<u32> {
        let (key, value) = (wide(WINDOWS_REGISTRY_KEY), wide(value));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_older_hill_is_told_exactly_what_it_lacks() {
        let install = HillInstall {
            path: PathBuf::from("Formiga Hill.app"),
            version: Some("0.1.0".to_owned()),
            travel_version: Some(1),
        };
        assert_eq!(install.incompatibility(1), None);
        assert_eq!(
            install.incompatibility(2).as_deref(),
            Some(
                "Formiga Hill 0.1.0 reads travel version 1, and this colony travels as version 2. \
                 Update Formiga Hill to take the colony there."
            )
        );
        let silent = HillInstall {
            travel_version: None,
            ..install
        };
        assert_eq!(silent.incompatibility(9), None, "Hill answers for itself");
    }
}
