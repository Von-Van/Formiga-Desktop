//! Jump'n'Catch: a companion climbs a window and leaps off the top of it, and a friend on the
//! ground judges where it will come down, dashes there, and catches it overhead — or misjudges it,
//! and it lands on its own. Like every game it consults window rectangles only, never what any
//! window contains, and it is runtime only: nothing about a round is saved. Returning `false`
//! winds the scene down.
use super::super::surfaces::support_below;
use super::play::{PlayRole, Session};
use super::*;

/// The least drop worth leaping off, at creature scale: anything lower is only a hop down.
const LEAP_DROP: f32 = 100.0;
/// How far out from the window's edge a leap comes down, at creature scale: a timid push off to a
/// bold spring. Boldness leans it outward; the rest is the moment.
const LEAP_REACH: (f32, f32) = (36.0, 110.0);
/// How far above the ledge a leap rises before it comes down, at creature scale.
const LEAP_APEX: f32 = 24.0;
/// What brings a leap back down, in points a second squared: a little gentler than a throw
/// (`interaction.rs`), so the one on the ground has a moment to get there.
const LEAP_GRAVITY: f32 = 900.0;
/// Where the catcher waits, out from the edge, at creature scale: past the farthest leap, so every
/// catch is a dash back in toward the window, and the leap is never straight onto its head.
const WAIT_OUT: f32 = 150.0;
/// How far a jumper or a catcher will go to take its place, at creature scale.
const GATHER_REACH: f32 = 260.0;
/// How near under a friend a catcher has to be when it comes down to arm's height, at creature
/// scale.
const CATCH_REACH: f32 = 18.0;
/// The most a catcher misjudges where a friend will come down, at creature scale. The least
/// attentive catcher can be out by this much either way; a watchful one, by very little.
const MISJUDGED: f32 = 48.0;
/// Seconds at the brink before the leap: a wobble at the edge, then wound up to spring.
const TEETER_SECS: f32 = 0.6;
const CROUCH_SECS: f32 = 0.7;
/// Seconds a caught friend is held up overhead, and then being set down beside the catcher.
const HELD_SECS: f32 = 0.9;
const SET_DOWN_SECS: f32 = 0.45;
/// How long the game waits for both of them to take their places before the climb, and for the
/// climb itself, before it gives up.
const GATHER_SECS: f32 = 0.8;
const CLIMB_PATIENCE: f32 = 14.0;
/// How long a jumper may take to walk along the top to its edge.
const EDGE_PATIENCE: f32 = 6.0;
/// How long the ending lasts: delight at a catch, a shake-off and a shrug at a miss.
const ENDING_SECS: f32 = 2.8;
/// How near its place along a surface a member counts as being there.
const IN_PLACE: f32 = 6.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Phase {
    /// Both look up at the window; the catcher heads for its place.
    Gather,
    /// The jumper climbs the window, on the ordinary climb anybody takes.
    Climb,
    /// Along the top to the edge.
    Edge,
    /// A wobble at the edge, then a crouch.
    Brink,
    /// In the air, with the catcher running for where it judged it would come down.
    Leap,
    /// Caught, held up overhead, and set down beside the catcher.
    Held,
    /// It came down out of reach and is landing on its own.
    Missed,
    /// Over, either way.
    Ending,
}

/// Where a round is played: the window climbed, which edge it is leapt from, and where the catcher
/// waits on the ground below.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Spot {
    pub(super) window: WindowKey,
    /// The edge leapt from, and the top of the window it is the edge of.
    pub(super) edge: f32,
    pub(super) top: f32,
    /// Which way the leap goes off the edge: positive is to the right.
    pub(super) direction: f32,
    /// Where the jumper stands on top to leap: as near the edge as its whole body stays on it.
    pub(super) brink: f32,
    pub(super) wait: Point,
}

impl Spot {
    fn edge(&self) -> f32 {
        self.edge
    }

    fn brink(&self) -> f32 {
        self.brink
    }
}

/// How far a round has got: all a session keeps of it beyond the fields every travelling game
/// shares, so a round costs the play runtime two bytes. The window it is played on is the
/// session's goal, as a race's finish line is; its places are kept in the session's four trail
/// points, as hide and seek keeps a last sighting there; and the moment the current phase began
/// is the session's turn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Progress {
    pub(super) phase: Phase,
    pub(super) caught: bool,
}

/// Which trail point holds which place of a round.
const TRAIL_EDGE: usize = 0;
const TRAIL_WAIT: usize = 1;
const TRAIL_LANDING: usize = 2;
/// Where the catcher judged the leap would come down, on the ground.
pub(super) const TRAIL_GUESS: usize = 3;

/// Lay a round out over a session that has just begun.
pub(super) fn begin_round(s: &mut Session, spot: Spot) {
    s.goal = Some(spot.window);
    s.trail = [None; 4];
    s.trail[TRAIL_EDGE] = Some(Point {
        x: spot.edge,
        y: spot.top,
    });
    s.trail[TRAIL_WAIT] = Some(spot.wait);
    s.turn_started = 0.0;
    s.catch = Some(Progress {
        phase: Phase::Gather,
        caught: false,
    });
}

/// One round as the game works on it within a tick, unpacked from its session and packed back.
#[derive(Clone, Copy, Debug)]
struct Round {
    spot: Spot,
    phase: Phase,
    /// When the current phase began, in the scene's own seconds.
    since: f32,
    /// Where the leap comes down, and where the catcher judged it would.
    landing: Point,
    guess: f32,
    caught: bool,
}

