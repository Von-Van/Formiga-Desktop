//! Scenery a village can be laid out on instead of its strip along the bottom of the display: a
//! picture with a spot on it for every house and both trees, and a walk map of the ground drawn
//! in it — paths, stairs, bridges and the cliffs between the levels.
//!
//! The picture itself is the art crate's. Everything here is measured in its own pixels, which are
//! shelter pixels: the picture is drawn at exactly the scale the houses are, so a house stands on it the size it stands
//! anywhere. `SceneryPlacement` says where on the desktop the picture is and turns its pixels into
//! points.
//!
//! The walk map is a handful of points joined by straight runs, each walked or climbed. One run of
//! it, the trail, crosses the picture from left to right with one height for every x along the
//! way, past every house that stands on it; the village's own reckoning of who stands where along
//! its ground is done on the trail, and getting anywhere else, or from one end of the trail to the
//! other, is a route over the whole map.

use crate::{MAX_COLONY_CREATURES, Point, TreeEnd, VillageScenery};

/// How a run of the walk map is crossed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Footing {
    /// On its feet: a path, a flight of stairs or a bridge.
    Walk,
    /// Hand over hand up or down a rock face.
    Climb,
}

/// How much further a climb counts than a walk of the same length when a route is chosen, so a
/// companion takes the stairs where the stairs are nearly as short.
const CLIMB_COST: f32 = 1.6;

/// How fast a climb goes, against the companion's walk: a little quicker, as a companion goes up
/// the side of a window. Any slower and a stroll's unhurried pace leaves it on a cliff face for a
/// quarter of a minute, which reads as stuck rather than climbing.
pub const CLIMB_PACE: f32 = 1.25;

/// The most points a walk map may have. Routes are worked out on the stack.
const MAX_NODES: usize = 40;

/// Nearer than this, in picture pixels, two places on the map are the same place.
const SAME_PLACE: f32 = 0.01;

/// A picture to lay a village out on, and its walk map.
pub struct SceneryMap {
    /// The picture's size, in shelter pixels.
    pub width: u32,
    pub height: u32,
    /// How far above the picture's top edge the houses and trees along its back reach.
    pub headroom: f32,
    nodes: &'static [(f32, f32)],
    edges: &'static [(u8, u8, Footing)],
    /// The trail, point by point from left to right.
    trail: &'static [u8],
    /// The points along the front a companion down on the floor hops up onto.
    ways_up: &'static [u8],
    /// Where each house's foot stands, in the order houses are given out: the colony house
    /// first.
    houses: [(f32, f32); MAX_COLONY_CREATURES],
    /// Where the outward tree and the inward tree stand.
    trees: [(f32, f32); 2],
}

