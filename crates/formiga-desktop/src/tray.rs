use crate::updater::UpdateStatus;
use anyhow::Result;
use formiga_core::Settings;
use std::time::Instant;
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

pub struct TrayState {
    tray: TrayIcon,
    /// The menu itself, kept to add and remove each companion app's item as it comes and goes.
    menu: Menu,
    /// One item for each companion app, in the order the app lists them, each in the menu only
    /// while it offers something.
    visits: Vec<VisitItem>,
    /// Where the colony is while it is away from the desktop.
    colony_at: Option<&'static str>,
    pub about: MenuItem,
    /// Whether the icon carries its small dot for something new in the journal.
    news: bool,
    /// Whether the tooltip is saying the colony could not be saved.
    trouble: bool,
    pub visible: CheckMenuItem,
    pub paused: CheckMenuItem,
    pub settings: MenuItem,
    pub gather: MenuItem,
    quiet: MenuItem,
    quiet_active: bool,
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
    QuietMoment,
    CheckForUpdates,
    OpenAbout,
    /// The item of the companion app at this place in the app's list.
    Visit(usize),
    None,
}

/// What one companion app's item says, and whether it can be chosen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VisitOffer {
    pub text: String,
    pub enabled: bool,
}

/// A companion app's item, and what it offers while it is in the menu.
struct VisitItem {
    item: MenuItem,
    offer: Option<VisitOffer>,
}

/// Where the companion apps' items sit in the menu: straight after the everyday colony actions.
const VISITS_POSITION: usize = 5;

