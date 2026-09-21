//! The right-click creature menu: what it offers, where its strip goes, what the cursor is over,
//! and when it closes.
//!
//! Everything here is arithmetic and two timers. No window, no GPU, no clock, no allocation: the
//! overlay hands in where the creature is this frame and gets back where to draw, `app.rs` hands in
//! the cursor and a few facts about the world and gets back whether to redraw and whether to close.
//! That keeps the fiddly parts — mixed-DPI placement, the flip, the clamp, the dismissal rules —
//! testable without a desktop.
//!
//! # Units
//!
//! Placement is done in *monitor-local physical pixels*, the same space `gpu.rs` lays its sprites
//! out in, where one art pixel is exactly `display_scale` physical pixels and positions have been
//! multiplied by the monitor's scale factor. Hit testing is done in *desktop logical points*,
//! because that is the space the global cursor sample arrives in.

use formiga_art::{
    LABEL_TAB_GAP, LABEL_TAB_HEIGHT, MENU_BODY_HEIGHT, MENU_NOTCH_HEIGHT, MENU_STRIP_HEIGHT,
    MenuIcon, MenuLayout,
};
use formiga_core::{CreatureId, DesktopRect, MonitorId, Point};

/// Art pixels between the bottom of a hovered item's label tab and the crown of the head. The tab
/// hangs under the strip, so this — not the notch — is what decides how high the strip floats.
pub const MENU_HEAD_CLEARANCE: i32 = 4;

/// How far above the crown of the head the strip's top-left sits, in art pixels. Fixed whether or
/// not anything is hovered, so the strip never jumps as the cursor crosses it.
pub const MENU_RISE: i32 =
    MENU_STRIP_HEIGHT as i32 + LABEL_TAB_GAP as i32 + LABEL_TAB_HEIGHT as i32 + MENU_HEAD_CLEARANCE;

/// Art pixels between the notch's tip and the creature. Above the head this falls out of
/// [`MENU_RISE`]; below it is applied directly, because nothing hangs between the two.
pub const MENU_NOTCH_GAP: i32 = MENU_RISE - (MENU_STRIP_HEIGHT as i32 - 1);

/// How far the cursor may wander from both the strip and the creature before the menu starts
/// counting down, in logical points. Generous on purpose: reaching for an icon should never be a
/// race, and a creature that walks out from under its own menu must not close it.
pub const STRAY_RADIUS: f32 = 120.0;

/// How long the cursor stays beyond [`STRAY_RADIUS`] before the menu closes.
pub const STRAY_SECS: f32 = 0.8;

/// How long an untouched menu stays open. The timer restarts whenever an item is hovered.
pub const UNTOUCHED_SECS: f32 = 8.0;

/// Art pixels between the menu and a strip opened beside it.
pub const SIDE_STRIP_GAP: i32 = 2;

/// Whose menu this is. A colony member can be sent home; a guest can be asked to stay instead.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuTarget {
    Member,
    Guest,
}

/// Why a menu closed. Only ever used for logging and tests: the menu simply goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuDismissal {
    /// The cursor left the neighbourhood of both the strip and the creature.
    Strayed,
    /// Nothing was hovered for long enough.
    Untouched,
    /// The creature is being dragged or tossed.
    Handled,
    /// The colony is hidden or paused.
    Hidden,
    /// A full-screen application covers the creature.
    Occluded,
    /// The creature moved to another display.
    MonitorChanged,
    /// The creature is gone: a member removed, or a guest that has left.
    Gone,
    /// The settings window took focus.
    SettingsFocused,
    /// The owner answered it: chose an item, or right-clicked the creature again.
    Answered,
}

/// The four items a menu offers. Every menu has exactly four, so the strip is always the same size
/// for the same kind of creature and the icons never shuffle under a cursor already on its way.
pub fn menu_items(target: MenuTarget, visitor_can_stay: bool) -> [MenuIcon; 4] {
    match target {
        MenuTarget::Member => [
            MenuIcon::Snack,
            MenuIcon::Toy,
            MenuIcon::Home,
            MenuIcon::Profile,
        ],
        MenuTarget::Guest => [
            MenuIcon::Snack,
            MenuIcon::Toy,
            if visitor_can_stay {
                MenuIcon::Stay
            } else {
                MenuIcon::CopyCode
            },
            MenuIcon::Profile,
        ],
    }
}

/// A rectangle in monitor-local physical pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LocalRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl LocalRect {
    pub fn right(self) -> f32 {
        self.x + self.width
    }

    pub fn bottom(self) -> f32 {
        self.y + self.height
    }
}

/// Where the creature is this frame, and what the monitor allows. Everything the placement maths
/// needs, and nothing it does not.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MenuAnchor {
    /// The creature's centre column, in monitor-local physical pixels.
    pub centre_x: f32,
    /// The crown of the creature's head — its topmost drawn pixel, not the frame's top edge — in
    /// monitor-local physical pixels.
    pub head_top: f32,
    /// The creature's lowest drawn pixel, in monitor-local physical pixels.
    pub foot_bottom: f32,
    /// Physical pixels per art pixel: the display scale.
    pub art_scale: f32,
    /// The drawable's own pixel grid, so the strip lands where the sprites do.
    pub grid: f32,
    /// The monitor's usable area, in monitor-local physical pixels.
    pub usable: LocalRect,
    /// The monitor's top-left corner in desktop logical points.
    pub monitor_origin: Point,
    /// Logical points to physical pixels on this monitor.
    pub scale_factor: f32,
}