impl SceneryMap {
    pub fn nodes(&self) -> &'static [(f32, f32)] {
        self.nodes
    }

    pub fn edges(&self) -> &'static [(u8, u8, Footing)] {
        self.edges
    }

    pub fn trail(&self) -> &'static [u8] {
        self.trail
    }

    pub fn houses(&self) -> &[(f32, f32); MAX_COLONY_CREATURES] {
        &self.houses
    }

    pub fn trees(&self) -> &[(f32, f32); 2] {
        &self.trees
    }

    fn node(&self, index: u8) -> (f32, f32) {
        self.nodes[usize::from(index)]
    }

    fn edge_cost(&self, footing: Footing) -> f32 {
        match footing {
            Footing::Walk => 1.0,
            Footing::Climb => CLIMB_COST,
        }
    }

    /// The nearest place on the walk map to `at`: which run it is on, and the place itself.
    fn locate(&self, at: (f32, f32)) -> (usize, (f32, f32)) {
        let mut best = (0, self.node(self.edges[0].0), f32::INFINITY);
        for (index, &(a, b, _)) in self.edges.iter().enumerate() {
            let (place, distance) = nearest_on_run(at, self.node(a), self.node(b));
            if distance < best.2 {
                best = (index, place, distance);
            }
        }
        (best.0, best.1)
    }

    /// The trail's height at `x`, held level past either end.
    fn trail_height(&self, x: f32) -> f32 {
        let first = self.node(self.trail[0]);
        if x <= first.0 {
            return first.1;
        }
        for pair in self.trail.windows(2) {
            let (a, b) = (self.node(pair[0]), self.node(pair[1]));
            if x <= b.0 {
                let t = if b.0 > a.0 {
                    (x - a.0) / (b.0 - a.0)
                } else {
                    1.0
                };
                return a.1 + (b.1 - a.1) * t;
            }
        }
        self.node(self.trail[self.trail.len() - 1]).1
    }

    /// The next place to head for, on the way from `at` to `goal` on run `goal_run`: the goal
    /// itself when the two are on the same run, and otherwise the first point of the shortest
    /// route between them that is not where `at` already is.
    fn waypoint(&self, at: (f32, f32), goal_run: usize, goal: (f32, f32)) -> (f32, f32) {
        let (run, at) = self.locate(at);
        if run == goal_run {
            return goal;
        }
        let count = self.nodes.len();
        let mut distance = [f32::INFINITY; MAX_NODES];
        let mut first = [u8::MAX; MAX_NODES];
        let mut done = [false; MAX_NODES];
        let (a, b, footing) = self.edges[run];
        let cost = self.edge_cost(footing);
        for end in [a, b] {
            distance[usize::from(end)] = span(at, self.node(end)) * cost;
            first[usize::from(end)] = end;
        }
        for _ in 0..count {
            let Some(here) = (0..count)
                .filter(|index| !done[*index] && distance[*index].is_finite())
                .min_by(|x, y| distance[*x].total_cmp(&distance[*y]))
            else {
                break;
            };
            done[here] = true;
            for &(a, b, footing) in self.edges {
                let next = if usize::from(a) == here {
                    b
                } else if usize::from(b) == here {
                    a
                } else {
                    continue;
                };
                let through =
                    distance[here] + span(self.node(a), self.node(b)) * self.edge_cost(footing);
                let slot = usize::from(next);
                if through < distance[slot] {
                    distance[slot] = through;
                    // Standing on `here` already, the route starts at the point after it.
                    first[slot] = if distance[here] <= SAME_PLACE {
                        next
                    } else {
                        first[here]
                    };
                }
            }
        }
        let (c, d, footing) = self.edges[goal_run];
        let cost = self.edge_cost(footing);
        let finish = |end: u8| distance[usize::from(end)] + span(self.node(end), goal) * cost;
        let end = if finish(c) <= finish(d) { c } else { d };
        if distance[usize::from(end)] <= SAME_PLACE || first[usize::from(end)] == u8::MAX {
            goal
        } else {
            self.node(first[usize::from(end)])
        }
    }

    /// How the stretch from `a` to `b`, both on the walk map, is crossed.
    fn footing_between(&self, a: (f32, f32), b: (f32, f32)) -> Footing {
        let (run, _) = self.locate(((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0));
        self.edges[run].2
    }
}

/// The nearest place to `at` on the straight run from `a` to `b`, and how far away it is.
fn nearest_on_run(at: (f32, f32), a: (f32, f32), b: (f32, f32)) -> ((f32, f32), f32) {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let length = dx * dx + dy * dy;
    let t = if length > 0.0 {
        (((at.0 - a.0) * dx + (at.1 - a.1) * dy) / length).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let place = (a.0 + dx * t, a.1 + dy * t);
    (place, span(at, place))
}

fn span(a: (f32, f32), b: (f32, f32)) -> f32 {
    (b.0 - a.0).hypot(b.1 - a.1)
}

impl VillageScenery {
    pub const fn map(self) -> &'static SceneryMap {
        match self {
            Self::Pond => &POND,
        }
    }
}

/// Where a scenery picture is on the desktop, and how many points one of its pixels covers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SceneryPlacement {
    pub scenery: VillageScenery,
    /// Where the picture's top-left corner is.
    pub origin: Point,
    /// Points per picture pixel: the same as points per shelter pixel for the houses on it.
    pub scale: f32,
}