impl Round {
    fn unpack(s: &Session, inset: f32) -> Option<Self> {
        let progress = s.catch?;
        let corner = s.trail[TRAIL_EDGE]?;
        let wait = s.trail[TRAIL_WAIT]?;
        let direction = if wait.x >= corner.x { 1.0 } else { -1.0 };
        Some(Self {
            spot: Spot {
                window: s.goal?,
                edge: corner.x,
                top: corner.y,
                direction,
                brink: corner.x - direction * inset,
                wait,
            },
            phase: progress.phase,
            since: s.turn_started,
            landing: s.trail[TRAIL_LANDING].unwrap_or_default(),
            guess: s.trail[TRAIL_GUESS].map_or(0.0, |guess| guess.x),
            caught: progress.caught,
        })
    }

    fn pack(&self, s: &mut Session) {
        s.catch = Some(Progress {
            phase: self.phase,
            caught: self.caught,
        });
        s.turn_started = self.since;
        if self.phase >= Phase::Leap {
            s.trail[TRAIL_LANDING] = Some(self.landing);
            s.trail[TRAIL_GUESS] = Some(Point {
                x: self.guess,
                y: self.landing.y,
            });
        }
    }

    fn enter(&mut self, phase: Phase, now: f32) {
        self.phase = phase;
        self.since = now;
    }
}

/// How high to lift a leap of `drop` points so that it rises `apex` points above where it started
/// before coming down. A hop's straight line plus its lift is one parabola, and this is the lift
/// that puts the top of that parabola at `apex`.
fn leap_lift(drop: f32, apex: f32) -> f32 {
    let b = 8.0 * drop + 16.0 * apex;
    (b + (b * b - 64.0 * drop * drop).max(0.0).sqrt()) / 32.0
}

/// How long a leap of `drop` points that rises `apex` points first is in the air.
fn leap_seconds(drop: f32, apex: f32) -> f32 {
    (2.0 * apex / LEAP_GRAVITY).sqrt() + (2.0 * (drop + apex) / LEAP_GRAVITY).sqrt()
}

impl World {
    /// Somewhere this pair could play: a window near the one who would climb it, tall enough to be
    /// worth the leap and no taller than it can climb, with its top open to stand on, and clear
    /// open ground beside one of its edges for the leap to come down on and the catcher to wait
    /// beyond. Both of them have to be on the ground already.
    pub(super) fn catch_spot(
        &self,
        jumper: &Creature,
        catcher: &Creature,
        desktop: &DesktopSnapshot,
    ) -> Option<Spot> {
        if !self.save.settings.window_ledges
            || jumper.state.surface.window_key.is_some()
            || catcher.state.surface.window_key.is_some()
        {
            return None;
        }
        let settings = &self.save.settings;
        let unit = creature_unit(jumper, settings, desktop);
        let monitor = desktop
            .monitors
            .iter()
            .find(|m| m.id == jumper.state.surface.monitor_id)?;
        let floor = jumper.state.position.y;
        let (max_rise, _) = traversal_ability(jumper);
        let head_rise = head_point(jumper, settings, desktop).y - jumper.state.position.y;
        let catcher_rise = head_point(catcher, settings, desktop).y - catcher.state.position.y;
        let inset = Self::brink_inset(monitor, settings.display_scale);
        desktop
            .windows
            .iter()
            .take(MAX_TOPOLOGY_WINDOWS)
            .filter(|w| {
                w.visible
                    && !w.minimized
                    && w.bounds.width >= 80.0 * unit
                    && (LEAP_DROP * unit..=max_rise).contains(&(floor - w.bounds.y))
            })
            .flat_map(|w| [-1.0, 1.0].map(|direction| (w, direction)))
            .filter_map(|(w, direction)| {
                let spot = Spot {
                    window: w.key,
                    edge: if direction > 0.0 {
                        w.bounds.right()
                    } else {
                        w.bounds.x
                    },
                    top: w.bounds.y,
                    direction,
                    brink: if direction > 0.0 {
                        w.bounds.right() - inset
                    } else {
                        w.bounds.x + inset
                    },
                    wait: Point { x: 0.0, y: floor },
                };
                let edge = spot.edge();
                let wait = Point {
                    x: edge + direction * WAIT_OUT * unit,
                    y: floor,
                };
                let stand = Point {
                    x: spot.brink(),
                    y: w.bounds.y + head_rise,
                };
                let open =
                    |x: f32| self.open_fall(x, w.bounds.y, floor, catcher_rise, monitor, desktop);
                (monitor.bounds.contains(stand)
                    && point_exposed(stand, Some(w.key), desktop)
                    && (jumper.state.position.x - edge).abs() <= GATHER_REACH * unit
                    && (catcher.state.position.x - wait.x).abs() <= GATHER_REACH * unit
                    && open(edge + direction * LEAP_REACH.0 * unit)
                    && open(edge + direction * LEAP_REACH.1 * unit)
                    && open(wait.x))
                .then_some(Spot { wait, ..spot })
            })
            .min_by(|a, b| {
                let far = |s: &Spot| {
                    (jumper.state.position.x - s.edge()).abs()
                        + (catcher.state.position.x - s.wait.x).abs()
                };
                far(a).total_cmp(&far(b))
            })
    }

