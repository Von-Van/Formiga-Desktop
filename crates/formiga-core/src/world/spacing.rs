//! How close two companions are allowed to end up, and what happens when they end up closer.
//!
//! Every creature draws as one 48x48 art frame, centred horizontally on its contact point. The
//! overlay draws them in `save.creatures` order, so a later creature is painted over an earlier
//! one. Overlap is fine for a moment — a tag, a vault, a hand-off — but a face that stays behind
//! another body is not, so anything that *chooses* where creatures come to rest spaces them by at
//! least the face-clear distance, and `resolve_overlaps` catches whatever slips through.
//!
//! This crate cannot depend on the art crate, so the two boxes below approximate the art. They are
//! validated against the real frames by
//! `formiga_art::renderer::tests::every_face_and_body_stays_inside_the_boxes_the_simulation_spaces_by`.
use super::*;

/// The closest two resting companions may stand, centre to centre, as a fraction of the width a
/// creature's frame draws at. This is shoulder to shoulder: bodies may touch and overlap a
/// little, but neither one's face is behind the other. Anything that arranges creatures to stay
/// put — the row at home, a sleep pile, a huddle — spaces them by at least this much.
///
/// A drawn face reaches at most 15 art pixels from the middle of its frame — the watching pose on
/// a long body; every other clip stays within 14 — and a drawn body 23, so 38 of 48 pixels is the
/// first separation at which no body pixel can land on a face pixel.
pub(super) const FACE_CLEAR_RATIO: f32 = 0.80;

/// The separation at which two frames share no pixel at all: twice the 23-pixel body box, over
/// the 48-pixel frame. Two awake companions who are simply standing about are held this far
/// apart, because there is no interaction to justify one being drawn through the other.
pub(super) const FULL_CLEAR_RATIO: f32 = 0.96;

/// How far apart two contact points may be vertically and still have one body over the other's
/// face. A face box sits 8 to 34 art pixels above its contact point and a body box 0 to 44, so
/// the bands stop meeting once the contact points are 36 of 48 pixels apart.
const FACE_BAND_RATIO: f32 = 0.75;

/// The same reach for body against body: two 44-pixel bodies stop meeting at 44 of 48 pixels.
const BODY_BAND_RATIO: f32 = 0.92;

/// How long a face may stay covered before somebody steps aside. Long enough that a tag, a vault,
/// or a hand-off reads as contact rather than a mistake; short enough to be over in a moment.
const COVER_GRACE_SECONDS: f32 = 1.25;

/// Two companions merely standing too close have longer, because nothing is hidden and the one
/// walking past is about to solve it by walking on.
const CROWD_GRACE_SECONDS: f32 = 2.0;

/// A step aside goes slightly past the distance that triggered it, so arriving does not leave the
/// pair balanced on the threshold and ready to trigger again.
const CLEAR_MARGIN: f32 = 1.08;

/// After a pair has been separated, it is left alone for this long. With the margin above, this
/// is what stops two resolvers passing the same pair back and forth.
const PAIR_COOLDOWN_SECONDS: f32 = 5.0;

/// The longest any scene is allowed to run. A contact a scene is still making is left alone while
/// the scene has less than this left, because the scene ends itself; a plan claiming longer than
/// any real scene does not earn the exemption.
const SCENE_DEADLINE_SECONDS: f32 = 21.0;

/// How long to wait before asking again when neither of a pair could be moved.
const RETRY_SECONDS: f32 = 0.5;

/// A sleeper shuffling over settles again within this long, or gives up and stays put.
const SHUFFLE_SECONDS: f32 = 3.0;

/// How fast a sleeper shuffles, in points per second. Slower than a walk: it never wakes up.
const SHUFFLE_SPEED: f32 = 30.0;

/// The shortest sideways move worth making, matching the shortest walk attention will set off on.
const STEP_POINTS: f32 = 8.0;

/// Four creatures make six pairs, and the colony is capped at four.
const MAX_PAIRS: usize = 6;
const MAX_SHUFFLES: usize = 4;

/// How wide a creature's frame draws, in desktop points.
pub(super) fn frame_width(display_scale: u8, monitor_scale: f32) -> f32 {
    CREATURE_ART_WIDTH * f32::from(display_scale) / monitor_scale.max(1.0)
}

