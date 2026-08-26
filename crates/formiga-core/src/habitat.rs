use crate::{
    ColonyHome, DesktopRect, HabitatPolicy, HabitatPreset, HabitatZoneKind, HomeCorner,
    MonitorInfo, Point,
};

pub const MAX_HABITAT_ZONES: usize = 32;
const MIN_REGION_SIZE: f32 = 48.0;

pub fn home_anchor(home: &ColonyHome, monitor: &MonitorInfo, display_scale: u8) -> Point {
    let shelter_half_width = 32.0 * f32::from(display_scale) / monitor.scale_factor.max(1.0);
    let margin = shelter_half_width + 8.0;
    Point {
        x: match home.corner {
            HomeCorner::BottomLeft => monitor.usable_bounds.x + margin,
            HomeCorner::BottomRight => monitor.usable_bounds.right() - margin,
        },
        y: monitor.usable_bounds.bottom() - 4.0,
    }
}

pub fn resolved_home_anchor(
    home: &ColonyHome,
    monitor: &MonitorInfo,
    display_scale: u8,
    policy: &HabitatPolicy,
) -> Option<Point> {
    let desired = home_anchor(home, monitor, display_scale);
    accessible_regions(policy, monitor)
        .into_iter()
        .map(|region| {
            let point = Point {
                x: desired.x.clamp(region.x + 8.0, region.right() - 8.0),
                y: region.bottom() - 4.0,
            };
            (desired.distance(point), point)
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, point)| point)
}

pub fn accessible_regions(policy: &HabitatPolicy, monitor: &MonitorInfo) -> Vec<DesktopRect> {
    if policy.preset == HabitatPreset::PrimaryDisplay && !monitor.primary {
        return Vec::new();
    }

    let mut allowed: Vec<_> = policy
        .zones
        .iter()
        .filter(|zone| {
            zone.enabled
                && zone.display == monitor.display_key
                && zone.kind == HabitatZoneKind::Allowed
        })
        .filter_map(|zone| denormalize(zone.normalized_bounds, monitor.usable_bounds))
        .collect();
    if allowed.is_empty() {
        allowed = preset_regions(policy.preset, monitor);
    }

    let exclusions: Vec<_> = policy
        .zones
        .iter()
        .filter(|zone| {
            zone.enabled
                && zone.display == monitor.display_key
                && zone.kind == HabitatZoneKind::Excluded
        })
        .filter_map(|zone| denormalize(zone.normalized_bounds, monitor.usable_bounds))
        .collect();
    for excluded in exclusions {
        allowed = allowed
            .into_iter()
            .flat_map(|region| subtract(region, excluded))
            .collect();
    }
    allowed
        .into_iter()
        .filter(|region| region.width >= MIN_REGION_SIZE && region.height >= MIN_REGION_SIZE)
        .collect()
}

pub fn habitat_contains(policy: &HabitatPolicy, monitor: &MonitorInfo, point: Point) -> bool {
    accessible_regions(policy, monitor)
        .into_iter()
        .any(|region| region.contains(point))
}

pub fn nearest_habitat_point(
    policy: &HabitatPolicy,
    monitors: &[MonitorInfo],
    point: Point,
) -> Option<(u64, Point)> {
    monitors
        .iter()
        .flat_map(|monitor| {
            accessible_regions(policy, monitor)
                .into_iter()
                .map(move |region| {
                    let candidate = Point {
                        x: point.x.clamp(region.x + 8.0, region.right() - 8.0),
                        y: region.bottom() - 4.0,
                    };
                    (monitor.id, candidate, point.distance(candidate))
                })
        })
        .min_by(|a, b| a.2.total_cmp(&b.2))
        .map(|(monitor_id, point, _)| (monitor_id, point))
}

pub fn validate_habitat(
    policy: &HabitatPolicy,
    monitors: &[MonitorInfo],
) -> Result<(), &'static str> {
    if policy.zones.len() > MAX_HABITAT_ZONES {
        return Err("a habitat can contain at most 32 zones");
    }
    let has_region = monitors
        .iter()
        .any(|monitor| !accessible_regions(policy, monitor).is_empty());
    has_region
        .then_some(())
        .ok_or("the habitat must leave at least one usable region")
}

