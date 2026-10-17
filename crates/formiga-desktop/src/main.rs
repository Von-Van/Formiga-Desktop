#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod app;
#[cfg(test)]
mod attention_review;
mod battle;
mod card_export;
mod clubhouse;
mod creature_menu;
#[cfg(test)]
mod desktop_ui_review;
mod expansion;
mod explain;
mod farm;
mod gpu;
mod hill;
mod house;
mod houses;
mod interaction;
mod notebook_window;
mod notices;
mod platform;
mod reference_match;
#[cfg(test)]
mod review_sheet;
mod settings;
mod sticker_export;
mod tray;
mod trip;
mod updater;

use anyhow::Result;
use app::{FormigaApp, UserEvent};
use directories::ProjectDirs;
use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tray_icon::menu::MenuEvent;
use winit::event_loop::EventLoop;

fn main() -> Result<()> {
    // Held, unread, for as long as this copy runs.
    let Some(_colony_held) = hold_colony(&data_dir()?)? else {
        already_running();
        return Ok(());
    };
    let log_dir = initialize_diagnostics()?;

    let mut builder = EventLoop::<UserEvent>::with_user_event();
    #[cfg(target_os = "macos")]
    {
        use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};
        builder
            .with_activation_policy(ActivationPolicy::Accessory)
            .with_activate_ignoring_other_apps(false)
            .with_default_menu(false);
    }
    let event_loop = builder.build()?;
    let proxy = event_loop.create_proxy();
    let menu_proxy = proxy.clone();
    MenuEvent::set_event_handler(Some(move |event| {
        let _ = menu_proxy.send_event(UserEvent::Menu(event));
    }));
    let hover_proxy = proxy.clone();
    tray_icon::TrayIconEvent::set_event_handler(Some(move |event| {
        if matches!(event, tray_icon::TrayIconEvent::Enter { .. }) {
            let _ = hover_proxy.send_event(UserEvent::TrayHover);
        }
    }));
    let mut app = FormigaApp::new(log_dir, proxy)?;
    event_loop.run_app(&mut app)?;
    Ok(())
}

/// Where the colony, its logs, and the update preferences live. Setting `FORMIGA_DATA_DIR` points
/// a development build at a scratch colony, so trying a change never touches the real one.
pub(crate) fn data_dir() -> Result<PathBuf> {
    if let Some(dir) = std::env::var_os("FORMIGA_DATA_DIR").filter(|dir| !dir.is_empty()) {
        return Ok(PathBuf::from(dir));
    }
    let project = ProjectDirs::from("com", "Formiga", "Formiga")
        .ok_or_else(|| anyhow::anyhow!("resolve application data directory"))?;
    Ok(project.data_dir().to_path_buf())
}

/// Takes the colony's data folder for this copy of Formiga, or `None` when another copy already has
/// it. Two copies on one colony would each save over the other's, and each take the other's open
/// trip, visit or match for one a crash left and close it under the app it is in. The lock goes
/// with the process, however it ends. A folder that cannot be locked at all is run as before.
fn hold_colony(data_dir: &Path) -> Result<Option<File>> {
    fs::create_dir_all(data_dir)?;
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(data_dir.join("formiga.lock"))?;
    match file.try_lock() {
        Ok(()) => Ok(Some(file)),
        Err(fs::TryLockError::WouldBlock) => Ok(None),
        Err(fs::TryLockError::Error(error)) => {
            eprintln!("Formiga could not lock its data folder, so runs without the lock: {error}");
            Ok(Some(file))
        }
    }
}

/// Said when Formiga is started while it is already running. On Windows, where a click on the
/// Start menu can start a second copy beside the one started at sign-in, a dialog says where it
/// is; on macOS, where opening the app again only brings the running copy forward, a second copy
/// can only come from starting the program directly, and it leaves quietly.
fn already_running() {
    #[cfg(target_os = "windows")]
    rfd::MessageDialog::new()
        .set_level(rfd::MessageLevel::Info)
        .set_title("Formiga is already running")
        .set_description(
            "Your colony is already out on the desktop. Formiga's icon is in the notification \
             area at the end of the taskbar; it may be under the arrow that shows hidden icons.",
        )
        .set_buttons(rfd::MessageButtons::Ok)
        .show();
    #[cfg(not(target_os = "windows"))]
    eprintln!("Formiga is already running for this colony.");
}

fn initialize_diagnostics() -> Result<PathBuf> {
    let log_dir = data_dir()?.join("logs");
    fs::create_dir_all(&log_dir)?;
    let current = log_dir.join("formiga.log");
    if current
        .metadata()
        .is_ok_and(|metadata| metadata.len() > 1_000_000)
    {
        let _ = fs::rename(&current, log_dir.join("formiga.previous.log"));
    }
    let file = OpenOptions::new().create(true).append(true).open(current)?;
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("formiga=info".parse()?),
        )
        .with_ansi(false)
        .with_writer(Mutex::new(file))
        .init();
    Ok(log_dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_second_copy_finds_the_colony_held_until_the_first_lets_go() {
        let directory =
            std::env::temp_dir().join(format!("formiga-one-copy-{}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);

        let first = hold_colony(&directory).unwrap();
        assert!(first.is_some());
        assert!(hold_colony(&directory).unwrap().is_none());
        drop(first);
        assert!(hold_colony(&directory).unwrap().is_some());
        fs::remove_dir_all(&directory).unwrap();
    }
}
