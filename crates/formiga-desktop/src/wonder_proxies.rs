//! The wonders the person at the desk has put out, as they can be clicked: where each is drawn,
//! which of its pixels are the wonder, and a native window over each that takes a click only on
//! those. A click opens the wonder's menu. While one is being moved, its window follows the
//! pointer instead and takes the click anywhere in its square, which is where it is set down.
//!
//! A wonder's pixels are cut from its standing frame — the one it shows while nobody is on it —
//! and grown a little all round, so a rope or a signpost a pixel or two wide can still be hit.
//! They are cut once per kind and colony, when it is first put out.

use crate::platform;
use anyhow::{Context, Result};
use formiga_art::{
    WONDER_CELL_HEIGHT, WONDER_CELL_WIDTH, WONDER_GROUND, WONDER_MIDDLE, WonderRenderer,
    wonder_frame, wonder_frames,
};
use formiga_core::{CursorSnapshot, DesktopRect, MonitorId, MonitorInfo, Point, WonderKind};
use std::sync::Arc;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event_loop::ActiveEventLoop;
use winit::window::{Cursor, CursorIcon, Window, WindowId, WindowLevel};

/// A wonder's square: as wide as its cell, and as tall, so a native window can be shaped to it
/// the way a creature's and a house's are.
pub const WONDER_SQUARE: u32 = WONDER_CELL_WIDTH;
/// How opaque a pixel has to be to count as the wonder.
const SOLID: u8 = 16;
/// How far, in art pixels, a wonder's pixels are grown all round for the pointer.
const GROWN: i32 = 2;

/// One wonder put out, placed where the overlay draws it.
#[derive(Clone, Debug, PartialEq)]
pub struct PlacedWonderShape {
    pub kind: WonderKind,
    pub monitor_id: MonitorId,
    /// The middle of where it stands on the ground, in desktop logical points.
    pub at: Point,
    /// Desktop logical points per art pixel here.
    pub point: f32,
    /// Which of its square's pixels are the wonder, row by row.
    pub mask: Arc<[bool]>,
}

impl PlacedWonderShape {
    /// Its square, in desktop logical points: the cell's middle column over where it stands, and
    /// the cell's ground row on the ground, with the room above the cell making it square.
    pub fn bounds(&self) -> DesktopRect {
        let side = WONDER_SQUARE as f32 * self.point;
        let below = (WONDER_CELL_HEIGHT as i32 - WONDER_GROUND) as f32 * self.point;
        DesktopRect {
            x: self.at.x - WONDER_MIDDLE as f32 * self.point,
            y: self.at.y + below - side,
            width: side,
            height: side,
        }
    }

    /// Whether a point on the desktop is on the wonder itself.
    pub fn hit_test(&self, at: Point) -> bool {
        let bounds = self.bounds();
        if !bounds.contains(at) {
            return false;
        }
        let pixel = |value: f32, from: f32| {
            ((value - from) / self.point)
                .floor()
                .clamp(0.0, WONDER_SQUARE as f32 - 1.0) as usize
        };
        let (x, y) = (pixel(at.x, bounds.x), pixel(at.y, bounds.y));
        self.mask[y * WONDER_SQUARE as usize + x]
    }

    /// The top of the wonder's own pixels, in desktop points: where its menu hangs from.
    pub fn top(&self) -> f32 {
        let bounds = self.bounds();
        let row = self
            .mask
            .chunks(WONDER_SQUARE as usize)
            .position(|row| row.contains(&true))
            .unwrap_or(0);
        bounds.y + row as f32 * self.point
    }

    /// The middle of the wonder, which a menu stays close to.
    pub fn centre(&self) -> Point {
        Point {
            x: self.at.x,
            y: (self.top() + self.at.y) / 2.0,
        }
    }
}

/// Each kind's pixels, kept until the colony's colours change.
#[derive(Default)]
pub struct WonderShapes {
    cut: Vec<(WonderKind, [u8; 32], Arc<[bool]>)>,
}

impl WonderShapes {
    /// Every wonder put out where it stands now, from what the colony says is out.
    pub fn placed(
        &mut self,
        views: &[formiga_core::WonderView],
        monitors: &[MonitorInfo],
        display_scale: u8,
        colony_seed: [u8; 32],
    ) -> Vec<PlacedWonderShape> {
        self.cut.retain(|(kind, seed, _)| {
            *seed == colony_seed && views.iter().any(|v| v.kind == *kind)
        });
        views
            .iter()
            .filter_map(|view| {
                let monitor = monitors
                    .iter()
                    .find(|monitor| monitor.id == view.monitor_id)?;
                let mask = match self.cut.iter().find(|(kind, ..)| *kind == view.kind) {
                    Some((.., mask)) => mask.clone(),
                    None => {
                        let mask = cut(view.kind, colony_seed);
                        self.cut.push((view.kind, colony_seed, mask.clone()));
                        mask
                    }
                };
                Some(PlacedWonderShape {
                    kind: view.kind,
                    monitor_id: view.monitor_id,
                    at: view.at,
                    point: f32::from(display_scale) / monitor.scale_factor,
                    mask,
                })
            })
            .collect()
    }
}

