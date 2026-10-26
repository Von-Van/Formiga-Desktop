//! What every companion app does the same way on its own side of a visit: where it keeps its
//! files, how large it draws the owner's text, and the two requests its packaging scripts make
//! of it before it parses anything else.
//!
//! Formiga Hill and Formiga Home each carry a copy of these (Hill in `memories.rs`, `app.rs` and
//! `main.rs`; Home in `store.rs`, `household.rs` and `main.rs`), and the apps after them copied
//! them again. They live here so each app can take them with the rest of Desktop's crates, by
//! tag, and drop its own copy without anything it does changing.

use std::ffi::OsString;
use std::path::PathBuf;

/// A companion app's data folder: the folder named by the environment variable `env_override`
/// if it is set and not empty, for development and tests as Desktop's `FORMIGA_DATA_DIR` is;
/// otherwise the platform's usual place for `app_name` under Formiga's name. `None` only when
/// the platform has no usual place, as when there is no home directory.
///
/// This is exactly what Hill's `Memories::folder` (with `FORMIGA_HILL_DATA_DIR` and
/// "Formiga Hill") and Home's `store::folder` (with `FORMIGA_HOME_DATA_DIR` and "Formiga Home")
/// do, so an app that adopts it keeps its files where they already are.
pub fn data_dir(env_override: &str, app_name: &str) -> Option<PathBuf> {
    data_dir_from(std::env::var_os(env_override), app_name)
}

/// [`data_dir`] with the environment variable already read, so the tests can give it any value
/// without changing the process's environment under the other tests.
fn data_dir_from(override_value: Option<OsString>, app_name: &str) -> Option<PathBuf> {
    if let Some(dir) = override_value.filter(|dir| !dir.is_empty()) {
        return Some(PathBuf::from(dir));
    }
    directories::ProjectDirs::from("com", "Formiga", app_name)
        .map(|dirs| dirs.data_dir().to_owned())
}

/// The zoom an app's window is drawn at for text `percent` large: the percent kept to 100..=150,
/// the range Desktop's text size setting offers, as a factor from 1.0 to 1.5. A plain `f32`, so
/// the rulebook needs no GUI crate; an app hands it to egui's `set_zoom_factor`.
///
/// Home's `Household::text_scale` clamps and divides this way; Hill clamps in `Play::text_scale`
/// and divides in `HillApp::new` and its sound settings. It takes a `u16` because Hill's chosen
/// size already is one, and the snapshot's `text_scale_percent` (a `u8`) converts into one
/// without loss.
pub fn text_zoom(percent: u16) -> f32 {
    f32::from(percent.clamp(100, 150)) / 100.0
}

/// What a companion app was asked to do before anything else, by its first argument.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EarlyRequest {
    /// Print the newest version of the app's contract it reads, and exit: the packaging scripts
    /// put it in the macOS bundle and the Windows registry for Desktop to find.
    PrintVersion,
    /// Write the app's icon files into this folder, and exit: the packaging scripts put them in
    /// the bundle and the installer.
    WriteIcon(PathBuf),
}

/// `--icon` came first with no folder after it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error("--icon needs a folder to write the icon into")]
pub struct IconFolderMissing;

