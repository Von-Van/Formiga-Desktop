//! The Home page: the village as it stands, arranged in place, painted, and sent as a postcard.

use super::shell::Shell;
use super::*;

/// The village the Home page last drew, and what it was drawn from, so it is only drawn again
/// when a house, its decorations or its residents change.
pub(super) struct HomeTexture {
    pub(super) look: formiga_art::VillageLook,
    pub(super) texture: TextureHandle,
}

/// The Home page's own state. Never saved.
#[derive(Default)]
pub(crate) struct HomeState {
    /// What the village preview has picked out, and whatever is being carried across it in
    /// Arrange mode.
    pub(crate) arrange: arrange::ArrangeState,
    /// The village the preview last drew, and the colony's things, each uploaded only when they
    /// change.
    pub(super) home_texture: Option<HomeTexture>,
    pub(super) object_texture: Option<([u8; 32], TextureHandle)>,
    /// Whether putting the village back as it grew has been asked for once, and is waiting to be
    /// confirmed.
    village_reset_asked: bool,
    /// The postcard being written: which scene, and the caption so far. A postcard is only ever
    /// the file it is exported to.
    pub postcard_scene: formiga_art::PostcardScene,
    pub postcard_caption: String,
}

impl HomeState {
    pub(super) fn texture_ids(&self) -> impl Iterator<Item = egui::TextureId> + '_ {
        self.home_texture
            .iter()
            .map(|home| home.texture.id())
            .chain(self.object_texture.iter().map(|(_, t)| t.id()))
    }

    pub(super) fn release_images(&mut self) {
        self.home_texture = None;
        self.object_texture = None;
    }

    /// The Home page.
    pub fn show(
        &mut self,
        ui: &mut Ui,
        shell: &mut Shell,
        save: &SaveFile,
        monitors: &[MonitorInfo],
        outcome: &mut SettingsOutcome,
    ) {
        journal::page_heading(
            ui,
            crate::settings::SettingsTab::Home,
            "Home & keepsakes",
            "The settlement as it stands. Somebody has moved the teapot again.",
        );
        let look = formiga_art::VillageLook::of(&save.home, &save.creatures);
        if self
            .home_texture
            .as_ref()
            .is_none_or(|home| home.look != look)
        {
            let texture = upload(
                ui.ctx(),
                "home-preview",
                &ShelterRenderer::render_home_row(&look),
            );
            self.home_texture = Some(HomeTexture { look, texture });
        }
        if self
            .object_texture
            .as_ref()
            .is_none_or(|(seed, _)| *seed != save.colony_seed)
        {
            self.object_texture = Some((
                save.colony_seed,
                upload(
                    ui.ctx(),
                    "home-objects",
                    &ColonyObjectRenderer::render_atlas(save.colony_seed),
                ),
            ));
        }
        let village = card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.strong("The village");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let label = if self.arrange.arranging {
                        "Done arranging"
                    } else {
                        "Arrange"
                    };
                    if ui
                        .add(egui::Button::new(label).fill(if self.arrange.arranging {
                            mint()
                        } else {
                            card_fill()
                        }))
                        .on_hover_text(
                            "Drag the cottages along the row and move anything on the ground \
                             where you like it. Arrow keys nudge whatever is picked out.",
                        )
                        .clicked()
                    {
                        self.arrange.arranging = !self.arrange.arranging;
                    }
                });
            });
            self.village_preview(shell, ui, save, monitors, outcome);
            if self.arrange.arranging {
                ui.small(
                    "The colony house always stands first. Things on the ground keep a little \
                     room between them, and settle where there is some.",
                );
            }
        });
        shell.tour_mark(ui, tour::TourMark::Village, village.response.rect);
        ui.add_space(10.0);
        self.picked_house(ui, save, outcome);
        ui.add_space(10.0);
        self.ground_catalogue(ui, save, outcome);
        ui.add_space(10.0);
        let corner = card(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.strong("Home corner");
                let mut corner = save.home.corner;
                ui.selectable_value(&mut corner, HomeCorner::BottomLeft, "Bottom left");
                ui.selectable_value(&mut corner, HomeCorner::BottomRight, "Bottom right");
                if corner != save.home.corner {
                    outcome.home_corner = Some(corner);
                }
                let choose = "Choose home display";
                make_room(ui, combo_width(ui, choose));
                egui::ComboBox::from_id_salt("home-display")
                    .selected_text(choose)
                    .show_ui(ui, |ui| {
                        for (index, monitor) in monitors.iter().enumerate() {
                            if ui
                                .selectable_label(
                                    save.home.display == Some(monitor.display_key),
                                    format!(
                                        "Display {}{}",
                                        index + 1,
                                        if monitor.primary { " · primary" } else { "" }
                                    ),
                                )
                                .clicked()
                            {
                                outcome.home_display = Some(monitor.display_key);
                            }
                        }
                    });
            });
        });
        shell.tour_mark(ui, tour::TourMark::HomeCorner, corner.response.rect);
        ui.add_space(10.0);
        self.village_colours(ui, save, outcome);
        ui.add_space(10.0);
        card(ui, |ui| {
            ui.strong("A portrait of everyone");
            ui.small("The village as it stands and every companion in front of it, as one 960×600 picture.");
            if ui.button("Export colony portrait…").clicked() {
                outcome.export_colony_card = true;
            }
        });
        ui.add_space(10.0);
        card(ui, |ui| {
            ui.strong("A postcard from the village");
            ui.small("Everyone together in a little scene, with a line of your own under it if you like.");
            ui.horizontal_wrapped(|ui| {
                for scene in formiga_art::PostcardScene::ALL {
                    ui.selectable_value(&mut self.postcard_scene, scene, scene.label());
                }
            });
            ui.add(
                egui::TextEdit::singleline(&mut self.postcard_caption)
                    .hint_text("A caption (optional)")
                    .char_limit(formiga_art::POSTCARD_CAPTION_LIMIT)
                    .desired_width(320.0),
            );
            if ui.button("Export postcard…").clicked() {
                outcome.export_postcard = Some((
                    self.postcard_scene,
                    formiga_art::postcard_caption(&self.postcard_caption),
                ));
            }
        });
        ui.add_space(16.0);
        ui.strong("Belongings in the yards");
        ui.small("The colony keeps its things in the two trees' yards, one end then the other. Slots run outward from a trunk; moving one changes where its influence is felt.");
        if save.objects.objects.is_empty() {
            ui.label("The first keepsake will find its way here in a few days.");
        }
        for (index, object) in save.objects.objects.iter().enumerate() {
            card(ui, |ui| {
                // Closer and Further go under the rest when there is no room beside it.
                ui.horizontal_wrapped(|ui| {
                    if let Some((_, texture)) = &self.object_texture {
                        ui.add(
                            egui::Image::new(texture)
                                .uv(arrange::object_uv(
                                    ColonyObjectRenderer::object_cell(object.kind),
                                    false,
                                ))
                                .maintain_aspect_ratio(false)
                                .fit_to_exact_size(egui::vec2(40.0, 40.0)),
                        );
                    }
                    ui.vertical(|ui| {
                        ui.strong(format!(
                            "{} · {}",
                            index + 1,
                            words(&format!("{:?}", object.kind))
                        ));
                        ui.small(match object.role {
                            ColonyObjectRole::Sleep => "A cozy place to rest",
                            ColonyObjectRole::Play => "Invites a little play",
                            ColonyObjectRole::Comfort => "A comforting presence",
                            ColonyObjectRole::Social => "A place to spend time together",
                            ColonyObjectRole::Curiosity => "Something worth investigating",
                        });
                    });
                    if ui
                        .add_enabled(index > 0, egui::Button::new("Closer"))
                        .clicked()
                    {
                        outcome.move_object = Some((index, index - 1));
                    }
                    if ui
                        .add_enabled(
                            index + 1 < save.objects.objects.len(),
                            egui::Button::new("Further"),
                        )
                        .clicked()
                    {
                        outcome.move_object = Some((index, index + 1));
                    }
                });
            });
        }
    }
}

