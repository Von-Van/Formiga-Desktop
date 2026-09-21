//! On-demand native UI resources. No resources or timers are added to desktop overlays.
use crate::settings::{GenerationPreview, PreviewAcceptance, SettingsOutcome};
use egui::{Color32, RichText, TextureHandle, Ui};
use formiga_art::{
    COLONY_OBJECT_ATLAS_WIDTH, COLONY_OBJECT_SIZE, Canvas, ColonyObjectRenderer, CreatureRenderer,
    SHELTER_SIZE, ShelterRenderer, StickerClip, TRINKET_ATLAS_HEIGHT, TRINKET_ATLAS_WIDTH,
    TRINKET_CELL, TRINKET_FRAME_REST, TrinketAtlasRenderer,
};
use formiga_core::*;
use std::collections::BTreeMap;
use time::OffsetDateTime;

/// The interface palette. Cream, forest, and mint by daylight; charcoal and sage after dark. The
/// settings window sets this once whenever the preference or the system appearance changes, and
/// the desktop overlays never consult it — creatures look the same either way.
static DARK_INTERFACE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn set_dark_interface(dark: bool) {
    DARK_INTERFACE.store(dark, std::sync::atomic::Ordering::Relaxed);
}

pub fn dark_interface() -> bool {
    DARK_INTERFACE.load(std::sync::atomic::Ordering::Relaxed)
}

fn shade(light: (u8, u8, u8), dark: (u8, u8, u8)) -> Color32 {
    let (r, g, b) = if dark_interface() { dark } else { light };
    Color32::from_rgb(r, g, b)
}

/// The page itself.
pub fn paper() -> Color32 {
    shade((249, 240, 212), (30, 36, 33))
}

/// Body text, and anything that has to be read on `paper`.
pub fn ink() -> Color32 {
    shade((35, 54, 47), (230, 237, 228))
}

/// Headings, banners, and the accent the interface is built around.
pub fn forest() -> Color32 {
    shade((28, 65, 55), (159, 196, 168))
}

/// Selection and the fill behind a chosen control.
pub fn mint() -> Color32 {
    shade((174, 221, 186), (47, 81, 71))
}

/// Hairlines and card edges.
pub fn gold() -> Color32 {
    shade((205, 177, 122), (78, 90, 79))
}

/// The face of a card, a shade away from the page.
pub fn card_fill() -> Color32 {
    shade((255, 248, 228), (38, 46, 42))
}

/// The navigation rail. By daylight it is the deep forest the interface is named for, with cream
/// on it. After dark it steps back instead of forward: a panel a shade off the page, with sage on
/// it, so the largest block on screen is not also the brightest thing in a dark room.
pub fn rail() -> Color32 {
    shade((28, 65, 55), (35, 43, 39))
}

/// Anything read against `rail`.
pub fn rail_ink() -> Color32 {
    shade((249, 240, 212), (159, 196, 168))
}

/// Text that is present but not being asked for attention.
pub fn muted() -> Color32 {
    shade((140, 148, 138), (150, 162, 148))
}

struct Candidate {
    preview: GenerationPreview,
    texture: TextureHandle,
}
#[derive(Default)]
pub struct Clubhouse {
    portraits: BTreeMap<CreatureId, (AppearanceGenome, TextureHandle)>,
    candidates: Vec<Candidate>,
    selected: usize,
    pub lock_colors: bool,
    pub lock_body: bool,
    pub animate: bool,
    pub seed_code: String,
    pub feedback: Option<(String, std::time::Instant)>,
    pub recovery: Option<String>,
    pub onboarding_step: usize,
    /// Which companion the journal is filtered to, if any. A view preference, never saved.
    pub journal_filter: Option<CreatureId>,
    pub restore_confirmed: bool,
    pub fresh_confirmed: bool,
    pub replace_confirmed: bool,
    pub show_intro: bool,
    /// Which clip the sticker export will use. A view preference, never saved.
    pub sticker_clip: StickerClip,
    /// Whether the sticker export draws at the smaller of the two offered scales. The default,
    /// `false`, is the large one, which is what most places want to post.
    pub small_sticker: bool,
    home_texture: Option<(ShelterGenome, Vec<ShelterDecorationKind>, TextureHandle)>,
    object_texture: Option<([u8; 32], TextureHandle)>,
    /// The colony's own sheet of found things: one texture the scrapbook cuts every slot out of,
    /// and the very same sheet the desktop samples when a companion holds one up.
    trinket_atlas: Option<([u8; 32], TextureHandle)>,
}

