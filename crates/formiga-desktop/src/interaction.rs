use crate::platform;
use anyhow::{Context, Result};
use formiga_art::{AnimationSpec, CreatureRenderer, FRAME_SIZE};
use formiga_core::{Creature, CreatureId, CursorSnapshot, DesktopRect, MonitorInfo, Settings};
use std::sync::Arc;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event_loop::ActiveEventLoop;
use winit::window::{Cursor, CursorIcon, Window, WindowId, WindowLevel};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct MaskSignature {
    action: formiga_core::ActionKind,
    frame: u8,
    facing_right: bool,
    reduce_motion: bool,
    scale: u8,
}

pub struct InteractionProxy {
    pub window: Arc<Window>,
    pub creature_id: CreatureId,
    monitor_id: u64,
    logical_bounds: DesktopRect,
    mask: Vec<bool>,
    signature: Option<MaskSignature>,
    hit_enabled: bool,
}

impl InteractionProxy {
    pub fn new(event_loop: &ActiveEventLoop, creature_id: CreatureId) -> Result<Self> {
        let window = Arc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_title("Formiga creature interaction")
                        .with_inner_size(PhysicalSize::new(FRAME_SIZE, FRAME_SIZE))
                        .with_resizable(false)
                        .with_decorations(false)
                        .with_transparent(true)
                        .with_window_level(WindowLevel::AlwaysOnTop)
                        .with_active(false)
                        .with_visible(false),
                )
                .context("create creature interaction proxy")?,
        );
        platform::configure_interaction_proxy(&window);
        window.set_cursor(Cursor::Icon(CursorIcon::Grab));
        Ok(Self {
            window,
            creature_id,
            monitor_id: 0,
            logical_bounds: DesktopRect::default(),
            mask: vec![false; (FRAME_SIZE * FRAME_SIZE) as usize],
            signature: None,
            hit_enabled: false,
        })
    }

    pub fn id(&self) -> WindowId {
        self.window.id()
    }

    pub fn sync(
        &mut self,
        creature: &Creature,
        settings: &Settings,
        monitor: &MonitorInfo,
        overlay_origin: PhysicalPosition<i32>,
        cursor: CursorSnapshot,
        dragging: bool,
    ) {
        self.monitor_id = monitor.id;
        let scale = settings.display_scale;
        let physical_size = FRAME_SIZE * u32::from(scale);
        let logical_size = physical_size as f32 / monitor.scale_factor;
        self.logical_bounds = DesktopRect {
            x: creature.state.position.x - logical_size * 0.5,
            y: creature.state.position.y - logical_size,
            width: logical_size,
            height: logical_size,
        };
        let local_anchor_x = (creature.state.position.x - monitor.bounds.x) * monitor.scale_factor;
        let local_anchor_y = (creature.state.position.y - monitor.bounds.y) * monitor.scale_factor;
        let position = PhysicalPosition::new(
            overlay_origin.x + (local_anchor_x - physical_size as f32 * 0.5).round() as i32,
            overlay_origin.y + (local_anchor_y - physical_size as f32).round() as i32,
        );
        self.window.set_outer_position(position);
        let _ = self
            .window
            .request_inner_size(PhysicalSize::new(physical_size, physical_size));

        let spec = AnimationSpec::for_action(creature.state.action);
        let frame = ((creature.state.action_elapsed * spec.fps as f32) as u8) % spec.frames;
        let signature = MaskSignature {
            action: creature.state.action,
            frame,
            facing_right: creature.state.facing_right,
            reduce_motion: settings.reduce_motion,
            scale,
        };
        if self.signature != Some(signature) {
            let canvas = CreatureRenderer::render_frame_with_options(
                &creature.appearance,
                creature.state.action,
                frame,
                creature.state.facing_right,
                settings.reduce_motion,
                0,
            );
            self.mask = canvas.pixels().iter().map(|pixel| pixel.a > 16).collect();
            platform::set_interaction_shape(&self.window, &self.mask, scale);
            self.signature = Some(signature);
        }

        let opaque = cursor.available && self.hit_test(cursor.position.x, cursor.position.y);
        let should_hit = dragging || opaque;
        if should_hit != self.hit_enabled {
            platform::set_interaction_hittest(&self.window, should_hit);
            self.hit_enabled = should_hit;
        }
        let visible = settings.visible
            && settings.direct_manipulation
            && creature.state.arrival_delay_secs <= 0.0;
        self.window.set_visible(visible);
    }

    pub fn hit_test(&self, desktop_x: f32, desktop_y: f32) -> bool {
        hit_mask(&self.mask, self.logical_bounds, desktop_x, desktop_y)
    }

    pub fn begin_capture(&self) {
        platform::begin_interaction_capture(&self.window);
        self.window.set_cursor(Cursor::Icon(CursorIcon::Grabbing));
    }

    pub fn end_capture(&self) {
        platform::end_interaction_capture();
        self.window.set_cursor(Cursor::Icon(CursorIcon::Grab));
    }
}

fn hit_mask(mask: &[bool], bounds: DesktopRect, desktop_x: f32, desktop_y: f32) -> bool {
    if !bounds.contains(formiga_core::Point {
        x: desktop_x,
        y: desktop_y,
    }) {
        return false;
    }
    let x = ((desktop_x - bounds.x) / bounds.width * FRAME_SIZE as f32)
        .floor()
        .clamp(0.0, FRAME_SIZE as f32 - 1.0) as u32;
    let y = ((desktop_y - bounds.y) / bounds.height * FRAME_SIZE as f32)
        .floor()
        .clamp(0.0, FRAME_SIZE as f32 - 1.0) as u32;
    mask[(y * FRAME_SIZE + x) as usize]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logical_alpha_hit_test_rejects_transparent_pixels() {
        let bounds = DesktopRect {
            x: 100.0,
            y: 100.0,
            width: 48.0,
            height: 48.0,
        };
        let mut mask = vec![false; (FRAME_SIZE * FRAME_SIZE) as usize];
        mask[(24 * FRAME_SIZE + 24) as usize] = true;
        assert!(hit_mask(&mask, bounds, 124.5, 124.5));
        assert!(!hit_mask(&mask, bounds, 101.0, 101.0));
        assert!(!hit_mask(&mask, bounds, 10.0, 10.0));
    }
}
