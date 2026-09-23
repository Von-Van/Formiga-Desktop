//! What residents do about the village while the houses are out, beyond strolling and the quiet
//! moments at their doors: tending the gardens, small chores at their own houses, going indoors
//! for a while, sitting up on the roof, turning up something at home — and the little mishaps
//! that happen along the way.
//!
//! Each is a short plan of a few steps — walk somewhere, do the thing, perhaps find something, go
//! back to strolling — and it owns the resident's feet while it lasts, the way a quiet moment at
//! the door does. Steps show through the resident's action and its `Beat`; the house it keeps,
//! the loose leaf or apple, and the curtain drawn across a door are read back by the overlay from
//! `World::house_occupancy`, `World::house_motions` and `World::loose_props`.
//!
//! Like everything else a village does, all of it is runtime only. A find at home is the one
//! thing that reaches the save, by the same `PresentDiscovery` completion any other find does.

use super::discovery::{self, DiscoveryCircumstances};
use super::*;

/// How fast a resident walks about the village on an errand of its own.
fn errand_speed(creature: &Creature) -> f32 {
    22.0 + creature.personality.activity * 18.0
}

/// The longest any one walk in a plan may take before the resident gives up on it and does its
/// thing where it has got to.
const WALK_LIMIT_SECS: f32 = 20.0;

/// One in this many chores, garden visits and roof sits turns something up.
const FIND_IN: u32 = 7;

/// What a garden visit is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum GardenTask {
    /// Watering a patch of flowers.
    Water,
    /// Crouched over a patch that is still coming up.
    Inspect,
    /// Picking something to eat, and eating it.
    Pick,
    /// Picking something and taking it over to show a friend.
    Show { friend: CreatureId },
}

/// Which plan a resident is following.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Plan {
    Garden {
        patch: GardenKind,
        /// Where to stand, beside the patch, and which way to face once there.
        stand: Point,
        face_right: bool,
        task: GardenTask,
    },
    /// A chore at its own door, in the way its house asks for.
    Chore {
        slot: usize,
        stand: Point,
        face_right: bool,
        chore: BeatKind,
    },
    /// Indoors for a while, behind the drawn curtain.
    Indoors {
        slot: usize,
        door: Point,
        stay: f32,
        nap: bool,
    },
    /// Up on the roof of its own house.
    Roof {
        slot: usize,
        beside: Point,
        top: Point,
        stay: f32,
    },
    /// A leaf came down onto its face.
    LeafOnFace,
    /// Its snack rolled away; after it, pick it up, and carry on eating.
    DroppedSnack {
        before: f32,
        from: Point,
        to: Point,
        eat: f32,
    },
    /// Sat down beside the cushion, and shuffled across onto it.
    MissedCushion {
        beside: Point,
        cushion: Point,
        nap: f32,
    },
}

/// The village as it stood when a plan began: its ground, where the resident's own house stood
/// and which of the houses it was, and where the patch it set off for was. A plan made for one
/// village is never carried out in another — a cottage moved along the row, a patch carried
/// across the ground, the corner changed — so a plan whose ground has changed is let go.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct PlanGround {
    pub(super) commons: HomeCommons,
    pub(super) house: Option<(Point, usize)>,
    pub(super) patch: Option<Point>,
}

/// A resident following a plan: which one, which step it has reached, and for how long it has
/// been on that step.
#[derive(Clone, Copy, Debug)]
pub(super) struct VillageActivity {
    pub(super) plan: Plan,
    step: u8,
    step_elapsed: f32,
    /// Something turned up at the end, in circumstances of its own.
    find: Option<DiscoveryCircumstances>,
    /// The ground the plan was made on, noted the first time it is carried on.
    ground: Option<PlanGround>,
}

impl VillageActivity {
    pub(super) fn new(plan: Plan) -> Self {
        Self {
            plan,
            step: 0,
            step_elapsed: 0.0,
            find: None,
            ground: None,
        }
    }

    /// Whether the village is still the one this plan was made on. The first time it is asked it
    /// notes the ground it is given; after that, any change lets the plan go.
    pub(super) fn still_on(&mut self, ground: Option<PlanGround>) -> bool {
        let Some(ground) = ground else {
            return false;
        };
        match self.ground {
            None => {
                self.ground = Some(ground);
                true
            }
            Some(before) => before == ground,
        }
    }

    fn next(&mut self) {
        self.step += 1;
        self.step_elapsed = 0.0;
    }

    /// Indoors right now, out of sight.
    pub(super) fn inside(&self) -> Option<(usize, bool)> {
        match self.plan {
            Plan::Indoors { slot, nap, .. } if self.step == 1 => Some((slot, nap)),
            _ => None,
        }
    }
}

/// What a resident's plan asks of the others this tick: a friend to look at a mishap, or to be
/// shown something grown.
#[derive(Clone, Copy, Debug)]
pub(super) struct Aside {
    pub(super) creature: CreatureId,
    pub(super) beat: BeatKind,
    pub(super) length: f32,
    pub(super) look: Point,
}