/// Whether the packaging scripts asked the app for something before its real argument parsing:
/// `version_flag` (such as `--home-version` or `--travel-version`) or `--icon <folder>`. `args` is
/// the whole command line as `std::env::args_os()` gives it, the program's name first.
///
/// Either flag counts only as the very first argument, and anything after the version flag or
/// after `--icon`'s folder is ignored, as in Hill's and Home's `main` today. The error says what
/// they say when `--icon` has no folder. Printing, writing the icon and exiting stay in each app,
/// since what each prints and writes is its own.
pub fn first_argument_request(
    version_flag: &str,
    args: impl IntoIterator<Item = OsString>,
) -> Result<Option<EarlyRequest>, IconFolderMissing> {
    let mut args = args.into_iter().skip(1);
    match args.next() {
        Some(first) if first == version_flag => Ok(Some(EarlyRequest::PrintVersion)),
        Some(first) if first == "--icon" => args
            .next()
            .map(|folder| Some(EarlyRequest::WriteIcon(PathBuf::from(folder))))
            .ok_or(IconFolderMissing),
        _ => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn command_line(args: &[&str]) -> Vec<OsString> {
        std::iter::once("formiga-test")
            .chain(args.iter().copied())
            .map(OsString::from)
            .collect()
    }

    fn request(args: &[&str]) -> Result<Option<EarlyRequest>, IconFolderMissing> {
        first_argument_request("--test-version", command_line(args))
    }

    #[test]
    fn a_set_override_names_the_data_folder() {
        assert_eq!(
            data_dir_from(Some(OsString::from("/tmp/formiga-test")), "Formiga Test"),
            Some(PathBuf::from("/tmp/formiga-test"))
        );
    }

    #[test]
    fn without_an_override_the_data_folder_is_the_platforms_place_for_the_app() {
        let usual = directories::ProjectDirs::from("com", "Formiga", "Formiga Test")
            .map(|dirs| dirs.data_dir().to_owned());
        assert_eq!(data_dir_from(None, "Formiga Test"), usual);
    }

    #[test]
    fn an_empty_override_counts_as_unset() {
        assert_eq!(
            data_dir_from(Some(OsString::new()), "Formiga Test"),
            data_dir_from(None, "Formiga Test")
        );
    }

    #[test]
    fn an_unset_environment_variable_gives_the_platforms_place() {
        assert_eq!(
            data_dir("FORMIGA_RULEBOOK_TEST_NEVER_SET_DATA_DIR", "Formiga Test"),
            data_dir_from(None, "Formiga Test")
        );
    }

    #[test]
    fn text_zoom_runs_from_one_to_one_and_a_half() {
        assert_eq!(text_zoom(100), 1.0);
        assert_eq!(text_zoom(125), 1.25);
        assert_eq!(text_zoom(150), 1.5);
    }

    #[test]
    fn text_zoom_keeps_sizes_outside_the_setting_to_its_bounds() {
        assert_eq!(text_zoom(0), 1.0);
        assert_eq!(text_zoom(99), 1.0);
        assert_eq!(text_zoom(151), 1.5);
        assert_eq!(text_zoom(u16::MAX), 1.5);
    }

    #[test]
    fn the_version_flag_first_asks_for_the_version() {
        assert_eq!(
            request(&["--test-version"]),
            Ok(Some(EarlyRequest::PrintVersion))
        );
        assert_eq!(
            request(&["--test-version", "--icon", "out"]),
            Ok(Some(EarlyRequest::PrintVersion)),
            "what follows the version flag is ignored"
        );
    }

    #[test]
    fn icon_first_asks_for_the_icon_in_the_folder_after_it() {
        assert_eq!(
            request(&["--icon", "packaging/out"]),
            Ok(Some(EarlyRequest::WriteIcon(PathBuf::from(
                "packaging/out"
            ))))
        );
        assert_eq!(
            request(&["--icon", "out", "--test-version"]),
            Ok(Some(EarlyRequest::WriteIcon(PathBuf::from("out")))),
            "what follows the folder is ignored"
        );
    }

    #[test]
    fn icon_without_a_folder_says_so_in_the_apps_words() {
        assert_eq!(request(&["--icon"]), Err(IconFolderMissing));
        assert_eq!(
            IconFolderMissing.to_string(),
            "--icon needs a folder to write the icon into"
        );
    }

    #[test]
    fn either_flag_anywhere_but_first_is_left_for_the_apps_own_parsing() {
        assert_eq!(request(&[]), Ok(None));
        assert_eq!(request(&["--sample"]), Ok(None));
        assert_eq!(request(&["--sample", "--test-version"]), Ok(None));
        assert_eq!(request(&["--sample", "--icon", "out"]), Ok(None));
        assert_eq!(request(&["/tmp/session", "--icon"]), Ok(None));
    }

    #[test]
    fn the_program_name_is_never_taken_for_a_flag() {
        assert_eq!(
            first_argument_request("--test-version", [OsString::from("--test-version")]),
            Ok(None)
        );
        assert_eq!(
            first_argument_request("--test-version", [OsString::from("--icon")]),
            Ok(None)
        );
    }

    #[test]
    fn another_apps_version_flag_is_not_this_apps() {
        assert_eq!(request(&["--home-version"]), Ok(None));
    }
}