/// How wide this creature draws where it is standing, in desktop points.
pub(super) fn creature_frame_width(
    creature: &Creature,
    display_scale: u8,
    desktop: &DesktopSnapshot,
) -> f32 {
    let monitor_scale = desktop
        .monitors
        .iter()
        .find(|monitor| monitor.id == creature.state.surface.monitor_id)
        .map_or(1.0, |monitor| monitor.scale_factor);
    frame_width(display_scale, monitor_scale)
}

/// The closest this creature may stand to a companion it is settling next to.
pub(super) fn face_clear_gap(
    creature: &Creature,
    display_scale: u8,
    desktop: &DesktopSnapshot,
) -> f32 {
    creature_frame_width(creature, display_scale, desktop) * FACE_CLEAR_RATIO
}

/// How long one pair has been drawn through each other, and how long it is being left alone.
#[derive(Clone, Copy)]
struct PairWatch {
    a: CreatureId,
    b: CreatureId,
    covered: f32,
    cooldown: f32,
}

/// A sleeper moving over a step without waking. Runtime only; nothing here is ever saved.
#[derive(Clone, Copy)]
struct Shuffle {
    creature: CreatureId,
    target_x: f32,
    remaining: f32,
    /// What the creature was doing when it set off. Anything else means something took it over.
    action: ActionKind,
}

/// Bounded, runtime-only bookkeeping for overlap handling.
#[derive(Default)]
pub(super) struct OverlapWatch {
    pairs: [Option<PairWatch>; MAX_PAIRS],
    shuffles: [Option<Shuffle>; MAX_SHUFFLES],
}

impl OverlapWatch {
    fn age(&mut self, dt: f32) {
        for watch in self.pairs.iter_mut().flatten() {
            watch.cooldown = (watch.cooldown - dt).max(0.0);
        }
    }

    fn watch(&mut self, a: CreatureId, b: CreatureId) -> &mut PairWatch {
        let existing = self
            .pairs
            .iter()
            .position(|slot| slot.is_some_and(|watch| watch.a == a && watch.b == b));
        let index = existing
            .or_else(|| self.pairs.iter().position(Option::is_none))
            .unwrap_or_else(|| {
                (0..MAX_PAIRS)
                    .min_by(|&x, &y| {
                        let covered = |i: usize| self.pairs[i].map_or(0.0, |watch| watch.covered);
                        covered(x).total_cmp(&covered(y))
                    })
                    .expect("the pair table is never empty")
            });
        self.pairs[index].get_or_insert(PairWatch {
            a,
            b,
            covered: 0.0,
            cooldown: 0.0,
        })
    }

    /// Drop everything about creatures the colony no longer has.
    fn retain(&mut self, present: &[CreatureId]) {
        for slot in &mut self.pairs {
            if slot.is_some_and(|watch| !present.contains(&watch.a) || !present.contains(&watch.b))
            {
                *slot = None;
            }
        }
        for slot in &mut self.shuffles {
            if slot.is_some_and(|shuffle| !present.contains(&shuffle.creature)) {
                *slot = None;
            }
        }
    }

    fn shuffling(&self, creature: CreatureId) -> bool {
        self.shuffles
            .iter()
            .flatten()
            .any(|shuffle| shuffle.creature == creature)
    }

    fn start_shuffle(&mut self, creature: CreatureId, target_x: f32, action: ActionKind) {
        let index = self
            .shuffles
            .iter()
            .position(|slot| slot.is_some_and(|shuffle| shuffle.creature == creature))
            .or_else(|| self.shuffles.iter().position(Option::is_none));
        if let Some(index) = index {
            self.shuffles[index] = Some(Shuffle {
                creature,
                target_x,
                remaining: SHUFFLE_SECONDS,
                action,
            });
        }
    }
}

/// Where a creature stands, and everything the resolver needs to know about it, gathered once so
/// the pair loop does not walk the colony repeatedly.
#[derive(Clone, Copy)]
struct Placed {
    id: CreatureId,
    position: Point,
    monitor: MonitorId,
    window: Option<WindowKey>,
    width: f32,
    action: ActionKind,
    hanging: f32,
    /// Still on its feet rather than part way through a walk of its own.
    settled: bool,
}

impl Placed {
    fn asleep(&self) -> bool {
        self.action == ActionKind::Sleep
    }

