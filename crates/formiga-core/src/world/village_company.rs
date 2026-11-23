//! Residents keeping one another company while the houses are out: going over to say hello and
//! staying a while, asking a friend to play and chasing one another up and down the grass, a
//! walk along the village together, stopping to look on while somebody works — and a nod in
//! passing.
//!
//! Company takes two. The one who goes over follows a [`Call`]; the one it goes over to is asked
//! with an [`Invite`] the moment the caller looks its way, and if it is free it stops, looks
//! back and follows an [`Answer`] of its own: a welcome, a shy one from behind its paws, or a
//! no, in the way its temperament says no. Each plan is stepped on its own resident's turn and
//! reads the other's from the survey taken as the tick began, so either can be picked up, called
//! away or ended at any moment and the other simply carries on from wherever that leaves it.
//! Every step has a time limit of its own, so nothing waits for ever on anybody.
//!
//! [`Memory`] is what the village remembers of all this between one turn and the next: who has
//! lately sought whom out, what each resident last did and what that leaves it inclined to do
//! next, and a job somebody was picked up in the middle of. Like everything else a village
//! does, none of it is saved, and none of it moves a bond, a counter or the journal.

use super::village_life::{
    self as life, Aside, Plan, VillageActivity, VillageContext, beat, beat_done, show, walk,
};
use super::*;
use crate::tuning::VILLAGE_LIFE;

/// How long a hello lasts, on both sides.
const GREET_SECS: f32 = 2.4;
/// How long the one going over looks at its friend before setting off.
const LOOK_SECS: f32 = 0.9;
/// How far apart two residents stand to be together, in creature widths: close, with both
/// faces clear.
const REACH: f32 = 0.95;
/// How near and how far, in frames, a neighbour's own place may be from somebody for one coming
/// to stand beside that somebody to stand exactly there instead.
const SNAP_REACH: std::ops::RangeInclusive<f32> = spacing::FACE_CLEAR_RATIO - 0.02..=1.3;
/// How much faster than an errand a game of chase is run.
const SPRINT: f32 = 2.2;
/// The longest one run of a chase may take before whoever is running it stops where it is.
const CHASE_LIMIT_SECS: f32 = 7.0;
/// How far a chase runs, in creature widths.
const CHASE_RUN_FRAMES: f32 = 3.0;
/// Company kept at least this long is kept sitting down.
const SIT_STAY_SECS: f32 = 9.0;
/// How long two who have been together leave it before one seeks the other out again.
const MET_AGAIN_SECS: std::ops::Range<f32> = 120.0..260.0;
/// And how long after a no.
const DECLINED_AGAIN_SECS: f32 = 300.0;
/// How long two who have nodded in passing leave it before nodding again, and how long two who
/// passed without a nod leave it before they might.
const NOD_AGAIN_SECS: f32 = 150.0;
const NO_NOD_AGAIN_SECS: f32 = 25.0;
/// How long a nod lasts.
const NOD_SECS: std::ops::Range<f32> = 0.7..1.1;
/// How long an inclination left by the last thing done lasts before it is forgotten.
const BENT_SECS: f32 = 25.0;
/// How long a job somebody was picked up from is remembered.
const RESUME_SECS: f32 = 45.0;

/// What going over to somebody is for, and so what follows a welcome.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Purpose {
    /// To say hello, and stay a while.
    Hello,
    /// To play: a short game of chase, there and back.
    Play,
    /// To go for a walk along the village together.
    Walk,
}

/// How somebody takes being called on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Reply {
    Warm,
    /// From behind its paws first.
    Shy,
    Decline,
}

/// Going over to another resident.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Call {
    pub(super) on: CreatureId,
    pub(super) purpose: Purpose,
    /// How long company is kept.
    pub(super) stay: f32,
    /// Where a walk together goes, and how far back that is; in a game of chase, where this one
    /// runs back to.
    pub(super) to: Point,
    pub(super) to_depth: f32,
}

/// Stopped for somebody coming over.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Answer {
    pub(super) to: CreatureId,
    pub(super) purpose: Purpose,
    pub(super) reply: Reply,
    pub(super) stay: f32,
    /// Where it ends a walk together, a step behind its friend; in a game of chase, where it
    /// runs to.
    pub(super) place: Point,
    pub(super) place_depth: f32,
    /// How a no is said, by one with a way of its own of saying it.
    pub(super) sulk: Option<BeatKind>,
}

/// Stopping to look on while somebody works.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct LookOn {
    pub(super) at: CreatureId,
    pub(super) spot: Point,
    pub(super) spot_depth: f32,
    /// Looks on from further off, and from behind its paws first.
    pub(super) shy: bool,
    /// Has a close look at the patch itself after a while.
    pub(super) join: bool,
    /// Looks on at somebody's snack the way the hungry do: sat up, paws together.
    pub(super) beg: bool,
    pub(super) stay: f32,
}

/// Whether a call that ended on `step` came to nothing: nobody looked back, or it was a no.
pub(super) const fn turned_down(step: u8) -> bool {
    step <= call::APPROACH || step == call::LET_DOWN
}

/// Whether somebody on this step of a plan in company is standing where it will stay a moment,
/// rather than on its way somewhere: a place for everybody else to keep clear of.
pub(super) fn keeps_still(plan: &Plan, step: u8) -> bool {
    match plan {
        Plan::Call(_) => matches!(
            step,
            call::GREET | call::COMPANY | call::ARRIVED | call::LET_DOWN
        ),
        Plan::Answer(_) => !matches!(step, answer::FLEE | answer::CHASE | answer::WALK),
        Plan::LookOn(_) => step >= look_on::PEEK,
        _ => false,
    }
}