    /// Whether something dropped at `x` from a ledge at `top` comes all the way down to the
    /// ground at `floor`, and can be stood under there: inside the habitat, under open sky, with
    /// no other window's top in between to land on instead.
    fn open_fall(
        &self,
        x: f32,
        top: f32,
        floor: f32,
        head_rise: f32,
        monitor: &MonitorInfo,
        desktop: &DesktopSnapshot,
    ) -> bool {
        let habitat = &self.save.settings.habitat;
        let half = body_half_width(monitor, self.save.settings.display_scale);
        [x - half, x, x + half]
            .into_iter()
            .all(|x| habitat_contains(habitat, monitor, Point { x, y: floor }))
            && point_exposed(
                Point {
                    x,
                    y: floor + head_rise,
                },
                None,
                desktop,
            )
            && !desktop.windows.iter().take(MAX_TOPOLOGY_WINDOWS).any(|w| {
                w.visible
                    && !w.minimized
                    && w.bounds.x <= x
                    && x <= w.bounds.right()
                    && w.bounds.y > top
                    && w.bounds.y < floor - 4.0
            })
    }

    /// The members of a round: whoever climbs and leaps leads, and the other one catches.
    fn catch_members(s: &Session) -> Option<(CreatureId, CreatureId)> {
        let jumper = s.with_role(PlayRole::Lead).next()?.1;
        let catcher = s.with_role(PlayRole::Follow).next()?.1;
        Some((jumper, catcher))
    }

    pub(super) fn advance_jump_and_catch(
        &mut self,
        s: &mut Session,
        desktop: &DesktopSnapshot,
        dt: f32,
    ) -> bool {
        let Some((jumper, catcher)) = Self::catch_members(s) else {
            return false;
        };
        let Some(inset) = self
            .save
            .creatures
            .iter()
            .find(|c| c.id == jumper)
            .and_then(|c| {
                desktop
                    .monitors
                    .iter()
                    .find(|m| m.id == c.state.surface.monitor_id)
            })
            .map(|m| Self::brink_inset(m, self.save.settings.display_scale))
        else {
            return false;
        };
        let Some(mut round) = Round::unpack(s, inset) else {
            return false;
        };
        let continuing = match round.phase {
            Phase::Gather => self.gather_for_catch(s, &mut round, jumper, catcher, desktop),
            Phase::Climb => self.climb_for_catch(s, &mut round, jumper, catcher, desktop),
            Phase::Edge => self.walk_to_brink(s, &mut round, jumper, catcher, desktop),
            Phase::Brink => self.teeter_and_leap(s, &mut round, jumper, catcher, desktop),
            Phase::Leap => self.run_to_catch(s, &mut round, jumper, catcher, desktop, dt),
            Phase::Held | Phase::Missed => self.come_down(s, &mut round, jumper, catcher, desktop),
            Phase::Ending => self.end_catch(s, &round, jumper, catcher, desktop),
        };
        round.pack(s);
        continuing
    }

    /// The catcher walks to its place while both of them look up at the top of the window; then
    /// the jumper sets off up it.
    fn gather_for_catch(
        &mut self,
        s: &mut Session,
        round: &mut Round,
        jumper: CreatureId,
        catcher: CreatureId,
        desktop: &DesktopSnapshot,
    ) -> bool {
        let spot = round.spot;
        let Some(bounds) = self.catch_window(&spot, desktop) else {
            return false;
        };
        let top = Point {
            x: spot.brink(),
            y: bounds.y,
        };
        self.take_catching_place(catcher, spot.wait.x, top, desktop);
        self.steer_play(jumper, None, ActionKind::InspectScreen);
        self.look_at(jumper, top);
        if s.elapsed - round.since < GATHER_SECS {
            return true;
        }
        let Some(creature) = self.save.creatures.iter().find(|c| c.id == jumper) else {
            return false;
        };
        let surface = SurfaceAttachment {
            kind: SurfaceKind::WindowLedge,
            monitor_id: creature.state.surface.monitor_id,
            window_key: Some(spot.window),
            relative_x: ((top.x - bounds.x) / bounds.width).clamp(0.05, 0.95),
        };
        let journey = build_window_journey(creature, top, surface, desktop);
        if !matches!(journey, WindowJourney::Climb(_)) {
            return false;
        }
        self.window_journeys.insert(jumper, journey);
        self.leave_ground(jumper);
        round.enter(Phase::Climb, s.elapsed);
        true
    }

    /// Up the window, watched from below. A climb that cannot finish, or ends anywhere but on top
    /// of this window, is the end of the game.
    fn climb_for_catch(
        &mut self,
        s: &mut Session,
        round: &mut Round,
        jumper: CreatureId,
        catcher: CreatureId,
        desktop: &DesktopSnapshot,
    ) -> bool {
        let spot = round.spot;
        if s.elapsed - round.since > CLIMB_PATIENCE || self.catch_window(&spot, desktop).is_none() {
            return false;
        }
        let Some(head) = self.play_head(jumper, desktop) else {
            return false;
        };
        self.take_catching_place(catcher, spot.wait.x, head, desktop);
        if self.window_journeys.contains_key(&jumper) {
            return true;
        }
        let on_top = self
            .save
            .creatures
            .iter()
            .any(|c| c.id == jumper && c.state.surface.window_key == Some(spot.window));
        if !on_top {
            return false;
        }
        self.rebase_player(jumper, desktop);
        round.enter(Phase::Edge, s.elapsed);
        true
    }