pub fn upload(context: &egui::Context, name: &str, canvas: &Canvas) -> TextureHandle {
    context.load_texture(
        name,
        egui::ColorImage::from_rgba_unmultiplied(
            [canvas.width() as usize, canvas.height() as usize],
            &canvas.rgba_bytes(),
        ),
        egui::TextureOptions::NEAREST,
    )
}
impl Clubhouse {
    pub fn notify(&mut self, message: impl Into<String>) {
        self.feedback = Some((message.into(), std::time::Instant::now()));
    }
    /// The scale the sticker export will draw at, from the two the art offers.
    pub fn sticker_scale(&self) -> u32 {
        let [small, large] = formiga_art::STICKER_SCALES;
        if self.small_sticker { small } else { large }
    }
    pub fn clear_previews(&mut self) {
        self.candidates.clear();
        self.selected = 0;
        self.replace_confirmed = false;
    }
    pub fn texture_ids(&self) -> Vec<egui::TextureId> {
        self.portraits
            .values()
            .map(|(_, t)| t.id())
            .chain(self.candidates.iter().map(|c| c.texture.id()))
            .chain(self.home_texture.iter().map(|(_, _, t)| t.id()))
            .chain(self.object_texture.iter().map(|(_, t)| t.id()))
            .chain(self.trinket_atlas.iter().map(|(_, t)| t.id()))
            .collect()
    }
    pub fn release_images(&mut self) {
        self.portraits.clear();
        self.clear_previews();
        self.home_texture = None;
        self.object_texture = None;
        self.trinket_atlas = None;
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
        for frame in 0..8 {
            let (action, index) = if frame < 6 {
                (ActionKind::Traverse, frame)
            } else {
                (ActionKind::Greet, frame - 6)
            };
            let tile = CreatureRenderer::render_frame(
                &preview.creature.appearance,
                action,
                index as u8,
                true,
            );
            for y in 0..48 {
                for x in 0..48 {
                    strip.set(frame * 48 + x, y, tile.get(x, y));
                }
            }
        }
        let texture = upload(context, "studio-candidate", &strip);
        if self.candidates.len() == 4 {
            self.candidates.remove(0);
        }
        self.candidates.push(Candidate { preview, texture });
        self.selected = self.candidates.len() - 1;
        self.replace_confirmed = false;
    }
    pub fn portrait(&mut self, ui: &mut Ui, creature: &Creature, size: f32) {
        let entry = self.portraits.entry(creature.id).or_insert_with(|| {
            (
                creature.appearance.clone(),
                upload(
                    ui.ctx(),
                    "colony-portrait",
                    &CreatureRenderer::render_frame(
                        &creature.appearance,
                        ActionKind::Greet,
                        2,
                        true,
                    ),
                ),
            )
        });
        if entry.0 != creature.appearance {
            *entry = (
                creature.appearance.clone(),
                upload(
                    ui.ctx(),
                    "colony-portrait",
                    &CreatureRenderer::render_frame(
                        &creature.appearance,
                        ActionKind::Greet,
                        2,
                        true,
                    ),
                ),
            );
        }
        egui::Frame::new()
            .fill(forest())
            .stroke(egui::Stroke::new(2.0, gold()))
            .inner_margin(8)
            .show(ui, |ui| {
                ui.add(
                    egui::Image::new(&entry.1)
                        .maintain_aspect_ratio(false)
                        .fit_to_exact_size(egui::vec2(size, size)),
                );
            });
    }
    pub fn retain_portraits(&mut self, creatures: &[Creature]) {
        self.portraits
            .retain(|id, _| creatures.iter().any(|c| c.id == *id));
    }
    pub fn intro(&mut self, ui: &mut Ui, save: &SaveFile, outcome: &mut SettingsOutcome) {
        if save.companion.onboarding_complete && !self.show_intro {
            return;
        }
        card(ui, |ui| {
            ui.label(
                RichText::new("WELCOME TO YOUR LITTLE COLONY")
                    .small()
                    .color(forest()),
            );
            let (title, description) = match self.onboarding_step {
                0 => (
                    "Meet your first companion",
                    "Choose a name below. Your creature will develop its own preferences as you spend time together.",
                ),
                1 => (
                    "Say hello",
                    "Click your creature on the desktop to pet it. A quick click is enough; watch its expression change.",
                ),
                2 => (
                    "Find a favorite spot",
                    "Drag your creature gently onto a window or the desktop. Release slowly to place it; a quick release gives it a soft toss.",
                ),
                _ => (
                    "A home that grows with you",
                    "The menu-bar or tray icon opens your colony, gathers companions, and pauses or hides them. A mini can arrive after an hour; more companions and keepsakes follow over time.",
                ),
            };
            ui.heading(title);
            ui.label(description);
            if let Some(first) = save.creatures.first() {
                let observed = match self.onboarding_step {
                    1 => first.memory.times_petted > 0,
                    2 => first.memory.placements > 0 || first.memory.times_tossed > 0,
                    _ => false,
                };
                if observed {
                    ui.colored_label(forest(), "Lovely — you've tried it!");
                }
            }
            ui.horizontal(|ui| {
                ui.label(format!("{} / 4", self.onboarding_step + 1));
                if ui
                    .button(if self.onboarding_step == 3 {
                        "Make yourself at home"
                    } else {
                        "Next"
                    })
                    .clicked()
                {
                    if self.onboarding_step == 3 {
                        outcome.complete_onboarding = true;
                        self.show_intro = false;
                    } else {
                        self.onboarding_step += 1;
                    }
                }
                if ui.small_button("Skip introduction").clicked() {
                    outcome.complete_onboarding = true;
                    self.show_intro = false;
                }
            });
        });
        ui.add_space(16.0);
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_secs(1));
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
                    "{} / 4 companions · {} / 3 full-size",
                    save.creatures.len(),
                    adults
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

