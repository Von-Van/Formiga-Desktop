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
    /// A matching smaller house for a mini.
    MiniCottage,
}

impl DwellingKind {
    /// Ground footprint in shelter pixels: the drawn house plus its shadow and any decoration
    /// that reaches past the wall, with a pixel or two of breathing room. Smaller than the 64px
    /// atlas cell it is sampled from, so neighbours sit close without their artwork touching.
    /// A companion house is near enough a standing creature's own width to read as a home rather
    /// than a model of one; the colony house stays plainly the largest building on the strip.
    pub const fn width(self) -> f32 {
        match self {
            Self::Main => 60.0,
            Self::Cottage => 46.0,
            Self::MiniCottage => 36.0,
        }
    }
}

/// Every dwelling is drawn from one 64px atlas cell, whatever its footprint.
pub const DWELLING_CELL: f32 = 64.0;

/// How wide a creature's frame draws, in shelter pixels. The village mirrors
/// `formiga_art::FRAME_SIZE` the same way `home_anchor` mirrors the shelter's own size.
pub const CREATURE_FRAME_WIDTH: f32 = 48.0;

/// The closest two resting companions may stand, centre to centre, as a fraction of
/// `CREATURE_FRAME_WIDTH`: shoulder to shoulder, with neither one's face behind the other.
/// Mirrors `world::spacing::FACE_CLEAR_RATIO`, which the simulation applies to every other
/// arrangement that holds creatures still.
pub const REST_CLEAR_RATIO: f32 = 0.80;

/// A lot on the strip: a dwelling, the porch beside one, or one of the colony's loose objects.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VillageLot {
    Dwelling(usize),
    /// The standing spot beside dwelling `n`'s door, where its resident waits out a home visit.
    Porch(usize),
    Object(usize),
}

const VILLAGE_GAP: f32 = 5.0;
const OBJECT_WIDTH: f32 = 16.0;

/// A porch is narrower than the frame that stands on it: the overhang is exactly the gap beside
/// it, so a resting creature reaches the edge of its neighbour's lot and never crosses it.
pub const PORCH_WIDTH: f32 = CREATURE_FRAME_WIDTH - VILLAGE_GAP * 2.0;

/// The ground each kind of lot claims, in shelter pixels.
fn lot_width(lot: VillageLot, cottages: &[DwellingKind]) -> f32 {
    match lot {
        VillageLot::Dwelling(0) => DwellingKind::Main.width(),
        VillageLot::Dwelling(index) => cottages
            .get(index - 1)
            .map_or(DwellingKind::MiniCottage.width(), |kind| kind.width()),
        VillageLot::Porch(_) => PORCH_WIDTH,
        VillageLot::Object(_) => OBJECT_WIDTH,
    }
}

/// How tall a lot's contents draw, for deciding whether the lot fits its region.
fn lot_height(lot: VillageLot) -> f32 {
    match lot {
        VillageLot::Dwelling(_) => DWELLING_CELL,
        VillageLot::Porch(_) => CREATURE_FRAME_WIDTH,
        VillageLot::Object(_) => OBJECT_WIDTH,
    }
}