/// A step towards `goal` at `speed`, facing the way it goes. Returns whether it has arrived.
fn walk(creature: &mut Creature, goal: Point, speed: f32, dt: f32) -> bool {
    let start = creature.state.position;
    let distance = start.distance(goal);
    if distance <= 0.5 {
        creature.state.position = goal;
        return true;
    }
    creature.state.facing_right = goal.x >= start.x;
    creature.state.position = lerp_point(start, goal, (speed * dt / distance).clamp(0.0, 1.0));
    creature.state.position == goal
}

/// Shows `action`, announcing it if it is new, the way every clip a resident shows at home is
/// announced: as started, and never completed.
fn show(creature: &mut Creature, action: ActionKind, events: &mut Vec<WorldEvent>) {
    if creature.state.action != action {
        creature.state.action = action;
        creature.state.action_elapsed = 0.0;
        creature.state.action_duration = f32::MAX;
        World::emit(
            events,
            WorldEvent::ActionStarted {
                creature_id: creature.id,
                action,
            },
        );
    }
}

/// Starts a beat on a creature.
fn beat(creature: &mut Creature, kind: BeatKind, length: f32, held: Option<VillageProp>) {
    creature.state.beat = Some(Beat {
        held,
        ..Beat::new(kind, length)
    });
}

/// Whether the beat started for this step is over.
fn beat_done(creature: &Creature) -> bool {
    creature.state.beat.is_none_or(|beat| beat.finished())
}

/// A resident standing about or strolling at home with nothing on: somebody who could stop to
/// look at a mishap, or be shown something grown.
#[derive(Clone, Copy, Debug)]
pub(super) struct Onlooker {
    pub(super) id: CreatureId,
    pub(super) at: Point,
    /// Already stopped to look at something.
    pub(super) watching: bool,
}

/// Everything a plan needs to know about the village around it this tick.
pub(super) struct VillageContext<'a> {
    pub(super) rng: &'a mut ChaCha12Rng,
    pub(super) events: &'a mut Vec<WorldEvent>,
    pub(super) asides: &'a mut Vec<Aside>,
    pub(super) found: &'a [ScrapbookRecord],
    pub(super) now: OffsetDateTime,
    pub(super) created: OffsetDateTime,
    pub(super) with_visitor: bool,
    pub(super) onlookers: &'a [Onlooker],
    pub(super) frame: f32,
}

impl VillageContext<'_> {
    /// The nearest onlooker to `at` besides `besides` that is not already watching something,
    /// within four creature widths.
    fn nearest_resting(&self, at: Point, besides: CreatureId) -> Option<(CreatureId, Point)> {
        self.onlookers
            .iter()
            .filter(|onlooker| onlooker.id != besides && !onlooker.watching)
            .map(|onlooker| (onlooker.at.distance(at), onlooker.id, onlooker.at))
            .filter(|(distance, ..)| *distance <= self.frame * 4.0)
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, id, point)| (id, point))
    }

    /// Somebody nearby looks over at `at`, if there is anybody.
    fn noticed(&mut self, at: Point, besides: CreatureId, length: f32) {
        if let Some((creature, _)) = self.nearest_resting(at, besides) {
            self.asides.push(Aside {
                creature,
                beat: BeatKind::Notice,
                length,
                look: at,
            });
        }
    }

    /// Whether this visit turns something up, and in what circumstances.
    fn maybe_find(&mut self, garden: bool, roof: bool) -> Option<DiscoveryCircumstances> {
        if !self.rng.random_ratio(1, FIND_IN) {
            return None;
        }
        Some(DiscoveryCircumstances {
            at_home: true,
            in_garden: garden,
            on_roof: roof,
            with_visitor: self.with_visitor,
            ..discovery::calendar_circumstances(self.now, self.created)
        })
    }
}

/// The steps of a find at home: holding it up for a moment, and then — once the presentation is
/// complete — it goes in the scrapbook like any other find.
const FIND_SECS: f32 = 2.6;

