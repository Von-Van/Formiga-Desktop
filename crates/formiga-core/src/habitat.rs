use crate::{
    ColonyHome, ColonyObject, Creature, CreatureRole, DesktopRect, GardenKind, HabitatPolicy,
    HabitatPreset, HabitatZoneKind, HangoutKind, HomeCorner, MonitorInfo, OrnamentKind, Point,
};

pub const MAX_HABITAT_ZONES: usize = 32;
const MIN_REGION_SIZE: f32 = 48.0;

pub fn home_anchor(home: &ColonyHome, monitor: &MonitorInfo, display_scale: u8) -> Point {
    let margin = HOME_EDGE_MARGIN * f32::from(display_scale) / monitor.scale_factor.max(1.0) + 8.0;
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
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DwellingKind {
    /// The shared colony house, always the first lot.
    Main,
    /// A full-size companion house for another adult. The kind a `Cottages` slot holds before
    /// anybody has moved into it, which is why it is the default.
    #[default]
    Cottage,
}

impl DwellingKind {
    /// Ground footprint in shelter pixels: the drawn house plus its shadow and any decoration
    /// that reaches past the wall, with a pixel or two of breathing room. Smaller than the 80px
    /// atlas cell it is sampled from, so neighbours sit close without their artwork touching.
    /// The houses are drawn a quarter larger than they were until 0.61.0, so a companion's house
    /// is a little wider than a standing creature and reads plainly as its home; the colony house
    /// stays plainly the largest building on the strip. The footprints grew less than the houses
    /// did, from 60 and 46, because shadows and decorations did not grow with them.
    pub const fn width(self) -> f32 {
        match self {
            Self::Main => 70.0,
            Self::Cottage => 56.0,
        }
    }
}

/// Every dwelling is drawn from one 80px atlas cell, whatever its footprint.
pub const DWELLING_CELL: f32 = 80.0;

/// How wide a creature's frame draws, in shelter pixels. The village mirrors
/// `formiga_art::FRAME_SIZE` the same way `home_anchor` mirrors the shelter's own size.
pub const CREATURE_FRAME_WIDTH: f32 = 48.0;

/// The closest two resting companions may stand, centre to centre, as a fraction of
/// `CREATURE_FRAME_WIDTH`: shoulder to shoulder, with neither one's face behind the other.
/// Mirrors `world::spacing::FACE_CLEAR_RATIO`, which the simulation applies to every other
/// arrangement that holds creatures still.
pub const REST_CLEAR_RATIO: f32 = 0.80;

/// Which end of the village a keepsake tree stands at. The pair are the village's walls: the
/// houses are contained between them, and everything the colony owns is kept in one of the two
/// yards at their feet.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TreeEnd {
    /// The tree nearest the screen corner, on the far side of the colony house from the cottages.
    /// The one `HOME_EDGE_MARGIN` reserves ground for against the edge of the display.
    Outward,
    /// The tree past the last house, at the inward end of the strip.
    Inward,
}

impl TreeEnd {
    /// Both ends, outward first, which is the order the walk lays them out in.
    pub const BOTH: [Self; 2] = [Self::Outward, Self::Inward];

    /// Which of the two trees a hook is on: the first eight hooks are on the outward tree by the
    /// door, the next eight at the far end. With nothing chosen by hand, the original sixteen finds
    /// each have a hook of their own — variant `n` on hook `n` — so they hang exactly where they
    /// always have, and never change ends when the village mirrors into the other corner.
    pub const fn of_hook(hook: usize) -> Self {
        if hook < TRINKETS_PER_TREE as usize {
            Self::Outward
        } else {
            Self::Inward
        }
    }

    /// This end's place in a two-slot array, so both yards can be resolved once and read back.
    const fn index(self) -> usize {
        match self {
            Self::Outward => 0,
            Self::Inward => 1,
        }
    }
}

/// How many keepsakes one tree carries: half the sixteen hooks each, and the number of anchors
/// `formiga_art::TRINKET_ANCHORS` holds.
pub const TRINKETS_PER_TREE: u8 = (crate::TREE_HOOKS / 2) as u8;

/// A lot on the strip: a dwelling, or one of the two keepsake trees and its yard. The ground in
/// front of the houses is no lot at all — the colony shares it and walks it — and the colony's
/// belongings are scattered inside the trees' yards rather than holding lots of their own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VillageLot {
    /// A tree the colony hangs its finds in and keeps its things under. There is one at each end
    /// of the walk: the outward one sits at a negative position, the inward one past the last
    /// house.
    Tree(TreeEnd),
    Dwelling(usize),
}

/// The air between two lots that are not part of the same thing. Three shelter pixels is a
/// visible seam at every scale the overlay draws at and no more: 0.58.0's five turned into
/// fifty-five pixels of empty lane once the strip was laid end to end.
const VILLAGE_GAP: f32 = 3.0;

/// How far each tree's lot reaches in over the house beside it. The houses grew a quarter in
/// 0.61.0 and the village only about a tenth, so the trees at either end stand a little in over
/// the end houses' ground rather than a seam clear of it; where a tree's drawing and a house's
/// meet, the tree is drawn in front. Six pixels is the shadow and whatever decoration stands on
/// the ground beside the wall, never the wall itself.
pub const TREE_OVERLAP: f32 = 6.0;

/// The ground one belonging claims. Every belonging draws from a 16-pixel quad, but the thing
/// inside it is thirteen or fourteen pixels wide with a transparent margin either side.
pub const OBJECT_WIDTH: f32 = 10.0;

/// The ground one tree's yard claims: the tree itself, the eight keepsakes hung in its branches,
/// and the belongings the colony has scattered around its roots. The tree's own drawing reaches
/// twenty-seven pixels either side of its trunk, so this is a pixel of air past the widest thing
/// in it — the yard costs no ground of its own now that the belongings are split between two of
/// them and each half sits in under the branches.
pub const TREE_WIDTH: f32 = 56.0;

/// How much of a house's own footprint a resting frame may reach across: the outermost pixels of
/// a wall or an eave, and the ground decoration standing against it. Every dwelling keeps its
/// doorway far inside this — the narrowest, a companion's cottage, has more than eighteen pixels
/// between its door's own middle and the edge of its lot, and its doorway is at most five of them
/// — so a resident waiting at its door never stands in front of one.
pub const REST_WALL_SLIVER: f32 = 9.0;

/// The ground a companion standing on the commons claims for itself. Less than the frame it
/// draws, because the outermost pixels either side are a wall's edge or empty air that a
/// neighbour's frame may reach over. The face-clear rule, not this width, is what keeps two
/// companions out of each other's faces.
pub const RESTING_WIDTH: f32 = CREATURE_FRAME_WIDTH - REST_WALL_SLIVER * 2.0;

/// How far the colony house's centre stands from the edge of the display it is tucked into. The
/// outward tree stands on the far side of the house from the cottages, so the corner has to leave
/// its whole lot room or that tree would be drawn half off the screen. The inward tree needs no
/// margin of its own: the strip runs away from the edge, and a region that cannot take the
/// inward end simply does not show it.
const HOME_EDGE_MARGIN: f32 = DwellingKind::Main.width() / 2.0 - TREE_OVERLAP + TREE_WIDTH;

/// What the whole village is allowed to measure end to end, in shelter pixels: a tree at either
/// end and a house for every one of six full-size companions. It is the ground a *full* colony
/// takes, not the ground every colony takes — a village lays out only the houses it has, so a
/// founder on its own is a third of this and grows toward it a house at a time.
///
/// It was 448 until 0.61.0, inherited from 0.58.5's four-companion village, which measured 445
/// with a house and a standing place for each of them; six houses fit inside the same ground once
/// the standing places came out of the strip. The houses then grew a quarter and the ground about
/// a tenth: a full village measures 465, with each tree standing a little in over its end house.
pub const VILLAGE_SPAN_LIMIT: f32 = 468.0;

