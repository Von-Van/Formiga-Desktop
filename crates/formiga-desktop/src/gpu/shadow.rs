//! The shadow every companion stands on: one small quad drawn just before its body, from the
//! contact shadows' own sheet, as wide as the frame it is drawn in has feet. It is worked out from
//! the frame's own silhouette and the desktop's window rectangles alone, never from anything on
//! the screen, and it is part of the creature's own stretch of the frame's vertices, so it costs
//! no wakeup and no draw of its own beyond one quad.
use super::*;

/// Whether a companion doing this might be off the ground, so its shadow is laid on whatever is
/// below it rather than on the line it is drawn standing on: thrown, coming down from a hop (a
/// hop is drawn with the landing clip all the way), hanging from a ledge, or climbing a window's
/// side.
pub(super) fn may_be_off_the_ground(creature: &Creature) -> bool {
    matches!(
        creature.state.action,
        ActionKind::Tossed | ActionKind::Landing | ActionKind::Dangle | ActionKind::ClimbWindow
    ) || creature
        .state
        .attention
        .is_some_and(|pose| pose.hanging > 0.0)
}

/// Where a shadow goes, in the drawable's pixels, before it is worked into a quad.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct ShadowSpot {
    /// The frame's own middle, across.
    pub(super) frame_centre_x: f32,
    /// How much narrower than its own width the frame is drawn (a squeeze).
    pub(super) horizontal_scale: f32,
    /// The line the shadow is laid on: where the feet stand, or the ground under them.
    pub(super) ground_y: f32,
    /// How far above that line the feet are, in art pixels.
    pub(super) lift: f32,
    /// Drawable pixels to the art pixel.
    pub(super) unit: f32,
    pub(super) ground: ShadowGround,
}

/// The shadow quad for feet reaching across `feet` (art pixels, the frame facing right) at
/// `spot`, sampling the shadow sheet, or `None` once it has shrunk to nothing. Every edge is
/// rounded onto the drawable's grid with `snap`.
pub(super) fn shadow_quad(
    feet: (u8, u8),
    facing_right: bool,
    spot: ShadowSpot,
    layout: PhysicalSize<u32>,
    occlusion_enabled: f32,
    snap: impl Fn(f32) -> f32,
) -> Option<[Vertex; 6]> {
    let mut shadow = ContactShadow::under(feet, spot.lift)?;
    if !facing_right {
        shadow = shadow.mirrored();
    }
    let across = spot.unit * spot.horizontal_scale;
    let half_frame = FRAME_SIZE as f32 / 2.0;
    let left = snap(spot.frame_centre_x + (shadow.left() - half_frame) * across);
    let right = snap(left + shadow.width as f32 * across);
    let top = snap(spot.ground_y) - spot.ground.above_contact() as f32 * spot.unit;
    let bottom = top + spot.ground.rows() as f32 * spot.unit;
    let cell = shadow.cell(spot.ground);
    let (sheet_width, sheet_height) = (SHADOW_SHEET_WIDTH as f32, SHADOW_SHEET_HEIGHT as f32);
    let (u_left, u_right) = (
        cell.x as f32 / sheet_width,
        (cell.x + cell.width) as f32 / sheet_width,
    );
    let (v_top, v_bottom) = (
        cell.y as f32 / sheet_height,
        (cell.y + cell.height) as f32 / sheet_height,
    );
    let x = |value: f32| value / layout.width as f32 * 2.0 - 1.0;
    let y = |value: f32| 1.0 - value / layout.height as f32 * 2.0;
    let vertex = |position, uv| Vertex {
        position,
        uv,
        occlusion_enabled,
    };
    Some([
        vertex([x(left), y(top)], [u_left, v_top]),
        vertex([x(right), y(top)], [u_right, v_top]),
        vertex([x(right), y(bottom)], [u_right, v_bottom]),
        vertex([x(left), y(top)], [u_left, v_top]),
        vertex([x(right), y(bottom)], [u_right, v_bottom]),
        vertex([x(left), y(bottom)], [u_left, v_bottom]),
    ])
}

/// The village's ground as the shadow pass needs it: its strip across, and the line its front
/// edge stands on, in the drawable's pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct VillageStrip {
    pub(super) left: f32,
    pub(super) right: f32,
    pub(super) line_y: f32,
}

/// Everything about this display a shadow is laid against: the display itself and how coarsely
/// the overlay draws on it, the windows a companion in the air could come down on, and the
/// village's grass, while the village is out on it.
pub(super) struct ShadowGrounds<'a> {
    pub(super) monitor: &'a MonitorInfo,
    pub(super) render_divisor: u32,
    pub(super) windows: &'a [DesktopWindow],
    pub(super) settings: &'a formiga_core::Settings,
    pub(super) village: Option<VillageStrip>,
}

impl ShadowGrounds<'_> {
    fn snap(&self, physical: f32) -> f32 {
        snap_to(physical, self.render_divisor)
    }

    /// Where a companion's shadow goes this frame: under its feet on whatever it is standing on,
    /// on the ground below while it is off it, or nowhere while it is carried, or with nothing
    /// under it to land on. `placed` is where it stands this frame, in desktop points;
    /// `frame_top` and `contact` are where its frame and its contact point are drawn, and
    /// `silhouette_bottom` is the row just past the frame's lowest drawn one.
    pub(super) fn spot(
        &self,
        creature: &Creature,
        placed: formiga_core::Point,
        silhouette_bottom: u8,
        (local_x, contact_y): (f32, f32),
        frame_top: f32,
        display_scale: u8,
    ) -> Option<ShadowSpot> {
        if creature.state.action == ActionKind::Dragged {
            return None;
        }
        let unit = f32::from(display_scale);
        let feet_y = frame_top + f32::from(silhouette_bottom) * unit;
        let (origin, scale_factor) = (self.monitor.bounds, self.monitor.scale_factor);
        // How far up the display standing back in the village draws it: nought elsewhere.
        let set_back = contact_y - self.snap((placed.y - origin.y) * scale_factor);
        let ground_y = if may_be_off_the_ground(creature) {
            let feet = formiga_core::Point {
                x: placed.x,
                y: origin.y + (feet_y - set_back) / scale_factor,
            };
            let ground =
                formiga_core::ground_below(feet, self.monitor, self.windows, self.settings)?;
            self.snap((ground - origin.y) * scale_factor) + set_back
        } else {
            contact_y
        };
        let ground = match self.village {
            Some(strip)
                if (strip.left..=strip.right).contains(&local_x)
                    && (ground_y - set_back - strip.line_y).abs() <= 2.0 * unit =>
            {
                ShadowGround::Village
            }
            _ => ShadowGround::Desktop,
        };
        Some(ShadowSpot {
            frame_centre_x: local_x,
            horizontal_scale: creature_horizontal_scale(creature.state.action),
            ground_y,
            lift: ((ground_y - feet_y) / unit).max(0.0),
            unit,
            ground,
        })
    }
}

impl OverlayRenderer {
    /// The village's strip of grass on this display, from its ground plan.
    pub(super) fn village_strip(&self, plan: &formiga_core::VillageGroundPlan) -> VillageStrip {
        let local_x = |x: f32| self.snap((x - self.monitor.bounds.x) * self.monitor.scale_factor);
        VillageStrip {
            left: local_x(plan.origin.x),
            right: local_x(plan.origin.x + plan.length * plan.scale),
            line_y: self.snap((plan.origin.y - self.monitor.bounds.y) * self.monitor.scale_factor),
        }
    }
}