/// One kind's standing frame, cut into its square and grown a little all round.
fn cut(kind: WonderKind, colony_seed: [u8; 32]) -> Arc<[bool]> {
    let strip = WonderRenderer::render(kind, colony_seed);
    let frame = wonder_frame(kind, 0.0, 0.0).min(wonder_frames(kind) - 1);
    let side = WONDER_SQUARE as i32;
    let above = side - WONDER_CELL_HEIGHT as i32;
    let solid = |x: i32, y: i32| {
        let cell_y = y - above;
        (0..WONDER_CELL_WIDTH as i32).contains(&x)
            && (0..WONDER_CELL_HEIGHT as i32).contains(&cell_y)
            && strip
                .get(frame as i32 * WONDER_CELL_WIDTH as i32 + x, cell_y)
                .a
                > SOLID
    };
    (0..side)
        .flat_map(|y| (0..side).map(move |x| (x, y)))
        .map(|(x, y)| (-GROWN..=GROWN).any(|dy| (-GROWN..=GROWN).any(|dx| solid(x + dx, y + dy))))
        .collect::<Vec<_>>()
        .into()
}

/// The native window that takes clicks on one wonder put out: the same kind of window as a
/// house's, taking a click only on the wonder's own pixels — or, while it is being moved, a
/// square that follows the pointer and takes the click that sets it down.
pub struct WonderProxy {
    window: Arc<Window>,
    pub kind: WonderKind,
    wonder: Option<PlacedWonderShape>,
    shaped: Option<(Arc<[bool]>, u8)>,
    hit_enabled: bool,
    physical_position: Option<PhysicalPosition<i32>>,
    physical_size: Option<u32>,
    visible: bool,
    following: bool,
}