/// The steps of a [`Call`].
mod call {
    pub(super) const LOOK: u8 = 0;
    pub(super) const APPROACH: u8 = 1;
    pub(super) const GREET: u8 = 2;
    pub(super) const COMPANY: u8 = 3;
    pub(super) const WIND_UP: u8 = 4;
    pub(super) const CHASE: u8 = 5;
    pub(super) const TAGGED: u8 = 6;
    pub(super) const RUN_BACK: u8 = 7;
    pub(super) const WAIT: u8 = 8;
    pub(super) const LAUGH: u8 = 9;
    pub(super) const WALK: u8 = 10;
    pub(super) const ARRIVED: u8 = 11;
    pub(super) const LET_DOWN: u8 = 12;
}

/// The steps of an [`Answer`].
mod answer {
    pub(super) const NOTICE: u8 = 0;
    pub(super) const REPLY: u8 = 1;
    pub(super) const COMPANY: u8 = 2;
    pub(super) const READY: u8 = 3;
    pub(super) const FLEE: u8 = 4;
    pub(super) const CAUGHT: u8 = 5;
    pub(super) const WIND_UP: u8 = 6;
    pub(super) const CHASE: u8 = 7;
    pub(super) const LAUGH: u8 = 8;
    pub(super) const WALK: u8 = 9;
    pub(super) const ARRIVED: u8 = 10;
    pub(super) const TURN_AWAY: u8 = 11;
}

/// The steps of a [`LookOn`].
mod look_on {
    pub(super) const LOOK: u8 = 0;
    pub(super) const WALK: u8 = 1;
    pub(super) const PEEK: u8 = 2;
    pub(super) const WATCH: u8 = 3;
    pub(super) const JOIN: u8 = 4;
    pub(super) const NOD: u8 = 5;
}

/// Somebody's part in company kept with another, as the other reads it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct PartView {
    pub(super) with: CreatureId,
    pub(super) step: u8,
    /// How it took being called on, for the one who was.
    pub(super) reply: Option<Reply>,
    /// Whether it has got as far as the hello: together, rather than on the way.
    pub(super) met: bool,
}

/// One resident as the tick began, for anybody whose plan is about it.
#[derive(Clone, Copy, Debug)]
pub(super) struct Resident {
    pub(super) id: CreatureId,
    pub(super) at: Point,
    pub(super) depth: f32,
    pub(super) part: Option<PartView>,
    /// At work in a garden or on its house: something to look on at.
    pub(super) working: bool,
    /// Where along the village its own place is, if it has one here.
    pub(super) home_x: Option<f32>,
}

/// One resident asking another for its company.
#[derive(Clone, Copy, Debug)]
pub(super) struct Invite {
    pub(super) from: CreatureId,
    pub(super) to: CreatureId,
    pub(super) purpose: Purpose,
    pub(super) stay: f32,
    pub(super) walk_to: Point,
    pub(super) walk_depth: f32,
}

/// The kinds of thing a resident does about the village, for remembering what it last did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Kind {
    Garden,
    Chore,
    Indoors,
    Roof,
    Mishap,
    Company,
    LookOn,
}

/// What one resident has lately been up to.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Recent {
    /// The last thing it did.
    pub(super) last: Option<Kind>,
    /// What that left it inclined to do next, and for how many seconds more.
    bent: Option<(Kind, f32)>,
    /// How many things it has done one straight after another.
    streak: u8,
    /// A job it was picked up in the middle of, and for how long it is still worth going back to.
    resume: Option<(Plan, f32)>,
}

impl Recent {
    /// What it is inclined to do next, if anything.
    pub(super) fn bent(&self) -> Option<Kind> {
        self.bent.map(|(kind, _)| kind)
    }
}

/// What the village remembers between one turn and the next.
#[derive(Clone, Debug, Default)]
pub(super) struct Memory {
    met: Vec<(CreatureId, CreatureId, f32)>,
    nodded: Vec<(CreatureId, CreatureId, f32)>,
    recent: BTreeMap<CreatureId, Recent>,
}

fn pair(a: CreatureId, b: CreatureId) -> (CreatureId, CreatureId) {
    if a < b { (a, b) } else { (b, a) }
}

fn note(list: &mut Vec<(CreatureId, CreatureId, f32)>, a: CreatureId, b: CreatureId, secs: f32) {
    let (a, b) = pair(a, b);
    match list.iter_mut().find(|(x, y, _)| (*x, *y) == (a, b)) {
        Some(entry) => entry.2 = entry.2.max(secs),
        None => list.push((a, b, secs)),
    }
}

fn noted(list: &[(CreatureId, CreatureId, f32)], a: CreatureId, b: CreatureId) -> bool {
    let (a, b) = pair(a, b);
    list.iter().any(|(x, y, _)| (*x, *y) == (a, b))
}

impl Memory {
    pub(super) fn clear(&mut self) {
        *self = Self::default();
    }

    pub(super) fn forget(&mut self, id: CreatureId) {
        self.met.retain(|(a, b, _)| *a != id && *b != id);
        self.nodded.retain(|(a, b, _)| *a != id && *b != id);
        self.recent.remove(&id);
    }

