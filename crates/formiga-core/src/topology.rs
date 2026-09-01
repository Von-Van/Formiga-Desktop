use crate::{DesktopRect, DesktopSnapshot, MonitorId, Point, WindowKey};
use std::collections::BTreeMap;

pub const MAX_TOPOLOGY_WINDOWS: usize = 64;
pub const MAX_TOPOLOGY_LANDMARKS: usize = 96;
const ISLAND_CLEARANCE: f32 = 24.0;
const INVITATION_DISTANCE: f32 = 24.0;
const INVITATION_SPEED: f32 = 25.0;
const INVITATION_DWELL_SECS: f32 = 1.5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TopologyLandmarkKind {
    WindowIsland,
    ExposedLeftCorner,
    ExposedRightCorner,
    SlowMovingPlatform,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TopologyWindow {
    pub key: WindowKey,
    pub bounds: DesktopRect,
    pub monitor_id: MonitorId,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TopologyLandmark {
    pub kind: TopologyLandmarkKind,
    pub window_key: WindowKey,
    pub point: Point,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CursorInvitation {
    pub window_key: WindowKey,
    pub point: Point,
    pub monitor_id: MonitorId,
}

#[derive(Clone, Copy, Debug)]
struct InvitationDwell {
    window_key: WindowKey,
    point: Point,
    monitor_id: MonitorId,
    elapsed: f32,
}

/// A bounded, runtime-only projection of privacy-safe desktop geometry. It never retains window
/// titles, application identity, cursor paths, or pixels and is rebuilt only when the visible
/// window geometry hash changes.
#[derive(Clone, Debug, Default)]
pub struct DesktopTopology {
    geometry_hash: u64,
    initialized: bool,
    windows: Vec<TopologyWindow>,
    landmarks: Vec<TopologyLandmark>,
    invitation_dwell: Option<InvitationDwell>,
    invitation: Option<CursorInvitation>,
    rebuild_count: u64,
}

impl DesktopTopology {
    pub fn rebuild_if_changed(
        &mut self,
        desktop: &DesktopSnapshot,
        previous: &BTreeMap<WindowKey, DesktopRect>,
    ) -> bool {
        let visible = bounded_visible_windows(desktop);
        let geometry_hash = geometry_hash(&visible);
        if self.initialized && geometry_hash == self.geometry_hash {
            return false;
        }

        self.initialized = true;
        self.geometry_hash = geometry_hash;
        self.rebuild_count = self.rebuild_count.saturating_add(1);
        self.invitation_dwell = None;
        self.invitation = None;
        self.windows = visible
            .iter()
            .filter_map(|window| {
                let point = Point {
                    x: window.bounds.x + window.bounds.width * 0.5,
                    y: window.bounds.y,
                };
                let monitor_id = desktop
                    .monitors
                    .iter()
                    .find(|monitor| monitor.bounds.contains(point))?
                    .id;
                Some(TopologyWindow {
                    key: window.key,
                    bounds: window.bounds,
                    monitor_id,
                })
            })
            .collect();

        let mut landmarks = Vec::with_capacity(MAX_TOPOLOGY_LANDMARKS);
        for window in &self.windows {
            let island = self.windows.iter().all(|other| {
                other.key == window.key
                    || !expanded(window.bounds, ISLAND_CLEARANCE).overlaps(other.bounds)
            });
            if island {
                push_landmark(
                    &mut landmarks,
                    TopologyLandmark {
                        kind: TopologyLandmarkKind::WindowIsland,
                        window_key: window.key,
                        point: Point {
                            x: window.bounds.x + window.bounds.width * 0.5,
                            y: window.bounds.y,
                        },
                    },
                );
            }

            let left = Point {
                x: window.bounds.x + 12.0,
                y: window.bounds.y,
            };
            if corner_exposed(window.key, left, &self.windows) {
                push_landmark(
                    &mut landmarks,
                    TopologyLandmark {
                        kind: TopologyLandmarkKind::ExposedLeftCorner,
                        window_key: window.key,
                        point: left,
                    },
                );
            }
            let right = Point {
                x: window.bounds.right() - 12.0,
                y: window.bounds.y,
            };
            if corner_exposed(window.key, right, &self.windows) {
                push_landmark(
                    &mut landmarks,
                    TopologyLandmark {
                        kind: TopologyLandmarkKind::ExposedRightCorner,
                        window_key: window.key,
                        point: right,
                    },
                );
            }

            if previous.get(&window.key).is_some_and(|old| {
                let movement = Point {
                    x: window.bounds.x - old.x,
                    y: window.bounds.y - old.y,
                };
                let distance = movement.distance(Point::default());
                let resize = (window.bounds.width - old.width).abs()
                    + (window.bounds.height - old.height).abs();
                distance > 0.0 && distance <= 80.0 && resize <= 8.0
            }) {
                push_landmark(
                    &mut landmarks,
                    TopologyLandmark {
                        kind: TopologyLandmarkKind::SlowMovingPlatform,
                        window_key: window.key,
                        point: Point {
                            x: window.bounds.x + window.bounds.width * 0.5,
                            y: window.bounds.y,
                        },
                    },
                );
            }
        }
        self.landmarks = landmarks;
        true
    }

    pub fn update_cursor_invitation(&mut self, desktop: &DesktopSnapshot, dt: f32) {
        if !desktop.cursor.available
            || desktop.cursor.velocity.distance(Point::default()) >= INVITATION_SPEED
        {
            self.invitation_dwell = None;
            self.invitation = None;
            return;
        }
        let candidate = self
            .windows
            .iter()
            .map(|window| {
                let point = Point {
                    x: desktop
                        .cursor
                        .position
                        .x
                        .clamp(window.bounds.x + 12.0, window.bounds.right() - 12.0),
                    y: window.bounds.y,
                };
                (desktop.cursor.position.distance(point), window, point)
            })
            .filter(|(distance, _, _)| *distance <= INVITATION_DISTANCE)
            .min_by(|a, b| a.0.total_cmp(&b.0));
        let Some((_, window, point)) = candidate else {
            self.invitation_dwell = None;
            self.invitation = None;
            return;
        };
        let continuing = self.invitation_dwell.as_ref().is_some_and(|dwell| {
            dwell.window_key == window.key && dwell.point.distance(point) <= INVITATION_DISTANCE
        });
        if continuing {
            let dwell = self
                .invitation_dwell
                .as_mut()
                .expect("continuing cursor dwell exists");
            dwell.point = point;
            dwell.elapsed += dt.max(0.0);
        } else {
            self.invitation_dwell = Some(InvitationDwell {
                window_key: window.key,
                point,
                monitor_id: window.monitor_id,
                elapsed: dt.max(0.0),
            });
        }
        let dwell = self
            .invitation_dwell
            .expect("cursor invitation candidate initializes dwell");
        self.invitation = (dwell.elapsed >= INVITATION_DWELL_SECS).then_some(CursorInvitation {
            window_key: dwell.window_key,
            point: dwell.point,
            monitor_id: dwell.monitor_id,
        });
    }

    pub fn invitation(&self) -> Option<CursorInvitation> {
        self.invitation
    }

    pub fn clear_invitation(&mut self) {
        self.invitation_dwell = None;
        self.invitation = None;
    }

    pub fn window(&self, key: WindowKey) -> Option<TopologyWindow> {
        self.windows
            .iter()
            .find(|window| window.key == key)
            .copied()
    }

    pub fn is_slow_platform(&self, key: WindowKey) -> bool {
        self.landmarks.iter().any(|landmark| {
            landmark.window_key == key && landmark.kind == TopologyLandmarkKind::SlowMovingPlatform
        })
    }

    pub fn nearest_corner(&self, window_key: WindowKey, point: Point) -> Option<Point> {
        self.landmarks
            .iter()
            .filter(|landmark| {
                landmark.window_key == window_key
                    && matches!(
                        landmark.kind,
                        TopologyLandmarkKind::ExposedLeftCorner
                            | TopologyLandmarkKind::ExposedRightCorner
                    )
            })
            .min_by(|a, b| point.distance(a.point).total_cmp(&point.distance(b.point)))
            .map(|landmark| landmark.point)
    }

    pub fn island_windows(&self) -> impl Iterator<Item = WindowKey> + '_ {
        self.landmarks.iter().filter_map(|landmark| {
            (landmark.kind == TopologyLandmarkKind::WindowIsland).then_some(landmark.window_key)
        })
    }

    pub fn windows(&self) -> &[TopologyWindow] {
        &self.windows
    }

    pub fn landmarks(&self) -> &[TopologyLandmark] {
        &self.landmarks
    }

    #[cfg(test)]
    fn rebuild_count(&self) -> u64 {
        self.rebuild_count
    }
}

fn bounded_visible_windows(desktop: &DesktopSnapshot) -> Vec<&crate::DesktopWindow> {
    let mut visible: Vec<_> = desktop
        .windows
        .iter()
        .filter(|window| {
            window.visible
                && !window.minimized
                && window.bounds.width >= 32.0
                && window.bounds.height >= 24.0
        })
        .collect();
    visible.sort_by_key(|window| (window.z_order, window.key));
    visible.truncate(MAX_TOPOLOGY_WINDOWS);
    visible
}

fn geometry_hash(windows: &[&crate::DesktopWindow]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for window in windows {
        for value in [
            window.key,
            u64::from(window.bounds.x.to_bits()),
            u64::from(window.bounds.y.to_bits()),
            u64::from(window.bounds.width.to_bits()),
            u64::from(window.bounds.height.to_bits()),
            u64::from(window.z_order),
        ] {
            hash ^= value;
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    hash
}

fn push_landmark(landmarks: &mut Vec<TopologyLandmark>, landmark: TopologyLandmark) {
    if landmarks.len() < MAX_TOPOLOGY_LANDMARKS {
        landmarks.push(landmark);
    }
}

fn corner_exposed(window_key: WindowKey, point: Point, windows: &[TopologyWindow]) -> bool {
    windows
        .iter()
        .all(|other| other.key == window_key || !other.bounds.contains(point))
}

fn expanded(rect: DesktopRect, amount: f32) -> DesktopRect {
    DesktopRect {
        x: rect.x - amount,
        y: rect.y - amount,
        width: rect.width + amount * 2.0,
        height: rect.height + amount * 2.0,
    }
}

trait RectOverlap {
    fn overlaps(self, other: Self) -> bool;
}

impl RectOverlap for DesktopRect {
    fn overlaps(self, other: Self) -> bool {
        self.x < other.right()
            && self.right() > other.x
            && self.y < other.bottom()
            && self.bottom() > other.y
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CursorSnapshot, DesktopWindow, DisplayKey, MonitorInfo};

    fn desktop(windows: Vec<DesktopWindow>) -> DesktopSnapshot {
        DesktopSnapshot {
            monitors: vec![MonitorInfo {
                id: 8,
                display_key: DisplayKey([8; 16]),
                bounds: DesktopRect {
                    x: -1600.0,
                    y: -200.0,
                    width: 3200.0,
                    height: 1400.0,
                },
                usable_bounds: DesktopRect {
                    x: -1600.0,
                    y: -160.0,
                    width: 3200.0,
                    height: 1320.0,
                },
                scale_factor: 2.0,
                primary: true,
            }],
            windows,
            ..DesktopSnapshot::default()
        }
    }

    fn window(key: u64, x: f32, y: f32, z_order: u32) -> DesktopWindow {
        DesktopWindow {
            key,
            bounds: DesktopRect {
                x,
                y,
                width: 240.0,
                height: 160.0,
            },
            z_order,
            visible: true,
            minimized: false,
            application: None,
            application_name: None,
        }
    }

    #[test]
    fn topology_handles_negative_coordinates_overlap_and_caps() {
        let mut windows = vec![window(1, -900.0, -80.0, 0), window(2, -850.0, -40.0, 1)];
        windows.extend((3..100).map(|key| {
            window(
                key,
                -1500.0 + ((key - 3) % 10) as f32 * 280.0,
                260.0 + ((key - 3) / 10) as f32 * 10.0,
                key as u32,
            )
        }));
        let desktop = desktop(windows);
        let mut topology = DesktopTopology::default();
        assert!(topology.rebuild_if_changed(&desktop, &BTreeMap::new()));
        assert_eq!(topology.windows().len(), MAX_TOPOLOGY_WINDOWS);
        assert!(topology.landmarks().len() <= MAX_TOPOLOGY_LANDMARKS);
        assert!(!topology.island_windows().any(|key| key == 1 || key == 2));
    }

    #[test]
    fn unchanged_geometry_does_not_rebuild() {
        let desktop = desktop(vec![window(1, -400.0, 120.0, 0)]);
        let mut topology = DesktopTopology::default();
        assert!(topology.rebuild_if_changed(&desktop, &BTreeMap::new()));
        assert!(!topology.rebuild_if_changed(&desktop, &BTreeMap::new()));
        assert_eq!(topology.rebuild_count(), 1);
    }

    #[test]
    fn slow_platform_and_rapid_change_are_distinguished() {
        let old = window(1, 100.0, 100.0, 0).bounds;
        let previous = BTreeMap::from([(1, old)]);
        let mut topology = DesktopTopology::default();
        assert!(topology.rebuild_if_changed(&desktop(vec![window(1, 124.0, 104.0, 0)]), &previous));
        assert!(topology.is_slow_platform(1));

        assert!(topology.rebuild_if_changed(&desktop(vec![window(1, 400.0, 100.0, 0)]), &previous));
        assert!(!topology.is_slow_platform(1));
    }

    #[test]
    fn calm_cursor_dwell_creates_and_motion_clears_invitation() {
        let mut desktop = desktop(vec![window(1, 100.0, 100.0, 0)]);
        desktop.cursor = CursorSnapshot {
            position: Point { x: 180.0, y: 112.0 },
            velocity: Point { x: 4.0, y: 0.0 },
            available: true,
        };
        let mut topology = DesktopTopology::default();
        topology.rebuild_if_changed(&desktop, &BTreeMap::new());
        topology.update_cursor_invitation(&desktop, 0.75);
        assert!(topology.invitation().is_none());
        topology.update_cursor_invitation(&desktop, 0.76);
        assert_eq!(topology.invitation().map(|value| value.window_key), Some(1));

        desktop.cursor.velocity.x = 26.0;
        topology.update_cursor_invitation(&desktop, 0.1);
        assert!(topology.invitation().is_none());
    }
}