/// One ground-line walk outward from the colony house. Houses, the porches beside them, and
/// belongings are laid out together, so nobody can stand in front of a door and a keepsake can
/// never land on a cottage. Every colony reserves the same belonging lots whether or not it has
/// collected them yet, so a house never shuffles sideways when a new keepsake turns up. Lots
/// which cannot fit the house's accessible region stay stored but hidden, rather than spilling
/// elsewhere.
fn village_walk(cottages: &[DwellingKind]) -> Vec<(VillageLot, f32, f32)> {
    // The colony house and its resident's porch first, then a belonging lot, a cottage and its
    // own porch for every later member, then the belongings that did not fit between houses.
    let interleaved = cottages.len().min(crate::MAX_COLONY_OBJECTS);
    let mut order = Vec::with_capacity(1 + cottages.len() * 3 + crate::MAX_COLONY_OBJECTS);
    order.push(VillageLot::Porch(0));
    for index in 0..cottages.len() {
        if index < interleaved {
            order.push(VillageLot::Object(index));
        }
        order.push(VillageLot::Dwelling(index + 1));
        order.push(VillageLot::Porch(index + 1));
    }
    for slot in interleaved..crate::MAX_COLONY_OBJECTS {
        order.push(VillageLot::Object(slot));
    }

    let mut lots = Vec::with_capacity(order.len() + 1);
    lots.push((VillageLot::Dwelling(0), 0.0, 0.0));
    let mut edge = DwellingKind::Main.width() / 2.0;
    let mut previous = VillageLot::Dwelling(0);
    for lot in order {
        // A porch belongs to its own house: the two share a lot line, so a resident waits at its
        // own door rather than a step down the lane. Everything else keeps its elbow room.
        let own_door =
            matches!((previous, lot), (VillageLot::Dwelling(a), VillageLot::Porch(b)) if a == b);
        let width = lot_width(lot, cottages);
        edge += if own_door { 0.0 } else { VILLAGE_GAP };
        lots.push((lot, edge + width / 2.0, 0.0));
        edge += width;
        previous = lot;
    }
    lots
}

/// Deterministic per-colony drift, so two colonies do not lay their belongings out identically.
fn object_drift(detail_seed: u64, slot: usize) -> (f32, f32) {
    let bits = (detail_seed >> ((slot % 12) * 5)) & 0x1f;
    ((bits & 0x3) as f32 - 1.5, ((bits >> 2) & 0x1) as f32)
}

/// How far a lot's contents sit from the centre of its lot, and how far off the ground.
fn lot_offset(lot: VillageLot, detail_seed: u64) -> (f32, f32) {
    match lot {
        VillageLot::Object(slot) => object_drift(detail_seed, slot),
        _ => (0.0, 0.0),
    }
}

/// The village's frame of reference on the display it belongs to: where its ground line starts,
/// which way the strip runs, how many desktop points one shelter pixel covers, and the room it
/// has to run into.
struct VillageGround<'a> {
    monitor: &'a MonitorInfo,
    anchor: Point,
    scale: f32,
    direction: f32,
    regions: Vec<DesktopRect>,
}

impl<'a> VillageGround<'a> {
    fn resolve(
        home: &ColonyHome,
        monitors: &'a [MonitorInfo],
        policy: &HabitatPolicy,
        display_scale: u8,
    ) -> Option<Self> {
        let monitor = village_monitor(home, monitors)?;
        let anchor = resolved_home_anchor(home, monitor, display_scale, policy)?;
        Some(Self {
            monitor,
            anchor,
            scale: f32::from(display_scale) / monitor.scale_factor.max(1.0),
            direction: if home.corner == HomeCorner::BottomLeft {
                1.0
            } else {
                -1.0
            },
            regions: accessible_regions(policy, monitor),
        })
    }

    /// The point `centre` shelter pixels out along the strip, `lift` pixels off the ground.
    fn point(&self, centre: f32, lift: f32) -> Point {
        Point {
            x: self.anchor.x + self.direction * centre * self.scale,
            y: self.anchor.y - lift * self.scale,
        }
    }

    /// The accessible region the village itself stands in.
    fn region(&self) -> Option<DesktopRect> {
        self.regions.iter().copied().find(|region| {
            self.anchor.x >= region.x
                && self.anchor.x <= region.right()
                && self.anchor.y >= region.y
                && self.anchor.y <= region.bottom()
        })
    }

    /// Whether a lot that wide and that tall fits the region the village stands in.
    fn fits(&self, point: Point, half: f32, height: f32) -> bool {
        self.region().is_some_and(|region| {
            point.x - half * self.scale >= region.x
                && point.x + half * self.scale <= region.right()
                && point.y - height * self.scale >= region.y
                && point.y <= region.bottom()
        })
    }

