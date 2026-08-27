use crate::updater::UpdateStatus;
use anyhow::Result;
use formiga_core::Settings;
use std::time::Instant;
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

pub struct TrayState {
    _tray: TrayIcon,
    pub visible: CheckMenuItem,
    pub paused: CheckMenuItem,
    pub settings: MenuItem,
    pub gather: MenuItem,
    pub window_ledges: CheckMenuItem,
    pub cursor_reactions: CheckMenuItem,
    pub launch_at_login: CheckMenuItem,
    pub reduce_motion: CheckMenuItem,
    pub scale_2: CheckMenuItem,
    pub scale_3: CheckMenuItem,
    pub scale_4: CheckMenuItem,
    pub reset: MenuItem,
    pub open_logs: MenuItem,
    pub check_updates: MenuItem,
    pub quit: MenuItem,
    reset_armed_at: Option<Instant>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrayAction {
    SettingsChanged,
    ResetColony,
    Quit,
    OpenLogs,
    OpenSettings,
    GatherCreatures,
    CheckForUpdates,
    None,
}

impl TrayState {
    pub fn new(settings: &Settings) -> Result<Self> {
        let visible = CheckMenuItem::new("Show ecosystem", true, settings.visible, None);
        let paused = CheckMenuItem::new("Pause ecosystem", true, settings.paused, None);
        let settings_item = MenuItem::new("Settings…", true, None);
        let gather = MenuItem::new("Gather creatures", true, None);
        let window_ledges =
            CheckMenuItem::new("Use window ledges", true, settings.window_ledges, None);
        let cursor_reactions =
            CheckMenuItem::new("React to cursor", true, settings.cursor_reactions, None);
        let launch_at_login =
            CheckMenuItem::new("Launch at login", true, settings.launch_at_login, None);
        let reduce_motion = CheckMenuItem::new("Reduce motion", true, settings.reduce_motion, None);
        let scale_2 = CheckMenuItem::new("Small (2x)", true, settings.display_scale == 2, None);
        let scale_3 = CheckMenuItem::new("Medium (3x)", true, settings.display_scale == 3, None);
        let scale_4 = CheckMenuItem::new("Large (4x)", true, settings.display_scale == 4, None);
        let reset = MenuItem::new("Start a new colony…", true, None);
        let open_logs = MenuItem::new("Open diagnostic logs", true, None);
        let check_updates = MenuItem::new("Check for updates…", true, None);
        let quit = MenuItem::new("Quit Formiga", true, None);
        let separator_a = PredefinedMenuItem::separator();
        let separator_b = PredefinedMenuItem::separator();
        let separator_c = PredefinedMenuItem::separator();
        let menu = Menu::with_items(&[
            &visible,
            &paused,
            &settings_item,
            &gather,
            &separator_a,
            &window_ledges,
            &cursor_reactions,
            &reduce_motion,
            &launch_at_login,
            &separator_b,
            &scale_2,
            &scale_3,
            &scale_4,
            &separator_c,
            &reset,
            &open_logs,
            &check_updates,
            &quit,
        ])?;
        let tray = TrayIconBuilder::new()
            .with_tooltip("Formiga desktop ecosystem")
            .with_icon(icon()?)
            .with_menu(Box::new(menu))
            .build()?;
        Ok(Self {
            _tray: tray,
            visible,
            paused,
            settings: settings_item,
            gather,
            window_ledges,
            cursor_reactions,
            launch_at_login,
            reduce_motion,
            scale_2,
            scale_3,
            scale_4,
            reset,
            open_logs,
            check_updates,
            quit,
            reset_armed_at: None,
        })
    }

