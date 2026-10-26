//! What every page of the notebook shares, and nothing else: the portraits companions are drawn
//! with, the colony's sheet of found things, the tour that walks across the pages, and the line
//! of feedback in the footer. Each page is drawn by its own state and handed this; a page can
//! change its own state and this, and no other page's.

use super::*;

#[derive(Default)]
pub(crate) struct Shell {
    portraits: BTreeMap<CreatureId, (AppearanceGenome, TextureHandle)>,
    /// The colony's own sheet of found things: one texture the scrapbook cuts every slot out of,
    /// and the very same sheet the desktop samples when a companion holds one up.
    trinket_atlas: Option<([u8; 32], TextureHandle)>,
    /// The creatures' speech bubbles, for the pictures moments are jotted beside.
    bubble_sheet: Option<TextureHandle>,
    /// Where the tour has got to, while it is being taken.
    pub(crate) tour: tour::TourState,
    /// Which section of each page is open, and which was read last.
    pub(crate) sections: sections::Sections,
    /// A line of feedback for the footer, and when it was given.
    pub feedback: Option<(String, std::time::Instant)>,
}

impl Shell {
    pub fn notify(&mut self, message: impl Into<String>) {
        self.feedback = Some((message.into(), std::time::Instant::now()));
    }

    pub(super) fn texture_ids(&self) -> impl Iterator<Item = egui::TextureId> + '_ {
        self.portraits
            .values()
            .map(|(_, t)| t.id())
            .chain(self.trinket_atlas.iter().map(|(_, t)| t.id()))
            .chain(self.bubble_sheet.iter().map(TextureHandle::id))
    }

    pub(super) fn release_images(&mut self) {
        self.portraits.clear();
        self.trinket_atlas = None;
        self.bubble_sheet = None;
    }

    /// A companion's portrait, greeting, drawn once and kept until its look changes.
    pub(crate) fn portrait_texture(
        &mut self,
        ctx: &egui::Context,
        creature: &Creature,
    ) -> TextureHandle {
        let drawn = |ctx: &egui::Context| {
            upload(
                ctx,
                "colony-portrait",
                &CreatureRenderer::render_frame(&creature.appearance, ActionKind::Greet, 2, true),
            )
        };
        let entry = self
            .portraits
            .entry(creature.id)
            .or_insert_with(|| (creature.appearance.clone(), drawn(ctx)));
        if entry.0 != creature.appearance {
            *entry = (creature.appearance.clone(), drawn(ctx));
        }
        entry.1.clone()
    }

    pub fn portrait(&mut self, ui: &mut Ui, creature: &Creature, size: f32) {
        let texture = self.portrait_texture(ui.ctx(), creature);
        egui::Frame::new()
            .fill(forest())
            .stroke(egui::Stroke::new(2.0, gold()))
            .inner_margin(8)
            .show(ui, |ui| {
                ui.add(
                    egui::Image::new(&texture)
                        .maintain_aspect_ratio(false)
                        .fit_to_exact_size(egui::vec2(size, size)),
                );
            });
    }

    /// The creatures' speech bubbles, the sheet the desktop draws them from, uploaded once.
    fn bubble_sheet(&mut self, ctx: &egui::Context) -> TextureHandle {
        self.bubble_sheet
            .get_or_insert_with(|| {
                upload(ctx, "bubble-sheet", &formiga_art::UiAtlasRenderer::render())
            })
            .clone()
    }

    /// One speech bubble, drawn at a whole number of its pixels to each of the sheet's so it stays
    /// crisp: as many as fit in `height`, and never fewer than one.
    pub(crate) fn bubble(
        &mut self,
        painter: &egui::Painter,
        at: egui::Pos2,
        height: f32,
        icon: formiga_core::BubbleIcon,
    ) -> egui::Rect {
        let sheet = self.bubble_sheet(painter.ctx());
        let cell = formiga_art::UiAtlasRenderer::bubble(icon, formiga_core::BubbleGrowth::Full);
        let times = (height / cell.height as f32).floor().max(1.0);
        let rect = egui::Rect::from_min_size(
            at,
            egui::vec2(cell.width as f32, cell.height as f32) * times,
        );
        let (w, h) = (
            formiga_art::UI_ATLAS_WIDTH as f32,
            formiga_art::UI_ATLAS_HEIGHT as f32,
        );
        let uv = egui::Rect::from_min_size(
            egui::pos2(cell.x as f32 / w, cell.y as f32 / h),
            egui::vec2(cell.width as f32 / w, cell.height as f32 / h),
        );
        painter.image(sheet.id(), rect, uv, Color32::WHITE);
        rect
    }

    pub fn retain_portraits(&mut self, creatures: &[Creature]) {
        self.portraits
            .retain(|id, _| creatures.iter().any(|c| c.id == *id));
    }
}

impl Shell {
    /// The colony's sheet of found things, uploaded once. It comes from the colony's own seed and
    /// the colours of whoever lives here, so the book always shows the keepsake the desktop would
    /// show, and a page of sixteen costs one texture rather than sixteen.
    pub(super) fn trinket_atlas(&mut self, ui: &Ui, save: &SaveFile) -> TextureHandle {
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
            // Only the resting half: the pages show every find, and never twinkle one.
            let canvas = TrinketAtlasRenderer::render_resting(save.colony_seed, &members);
            let texture = upload(ui.ctx(), "colony-trinkets", &canvas);
            self.trinket_atlas = Some((save.colony_seed, texture));
        }
        self.trinket_atlas.as_ref().unwrap().1.clone()
    }

    /// One keepsake, drawn square. A keepsake is a 16x16 cell of a sheet sixteen cells wide, and
    /// egui measures an image by its whole texture rather than by the part being shown: left to
    /// keep the sheet's shape it would letterbox a 256x32 atlas into a 48x6 sliver and squash the
    /// keepsake into it. Telling it to fill the box is what makes the cell square again.
    pub(super) fn trinket_image<'a>(
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
    pub(super) fn trinket_uv(variant: u8) -> egui::Rect {
        let (x, y, width, height) = TrinketAtlasRenderer::cell_rect(variant, TRINKET_FRAME_REST);
        let sheet = formiga_art::TRINKET_RESTING_HEIGHT as f32;
        egui::Rect::from_min_size(
            egui::pos2(x as f32 / TRINKET_ATLAS_WIDTH as f32, y as f32 / sheet),
            egui::vec2(
                width as f32 / TRINKET_ATLAS_WIDTH as f32,
                height as f32 / sheet,
            ),
        )
    }
}