impl WonderProxy {
    pub fn new(event_loop: &ActiveEventLoop, kind: WonderKind) -> Result<Self> {
        let window = Arc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_title("Formiga wonder")
                        .with_inner_size(PhysicalSize::new(WONDER_SQUARE, WONDER_SQUARE))
                        .with_resizable(false)
                        .with_decorations(false)
                        .with_transparent(true)
                        .with_window_level(WindowLevel::AlwaysOnTop)
                        .with_active(false)
                        .with_visible(false),
                )
                .context("create wonder proxy")?,
        );
        platform::configure_interaction_proxy(&window);
        window.set_cursor(Cursor::Icon(CursorIcon::Pointer));
        Ok(Self {
            window,
            kind,
            wonder: None,
            shaped: None,
            hit_enabled: false,
            physical_position: None,
            physical_size: None,
            visible: false,
            following: false,
        })
    }

    pub fn id(&self) -> WindowId {
        self.window.id()
    }

    /// Put the window over `wonder`, shaped to it, taking clicks only while the cursor is on it
    /// and on nothing in front of it. While `following`, the window is centred on the cursor
    /// instead, whole, so the next click anywhere near the pointer reaches it.
    #[allow(clippy::too_many_arguments)]
    pub fn sync(
        &mut self,
        wonder: &PlacedWonderShape,
        monitor: &MonitorInfo,
        overlay_origin: PhysicalPosition<i32>,
        cursor: CursorSnapshot,
        in_front: bool,
        display_scale: u8,
        following: bool,
    ) {
        let physical_size = WONDER_SQUARE * u32::from(display_scale);
        let bounds = wonder.bounds();
        let (left, top) = if following && cursor.available {
            let half = bounds.width / 2.0;
            (cursor.position.x - half, cursor.position.y - half)
        } else {
            (bounds.x, bounds.y)
        };
        let position = PhysicalPosition::new(
            overlay_origin.x + ((left - monitor.bounds.x) * monitor.scale_factor).round() as i32,
            overlay_origin.y + ((top - monitor.bounds.y) * monitor.scale_factor).round() as i32,
        );
        if self.physical_size != Some(physical_size) {
            let _ = self
                .window
                .request_inner_size(PhysicalSize::new(physical_size, physical_size));
            self.physical_size = Some(physical_size);
            self.physical_position = None;
        }
        if self.physical_position != Some(position) {
            self.window.set_outer_position(position);
            self.physical_position = Some(position);
        }
        if following != self.following {
            self.following = following;
            self.shaped = None;
            self.window.set_cursor(Cursor::Icon(if following {
                CursorIcon::Grabbing
            } else {
                CursorIcon::Pointer
            }));
        }
        let mask = if following {
            Arc::from(vec![true; (WONDER_SQUARE * WONDER_SQUARE) as usize])
        } else {
            wonder.mask.clone()
        };
        let shaped = self.shaped.as_ref().is_some_and(|(shaped, scale)| {
            *scale == display_scale
                && (if following {
                    shaped.iter().all(|pixel| *pixel)
                } else {
                    Arc::ptr_eq(shaped, &wonder.mask)
                })
        });
        if !shaped {
            platform::set_interaction_shape(&self.window, &mask, WONDER_SQUARE, display_scale);
            self.shaped = Some((mask, display_scale));
        }
        let over = following || (cursor.available && !in_front && wonder.hit_test(cursor.position));
        if over != self.hit_enabled {
            platform::set_interaction_hittest(&self.window, over);
            self.hit_enabled = over;
        }
        if !self.visible {
            self.window.set_visible(true);
            self.visible = true;
            self.physical_position = None;
            self.physical_size = None;
            self.shaped = None;
        }
        self.wonder = Some(wonder.clone());
    }

    /// Whether a point on the desktop is on this wonder, as last placed, or anywhere near the
    /// pointer while it is being moved.
    pub fn hit_test(&self, at: Point) -> bool {
        self.visible && (self.following || self.wonder.as_ref().is_some_and(|w| w.hit_test(at)))
    }

    /// Order the window out without losing track of it.
    pub fn hide(&mut self) {
        if self.visible {
            self.window.set_visible(false);
            self.visible = false;
        }
        if self.hit_enabled {
            platform::set_interaction_hittest(&self.window, false);
            self.hit_enabled = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_wonder_is_clicked_on_its_own_pixels_and_a_little_round_them() {
        let seed = [3; 32];
        for kind in WonderKind::ALL {
            let mask = cut(kind, seed);
            let wonder = PlacedWonderShape {
                kind,
                monitor_id: 1,
                at: Point { x: 500.0, y: 800.0 },
                point: 1.5,
                mask,
            };
            let bounds = wonder.bounds();
            assert!(
                (bounds.x + WONDER_MIDDLE as f32 * 1.5 - 500.0).abs() < 0.01,
                "{kind:?} is centred on where it stands"
            );
            assert!(wonder.top() < 800.0, "{kind:?} stands up off the ground");
            // Something of it is just above where it stands, and nothing far off to the side.
            let near_ground = (0..=10).any(|up| {
                wonder.hit_test(Point {
                    x: 500.0,
                    y: 800.0 - up as f32 * 1.5,
                })
            });
            assert!(near_ground || kind == WonderKind::Tightrope, "{kind:?}");
            assert!(
                !wonder.hit_test(Point {
                    x: 500.0,
                    y: bounds.y + 1.0
                }),
                "{kind:?} sky"
            );
            assert!(!wonder.hit_test(Point {
                x: 2000.0,
                y: 790.0
            }));
        }
    }

    #[test]
    fn each_kind_is_cut_once() {
        let monitor = MonitorInfo {
            id: 1,
            display_key: formiga_core::DisplayKey([1; 16]),
            bounds: DesktopRect {
                x: 0.0,
                y: 0.0,
                width: 1440.0,
                height: 900.0,
            },
            usable_bounds: DesktopRect {
                x: 0.0,
                y: 24.0,
                width: 1440.0,
                height: 826.0,
            },
            scale_factor: 2.0,
            primary: true,
        };
        let view = formiga_core::WonderView {
            kind: WonderKind::Seesaw,
            at: Point { x: 300.0, y: 846.0 },
            monitor_id: 1,
            window_key: None,
            mirrored: false,
            presence: 1.0,
            elapsed: 0.0,
            motion: 0.0,
            glint: false,
        };
        let mut shapes = WonderShapes::default();
        let first = shapes.placed(&[view], std::slice::from_ref(&monitor), 3, [1; 32]);
        let again = shapes.placed(&[view], std::slice::from_ref(&monitor), 3, [1; 32]);
        assert_eq!(first.len(), 1);
        assert!(Arc::ptr_eq(&first[0].mask, &again[0].mask));
        assert!((first[0].point - 1.5).abs() < f32::EPSILON);
        assert!(
            shapes
                .placed(&[], std::slice::from_ref(&monitor), 3, [1; 32])
                .is_empty()
        );
    }
}