    /// Along the top to the edge the leap is taken from, looking out over the drop.
    fn walk_to_brink(
        &mut self,
        s: &mut Session,
        round: &mut Round,
        jumper: CreatureId,
        catcher: CreatureId,
        desktop: &DesktopSnapshot,
    ) -> bool {
        let spot = round.spot;
        if s.elapsed - round.since > EDGE_PATIENCE {
            return false;
        }
        let Some(here) = self.play_position(jumper) else {
            return false;
        };
        let Some(head) = self.play_head(jumper, desktop) else {
            return false;
        };
        self.take_catching_place(catcher, spot.wait.x, head, desktop);
        let out = Point {
            x: spot.edge() + spot.direction * WAIT_OUT * 0.5,
            y: spot.wait.y,
        };
        if (here.x - spot.brink()).abs() > IN_PLACE {
            let goal = self.step_toward(jumper, spot.brink(), desktop);
            if goal.is_none() {
                return false;
            }
            self.steer_play(jumper, goal, ActionKind::Traverse);
            self.look_at(jumper, out);
            return true;
        }
        self.steer_play(jumper, None, ActionKind::Perch);
        self.look_at(jumper, out);
        round.enter(Phase::Brink, s.elapsed);
        true
    }

    /// A wobble at the edge, then wound up low, then the leap. The catcher winds up too.
    fn teeter_and_leap(
        &mut self,
        s: &mut Session,
        round: &mut Round,
        jumper: CreatureId,
        catcher: CreatureId,
        desktop: &DesktopSnapshot,
    ) -> bool {
        let spot = round.spot;
        let at = s.elapsed - round.since;
        let Some(head) = self.play_head(jumper, desktop) else {
            return false;
        };
        let catcher_head = self.play_head(catcher, desktop).unwrap_or(spot.wait);
        let out = Point {
            x: spot.edge() + spot.direction * WAIT_OUT * 0.5,
            y: spot.wait.y,
        };
        self.steer_play(jumper, None, ActionKind::Perch);
        self.take_catching_place(catcher, spot.wait.x, head, desktop);
        if at < TEETER_SECS {
            self.look_at(jumper, catcher_head);
            self.pose_play(jumper, Gesture::Balance);
            return true;
        }
        if at < TEETER_SECS + CROUCH_SECS {
            self.look_at(jumper, out);
            self.pose_play(jumper, Gesture::Crouch);
            if self.catcher_waiting(catcher, spot.wait.x) {
                self.pose_play(catcher, Gesture::Crouch);
            }
            return true;
        }
        self.leap_off(s, round, jumper, catcher, desktop)
    }

    /// Off the edge on a thrown arc, out to wherever this leap carries it; the catcher judges
    /// where that is, as well as it can.
    fn leap_off(
        &mut self,
        s: &mut Session,
        round: &mut Round,
        jumper: CreatureId,
        catcher: CreatureId,
        desktop: &DesktopSnapshot,
    ) -> bool {
        let spot = round.spot;
        let Some(c) = self.save.creatures.iter().find(|c| c.id == jumper) else {
            return false;
        };
        let unit = creature_unit(c, &self.save.settings, desktop);
        let spring = c.personality.boldness * 0.35 + self.ambient_rng.random_range(0.0..0.65);
        let reach = LEAP_REACH.0 + (LEAP_REACH.1 - LEAP_REACH.0) * spring;
        // Wherever it actually comes down has to be the ground; a nearer landing will do if the
        // farther one is not, and with neither the jumper thinks better of it.
        let Some((landing, surface)) = [reach, LEAP_REACH.0].into_iter().find_map(|reach| {
            support_below(
                c,
                spot.edge() + spot.direction * reach * unit,
                desktop,
                &self.save.settings,
            )
            .filter(|(_, surface)| surface.window_key.is_none())
        }) else {
            return false;
        };
        let start = c.state.position;
        let drop = (landing.y - start.y).max(1.0);
        let apex = LEAP_APEX * unit;
        self.window_journeys.insert(
            jumper,
            WindowJourney::Hop(HopJourney {
                start,
                target: landing,
                surface,
                elapsed: 0.0,
                duration: leap_seconds(drop, apex),
                lift: leap_lift(drop, apex),
                ballistic: true,
            }),
        );
        self.leave_ground(jumper);
        // A watchful catcher, one keyed up and ready, and one that knows this friend well all
        // read the leap better; nobody reads it perfectly.
        let judgement = self
            .save
            .creatures
            .iter()
            .find(|c| c.id == catcher)
            .map_or(0.0, |c| {
                let affinity = self
                    .save
                    .relationships
                    .iter()
                    .find(|r| {
                        (r.a == jumper && r.b == catcher) || (r.a == catcher && r.b == jumper)
                    })
                    .map_or(0.0, |r| f32::from(r.affinity) / 255.0);
                (c.personality.curiosity * 0.45 + c.state.drives.energy * 0.25 + affinity * 0.3)
                    .clamp(0.0, 1.0)
            });
        let error =
            (1.0 - judgement) * MISJUDGED * unit * self.ambient_rng.random_range(-1.0_f32..1.0);
        round.landing = landing;
        round.guess = landing.x + error;
        round.enter(Phase::Leap, s.elapsed);
        s.turn_started = s.elapsed;
        true
    }