fn preset_regions(preset: HabitatPreset, monitor: &MonitorInfo) -> Vec<DesktopRect> {
    let bounds = monitor.usable_bounds;
    match preset {
        HabitatPreset::EntireDesktop | HabitatPreset::PrimaryDisplay | HabitatPreset::Custom => {
            vec![bounds]
        }
        HabitatPreset::BottomEdge => vec![DesktopRect {
            x: bounds.x,
            y: bounds.y + bounds.height * 0.75,
            width: bounds.width,
            height: bounds.height * 0.25,
        }],
        HabitatPreset::BottomCorners => {
            let width = bounds.width * 0.25;
            let height = bounds.height * 0.3;
            vec![
                DesktopRect {
                    x: bounds.x,
                    y: bounds.bottom() - height,
                    width,
                    height,
                },
                DesktopRect {
                    x: bounds.right() - width,
                    y: bounds.bottom() - height,
                    width,
                    height,
                },
            ]
        }
        HabitatPreset::LowerHalf => vec![DesktopRect {
            x: bounds.x,
            y: bounds.y + bounds.height * 0.5,
            width: bounds.width,
            height: bounds.height * 0.5,
        }],
    }
}

fn denormalize(normalized: DesktopRect, bounds: DesktopRect) -> Option<DesktopRect> {
    let normalized = DesktopRect {
        x: normalized.x.clamp(0.0, 1.0),
        y: normalized.y.clamp(0.0, 1.0),
        width: normalized.width.clamp(0.0, 1.0),
        height: normalized.height.clamp(0.0, 1.0),
    };
    DesktopRect {
        x: bounds.x + normalized.x * bounds.width,
        y: bounds.y + normalized.y * bounds.height,
        width: normalized.width * bounds.width,
        height: normalized.height * bounds.height,
    }
    .intersection(bounds)
}

fn subtract(source: DesktopRect, cut: DesktopRect) -> Vec<DesktopRect> {
    let Some(overlap) = source.intersection(cut) else {
        return vec![source];
    };
    let candidates = [
        DesktopRect {
            x: source.x,
            y: source.y,
            width: source.width,
            height: overlap.y - source.y,
        },
        DesktopRect {
            x: source.x,
            y: overlap.bottom(),
            width: source.width,
            height: source.bottom() - overlap.bottom(),
        },
        DesktopRect {
            x: source.x,
            y: overlap.y,
            width: overlap.x - source.x,
            height: overlap.height,
        },
        DesktopRect {
            x: overlap.right(),
            y: overlap.y,
            width: source.right() - overlap.right(),
            height: overlap.height,
        },
    ];
    candidates
        .into_iter()
        .filter(|rect| rect.width > 0.0 && rect.height > 0.0)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DisplayKey, HabitatZone, MonitorInfo};

    fn monitor() -> MonitorInfo {
        MonitorInfo {
            id: 1,
            display_key: DisplayKey([1; 16]),
            bounds: DesktopRect {
                x: 0.0,
                y: 0.0,
                width: 1000.0,
                height: 800.0,
            },
            usable_bounds: DesktopRect {
                x: 0.0,
                y: 20.0,
                width: 1000.0,
                height: 740.0,
            },
            scale_factor: 2.0,
            primary: true,
        }
    }

    #[test]
    fn exclusion_is_subtracted_from_allowed_space() {
        let mut policy = HabitatPolicy::default();
        policy.zones.push(HabitatZone {
            id: 1,
            display: DisplayKey([1; 16]),
            normalized_bounds: DesktopRect {
                x: 0.4,
                y: 0.0,
                width: 0.2,
                height: 1.0,
            },
            kind: HabitatZoneKind::Excluded,
            enabled: true,
        });
        let regions = accessible_regions(&policy, &monitor());
        assert_eq!(regions.len(), 2);
        assert!(
            !regions
                .iter()
                .any(|region| region.contains(Point { x: 500.0, y: 400.0 }))
        );
    }

    #[test]
    fn primary_preset_excludes_secondary_displays() {
        let mut secondary = monitor();
        secondary.primary = false;
        let policy = HabitatPolicy {
            preset: HabitatPreset::PrimaryDisplay,
            zones: Vec::new(),
        };
        assert!(accessible_regions(&policy, &secondary).is_empty());
    }
}
