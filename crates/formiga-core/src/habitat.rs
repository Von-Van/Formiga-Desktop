use crate::{
    ColonyHome, ColonyObject, Creature, CreatureRole, DesktopRect, GardenKind, HabitatPolicy,
    HabitatPreset, HabitatZoneKind, HangoutKind, HomeCorner, MonitorInfo, OrnamentKind, Point,
};

pub const MAX_HABITAT_ZONES: usize = 32;
const MIN_REGION_SIZE: f32 = 48.0;

/// Where the colony house would stand in its corner of a display with all the room it wants:
/// far enough in from the edge for the outward tree beside it.
pub fn home_anchor(home: &ColonyHome, monitor: &MonitorInfo, display_scale: u8) -> Point {
    anchor_for(home, monitor, display_scale, VillageFit::Comfortable)
}

fn anchor_for(
    home: &ColonyHome,
    monitor: &MonitorInfo,
    display_scale: u8,
    fit: VillageFit,
) -> Point {
    let margin = fit.edge_margin() * f32::from(display_scale) / monitor.scale_factor.max(1.0) + 8.0;
    Point {
        x: match home.corner {
            HomeCorner::BottomLeft => monitor.usable_bounds.x + margin,
            HomeCorner::BottomRight => monitor.usable_bounds.right() - margin,
        },
        y: monitor.usable_bounds.bottom() - 4.0,
    }
}

/// Where the colony house actually stands on this display, for a colony with these cottages: in
/// its corner, inside the habitat, and tucked closer to the edge when the village is laid out
/// snug to fit a narrow display. `None` when the habitat leaves it nowhere to stand.
pub fn resolved_home_anchor(
    home: &ColonyHome,
    cottages: &[DwellingKind],
    monitor: &MonitorInfo,
    display_scale: u8,
    policy: &HabitatPolicy,
) -> Option<Point> {
    VillageGround::resolve(
        home,
        cottages,
        std::slice::from_ref(monitor),
        policy,
        display_scale,
    )
    .map(|ground| ground.anchor)
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
        .or_else(|| nearest_floor_point(policy, monitors, intended, |_| MIN_WALL_CLEARANCE))
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
    /// Ground footprint in shelter pixels: the drawn house plus its shadow and the decorations
    /// standing against its corners, with a pixel or two of breathing room. Smaller than the 96px
    /// atlas cell it is sampled from, so neighbours sit close without their artwork touching. A
    /// companion's house stands plainly taller and wider than the companion who keeps it, and the
    /// colony house stays plainly the largest building on the strip. The decorations that stand
    /// against a house's corners stand half in front of its wall rather than out beside it, so a
    /// footprint keeps to the proportions of the house it is under.
    pub const fn width(self) -> f32 {
        match self {
            Self::Main => 84.0,
            Self::Cottage => 68.0,
        }
    }

    /// The ground it claims when the village is laid out snug on a narrow display: only as wide
    /// as the widest house of its kind is drawn, decorations, shadow and all, so neighbours stand
    /// shoulder to shoulder without any drawing reaching over another's.
    pub const fn snug_width(self) -> f32 {
        match self {
            Self::Main => 80.0,
            Self::Cottage => 66.0,
        }
    }
}

/// Every dwelling is drawn from one 96px atlas cell, whatever its footprint.
pub const DWELLING_CELL: f32 = 96.0;

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

/// The air between two lots that are not part of the same thing. Three shelter pixels is a visible
/// seam at every scale the overlay draws at and no more: the seams add up along the strip, and five
/// would leave fifty-five pixels of empty lane once it is laid end to end.
const VILLAGE_GAP: f32 = 3.0;

/// How closely the village is laid out on the display it is on. Comfortable is the village as it is
/// drawn everywhere it fits: a seam between houses, a footprint round each that leaves its
/// decorations room, and the outward tree's yard against the edge of the display. A display too
/// narrow for every house that way — a 1280- to 1440-point laptop at 100%, at the default size —
/// lays it out snug instead, so that nobody's house is the one left off the end: the outward tree
/// gives up its ground and is not shown, the colony house tucks into the corner, and the houses
/// stand shoulder to shoulder on footprints that are only as wide as what is drawn on them. Snug is
/// only ever used where it shows more houses than comfortable would.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum VillageFit {
    #[default]
    Comfortable,
    Snug,
}