    /// The catcher runs for where it judged the leap would come down and waits there with its
    /// arms up. Whether it is under its friend when its friend comes down to arm's height decides
    /// it: held up overhead, or landing on its own.
    fn run_to_catch(
        &mut self,
        s: &mut Session,
        round: &mut Round,
        jumper: CreatureId,
        catcher: CreatureId,
        desktop: &DesktopSnapshot,
        dt: f32,
    ) -> bool {
        let Some(falling) = self.play_position(jumper) else {
            return false;
        };
        let Some(arms) = self.arms_above(catcher, desktop) else {
            return false;
        };
        self.look_at(catcher, falling);
        let goal = self
            .save
            .creatures
            .iter()
            .find(|c| c.id == catcher)
            .and_then(|c| motion::catch_goal(self, c, round.guess, desktop, jumper));
        let running = self
            .attention
            .plans
            .get(&catcher)
            .is_some_and(|p| p.walk.is_some());
        match goal {
            Some(goal) if !running => self.dash_play(catcher, goal, jumper),
            Some(_) => {}
            None if !running => {
                self.steer_play(catcher, None, ActionKind::InspectScreen);
                self.pose_play(catcher, Gesture::Stretch);
            }
            None => {}
        }
        // Decided the moment before it comes past arm's height, from where this tick's step of
        // the leap will take it, so a catch never has to pull it back up out of the catcher's way.
        let Some(WindowJourney::Hop(leap)) = self.window_journeys.get(&jumper).cloned() else {
            // Down already, by some other way than the leap: nothing left to catch.
            round.enter(Phase::Missed, s.elapsed);
            return true;
        };
        let next = WindowJourney::Hop(leap.clone())
            .advance(dt.max(0.01))
            .position;
        if next.y < arms.y {
            return true;
        }
        let unit = self.play_unit(catcher, desktop);
        if (arms.x - falling.x).abs() > CATCH_REACH * unit {
            // Out of reach as it comes past: it lands on its own.
            self.steer_play(catcher, None, ActionKind::InspectScreen);
            self.pose_play(catcher, Gesture::Gasp);
            round.enter(Phase::Missed, s.elapsed);
            return true;
        }
        // Caught. The rest of the leap is taken over by the catch: held where it was caught, up
        // on the catcher's raised paws, and then a little hop down beside it. The hold is the
        // hop's own wait before it sets off, so even a game called off mid-hold still sets it
        // down rather than leaving it up there with nobody holding it.
        let beside = self.held_gap(jumper, desktop).max(20.0 * unit);
        let monitor = desktop
            .monitors
            .iter()
            .find(|m| m.id == leap.surface.monitor_id);
        // Down on the window's side of the catcher, where it came from, or on the far side when
        // that is not open ground.
        let Some(down) = [-round.spot.direction, round.spot.direction]
            .into_iter()
            .map(|side| Point {
                x: arms.x + side * beside,
                y: round.landing.y,
            })
            .find(|point| {
                monitor.is_some_and(|m| habitat_contains(&self.save.settings.habitat, m, *point))
            })
        else {
            round.enter(Phase::Missed, s.elapsed);
            return true;
        };
        let held = Point {
            x: falling.x,
            y: arms.y,
        };
        self.window_journeys.insert(
            jumper,
            WindowJourney::Hop(HopJourney {
                start: held,
                target: down,
                surface: leap.surface,
                elapsed: -HELD_SECS,
                duration: SET_DOWN_SECS,
                lift: 10.0 * unit,
                ballistic: false,
            }),
        );
        if let Some(c) = creature_mut(&mut self.save.creatures, jumper) {
            c.state.position = held;
        }
        self.steer_play(catcher, None, ActionKind::InspectScreen);
        self.pose_play(catcher, Gesture::Stretch);
        round.caught = true;
        round.enter(Phase::Held, s.elapsed);
        true
    }

    /// Back on the ground, set down or landed. Once it is, both go back to playing on the ground,
    /// and the ending begins.
    fn come_down(
        &mut self,
        s: &mut Session,
        round: &mut Round,
        jumper: CreatureId,
        catcher: CreatureId,
        desktop: &DesktopSnapshot,
    ) -> bool {
        let Some(falling) = self.play_position(jumper) else {
            return false;
        };
        let held = round.phase == Phase::Held && s.elapsed - round.since < HELD_SECS;
        if held {
            // Holding it up: paws overhead, looking up at it, and it looking down at its catcher.
            if let Some(catcher_head) = self.play_head(catcher, desktop) {
                self.look_at(jumper, catcher_head);
            }
            self.steer_play(catcher, None, ActionKind::InspectScreen);
            self.pose_play(catcher, Gesture::Stretch);
        }
        self.look_at(catcher, falling);
        if round.phase == Phase::Missed {
            self.steer_play(catcher, None, ActionKind::InspectScreen);
            self.pose_play(
                catcher,
                if s.elapsed - round.since < 0.4 {
                    Gesture::Gasp
                } else {
                    Gesture::Worry
                },
            );
        } else if !held {
            self.steer_play(catcher, None, ActionKind::InspectScreen);
        }
        if self.window_journeys.contains_key(&jumper) {
            return true;
        }
        self.rebase_player(jumper, desktop);
        if round.caught {
            for id in [jumper, catcher] {
                self.show_bubble(id, BubbleIcon::Heart);
            }
        } else {
            self.show_bubble(jumper, BubbleIcon::Dizzy);
        }
        round.enter(Phase::Ending, s.elapsed);
        s.ends_at = s.ends_at.min(s.elapsed + ENDING_SECS);
        true
    }

