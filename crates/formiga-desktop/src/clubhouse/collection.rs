//! Everything the colony has found, and what it is wearing from it.
//!
//! The Collection on the Your colony page is every keepsake there is: what has been found in full
//! colour, and what has not as a shape with a hint beside it, with sixteen of the found ones picked
//! to hang in the village's two trees. The Journal keeps its own record of the finds themselves —
//! only what has actually turned up, with who found it and when. And each companion has a place to
//! choose one thing to wear, made from a find, with a look at how it sits in each of a few poses
//! before it is put on.

use super::*;
use formiga_art::AccessoryArt;
use formiga_core::{Accessory, AccessoryKind, TREE_HOOKS, available_accessories, hung_keepsakes};

/// The poses a companion is shown trying something on in: standing, out for a walk, sitting up
/// on a ledge, and asleep — so a hat that sits well standing is seen to sit well lying down too.
const TRY_ON_POSES: [(ActionKind, u8, &str); 4] = [
    (ActionKind::Idle, 0, "Standing"),
    (ActionKind::Traverse, 2, "Walking"),
    (ActionKind::Perch, 0, "Up high"),
    (ActionKind::Sleep, 0, "Asleep"),
];

/// Which part of the Collection is showing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum CollectionView {
    #[default]
    Everything,
    Found,
    StillToFind,
}

/// A companion trying something on: who, as they look now, what, and the poses drawn.
struct TryOn {
    creature: CreatureId,
    appearance: AppearanceGenome,
    wearing: Option<Accessory>,
    poses: Vec<TextureHandle>,
}

/// The Collection's and the wardrobe's view state. Never saved.
#[derive(Default)]
pub(super) struct CollectionState {
    view: CollectionView,
    try_on: Option<TryOn>,
    /// What the pointer was over among the things to wear last frame, shown on the companion
    /// until it moves off.
    trying: Option<(CreatureId, Option<Accessory>)>,
}

impl CollectionState {
    pub(super) fn texture_ids(&self) -> Vec<egui::TextureId> {
        self.try_on
            .iter()
            .flat_map(|try_on| try_on.poses.iter().map(TextureHandle::id))
            .collect()
    }

    pub(super) fn release_images(&mut self) {
        self.try_on = None;
    }
}

/// What hangs in the trees with `variant` taken down if it is up, or put up on the first free
/// hook if it is not; `None` if it is not up and every hook is taken.
fn toggled_in_trees(save: &SaveFile, variant: u8) -> Option<[Option<u8>; TREE_HOOKS]> {
    let mut hooks = hung_keepsakes(save.home.tree_keepsakes.as_ref(), &save.companion.scrapbook);
    if let Some(hook) = hooks.iter_mut().find(|hook| **hook == Some(variant)) {
        *hook = None;
        return Some(hooks);
    }
    let free = hooks.iter_mut().find(|hook| hook.is_none())?;
    *free = Some(variant);
    Some(hooks)
}

/// One choice among the things to wear, the same size whether or not it is pointed at. egui
/// takes a frame's border back off its padding, but a choice that is not the one worn draws no
/// frame until it is pointed at, so under this window's bordered style it would grow by a pixel
/// all round when pointed at, and nudge every choice after it along the row, or over onto the
/// next. Wrapped in a scope it would stop wrapping, so the border is set and put back instead.
fn wear_choice(ui: &mut Ui, chosen: bool, enabled: bool, label: &str) -> egui::Response {
    let border = ui.visuals().widgets.inactive.bg_stroke;
    if !chosen {
        ui.visuals_mut().widgets.inactive.bg_stroke = egui::Stroke::NONE;
    }
    let response = ui.add_enabled(enabled, egui::Button::selectable(chosen, label));
    ui.visuals_mut().widgets.inactive.bg_stroke = border;
    response
}