impl VillageFit {
    /// The seam between two houses.
    const fn gap(self) -> f32 {
        match self {
            Self::Comfortable => VILLAGE_GAP,
            Self::Snug => 0.0,
        }
    }

    /// The ground a house claims.
    const fn width(self, kind: DwellingKind) -> f32 {
        match self {
            Self::Comfortable => kind.width(),
            Self::Snug => kind.snug_width(),
        }
    }

    /// How far the colony house's middle stands in from the edge of the display.
    const fn edge_margin(self) -> f32 {
        match self {
            Self::Comfortable => HOME_EDGE_MARGIN,
            Self::Snug => DwellingKind::Main.snug_width() / 2.0,
        }
    }
}

/// How far each tree's lot reaches in over the house beside it. The village is laid out a little
/// tighter than its houses are drawn, so the trees at either end stand a little in over the end
/// houses' ground rather than a seam clear of it; where a tree's drawing and a house's meet, the
/// tree is drawn in front. Six pixels is the shadow and whatever decoration stands on the ground
/// beside the wall, never the wall itself.
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
/// doorway far inside this — the narrowest, a companion's cottage, has thirty-four pixels between
/// its door's own middle and the edge of its lot, and its doorway and frame are at most nine of
/// them — so a resident waiting at its door never stands in front of one.
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
/// A full village measures 539, with each tree standing a little in over its end house.
pub const VILLAGE_SPAN_LIMIT: f32 = 540.0;

/// Where the outward tree's lot sits on the walk, counting outward from the colony house: reaching
/// `TREE_OVERLAP` in over the colony house's own footprint, on the negative side of the origin.
/// The inward tree's place depends on how many cottages there are, so the walk works it out as it
/// goes.
const fn outward_tree_centre(fit: VillageFit) -> f32 {
    -(fit.width(DwellingKind::Main) / 2.0 - TREE_OVERLAP + TREE_WIDTH / 2.0)
}

/// The ground each kind of lot claims, in shelter pixels.
fn lot_width(lot: VillageLot, cottages: &[DwellingKind], fit: VillageFit) -> f32 {
    match lot {
        VillageLot::Tree(_) => TREE_WIDTH,
        VillageLot::Dwelling(0) => fit.width(DwellingKind::Main),
        VillageLot::Dwelling(index) => fit.width(
            cottages
                .get(index - 1)
                .copied()
                .unwrap_or(DwellingKind::Cottage),
        ),
    }
}

/// How far above the ground line the tallest house is drawn, pennant and all: 78 shelter pixels,
/// held to the drawings by a test in the art crate. The lot fits by this rather than by the
/// 96-pixel cell it is drawn from, whose top is empty air, so a band along the bottom of a display
/// — above a Dock or a taskbar, say — has room for the village. It allows 80, a little more than
/// the tallest house, so every band that has held the village still does.
pub const DWELLING_DRAWN_HEIGHT: f32 = 80.0;

