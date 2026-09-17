use crate::{
    ColonyHome, ColonyObject, Creature, CreatureRole, DesktopRect, HabitatPolicy, HabitatPreset,
    HabitatZoneKind, HomeCorner, MonitorInfo, Point,
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

pub fn resolved_colony_object_position(
    object: &ColonyObject,
    monitors: &[MonitorInfo],
    policy: &HabitatPolicy,
) -> Option<(u64, Point)> {
    let preferred = monitors
        .iter()
        .find(|monitor| monitor.display_key == object.display)
        .or_else(|| monitors.iter().find(|monitor| monitor.primary))
        .or_else(|| monitors.first())?;
    let intended = Point {
        x: preferred.usable_bounds.x
            + object.normalized_position.x.clamp(0.0, 1.0) * preferred.usable_bounds.width,
        y: preferred.usable_bounds.y
            + object.normalized_position.y.clamp(0.0, 1.0) * preferred.usable_bounds.height,
    };
    accessible_regions(policy, preferred)
        .into_iter()
        .map(|region| {
            let point = Point {
                x: intended.x.clamp(region.x + 8.0, region.right() - 8.0),
                y: region.bottom() - 4.0,
            };
            (preferred.id, point, intended.distance(point))
        })
        .min_by(|a, b| a.2.total_cmp(&b.2))
        .map(|(monitor_id, point, _)| (monitor_id, point))
        .or_else(|| nearest_habitat_point(policy, monitors, intended))
}

/// What stands on a lot in the village strip beside the colony house.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DwellingKind {
    /// The shared colony house, always the first lot.
    Main,
    /// A full-size companion house for another adult.
    Cottage,
    /// A matching half-size house for a mini.
    MiniCottage,
}

impl DwellingKind {
    /// Ground footprint in shelter pixels: the drawn house plus its shadow and any decoration
    /// that reaches past the wall. Smaller than the 64px atlas cell it is sampled from, so
    /// neighbours sit close without their artwork touching.
    pub const fn width(self) -> f32 {
        match self {
            Self::Main => 60.0,
            Self::Cottage => 34.0,
            Self::MiniCottage => 22.0,
        }
    }
}

/// Every dwelling is drawn from one 64px atlas cell, whatever its footprint.
pub const DWELLING_CELL: f32 = 64.0;

/// A lot on the strip: either a dwelling, or one of the colony's loose objects.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VillageLot {
    Dwelling(usize),
    Object(usize),
}

const VILLAGE_GAP: f32 = 5.0;
const OBJECT_WIDTH: f32 = 16.0;

/// One ground-line walk outward from the colony house. Houses and belongings are laid out
/// together, so a keepsake can never land on a cottage and the corner reads as one small
/// village rather than a grid of shelves. Lots which cannot fit the house's accessible
/// region stay stored but hidden, rather than spilling elsewhere.
fn village_walk(cottages: &[DwellingKind], objects: usize) -> Vec<(VillageLot, f32, f32)> {
    // Belongings and cottages alternate near the house, then the remaining belongings
    // trail off along the strip.
    let mut order = Vec::with_capacity(1 + cottages.len() + objects);
    let mut next_object = 0;
    for index in 0..cottages.len() {
        if next_object < objects {
            order.push(VillageLot::Object(next_object));
            next_object += 1;
        }
        order.push(VillageLot::Dwelling(index + 1));
    }
    while next_object < objects {
        order.push(VillageLot::Object(next_object));
        next_object += 1;
    }

    let mut lots = Vec::with_capacity(order.len() + 1);
    lots.push((VillageLot::Dwelling(0), 0.0, 0.0));
    let mut edge = DwellingKind::Main.width() / 2.0;
    for lot in order {
        let width = match lot {
            VillageLot::Dwelling(index) => cottages[index - 1].width(),
            VillageLot::Object(_) => OBJECT_WIDTH,
        };
        edge += VILLAGE_GAP;
        lots.push((lot, edge + width / 2.0, 0.0));
        edge += width;
    }
    lots
}

/// Deterministic per-colony drift, so two colonies do not lay their belongings out identically.
fn object_drift(detail_seed: u64, slot: usize) -> (f32, f32) {
    let bits = (detail_seed >> ((slot % 12) * 5)) & 0x1f;
    ((bits & 0x3) as f32 - 1.5, ((bits >> 2) & 0x1) as f32)
}

fn village_position(
    home: &ColonyHome,
    lot: VillageLot,
    cottages: &[DwellingKind],
    objects: usize,
    monitors: &[MonitorInfo],
    policy: &HabitatPolicy,
    display_scale: u8,
) -> Option<(u64, Point)> {
    let monitor = monitors
        .iter()
        .find(|m| Some(m.display_key) == home.display)
        .or_else(|| monitors.iter().find(|m| m.primary))
        .or_else(|| monitors.first())?;
    let anchor = resolved_home_anchor(home, monitor, display_scale, policy)?;
    let scale = f32::from(display_scale) / monitor.scale_factor.max(1.0);
    let direction = if home.corner == HomeCorner::BottomLeft {
        1.0
    } else {
        -1.0
    };
    let (_, center, _) = village_walk(cottages, objects)
        .into_iter()
        .find(|(candidate, _, _)| *candidate == lot)?;
    let (half, drift_x, lift) = match lot {
        VillageLot::Dwelling(0) => (DwellingKind::Main.width() / 2.0, 0.0, 0.0),
        VillageLot::Dwelling(index) => (cottages[index - 1].width() / 2.0, 0.0, 0.0),
        VillageLot::Object(slot) => {
            let (drift_x, lift) = object_drift(home.shelter.detail_seed, slot);
            (OBJECT_WIDTH / 2.0, drift_x, lift)
        }
    };
    let point = Point {
        x: anchor.x + direction * (center + drift_x) * scale,
        y: anchor.y - lift * scale,
    };
    let height = match lot {
        VillageLot::Object(_) => OBJECT_WIDTH,
        _ => DWELLING_CELL,
    };
    accessible_regions(policy, monitor)
        .iter()
        .any(|r| {
            anchor.x >= r.x
                && anchor.x <= r.right()
                && anchor.y >= r.y
                && anchor.y <= r.bottom()
                && point.x - half * scale >= r.x
                && point.x + half * scale <= r.right()
                && point.y - height * scale >= r.y
                && point.y <= r.bottom()
        })
        .then_some((monitor.id, point))
}

