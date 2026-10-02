//! A colony that could not be read, or cannot be saved: the cards that say so above every page,
//! and the About page's way to restore a backup in its place.

use super::*;

/// What the notebook knows about trouble with the colony's files. Never saved.
#[derive(Default)]
pub(crate) struct RecoveryState {
    /// Why the colony saved last time could not be read, while the colony on the desktop is a
    /// temporary one waiting to be told what to do.
    pub reason: Option<String>,
    /// Why the colony cannot be saved, while it cannot. Set by the app before every frame.
    pub save_trouble: Option<String>,
    /// Whether starting fresh, with the unreadable files kept as recovery copies, is ticked.
    pub fresh_confirmed: bool,
    /// Whether restoring a backup in place of this colony is ticked, on the About page.
    pub restore_confirmed: bool,
}

impl RecoveryState {
    /// The cards above every page while there is trouble with the colony's files.
    pub(crate) fn cards(
        &mut self,
        ui: &mut Ui,
        save_location: &str,
        outcome: &mut SettingsOutcome,
    ) {
        if let Some(reason) = self.reason.clone() {
            card(ui, |ui| {
                journal::kicker(ui, "Your saved colony needs attention");
                ui.heading("Nothing has been lost");
                ui.label(format!(
                    "Formiga could not read the colony it saved last time: {reason}"
                ));
                ui.label("Those files are kept exactly as they were, and nothing will be written over them. The colony on your desktop for now is a temporary one, and it will not be saved until you choose what to do.");
                ui.add_space(4.0);
                ui.strong("Choose one");
                ui.small("Restore a backup you exported earlier, or a copy of colony.json from another computer:");
                if ui.button("Restore a colony backup…").clicked() {
                    outcome.restore_colony = true;
                }
                ui.add_space(4.0);
                ui.small("Or keep the unreadable files as recovery copies beside the colony, and carry on with this new one:");
                ui.checkbox(
                    &mut self.fresh_confirmed,
                    "Keep recovery copies and start a new colony",
                );
                if ui
                    .add_enabled(
                        self.fresh_confirmed,
                        egui::Button::new("Start fresh with recovery copies"),
                    )
                    .clicked()
                {
                    outcome.start_fresh_recovery = true;
                }
                ui.horizontal_wrapped(|ui| {
                    ui.small(format!("The files are in {save_location}"));
                    if ui.small_button("Open diagnostic logs").clicked() {
                        outcome.open_logs = true;
                    }
                });
            });
            ui.add_space(16.0);
        }
        if let Some(trouble) = self.save_trouble.clone() {
            card(ui, |ui| {
                journal::kicker(ui, "Saving has stopped for now");
                ui.label(trouble);
                ui.label("Your colony carries on as usual, and the last good save and its backup are untouched. Formiga keeps trying every few seconds and will say so here when it works again.");
                ui.horizontal_wrapped(|ui| {
                    if ui.button("Try again now").clicked() {
                        outcome.retry_save = true;
                    }
                    if ui
                        .button("Export a backup elsewhere…")
                        .on_hover_text("Write the colony as it is now to another folder or drive.")
                        .clicked()
                    {
                        outcome.export_colony = true;
                    }
                    if ui.small_button("Open diagnostic logs").clicked() {
                        outcome.open_logs = true;
                    }
                });
            });
            ui.add_space(16.0);
        }
    }

    /// The About page's backups: export the whole colony, or restore one in place of it once
    /// that has been ticked.
    pub(crate) fn backups(&mut self, ui: &mut Ui, outcome: &mut SettingsOutcome) {
        wide_card(ui, |ui| {
            ui.strong("Colony backups");
            ui.label("A full backup includes names, memories, relationships, the journal, and preferences. Keep it private or move it to another computer.");
            if ui.button("Export full colony…").clicked() {
                outcome.export_colony = true;
            }
            ui.checkbox(
                &mut self.restore_confirmed,
                "Restore a backup in place of this colony; keep recovery copies first",
            );
            if ui
                .add_enabled(
                    self.restore_confirmed,
                    egui::Button::new("Choose backup to restore…"),
                )
                .clicked()
            {
                outcome.restore_colony = true;
            }
        });
    }
}