    /// Awake, on its feet, and simply standing about rather than doing anything with anybody.
    fn loitering(&self) -> bool {
        self.settled
            && matches!(
                self.action,
                ActionKind::Idle
                    | ActionKind::Perch
                    | ActionKind::RideWindow
                    | ActionKind::InspectScreen
            )
    }
}

/// True when the later-drawn `coverer` has its body across `covered`'s face.
fn face_is_covered(covered: &Placed, coverer: &Placed) -> bool {
    let width = covered.width.max(coverer.width);
    covered.monitor == coverer.monitor
        && (covered.position.x - coverer.position.x).abs() < width * FACE_CLEAR_RATIO
        && (covered.position.y - coverer.position.y).abs() < width * FACE_BAND_RATIO
}

/// True when two frames share any pixel at all.
fn bodies_overlap(a: &Placed, b: &Placed) -> bool {
    let width = a.width.max(b.width);
    a.monitor == b.monitor
        && (a.position.x - b.position.x).abs() < width * FULL_CLEAR_RATIO
        && (a.position.y - b.position.y).abs() < width * BODY_BAND_RATIO
}

/// How far apart this particular pair ought to be. Two companions loitering have no interaction
/// to justify being drawn through one another, so they separate completely; everybody else is
/// held to shoulder-to-shoulder, which is what piles, huddles, and held game poses want.
fn required_gap(a: &Placed, b: &Placed, shared_formation: bool) -> f32 {
    let width = a.width.max(b.width);
    if !shared_formation && a.loitering() && b.loitering() {
        width * FULL_CLEAR_RATIO
    } else {
        width * FACE_CLEAR_RATIO
    }
}

impl World {
    /// Keep companions from standing on top of one another for longer than a moment. Runs once at
    /// the end of every ordinary tick, after each creature has moved and been kept in its habitat.
    pub(super) fn resolve_overlaps(&mut self, dt: f32, desktop: &DesktopSnapshot) {
        self.advance_shuffles(dt, desktop);
        let present: Vec<_> = self.save.creatures.iter().map(|c| c.id).collect();
        self.overlaps.retain(&present);
        self.overlaps.age(dt);
        let placed = self.placements(desktop);
        if placed.len() < 2 {
            return;
        }
        // `save.creatures` order is draw order, so only the earlier creature's face can be behind
        // the later creature's body. Full separation is symmetric and handled alongside it.
        for later in 1..placed.len() {
            for earlier in 0..later {
                let (front, back) = (placed[later], placed[earlier]);
                let shared = self.shared_formation(back.id, front.id);
                let hidden = face_is_covered(&back, &front);
                let crowded = !shared
                    && back.loitering()
                    && front.loitering()
                    && bodies_overlap(&back, &front);
                let grace = if hidden {
                    COVER_GRACE_SECONDS
                } else {
                    CROWD_GRACE_SECONDS
                };
                let (covered, cooldown) = {
                    let watch = self.overlaps.watch(back.id, front.id);
                    if hidden || crowded {
                        watch.covered += dt;
                    } else {
                        watch.covered = 0.0;
                    }
                    (watch.covered, watch.cooldown)
                };
                if covered < grace || cooldown > 0.0 {
                    continue;
                }
                if self.contact_excused(back.id) || self.contact_excused(front.id) {
                    continue;
                }
                // Leaving a surface altogether is the last thing tried, so it waits out several
                // graces first: a companion crowding a ledge is usually only passing through.
                let pressing = hidden && covered >= COVER_GRACE_SECONDS * 5.0;
                let moved = self.step_one_aside(&back, &front, shared, pressing, &placed, desktop);
                let watch = self.overlaps.watch(back.id, front.id);
                if moved {
                    watch.covered = 0.0;
                    watch.cooldown = PAIR_COOLDOWN_SECONDS;
                } else {
                    // Nobody could be asked this time. Ask again shortly rather than on every
                    // tick, and keep the episode's own clock running.
                    watch.cooldown = RETRY_SECONDS;
                }
            }
        }
    }

