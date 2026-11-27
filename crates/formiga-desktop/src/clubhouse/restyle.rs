//! Restyling a companion in the creature studio: the parts, proportions, colours and face of a
//! companion who already lives here, changed and kept as one change the footer can take back.
//!
//! This is the customizing half of what Formiga Farm offers, on Desktop's own design model and
//! drawn by Desktop's own renderer. Making a creature — the sculpted body plans, the animals to
//! start from, rolling a whole design — stays in Farm. A sculpted form keeps its body here and only
//! its face can change; a look from before recipes keeps its body and colours the same way.
//!
//! Nothing is held while the studio is closed: the working copy and its preview are made when a
//! companion is chosen on this page and let go of as soon as another page is shown.

use super::*;
use formiga_core::forms::{Design, Form};

/// Give companion `id` the look `design`, as one change the footer can take back
/// (`ColonyEdit::Restyled`). The look is changed the one way Desktop changes any look, by the
/// Farm contract's `apply_design`, which touches nothing else about the companion. A design that
/// is not exactly in range is refused, never repaired. Returns whether it was kept.
pub(crate) fn keep_look(world: &mut World, id: CreatureId, design: &Design) -> bool {
    if design.validate().is_err() {
        return false;
    }
    let Some(name) = world
        .save
        .creatures
        .iter()
        .find(|creature| creature.id == id)
        .map(|creature| creature.name.clone())
    else {
        return false;
    };
    world.edit(ColonyEdit::Restyled { name }, |world| {
        if let Some(creature) = world.save.creatures.iter_mut().find(|c| c.id == id) {
            formiga_farm_contract::apply_design(creature, design);
        }
    });
    true
}

/// The working copy of one companion's look. Never saved.
#[derive(Default)]
pub(crate) struct RestyleState {
    target: Option<CreatureId>,
    /// The look as it was chosen or last kept, to put back.
    started: Option<Design>,
    draft: Option<Design>,
    /// The preview, and the design it was drawn for.
    preview: Option<(Design, TextureHandle)>,
    /// Whether the section was open on the last frame, so the preview is pinned only while it is.
    open: bool,
    /// Where the pinned preview was last drawn, for the review tests.
    pub(crate) preview_rect: Option<egui::Rect>,
}

/// The four views of the preview: facing the screen, side-on, turned away, and waving.
const VIEWS: u32 = 4;

/// How wide the pinned preview stands beside the choices, at 100% text, and how wide the choices
/// must be able to be beside it; narrower than both together, it is pinned above them instead.
pub(crate) const SIDE_PANE: f32 = 236.0;
pub(crate) const CHOICES_MIN: f32 = 330.0;

const LAYOUTS: [&str; 12] = [
    "Button",
    "Wide button",
    "Low and wide",
    "Modular",
    "Beady",
    "Low beads",
    "Tall",
    "Tall and wide",
    "Square",
    "Mask",
    "Visor",
    "Beak",
];