/// Companion houses for the colony, in stable colony order. The first member shares the
/// colony house; every later arrival gets one of its own, and a mini gets a matching half-size
/// one, so the corner grows into a small village as the colony does.
pub fn colony_cottages(creatures: &[Creature]) -> Vec<DwellingKind> {
    let mut members: Vec<_> = creatures.iter().collect();
    members.sort_by_key(|creature| (creature.colony_order, creature.id));
    members
        .into_iter()
        .skip(1)
        .take(crate::MAX_COLONY_CREATURES - 1)
        .map(|creature| match creature.role {
            CreatureRole::Mini { .. } => DwellingKind::MiniCottage,
            CreatureRole::Adult => DwellingKind::Cottage,
        })
        .collect()
}

/// Position of one companion house in the village. Slot 0 is the colony house itself.
pub fn home_dwelling_position(
    home: &ColonyHome,
    slot: usize,
    cottages: &[DwellingKind],
    objects: usize,
    monitors: &[MonitorInfo],
    policy: &HabitatPolicy,
    display_scale: u8,
) -> Option<(u64, Point)> {
    if slot > cottages.len() {
        return None;
    }
    village_position(
        home,
        VillageLot::Dwelling(slot),
        cottages,
        objects,
        monitors,
        policy,
        display_scale,
    )
}

/// Position of one loose colony object on the village ground line.
pub fn home_object_position(
    home: &ColonyHome,
    slot: usize,
    cottages: &[DwellingKind],
    monitors: &[MonitorInfo],
    policy: &HabitatPolicy,
    display_scale: u8,
) -> Option<(u64, Point)> {
    if slot >= crate::MAX_COLONY_OBJECTS {
        return None;
    }
    village_position(
        home,
        VillageLot::Object(slot),
        cottages,
        crate::MAX_COLONY_OBJECTS,
        monitors,
        policy,
        display_scale,
    )
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

    /// A display can sit left of and above the origin, and at any scale factor. Zones are stored
    /// as a fraction of the display they were drawn on, so both have to come back as ordinary
    /// rectangles in desktop points, and a zone drawn on a display that is no longer plugged in
    /// must not take away the ground on the one that is.
    #[test]
    fn a_negative_origin_display_keeps_its_own_zones_and_a_lost_one_leaves_room_to_stand() {
        let far = |scale: f32| MonitorInfo {
            id: 2,
            display_key: DisplayKey([2; 16]),
            bounds: DesktopRect {
                x: -1_000.0,
                y: -600.0,
                width: 1_000.0,
                height: 600.0,
            },
            usable_bounds: DesktopRect {
                x: -1_000.0,
                y: -580.0,
                width: 1_000.0,
                height: 580.0,
            },
            scale_factor: scale,
            primary: false,
        };
        let mut policy = HabitatPolicy::default();
        policy.zones.push(HabitatZone {
            id: 4,
            display: DisplayKey([2; 16]),
            normalized_bounds: DesktopRect {
                x: 0.5,
                y: 0.0,
                width: 0.5,
                height: 1.0,
            },
            kind: HabitatZoneKind::Allowed,
            enabled: true,
        });
        let regions = accessible_regions(&policy, &far(1.0));
        assert_eq!(
            regions,
            vec![DesktopRect {
                x: -500.0,
                y: -580.0,
                width: 500.0,
                height: 580.0,
            }]
        );
        assert!(habitat_contains(
            &policy,
            &far(1.0),
            Point {
                x: -250.0,
                y: -300.0
            }
        ));
        assert!(!habitat_contains(
            &policy,
            &far(1.0),
            Point {
                x: -750.0,
                y: -300.0
            }
        ));
        assert_eq!(
            accessible_regions(&policy, &far(3.0)),
            regions,
            "a zone is a fraction of a display, not of its pixels"
        );
        // The zone belongs to that display alone; the display beside it keeps its whole preset.
        assert_eq!(
            accessible_regions(&policy, &monitor()),
            vec![monitor().usable_bounds]
        );

        // Standing between the two, a creature is sent to the nearer one; with the far display
        // unplugged, the zone that named it cannot leave the colony with nowhere to stand.
        let both = [monitor(), far(1.0)];
        assert_eq!(
            nearest_habitat_point(
                &policy,
                &both,
                Point {
                    x: -200.0,
                    y: -100.0
                }
            )
            .map(|(id, _)| id),
            Some(2)
        );
        assert_eq!(
            nearest_habitat_point(&policy, &both, Point { x: 800.0, y: 700.0 }).map(|(id, _)| id),
            Some(1)
        );
        assert!(validate_habitat(&policy, &[monitor()]).is_ok());
        assert!(!accessible_regions(&policy, &monitor()).is_empty());
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
