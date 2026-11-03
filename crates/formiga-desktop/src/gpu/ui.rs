//! The on-desktop interface: bubbles, from the talk sheet, and the creature menu and its side
//! strip, from the interface atlas.
use super::*;

/// Where a creature's bubble goes, in monitor-local physical pixels, for a bubble `size` art
/// pixels across and down, tail included.
///
/// The tail's tip is the middle of the bubble's bottom row, and it lands one art pixel above the
/// crown of the head, so the tail reaches down to a one-pixel gap and never covers the creature. A
/// creature at the very top or the very edge of a display keeps its whole bubble on screen.
pub(super) fn talk_origin(
    centre_x: f32,
    head_top: f32,
    scale: f32,
    drawable: PhysicalSize<u32>,
    (width, height): (f32, f32),
) -> (f32, f32) {
    let x = centre_x - (width / 2.0).floor() * scale;
    let y = head_top - (height + 1.0) * scale;
    (
        x.clamp(0.0, (drawable.width as f32 - width * scale).max(0.0)),
        y.clamp(0.0, (drawable.height as f32 - height * scale).max(0.0)),
    )
}

/// The open menu, drawn: its frame, one quad per cell, and the label tab under whichever cell is
/// hovered. Below the creature the frame sprite is drawn upside down, which puts the notch on top
/// pointing up; the frame is symmetric about its body, so only the notch actually moves, and the
/// cells and the tab are placed the right way up on top of it.
pub(super) fn menu_quads(menu: MenuView<'_>, drawable: PhysicalSize<u32>) -> Vec<Vertex> {
    let placement = menu.placement;
    let art = placement.art_scale;
    let Some(frame) = UiAtlasRenderer::menu_frame(menu.items.len() as u8) else {
        return Vec::new();
    };
    let mut vertices = Vec::with_capacity(6 * (menu.items.len() + 2));
    vertices.extend_from_slice(&ui_atlas_quad(
        frame,
        placement.x,
        placement.y,
        art,
        placement.below,
        1.0,
        drawable,
    ));
    let body = placement.body_rect();
    for (index, icon) in menu.items.iter().enumerate() {
        let Some(cell) = menu.layout.cell(index) else {
            continue;
        };
        vertices.extend_from_slice(&ui_atlas_quad(
            UiAtlasRenderer::menu_icon(*icon, menu.hovered == Some(index)),
            body.x + cell.x as f32 * art,
            body.y + cell.y as f32 * art,
            art,
            false,
            1.0,
            drawable,
        ));
    }
    if let Some(hovered) = menu.hovered
        && let (Some(tab), Some(icon)) = (menu.layout.label_tab(hovered), menu.layout.item(hovered))
    {
        vertices.extend_from_slice(&ui_atlas_quad(
            UiAtlasRenderer::menu_label(icon),
            placement.x + tab.x as f32 * art,
            placement.y + tab.y as f32 * art,
            art,
            false,
            1.0,
            drawable,
        ));
    }
    if let Some(side) = menu.side {
        vertices.extend(side_strip_quads(side, drawable));
    }
    vertices
}

/// A strip beside the menu: its plain tray, its cells, and the label of whichever is hovered,
/// hung level with the menu's own labels.
pub(super) fn side_strip_quads(side: SideView<'_>, drawable: PhysicalSize<u32>) -> Vec<Vertex> {
    let placement = side.placement;
    let art = placement.art_scale;
    let Some(frame) = UiAtlasRenderer::menu_frame_plain(side.layout.len() as u8) else {
        return Vec::new();
    };
    let mut vertices = Vec::with_capacity(6 * (side.layout.len() + 2));
    vertices.extend_from_slice(&ui_atlas_quad(
        frame,
        placement.x,
        placement.y,
        art,
        false,
        1.0,
        drawable,
    ));
    for index in 0..side.layout.len() {
        let (Some(cell), Some(icon)) = (side.layout.cell(index), side.layout.item(index)) else {
            continue;
        };
        vertices.extend_from_slice(&ui_atlas_quad(
            UiAtlasRenderer::menu_icon(icon, side.hovered == Some(index)),
            placement.x + cell.x as f32 * art,
            placement.y + cell.y as f32 * art,
            art,
            false,
            1.0,
            drawable,
        ));
    }
    if let Some(hovered) = side.hovered
        && let (Some(tab), Some(icon)) = (side.layout.label_tab(hovered), side.layout.item(hovered))
    {
        vertices.extend_from_slice(&ui_atlas_quad(
            UiAtlasRenderer::menu_label(icon),
            placement.x + tab.x as f32 * art,
            placement.label_y,
            art,
            false,
            1.0,
            drawable,
        ));
    }
    vertices
}