    /// Lets `dt` go by.
    pub(super) fn age(&mut self, dt: f32) {
        for list in [&mut self.met, &mut self.nodded] {
            list.retain_mut(|(.., left)| {
                *left -= dt;
                *left > 0.0
            });
        }
        for recent in self.recent.values_mut() {
            for left in [
                recent.bent.as_mut().map(|(_, left)| left),
                recent.resume.as_mut().map(|(_, left)| left),
            ]
            .into_iter()
            .flatten()
            {
                *left -= dt;
            }
            recent.bent = recent.bent.filter(|(_, left)| *left > 0.0);
            recent.resume = recent.resume.filter(|(_, left)| *left > 0.0);
        }
    }

    /// Whether these two have lately sought one another out.
    pub(super) fn met(&self, a: CreatureId, b: CreatureId) -> bool {
        noted(&self.met, a, b)
    }

    pub(super) fn note_met(&mut self, a: CreatureId, b: CreatureId, secs: f32) {
        note(&mut self.met, a, b, secs);
    }

    pub(super) fn recent(&self, id: CreatureId) -> Recent {
        self.recent.get(&id).copied().unwrap_or_default()
    }

    /// Whether this resident has something it means to get on with straight away.
    pub(super) fn eager(&self, id: CreatureId) -> bool {
        self.recent
            .get(&id)
            .is_some_and(|recent| recent.bent.is_some() || recent.resume.is_some())
    }

    /// It has had its turn: whatever it was inclined to do has been weighed, and is let go.
    pub(super) fn spent(&mut self, id: CreatureId) {
        if let Some(recent) = self.recent.get_mut(&id) {
            recent.bent = None;
        }
    }

    /// Notes what a resident has just finished, and what that leaves it inclined to do next.
    /// Returns whether it means to get on with that straight away rather than rest first.
    pub(super) fn finished(
        &mut self,
        id: CreatureId,
        plan: &Plan,
        turned_down: bool,
        rng: &mut ChaCha12Rng,
    ) -> bool {
        let recent = self.recent.entry(id).or_default();
        let kind = plan.kind();
        recent.last = Some(kind);
        recent.resume = None;
        let next = match *plan {
            // Up from a nap: a tidy round the house. Up from pottering about: out to see
            // somebody.
            Plan::Indoors { nap: true, .. } => Some(Kind::Chore),
            Plan::Indoors { nap: false, .. } => Some(Kind::Company),
            // The house seen to: the garden next, or company.
            Plan::Chore { .. } => Some(if rng.random_bool(0.5) {
                Kind::Garden
            } else {
                Kind::Company
            }),
            // One patch leads to another.
            Plan::Garden {
                task: life::GardenTask::Water | life::GardenTask::Inspect,
                ..
            } => Some(Kind::Garden),
            // Having looked out over everybody from up there: over to one of them.
            Plan::Roof { .. } => Some(Kind::Company),
            // Having watched somebody at it: a go itself.
            Plan::LookOn(_) => Some(Kind::Garden),
            // Turned down: something of its own instead.
            Plan::Call(_) if turned_down => Some(Kind::Garden),
            _ => None,
        };
        let follows = recent.streak < VILLAGE_LIFE.follow_on_most
            && next.is_some()
            && rng.random_range(0..100) < VILLAGE_LIFE.follow_on_chance;
        if follows {
            recent.streak += 1;
            recent.bent = next.map(|kind| (kind, BENT_SECS));
        } else {
            recent.streak = 0;
            recent.bent = None;
        }
        follows
    }

    /// Notes a job somebody was taken from in the middle of, to go back to if it is soon free.
    pub(super) fn interrupted(&mut self, id: CreatureId, plan: Plan) {
        if matches!(plan, Plan::Garden { .. } | Plan::Chore { .. }) {
            self.recent.entry(id).or_default().resume = Some((plan, RESUME_SECS));
        }
    }

    /// The job this resident was taken from, if it is still worth going back to. Asked once.
    pub(super) fn resume(&mut self, id: CreatureId) -> Option<Plan> {
        self.recent
            .get_mut(&id)?
            .resume
            .take()
            .map(|(plan, _)| plan)
    }
}

impl VillageContext<'_> {
    fn resident(&self, id: CreatureId) -> Option<Resident> {
        self.everyone.iter().copied().find(|other| other.id == id)
    }

    /// Where to stand to be beside somebody at `at`, `depth` back, coming from `from`: on the
    /// side it comes from, unless the grass ends there, and no further back than the grass
    /// there goes.
    fn beside(
        &self,
        pair: (CreatureId, CreatureId),
        at: Point,
        depth: f32,
        from: Point,
        apart: f32,
    ) -> (Point, f32) {
        let (low, high) = self.span;
        let side = if from.x >= at.x { 1.0 } else { -1.0 };
        let reach = self.frame * apart;
        // Not on top of a third who is standing there already.
        let crowded = |x: f32| {
            self.everyone.iter().any(|other| {
                other.id != pair.0
                    && other.id != pair.1
                    && (other.at.x - x).abs() < self.frame * 0.85
            })
        };
        // Somebody standing a while keeps whoever lives right there out of its own place, and
        // between two places it keeps two out, who then have one free place between them to
        // wait in. So where a neighbour's place is about that far off anyway, it is stood at
        // exactly, and only that neighbour waits — in the place this one left.
        let clear = self.frame * spacing::FACE_CLEAR_RATIO;
        let homes = || {
            self.everyone
                .iter()
                .filter(|other| other.id != pair.0 && other.id != pair.1)
                .filter_map(|other| other.home_x)
        };
        let settle = |x: f32| {
            homes()
                .filter(|home| {
                    (home - x).abs() < clear
                        && SNAP_REACH.contains(&((home - at.x).abs() / self.frame))
                })
                .min_by(|a, b| (a - x).abs().total_cmp(&(b - x).abs()))
                .unwrap_or(x)
        };
        let sides = [at.x + side * reach, at.x - side * reach].map(settle);
        let x = sides
            .into_iter()
            .find(|x| (low..=high).contains(x) && !crowded(*x))
            .or_else(|| sides.into_iter().find(|x| (low..=high).contains(x)))
            .unwrap_or((at.x + side * reach).clamp(low, high));
        (
            Point {
                x,
                y: self.ground_y,
            },
            depth.min(self.paths.open_depth(x, self.half)),
        )
    }
}

