use crate::platform;
use anyhow::{Context, Result};
use formiga_art::{
    BodyClip, BodyPresentation, CreatureRenderer, FRAME_SIZE, FramePlacement, Holding,
};
use formiga_core::{
    AppearanceGenome, Creature, CreatureId, CursorSnapshot, DesktopRect, MonitorInfo, Settings,
};
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
    logical_size: u8,
    scale: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct MaskArtworkSignature {
    clip: BodyClip,
    frame: u8,
    facing_right: bool,
    reduce_motion: bool,
    /// The size the art draws the body at: its own, or smaller standing back in the village.
    logical_size: u8,
}

pub struct InteractionProxy {
    /// Private on purpose. Visibility is bookkeeping the proxy keeps for itself, so hiding one of
    /// these goes through `hide` and showing one through `sync`; reaching past them to the window
    /// leaves the two disagreeing and the proxy never comes back.
    window: Arc<Window>,
    pub creature_id: CreatureId,
    monitor_id: u64,
    logical_bounds: DesktopRect,
    mask: Arc<[bool]>,
    mask_cache: HashMap<MaskArtworkSignature, Arc<[bool]>>,
    signature: Option<MaskSignature>,
    /// Parts of the desktop where the overlay hides this creature behind a chosen application's
    /// window, in logical points (see `set_covered`).
    covered: Vec<DesktopRect>,
    /// The mask less whatever of it is `covered`: what takes a press, in-process and natively.
    /// The same allocation as `mask` while nothing covers the creature.
    shape: Arc<[bool]>,
    /// The mask the native window was last given. The in-process mask stays current every tick
    /// for hit-testing; pushing it to the window only matters while the cursor is close.
    applied_signature: Option<MaskSignature>,
    /// The shape the native window was last given, alongside `applied_signature`.
    applied_shape: Option<Arc<[bool]>>,
    /// The native window has been left behind where its creature used to be and given an empty
    /// shape, so the old spot takes no clicks (see `sync`).
    parked: bool,
    hit_enabled: bool,
    physical_position: Option<PhysicalPosition<i32>>,
    physical_size: Option<u32>,
    visible: bool,
    interactive: bool,
    /// Read for reduced motion or not, at the size the body is drawn: `(reduced, size, rows)`.
    resting_baseline: Option<(bool, u8, u32)>,
    /// The look the mask and the baseline were read from. A companion given a new look in
    /// Formiga Farm keeps its id, so both are read again when this changes.
    appearance: Option<AppearanceGenome>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ProxyRuntimeState {
    pub dragging: bool,
    pub occluded: bool,
    /// Drawn from its set-back atlas, smaller, standing back in the village
    /// (`gpu::drawn_set_back`): the mask and its footing are read from that size too.
    pub set_back: bool,
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
            covered: Vec::new(),
            shape: vec![false; (FRAME_SIZE * FRAME_SIZE) as usize].into(),
            applied_signature: None,
            applied_shape: None,
            parked: false,
            hit_enabled: false,
            physical_position: None,
            physical_size: None,
            visible: false,
            interactive: false,
            resting_baseline: None,
            appearance: None,
        })
    }

    pub fn id(&self) -> WindowId {
        self.window.id()
    }

    /// Where the overlay hides this creature behind a chosen application's window just now, in
    /// logical points, for the next `sync`: a press there is the application's, not the creature's,
    /// while the part of the body still drawn keeps taking them.
    pub fn set_covered(&mut self, rects: &[DesktopRect]) {
        self.covered.clear();
        self.covered.extend_from_slice(rects);
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
        if self.appearance.as_ref() != Some(&creature.appearance) {
            self.mask_cache.clear();
            self.signature = None;
            self.applied_signature = None;
            self.resting_baseline = None;
            self.appearance = Some(creature.appearance.clone());
        }
        let scale = settings.display_scale;
        let physical_size = FRAME_SIZE * u32::from(scale);
        let logical_size = physical_size as f32 / monitor.scale_factor;
        // The size the overlay draws the body at, which is what the mask is read from: standing
        // back in the village it is drawn afresh smaller, and a mask read at its own size would
        // reach past the body it can see, or miss its edge.
        let drawn_size = if runtime.set_back {
            formiga_core::set_back_size(creature.appearance.logical_size)
        } else {
            creature.appearance.logical_size
        };
        let drawn_appearance = || {
            let mut appearance = creature.appearance.clone();
            appearance.logical_size = drawn_size;
            appearance
        };
        // The overlay seats the sprite by the creature's authored under-body clearance so its feet
        // meet the contact point. The grab mask has to move with it, or the clickable silhouette
        // drifts above the creature the user can see.
        let baseline = match self.resting_baseline {
            Some((cached_motion, cached_size, value))
                if cached_motion == settings.reduce_motion && cached_size == drawn_size =>
            {
                value
            }
            _ => {
                let value =
                    CreatureRenderer::resting_baseline(&drawn_appearance(), settings.reduce_motion);
                self.resting_baseline = Some((settings.reduce_motion, drawn_size, value));
                value
            }
        };
        let placement = FramePlacement::for_creature(creature, baseline);
        let origin_y = placement.origin_y as f32 * f32::from(scale);
        // Wherever the overlay draws it: higher up for somebody standing back in the village.
        let drawn = creature
            .state
            .drawn_position(f32::from(scale) / monitor.scale_factor.max(1.0));
        // Squeezing through a gap, the overlay draws the frame narrower about its middle.
        let logical_width =
            logical_size * crate::gpu::creature_horizontal_scale(creature.state.action);
        self.logical_bounds = DesktopRect {
            x: drawn.x - logical_width * 0.5,
            y: drawn.y + origin_y / monitor.scale_factor,
            width: logical_width,
            height: logical_size,
        };
        let local_anchor_x = (drawn.x - monitor.bounds.x) * monitor.scale_factor;
        let local_anchor_y = (drawn.y - monitor.bounds.y) * monitor.scale_factor + origin_y;
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
        } else if !near && self.physical_position != Some(position) && !self.parked {
            // Left behind where its creature was. Where the window's own shape is what takes a
            // press (Windows), that shape would go on taking clicks meant for whatever is there
            // now, so it is emptied until the window is back over its creature, which puts it in
            // place first and shapes it again after.
            let empty = vec![false; (FRAME_SIZE * FRAME_SIZE) as usize];
            platform::set_interaction_shape(&self.window, &empty, FRAME_SIZE, scale);
            self.parked = true;
            self.applied_signature = None;
            self.applied_shape = None;
        }

        // The hit proxy must show the same frame the overlay draws: gestures, celebrations and
        // habits included, turned whichever way the overlay has turned them.
        let BodyPresentation {
            clip,
            frame,
            facing_right,
        } = BodyPresentation::for_creature(creature);
        let face_state =
            CreatureRenderer::resolve_face_state(creature, cursor, settings.cursor_reactions);
        let signature = MaskSignature {
            clip,
            frame,
            facing_right,
            reduce_motion: settings.reduce_motion,
            logical_size: drawn_size,
            scale,
        };
        if self.signature != Some(signature) {
            let artwork = MaskArtworkSignature {
                clip,
                frame,
                facing_right,
                reduce_motion: settings.reduce_motion,
                logical_size: drawn_size,
            };
            self.mask = self
                .mask_cache
                .entry(artwork)
                .or_insert_with(|| {
                    body_mask(
                        &drawn_appearance(),
                        clip,
                        frame,
                        facing_right,
                        settings.reduce_motion,
                        face_state,
                    )
                })
                .clone();
            self.signature = Some(signature);
        }
        // Whatever of the body a chosen application's window hides takes no press, as it is not
        // drawn. The shape keeps its old allocation while it has not changed, so it is not pushed
        // to the window again for nothing.
        let shape = clip_mask(&self.mask, self.logical_bounds, &self.covered);
        if *shape != *self.shape {
            self.shape = shape;
        }
        let shape_applied = self
            .applied_shape
            .as_ref()
            .is_some_and(|applied| Arc::ptr_eq(applied, &self.shape));
        if near && (self.applied_signature != self.signature || !shape_applied) {
            platform::set_interaction_shape(&self.window, &self.shape, FRAME_SIZE, scale);
            self.applied_signature = self.signature;
            self.applied_shape = Some(self.shape.clone());
            self.parked = false;
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
            && !creature.state.indoors
            && !runtime.occluded;
        if self.visible != visible {
            self.window.set_visible(visible);
            self.visible = visible;
            // Geometry applied to a window that has never been ordered in is not guaranteed to
            // survive being shown, so re-apply it on the next sync rather than trusting the cache.
            if visible {
                self.physical_position = None;
                self.applied_signature = None;
                self.applied_shape = None;
            }
        }
    }

    /// Order the proxy out without losing track of it.
    ///
    /// Everything that hides one of these has to come through here. `sync` only touches the
    /// window when the visibility it wants differs from the one it last set, so a proxy hidden
    /// behind its own bookkeeping is never ordered back in: the creature stays there on screen
    /// with nothing underneath it to click, unpettable and with no menu, until the application is
    /// restarted.
    pub fn hide(&mut self) {
        if !self.visible {
            return;
        }
        self.window.set_visible(false);
        self.visible = false;
    }

    pub fn hit_test(&self, desktop_x: f32, desktop_y: f32) -> bool {
        self.interactive && hit_mask(&self.shape, self.logical_bounds, desktop_x, desktop_y)
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

/// Which pixels of a frame take a press: the body and the face only, as the overlay draws them.
/// What the paws hold is drawn apart from the body, and is often not what the clip holds by
/// default (a meal's own food, a game's own thing, flung out of the paws), so it is left
/// click-through rather than claiming pixels that may not be there.
fn body_mask(
    appearance: &AppearanceGenome,
    clip: BodyClip,
    frame: u8,
    facing_right: bool,
    reduce_motion: bool,
    face_state: formiga_art::FaceRenderState,
) -> Arc<[bool]> {
    CreatureRenderer::render_composited_frame_holding(
        appearance,
        None,
        clip,
        frame,
        facing_right,
        reduce_motion,
        face_state,
        Holding::Nothing,
    )
    .pixels()
    .iter()
    .map(|pixel| pixel.a > 16)
    .collect::<Vec<_>>()
    .into()
}

/// `mask` less every pixel whose middle lies in one of `covered`, for a frame drawn over `bounds`.
/// The same allocation when nothing covered reaches the frame.
fn clip_mask(mask: &Arc<[bool]>, bounds: DesktopRect, covered: &[DesktopRect]) -> Arc<[bool]> {
    let reaching: Vec<DesktopRect> = covered
        .iter()
        .filter_map(|rect| rect.intersection(bounds))
        .collect();
    if reaching.is_empty() {
        return mask.clone();
    }
    let side = FRAME_SIZE as usize;
    mask.iter()
        .enumerate()
        .map(|(index, opaque)| {
            let middle = formiga_core::Point {
                x: bounds.x + ((index % side) as f32 + 0.5) / side as f32 * bounds.width,
                y: bounds.y + ((index / side) as f32 + 0.5) / side as f32 * bounds.height,
            };
            *opaque && !reaching.iter().any(|rect| rect.contains(middle))
        })
        .collect::<Vec<_>>()
        .into()
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

    /// A companion half behind a chosen application's window takes presses on the half still
    /// drawn and none on the half the window hides; with nothing over it, its mask is untouched.
    #[test]
    fn a_covered_part_of_the_body_passes_presses_through() {
        let bounds = DesktopRect {
            x: 100.0,
            y: 100.0,
            width: 48.0,
            height: 48.0,
        };
        let mask: Arc<[bool]> = vec![true; (FRAME_SIZE * FRAME_SIZE) as usize].into();
        assert!(Arc::ptr_eq(&clip_mask(&mask, bounds, &[]), &mask));
        let elsewhere = DesktopRect {
            x: 400.0,
            y: 100.0,
            width: 200.0,
            height: 200.0,
        };
        assert!(Arc::ptr_eq(&clip_mask(&mask, bounds, &[elsewhere]), &mask));
        let window = DesktopRect {
            x: 124.0,
            y: 0.0,
            width: 300.0,
            height: 300.0,
        };
        let shape = clip_mask(&mask, bounds, &[window]);
        assert!(hit_mask(&shape, bounds, 110.5, 120.5), "the drawn half");
        assert!(!hit_mask(&shape, bounds, 130.5, 120.5), "the hidden half");
    }

    fn face() -> formiga_art::FaceRenderState {
        formiga_art::FaceRenderState {
            expression: formiga_art::ExpressionKind::Content,
            eyelids: formiga_art::EyelidPose::Open,
            gaze: formiga_art::GazeDirection::new(0, 0),
        }
    }

    fn companion() -> AppearanceGenome {
        formiga_core::World::preview_adult(
            [41; 32],
            time::OffsetDateTime::UNIX_EPOCH,
            &formiga_core::DesktopSnapshot::default(),
        )
        .appearance
    }

    /// Whatever a companion is eating, its grab mask claims no pixel the overlay leaves clear:
    /// the clip's own default snack is not in it, and the body under any food still is.
    #[test]
    fn the_grab_mask_is_the_body_whatever_is_in_the_paws() {
        let appearance = companion();
        let clip = BodyClip::Action(formiga_core::ActionKind::Eat);
        for frame in 0..4 {
            let mask = body_mask(&appearance, clip, frame, true, false, face());
            assert!(mask.iter().any(|opaque| *opaque));
            for food in formiga_core::meals::Food::ALL {
                let drawn = CreatureRenderer::render_composited_frame_holding(
                    &appearance,
                    None,
                    clip,
                    frame,
                    true,
                    false,
                    face(),
                    Holding::Thing(formiga_art::HeldThing::Food { food, bite: 1 }),
                );
                let stray = mask
                    .iter()
                    .zip(drawn.pixels())
                    .filter(|(opaque, pixel)| **opaque && pixel.a <= 16)
                    .count();
                assert_eq!(stray, 0, "frame {frame} with {food:?}");
            }
        }
    }

    /// Standing back in the village, the mask is read from the smaller body the overlay draws
    /// there, not the companion's own size.
    #[test]
    fn a_set_back_companion_is_grabbed_by_its_smaller_body() {
        let appearance = companion();
        let mut set_back = appearance.clone();
        set_back.logical_size = formiga_core::set_back_size(appearance.logical_size);
        let clip = BodyClip::Action(formiga_core::ActionKind::Idle);
        let own = body_mask(&appearance, clip, 0, true, false, face());
        let small = body_mask(&set_back, clip, 0, true, false, face());
        let count = |mask: &[bool]| mask.iter().filter(|opaque| **opaque).count();
        assert!(
            count(&small) < count(&own),
            "{} vs {}",
            count(&small),
            count(&own)
        );
    }
}
