//! The little lights about a companion under a magic spell: a few quads at the end of its own
//! stretch of the frame's vertices, sampled from the shadow sheet the overlay already holds, so a
//! spell costs no texture, no draw of its own beyond them, and nothing at all when nobody is under
//! one. Where they go is `formiga_core::magic::Spell::glints`, from the spell's own clock.
use super::*;
use formiga_art::{GLINT_CELL, glint_cell};

impl OverlayRenderer {
    /// Appends a quad for each light about `creature` this frame, if it is under a spell.
    pub(super) fn glint_vertices(
        &self,
        creature: &Creature,
        display_scale: u8,
        vertices: &mut Vec<Vertex>,
    ) {
        let Some(spell) = creature.state.magic else {
            return;
        };
        let presence = &creature.state.presence;
        let glints = spell.glints(
            presence.stepping,
            presence.stride,
            creature.state.facing_right,
        );
        if glints.iter().all(Option::is_none) {
            return;
        }
        let (local_x, ground_y) = self.drawn_contact(creature, display_scale);
        let unit = f32::from(display_scale);
        let half = GLINT_CELL as f32 * unit / 2.0;
        let occlusion_enabled = (creature.state.action != ActionKind::Dragged) as u8 as f32;
        let (sheet_width, sheet_height) = (SHADOW_SHEET_WIDTH as f32, SHADOW_SHEET_HEIGHT as f32);
        let x = |value: f32| value / self.layout.width as f32 * 2.0 - 1.0;
        let y = |value: f32| 1.0 - value / self.layout.height as f32 * 2.0;
        for glint in glints.into_iter().flatten() {
            let centre_x = self.snap(local_x + glint.x * unit);
            let centre_y = self.snap(ground_y + glint.y * unit);
            let (left, right) = (centre_x - half, centre_x + half);
            let (top, bottom) = (centre_y - half, centre_y + half);
            let cell = glint_cell(glint.size);
            let (u_left, u_right) = (
                cell.x as f32 / sheet_width,
                (cell.x + cell.width) as f32 / sheet_width,
            );
            let (v_top, v_bottom) = (
                cell.y as f32 / sheet_height,
                (cell.y + cell.height) as f32 / sheet_height,
            );
            let vertex = |position, uv| Vertex {
                position,
                uv,
                occlusion_enabled,
            };
            vertices.extend_from_slice(&[
                vertex([x(left), y(top)], [u_left, v_top]),
                vertex([x(right), y(top)], [u_right, v_top]),
                vertex([x(right), y(bottom)], [u_right, v_bottom]),
                vertex([x(left), y(top)], [u_left, v_top]),
                vertex([x(right), y(bottom)], [u_right, v_bottom]),
                vertex([x(left), y(bottom)], [u_left, v_bottom]),
            ]);
        }
    }
}
