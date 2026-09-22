//! The release check and update download, run on short-lived workers that report back through
//! `UserEvent`, and what the settings window shows about them.
use super::*;

impl FormigaApp {
    pub(super) fn start_update_check(&mut self) {
        if !self.updates.begin_check() {
            return;
        }
        self.sync_update_ui();
        let proxy = self.event_proxy.clone();
        std::thread::Builder::new()
            .name("formiga-update-check".into())
            .spawn(move || {
                let result = check_github().map_err(|error| error.to_string());
                let _ = proxy.send_event(UserEvent::Update(UpdateEvent::CheckFinished(result)));
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
            UpdateEvent::CheckFinished(result) => self.updates.finish_check(result),
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