/// Moves one resident's plan on by `dt`. Returns `false` once the plan is over and the resident
/// should go back to strolling.
pub(super) fn advance(
    activity: &mut VillageActivity,
    creature: &mut Creature,
    dt: f32,
    context: &mut VillageContext<'_>,
) -> bool {
    activity.step_elapsed += dt;
    creature.state.velocity = Point::default();
    // A find, once it has been chosen, is held up for a moment and goes in the scrapbook. It is
    // the last thing a plan does, but for coming down off a roof.
    if let Some(circumstances) = activity.find {
        if creature.state.action != ActionKind::PresentDiscovery {
            creature.state.activity_variant =
                discovery::choose_trinket_variant(context.rng, circumstances, context.found);
            creature.state.beat = None;
            show(creature, ActionKind::PresentDiscovery, context.events);
            activity.step_elapsed = 0.0;
            context.noticed(creature.state.position, creature.id, 1.6);
            return true;
        }
        if activity.step_elapsed >= FIND_SECS {
            World::emit(
                context.events,
                WorldEvent::ActionCompleted {
                    creature_id: creature.id,
                    action: ActionKind::PresentDiscovery,
                },
            );
            activity.find = None;
            if let Plan::Roof { .. } = activity.plan {
                show(creature, ActionKind::Landing, context.events);
                activity.step = 3;
                activity.step_elapsed = 0.0;
                return true;
            }
            return false;
        }
        return true;
    }
    let speed = errand_speed(creature);
    match activity.plan {
        Plan::Garden {
            patch,
            stand,
            face_right,
            task,
        } => match (activity.step, task) {
            // To the patch.
            (0, _) => {
                show(creature, ActionKind::Traverse, context.events);
                if walk(creature, stand, speed, dt) || activity.step_elapsed > WALK_LIMIT_SECS {
                    creature.state.facing_right = face_right;
                    show(creature, ActionKind::Idle, context.events);
                    let (kind, length, held) = match task {
                        GardenTask::Water => (
                            BeatKind::Watering,
                            context.rng.random_range(4.0..6.5),
                            Some(VillageProp::WateringCan),
                        ),
                        GardenTask::Inspect => (
                            BeatKind::InspectSprout,
                            context.rng.random_range(3.5..6.0),
                            None,
                        ),
                        GardenTask::Pick | GardenTask::Show { .. } => {
                            (BeatKind::Picking, 1.4, Some(VillageProp::Produce(patch)))
                        }
                    };
                    beat(creature, kind, length, held);
                    activity.next();
                }
                true
            }
            // Watering or looking closely, and perhaps turning something up among the plants.
            (1, GardenTask::Water | GardenTask::Inspect) => {
                if beat_done(creature) {
                    creature.state.beat = None;
                    activity.find = context.maybe_find(true, false);
                    return activity.find.is_some();
                }
                true
            }
            // Picked; now eaten.
            (1, GardenTask::Pick) => {
                if beat_done(creature) {
                    creature.state.beat = None;
                    show(creature, ActionKind::Eat, context.events);
                    activity.next();
                }
                true
            }
            (2, GardenTask::Pick) => activity.step_elapsed < 6.0,
            // Picked; now carried over to the friend, who looks up to see it coming.
            (1, GardenTask::Show { friend }) => {
                if beat_done(creature) {
                    beat(
                        creature,
                        BeatKind::Carrying,
                        WALK_LIMIT_SECS,
                        Some(VillageProp::Produce(patch)),
                    );
                    if let Some(onlooker) = context
                        .onlookers
                        .iter()
                        .find(|onlooker| onlooker.id == friend)
                    {
                        let walk = (onlooker.at.x - creature.state.position.x).abs() / speed;
                        context.asides.push(Aside {
                            creature: friend,
                            beat: BeatKind::Notice,
                            length: (walk + 1.0).min(WALK_LIMIT_SECS),
                            look: creature.state.position,
                        });
                    }
                    activity.next();
                }
                true
            }
            (2, GardenTask::Show { friend }) => {
                let Some(friend_at) = context
                    .onlookers
                    .iter()
                    .find(|onlooker| onlooker.id == friend)
                    .map(|onlooker| onlooker.at)
                else {
                    // The friend has gone off somewhere: eaten where it stands instead.
                    creature.state.beat = None;
                    show(creature, ActionKind::Eat, context.events);
                    activity.plan = Plan::Garden {
                        patch,
                        stand,
                        face_right,
                        task: GardenTask::Pick,
                    };
                    activity.step = 2;
                    activity.step_elapsed = 0.0;
                    return true;
                };
                let side = if friend_at.x >= creature.state.position.x {
                    -1.0
                } else {
                    1.0
                };
                let goal = Point {
                    x: friend_at.x + side * context.frame * 0.85,
                    y: friend_at.y,
                };
                show(creature, ActionKind::Traverse, context.events);
                if walk(creature, goal, speed, dt) || activity.step_elapsed > WALK_LIMIT_SECS {
                    creature.state.facing_right = friend_at.x > creature.state.position.x;
                    show(creature, ActionKind::Idle, context.events);
                    beat(
                        creature,
                        BeatKind::ShowingOff,
                        3.2,
                        Some(VillageProp::Produce(patch)),
                    );
                    context.asides.push(Aside {
                        creature: friend,
                        beat: BeatKind::Admiring,
                        length: 3.0,
                        look: creature.state.position,
                    });
                    activity.next();
                }
                true
            }
            (3, GardenTask::Show { .. }) => !beat_done(creature),
            _ => false,
        },
        Plan::Chore {
            stand,
            face_right,
            chore,
            ..
        } => match activity.step {
            0 => {
                show(creature, ActionKind::Traverse, context.events);
                if walk(creature, stand, speed, dt) || activity.step_elapsed > WALK_LIMIT_SECS {
                    creature.state.facing_right = face_right;
                    show(creature, ActionKind::Idle, context.events);
                    let length = context.rng.random_range(3.0..4.5);
                    beat(creature, chore, length, None);
                    activity.next();
                }
                true
            }
            1 => {
                if beat_done(creature) {
                    creature.state.beat = None;
                    activity.find = context.maybe_find(false, false);
                    return activity.find.is_some();
                }
                true
            }
            _ => false,
        },
        Plan::Indoors {
            door, stay, nap, ..
        } => match activity.step {
            0 => {
                show(creature, ActionKind::Traverse, context.events);
                if walk(creature, door, speed, dt) || activity.step_elapsed > WALK_LIMIT_SECS {
                    creature.state.position = door;
                    creature.state.indoors = true;
                    show(
                        creature,
                        if nap {
                            ActionKind::Sleep
                        } else {
                            ActionKind::Idle
                        },
                        context.events,
                    );
                    activity.next();
                }
                true
            }
            1 => {
                if activity.step_elapsed >= stay {
                    creature.state.indoors = false;
                    show(creature, ActionKind::Homebound, context.events);
                    activity.next();
                }
                true
            }
            // A moment on the doorstep, and off again.
            _ => activity.step_elapsed < 1.2,
        },
        Plan::Roof {
            beside, top, stay, ..
        } => match activity.step {
            0 => {
                show(creature, ActionKind::Traverse, context.events);
                if walk(creature, beside, speed, dt) || activity.step_elapsed > WALK_LIMIT_SECS {
                    show(creature, ActionKind::Landing, context.events);
                    activity.next();
                }
                true
            }
            // Up in a little hop.
            1 => {
                let t = (activity.step_elapsed / 0.55).clamp(0.0, 1.0);
                creature.state.position = hop_point(beside, top, t);
                if t >= 1.0 {
                    creature.state.position = top;
                    show(creature, ActionKind::Perch, context.events);
                    beat(creature, BeatKind::RoofSit, stay, None);
                    activity.next();
                }
                true
            }
            // Sitting up there, looking out.
            2 => {
                creature.state.position = top;
                if beat_done(creature) {
                    creature.state.beat = None;
                    if let Some(find) = context.maybe_find(false, true) {
                        activity.find = Some(find);
                        return true;
                    }
                    show(creature, ActionKind::Landing, context.events);
                    activity.next();
                }
                true
            }
            // And down again.
            3 => {
                let t = (activity.step_elapsed / 0.45).clamp(0.0, 1.0);
                creature.state.position = hop_point(top, beside, t);
                if t >= 1.0 {
                    creature.state.position = beside;
                    show(creature, ActionKind::Homebound, context.events);
                    return false;
                }
                true
            }
            _ => false,
        },
        Plan::LeafOnFace => match activity.step {
            // The leaf drifting down.
            0 => {
                show(creature, ActionKind::Homebound, context.events);
                if activity.step_elapsed >= LEAF_FALL_SECS {
                    beat(creature, BeatKind::LeafOnFace, LEAF_BEAT_SECS, None);
                    context.noticed(creature.state.position, creature.id, 1.6);
                    activity.next();
                }
                true
            }
            1 => {
                if beat_done(creature) {
                    creature.state.beat = None;
                    activity.next();
                }
                true
            }
            // The leaf on its way to the ground, and carrying on.
            _ => activity.step_elapsed < LEAF_DROP_SECS,
        },
        Plan::DroppedSnack {
            before,
            from,
            to,
            eat,
        } => match activity.step {
            // Eating, until it gets away.
            0 => {
                show(creature, ActionKind::Eat, context.events);
                if activity.step_elapsed >= before {
                    show(creature, ActionKind::Idle, context.events);
                    beat(creature, BeatKind::DroppedSnack, 0.9, None);
                    context.noticed(from, creature.id, 1.4);
                    activity.next();
                }
                true
            }
            // A start, as it rolls away.
            1 => {
                if beat_done(creature) {
                    creature.state.beat = None;
                    activity.next();
                }
                true
            }
            // After it.
            2 => {
                show(creature, ActionKind::Traverse, context.events);
                let side = if to.x >= from.x { -1.0 } else { 1.0 };
                let goal = Point {
                    x: to.x + side * context.frame * 0.3,
                    y: from.y,
                };
                if walk(creature, goal, speed, dt) || activity.step_elapsed > WALK_LIMIT_SECS {
                    creature.state.facing_right = to.x > creature.state.position.x;
                    show(creature, ActionKind::Idle, context.events);
                    beat(creature, BeatKind::Retrieve, 0.9, None);
                    activity.next();
                }
                true
            }
            // Picked up.
            3 => {
                if beat_done(creature) {
                    creature.state.beat = None;
                    show(creature, ActionKind::Eat, context.events);
                    activity.next();
                }
                true
            }
            // And carrying on with it.
            _ => activity.step_elapsed < eat,
        },
        Plan::MissedCushion {
            beside,
            cushion,
            nap,
        } => match activity.step {
            // Over to the cushion, or nearly.
            0 => {
                show(creature, ActionKind::Traverse, context.events);
                if walk(creature, beside, speed, dt) || activity.step_elapsed > WALK_LIMIT_SECS {
                    creature.state.facing_right = cushion.x > creature.state.position.x;
                    show(creature, ActionKind::Idle, context.events);
                    beat(creature, BeatKind::MissedCushion, 2.4, None);
                    activity.next();
                }
                true
            }
            // Sat down just off it, and a start at finding the ground.
            1 => {
                if creature
                    .state
                    .beat
                    .is_some_and(|beat| beat.progress() >= 0.3 && beat.progress() < 0.35)
                {
                    context.noticed(creature.state.position, creature.id, 1.4);
                }
                if beat_done(creature) {
                    creature.state.beat = None;
                    activity.next();
                }
                true
            }
            // A shuffle across.
            2 => {
                show(creature, ActionKind::Traverse, context.events);
                if walk(creature, cushion, speed * 0.6, dt) || activity.step_elapsed > 6.0 {
                    show(creature, ActionKind::Sleep, context.events);
                    activity.next();
                }
                true
            }
            _ => activity.step_elapsed < nap,
        },
    }
}

