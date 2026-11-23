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
use super::village_company::{self as company, Answer, Call, Kind, LookOn, Purpose, Recent};
use super::*;
use crate::tuning::{FINDS, VILLAGE_LIFE};

/// How fast a resident walks about the village on an errand of its own.
pub(super) fn errand_speed(creature: &Creature) -> f32 {
    22.0 + creature.personality.activity * 18.0
}

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
    /// A chore at its own door, in the way its house asks for: up its path to the front of the
    /// house however far back it stands, and along to one side of the door.
    Chore {
        slot: usize,
        stand: Point,
        door_x: f32,
        depth: f32,
        face_right: bool,
        chore: BeatKind,
    },
    /// Indoors for a while, behind the drawn curtain: up its path to the door, and in.
    Indoors {
        slot: usize,
        door: Point,
        depth: f32,
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
    /// Its snack rolled away; after it, pick it up, and back with it to carry on eating.
    DroppedSnack {
        before: f32,
        from: Point,
        to: Point,
        /// Where it was eating, and goes back to.
        back: Point,
        eat: f32,
    },
    /// Sat down beside the cushion, and shuffled across onto it.
    MissedCushion {
        beside: Point,
        cushion: Point,
        nap: f32,
    },
    /// Going over to another resident: to say hello and stay a while, to play, or for a walk
    /// along the village together (see [`company`]).
    Call(Call),
    /// Stopped for somebody coming over, and whatever comes of it.
    Answer(Answer),
    /// Stopped to look on while somebody works.
    LookOn(LookOn),
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
    pub(super) step: u8,
    pub(super) step_elapsed: f32,
    /// Whether the step it is on has only just begun.
    fresh: bool,
    /// Something turned up at the end, in circumstances of its own.
    find: Option<DiscoveryCircumstances>,
    /// The ground the plan was made on, noted the first time it is carried on.
    ground: Option<PlanGround>,
}

impl VillageActivity {
    /// Whether this plan has turned something up that is not yet written down.
    pub(super) const fn finding(&self) -> bool {
        self.find.is_some()
    }

    pub(super) fn new(plan: Plan) -> Self {
        Self {
            plan,
            step: 0,
            step_elapsed: 0.0,
            fresh: true,
            find: None,
            ground: None,
        }
    }

