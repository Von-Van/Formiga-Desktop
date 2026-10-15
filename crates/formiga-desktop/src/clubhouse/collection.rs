//! Everything the colony has found, and what it is wearing from it.
//!
//! The Collection on the Your colony page is every keepsake there is: what has been found in full
//! colour, and what has not as a shape with a hint beside it, with sixteen of the found ones picked
//! to hang in the village's two trees. The Journal keeps its own record of the finds themselves —
//! only what has actually turned up, with who found it and when. And each companion has a place to
//! choose one thing to wear, made from a find, each pictured beside its name as it would sit on
//! that companion, with a larger look at how it sits in each of a few poses before it is put on.
//!
//! Every picture is drawn at a whole number of points to its pixels, so the art is as crisp in the
//! notebook as on the desktop.

use super::shell::Shell;
use super::tour::TourMark;
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

/// Each thing there is to wear, pictured on its own as it sits on one companion as they look now:
/// one strip of boxes in the order of `AccessoryKind::ALL`, and which of them show anything.
struct Pieces {
    creature: CreatureId,
    appearance: AppearanceGenome,
    strip: TextureHandle,
    shown: Vec<bool>,
}

/// The box a piece's picture is drawn in beside its name, in pixels, drawn at two points to the
/// pixel. Every picture is the same size, so every choice is too; a piece larger than the box,
/// which none is on any body Formiga draws, would be cut to it.
const PIECE_BOX: (u32, u32) = (28, 18);
/// How large the standing pose is shown, and the other poses.
const STANDING_SIZE: f32 = 144.0;
const POSE_SIZE: f32 = 96.0;
/// A find's tile in the Collection, and the find drawn on it at two points to the pixel.
const FIND_TILE: f32 = 38.0;
const FIND_SIZE: f32 = 32.0;
/// A pin's tile in the wardrobe.
const PIN_TILE: f32 = 36.0;

/// The Collection's and the wardrobe's view state. Never saved.
#[derive(Default)]
pub(super) struct CollectionState {
    view: CollectionView,
    try_on: Option<TryOn>,
    pieces: Option<Pieces>,
    /// What the pointer was over among the things to wear last frame, shown on the companion
    /// until it moves off.
    trying: Option<(CreatureId, Option<Accessory>)>,
}

impl CollectionState {
    pub(super) fn texture_ids(&self) -> Vec<egui::TextureId> {
        self.try_on
            .iter()
            .flat_map(|try_on| try_on.poses.iter().map(TextureHandle::id))
            .chain(self.pieces.iter().map(|pieces| pieces.strip.id()))
            .collect()
    }

    pub(super) fn release_images(&mut self) {
        self.try_on = None;
        self.pieces = None;
    }
}

/// Puts a piece's picture in box `index` of a strip, centred, and cut to the box if it is larger.
fn box_piece(strip: &mut Canvas, index: usize, piece: &Canvas) {
    let (width, height) = PIECE_BOX;
    let left = index as i32 * width as i32 + (width as i32 - piece.width() as i32) / 2;
    let top = (height as i32 - piece.height() as i32) / 2;
    let cell = index as i32 * width as i32..(index as i32 + 1) * width as i32;
    for y in 0..piece.height() as i32 {
        for x in 0..piece.width() as i32 {
            let pixel = piece.get(x, y);
            if pixel.a > 0 && cell.contains(&(left + x)) {
                strip.set(left + x, top + y, pixel);
            }
        }
    }
}