impl RestyleState {
    pub(crate) fn texture_ids(&self) -> impl Iterator<Item = egui::TextureId> + '_ {
        self.preview.iter().map(|(_, texture)| texture.id())
    }

    /// Let go of the working copy and its preview.
    pub(crate) fn release(&mut self) {
        *self = Self::default();
    }

    /// Whether a companion is being restyled with the section open, so its preview is pinned in
    /// view outside the page's scrolling.
    pub(crate) fn is_pinned(&self) -> bool {
        self.open && self.target.is_some() && self.draft.is_some()
    }

    pub(super) fn show(&mut self, ui: &mut Ui, save: &SaveFile) {
        let shown = egui::CollapsingHeader::new("Restyle a companion")
            .id_salt("restyle-a-companion")
            .default_open(self.target.is_some())
            .show(ui, |ui| self.contents(ui, save));
        self.open = shown.body_returned.is_some();
        if !self.open {
            self.preview_rect = None;
        }
    }

    fn contents(&mut self, ui: &mut Ui, save: &SaveFile) {
        ui.label(
            "Change the parts, proportions, colours and face of someone who already lives here. \
             Who they are stays the same. The picture of them stays in view as you change them.",
        );
        let chosen = save
            .creatures
            .iter()
            .find(|creature| Some(creature.id) == self.target);
        egui::ComboBox::from_id_salt("restyle-target")
            .selected_text(chosen.map_or("Choose a companion", |c| c.name.as_str()))
            .show_ui(ui, |ui| {
                for creature in &save.creatures {
                    if ui
                        .selectable_label(Some(creature.id) == self.target, &creature.name)
                        .clicked()
                    {
                        let design = Design::of(&creature.appearance);
                        self.target = Some(creature.id);
                        self.started = Some(design.clone());
                        self.draft = Some(design);
                        self.preview = None;
                    }
                }
            });
        if !save
            .creatures
            .iter()
            .any(|creature| Some(creature.id) == self.target)
        {
            let open = self.open;
            self.release();
            self.open = open;
            return;
        }
        let Some(mut draft) = self.draft.clone() else {
            return;
        };
        ui.add_space(8.0);
        let swatches = colony_swatches(save);
        match &mut draft.form {
            Form::Companion { recipe } => {
                let before = *recipe;
                let mut r = *recipe;
                body_controls(ui, &mut r);
                parts_controls(ui, &mut r);
                finish_controls(ui, &mut r, &swatches);
                if r != before {
                    *recipe = r.bounded();
                }
            }
            Form::Sculpted { .. } => {
                ui.small(
                    "A form sculpted in Formiga Farm is reshaped there. Its face can change here.",
                );
            }
            Form::Original => {
                ui.small("An original look keeps its body and colours. Its face can change here.");
            }
        }
        face_controls(ui, &mut draft);
        self.draft = Some(draft);
    }

    /// The companion as the draft would have it, with what to do with the look beside or under
    /// it: drawn above or beside the page, outside its scrolling, so it stays in view while the
    /// choices scroll. `beside` is a column beside the choices, the four views two by two over the
    /// buttons. Otherwise it is a band above them, as short as it can be: the views two by two with
    /// the buttons beside them where there is room, or in a row with the buttons under them.
    pub(crate) fn pinned(
        &mut self,
        ui: &mut Ui,
        save: &SaveFile,
        outcome: &mut SettingsOutcome,
        beside: bool,
    ) {
        let Some(creature) = save
            .creatures
            .iter()
            .find(|creature| Some(creature.id) == self.target)
        else {
            return;
        };
        let Some(mut draft) = self.draft.clone() else {
            return;
        };
        let texture = self.drawn(ui.ctx(), creature, &draft);
        // Every picture in the notebook is at a whole number of points to the pixel: two here
        // where there is room, one where there is not.
        let room = ui.available_width();
        let square = 2.0 * 96.0 + 14.0;
        if beside {
            self.picture(ui, texture, 2, 2.0);
            self.actions(ui, creature, &mut draft, outcome, true);
        } else if room >= square + 200.0 * journal::text_scale(ui) {
            ui.horizontal_top(|ui| {
                self.picture(ui, texture, 2, 2.0);
                ui.vertical(|ui| self.actions(ui, creature, &mut draft, outcome, false));
            });
        } else {
            let unit = if room >= 4.0 * 96.0 + 14.0 { 2.0 } else { 1.0 };
            self.picture(ui, texture, VIEWS, unit);
            self.actions(ui, creature, &mut draft, outcome, false);
        }
        self.draft = Some(draft);
    }

    /// The four views, `columns` to a row, at `unit` points to the pixel.
    fn picture(&mut self, ui: &mut Ui, texture: egui::TextureId, columns: u32, unit: f32) {
        let rows = VIEWS / columns;
        let cell = 48.0 * unit;
        let frame = egui::Frame::new()
            .fill(forest())
            .stroke(egui::Stroke::new(2.0, gold()))
            .inner_margin(6)
            .show(ui, |ui| {
                let (rect, response) = ui.allocate_exact_size(
                    egui::vec2(cell * columns as f32, cell * rows as f32),
                    egui::Sense::hover(),
                );
                response.widget_info(|| {
                    egui::WidgetInfo::labeled(
                        egui::WidgetType::Image,
                        true,
                        "The new look facing you, side-on, from behind and waving",
                    )
                });
                for view in 0..VIEWS {
                    let at = rect.min
                        + egui::vec2(
                            (view % columns) as f32 * cell,
                            (view / columns) as f32 * cell,
                        );
                    let uv = egui::Rect::from_min_max(
                        egui::pos2(view as f32 / VIEWS as f32, 0.0),
                        egui::pos2((view + 1) as f32 / VIEWS as f32, 1.0),
                    );
                    ui.painter().image(
                        texture,
                        egui::Rect::from_min_size(at, egui::vec2(cell, cell)),
                        uv,
                        Color32::WHITE,
                    );
                }
            });
        self.preview_rect = Some(frame.response.rect);
    }

    /// The companion's name, "Keep this look" and "Put back", and what is worth knowing about
    /// keeping it, with a line under the buttons saying a kept look can be undone if `undo_line`.
    fn actions(
        &mut self,
        ui: &mut Ui,
        creature: &Creature,
        draft: &mut Design,
        outcome: &mut SettingsOutcome,
        undo_line: bool,
    ) {
        ui.strong(&creature.name);
        let changed = Some(&*draft) != self.started.as_ref();
        ui.horizontal_wrapped(|ui| {
            let keep = ui
                .add_enabled(changed, egui::Button::new("Keep this look").fill(mint()))
                .on_hover_text("Kept looks can be undone from the footer.")
                .on_disabled_hover_text("Nothing has changed yet.");
            if keep.clicked() {
                let kept = draft.normalized();
                outcome.restyle = Some((creature.id, kept.clone()));
                self.started = Some(kept.clone());
                *draft = kept;
            }
            if ui
                .add_enabled(changed, egui::Button::new("Put back"))
                .on_disabled_hover_text("Nothing has changed yet.")
                .clicked()
                && let Some(started) = &self.started
            {
                *draft = started.clone();
            }
        });
        if undo_line {
            ui.small("Kept looks can be undone from the footer, like any change to the colony.");
        }
        // Anatomy is new in this release: a Formiga Hill or Home built before it cannot draw it.
        if draft
            .form
            .recipe()
            .is_some_and(|recipe| recipe.details.has_anatomy())
        {
            ui.small(
                "Antlers, ear tufts, a ruff, a brush and a plume need Formiga Hill and Formiga \
                 Home from this release or later. An older one will say this companion is too \
                 new to visit.",
            );
        }
    }

    /// The draft on the companion, facing the screen, side-on, turned away and waving, drawn again
    /// only when the draft changes.
    fn drawn(
        &mut self,
        context: &egui::Context,
        creature: &Creature,
        draft: &Design,
    ) -> egui::TextureId {
        let stale = self
            .preview
            .as_ref()
            .is_none_or(|(drawn, _)| drawn != draft);
        if stale {
            let mut wearing = creature.clone();
            formiga_farm_contract::apply_design(&mut wearing, draft);
            let genome = &wearing.appearance;
            let face = formiga_art::FaceRenderState {
                expression: formiga_art::ExpressionKind::Content,
                eyelids: formiga_art::EyelidPose::Open,
                gaze: formiga_art::GazeDirection::default(),
            };
            let mut strip = Canvas::new(48 * VIEWS, 48);
            let tiles = [
                CreatureRenderer::render_turned_frame(
                    genome,
                    formiga_art::View::Front,
                    None,
                    true,
                    face,
                ),
                CreatureRenderer::render_turned_frame(
                    genome,
                    formiga_art::View::Side,
                    None,
                    true,
                    face,
                ),
                CreatureRenderer::render_turned_frame(
                    genome,
                    formiga_art::View::Back,
                    None,
                    true,
                    face,
                ),
                Some(CreatureRenderer::render_studio_frame(
                    genome,
                    ActionKind::Greet,
                    1,
                )),
            ];
            for (index, tile) in tiles.into_iter().enumerate() {
                let Some(tile) = tile else { continue };
                for y in 0..48 {
                    for x in 0..48 {
                        strip.set(index as i32 * 48 + x, y, tile.get(x, y));
                    }
                }
            }
            let texture = upload(context, "restyle-preview", &strip);
            self.preview = Some((draft.clone(), texture));
        }
        self.preview
            .as_ref()
            .map(|(_, texture)| texture.id())
            .expect("drawn above")
    }
}