/// Where the outward tree's lot sits on the walk, counting outward from the colony house: reaching
/// `TREE_OVERLAP` in over the colony house's own footprint, on the negative side of the origin.
/// The inward tree's place depends on how many cottages there are, so the walk works it out as it
/// goes.
const OUTWARD_TREE_CENTRE: f32 =
    -(DwellingKind::Main.width() / 2.0 - TREE_OVERLAP + TREE_WIDTH / 2.0);

/// The ground each kind of lot claims, in shelter pixels.
fn lot_width(lot: VillageLot, cottages: &[DwellingKind]) -> f32 {
    match lot {
        VillageLot::Tree(_) => TREE_WIDTH,
        VillageLot::Dwelling(0) => DwellingKind::Main.width(),
        VillageLot::Dwelling(index) => cottages
            .get(index - 1)
            .map_or(DwellingKind::Cottage.width(), |kind| kind.width()),
    }
}

/// How tall a lot's contents draw, for deciding whether the lot fits its region.
fn lot_height(lot: VillageLot) -> f32 {
    match lot {
        VillageLot::Tree(_) | VillageLot::Dwelling(_) => DWELLING_CELL,
    }
}

/// The most lots a village can ever lay out: two trees and a house for every full-size member.
const MAX_VILLAGE_LOTS: usize = 2 + crate::MAX_COLONY_CREATURES;

/// One ground-line walk outward from the colony house, bookended by a tree at each end. It is the
/// same handful of lots every time, so it is built on the stack: the simulation asks for it
/// several times a tick and a village never has more than ten lots in it.
struct VillageWalk {
    lots: [(VillageLot, f32); MAX_VILLAGE_LOTS],
    len: usize,
}

impl VillageWalk {
    fn iter(&self) -> impl Iterator<Item = (VillageLot, f32)> + '_ {
        self.lots[..self.len].iter().copied()
    }

    fn centre_of(&self, lot: VillageLot) -> Option<f32> {
        self.iter()
            .find(|(candidate, _)| *candidate == lot)
            .map(|(_, centre)| centre)
    }
}

/// The walk: a tree, then the colony house, then a cottage for every later member, then the
/// second tree. Nothing is laid between the houses — they stand a seam apart and the ground in
/// front of the whole row belongs to the colony. Lots which cannot fit the house's accessible
/// region stay stored but hidden, rather than spilling elsewhere — and because the trees are the
/// outermost lots at both ends, a corner that runs out of ground gives up a tree before a house.
///
/// The colony's belongings are not on the strip at all: they live in the two trees' yards, which
/// is what took about a hundred shelter pixels out of the village.
fn village_walk(cottages: &[DwellingKind]) -> VillageWalk {
    let mut walk = VillageWalk {
        lots: [(VillageLot::Tree(TreeEnd::Outward), OUTWARD_TREE_CENTRE); MAX_VILLAGE_LOTS],
        len: 1,
    };
    let mut push = |lot: VillageLot, centre: f32| {
        walk.lots[walk.len] = (lot, centre);
        walk.len += 1;
    };
    push(VillageLot::Dwelling(0), 0.0);
    let mut edge = DwellingKind::Main.width() / 2.0;
    for house in 1..=cottages.len().min(crate::MAX_COLONY_CREATURES - 1) {
        // Houses stand a seam apart, and nothing else is laid between them. The ground in front
        // of the whole row is the colony's own, walked rather than parcelled out.
        edge += VILLAGE_GAP;
        let width = lot_width(VillageLot::Dwelling(house), cottages);
        push(VillageLot::Dwelling(house), edge + width / 2.0);
        edge += width;
    }
    // The far wall: the second tree closes the strip off, a little in over the last house.
    push(
        VillageLot::Tree(TreeEnd::Inward),
        edge - TREE_OVERLAP + TREE_WIDTH / 2.0,
    );
    walk
}

/// How far the whole village reaches, in shelter pixels, measured end to end: from the outer edge
/// of one tree's yard to the outer edge of the other's.
pub fn village_span(cottages: &[DwellingKind]) -> f32 {
    let (low, high) =
        village_walk(cottages)
            .iter()
            .fold((f32::MAX, f32::MIN), |(low, high), (lot, centre)| {
                let half = lot_width(lot, cottages) / 2.0;
                ((centre - half).min(low), (centre + half).max(high))
            });
    high - low
}

/// Where each belonging sits: which yard it is kept in, and `(toward, forward)` in shelter pixels
/// from that tree's own middle — positive toward is a step toward the houses, positive forward is
/// a step back from the ground line and negative a step in front of it. The inward yard mirrors,
/// so `toward` means the same thing at both ends and the two yards read as a matched pair.
///
/// The ends alternate by slot, so a colony with three things has two at one end and one at the
/// other rather than a full yard and an empty one, and so a belonging cannot change ends when the
/// next one arrives: the slot a belonging holds is the slot it keeps.
///
/// Scattered rather than shelved. Some sit close in against the roots, some further out under the
/// branches, one tucked almost behind the trunk — what a yard somebody actually keeps things in
/// looks like, rather than a row of exhibits. Every one of them is now inside the spread of its
/// own tree's roots rather than out on the edge of the lot reading as a thing of its own, and no
/// two in a yard come within `BELONGING_CLEARANCE` even after the per-colony drift below.
const BELONGING_SPOTS: [(TreeEnd, f32, f32); crate::MAX_COLONY_OBJECTS] = [
    (TreeEnd::Outward, 11.0, -1.0),
    (TreeEnd::Inward, 6.0, 6.0),
    (TreeEnd::Outward, -6.0, 4.0),
    (TreeEnd::Inward, -11.0, 0.0),
    (TreeEnd::Outward, 3.0, 6.0),
    (TreeEnd::Inward, 14.0, -1.0),
    (TreeEnd::Outward, -15.0, 0.0),
    (TreeEnd::Inward, -3.0, 4.0),
];

/// The closest two belongings may end up, in shelter pixels, once the drift has moved them. A
/// belonging draws thirteen or fourteen pixels wide, so at this distance they overlap at the
/// edges and each one still shows its own middle.
pub const BELONGING_CLEARANCE: f32 = 6.0;

/// How far a belonging may stand behind, or a pixel in front of, the village's ground line.
pub const BELONGING_DEPTH: f32 = 7.0;

/// Deterministic per-colony scatter, so two colonies never keep their yard the same way. One
/// pixel either way and one pixel of depth is enough to break the arrangement up without letting
/// anything land on its neighbour.
fn object_drift(detail_seed: u64, slot: usize) -> (f32, f32) {
    let bits = (detail_seed >> ((slot % 12) * 5)) & 0x1f;
    ((bits & 0x3).min(2) as f32 - 1.0, ((bits >> 2) & 0x1) as f32)
}

/// Which yard belonging `slot` is kept in, and where in it: in shelter pixels along the walk from
/// that tree's middle, and off the ground line. The inward yard is the mirror of the outward one,
/// so a spot written down as a step toward the houses is a step toward them at either end.
fn belonging_offset(slot: usize, detail_seed: u64) -> (TreeEnd, f32, f32) {
    let (end, toward, forward) = BELONGING_SPOTS[slot % BELONGING_SPOTS.len()];
    let (drift_x, drift_lift) = object_drift(detail_seed, slot);
    let sideways = match end {
        TreeEnd::Outward => toward + drift_x,
        TreeEnd::Inward => -(toward + drift_x),
    };
    (end, sideways, forward + drift_lift)
}