    /// Where the resident is in a hop up onto its roof or back down, if it is hopping.
    pub(super) fn hop_moment(&self) -> Option<HopMoment> {
        let length = match (self.plan, self.step) {
            (Plan::Roof { .. }, 1) => ROOF_HOP_UP_SECS,
            (Plan::Roof { .. }, 3) => ROOF_HOP_DOWN_SECS,
            _ => return None,
        };
        Some(hop_moment(
            (self.step_elapsed - HOP_GATHER_SECS) / length,
            self.step_elapsed,
        ))
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

    pub(super) fn next(&mut self) {
        self.go_to(self.step + 1);
    }

    pub(super) fn go_to(&mut self, step: u8) {
        self.step = step;
        self.step_elapsed = 0.0;
        self.fresh = true;
    }

    /// Whether this is the first time the step it is on has been asked: asked once a step.
    pub(super) fn entering(&mut self) -> bool {
        std::mem::take(&mut self.fresh)
    }

    /// Where it is at work, or on its way to be, somewhere it can be watched; how far back that
    /// is; and whether it is a garden.
    pub(super) fn working(&self) -> Option<(Point, f32, bool)> {
        if self.step > 1 || self.find.is_some() {
            return None;
        }
        match self.plan {
            Plan::Garden {
                stand,
                task: GardenTask::Water | GardenTask::Inspect,
                ..
            } => Some((stand, 0.0, true)),
            Plan::Chore { stand, depth, .. } => Some((stand, depth, false)),
            _ => None,
        }
    }

    /// Whether it is standing with or beside somebody, where it will stay a moment.
    pub(super) fn keeps_still(&self) -> bool {
        company::keeps_still(&self.plan, self.step)
    }

    /// Whether a call it made came to nothing.
    pub(super) fn turned_down(&self) -> bool {
        matches!(self.plan, Plan::Call(_)) && company::turned_down(self.step)
    }

    /// Its part in company kept with another, as the other reads it.
    pub(super) fn part(&self) -> Option<company::PartView> {
        match self.plan {
            Plan::Call(call) => Some(company::PartView {
                with: call.on,
                step: self.step,
                reply: None,
                met: self.step >= 2,
            }),
            Plan::Answer(answer) => Some(company::PartView {
                with: answer.to,
                step: self.step,
                reply: Some(answer.reply),
                met: self.step >= 1,
            }),
            _ => None,
        }
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
pub(super) fn walk(creature: &mut Creature, goal: Point, speed: f32, dt: f32) -> bool {
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
pub(super) fn show(creature: &mut Creature, action: ActionKind, events: &mut Vec<WorldEvent>) {
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
pub(super) fn beat(
    creature: &mut Creature,
    kind: BeatKind,
    length: f32,
    held: Option<VillageProp>,
) {
    creature.state.beat = Some(Beat {
        held,
        ..Beat::new(kind, length)
    });
}

/// Whether the beat started for this step is over.
pub(super) fn beat_done(creature: &Creature) -> bool {
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
    /// Home in the village, rather than still on its way back to it: somebody to go over to.
    pub(super) home: bool,
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
    /// Whether the colony has a find left today.
    pub(super) find_allowed: bool,
    /// The way down to the front from somewhere back in the village.
    pub(super) paths: &'a village_paths::VillagePaths,
    /// Everybody as the tick began, for a plan that is about somebody else.
    pub(super) everyone: &'a [company::Resident],
    /// Who was asked for their company this tick.
    pub(super) invites: &'a mut Vec<company::Invite>,
    /// The village's ground line, the stretch of it there is to stand on, and half the ground
    /// one resident stands on.
    pub(super) ground_y: f32,
    pub(super) span: (f32, f32),
    pub(super) half: f32,
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
        if !self.rng.random_ratio(1, FINDS.at_home_in) || !self.find_allowed {
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
        if activity.step_elapsed >= VILLAGE_LIFE.find_secs {
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
    // The garden, the cushions and the hop up onto a roof are all at the front of the grass: a
    // resident somewhere back in the village goes down a path to the front first, and the plan
    // starts from there.
    if activity.step == 0
        && creature.state.depth > 0.0
        && let Some(goal) = activity.plan.at_the_front()
    {
        show(creature, ActionKind::Traverse, context.events);
        context.paths.walk_to(creature, goal, 0.0, speed, dt);
        activity.step_elapsed = 0.0;
        return true;
    }
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
                if walk(creature, stand, speed, dt)
                    || activity.step_elapsed > VILLAGE_LIFE.walk_limit_secs
                {
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
                        VILLAGE_LIFE.walk_limit_secs,
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
                            length: (walk + 1.0).min(VILLAGE_LIFE.walk_limit_secs),
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
                if walk(creature, goal, speed, dt)
                    || activity.step_elapsed > VILLAGE_LIFE.walk_limit_secs
                {
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
            door_x,
            depth,
            face_right,
            chore,
            ..
        } => match activity.step {
            0 => {
                show(creature, ActionKind::Traverse, context.events);
                if village_paths::by_own_door(
                    creature,
                    context.paths,
                    door_x,
                    stand,
                    depth,
                    speed,
                    context.frame / CREATURE_ART_WIDTH,
                    dt,
                ) || activity.step_elapsed > VILLAGE_LIFE.walk_limit_secs
                {
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
            door,
            depth,
            stay,
            nap,
            ..
        } => match activity.step {
            0 => {
                show(creature, ActionKind::Traverse, context.events);
                if village_paths::by_own_door(
                    creature,
                    context.paths,
                    door.x,
                    door,
                    depth,
                    speed,
                    context.frame / CREATURE_ART_WIDTH,
                    dt,
                ) || activity.step_elapsed > VILLAGE_LIFE.walk_limit_secs
                {
                    creature.state.position = door;
                    creature.state.depth = depth;
                    // Somebody close by sees it go in.
                    context.noticed(door, creature.id, 1.2);
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
                    // Out again, and somebody close by looks round; up from a nap, it has a
                    // stretch on the doorstep first.
                    context.noticed(door, creature.id, 1.4);
                    if nap {
                        beat(creature, BeatKind::Yawn, super::beats::YAWN_SECS, None);
                    }
                    activity.next();
                }
                true
            }
            // A moment on the doorstep, and off again.
            _ => activity.step_elapsed < 1.2 || !beat_done(creature),
        },
        Plan::Roof {
            beside, top, stay, ..
        } => match activity.step {
            0 => {
                show(creature, ActionKind::Traverse, context.events);
                if walk(creature, beside, speed, dt)
                    || activity.step_elapsed > VILLAGE_LIFE.walk_limit_secs
                {
                    show(creature, ActionKind::Landing, context.events);
                    activity.next();
                }
                true
            }
            // Up in a little hop, from a crouch.
            1 => {
                let t =
                    ((activity.step_elapsed - HOP_GATHER_SECS) / ROOF_HOP_UP_SECS).clamp(0.0, 1.0);
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
                let t = ((activity.step_elapsed - HOP_GATHER_SECS) / ROOF_HOP_DOWN_SECS)
                    .clamp(0.0, 1.0);
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
                if activity.step_elapsed >= VILLAGE_LIFE.leaf_fall_secs {
                    beat(
                        creature,
                        BeatKind::LeafOnFace,
                        VILLAGE_LIFE.leaf_beat_secs,
                        None,
                    );
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
            _ => activity.step_elapsed < VILLAGE_LIFE.leaf_drop_secs,
        },
        Plan::DroppedSnack {
            before,
            from,
            to,
            back,
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
                if walk(creature, goal, speed, dt)
                    || activity.step_elapsed > VILLAGE_LIFE.walk_limit_secs
                {
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
                    activity.next();
                }
                true
            }
            // Back to where it was: wherever the snack ran off to, somebody else is likely
            // standing, and the rest of it is eaten at its own spot.
            4 => {
                show(creature, ActionKind::Traverse, context.events);
                if walk(creature, back, speed, dt)
                    || activity.step_elapsed > VILLAGE_LIFE.walk_limit_secs
                {
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
                if walk(creature, beside, speed, dt)
                    || activity.step_elapsed > VILLAGE_LIFE.walk_limit_secs
                {
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
        Plan::Call(call) => company::advance_call(activity, call, creature, dt, context),
        Plan::Answer(answer) => company::advance_answer(activity, answer, creature, dt, context),
        Plan::LookOn(plan) => company::advance_look_on(activity, plan, creature, dt, context),
    }
}

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
                let t = (activity.step_elapsed / VILLAGE_LIFE.leaf_fall_secs).clamp(0.0, 1.0);
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
                let t = (activity.step_elapsed / VILLAGE_LIFE.leaf_drop_secs).clamp(0.0, 1.0);
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
    /// How far back on the ground it stands, in shelter pixels.
    pub(super) depth: f32,
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
    /// Where everybody else is busy or standing just now: no plan settles in for a while in
    /// front of any of them.
    pub(super) taken: &'a [Point],
    /// Whether one more can go indoors without leaving the village looking empty.
    pub(super) may_go_in: bool,
    /// How many more are out of doors than a village of this size has room for (`uncrowded`),
    /// up to `CROWD_MOST`: each one more makes going in likelier.
    pub(super) crowd: u32,
    /// Whether this is a village where about half keep indoors, and a spell in there is long.
    pub(super) long_stays: bool,
    /// Whether nobody is up on its roof already.
    pub(super) roof_free: bool,
    /// Whether one more of the quiet moments at a door may start, by a resident standing at its
    /// own place to have one: only so many at a time, for the size of the village.
    pub(super) moment_free: bool,
    /// Whether one more plan out and about may start: only so many of those at a time, too.
    pub(super) plan_free: bool,
    /// Whether no mishap is under way: one of those at a time, so each is worth noticing.
    pub(super) mishap_free: bool,
    /// Who is free for company, and has not lately been sought out by this resident.
    pub(super) friends: &'a [Friend],
    /// Who is at work somewhere it can be watched.
    pub(super) working: &'a [Working],
    /// What this resident has lately been up to.
    pub(super) recent: Recent,
    pub(super) paths: &'a village_paths::VillagePaths,
}

/// A resident free for company.
#[derive(Clone, Copy, Debug)]
pub(super) struct Friend {
    pub(super) id: CreatureId,
    pub(super) at: Point,
    /// How close the two are, from nothing to as close as it gets.
    pub(super) bond: f32,
}

/// A resident at work in a garden or on its house.
#[derive(Clone, Copy, Debug)]
pub(super) struct Working {
    pub(super) id: CreatureId,
    pub(super) at: Point,
    pub(super) depth: f32,
    pub(super) garden: bool,
    /// Having something to eat.
    pub(super) eating: bool,
}

/// The pastime a resident is fondest of, all its life: read from its own seed, so two residents
/// alike in every other way still spend their afternoons differently.
pub(super) fn favourite(creature: &Creature) -> Kind {
    const KINDS: [Kind; 5] = [
        Kind::Garden,
        Kind::Chore,
        Kind::Roof,
        Kind::Company,
        Kind::LookOn,
    ];
    KINDS[usize::from(creature.behavior_seed[5]) % KINDS.len()]
}

/// Who to go over to: anybody free, but a close friend sooner, and somebody near sooner than
/// somebody at the far end of the village.
fn choose_friend(
    rng: &mut ChaCha12Rng,
    creature: &Creature,
    options: &VillageOptions<'_>,
) -> Option<Friend> {
    let weights: Vec<u32> = options
        .friends
        .iter()
        .map(|friend| {
            let near = friend.at.distance(creature.state.position) <= options.frame * 4.0;
            2 + (friend.bond * 8.0).round() as u32 + 2 * u32::from(near)
        })
        .collect();
    Some(options.friends[weighted(rng, &weights)?])
}

/// Where a walk together goes: somewhere along the village a good few steps from here, clear of
/// anybody standing or busy there — up on the trodden path along the middle, as often as not,
/// where the village has one and no house stands in front of that stretch of it.
fn walk_destination(
    rng: &mut ChaCha12Rng,
    creature: &Creature,
    options: &VillageOptions<'_>,
) -> Option<(Point, f32)> {
    let frame = options.frame;
    let half = RESTING_WIDTH / 2.0 * options.commons.scale;
    let face_clear = frame * spacing::FACE_CLEAR_RATIO;
    (0..6).find_map(|_| {
        let to = options.commons.along(rng.random_range(0.0..1.0));
        let far = (to.x - creature.state.position.x).abs() >= frame * 3.0;
        // Room for two: the place itself, and a step either side of it.
        let clear = !options.taken.iter().any(|busy| {
            spacing::closer_than(busy.x - to.x, face_clear + frame)
                && busy.distance(creature.state.position) > 1.0
        });
        if !far || !clear {
            return None;
        }
        let on_path = options
            .paths
            .path_span()
            .is_some_and(|(low, high)| (low + frame..=high - frame).contains(&to.x))
            && [-frame, 0.0, frame].into_iter().all(|step| {
                !options
                    .paths
                    .hidden(to.x + step, village_paths::PATH_DEPTH, half)
            });
        let depth = if on_path && rng.random_bool(0.6) {
            village_paths::PATH_DEPTH
        } else {
            0.0
        };
        Some((to, depth))
    })
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
/// plan about the village — the garden, its house, indoors, its roof, a small mishap, or
/// somebody else: going over to one who is free, or stopping to look on at one who is busy.
///
/// How the choices weigh against one another is the resident's own: what it is like, what it is
/// fondest of, what it did last — seldom the same thing twice running — and what that left it
/// inclined to do next.
pub(super) fn choose(
    rng: &mut ChaCha12Rng,
    creature: &Creature,
    options: &VillageOptions<'_>,
) -> Choice {
    // Everything but going indoors is somebody else's turn: wait for one of those to end rather
    // than going in because there is nothing else to do — unless more are out than usually are,
    // when having nothing to do out here is as good a reason to go in as any.
    if !options.moment_free && !options.plan_free && options.crowd == 0 {
        return Choice::Wait;
    }
    let personality = &creature.personality;
    let sleepy = creature.state.drives.sleep_pressure > 0.5;
    let hungry = creature.state.drives.energy < 0.5;
    let garden = if options.plan_free {
        choose_garden(rng, creature, options)
    } else {
        None
    };
    let recent = options.recent;
    let fond = favourite(creature);
    // One choice's weight for this resident: more for one whose nature leans that way, for its
    // favourite, and for what it has just been left inclined to do; less for what it did last.
    let lean = |free: bool, weight: u32, kind: Kind, leaning: f32| -> u32 {
        if !free || weight == 0 {
            return 0;
        }
        let mut weight = weight as f32 * (0.25 + 1.5 * leaning.clamp(0.0, 1.0));
        if fond == kind {
            weight *= VILLAGE_LIFE.favourite_gain;
        }
        if recent.last == Some(kind) {
            weight *= VILLAGE_LIFE.again_share;
        }
        if recent.bent() == Some(kind) {
            weight *= VILLAGE_LIFE.bent_gain;
        }
        (weight.round() as u32).max(1)
    };
    let out = options.plan_free;
    let mishap = |weight: u32| {
        if out && options.mishap_free {
            weight
        } else {
            0
        }
    };
    let house = options.house;
    let company = out && !options.friends.is_empty();
    let rested = !sleepy && creature.state.drives.energy >= 0.3;
    let weights = [
        if !options.moment_free {
            0
        } else if recent.bent().is_some() {
            VILLAGE_LIFE.moment_weight / 2
        } else {
            VILLAGE_LIFE.moment_weight
        },
        lean(
            garden.is_some(),
            VILLAGE_LIFE.garden_weight,
            Kind::Garden,
            (personality.curiosity + personality.routine_affinity) / 2.0,
        ),
        lean(
            out && house.is_some(),
            VILLAGE_LIFE.chore_weight,
            Kind::Chore,
            personality.routine_affinity,
        ),
        // The more there are out beyond the usual, the likelier anybody is to go in, the
        // stay-at-home soonest; with no more out than usual it is a matter of its own nature.
        lean(
            house.is_some() && options.may_go_in,
            VILLAGE_LIFE.indoors_weight
                + 2 * u32::from(sleepy)
                + VILLAGE_LIFE.crowded_weight * options.crowd,
            Kind::Indoors,
            if options.crowd > 0 {
                0.35 + 0.65 * (1.0 - personality.activity)
            } else {
                1.0 - personality.activity
            },
        ),
        lean(
            out && house.is_some() && options.roof_free,
            VILLAGE_LIFE.roof_weight,
            Kind::Roof,
            (personality.curiosity + personality.boldness) / 2.0,
        ),
        mishap(VILLAGE_LIFE.leaf_weight),
        mishap(VILLAGE_LIFE.snack_weight + u32::from(hungry)),
        mishap(if options.cushions.is_empty() {
            0
        } else {
            VILLAGE_LIFE.cushion_weight + u32::from(sleepy)
        }),
        lean(
            company,
            VILLAGE_LIFE.hello_weight,
            Kind::Company,
            personality.sociability - 0.3 * f32::from(u8::from(sleepy)),
        ),
        lean(
            company && rested,
            VILLAGE_LIFE.play_weight,
            Kind::Company,
            personality.playfulness,
        ),
        lean(
            company && rested,
            VILLAGE_LIFE.walk_weight,
            Kind::Company,
            (personality.sociability + personality.activity) / 2.0,
        ),
        lean(
            out && !options.working.is_empty(),
            VILLAGE_LIFE.look_on_weight,
            Kind::LookOn,
            personality.curiosity,
        ),
    ];
    let Some(pick) = weighted(rng, &weights) else {
        return Choice::Wait;
    };
    let at = creature.state.position;
    let ground = options.commons.ground_y;
    let (low, high) = options.commons.standing_span();
    // The side of `x` this resident is coming from.
    let side_of = |x: f32| if at.x > x { 1.0 } else { -1.0 };
    // Somebody already standing or busy right there.
    let face_clear = options.frame * spacing::FACE_CLEAR_RATIO;
    let spoken_for = |x: f32| {
        options
            .taken
            .iter()
            .any(|busy| spacing::closer_than(busy.x - x, face_clear))
    };
    let plan = match (pick, house) {
        (0, _) => return Choice::Moment,
        (1, _) => match garden {
            Some(plan) => plan,
            None => return Choice::Wait,
        },
        (2, Some(house)) => {
            // On the side of the door it comes from, or the other side if somebody is standing
            // or busy there. A chore is over in a moment, so with both taken it goes ahead.
            let stand_at = |side: f32| house.at.x + side * house.half * 0.55;
            let side = side_of(house.at.x);
            let side = if spoken_for(stand_at(side)) && !spoken_for(stand_at(-side)) {
                -side
            } else {
                side
            };
            Plan::Chore {
                slot: house.slot,
                stand: Point {
                    x: stand_at(side),
                    y: ground,
                },
                door_x: house.at.x,
                depth: house.depth,
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
                depth: house.depth,
                stay: rng.random_range(match (nap, options.long_stays) {
                    (true, false) => VILLAGE_LIFE.indoors_nap_secs,
                    (false, false) => VILLAGE_LIFE.indoors_potter_secs,
                    (true, true) => VILLAGE_LIFE.indoors_long_nap_secs,
                    (false, true) => VILLAGE_LIFE.indoors_long_potter_secs,
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
            stay: rng.random_range(VILLAGE_LIFE.roof_secs),
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
            // No room ahead, or somebody standing or busy where it would stop: it rolls the other
            // way instead.
            let roll = rng.random_range(0.9..1.6) * options.frame;
            let ways = [from.x + ahead * roll, from.x - ahead * roll];
            let to = ways
                .into_iter()
                .find(|x| (low..=high).contains(x) && !spoken_for(*x))
                .unwrap_or(if (low..=high).contains(&ways[0]) {
                    ways[0]
                } else {
                    ways[1]
                });
            Plan::DroppedSnack {
                before: rng.random_range(2.0..4.0),
                from,
                to: Point {
                    x: to.clamp(low, high),
                    y: ground,
                },
                back: Point { x: at.x, y: ground },
                eat: rng.random_range(4.0..8.0),
            }
        }
        (8..=10, _) => {
            let Some(friend) = choose_friend(rng, creature, options) else {
                return Choice::Wait;
            };
            let walk = if pick == 10 {
                walk_destination(rng, creature, options)
            } else {
                None
            };
            let purpose = match (pick, walk) {
                (9, _) => Purpose::Play,
                (10, Some(_)) => Purpose::Walk,
                _ => Purpose::Hello,
            };
            let (to, to_depth) = walk.unwrap_or((at, creature.state.depth));
            // The sociable stay longer.
            let stay =
                rng.random_range(VILLAGE_LIFE.company_secs) * (0.7 + 0.6 * personality.sociability);
            Plan::Call(Call {
                on: friend.id,
                purpose,
                stay,
                to,
                to_depth,
            })
        }
        (11, _) => {
            let nearest = options
                .working
                .iter()
                .copied()
                .min_by(|a, b| a.at.distance(at).total_cmp(&b.at.distance(at)));
            let Some(worker) = nearest else {
                return Choice::Wait;
            };
            Plan::LookOn(LookOn {
                at: worker.id,
                spot: worker.at,
                spot_depth: worker.depth,
                shy: creature.temperament().kind == TemperamentKind::Wallflower
                    || personality.sociability < 0.3,
                join: worker.garden && personality.curiosity > 0.55,
                beg: worker.eating && hungry,
                stay: rng.random_range(VILLAGE_LIFE.look_on_secs),
            })
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
    // Never settling in for a while in front of somebody standing or busy there: it waits its
    // turn instead.
    if plan.spot().is_some_and(|spot| spoken_for(spot.x)) {
        return Choice::Wait;
    }
    Choice::Plan(plan)
}

impl Plan {
    /// Where a plan that is had at the front of the grass first takes its resident.
    fn at_the_front(&self) -> Option<Point> {
        match *self {
            Plan::Garden { stand, .. } => Some(stand),
            Plan::Roof { beside, .. } | Plan::MissedCushion { beside, .. } => Some(beside),
            _ => None,
        }
    }

    /// Where on the ground this plan keeps its resident a while, from the moment it sets off: the
    /// spot everybody else keeps clear of until it is done. A chore, a leaf on its face and the
    /// chase after a snack are over in a moment, and indoors and up on the roof it is nowhere
    /// anybody could stand in front of it.
    pub(super) fn spot(&self) -> Option<Point> {
        match *self {
            Self::Garden { stand, .. } => Some(stand),
            Self::MissedCushion { cushion, .. } => Some(cushion),
            Self::Chore { .. }
            | Self::DroppedSnack { .. }
            | Self::Indoors { .. }
            | Self::Roof { .. }
            | Self::LeafOnFace
            | Self::Call(_)
            | Self::Answer(_)
            | Self::LookOn(_) => None,
        }
    }

    /// Which kind of thing this is, for remembering what a resident last did.
    pub(super) fn kind(&self) -> Kind {
        match self {
            Self::Garden { .. } => Kind::Garden,
            Self::Chore { .. } => Kind::Chore,
            Self::Indoors { .. } => Kind::Indoors,
            Self::Roof { .. } => Kind::Roof,
            Self::LeafOnFace | Self::DroppedSnack { .. } | Self::MissedCushion { .. } => {
                Kind::Mishap
            }
            Self::Call(_) | Self::Answer(_) => Kind::Company,
            Self::LookOn(_) => Kind::LookOn,
        }
    }

    /// One of the little mishaps: only one of those at a time.
    pub(super) fn mishap(&self) -> bool {
        self.kind() == Kind::Mishap
    }

    /// Somebody else's idea: stopped for a caller, which counts against nobody's turn.
    pub(super) fn answering(&self) -> bool {
        matches!(self, Self::Answer(_))
    }

    /// Whether this is a spell indoors, which counts against how many may be in at once rather
    /// than against how many may be out and about.
    pub(super) fn indoors(&self) -> bool {
        matches!(self, Self::Indoors { .. })
    }
}

/// How long the hop up onto a roof takes, once the crouch it springs from is done.
const ROOF_HOP_UP_SECS: f32 = 0.55;
/// How long getting down off a roof takes, once crouched to go: the same whether the sit up there
/// ran its course or ended early.
const ROOF_HOP_DOWN_SECS: f32 = 0.45;

/// Puts a resident whose plan has ended early back on its own feet outside: out of doors, with
/// nothing in its hands, and resting. One up on its roof is not set down on the ground — that is
/// a jump of a house's height between two frames — but gets down the way it went up: the spot it
/// lands on is returned, and the caller starts the hop there with [`roof_hop_down`].
pub(super) fn settle(activity: &VillageActivity, creature: &mut Creature) -> Option<Point> {
    creature.state.indoors = false;
    creature.state.beat = None;
    creature.state.velocity = Point::default();
    if activity.plan.kind() == Kind::Company {
        // Sat down with a friend: up again.
        creature.state.attention = None;
    }
    let landing = match activity.plan {
        Plan::Roof { beside, .. } if activity.step >= 1 => Some(beside),
        Plan::Indoors { door, depth, .. } if activity.step >= 1 => {
            creature.state.position = door;
            creature.state.depth = depth;
            None
        }
        _ => None,
    };
    if creature.state.action != ActionKind::PetReaction {
        creature.state.action = ActionKind::Homebound;
        creature.state.action_elapsed = 0.0;
        creature.state.action_duration = super::home::HOME_DURATION.whole_seconds() as f32;
    }
    landing.filter(|beside| creature.state.position.distance(*beside) > 0.5)
}

/// A little hop from wherever `creature` is up on its roof down to `landing` on the village
/// ground, arcing like the roof plan's own way down.
pub(super) fn roof_hop_down(creature: &Creature, landing: Point) -> WindowJourney {
    let start = creature.state.position;
    WindowJourney::Hop(HopJourney {
        start,
        target: landing,
        surface: SurfaceAttachment {
            kind: SurfaceKind::ScreenFloor,
            monitor_id: creature.state.surface.monitor_id,
            window_key: None,
            relative_x: creature.state.surface.relative_x,
        },
        elapsed: 0.0,
        duration: ROOF_HOP_DOWN_SECS,
        lift: (landing.y - start.y).abs().max(6.0) * 0.5 + 6.0,
        ballistic: false,
    })
}

/// The most residents indoors at once for a village of `residents`: nobody while there is only
/// one, so a lone companion is never out of sight, one of two or three, and all but two of a
/// bigger village, so the village never looks empty.
pub(super) const fn max_indoors(residents: usize) -> usize {
    match residents {
        0 | 1 => 0,
        2 | 3 => 1,
        more => more - 2,
    }
}

/// How many of a village of `residents` may be out of doors before it counts as crowded:
/// everybody in a small one, and one fewer than half, though never fewer than two, of a village
/// of four or more. It is a leaning and not a limit: each resident out beyond it, up to
/// `CROWD_MOST`, makes going indoors likelier (`crowded_weight`), and the village settles about
/// one above it, with half its residents out. Everybody is still out together now and then, as
/// at the start of every visit home.
pub(super) const fn uncrowded(residents: usize) -> usize {
    if residents < 4 {
        residents
    } else {
        let fewer = residents / 2 - 1;
        if fewer < 2 { 2 } else { fewer }
    }
}

/// The most residents beyond `uncrowded` that add to the pull indoors: past this a full village
/// empties no faster, so everybody home together stays a sight for a little while.
pub(super) const CROWD_MOST: usize = 2;

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

    /// Ends one resident's plan, if it has one, and puts it back on the ground outside — in a
    /// hop, from up on a roof. Returns where that hop lands, if there is one.
    pub(super) fn end_village_activity(&mut self, creature_id: CreatureId) -> Option<Point> {
        let activity = self.village_life.remove(&creature_id)?;
        let creature = creature_mut(&mut self.save.creatures, creature_id)?;
        let landing = settle(&activity, creature)?;
        let hop = roof_hop_down(creature, landing);
        self.window_journeys.insert(creature_id, hop);
        Some(landing)
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
            if activity.plan.kind() == Kind::Company {
                creature.state.attention = None;
            }
        }
        // A job it was in the middle of is one to go back to, if it is soon free again.
        self.village_memory.interrupted(creature_id, activity.plan);
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
        let points_per_pixel = f32::from(self.save.settings.display_scale) / scale_factor.max(1.0);
        self.village_life
            .iter()
            .filter_map(|(creature_id, activity)| {
                let creature = self
                    .save
                    .creatures
                    .iter()
                    .find(|creature| creature.id == *creature_id)?;
                // A mishap back in the village happens as far up the display as it is drawn.
                let (prop, at) = loose_prop(activity, creature, face)?;
                Some((
                    prop,
                    Point {
                        y: at.y - creature.state.depth * points_per_pixel,
                        ..at
                    },
                ))
            })
            .collect()
    }
}
