//! Coarse, transient exploration preferences derived from exposed desktop geometry.
use crate::{DesktopRect, DesktopSnapshot, MAX_TOPOLOGY_WINDOWS, MonitorId, Point, WindowKey};

const MAX_AMBIENCE_DISPLAYS: usize = 8;

/// One window as the sampling below sees it. The desktop's own window record carries an
/// application name and other things none of this cares about; the geometry sampling walks the
/// same list once per sample point, so it walks this compact copy instead.
#[derive(Clone, Copy, Default)]
struct Frame {
    key: WindowKey,
    z_order: u32,
    bounds: DesktopRect,
}

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
    /// The windows the last sampling looked at, kept so that re-reading a display costs no
    /// allocation. It is capped at `MAX_TOPOLOGY_WINDOWS` like every other geometry list here,
    /// and grows to what a desktop actually shows, so a desktop with no windows never asks for
    /// any of it.
    frames: Vec<Frame>,
}

impl Default for AmbienceTracker {
    fn default() -> Self {
        Self {
            displays: Vec::with_capacity(MAX_AMBIENCE_DISPLAYS),
            signature: None,
            frames: Vec::new(),
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
        let signature = geometry_signature(desktop);
        if self.signature != Some(signature) {
            self.resample(desktop);
            self.signature = Some(signature);
        }
        let blend = (elapsed / 5.0).clamp(0.0, 1.0);
        for display in &mut self.displays {
            display.current.roaming += (display.desired.roaming - display.current.roaming) * blend;
            display.current.climbing +=
                (display.desired.climbing - display.current.climbing) * blend;
        }
    }

    /// Read what each display now offers. Only called when the geometry actually changed.
    fn resample(&mut self, desktop: &DesktopSnapshot) {
        self.displays
            .retain(|d| desktop.monitors.iter().any(|m| m.id == d.monitor));

        // Every sample point below asks the same question of the same windows, so the ones that
        // are really on screen are gathered once rather than re-filtered out of the snapshot on
        // each of the hundreds of tests that follow.
        //
        // Front to back: a window's top edge can only be covered by one that sits in front of it,
        // which after the sort is one of the frames already passed, so the search for a cover
        // stops at the window itself instead of running to the back of the desktop every time.
        // Windows that share a z-order cover nothing, which the test below still says.
        self.frames.clear();
        self.frames.extend(
            desktop
                .windows
                .iter()
                .take(MAX_TOPOLOGY_WINDOWS)
                .filter(|window| window.visible && !window.minimized)
                .map(|window| Frame {
                    key: window.key,
                    z_order: window.z_order,
                    bounds: window.bounds,
                }),
        );
        self.frames.sort_unstable_by_key(|frame| frame.z_order);
        let frames = &self.frames;

        for monitor in desktop.monitors.iter().take(MAX_AMBIENCE_DISPLAYS) {
            let mut exposed_tops = 0_usize;
            let mut overlapping_tops = 0_usize;
            for (index, window) in frames.iter().enumerate() {
                let Some(visible_bounds) = window.bounds.intersection(monitor.usable_bounds) else {
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
                    !frames[..index]
                        .iter()
                        .any(|other| other.z_order < window.z_order && other.bounds.contains(top))
                });
                if exposed {
                    exposed_tops += 1;
                    if frames.iter().any(|other| {
                        other.key != window.key
                            && other.bounds.intersection(window.bounds).is_some()
                    }) {
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
                    !frames.iter().any(|w| w.bounds.contains(point))
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
    }

    #[cfg(test)]
    pub fn reserved_bytes(&self) -> usize {
        self.displays.capacity() * std::mem::size_of::<DisplayAmbience>()
            + self.frames.capacity() * std::mem::size_of::<Frame>()
    }
}

/// A value that changes exactly when the visible window geometry does. It is only ever compared
/// with the previous one, so it uses the same cheap FNV-1a mixing as the topology's own hash
/// rather than a general-purpose hasher.
fn geometry_signature(desktop: &DesktopSnapshot) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for window in desktop.windows.iter().take(MAX_TOPOLOGY_WINDOWS) {
        for value in [
            window.key,
            u64::from(window.z_order),
            u64::from(window.visible) | (u64::from(window.minimized) << 1),
            u64::from(window.bounds.x.to_bits()),
            u64::from(window.bounds.y.to_bits()),
            u64::from(window.bounds.width.to_bits()),
            u64::from(window.bounds.height.to_bits()),
        ] {
            hash ^= value;
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    hash
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
