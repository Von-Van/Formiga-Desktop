//! Coarse, transient exploration preferences derived from exposed desktop geometry.
use crate::{DesktopSnapshot, MAX_TOPOLOGY_WINDOWS, MonitorId, Point};
use std::hash::{Hash, Hasher};

const MAX_AMBIENCE_DISPLAYS: usize = 8;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DesktopAmbience {
    pub roaming: f32,
    pub climbing: f32,
}

#[derive(Clone, Copy)]
struct DisplayAmbience {
    monitor: MonitorId,
    current: DesktopAmbience,
    desired: DesktopAmbience,
}

pub(crate) struct AmbienceTracker {
    displays: Vec<DisplayAmbience>,
    signature: Option<u64>,
}

impl Default for AmbienceTracker {
    fn default() -> Self {
        Self {
            displays: Vec::with_capacity(MAX_AMBIENCE_DISPLAYS),
            signature: None,
        }
    }
}

impl AmbienceTracker {
    pub fn clear(&mut self) {
        self.displays.clear();
        self.signature = None;
    }

    pub fn for_monitor(&self, monitor: MonitorId) -> DesktopAmbience {
        self.displays
            .iter()
            .find(|d| d.monitor == monitor)
            .map_or(DesktopAmbience::default(), |d| d.current)
    }

    pub fn update(&mut self, desktop: &DesktopSnapshot, elapsed: f32) {
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        for window in desktop.windows.iter().take(MAX_TOPOLOGY_WINDOWS) {
            window.key.hash(&mut hash);
            window.z_order.hash(&mut hash);
            window.visible.hash(&mut hash);
            window.minimized.hash(&mut hash);
            for value in [
                window.bounds.x,
                window.bounds.y,
                window.bounds.width,
                window.bounds.height,
            ] {
                value.to_bits().hash(&mut hash);
            }
        }
        let signature = hash.finish();
        if self.signature != Some(signature) {
            self.displays
                .retain(|d| desktop.monitors.iter().any(|m| m.id == d.monitor));
            for monitor in desktop.monitors.iter().take(MAX_AMBIENCE_DISPLAYS) {
                let mut exposed_tops = 0_usize;
                let mut overlapping_tops = 0_usize;
                for window in desktop
                    .windows
                    .iter()
                    .take(MAX_TOPOLOGY_WINDOWS)
                    .filter(|w| w.visible && !w.minimized)
                {
                    let Some(visible_bounds) = window.bounds.intersection(monitor.usable_bounds)
                    else {
                        continue;
                    };
                    if window.bounds.y < monitor.usable_bounds.y {
                        continue;
                    }
                    let exposed = [0.05, 0.5, 0.95].into_iter().any(|fraction| {
                        let top = Point {
                            x: visible_bounds.x + visible_bounds.width * fraction,
                            y: window.bounds.y,
                        };
                        !desktop
                            .windows
                            .iter()
                            .take(MAX_TOPOLOGY_WINDOWS)
                            .any(|other| {
                                other.visible
                                    && !other.minimized
                                    && other.z_order < window.z_order
                                    && other.bounds.contains(top)
                            })
                    });
                    if exposed {
                        exposed_tops += 1;
                        if desktop
                            .windows
                            .iter()
                            .take(MAX_TOPOLOGY_WINDOWS)
                            .any(|other| {
                                other.key != window.key
                                    && other.visible
                                    && !other.minimized
                                    && other.bounds.intersection(window.bounds).is_some()
                            })
                        {
                            overlapping_tops += 1;
                        }
                    }
                }
                // Fifteen geometry samples estimate free space; no desktop pixels are read.
                let open_cells = (0..15)
                    .filter(|index| {
                        let point = Point {
                            x: monitor.usable_bounds.x
                                + monitor.usable_bounds.width * ((index % 5) as f32 + 0.5) / 5.0,
                            y: monitor.usable_bounds.y
                                + monitor.usable_bounds.height * ((index / 5) as f32 + 0.5) / 3.0,
                        };
                        !desktop
                            .windows
                            .iter()
                            .take(MAX_TOPOLOGY_WINDOWS)
                            .any(|w| w.visible && !w.minimized && w.bounds.contains(point))
                    })
                    .count();
                let desired = DesktopAmbience {
                    roaming: open_cells as f32 / 15.0
                        * (1.0 - exposed_tops.saturating_sub(1) as f32 / 3.0).clamp(0.0, 1.0),
                    climbing: (exposed_tops.saturating_sub(1) as f32 / 3.0).clamp(0.0, 1.0)
                        * (overlapping_tops as f32 / 2.0).min(1.0),
                };
                if let Some(display) = self.displays.iter_mut().find(|d| d.monitor == monitor.id) {
                    display.desired = desired;
                } else if self.displays.len() < MAX_AMBIENCE_DISPLAYS {
                    self.displays.push(DisplayAmbience {
                        monitor: monitor.id,
                        current: DesktopAmbience::default(),
                        desired,
                    });
                }
            }
            self.signature = Some(signature);
        }
        let blend = (elapsed / 5.0).clamp(0.0, 1.0);
        for display in &mut self.displays {
            display.current.roaming += (display.desired.roaming - display.current.roaming) * blend;
            display.current.climbing +=
                (display.desired.climbing - display.current.climbing) * blend;
        }
    }