impl VillageContext<'_> {
    /// Whether somebody other than these two is standing right beside `creature`, however far
    /// back either is — the grass is shallow, and one stands in the other's face all the same:
    /// company is not kept in a neighbour's face.
    fn crowded(&self, creature: &Creature, friend: CreatureId) -> bool {
        self.everyone.iter().any(|other| {
            other.id != creature.id
                && other.id != friend
                && (other.at.x - creature.state.position.x).abs()
                    < self.frame * spacing::FACE_CLEAR_RATIO
        })
    }
}

/// How long company is kept with somebody else standing right beside it before the two make
/// room and go their ways.
const CROWDED_SECS: f32 = 3.0;

/// Turns `creature` to face `at`, unless it is right on top of it.
fn face(creature: &mut Creature, at: Point) {
    if (at.x - creature.state.position.x).abs() > 1.0 {
        creature.state.facing_right = at.x > creature.state.position.x;
    }
}

/// Starts a look at `at`.
fn look(creature: &mut Creature, kind: BeatKind, length: f32, at: Point) {
    creature.state.beat = Some(Beat {
        look: Some(at),
        ..Beat::new(kind, length)
    });
}

/// Where a run away from `from` ends for somebody at `at`, `depth` back: `CHASE_RUN_FRAMES`
/// off if there is room, the other way if the grass ends too soon on that side, and never
/// somewhere out of sight behind a house nearer the front — a shorter run, or none, rather
/// than that.
pub(super) fn run_from(
    at: Point,
    from: Point,
    depth: f32,
    frame: f32,
    span: (f32, f32),
    paths: &village_paths::VillagePaths,
    half: f32,
) -> Point {
    let (low, high) = span;
    let away = if at.x >= from.x { 1.0 } else { -1.0 };
    let room = |way: f32| {
        if way > 0.0 { high - at.x } else { at.x - low }
    };
    let full = frame * CHASE_RUN_FRAMES;
    let ways = if room(away) >= full * 0.6 || room(away) >= room(-away) {
        [away, -away]
    } else {
        [-away, away]
    };
    let x = ways
        .into_iter()
        .flat_map(|way| {
            [1.0, 0.8, 0.6, 0.45]
                .into_iter()
                .map(move |share| at.x + way * full * share)
        })
        .find(|x| (low..=high).contains(x) && !paths.hidden(*x, depth, half))
        .unwrap_or(at.x);
    Point { x, y: at.y }
}

/// One step of a chase after somebody at `quarry`, along the strip. Returns whether it has
/// caught up.
fn chase(creature: &mut Creature, quarry: Point, speed: f32, frame: f32, dt: f32) -> bool {
    let at = creature.state.position;
    let gap = quarry.x - at.x;
    if gap.abs() <= frame * (REACH + 0.1) {
        return true;
    }
    let goal = Point {
        x: quarry.x - gap.signum() * frame * REACH,
        y: at.y,
    };
    walk(creature, goal, speed * SPRINT, dt);
    (quarry.x - creature.state.position.x).abs() <= frame * (REACH + 0.1)
}

/// Company kept: turned to one another, and, kept long enough, sat down — looking at the friend
/// and out over the grass by turns, the two of them out of step. Kept standing, it is a glance
/// across now and then.
fn keep_company(
    creature: &mut Creature,
    friend: Point,
    lead: bool,
    elapsed: f32,
    stay: f32,
    frame: f32,
    dt: f32,
) {
    face(creature, friend);
    let turn = (elapsed + if lead { 0.0 } else { 3.0 }) % 6.0;
    if stay >= SIT_STAY_SECS && elapsed > 1.5 && elapsed < stay - 0.8 {
        let unit = frame / CREATURE_ART_WIDTH;
        let at = creature.state.position;
        let ahead = if creature.state.facing_right {
            1.0
        } else {
            -1.0
        };
        let target = if turn < 3.5 {
            Point {
                x: friend.x,
                y: at.y - 12.0 * unit,
            }
        } else {
            Point {
                x: at.x - ahead * 30.0 * unit,
                y: at.y - 26.0 * unit,
            }
        };
        creature.state.beat = None;
        creature.state.attention = Some(AttentionPose {
            target,
            emotion: AttentionEmotion::Enjoying,
            hanging: 0.0,
            gesture: Some(Gesture::Sit),
        });
    } else {
        creature.state.attention = None;
        if creature.state.beat.is_none() && turn < dt && elapsed > 1.0 {
            look(creature, BeatKind::Notice, 1.0, friend);
        }
    }
}

