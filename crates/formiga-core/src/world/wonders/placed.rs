//! Wonders the person at the desk puts out from the notebook.
//!
//! A wonder put out stays where it was left — on the floor of a display, or up on top of a window,
//! riding it — until it is put away. Asked to, the nearest companion free for it goes over and has
//! a go, with a friend if it is a wonder for two; now and then one goes over of its own accord, in
//! place of a wonder turning up by itself. A go on one is the same script, with the same players'
//! feet owned the same way, as a go on a wonder that turned up: only, when it is over, the wonder
//! stays.

use super::*;
use crate::{MAX_PLACED_WONDERS, PlacedWonder};

/// When a wonder is due and one has been put out, how often somebody goes over to one of those
/// instead of a new one turning up.
pub(super) const PUT_OUT_SHARE: f64 = 0.5;
/// How far above or below a window's top edge a wonder can be let go of and still be set on it.
const LEDGE_REACH: f32 = 40.0;
/// How far in from a window's corners a wonder on its top edge stands.
const LEDGE_INSET: f32 = 12.0;
/// Where along a floor a wonder just put out is tried, in turn, as fractions of its width.
const PUT_OUT_PLACES: [f32; 7] = [0.5, 0.35, 0.65, 0.2, 0.8, 0.08, 0.92];

/// Art pixels to points on this display.
fn unit_on(monitor: &MonitorInfo, display_scale: u8) -> f32 {
    spacing::frame_width(display_scale, monitor.scale_factor) / CREATURE_ART_WIDTH
}

/// Half the floor a wonder needs: its players' room where the floor has it, its own span where
/// it does not.
fn half_room(kind: WonderKind, unit: f32, across: f32) -> f32 {
    let room = kind.room() * unit / 2.0;
    if room * 2.0 <= across {
        room
    } else {
        (kind.span() * unit / 2.0).min(across / 2.0)
    }
}

/// `x` moved off the stretch `low..high` to whichever side is nearer, if a wonder `half` either
/// side of it would reach into it, and kept inside `min..max`.
fn clear_of(x: f32, half: f32, span: Option<(f32, f32)>, min: f32, max: f32) -> f32 {
    let x = x.clamp(min, max.max(min));
    let Some((low, high)) = span else {
        return x;
    };
    if x + half <= low || x - half >= high {
        return x;
    }
    let left = low - half;
    let right = high + half;
    let left_fits = left >= min;
    let right_fits = right <= max;
    match (left_fits, right_fits) {
        (true, true) if x - left <= right - x => left,
        (true, false) => left,
        (_, true) => right,
        _ => x,
    }
}

impl World {
    /// Every wonder put out, where it stands now, for the overlay to draw and the pointer to
    /// click. One being played on moves as it does then; the rest stand still, running only
    /// their own little loops.
    pub fn placed_wonders(&self) -> Vec<WonderView> {
        self.wonders
            .placed
            .iter()
            .map(|spot| {
                let active = self.wonders.active.as_ref().filter(|active| {
                    active.placed && active.kind == spot.kind && !active.leaving()
                });
                let t = active.and_then(Active::script_time);
                let length = active.map_or(script_length(spot.kind), |active| active.length);
                let toss = active.is_some_and(|active| active.toss);
                let mut at = spot.at;
                if let Some(active) = active {
                    at = active.place(carried(spot.kind, t, length), 0.0);
                    at.y = spot.at.y;
                }
                WonderView {
                    kind: spot.kind,
                    at,
                    monitor_id: spot.ground.monitor(),
                    window_key: spot.ground.window(),
                    mirrored: active.is_some_and(|active| active.mirrored),
                    presence: 1.0,
                    elapsed: active.map_or(0.0, |active| active.elapsed),
                    motion: motion(spot.kind, t, length, toss),
                    glint: false,
                }
            })
            .collect()
    }

    /// Whether any wonder put out is being played on just now, or has players on their way.
    pub fn placed_wonder_playing(&self) -> bool {
        self.wonders
            .active
            .as_ref()
            .is_some_and(|active| active.placed && !active.leaving())
    }

    /// Whether this wonder is being played on just now, or its players are on their way to it.
    pub fn placed_wonder_in_use(&self, kind: WonderKind) -> bool {
        self.wonders
            .active
            .as_ref()
            .is_some_and(|active| active.placed && active.kind == kind && !active.leaving())
    }

