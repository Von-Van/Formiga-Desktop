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
}

/// The four views of the preview: facing the screen, side-on, turned away, and waving.
const VIEWS: u32 = 4;

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

    pub(super) fn show(&mut self, ui: &mut Ui, save: &SaveFile, outcome: &mut SettingsOutcome) {
        egui::CollapsingHeader::new("Restyle a companion")
            .id_salt("restyle-a-companion")
            .default_open(self.target.is_some())
            .show(ui, |ui| self.contents(ui, save, outcome));
    }

    fn contents(&mut self, ui: &mut Ui, save: &SaveFile, outcome: &mut SettingsOutcome) {
        ui.label(
            "Change the parts, proportions, colours and face of someone who already lives here. \
             Who they are stays the same.",
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
        let Some(creature) = save
            .creatures
            .iter()
            .find(|creature| Some(creature.id) == self.target)
        else {
            self.release();
            return;
        };
        let Some(mut draft) = self.draft.clone() else {
            return;
        };
        self.preview(ui, creature, &draft);
        // What to do with the look sits with the picture of it, above the long list of choices.
        let changed = Some(&draft) != self.started.as_ref();
        ui.horizontal_wrapped(|ui| {
            let keep = ui
                .add_enabled(changed, egui::Button::new("Keep this look").fill(mint()))
                .on_disabled_hover_text("Nothing has changed yet.");
            if keep.clicked() {
                let kept = draft.normalized();
                outcome.restyle = Some((creature.id, kept.clone()));
                self.started = Some(kept.clone());
                draft = kept;
            }
            if ui
                .add_enabled(changed, egui::Button::new("Put back"))
                .on_disabled_hover_text("Nothing has changed yet.")
                .clicked()
                && let Some(started) = &self.started
            {
                draft = started.clone();
            }
        });
        ui.small("Kept looks can be undone from the footer, like any change to the colony.");
        // Anatomy is new in this release: a Formiga Hill or Home built before it cannot draw it.
        let has_anatomy = |design: &Design| {
            design
                .form
                .recipe()
                .is_some_and(|recipe| recipe.details.has_anatomy())
        };
        if has_anatomy(&draft) {
            ui.small(
                "Antlers, ear tufts, a ruff, a brush and a plume need Formiga Hill and Formiga \
                 Home from this release or later. An older one will say this companion is too \
                 new to visit.",
            );
        }
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

    /// The draft on the companion, facing the screen, side-on, turned away and waving, drawn again
    /// only when the draft changes.
    fn preview(&mut self, ui: &mut Ui, creature: &Creature, draft: &Design) {
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
            let texture = upload(ui.ctx(), "restyle-preview", &strip);
            self.preview = Some((draft.clone(), texture));
        }
        if let Some((_, texture)) = &self.preview {
            egui::Frame::new()
                .fill(forest())
                .stroke(egui::Stroke::new(2.0, gold()))
                .inner_margin(6)
                .show(ui, |ui| {
                    ui.add(
                        egui::Image::new(texture)
                            .fit_to_exact_size(egui::vec2(48.0 * VIEWS as f32 * 2.0, 96.0))
                            .alt_text("The new look facing you, side-on, from behind and waving"),
                    );
                });
        }
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
    label: &str,
    value: &mut T,
    options: &[(T, &str)],
) -> bool {
    let mut changed = false;
    ui.label(label);
    let current = options
        .iter()
        .find(|(option, _)| option == value)
        .map_or("", |(_, name)| *name);
    egui::ComboBox::from_id_salt(("restyle", label))
        .selected_text(current)
        .show_ui(ui, |ui| {
            for (option, name) in options {
                if ui.selectable_label(*value == *option, *name).clicked() && *value != *option {
                    *value = *option;
                    changed = true;
                }
            }
        });
    ui.end_row();
    changed
}

fn slider(ui: &mut Ui, label: &str, value: &mut u8, low: u8, high: u8) {
    ui.label(label);
    ui.add(egui::Slider::new(value, low..=high));
    ui.end_row();
}

fn color(ui: &mut Ui, label: &str, value: &mut [u8; 3], swatches: &[[u8; 3]]) {
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
    ui.end_row();
}

fn grid(ui: &mut Ui, id: &str, add: impl FnOnce(&mut Ui)) {
    egui::Grid::new(("restyle-grid", id))
        .num_columns(2)
        .spacing([12.0, 6.0])
        .show(ui, add);
}

fn body_controls(ui: &mut Ui, r: &mut CreatureDesign) {
    journal::kicker(ui, "Body");
    grid(ui, "body", |ui| {
        let plans: Vec<(BodyPlan, &str)> = BodyPlan::ALL
            .iter()
            .map(|plan| (*plan, plan.label()))
            .collect();
        choose(ui, "Body", &mut r.body, &plans);
        slider(ui, "Width", &mut r.width, 8, 12);
        slider(ui, "Height", &mut r.height, 7, 11);
        slider(ui, "Head size", &mut r.head, 7, 9);
        slider(ui, "Leg length", &mut r.legs, 3, 6);
    });
}

fn parts_controls(ui: &mut Ui, r: &mut CreatureDesign) {
    journal::kicker(ui, "Parts");
    grid(ui, "parts", |ui| {
        choose(
            ui,
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
        slider(ui, "Ear size", &mut r.ear_size, 3, 7);
        choose(
            ui,
            "Muzzle",
            &mut r.muzzle,
            &[(0, "None"), (1, "Small"), (2, "Wide")],
        );
        choose(
            ui,
            "Crown",
            &mut r.classic.crown,
            &[(0, "Its ears"), (1, "Antennae"), (2, "Sprouts")],
        );
        choose(
            ui,
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
            "Odd tail",
            &mut r.classic.tail,
            &[(0, "None"), (1, "Curl"), (2, "Star")],
        );
        choose(
            ui,
            "Limbs",
            &mut r.classic.limbs,
            &[(0, "Paws"), (1, "Nubs"), (2, "Stick legs")],
        );
        // Details belong to recipes drawn since archetypes; an older recipe has none to change.
        if r.archetype > 0 {
            if r.body != BodyPlan::Winged {
                choose(
                    ui,
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
            fitting(ui, "On the head", &mut r.details.horns, &head, &shape);
            let neck = [
                (0, "None", None),
                (1, "Belly patch", None),
                (2, "Ruff", Some(Anatomy::Ruff)),
            ];
            let shape = *r;
            fitting(ui, "Chest", &mut r.details.belly, &neck, &shape);
            if matches!(r.tail, 2 | 3) && r.classic.tail == 0 {
                let tip = [
                    (0, "None", None),
                    (1, "Flame", None),
                    (2, "Bobble", None),
                    (3, "Brush", Some(Anatomy::Brush)),
                    (4, "Plume", Some(Anatomy::Plume)),
                ];
                let shape = *r;
                fitting(ui, "Tail tip", &mut r.details.tip, &tip, &shape);
            }
        }
    });
}

/// A choice among parts, the ones this body cannot carry shown but not offered.
fn fitting(
    ui: &mut Ui,
    label: &str,
    value: &mut u8,
    options: &[(u8, &str, Option<Anatomy>)],
    recipe: &CreatureDesign,
) {
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
    ui.end_row();
}

fn finish_controls(ui: &mut Ui, r: &mut CreatureDesign, swatches: &[[u8; 3]]) {
    journal::kicker(ui, "Colours and markings");
    grid(ui, "finish", |ui| {
        color(ui, "Coat", &mut r.coat, swatches);
        color(ui, "Accent", &mut r.accent, swatches);
        let mut candy = r.classic.coat > 0;
        ui.label("Coat style");
        if ui.checkbox(&mut candy, "Candy colours").changed() {
            r.classic.coat = u8::from(candy);
        }
        ui.end_row();
        choose(
            ui,
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
            "Pattern",
            &mut r.classic.pattern,
            &[(0, "None"), (1, "Stripes"), (2, "Spots"), (3, "Patches")],
        );
        if r.details.belly > 0 {
            color(ui, "Belly colour", &mut r.details.belly_color, swatches);
        }
        if r.details.tip > 0 {
            color(ui, "Tip colour", &mut r.details.tip_color, swatches);
        }
    });
}

fn face_controls(ui: &mut Ui, design: &mut Design) {
    journal::kicker(ui, "Face");
    grid(ui, "face", |ui| {
        if let Form::Companion { recipe } = &mut design.form {
            if recipe.archetype > 0 {
                let layouts: Vec<(u8, &str)> = LAYOUTS
                    .iter()
                    .enumerate()
                    .map(|(index, name)| (index as u8 + 1, *name))
                    .collect();
                choose(ui, "Layout", &mut recipe.face_template, &layouts);
            } else {
                choose(
                    ui,
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
            "Eye shape",
            &mut face.eye_shape,
            &[
                (EyeShape::Round, "Round"),
                (EyeShape::Tall, "Tall"),
                (EyeShape::SoftSquare, "Soft square"),
            ],
        );
        slider(ui, "Eye size", &mut face.eye_size, 1, 2);
        slider(ui, "Eye spacing", &mut face.eye_spacing, 4, 7);
        ui.label("Eye height");
        ui.add(egui::Slider::new(&mut face.vertical_offset, -1..=1));
        ui.end_row();
        choose(
            ui,
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
            "Shine",
            &mut face.highlight_style,
            &[
                (HighlightStyle::Single, "One"),
                (HighlightStyle::Double, "Two"),
                (HighlightStyle::Diagonal, "Slanted"),
            ],
        );
        choose(
            ui,
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