/// One nearest-sampled quad out of the UI atlas, placed by its top-left corner in monitor-local
/// physical pixels.
/// One quad off the talk sheet at `x`, `y`, its art pixels `scale` screen pixels each: the sheet
/// is drawn [`TALK_DETAIL`] texture pixels to the art pixel, which is what makes a bubble finer
/// than the companion under it.
pub(super) fn talk_quad(
    rect: SpriteRect,
    x: f32,
    y: f32,
    scale: f32,
    occlusion_enabled: f32,
    drawable: PhysicalSize<u32>,
) -> [Vertex; 6] {
    let detail = TALK_DETAIL as f32;
    let (w, h) = (
        rect.width as f32 / detail * scale,
        rect.height as f32 / detail * scale,
    );
    let left = x / drawable.width as f32 * 2.0 - 1.0;
    let right = (x + w) / drawable.width as f32 * 2.0 - 1.0;
    let top = 1.0 - y / drawable.height as f32 * 2.0;
    let bottom = 1.0 - (y + h) / drawable.height as f32 * 2.0;
    let u_left = rect.x as f32 / TALK_SHEET_WIDTH as f32;
    let u_right = (rect.x + rect.width) as f32 / TALK_SHEET_WIDTH as f32;
    let v_top = rect.y as f32 / TALK_SHEET_HEIGHT as f32;
    let v_bottom = (rect.y + rect.height) as f32 / TALK_SHEET_HEIGHT as f32;
    let vertex = |position, uv| Vertex {
        position,
        uv,
        occlusion_enabled,
    };
    [
        vertex([left, top], [u_left, v_top]),
        vertex([right, top], [u_right, v_top]),
        vertex([right, bottom], [u_right, v_bottom]),
        vertex([left, top], [u_left, v_top]),
        vertex([right, bottom], [u_right, v_bottom]),
        vertex([left, bottom], [u_left, v_bottom]),
    ]
}

pub(super) fn ui_atlas_quad(
    rect: SpriteRect,
    x: f32,
    y: f32,
    scale: f32,
    flip_vertically: bool,
    occlusion_enabled: f32,
    drawable: PhysicalSize<u32>,
) -> [Vertex; 6] {
    let left = x / drawable.width as f32 * 2.0 - 1.0;
    let right = (x + rect.width as f32 * scale) / drawable.width as f32 * 2.0 - 1.0;
    let top = 1.0 - y / drawable.height as f32 * 2.0;
    let bottom = 1.0 - (y + rect.height as f32 * scale) / drawable.height as f32 * 2.0;
    let u_left = rect.x as f32 / UI_ATLAS_WIDTH as f32;
    let u_right = (rect.x + rect.width) as f32 / UI_ATLAS_WIDTH as f32;
    let mut v_top = rect.y as f32 / UI_ATLAS_HEIGHT as f32;
    let mut v_bottom = (rect.y + rect.height) as f32 / UI_ATLAS_HEIGHT as f32;
    if flip_vertically {
        std::mem::swap(&mut v_top, &mut v_bottom);
    }
    let vertex = |position, uv| Vertex {
        position,
        uv,
        occlusion_enabled,
    };
    [
        vertex([left, top], [u_left, v_top]),
        vertex([right, top], [u_right, v_top]),
        vertex([right, bottom], [u_right, v_bottom]),
        vertex([left, top], [u_left, v_top]),
        vertex([right, bottom], [u_right, v_bottom]),
        vertex([left, bottom], [u_left, v_bottom]),
    ]
}