    pub fn handle(&mut self, event: &MenuEvent, settings: &mut Settings) -> TrayAction {
        if event.id() == self.quit.id() {
            return TrayAction::Quit;
        }
        if event.id() == self.open_logs.id() {
            return TrayAction::OpenLogs;
        }
        if event.id() == self.check_updates.id() {
            return TrayAction::CheckForUpdates;
        }
        if event.id() == self.settings.id() {
            return TrayAction::OpenSettings;
        }
        if event.id() == self.gather.id() {
            return TrayAction::GatherCreatures;
        }
        if event.id() == self.reset.id() {
            let confirmed = self
                .reset_armed_at
                .is_some_and(|armed| armed.elapsed().as_secs() <= 10);
            if confirmed {
                self.reset_armed_at = None;
                self.reset.set_text("Start a new colony…");
                return TrayAction::ResetColony;
            }
            self.reset_armed_at = Some(Instant::now());
            self.reset.set_text("Click again within 10s to confirm");
            return TrayAction::None;
        }
        if event.id() == self.visible.id() {
            settings.visible = !settings.visible;
            self.visible.set_checked(settings.visible);
        } else if event.id() == self.paused.id() {
            settings.paused = !settings.paused;
            self.paused.set_checked(settings.paused);
        } else if event.id() == self.window_ledges.id() {
            settings.window_ledges = !settings.window_ledges;
            self.window_ledges.set_checked(settings.window_ledges);
        } else if event.id() == self.cursor_reactions.id() {
            settings.cursor_reactions = !settings.cursor_reactions;
            self.cursor_reactions.set_checked(settings.cursor_reactions);
        } else if event.id() == self.launch_at_login.id() {
            settings.launch_at_login = !settings.launch_at_login;
            self.launch_at_login.set_checked(settings.launch_at_login);
        } else if event.id() == self.reduce_motion.id() {
            settings.reduce_motion = !settings.reduce_motion;
            self.reduce_motion.set_checked(settings.reduce_motion);
        } else if event.id() == self.scale_2.id() {
            self.set_scale(settings, 2);
        } else if event.id() == self.scale_3.id() {
            self.set_scale(settings, 3);
        } else if event.id() == self.scale_4.id() {
            self.set_scale(settings, 4);
        } else {
            return TrayAction::None;
        }
        TrayAction::SettingsChanged
    }

    fn set_scale(&self, settings: &mut Settings, scale: u8) {
        settings.display_scale = scale;
        self.scale_2.set_checked(scale == 2);
        self.scale_3.set_checked(scale == 3);
        self.scale_4.set_checked(scale == 4);
    }

    pub fn sync(&self, settings: &Settings) {
        self.visible.set_checked(settings.visible);
        self.paused.set_checked(settings.paused);
        self.window_ledges.set_checked(settings.window_ledges);
        self.cursor_reactions.set_checked(settings.cursor_reactions);
        self.launch_at_login.set_checked(settings.launch_at_login);
        self.reduce_motion.set_checked(settings.reduce_motion);
        self.scale_2.set_checked(settings.display_scale == 2);
        self.scale_3.set_checked(settings.display_scale == 3);
        self.scale_4.set_checked(settings.display_scale == 4);
    }

    pub fn sync_update(&self, status: &UpdateStatus) {
        match status {
            UpdateStatus::Checking => {
                self.check_updates.set_text("Checking for updates…");
                self.check_updates.set_enabled(false);
            }
            UpdateStatus::Available(release) => {
                self.check_updates
                    .set_text(format!("Update available: {}…", release.version));
                self.check_updates.set_enabled(true);
            }
            UpdateStatus::Downloading(release) => {
                self.check_updates
                    .set_text(format!("Downloading {}…", release.version));
                self.check_updates.set_enabled(false);
            }
            UpdateStatus::Ready(downloaded) => {
                self.check_updates.set_text(format!(
                    "Install downloaded update {}…",
                    downloaded.release.version
                ));
                self.check_updates.set_enabled(true);
            }
            _ => {
                self.check_updates.set_text("Check for updates…");
                self.check_updates.set_enabled(true);
            }
        }
    }
}

fn icon() -> Result<Icon> {
    let size = 32;
    let mut rgba = vec![0_u8; size * size * 4];
    for y in 5..27 {
        for x in 4..28 {
            let dx = x as i32 - 16;
            let dy = y as i32 - 16;
            if dx * dx * 4 + dy * dy * 5 <= 23 * 23 * 4 {
                let index = (y * size + x) * 4;
                rgba[index..index + 4].copy_from_slice(&[112, 196, 155, 255]);
            }
        }
    }
    for (x, y) in [(11, 14), (21, 14)] {
        for oy in 0..5 {
            for ox in 0..4 {
                let index = ((y + oy) * size + x + ox) * 4;
                rgba[index..index + 4].copy_from_slice(&[22, 34, 29, 255]);
            }
        }
        let index = (y * size + x) * 4;
        rgba[index..index + 4].copy_from_slice(&[255, 255, 240, 255]);
    }
    Ok(Icon::from_rgba(rgba, size as u32, size as u32)?)
}
