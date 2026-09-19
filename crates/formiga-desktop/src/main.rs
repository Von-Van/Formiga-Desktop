#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod app;
#[cfg(test)]
mod attention_review;
mod card_export;
mod clubhouse;
mod creature_menu;
#[cfg(test)]
mod desktop_ui_review;
mod gpu;
mod interaction;
mod platform;
mod reference_match;
mod settings;
mod sticker_export;
mod tray;
mod updater;

use anyhow::Result;
use app::{FormigaApp, UserEvent};
use directories::ProjectDirs;
use std::fs::{self, OpenOptions};
use std::path::PathBuf;
use std::sync::Mutex;
use tray_icon::menu::MenuEvent;
use winit::event_loop::EventLoop;

fn main() -> Result<()> {
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