    fn placements(&self, desktop: &DesktopSnapshot) -> Vec<Placed> {
        let display_scale = self.save.settings.display_scale;
        self.save
            .creatures
            .iter()
            .filter(|creature| creature.state.arrival_delay_secs <= 0.0)
            .map(|creature| {
                let monitor_scale = desktop
                    .monitors
                    .iter()
                    .find(|monitor| monitor.id == creature.state.surface.monitor_id)
                    .map_or(1.0, |monitor| monitor.scale_factor);
                Placed {
                    id: creature.id,
                    position: creature.state.position,
                    monitor: creature.state.surface.monitor_id,
                    window: creature.state.surface.window_key,
                    width: frame_width(display_scale, monitor_scale),
                    action: creature.state.action,
                    hanging: creature.state.attention.map_or(0.0, |pose| pose.hanging),
                    settled: creature.state.velocity.x.abs() < 1.0,
                }
            })
            .collect()
    }

    /// True when the two are held together on purpose: the same ritual, the same scene, or one
    /// approaching the other. Their formation is spaced face-clear where it is chosen, so the
    /// resolver holds them to that rather than pulling them fully apart.
    fn shared_formation(&self, a: CreatureId, b: CreatureId) -> bool {
        let ritual = self.colony_plan.as_ref().is_some_and(|plan| {
            let member = |id| plan.participants.iter().any(|p| p.creature_id == id);
            member(a) && member(b)
        });
        let bonded = self.bond_plans.get(&a).is_some_and(|plan| plan.target == b)
            || self.bond_plans.get(&b).is_some_and(|plan| plan.target == a);
        ritual || bonded || (self.attention.owns(a) && self.attention.owns(b))
    }

    /// Contact nobody should interrupt: the user has hold of this creature, it is off the ground,
    /// it is on its way up or hanging, or a scene is still making the contact and will end.
    fn contact_excused(&self, id: CreatureId) -> bool {
        let Some(creature) = self.save.creatures.iter().find(|c| c.id == id) else {
            return true;
        };
        if matches!(
            creature.state.action,
            ActionKind::Dragged
                | ActionKind::Tossed
                | ActionKind::ClimbWindow
                | ActionKind::Landing
        ) || creature
            .state
            .attention
            .is_some_and(|pose| pose.hanging > 0.05)
        {
            return true;
        }
        if self
            .interaction
            .as_ref()
            .is_some_and(|session| session.creature_id == id)
            || self.tosses.contains_key(&id)
            || self.window_journeys.contains_key(&id)
            || self.attention.airborne(id)
        {
            return true;
        }
        self.attention
            .contact_scene_remaining(id)
            .is_some_and(|remaining| remaining <= SCENE_DEADLINE_SECONDS)
    }

    /// What it would cost to ask this creature to move, or `None` if it cannot be asked at all.
    /// Lower moves first: awake before asleep, a bystander before the anchor of a pile, an empty
    /// moment before one holding a prop, and an idle companion before one that is mid-scene.
    fn step_aside_cost(&self, who: &Placed, desktop: &DesktopSnapshot) -> Option<u32> {
        // Already on its way; asking again would only restart the same move.
        if self.contact_excused(who.id) || who.hanging > 0.05 || self.overlaps.shuffling(who.id) {
            return None;
        }
        // A scene that owns this creature is steering it somewhere of its own. Overlap
        // handling never argues with that: it asks the other one, or waits for the scene to end,
        // which every scene does on a deadline of its own.
        if matches!(who.action, ActionKind::Homebound) || self.attention.owns(who.id) {
            return None;
        }
        let mut cost = 0;
        if who.asleep() {
            // Asleep is the last thing to disturb, and the longer a companion has been down the
            // less it is the one to ask: between two sleepers, the lighter one shuffles over.
            let slept = self.sleep_elapsed.get(&who.id).copied().unwrap_or(0.0);
            cost += 8 + (slept / 15.0).clamp(0.0, 4.0) as u32;
        }
        // A ritual can be asked to shuffle along, never to walk out of itself.
        if self.in_ritual(who.id) {
            cost += 7;
        }
        // Somebody is on their way to this creature, so it is the one being joined.
        if self
            .attention
            .reserved_spots()
            .any(|(id, point)| id != who.id && point.distance(who.position) < who.width)
            || self.bond_plans.values().any(|plan| plan.target == who.id)
        {
            cost += 6;
        }
        if who.action == ActionKind::PresentDiscovery {
            cost += 5;
        }
        if self.bond_plans.contains_key(&who.id) {
            cost += 2;
        }
        let _ = desktop;
        Some(cost)
    }