/// Where box `index` is in the strip of pieces, as texture coordinates.
fn piece_uv(index: usize) -> egui::Rect {
    let count = AccessoryKind::ALL.len() as f32;
    egui::Rect::from_min_max(
        egui::pos2(index as f32 / count, 0.0),
        egui::pos2((index + 1) as f32 / count, 1.0),
    )
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
/// A choice with a picture of the piece shows it beside its name, greyed out with the choice
/// while it cannot be worn.
fn wear_choice(
    ui: &mut Ui,
    chosen: bool,
    enabled: bool,
    label: &str,
    picture: Option<(&TextureHandle, egui::Rect)>,
) -> egui::Response {
    let border = ui.visuals().widgets.inactive.bg_stroke;
    if !chosen {
        ui.visuals_mut().widgets.inactive.bg_stroke = egui::Stroke::NONE;
    }
    let (width, height) = PIECE_BOX;
    let button = match picture {
        Some((strip, uv)) => egui::Button::image_and_text(
            egui::Image::new(strip)
                .uv(uv)
                .maintain_aspect_ratio(false)
                .fit_to_exact_size(egui::vec2(width as f32 * 2.0, height as f32 * 2.0))
                .tint(if enabled {
                    Color32::WHITE
                } else {
                    Color32::from_white_alpha(90)
                }),
            label,
        )
        .selected(chosen),
        None => egui::Button::selectable(chosen, label),
    };
    let response = ui.add_enabled(enabled, button);
    ui.visuals_mut().widgets.inactive.bg_stroke = border;
    response
}

/// The Colony page's own state: the Collection and the wardrobe, and the sticker being made of a
/// companion. Never saved.
#[derive(Default)]
pub(crate) struct ColonyState {
    collection: CollectionState,
    /// Which clip the sticker export will use.
    pub sticker_clip: StickerClip,
    /// Whether the sticker export draws at the smaller of the two offered scales. The default,
    /// `false`, is the large one, which is what most places want to post.
    pub small_sticker: bool,
}

impl ColonyState {
    pub(super) fn texture_ids(&self) -> Vec<egui::TextureId> {
        self.collection.texture_ids()
    }

    pub(super) fn release_images(&mut self) {
        self.collection.release_images();
    }

    /// The scale the sticker export will draw at, from the two the art offers.
    pub fn sticker_scale(&self) -> u32 {
        let [small, large] = formiga_art::STICKER_SCALES;
        if self.small_sticker { small } else { large }
    }

    /// Every keepsake there is, as one page: the found ones in their colours and the rest as the
    /// shape of what belongs there, with a hint. A found one can be hung in the trees, sixteen at
    /// most; with none chosen by hand the trees fill themselves as finds come in.
    pub fn collection(
        &mut self,
        ui: &mut Ui,
        shell: &mut Shell,
        save: &SaveFile,
        outcome: &mut SettingsOutcome,
    ) {
        let atlas = shell.trinket_atlas(ui, save);
        let offset = local_offset();
        let found = &save.companion.scrapbook;
        let hung = hung_keepsakes(save.home.tree_keepsakes.as_ref(), found);
        let hanging = hung.iter().flatten().count();
        let collection = card(ui, |ui| {
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
            if found.is_empty() {
                ui.label(
                    "Nothing has turned up yet. Companions find things by themselves as they go \
                     about — waking from a nap, up on a high window, after a visit — and bring \
                     each one to show you. There is nothing to do but let them.",
                );
                ui.add_space(6.0);
            } else if self.collection.view == CollectionView::StillToFind
                && found.len() == usize::from(TRINKET_VARIANTS)
            {
                ui.label("Everything there is to find has been found. A remarkable colony.");
            }
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
                    let spoken = if record.is_some() {
                        info.name.to_owned()
                    } else {
                        format!("Still to find: {}", info.hint)
                    };
                    let (response, rect) = tile(
                        ui,
                        egui::vec2(FIND_TILE, FIND_TILE),
                        if in_trees { mint() } else { card_fill() },
                        if in_trees { forest() } else { gold() },
                        &spoken,
                        in_trees,
                    );
                    ui.painter().image(
                        atlas.id(),
                        egui::Rect::from_center_size(
                            rect.center(),
                            egui::vec2(FIND_SIZE, FIND_SIZE),
                        ),
                        Shell::trinket_uv(info.variant),
                        tint,
                    );
                    let response = match record {
                        Some(record) => {
                            let finder = record
                                .finder
                                .and_then(|id| save.creatures.iter().find(|c| c.id == id))
                                .map_or(record.finder_name.as_str(), |c| c.name.as_str());
                            let at = record.first_at.to_offset(offset);
                            let text = format!(
                                "{}\n{}\nFound by {finder} · {}\n{}",
                                info.name,
                                info.description,
                                at.date(),
                                if in_trees {
                                    "Hanging in the trees · click to take it down"
                                } else {
                                    "Click to hang it in the trees"
                                }
                            );
                            // The find large, at four points to the pixel, beside what is
                            // known about it.
                            response.on_hover_ui(|ui| {
                                ui.horizontal(|ui| {
                                    let (rect, _) = ui.allocate_exact_size(
                                        egui::vec2(FIND_SIZE * 2.0, FIND_SIZE * 2.0),
                                        egui::Sense::hover(),
                                    );
                                    ui.painter().image(
                                        atlas.id(),
                                        rect,
                                        Shell::trinket_uv(info.variant),
                                        Color32::WHITE,
                                    );
                                    ui.label(text);
                                });
                            })
                        }
                        None => response.on_hover_text(format!("Still to find\n{}", info.hint)),
                    };
                    if response.clicked() && record.is_some() {
                        match toggled_in_trees(save, info.variant) {
                            Some(hooks) => outcome.tree_keepsakes = Some(Some(hooks)),
                            None => shell.notify(format!(
                                "The trees hold {TREE_HOOKS}. Take one down to hang {}.",
                                info.name
                            )),
                        }
                    }
                }
            });
        });
        shell.tour_mark(ui, TourMark::Collection, collection.response.rect);
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

    /// Each thing there is to wear, pictured on its own as it would sit on `creature`. Drawn again
    /// only when who it is or how they look changes.
    fn piece_pictures(
        &mut self,
        ui: &Ui,
        save: &SaveFile,
        creature: &Creature,
    ) -> Option<(TextureHandle, Vec<bool>)> {
        let current = self.collection.pieces.as_ref().is_some_and(|pieces| {
            pieces.creature == creature.id && pieces.appearance == creature.appearance
        });
        if !current {
            let members: Vec<formiga_art::Palette> = save
                .creatures
                .iter()
                .map(|member| formiga_art::palette_for(&member.appearance))
                .collect();
            let (width, height) = PIECE_BOX;
            let mut strip = Canvas::new(width * AccessoryKind::ALL.len() as u32, height);
            let shown = AccessoryKind::ALL
                .into_iter()
                .enumerate()
                .map(|(index, kind)| {
                    let art =
                        AccessoryArt::resolve(Accessory::Worn(kind), save.colony_seed, &members);
                    CreatureRenderer::render_worn_piece(&creature.appearance, art)
                        .map(|piece| box_piece(&mut strip, index, &piece))
                        .is_some()
                })
                .collect();
            self.collection.pieces = Some(Pieces {
                creature: creature.id,
                appearance: creature.appearance.clone(),
                strip: upload(ui.ctx(), "wear-pieces", &strip),
                shown,
            });
        }
        self.collection
            .pieces
            .as_ref()
            .map(|pieces| (pieces.strip.clone(), pieces.shown.clone()))
    }

    /// One thing to wear, chosen from what the colony has found: something made from a find, or
    /// a find itself worn as a pin — or nothing at all. Each choice is pictured as it would sit on
    /// this companion. Pointing at one shows the companion wearing it, large and in each pose,
    /// before it is put on; clicking puts it on, and clicking Nothing takes it off again.
    pub fn wardrobe(
        &mut self,
        ui: &mut Ui,
        shell: &mut Shell,
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
        let pieces = self.piece_pictures(ui, save, creature);
        let atlas = shell.trinket_atlas(ui, save);
        let mut pointed: Option<Option<Accessory>> = None;
        let wardrobe = card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.strong("Wears");
                ui.label(creature.accessory.map_or_else(
                    || "nothing just now".to_owned(),
                    |accessory| accessory.label(),
                ));
            });
            // Standing, large, then the other poses beside it; where they do not all fit, at the
            // largest text in the smallest window, the rest go on to a row of their own.
            ui.horizontal_wrapped(|ui| {
                for (index, (texture, (_, _, label))) in poses.iter().zip(TRY_ON_POSES).enumerate()
                {
                    let size = if index == 0 { STANDING_SIZE } else { POSE_SIZE };
                    make_room(ui, size + 2.0 * 4.0);
                    ui.vertical(|ui| {
                        // A whole number of points to each pixel, so every pixel stays square.
                        egui::Frame::new()
                            .fill(forest())
                            .inner_margin(4)
                            .show(ui, |ui| {
                                ui.add(
                                    egui::Image::new(texture)
                                        .maintain_aspect_ratio(false)
                                        .fit_to_exact_size(egui::vec2(size, size)),
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
                let none = wear_choice(ui, creature.accessory.is_none(), true, "Nothing", None);
                if none.hovered() {
                    pointed = Some(None);
                }
                if none.clicked() && creature.accessory.is_some() {
                    outcome.set_accessory = Some((creature.id, None));
                }
                for (index, kind) in AccessoryKind::ALL.into_iter().enumerate() {
                    let accessory = Accessory::Worn(kind);
                    let picture = pieces
                        .as_ref()
                        .filter(|(_, shown)| shown[index])
                        .map(|(strip, _)| (strip, piece_uv(index)));
                    let made = available.contains(&accessory);
                    let chip = wear_choice(
                        ui,
                        creature.accessory == Some(accessory),
                        made,
                        kind.label(),
                        picture,
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
                                    egui::vec2(PIN_TILE, PIN_TILE),
                                    if worn { mint() } else { card_fill() },
                                    gold(),
                                    &pin.label(),
                                    worn,
                                );
                                ui.painter().image(
                                    atlas.id(),
                                    egui::Rect::from_center_size(
                                        rect.center(),
                                        egui::vec2(FIND_SIZE, FIND_SIZE),
                                    ),
                                    Shell::trinket_uv(variant),
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
        shell.tour_mark(ui, TourMark::Wardrobe, wardrobe.response.rect);
        // What the pointer is over now is what the poses show next frame; moving off every
        // choice goes back to what is actually worn.
        self.collection.trying = pointed.map(|candidate| (creature.id, candidate));
    }
}