impl HomeState {
    /// The palette the village is painted in, and a way to put the whole arrangement back as the
    /// village grew: the order of the cottages, their types and decorations, the colours, and
    /// everything on the ground.
    fn village_colours(&mut self, ui: &mut Ui, save: &SaveFile, outcome: &mut SettingsOutcome) {
        card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("Colours");
                let chosen = save.home.palette;
                let name = |palette: Option<VillagePalette>| {
                    palette.map_or("From the colony", VillagePalette::label)
                };
                egui::ComboBox::from_id_salt("village-palette")
                    .selected_text(name(chosen))
                    .show_ui(ui, |ui| {
                        for palette in std::iter::once(None).chain(VillagePalette::ALL.map(Some)) {
                            ui.horizontal(|ui| {
                                let (main, accent) = palette.map_or(
                                    (
                                        save.home.shelter.palette_index,
                                        save.home.shelter.accent_index,
                                    ),
                                    VillagePalette::palettes,
                                );
                                let colours = formiga_art::PALETTES;
                                swatch(
                                    ui,
                                    colours[usize::from(main) % colours.len()].coat,
                                    colours[usize::from(accent) % colours.len()].coat,
                                );
                                if ui
                                    .selectable_label(chosen == palette, name(palette))
                                    .clicked()
                                    && chosen != palette
                                {
                                    outcome.village_palette = Some(palette);
                                }
                            });
                        }
                    });
            });
            ui.small("A palette paints every house and both trees; the colony's own colours come back with \"From the colony\".");
        });
        let arranged = !save.home.cottage_order.is_empty()
            || save.home.palette.is_some()
            || !save.home.gardens.is_empty()
            || !save.home.ornaments.is_empty()
            || !save.home.house_styles.is_empty();
        ui.vertical(|ui| {
            if self.village_reset_asked && arranged {
                ui.label(
                    "Put the houses, colours, gardens and ornaments back as they grew? Spots and \
                     decorations stay where they are.",
                );
                ui.horizontal(|ui| {
                    if ui.button("Put back").clicked() {
                        outcome.reset_village = true;
                        self.village_reset_asked = false;
                    }
                    if ui.button("Keep them").clicked() {
                        self.village_reset_asked = false;
                    }
                });
            } else {
                self.village_reset_asked = false;
                if ui
                    .add_enabled(
                        arranged,
                        egui::Button::new("Put the village back as it grew…"),
                    )
                    .clicked()
                {
                    self.village_reset_asked = true;
                }
            }
        });
    }
}

/// A little two-colour chip: a curtain's cloth and tie, or a palette's main colour and accent.
pub(super) fn swatch(ui: &mut Ui, main: formiga_art::Rgba, accent: formiga_art::Rgba) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(22.0, 12.0), egui::Sense::hover());
    let split = rect.left() + rect.width() * 0.6;
    let colour = |rgba: formiga_art::Rgba| Color32::from_rgb(rgba.r, rgba.g, rgba.b);
    let painter = ui.painter();
    painter.rect_filled(
        egui::Rect::from_min_max(rect.min, egui::pos2(split, rect.bottom())),
        0.0,
        colour(main),
    );
    painter.rect_filled(
        egui::Rect::from_min_max(egui::pos2(split, rect.top()), rect.max),
        0.0,
        colour(accent),
    );
    painter.rect_stroke(
        rect,
        0.0,
        egui::Stroke::new(1.0, gold()),
        egui::StrokeKind::Inside,
    );
}
