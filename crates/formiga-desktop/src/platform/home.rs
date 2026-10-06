//! Finding Formiga Home and starting it for a visit to a house, by the names
//! `formiga_home_contract::discovery` gives. How any companion app is found and started is in
//! [`super::companion_app`].

use super::companion_app::{self, AppNames};
use formiga_home_contract::{LAUNCH_ARGUMENT, discovery};
use std::path::{Path, PathBuf};
use std::process::Child;

const HOME: AppNames = AppNames {
    path_override_env: discovery::PATH_OVERRIDE_ENV,
    macos_bundle_id: discovery::MACOS_BUNDLE_ID,
    macos_reads_key: discovery::MACOS_HOME_VERSION_KEY,
    windows_registry_key: discovery::WINDOWS_REGISTRY_KEY,
    windows_path_value: discovery::WINDOWS_PATH_VALUE,
    windows_version_value: discovery::WINDOWS_VERSION_VALUE,
    windows_reads_value: discovery::WINDOWS_HOME_VERSION_VALUE,
};

/// An installed Formiga Home, as far as Desktop can tell without starting it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HomeInstall {
    /// An app bundle on macOS, an executable anywhere else.
    pub path: PathBuf,
    pub version: Option<String>,
    /// The newest household version it reads, when it says.
    pub home_version: Option<u32>,
}

impl HomeInstall {
    /// Why this Home cannot open a house written as version `needs`, said so that it can be acted
    /// on. `None` when it can, or when it does not say which versions it reads; Home then answers
    /// for itself once it has read the snapshot.
    pub fn incompatibility(&self, needs: u32) -> Option<String> {
        let reads = self.home_version?;
        (reads < needs).then(|| {
            let which = self
                .version
                .as_deref()
                .map(|version| format!("Formiga Home {version}"))
                .unwrap_or_else(|| "This copy of Formiga Home".to_owned());
            format!(
                "{which} reads household version {reads}, and this colony needs version \
                 {needs}. Update Formiga Home to open its houses."
            )
        })
    }
}

/// Formiga Home, if it is installed: the development override first, then wherever the platform
/// says Home is.
pub fn find_formiga_home() -> Option<HomeInstall> {
    companion_app::find(&HOME).map(|app| HomeInstall {
        path: app.path,
        version: app.version,
        home_version: app.reads,
    })
}

/// Start Home for the visit whose session directory is `session`.
pub fn launch_formiga_home(install: &HomeInstall, session: &Path) -> std::io::Result<Child> {
    companion_app::launch(&install.path, LAUNCH_ARGUMENT, session)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_older_home_is_told_exactly_what_it_lacks() {
        let install = HomeInstall {
            path: PathBuf::from("Formiga Home.app"),
            version: Some("0.1.0".to_owned()),
            home_version: Some(1),
        };
        assert_eq!(install.incompatibility(1), None);
        assert_eq!(
            install.incompatibility(2).as_deref(),
            Some(
                "Formiga Home 0.1.0 reads household version 1, and this colony needs version 2. \
                 Update Formiga Home to open its houses."
            )
        );
        let silent = HomeInstall {
            home_version: None,
            ..install
        };
        assert_eq!(silent.incompatibility(9), None, "Home answers for itself");
    }
}
