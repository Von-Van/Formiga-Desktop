//! Bounded, transient observations of geometry, never desktop content or persisted history.
use crate::{DesktopRect, DesktopSnapshot, MAX_TOPOLOGY_WINDOWS, Point, WindowKey};
use std::hash::{Hash, Hasher};

const MAX_SIGNALS: usize = 16;
const SIGNAL_LIFETIME: f32 = 3.5;

/// Timestamp of an actual OS window scan, rather than of a simulation tick using cached windows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WindowSample {
    pub monotonic_millis: u64,
    pub reliable: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttentionEmotion {
    Curious,
    Startled,
    Enjoying,
    Concerned,
    Relieved,
    Averting,
}

/// A presentation hint shared with the art renderer. This is explicitly excluded from saves.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AttentionPose {
    pub target: Point,
    pub emotion: AttentionEmotion,
    /// 0 is planted foot contact, 1 is a hand-supported pose. Runtime only.
    pub hanging: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GeometryChange {
    Appeared,
    Expanded,
    Moved,
    Rearranged,
    Disappeared,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct GeometrySignal {
    pub origin: u64,
    pub window: WindowKey,
    pub bounds: DesktopRect,
    pub kind: GeometryChange,
    pub strength: f32,
    remaining: f32,
    confirmed: bool,
}

#[derive(Clone, Copy, Debug)]
struct WindowShape {
    key: WindowKey,
    bounds: DesktopRect,
    moved_at: Option<f64>,
}

pub(crate) struct GeometryObserver {
    ambience: crate::ambience::AmbienceTracker,
    previous: Vec<WindowShape>,
    current: Vec<WindowShape>,
    signals: Vec<GeometrySignal>,
    sample_at: Option<f64>,
    fixture_clock: f64,
    monitor_hash: u64,
    initialized: bool,
    next_origin: u64,
}

impl Default for GeometryObserver {
    fn default() -> Self {
        Self {
            ambience: crate::ambience::AmbienceTracker::default(),
            previous: Vec::with_capacity(MAX_TOPOLOGY_WINDOWS),
            current: Vec::with_capacity(MAX_TOPOLOGY_WINDOWS),
            signals: Vec::with_capacity(MAX_SIGNALS),
            sample_at: None,
            fixture_clock: 0.0,
            monitor_hash: 0,
            initialized: false,
            next_origin: 0,
        }
    }
}

impl GeometryObserver {
    pub fn ambience(&self, monitor: crate::MonitorId) -> crate::DesktopAmbience {
        self.ambience.for_monitor(monitor)
    }
    pub fn signals(&self) -> impl Iterator<Item = GeometrySignal> + '_ {
        self.signals.iter().filter(|s| s.confirmed).copied()
    }

    /// Return false when attention must be cancelled and fresh observations re-established.
    pub fn update(&mut self, desktop: &DesktopSnapshot, dt: f32, active: bool) -> bool {
        let dt = if dt.is_finite() { dt.max(0.0) } else { 0.0 };
        self.fixture_clock += f64::from(dt);
        for signal in &mut self.signals {
            signal.remaining -= dt;
        }
        self.signals.retain(|s| s.remaining > 0.0);
        let at = desktop.window_sample.map_or(self.fixture_clock, |sample| {
            sample.monotonic_millis as f64 / 1_000.0
        });
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        for monitor in &desktop.monitors {
            monitor.id.hash(&mut hash);
            monitor.display_key.hash(&mut hash);
            for value in [
                monitor.bounds.x,
                monitor.bounds.y,
                monitor.bounds.width,
                monitor.bounds.height,
                monitor.usable_bounds.x,
                monitor.usable_bounds.y,
                monitor.usable_bounds.width,
                monitor.usable_bounds.height,
                monitor.scale_factor,
            ] {
                value.to_bits().hash(&mut hash);
            }
        }
        let monitor_hash = hash.finish();
        let reliable = desktop.window_sample.is_none_or(|sample| sample.reliable)
            && !desktop.monitors.is_empty()
            && desktop.monitors.iter().all(|m| {
                valid_rect(m.bounds)
                    && valid_rect(m.usable_bounds)
                    && m.scale_factor.is_finite()
                    && m.scale_factor > 0.0
            })
            && desktop
                .windows
                .iter()
                .all(|window| valid_rect(window.bounds));
        if !active || !reliable {
            self.ambience.clear();
            self.signals.clear();
            self.initialized = false;
            self.sample_at = Some(at);
            return false;
        }
        // Cached scans are not fresh evidence of movement or of a disappeared surface.
        if self.initialized && self.sample_at == Some(at) && monitor_hash == self.monitor_hash {
            return true;
        }
        // A desktop with more windows than the cap is still a desktop worth living on, so the
        // cap truncates rather than suspending every behavior. The frontmost windows are the ones
        // kept, chosen exactly as the topology chooses them, so the two always agree on which
        // desktop the creatures are looking at. The order is stable, so a window only leaves the
        // observed set when something genuinely moves in front of it — and the wholesale-change
        // guard below still keeps a large reshuffle from reading as dozens of closures.
        let mut visible: Vec<_> = desktop
            .windows
            .iter()
            .filter(|w| w.visible && !w.minimized)
            .collect();
        visible.sort_by_key(|w| (w.z_order, w.key));
        visible.truncate(MAX_TOPOLOGY_WINDOWS);
        self.current.clear();
        self.current.extend(visible.into_iter().map(|w| {
            WindowShape {
                key: w.key,
                bounds: w.bounds,
                moved_at: self
                    .previous
                    .iter()
                    .find(|old| old.key == w.key)
                    .and_then(|old| old.moved_at),
            }
        }));
        let elapsed = self.sample_at.map_or(0.0, |previous| at - previous);
        let common = self
            .current
            .iter()
            .filter(|w| self.previous.iter().any(|old| old.key == w.key))
            .count();
        let wholesale_change = self.previous.len().max(self.current.len()) >= 4
            && common * 2 < self.previous.len().max(self.current.len());
        let baseline = !self.initialized
            || self.monitor_hash != monitor_hash
            || !(0.0..=3.0).contains(&elapsed)
            || elapsed == 0.0
            || wholesale_change;
        if baseline {
            self.signals.clear();
            self.ambience.clear();
            for window in &mut self.current {
                window.moved_at = None;
            }
        } else {
            let returned: [Option<WindowKey>; MAX_SIGNALS] = std::array::from_fn(|index| {
                self.signals
                    .get(index)
                    .filter(|s| {
                        s.kind == GeometryChange::Disappeared
                            && !s.confirmed
                            && self.current.iter().any(|w| w.key == s.window)
                    })
                    .map(|s| s.window)
            });
            // Require another fresh scan before reacting to a lost surface. A one-scan omission
            // cannot start a search or a social cascade.
            self.signals.retain_mut(|signal| {
                if signal.kind != GeometryChange::Disappeared {
                    return true;
                }
                if self.current.iter().any(|w| w.key == signal.window) {
                    return false;
                }
                signal.confirmed = true;
                true
            });
            for index in 0..self.current.len() {
                let current = self.current[index];
                let old = self.previous.iter().find(|w| w.key == current.key).copied();
                match old {
                    None if returned.contains(&Some(current.key)) => {}
                    None => match renamed_from(&self.previous, &self.current, current) {
                        // A native identifier changed while the frame stayed put: the same surface.
                        Some(renamed) => self.current[index].moved_at = renamed.moved_at,
                        None => self.push(current, GeometryChange::Appeared, 0.5),
                    },
                    Some(old) => {
                        let distance = translation(old.bounds, current.bounds);
                        let growth = (current.bounds.width - old.bounds.width).max(0.0)
                            + (current.bounds.height - old.bounds.height).max(0.0);
                        let speed = distance / elapsed as f32;
                        if growth >= 48.0 && growth / elapsed as f32 >= 80.0 {
                            self.push(
                                current,
                                GeometryChange::Expanded,
                                (growth / 180.0).clamp(0.4, 1.0),
                            );
                        } else if distance >= 8.0 && speed >= 35.0 {
                            self.current[index].moved_at = Some(at);
                            self.push(
                                current,
                                GeometryChange::Moved,
                                (speed / 600.0).min(distance / 150.0).clamp(0.2, 1.0),
                            );
                        }
                    }
                }
            }
            for index in 0..self.previous.len() {
                let previous = self.previous[index];
                if !self.current.iter().any(|w| w.key == previous.key)
                    && renamed_from(&self.current, &self.previous, previous).is_none()
                {
                    self.push(previous, GeometryChange::Disappeared, 0.65);
                }
            }
            // Several different windows moving in one neighborhood become one curiosity cue.
            // A long drag of one window never counts as desktop rearrangement.
            for index in 0..self.current.len() {
                let anchor = self.current[index];
                let center = Point {
                    x: anchor.bounds.x + anchor.bounds.width * 0.5,
                    y: anchor.bounds.y + anchor.bounds.height * 0.5,
                };
                let recent = self
                    .current
                    .iter()
                    .filter(|w| w.moved_at.is_some_and(|moved| at - moved <= 2.0))
                    .filter(|w| {
                        center.distance(Point {
                            x: w.bounds.x + w.bounds.width * 0.5,
                            y: w.bounds.y + w.bounds.height * 0.5,
                        }) <= 360.0
                    })
                    .count();
                if recent >= 3 && anchor.moved_at.is_some_and(|moved| at - moved <= 2.0) {
                    self.push(anchor, GeometryChange::Rearranged, 0.8);
                    break;
                }
            }
            self.ambience.update(desktop, elapsed as f32);
        }
        std::mem::swap(&mut self.previous, &mut self.current);
        self.monitor_hash = monitor_hash;
        self.sample_at = Some(at);
        self.initialized = true;
        !baseline
    }

    fn push(&mut self, window: WindowShape, kind: GeometryChange, strength: f32) {
        if let Some(existing) = self.signals.iter_mut().find(|s| s.window == window.key) {
            existing.bounds = window.bounds;
            // Follow the moving target without extending the signal or minting a new origin.
            if kind == GeometryChange::Disappeared && existing.kind != kind {
                existing.kind = kind;
                existing.confirmed = false;
                self.next_origin = self.next_origin.wrapping_add(1);
                existing.origin = self.next_origin;
                existing.remaining = SIGNAL_LIFETIME;
            } else if matches!(kind, GeometryChange::Expanded | GeometryChange::Rearranged) {
                existing.kind = kind;
            }
            existing.strength = existing.strength.max(strength);
            return;
        }
        if self.signals.len() == MAX_SIGNALS {
            return;
        }
        self.next_origin = self.next_origin.wrapping_add(1);
        self.signals.push(GeometrySignal {
            origin: self.next_origin,
            window: window.key,
            bounds: window.bounds,
            kind,
            strength,
            remaining: SIGNAL_LIFETIME,
            confirmed: kind != GeometryChange::Disappeared,
        });
    }
}