/// The village's frame of reference on the display it belongs to: where its ground line starts,
/// which way the strip runs, how many desktop points one shelter pixel covers, and the room it
/// has to run into.
struct VillageGround<'a> {
    monitor: &'a MonitorInfo,
    anchor: Point,
    scale: f32,
    direction: f32,
    /// The one accessible region the village stands in, resolved with the anchor rather than kept
    /// as a list and searched again for every lot.
    region: Option<DesktopRect>,
}

impl<'a> VillageGround<'a> {
    fn resolve(
        home: &ColonyHome,
        monitors: &'a [MonitorInfo],
        policy: &HabitatPolicy,
        display_scale: u8,
    ) -> Option<Self> {
        let monitor = village_monitor(home, monitors)?;
        // One pass over the habitat's regions answers both questions the ground has: where the
        // corner ends up, and which region it ended up in. Asking twice used to cost this two
        // fresh lists of rectangles on a path the simulation walks several times a tick.
        let regions = accessible_regions(policy, monitor);
        let desired = home_anchor(home, monitor, display_scale);
        let (_, anchor) = regions
            .iter()
            .map(|region| {
                let point = Point {
                    x: desired.x.clamp(region.x + 8.0, region.right() - 8.0),
                    y: region.bottom() - 4.0,
                };
                (desired.distance(point), point)
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))?;
        Some(Self {
            monitor,
            anchor,
            scale: f32::from(display_scale) / monitor.scale_factor.max(1.0),
            direction: if home.corner == HomeCorner::BottomLeft {
                1.0
            } else {
                -1.0
            },
            region: regions.into_iter().find(|region| {
                anchor.x >= region.x
                    && anchor.x <= region.right()
                    && anchor.y >= region.y
                    && anchor.y <= region.bottom()
            }),
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
        self.region
    }

    /// Whether a lot that wide and that tall fits the region the village stands in.
    fn fits(&self, point: Point, half: f32, height: f32) -> bool {
        self.region.is_some_and(|region| {
            point.x - half * self.scale >= region.x
                && point.x + half * self.scale <= region.right()
                && point.y - height * self.scale >= region.y
                && point.y <= region.bottom()
        })
    }

    /// Where a lot's contents stand, if the strip has room for the lot here.
    fn place(&self, lot: VillageLot, centre: f32, cottages: &[DwellingKind]) -> Option<Point> {
        let point = self.point(centre, 0.0);
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
    let centre = village_walk(cottages).centre_of(lot)?;
    let point = ground.place(lot, centre, cottages)?;
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

/// The run of ground the colony has to itself while it is home: from the outward tree's outer
/// edge to the inward one's, along the village's own ground line. The houses stand at the back of
/// it and the trees close it at either end, so this is what a companion may walk without leaving
/// the village — and what it roams when it has nothing else to do.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HomeCommons {
    pub monitor_id: u64,
    /// The ground line every companion and every house stands on.
    pub ground_y: f32,
    /// The ends of the walk, in ascending screen order whichever corner the village is in.
    pub low_x: f32,
    pub high_x: f32,
    /// The part of it a companion may stand on: the same walk without the two trees' yards,
    /// which are full of the colony's own belongings.
    pub stand_low_x: f32,
    pub stand_high_x: f32,
    /// Shelter pixels to desktop points here, so callers can measure in the village's own units.
    pub scale: f32,
}

impl HomeCommons {
    /// How wide the walk is, in points.
    pub fn width(&self) -> f32 {
        (self.high_x - self.low_x).max(0.0)
    }

    /// A point a companion may stand on, given as a fraction from one end of the walk to the
    /// other. Kept out of the trees' yards, and off the very ends, so a companion standing there
    /// is inside the village and not in its belongings.
    pub fn along(&self, fraction: f32) -> Point {
        let (low, high) = self.standing_span();
        Point {
            x: low + (high - low) * fraction.clamp(0.0, 1.0),
            y: self.ground_y,
        }
    }

    /// The first and last places along the walk a companion may stand, as `along` measures them.
    pub fn standing_span(&self) -> (f32, f32) {
        let half = RESTING_WIDTH / 2.0 * self.scale;
        let (low, high) = (self.stand_low_x + half, self.stand_high_x - half);
        if high < low {
            // A village too cramped to keep its yards clear: the whole walk is fair ground.
            let low = self.low_x + half;
            (low, (self.high_x - half).max(low))
        } else {
            (low, high)
        }
    }
}

/// The colony's own ground: the whole strip between the two trees, whatever of it this corner
/// can show. A village with no room for even one companion to stand has no commons.
pub fn home_commons(
    home: &ColonyHome,
    cottages: &[DwellingKind],
    monitors: &[MonitorInfo],
    policy: &HabitatPolicy,
    display_scale: u8,
) -> Option<HomeCommons> {
    let ground = VillageGround::resolve(home, monitors, policy, display_scale)?;
    let region = ground.region()?;
    let (mut low, mut high) = (f32::MAX, f32::MIN);
    for (lot, centre) in village_walk(cottages).iter() {
        let Some(point) = ground.place(lot, centre, cottages) else {
            continue;
        };
        let half = lot_width(lot, cottages) / 2.0 * ground.scale;
        low = low.min(point.x - half);
        high = high.max(point.x + half);
    }
    if low > high {
        return None;
    }
    let low = low.max(region.x);
    let high = high.min(region.right());
    // The frontage: the ground in front of the houses themselves. The yards at either end
    // belong to the trees and to the belongings scattered under them, so a companion standing
    // still keeps off them — while roaming it may walk the whole thing.
    let (mut front_low, mut front_high) = (f32::MAX, f32::MIN);
    for (lot, centre) in village_walk(cottages).iter() {
        let VillageLot::Dwelling(_) = lot else {
            continue;
        };
        let Some(point) = ground.place(lot, centre, cottages) else {
            continue;
        };
        let half = lot_width(lot, cottages) / 2.0 * ground.scale;
        front_low = front_low.min(point.x - half);
        front_high = front_high.max(point.x + half);
    }
    (high - low >= RESTING_WIDTH * ground.scale).then_some(HomeCommons {
        monitor_id: ground.monitor.id,
        ground_y: ground.anchor.y,
        low_x: low,
        high_x: high,
        stand_low_x: front_low.clamp(low, high),
        stand_high_x: front_high.clamp(low, high),
        scale: ground.scale,
    })
}

/// Where the `slot`-th of `of` companions settles when the colony is home and nobody is walking
/// anywhere: spread evenly along the commons, far enough apart that no face is behind anybody.
///
/// These are resting places on shared ground, not addresses. A companion is free to wander off
/// one and come back to another, and with a full village of six there is no arrangement that
/// leaves every doorway clear — the colony lives in front of its houses, which is what a village
/// looks like.
pub fn home_resting_position(
    home: &ColonyHome,
    slot: usize,
    of: usize,
    cottages: &[DwellingKind],
    monitors: &[MonitorInfo],
    policy: &HabitatPolicy,
    display_scale: u8,
) -> Option<(u64, Point)> {
    let commons = home_commons(home, cottages, monitors, policy, display_scale)?;
    let of = of.max(1);
    if slot >= of {
        return None;
    }
    let clear = CREATURE_FRAME_WIDTH * REST_CLEAR_RATIO * commons.scale;
    let half = RESTING_WIDTH / 2.0 * commons.scale;
    let wanted = (of as f32 - 1.0) * clear;
    // The frontage first — the ground in front of the houses, which leaves the yards to the
    // trees and to what the colony keeps under them. A colony too big to stand there face-clear
    // spills onto the whole walk instead, yards and all; that is what the yards are for.
    let frontage = commons.stand_high_x - commons.stand_low_x - half * 2.0;
    let (low, high) = if frontage >= wanted {
        (commons.stand_low_x, commons.stand_high_x)
    } else {
        (commons.low_x, commons.high_x)
    };
    let usable = (high - low - half * 2.0).max(0.0);
    // Even spacing, never wider than face-clear and never so tight that two companions share a
    // spot: a commons with no room left still gives everybody a place of their own on it.
    let step = if of > 1 {
        (usable / (of as f32 - 1.0)).min(clear)
    } else {
        0.0
    };
    let spread = step * (of as f32 - 1.0);
    let first = low + half + (usable - spread).max(0.0) / 2.0;
    Some((
        commons.monitor_id,
        Point {
            x: first + slot as f32 * step,
            y: commons.ground_y,
        },
    ))
}

/// Where a visitor stands: on the village ground line just past everything the colony actually
/// shows — the outermost house, tree or belonging that fits here, and any resident who had to
/// stand past the strip — so a guest is beside the village rather than in front of it or on top
/// of somebody. A corner with no room left for a guest to keep its distance has no guest spot.
pub fn home_guest_position(
    home: &ColonyHome,
    cottages: &[DwellingKind],
    // How many belongings the colony has. It no longer moves the guest: belongings live in the
    // trees' yards, and a yard is inside its own tree's lot, which the walk already accounts for.
    _objects: usize,
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
    let mut outer = ground.anchor.x;
    for (lot, centre) in village_walk(cottages).iter() {
        let Some(point) = ground.place(lot, centre, cottages) else {
            continue;
        };
        let reach = point.x + ground.direction * lot_width(lot, cottages) / 2.0 * ground.scale;
        outer = ground.outward_max(outer, reach);
    }
    // A guest keeps clear of a full colony's worth of resting places, whether or not every one
    // of them is taken: where the village *could* seat somebody is not ground for a visitor.
    let mut resting = [None; crate::MAX_COLONY_CREATURES];
    let residents = crate::MAX_COLONY_CREATURES;
    for (slot, spot) in resting.iter_mut().enumerate() {
        *spot = home_resting_position(
            home,
            slot,
            residents,
            cottages,
            monitors,
            policy,
            display_scale,
        )
        .map(|(_, point)| point);
        if let Some(point) = spot {
            outer = ground.outward_max(outer, point.x + ground.direction * half);
        }
    }
    let desired = outer + ground.direction * (VILLAGE_GAP * ground.scale + half);
    let point = Point {
        x: desired.clamp(region.x + half, region.right() - half),
        y: ground.anchor.y,
    };
    let clear = CREATURE_FRAME_WIDTH * REST_CLEAR_RATIO * ground.scale;
    resting
        .iter()
        .flatten()
        .all(|resident| (point.x - resident.x).abs() >= clear - 0.01)
        .then_some((ground.monitor.id, point))
}

/// The companion houses of a colony, on the stack: at most three, in colony order.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cottages {
    kinds: [DwellingKind; crate::MAX_COLONY_CREATURES - 1],
    len: usize,
}

impl Cottages {
    pub fn as_slice(&self) -> &[DwellingKind] {
        &self.kinds[..self.len]
    }
}

/// Companion houses for the colony, in stable colony order, without touching the allocator. The
/// first full-size member shares the colony house, every later full-size arrival gets a cottage
/// of its own, and a mini lives in its big version's, so the corner grows into a small village as
/// the colony does.
///
/// A colony is never more than six, so the six smallest `(colony_order, id)` keys are picked out
/// one at a time rather than by sorting the whole list into a fresh `Vec` — which this used to do
/// twice, on a path the simulation walks for every belonging on every tick.
pub fn colony_cottage_list(creatures: &[Creature]) -> Cottages {
    let mut cottages = Cottages::default();
    let mut taken: Option<(u8, crate::CreatureId)> = None;
    let mut houses = 0;
    for _ in 0..crate::MAX_COLONY_CREATURES {
        let Some(next) = creatures
            .iter()
            .filter(|creature| taken.is_none_or(|last| (creature.colony_order, creature.id) > last))
            .min_by_key(|creature| (creature.colony_order, creature.id))
        else {
            break;
        };
        taken = Some((next.colony_order, next.id));
        // A house belongs to whoever arrived full-size. A mini has no house of its own: it lives
        // in the one its big version already keeps, which is why the village grows a house at a
        // time rather than one per companion.
        if !next.role.is_adult() {
            continue;
        }
        // The founder shares the colony house; every later full-size arrival gets one of its own.
        if houses > 0 {
            cottages.kinds[houses - 1] = DwellingKind::Cottage;
            cottages.len = houses;
        }
        houses += 1;
    }
    cottages
}

/// Who keeps each house, in the order they stand along the village: the founder in the colony
/// house, then the cottages as the owner arranged them, and any cottage never arranged in the
/// order its keeper arrived. Only full-size companions keep a house; a mini lives in its big
/// version's. Held in place, like `Cottages`, since the homebound walk asks every tick.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HouseOwners {
    ids: [crate::CreatureId; crate::MAX_COLONY_CREATURES],
    len: usize,
}

impl HouseOwners {
    pub fn as_slice(&self) -> &[crate::CreatureId] {
        &self.ids[..self.len]
    }