    pub fn studio(
        &mut self,
        ui: &mut Ui,
        save: &SaveFile,
        selected_creature: &mut Option<CreatureId>,
        outcome: &mut SettingsOutcome,
    ) {
        title(
            ui,
            "Creature studio",
            "Find a companion that feels like yours.",
        );
        ui.horizontal_wrapped(|ui| {
            if ui.button("Discover four companions").clicked() {
                outcome.request_random_creature = true;
            }
            if ui.button("Create from image…").clicked() {
                outcome.request_reference_creature = true;
            }
        });
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.lock_colors, "Keep selected colors");
            ui.checkbox(&mut self.lock_body, "Keep selected body");
        });
        ui.small("Locks guide the next random set. Images are interpreted locally and discarded after preview.");
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
                            ui.colored_label(Color32::from_rgb(146, 61, 44), error.to_string());
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
        ui.horizontal_wrapped(|ui| {
            for (index, candidate) in self.candidates.iter().enumerate() {
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
                        || ui
                            .selectable_label(
                                self.selected == index,
                                format!("Companion {}", index + 1),
                            )
                            .clicked()
                    {
                        self.selected = index;
                        self.replace_confirmed = false;
                    }
                });
            }
        });
        ui.separator();
        let candidate = &self.candidates[self.selected];
        let animate = self.animate && !save.settings.reduce_motion;
        let frame = if animate {
            (ui.input(|i| i.time) * 6.0) as usize % 8
        } else {
            6
        };
        ui.horizontal(|ui| {
            egui::Frame::new()
                .fill(forest())
                .stroke(egui::Stroke::new(2.0, gold()))
                .inner_margin(8)
                .show(ui, |ui| {
                    ui.add(
                        egui::Image::new(&candidate.texture)
                            .uv(egui::Rect::from_min_max(
                                egui::pos2(frame as f32 / 8.0, 0.0),
                                egui::pos2((frame + 1) as f32 / 8.0, 1.0),
                            ))
                            .maintain_aspect_ratio(false)
                            .fit_to_exact_size(egui::vec2(144.0, 144.0)),
                    );
                });
            ui.vertical(|ui| {
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
                    ui.checkbox(&mut self.animate, "Preview movement & expressions");
                });
            });
        });
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
    pub fn home(
        &mut self,
        ui: &mut Ui,
        save: &SaveFile,
        monitors: &[MonitorInfo],
        outcome: &mut SettingsOutcome,
    ) {
        title(
            ui,
            "A place to call home",
            "Arrange the little things your colony has collected.",
        );
        let decorations: Vec<_> = save
            .home
            .decorations
            .decorations
            .iter()
            .copied()
            .filter(|k| save.home.hidden_decorations & (1 << k.index()) == 0)
            .collect();
        if self
            .home_texture
            .as_ref()
            .is_none_or(|(genome, kinds, _)| *genome != save.home.shelter || *kinds != decorations)
        {
            self.home_texture = Some((
                save.home.shelter,
                decorations.clone(),
                upload(
                    ui.ctx(),
                    "home-preview",
                    &ShelterRenderer::render_village(&save.home.shelter, &decorations),
                ),
            ));
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
        card(ui, |ui| {
            ui.horizontal(|ui| {
                self.village_preview(ui, save, monitors);
                ui.vertical(|ui| {
                    ui.strong("Home corner");
                    let mut corner = save.home.corner;
                    ui.selectable_value(&mut corner, HomeCorner::BottomLeft, "Bottom left");
                    ui.selectable_value(&mut corner, HomeCorner::BottomRight, "Bottom right");
                    if corner != save.home.corner {
                        outcome.home_corner = Some(corner);
                    }
                    egui::ComboBox::from_id_salt("home-display")
                        .selected_text("Choose home display")
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
        });
        ui.add_space(10.0);
        card(ui, |ui| {
            ui.strong("A portrait of everyone");
            ui.small("The village as it stands and every companion in front of it, as one 960×600 picture.");
            if ui.button("Export colony portrait…").clicked() {
                outcome.export_colony_card = true;
            }
        });
        ui.add_space(16.0);
        ui.strong("Earned decorations");
        if save.home.decorations.decorations.is_empty() {
            ui.label(
                "Your home will earn decorations as your colony grows. They arrive every few days.",
            );
        }
        let mut hidden = save.home.hidden_decorations;
        ui.horizontal_wrapped(|ui| {
            for kind in &save.home.decorations.decorations {
                let mut visible = hidden & (1 << kind.index()) == 0;
                if ui
                    .checkbox(&mut visible, words(&format!("{kind:?}")))
                    .changed()
                {
                    if visible {
                        hidden &= !(1 << kind.index());
                    } else {
                        hidden |= 1 << kind.index();
                    }
                }
            }
        });
        if hidden != save.home.hidden_decorations {
            outcome.hidden_decorations = Some(hidden);
        }
        ui.add_space(16.0);
        ui.strong("Keepsake arrangement");
        ui.small("The colony keeps its things in the two trees' yards, one end then the other. Slots run outward from a trunk; moving one changes where its influence is felt.");
        if save.objects.objects.is_empty() {
            ui.label("The first keepsake will find its way here in a few days.");
        }
        for (index, object) in save.objects.objects.iter().enumerate() {
            card(ui, |ui| {
                ui.horizontal(|ui| {
                    if let Some((_, texture)) = &self.object_texture {
                        let cell = f32::from(object.kind.index());
                        ui.add(
                            egui::Image::new(texture)
                                .uv(egui::Rect::from_min_max(
                                    egui::pos2(cell / 8.0, 0.0),
                                    egui::pos2((cell + 1.0) / 8.0, 1.0),
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
        ui.add_space(20.0);
        ui.separator();
        ui.add_space(14.0);
        self.scrapbook(ui, save);
    }
}
/// What the village preview draws each lot from, and in what order it lays them down: the houses
/// and the two trees, then the keepsakes hanging in their branches, then the belongings standing
/// on the ground in front of the trunks.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum VillageLayer {
    Dwelling,
    Trinket,
    Belonging,
}

impl Clubhouse {
    /// The colony's sheet of found things, uploaded once. It comes from the colony's own seed and
    /// the colours of whoever lives here, so the book always shows the keepsake the desktop would
    /// show, and a page of sixteen costs one texture rather than sixteen.
    fn trinket_atlas(&mut self, ui: &Ui, save: &SaveFile) -> TextureHandle {
        if self
            .trinket_atlas
            .as_ref()
            .is_none_or(|(seed, _)| *seed != save.colony_seed)
        {
            let members: Vec<formiga_art::Palette> = save
                .creatures
                .iter()
                .map(|creature| formiga_art::palette_for(&creature.appearance))
                .collect();
            let canvas = TrinketAtlasRenderer::render(save.colony_seed, &members);
            let texture = upload(ui.ctx(), "colony-trinkets", &canvas);
            self.trinket_atlas = Some((save.colony_seed, texture));
        }
        self.trinket_atlas.as_ref().unwrap().1.clone()
    }

    /// One keepsake, drawn square. A keepsake is a 16x16 cell of a sheet sixteen cells wide, and
    /// egui measures an image by its whole texture rather than by the part being shown: left to
    /// keep the sheet's shape it would letterbox a 256x32 atlas into a 48x6 sliver and squash the
    /// keepsake into it. Telling it to fill the box is what makes the cell square again.
    fn trinket_image<'a>(
        atlas: &'a egui::TextureHandle,
        variant: u8,
        size: f32,
    ) -> egui::Image<'a> {
        egui::Image::new(atlas)
            .uv(Self::trinket_uv(variant))
            .maintain_aspect_ratio(false)
            .fit_to_exact_size(egui::vec2(size, size))
    }

    /// Where one keepsake lives on that sheet, in the 0..1 coordinates egui samples with.
    fn trinket_uv(variant: u8) -> egui::Rect {
        let (x, y, width, height) = TrinketAtlasRenderer::cell_rect(variant, TRINKET_FRAME_REST);
        egui::Rect::from_min_size(
            egui::pos2(
                x as f32 / TRINKET_ATLAS_WIDTH as f32,
                y as f32 / TRINKET_ATLAS_HEIGHT as f32,
            ),
            egui::vec2(
                width as f32 / TRINKET_ATLAS_WIDTH as f32,
                height as f32 / TRINKET_ATLAS_HEIGHT as f32,
            ),
        )
    }

    /// The corner as it actually is: the two keepsake trees that bookend the houses with
    /// everything the scrapbook holds hung between them, the colony house with the decorations it
    /// has earned, one cottage per companion in colony order, and the loose belongings scattered
    /// in the two yards, all placed by the very functions the desktop places them with. Looking
    /// at it never calls the colony home or changes what any creature is doing.
    fn village_preview(&mut self, ui: &mut Ui, save: &SaveFile, monitors: &[MonitorInfo]) {
        let trinkets = self.trinket_atlas(ui, save);
        let (Some((_, _, village)), Some((_, objects_texture))) =
            (&self.home_texture, &self.object_texture)
        else {
            return;
        };
        let (village, objects_texture) = (village.clone(), objects_texture.clone());
        let cottages = formiga_core::colony_cottages(&save.creatures);
        let objects = save.objects.objects.len().min(MAX_COLONY_OBJECTS);
        let scale = save.settings.display_scale;
        // Every lot, in the desktop's own coordinates, on the display the home belongs to, with
        // the sheet it is drawn from. Houses and trees go behind, then the keepsakes hung in the
        // branches, then the belongings in front, exactly as the overlay layers them.
        let mut lots: Vec<(egui::Rect, egui::Rect, VillageLayer)> = Vec::new();
        let mut home_monitor = None;
        let mut found: Vec<u8> = save
            .companion
            .scrapbook
            .iter()
            .map(|record| record.variant)
            .filter(|variant| formiga_art::trinket_place(*variant).is_some())
            .collect();
        found.sort_unstable();
        found.dedup();
        for end in formiga_core::TreeEnd::BOTH {
            let Some((monitor_id, point)) = formiga_core::home_tree_position(
                &save.home,
                end,
                &cottages,
                monitors,
                &save.settings.habitat,
                scale,
            ) else {
                continue;
            };
            home_monitor.get_or_insert(monitor_id);
            let size = SHELTER_SIZE as f32;
            let corner = egui::pos2(point.x - size / 2.0, point.y - size);
            // The inward tree is the same atlas cell sampled the other way round, so the two
            // bookends are not the same drawing twice.
            let (u_left, u_right) = if end == formiga_core::TreeEnd::Inward {
                (1.0, 0.5)
            } else {
                (0.5, 1.0)
            };
            lots.push((
                egui::Rect::from_min_size(corner, egui::vec2(size, size)),
                egui::Rect::from_min_max(egui::pos2(u_left, 0.5), egui::pos2(u_right, 1.0)),
                VillageLayer::Dwelling,
            ));
            let half = TRINKET_CELL as f32 / 2.0;
            for variant in found.iter().copied() {
                let Some((hangs_in, anchor)) = formiga_art::trinket_place(variant) else {
                    continue;
                };
                if hangs_in != end {
                    continue;
                }
                lots.push((
                    egui::Rect::from_min_size(
                        corner + egui::vec2(anchor.x as f32 - half, anchor.y as f32 - half),
                        egui::vec2(TRINKET_CELL as f32, TRINKET_CELL as f32),
                    ),
                    Self::trinket_uv(variant),
                    VillageLayer::Trinket,
                ));
            }
        }
        for slot in 0..=cottages.len() {
            let Some((monitor_id, point)) = formiga_core::home_dwelling_position(
                &save.home,
                slot,
                &cottages,
                monitors,
                &save.settings.habitat,
                scale,
            ) else {
                continue;
            };
            home_monitor.get_or_insert(monitor_id);
            if home_monitor != Some(monitor_id) {
                continue;
            }
            let kind = if slot == 0 {
                formiga_core::DwellingKind::Main
            } else {
                cottages[slot - 1]
            };
            // The same quadrants the desktop samples from the same atlas.
            let (u, v) = match kind {
                formiga_core::DwellingKind::Main => (0.0, 0.0),
                formiga_core::DwellingKind::Cottage => (0.5, 0.0),
            };
            let size = SHELTER_SIZE as f32;
            lots.push((
                egui::Rect::from_min_size(
                    egui::pos2(point.x - size / 2.0, point.y - size),
                    egui::vec2(size, size),
                ),
                egui::Rect::from_min_size(egui::pos2(u, v), egui::vec2(0.5, 0.5)),
                VillageLayer::Dwelling,
            ));
        }
        let yard = formiga_core::home_object_positions(
            &save.home,
            &cottages,
            monitors,
            &save.settings.habitat,
            scale,
        );
        for (slot, place) in yard.iter().enumerate().take(objects) {
            let Some((monitor_id, point)) = place else {
                continue;
            };
            if home_monitor != Some(*monitor_id) {
                continue;
            }
            let kind = save.objects.objects[slot].kind;
            let size = COLONY_OBJECT_SIZE as f32;
            let cell = size / COLONY_OBJECT_ATLAS_WIDTH as f32;
            lots.push((
                egui::Rect::from_min_size(
                    egui::pos2(point.x - size / 2.0, point.y - size),
                    egui::vec2(size, size),
                ),
                egui::Rect::from_min_size(
                    egui::pos2(f32::from(kind.index()) * cell, 0.0),
                    egui::vec2(cell, 1.0),
                ),
                VillageLayer::Belonging,
            ));
        }
        let frame = egui::vec2(200.0, 160.0);
        let (response, painter) = ui.allocate_painter(frame, egui::Sense::hover());
        let area = response.rect;
        painter.rect_filled(area, 4.0, card_fill());
        if lots.is_empty() {
            painter.text(
                area.center(),
                egui::Align2::CENTER_CENTER,
                if save.home.is_active() {
                    "No room for the village here"
                } else {
                    "The colony is out exploring"
                },
                egui::TextStyle::Small.resolve(ui.style()),
                muted(),
            );
            return;
        }
        // Fit the whole strip, without ever stretching a pixel out of square.
        let village_bounds = lots
            .iter()
            .fold(lots[0].0, |acc, (rect, _, _)| acc.union(*rect));
        let fit = (area.width() / village_bounds.width().max(1.0))
            .min(area.height() / village_bounds.height().max(1.0))
            .clamp(0.25, 2.0);
        let origin = area.center() - village_bounds.size() * fit / 2.0;
        // Houses and both trees behind, then what is hung in them, then the belongings in front,
        // exactly as the desktop layers them.
        lots.sort_by_key(|(_, _, layer)| *layer);
        for (rect, uv, layer) in lots {
            let placed = egui::Rect::from_min_size(
                origin + (rect.min - village_bounds.min) * fit,
                rect.size() * fit,
            );
            painter.image(
                match layer {
                    VillageLayer::Dwelling => village.id(),
                    VillageLayer::Trinket => trinkets.id(),
                    VillageLayer::Belonging => objects_texture.id(),
                },
                placed,
                uv,
                Color32::WHITE,
            );
        }
    }

    pub fn scrapbook(&mut self, ui: &mut Ui, save: &SaveFile) {
        let offset = local_offset();
        let found = save.companion.scrapbook.clone();
        let atlas = self.trinket_atlas(ui, save);
        ui.label(
            RichText::new(format!(
                "THE SCRAPBOOK · {} of {}",
                found.len(),
                TRINKET_VARIANTS
            ))
            .color(forest())
            .size(11.0),
        );
        ui.add_space(6.0);
        if found.is_empty() {
            card(ui, |ui| {
                ui.strong("Nothing found yet");
                ui.label(
                    "When a companion brings something back for the first time, it is recorded \
                     here with the date and who found it.",
                );
            });
            ui.add_space(8.0);
        }
        for info in formiga_core::all_trinkets() {
            let record = found.iter().find(|r| r.variant == info.variant);
            card(ui, |ui| {
                ui.horizontal(|ui| {
                    // An empty slot still shows the shape of what belongs in it, dimmed to a
                    // silhouette, so the page reads as a book with room left in it.
                    let tint = if record.is_some() {
                        Color32::WHITE
                    } else {
                        Color32::from_rgba_unmultiplied(255, 255, 255, 40)
                    };
                    ui.add(Self::trinket_image(&atlas, info.variant, 48.0).tint(tint));
                    ui.vertical(|ui| match record {
                        Some(record) => {
                            ui.strong(info.name);
                            ui.label(info.description);
                            // The finder's name was kept when it was found, so a companion who
                            // has since left is still the one credited.
                            let finder = record
                                .finder
                                .and_then(|id| save.creatures.iter().find(|c| c.id == id))
                                .map_or(record.finder_name.as_str(), |c| c.name.as_str());
                            let at = record.first_at.to_offset(offset);
                            ui.small(format!("Found by {finder} · {}", at.date()));
                        }
                        None => {
                            ui.strong(RichText::new(info.name).color(muted()));
                            ui.small(RichText::new(info.hint).color(muted()));
                        }
                    });
                });
            });
            ui.add_space(8.0);
        }
        ui.small("Only the first find of each kind is recorded, and only on this computer.");
    }
}

pub fn title(ui: &mut Ui, heading: &str, description: &str) {
    ui.label(
        RichText::new("FORMIGA  /  A LITTLE LIFE HERE")
            .color(forest())
            .size(11.0),
    );
    ui.add_space(8.0);
    ui.heading(RichText::new(heading).size(27.0));
    ui.label(description);
    ui.add_space(20.0);
}
pub fn card<R>(ui: &mut Ui, contents: impl FnOnce(&mut Ui) -> R) -> egui::InnerResponse<R> {
    egui::Frame::new()
        .fill(card_fill())
        .stroke(egui::Stroke::new(1.0, gold()))
        .inner_margin(14)
        .show(ui, contents)
}
pub fn words(value: &str) -> String {
    let mut result = String::new();
    for (index, c) in value.chars().enumerate() {
        if index > 0 && c.is_uppercase() {
            result.push(' ');
        }
        result.push(c);
    }
    result
}
/// The reader's own clock, so a journal reads in local days rather than in UTC.
pub fn local_offset() -> time::UtcOffset {
    time::UtcOffset::local_offset_at(OffsetDateTime::now_utc()).unwrap_or(time::UtcOffset::UTC)
}

/// The heading one moment belongs under: Today, Yesterday, or its own local date. A clock that
/// has been wound back can leave a moment in the future; it still reads as today.
fn day_heading(at: OffsetDateTime, now: OffsetDateTime, offset: time::UtcOffset) -> String {
    let (local, today) = (at.to_offset(offset).date(), now.to_offset(offset).date());
    match (today - local).whole_days() {
        days if days <= 0 => "Today".into(),
        1 => "Yesterday".into(),
        _ => local.to_string(),
    }
}

/// What one moment says. Names are read from the colony as it is now, so a rename reads through.
pub fn moment_text(save: &SaveFile, entry: &JournalEntry) -> String {
    let name = entry
        .creature
        .and_then(|id| save.creatures.iter().find(|c| c.id == id))
        .map_or("A past companion", |c| c.name.as_str());
    match entry.moment {
        JournalMoment::Arrival => format!("{name} joined the colony"),
        JournalMoment::Friendship(friend_id) => format!(
            "{name} grew close to {}",
            save.creatures
                .iter()
                .find(|c| c.id == friend_id)
                .map_or("a past companion", |c| c.name.as_str())
        ),
        JournalMoment::Discovery => format!("{name} found a little treasure"),
        JournalMoment::Preference(descriptor) => format!(
            "{name} is becoming known for: {}",
            descriptor.label().to_lowercase()
        ),
        JournalMoment::Ritual(kind) => format!(
            "The colony shared {}",
            match kind {
                RitualKind::Picnic => "a picnic",
                RitualKind::GroupNap => "a group nap",
                RitualKind::FloorRace => "a little race",
                RitualKind::ShelterGathering => "a gathering at home",
                RitualKind::Catch => "a game of catch",
                RitualKind::GroupPresentation => "their discoveries",
                RitualKind::HatchDay => "a hatch-day celebration",
                RitualKind::QuietDayHuddle => "a quiet huddle",
                RitualKind::LateNightSleepPile => "a late-night sleep pile",
            }
        ),
        JournalMoment::Object(kind) => format!(
            "A {} found a place beside the home",
            words(&format!("{kind:?}")).to_lowercase()
        ),
        JournalMoment::Decoration(kind) => format!(
            "The home earned a {}",
            words(&format!("{kind:?}")).to_lowercase()
        ),
        // A visitor is never a colony member, so the moment carries the name it went by.
        JournalMoment::Visit(ref visitor) => format!("{visitor} came by the houses"),
    }
}

fn moment_card(
    ui: &mut Ui,
    save: &SaveFile,
    entry: &JournalEntry,
    offset: time::UtcOffset,
    outcome: &mut SettingsOutcome,
) {
    let local = entry.at.to_offset(offset);
    let pinned = save.companion.pinned(entry);
    card(ui, |ui| {
        ui.small(format!(
            "{} · {:02}:{:02}",
            local.date(),
            local.hour(),
            local.minute()
        ));
        ui.strong(moment_text(save, entry));
        ui.horizontal(|ui| {
            if pinned {
                if ui.small_button("Unpin").clicked() {
                    outcome.unpin_moment = Some(entry.clone());
                }
            } else {
                let room = save.companion.pins.len() < MAX_PINNED_ENTRIES;
                let button = ui.add_enabled(room, egui::Button::new("Pin").small());
                if button.clicked() {
                    outcome.pin_moment = Some(entry.clone());
                }
                if !room {
                    button.on_hover_text(format!(
                        "All {MAX_PINNED_ENTRIES} pins are in use. Unpin one to keep another."
                    ));
                }
            }
        });
    });
    ui.add_space(8.0);
}

fn label_was_shortened(name: &str) -> bool {
    name.chars().count() > 16
}

/// Who has come by the houses: whoever is here now, and the last two dozen before them. A line
/// in the book carries a name, a day, and the code that recreates the visitor — the same code
/// they would have handed over themselves.
fn guest_book(
    ui: &mut Ui,
    save: &SaveFile,
    clubhouse: &mut Clubhouse,
    outcome: &mut SettingsOutcome,
    offset: time::UtcOffset,
) {
    let visiting = save.visitors.guest.as_ref();
    if visiting.is_none() && save.visitors.guest_book.is_empty() {
        return;
    }
    ui.label(RichText::new("THE GUEST BOOK").color(forest()).size(11.0));
    ui.add_space(6.0);
    if let Some(guest) = visiting {
        card(ui, |ui| {
            ui.strong(format!("{} is visiting", guest.creature.name));
            ui.small(match guest.source {
                VisitorSource::Invited => "Invited with a code, here for the day.",
                VisitorSource::Wanderer => "Passing through, and stopped at the houses.",
            });
            ui.horizontal_wrapped(|ui| {
                let room = save.visitors.can_stay(&save.creatures);
                let stay = ui.add_enabled(room, egui::Button::new("Ask to stay").fill(mint()));
                if stay.clicked() {
                    outcome.ask_visitor_to_stay = true;
                }
                if !room {
                    stay.on_hover_text(
                        "Your colony is full. Keep their code and ask them again another time.",
                    );
                }
                if ui.button("Copy code").clicked() {
                    ui.ctx()
                        .copy_text(encode_creature_seed(guest.creature.origin));
                    clubhouse.notify("Visitor code copied");
                }
            });
        });
        ui.add_space(8.0);
    }
    if !save.visitors.guest_book.is_empty() {
        let heading = format!("Visitors before this · {}", save.visitors.guest_book.len());
        egui::CollapsingHeader::new(heading)
            .id_salt("guest-book")
            .show(ui, |ui| {
                for entry in save.visitors.guest_book.iter().rev() {
                    let local = entry.visited_at_utc.to_offset(offset);
                    card(ui, |ui| {
                        ui.small(format!(
                            "{} · {}",
                            local.date(),
                            match entry.source {
                                VisitorSource::Invited => "invited for a day",
                                VisitorSource::Wanderer => "came by the houses",
                            }
                        ));
                        ui.strong(&entry.name);
                        if ui.small_button("Copy code").clicked() {
                            ui.ctx().copy_text(encode_creature_seed(entry.origin));
                            clubhouse.notify("Visitor code copied");
                        }
                    });
                    ui.add_space(8.0);
                }
                ui.small(format!(
                    "The last {MAX_GUEST_BOOK_ENTRIES} visitors stay on this computer. A code \
                     recreates how a visitor looked and what they were like, and nothing else."
                ));
            });
    }
    ui.add_space(10.0);
    ui.separator();
    ui.add_space(8.0);
}

pub fn journal(
    ui: &mut Ui,
    save: &SaveFile,
    clubhouse: &mut Clubhouse,
    outcome: &mut SettingsOutcome,
) {
    title(ui, "The colony journal", "Small moments, kept close.");
    let offset = local_offset();
    let now = OffsetDateTime::now_utc();
    guest_book(ui, save, clubhouse, outcome, offset);
    if save.companion.journal.is_empty() && save.companion.pins.is_empty() {
        card(ui, |ui| {
            ui.heading("The story is just beginning");
            ui.label("New arrivals, discoveries, learned preferences, and shared rituals will appear here as they happen.");
        });
        return;
    }
    // Kept moments sit above the rolling journal and are not repeated inside it. They are read
    // from the pins themselves rather than from the journal, so keeping a moment still keeps it
    // once the rolling sixty-four have moved on past it — which is the whole point of a pin.
    let mut pinned: Vec<JournalEntry> = save
        .companion
        .pins
        .iter()
        .map(|pin| JournalEntry {
            at: pin.at,
            creature: pin.creature,
            moment: pin.moment.clone(),
        })
        .collect();
    pinned.sort_by_key(|entry| std::cmp::Reverse(entry.at));
    if !pinned.is_empty() {
        ui.label(
            RichText::new(format!("KEPT · {} of {MAX_PINNED_ENTRIES}", pinned.len()))
                .color(forest())
                .size(11.0),
        );
        ui.add_space(6.0);
        for entry in &pinned {
            moment_card(ui, save, entry, offset, outcome);
        }
        ui.separator();
        ui.add_space(8.0);
    }
    // One chip per companion the journal actually mentions, so the filter never offers an
    // empty result. A moment about the whole colony belongs to everyone.
    let mut mentioned: Vec<CreatureId> = save
        .companion
        .journal
        .iter()
        .filter_map(|entry| entry.creature)
        .collect();
    mentioned.sort_unstable();
    mentioned.dedup();
    if mentioned.len() > 1 {
        ui.horizontal_wrapped(|ui| {
            if ui
                .selectable_label(clubhouse.journal_filter.is_none(), "Everyone")
                .clicked()
            {
                clubhouse.journal_filter = None;
            }
            for id in mentioned {
                let name = save
                    .creatures
                    .iter()
                    .find(|c| c.id == id)
                    .map_or_else(|| "A past companion".to_string(), |c| c.name.clone());
                // Names are capped at 24 characters when they are chosen, but a filter row is a
                // row of chips: keep one long name from pushing the rest onto its own line.
                let label = if name.chars().count() > 16 {
                    format!("{}…", name.chars().take(15).collect::<String>())
                } else {
                    name.clone()
                };
                let chip = ui.selectable_label(clubhouse.journal_filter == Some(id), label);
                if label_was_shortened(&name) {
                    chip.clone().on_hover_text(name);
                }
                if chip.clicked() {
                    clubhouse.journal_filter = (clubhouse.journal_filter != Some(id)).then_some(id);
                }
            }
        });
        ui.add_space(10.0);
    }
    let showing: Vec<&JournalEntry> = save
        .companion
        .journal
        .iter()
        .rev()
        .filter(|entry| !save.companion.pinned(entry))
        .filter(|entry| {
            clubhouse
                .journal_filter
                .is_none_or(|id| entry.creature == Some(id))
        })
        .collect();
    if showing.is_empty() {
        card(ui, |ui| {
            ui.strong("Nothing else here yet");
            ui.label("Every moment about this companion is already kept above.");
        });
        ui.add_space(8.0);
    }
    let mut heading = String::new();
    for entry in showing {
        let day = day_heading(entry.at, now, offset);
        if day != heading {
            ui.add_space(4.0);
            ui.label(RichText::new(day.to_uppercase()).color(forest()).size(11.0));
            ui.add_space(6.0);
            heading = day;
        }
        moment_card(ui, save, &entry.clone(), offset, outcome);
    }
    ui.small(format!(
        "The most recent {MAX_JOURNAL_ENTRIES} moments stay on this computer, plus up to \
         {MAX_PINNED_ENTRIES} you keep. Missed time is never replayed."
    ));
}
/// Charcoal or cream, and how large the words are. These change the settings window only; the
/// creatures on the desktop are untouched by either.
pub fn appearance_controls(ui: &mut Ui, save: &SaveFile, outcome: &mut SettingsOutcome) {
    let mut appearance = save.companion.appearance;
    card(ui, |ui| {
        ui.strong("Appearance");
        ui.small("Applies to this window. Your colony looks the same either way.");
        ui.horizontal_wrapped(|ui| {
            ui.label("Theme");
            for (choice, name) in [
                (ThemeChoice::System, "Match system"),
                (ThemeChoice::Light, "Cream"),
                (ThemeChoice::Dark, "Charcoal"),
            ] {
                ui.selectable_value(&mut appearance.theme, choice, name);
            }
        });
        ui.horizontal_wrapped(|ui| {
            ui.label("Text size");
            for scale in [100u8, 110, 125, 150] {
                ui.selectable_value(&mut appearance.text_scale, scale, format!("{scale}%"));
            }
        });
        ui.checkbox(
            &mut appearance.sprite_outline,
            "Outline creatures for busy wallpaper",
        )
        .on_hover_text(
            "Draws a soft edge behind each creature so it stays readable on bright or detailed \
             backgrounds. It never changes where you can click.",
        );
    });
    if appearance != save.companion.appearance {
        appearance.normalize();
        outcome.appearance = Some(appearance);
    }
}

const WEEKDAYS: [(&str, u8); 7] = [
    ("Mon", 0),
    ("Tue", 1),
    ("Wed", 2),
    ("Thu", 3),
    ("Fri", 4),
    ("Sat", 5),
    ("Sun", 6),
];

/// Opt-in weekday changes between the two saved routines. Nothing here runs on a timer: the next
/// change is simply a moment the app already knows to wake for.
pub fn schedule_controls(ui: &mut Ui, save: &SaveFile, outcome: &mut SettingsOutcome) {
    let mut schedule = save.companion.schedule.clone();
    let offset = local_offset();
    let saved = [
        save.companion.modes[0].is_some(),
        save.companion.modes[1].is_some(),
    ];
    card(ui, |ui| {
        ui.strong("A weekly routine");
        ui.small(
            "Move between your saved Work and Relax routines at times you choose. Everything \
             else — showing the colony, pausing, a quiet moment — stays yours.",
        );
        ui.checkbox(&mut schedule.enabled, "Follow a weekly routine");
        if !schedule.enabled {
            return;
        }
        if !saved[0] || !saved[1] {
            ui.colored_label(
                forest(),
                "Save both a Work and a Relax routine above for this to have somewhere to go.",
            );
        }
        // What is in force now, and what happens next.
        let local = OffsetDateTime::now_utc().to_offset(offset);
        let active = schedule.intended(local);
        ui.horizontal_wrapped(|ui| {
            ui.label("Now:");
            ui.strong(match active {
                Some(0) => "Work",
                Some(_) => "Relax",
                None => "Nothing scheduled yet",
            });
            if schedule.overridden {
                ui.colored_label(forest(), "· changed by hand");
                if ui.small_button("Follow the routine again").clicked() {
                    outcome.resume_routine = true;
                }
            }
        });
        if let Some((at, preset)) = schedule.next_after(local) {
            ui.small(format!(
                "Next: {} at {:02}:{:02} on {}",
                if preset == 0 { "Work" } else { "Relax" },
                at.hour(),
                at.minute(),
                at.date()
            ));
        }
        ui.add_space(8.0);
        let mut remove = None;
        for (index, transition) in schedule.transitions.iter_mut().enumerate() {
            ui.horizontal_wrapped(|ui| {
                for (name, bit) in WEEKDAYS {
                    let mut on = transition.days & (1 << bit) != 0;
                    if ui.selectable_label(on, name).clicked() {
                        on = !on;
                        if on {
                            transition.days |= 1 << bit;
                        } else {
                            transition.days &= !(1 << bit);
                        }
                    }
                }
                let (mut hour, mut minute) = (transition.minute / 60, transition.minute % 60);
                ui.add(egui::DragValue::new(&mut hour).range(0..=23).prefix("at "));
                ui.add(egui::DragValue::new(&mut minute).range(0..=59).prefix(":"));
                transition.minute = (hour.min(23) * 60 + minute.min(59)).min(1439);
                ui.selectable_value(&mut transition.preset, 0, "Work");
                ui.selectable_value(&mut transition.preset, 1, "Relax");
                if ui.small_button("Remove").clicked() {
                    remove = Some(index);
                }
            });
            if transition.days == 0 {
                ui.small(
                    RichText::new("Choose at least one day for this change to mean anything.")
                        .color(muted()),
                );
            }
        }
        if let Some(index) = remove {
            schedule.transitions.remove(index);
        }
        let room = schedule.transitions.len() < MAX_SCHEDULED_TRANSITIONS;
        let add = ui.add_enabled(room, egui::Button::new("Add a change").small());
        if add.clicked() {
            schedule.transitions.push(ScheduledTransition {
                days: 0b0011111,
                minute: 9 * 60,
                preset: 0,
            });
        }
        if !room {
            add.on_hover_text(format!(
                "A week holds {MAX_SCHEDULED_TRANSITIONS} changes. Remove one to add another."
            ));
        }
        ui.small("Times are your own local time. A change you miss while away is not replayed.");
    });
    if schedule != save.companion.schedule {
        outcome.schedule = Some(schedule);
    }
}

pub fn quiet_controls(ui: &mut Ui, save: &SaveFile, outcome: &mut SettingsOutcome) {
    card(ui, |ui| {
        ui.strong("A quiet moment");
        if let Some(until) = save.companion.quiet_until {
            let minutes = (until - OffsetDateTime::now_utc()).whole_minutes().max(0) + 1;
            ui.label(format!("Settling at home · about {minutes} min remaining"));
            if ui.button("Return to normal").clicked() {
                outcome.quiet_minutes = Some(0);
            }
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_secs(30));
        } else {
            ui.label("Let everyone settle at home, then return to their usual adventures.");
            ui.horizontal(|ui| {
                for minutes in [15, 30, 60] {
                    if ui.button(format!("{minutes} minutes")).clicked() {
                        outcome.quiet_minutes = Some(minutes);
                    }
                }
            });
        }
    });
}
pub fn habitat_map(ui: &mut Ui, policy: &HabitatPolicy, monitors: &[MonitorInfo]) {
    if monitors.is_empty() {
        return;
    }
    let min_x = monitors
        .iter()
        .map(|m| m.bounds.x)
        .fold(f32::INFINITY, f32::min);
    let min_y = monitors
        .iter()
        .map(|m| m.bounds.y)
        .fold(f32::INFINITY, f32::min);
    let max_x = monitors
        .iter()
        .map(|m| m.bounds.right())
        .fold(f32::NEG_INFINITY, f32::max);
    let max_y = monitors
        .iter()
        .map(|m| m.bounds.bottom())
        .fold(f32::NEG_INFINITY, f32::max);
    let (area, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), 176.0),
        egui::Sense::hover(),
    );
    let factor =
        ((area.width() - 20.0) / (max_x - min_x).max(1.0)).min(140.0 / (max_y - min_y).max(1.0));
    for (index, monitor) in monitors.iter().enumerate() {
        let rect = egui::Rect::from_min_size(
            area.min
                + egui::vec2(
                    10.0 + (monitor.bounds.x - min_x) * factor,
                    (monitor.bounds.y - min_y) * factor,
                ),
            egui::vec2(
                monitor.bounds.width * factor,
                monitor.bounds.height * factor,
            ),
        );
        ui.painter()
            .rect_filled(rect, 0.0, Color32::from_rgb(219, 210, 185));
        for region in accessible_regions(policy, monitor) {
            let zone = egui::Rect::from_min_size(
                rect.min
                    + egui::vec2(
                        (region.x - monitor.bounds.x) * factor,
                        (region.y - monitor.bounds.y) * factor,
                    ),
                egui::vec2(region.width * factor, region.height * factor),
            );
            ui.painter().rect_filled(zone.intersect(rect), 0.0, mint());
        }
        for zone in policy.zones.iter().filter(|z| {
            z.enabled && z.display == monitor.display_key && z.kind == HabitatZoneKind::Excluded
        }) {
            let bounds = zone.normalized_bounds;
            let usable = monitor.usable_bounds;
            let zone = egui::Rect::from_min_size(
                rect.min
                    + egui::vec2(
                        (usable.x - monitor.bounds.x + bounds.x * usable.width) * factor,
                        (usable.y - monitor.bounds.y + bounds.y * usable.height) * factor,
                    ),
                egui::vec2(
                    bounds.width * usable.width * factor,
                    bounds.height * usable.height * factor,
                ),
            );
            ui.painter()
                .rect_filled(zone.intersect(rect), 0.0, Color32::from_rgb(217, 159, 137));
        }
        ui.painter().rect_stroke(
            rect,
            0.0,
            egui::Stroke::new(2.0, forest()),
            egui::StrokeKind::Inside,
        );
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            format!("Display {}", index + 1),
            egui::FontId::proportional(13.0),
            ink(),
        );
    }
    ui.small("Mint: allowed · Clay: excluded · Gray: outside the habitat");
    ui.add_space(14.0);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A keepsake is one cell of a sheet sixteen cells wide. egui measures an image by its whole
    /// texture and not by the part a uv shows, so an image left to keep that sheet's shape is
    /// letterboxed into a sliver and the keepsake inside it is squashed flat. Every keepsake the
    /// scrapbook draws has to come out square.
    #[test]
    fn a_keepsake_is_drawn_square_however_wide_the_sheet_it_is_cut_from() {
        let sheet = egui::vec2(TRINKET_ATLAS_WIDTH as f32, TRINKET_ATLAS_HEIGHT as f32);
        assert!(
            sheet.x > sheet.y * 4.0,
            "the sheet has to be far wider than it is tall for this to be worth pinning"
        );
        let context = egui::Context::default();
        let atlas = context.load_texture(
            "colony-trinkets",
            egui::ColorImage::filled(
                [TRINKET_ATLAS_WIDTH as usize, TRINKET_ATLAS_HEIGHT as usize],
                egui::Color32::WHITE,
            ),
            egui::TextureOptions::NEAREST,
        );
        let plenty = egui::vec2(512.0, 512.0);
        for size in [24.0_f32, 48.0, 96.0] {
            for variant in [0, 7, 15] {
                let image = Clubhouse::trinket_image(&atlas, variant, size);
                let drawn = image.calc_size(plenty, image.size());
                assert_eq!(
                    drawn,
                    egui::vec2(size, size),
                    "keepsake {variant} asked for at {size} came out {drawn:?}"
                );
            }
        }
    }
}