impl OverlayRenderer {
    pub(super) fn bubble_vertices(
        &self,
        creature: &Creature,
        display_scale: u8,
    ) -> Option<[Vertex; 6]> {
        let bubble = self
            .bubble
            .as_ref()
            .filter(|bubble| bubble.creature_id == creature.id)?;
        let scale = 2.0;
        let width = bubble.width as f32 * scale;
        let height = bubble.height as f32 * scale;
        let (local_x, contact_y) = self.drawn_contact(creature, display_scale);
        let creature_height = FRAME_SIZE as f32 * f32::from(display_scale);
        let center_x = local_x.clamp(width / 2.0, self.layout.width as f32 - width / 2.0);
        let bottom_px = (contact_y - creature_height - 6.0).max(height);
        let top_px = bottom_px - height;
        let left = (center_x - width / 2.0) / self.layout.width as f32 * 2.0 - 1.0;
        let right = (center_x + width / 2.0) / self.layout.width as f32 * 2.0 - 1.0;
        let top = 1.0 - top_px / self.layout.height as f32 * 2.0;
        let bottom = 1.0 - bottom_px / self.layout.height as f32 * 2.0;
        let vertex = |position, uv| Vertex {
            position,
            uv,
            occlusion_enabled: 1.0,
        };
        Some([
            vertex([left, top], [0.0, 0.0]),
            vertex([right, top], [1.0, 0.0]),
            vertex([right, bottom], [1.0, 1.0]),
            vertex([left, top], [0.0, 0.0]),
            vertex([right, bottom], [1.0, 1.0]),
            vertex([left, bottom], [0.0, 1.0]),
        ])
    }

    /// Every bubble this frame, from the talk sheet: a creature answering the person at the desk,
    /// and what companions are saying to each other. Each is its frame, then its one or two
    /// pictures once it has grown in. An answer to the person wins over talk.
    pub(super) fn talk_vertices(
        &self,
        visible: &[&Creature],
        display_scale: u8,
        ui: OverlayUi<'_>,
    ) -> Vec<Vertex> {
        let answers = ui.bubbles.iter().map(|bubble| {
            (
                bubble.creature_id,
                Saying::answering(bubble.icon),
                Some(bubble.growth(ui.reduce_motion)),
            )
        });
        let talk = ui
            .talk
            .iter()
            .filter(|said| {
                !ui.bubbles
                    .iter()
                    .any(|bubble| bubble.creature_id == said.creature_id)
            })
            .map(|said| (said.creature_id, said.saying, said.growth(ui.reduce_motion)));
        let scale = f32::from(display_scale);
        let detail = TALK_DETAIL as f32;
        let mut vertices = Vec::new();
        for (creature_id, saying, growth) in answers.chain(talk) {
            let Some(growth) = growth else {
                continue;
            };
            let Some(creature) = visible.iter().find(|creature| creature.id == creature_id) else {
                continue;
            };
            let Some(sprite) = self.sprite_for(creature) else {
                continue;
            };
            let (centre_x, head_top, _) = self.creature_extent(creature, sprite, display_scale);
            let occlusion = (creature.state.action != ActionKind::Dragged) as u8 as f32;
            let frame = TalkSheet::frame(saying.style, saying.second.is_some(), growth);
            // In art pixels: the tail's tip, at the middle of the bottom row, sits just over the
            // crown.
            let size = (frame.width as f32 / detail, frame.height as f32 / detail);
            let (x, y) = talk_origin(centre_x, head_top, scale, self.layout, size);
            let (x, y) = (self.snap(x), self.snap(y));
            vertices.extend_from_slice(&talk_quad(frame, x, y, scale, occlusion, self.layout));
            if growth == BubbleGrowth::Full {
                let spots = if saying.second.is_some() {
                    &TALK_ICON_AT[1..3]
                } else {
                    &TALK_ICON_AT[0..1]
                };
                for (topic, (ix, iy)) in [Some(saying.first), saying.second]
                    .into_iter()
                    .flatten()
                    .zip(spots)
                {
                    vertices.extend_from_slice(&talk_quad(
                        TalkSheet::icon(topic),
                        x + *ix as f32 * scale,
                        y + *iy as f32 * scale,
                        scale,
                        occlusion,
                        self.layout,
                    ));
                }
            }
        }
        vertices
    }