/// Where a menu's strip is drawn, and how to read the cursor against it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MenuPlacement {
    /// The strip sprite's top-left, in monitor-local physical pixels, on the drawable's grid.
    pub x: f32,
    pub y: f32,
    /// Physical pixels per art pixel.
    pub art_scale: f32,
    /// True when there was no room above and the strip hangs under the creature instead, its
    /// sprite flipped so the notch points up. The cells then start one notch further down.
    pub below: bool,
    /// The strip's width in art pixels.
    pub width_art: u32,
    monitor_origin: Point,
    scale_factor: f32,
}

impl MenuPlacement {
    /// The art-pixel row inside the strip sprite that the cells are measured from. Flipping the
    /// sprite moves the notch from the bottom to the top, and the body down with it.
    pub fn cell_offset_art(&self) -> i32 {
        if self.below {
            MENU_NOTCH_HEIGHT as i32
        } else {
            0
        }
    }

    /// The whole strip sprite, notch included, in monitor-local physical pixels.
    pub fn frame_rect(&self) -> LocalRect {
        LocalRect {
            x: self.x,
            y: self.y,
            width: self.width_art as f32 * self.art_scale,
            height: MENU_STRIP_HEIGHT as f32 * self.art_scale,
        }
    }

    /// The part of the strip that takes clicks: the framed body, without the notch and without the
    /// label tab. This is exactly the native proxy window, so nothing the owner cannot click on
    /// stands between the cursor and the desktop.
    pub fn body_rect(&self) -> LocalRect {
        let frame = self.frame_rect();
        LocalRect {
            x: frame.x,
            y: frame.y + self.cell_offset_art() as f32 * self.art_scale,
            width: frame.width,
            height: MENU_BODY_HEIGHT as f32 * self.art_scale,
        }
    }

    /// [`MenuPlacement::body_rect`] in desktop logical points: where the proxy window goes and the
    /// frame the global cursor is read against.
    pub fn body_desktop(&self) -> DesktopRect {
        let body = self.body_rect();
        DesktopRect {
            x: self.monitor_origin.x + body.x / self.scale_factor,
            y: self.monitor_origin.y + body.y / self.scale_factor,
            width: body.width / self.scale_factor,
            height: body.height / self.scale_factor,
        }
    }

    /// Which item the cursor is over, from a global cursor sample in logical points.
    pub fn hover(&self, layout: &MenuLayout, cursor: Point) -> Option<usize> {
        let body = self.body_desktop();
        let art = self.art_scale / self.scale_factor;
        if art <= 0.0 {
            return None;
        }
        // `MenuLayout` measures its cells from the top of the strip's *body*, which is where the
        // body rect begins whichever way up the sprite is drawn.
        layout.hit_test((cursor.x - body.x) / art, (cursor.y - body.y) / art)
    }
}

/// Place a strip against a creature. Above the head when it fits, under the creature when it does
/// not, always clamped inside the monitor's usable area and always on the drawable's pixel grid.
pub fn place(layout: &MenuLayout, anchor: MenuAnchor) -> MenuPlacement {
    let (width_art, _) = layout.size();
    let width = width_art as f32 * anchor.art_scale;
    let height = MENU_STRIP_HEIGHT as f32 * anchor.art_scale;

    let above_y = anchor.head_top - MENU_RISE as f32 * anchor.art_scale;
    // Below, the notch is at the top of the sprite, so only the gap separates it from the creature.
    let below_y = anchor.foot_bottom + MENU_NOTCH_GAP as f32 * anchor.art_scale;
    let below = above_y < anchor.usable.y;
    let y = if below { below_y } else { above_y };

    // The notch is baked into the middle of the frame sprite, so the strip can only point at the
    // creature by being centred on it. Clamping therefore moves the notch too — as far as it can.
    let left = anchor.centre_x - layout.notch_x() as f32 * anchor.art_scale;
    let limit = anchor.usable.right() - width;
    let x = if limit >= anchor.usable.x {
        left.clamp(anchor.usable.x, limit)
    } else {
        // A strip wider than the usable area is centred on it instead of hanging off one side.
        anchor.usable.x + (anchor.usable.width - width) / 2.0
    };
    // The label tab hangs below the strip; keep the whole thing on screen when it can be.
    let tail = (LABEL_TAB_GAP + LABEL_TAB_HEIGHT) as f32 * anchor.art_scale;
    let bottom_limit = anchor.usable.bottom() - height - tail;
    let y = if bottom_limit >= anchor.usable.y {
        y.min(bottom_limit).max(anchor.usable.y)
    } else {
        y
    };

    let grid = if anchor.grid > 0.0 { anchor.grid } else { 1.0 };
    let snap = |value: f32| (value / grid).round() * grid;
    MenuPlacement {
        x: snap(x),
        y: snap(y),
        art_scale: anchor.art_scale,
        below,
        width_art,
        monitor_origin: anchor.monitor_origin,
        scale_factor: anchor.scale_factor,
    }
}

/// Where a strip opened beside a menu is drawn. It has no notch: it points at nothing, and sits on
/// the menu's own row, level with its body, to the right where there is room and to the left
/// where there is not. The menu never moves to make room for it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SidePlacement {
    /// The strip's top-left, which is also its body's, in monitor-local physical pixels.
    pub x: f32,
    pub y: f32,
    pub art_scale: f32,
    /// The strip's width in art pixels.
    pub width_art: u32,
    /// Where a hovered item's label tab hangs: level with the menu's own tabs.
    pub label_y: f32,
    monitor_origin: Point,
    scale_factor: f32,
}

impl SidePlacement {
    /// The whole strip, which is all body, in monitor-local physical pixels.
    pub fn body_rect(&self) -> LocalRect {
        LocalRect {
            x: self.x,
            y: self.y,
            width: self.width_art as f32 * self.art_scale,
            height: MENU_BODY_HEIGHT as f32 * self.art_scale,
        }
    }