/// Colours to offer beside every colour: the coats and accents of everyone in the colony.
fn colony_swatches(save: &SaveFile) -> Vec<[u8; 3]> {
    let mut seen = Vec::new();
    for creature in &save.creatures {
        if let Some(design) = creature.appearance.design {
            for color in [design.coat, design.accent] {
                if !seen.contains(&color) {
                    seen.push(color);
                }
            }
        }
    }
    seen.truncate(12);
    seen
}

/// One labelled choice among `options`, as a menu.
fn choose<T: PartialEq + Copy>(
    ui: &mut Ui,
    flow: &mut Flow,
    label: &str,
    value: &mut T,
    options: &[(T, &str)],
) -> bool {
    let mut changed = false;
    ui.vertical(|ui| {
        ui.label(label);
        let current = options
            .iter()
            .find(|(option, _)| option == value)
            .map_or("", |(_, name)| *name);
        egui::ComboBox::from_id_salt(("restyle", label))
            .selected_text(current)
            .show_ui(ui, |ui| {
                for (option, name) in options {
                    if ui.selectable_label(*value == *option, *name).clicked() && *value != *option
                    {
                        *value = *option;
                        changed = true;
                    }
                }
            });
    });
    flow.cell(ui);
    changed
}

fn slider(ui: &mut Ui, flow: &mut Flow, label: &str, value: &mut u8, low: u8, high: u8) {
    ui.vertical(|ui| {
        ui.label(label);
        ui.add(egui::Slider::new(value, low..=high));
    });
    flow.cell(ui);
}