    /// Ask the better-placed of the two to take a short walk clear. Returns whether one set off.
    fn step_one_aside(
        &mut self,
        back: &Placed,
        front: &Placed,
        shared: bool,
        pressing: bool,
        placed: &[Placed],
        desktop: &DesktopSnapshot,
    ) -> bool {
        let costs = (
            self.step_aside_cost(back, desktop),
            self.step_aside_cost(front, desktop),
        );
        // Ties break on creature id, so the same pair always resolves the same way.
        let mover = match costs {
            (Some(back_cost), Some(front_cost)) => {
                if (front_cost, front.id) < (back_cost, back.id) {
                    front
                } else {
                    back
                }
            }
            (Some(_), None) => back,
            (None, Some(_)) => front,
            (None, None) => return false,
        };
        let Some(target_x) = self.clear_spot(mover, placed, shared, desktop) else {
            // Somewhere else entirely is the last resort, for a ledge with no room left and a
            // face that has been hidden for a while. A ledge that is merely crowded is left be:
            // the companion crowding it is usually passing through.
            return pressing && self.leave_the_surface(mover.id, desktop);
        };
        // A pose that must survive the move — a sleep, or a place in a ceremony — is shuffled out
        // of the way without being touched. Everybody else takes an ordinary walk.
        if mover.asleep() || self.in_ritual(mover.id) {
            self.shuffle_in_place(mover.id, target_x);
        } else {
            self.walk_aside(mover.id, target_x);
        }
        true
    }

    /// Every pair, in draw order, whose earlier face is behind the later body right now. This is
    /// the same geometry the resolver acts on, so a test can measure exactly what it enforces.
    #[cfg(test)]
    pub(super) fn covered_faces(&self, desktop: &DesktopSnapshot) -> Vec<(CreatureId, CreatureId)> {
        let placed = self.placements(desktop);
        let mut covered = Vec::new();
        for later in 1..placed.len() {
            for earlier in 0..later {
                if face_is_covered(&placed[earlier], &placed[later]) {
                    covered.push((placed[earlier].id, placed[later].id));
                }
            }
        }
        covered
    }

    /// Every pair of companions who are awake, standing about with nothing between them, and
    /// still drawn through one another. This is the stricter rule, and it applies to nobody who
    /// is asleep, busy, or part of a formation somebody put them in.
    #[cfg(test)]
    pub(super) fn crowded_pairs(&self, desktop: &DesktopSnapshot) -> Vec<(CreatureId, CreatureId)> {
        let placed = self.placements(desktop);
        let mut crowded = Vec::new();
        for later in 1..placed.len() {
            for earlier in 0..later {
                let (back, front) = (placed[earlier], placed[later]);
                if !self.shared_formation(back.id, front.id)
                    && back.loitering()
                    && front.loitering()
                    && bodies_overlap(&back, &front)
                {
                    crowded.push((back.id, front.id));
                }
            }
        }
        crowded
    }

    /// How long a face may stay covered before the resolver acts, plus the walk it then takes.
    #[cfg(test)]
    pub(super) const fn cover_grace() -> f32 {
        COVER_GRACE_SECONDS
    }

    #[cfg(test)]
    pub(super) const fn crowd_grace() -> f32 {
        CROWD_GRACE_SECONDS
    }

    fn in_ritual(&self, id: CreatureId) -> bool {
        self.colony_plan
            .as_ref()
            .is_some_and(|plan| plan.participants.iter().any(|p| p.creature_id == id))
    }

