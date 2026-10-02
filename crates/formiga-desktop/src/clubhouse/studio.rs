//! The Creature studio page: sketches of companions not yet met, drawn from a seed, a shared
//! code or a picture, and the choice to bring one home.

use super::shell::Shell;
use super::*;

struct Candidate {
    preview: GenerationPreview,
    texture: TextureHandle,
    /// The row of its frames it stands on, so the large preview can stand it beside the tree.
    floor: u32,
}

/// The studio's own state: the sketches on show and what is being asked of the next ones. Never
/// saved.
#[derive(Default)]
pub(crate) struct StudioState {
    candidates: Vec<Candidate>,
    pub(crate) selected: usize,
    pub lock_colors: bool,
    pub lock_body: bool,
    pub animate: bool,
    pub seed_code: String,
    pub replace_confirmed: bool,
    /// The picture the takes on show were read from, kept only while they are on show so that
    /// "Four more takes" can read it again. Only the path is kept, never the picture, and it is
    /// never saved.
    pub reference: Option<std::path::PathBuf>,
    /// The village tree the large preview stands a companion beside, uploaded once.
    tree: Option<TextureHandle>,
}

impl StudioState {
    pub(super) fn texture_ids(&self) -> impl Iterator<Item = egui::TextureId> + '_ {
        self.candidates
            .iter()
            .map(|c| c.texture.id())
            .chain(self.tree.iter().map(|t| t.id()))
    }

    pub(super) fn release_images(&mut self) {
        self.clear_previews();
        self.tree = None;
    }

    pub fn clear_previews(&mut self) {
        self.candidates.clear();
        self.reference = None;
        self.selected = 0;
        self.replace_confirmed = false;
    }
    pub fn locks(&self) -> (Option<CreatureDesign>, bool, bool) {
        (
            self.candidates
                .get(self.selected)
                .and_then(|c| c.preview.creature.appearance.design),
            self.lock_colors,
            self.lock_body,
        )
    }
    pub fn push_preview(&mut self, context: &egui::Context, preview: GenerationPreview) {
        // Eight tiny pre-baked frames, not a full desktop animation atlas.
        let mut strip = Canvas::new(48 * 8, 48);
        let mut tiles = Vec::with_capacity(8);
        for frame in 0..8 {
            let (action, index) = if frame < 6 {
                (ActionKind::Traverse, frame)
            } else {
                (ActionKind::Greet, frame - 6)
            };
            let tile = CreatureRenderer::render_studio_frame(
                &preview.creature.appearance,
                action,
                index as u8,
            );
            for y in 0..48 {
                for x in 0..48 {
                    strip.set(frame * 48 + x, y, tile.get(x, y));
                }
            }
            tiles.push(tile);
        }
        let texture = upload(context, "studio-candidate", &strip);
        // Every frame stands on the ground the first one stands on, so a step or a wave lifts the
        // companion off it the way it does on the desktop.
        let floor = formiga_art::standing_row(&tiles[0]);
        if self.candidates.len() == 4 {
            self.candidates.remove(0);
        }
        self.candidates.push(Candidate {
            preview,
            texture,
            floor,
        });
        self.selected = self.candidates.len() - 1;
        self.replace_confirmed = false;
    }
    pub fn adoption_footer(
        &self,
        ui: &mut Ui,
        save: &SaveFile,
        outcome: &mut SettingsOutcome,
    ) -> bool {
        let Some(candidate) = self.candidates.get(self.selected) else {
            return false;
        };
        let adults = save.creatures.iter().filter(|c| c.role.is_adult()).count();
        let duplicate = save
            .creatures
            .iter()
            .any(|c| c.id == candidate.preview.creature.id);
        let can_add = save.creatures.len() < MAX_COLONY_CREATURES
            && adults < MAX_ADULT_CREATURES
            && !duplicate;
        ui.horizontal(|ui| {
            ui.label(if duplicate {
                "This companion already lives here".into()
            } else {
                format!(
                    "{} / {MAX_COLONY_CREATURES} companions",
                    save.creatures.len()
                )
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add_enabled(can_add, egui::Button::new("Adopt into colony").fill(mint()))
                    .clicked()
                {
                    outcome.accept_creature_preview = Some(match candidate.preview.shared {
                        Some(shared) => PreviewAcceptance::Shared {
                            shared,
                            replace: None,
                        },
                        None => PreviewAcceptance::Add {
                            source_seed: candidate.preview.source_seed,
                            design: candidate.preview.creature.appearance.design,
                        },
                    });
                }
            });
        });
        true
    }

    /// The Creature studio page.
    pub fn show(
        &mut self,
        ui: &mut Ui,
        shell: &mut Shell,
        save: &SaveFile,
        selected_creature: &mut Option<CreatureId>,
        outcome: &mut SettingsOutcome,
    ) {
        journal::page_heading(
            ui,
            crate::settings::SettingsTab::Studio,
            "Creature studio",
            "Sketches of faces not yet met. Some of them may come to stay.",
        );
        let discover = ui.scope(|ui| {
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add(egui::Button::new("Discover four companions").fill(mint()))
                    .clicked()
                {
                    outcome.request_random_creature = true;
                }
                if ui.button("Create from image…").clicked() {
                    outcome.request_reference_creature = true;
                }
            });
            ui.horizontal_wrapped(|ui| {
                ui.checkbox(&mut self.lock_colors, "Keep selected colors");
                ui.checkbox(&mut self.lock_body, "Keep selected body");
            });
            ui.small(
                "Locks guide the next random set. A picture is read on this computer for its \
                 colors and shape, and becomes four companions of their own, never a copy of it.",
            );
        });
        shell.tour_mark(ui, tour::TourMark::Discover, discover.response.rect);
        ui.add_space(16.0);
        egui::CollapsingHeader::new("Adopt a shared companion from a code")
            .default_open(!self.seed_code.trim().is_empty())
            .show(ui, |ui| {
                ui.label(
                    "Paste a creature code to preview it. From the preview you can adopt them, \
                     or ask them over for a day. Your colony stays together either way.",
                );
                ui.add(
                    egui::TextEdit::singleline(&mut self.seed_code)
                        .char_limit(256)
                        .hint_text("FORMIGA-…")
                        .desired_width(f32::INFINITY),
                );
                let decoded = decode_creature_seed(&self.seed_code);
                if !self.seed_code.trim().is_empty() {
                    match decoded {
                        Ok(shared) => {
                            if ui.button("Preview shared creature").clicked() {
                                outcome.preview_shared = Some(shared);
                            }
                        }
                        Err(error) => {
                            ui.add(
                                egui::Label::new(
                                    RichText::new(crate::explain::share_code(&error))
                                        .color(Color32::from_rgb(146, 61, 44)),
                                )
                                .wrap(),
                            );
                        }
                    }
                }
            });
        if self.candidates.is_empty() {
            ui.add_space(20.0);
            ui.label("A few new faces are waiting to be discovered.");
            return;
        }
        ui.add_space(18.0);
        if self.reference.is_some() && ui.button("Four more takes").clicked() {
            outcome.request_reference_retry = true;
        }
        let takes = self.candidates.len();
        journal::kicker(
            ui,
            &format!(
                "Sketches · {takes} {}",
                if takes == 1 { "take" } else { "takes" }
            ),
        );
        ui.horizontal_wrapped(|ui| {
            for (index, candidate) in self.candidates.iter().enumerate() {
                let label = format!("Take {}", index + 1);
                // As wide as the picture or its label, whichever is wider, with room for the
                // label's frame when it is pointed at or chosen.
                let width = (text_width(ui, label.as_str()) + 2.0 * ui.spacing().button_padding.x)
                    .max(84.0);
                make_room(ui, width);
                ui.vertical(|ui| {
                    let response = ui.add(
                        egui::Image::new(&candidate.texture)
                            .uv(egui::Rect::from_min_max(
                                egui::pos2(0.0, 0.0),
                                egui::pos2(0.125, 1.0),
                            ))
                            .maintain_aspect_ratio(false)
                            .fit_to_exact_size(egui::vec2(84.0, 84.0))
                            .sense(egui::Sense::click()),
                    );
                    if response.clicked()
                        || ui.selectable_label(self.selected == index, label).clicked()
                    {
                        self.selected = index;
                        self.replace_confirmed = false;
                    }
                });
            }
        });
        ui.separator();
        let tree = self
            .tree
            .get_or_insert_with(|| upload(ui.ctx(), "studio-tree", &formiga_art::reference_tree()))
            .id();
        let candidate = &self.candidates[self.selected];
        let animate = self.animate && !save.settings.reduce_motion;
        let frame = if animate {
            (ui.input(|i| i.time) * 6.0) as usize % 8
        } else {
            6
        };
        // Three screen pixels to an art pixel, as before, now standing beside the village tree
        // the review sheets use, so a small or a large companion reads as one. Beside its
        // details where there is room, and above them, smaller if it must be, where there is not.
        let scene = formiga_art::tree_scene(48, candidate.floor);
        let side_by_side = ui.available_width() >= scene.width as f32 * 3.0 + 20.0 + 240.0;
        let unit = if side_by_side {
            3.0
        } else {
            ((ui.available_width() - 20.0) / scene.width as f32).clamp(1.0, 3.0)
        };
        let preview = |ui: &mut Ui| {
            egui::Frame::new()
                .fill(forest())
                .stroke(egui::Stroke::new(2.0, gold()))
                .inner_margin(8)
                .show(ui, |ui| {
                    let (rect, _) = ui.allocate_exact_size(
                        egui::vec2(scene.width as f32 * unit, scene.height as f32 * unit),
                        egui::Sense::hover(),
                    );
                    let at = |(x, y): (i32, i32), size: f32| {
                        egui::Rect::from_min_size(
                            rect.min + egui::vec2(x as f32 * unit, y as f32 * unit),
                            egui::vec2(size * unit, size * unit),
                        )
                    };
                    let whole =
                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                    ui.painter().image(
                        tree,
                        at(scene.tree, formiga_art::TREE_CELL as f32),
                        whole,
                        Color32::WHITE,
                    );
                    ui.painter().image(
                        candidate.texture.id(),
                        at(scene.frame, 48.0),
                        egui::Rect::from_min_max(
                            egui::pos2(frame as f32 / 8.0, 0.0),
                            egui::pos2((frame + 1) as f32 / 8.0, 1.0),
                        ),
                        Color32::WHITE,
                    );
                });
        };
        let animate_choice = &mut self.animate;
        let mut details = |ui: &mut Ui| {
            ui.heading(&candidate.preview.creature.name);
            ui.label(
                candidate
                    .preview
                    .creature
                    .appearance
                    .design
                    .map_or("Classic companion", |d| d.body.label()),
            );
            ui.label(&candidate.preview.summary);
            if let Some(similarity) = candidate.preview.similarity {
                ui.small(format!("Color & shape affinity: {similarity}%"));
            }
            ui.add_enabled_ui(!save.settings.reduce_motion, |ui| {
                ui.checkbox(animate_choice, "Preview movement & expressions");
            });
        };
        if side_by_side {
            ui.horizontal(|ui| {
                preview(ui);
                ui.vertical(details);
            });
        } else {
            preview(ui);
            details(ui);
        }
        if animate {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(166));
        }
        // A code can also be an invitation rather than an adoption: the same preview, and a
        // friend who goes home again at the end of the day.
        if let Some(shared) = candidate.preview.shared {
            ui.add_space(12.0);
            card(ui, |ui| {
                ui.strong("Or ask them over for a day");
                ui.small(
                    "They come by the houses at every gathering for a day, then head home. Nobody \
                     joins your colony, and nothing about their own is read.",
                );
                let visiting = save.visitors.guest.is_some();
                let already_home = save
                    .creatures
                    .iter()
                    .any(|c| c.id == candidate.preview.creature.id);
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .add_enabled(
                            !visiting && !already_home,
                            egui::Button::new("Invite for a day"),
                        )
                        .clicked()
                    {
                        outcome.invite_visitor = Some(shared);
                    }
                    if visiting {
                        ui.label("Someone is already visiting the houses.");
                    } else if already_home {
                        ui.label("This companion already lives here.");
                    }
                });
            });
        }
        let adults = save.creatures.iter().filter(|c| c.role.is_adult()).count();
        let accept = |replace| match candidate.preview.shared {
            Some(shared) => PreviewAcceptance::Shared { shared, replace },
            None => match replace {
                Some(creature_id) => PreviewAcceptance::Replace {
                    creature_id,
                    source_seed: candidate.preview.source_seed,
                    design: candidate.preview.creature.appearance.design,
                },
                None => PreviewAcceptance::Add {
                    source_seed: candidate.preview.source_seed,
                    design: candidate.preview.creature.appearance.design,
                },
            },
        };
        ui.add_space(12.0);
        ui.collapsing("Replace a companion…", |ui| {
            egui::ComboBox::from_id_salt("replacement-target").selected_text(save.creatures.iter().find(|c| Some(c.id) == *selected_creature).map_or("Choose companion", |c| c.name.as_str())).show_ui(ui, |ui| {
                for c in &save.creatures { if ui.selectable_value(selected_creature, Some(c.id), &c.name).changed() { self.replace_confirmed = false; } }
            });
            if let Some(target) = save.creatures.iter().find(|c| Some(c.id) == *selected_creature) {
                if target.kept { ui.label("This companion is kept. Turn off Keep in its Colony profile before replacing it."); }
                ui.checkbox(&mut self.replace_confirmed, format!("Replace {} and start fresh history for this companion", target.name));
                if ui.add_enabled(self.replace_confirmed && !target.kept && (target.role.is_adult() || adults < MAX_ADULT_CREATURES), egui::Button::new("Confirm replacement")).clicked() {
                    outcome.accept_creature_preview = Some(accept(Some(target.id)));
                }
            }
        });
        if ui.small_button("Clear previews").clicked() {
            self.clear_previews();
        }
    }
}