fn color(ui: &mut Ui, flow: &mut Flow, label: &str, value: &mut [u8; 3], swatches: &[[u8; 3]]) {
    ui.vertical(|ui| {
        ui.label(label);
        ui.horizontal_wrapped(|ui| {
            ui.color_edit_button_srgb(value);
            for swatch in swatches {
                let fill = Color32::from_rgb(swatch[0], swatch[1], swatch[2]);
                let button = egui::Button::new("")
                    .fill(fill)
                    .min_size(egui::vec2(14.0, 14.0));
                let name = format!(
                    "{label} #{:02x}{:02x}{:02x}, as someone in the colony wears it",
                    swatch[0], swatch[1], swatch[2]
                );
                let response = ui.add(button);
                response.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::WidgetType::Button, true, name.clone())
                });
                if response.on_hover_text(name.clone()).clicked() {
                    *value = *swatch;
                }
            }
        });
    });
    flow.cell(ui);
}

/// Choices laid out a few to a row, as many as the width and the text size have room for, each
/// its label over its control, so the page's width is used rather than one narrow column.
pub(crate) struct Flow {
    per_row: usize,
    in_row: usize,
}

impl Flow {
    /// The next `count` choices belong together: start them on a row of their own if they would
    /// not all fit on what is left of this one.
    fn together(&mut self, ui: &mut Ui, count: usize) {
        if self.in_row > 0 && self.in_row + count.min(self.per_row) > self.per_row {
            ui.end_row();
            self.in_row = 0;
        }
    }

    /// One choice has been laid out.
    fn cell(&mut self, ui: &mut Ui) {
        self.in_row += 1;
        if self.in_row == self.per_row {
            ui.end_row();
            self.in_row = 0;
        }
    }
}

/// How many choices go to a row in `width` at the window's text size: one to three.
pub(crate) fn per_row(width: f32, text_scale: f32) -> usize {
    ((width / (170.0 * text_scale)).floor() as usize).clamp(1, 3)
}

fn grid(ui: &mut Ui, id: &str, most: usize, add: impl FnOnce(&mut Ui, &mut Flow)) {
    let per_row = per_row(ui.available_width(), journal::text_scale(ui)).min(most);
    let mut flow = Flow { per_row, in_row: 0 };
    egui::Grid::new(("restyle-grid", id))
        .num_columns(per_row)
        .spacing([18.0, 8.0])
        .show(ui, |ui| add(ui, &mut flow));
}

fn body_controls(ui: &mut Ui, r: &mut CreatureDesign) {
    journal::kicker(ui, "Body");
    grid(ui, "body", 3, |ui, flow| {
        let plans: Vec<(BodyPlan, &str)> = BodyPlan::ALL
            .iter()
            .map(|plan| (*plan, plan.label()))
            .collect();
        choose(ui, flow, "Body", &mut r.body, &plans);
        flow.together(ui, 2);
        slider(ui, flow, "Width", &mut r.width, 8, 12);
        slider(ui, flow, "Height", &mut r.height, 7, 11);
        flow.together(ui, 2);
        slider(ui, flow, "Head size", &mut r.head, 7, 9);
        slider(ui, flow, "Leg length", &mut r.legs, 3, 6);
    });
}