/// The unmatched shape on the other side of a scan with an identical frame, when the pairing is
/// one-to-one. Ambiguous identical frames remain ordinary appearance/disappearance evidence.
fn renamed_from(
    others: &[WindowShape],
    same_side: &[WindowShape],
    shape: WindowShape,
) -> Option<WindowShape> {
    fn unique(
        pool: &[WindowShape],
        matched: &[WindowShape],
        bounds: DesktopRect,
    ) -> Option<WindowShape> {
        let mut found = pool
            .iter()
            .filter(|w| w.bounds == bounds && !matched.iter().any(|m| m.key == w.key));
        let first = found.next().copied();
        found.next().is_none().then_some(first).flatten()
    }
    let other = unique(others, same_side, shape.bounds)?;
    (unique(same_side, others, other.bounds)?.key == shape.key).then_some(other)
}

/// Distance both opposing edges travelled together. Dragging one edge is a resize, not a move;
/// a snap that shifts and shrinks a window still moves by its smaller edge displacement.
pub(crate) fn translation(old: DesktopRect, current: DesktopRect) -> f32 {
    let axis = |near: f32, far: f32| {
        if near * far > 0.0 {
            near.abs().min(far.abs()) * near.signum()
        } else {
            0.0
        }
    };
    Point {
        x: axis(current.x - old.x, current.right() - old.right()),
        y: axis(current.y - old.y, current.bottom() - old.bottom()),
    }
    .distance(Point::default())
}

