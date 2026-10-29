//! How far into the village a resident goes: the trodden path along the middle of the grass, a
//! neighbour's doorstep set back from the front, and the way up and down to them.
//!
//! The village ground is a shallow strip seen a little from above, and a house set back on it is
//! drawn higher up the display ([`crate::ColonyHome::house_depth_list`]). A resident's
//! `CreatureState::depth` is how far back on the same strip it stands, in the same shelter
//! pixels; the overlay draws it that much higher, in front of every house standing as far back
//! or further and behind every house nearer the front. Houses are flat pictures facing the desk,
//! so a resident never changes how far back it stands where a house stands across its way: it
//! goes up and down only on the paths the ground draws in from the front of the grass to each
//! door, which run in front of their own house and nobody else's. Along the strip it keeps
//! whatever depth it has, passing behind the houses nearer the front and in front of the ones
//! further back.
//!
//! All of it is runtime only, like everything else a village does.

use super::*;

/// How far back the trodden path along the middle of the grass is, in shelter pixels: between
/// the two rows the ground draws it on.
pub const PATH_DEPTH: f32 = 4.5;

/// How fast a resident goes up or down a path, against its walk along the strip: a step back
/// covers less of the display than a step along.
const DEPTH_PACE: f32 = 0.6;

/// How close two depths are to be one.
const SAME_DEPTH: f32 = 0.01;

/// How far back a house has to stand for its doorstep to be worth a stroll up to.
const DOORSTEP_DEPTH: f32 = 3.0;

/// How much of a resident's width, either side of its middle, has to be clear of a house nearer
/// the front for it to stand somewhere: its face. A lot is wider than the house drawn on it, so
/// the rest of it is clear of the house too, or nearly.
const FACE_SHARE: f32 = 0.3;

/// Whether two depths are one.
pub(super) fn same_depth(a: f32, b: f32) -> bool {
    (a - b).abs() <= SAME_DEPTH
}

/// One step of a resident's way to `goal`, `goal_depth` back, by the path to its own door at
/// `door_x`: along the strip to the foot of the path while it is not yet as far back as it is
/// going, up the path there, and then along to the goal. A resident standing further back than
/// its own door comes down by whichever path runs back as far as it is first. `scale` is
/// shelter pixels to desktop points. Returns whether it is there.
#[allow(clippy::too_many_arguments)]
pub(super) fn by_own_door(
    creature: &mut Creature,
    paths: &VillagePaths,
    door_x: f32,
    goal: Point,
    goal_depth: f32,
    speed: f32,
    scale: f32,
    dt: f32,
) -> bool {
    if creature.state.depth > goal_depth + SAME_DEPTH {
        return paths.walk_to(creature, goal, goal_depth, speed, dt);
    }
    let own = VillagePaths::new(
        vec![Door {
            slot: 0,
            x: door_x,
            half: 0.0,
            depth: goal_depth,
        }],
        scale,
    );
    own.walk_to(creature, goal, goal_depth, speed, dt)
}

/// The paths up from the front of the grass, one to every house's door, and the trodden path
/// along the middle.
#[derive(Clone, Debug, Default)]
pub(super) struct VillagePaths {
    /// Every house shown, along the strip: where its door path comes in, half the width of its
    /// lot, and how far back the house stands, in shelter pixels.
    doors: Vec<Door>,
    /// Shelter pixels to desktop points.
    scale: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Door {
    pub(super) slot: usize,
    pub(super) x: f32,
    pub(super) half: f32,
    pub(super) depth: f32,
}

/// Where a resident goes next on its way somewhere in the village, and how far back it stands
/// there.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Waypoint {
    pub(super) point: Point,
    pub(super) depth: f32,
}

impl VillagePaths {
    pub(super) fn new(mut doors: Vec<Door>, scale: f32) -> Self {
        doors.sort_by(|a, b| a.x.total_cmp(&b.x));
        Self { doors, scale }
    }

    /// The trodden path along the middle, from the first door to the last, where some door path
    /// reaches back as far as it: a village whose houses all stand near the front has a path
    /// drawn behind them that nobody can get onto.
    pub(super) fn path_span(&self) -> Option<(f32, f32)> {
        let (first, last) = (self.doors.first()?, self.doors.last()?);
        (self.doors.len() >= 2 && self.doors.iter().any(|door| door.depth >= PATH_DEPTH))
            .then_some((first.x, last.x))
    }