/// Moves a call on by `dt`. Returns `false` once it is over.
pub(super) fn advance_call(
    activity: &mut VillageActivity,
    call: Call,
    creature: &mut Creature,
    dt: f32,
    context: &mut VillageContext<'_>,
) -> bool {
    let Some(friend) = context.resident(call.on) else {
        return false;
    };
    let frame = context.frame;
    let speed = life::errand_speed(creature);
    let with_me = friend.part.filter(|part| part.with == creature.id);
    let entering = activity.entering();
    match activity.step {
        // A look across first, and the friend asked.
        call::LOOK => {
            if entering {
                face(creature, friend.at);
                show(creature, ActionKind::Idle, context.events);
                look(creature, BeatKind::Notice, LOOK_SECS, friend.at);
                context.invites.push(Invite {
                    from: creature.id,
                    to: call.on,
                    purpose: call.purpose,
                    stay: call.stay,
                    walk_to: call.to,
                    walk_depth: call.to_depth,
                });
            }
            if activity.step_elapsed >= LOOK_SECS {
                creature.state.beat = None;
                // Busy with something of its own after all: another time.
                if with_me.is_none() {
                    return false;
                }
                activity.next();
            }
            true
        }
        call::APPROACH => {
            if with_me.is_none() {
                return false;
            }
            let (goal, depth) = context.beside(
                (creature.id, call.on),
                friend.at,
                friend.depth,
                creature.state.position,
                REACH,
            );
            show(creature, ActionKind::Traverse, context.events);
            let there = context.paths.walk_to(creature, goal, depth, speed, dt);
            if there || activity.step_elapsed > VILLAGE_LIFE.walk_limit_secs {
                if !there && (friend.at.x - creature.state.position.x).abs() > frame * 2.0 {
                    return false;
                }
                face(creature, friend.at);
                show(creature, ActionKind::Greet, context.events);
                activity.next();
            }
            true
        }
        call::GREET => {
            face(creature, friend.at);
            if activity.step_elapsed < GREET_SECS {
                return true;
            }
            let Some(reply) = with_me.and_then(|part| part.reply) else {
                return false;
            };
            show(creature, ActionKind::Idle, context.events);
            activity.go_to(match (reply, call.purpose) {
                (Reply::Decline, _) => call::LET_DOWN,
                (_, Purpose::Hello) => call::COMPANY,
                (_, Purpose::Play) => {
                    beat(creature, BeatKind::Pounce, 0.6, None);
                    call::WIND_UP
                }
                (_, Purpose::Walk) => call::WALK,
            });
            true
        }
        call::COMPANY => {
            let crowded =
                activity.step_elapsed > CROWDED_SECS && context.crowded(creature, call.on);
            if activity.step_elapsed >= call.stay || with_me.is_none() || crowded {
                creature.state.attention = None;
                return false;
            }
            keep_company(
                creature,
                friend.at,
                true,
                activity.step_elapsed,
                call.stay,
                frame,
                dt,
            );
            true
        }
        // Crouched, and off after it.
        call::WIND_UP => {
            face(creature, friend.at);
            if beat_done(creature) {
                creature.state.beat = None;
                activity.next();
            }
            true
        }
        call::CHASE => {
            if with_me.is_none() {
                return false;
            }
            show(creature, ActionKind::Sprint, context.events);
            if chase(creature, friend.at, speed, frame, dt)
                || activity.step_elapsed > CHASE_LIMIT_SECS
            {
                face(creature, friend.at);
                show(creature, ActionKind::Idle, context.events);
                activity.next();
            }
            true
        }
        // Caught it: a moment, and then it is this one's turn to run.
        call::TAGGED => {
            face(creature, friend.at);
            if activity.step_elapsed >= 0.8 {
                let to = run_from(
                    creature.state.position,
                    friend.at,
                    creature.state.depth,
                    frame,
                    context.span,
                    context.paths,
                    context.half,
                );
                activity.plan = Plan::Call(Call { to, ..call });
                activity.next();
            }
            true
        }
        call::RUN_BACK => {
            show(creature, ActionKind::Sprint, context.events);
            let goal = Point {
                x: call.to.x,
                y: creature.state.position.y,
            };
            if walk(creature, goal, speed * SPRINT * 0.9, dt)
                || activity.step_elapsed > CHASE_LIMIT_SECS
            {
                face(creature, friend.at);
                show(creature, ActionKind::Idle, context.events);
                activity.next();
            }
            true
        }
        call::WAIT => {
            face(creature, friend.at);
            let caught = (friend.at.x - creature.state.position.x).abs() <= frame * (REACH + 0.2);
            if caught || with_me.is_none() || activity.step_elapsed > CHASE_LIMIT_SECS {
                show(creature, ActionKind::SocialPlay, context.events);
                activity.next();
            }
            true
        }
        call::LAUGH => {
            face(creature, friend.at);
            activity.step_elapsed < GREET_SECS
        }
        call::WALK => {
            show(creature, ActionKind::Traverse, context.events);
            if context
                .paths
                .walk_to(creature, call.to, call.to_depth, speed * 0.85, dt)
                || activity.step_elapsed > VILLAGE_LIFE.walk_limit_secs
            {
                face(creature, friend.at);
                show(creature, ActionKind::Idle, context.events);
                look(creature, BeatKind::Notice, 1.4, friend.at);
                activity.next();
            }
            true
        }
        // There: a look round at the friend coming up behind, and each its own way.
        call::ARRIVED => {
            face(creature, friend.at);
            activity.step_elapsed < 2.5
        }
        // Not just now: a moment standing there, and off again.
        _ => activity.step_elapsed < 1.2,
    }
}