    /// Where a lot's contents stand, if the strip has room for the lot here.
    fn place(
        &self,
        lot: VillageLot,
        centre: f32,
        cottages: &[DwellingKind],
        seed: u64,
    ) -> Option<Point> {
        let (drift_x, lift) = lot_offset(lot, seed);
        let point = self.point(centre + drift_x, lift);
        self.fits(point, lot_width(lot, cottages) / 2.0, lot_height(lot))
            .then_some(point)
    }

    /// The further of two points along the strip, counting outward from the colony house.
    fn outward_max(&self, a: f32, b: f32) -> f32 {
        if self.direction > 0.0 {
            a.max(b)
        } else {
            a.min(b)
        }
    }
}

fn village_position(
    home: &ColonyHome,
    lot: VillageLot,
    cottages: &[DwellingKind],
    monitors: &[MonitorInfo],
    policy: &HabitatPolicy,
    display_scale: u8,
) -> Option<(u64, Point)> {
    let ground = VillageGround::resolve(home, monitors, policy, display_scale)?;
    let (_, centre, _) = village_walk(cottages)
        .into_iter()
        .find(|(candidate, _, _)| *candidate == lot)?;
    let point = ground.place(lot, centre, cottages, home.shelter.detail_seed)?;
    Some((ground.monitor.id, point))
}

fn village_monitor<'a>(home: &ColonyHome, monitors: &'a [MonitorInfo]) -> Option<&'a MonitorInfo> {
    monitors
        .iter()
        .find(|m| Some(m.display_key) == home.display)
        .or_else(|| monitors.iter().find(|m| m.primary))
        .or_else(|| monitors.first())
}

/// Half the width a standing creature takes up, in shelter pixels.
const GUEST_HALF_WIDTH: f32 = CREATURE_FRAME_WIDTH / 2.0;

/// Where a member of the colony waits out a home visit: the porch beside its own door, so every
/// wall and doorway behind the row stays in view. `slot` is the member's place in colony order —
/// slot 0 shares the colony house, and every later slot has the cottage of the same number.
///
/// When the strip runs out of room for a porch — a narrow display, a habitat cut down to a
/// sliver — that member stands on the free ground just past whatever the village does manage to
/// show, spaced from everyone else by `REST_CLEAR_RATIO` of a frame. A region with no room even
/// for that gives up on porches entirely and lines the whole colony up along the ground from the
/// far edge inward, which can put somebody in front of a wall: keeping the colony on the display
/// and out of one another's faces matters more than a clear view of a house nobody can see
/// properly on a display that narrow anyway.
pub fn home_resting_position(
    home: &ColonyHome,
    slot: usize,
    cottages: &[DwellingKind],
    monitors: &[MonitorInfo],
    policy: &HabitatPolicy,
    display_scale: u8,
) -> Option<(u64, Point)> {
    if slot > cottages.len() {
        return None;
    }
    let ground = VillageGround::resolve(home, monitors, policy, display_scale)?;
    let seed = home.shelter.detail_seed;
    let mut edge: f32 = 0.0;
    let mut porches = vec![None; cottages.len() + 1];
    for (lot, centre, _) in village_walk(cottages) {
        if ground.place(lot, centre, cottages, seed).is_none() {
            continue;
        }
        edge = edge.max(centre + lot_width(lot, cottages) / 2.0);
        if let VillageLot::Porch(member) = lot
            && let Some(spot) = porches.get_mut(member)
        {
            *spot = ground.place(lot, centre, cottages, seed);
        }
    }
    let region = ground.region()?;
    let step = CREATURE_FRAME_WIDTH * REST_CLEAR_RATIO * ground.scale;
    let half = GUEST_HALF_WIDTH * ground.scale;
    // How far the region runs from the colony house, counting outward along the strip.
    let region_out = (if ground.direction > 0.0 {
        region.right()
    } else {
        region.x
    } - ground.anchor.x)
        * ground.direction;
    let missing: Vec<usize> = (0..porches.len())
        .filter(|member| porches[*member].is_none())
        .collect();
    // The free ground line beside the village: past the outermost lot that did fit.
    let free = (edge + VILLAGE_GAP + GUEST_HALF_WIDTH) * ground.scale;
    let spread = (missing.len() as f32 - 1.0).max(0.0) * step;
    let crowded = !missing.is_empty() && free + spread + half > region_out;

    if !crowded && let Some(point) = porches[slot] {
        return Some((ground.monitor.id, point));
    }
    let out = if crowded {
        // Not even the free ground will take them. The colony gives up on its porches and lines
        // up along the ground instead, packed in from the far edge: standing in front of a wall
        // is a poor look, and standing on one another is a worse one.
        region_out - half - (porches.len() as f32 - 1.0 - slot as f32) * step
    } else {
        let rank = missing
            .iter()
            .position(|member| *member == slot)
            .unwrap_or(0) as f32;
        free + rank * step
    };
    Some((
        ground.monitor.id,
        Point {
            x: ground.anchor.x + ground.direction * out,
            y: ground.anchor.y,
        },
    ))
}