/// How long a leaf takes to drift down onto a face, how long the start and the shake take, and
/// how long it takes to reach the ground once shaken off.
const LEAF_FALL_SECS: f32 = 1.1;
const LEAF_BEAT_SECS: f32 = 3.0;
const LEAF_DROP_SECS: f32 = 0.8;

/// A point along a little hop from `from` to `to`: straight across, and up in an arc.
fn hop_point(from: Point, to: Point, t: f32) -> Point {
    let lift = (to.y - from.y).abs().max(6.0) * 0.5 + 6.0;
    Point {
        x: lerp(from.x, to.x, t),
        y: lerp(from.y, to.y, smoothstep(t)) - lift * 4.0 * t * (1.0 - t),
    }
}

/// Where the loose leaf or apple of a mishap is right now, if it is showing: the leaf drifting
/// down to the face, sitting on it while it is shaken, and falling off to the ground; the apple
/// rolling from the paws to where it stops, and gone once it is picked up.
pub(super) fn loose_prop(
    activity: &VillageActivity,
    creature: &Creature,
    face_height: f32,
) -> Option<(VillageProp, Point)> {
    let face = Point {
        x: creature.state.position.x,
        y: creature.state.position.y - face_height,
    };
    match activity.plan {
        Plan::LeafOnFace => match activity.step {
            0 => {
                let t = (activity.step_elapsed / LEAF_FALL_SECS).clamp(0.0, 1.0);
                let sway = (t * std::f32::consts::TAU * 1.5).sin() * face_height * 0.3 * (1.0 - t);
                Some((
                    VillageProp::Leaf,
                    Point {
                        x: face.x + sway,
                        y: face.y - face_height * 1.4 * (1.0 - t),
                    },
                ))
            }
            1 => {
                let progress = creature.state.beat.map_or(1.0, |beat| beat.progress());
                (progress < 0.8).then_some((VillageProp::Leaf, face))
            }
            _ => {
                let t = (activity.step_elapsed / LEAF_DROP_SECS).clamp(0.0, 1.0);
                let drift = if creature.state.facing_right {
                    -1.0
                } else {
                    1.0
                };
                Some((
                    VillageProp::Leaf,
                    Point {
                        x: face.x + drift * face_height * 0.5 * t,
                        y: lerp(face.y, creature.state.position.y, t),
                    },
                ))
            }
        },
        Plan::DroppedSnack { from, to, .. } => match activity.step {
            1 => {
                let t = creature
                    .state
                    .beat
                    .map_or(1.0, |beat| beat.progress())
                    .clamp(0.0, 1.0);
                let bounce = (t * std::f32::consts::PI * 2.0).sin().abs() * (1.0 - t) * 4.0;
                Some((
                    VillageProp::Apple,
                    Point {
                        x: lerp(from.x, to.x, 1.0 - (1.0 - t) * (1.0 - t)),
                        y: from.y - bounce,
                    },
                ))
            }
            2 => Some((VillageProp::Apple, Point { x: to.x, y: from.y })),
            3 => {
                let progress = creature.state.beat.map_or(1.0, |beat| beat.progress());
                (progress < 0.6).then_some((VillageProp::Apple, Point { x: to.x, y: from.y }))
            }
            _ => None,
        },
        _ => None,
    }
}