    /// Works out again where every wonder put out stands, from the save and the desktop.
    pub(super) fn place_wonders(&mut self, desktop: &DesktopSnapshot) {
        let placed = self.save.companion.placed_wonders.clone();
        self.wonders
            .on_window
            .retain(|(kind, ..)| placed.iter().any(|wonder| wonder.kind == *kind));
        let village = self.shown_village_span(desktop);
        self.wonders.placed = placed
            .iter()
            .filter_map(|wonder| self.resolve_placed(wonder, desktop, village))
            .collect();
    }

    /// Where the village stands on the floor while it is on the desktop at all, and on which
    /// display: nothing put out stands in among the houses.
    fn shown_village_span(&self, desktop: &DesktopSnapshot) -> Option<(MonitorId, f32, f32)> {
        if !self.save.home.shown() {
            return None;
        }
        let commons = home_commons(
            &self.save.home,
            colony_cottage_list(&self.save.creatures).as_slice(),
            &desktop.monitors,
            &self.save.settings.habitat,
            self.save.settings.display_scale,
        )?;
        Some((commons.monitor_id, commons.low_x, commons.high_x))
    }

    fn resolve_placed(
        &mut self,
        wonder: &PlacedWonder,
        desktop: &DesktopSnapshot,
        village: Option<(MonitorId, f32, f32)>,
    ) -> Option<PlacedSpot> {
        let display_scale = self.save.settings.display_scale;
        if let Some(index) = self
            .wonders
            .on_window
            .iter()
            .position(|(kind, ..)| *kind == wonder.kind)
        {
            let (_, key, along) = self.wonders.on_window[index];
            let riding = desktop
                .windows
                .iter()
                .find(|window| window.key == key && window.visible && !window.minimized)
                .and_then(|window| {
                    let bounds = window.bounds;
                    let x = bounds.x + along;
                    let monitor = desktop
                        .monitors
                        .iter()
                        .find(|monitor| monitor.bounds.contains(Point { x, y: bounds.y }))?;
                    let unit = unit_on(monitor, display_scale);
                    let half = wonder.kind.span() * unit / 2.0;
                    let (low, high) = (
                        bounds.x + LEDGE_INSET + half,
                        bounds.right() - LEDGE_INSET - half,
                    );
                    (high >= low).then(|| (monitor.clone(), bounds, x.clamp(low, high), unit))
                });
            if let Some((monitor, bounds, x, unit)) = riding {
                // The floor under it is kept, so a relaunch finds it there.
                if let Some(saved) = self
                    .save
                    .companion
                    .placed_wonders
                    .iter_mut()
                    .find(|saved| saved.kind == wonder.kind)
                {
                    let usable = monitor.usable_bounds;
                    saved.display = monitor.display_key;
                    saved.along = ((x - usable.x) / usable.width.max(1.0)).clamp(0.0, 1.0);
                }
                return Some(PlacedSpot {
                    kind: wonder.kind,
                    ground: Ground::Ledge {
                        monitor: monitor.id,
                        window: key,
                        bounds,
                    },
                    at: Point { x, y: bounds.y },
                    unit,
                });
            }
            // The window closed, shrank or went away: down onto the floor under it.
            self.wonders.on_window.remove(index);
        }
        let monitor = desktop
            .monitors
            .iter()
            .find(|monitor| monitor.display_key == wonder.display)
            .or_else(|| desktop.monitors.iter().find(|monitor| monitor.primary))
            .or_else(|| desktop.monitors.first())?;
        let unit = unit_on(monitor, display_scale);
        let usable = monitor.usable_bounds;
        let half = half_room(wonder.kind, unit, usable.width);
        let span = village
            .filter(|(id, ..)| *id == monitor.id)
            .map(|(_, low, high)| (low, high));
        let x = clear_of(
            usable.x + wonder.along * usable.width,
            wonder.kind.span() * unit / 2.0,
            span,
            usable.x + half,
            usable.right() - half,
        );
        Some(PlacedSpot {
            kind: wonder.kind,
            ground: Ground::Floor {
                monitor: monitor.id,
            },
            at: Point {
                x,
                y: usable.bottom() - 4.0,
            },
            unit,
        })
    }