    /// A catch is cheered by both of them; a miss is shaken off by the one that landed, while the
    /// one that missed it frets a moment and then the two of them make light of it.
    fn end_catch(
        &mut self,
        s: &mut Session,
        round: &Round,
        jumper: CreatureId,
        catcher: CreatureId,
        desktop: &DesktopSnapshot,
    ) -> bool {
        let at = s.elapsed - round.since;
        let (Some(jumper_head), Some(catcher_head)) = (
            self.play_head(jumper, desktop),
            self.play_head(catcher, desktop),
        ) else {
            return false;
        };
        self.steer_play(jumper, None, ActionKind::Greet);
        self.steer_play(catcher, None, ActionKind::Greet);
        self.look_at(jumper, catcher_head);
        self.look_at(catcher, jumper_head);
        // A leap that came down right on top of the catcher leaves them in a heap, and nobody can
        // see a face in a heap: the catcher steps clear first, and is glad of it from there.
        let apart = catcher_head.x - jumper_head.x;
        let clear = self.held_gap(catcher, desktop);
        if apart.abs() < clear - IN_PLACE {
            let side = if apart == 0.0 {
                round.spot.direction
            } else {
                apart.signum()
            };
            if let Some(goal) = self.step_toward(catcher, jumper_head.x + side * clear, desktop) {
                self.steer_play(catcher, Some(goal), ActionKind::Traverse);
                return true;
            }
        }
        if round.caught {
            self.pose_play(jumper, Gesture::Cheer);
            self.pose_play(catcher, Gesture::Cheer);
            self.feel(jumper, AttentionEmotion::Enjoying);
            self.feel(catcher, AttentionEmotion::Enjoying);
        } else if at < 1.0 {
            self.steer_play(jumper, None, ActionKind::Idle);
            self.steer_play(catcher, None, ActionKind::InspectScreen);
            self.pose_play(jumper, Gesture::Shake);
            self.pose_play(catcher, Gesture::Worry);
            self.feel(jumper, AttentionEmotion::Relieved);
            self.feel(catcher, AttentionEmotion::Concerned);
        } else {
            self.feel(jumper, AttentionEmotion::Enjoying);
            self.feel(catcher, AttentionEmotion::Enjoying);
        }
        true
    }

    /// The window a round is played on, while its top and the edge leapt from are still where
    /// they were when the round began.
    fn catch_window(&self, spot: &Spot, desktop: &DesktopSnapshot) -> Option<DesktopRect> {
        desktop
            .windows
            .iter()
            .take(MAX_TOPOLOGY_WINDOWS)
            .find(|w| {
                let edge = if spot.direction > 0.0 {
                    w.bounds.right()
                } else {
                    w.bounds.x
                };
                w.key == spot.window
                    && w.visible
                    && !w.minimized
                    && w.bounds.y == spot.top
                    && edge == spot.edge
            })
            .map(|w| w.bounds)
    }

    /// How far in from the edge the jumper stands to leap: where a climb tops out, eighteen
    /// points in, and never nearer an edge than half its own width.
    fn brink_inset(monitor: &MonitorInfo, display_scale: u8) -> f32 {
        body_half_width(monitor, display_scale).max(18.0)
    }

    /// The catcher makes for its place on the ground, then waits there watching `watching`.
    fn take_catching_place(
        &mut self,
        catcher: CreatureId,
        wait: f32,
        watching: Point,
        desktop: &DesktopSnapshot,
    ) {
        self.look_at(catcher, watching);
        if self.catcher_waiting(catcher, wait) {
            self.steer_play(catcher, None, ActionKind::InspectScreen);
            self.pose_play(catcher, Gesture::Watch);
            return;
        }
        let goal = self.step_toward(catcher, wait, desktop);
        self.steer_play(
            catcher,
            goal,
            if goal.is_some() {
                ActionKind::Traverse
            } else {
                ActionKind::InspectScreen
            },
        );
    }

    fn catcher_waiting(&self, catcher: CreatureId, wait: f32) -> bool {
        self.play_position(catcher)
            .is_some_and(|here| (here.x - wait).abs() <= IN_PLACE)
    }

    /// A run flat out to `goal`, to be under `leaper` as it comes down.
    fn dash_play(&mut self, id: CreatureId, goal: Point, leaper: CreatureId) {
        let Some(plan) = self.attention.plans.get_mut(&id) else {
            return;
        };
        if let Role::Play { gesture, pose, .. } = &mut plan.role {
            *gesture = ActionKind::Sprint;
            *pose = None;
        }
        plan.walk = Some(ShortWalk::dashing(goal, leaper));
    }

    /// Off its feet: the climb or the leap has it until it is down, as a leapfrog vault does.
    fn leave_ground(&mut self, id: CreatureId) {
        if let Some(plan) = self.attention.plans.get_mut(&id)
            && let Role::Play {
                hopping, gesture, ..
            } = &mut plan.role
        {
            *hopping = true;
            *gesture = ActionKind::Landing;
            plan.walk = None;
        }
    }

    /// A member that has climbed, leapt or been set down plays on from the surface it is on now,
    /// and as it stands there: up on a ledge, a climb leaves it perched, and down on the ground a
    /// leap leaves it idle.
    fn rebase_player(&mut self, id: CreatureId, desktop: &DesktopSnapshot) {
        let Some(c) = self.save.creatures.iter().find(|c| c.id == id) else {
            return;
        };
        let action = c.state.action;
        let surface = (c.state.surface.monitor_id, c.state.surface.window_key);
        let bounds = surface.1.and_then(|key| {
            desktop
                .windows
                .iter()
                .find(|w| w.key == key)
                .map(|w| w.bounds)
        });
        if let Some(plan) = self.attention.plans.get_mut(&id) {
            plan.surface = surface;
            plan.action = action;
            if let Role::Play {
                bounds: held,
                gesture,
                ..
            } = &mut plan.role
            {
                *held = bounds;
                *gesture = action;
            }
        }
    }

    fn play_head(&self, id: CreatureId, desktop: &DesktopSnapshot) -> Option<Point> {
        self.save
            .creatures
            .iter()
            .find(|c| c.id == id)
            .map(|c| head_point(c, &self.save.settings, desktop))
    }