impl TrayState {
    /// A tray with room for `visits` companion apps' items.
    pub fn new(settings: &Settings, visits: usize) -> Result<Self> {
        let visible = CheckMenuItem::new("Show colony", true, settings.visible, None);
        let paused = CheckMenuItem::new("Pause colony", true, settings.paused, None);
        let settings_item = MenuItem::new("Your colony…", true, None);
        let quiet = MenuItem::new("Settle at home for 30 minutes", true, None);
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
        let about = MenuItem::new("About Formiga", true, None);
        let quit = MenuItem::new("Quit Formiga", true, None);
        let separator_a = PredefinedMenuItem::separator();
        let separator_b = PredefinedMenuItem::separator();
        let separator_c = PredefinedMenuItem::separator();
        let preferences = Submenu::with_items(
            "Preferences",
            true,
            &[
                &window_ledges,
                &cursor_reactions,
                &reduce_motion,
                &launch_at_login,
                &separator_b,
                &scale_2,
                &scale_3,
                &scale_4,
            ],
        )?;
        let more = Submenu::with_items(
            "More",
            true,
            &[&about, &check_updates, &open_logs, &separator_c, &reset],
        )?;
        let menu = Menu::with_items(&[
            &settings_item,
            &visible,
            &paused,
            &gather,
            &quiet,
            &separator_a,
            &preferences,
            &more,
            &quit,
        ])?;
        let tray = TrayIconBuilder::new()
            .with_tooltip(TOOLTIP)
            .with_icon(icon(false)?)
            .with_menu(Box::new(menu.clone()))
            .build()?;
        Ok(Self {
            tray,
            menu,
            visits: (0..visits)
                .map(|_| VisitItem {
                    item: MenuItem::new("", true, None),
                    offer: None,
                })
                .collect(),
            colony_at: None,
            about,
            news: false,
            trouble: false,
            visible,
            paused,
            settings: settings_item,
            gather,
            quiet,
            quiet_active: false,
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
        if event.id() == self.about.id() {
            return TrayAction::OpenAbout;
        }
        if event.id() == self.settings.id() {
            return TrayAction::OpenSettings;
        }
        if event.id() == self.quiet.id() {
            return TrayAction::QuietMoment;
        }
        if event.id() == self.gather.id() {
            return TrayAction::GatherCreatures;
        }
        if let Some(slot) = self
            .visits
            .iter()
            .position(|visit| event.id() == visit.item.id())
        {
            let offered = self.visits[slot]
                .offer
                .as_ref()
                .is_some_and(|offer| offer.enabled);
            return if offered {
                TrayAction::Visit(slot)
            } else {
                TrayAction::None
            };
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

    /// Show what the companion app at `slot` in the app's list offers, or withdraw its item. The
    /// items keep the app's order, straight after the everyday colony actions.
    pub fn sync_visit(&mut self, slot: usize, offer: Option<VisitOffer>) {
        let Some(visit) = self.visits.get(slot) else {
            return;
        };
        if visit.offer == offer {
            return;
        }
        let (shown, showing) = (visit.offer.is_some(), offer.is_some());
        if showing && !shown {
            let position = VISITS_POSITION
                + self.visits[..slot]
                    .iter()
                    .filter(|visit| visit.offer.is_some())
                    .count();
            if let Err(error) = self.menu.insert(&visit.item, position) {
                tracing::warn!(%error, "could not offer a companion app in the tray");
                return;
            }
        } else if shown
            && !showing
            && let Err(error) = self.menu.remove(&visit.item)
        {
            tracing::warn!(%error, "could not withdraw a companion app from the tray");
        }
        if let Some(offer) = &offer {
            visit.item.set_text(&offer.text);
            visit.item.set_enabled(offer.enabled);
        }
        self.visits[slot].offer = offer;
    }

    /// While the colony is away from the desktop, at the app `at` names, the actions that move it
    /// about the desktop wait for it to come back, and the tooltip says where it is.
    pub fn sync_colony_away(&mut self, at: Option<&'static str>) {
        if at == self.colony_at {
            return;
        }
        self.colony_at = at;
        let home = at.is_none();
        for item in [&self.gather, &self.quiet] {
            item.set_enabled(home);
        }
        self.paused.set_enabled(home);
        self.reset.set_enabled(home);
        if !self.trouble
            && let Err(error) = self.tray.set_tooltip(Some(self.resting_tooltip()))
        {
            tracing::warn!(%error, "could not update the tray tooltip");
        }
    }

    /// The tooltip while nothing is wrong: where the colony is.
    fn resting_tooltip(&self) -> String {
        match self.colony_at {
            None => TOOLTIP.to_owned(),
            Some(at) => format!("Formiga · the colony is at {at}"),
        }
    }

    pub fn sync_quiet(&mut self, active: bool) {
        if active != self.quiet_active {
            self.quiet_active = active;
            self.quiet.set_text(if active {
                "End quiet moment"
            } else {
                "Settle at home for 30 minutes"
            });
        }
    }

    /// A small dot on the icon while something noteworthy in the journal has not been read: no
    /// sound, no badge count, nothing on the desktop. It is only redrawn when it changes.
    pub fn sync_news(&mut self, news: bool) {
        if news == self.news {
            return;
        }
        self.news = news;
        match icon(news) {
            Ok(icon) => {
                if let Err(error) = self.tray.set_icon(Some(icon)) {
                    tracing::warn!(%error, "could not update the tray icon");
                }
            }
            Err(error) => tracing::warn!(%error, "could not draw the tray icon"),
        }
    }

    /// Say in the icon's tooltip that the colony could not be saved, for as long as that lasts.
    pub fn sync_trouble(&mut self, trouble: bool) {
        if trouble == self.trouble {
            return;
        }
        self.trouble = trouble;
        let text = if trouble {
            "Formiga · could not save the colony just now; open the notebook for details".to_owned()
        } else {
            self.resting_tooltip()
        };
        if let Err(error) = self.tray.set_tooltip(Some(text)) {
            tracing::warn!(%error, "could not update the tray tooltip");
        }
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

const TOOLTIP: &str = "Formiga desktop ecosystem";

/// The app's own icon, shrunk for the menu bar and the notification area by `formiga-tools
/// app-icon` from the same picture the Dock, the Finder and the Start menu show.
const TRAY_PNG: &[u8] = include_bytes!("../../../packaging/shared/Formiga-tray.png");
const TRAY_SIZE: usize = 64;

fn icon(news: bool) -> Result<Icon> {
    Ok(Icon::from_rgba(
        icon_pixels(news)?,
        TRAY_SIZE as u32,
        TRAY_SIZE as u32,
    )?)
}

/// The icon's pixels: the app icon, and with `news` a small ink dot ringed in cream at its upper
/// right.
fn icon_pixels(news: bool) -> Result<Vec<u8>> {
    let image = image::load_from_memory(TRAY_PNG)?.to_rgba8();
    anyhow::ensure!(
        image.width() as usize == TRAY_SIZE && image.height() as usize == TRAY_SIZE,
        "the tray icon is {}x{}, not {TRAY_SIZE}x{TRAY_SIZE}",
        image.width(),
        image.height()
    );
    let mut rgba = image.into_raw();
    if news {
        for y in 0..20_i32 {
            for x in 44..64_i32 {
                let (dx, dy) = (x - 54, y - 10);
                let distance = dx * dx + dy * dy;
                let color = if distance <= 36 {
                    [196, 88, 64, 255]
                } else if distance <= 68 {
                    [255, 250, 234, 255]
                } else {
                    continue;
                };
                let index = (y as usize * TRAY_SIZE + x as usize) * 4;
                rgba[index..index + 4].copy_from_slice(&color);
            }
        }
    }
    Ok(rgba)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_news_dot_changes_only_its_corner_of_the_icon() {
        let (plain, dotted) = (icon_pixels(false).unwrap(), icon_pixels(true).unwrap());
        assert_eq!(plain.len(), TRAY_SIZE * TRAY_SIZE * 4);
        let mut changed = 0;
        for (index, (a, b)) in plain.chunks(4).zip(dotted.chunks(4)).enumerate() {
            if a != b {
                changed += 1;
                let (x, y) = (index % TRAY_SIZE, index / TRAY_SIZE);
                assert!(x >= 44 && y < 20, "the dot reached ({x}, {y})");
            }
        }
        assert!(
            changed > 120,
            "the dot is big enough to see: {changed} pixels"
        );
    }
}