    /// [`SidePlacement::body_rect`] in desktop logical points.
    pub fn body_desktop(&self) -> DesktopRect {
        let body = self.body_rect();
        DesktopRect {
            x: self.monitor_origin.x + body.x / self.scale_factor,
            y: self.monitor_origin.y + body.y / self.scale_factor,
            width: body.width / self.scale_factor,
            height: body.height / self.scale_factor,
        }
    }

    /// Which item the cursor is over, from a global cursor sample in logical points.
    pub fn hover(&self, layout: &MenuLayout, cursor: Point) -> Option<usize> {
        let body = self.body_desktop();
        let art = self.art_scale / self.scale_factor;
        if art <= 0.0 {
            return None;
        }
        layout.hit_test((cursor.x - body.x) / art, (cursor.y - body.y) / art)
    }
}

/// Put a strip beside a placed menu, on the menu's row and inside the usable area.
pub fn place_beside(layout: &MenuLayout, menu: MenuPlacement, usable: LocalRect) -> SidePlacement {
    let art = menu.art_scale;
    let width = layout.size().0 as f32 * art;
    let gap = SIDE_STRIP_GAP as f32 * art;
    let frame = menu.frame_rect();
    let right = frame.right() + gap;
    let x = if right + width <= usable.right() {
        right
    } else {
        (frame.x - gap - width).max(usable.x)
    };
    SidePlacement {
        x,
        y: menu.body_rect().y,
        art_scale: art,
        width_art: layout.size().0,
        label_y: menu.y + (MENU_STRIP_HEIGHT + LABEL_TAB_GAP) as f32 * art,
        monitor_origin: menu.monitor_origin,
        scale_factor: menu.scale_factor,
    }
}

/// A strip opened beside the menu: the moments the village could share, or the way to stop one.
#[derive(Clone, Debug)]
pub struct SideStrip {
    layout: MenuLayout,
    placement: Option<SidePlacement>,
    hovered: Option<usize>,
}

impl SideStrip {
    pub fn layout(&self) -> &MenuLayout {
        &self.layout
    }

    pub fn placement(&self) -> Option<SidePlacement> {
        self.placement
    }

    pub fn hovered(&self) -> Option<usize> {
        self.hovered
    }
}

/// The facts that close a menu regardless of what the cursor is doing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MenuWorld {
    /// Whether the creature this menu belongs to is still there to be drawn.
    pub present: bool,
    /// The display the creature is on now.
    pub monitor_id: MonitorId,
    /// The creature is being dragged or tossed.
    pub handled: bool,
    /// The colony is hidden or paused.
    pub hidden: bool,
    /// A full-screen application covers the creature.
    pub occluded: bool,
    /// The settings window is up and has focus right now.
    pub settings_focused: bool,
}

/// What one frame of tracking changed.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MenuTick {
    /// The hovered item changed, so the strip has to be drawn again now rather than at the next
    /// scheduled frame.
    pub redraw: bool,
    /// The menu should close.
    pub dismissal: Option<MenuDismissal>,
}

/// One open menu. There is never more than one.
#[derive(Clone, Debug)]
pub struct CreatureMenu {
    creature_id: CreatureId,
    target: MenuTarget,
    items: [MenuIcon; 4],
    layout: MenuLayout,
    monitor_id: MonitorId,
    hovered: Option<usize>,
    placement: Option<MenuPlacement>,
    strayed_for: f32,
    untouched_for: f32,
    /// Whether the settings window had focus when this was last looked at. Only *taking* focus
    /// closes a menu: a settings window that was already up and focused when the owner
    /// right-clicked a creature is not a reason to refuse them the menu.
    settings_focused: bool,
    /// A strip opened beside this one, if any.
    side: Option<SideStrip>,
    /// The usable area the menu was last placed in, which a strip beside it keeps inside too.
    usable: Option<LocalRect>,
}

impl CreatureMenu {
    pub fn new(
        creature_id: CreatureId,
        target: MenuTarget,
        visitor_can_stay: bool,
        monitor_id: MonitorId,
        settings_focused: bool,
    ) -> Self {
        let items = menu_items(target, visitor_can_stay);
        Self {
            creature_id,
            target,
            items,
            layout: MenuLayout::new(&items),
            monitor_id,
            hovered: None,
            placement: None,
            strayed_for: 0.0,
            untouched_for: 0.0,
            settings_focused,
            side: None,
            usable: None,
        }
    }

    /// While the houses are out nobody needs sending home, so the same cell offers the moments
    /// the village could share instead, and nothing else in the strip moves.
    pub fn offering_moments(mut self) -> Self {
        for item in &mut self.items {
            if *item == MenuIcon::Home {
                *item = MenuIcon::Moment;
            }
        }
        self.layout = MenuLayout::new(&self.items);
        self
    }

    /// Open a strip beside the menu holding `items`, or close the one that is open. A strip needs
    /// between two and four items; anything else opens nothing. Returns whether one is open now.
    pub fn toggle_side(&mut self, items: &[MenuIcon]) -> bool {
        if self.side.take().is_some() {
            return false;
        }
        let layout = MenuLayout::new(items);
        if layout.is_empty() {
            return false;
        }
        let placement = self
            .placement
            .zip(self.usable)
            .map(|(menu, usable)| place_beside(&layout, menu, usable));
        self.side = Some(SideStrip {
            layout,
            placement,
            hovered: None,
        });
        true
    }

    pub fn side(&self) -> Option<&SideStrip> {
        self.side.as_ref()
    }