    fn push(&mut self, id: crate::CreatureId) {
        if self.len < self.ids.len() && !self.as_slice().contains(&id) {
            self.ids[self.len] = id;
            self.len += 1;
        }
    }
}

pub fn house_owners(creatures: &[Creature], cottage_order: &[crate::CreatureId]) -> HouseOwners {
    let mut owners = HouseOwners::default();
    let adults = || creatures.iter().filter(|creature| creature.role.is_adult());
    let Some(founder) = adults().min_by_key(|creature| (creature.colony_order, creature.id)) else {
        return owners;
    };
    owners.push(founder.id);
    for id in cottage_order {
        if adults().any(|creature| creature.id == *id) {
            owners.push(*id);
        }
    }
    // Everyone else in the order they arrived, picked in place.
    let mut taken: Option<(u8, crate::CreatureId)> = None;
    while let Some(next) = adults()
        .filter(|creature| taken.is_none_or(|last| (creature.colony_order, creature.id) > last))
        .min_by_key(|creature| (creature.colony_order, creature.id))
    {
        taken = Some((next.colony_order, next.id));
        owners.push(next.id);
    }
    owners
}

/// Which house a companion belongs to, counting the colony house as slot zero. A mini takes its
/// big version's. The founder keeps the colony house; the cottages stand in the order the owner
/// arranged them, or the order their keepers arrived.
pub fn house_slot_for(
    creature: &Creature,
    creatures: &[Creature],
    cottage_order: &[crate::CreatureId],
) -> usize {
    let of_interest = match creature.role {
        CreatureRole::Mini { parent_id } => creatures
            .iter()
            .find(|candidate| candidate.id == parent_id)
            .unwrap_or(creature),
        CreatureRole::Adult => creature,
    };
    let owners = house_owners(creatures, cottage_order);
    let owners = owners.as_slice();
    if let Some(slot) = owners.iter().position(|id| *id == of_interest.id) {
        return slot;
    }
    // A mini whose big version has gone: the house of the last companion to arrive before it.
    let before = owners
        .iter()
        .filter(|id| {
            creatures.iter().any(|candidate| {
                candidate.id == **id
                    && (candidate.colony_order, candidate.id)
                        < (of_interest.colony_order, of_interest.id)
            })
        })
        .count();
    before.saturating_sub(1)
}

/// The same houses as `colony_cottage_list`, for callers that want them owned.
pub fn colony_cottages(creatures: &[Creature]) -> Vec<DwellingKind> {
    colony_cottage_list(creatures).as_slice().to_vec()
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

/// Where one of the two keepsake trees stands. The outward one is on the side of the colony house
/// away from the cottages — left of the houses in a bottom-left village, and mirrored to their
/// right in a bottom-right one; the inward one closes the strip off past the last house. A region
/// with no room for a tree shows no tree at that end; the houses never move aside to make room
/// for one, and a corner that can take only one of them still reads as a village.
pub fn home_tree_position(
    home: &ColonyHome,
    end: TreeEnd,
    cottages: &[DwellingKind],
    monitors: &[MonitorInfo],
    policy: &HabitatPolicy,
    display_scale: u8,
) -> Option<(u64, Point)> {
    village_position(
        home,
        VillageLot::Tree(end),
        cottages,
        monitors,
        policy,
        display_scale,
    )
}

/// Where belonging `slot` is drawn: in one of the two trees' yards, scattered around the trunk
/// and roots rather than shelved along the strip. Which yard is fixed by the slot, so a thing
/// never hops ends when another arrives. A colony keeps its things where its trees are, so a
/// belonging appears exactly when its own tree does — a corner too narrow for that end has no
/// yard there to put anything in.
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
    home_object_positions(home, cottages, monitors, policy, display_scale)[slot]
}

/// Every belonging's place in the two yards, resolved in one go. Anything that wants more than
/// one of them — the simulation reconciling the colony's things every tick, the overlay building
/// its quads — asks for this rather than paying to resolve the village eight times over. Both
/// yards are found once here as well, rather than once per belonging.
pub fn home_object_positions(
    home: &ColonyHome,
    cottages: &[DwellingKind],
    monitors: &[MonitorInfo],
    policy: &HabitatPolicy,
    display_scale: u8,
) -> [Option<(u64, Point)>; crate::MAX_COLONY_OBJECTS] {
    let mut places = [None; crate::MAX_COLONY_OBJECTS];
    let Some(ground) = VillageGround::resolve(home, monitors, policy, display_scale) else {
        return places;
    };
    let walk = village_walk(cottages);
    // A yard is the ground its own tree stands on, so an end the region cannot take has nowhere
    // to put anything down.
    let mut yards = [None; TreeEnd::BOTH.len()];
    for end in TreeEnd::BOTH {
        let lot = VillageLot::Tree(end);
        yards[end.index()] = walk
            .centre_of(lot)
            .filter(|centre| ground.place(lot, *centre, cottages).is_some());
    }
    for (slot, place) in places.iter_mut().enumerate() {
        let (end, sideways, forward) = belonging_offset(slot, home.shelter.detail_seed);
        let Some(centre) = yards[end.index()] else {
            continue;
        };
        *place = Some((ground.monitor.id, ground.point(centre + sideways, forward)));
    }
    places
}

/// How far above the village ground line the top of a house reaches, in shelter pixels: where a
/// companion sitting on its roof has its feet. Measured the way the house is drawn — its size in
/// twenty-fourths of the colony house as it was drawn until 0.61.0, the dwelling's own
/// proportions from the colony's shelter genome, and where each type's roof actually tops out —
/// and held to the drawing by a test in the art crate. The ground line is three rows below where
/// a house's walls stand in its cell.
pub fn house_roof_height(
    shelter: &crate::ShelterGenome,
    style: crate::ShelterStyle,
    colony_house: bool,
) -> f32 {
    let span = if colony_house { 30 } else { 25 };
    let height = (i32::from(shelter.height).clamp(27, 36) * span / 24).max(13);
    let unit = |value: i32| (value * span / 24).max(1);
    let top = match style {
        // The canvas comes to its apex a little below the top of the pole.
        crate::ShelterStyle::Tent => height - unit(3) - 2,
        crate::ShelterStyle::Mushroom => height,
        // The roof pillow puffs up a few pixels above the walls.
        crate::ShelterStyle::PillowFort => height + 3,
        // The ridge of leaves sits a little below the house's full height.
        crate::ShelterStyle::LeafHouse => height - 2,
    };
    (top + 3) as f32
}

/// How wide a hangout spot stands on the ground, in shelter pixels: the art's own cell.
pub const HANGOUT_WIDTH: f32 = 16.0;

/// Something put down on the village ground by the person at the desk.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GroundItem {
    Hangout(HangoutKind),
    Garden(GardenKind),
    Ornament(OrnamentKind),
}