impl Clubhouse {
    /// Every keepsake there is, as one page: the found ones in their colours and the rest as the
    /// shape of what belongs there, with a hint. A found one can be hung in the trees, sixteen at
    /// most; with none chosen by hand the trees fill themselves as finds come in.
    pub fn collection(&mut self, ui: &mut Ui, save: &SaveFile, outcome: &mut SettingsOutcome) {
        let atlas = self.trinket_atlas(ui, save);
        let offset = local_offset();
        let found = &save.companion.scrapbook;
        let hung = hung_keepsakes(save.home.tree_keepsakes.as_ref(), found);
        let hanging = hung.iter().flatten().count();
        card(ui, |ui| {
            ui.label(
                RichText::new(format!(
                    "COLLECTION · {} of {} found",
                    found.len(),
                    TRINKET_VARIANTS
                ))
                .color(forest())
                .size(11.0),
            );
            ui.small(
                "Everything the colony could ever bring back. What hasn't turned up yet shows \
                 only its shape — hover it for a hint about where it might be found.",
            );
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                let view = &mut self.collection.view;
                ui.selectable_value(view, CollectionView::Everything, "Everything");
                ui.selectable_value(view, CollectionView::Found, "Found");
                ui.selectable_value(view, CollectionView::StillToFind, "Still to find");
            });
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                ui.label(format!(
                    "Hanging in the village trees: {hanging} of {TREE_HOOKS}"
                ));
                if save.home.tree_keepsakes.is_some() {
                    ui.small("· chosen by you");
                    if ui.small_button("Let the trees fill themselves").clicked() {
                        outcome.tree_keepsakes = Some(None);
                    }
                } else {
                    ui.small("· filling themselves as finds come in");
                }
            });
            ui.small("Click something found to hang it in the trees, or to take it down again.");
            ui.add_space(8.0);
            let view = self.collection.view;
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
                for info in formiga_core::all_trinkets() {
                    let record = found.iter().find(|record| record.variant == info.variant);
                    let show = match view {
                        CollectionView::Everything => true,
                        CollectionView::Found => record.is_some(),
                        CollectionView::StillToFind => record.is_none(),
                    };
                    if !show {
                        continue;
                    }
                    let in_trees = hung.contains(&Some(info.variant));
                    // Something still to find is its own shape and nothing more: a shadow of
                    // what belongs there.
                    let tint = if record.is_some() {
                        Color32::WHITE
                    } else {
                        Color32::from_rgba_unmultiplied(0, 0, 0, 70)
                    };
                    let (response, rect) = tile(
                        ui,
                        egui::vec2(42.0, 42.0),
                        if in_trees { mint() } else { card_fill() },
                        if in_trees { forest() } else { gold() },
                    );
                    ui.painter().image(
                        atlas.id(),
                        rect.shrink(3.0),
                        Self::trinket_uv(info.variant),
                        tint,
                    );
                    let response = match record {
                        Some(record) => {
                            let finder = record
                                .finder
                                .and_then(|id| save.creatures.iter().find(|c| c.id == id))
                                .map_or(record.finder_name.as_str(), |c| c.name.as_str());
                            let at = record.first_at.to_offset(offset);
                            response.on_hover_text(format!(
                                "{}\n{}\nFound by {finder} · {}\n{}",
                                info.name,
                                info.description,
                                at.date(),
                                if in_trees {
                                    "Hanging in the trees · click to take it down"
                                } else {
                                    "Click to hang it in the trees"
                                }
                            ))
                        }
                        None => response.on_hover_text(format!("Still to find\n{}", info.hint)),
                    };
                    if response.clicked() && record.is_some() {
                        match toggled_in_trees(save, info.variant) {
                            Some(hooks) => outcome.tree_keepsakes = Some(Some(hooks)),
                            None => self.notify(format!(
                                "The trees hold {TREE_HOOKS}. Take one down to hang {}.",
                                info.name
                            )),
                        }
                    }
                }
            });
        });
    }

    /// The finds themselves, for the Journal: only what has actually turned up, newest first,
    /// with who found it and when. What is still to find lives in the Collection.
    pub fn scrapbook(&mut self, ui: &mut Ui, save: &SaveFile) {
        let offset = local_offset();
        let atlas = self.trinket_atlas(ui, save);
        let mut found = save.companion.scrapbook.clone();
        found.sort_by_key(|record| std::cmp::Reverse((record.first_at, record.variant)));
        ui.label(
            RichText::new(format!("THE SCRAPBOOK · {} found", found.len()))
                .color(forest())
                .size(11.0),
        );
        ui.add_space(6.0);
        if found.is_empty() {
            card(ui, |ui| {
                ui.strong("Nothing found yet");
                ui.label(
                    "When a companion brings something back for the first time, it is recorded \
                     here with the date and who found it. Everything still to find is in the \
                     Collection, on the Your colony page.",
                );
            });
            ui.add_space(8.0);
            return;
        }
        for record in &found {
            let Some(info) = formiga_core::trinket_info(record.variant) else {
                continue;
            };
            card(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.add(Self::trinket_image(&atlas, info.variant, 48.0));
                    ui.vertical(|ui| {
                        ui.strong(info.name);
                        ui.label(info.description);
                        // The finder's name was kept when it was found, so a companion who has
                        // since left is still the one credited.
                        let finder = record
                            .finder
                            .and_then(|id| save.creatures.iter().find(|c| c.id == id))
                            .map_or(record.finder_name.as_str(), |c| c.name.as_str());
                        let at = record.first_at.to_offset(offset);
                        ui.small(format!("Found by {finder} · {}", at.date()));
                    });
                });
            });
            ui.add_space(8.0);
        }
        ui.small("Only the first find of each kind is recorded, and only on this computer.");
    }

    /// The companion wearing `wearing`, drawn in each of the try-on poses, from the colony's own
    /// inks for the find it is made from. Drawn again only when who, how they look, or what they
    /// are trying on changes.
    fn try_on_poses(
        &mut self,
        ui: &Ui,
        save: &SaveFile,
        creature: &Creature,
        wearing: Option<Accessory>,
    ) -> Vec<TextureHandle> {
        let current = self.collection.try_on.as_ref().is_some_and(|try_on| {
            try_on.creature == creature.id
                && try_on.appearance == creature.appearance
                && try_on.wearing == wearing
        });
        if !current {
            let members: Vec<formiga_art::Palette> = save
                .creatures
                .iter()
                .map(|member| formiga_art::palette_for(&member.appearance))
                .collect();
            let dress = wearing
                .map(|accessory| AccessoryArt::resolve(accessory, save.colony_seed, &members));
            let poses = TRY_ON_POSES
                .iter()
                .map(|(action, frame, _)| {
                    upload(
                        ui.ctx(),
                        "try-on",
                        &CreatureRenderer::render_dressed_frame(
                            &creature.appearance,
                            dress,
                            *action,
                            *frame,
                            true,
                        ),
                    )
                })
                .collect();
            self.collection.try_on = Some(TryOn {
                creature: creature.id,
                appearance: creature.appearance.clone(),
                wearing,
                poses,
            });
        }
        self.collection
            .try_on
            .as_ref()
            .map(|try_on| try_on.poses.clone())
            .unwrap_or_default()
    }

    /// One thing to wear, chosen from what the colony has found: something made from a find, or
    /// a find itself worn as a pin — or nothing at all. Pointing at a choice shows the companion
    /// wearing it in each pose before it is put on; clicking puts it on, and clicking Nothing
    /// takes it off again.
    pub fn wardrobe(
        &mut self,
        ui: &mut Ui,
        save: &SaveFile,
        creature: &Creature,
        outcome: &mut SettingsOutcome,
    ) {
        let available = available_accessories(&save.companion.scrapbook);
        let trying = self
            .collection
            .trying
            .filter(|(id, _)| *id == creature.id)
            .map_or(creature.accessory, |(_, candidate)| candidate);
        let poses = self.try_on_poses(ui, save, creature, trying);
        let atlas = self.trinket_atlas(ui, save);
        let mut pointed: Option<Option<Accessory>> = None;
        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.strong("Wears");
                ui.label(creature.accessory.map_or_else(
                    || "nothing just now".to_owned(),
                    |accessory| accessory.label(),
                ));
            });
            // Where four poses do not fit beside one another, at the largest text in the
            // smallest window, the last goes on to a row of its own.
            ui.horizontal_wrapped(|ui| {
                for (texture, (_, _, label)) in poses.iter().zip(TRY_ON_POSES) {
                    make_room(ui, 96.0 + 2.0 * 4.0);
                    ui.vertical(|ui| {
                        // Twice the size it is drawn at, so every pixel stays square.
                        egui::Frame::new()
                            .fill(forest())
                            .inner_margin(4)
                            .show(ui, |ui| {
                                ui.add(
                                    egui::Image::new(texture)
                                        .maintain_aspect_ratio(false)
                                        .fit_to_exact_size(egui::vec2(96.0, 96.0)),
                                );
                            });
                        ui.small(label);
                    });
                }
            });
            // Always exactly one line, whatever is pointed at. A line that came and went with the
            // pointer pushed every choice below it out from under the pointer, which took the
            // line away again, over and over.
            let note = match trying {
                _ if trying == creature.accessory => "Point at something to try it on".to_owned(),
                Some(accessory) => format!("Trying on {} · click to put it on", accessory.label()),
                None => "Trying on nothing · click to take it off".to_owned(),
            };
            ui.add(egui::Label::new(RichText::new(note).small()).truncate());
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                let none = wear_choice(ui, creature.accessory.is_none(), true, "Nothing");
                if none.hovered() {
                    pointed = Some(None);
                }
                if none.clicked() && creature.accessory.is_some() {
                    outcome.set_accessory = Some((creature.id, None));
                }
                for kind in AccessoryKind::ALL {
                    let accessory = Accessory::Worn(kind);
                    let made = available.contains(&accessory);
                    let chip = wear_choice(
                        ui,
                        creature.accessory == Some(accessory),
                        made,
                        kind.label(),
                    );
                    if !made {
                        chip.on_disabled_hover_text(
                            "Made from something the colony hasn't found yet.",
                        );
                        continue;
                    }
                    if chip.hovered() {
                        pointed = Some(Some(accessory));
                    }
                    if chip.clicked() && creature.accessory != Some(accessory) {
                        outcome.set_accessory = Some((creature.id, Some(accessory)));
                    }
                }
            });
            let pins: Vec<u8> = available
                .iter()
                .filter_map(|accessory| match accessory {
                    Accessory::Pin(variant) => Some(*variant),
                    Accessory::Worn(_) => None,
                })
                .collect();
            if pins.is_empty() {
                ui.small("Anything the colony finds can be worn as a pin, once it has been found.");
            } else {
                egui::CollapsingHeader::new(format!("Wear a find as a pin · {}", pins.len()))
                    .id_salt(("pins", creature.id))
                    .show(ui, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            for variant in pins {
                                let pin = Accessory::Pin(variant);
                                let worn = creature.accessory == Some(pin);
                                let (response, rect) = tile(
                                    ui,
                                    egui::vec2(32.0, 32.0),
                                    if worn { mint() } else { card_fill() },
                                    gold(),
                                );
                                ui.painter().image(
                                    atlas.id(),
                                    rect.shrink(2.0),
                                    Self::trinket_uv(variant),
                                    Color32::WHITE,
                                );
                                let response = response.on_hover_text(pin.label());
                                if response.hovered() {
                                    pointed = Some(Some(pin));
                                }
                                if response.clicked() && !worn {
                                    outcome.set_accessory = Some((creature.id, Some(pin)));
                                }
                            }
                        });
                    });
            }
        });
        // What the pointer is over now is what the poses show next frame; moving off every
        // choice goes back to what is actually worn.
        self.collection.trying = pointed.map(|candidate| (creature.id, candidate));
    }
}