    /// The creature's centre column, the crown of its head, and the row just past its lowest drawn
    /// pixel, all in monitor-local physical pixels. This is the frame the overlay actually draws,
    /// gesture and pose included, not the 48x48 box it is drawn inside.
    pub(super) fn creature_extent(
        &self,
        creature: &Creature,
        sprite: &SpriteGpu,
        display_scale: u8,
    ) -> (f32, f32, f32) {
        let scale = f32::from(display_scale);
        let (local_x, contact_y) = self.drawn_contact(creature, display_scale);
        let placement = FramePlacement::for_creature(creature, sprite.resting_baseline);
        let frame_top = contact_y + placement.origin_y as f32 * scale;
        let BodyPresentation { clip, frame, .. } = BodyPresentation::for_creature(creature);
        // Mirroring a frame never changes which rows it fills, so one silhouette serves both ways.
        let (top, bottom) = sprite
            .silhouette
            .get(atlas_slot(clip, frame) as usize)
            .copied()
            .unwrap_or((0, FRAME_SIZE as u8));
        (
            local_x,
            frame_top + f32::from(top) * scale,
            frame_top + f32::from(bottom) * scale,
        )
    }

    /// Where a menu for this creature would go, for `creature_menu` to place a strip against.
    /// `None` until the creature's own atlas has been baked, which happens on its first frame.
    pub fn menu_anchor(&self, creature: &Creature, display_scale: u8) -> Option<MenuAnchor> {
        let sprite = self.sprite_for(creature)?;
        let (centre_x, head_top, foot_bottom) =
            self.creature_extent(creature, sprite, display_scale);
        let bounds = self.monitor.bounds;
        let usable = self.monitor.usable_bounds;
        let factor = self.monitor.scale_factor;
        Some(MenuAnchor {
            centre_x,
            head_top,
            foot_bottom,
            art_scale: f32::from(display_scale),
            grid: self.render_divisor as f32,
            usable: LocalRect {
                x: (usable.x - bounds.x) * factor,
                y: (usable.y - bounds.y) * factor,
                width: usable.width * factor,
                height: usable.height * factor,
            },
            monitor_origin: Point {
                x: bounds.x,
                y: bounds.y,
            },
            scale_factor: factor,
        })
    }

    /// Where a menu for a house goes: over its roof, as a creature's goes over its head.
    pub fn house_menu_anchor(
        &self,
        house: &crate::houses::PlacedHouse,
        display_scale: u8,
    ) -> MenuAnchor {
        let unit = f32::from(display_scale);
        let bounds = self.monitor.bounds;
        let usable = self.monitor.usable_bounds;
        let factor = self.monitor.scale_factor;
        let foot = self.snap((house.foot.y - bounds.y) * factor);
        MenuAnchor {
            centre_x: self.snap((house.foot.x - bounds.x) * factor),
            head_top: foot - (formiga_art::SHELTER_SIZE - house.roof) as f32 * unit,
            foot_bottom: foot,
            art_scale: unit,
            grid: self.render_divisor as f32,
            usable: LocalRect {
                x: (usable.x - bounds.x) * factor,
                y: (usable.y - bounds.y) * factor,
                width: usable.width * factor,
                height: usable.height * factor,
            },
            monitor_origin: Point {
                x: bounds.x,
                y: bounds.y,
            },
            scale_factor: factor,
        }
    }

    /// Every quad sampled from the UI atlas this frame: the open menu's frame, its four cells, and
    /// the label tab under whichever one is hovered.
    pub(super) fn ui_vertices(&self, visible: &[&Creature], ui: OverlayUi<'_>) -> Vec<Vertex> {
        let mut vertices = Vec::new();
        if let Some(menu) = ui.menu
            && menu
                .creature_id
                .is_none_or(|creature_id| visible.iter().any(|creature| creature.id == creature_id))
        {
            vertices.extend(menu_quads(menu, self.layout));
        }
        vertices
    }
}
