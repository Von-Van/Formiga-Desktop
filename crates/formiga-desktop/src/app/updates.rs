//! The release check and update download, run on short-lived workers that report back through
//! `UserEvent`, and what the settings window shows about them.
use super::*;

impl FormigaApp {
    /// A check the owner asked for, or the one as Formiga starts: a release it finds opens the
    /// About page.
    pub(super) fn start_update_check(&mut self) {
        self.begin_update_check(true);
    }

    /// The daily check while Formiga runs, when one is due. A release it finds is shown in the tray
    /// and on the About page, without opening the notebook over whatever the owner is doing.
    pub(super) fn check_for_updates_when_due(&mut self) {
        if self
            .updates
            .should_check_automatically(OffsetDateTime::now_utc())
        {
            self.begin_update_check(false);
        }
    }

    fn begin_update_check(&mut self, reveal: bool) {
        if !self.updates.begin_check(reveal, OffsetDateTime::now_utc()) {
            return;
        }
        self.sync_update_ui();
        // Each installed companion app with downloads of its own is looked up in the same check,
        // after Formiga itself, so the owner hears when one has fallen behind.
        let companions: Vec<_> = companions::Companion::ALL
            .into_iter()
            .map(|app| self.slot(app))
            .filter(|slot| slot.install.is_some())
            .filter_map(|slot| Some((slot.expansion.name, slot.expansion.releases?)))
            .collect();
        let proxy = self.event_proxy.clone();
        std::thread::Builder::new()
            .name("formiga-update-check".into())
            .spawn(move || {
                let result = check_github().map_err(|error| error.to_string());
                let _ = proxy.send_event(UserEvent::Update(UpdateEvent::CheckFinished(result)));
                for (name, releases) in companions {
                    let result = check_companion(name, releases).map_err(|error| error.to_string());
                    let _ = proxy.send_event(UserEvent::Update(UpdateEvent::CompanionChecked(
                        name, result,
                    )));
                }
            })
            .expect("spawn update-check worker");
    }

    pub(super) fn start_update_download(&mut self) {
        let Some((release, directory)) = self.updates.begin_download() else {
            return;
        };
        self.sync_update_ui();
        let proxy = self.event_proxy.clone();
        std::thread::Builder::new()
            .name("formiga-update-download".into())
            .spawn(move || {
                let result = download_update(release, directory).map_err(|error| error.to_string());
                let _ = proxy.send_event(UserEvent::Update(UpdateEvent::DownloadFinished(result)));
            })
            .expect("spawn update-download worker");
    }

    pub(super) fn handle_update_event(&mut self, event_loop: &ActiveEventLoop, event: UpdateEvent) {
        let reveal = match event {
            UpdateEvent::CompanionChecked(name, result) => {
                match result {
                    Ok(Some(release)) => {
                        self.companion_releases.insert(name, release);
                    }
                    Ok(None) => {
                        self.companion_releases.remove(name);
                    }
                    Err(error) => tracing::warn!(%error, "could not look up {name}'s releases"),
                }
                self.sync_update_ui();
                return;
            }
            UpdateEvent::CheckFinished(result) => {
                self.updates.finish_check(result, OffsetDateTime::now_utc())
            }
            UpdateEvent::DownloadFinished(result) => {
                self.updates.finish_download(result);
                true
            }
        };
        if let UpdateStatus::Failed(error) = self.updates.status() {
            tracing::warn!(%error, "update operation failed");
        }
        self.sync_update_ui();
        if reveal {
            self.show_update_settings(event_loop);
        }
    }

    pub(super) fn show_update_settings(&mut self, event_loop: &ActiveEventLoop) {
        self.show_settings(event_loop);
        if let Some(window) = &mut self.settings_window {
            window.select_about();
        }
    }

    pub(super) fn sync_update_ui(&mut self) {
        if let Some(tray) = &self.tray {
            tray.sync_update(self.updates.status());
        }
        if let Some(window) = &self.settings_window {
            window.window.request_redraw();
        }
    }
}