    /// Everything that takes clicks — the menu's body, and a strip beside it — as one rectangle
    /// in desktop logical points: the frame the click proxy is laid over.
    pub fn click_area(&self) -> Option<DesktopRect> {
        let menu = self.placement?.body_desktop();
        let Some(side) = self.side.as_ref().and_then(|side| side.placement) else {
            return Some(menu);
        };
        let side = side.body_desktop();
        let (x, y) = (menu.x.min(side.x), menu.y.min(side.y));
        Some(DesktopRect {
            x,
            y,
            width: menu.right().max(side.right()) - x,
            height: menu.bottom().max(side.bottom()) - y,
        })
    }

    pub fn creature_id(&self) -> CreatureId {
        self.creature_id
    }

    pub fn target(&self) -> MenuTarget {
        self.target
    }

    pub fn items(&self) -> &[MenuIcon; 4] {
        &self.items
    }

    pub fn layout(&self) -> &MenuLayout {
        &self.layout
    }

    pub fn monitor_id(&self) -> MonitorId {
        self.monitor_id
    }

    pub fn hovered(&self) -> Option<usize> {
        self.hovered
    }

    pub fn placement(&self) -> Option<MenuPlacement> {
        self.placement
    }

    /// Re-attach the strip to the creature. Called every frame the creature can be drawn; called
    /// with `None` when it cannot, which parks the menu until it can be placed again.
    pub fn attach(&mut self, anchor: Option<MenuAnchor>) {
        self.placement = anchor.map(|anchor| place(&self.layout, anchor));
        self.usable = anchor.map(|anchor| anchor.usable);
        if let Some(side) = &mut self.side {
            side.placement = self
                .placement
                .zip(self.usable)
                .map(|(menu, usable)| place_beside(&side.layout, menu, usable));
        }
    }

    /// The item under the cursor right now, on the menu or on a strip beside it, without changing
    /// anything. Used by the click handler so a press is resolved against the same geometry the
    /// last frame drew.
    pub fn item_at(&self, cursor: Point) -> Option<MenuIcon> {
        if let Some(index) = self
            .placement
            .and_then(|placement| placement.hover(&self.layout, cursor))
        {
            return self.layout.item(index);
        }
        let side = self.side.as_ref()?;
        let index = side.placement?.hover(&side.layout, cursor)?;
        side.layout.item(index)
    }

    /// Whether a point is on the strip's body — the whole control, cells, border and gaps alike —
    /// or on a strip opened beside it.
    pub fn contains(&self, cursor: Point) -> bool {
        let inside = |body: DesktopRect| {
            cursor.x >= body.x
                && cursor.y >= body.y
                && cursor.x < body.right()
                && cursor.y < body.bottom()
        };
        self.placement
            .is_some_and(|placement| inside(placement.body_desktop()))
            || self
                .side
                .as_ref()
                .and_then(|side| side.placement)
                .is_some_and(|placement| inside(placement.body_desktop()))
    }

    /// The conditions that close a menu at once, whatever the cursor is doing.
    pub fn interruption(&mut self, world: MenuWorld) -> Option<MenuDismissal> {
        let took_focus = world.settings_focused && !self.settings_focused;
        self.settings_focused = world.settings_focused;
        if !world.present {
            return Some(MenuDismissal::Gone);
        }
        if world.hidden {
            return Some(MenuDismissal::Hidden);
        }
        if world.handled {
            return Some(MenuDismissal::Handled);
        }
        if world.occluded {
            return Some(MenuDismissal::Occluded);
        }
        if world.monitor_id != self.monitor_id {
            return Some(MenuDismissal::MonitorChanged);
        }
        if took_focus {
            return Some(MenuDismissal::SettingsFocused);
        }
        None
    }

    /// Advance the hover highlight and the two dismissal timers by one frame. `creature` is the
    /// creature's centre in logical points, so wandering away from the creature counts as staying
    /// with the menu even when the strip itself is some way off.
    pub fn track(&mut self, dt: f32, cursor: Option<Point>, creature: Point) -> MenuTick {
        let hovered = cursor.and_then(|cursor| {
            self.placement
                .and_then(|placement| placement.hover(&self.layout, cursor))
        });
        let mut redraw = hovered != self.hovered;
        self.hovered = hovered;
        let mut side_hovered = None;
        if let Some(side) = &mut self.side {
            side_hovered = cursor.and_then(|cursor| {
                side.placement
                    .and_then(|placement| placement.hover(&side.layout, cursor))
            });
            redraw |= side_hovered != side.hovered;
            side.hovered = side_hovered;
        }

        if hovered.is_some() || side_hovered.is_some() {
            self.untouched_for = 0.0;
        } else {
            self.untouched_for += dt;
        }

        let near = match cursor {
            Some(cursor) => {
                self.contains(cursor)
                    || cursor.distance(creature) <= STRAY_RADIUS
                    || self.placement.is_some_and(|placement| {
                        distance_to(placement.body_desktop(), cursor) <= STRAY_RADIUS
                    })
                    || self
                        .side
                        .as_ref()
                        .and_then(|side| side.placement)
                        .is_some_and(|placement| {
                            distance_to(placement.body_desktop(), cursor) <= STRAY_RADIUS
                        })
            }
            // A cursor the platform cannot see cannot be straying towards anything.
            None => false,
        };
        if near {
            self.strayed_for = 0.0;
        } else {
            self.strayed_for += dt;
        }

        let dismissal = if self.strayed_for >= STRAY_SECS {
            Some(MenuDismissal::Strayed)
        } else if self.untouched_for >= UNTOUCHED_SECS {
            Some(MenuDismissal::Untouched)
        } else {
            None
        };
        MenuTick { redraw, dismissal }
    }
}

