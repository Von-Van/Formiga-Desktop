use crate::{DesktopRect, DesktopSnapshot, MonitorId, Point, WindowKey};
use std::collections::{BTreeMap, VecDeque};

pub const MAX_TOPOLOGY_WINDOWS: usize = 64;
pub const MAX_TOPOLOGY_LANDMARKS: usize = 96;
const ISLAND_CLEARANCE: f32 = 24.0;
const INVITATION_DISTANCE: f32 = 24.0;
const INVITATION_SPEED: f32 = 25.0;
const INVITATION_DWELL_SECS: f32 = 1.5;
pub const MAX_WINDOW_ROUTE_HOPS: usize = 4;
const MIN_NARROW_GAP: f32 = 10.0;
const MAX_NARROW_GAP: f32 = 28.0;
const MIN_GAP_OVERLAP_HEIGHT: f32 = 64.0;

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RouteHopKind {
    WindowTier,
    NarrowGap,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TopologyRouteHop {
    pub kind: RouteHopKind,
    pub from_window: WindowKey,
    pub to_window: WindowKey,
    pub from_bounds: DesktopRect,
    pub to_bounds: DesktopRect,
    pub target: Point,
    pub monitor_id: MonitorId,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct RoutePreferences {
    pub climbing: i8,
    pub exploration: i8,
    pub cursor_trust: i8,
    pub target_hint: Option<Point>,
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

    pub fn geometry_hash(&self) -> u64 {
        self.geometry_hash
    }

    pub fn plan_route(
        &self,
        start_window: WindowKey,
        preferences: RoutePreferences,
    ) -> Vec<TopologyRouteHop> {
        let Some(start) = self.window(start_window) else {
            return Vec::new();
        };
        let mut queue = VecDeque::from([(start_window, Vec::<TopologyRouteHop>::new())]);
        let mut best_depth: BTreeMap<WindowKey, usize> = BTreeMap::from([(start_window, 0)]);
        let mut candidates = Vec::new();
        while let Some((window_key, path)) = queue.pop_front() {
            if path.len() >= MAX_WINDOW_ROUTE_HOPS {
                continue;
            }
            let Some(from) = self.window(window_key) else {
                continue;
            };
            let mut neighbors: Vec<_> = self
                .windows
                .iter()
                .copied()
                .filter(|to| to.key != from.key && to.monitor_id == from.monitor_id)
                .filter_map(|to| {
                    route_hop(from, to).map(|hop| {
                        let vertical = (to.bounds.y - from.bounds.y).abs();
                        (vertical, to.key, hop)
                    })
                })
                .collect();
            neighbors.sort_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
            for (_, to_key, hop) in neighbors {
                let next_depth = path.len() + 1;
                if best_depth
                    .get(&to_key)
                    .is_some_and(|depth| *depth <= next_depth)
                {
                    continue;
                }
                best_depth.insert(to_key, next_depth);
                let mut next = path.clone();
                next.push(hop);
                candidates.push(next.clone());
                queue.push_back((to_key, next));
            }
        }
        candidates
            .into_iter()
            .max_by(|a, b| {
                route_score(start, a, preferences)
                    .total_cmp(&route_score(start, b, preferences))
                    .then_with(|| route_destination(a).cmp(&route_destination(b)).reverse())
            })
            .unwrap_or_default()
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

fn route_hop(from: TopologyWindow, to: TopologyWindow) -> Option<TopologyRouteHop> {
    let overlap_x = from.bounds.right().min(to.bounds.right()) - from.bounds.x.max(to.bounds.x);
    let vertical = (from.bounds.y - to.bounds.y).abs();
    let gap = if from.bounds.right() <= to.bounds.x {
        to.bounds.x - from.bounds.right()
    } else if to.bounds.right() <= from.bounds.x {
        from.bounds.x - to.bounds.right()
    } else {
        0.0
    };
    let overlap_y = from.bounds.bottom().min(to.bounds.bottom()) - from.bounds.y.max(to.bounds.y);
    let kind = if (MIN_NARROW_GAP..=MAX_NARROW_GAP).contains(&gap)
        && overlap_y >= MIN_GAP_OVERLAP_HEIGHT
        && vertical <= 80.0
    {
        RouteHopKind::NarrowGap
    } else if overlap_x >= 48.0 && (36.0..=360.0).contains(&vertical) {
        RouteHopKind::WindowTier
    } else {
        return None;
    };
    let target_x = match kind {
        RouteHopKind::WindowTier => (from.bounds.x + from.bounds.width * 0.5)
            .clamp(to.bounds.x + 12.0, to.bounds.right() - 12.0),
        RouteHopKind::NarrowGap if to.bounds.x > from.bounds.x => to.bounds.x + 12.0,
        RouteHopKind::NarrowGap => to.bounds.right() - 12.0,
    };
    Some(TopologyRouteHop {
        kind,
        from_window: from.key,
        to_window: to.key,
        from_bounds: from.bounds,
        to_bounds: to.bounds,
        target: Point {
            x: target_x,
            y: to.bounds.y,
        },
        monitor_id: to.monitor_id,
    })
}

fn route_score(
    start: TopologyWindow,
    route: &[TopologyRouteHop],
    preferences: RoutePreferences,
) -> f32 {
    let Some(last) = route.last() else {
        return f32::NEG_INFINITY;
    };
    let climbing = f32::from(preferences.climbing) / 100.0;
    let exploration = f32::from(preferences.exploration) / 100.0;
    let trust = f32::from(preferences.cursor_trust) / 100.0;
    let height_gain = start.bounds.y - last.to_bounds.y;
    let distance = Point {
        x: start.bounds.x + start.bounds.width * 0.5,
        y: start.bounds.y,
    }
    .distance(last.target);
    let gap_count = route
        .iter()
        .filter(|hop| hop.kind == RouteHopKind::NarrowGap)
        .count() as f32;
    let target_score = preferences.target_hint.map_or(0.0, |target| {
        (600.0 - target.distance(last.target)).clamp(-600.0, 600.0)
            * (0.25 + trust.max(-0.5) * 0.15)
    });
    height_gain * (0.35 + climbing * 0.25)
        + distance * exploration.max(-0.5) * 0.12
        + route.len() as f32 * (20.0 + exploration * 18.0)
        + gap_count * (trust * 18.0 + exploration * 12.0 - 4.0)
        + target_score
}

fn route_destination(route: &[TopologyRouteHop]) -> WindowKey {
    route.last().map_or(0, |hop| hop.to_window)
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

    #[test]
    fn window_construction_routes_are_bounded_to_four_tiers() {
        let windows = (0..7)
            .map(|index| window(index + 1, 100.0, 700.0 - index as f32 * 100.0, index as u32))
            .collect();
        let desktop = desktop(windows);
        let mut topology = DesktopTopology::default();
        topology.rebuild_if_changed(&desktop, &BTreeMap::new());
        let route = topology.plan_route(
            1,
            RoutePreferences {
                climbing: 100,
                exploration: 100,
                ..RoutePreferences::default()
            },
        );
        assert!(!route.is_empty());
        assert!(route.len() <= MAX_WINDOW_ROUTE_HOPS);
        assert!(route.iter().all(|hop| hop.kind == RouteHopKind::WindowTier));
        assert!(
            route
                .windows(2)
                .all(|pair| pair[0].to_window == pair[1].from_window)
        );
    }

    #[test]
    fn only_ten_to_twenty_eight_point_gaps_with_vertical_overlap_are_squeezes() {
        let mut topology = DesktopTopology::default();
        topology.rebuild_if_changed(
            &desktop(vec![window(1, 100.0, 200.0, 0), window(2, 360.0, 210.0, 1)]),
            &BTreeMap::new(),
        );
        let route = topology.plan_route(1, RoutePreferences::default());
        assert_eq!(route.len(), 1);
        assert_eq!(route[0].kind, RouteHopKind::NarrowGap);

        for x in [349.0, 369.0] {
            let mut invalid = DesktopTopology::default();
            invalid.rebuild_if_changed(
                &desktop(vec![window(1, 100.0, 200.0, 0), window(2, x, 210.0, 1)]),
                &BTreeMap::new(),
            );
            assert!(
                invalid
                    .plan_route(1, RoutePreferences::default())
                    .is_empty()
            );
        }
    }
}
