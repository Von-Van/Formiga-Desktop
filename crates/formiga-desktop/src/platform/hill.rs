//! Finding Formiga Hill and starting it for a trip, by the names `formiga_travel::discovery`
//! gives. How any companion app is found and started is in [`super::companion_app`].

use super::companion_app::{self, AppNames};
use formiga_travel::{LAUNCH_ARGUMENT, discovery};
use std::path::{Path, PathBuf};
use std::process::Child;

const HILL: AppNames = AppNames {
    path_override_env: discovery::PATH_OVERRIDE_ENV,
    macos_bundle_id: discovery::MACOS_BUNDLE_ID,
    macos_reads_key: discovery::MACOS_TRAVEL_VERSION_KEY,
    windows_registry_key: discovery::WINDOWS_REGISTRY_KEY,
    windows_path_value: discovery::WINDOWS_PATH_VALUE,
    windows_version_value: discovery::WINDOWS_VERSION_VALUE,
    windows_reads_value: discovery::WINDOWS_TRAVEL_VERSION_VALUE,
};

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
                "{which} reads travel version {reads}, and this colony needs version {needs}. \
                 Update Formiga Hill to take the colony there."
            )
        })
    }
}

/// Formiga Hill, if it is installed: the development override first, then wherever the platform
/// says Hill is.
pub fn find_formiga_hill() -> Option<HillInstall> {
    companion_app::find(&HILL).map(|app| HillInstall {
        path: app.path,
        version: app.version,
        travel_version: app.reads,
    })
}

/// Start Hill for the trip whose session directory is `session`.
pub fn launch_formiga_hill(install: &HillInstall, session: &Path) -> std::io::Result<Child> {
    companion_app::launch(&install.path, LAUNCH_ARGUMENT, session)
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
                "Formiga Hill 0.1.0 reads travel version 1, and this colony needs version 2. \
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