fn parts_controls(ui: &mut Ui, r: &mut CreatureDesign) {
    journal::kicker(ui, "Parts");
    grid(ui, "parts", 3, |ui, flow| {
        choose(
            ui,
            flow,
            "Ears",
            &mut r.ears,
            &[
                (EarStyle::None, "None"),
                (EarStyle::Round, "Round"),
                (EarStyle::Pointed, "Pointed"),
                (EarStyle::Long, "Long"),
                (EarStyle::Floppy, "Floppy"),
                (EarStyle::Tuft, "Tufts"),
            ],
        );
        slider(ui, flow, "Ear size", &mut r.ear_size, 3, 7);
        flow.together(ui, 2);
        choose(
            ui,
            flow,
            "Muzzle",
            &mut r.muzzle,
            &[(0, "None"), (1, "Small"), (2, "Wide")],
        );
        choose(
            ui,
            flow,
            "Crown",
            &mut r.classic.crown,
            &[(0, "Its ears"), (1, "Antennae"), (2, "Sprouts")],
        );
        flow.together(ui, 2);
        choose(
            ui,
            flow,
            "Tail",
            &mut r.tail,
            &[
                (0, "None"),
                (1, "Puff"),
                (2, "Taper"),
                (3, "Tufted taper"),
                (4, "Big and round"),
            ],
        );
        choose(
            ui,
            flow,
            "Odd tail",
            &mut r.classic.tail,
            &[(0, "None"), (1, "Curl"), (2, "Star")],
        );
        flow.together(ui, 2);
        choose(
            ui,
            flow,
            "Limbs",
            &mut r.classic.limbs,
            &[(0, "Paws"), (1, "Nubs"), (2, "Stick legs")],
        );
        // Details belong to recipes drawn since archetypes; an older recipe has none to change.
        if r.archetype > 0 {
            if r.body != BodyPlan::Winged {
                choose(
                    ui,
                    flow,
                    "Wing-nubs",
                    &mut r.details.wings,
                    &[(0, "None"), (1, "Membrane"), (2, "Feathered")],
                );
            }
            let head = [
                (0, "None", None),
                (1, "Nubs", None),
                (2, "Horns swept back", None),
                (3, "Antlers", Some(Anatomy::Antlers)),
                (4, "Ear tufts", Some(Anatomy::EarTufts)),
            ];
            let shape = *r;
            flow.together(ui, 2);
            fitting(ui, flow, "On the head", &mut r.details.horns, &head, &shape);
            let neck = [
                (0, "None", None),
                (1, "Belly patch", None),
                (2, "Ruff", Some(Anatomy::Ruff)),
            ];
            let shape = *r;
            fitting(ui, flow, "Chest", &mut r.details.belly, &neck, &shape);
            if matches!(r.tail, 2 | 3) && r.classic.tail == 0 {
                let tip = [
                    (0, "None", None),
                    (1, "Flame", None),
                    (2, "Bobble", None),
                    (3, "Brush", Some(Anatomy::Brush)),
                    (4, "Plume", Some(Anatomy::Plume)),
                ];
                let shape = *r;
                fitting(ui, flow, "Tail tip", &mut r.details.tip, &tip, &shape);
            }
        }
    });
}

/// A choice among parts, the ones this body cannot carry shown but not offered.
fn fitting(
    ui: &mut Ui,
    flow: &mut Flow,
    label: &str,
    value: &mut u8,
    options: &[(u8, &str, Option<Anatomy>)],
    recipe: &CreatureDesign,
) {
    ui.vertical(|ui| {
        ui.label(label);
        let current = options
            .iter()
            .find(|(option, _, _)| option == value)
            .map_or("", |(_, name, _)| *name);
        egui::ComboBox::from_id_salt(("restyle", label))
            .selected_text(current)
            .show_ui(ui, |ui| {
                for (option, name, part) in options {
                    let fits = part.is_none_or(|part| part.fits(recipe));
                    let response = ui
                        .add_enabled(fits, egui::Button::selectable(*value == *option, *name))
                        .on_disabled_hover_text("This body has nowhere to carry it.");
                    if response.clicked() {
                        *value = *option;
                    }
                }
            });
    });
    flow.cell(ui);
}