    /// The doorsteps set back far enough to stroll up to.
    pub(super) fn doorsteps(&self) -> impl Iterator<Item = Door> + '_ {
        self.doors
            .iter()
            .copied()
            .filter(|door| door.depth >= DOORSTEP_DEPTH)
    }

    /// Whether a resident `half` wide standing at `x`, `depth` back, is out of sight behind a
    /// house nearer the front.
    pub(super) fn hidden(&self, x: f32, depth: f32, half: f32) -> bool {
        self.doors.iter().any(|door| {
            door.depth + SAME_DEPTH < depth && (door.x - x).abs() + half * 0.5 < door.half
        })
    }

    /// How far back a resident `half` wide can stand at `x` with no house in front of its face:
    /// as far back as the house whose lot it stands in front of, and no further. Off the end of
    /// the row of houses, toward the trees, it is the front.
    pub(super) fn open_depth(&self, x: f32, half: f32) -> f32 {
        let (Some(first), Some(last)) = (self.doors.first(), self.doors.last()) else {
            return 0.0;
        };
        if x < first.x - first.half || x > last.x + last.half {
            return 0.0;
        }
        self.doors
            .iter()
            .filter(|door| (door.x - x).abs() < door.half + half * FACE_SHARE)
            .map(|door| door.depth)
            .fold(f32::from(crate::MAX_HOUSE_DEPTH), f32::min)
    }

    /// One step of the way to `goal`, `goal_depth` back, by whichever path takes it there.
    /// Returns whether it is there.
    pub(super) fn walk_to(
        &self,
        creature: &mut Creature,
        goal: Point,
        goal_depth: f32,
        speed: f32,
        dt: f32,
    ) -> bool {
        let waypoint = self.toward(
            creature.state.position,
            creature.state.depth,
            goal,
            goal_depth,
        );
        self.step(creature, waypoint, speed, dt);
        creature.state.position == goal && same_depth(creature.state.depth, goal_depth)
    }

    /// The next place on the way from `at`, `depth` back, to `goal`, `goal_depth` back. At the
    /// same depth that is the goal itself. Otherwise it is the foot or head of the door path that
    /// takes it from the one depth to the other with the least walking — any path running back
    /// as far as the further of the two — and, once there, the other end of it. A village with
    /// no such path left, because a house moved under somebody standing on its doorstep, is
    /// stepped down where it stands.
    pub(super) fn toward(&self, at: Point, depth: f32, goal: Point, goal_depth: f32) -> Waypoint {
        if (depth - goal_depth).abs() <= SAME_DEPTH {
            return Waypoint {
                point: goal,
                depth: goal_depth,
            };
        }
        let reach = depth.max(goal_depth) - SAME_DEPTH;
        let crossing = self
            .doors
            .iter()
            .filter(|door| door.depth >= reach)
            .map(|door| door.x)
            .min_by(|a, b| {
                let detour = |x: f32| (at.x - x).abs() + (x - goal.x).abs();
                detour(*a).total_cmp(&detour(*b))
            })
            .unwrap_or(at.x);
        if (at.x - crossing).abs() <= 0.5 {
            Waypoint {
                point: Point {
                    x: crossing,
                    y: at.y,
                },
                depth: goal_depth,
            }
        } else {
            Waypoint {
                point: Point {
                    x: crossing,
                    y: goal.y,
                },
                depth,
            }
        }
    }

    /// One step toward `waypoint` for somebody walking `speed` points a second: up or down the
    /// path it is standing at the foot or head of, or else along the strip. Returns whether it
    /// is there.
    pub(super) fn step(
        &self,
        creature: &mut Creature,
        waypoint: Waypoint,
        speed: f32,
        dt: f32,
    ) -> bool {
        let state = &mut creature.state;
        // Up or down, all the way, before another step along: even the last sliver of the way
        // up is taken standing at the path, never while walking off along the strip.
        if state.depth != waypoint.depth {
            let rise = speed * DEPTH_PACE / self.scale.max(0.01) * dt;
            state.depth = if state.depth < waypoint.depth {
                (state.depth + rise).min(waypoint.depth)
            } else {
                (state.depth - rise).max(waypoint.depth)
            };
            return false;
        }
        let distance = state.position.distance(waypoint.point);
        if distance > 0.0 {
            state.facing_right = waypoint.point.x >= state.position.x;
            state.position = lerp_point(
                state.position,
                waypoint.point,
                (speed * dt / distance).clamp(0.0, 1.0),
            );
        }
        state.position == waypoint.point
    }
}

/// How fast somebody no longer walking the village comes back to the front of it, in shelter
/// pixels a second: a visit over, picked up, gone off to something else.
const STEP_FORWARD_PACE: f32 = 14.0;

impl World {
    /// Brings everybody the village did not walk this tick back to the front of the grass: a
    /// visit that has ended, a companion picked up or busy with something else. Whoever the
    /// village did walk keeps the depth it walked to.
    pub(super) fn step_forward_out_of_the_village(&mut self, dt: f32, kept: &[CreatureId]) {
        for creature in &mut self.save.creatures {
            if creature.state.depth > 0.0 && !kept.contains(&creature.id) {
                creature.state.depth = (creature.state.depth - STEP_FORWARD_PACE * dt).max(0.0);
            }
        }
    }
}

impl World {
    /// Somebody picked up from somewhere back in the village is lifted from where it is drawn,
    /// not from the front of the grass below it: it stays under the hand that took hold of it,
    /// and is carried from there at the front.
    pub(super) fn lift_out_of_the_village(&mut self, desktop: &DesktopSnapshot) {
        let Some(interaction) = self.interaction.as_mut() else {
            return;
        };
        let Some(creature) = self
            .save
            .creatures
            .iter_mut()
            .find(|creature| creature.id == interaction.creature_id)
        else {
            return;
        };
        if creature.state.depth <= 0.0 {
            return;
        }
        let points_per_pixel = desktop
            .monitors
            .iter()
            .find(|monitor| monitor.id == creature.state.surface.monitor_id)
            .map_or(1.0, |monitor| {
                f32::from(self.save.settings.display_scale) / monitor.scale_factor.max(1.0)
            });
        let lift = creature.state.depth * points_per_pixel;
        creature.state.position.y -= lift;
        interaction.grab_offset.y += lift;
        creature.state.depth = 0.0;
    }
}

impl World {
    /// Whether this companion is out of sight behind a house nearer the front of the village:
    /// out of reach of the pointer as well, which takes the house it is behind.
    pub fn behind_a_house(&self, creature_id: CreatureId) -> bool {
        self.behind_houses.contains(&creature_id)
    }
}
