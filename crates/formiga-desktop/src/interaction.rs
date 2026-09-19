use crate::platform;
use anyhow::{Context, Result};
use formiga_art::{BodyClip, CreatureRenderer, FRAME_SIZE, FramePlacement, MotionSignature};
use formiga_core::{Creature, CreatureId, CursorSnapshot, DesktopRect, MonitorInfo, Settings};
use std::collections::HashMap;
use std::sync::Arc;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event_loop::ActiveEventLoop;
use winit::window::{Cursor, CursorIcon, Window, WindowId, WindowLevel};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct MaskSignature {
    clip: BodyClip,
    frame: u8,
    facing_right: bool,
    reduce_motion: bool,
    scale: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct MaskArtworkSignature {
    clip: BodyClip,
    frame: u8,
    facing_right: bool,
    reduce_motion: bool,
}

pub struct InteractionProxy {
    pub window: Arc<Window>,
    pub creature_id: CreatureId,
    monitor_id: u64,
    logical_bounds: DesktopRect,
    mask: Arc<[bool]>,
    mask_cache: HashMap<MaskArtworkSignature, Arc<[bool]>>,
    signature: Option<MaskSignature>,
    /// The mask the native window was last given. The in-process mask stays current every tick
    /// for hit-testing; pushing it to the window only matters while the cursor is close.
    applied_signature: Option<MaskSignature>,
    hit_enabled: bool,
    physical_position: Option<PhysicalPosition<i32>>,
    physical_size: Option<u32>,
    visible: bool,
    interactive: bool,
    resting_baseline: Option<(bool, u32)>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ProxyRuntimeState {
    pub dragging: bool,
    pub occluded: bool,
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
            mask: vec![false; (FRAME_SIZE * FRAME_SIZE) as usize].into(),
            mask_cache: HashMap::new(),
            signature: None,
            applied_signature: None,
            hit_enabled: false,
            physical_position: None,
            physical_size: None,
            visible: false,
            interactive: false,
            resting_baseline: None,
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
        runtime: ProxyRuntimeState,
    ) {
        self.monitor_id = monitor.id;
        let scale = settings.display_scale;
        let physical_size = FRAME_SIZE * u32::from(scale);
        let logical_size = physical_size as f32 / monitor.scale_factor;
        // The overlay seats the sprite by the creature's authored under-body clearance so its feet
        // meet the contact point. The grab mask has to move with it, or the clickable silhouette
        // drifts above the creature the user can see.
        let baseline = match self.resting_baseline {
            Some((cached_motion, value)) if cached_motion == settings.reduce_motion => value,
            _ => {
                let value = CreatureRenderer::resting_baseline(
                    &creature.appearance,
                    settings.reduce_motion,
                );
                self.resting_baseline = Some((settings.reduce_motion, value));
                value
            }
        };
        let placement = FramePlacement::for_creature(creature, baseline);
        let origin_y = placement.origin_y as f32 * f32::from(scale);
        self.logical_bounds = DesktopRect {
            x: creature.state.position.x - logical_size * 0.5,
            y: creature.state.position.y + origin_y / monitor.scale_factor,
            width: logical_size,
            height: logical_size,
        };
        let local_anchor_x = (creature.state.position.x - monitor.bounds.x) * monitor.scale_factor;
        let local_anchor_y =
            (creature.state.position.y - monitor.bounds.y) * monitor.scale_factor + origin_y;
        let position = PhysicalPosition::new(
            overlay_origin.x + (local_anchor_x - physical_size as f32 * 0.5).round() as i32,
            overlay_origin.y + local_anchor_y.round() as i32,
        );
        // Moving a native window is one of the most expensive things this app does: every move is
        // a window-server transaction, and AppKit answers each one by re-reading the display
        // configuration. A walking creature would pay that twenty times a second. But the proxy
        // only accepts a click while the cursor is actually over the creature — hit-testing is
        // decided here, in-process, from the bounds and mask above — so where the native window
        // sits is irrelevant while the cursor is elsewhere. It follows the creature only while
        // the cursor is within a creature's width of it, or a drag is under way, and it is always
        // put in place before hit-testing is switched on, never after.
        let margin = logical_size.max(64.0);
        let near = runtime.dragging
            || self.hit_enabled
            || (cursor.available
                && cursor.position.x >= self.logical_bounds.x - margin
                && cursor.position.x <= self.logical_bounds.x + self.logical_bounds.width + margin
                && cursor.position.y >= self.logical_bounds.y - margin
                && cursor.position.y
                    <= self.logical_bounds.y + self.logical_bounds.height + margin);
        // Size before position, and re-apply the position whenever the size changes. winit's macOS
        // `set_outer_position` flips the Y origin using the window's *current* frame height, and
        // `request_inner_size` resizes from the bottom-left corner. Positioning first therefore
        // lands the window off by the size delta -- the proxy is born 48x48 and grows to
        // 48 * display_scale -- and both calls are change-gated, so nothing ever corrects it. A
        // creature that moves repositions on the next tick and self-heals; one sitting still at
        // the shelter stayed displaced for its whole visit, which put its clickable window well
        // above the creature the user was aiming at.
        if self.physical_size != Some(physical_size) {
            let _ = self
                .window
                .request_inner_size(PhysicalSize::new(physical_size, physical_size));
            self.physical_size = Some(physical_size);
            self.physical_position = None;
        }
        if near && self.physical_position != Some(position) {
            self.window.set_outer_position(position);
            self.physical_position = Some(position);
        }

        // The hit proxy must show the same frame the overlay draws, gesture included.
        let clip = BodyClip::for_creature(creature);
        let frame =
            MotionSignature::for_creature(creature).frame(clip, creature.state.action_elapsed);
        let face_state =
            CreatureRenderer::resolve_face_state(creature, cursor, settings.cursor_reactions);
        let signature = MaskSignature {
            clip,
            frame,
            facing_right: creature.state.facing_right,
            reduce_motion: settings.reduce_motion,
            scale,
        };
        if self.signature != Some(signature) {
            let artwork = MaskArtworkSignature {
                clip,
                frame,
                facing_right: creature.state.facing_right,
                reduce_motion: settings.reduce_motion,
            };
            self.mask = self
                .mask_cache
                .entry(artwork)
                .or_insert_with(|| {
                    let canvas = CreatureRenderer::render_composited_frame(
                        &creature.appearance,
                        clip,
                        frame,
                        creature.state.facing_right,
                        settings.reduce_motion,
                        face_state,
                    );
                    canvas
                        .pixels()
                        .iter()
                        .map(|pixel| pixel.a > 16)
                        .collect::<Vec<_>>()
                        .into()
                })
                .clone();
            self.signature = Some(signature);
        }
        if near && self.applied_signature != self.signature {
            platform::set_interaction_shape(&self.window, &self.mask, scale);
            self.applied_signature = self.signature;
        }

        self.interactive = !runtime.occluded;
        let opaque = cursor.available && self.hit_test(cursor.position.x, cursor.position.y);
        let should_hit = !runtime.occluded && (runtime.dragging || opaque);
        if should_hit != self.hit_enabled {
            platform::set_interaction_hittest(&self.window, should_hit);
            self.hit_enabled = should_hit;
        }
        let visible = settings.visible
            && settings.direct_manipulation
            && creature.state.arrival_delay_secs <= 0.0
            && !runtime.occluded;
        if self.visible != visible {
            self.window.set_visible(visible);
            self.visible = visible;
            // Geometry applied to a window that has never been ordered in is not guaranteed to
            // survive being shown, so re-apply it on the next sync rather than trusting the cache.
            if visible {
                self.physical_position = None;
                self.applied_signature = None;
            }
        }
    }

    pub fn hit_test(&self, desktop_x: f32, desktop_y: f32) -> bool {
        self.interactive && hit_mask(&self.mask, self.logical_bounds, desktop_x, desktop_y)
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

/// The native window that takes clicks on an open creature menu.
///
/// The overlay the strip is drawn on is click-through, so without this the icons would be a
/// picture. It is the same kind of window as a creature's interaction proxy and is configured
/// through the same platform calls — borderless, transparent, always on top, never activated,
/// never given keyboard focus — so the menu can neither pull the desktop's focus away nor put a
/// window in front of anything. It covers only the strip's framed body: the notch and the label
/// tab hang outside it, so the only desktop pixels it claims are the ones with icons on them.
pub struct MenuProxy {
    pub window: Arc<Window>,
    physical_position: Option<PhysicalPosition<i32>>,
    physical_size: Option<PhysicalSize<u32>>,
    visible: bool,
}

impl MenuProxy {
    pub fn new(event_loop: &ActiveEventLoop) -> Result<Self> {
        let window = Arc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_title("Formiga creature menu")
                        .with_inner_size(PhysicalSize::new(FRAME_SIZE, FRAME_SIZE))
                        .with_resizable(false)
                        .with_decorations(false)
                        .with_transparent(true)
                        .with_window_level(WindowLevel::AlwaysOnTop)
                        .with_active(false)
                        .with_visible(false),
                )
                .context("create creature menu proxy")?,
        );
        platform::configure_interaction_proxy(&window);
        // A creature proxy switches hit testing on and off as the cursor nears the creature. A
        // menu only exists while it is being pointed at, so it is simply always live.
        platform::set_interaction_hittest(&window, true);
        window.set_cursor(Cursor::Icon(CursorIcon::Default));
        Ok(Self {
            window,
            physical_position: None,
            physical_size: None,
            visible: false,
        })
    }

    pub fn id(&self) -> WindowId {
        self.window.id()
    }

    /// Put the window over the strip. `body` is the strip's clickable body in desktop logical
    /// points, straight out of `creature_menu`.
    pub fn sync(
        &mut self,
        body: DesktopRect,
        monitor: &MonitorInfo,
        overlay_origin: PhysicalPosition<i32>,
    ) {
        let factor = monitor.scale_factor;
        let size = PhysicalSize::new(
            (body.width * factor).round().max(1.0) as u32,
            (body.height * factor).round().max(1.0) as u32,
        );
        let position = PhysicalPosition::new(
            overlay_origin.x + ((body.x - monitor.bounds.x) * factor).round() as i32,
            overlay_origin.y + ((body.y - monitor.bounds.y) * factor).round() as i32,
        );
        // Size before position, and re-apply the position whenever the size changes, for the same
        // reason the creature proxy does: winit's macOS `set_outer_position` flips the Y origin
        // using the window's current frame height, so positioning a window that is about to be
        // resized lands it off by the size delta.
        if self.physical_size != Some(size) {
            let _ = self.window.request_inner_size(size);
            platform::set_menu_proxy_shape(&self.window, size.width, size.height);
            self.physical_size = Some(size);
            self.physical_position = None;
        }
        if self.physical_position != Some(position) {
            self.window.set_outer_position(position);
            self.physical_position = Some(position);
        }
        if !self.visible {
            self.window.set_visible(true);
            self.visible = true;
            // Geometry applied to a window that has never been ordered in is not guaranteed to
            // survive being shown, so ask for all of it again on the next sync — the size and the
            // Windows region with it — rather than trusting what was set before it appeared.
            self.physical_size = None;
            self.physical_position = None;
        }
    }

    /// Take the window back out of the way when the menu closes. It is hidden rather than dropped
    /// so that closing a menu from inside the window's own click never destroys the window that
    /// event came from, and so the next right-click does not pay for a new one.
    pub fn hide(&mut self) {
        if self.visible {
            self.window.set_visible(false);
            self.visible = false;
        }
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