/// A resident's own house, as a plan needs it: which it is, where it stands and how wide, its
/// type, and where somebody sitting on its roof has its feet.
#[derive(Clone, Copy, Debug)]
pub(super) struct OwnHouse {
    pub(super) slot: usize,
    pub(super) at: Point,
    pub(super) half: f32,
    pub(super) style: ShelterStyle,
    pub(super) roof: Point,
}

/// A garden patch on the ground, and how far along it is.
#[derive(Clone, Copy, Debug)]
pub(super) struct PlantedPatch {
    pub(super) kind: GardenKind,
    pub(super) at: Point,
    pub(super) stage: GardenStage,
}

/// What a resident with time on its hands could do about the village just now.
pub(super) struct VillageOptions<'a> {
    pub(super) commons: HomeCommons,
    pub(super) frame: f32,
    pub(super) house: Option<OwnHouse>,
    pub(super) gardens: &'a [PlantedPatch],
    /// Cushions nobody is on.
    pub(super) cushions: &'a [Point],
    pub(super) onlookers: &'a [Onlooker],
    /// Whether one more can go indoors without leaving the village looking empty.
    pub(super) may_go_in: bool,
    /// Whether nobody is up on its roof already.
    pub(super) roof_free: bool,
    /// Whether one of the quiet moments at the door may start: only one of those at a time.
    pub(super) moment_free: bool,
    /// Whether a plan out and about may start: only one of those at a time, too.
    pub(super) plan_free: bool,
}