impl GroundItem {
    /// A stable order for items that land on the same spot, so a tie always breaks one way.
    const fn rank(self) -> u8 {
        match self {
            Self::Hangout(kind) => kind.index(),
            Self::Garden(kind) => 32 + kind.index(),
            Self::Ornament(kind) => 64 + kind.index(),
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Hangout(kind) => kind.label(),
            Self::Garden(kind) => kind.label(),
            Self::Ornament(kind) => kind.label(),
        }
    }
}

/// Where everything put down on the village ground stands, resolved in one go: hangout spots and
/// garden patches each at its own fraction of the ground a companion may stand on, and none
/// nearer another than its own width and a little more, so two never sit on top of one another
/// however they were placed. A village with no ground puts nothing down, and a ground too short
/// for all of them keeps the ones that fit.
pub fn home_ground_positions(
    home: &ColonyHome,
    cottages: &[DwellingKind],
    monitors: &[MonitorInfo],
    policy: &HabitatPolicy,
    display_scale: u8,
) -> Vec<(GroundItem, u64, Point)> {
    if !home.has_ground_items() {
        return Vec::new();
    }
    let Some(commons) = home_commons(home, cottages, monitors, policy, display_scale) else {
        return Vec::new();
    };
    let (low, high) = commons.standing_span();
    let gap = (HANGOUT_WIDTH + 2.0) * commons.scale;
    let at = |along: f32| low + (high - low) * along.clamp(0.0, 1.0);
    let mut spots: Vec<(GroundItem, f32)> = home
        .hangouts
        .iter()
        .map(|spot| (GroundItem::Hangout(spot.kind), at(spot.along)))
        .chain(
            home.gardens
                .iter()
                .map(|patch| (GroundItem::Garden(patch.kind), at(patch.along))),
        )
        .chain(
            home.ornaments
                .iter()
                .map(|spot| (GroundItem::Ornament(spot.kind), at(spot.along))),
        )
        .collect();
    spots.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.rank().cmp(&b.0.rank())));
    // Pushed apart from the left, then back inside the right-hand end.
    for index in 1..spots.len() {
        spots[index].1 = spots[index].1.max(spots[index - 1].1 + gap);
    }
    if let Some(last) = spots.last_mut() {
        last.1 = last.1.min(high);
    }
    for index in (0..spots.len().saturating_sub(1)).rev() {
        spots[index].1 = spots[index].1.min(spots[index + 1].1 - gap);
    }
    spots
        .into_iter()
        .filter(|(_, x)| *x >= low - 0.01)
        .map(|(item, x)| {
            (
                item,
                commons.monitor_id,
                Point {
                    x,
                    y: commons.ground_y,
                },
            )
        })
        .collect()
}