    /// Puts a wonder the colony has found out on the floor of the village's display, or the main
    /// one, somewhere clear of the village and of anything else put out. False when it is not one
    /// the colony has found, is out already, or as many are out as can be.
    pub(in crate::world) fn put_out_wonder(
        &mut self,
        kind: WonderKind,
        desktop: &DesktopSnapshot,
    ) -> bool {
        let placed = &self.save.companion.placed_wonders;
        if placed.len() >= MAX_PLACED_WONDERS
            || placed.iter().any(|wonder| wonder.kind == kind)
            || !self
                .save
                .companion
                .wonders
                .iter()
                .any(|record| record.kind == kind)
        {
            return false;
        }
        self.place_wonders(desktop);
        let Some(monitor) = self
            .save
            .home
            .display
            .and_then(|display| {
                desktop
                    .monitors
                    .iter()
                    .find(|monitor| monitor.display_key == display)
            })
            .or_else(|| desktop.monitors.iter().find(|monitor| monitor.primary))
            .or_else(|| desktop.monitors.first())
            .cloned()
        else {
            return false;
        };
        let unit = unit_on(&monitor, self.save.settings.display_scale);
        let usable = monitor.usable_bounds;
        let half = half_room(kind, unit, usable.width);
        let mut taken: Vec<(f32, f32)> = self
            .wonders
            .placed
            .iter()
            .filter(|spot| {
                spot.ground
                    == Ground::Floor {
                        monitor: monitor.id,
                    }
            })
            .map(|spot| {
                let other = half_room(spot.kind, spot.unit, usable.width);
                (spot.at.x - other, spot.at.x + other)
            })
            .collect();
        if let Some((id, low, high)) = self.shown_village_span(desktop)
            && id == monitor.id
        {
            taken.push((low, high));
        }
        let fits = |x: f32| {
            taken
                .iter()
                .all(|(low, high)| x + half <= *low || x - half >= *high)
        };
        let along = PUT_OUT_PLACES
            .into_iter()
            .find(|along| fits(usable.x + along * usable.width))
            .unwrap_or(0.5);
        self.save.companion.placed_wonders.push(PlacedWonder {
            kind,
            display: monitor.display_key,
            along,
        });
        self.place_wonders(desktop);
        true
    }

    /// Puts a wonder back away. False when it was not out.
    pub(in crate::world) fn put_away_wonder(
        &mut self,
        kind: WonderKind,
        desktop: &DesktopSnapshot,
    ) -> bool {
        let before = self.save.companion.placed_wonders.len();
        self.save
            .companion
            .placed_wonders
            .retain(|wonder| wonder.kind != kind);
        if self.save.companion.placed_wonders.len() == before {
            return false;
        }
        self.leave_placed_wonder(kind, desktop);
        self.place_wonders(desktop);
        true
    }

    /// Sets a wonder down where the pointer let go of it: on top of the window whose top edge is
    /// right there, if window ledges are on and that edge is in plain view and wide enough, and
    /// otherwise on the floor under the pointer, clear of the village. False when it was not out.
    pub(in crate::world) fn move_wonder(
        &mut self,
        kind: WonderKind,
        to: Point,
        desktop: &DesktopSnapshot,
    ) -> bool {
        if !self
            .save
            .companion
            .placed_wonders
            .iter()
            .any(|wonder| wonder.kind == kind)
        {
            return false;
        }
        self.leave_placed_wonder(kind, desktop);
        self.wonders
            .on_window
            .retain(|(placed, ..)| *placed != kind);
        let display_scale = self.save.settings.display_scale;
        let Some(monitor) = desktop
            .monitors
            .iter()
            .find(|monitor| monitor.bounds.contains(to))
            .or_else(|| {
                let middle = |bounds: DesktopRect| Point {
                    x: bounds.x + bounds.width / 2.0,
                    y: bounds.y + bounds.height / 2.0,
                };
                desktop.monitors.iter().min_by(|a, b| {
                    middle(a.bounds)
                        .distance(to)
                        .total_cmp(&middle(b.bounds).distance(to))
                })
            })
            .cloned()
        else {
            return false;
        };
        let unit = unit_on(&monitor, display_scale);
        let half = kind.span() * unit / 2.0;
        if self.save.settings.window_ledges {
            let mut near: Vec<&DesktopWindow> = desktop
                .windows
                .iter()
                .filter(|window| {
                    let bounds = window.bounds;
                    window.visible
                        && !window.minimized
                        && (to.y - bounds.y).abs() <= LEDGE_REACH
                        && to.x >= bounds.x + LEDGE_INSET
                        && to.x <= bounds.right() - LEDGE_INSET
                        && bounds.width >= kind.span() * unit + LEDGE_INSET * 2.0
                        && point_exposed(
                            Point {
                                x: to.x,
                                y: bounds.y - 4.0,
                            },
                            Some(window.key),
                            desktop,
                        )
                })
                .collect();
            // The frontmost of them: lower is in front.
            near.sort_by_key(|window| window.z_order);
            if let Some(window) = near.first() {
                let bounds = window.bounds;
                let x = to.x.clamp(
                    bounds.x + LEDGE_INSET + half,
                    bounds.right() - LEDGE_INSET - half,
                );
                self.wonders
                    .on_window
                    .push((kind, window.key, x - bounds.x));
                self.place_wonders(desktop);
                return true;
            }
        }
        let usable = monitor.usable_bounds;
        let along = ((to.x - usable.x) / usable.width.max(1.0)).clamp(0.0, 1.0);
        if let Some(saved) = self
            .save
            .companion
            .placed_wonders
            .iter_mut()
            .find(|wonder| wonder.kind == kind)
        {
            saved.display = monitor.display_key;
            saved.along = along;
        }
        // Set down in among the houses, it stands beside them instead, and is kept there.
        self.place_wonders(desktop);
        if let Some(spot) = self.wonders.placed.iter().find(|spot| spot.kind == kind)
            && let Some(saved) = self
                .save
                .companion
                .placed_wonders
                .iter_mut()
                .find(|wonder| wonder.kind == kind)
        {
            saved.along = ((spot.at.x - usable.x) / usable.width.max(1.0)).clamp(0.0, 1.0);
        }
        true
    }