impl SceneryPlacement {
    pub fn map(&self) -> &'static SceneryMap {
        self.scenery.map()
    }

    /// The desktop point of a place in the picture.
    pub fn point(&self, (x, y): (f32, f32)) -> Point {
        Point {
            x: self.origin.x + x * self.scale,
            y: self.origin.y + y * self.scale,
        }
    }

    /// The place in the picture under a desktop point.
    fn local(&self, point: Point) -> (f32, f32) {
        (
            (point.x - self.origin.x) / self.scale,
            (point.y - self.origin.y) / self.scale,
        )
    }

    /// Where house `slot` stands.
    pub fn house(&self, slot: usize) -> Option<Point> {
        self.map().houses.get(slot).map(|spot| self.point(*spot))
    }

    /// Where a tree stands.
    pub fn tree(&self, end: TreeEnd) -> Point {
        self.point(
            self.map().trees[match end {
                TreeEnd::Outward => 0,
                TreeEnd::Inward => 1,
            }],
        )
    }

    /// The left and right ends of the trail, as desktop x.
    pub fn trail_span(&self) -> (f32, f32) {
        let map = self.map();
        let first = map.node(map.trail[0]);
        let last = map.node(map.trail[map.trail.len() - 1]);
        (self.point(first).x, self.point(last).x)
    }

    /// How high the trail is at desktop x, as a desktop y.
    pub fn ground_at(&self, x: f32) -> f32 {
        let local = (x - self.origin.x) / self.scale;
        self.origin.y + self.map().trail_height(local) * self.scale
    }

    /// The nearest place anybody can stand to `point`.
    pub fn nearest(&self, point: Point) -> Point {
        let (_, place) = self.map().locate(self.local(point));
        self.point(place)
    }

    /// Where a companion down on the floor at desktop x gets up onto the picture: the nearest of
    /// its ways up along the front, as a desktop point.
    pub fn way_up(&self, x: f32) -> Point {
        let map = self.map();
        map.ways_up
            .iter()
            .map(|&node| self.point(map.node(node)))
            .min_by(|a, b| (a.x - x).abs().total_cmp(&(b.x - x).abs()))
            .unwrap_or(self.origin)
    }

    /// Whether `point` is somewhere on the walk map, give or take a fraction of a point.
    pub fn on_map(&self, point: Point) -> bool {
        self.nearest(point).distance(point) <= 0.5
    }

    /// Where a companion at `from` heading for `to` gets to after covering `distance` points of
    /// walk, and how it is crossing the ground it ends on. It keeps to the walk map all the way,
    /// up and down stairs and hand over hand on a climb, which goes slower. A destination off
    /// the map is reached at the nearest place on it; one on the map is landed on exactly.
    pub fn step(&self, from: Point, to: Point, distance: f32) -> (Point, Footing) {
        let map = self.map();
        let wanted = self.local(to);
        let (goal_run, goal) = map.locate(wanted);
        let exact = span(goal, wanted) <= SAME_PLACE * 10.0;
        let (_, mut at) = map.locate(self.local(from));
        let mut left = distance.max(0.0) / self.scale;
        let mut footing = Footing::Walk;
        // A route crosses a few points at most in one step; the bound only keeps a step that
        // somehow made no progress from going round for ever.
        for _ in 0..MAX_NODES {
            if span(at, goal) <= SAME_PLACE {
                at = goal;
                break;
            }
            let way = map.waypoint(at, goal_run, goal);
            let length = span(at, way);
            footing = map.footing_between(at, way);
            if left <= 0.0 {
                break;
            }
            let pace = match footing {
                Footing::Walk => 1.0,
                Footing::Climb => CLIMB_PACE,
            };
            let reach = left * pace;
            if reach >= length {
                at = way;
                left -= length / pace;
            } else {
                at = (
                    at.0 + (way.0 - at.0) * reach / length,
                    at.1 + (way.1 - at.1) * reach / length,
                );
                left = 0.0;
            }
        }
        if exact && span(at, goal) <= SAME_PLACE {
            return (to, footing);
        }
        (self.point(at), footing)
    }
}