fn finish_controls(ui: &mut Ui, r: &mut CreatureDesign, swatches: &[[u8; 3]]) {
    journal::kicker(ui, "Colours and markings");
    // A colour and its swatches take a row each; the rest go a few to a row.
    grid(ui, "colours", 1, |ui, flow| {
        color(ui, flow, "Coat", &mut r.coat, swatches);
        color(ui, flow, "Accent", &mut r.accent, swatches);
        if r.details.belly > 0 {
            color(
                ui,
                flow,
                "Belly colour",
                &mut r.details.belly_color,
                swatches,
            );
        }
        if r.details.tip > 0 {
            color(ui, flow, "Tip colour", &mut r.details.tip_color, swatches);
        }
    });
    grid(ui, "markings", 3, |ui, flow| {
        let mut candy = r.classic.coat > 0;
        ui.vertical(|ui| {
            ui.label("Coat style");
            if ui.checkbox(&mut candy, "Candy colours").changed() {
                r.classic.coat = u8::from(candy);
            }
        });
        flow.cell(ui);
        flow.together(ui, 2);
        choose(
            ui,
            flow,
            "Marking",
            &mut r.marking,
            &[
                (0, "None"),
                (1, "Pale belly"),
                (6, "Two pale spots"),
                (5, "Band"),
                (4, "Patch"),
                (2, "Stripes"),
                (3, "Spots"),
            ],
        );
        choose(
            ui,
            flow,
            "Pattern",
            &mut r.classic.pattern,
            &[(0, "None"), (1, "Stripes"), (2, "Spots"), (3, "Patches")],
        );
    });
}