/// Moves an answer on by `dt`. Returns `false` once it is over.
pub(super) fn advance_answer(
    activity: &mut VillageActivity,
    answer: Answer,
    creature: &mut Creature,
    dt: f32,
    context: &mut VillageContext<'_>,
) -> bool {
    let Some(caller) = context.resident(answer.to) else {
        return false;
    };
    let frame = context.frame;
    let speed = life::errand_speed(creature);
    let calling = caller.part.filter(|part| part.with == creature.id);
    let entering = activity.entering();
    match activity.step {
        // Stopped, and watching it come.
        answer::NOTICE => {
            let Some(part) = calling else {
                return false;
            };
            if entering {
                show(creature, ActionKind::Idle, context.events);
            }
            face(creature, caller.at);
            look(
                creature,
                BeatKind::Notice,
                VILLAGE_LIFE.walk_limit_secs + 4.0,
                caller.at,
            );
            if let Some(beat) = &mut creature.state.beat {
                beat.elapsed = activity.step_elapsed;
            }
            if part.step >= call::GREET {
                creature.state.beat = None;
                activity.next();
            }
            activity.step_elapsed < VILLAGE_LIFE.walk_limit_secs + 3.0
        }
        answer::REPLY => {
            if entering {
                face(creature, caller.at);
                match answer.reply {
                    Reply::Warm => show(creature, ActionKind::Greet, context.events),
                    Reply::Shy => beat(creature, BeatKind::Peek, GREET_SECS - 0.2, None),
                    Reply::Decline => match answer.sulk {
                        Some(sulk) => beat(creature, sulk, GREET_SECS - 0.2, None),
                        None => look(creature, BeatKind::Notice, 0.9, caller.at),
                    },
                }
            }
            // A no with no way of its own of saying it is a look, and a turn away.
            if answer.reply == Reply::Decline
                && answer.sulk.is_none()
                && activity.step_elapsed > 1.0
            {
                creature.state.facing_right = caller.at.x < creature.state.position.x;
            }
            if activity.step_elapsed < GREET_SECS + 0.2 {
                return true;
            }
            creature.state.beat = None;
            show(creature, ActionKind::Idle, context.events);
            activity.go_to(match (answer.reply, answer.purpose) {
                (Reply::Decline, _) => {
                    creature.state.facing_right = caller.at.x < creature.state.position.x;
                    answer::TURN_AWAY
                }
                (_, Purpose::Hello) => answer::COMPANY,
                (_, Purpose::Play) => answer::READY,
                (_, Purpose::Walk) => answer::WALK,
            });
            true
        }
        answer::COMPANY => {
            let crowded =
                activity.step_elapsed > CROWDED_SECS && context.crowded(creature, answer.to);
            if activity.step_elapsed >= answer.stay || calling.is_none() || crowded {
                creature.state.attention = None;
                return false;
            }
            keep_company(
                creature,
                caller.at,
                false,
                activity.step_elapsed,
                answer.stay,
                frame,
                dt,
            );
            true
        }
        // The friend crouches: off.
        answer::READY => {
            face(creature, caller.at);
            let Some(part) = calling else {
                return false;
            };
            if part.step >= call::WIND_UP {
                activity.next();
            }
            activity.step_elapsed < 3.0
        }
        answer::FLEE => {
            let Some(part) = calling else {
                return false;
            };
            let goal = Point {
                x: answer.place.x,
                y: creature.state.position.y,
            };
            if walk(creature, goal, speed * SPRINT * 0.9, dt) {
                face(creature, caller.at);
                show(creature, ActionKind::Idle, context.events);
            } else {
                show(creature, ActionKind::Sprint, context.events);
            }
            if part.step >= call::TAGGED || activity.step_elapsed > CHASE_LIMIT_SECS + 2.0 {
                face(creature, caller.at);
                show(creature, ActionKind::Idle, context.events);
                beat(creature, BeatKind::Startle, 0.7, None);
                activity.next();
            }
            true
        }
        // Caught. And now the other one runs.
        answer::CAUGHT => {
            face(creature, caller.at);
            let running = calling.is_some_and(|part| part.step >= call::RUN_BACK);
            if beat_done(creature) && (running || activity.step_elapsed > 2.5) {
                if calling.is_none() {
                    return false;
                }
                beat(creature, BeatKind::Pounce, 0.6, None);
                activity.next();
            }
            true
        }
        answer::WIND_UP => {
            face(creature, caller.at);
            if beat_done(creature) {
                creature.state.beat = None;
                activity.next();
            }
            true
        }
        answer::CHASE => {
            if calling.is_none() {
                return false;
            }
            show(creature, ActionKind::Sprint, context.events);
            if chase(creature, caller.at, speed, frame, dt)
                || activity.step_elapsed > CHASE_LIMIT_SECS
            {
                face(creature, caller.at);
                show(creature, ActionKind::SocialPlay, context.events);
                activity.next();
            }
            true
        }
        answer::LAUGH => {
            face(creature, caller.at);
            activity.step_elapsed < GREET_SECS
        }
        // A step behind its friend, all the way there.
        answer::WALK => {
            if activity.step_elapsed < 0.6 {
                face(creature, caller.at);
                return true;
            }
            show(creature, ActionKind::Traverse, context.events);
            if context
                .paths
                .walk_to(creature, answer.place, answer.place_depth, speed * 0.85, dt)
                || activity.step_elapsed > VILLAGE_LIFE.walk_limit_secs
            {
                face(creature, caller.at);
                show(creature, ActionKind::Idle, context.events);
                look(creature, BeatKind::Notice, 1.4, caller.at);
                activity.next();
            }
            true
        }
        answer::ARRIVED => {
            face(creature, caller.at);
            activity.step_elapsed < 2.5
        }
        _ => activity.step_elapsed < 1.4,
    }
}