/// The pond: terraces stepping down from the top left to the bottom right round a pond, with a
/// waterfall off the top terrace, a bridge over the pond's top and another over its foot, and
/// stairs between the levels. The colony house stands at the back of the top terrace beside the
/// outward tree, the next two houses on the levels below it as the trail runs down to the inward
/// tree, one more at the back of the top terrace between the outward tree and the colony house,
/// and the last two on the ledge beside the falls and the terrace at the pond's foot below it. The
/// two on the top terrace stand back from its front edge, so the top of the stairs is left clear,
/// and a short path runs back from the trail to each of their doors.
///
/// The trail comes down off the top terrace by the cliff under its right-hand end, which is the
/// short way; the stairs beside the colony house are the long way round. The left-hand side is
/// two more cliffs, from the top terrace to the ledge and from the ledge to the foot of the pond,
/// and a third drops from the terrace under the second house to the upper bridge.
pub const POND: SceneryMap = SceneryMap {
    width: 749,
    height: 457,
    headroom: 46.0,
    nodes: &[
        // The trail, from the top terrace's left end down to the inward tree.
        (24.0, 56.0),   // 0: the top terrace's left end
        (40.0, 56.0),   // 1: the top of the cliff down to the ledge
        (82.0, 58.0),   // 2: the outward tree
        (150.0, 57.0),  // 3
        (186.0, 62.0),  // 4: the top of the stairs
        (230.0, 61.0),  // 5: in front of the colony house
        (300.0, 55.0),  // 6
        (336.0, 55.0),  // 7: the top of the cliff down to the second terrace
        (337.0, 104.0), // 8: its foot
        (360.0, 118.0), // 9
        (395.0, 124.0), // 10: in front of the second house
        (462.0, 146.0), // 11: the top of the second stairs
        (510.0, 176.0), // 12: their foot
        (572.0, 207.0), // 13: in front of the third house
        (626.0, 236.0), // 14: the top of the stairs down the right-hand side
        (695.0, 258.0), // 15: the inward tree
        (728.0, 266.0), // 16: the trail's right end
        // The stairs down from beside the colony house, the long way to the second terrace.
        (238.0, 114.0), // 17: their foot
        (292.0, 117.0), // 18
        (322.0, 120.0), // 19: the top of the cliff down to the upper bridge
        // Down the right-hand side to the front, and round the foot of the pond.
        (598.0, 288.0), // 20: the foot of the right-hand stairs
        (662.0, 312.0), // 21
        (642.0, 343.0), // 22: the top of the front stairs
        (598.0, 364.0), // 23: their foot
        (520.0, 384.0), // 24
        (420.0, 392.0), // 25
        (330.0, 378.0), // 26
        (288.0, 366.0), // 27: the lower bridge's right end
        (150.0, 326.0), // 28: its left end
        (100.0, 292.0), // 29
        (88.0, 262.0),  // 30: the foot of the cliff up to the ledge
        // The ledge under the top terrace, beside the falls.
        (40.0, 160.0),  // 31: the foot of the cliff up to the top terrace
        (88.0, 166.0),  // 32: the top of the cliff down to the foot of the pond
        (186.0, 170.0), // 33: the ledge's end by the falls
        // The rocks at the foot of the cliff under the second terrace, and the upper bridge.
        (322.0, 205.0), // 34: the upper bridge's left end
        (482.0, 240.0), // 35: its right end
        // Back from the trail across the top terrace to the two doors there.
        (232.0, 42.0), // 36: the colony house's door
        (150.0, 36.0), // 37: the door of the house beside it
    ],
    edges: &[
        (0, 1, Footing::Walk),
        (1, 2, Footing::Walk),
        (2, 3, Footing::Walk),
        (3, 4, Footing::Walk),
        (4, 5, Footing::Walk),
        (5, 6, Footing::Walk),
        (6, 7, Footing::Walk),
        (7, 8, Footing::Climb),
        (8, 9, Footing::Walk),
        (9, 10, Footing::Walk),
        (10, 11, Footing::Walk),
        (11, 12, Footing::Walk),
        (12, 13, Footing::Walk),
        (13, 14, Footing::Walk),
        (14, 15, Footing::Walk),
        (15, 16, Footing::Walk),
        (4, 17, Footing::Walk),
        (17, 18, Footing::Walk),
        (18, 19, Footing::Walk),
        (19, 9, Footing::Walk),
        (14, 20, Footing::Walk),
        (20, 21, Footing::Walk),
        (21, 22, Footing::Walk),
        (22, 23, Footing::Walk),
        (23, 24, Footing::Walk),
        (24, 25, Footing::Walk),
        (25, 26, Footing::Walk),
        (26, 27, Footing::Walk),
        (27, 28, Footing::Walk),
        (28, 29, Footing::Walk),
        (29, 30, Footing::Walk),
        (30, 32, Footing::Climb),
        (1, 31, Footing::Climb),
        (31, 32, Footing::Walk),
        (32, 33, Footing::Walk),
        (19, 34, Footing::Climb),
        (34, 35, Footing::Walk),
        (35, 13, Footing::Walk),
        (5, 36, Footing::Walk),
        (3, 37, Footing::Walk),
    ],
    trail: &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16],
    // The path along the front of the pond, lowest of all.
    ways_up: &[23, 24, 25, 26],
    houses: [
        (232.0, 42.0),  // the colony house, at the back of the top terrace
        (395.0, 123.0), // below it, on the second terrace
        (572.0, 213.0), // below that, past the second stairs
        (150.0, 36.0),  // between the outward tree and the colony house, as far back
        (126.0, 166.0), // on the ledge beside the falls
        (112.0, 298.0), // at the foot of the pond, under the ledge
    ],
    trees: [(82.0, 58.0), (695.0, 260.0)],
};