/// What a resident does with its turn.
#[derive(Clone, Copy, Debug)]
pub(super) enum Choice {
    /// One of the quiet moments at its door.
    Moment,
    Plan(Plan),
    /// Nothing just now: everything it could do is somebody else's turn.
    Wait,
}

/// How the choices weigh against one another: an ordinary quiet moment is still the commonest
/// thing, the garden and the house come next, and a mishap is rare enough to be worth noticing.
const MOMENT_WEIGHT: u32 = 10;
const GARDEN_WEIGHT: u32 = 6;
const CHORE_WEIGHT: u32 = 4;
const INDOORS_WEIGHT: u32 = 3;
const ROOF_WEIGHT: u32 = 2;
const LEAF_WEIGHT: u32 = 1;
const SNACK_WEIGHT: u32 = 1;
const CUSHION_WEIGHT: u32 = 1;

/// How long a resident stays indoors: a nap, or a while pottering about in there.
const INDOORS_NAP_SECS: std::ops::Range<f32> = 40.0..110.0;
const INDOORS_POTTER_SECS: std::ops::Range<f32> = 12.0..30.0;
/// How long a resident sits up on its roof.
const ROOF_SECS: std::ops::Range<f32> = 18.0..45.0;

/// One weighted pick from `weights`, or `None` if they are all zero.
fn weighted(rng: &mut ChaCha12Rng, weights: &[u32]) -> Option<usize> {
    let total: u32 = weights.iter().sum();
    if total == 0 {
        return None;
    }
    let mut roll = rng.random_range(0..total);
    weights.iter().position(|weight| {
        let hit = roll < *weight;
        roll = roll.saturating_sub(*weight);
        hit
    })
}

/// The task a garden visit is for, chosen from what this patch can offer just now: water for
/// anything, a close look at something still coming up, something to eat from a patch that is
/// ready, and something to show off from one at its fullest if there is a friend to show.
fn choose_garden(
    rng: &mut ChaCha12Rng,
    creature: &Creature,
    options: &VillageOptions<'_>,
) -> Option<Plan> {
    let hungry = creature.state.drives.energy < 0.5;
    let mut picks: Vec<(u32, PlantedPatch, GardenTask)> = Vec::new();
    for patch in options.gardens.iter().copied() {
        picks.push((
            2 + u32::from(!patch.kind.edible()),
            patch,
            GardenTask::Water,
        ));
        if patch.stage <= GardenStage::Growing {
            picks.push((3, patch, GardenTask::Inspect));
        }
        if patch.kind.edible() && patch.stage >= GardenStage::Grown {
            picks.push((2 + u32::from(hungry), patch, GardenTask::Pick));
        }
        if patch.kind.harvest()
            && patch.stage == GardenStage::Bounty
            && let Some(friend) = options
                .onlookers
                .iter()
                .filter(|onlooker| onlooker.id != creature.id && !onlooker.watching)
                .map(|onlooker| (onlooker.at.distance(patch.at), onlooker.id))
                .filter(|(distance, _)| *distance <= options.frame * 8.0)
                .min_by(|a, b| a.0.total_cmp(&b.0))
                .map(|(_, id)| id)
        {
            picks.push((3, patch, GardenTask::Show { friend }));
        }
    }
    let weights: Vec<u32> = picks.iter().map(|(weight, ..)| *weight).collect();
    let (_, patch, task) = picks[weighted(rng, &weights)?];
    let from_right = creature.state.position.x > patch.at.x;
    let reach = if task == GardenTask::Inspect {
        0.4
    } else {
        0.45
    };
    let stand = Point {
        x: (patch.at.x + if from_right { reach } else { -reach } * options.frame)
            .clamp(options.commons.low_x, options.commons.high_x),
        y: options.commons.ground_y,
    };
    Some(Plan::Garden {
        patch: patch.kind,
        stand,
        face_right: !from_right,
        task,
    })
}

/// The chore a house of this type asks for.
pub(super) const fn chore_for(style: ShelterStyle) -> BeatKind {
    match style {
        ShelterStyle::Tent => BeatKind::AdjustFlap,
        ShelterStyle::PillowFort => BeatKind::FluffCushion,
        ShelterStyle::Mushroom => BeatKind::InspectCap,
        ShelterStyle::LeafHouse => BeatKind::TidyLeaves,
    }
}