pub(crate) fn valid_rect(rect: DesktopRect) -> bool {
    [
        rect.x,
        rect.y,
        rect.width,
        rect.height,
        rect.right(),
        rect.bottom(),
    ]
    .iter()
    .all(|n| n.is_finite())
        && rect.width > 0.0
        && rect.height > 0.0
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::{DesktopWindow, MonitorInfo};

    pub(crate) fn snapshot(count: usize, millis: u64) -> DesktopSnapshot {
        DesktopSnapshot {
            monitors: vec![MonitorInfo {
                id: 1,
                display_key: Default::default(),
                bounds: DesktopRect {
                    x: 0.0,
                    y: 0.0,
                    width: 1600.0,
                    height: 1000.0,
                },
                usable_bounds: DesktopRect {
                    x: 0.0,
                    y: 0.0,
                    width: 1600.0,
                    height: 960.0,
                },
                scale_factor: 1.0,
                primary: true,
            }],
            windows: (0..count)
                .map(|index| DesktopWindow {
                    key: index as u64 + 1,
                    bounds: DesktopRect {
                        x: 100.0,
                        y: 300.0,
                        width: 400.0,
                        height: 300.0,
                    },
                    z_order: index as u32,
                    visible: true,
                    minimized: false,
                    application: None,
                    application_name: None,
                })
                .collect(),
            window_sample: Some(WindowSample {
                monotonic_millis: millis,
                reliable: true,
            }),
            ..Default::default()
        }
    }

    #[test]
    fn fresh_scan_time_controls_speed_and_cached_ticks_do_not_restart_signals() {
        let mut observer = GeometryObserver::default();
        observer.update(&snapshot(1, 0), 0.05, true);
        for _ in 0..10 {
            observer.update(&snapshot(1, 0), 0.05, true);
        }
        let mut moved = snapshot(1, 1_000);
        moved.windows[0].bounds.x += 60.0;
        observer.update(&moved, 0.05, true);
        let signal = observer.signals().next().unwrap();
        assert_eq!(signal.kind, GeometryChange::Moved);
        assert_eq!(signal.strength, 0.2); // 60 points/sec, not 1,200 points/sec.
        moved.window_sample.as_mut().unwrap().monotonic_millis = 1_250;
        moved.windows[0].bounds.x += 150.0;
        observer.update(&moved, 0.25, true);
        assert_eq!(observer.signals().next().unwrap().origin, signal.origin);
        for _ in 0..80 {
            observer.update(&moved, 0.05, true);
        }
        assert_eq!(observer.signals().count(), 0);
    }

    #[test]
    fn novelty_growth_and_disappearance_have_distinct_causes_and_confirmation() {
        let mut observer = GeometryObserver::default();
        observer.update(&snapshot(0, 0), 0.05, true);
        observer.update(&snapshot(1, 250), 0.25, true);
        assert_eq!(
            observer.signals().next().unwrap().kind,
            GeometryChange::Appeared
        );
        let mut grown = snapshot(1, 500);
        grown.windows[0].bounds.width += 100.0;
        observer.update(&grown, 0.25, true);
        assert_eq!(
            observer.signals().next().unwrap().kind,
            GeometryChange::Expanded
        );
        observer.update(&snapshot(0, 750), 0.25, true);
        assert_eq!(observer.signals().count(), 0);
        observer.update(&snapshot(0, 750), 0.05, true);
        assert_eq!(observer.signals().count(), 0);
        observer.update(&snapshot(0, 1_000), 0.2, true);
        assert_eq!(
            observer.signals().next().unwrap().kind,
            GeometryChange::Disappeared
        );
    }

    #[test]
    fn unavailable_paused_stale_and_reconfigured_desktops_rebaseline_without_replay() {
        for reason in 0..6 {
            let mut observer = GeometryObserver::default();
            observer.update(&snapshot(1, 0), 0.05, true);
            let mut interrupted = snapshot(1, 250);
            match reason {
                0 => interrupted.window_sample.as_mut().unwrap().reliable = false,
                1 => interrupted.monitors.clear(),
                2 => interrupted.monitors[0].id = 2,
                3 => interrupted.window_sample.as_mut().unwrap().monotonic_millis = 30_000,
                4 => interrupted.windows[0].bounds.width = f32::NAN,
                _ => {}
            }
            assert!(!observer.update(&interrupted, 0.25, reason != 5));
            observer.update(&snapshot(1, 500), 0.25, true);
            assert_eq!(observer.signals().count(), 0);
        }
    }

    #[test]
    fn bursts_are_bounded_and_truncation_or_workspace_switches_do_not_look_like_closures() {
        let mut observer = GeometryObserver::default();
        observer.update(&snapshot(32, 0), 0.05, true);
        let mut moved = snapshot(32, 250);
        for window in &mut moved.windows {
            window.bounds.x += 120.0;
        }
        observer.update(&moved, 0.25, true);
        assert_eq!(observer.signals().count(), MAX_SIGNALS);
        // A desktop fuller than the cap is still watched: the frontmost windows are kept, and the
        // ones past the end simply never enter the picture. Opening and closing windows out there
        // is not something a creature can see, so it is neither an arrival nor a closure.
        // A desktop fuller than the cap is still watched: the frontmost windows are kept and the
        // ones past the end never enter the picture. Windows opening and closing out there are
        // not something a creature could see, so the cap must never manufacture a closure.
        observer.update(&snapshot(64, 500), 4.0, true);
        let closures = |observer: &GeometryObserver| {
            observer
                .signals()
                .filter(|signal| signal.kind == GeometryChange::Disappeared)
                .count()
        };
        assert!(observer.update(&snapshot(80, 750), 0.25, true));
        assert_eq!(closures(&observer), 0);
        assert!(observer.update(&snapshot(64, 1_000), 0.25, true));
        assert_eq!(closures(&observer), 0);
        let mut workspace = snapshot(32, 1_500);
        for window in &mut workspace.windows {
            window.key += 100;
        }
        assert!(!observer.update(&workspace, 0.25, true));
        assert_eq!(observer.signals().count(), 0);
    }

    #[test]
    fn a_one_scan_omission_is_neither_a_disappearance_nor_a_new_arrival() {
        let mut observer = GeometryObserver::default();
        observer.update(&snapshot(1, 0), 0.05, true);
        observer.update(&snapshot(0, 250), 0.25, true);
        observer.update(&snapshot(1, 500), 0.25, true);
        assert_eq!(observer.signals().count(), 0);
        // Observer storage is reserved once; no unbounded geometry or event allocation.
        let bytes = std::mem::size_of::<GeometryObserver>()
            + observer.ambience.reserved_bytes()
            + (observer.current.capacity() + observer.previous.capacity())
                * std::mem::size_of::<WindowShape>()
            + observer.signals.capacity() * std::mem::size_of::<GeometrySignal>();
        assert!(bytes <= 8 * 1024, "geometry observation uses {bytes} bytes");
    }
    /// Minimising a window and hiding one look the same from here, and the same as closing it:
    /// the surface is simply not available any more. Nothing claims to know which of the three
    /// happened, and bringing it back is a surface that is there again rather than one that
    /// silently never left.
    #[test]
    fn a_minimised_or_hidden_window_is_a_lost_surface_and_its_return_is_a_new_arrival() {
        for hidden in 0..2 {
            let mut observer = GeometryObserver::default();
            observer.update(&snapshot(1, 0), 0.05, true);
            let mut gone = snapshot(1, 250);
            if hidden == 0 {
                gone.windows[0].minimized = true;
            } else {
                gone.windows[0].visible = false;
            }
            observer.update(&gone, 0.25, true);
            assert_eq!(
                observer.signals().count(),
                0,
                "one scan is not enough to call a surface lost"
            );
            gone.window_sample.as_mut().unwrap().monotonic_millis = 500;
            observer.update(&gone, 0.25, true);
            assert_eq!(
                observer.signals().next().unwrap().kind,
                GeometryChange::Disappeared
            );
            observer.update(&snapshot(1, 750), 0.25, true);
            let back = observer.signals().next().unwrap();
            assert_eq!(back.kind, GeometryChange::Appeared);
        }
    }

    /// Displays can sit left of and above the origin. Every threshold here is a distance, so the
    /// same window doing the same thing has to read the same on either side of zero.
    #[test]
    fn a_window_left_of_the_origin_reports_exactly_the_same_movement() {
        let measure = |negative: bool| {
            let shift = |desktop: &mut DesktopSnapshot| {
                if !negative {
                    return;
                }
                for monitor in &mut desktop.monitors {
                    monitor.bounds.x -= 3_000.0;
                    monitor.bounds.y -= 1_200.0;
                    monitor.usable_bounds.x -= 3_000.0;
                    monitor.usable_bounds.y -= 1_200.0;
                }
                for window in &mut desktop.windows {
                    window.bounds.x -= 3_000.0;
                    window.bounds.y -= 1_200.0;
                }
            };
            let mut observer = GeometryObserver::default();
            let mut resting = snapshot(1, 0);
            let mut moved = snapshot(1, 250);
            moved.windows[0].bounds.x += 60.0;
            shift(&mut resting);
            shift(&mut moved);
            observer.update(&resting, 0.05, true);
            observer.update(&moved, 0.25, true);
            let signal = observer.signals().next().unwrap();
            (signal.kind, signal.strength)
        };
        assert_eq!(measure(true), measure(false));
    }

    #[test]
    fn rearrangement_requires_several_distinct_local_windows_and_expires() {
        for moved_count in [1, 3] {
            let mut observer = GeometryObserver::default();
            let mut desktop = snapshot(3, 0);
            observer.update(&desktop, 0.05, true);
            for step in 1..=3 {
                desktop.window_sample.as_mut().unwrap().monotonic_millis = step * 250;
                desktop.windows[(step as usize - 1) % moved_count].bounds.x += 30.0;
                observer.update(&desktop, 0.25, true);
            }
            assert_eq!(
                observer
                    .signals()
                    .any(|s| s.kind == GeometryChange::Rearranged),
                moved_count == 3
            );
            for step in 4..=22 {
                desktop.window_sample.as_mut().unwrap().monotonic_millis = step * 250;
                observer.update(&desktop, 0.25, true);
            }
            assert_eq!(observer.signals().count(), 0);
        }
        let mut observer = GeometryObserver::default();
        let mut desktop = snapshot(3, 0);
        for (index, window) in desktop.windows.iter_mut().enumerate() {
            window.bounds.x += index as f32 * 500.0;
        }
        observer.update(&desktop, 0.05, true);
        for window in &mut desktop.windows {
            window.bounds.x += 30.0;
        }
        desktop.window_sample.as_mut().unwrap().monotonic_millis = 250;
        observer.update(&desktop, 0.25, true);
        assert!(
            !observer
                .signals()
                .any(|s| s.kind == GeometryChange::Rearranged)
        );
    }

    #[test]
    fn sparse_desktop_preference_uses_fresh_reliable_scans_and_resets_on_uncertainty() {
        let mut observer = GeometryObserver::default();
        let mut desktop = snapshot(0, 0);
        observer.update(&desktop, 0.05, true);
        assert_eq!(observer.ambience(1), crate::DesktopAmbience::default());
        desktop.window_sample.as_mut().unwrap().monotonic_millis = 1_000;
        observer.update(&desktop, 0.05, true);
        let preference = observer.ambience(1);
        assert!(preference.roaming > 0.0 && preference.roaming < 0.3);
        for _ in 0..20 {
            observer.update(&desktop, 0.05, true);
        }
        assert_eq!(observer.ambience(1), preference);
        desktop.window_sample.as_mut().unwrap().reliable = false;
        observer.update(&desktop, 0.05, true);
        assert_eq!(observer.ambience(1), crate::DesktopAmbience::default());
    }

    #[test]
    fn dragging_one_edge_is_a_resize_while_a_snap_still_moves_the_window() {
        let old = DesktopRect {
            x: 100.0,
            y: 300.0,
            width: 400.0,
            height: 300.0,
        };
        let shrink_left = DesktopRect {
            x: 180.0,
            width: 320.0,
            ..old
        };
        let grow_top = DesktopRect {
            y: 200.0,
            height: 400.0,
            ..old
        };
        let snap = DesktopRect {
            x: 0.0,
            y: 0.0,
            width: 300.0,
            height: 900.0,
        };
        assert_eq!(translation(old, shrink_left), 0.0);
        assert_eq!(translation(old, grow_top), 0.0);
        assert_eq!(translation(old, DesktopRect { x: 160.0, ..old }), 60.0);
        assert_eq!(translation(old, snap), 100.0);

        let mut observer = GeometryObserver::default();
        observer.update(&snapshot(1, 0), 0.05, true);
        let mut resized = snapshot(1, 250);
        resized.windows[0].bounds = shrink_left;
        observer.update(&resized, 0.25, true);
        assert_eq!(
            observer.signals().count(),
            0,
            "a left-edge shrink is not a move"
        );
    }

    #[test]
    fn an_identifier_change_with_an_identical_frame_is_the_same_surface() {
        let mut observer = GeometryObserver::default();
        observer.update(&snapshot(2, 0), 0.05, true);
        let mut renamed = snapshot(2, 250);
        renamed.windows[1].key = 900;
        observer.update(&renamed, 0.25, true);
        renamed.window_sample.as_mut().unwrap().monotonic_millis = 500;
        observer.update(&renamed, 0.25, true);
        assert_eq!(observer.signals().count(), 0);
        // Two identical unmatched frames are ambiguous, so ordinary evidence applies.
        let mut observer = GeometryObserver::default();
        let mut desktop = snapshot(2, 0);
        desktop.windows[0].bounds.x = 900.0;
        observer.update(&desktop, 0.05, true);
        desktop.window_sample.as_mut().unwrap().monotonic_millis = 250;
        desktop.windows[0].key = 901;
        desktop.windows[0].bounds.x = 100.0;
        desktop.windows[1].key = 902;
        observer.update(&desktop, 0.25, true);
        assert!(
            observer
                .signals()
                .any(|s| s.kind == GeometryChange::Appeared)
        );
    }

    #[test]
    fn disappearing_after_a_long_ride_gets_a_new_confirmed_origin() {
        let mut observer = GeometryObserver::default();
        observer.update(&snapshot(1, 0), 0.05, true);
        let mut moved = snapshot(1, 250);
        moved.windows[0].bounds.x += 120.0;
        observer.update(&moved, 0.25, true);
        let origin = observer.signals().next().unwrap().origin;
        moved.window_sample.as_mut().unwrap().monotonic_millis = 3_000;
        observer.update(&moved, 2.75, true);
        observer.update(&snapshot(0, 3_250), 0.25, true);
        assert_eq!(observer.signals().count(), 0);
        observer.update(&snapshot(0, 4_250), 1.0, true);
        let loss = observer.signals().next().unwrap();
        assert_eq!(loss.kind, GeometryChange::Disappeared);
        assert_ne!(loss.origin, origin);
    }
}