const _: () = assert!(POND.nodes.len() <= MAX_NODES);

#[cfg(test)]
mod tests {
    use super::*;

    fn placed() -> SceneryPlacement {
        SceneryPlacement {
            scenery: VillageScenery::Pond,
            origin: Point { x: 100.0, y: 200.0 },
            scale: 1.5,
        }
    }

    #[test]
    fn the_trail_runs_left_to_right_along_joined_runs() {
        let map = VillageScenery::Pond.map();
        for pair in map.trail.windows(2) {
            assert!(map.node(pair[0]).0 < map.node(pair[1]).0);
            assert!(
                map.edges
                    .iter()
                    .any(|&(a, b, _)| (a, b) == (pair[0], pair[1]) || (b, a) == (pair[0], pair[1])),
                "{pair:?} is not a run of the map"
            );
        }
    }

    #[test]
    fn every_point_on_the_map_can_be_reached_from_the_colony_house() {
        let placement = placed();
        let map = placement.map();
        let start = placement.house(0).unwrap();
        let start = placement.nearest(start);
        for &node in map.nodes {
            let goal = placement.point(node);
            let mut at = start;
            for _ in 0..2_000 {
                (at, _) = placement.step(at, goal, 4.0);
                if at == goal {
                    break;
                }
            }
            assert_eq!(at, goal, "never reached {node:?}");
        }
    }

    #[test]
    fn a_step_keeps_to_the_map_and_lands_exactly_on_its_goal() {
        let placement = placed();
        let from = placement.point((24.0, 56.0));
        let to = placement.point((420.0, 392.0));
        let mut at = from;
        let mut climbed = false;
        for _ in 0..4_000 {
            let footing;
            (at, footing) = placement.step(at, to, 3.0);
            climbed |= footing == Footing::Climb;
            assert!(placement.on_map(at), "{at:?} is off the map");
            if at == to {
                break;
            }
        }
        assert_eq!(at, to);
        assert!(climbed, "the way down the left side is by the cliffs");
    }

    #[test]
    fn the_trail_has_one_height_for_every_x() {
        let placement = placed();
        let (low, high) = placement.trail_span();
        let mut x = low;
        while x <= high {
            let point = Point {
                x,
                y: placement.ground_at(x),
            };
            assert!(placement.on_map(point), "{point:?} is not on the trail");
            x += 0.5;
        }
    }

    #[test]
    fn every_house_and_tree_stands_inside_the_picture() {
        let map = VillageScenery::Pond.map();
        for &(x, y) in map.houses.iter().chain(map.trees.iter()) {
            assert!(x > 0.0 && x < map.width as f32 && y > 0.0 && y < map.height as f32);
        }
    }
}