/// What a resident with time on its hands does next: one of the quiet moments at its door, or a
/// plan about the village — the garden, its house, indoors, its roof, or a small mishap.
pub(super) fn choose(
    rng: &mut ChaCha12Rng,
    creature: &Creature,
    options: &VillageOptions<'_>,
) -> Choice {
    // Everything but going indoors is somebody else's turn: wait for one of those to end rather
    // than going in because there is nothing else to do.
    if !options.moment_free && !options.plan_free {
        return Choice::Wait;
    }
    let sleepy = creature.state.drives.sleep_pressure > 0.5;
    let hungry = creature.state.drives.energy < 0.5;
    let garden = if options.plan_free {
        choose_garden(rng, creature, options)
    } else {
        None
    };
    let plan_weight = |weight: u32| if options.plan_free { weight } else { 0 };
    let house = options.house;
    let weights = [
        if options.moment_free {
            MOMENT_WEIGHT
        } else {
            0
        },
        if garden.is_some() { GARDEN_WEIGHT } else { 0 },
        plan_weight(if house.is_some() { CHORE_WEIGHT } else { 0 }),
        if house.is_some() && options.may_go_in {
            INDOORS_WEIGHT + 2 * u32::from(sleepy)
        } else {
            0
        },
        plan_weight(if house.is_some() && options.roof_free {
            ROOF_WEIGHT
        } else {
            0
        }),
        plan_weight(LEAF_WEIGHT),
        plan_weight(SNACK_WEIGHT + u32::from(hungry)),
        plan_weight(if options.cushions.is_empty() {
            0
        } else {
            CUSHION_WEIGHT + u32::from(sleepy)
        }),
    ];
    let Some(pick) = weighted(rng, &weights) else {
        return Choice::Wait;
    };
    let at = creature.state.position;
    let ground = options.commons.ground_y;
    let (low, high) = options.commons.standing_span();
    // The side of `x` this resident is coming from.
    let side_of = |x: f32| if at.x > x { 1.0 } else { -1.0 };
    let plan = match (pick, house) {
        (0, _) => return Choice::Moment,
        (1, _) => match garden {
            Some(plan) => plan,
            None => return Choice::Wait,
        },
        (2, Some(house)) => {
            let side = side_of(house.at.x);
            Plan::Chore {
                slot: house.slot,
                stand: Point {
                    x: house.at.x + side * house.half * 0.55,
                    y: ground,
                },
                face_right: side < 0.0,
                chore: chore_for(house.style),
            }
        }
        (3, Some(house)) => {
            let nap = sleepy || rng.random_ratio(1, 3);
            Plan::Indoors {
                slot: house.slot,
                door: Point {
                    x: house.at.x,
                    y: ground,
                },
                stay: rng.random_range(if nap {
                    INDOORS_NAP_SECS
                } else {
                    INDOORS_POTTER_SECS
                }),
                nap,
            }
        }
        (4, Some(house)) => Plan::Roof {
            slot: house.slot,
            beside: Point {
                x: house.at.x + side_of(house.at.x) * house.half * 0.8,
                y: ground,
            },
            top: house.roof,
            stay: rng.random_range(ROOF_SECS),
        },
        (5, _) => Plan::LeafOnFace,
        (6, _) => {
            let ahead = if creature.state.facing_right {
                1.0
            } else {
                -1.0
            };
            let from = Point {
                x: at.x + ahead * options.frame * 0.2,
                y: ground,
            };
            let mut to = from.x + ahead * rng.random_range(0.9..1.6) * options.frame;
            if !(low..=high).contains(&to) {
                // No room ahead: it rolls the other way instead.
                to = from.x - ahead * rng.random_range(0.9..1.6) * options.frame;
            }
            Plan::DroppedSnack {
                before: rng.random_range(2.0..4.0),
                from,
                to: Point {
                    x: to.clamp(low, high),
                    y: ground,
                },
                eat: rng.random_range(4.0..8.0),
            }
        }
        (7, _) => {
            let Some(cushion) = options
                .cushions
                .iter()
                .copied()
                .min_by(|a, b| a.distance(at).total_cmp(&b.distance(at)))
            else {
                return Choice::Wait;
            };
            Plan::MissedCushion {
                beside: Point {
                    x: cushion.x + side_of(cushion.x) * options.frame * 0.3,
                    y: ground,
                },
                cushion: Point {
                    x: cushion.x,
                    y: ground,
                },
                nap: rng.random_range(40.0..100.0),
            }
        }
        _ => return Choice::Wait,
    };
    Choice::Plan(plan)
}

impl Plan {
    /// Roughly how long this plan takes, for spacing out the resident's next turn.
    pub(super) fn rough_length(&self) -> f32 {
        match *self {
            Self::Garden { .. } => 12.0,
            Self::Chore { .. } => 8.0,
            Self::Indoors { stay, .. } | Self::Roof { stay, .. } => stay + 4.0,
            Self::LeafOnFace => 5.0,
            Self::DroppedSnack { before, eat, .. } => before + eat + 5.0,
            Self::MissedCushion { nap, .. } => nap + 5.0,
        }
    }

    /// Whether this is a spell indoors, which counts against how many may be in at once rather
    /// than against how many may be out and about.
    pub(super) fn indoors(&self) -> bool {
        matches!(self, Self::Indoors { .. })
    }
}

/// Puts a resident whose plan has ended early back on its own feet outside: out of doors, down
/// off a roof, with nothing in its hands, and resting.
pub(super) fn settle(activity: &VillageActivity, creature: &mut Creature) {
    creature.state.indoors = false;
    creature.state.beat = None;
    creature.state.velocity = Point::default();
    match activity.plan {
        Plan::Roof { beside, .. } if activity.step >= 1 => creature.state.position = beside,
        Plan::Indoors { door, .. } if activity.step >= 1 => creature.state.position = door,
        _ => {}
    }
    if creature.state.action != ActionKind::PetReaction {
        creature.state.action = ActionKind::Homebound;
        creature.state.action_elapsed = 0.0;
        creature.state.action_duration = super::home::HOME_DURATION.whole_seconds() as f32;
    }
}