fn face_controls(ui: &mut Ui, design: &mut Design) {
    journal::kicker(ui, "Face");
    grid(ui, "face", 3, |ui, flow| {
        if let Form::Companion { recipe } = &mut design.form {
            if recipe.archetype > 0 {
                let layouts: Vec<(u8, &str)> = LAYOUTS
                    .iter()
                    .enumerate()
                    .map(|(index, name)| (index as u8 + 1, *name))
                    .collect();
                choose(ui, flow, "Layout", &mut recipe.face_template, &layouts);
            } else {
                choose(
                    ui,
                    flow,
                    "Eyes",
                    &mut recipe.classic.face,
                    &[
                        (0, "Large"),
                        (1, "Mask"),
                        (2, "Visor"),
                        (3, "Beads"),
                        (4, "Tall"),
                        (5, "Square"),
                    ],
                );
            }
            *recipe = recipe.bounded();
        }
        let face = &mut design.face;
        choose(
            ui,
            flow,
            "Eye shape",
            &mut face.eye_shape,
            &[
                (EyeShape::Round, "Round"),
                (EyeShape::Tall, "Tall"),
                (EyeShape::SoftSquare, "Soft square"),
            ],
        );
        flow.together(ui, 3);
        slider(ui, flow, "Eye size", &mut face.eye_size, 1, 2);
        slider(ui, flow, "Eye spacing", &mut face.eye_spacing, 4, 7);
        ui.vertical(|ui| {
            ui.label("Eye height");
            ui.add(egui::Slider::new(&mut face.vertical_offset, -1..=1));
        });
        flow.cell(ui);
        flow.together(ui, 2);
        choose(
            ui,
            flow,
            "Pupils",
            &mut face.pupil_style,
            &[
                (PupilStyle::Dot, "Dot"),
                (PupilStyle::Wide, "Wide"),
                (PupilStyle::Spark, "Spark"),
            ],
        );
        choose(
            ui,
            flow,
            "Shine",
            &mut face.highlight_style,
            &[
                (HighlightStyle::Single, "One"),
                (HighlightStyle::Double, "Two"),
                (HighlightStyle::Diagonal, "Slanted"),
            ],
        );
        flow.together(ui, 3);
        choose(
            ui,
            flow,
            "Brows",
            &mut face.brow_style,
            &[
                (BrowStyle::None, "None"),
                (BrowStyle::Soft, "Soft"),
                (BrowStyle::Bold, "Bold"),
            ],
        );
        choose(
            ui,
            flow,
            "Mouth",
            &mut face.mouth_style,
            &[
                (MouthStyle::Tiny, "Tiny"),
                (MouthStyle::Smile, "Smile"),
                (MouthStyle::Cat, "Cat"),
                (MouthStyle::Beak, "Beak"),
            ],
        );
        choose(
            ui,
            flow,
            "Cheeks",
            &mut face.cheek_style,
            &[
                (CheekStyle::None, "None"),
                (CheekStyle::Dots, "Dots"),
                (CheekStyle::Blush, "Blush"),
            ],
        );
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An edited look is kept on the companion and drawn on it, survives the colony being saved
    /// and opened again, and Undo puts back exactly the look it had.
    #[test]
    fn a_restyled_look_is_kept_drawn_saved_and_undone() {
        let desktop = DesktopSnapshot::default();
        let mut world = World::new([29; 32], OffsetDateTime::UNIX_EPOCH, &desktop);
        let creature = world.save.creatures[0].clone();
        let id = creature.id;
        let before = creature.appearance.clone();
        let mut design = Design::of(&creature.appearance);
        let Form::Companion { recipe } = &mut design.form else {
            panic!("a new companion has a recipe");
        };
        recipe.muzzle = (recipe.muzzle + 1) % 3;
        recipe.coat = [200, 120, 90];
        recipe.archetype = recipe.archetype.max(1);
        recipe.face_template = recipe.face_template.max(1);
        recipe.body = BodyPlan::Round;
        *recipe = Anatomy::Ruff.put_on(recipe.bounded(), [240, 226, 200]);
        assert!(Anatomy::Ruff.is_on(&recipe.details));
        design.face.mouth_style = MouthStyle::Cat;
        let design = design.normalized();

        assert!(keep_look(&mut world, id, &design));
        let kept = world.save.creatures.iter().find(|c| c.id == id).unwrap();
        assert_eq!(Design::of(&kept.appearance), design);
        assert_eq!(kept.name, creature.name);
        assert_eq!(kept.temperament, creature.temperament);
        // Drawn on it: its standing frame is not the one it had.
        let face = formiga_art::FaceRenderState {
            expression: formiga_art::ExpressionKind::Neutral,
            eyelids: formiga_art::EyelidPose::Open,
            gaze: formiga_art::GazeDirection::default(),
        };
        let drawn = |genome: &AppearanceGenome| {
            CreatureRenderer::render_composited_frame(genome, ActionKind::Idle, 0, true, true, face)
        };
        assert_ne!(drawn(&kept.appearance), drawn(&before));

        // Saved and opened again.
        let path = std::env::temp_dir().join(format!(
            "formiga-restyle-{}-{}.json",
            std::process::id(),
            id
        ));
        let store = formiga_core::SaveStore::new(&path);
        store.save(&world.save).unwrap();
        let reopened = store.load().unwrap().unwrap().into_inner();
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(path.with_extension("json.bak"));
        let again = reopened.creatures.iter().find(|c| c.id == id).unwrap();
        assert_eq!(Design::of(&again.appearance), design);

        // Undone: exactly the look it had.
        let edit = world.undo_last_edit().unwrap();
        assert_eq!(edit.describe(), format!("restyling {}", creature.name));
        let undone = world.save.creatures.iter().find(|c| c.id == id).unwrap();
        assert_eq!(undone.appearance, before);
        assert_eq!(undone.origin, creature.origin);
    }

    /// A look out of range is refused, never repaired, and nothing is recorded.
    #[test]
    fn a_look_out_of_range_is_not_kept() {
        let desktop = DesktopSnapshot::default();
        let mut world = World::new([31; 32], OffsetDateTime::UNIX_EPOCH, &desktop);
        let id = world.save.creatures[0].id;
        let mut design = Design::of(&world.save.creatures[0].appearance);
        if let Form::Companion { recipe } = &mut design.form {
            recipe.width = 99;
        }
        let before = world.save.creatures[0].appearance.clone();
        assert!(!keep_look(&mut world, id, &design));
        assert_eq!(world.save.creatures[0].appearance, before);
        assert_eq!(world.undoable_edits(), 0);
    }
}