    #[cfg(test)]
    pub fn reserved_bytes(&self) -> usize {
        self.displays.capacity() * std::mem::size_of::<DisplayAmbience>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attention::tests::snapshot;

    #[test]
    fn exposed_overlapping_tiers_encourage_climbing_but_a_covered_stack_does_not() {
        let mut desktop = snapshot(4, 0);
        let mut tracker = AmbienceTracker::default();
        tracker.update(&desktop, 1.0);
        assert_eq!(tracker.for_monitor(1).climbing, 0.0);
        // Lower windows project out from behind the frontmost one, exposing four actual tiers.
        for (index, window) in desktop.windows.iter_mut().enumerate() {
            window.bounds.x += index as f32 * 100.0;
            window.bounds.y += index as f32 * 50.0;
        }
        tracker.update(&desktop, 1.0);
        let busy = tracker.for_monitor(1);
        assert!(busy.climbing > 0.0 && busy.climbing < 0.3);
        for _ in 0..20 {
            tracker.update(&desktop, 1.0);
        }
        assert!(tracker.for_monitor(1).climbing > 0.95);
        desktop.windows.clear();
        tracker.update(&desktop, 1.0);
        let opening = tracker.for_monitor(1);
        assert!(opening.climbing > 0.7 && opening.climbing < 0.85);
        assert!(opening.roaming > busy.roaming);
        for _ in 0..30 {
            tracker.update(&desktop, 1.0);
        }
        let empty = tracker.for_monitor(1);
        assert!(empty.roaming > 0.99 && empty.climbing < 0.01);
    }

    #[test]
    fn preferences_are_local_to_a_display_and_ignore_minimized_windows() {
        let mut desktop = snapshot(4, 0);
        let mut second = desktop.monitors[0].clone();
        second.id = 2;
        second.bounds.x += 1_600.0;
        second.usable_bounds.x += 1_600.0;
        desktop.monitors.push(second);
        for (index, window) in desktop.windows.iter_mut().enumerate() {
            window.bounds.x += index as f32 * 100.0;
            window.bounds.y += index as f32 * 50.0;
        }
        let mut tracker = AmbienceTracker::default();
        tracker.update(&desktop, 5.0);
        assert!(tracker.for_monitor(1).climbing > 0.9);
        assert_eq!(
            tracker.for_monitor(2),
            DesktopAmbience {
                roaming: 1.0,
                climbing: 0.0
            }
        );
        assert_eq!(tracker.for_monitor(99), DesktopAmbience::default());
        for window in &mut desktop.windows {
            window.minimized = true;
        }
        tracker.update(&desktop, 5.0);
        assert_eq!(tracker.for_monitor(1), tracker.for_monitor(2));
    }
}