    /// Asks for a go on a wonder put out: the nearest companion free for it goes straight over,
    /// with the nearest friend free too if it is one for two, leaving whatever wonder was out
    /// before. False when nobody free can get to it, or it is not out.
    pub(in crate::world) fn play_on_wonder(
        &mut self,
        kind: WonderKind,
        desktop: &DesktopSnapshot,
    ) -> bool {
        if self.placed_wonder_in_use(kind) {
            return true;
        }
        if self.save.settings.paused || !self.save.settings.visible {
            return false;
        }
        self.place_wonders(desktop);
        let Some(spot) = self
            .wonders
            .placed
            .iter()
            .copied()
            .find(|spot| spot.kind == kind)
        else {
            return false;
        };
        self.start_placed_wonder(spot, desktop, true)
    }

    /// Sends the nearest free companion, and a friend for a wonder for two, over to a wonder put
    /// out. `asked` when the person at the desk asked for it: a wonder already out is left for
    /// it, and a wonder for one or two always takes two if two are free.
    pub(super) fn start_placed_wonder(
        &mut self,
        spot: PlacedSpot,
        desktop: &DesktopSnapshot,
        asked: bool,
    ) -> bool {
        let home = self.save.home.is_active();
        let mut free: Vec<(f32, CreatureId)> = self
            .save
            .creatures
            .iter()
            .filter(|creature| self.free_for_a_wonder(creature, home))
            .map(|creature| {
                // Nearest first, and anyone on another display after everyone on this one.
                let elsewhere = if creature.state.surface.monitor_id == spot.ground.monitor() {
                    0.0
                } else {
                    1.0e6
                };
                (
                    creature.state.position.distance(spot.at) + elsewhere,
                    creature.id,
                )
            })
            .collect();
        free.sort_by(|a, b| a.0.total_cmp(&b.0));
        let Some(&(_, lead)) = free.first() else {
            return false;
        };
        let partners: Vec<CreatureId> = free.iter().skip(1).map(|(_, id)| *id).collect();
        let pair = match spot.kind.seats() {
            WonderSeats::One => false,
            WonderSeats::Two => true,
            WonderSeats::OneOrTwo => {
                !partners.is_empty() && (asked || self.wonders.rng.random_bool(0.7))
            }
        };
        if pair && partners.is_empty() {
            return false;
        }
        let toss = self.wonders.rng.random_bool(0.5);
        if asked && self.wonders.active.is_some() {
            self.send_wonder_away(Some(desktop));
            self.wonders.active = None;
        }
        self.begin_wonder(
            Active {
                kind: spot.kind,
                ground: spot.ground,
                at: spot.at,
                unit: spot.unit,
                mirrored: false,
                players: Vec::new(),
                phase: Phase::Gathering,
                elapsed: 0.0,
                waited: 0.0,
                length: script_length(spot.kind),
                toss,
                first: false,
                placed: true,
            },
            lead,
            &partners,
            pair,
            desktop,
        )
    }

    /// Lets the players of this wonder go, if it is the one being played on, where they stand.
    fn leave_placed_wonder(&mut self, kind: WonderKind, desktop: &DesktopSnapshot) {
        if self
            .wonders
            .active
            .as_ref()
            .is_some_and(|active| active.placed && active.kind == kind)
        {
            self.send_wonder_away(Some(desktop));
            self.wonders.active = None;
        }
    }
}