/// The most residents indoors at once for a village of `residents`: nobody while there is only
/// one, so a lone companion is never out of sight, and at most two however big it grows, so the
/// village never looks empty.
pub(super) const fn max_indoors(residents: usize) -> usize {
    match residents {
        0 | 1 => 0,
        2 | 3 => 1,
        _ => 2,
    }
}

impl World {
    /// Ends every resident's plan about the village and puts each back on its own feet outside:
    /// down off a roof, out of doors, and with nothing in its hands. A visit that is over, a
    /// paused, hidden or still colony, and one being called away all use this.
    pub(super) fn end_village_life(&mut self) {
        if self.village_life.is_empty() {
            return;
        }
        let ended: Vec<CreatureId> = self.village_life.keys().copied().collect();
        for creature_id in ended {
            self.end_village_activity(creature_id);
        }
    }

    /// Ends one resident's plan, if it has one, and puts it back on the ground outside.
    pub(super) fn end_village_activity(&mut self, creature_id: CreatureId) {
        let Some(activity) = self.village_life.remove(&creature_id) else {
            return;
        };
        if let Some(creature) = creature_mut(&mut self.save.creatures, creature_id) {
            settle(&activity, creature);
        }
    }

    /// Lets go of one resident's plan, if it has one, leaving it exactly where it is: somebody
    /// picked up off a roof is picked up off the roof. Returns how far it had got.
    pub(super) fn drop_village_activity(
        &mut self,
        creature_id: CreatureId,
    ) -> Option<VillageActivity> {
        let activity = self.village_life.remove(&creature_id)?;
        if let Some(creature) = creature_mut(&mut self.save.creatures, creature_id) {
            creature.state.indoors = false;
            creature.state.beat = None;
            creature.state.velocity = Point::default();
        }
        Some(activity)
    }

    /// Hands out what this tick's plans asked of the others: a look over at a mishap or a find,
    /// a look up at a friend coming over with something, and a pleased look at what it is. Only
    /// somebody standing about or strolling at home with nothing on takes one — a stroll stops
    /// for it — and a look already under way gives way to the next.
    pub(super) fn apply_asides(&mut self, asides: Vec<Aside>) {
        for aside in asides {
            if self.village_life.contains_key(&aside.creature)
                || self.home_moments.contains_key(&aside.creature)
            {
                continue;
            }
            let Some(creature) = creature_mut(&mut self.save.creatures, aside.creature) else {
                continue;
            };
            let replaceable = creature
                .state
                .beat
                .is_none_or(|beat| beat.kind == BeatKind::Notice);
            if !replaceable
                || creature.state.indoors
                || !matches!(
                    creature.state.action,
                    ActionKind::Homebound | ActionKind::Traverse
                )
                || creature.state.arrival_delay_secs > 0.0
            {
                continue;
            }
            if (aside.look.x - creature.state.position.x).abs() > 1.0 {
                creature.state.facing_right = aside.look.x > creature.state.position.x;
            }
            creature.state.beat = Some(Beat {
                look: Some(aside.look),
                ..Beat::new(aside.beat, aside.length)
            });
        }
    }

    /// The houses somebody is inside just now, and whether they are napping in there.
    pub fn house_occupancy(&self) -> Vec<HouseOccupancy> {
        let mut occupied: Vec<HouseOccupancy> = Vec::new();
        for (slot, napping) in self
            .village_life
            .values()
            .filter_map(VillageActivity::inside)
        {
            match occupied.iter_mut().find(|house| house.slot == slot) {
                Some(house) => house.napping |= napping,
                None => occupied.push(HouseOccupancy { slot, napping }),
            }
        }
        occupied.sort_by_key(|house| house.slot);
        occupied
    }

    /// The houses being seen to by their keepers just now, and how far through the chore each is.
    pub fn house_motions(&self) -> Vec<HouseMotion> {
        self.village_life
            .iter()
            .filter_map(|(creature_id, activity)| {
                let Plan::Chore { slot, chore, .. } = activity.plan else {
                    return None;
                };
                let beat = self
                    .save
                    .creatures
                    .iter()
                    .find(|creature| creature.id == *creature_id)?
                    .state
                    .beat
                    .filter(|beat| beat.kind == chore)?;
                Some(HouseMotion {
                    slot,
                    chore,
                    progress: beat.progress(),
                })
            })
            .collect()
    }

    /// The leaves and apples loose about the village just now, and where each is.
    pub fn loose_props(&self, monitors: &[MonitorInfo]) -> Vec<(VillageProp, Point)> {
        if self.village_life.is_empty() {
            return Vec::new();
        }
        let scale_factor = monitors
            .iter()
            .find(|monitor| Some(monitor.display_key) == self.save.home.display)
            .map_or(1.0, |monitor| monitor.scale_factor);
        let face = spacing::frame_width(self.save.settings.display_scale, scale_factor) * 0.5;
        self.village_life
            .iter()
            .filter_map(|(creature_id, activity)| {
                let creature = self
                    .save
                    .creatures
                    .iter()
                    .find(|creature| creature.id == *creature_id)?;
                loose_prop(activity, creature, face)
            })
            .collect()
    }
}