/// Moves a look on at somebody working on by `dt`. Returns `false` once it is over.
pub(super) fn advance_look_on(
    activity: &mut VillageActivity,
    plan: LookOn,
    creature: &mut Creature,
    dt: f32,
    context: &mut VillageContext<'_>,
) -> bool {
    let Some(worker) = context.resident(plan.at) else {
        return false;
    };
    let speed = life::errand_speed(creature);
    let entering = activity.entering();
    let watch = |creature: &mut Creature| {
        if plan.beg {
            beat(creature, BeatKind::Beg, plan.stay, None);
        } else {
            look(creature, BeatKind::Notice, plan.stay, plan.spot);
        }
    };
    match activity.step {
        look_on::LOOK => {
            if entering {
                face(creature, plan.spot);
                show(creature, ActionKind::Idle, context.events);
                look(creature, BeatKind::Notice, LOOK_SECS, plan.spot);
            }
            if activity.step_elapsed >= LOOK_SECS {
                creature.state.beat = None;
                activity.next();
            }
            true
        }
        look_on::WALK => {
            // Beside whoever is at it, on the side this one comes from; from a step or two
            // further off for one too shy to stand close.
            let apart = if plan.shy { 2.0 } else { 1.1 };
            let (goal, depth) = context.beside(
                (creature.id, plan.at),
                plan.spot,
                plan.spot_depth,
                creature.state.position,
                apart,
            );
            show(creature, ActionKind::Traverse, context.events);
            if context.paths.walk_to(creature, goal, depth, speed, dt)
                || activity.step_elapsed > VILLAGE_LIFE.walk_limit_secs
            {
                face(creature, plan.spot);
                show(creature, ActionKind::Idle, context.events);
                if !worker.working {
                    // Done by the time it got there: a look at how it came out all the same.
                    activity.go_to(look_on::NOD);
                } else if plan.shy {
                    beat(creature, BeatKind::Peek, 2.0, None);
                    activity.next();
                } else {
                    watch(creature);
                    activity.go_to(look_on::WATCH);
                }
            }
            true
        }
        look_on::PEEK => {
            if beat_done(creature) {
                watch(creature);
                activity.next();
            }
            true
        }
        look_on::WATCH => {
            face(creature, plan.spot);
            let crowded =
                activity.step_elapsed > CROWDED_SECS && context.crowded(creature, plan.at);
            if beat_done(creature) || !worker.working || crowded {
                creature.state.beat = None;
                if plan.join && worker.working {
                    beat(creature, BeatKind::InspectSprout, 3.5, None);
                    activity.next();
                } else {
                    activity.go_to(look_on::NOD);
                }
            }
            true
        }
        // A close look of its own at what is coming up.
        look_on::JOIN => {
            if beat_done(creature) || !worker.working {
                creature.state.beat = None;
                activity.next();
            }
            true
        }
        // Done: the one who was at it looks round, and gets a pleased look back.
        _ => {
            if entering {
                if !worker.working && worker.part.is_none() {
                    context.asides.push(Aside {
                        creature: plan.at,
                        beat: BeatKind::Notice,
                        length: 1.4,
                        look: creature.state.position,
                    });
                    face(creature, worker.at);
                    look(creature, BeatKind::Admiring, 1.6, worker.at);
                } else {
                    return false;
                }
            }
            activity.step_elapsed < 1.8
        }
    }
}

/// How a resident takes being called on, and how it says no if it does: down to how sociable
/// it is, how close the two are, how sleepy it is, and its temperament.
pub(super) fn reply_of(
    creature: &Creature,
    purpose: Purpose,
    bond: f32,
    rng: &mut ChaCha12Rng,
) -> (Reply, Option<BeatKind>) {
    let personality = &creature.personality;
    let drives = &creature.state.drives;
    let kind = creature.temperament().kind;
    let sleepy = drives.sleep_pressure > 0.6;
    let mut welcome = 0.5 + personality.sociability * 0.35 + bond * 0.3;
    if purpose == Purpose::Play {
        welcome += personality.playfulness * 0.4 - 0.25;
        if drives.energy < 0.3 {
            welcome -= 0.3;
        }
    }
    if sleepy {
        welcome -= 0.6;
    }
    match kind {
        TemperamentKind::Grump => welcome -= 0.3,
        TemperamentKind::Lazybones if purpose != Purpose::Hello => welcome -= 0.3,
        TemperamentKind::Sweetheart => welcome += 0.2,
        _ => {}
    }
    if !rng.random_bool(f64::from(welcome.clamp(0.08, 0.97))) {
        let sulk = if sleepy {
            Some(BeatKind::Yawn)
        } else {
            match kind {
                TemperamentKind::Grump => Some(BeatKind::Huff),
                TemperamentKind::Troublemaker | TemperamentKind::Showoff => Some(BeatKind::Stomp),
                _ => None,
            }
        };
        return (Reply::Decline, sulk);
    }
    let shy = kind == TemperamentKind::Wallflower || personality.sociability < 0.3;
    (if shy { Reply::Shy } else { Reply::Warm }, None)
}

