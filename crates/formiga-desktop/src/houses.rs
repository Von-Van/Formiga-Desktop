//! The village's houses as the owner can click them, while Formiga Home is installed: where each
//! stands on the desktop, which of its pixels are the house, and a native window over each that
//! takes a click only on those pixels. A click opens the house's menu. Without Home installed none
//! of this exists, and the village is exactly as click-through as it always was.
//!
//! A house's pixels are cut from the very village the overlay draws, by day: lit or with somebody
//! at home, a house keeps the same outline. They are cut again only when the village's look
//! changes, which is when a house is built, dressed or arranged anew.

use crate::platform;
use anyhow::{Context, Result};
use formiga_art::{SHELTER_SIZE, ShelterRenderer, VILLAGE_HOUSES, VillageCell, VillageLook};
use formiga_core::{
    CreatureId, CursorSnapshot, DesktopRect, MonitorId, MonitorInfo, Point, SaveFile,
};
use std::sync::Arc;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event_loop::ActiveEventLoop;
use winit::window::{Cursor, CursorIcon, Window, WindowId, WindowLevel};

/// How opaque a pixel has to be to count as the house, as for a creature.
const SOLID: u8 = 16;

/// One house, placed where the overlay draws it.
#[derive(Clone, Debug, PartialEq)]
pub struct PlacedHouse {
    /// Who keeps it.
    pub keeper: CreatureId,
    pub slot: usize,
    pub monitor_id: MonitorId,
    /// The foot of the house's cell, the point it stands on, in desktop logical points.
    pub foot: Point,
    /// The cell's side, in desktop logical points.
    pub side: f32,
    /// Which of the cell's pixels are the house, row by row.
    pub mask: Arc<[bool]>,
    /// The first row of the cell with any of the house in it: the top of its roof.
    pub roof: u32,
}

impl PlacedHouse {
    /// The cell, in desktop logical points.
    pub fn bounds(&self) -> DesktopRect {
        DesktopRect {
            x: self.foot.x - self.side / 2.0,
            y: self.foot.y - self.side,
            width: self.side,
            height: self.side,
        }
    }

    /// Whether a point on the desktop is on the house itself, not just inside its cell.
    pub fn hit_test(&self, at: Point) -> bool {
        let bounds = self.bounds();
        if !bounds.contains(at) {
            return false;
        }
        let pixel = |value: f32, from: f32| {
            ((value - from) / self.side * SHELTER_SIZE as f32)
                .floor()
                .clamp(0.0, SHELTER_SIZE as f32 - 1.0) as usize
        };
        let (x, y) = (pixel(at.x, bounds.x), pixel(at.y, bounds.y));
        self.mask[y * SHELTER_SIZE as usize + x]
    }

    /// The middle of the house, which a menu stays close to.
    pub fn centre(&self) -> Point {
        let roof = self.side * self.roof as f32 / SHELTER_SIZE as f32;
        Point {
            x: self.foot.x,
            y: self.foot.y - (self.side - roof) / 2.0,
        }
    }
}

/// Each house's pixels, kept until the village's look changes.
#[derive(Default)]
pub struct HouseShapes {
    look: Option<VillageLook>,
    cut: Vec<(Arc<[bool]>, u32)>,
}