/// How tall a lot's contents draw, for deciding whether the lot fits its region. A keepsake tree
/// draws inside its own 64-pixel cell, well under a house.
fn lot_height(lot: VillageLot) -> f32 {
    match lot {
        VillageLot::Tree(_) | VillageLot::Dwelling(_) => DWELLING_DRAWN_HEIGHT,
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
fn village_walk(cottages: &[DwellingKind], fit: VillageFit) -> VillageWalk {
    let mut walk = VillageWalk {
        lots: [(VillageLot::Tree(TreeEnd::Outward), outward_tree_centre(fit)); MAX_VILLAGE_LOTS],
        len: 1,
    };
    let mut push = |lot: VillageLot, centre: f32| {
        walk.lots[walk.len] = (lot, centre);
        walk.len += 1;
    };
    push(VillageLot::Dwelling(0), 0.0);
    let mut edge = fit.width(DwellingKind::Main) / 2.0;
    for house in 1..=cottages.len().min(crate::MAX_COLONY_CREATURES - 1) {
        // Houses stand a seam apart, and nothing else is laid between them. The ground in front
        // of the whole row is the colony's own, walked rather than parcelled out.
        edge += fit.gap();
        let width = lot_width(VillageLot::Dwelling(house), cottages, fit);
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
    let (low, high) = village_walk(cottages, VillageFit::Comfortable).iter().fold(
        (f32::MAX, f32::MIN),
        |(low, high), (lot, centre)| {
            let half = lot_width(lot, cottages, VillageFit::Comfortable) / 2.0;
            ((centre - half).min(low), (centre + half).max(high))
        },
    );
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
    /// How closely the village is laid out here.
    fit: VillageFit,
}

impl<'a> VillageGround<'a> {
    /// The village's ground on its display, laid out comfortably wherever every house fits that
    /// way, and snug where that shows more of them.
    fn resolve(
        home: &ColonyHome,
        cottages: &[DwellingKind],
        monitors: &'a [MonitorInfo],
        policy: &HabitatPolicy,
        display_scale: u8,
    ) -> Option<Self> {
        let comfortable = Self::resolve_fit(
            home,
            monitors,
            policy,
            display_scale,
            VillageFit::Comfortable,
        )?;
        // The houses run outward from the colony house in one unbroken row of one height, so if
        // the first and the last both fit, every one between them does: two lots decide the
        // ordinary case, which the simulation and the overlay ask about many times a second.
        if comfortable.shows_every_house(cottages) {
            return Some(comfortable);
        }
        let shown = comfortable.houses_shown(cottages);
        match Self::resolve_fit(home, monitors, policy, display_scale, VillageFit::Snug) {
            Some(snug) if snug.houses_shown(cottages) > shown => Some(snug),
            _ => Some(comfortable),
        }
    }

    /// Whether this ground has room for every one of the colony's houses: the colony house's inner
    /// edge and the last cottage's outer edge, worked out with the same arithmetic the walk lays
    /// them out by, without laying out the walk. It is asked every time the ground is resolved.
    fn shows_every_house(&self, cottages: &[DwellingKind]) -> bool {
        let Some(region) = self.region else {
            return false;
        };
        let main_half = self.fit.width(DwellingKind::Main) / 2.0;
        let mut outer = main_half;
        for kind in cottages.iter().take(crate::MAX_COLONY_CREATURES - 1) {
            outer += self.fit.gap() + self.fit.width(*kind);
        }
        let inner_x = self.anchor.x - self.direction * main_half * self.scale;
        let outer_x = self.anchor.x + self.direction * outer * self.scale;
        let (low, high) = (inner_x.min(outer_x), inner_x.max(outer_x));
        low >= region.x
            && high <= region.right()
            && self.anchor.y - DWELLING_DRAWN_HEIGHT * self.scale >= region.y
            && self.anchor.y <= region.bottom()
    }

    /// How many of the colony's houses this ground has room for.
    fn houses_shown(&self, cottages: &[DwellingKind]) -> usize {
        self.walk(cottages)
            .iter()
            .filter(|(lot, centre)| {
                matches!(lot, VillageLot::Dwelling(_))
                    && self.place(*lot, *centre, cottages).is_some()
            })
            .count()
    }

    /// The walk along this ground, at the fit it is laid out at.
    fn walk(&self, cottages: &[DwellingKind]) -> VillageWalk {
        village_walk(cottages, self.fit)
    }

    /// The ground a lot claims here.
    fn lot_width(&self, lot: VillageLot, cottages: &[DwellingKind]) -> f32 {
        lot_width(lot, cottages, self.fit)
    }

    fn resolve_fit(
        home: &ColonyHome,
        monitors: &'a [MonitorInfo],
        policy: &HabitatPolicy,
        display_scale: u8,
        fit: VillageFit,
    ) -> Option<Self> {
        let monitor = village_monitor(home, monitors)?;
        // One pass over the habitat's regions answers both questions the ground has: where the
        // corner ends up, and which region it ended up in. Asking twice used to cost this two
        // fresh lists of rectangles on a path the simulation walks several times a tick.
        let regions = accessible_regions(policy, monitor);
        let desired = anchor_for(home, monitor, display_scale, fit);
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
            fit,
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
        self.fits(point, self.lot_width(lot, cottages) / 2.0, lot_height(lot))
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
    let ground = VillageGround::resolve(home, cottages, monitors, policy, display_scale)?;
    let centre = ground.walk(cottages).centre_of(lot)?;
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
    let ground = VillageGround::resolve(home, cottages, monitors, policy, display_scale)?;
    let region = ground.region()?;
    let (mut low, mut high) = (f32::MAX, f32::MIN);
    for (lot, centre) in ground.walk(cottages).iter() {
        let Some(point) = ground.place(lot, centre, cottages) else {
            continue;
        };
        let half = ground.lot_width(lot, cottages) / 2.0 * ground.scale;
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
    for (lot, centre) in ground.walk(cottages).iter() {
        let VillageLot::Dwelling(_) = lot else {
            continue;
        };
        let Some(point) = ground.place(lot, centre, cottages) else {
            continue;
        };
        let half = ground.lot_width(lot, cottages) / 2.0 * ground.scale;
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
    monitors: &[MonitorInfo],
    policy: &HabitatPolicy,
    display_scale: u8,
) -> Option<(u64, Point)> {
    let ground = VillageGround::resolve(home, cottages, monitors, policy, display_scale)?;
    let region = ground.region()?;
    let half = GUEST_HALF_WIDTH * ground.scale;
    if region.width < half * 2.0 {
        return None;
    }
    let mut outer = ground.anchor.x;
    for (lot, centre) in ground.walk(cottages).iter() {
        let Some(point) = ground.place(lot, centre, cottages) else {
            continue;
        };
        let reach =
            point.x + ground.direction * ground.lot_width(lot, cottages) / 2.0 * ground.scale;
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

/// How much ground each house's lot claims where the village stands now, in shelter pixels, by
/// house slot: its comfortable footprint, or its snug one on a display where the village is laid
/// out snug. Worked out from one look at the ground for every house at once. `None` for a slot
/// past the colony's houses, or everywhere when the village has nowhere to stand.
pub fn home_lot_widths(
    home: &ColonyHome,
    cottages: &[DwellingKind],
    monitors: &[MonitorInfo],
    policy: &HabitatPolicy,
    display_scale: u8,
) -> [Option<f32>; crate::MAX_COLONY_CREATURES] {
    let mut widths = [None; crate::MAX_COLONY_CREATURES];
    if let Some(ground) = VillageGround::resolve(home, cottages, monitors, policy, display_scale) {
        let houses = cottages.len().min(crate::MAX_COLONY_CREATURES - 1) + 1;
        for (slot, width) in widths.iter_mut().enumerate().take(houses) {
            *width = Some(ground.lot_width(VillageLot::Dwelling(slot), cottages));
        }
    }
    widths
}

/// The village's ground as it is laid out where it stands now: the left end of the strip on the
/// ground line, how many desktop points one shelter pixel covers, and every lot shown, by where
/// its middle is in shelter pixels from that end, left to right whichever way the village runs.
/// The overlay draws the grass the village stands on from it.
#[derive(Clone, Debug, PartialEq)]
pub struct VillageGroundPlan {
    pub monitor_id: u64,
    /// The left end of the strip, on the ground line, in desktop points.
    pub origin: Point,
    /// Desktop points per shelter pixel.
    pub scale: f32,
    /// Every lot shown, with its middle in shelter pixels from `origin` and the ground it claims.
    pub lots: Vec<(VillageLot, f32, f32)>,
    /// How far the strip reaches, in shelter pixels, from the outer edge of one end lot to the
    /// outer edge of the other.
    pub length: f32,
}

/// The ground the village stands on where it stands now, or `None` while it has nowhere to stand
/// or not one lot fits.
pub fn home_ground_plan(
    home: &ColonyHome,
    cottages: &[DwellingKind],
    monitors: &[MonitorInfo],
    policy: &HabitatPolicy,
    display_scale: u8,
) -> Option<VillageGroundPlan> {
    let ground = VillageGround::resolve(home, cottages, monitors, policy, display_scale)?;
    let shown: Vec<(VillageLot, Point, f32)> = ground
        .walk(cottages)
        .iter()
        .filter_map(|(lot, centre)| {
            let point = ground.place(lot, centre, cottages)?;
            Some((lot, point, ground.lot_width(lot, cottages)))
        })
        .collect();
    let left = shown
        .iter()
        .map(|(_, point, width)| point.x - width / 2.0 * ground.scale)
        .reduce(f32::min)?;
    let right = shown
        .iter()
        .map(|(_, point, width)| point.x + width / 2.0 * ground.scale)
        .reduce(f32::max)?;
    Some(VillageGroundPlan {
        monitor_id: ground.monitor.id,
        origin: Point {
            x: left,
            y: ground.anchor.y,
        },
        scale: ground.scale,
        lots: shown
            .iter()
            .map(|(lot, point, width)| (*lot, (point.x - left) / ground.scale, *width))
            .collect(),
        length: (right - left) / ground.scale,
    })
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
    let Some(ground) = VillageGround::resolve(home, cottages, monitors, policy, display_scale)
    else {
        return places;
    };
    let walk = ground.walk(cottages);
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
/// companion sitting on its roof has its feet. Measured the way the house is drawn — its span in
/// twenty-fourths of the size its drawing is written at, the dwelling's own proportions from the
/// colony's shelter genome, and where each type's roof actually tops out — and held to the drawing
/// by a test in the art crate. The ground line is three rows below where a house's walls stand in
/// its cell.
pub fn house_roof_height(
    shelter: &crate::ShelterGenome,
    style: crate::ShelterStyle,
    colony_house: bool,
) -> f32 {
    let span = if colony_house { 36 } else { 30 };
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

/// The least room any wall leaves a creature's feet, in points, however small it is drawn.
const MIN_WALL_CLEARANCE: f32 = 8.0;

/// Half the width a creature's frame draws on `monitor`, in desktop points.
///
/// Each display's overlay draws only the creatures standing on it, so whatever of a frame reaches
/// past the side of its display is drawn nowhere at all — and past the side of a region the
/// habitat allows, it is drawn where the person asked it not to be. Feet kept this far in from
/// either side keep the whole frame inside.
pub fn body_half_width(monitor: &MonitorInfo, display_scale: u8) -> f32 {
    (CREATURE_FRAME_WIDTH * f32::from(display_scale) / monitor.scale_factor.max(1.0) / 2.0)
        .max(MIN_WALL_CLEARANCE)
}

/// The run between `low` and `high` a creature's feet may use with its whole body inside: each
/// end brought in by `half`. A run narrower than one creature has a single place to stand, in its
/// middle.
pub fn standing_span(low: f32, high: f32, half: f32) -> (f32, f32) {
    let (min, max) = (low + half, high - half);
    if min <= max {
        (min, max)
    } else {
        let middle = (low + high) / 2.0;
        (middle, middle)
    }
}

/// `x` brought onto the part of `region` a creature can stand on whole.
pub fn clamp_to_standing(x: f32, region: DesktopRect, half: f32) -> f32 {
    let (min, max) = standing_span(region.x, region.right(), half);
    x.clamp(min, max)
}

/// `point` with the creature standing there kept whole inside whichever of `regions` holds it,
/// brought in from the sides of the region it would reach past. Regions cut around an exclusion
/// overlap, so a point any of them already holds whole is left alone; a point none of them holds
/// is returned as it is, for the caller's own fallback.
pub fn keep_whole_in(point: Point, regions: &[DesktopRect], half: f32) -> Point {
    let mut nearest: Option<(f32, f32)> = None;
    for region in regions.iter().filter(|region| region.contains(point)) {
        let x = clamp_to_standing(point.x, *region, half);
        if x == point.x {
            return point;
        }
        let moved = (x - point.x).abs();
        if nearest.is_none_or(|(_, best)| moved < best) {
            nearest = Some((x, moved));
        }
    }
    nearest.map_or(point, |(x, _)| Point { x, ..point })
}

/// The nearest point on any habitat floor where a whole creature fits, and the display it is on.
pub fn nearest_habitat_point(
    policy: &HabitatPolicy,
    monitors: &[MonitorInfo],
    point: Point,
    display_scale: u8,
) -> Option<(u64, Point)> {
    nearest_floor_point(policy, monitors, point, |monitor| {
        body_half_width(monitor, display_scale)
    })
}

/// The nearest point on any habitat floor at least `half(monitor)` in from either side.
fn nearest_floor_point(
    policy: &HabitatPolicy,
    monitors: &[MonitorInfo],
    point: Point,
    half: impl Fn(&MonitorInfo) -> f32,
) -> Option<(u64, Point)> {
    monitors
        .iter()
        .flat_map(|monitor| {
            let half = half(monitor);
            accessible_regions(policy, monitor)
                .into_iter()
                .map(move |region| {
                    let candidate = Point {
                        x: clamp_to_standing(point.x, region, half),
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
mod tests;