    /// Where a friend is held up overhead: standing on the catcher's raised paws, just above the
    /// top of its head.
    fn arms_above(&self, catcher: CreatureId, desktop: &DesktopSnapshot) -> Option<Point> {
        let c = self.save.creatures.iter().find(|c| c.id == catcher)?;
        let head = head_point(c, &self.save.settings, desktop);
        let unit = creature_unit(c, &self.save.settings, desktop);
        Some(Point {
            x: c.state.position.x,
            y: head.y - 10.0 * unit,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::play::Kind;
    use super::super::play::tests::{everyone_settled, scene, tick};
    use super::*;

    const FLOOR: f32 = 846.0;

    /// Two companions on the ground to the right of the colony's window, which stands 246 points
    /// tall: the first bold and playful enough to climb it and leap, the second there to catch.
    fn catching_scene() -> (World, DesktopSnapshot, OffsetDateTime) {
        let (mut w, d, now) = scene(false);
        w.save.creatures.truncate(2);
        for (index, c) in w.save.creatures.iter_mut().enumerate() {
            c.personality.playfulness = 1.0;
            c.personality.sociability = 0.9;
            c.personality.boldness = if index == 0 { 1.0 } else { 0.3 };
            c.personality.curiosity = 1.0;
            c.personality.activity = 1.0;
            c.state.drives = Drives::default();
            c.state.drives.energy = 0.9;
            c.state.drives.social_need = 0.0;
            c.state.surface = SurfaceAttachment {
                kind: SurfaceKind::ScreenFloor,
                monitor_id: 1,
                window_key: None,
                relative_x: 0.5,
            };
            c.state.position = Point {
                x: 860.0 + index as f32 * 140.0,
                y: FLOOR,
            };
            c.state.action = ActionKind::Idle;
            c.state.action_elapsed = 0.0;
            c.state.action_duration = 100.0;
            c.state.facing_right = index == 0;
        }
        (w, d, now)
    }

    /// Everything a round did that a person watching would notice, gathered tick by tick.
    #[derive(Default, Debug)]
    struct Seen {
        phases: Vec<Phase>,
        climbed: bool,
        caught: Option<bool>,
        /// Ticks the jumper spent held up above the catcher's head.
        held_overhead: usize,
        /// The catcher's feet were going as it dashed in.
        dashed_on_its_feet: bool,
        hearts: bool,
        dizzy: bool,
        /// Ticks of the ending both of them were still in.
        ending: usize,
        /// Stood face-clear of each other by the end of it.
        apart_at_end: bool,
        finished: bool,
    }

    /// Run the scene until a round has come and gone, calling `meddle` every tick of it.
    fn play_round(
        w: &mut World,
        d: &mut DesktopSnapshot,
        now: OffsetDateTime,
        mut meddle: impl FnMut(&mut World, &mut DesktopSnapshot),
    ) -> Seen {
        let mut seen = Seen::default();
        let (jumper, catcher) = (w.save.creatures[0].id, w.save.creatures[1].id);
        let mut started = false;
        for step in 1..1_400 {
            tick(w, d, now, step);
            let Some(s) = w
                .attention
                .play
                .session
                .filter(|s| s.kind == Kind::JumpAndCatch)
            else {
                if started {
                    seen.finished = true;
                    break;
                }
                continue;
            };
            started = true;
            let round = s.catch.expect("a round knows where it is played");
            if seen.phases.last() != Some(&round.phase) {
                seen.phases.push(round.phase);
            }
            let j = w.save.creatures.iter().find(|c| c.id == jumper).unwrap();
            let c = w.save.creatures.iter().find(|c| c.id == catcher).unwrap();
            seen.climbed |= j.state.surface.window_key == Some(701) && j.state.position.y < 610.0;
            if round.phase == Phase::Held
                && (j.state.position.x - c.state.position.x).abs() < 20.0
                && j.state.position.y < c.state.position.y - 20.0
            {
                seen.held_overhead += 1;
            }
            if round.phase == Phase::Leap && c.state.action == ActionKind::Sprint {
                seen.dashed_on_its_feet |= c.state.presence.stepping;
            }
            if matches!(round.phase, Phase::Ending) {
                seen.caught = Some(round.caught);
                if w.attention.playing(jumper) && w.attention.playing(catcher) {
                    seen.ending += 1;
                }
                seen.apart_at_end = (j.state.position.x - c.state.position.x).abs()
                    >= w.held_gap(catcher, d) - IN_PLACE - 1.0;
            }
            for bubble in &w.bubbles {
                seen.hearts |= bubble.icon == BubbleIcon::Heart;
                seen.dizzy |= bubble.creature_id == jumper && bubble.icon == BubbleIcon::Dizzy;
            }
            meddle(w, d);
        }
        seen
    }

    #[test]
    fn a_companion_climbs_a_window_leaps_and_a_quick_friend_catches_it_overhead() {
        let (mut w, mut d, now) = catching_scene();
        let seen = play_round(&mut w, &mut d, now, |_, _| {});
        assert!(seen.finished, "{seen:?}");
        assert_eq!(
            seen.phases,
            [
                Phase::Gather,
                Phase::Climb,
                Phase::Edge,
                Phase::Brink,
                Phase::Leap,
                Phase::Held,
                Phase::Ending
            ],
            "{seen:?}"
        );
        assert!(seen.climbed, "{seen:?}");
        assert_eq!(seen.caught, Some(true), "{seen:?}");
        // Held up overhead for most of a second, at fifty milliseconds a tick.
        assert!(seen.held_overhead >= 12, "{seen:?}");
        assert!(seen.dashed_on_its_feet, "{seen:?}");
        assert!(seen.hearts && !seen.dizzy, "{seen:?}");
        assert!(seen.apart_at_end, "{seen:?}");
        // The two of them are delighted together for a couple of seconds before it is over.
        assert!(seen.ending >= 40, "{seen:?}");
        assert!(everyone_settled(&w));
        assert!(w.window_journeys.is_empty());
        for c in &w.save.creatures {
            assert_eq!(c.state.surface.window_key, None, "both end on the ground");
            assert!(
                (c.state.position.y - FLOOR).abs() < 1.0,
                "{:?}",
                c.state.position
            );
        }
    }

    #[test]
    fn a_misjudged_leap_lands_on_its_own_and_is_shaken_off() {
        let (mut w, mut d, now) = catching_scene();
        // The catcher reads the leap as coming down a little further out than it does: it runs
        // to just past it, and the leap comes down out of its reach and nearly on top of it.
        let seen = play_round(&mut w, &mut d, now, |w, _| {
            if let Some(s) = &mut w.attention.play.session
                && s.catch.is_some_and(|round| round.phase == Phase::Leap)
                && let (Some(landing), Some(guess)) = (s.trail[TRAIL_LANDING], s.trail[TRAIL_GUESS])
            {
                s.trail[TRAIL_GUESS] = Some(Point {
                    x: landing.x + 24.0,
                    ..guess
                });
            }
        });
        assert!(seen.finished, "{seen:?}");
        assert!(seen.phases.contains(&Phase::Missed), "{seen:?}");
        assert_eq!(seen.caught, Some(false), "{seen:?}");
        assert_eq!(seen.held_overhead, 0, "{seen:?}");
        assert!(seen.dizzy && !seen.hearts, "{seen:?}");
        assert!(seen.ending >= 40, "{seen:?}");
        assert!(everyone_settled(&w));
        let jumper = &w.save.creatures[0];
        assert_eq!(jumper.state.surface.window_key, None);
        assert!(seen.apart_at_end, "nobody ends in a heap: {seen:?}");
        assert!((jumper.state.position.y - FLOOR).abs() < 1.0);
    }

    #[test]
    fn a_window_closed_mid_climb_calls_the_game_off_with_nobody_left_up_in_the_air() {
        let (mut w, mut d, now) = catching_scene();
        let seen = play_round(&mut w, &mut d, now, |w, d| {
            if w.attention
                .play
                .session
                .and_then(|s| s.catch)
                .is_some_and(|round| round.phase == Phase::Climb)
                && w.window_journeys.contains_key(&w.save.creatures[0].id)
            {
                d.windows.retain(|window| window.key != 701);
            }
        });
        assert!(seen.finished, "{seen:?}");
        assert!(!seen.phases.contains(&Phase::Leap), "{seen:?}");
        for step in 1_400..1_480 {
            tick(&mut w, &mut d, now, step);
        }
        assert!(everyone_settled(&w));
        for c in &w.save.creatures {
            assert!(
                (c.state.position.y - FLOOR).abs() < 1.0,
                "{:?}",
                c.state.position
            );
        }
    }

    #[test]
    fn nobody_leaps_under_reduced_motion_or_without_a_window_worth_leaping_off() {
        let runs: [fn(&mut World, &mut DesktopSnapshot); 3] = [
            |w, _| w.save.settings.reduce_motion = true,
            |_, d| d.windows.clear(),
            // Too low to be worth a leap: a hop down at most.
            |_, d| d.windows[0].bounds.y = 780.0,
        ];
        for setup in runs {
            let (mut w, mut d, now) = catching_scene();
            setup(&mut w, &mut d);
            for step in 1..600 {
                tick(&mut w, &mut d, now, step);
                assert!(
                    w.attention
                        .play
                        .session
                        .is_none_or(|s| s.kind != Kind::JumpAndCatch)
                );
            }
        }
    }

    #[test]
    fn a_leap_rises_to_its_apex_and_comes_down_where_it_was_aimed() {
        let (drop, apex) = (250.0, 24.0);
        let lift = leap_lift(drop, apex);
        let journey = HopJourney {
            start: Point { x: 0.0, y: 0.0 },
            target: Point { x: 100.0, y: drop },
            surface: SurfaceAttachment {
                kind: SurfaceKind::ScreenFloor,
                monitor_id: 1,
                window_key: None,
                relative_x: 0.5,
            },
            elapsed: 0.0,
            duration: leap_seconds(drop, apex),
            lift,
            ballistic: true,
        };
        let mut hop = WindowJourney::Hop(journey);
        let mut highest = 0.0_f32;
        let mut fell = Vec::new();
        let mut last = 0.0;
        loop {
            let step = hop.advance(0.01);
            highest = highest.min(step.position.y);
            fell.push(step.position.y - last);
            last = step.position.y;
            if step.complete {
                assert_eq!(step.position, Point { x: 100.0, y: drop });
                break;
            }
        }
        assert!(
            (highest + apex).abs() < 1.0,
            "rises {apex} first: {highest}"
        );
        // Thrown, not eased: every step down is longer than the one before it, right to the end.
        // The last step is the part of a tick left before touching down, so it is left out.
        fell.pop();
        let falling: Vec<f32> = fell.into_iter().skip_while(|dy| *dy <= 0.0).collect();
        assert!(
            falling.windows(2).all(|w| w[1] > w[0] - 1e-3),
            "{falling:?}"
        );
        // About a second in the air from a window's height.
        let seconds = leap_seconds(drop, apex);
        assert!((0.9..1.2).contains(&seconds), "{seconds}");
    }
}
