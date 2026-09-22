//! The on-desktop interface drawn from the interface atlas: thought bubbles, the creature menu, and
//! its side strip.
use super::*;

/// Where a creature's bubble cell goes, in monitor-local physical pixels.
///
/// `BUBBLE_ANCHOR` names the blank cell pixel one row under the tail's tip, and it lands on the
/// art pixel directly above the crown of the head, so the tail reaches down to a one-pixel gap and
/// never covers the creature. A creature at the very top or the very edge of a display keeps its
/// whole bubble on screen.
pub(super) fn bubble_origin(
    centre_x: f32,
    head_top: f32,
    scale: f32,
    drawable: PhysicalSize<u32>,
) -> (f32, f32) {
    let width = BUBBLE_CELL.0 as f32 * scale;
    let height = BUBBLE_CELL.1 as f32 * scale;
    let x = centre_x - BUBBLE_ANCHOR.0 as f32 * scale;
    let y = head_top - (BUBBLE_ANCHOR.1 + 1) as f32 * scale;
    (
        x.clamp(0.0, (drawable.width as f32 - width).max(0.0)),
        y.clamp(0.0, (drawable.height as f32 - height).max(0.0)),
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
        let local_x = self
            .snap((creature.state.position.x - self.monitor.bounds.x) * self.monitor.scale_factor);
        let contact_y = self
            .snap((creature.state.position.y - self.monitor.bounds.y) * self.monitor.scale_factor);
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
        let local_x = self
            .snap((creature.state.position.x - self.monitor.bounds.x) * self.monitor.scale_factor);
        let contact_y = self
            .snap((creature.state.position.y - self.monitor.bounds.y) * self.monitor.scale_factor);
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
        let sprite = self.sprites.get(&creature.id)?;
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

    /// Every quad sampled from the UI atlas this frame: one bubble per creature answering, then
    /// the open menu's frame, its four cells, and the label tab under whichever one is hovered.
    pub(super) fn ui_vertices(
        &self,
        visible: &[&Creature],
        display_scale: u8,
        ui: OverlayUi<'_>,
    ) -> Vec<Vertex> {
        let mut vertices = Vec::new();
        for bubble in ui.bubbles {
            let Some(creature) = visible
                .iter()
                .find(|creature| creature.id == bubble.creature_id)
            else {
                continue;
            };
            let Some(sprite) = self.sprites.get(&creature.id) else {
                continue;
            };
            let (centre_x, head_top, _) = self.creature_extent(creature, sprite, display_scale);
            let (x, y) = bubble_origin(centre_x, head_top, f32::from(display_scale), self.layout);
            // A dragged creature opts out of occlusion, and so does what it is thinking.
            let occlusion = (creature.state.action != ActionKind::Dragged) as u8 as f32;
            vertices.extend_from_slice(&ui_atlas_quad(
                UiAtlasRenderer::bubble(bubble.icon, bubble.growth(ui.reduce_motion)),
                self.snap(x),
                self.snap(y),
                f32::from(display_scale),
                false,
                occlusion,
                self.layout,
            ));
        }

        if let Some(menu) = ui.menu
            && visible
                .iter()
                .any(|creature| creature.id == menu.creature_id)
        {
            vertices.extend(menu_quads(menu, self.layout));
        }
        vertices
    }
}