    /// The nearest point along this creature's own surface that clears every face, respecting the
    /// habitat, the ledge it is standing on, and spots companions have already claimed.
    fn clear_spot(
        &self,
        mover: &Placed,
        placed: &[Placed],
        shared: bool,
        desktop: &DesktopSnapshot,
    ) -> Option<f32> {
        let monitor = desktop
            .monitors
            .iter()
            .find(|monitor| monitor.id == mover.monitor)?;
        let regions = accessible_regions(&self.save.settings.habitat, monitor);
        let ledge = mover.window.and_then(|key| {
            desktop
                .windows
                .iter()
                .find(|window| window.key == key && window.visible && !window.minimized)
                .map(|window| window.bounds)
        });
        // Step away from the companion first, so the two do not both pick the same side.
        let away = placed
            .iter()
            .find(|other| other.id != mover.id && face_is_covered(mover, other))
            .map_or(1.0, |other| {
                if mover.position.x >= other.position.x {
                    1.0
                } else {
                    -1.0
                }
            });
        // How far the nearest companion sharing this row would be, from a given point.
        let elbow_room = |x: f32| {
            placed
                .iter()
                .filter(|other| {
                    let width = mover.width.max(other.width);
                    other.id != mover.id
                        && other.monitor == mover.monitor
                        && (mover.position.y - other.position.y).abs() < width * BODY_BAND_RATIO
                })
                .map(|other| {
                    // Measured against what this pair actually owes each other, so one companion
                    // being owed more than another does not hide behind an average.
                    (x - other.position.x).abs() / required_gap(mover, other, shared)
                })
                .fold(f32::INFINITY, f32::min)
        };
        // The same, against the face-clear distance alone: bodies may touch, faces may not.
        let face_room = |x: f32| {
            placed
                .iter()
                .filter(|other| {
                    let width = mover.width.max(other.width);
                    other.id != mover.id
                        && other.monitor == mover.monitor
                        && (mover.position.y - other.position.y).abs() < width * BODY_BAND_RATIO
                })
                .map(|other| {
                    (x - other.position.x).abs() / (mover.width.max(other.width) * FACE_CLEAR_RATIO)
                })
                .fold(f32::INFINITY, f32::min)
        };
        let standable = |x: f32| {
            let point = Point {
                x,
                y: mover.position.y,
            };
            if let Some(bounds) = ledge
                && (x < bounds.x + 12.0 || x > bounds.right() - 12.0)
            {
                return false;
            }
            regions.iter().any(|region| {
                region.contains(point) && x >= region.x + 8.0 && x <= region.right() - 8.0
            }) && self.spot_unclaimed(mover, point)
        };
        let reach = mover.width * FULL_CLEAR_RATIO * 2.0 + STEP_POINTS;
        let steps = (reach / STEP_POINTS).ceil() as i32;
        let candidates: Vec<f32> = (1..=steps)
            .flat_map(|step| {
                let offset = step as f32 * STEP_POINTS;
                [away * offset, -away * offset]
            })
            .map(|offset| mover.position.x + offset)
            .filter(|&x| standable(x))
            .collect();
        // The nearest spot that clears everybody, or — on a surface with no such spot — the one
        // that gives the most room, which is as much as walking can do about it.
        candidates
            .iter()
            .copied()
            .find(|&x| elbow_room(x) >= CLEAR_MARGIN)
            .or_else(|| {
                let here = elbow_room(mover.position.x);
                candidates
                    .iter()
                    .copied()
                    .max_by(|a, b| elbow_room(*a).total_cmp(&elbow_room(*b)))
                    // Bodies may still touch here, but every face has to come clear; a surface
                    // that cannot manage even that is one to leave.
                    .filter(|&x| face_room(x) >= 1.0 && elbow_room(x) > here * 1.1)
            })
    }

    /// Nobody else has walked toward this point or planned to land on it.
    fn spot_unclaimed(&self, mover: &Placed, point: Point) -> bool {
        let gap = mover.width * FACE_CLEAR_RATIO;
        !self
            .attention
            .reserved_spots()
            .any(|(id, spot)| id != mover.id && spot.distance(point) < gap)
            && !self.window_journeys.iter().any(|(id, journey)| {
                *id != mover.id && journey.surface().monitor_id == mover.monitor && {
                    let landing = match journey {
                        WindowJourney::Gap(gap) => gap.hop.target,
                        WindowJourney::Hop(hop) => hop.target,
                        WindowJourney::Climb(climb) => climb.target,
                        WindowJourney::Squeeze(squeeze) => squeeze.target,
                    };
                    landing.distance(point) < gap
                }
            })
    }

    /// Moves a creature over without disturbing what it is doing: its action, its elapsed time,
    /// and — for a sleeper — the rest it is banking are all left alone, so no interruption and no
    /// waking is recorded, and a ceremony keeps its participant.
    fn shuffle_in_place(&mut self, id: CreatureId, target_x: f32) {
        if self.save.settings.reduce_motion {
            if let Some(creature) = creature_mut(&mut self.save.creatures, id) {
                creature.state.position.x = target_x;
            }
            return;
        }
        let Some(action) = self
            .save
            .creatures
            .iter()
            .find(|creature| creature.id == id)
            .map(|creature| creature.state.action)
        else {
            return;
        };
        self.overlaps.start_shuffle(id, target_x, action);
    }