/// Where every hangout spot stands: the hangout spots among everything on the ground.
pub fn home_hangout_positions(
    home: &ColonyHome,
    cottages: &[DwellingKind],
    monitors: &[MonitorInfo],
    policy: &HabitatPolicy,
    display_scale: u8,
) -> Vec<(HangoutKind, u64, Point)> {
    home_ground_positions(home, cottages, monitors, policy, display_scale)
        .into_iter()
        .filter_map(|(item, monitor_id, point)| match item {
            GroundItem::Hangout(kind) => Some((kind, monitor_id, point)),
            GroundItem::Garden(_) | GroundItem::Ornament(_) => None,
        })
        .collect()
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

    /// The widest village there is: four adults, every house at its full footprint. Every rule
    /// about ground and clearance is tightest here, so this is the colony the layout is checked
    /// against rather than every shape it can take.
    /// The village at its widest: a house for every one of six full-size companions.
    fn widest_colony() -> Vec<DwellingKind> {
        vec![DwellingKind::Cottage; crate::MAX_COLONY_CREATURES - 1]
    }

    /// A colony part-way there, which should take correspondingly less ground.
    fn small_colony() -> Vec<DwellingKind> {
        vec![DwellingKind::Cottage; 2]
    }

    /// The tightest drawing the overlay does: the largest scale on the display that magnifies it
    /// least, so one shelter pixel is four desktop points and every clearance is at its coarsest.
    const TIGHTEST_SCALE: u8 = 4;

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
            .iter()
            .filter_map(|(lot, _)| {
                let point = match lot {
                    VillageLot::Tree(end) => {
                        home_tree_position(home, end, cottages, monitors, &policy, display_scale)
                    }
                    VillageLot::Dwelling(slot) => home_dwelling_position(
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
                Some((
                    lot,
                    (point.x - anchor.x) * direction,
                    lot_width(lot, cottages) / 2.0 * unit,
                ))
            })
            .collect()
    }

    /// The houses and both trees come out of one walk, so no two houses may ever claim the same
    /// ground and a tree reaches in over its end house by `TREE_OVERLAP` and no further — and the
    /// two corners have to be the same village, measured from the house outward.
    #[test]
    fn the_houses_and_the_trees_share_one_walk_and_mirror_in_both_corners() {
        let monitor = wide_monitor(2.0);
        let cottages = widest_colony();
        let mut mirrored: Option<Vec<(VillageLot, f32, f32)>> = None;
        for corner in [HomeCorner::BottomLeft, HomeCorner::BottomRight] {
            let home = village_home(&monitor, corner);
            let lots = placed_lots(&home, &cottages, &monitor, TIGHTEST_SCALE);
            assert_eq!(
                lots.len(),
                3 + cottages.len(),
                "two trees and a house each should all fit a display this size"
            );
            // The bookends: one past the colony house away from the cottages, one past the last
            // cottage, with every house between them.
            let tree_of = |end| {
                lots.iter()
                    .find(|(lot, _, _)| *lot == VillageLot::Tree(end))
                    .map(|(_, centre, _)| *centre)
                    .expect("both trees fit a display this size")
            };
            let (outward, inward) = (tree_of(TreeEnd::Outward), tree_of(TreeEnd::Inward));
            assert!(
                outward < -DwellingKind::Main.width() / 2.0,
                "the outward tree stands past the colony house, away from the cottages"
            );
            for (lot, centre, _) in &lots {
                if matches!(lot, VillageLot::Tree(_)) {
                    continue;
                }
                assert!(
                    (outward..=inward).contains(centre),
                    "{lot:?} stands outside the two trees"
                );
            }
            let unit = f32::from(TIGHTEST_SCALE) / monitor.scale_factor.max(1.0);
            for (index, (lot, centre, half)) in lots.iter().enumerate() {
                for (other, other_centre, other_half) in &lots[index + 1..] {
                    let clearance = (centre - other_centre).abs();
                    let tree_over_house = matches!(
                        (lot, other),
                        (VillageLot::Tree(_), VillageLot::Dwelling(_))
                            | (VillageLot::Dwelling(_), VillageLot::Tree(_))
                    );
                    let overlap = if tree_over_house {
                        TREE_OVERLAP * unit
                    } else {
                        0.0
                    };
                    assert!(
                        clearance >= half + other_half - overlap - 0.01,
                        "{corner:?}: {lot:?} and {other:?} share ground ({clearance} apart, \
                         {half} + {other_half} wide)"
                    );
                }
            }
            match &mirrored {
                None => mirrored = Some(lots),
                Some(first) => {
                    assert_eq!(first.len(), lots.len());
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

    /// The whole village, both trees and both yards included, has to fit the ground a corner can
    /// spare. The widest colony is the measurement that matters; the rest are smaller by
    /// construction.
    #[test]
    fn the_whole_village_including_both_trees_fits_the_width_it_is_allowed() {
        let span = village_span(&widest_colony());
        assert!(
            span <= VILLAGE_SPAN_LIMIT,
            "the widest village spans {span} shelter pixels"
        );
        assert!(
            village_span(&[]) < span,
            "a founder on its own should not take more ground than a village of six"
        );
        // And on a real display, at the scale that magnifies it most.
        let monitor = wide_monitor(1.0);
        let home = village_home(&monitor, HomeCorner::BottomLeft);
        let lots = placed_lots(&home, &widest_colony(), &monitor, TIGHTEST_SCALE);
        let (low, high) = lots
            .iter()
            .fold((f32::MAX, f32::MIN), |(low, high), (_, centre, half)| {
                ((centre - half).min(low), (centre + half).max(high))
            });
        let unit = f32::from(TIGHTEST_SCALE);
        assert!(
            high - low <= VILLAGE_SPAN_LIMIT * unit + 0.01,
            "the village covers {} points, more than the {} it is allowed",
            high - low,
            VILLAGE_SPAN_LIMIT * unit
        );
    }

    /// Where the colony settles when it is standing still. The houses stand shoulder to shoulder
    /// now and the ground in front of them is shared, so a resting place is a spot on the commons
    /// rather than a parcel beside one door: everybody has to be on that ground, on its line, and
    /// far enough from everybody else that no face is behind another. Keeping out of the doorways
    /// is a preference a roaming companion applies when there is somewhere to apply it, not a
    /// promise the arrangement can keep once six houses fill the village.
    #[test]
    fn every_resting_companion_stands_on_the_commons_and_clear_of_the_others() {
        let policy = HabitatPolicy::default();
        let monitor = wide_monitor(1.0);
        let monitors = std::slice::from_ref(&monitor);
        let unit = f32::from(TIGHTEST_SCALE);
        for cottages in [small_colony(), widest_colony()] {
            let residents = crate::MAX_COLONY_CREATURES;
            for corner in [HomeCorner::BottomLeft, HomeCorner::BottomRight] {
                let home = village_home(&monitor, corner);
                let commons =
                    home_commons(&home, &cottages, monitors, &policy, TIGHTEST_SCALE).unwrap();
                let resting: Vec<Point> = (0..residents)
                    .map(|slot| {
                        home_resting_position(
                            &home,
                            slot,
                            residents,
                            &cottages,
                            monitors,
                            &policy,
                            TIGHTEST_SCALE,
                        )
                        .unwrap()
                        .1
                    })
                    .collect();
                let half = RESTING_WIDTH / 2.0 * unit;
                for (slot, point) in resting.iter().enumerate() {
                    assert!(
                        point.x >= commons.low_x - 0.01 && point.x <= commons.high_x + 0.01,
                        "{corner:?}: companion {slot} settled off the commons"
                    );
                    assert!(
                        (point.y - commons.ground_y).abs() < 0.01,
                        "{corner:?}: companion {slot} left the ground line"
                    );
                    assert!(
                        commons.width() >= half * 2.0,
                        "{corner:?}: the commons is too narrow to stand on"
                    );
                }
                let clear = CREATURE_FRAME_WIDTH * REST_CLEAR_RATIO * unit;
                for (first, one) in resting.iter().enumerate() {
                    for other in resting.iter().skip(first + 1) {
                        assert!(
                            (one.x - other.x).abs() >= clear - 0.01,
                            "{corner:?}: two companions settled {} apart, closer than {clear}",
                            (one.x - other.x).abs()
                        );
                    }
                }
            }
        }
    }

    /// Everything the colony owns lies in one of the two trees' yards, split between the ends:
    /// never on a house, never on the tree itself, and never under the feet of the nearest
    /// resident, who is standing out on the shared ground in front of the row.
    #[test]
    fn the_belongings_split_between_both_yards_without_landing_on_anything() {
        let policy = HabitatPolicy::default();
        let monitor = wide_monitor(1.0);
        let monitors = std::slice::from_ref(&monitor);
        let cottages = widest_colony();
        let unit = f32::from(TIGHTEST_SCALE);
        for corner in [HomeCorner::BottomLeft, HomeCorner::BottomRight] {
            let home = village_home(&monitor, corner);
            let places = home_object_positions(&home, &cottages, monitors, &policy, TIGHTEST_SCALE);
            assert_eq!(
                places.iter().flatten().count(),
                crate::MAX_COLONY_OBJECTS,
                "a display this size has room for both yards"
            );
            // Sorted into the yard each slot belongs to, so neither end carries everything.
            let mut yards: [Vec<Point>; TreeEnd::BOTH.len()] = Default::default();
            for end in TreeEnd::BOTH {
                let (_, tree) =
                    home_tree_position(&home, end, &cottages, monitors, &policy, TIGHTEST_SCALE)
                        .unwrap();
                for (slot, place) in places.iter().enumerate() {
                    if belonging_offset(slot, home.shelter.detail_seed).0 != end {
                        continue;
                    }
                    let thing = place.unwrap().1;
                    assert!(
                        (thing.y - tree.y).abs() <= BELONGING_DEPTH * unit + 0.01,
                        "belonging {slot} floats {} points off the ground line",
                        (thing.y - tree.y).abs()
                    );
                    assert!(
                        (thing.x - tree.x).abs() + OBJECT_WIDTH / 2.0 * unit
                            <= TREE_WIDTH / 2.0 * unit + 0.01,
                        "belonging {slot} spilled out of the {end:?} yard"
                    );
                    yards[end.index()].push(thing);
                }
                assert_eq!(
                    yards[end.index()].len(),
                    crate::MAX_COLONY_OBJECTS / 2,
                    "{end:?} should keep half the colony's things"
                );
            }
            // Clearance is a rule inside a yard: the two yards are a village apart.
            for (end, yard) in yards.iter().enumerate() {
                for (index, thing) in yard.iter().enumerate() {
                    for (other, second) in yard.iter().enumerate().skip(index + 1) {
                        let apart = thing.distance(*second);
                        assert!(
                            apart >= BELONGING_CLEARANCE * unit - 0.01,
                            "yard {end}: belongings {index} and {other} are {apart} apart, {} \
                             needed",
                            BELONGING_CLEARANCE * unit
                        );
                    }
                }
            }
            // Nobody rests in a yard. The founder is nearest the outward one, the last member
            // nearest the inward one.
            let reach = (CREATURE_FRAME_WIDTH + OBJECT_WIDTH) / 2.0 * unit;
            let residents = crate::MAX_COLONY_CREATURES;
            for slot in 0..residents {
                let (_, rest) = home_resting_position(
                    &home,
                    slot,
                    residents,
                    &cottages,
                    monitors,
                    &policy,
                    TIGHTEST_SCALE,
                )
                .unwrap();
                for (index, thing) in places.iter().flatten().enumerate() {
                    assert!(
                        (thing.1.x - rest.x).abs() >= reach,
                        "{corner:?}: member {slot} stands on belonging {index}"
                    );
                }
            }
        }
        // Everything that wants the whole yard now asks for it in one go. The batch and the
        // single lookup have to keep agreeing, slot for slot.
        let home = village_home(&monitor, HomeCorner::BottomLeft);
        let places = home_object_positions(&home, &cottages, monitors, &policy, TIGHTEST_SCALE);
        for (slot, place) in places.iter().enumerate() {
            assert_eq!(
                *place,
                home_object_position(&home, slot, &cottages, monitors, &policy, TIGHTEST_SCALE),
                "belonging {slot} moved when it was asked for on its own"
            );
        }
        // The scatter comes from the colony's own seed, so it is the same every time and two
        // colonies do not arrange their yard identically.
        let one = village_home(&monitor, HomeCorner::BottomLeft);
        let mut other = one.clone();
        other.shelter.detail_seed ^= 0xffff;
        let places = |home: &ColonyHome| {
            home_object_positions(home, &cottages, monitors, &policy, TIGHTEST_SCALE)
        };
        assert_eq!(places(&one), places(&one));
        assert_ne!(places(&one), places(&other));
    }

    /// A tree is the first thing a cramped corner gives up. A region with room for the houses but
    /// not for what stands at the ends of them shows the houses: a village with no trees in it
    /// still reads as a village, and one with its colony house half off the screen does not. The
    /// colony's belongings live in those yards, so they go with them.
    #[test]
    fn a_narrow_corner_hides_the_trees_and_their_yards_rather_than_a_house() {
        let cottages = vec![DwellingKind::Cottage];
        let mut monitor = wide_monitor(1.0);
        monitor.bounds.width = 800.0;
        monitor.usable_bounds.width = 800.0;
        let monitors = std::slice::from_ref(&monitor);
        // A habitat cut back to a band in the middle of the display: room for the houses, none
        // for what stands past them against the edge.
        for (corner, left) in [
            (HomeCorner::BottomLeft, 0.1_f32),
            (HomeCorner::BottomRight, 0.5),
        ] {
            let policy = HabitatPolicy {
                preset: HabitatPreset::Custom,
                zones: vec![HabitatZone {
                    id: 1,
                    display: monitor.display_key,
                    normalized_bounds: DesktopRect {
                        x: left,
                        y: 0.0,
                        width: 0.4,
                        height: 1.0,
                    },
                    kind: HabitatZoneKind::Allowed,
                    enabled: true,
                }],
            };
            let home = village_home(&monitor, corner);
            assert!(
                home_dwelling_position(&home, 0, &cottages, monitors, &policy, 2).is_some()
                    && home_dwelling_position(&home, 1, &cottages, monitors, &policy, 2).is_some(),
                "{corner:?}: a house went missing"
            );
            for end in TreeEnd::BOTH {
                assert!(
                    home_tree_position(&home, end, &cottages, monitors, &policy, 2).is_none(),
                    "{corner:?}: the {end:?} tree stood where there was no room for one"
                );
            }
            for slot in 0..crate::MAX_COLONY_OBJECTS {
                assert!(
                    home_object_position(&home, slot, &cottages, monitors, &policy, 2).is_none(),
                    "{corner:?}: belonging {slot} was left standing in a yard that is not there"
                );
            }
        }
        // Given the room, both are back: one on the far side of the house from the cottages, one
        // past the last cottage, and each inside the display it belongs to.
        let policy = HabitatPolicy::default();
        let monitor = wide_monitor(1.0);
        let monitors = std::slice::from_ref(&monitor);
        for corner in [HomeCorner::BottomLeft, HomeCorner::BottomRight] {
            let home = village_home(&monitor, corner);
            let (_, house) =
                home_dwelling_position(&home, 0, &cottages, monitors, &policy, 2).unwrap();
            let (_, cottage) =
                home_dwelling_position(&home, 1, &cottages, monitors, &policy, 2).unwrap();
            let (_, outward) =
                home_tree_position(&home, TreeEnd::Outward, &cottages, monitors, &policy, 2)
                    .unwrap();
            let (_, inward) =
                home_tree_position(&home, TreeEnd::Inward, &cottages, monitors, &policy, 2)
                    .unwrap();
            assert!(
                (outward.x - house.x).signum() != (cottage.x - house.x).signum(),
                "{corner:?}: the outward tree stood on the cottages' side of the house"
            );
            assert_eq!(
                (inward.x - house.x).signum(),
                (cottage.x - house.x).signum(),
                "{corner:?}: the inward tree left the cottages' end"
            );
            for (end, tree) in [("outward", outward), ("inward", inward)] {
                assert!(
                    tree.x - TREE_WIDTH >= monitor.usable_bounds.x
                        && tree.x + TREE_WIDTH <= monitor.usable_bounds.right(),
                    "{corner:?}: the {end} tree was clipped by the edge of the display"
                );
            }
        }
    }

    /// A corner too narrow for the whole village still seats everybody. The commons is whatever
    /// ground the village did manage to lay out, and the colony spreads along it — packed in
    /// against the ends if it has to, but never standing on one another.
    #[test]
    fn a_narrow_strip_still_seats_everybody_without_stacking_them() {
        let policy = HabitatPolicy::default();
        let mut monitor = wide_monitor(1.0);
        monitor.bounds.width = 320.0;
        monitor.usable_bounds.width = 320.0;
        let monitors = std::slice::from_ref(&monitor);
        let cottages = widest_colony();
        let residents = crate::MAX_COLONY_CREATURES;
        for corner in [HomeCorner::BottomLeft, HomeCorner::BottomRight] {
            let home = village_home(&monitor, corner);
            let region = accessible_regions(&policy, &monitor)[0];
            let commons = home_commons(&home, &cottages, monitors, &policy, 2).unwrap();
            assert!(
                commons.width() < VILLAGE_SPAN_LIMIT * 2.0,
                "{corner:?}: this display should be too narrow for the whole village"
            );
            let spots: Vec<f32> = (0..residents)
                .map(|slot| {
                    home_resting_position(&home, slot, residents, &cottages, monitors, &policy, 2)
                        .unwrap()
                        .1
                        .x
                })
                .collect();
            for (index, a) in spots.iter().enumerate() {
                assert!(
                    *a >= region.x - 0.01 && *a <= region.right() + 0.01,
                    "{corner:?}: a companion was sent off the display"
                );
                for b in &spots[index + 1..] {
                    assert!(
                        (a - b).abs() >= 1.0,
                        "{corner:?}: companions stacked {} apart",
                        (a - b).abs()
                    );
                }
            }
        }
    }

    /// never within a face's width of anybody resting.
    #[test]
    fn a_guest_waits_past_every_lot_and_every_resident() {
        let policy = HabitatPolicy::default();
        let monitor = wide_monitor(1.0);
        let monitors = std::slice::from_ref(&monitor);
        let cottages = widest_colony();
        let unit = f32::from(TIGHTEST_SCALE);
        for corner in [HomeCorner::BottomLeft, HomeCorner::BottomRight] {
            let home = village_home(&monitor, corner);
            let direction = if corner == HomeCorner::BottomLeft {
                1.0
            } else {
                -1.0
            };
            let anchor = resolved_home_anchor(&home, &monitor, TIGHTEST_SCALE, &policy).unwrap();
            let (_, guest) = home_guest_position(
                &home,
                &cottages,
                crate::MAX_COLONY_OBJECTS,
                monitors,
                &policy,
                TIGHTEST_SCALE,
            )
            .unwrap();
            assert_eq!(guest.y, anchor.y);
            let out = (guest.x - anchor.x) * direction;
            for (lot, centre) in village_walk(&cottages).iter() {
                let edge = (centre + lot_width(lot, &cottages) / 2.0) * unit;
                assert!(
                    out >= edge - 0.01,
                    "{corner:?}: the guest stands on {lot:?}"
                );
            }
            let clear = CREATURE_FRAME_WIDTH * REST_CLEAR_RATIO * unit;
            let residents = crate::MAX_COLONY_CREATURES;
            for slot in 0..residents {
                let (_, rest) = home_resting_position(
                    &home,
                    slot,
                    residents,
                    &cottages,
                    monitors,
                    &policy,
                    TIGHTEST_SCALE,
                )
                .unwrap();
                assert!(
                    (guest.x - rest.x).abs() >= clear - 0.01,
                    "{corner:?}: the guest stands on member {slot}"
                );
            }
        }
    }

    /// The village ground works its own anchor out rather than calling `resolved_home_anchor` and
    /// then listing the habitat's regions all over again, and the colony's houses are counted onto
    /// the stack rather than sorted into a pair of fresh lists. Neither may change an answer.
    #[test]
    fn the_cheaper_layout_gives_the_same_answers_the_allocating_one_did() {
        for preset in [
            HabitatPreset::EntireDesktop,
            HabitatPreset::BottomCorners,
            HabitatPreset::BottomEdge,
        ] {
            let policy = HabitatPolicy {
                preset,
                zones: Vec::new(),
            };
            let monitor = wide_monitor(2.0);
            for corner in [HomeCorner::BottomLeft, HomeCorner::BottomRight] {
                let home = village_home(&monitor, corner);
                let ground = VillageGround::resolve(
                    &home,
                    std::slice::from_ref(&monitor),
                    &policy,
                    TIGHTEST_SCALE,
                )
                .unwrap();
                assert_eq!(
                    Some(ground.anchor),
                    resolved_home_anchor(&home, &monitor, TIGHTEST_SCALE, &policy),
                    "{preset:?} {corner:?}: the ground drifted from the habitat's own answer"
                );
                assert_eq!(
                    ground.region(),
                    accessible_regions(&policy, &monitor)
                        .into_iter()
                        .find(|region| {
                            ground.anchor.x >= region.x
                                && ground.anchor.x <= region.right()
                                && ground.anchor.y >= region.y
                                && ground.anchor.y <= region.bottom()
                        }),
                    "{preset:?} {corner:?}: the ground kept the wrong region"
                );
            }
        }
        // A colony listed out of order, with a mini in the middle of it.
        let mut creatures = Vec::new();
        for index in 0..4_u8 {
            let mut creature = crate::World::preview_adult(
                [index; 32],
                time::OffsetDateTime::UNIX_EPOCH,
                &crate::DesktopSnapshot::default(),
            );
            creature.colony_order = 3 - index;
            creature.id = u64::from(index) + 10;
            if index == 1 {
                creature.role = CreatureRole::Mini { parent_id: 1 };
            }
            creatures.push(creature);
        }
        assert_eq!(
            colony_cottage_list(&creatures).as_slice(),
            colony_cottages(&creatures).as_slice()
        );
        // Three full-size companions and one mini: the mini lives in its big version's house,
        // so the village lays out two cottages beside the colony house rather than three.
        assert_eq!(
            colony_cottages(&creatures),
            vec![DwellingKind::Cottage, DwellingKind::Cottage],
            "the houses follow colony order, not the order the save happens to hold"
        );
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