impl World {
    /// How close two residents are, from nothing to as close as it gets.
    pub(super) fn bond(&self, a: CreatureId, b: CreatureId) -> f32 {
        relationship_between(&self.save.relationships, a, b)
            .map_or(0.0, |relationship| f32::from(relationship.affinity) / 255.0)
    }

    /// Asks each resident somebody looked across at this tick for its company. One who is free
    /// stops and answers, in its own way; one busy with anything at all is left to it, and
    /// whoever asked finds out a moment later that nobody looked back.
    pub(super) fn apply_invites(
        &mut self,
        invites: Vec<Invite>,
        frame: f32,
        span: (f32, f32),
        paths: &village_paths::VillagePaths,
        half: f32,
    ) {
        for invite in invites {
            // Somebody out of sight behind a house is not stopped there to wait for a caller.
            if self.village_life.contains_key(&invite.to)
                || self.behind_houses.contains(&invite.to)
                || self.home_moments.contains_key(&invite.to)
                || self.wonders.owns(invite.to)
                || self
                    .interaction
                    .as_ref()
                    .is_some_and(|interaction| interaction.creature_id == invite.to)
            {
                continue;
            }
            let bond = self.bond(invite.from, invite.to);
            let Some(from) = self
                .save
                .creatures
                .iter()
                .find(|creature| creature.id == invite.from)
                .map(|creature| creature.state.position)
            else {
                continue;
            };
            let Some(creature) = creature_mut(&mut self.save.creatures, invite.to) else {
                continue;
            };
            let free = creature
                .state
                .beat
                .is_none_or(|beat| beat.kind == BeatKind::Notice)
                && !creature.state.indoors
                && creature.state.attention.is_none()
                && creature.state.arrival_delay_secs <= 0.0
                && matches!(
                    creature.state.action,
                    ActionKind::Homebound | ActionKind::Traverse
                );
            if !free {
                continue;
            }
            let (reply, sulk) =
                reply_of(creature, invite.purpose, bond, &mut self.village_life_rng);
            let at = creature.state.position;
            let (place, place_depth) = match invite.purpose {
                Purpose::Play => (
                    run_from(at, from, creature.state.depth, frame, span, paths, half),
                    creature.state.depth,
                ),
                // A step behind, on the side the two set out from.
                Purpose::Hello | Purpose::Walk => {
                    let behind = if invite.walk_to.x >= from.x {
                        -1.0
                    } else {
                        1.0
                    };
                    (
                        Point {
                            x: (invite.walk_to.x + behind * frame * REACH).clamp(span.0, span.1),
                            y: invite.walk_to.y,
                        },
                        invite.walk_depth,
                    )
                }
            };
            creature.state.beat = None;
            creature.state.velocity = Point::default();
            self.village_life.insert(
                invite.to,
                VillageActivity::new(Plan::Answer(Answer {
                    to: invite.from,
                    purpose: invite.purpose,
                    reply,
                    stay: invite.stay,
                    place,
                    place_depth,
                    sulk,
                })),
            );
            let again = if reply == Reply::Decline {
                DECLINED_AGAIN_SECS
            } else {
                self.village_life_rng.random_range(MET_AGAIN_SECS)
            };
            self.village_memory.note_met(invite.from, invite.to, again);
        }
    }

    /// Two residents with nothing on who come face to face as one of them walks by stop for a
    /// moment and look at one another — the likelier the more sociable the pair — and walk on.
    /// Not every time, and not the same two again for a while.
    pub(super) fn nod_in_passing(&mut self, frame: f32) {
        let count = self.save.creatures.len();
        for walker in 0..count {
            let a = &self.save.creatures[walker];
            if a.state.action != ActionKind::Traverse || !beats::free_for_a_beat(self, a) {
                continue;
            }
            for other in 0..count {
                let b = &self.save.creatures[other];
                let apart = (b.state.position.x - a.state.position.x).abs();
                if other == walker
                    || apart > frame * 1.2
                    || apart < frame * 0.8
                    || (a.state.depth - b.state.depth).abs() > 2.0
                    || noted(&self.village_memory.nodded, a.id, b.id)
                    || !beats::free_for_a_beat(self, b)
                {
                    continue;
                }
                let (a_id, b_id) = (a.id, b.id);
                let (a_at, b_at) = (a.state.position, b.state.position);
                let keen = f64::from(a.personality.sociability + b.personality.sociability) / 2.0;
                let nods = self
                    .village_life_rng
                    .random_bool((0.1 + keen * 0.4).clamp(0.0, 1.0));
                note(
                    &mut self.village_memory.nodded,
                    a_id,
                    b_id,
                    if nods {
                        NOD_AGAIN_SECS
                    } else {
                        NO_NOD_AGAIN_SECS
                    },
                );
                if nods {
                    let length = self.village_life_rng.random_range(NOD_SECS);
                    for (index, at) in [(walker, b_at), (other, a_at)] {
                        let creature = &mut self.save.creatures[index];
                        face(creature, at);
                        look(creature, BeatKind::Notice, length, at);
                    }
                }
                break;
            }
        }
    }
}