    fn advance_shuffles(&mut self, dt: f32, desktop: &DesktopSnapshot) {
        for index in 0..MAX_SHUFFLES {
            let Some(mut shuffle) = self.overlaps.shuffles[index] else {
                continue;
            };
            shuffle.remaining -= dt;
            let Some(creature) = creature_mut(&mut self.save.creatures, shuffle.creature) else {
                self.overlaps.shuffles[index] = None;
                continue;
            };
            // Anything that picked this creature up, carried it off, or started it moving on
            // its own owns it now.
            let done = shuffle.remaining <= 0.0
                || creature.state.action != shuffle.action
                || self.window_journeys.contains_key(&shuffle.creature)
                || self.tosses.contains_key(&shuffle.creature);
            if done {
                self.overlaps.shuffles[index] = None;
                continue;
            }
            let dx = shuffle.target_x - creature.state.position.x;
            let step = (SHUFFLE_SPEED * dt).min(dx.abs());
            creature.state.position.x += dx.signum() * step;
            if dx.abs() <= step + f32::EPSILON {
                self.overlaps.shuffles[index] = None;
            } else {
                self.overlaps.shuffles[index] = Some(shuffle);
            }
        }
        // Keep a shuffled sleeper on its own surface rather than letting it drift off a ledge.
        let shuffling: Vec<_> = self
            .overlaps
            .shuffles
            .iter()
            .flatten()
            .map(|shuffle| shuffle.creature)
            .collect();
        for id in shuffling {
            if let Some(creature) = creature_mut(&mut self.save.creatures, id) {
                constrain_to_surface(creature, desktop, &self.save.settings.habitat);
            }
        }
    }

    /// An awake companion takes an ordinary walk to the clear spot, after whatever was holding it
    /// has been let go of through its own cancellation rules.
    fn walk_aside(&mut self, id: CreatureId, target_x: f32) {
        self.bond_plans.remove(&id);
        let Some(creature) = creature_mut(&mut self.save.creatures, id) else {
            return;
        };
        // Nothing is announced. Stepping out of somebody's way is housekeeping, not a moment the
        // colony lived through, and a journal entry or a learned habit for it would be a lie.
        let target = Point {
            x: target_x,
            y: creature.state.position.y,
        };
        creature.state.action = ActionKind::Traverse;
        creature.state.action_elapsed = 0.0;
        // Only as long as the walk itself takes. A step aside is not an errand, and holding the
        // creature in it for longer would keep it out of whatever it would rather be doing.
        let speed = 24.0 + creature.personality.activity * 34.0;
        creature.state.action_duration =
            ((target_x - creature.state.position.x).abs() / speed + 0.3).clamp(0.4, 4.0);
        creature.state.facing_right = target_x >= creature.state.position.x;
        creature.state.velocity = Point::default();
        self.action_choices.insert(
            id,
            ActionChoice {
                action: ActionKind::Traverse,
                target_creature: None,
                target_point: Some(target),
            },
        );
    }

    /// A ledge with no room left. Rather than inventing a movement, the creature takes the drop
    /// the rest of the simulation already uses to get off a surface it cannot stay on.
    fn leave_the_surface(&mut self, id: CreatureId, desktop: &DesktopSnapshot) -> bool {
        let Some(creature) = self.save.creatures.iter().find(|c| c.id == id) else {
            return false;
        };
        if creature.state.surface.window_key.is_none() || self.window_journeys.contains_key(&id) {
            return false;
        }
        let Some((target, surface)) = find_drop_support(
            creature.state.position,
            desktop,
            &self.save.settings.habitat,
            false,
        ) else {
            return false;
        };
        let journey = build_window_journey(creature, target, surface, desktop);
        let next = journey.initial_action();
        self.window_journeys.insert(id, journey);
        self.window_routes.remove(&id);
        self.bond_plans.remove(&id);
        self.action_choices.remove(&id);
        if let Some(creature) = creature_mut(&mut self.save.creatures, id) {
            creature.state.action = next;
            creature.state.action_elapsed = 0.0;
            creature.state.action_duration = f32::MAX;
            creature.state.velocity = Point::default();
        }
        true
    }
}