/// How far a point lies outside a rectangle. Zero inside it.
fn distance_to(rect: DesktopRect, point: Point) -> f32 {
    let dx = (rect.x - point.x).max(point.x - rect.right()).max(0.0);
    let dy = (rect.y - point.y).max(point.y - rect.bottom()).max(0.0);
    (dx * dx + dy * dy).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use formiga_art::{FRAME_SIZE, MENU_CELL};

    const MONITOR: LocalRect = LocalRect {
        x: 0.0,
        y: 48.0,
        width: 1920.0,
        height: 1032.0,
    };

    fn anchor(centre_x: f32, head_top: f32, display_scale: f32, scale_factor: f32) -> MenuAnchor {
        MenuAnchor {
            centre_x,
            head_top,
            foot_bottom: head_top + 36.0 * display_scale,
            art_scale: display_scale,
            grid: 1.0,
            usable: LocalRect {
                x: MONITOR.x,
                y: MONITOR.y * scale_factor,
                width: MONITOR.width * scale_factor,
                height: MONITOR.height * scale_factor,
            },
            monitor_origin: Point { x: 0.0, y: 0.0 },
            scale_factor,
        }
    }

    fn member_layout() -> MenuLayout {
        MenuLayout::new(&menu_items(MenuTarget::Member, false))
    }

    #[test]
    fn a_member_is_offered_home_and_a_guest_is_offered_stay_or_a_code() {
        assert_eq!(
            menu_items(MenuTarget::Member, false),
            [
                MenuIcon::Snack,
                MenuIcon::Toy,
                MenuIcon::Home,
                MenuIcon::Profile
            ]
        );
        assert_eq!(
            menu_items(MenuTarget::Member, true),
            menu_items(MenuTarget::Member, false),
            "whether a visitor could stay says nothing about a member"
        );
        assert_eq!(
            menu_items(MenuTarget::Guest, true),
            [
                MenuIcon::Snack,
                MenuIcon::Toy,
                MenuIcon::Stay,
                MenuIcon::Profile
            ]
        );
        assert_eq!(
            menu_items(MenuTarget::Guest, false),
            [
                MenuIcon::Snack,
                MenuIcon::Toy,
                MenuIcon::CopyCode,
                MenuIcon::Profile
            ]
        );
        // Stay and copy code swap in the same cell, so the other three never move under the cursor.
        for index in [0, 1, 3] {
            assert_eq!(
                menu_items(MenuTarget::Guest, true)[index],
                menu_items(MenuTarget::Guest, false)[index]
            );
        }
        // Every menu is four cells, so every strip is the frame the atlas has for four.
        for items in [
            menu_items(MenuTarget::Member, false),
            menu_items(MenuTarget::Guest, true),
            menu_items(MenuTarget::Guest, false),
        ] {
            let layout = MenuLayout::new(&items);
            assert_eq!(layout.len(), 4);
            assert!(!layout.is_empty());
        }
    }

    #[test]
    fn a_menu_floats_above_the_head_with_the_notch_on_the_creature() {
        for scale in [2.0_f32, 3.0, 4.0] {
            for factor in [1.0_f32, 2.0] {
                let layout = member_layout();
                let head_top = 600.0;
                let placement = place(&layout, anchor(900.0, head_top, scale, factor));
                assert!(
                    !placement.below,
                    "there is room above at {scale}x/{factor}x"
                );
                assert_eq!(placement.y, head_top - MENU_RISE as f32 * scale);
                assert_eq!(placement.cell_offset_art(), 0);
                // The notch column lands on the creature's centre.
                let notch = placement.x + layout.notch_x() as f32 * scale;
                assert_eq!(notch, 900.0, "the notch points at the creature");
                // And the label tab still clears the head.
                let tab_bottom = placement.y
                    + (MENU_STRIP_HEIGHT + LABEL_TAB_GAP + LABEL_TAB_HEIGHT) as f32 * scale;
                assert_eq!(head_top - tab_bottom, MENU_HEAD_CLEARANCE as f32 * scale);
                assert!(tab_bottom < head_top, "the tab must not cover the creature");
            }
        }
    }

    #[test]
    fn a_menu_with_no_room_above_flips_under_the_creature_with_the_notch_up() {
        let layout = member_layout();
        for scale in [2.0_f32, 3.0, 4.0] {
            let usable_top = MONITOR.y;
            // A creature right under the menu bar has nowhere to put a strip above it.
            let head_top = usable_top + 4.0;
            let anchor = anchor(900.0, head_top, scale, 1.0);
            let placement = place(&layout, anchor);
            assert!(placement.below, "no room above at {scale}x");
            assert_eq!(
                placement.y,
                anchor.foot_bottom + MENU_NOTCH_GAP as f32 * scale
            );
            // Flipped, the notch is the sprite's top rows and the cells start below them.
            assert_eq!(placement.cell_offset_art(), MENU_NOTCH_HEIGHT as i32);
            assert_eq!(
                placement.body_rect().y,
                placement.y + MENU_NOTCH_HEIGHT as f32 * scale
            );
            assert!(placement.body_rect().y > head_top, "the strip hangs below");
            // Flipping never moves the notch sideways.
            let notch = placement.x + layout.notch_x() as f32 * scale;
            assert_eq!(notch, 900.0);
        }
    }

    #[test]
    fn a_menu_at_the_edge_of_a_display_is_clamped_inside_the_usable_area() {
        let layout = member_layout();
        let scale = 3.0;
        let width = layout.size().0 as f32 * scale;
        for factor in [1.0_f32, 2.0] {
            let usable = anchor(0.0, 600.0, scale, factor).usable;
            // Hard against the left edge.
            let left = place(&layout, anchor(usable.x + 4.0, 600.0, scale, factor));
            assert_eq!(left.x, usable.x);
            assert!(left.frame_rect().right() <= usable.right());
            // And against the right.
            let right = place(&layout, anchor(usable.right() - 4.0, 600.0, scale, factor));
            assert_eq!(right.x, usable.right() - width);
            assert!(right.x >= usable.x);
            // The notch follows the strip as far as the clamp allows, and no further.
            let notch = right.x + layout.notch_x() as f32 * scale;
            assert!(notch < usable.right() - 4.0);
            assert!(notch > usable.right() - width);
        }
    }

    #[test]
    fn a_menu_below_a_creature_near_the_floor_stays_on_the_display() {
        let layout = member_layout();
        let scale = 4.0;
        let mut anchor = anchor(900.0, MONITOR.y + 4.0, scale, 1.0);
        anchor.foot_bottom = anchor.usable.bottom() - 8.0;
        let placement = place(&layout, anchor);
        let tail = (LABEL_TAB_GAP + LABEL_TAB_HEIGHT) as f32 * scale;
        assert!(
            placement.frame_rect().bottom() + tail <= anchor.usable.bottom(),
            "the label tab must stay on the display"
        );
        assert!(placement.y >= anchor.usable.y);
    }

    #[test]
    fn placement_lands_on_the_drawables_own_pixel_grid() {
        let layout = member_layout();
        for grid in [1.0_f32, 2.0] {
            // Odd numbers on both axes, so a two-pixel grid has something to round.
            let mut anchor = anchor(902.0, 602.0, 3.0, 2.0);
            anchor.grid = grid;
            let placement = place(&layout, anchor);
            assert_eq!(placement.x % grid, 0.0, "x is off the {grid}px grid");
            assert_eq!(placement.y % grid, 0.0, "y is off the {grid}px grid");
        }
    }

    #[test]
    fn the_cursor_finds_every_cell_and_nothing_between_them() {
        let layout = member_layout();
        for scale in [2.0_f32, 3.0, 4.0] {
            for factor in [1.0_f32, 2.0] {
                let mut anchor = anchor(900.0, 600.0, scale, factor);
                anchor.monitor_origin = Point {
                    x: -1600.0,
                    y: 40.0,
                };
                let placement = place(&layout, anchor);
                let body = placement.body_desktop();
                let art = scale / factor;
                for index in 0..4 {
                    let cell = layout.cell(index).expect("four cells");
                    let centre = Point {
                        x: body.x + (cell.x as f32 + MENU_CELL as f32 / 2.0) * art,
                        y: body.y + (cell.y as f32 + MENU_CELL as f32 / 2.0) * art,
                    };
                    assert_eq!(
                        placement.hover(&layout, centre),
                        Some(index),
                        "cell {index} at {scale}x/{factor}x"
                    );
                }
                // The border and the gaps between cells belong to no item.
                let corner = Point {
                    x: body.x,
                    y: body.y,
                };
                assert_eq!(placement.hover(&layout, corner), None);
                let outside = Point {
                    x: body.x - 1.0,
                    y: body.y - 1.0,
                };
                assert_eq!(placement.hover(&layout, outside), None);
            }
        }
    }

    fn open() -> CreatureMenu {
        let mut menu = CreatureMenu::new(7, MenuTarget::Member, false, 1, false);
        menu.attach(Some(anchor(900.0, 600.0, 3.0, 1.0)));
        menu
    }

    fn cell_centre(menu: &CreatureMenu, index: usize) -> Point {
        let placement = menu.placement().expect("placed");
        let body = placement.body_desktop();
        let cell = menu.layout().cell(index).expect("cell");
        let art = placement.art_scale;
        Point {
            x: body.x + (cell.x as f32 + MENU_CELL as f32 / 2.0) * art,
            y: body.y + (cell.y as f32 + MENU_CELL as f32 / 2.0) * art,
        }
    }

    #[test]
    fn hovering_a_cell_redraws_once_and_names_the_item() {
        let mut menu = open();
        let creature = Point { x: 900.0, y: 648.0 };
        let over = cell_centre(&menu, 2);
        let first = menu.track(0.05, Some(over), creature);
        assert!(first.redraw, "the highlight has just appeared");
        assert_eq!(menu.hovered(), Some(2));
        assert_eq!(menu.item_at(over), Some(MenuIcon::Home));
        // Staying still is not a change, so the overlay is not asked to draw again.
        assert!(!menu.track(0.05, Some(over), creature).redraw);
        let away = cell_centre(&menu, 0);
        assert!(menu.track(0.05, Some(away), creature).redraw);
        assert_eq!(menu.hovered(), Some(0));
        assert_eq!(menu.item_at(away), Some(MenuIcon::Snack));
    }

    #[test]
    fn a_menu_nobody_touches_closes_after_eight_seconds() {
        let mut menu = open();
        let creature = Point { x: 900.0, y: 648.0 };
        // Beside the creature: near enough not to stray, but on no cell.
        let beside = Point { x: 930.0, y: 648.0 };
        let mut elapsed = 0.0;
        loop {
            let tick = menu.track(0.1, Some(beside), creature);
            elapsed += 0.1;
            if let Some(dismissal) = tick.dismissal {
                assert_eq!(dismissal, MenuDismissal::Untouched);
                break;
            }
            assert!(elapsed < UNTOUCHED_SECS + 1.0, "the menu never closed");
        }
        assert!(elapsed >= UNTOUCHED_SECS);

        // Touching an item restarts the count.
        let mut menu = open();
        for _ in 0..70 {
            assert!(menu.track(0.1, Some(beside), creature).dismissal.is_none());
            menu.track(0.1, Some(cell_centre(&menu, 1)), creature);
        }
    }

    #[test]
    fn a_cursor_that_walks_away_from_both_the_strip_and_the_creature_closes_the_menu() {
        let mut menu = open();
        let creature = Point { x: 900.0, y: 648.0 };
        let far = Point {
            x: 1800.0,
            y: 100.0,
        };
        let mut elapsed = 0.0;
        loop {
            let tick = menu.track(0.1, Some(far), creature);
            elapsed += 0.1;
            if let Some(dismissal) = tick.dismissal {
                assert_eq!(dismissal, MenuDismissal::Strayed);
                break;
            }
            assert!(elapsed < STRAY_SECS + 1.0, "the menu never closed");
        }
        assert!(elapsed >= STRAY_SECS);
        assert!(
            elapsed < UNTOUCHED_SECS,
            "straying closes long before idling"
        );
    }

    #[test]
    fn staying_beside_the_creature_or_the_strip_never_counts_as_straying() {
        let mut menu = open();
        let creature = Point { x: 900.0, y: 648.0 };
        for point in [
            Point { x: 900.0, y: 648.0 },
            Point {
                x: 900.0,
                y: 648.0 + STRAY_RADIUS - 1.0,
            },
            cell_centre(&menu, 0),
            Point {
                x: menu.placement().expect("placed").body_desktop().x - STRAY_RADIUS + 1.0,
                y: menu.placement().expect("placed").body_desktop().y,
            },
        ] {
            for _ in 0..20 {
                assert_eq!(
                    menu.track(0.1, Some(point), creature).dismissal,
                    None,
                    "{point:?} is still in the neighbourhood"
                );
            }
        }
    }

    #[test]
    fn a_cursor_the_platform_cannot_see_closes_the_menu() {
        let mut menu = open();
        let creature = Point { x: 900.0, y: 648.0 };
        let mut dismissal = None;
        for _ in 0..20 {
            dismissal = dismissal.or(menu.track(0.1, None, creature).dismissal);
        }
        assert_eq!(dismissal, Some(MenuDismissal::Strayed));
    }

    #[test]
    fn every_disturbance_closes_the_menu_at_once() {
        let mut menu = open();
        let calm = MenuWorld {
            present: true,
            monitor_id: 1,
            handled: false,
            hidden: false,
            occluded: false,
            settings_focused: false,
        };
        assert_eq!(menu.interruption(calm), None);
        for (world, expected) in [
            (
                MenuWorld {
                    present: false,
                    ..calm
                },
                MenuDismissal::Gone,
            ),
            (
                MenuWorld {
                    handled: true,
                    ..calm
                },
                MenuDismissal::Handled,
            ),
            (
                MenuWorld {
                    hidden: true,
                    ..calm
                },
                MenuDismissal::Hidden,
            ),
            (
                MenuWorld {
                    occluded: true,
                    ..calm
                },
                MenuDismissal::Occluded,
            ),
            (
                MenuWorld {
                    monitor_id: 2,
                    ..calm
                },
                MenuDismissal::MonitorChanged,
            ),
            (
                MenuWorld {
                    settings_focused: true,
                    ..calm
                },
                MenuDismissal::SettingsFocused,
            ),
        ] {
            let mut fresh = open();
            assert_eq!(fresh.interruption(world), Some(expected), "{world:?}");
        }
    }

    #[test]
    fn a_settings_window_that_was_already_focused_does_not_refuse_the_menu() {
        let focused = MenuWorld {
            present: true,
            monitor_id: 1,
            handled: false,
            hidden: false,
            occluded: false,
            settings_focused: true,
        };
        // Opened while the settings window happened to have focus: the owner still gets a menu,
        // and keeps it for as long as nothing changes.
        let mut menu = CreatureMenu::new(7, MenuTarget::Member, false, 1, true);
        for _ in 0..5 {
            assert_eq!(menu.interruption(focused), None);
        }
        // Losing focus and then taking it back is a change, and that does close the menu.
        assert_eq!(
            menu.interruption(MenuWorld {
                settings_focused: false,
                ..focused
            }),
            None
        );
        assert_eq!(
            menu.interruption(focused),
            Some(MenuDismissal::SettingsFocused)
        );
    }

    #[test]
    fn a_menu_that_cannot_be_placed_answers_nothing_and_closes_on_its_own() {
        let mut menu = open();
        menu.attach(None);
        assert_eq!(menu.placement(), None);
        let cursor = Point { x: 900.0, y: 600.0 };
        assert_eq!(menu.item_at(cursor), None);
        assert!(!menu.contains(cursor));
        // With no strip to be near, only the creature keeps it open — and it is not there either.
        let mut dismissal = None;
        for _ in 0..20 {
            dismissal = dismissal.or(menu
                .track(
                    0.1,
                    Some(Point { x: 10.0, y: 10.0 }),
                    Point {
                        x: 1800.0,
                        y: 900.0,
                    },
                )
                .dismissal);
        }
        assert_eq!(dismissal, Some(MenuDismissal::Strayed));
    }

    #[test]
    fn a_guest_menu_carries_the_guest_and_swaps_only_its_third_cell() {
        let stay = CreatureMenu::new(3, MenuTarget::Guest, true, 1, false);
        let code = CreatureMenu::new(3, MenuTarget::Guest, false, 1, false);
        assert_eq!(stay.target(), MenuTarget::Guest);
        assert_eq!(stay.creature_id(), 3);
        assert_eq!(stay.layout().item(2), Some(MenuIcon::Stay));
        assert_eq!(code.layout().item(2), Some(MenuIcon::CopyCode));
        assert_eq!(stay.layout().size(), code.layout().size());
        for index in [0, 1, 3] {
            assert_eq!(stay.layout().item(index), code.layout().item(index));
            assert_eq!(stay.layout().cell(index), code.layout().cell(index));
        }
    }

    /// While the houses are out, the cell that would send everyone home offers the village's
    /// moments instead, and nothing else in the strip moves.
    #[test]
    fn with_the_houses_out_home_becomes_moment_in_the_same_cell() {
        let plain = CreatureMenu::new(7, MenuTarget::Member, false, 1, false);
        let moments = CreatureMenu::new(7, MenuTarget::Member, false, 1, false).offering_moments();
        assert_eq!(plain.layout().item(2), Some(MenuIcon::Home));
        assert_eq!(moments.layout().item(2), Some(MenuIcon::Moment));
        assert_eq!(plain.layout().size(), moments.layout().size());
        for index in [0, 1, 3] {
            assert_eq!(plain.layout().item(index), moments.layout().item(index));
            assert_eq!(plain.layout().cell(index), moments.layout().cell(index));
        }
        // A guest's menu has no Home to swap.
        let guest = CreatureMenu::new(3, MenuTarget::Guest, true, 1, false).offering_moments();
        assert_eq!(guest.layout().item(2), Some(MenuIcon::Stay));
    }

    fn side_cell_centre(menu: &CreatureMenu, index: usize) -> Point {
        let side = menu.side().expect("a strip beside the menu");
        let placement = side.placement().expect("placed");
        let body = placement.body_desktop();
        let cell = side.layout().cell(index).expect("cell");
        let art = placement.art_scale / 1.0;
        Point {
            x: body.x + (cell.x as f32 + MENU_CELL as f32 / 2.0) * art,
            y: body.y + (cell.y as f32 + MENU_CELL as f32 / 2.0) * art,
        }
    }

    /// Opening the moments puts a second strip beside the first, on its row and level with its
    /// body, without moving the menu under a pointer already on it. The strip answers the cursor
    /// and clicks like the menu does, and choosing Moment again puts it away.
    #[test]
    fn the_moments_open_beside_the_menu_without_moving_it() {
        let mut menu = open().offering_moments();
        menu.attach(Some(anchor(900.0, 600.0, 3.0, 1.0)));
        let before = menu.placement().unwrap();
        let moments = [MenuIcon::Picnic, MenuIcon::Dance, MenuIcon::Nap];
        assert!(menu.toggle_side(&moments));
        menu.attach(Some(anchor(900.0, 600.0, 3.0, 1.0)));
        assert_eq!(menu.placement(), Some(before), "the menu stays put");
        let side = menu.side().unwrap().placement().unwrap();
        let body = before.body_rect();
        assert_eq!(side.y, body.y, "level with the menu's body");
        assert_eq!(
            side.x,
            before.frame_rect().right() + SIDE_STRIP_GAP as f32 * 3.0,
            "to the right, where there is room"
        );
        assert_eq!(
            side.label_y,
            before.y + (MENU_STRIP_HEIGHT + LABEL_TAB_GAP) as f32 * 3.0,
            "its labels hang level with the menu's"
        );
        for (index, icon) in moments.into_iter().enumerate() {
            let centre = side_cell_centre(&menu, index);
            assert_eq!(menu.item_at(centre), Some(icon));
            assert!(menu.contains(centre));
            let tick = menu.track(0.1, Some(centre), Point { x: 900.0, y: 700.0 });
            assert_eq!(menu.side().unwrap().hovered(), Some(index));
            assert_eq!(menu.hovered(), None);
            assert_eq!(tick.dismissal, None);
        }
        // The menu's own cells still answer as they did.
        assert_eq!(menu.item_at(cell_centre(&menu, 0)), Some(MenuIcon::Snack));
        // One click target covers both strips, and the gap between them.
        let area = menu.click_area().unwrap();
        let menu_body = before.body_desktop();
        let side_body = side.body_desktop();
        assert_eq!(area.x, menu_body.x);
        assert_eq!(area.right(), side_body.right());
        assert_eq!(area.height, menu_body.height);
        // Choosing Moment again puts the strip away.
        assert!(!menu.toggle_side(&moments));
        assert!(menu.side().is_none());
        assert_eq!(menu.click_area(), Some(menu_body));
        // A strip needs two to four items.
        assert!(!menu.toggle_side(&[MenuIcon::Stop]));
        assert!(menu.side().is_none());
    }

    /// Against the right edge of the display the strip opens to the left of the menu instead,
    /// still without moving it.
    #[test]
    fn at_the_right_edge_the_moments_open_to_the_left() {
        let scale = 3.0;
        let usable = anchor(0.0, 600.0, scale, 1.0).usable;
        let at = anchor(usable.right() - 40.0, 600.0, scale, 1.0);
        let mut menu = CreatureMenu::new(7, MenuTarget::Member, false, 1, false).offering_moments();
        menu.attach(Some(at));
        let before = menu.placement().unwrap();
        assert!(menu.toggle_side(&[MenuIcon::Stop, MenuIcon::Nap]));
        menu.attach(Some(at));
        assert_eq!(menu.placement(), Some(before));
        let side = menu.side().unwrap().placement().unwrap();
        assert!(
            side.body_rect().right() <= before.x,
            "to the left of the menu"
        );
        assert!(side.x >= usable.x);
    }

    #[test]
    fn a_mini_gets_its_menu_closer_than_an_adult_because_its_head_is_lower() {
        let layout = member_layout();
        let scale = 3.0;
        // Two creatures standing on the same line: a tall one and a small one. Whole art pixels,
        // so the pixel grid cannot absorb the difference the test is looking for.
        let contact = 800.0;
        let adult_head = contact - FRAME_SIZE as f32 * scale * 0.75;
        let mini_head = contact - FRAME_SIZE as f32 * scale * 0.375;
        let adult = place(&layout, anchor(900.0, adult_head, scale, 1.0));
        let mini = place(&layout, anchor(900.0, mini_head, scale, 1.0));
        assert!(mini.y > adult.y, "the small creature's menu hangs lower");
        assert_eq!(mini.y - adult.y, mini_head - adult_head);
    }
}