impl HouseShapes {
    /// Every house of the village where it stands now, or none while the houses are not out.
    pub fn placed(&mut self, save: &SaveFile, monitors: &[MonitorInfo]) -> Vec<PlacedHouse> {
        if !save.home.is_active() {
            return Vec::new();
        }
        let look = VillageLook::of(&save.home, &save.creatures);
        if self.look.as_ref() != Some(&look) {
            let village = ShelterRenderer::render_look(&look, false);
            self.cut = (0..VILLAGE_HOUSES)
                .map(|slot| cut(&village, slot))
                .collect();
            self.look = Some(look);
        }
        let cottages = formiga_core::colony_cottages(&save.creatures);
        let owners = formiga_core::house_owners(&save.creatures, &save.home.cottage_order);
        let scale = save.settings.display_scale;
        (0..=cottages.len())
            .filter_map(|slot| {
                let keeper = *owners.as_slice().get(slot)?;
                let (monitor_id, foot) = formiga_core::home_dwelling_position(
                    &save.home,
                    slot,
                    &cottages,
                    monitors,
                    &save.settings.habitat,
                    scale,
                )?;
                let monitor = monitors.iter().find(|monitor| monitor.id == monitor_id)?;
                let (mask, roof) = self.cut.get(slot)?.clone();
                Some(PlacedHouse {
                    keeper,
                    slot,
                    monitor_id,
                    foot,
                    side: SHELTER_SIZE as f32 * f32::from(scale) / monitor.scale_factor,
                    mask,
                    roof,
                })
            })
            .collect()
    }
}

/// One house's pixels from the village by day, and the row its roof starts on.
fn cut(village: &formiga_art::Canvas, slot: usize) -> (Arc<[bool]>, u32) {
    let (left, top) = ShelterRenderer::village_cell(VillageCell::House {
        slot,
        lit: false,
        occupied: false,
    });
    let side = SHELTER_SIZE as i32;
    let mask: Vec<bool> = (0..side)
        .flat_map(|y| (0..side).map(move |x| (x, y)))
        .map(|(x, y)| village.get(left as i32 + x, top as i32 + y).a > SOLID)
        .collect();
    let roof = mask
        .chunks(SHELTER_SIZE as usize)
        .position(|row| row.contains(&true))
        .unwrap_or(0) as u32;
    (mask.into(), roof)
}

/// The native window that takes clicks on one house. The same kind of window as a creature's: a
/// borderless, transparent, always-on-top window that is never activated, taking a click only on
/// the house's own pixels, so the desktop around and behind it stays click-through. Where a
/// companion stands in front of a house, the companion wins.
pub struct HouseProxy {
    window: Arc<Window>,
    pub keeper: CreatureId,
    house: Option<PlacedHouse>,
    /// The shape the native window was last given, and at what scale.
    shaped: Option<(Arc<[bool]>, u8)>,
    hit_enabled: bool,
    physical_position: Option<PhysicalPosition<i32>>,
    physical_size: Option<u32>,
    visible: bool,
}