/// Where a visitor stands: on the village ground line just past everything the colony actually
/// shows — the outermost house, porch or belonging that fits here, and any resident who had to
/// stand past the strip — so a guest is beside the village rather than in front of it or on top
/// of somebody. A corner with no room left for a guest to keep its distance has no guest spot.
pub fn home_guest_position(
    home: &ColonyHome,
    cottages: &[DwellingKind],
    objects: usize,
    monitors: &[MonitorInfo],
    policy: &HabitatPolicy,
    display_scale: u8,
) -> Option<(u64, Point)> {
    let ground = VillageGround::resolve(home, monitors, policy, display_scale)?;
    let region = ground.region()?;
    let half = GUEST_HALF_WIDTH * ground.scale;
    if region.width < half * 2.0 {
        return None;
    }
    let seed = home.shelter.detail_seed;
    let mut outer = ground.anchor.x;
    for (lot, centre, _) in village_walk(cottages) {
        // A belonging lot the colony has not filled yet is bare ground, not something to stand
        // past; every house and every porch belongs to somebody the moment it is laid out.
        if matches!(lot, VillageLot::Object(slot) if slot >= objects) {
            continue;
        }
        let Some(point) = ground.place(lot, centre, cottages, seed) else {
            continue;
        };
        let reach = point.x + ground.direction * lot_width(lot, cottages) / 2.0 * ground.scale;
        outer = ground.outward_max(outer, reach);
    }
    let resting: Vec<Point> = (0..=cottages.len())
        .filter_map(|slot| {
            home_resting_position(home, slot, cottages, monitors, policy, display_scale)
                .map(|(_, point)| point)
        })
        .collect();
    for point in &resting {
        outer = ground.outward_max(outer, point.x + ground.direction * half);
    }
    let desired = outer + ground.direction * (VILLAGE_GAP * ground.scale + half);
    let point = Point {
        x: desired.clamp(region.x + half, region.right() - half),
        y: ground.anchor.y,
    };
    let clear = CREATURE_FRAME_WIDTH * REST_CLEAR_RATIO * ground.scale;
    resting
        .iter()
        .all(|resident| (point.x - resident.x).abs() >= clear - 0.01)
        .then_some((ground.monitor.id, point))
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
    use crate::{ColonyHome, DisplayKey, HabitatZone, MonitorInfo};

    /// Room enough for the longest strip a colony can grow at any supported scale, so a test
    /// about where things sit is never really a test about what did not fit.
    fn wide_monitor(scale_factor: f32) -> MonitorInfo {
        MonitorInfo {
            id: 7,
            display_key: DisplayKey([9; 16]),
            bounds: DesktopRect {
                x: 0.0,
                y: 0.0,
                width: 3840.0,
                height: 1200.0,
            },
            usable_bounds: DesktopRect {
                x: 0.0,
                y: 0.0,
                width: 3840.0,
                height: 1200.0,
            },
            scale_factor,
            primary: true,
        }
    }

    fn village_home(monitor: &MonitorInfo, corner: HomeCorner) -> ColonyHome {
        let mut home = ColonyHome::from_seed([13; 32], Some(monitor.display_key), None, None);
        home.corner = corner;
        home
    }

    /// Every colony shape the village has to lay out: alone, with one companion, and full with
    /// two minis in it.
    fn colonies() -> Vec<Vec<DwellingKind>> {
        vec![
            Vec::new(),
            vec![DwellingKind::Cottage],
            vec![DwellingKind::Cottage, DwellingKind::MiniCottage],
            vec![
                DwellingKind::Cottage,
                DwellingKind::MiniCottage,
                DwellingKind::MiniCottage,
            ],
        ]
    }

    /// Every lot the village lays out, as `(lot, centre, half width)` in desktop points measured
    /// outward from the colony house, so both corners can be compared as one set of numbers.
    fn placed_lots(
        home: &ColonyHome,
        cottages: &[DwellingKind],
        monitor: &MonitorInfo,
        display_scale: u8,
    ) -> Vec<(VillageLot, f32, f32)> {
        let policy = HabitatPolicy::default();
        let monitors = std::slice::from_ref(monitor);
        let anchor = resolved_home_anchor(home, monitor, display_scale, &policy).unwrap();
        let unit = f32::from(display_scale) / monitor.scale_factor.max(1.0);
        let direction = if home.corner == HomeCorner::BottomLeft {
            1.0
        } else {
            -1.0
        };
        village_walk(cottages)
            .into_iter()
            .filter_map(|(lot, centre, _)| {
                let point = match lot {
                    VillageLot::Dwelling(slot) => home_dwelling_position(
                        home,
                        slot,
                        cottages,
                        monitors,
                        &policy,
                        display_scale,
                    ),
                    VillageLot::Object(slot) => {
                        home_object_position(home, slot, cottages, monitors, &policy, display_scale)
                    }
                    VillageLot::Porch(slot) => home_resting_position(
                        home,
                        slot,
                        cottages,
                        monitors,
                        &policy,
                        display_scale,
                    ),
                }?
                .1;
                assert!(
                    (point.y - anchor.y).abs() <= unit + 0.01,
                    "{lot:?} left the ground line"
                );
                let _ = centre;
                Some((
                    lot,
                    (point.x - anchor.x) * direction,
                    lot_width(lot, cottages) / 2.0 * unit,
                ))
            })
            .collect()
    }

    /// Houses, the porches beside them, and the belongings all come out of one walk, so no two
    /// of them may ever claim the same ground — at any scale, on any display, in either corner,
    /// and measured identically from the colony house whichever corner it is.
    #[test]
    fn porches_houses_and_belongings_share_one_walk_and_mirror_in_both_corners() {
        for scale_factor in [1.0_f32, 2.0] {
            let monitor = wide_monitor(scale_factor);
            for display_scale in [2_u8, 3, 4] {
                for cottages in colonies() {
                    let mut mirrored: Option<Vec<(VillageLot, f32, f32)>> = None;
                    for corner in [HomeCorner::BottomLeft, HomeCorner::BottomRight] {
                        let home = village_home(&monitor, corner);
                        let lots = placed_lots(&home, &cottages, &monitor, display_scale);
                        assert_eq!(
                            lots.len(),
                            1 + cottages.len() * 2 + 1 + crate::MAX_COLONY_OBJECTS,
                            "every lot should fit a display this size"
                        );
                        for (index, (lot, centre, half)) in lots.iter().enumerate() {
                            for (other, other_centre, other_half) in &lots[index + 1..] {
                                let clearance = (centre - other_centre).abs();
                                assert!(
                                    clearance >= half + other_half - 0.01,
                                    "{corner:?} scale {display_scale}: {lot:?} and {other:?} \
                                     share ground ({clearance} apart, {half} + {other_half} wide)"
                                );
                            }
                        }
                        match &mirrored {
                            None => mirrored = Some(lots),
                            Some(first) => {
                                assert_eq!(
                                    first.len(),
                                    lots.len(),
                                    "the two corners laid out different villages"
                                );
                                for ((lot, a, _), (other, b, _)) in first.iter().zip(&lots) {
                                    assert_eq!(lot, other);
                                    assert!(
                                        (a - b).abs() < 0.01,
                                        "{lot:?} sits {a} out in one corner and {b} in the other"
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// The rule the whole overhaul exists for: a resting companion stands beside a house, never
    /// in front of one. Its frame may cover the outermost sliver of a lot — the gap's worth of
    /// wall edge it stands against — and must leave every door in the village in plain view.
    #[test]
    fn a_resting_frame_never_covers_a_door_or_more_than_a_sliver_of_a_house() {
        let policy = HabitatPolicy::default();
        for scale_factor in [1.0_f32, 2.0] {
            let monitor = wide_monitor(scale_factor);
            let monitors = std::slice::from_ref(&monitor);
            let unit = |display_scale: u8| f32::from(display_scale) / scale_factor.max(1.0);
            for display_scale in [2_u8, 3, 4] {
                for cottages in colonies() {
                    for corner in [HomeCorner::BottomLeft, HomeCorner::BottomRight] {
                        let home = village_home(&monitor, corner);
                        let houses: Vec<(f32, f32)> = (0..=cottages.len())
                            .map(|slot| {
                                let (_, point) = home_dwelling_position(
                                    &home,
                                    slot,
                                    &cottages,
                                    monitors,
                                    &policy,
                                    display_scale,
                                )
                                .unwrap();
                                let kind = if slot == 0 {
                                    DwellingKind::Main
                                } else {
                                    cottages[slot - 1]
                                };
                                (point.x, kind.width() / 2.0 * unit(display_scale))
                            })
                            .collect();
                        for slot in 0..=cottages.len() {
                            let (_, rest) = home_resting_position(
                                &home,
                                slot,
                                &cottages,
                                monitors,
                                &policy,
                                display_scale,
                            )
                            .unwrap();
                            let reach = CREATURE_FRAME_WIDTH / 2.0 * unit(display_scale);
                            for (house_x, house_half) in &houses {
                                let overlap = house_half + reach - (rest.x - house_x).abs();
                                assert!(
                                    overlap <= VILLAGE_GAP * unit(display_scale) + 0.01,
                                    "{corner:?} scale {display_scale}: member {slot} covers \
                                     {overlap} points of a house"
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    /// Nobody's face is ever behind somebody else's, however the colony is made up.
    #[test]
    fn every_resident_keeps_a_clear_face_from_its_neighbours() {
        let policy = HabitatPolicy::default();
        for scale_factor in [1.0_f32, 2.0] {
            let monitor = wide_monitor(scale_factor);
            let monitors = std::slice::from_ref(&monitor);
            for display_scale in [2_u8, 3, 4] {
                for cottages in colonies() {
                    for corner in [HomeCorner::BottomLeft, HomeCorner::BottomRight] {
                        let home = village_home(&monitor, corner);
                        let unit = f32::from(display_scale) / scale_factor.max(1.0);
                        let clear = CREATURE_FRAME_WIDTH * REST_CLEAR_RATIO * unit;
                        let spots: Vec<f32> = (0..=cottages.len())
                            .map(|slot| {
                                home_resting_position(
                                    &home,
                                    slot,
                                    &cottages,
                                    monitors,
                                    &policy,
                                    display_scale,
                                )
                                .unwrap()
                                .1
                                .x
                            })
                            .collect();
                        for (index, a) in spots.iter().enumerate() {
                            for b in &spots[index + 1..] {
                                assert!(
                                    (a - b).abs() >= clear - 0.01,
                                    "{corner:?} scale {display_scale}: two companions only \
                                     {} apart, {clear} needed",
                                    (a - b).abs()
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    /// A display too narrow for the whole village still has to seat everybody: those whose
    /// porches did not fit stand out on the free ground beyond it, spaced, never stacked.
    #[test]
    fn a_strip_with_no_room_still_seats_everybody_on_the_free_ground() {
        let policy = HabitatPolicy::default();
        let mut monitor = wide_monitor(1.0);
        monitor.bounds.width = 320.0;
        monitor.usable_bounds.width = 320.0;
        let monitors = std::slice::from_ref(&monitor);
        let cottages = vec![
            DwellingKind::Cottage,
            DwellingKind::MiniCottage,
            DwellingKind::MiniCottage,
        ];
        for corner in [HomeCorner::BottomLeft, HomeCorner::BottomRight] {
            let home = village_home(&monitor, corner);
            let region = accessible_regions(&policy, &monitor)[0];
            let mut crowded = false;
            let spots: Vec<f32> = (0..=cottages.len())
                .map(|slot| {
                    crowded |= village_position(
                        &home,
                        VillageLot::Porch(slot),
                        &cottages,
                        monitors,
                        &policy,
                        2,
                    )
                    .is_none();
                    home_resting_position(&home, slot, &cottages, monitors, &policy, 2)
                        .unwrap()
                        .1
                        .x
                })
                .collect();
            assert!(crowded, "{corner:?}: this display should be too narrow");
            let clear = CREATURE_FRAME_WIDTH * REST_CLEAR_RATIO * 2.0;
            for (index, a) in spots.iter().enumerate() {
                assert!(
                    *a >= region.x - 0.01 && *a <= region.right() + 0.01,
                    "{corner:?}: a companion was sent off the display"
                );
                for b in &spots[index + 1..] {
                    assert!(
                        (a - b).abs() >= clear - 0.01,
                        "{corner:?}: companions stacked {} apart",
                        (a - b).abs()
                    );
                }
            }
        }
    }

    /// A guest waits past the whole village: past every house, porch and belonging the colony
    /// actually shows, and never within a face's width of anybody resting.
    #[test]
    fn a_guest_waits_past_every_lot_and_every_resident() {
        let policy = HabitatPolicy::default();
        for scale_factor in [1.0_f32, 2.0] {
            let monitor = wide_monitor(scale_factor);
            let monitors = std::slice::from_ref(&monitor);
            for display_scale in [2_u8, 3, 4] {
                for cottages in colonies() {
                    for objects in [0_usize, 3, crate::MAX_COLONY_OBJECTS] {
                        for corner in [HomeCorner::BottomLeft, HomeCorner::BottomRight] {
                            let home = village_home(&monitor, corner);
                            let unit = f32::from(display_scale) / scale_factor.max(1.0);
                            let direction = if corner == HomeCorner::BottomLeft {
                                1.0
                            } else {
                                -1.0
                            };
                            let anchor =
                                resolved_home_anchor(&home, &monitor, display_scale, &policy)
                                    .unwrap();
                            let (_, guest) = home_guest_position(
                                &home,
                                &cottages,
                                objects,
                                monitors,
                                &policy,
                                display_scale,
                            )
                            .unwrap();
                            assert_eq!(guest.y, anchor.y);
                            let out = (guest.x - anchor.x) * direction;
                            for (lot, centre, _) in village_walk(&cottages) {
                                if matches!(lot, VillageLot::Object(slot) if slot >= objects) {
                                    continue;
                                }
                                let edge = (centre + lot_width(lot, &cottages) / 2.0) * unit;
                                assert!(
                                    out >= edge - 0.01,
                                    "{corner:?} scale {display_scale}: the guest stands on {lot:?}"
                                );
                            }
                            let clear = CREATURE_FRAME_WIDTH * REST_CLEAR_RATIO * unit;
                            for slot in 0..=cottages.len() {
                                let (_, rest) = home_resting_position(
                                    &home,
                                    slot,
                                    &cottages,
                                    monitors,
                                    &policy,
                                    display_scale,
                                )
                                .unwrap();
                                assert!(
                                    (guest.x - rest.x).abs() >= clear - 0.01,
                                    "{corner:?}: the guest stands on member {slot}"
                                );
                            }
                        }
                    }
                }
            }
        }
    }

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