impl HouseProxy {
    pub fn new(event_loop: &ActiveEventLoop, keeper: CreatureId) -> Result<Self> {
        let window = Arc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_title("Formiga house")
                        .with_inner_size(PhysicalSize::new(SHELTER_SIZE, SHELTER_SIZE))
                        .with_resizable(false)
                        .with_decorations(false)
                        .with_transparent(true)
                        .with_window_level(WindowLevel::AlwaysOnTop)
                        .with_active(false)
                        .with_visible(false),
                )
                .context("create house proxy")?,
        );
        platform::configure_interaction_proxy(&window);
        window.set_cursor(Cursor::Icon(CursorIcon::Pointer));
        Ok(Self {
            window,
            keeper,
            house: None,
            shaped: None,
            hit_enabled: false,
            physical_position: None,
            physical_size: None,
            visible: false,
        })
    }

    pub fn id(&self) -> WindowId {
        self.window.id()
    }

    /// Put the window over `house`, shaped to it, and take clicks only while the cursor is on the
    /// house and on nothing in front of it.
    pub fn sync(
        &mut self,
        house: &PlacedHouse,
        monitor: &MonitorInfo,
        overlay_origin: PhysicalPosition<i32>,
        cursor: CursorSnapshot,
        in_front: bool,
        display_scale: u8,
    ) {
        let physical_size = SHELTER_SIZE * u32::from(display_scale);
        let local_x = (house.foot.x - monitor.bounds.x) * monitor.scale_factor;
        let local_y = (house.foot.y - monitor.bounds.y) * monitor.scale_factor;
        let position = PhysicalPosition::new(
            overlay_origin.x + (local_x - physical_size as f32 / 2.0).round() as i32,
            overlay_origin.y + (local_y - physical_size as f32).round() as i32,
        );
        // Size before position, and the position again whenever the size changes, as for a
        // creature's proxy: winit's macOS `set_outer_position` flips the Y origin using the
        // window's current height.
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
        let shaped = self
            .shaped
            .as_ref()
            .is_some_and(|(mask, scale)| Arc::ptr_eq(mask, &house.mask) && *scale == display_scale);
        if !shaped {
            platform::set_interaction_shape(&self.window, &house.mask, SHELTER_SIZE, display_scale);
            self.shaped = Some((house.mask.clone(), display_scale));
        }
        let over = cursor.available && !in_front && house.hit_test(cursor.position);
        if over != self.hit_enabled {
            platform::set_interaction_hittest(&self.window, over);
            self.hit_enabled = over;
        }
        if !self.visible {
            self.window.set_visible(true);
            self.visible = true;
            // Geometry applied to a window that has never been ordered in is not guaranteed to
            // survive being shown, so all of it is applied again on the next sync.
            self.physical_position = None;
            self.physical_size = None;
            self.shaped = None;
        }
        self.house = Some(house.clone());
    }

    /// Whether a point on the desktop is on this house, as last placed.
    pub fn hit_test(&self, at: Point) -> bool {
        self.visible && self.house.as_ref().is_some_and(|house| house.hit_test(at))
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

    fn house(mask: Vec<bool>) -> PlacedHouse {
        PlacedHouse {
            keeper: 1,
            slot: 0,
            monitor_id: 1,
            foot: Point { x: 400.0, y: 800.0 },
            side: 192.0,
            mask: mask.into(),
            roof: 10,
        }
    }

    #[test]
    fn a_house_is_clicked_only_on_its_own_pixels() {
        let side = SHELTER_SIZE as usize;
        let mut mask = vec![false; side * side];
        // One opaque pixel in the middle of the cell's bottom row.
        mask[(side - 1) * side + side / 2] = true;
        let house = house(mask);
        let bounds = house.bounds();
        assert_eq!((bounds.x, bounds.y), (304.0, 608.0));
        assert!(house.hit_test(Point { x: 400.5, y: 799.0 }));
        assert!(!house.hit_test(Point { x: 310.0, y: 610.0 }), "clear sky");
        assert!(
            !house.hit_test(Point { x: 400.0, y: 900.0 }),
            "below its cell"
        );
    }

    #[test]
    fn every_house_in_a_village_is_cut_with_its_roof() {
        let save = formiga_home_contract::sample::colony();
        let mut save = save;
        save.home.active_since_utc = Some(save.created_at_utc);
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
        save.home.display = Some(monitor.display_key);
        let mut shapes = HouseShapes::default();
        let placed = shapes.placed(&save, std::slice::from_ref(&monitor));
        let owners = formiga_core::house_owners(&save.creatures, &save.home.cottage_order);
        assert_eq!(placed.len(), owners.as_slice().len());
        for house in &placed {
            assert_eq!(house.keeper, owners.as_slice()[house.slot]);
            assert!(house.mask.contains(&true), "house {} is there", house.slot);
            assert!(
                house.roof > 0 && house.roof < SHELTER_SIZE,
                "a roof below the cell's top"
            );
            assert!(house.hit_test(house.centre()), "its middle is the house");
        }
        // The same village again cuts nothing new.
        let again = shapes.placed(&save, std::slice::from_ref(&monitor));
        assert!(Arc::ptr_eq(&again[0].mask, &placed[0].mask));
        // With the houses not out, there is nothing to click.
        save.home.active_since_utc = None;
        assert!(
            shapes
                .placed(&save, std::slice::from_ref(&monitor))
                .is_empty()
        );
    }
}
