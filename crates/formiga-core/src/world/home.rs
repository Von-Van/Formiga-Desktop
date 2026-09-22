use super::*;

pub(super) const HOME_DURATION: time::Duration = time::Duration::minutes(15);
const HOME_COOLDOWN: time::Duration = time::Duration::minutes(15);

/// The village spaces its residents with its own copy of the colony-wide spacing rule, because
/// `habitat` is below `world` and cannot see into it. The two must never drift apart.
const _: () = assert!(crate::REST_CLEAR_RATIO == spacing::FACE_CLEAR_RATIO);

/// How long a resident waits before the first quiet moment of a visit, and between later ones:
/// roughly one small thing each every couple of minutes, never two residents at once.
const FIRST_MOMENT_SECS: std::ops::Range<f32> = 12.0..80.0;
const NEXT_MOMENT_SECS: std::ops::Range<f32> = 60.0..180.0;
/// How far a resident will go for a belonging: two or three steps along its own stretch of the
/// strip. Anything further stays where it is.
const ERRAND_REACH_FRAMES: f32 = 1.5;
/// How long the walk to a belonging and back may take before it gives up and settles.
const ERRAND_WALK_SECS: f32 = 8.0;
/// The pause at the belonging itself, before turning round.
const ERRAND_PAUSE_SECS: f32 = 2.5;

/// What stage of a quiet moment a resident is in. Everything but an errand is one held clip.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MomentPhase {
    Hold,
    Away,
    Poke,
    Back,
}

/// Where a resident is strolling to while the colony is home, and how long it means to stay once
/// it arrives. Runtime-only, like every other thing a village does: a corner that moves underneath
/// the colony simply hands out new places.
#[derive(Clone, Copy, Debug)]
pub(super) struct RoamStep {
    target: Point,
    dwell: f32,
    leg: RoamLeg,
}

/// Which part of its afternoon a resident is in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RoamLeg {
    /// The walk home and the first stop, beside its own door.
    Arriving,
    /// At its own place on the commons, between strolls.
    Resting,
    /// Out on a stroll, or looking about at the far end of one.
    Out,
    /// Strolling back to its own place.
    Back,
}

impl RoamStep {
    /// Out on a stroll, there or back, rather than at home on its own place.
    fn strolling(&self) -> bool {
        matches!(self.leg, RoamLeg::Out | RoamLeg::Back)
    }
}

/// How long a resident stays at its own place between strolls, and how long it looks about at the
/// far end of one before heading back.
const STROLL_REST: std::ops::Range<f32> = 9.0..26.0;
const STROLL_PAUSE: std::ops::Range<f32> = 1.5..5.0;

/// The pause at the far end of a stroll that ends in front of somebody: long enough to turn round,
/// not long enough to stand on anybody's face.
const STROLL_PASSING_PAUSE: std::ops::Range<f32> = 0.4..1.0;

/// How long a resident waits before asking again when the commons already has its fill of people
/// out strolling.
const STROLL_WAIT: std::ops::Range<f32> = 2.0..6.0;

/// The most residents out strolling at once. The rest are at home on their own places, or doing
/// one of the quiet things residents do there.
pub(super) const MAX_STROLLING: usize = 3;

/// How fast a stroll goes, against a companion's ordinary walk. The walk cycle slows to match, so
/// the feet keep up with the ground rather than skating over it.
pub(super) const STROLL_PACE: f32 = 0.45;

/// How far a stroll goes at the least, in creature widths: far enough to read as going somewhere.
const STROLL_REACH_FRAMES: f32 = 1.5;

/// How many places a companion considers before settling for the best of a bad lot. Six is
/// enough that a busy commons still finds somewhere, and few enough to be free.
const ROAM_TRIES: usize = 6;

/// Where a resident goes next, and at what pace. The whole run of ground between the two trees is
/// the colony's, and a companion with nothing to do strolls it: from its own place out to
/// somewhere along it, a look about, and back again, then a rest before the next. Its own place is
/// kept for it while it is out, so the village always has somewhere clear for everyone to come back
/// to. At most `MAX_STROLLING` are out at once, and the rest wait their turn. The far end of a
/// stroll keeps clear of doorways and of whoever is standing there if it can, and where it cannot
/// it is only a place to turn round.
#[allow(clippy::too_many_arguments)]
fn roam_target(
    roam: &mut BTreeMap<CreatureId, RoamStep>,
    rng: &mut ChaCha12Rng,
    creature: &Creature,
    commons: HomeCommons,
    house: Option<(Point, f32)>,
    occupied: &[(CreatureId, Point)],
    doorways: &[(f32, f32)],
    settled: Point,
    dt: f32,
) -> (Point, f32) {
    let clear = CREATURE_FRAME_WIDTH * REST_CLEAR_RATIO * commons.scale;
    let pace = |leg: RoamLeg| {
        if matches!(leg, RoamLeg::Out | RoamLeg::Back) {
            STROLL_PACE
        } else {
            1.0
        }
    };
    let Some(step) = roam.get(&creature.id).copied() else {
        // The first place a companion goes when the houses appear is its own doorstep — or its
        // big version's, for a mini, which has no house of its own. Beside the door rather than
        // across it: the walk home ends where somebody lives, not in front of where they live.
        let first = house.map_or(settled, |(point, half)| {
            let toward = if commons.high_x - point.x >= point.x - commons.low_x {
                1.0
            } else {
                -1.0
            };
            let beside = point.x + toward * (half + RESTING_WIDTH / 2.0 * commons.scale);
            Point {
                x: beside.clamp(commons.low_x, commons.high_x),
                y: commons.ground_y,
            }
        });
        roam.insert(
            creature.id,
            RoamStep {
                target: first,
                dwell: rng.random_range(STROLL_REST),
                leg: RoamLeg::Arriving,
            },
        );
        return (first, 1.0);
    };
    // Still on the way there, or still where it meant to be for a while.
    if creature.state.position != step.target || step.dwell > 0.0 {
        if creature.state.position == step.target
            && let Some(step) = roam.get_mut(&creature.id)
        {
            step.dwell -= dt;
        }
        return (step.target, pace(step.leg));
    }
    let next = match step.leg {
        // Seen the far end: back to its own place, and a rest there.
        RoamLeg::Out => RoamStep {
            target: settled,
            dwell: rng.random_range(STROLL_REST),
            leg: RoamLeg::Back,
        },
        RoamLeg::Arriving | RoamLeg::Resting | RoamLeg::Back if step.target != settled => {
            RoamStep {
                target: settled,
                dwell: rng.random_range(STROLL_REST),
                leg: RoamLeg::Resting,
            }
        }
        _ => {
            let out = roam
                .iter()
                .filter(|(id, step)| **id != creature.id && step.strolling())
                .count();
            if out >= MAX_STROLLING {
                RoamStep {
                    target: settled,
                    dwell: rng.random_range(STROLL_WAIT),
                    leg: RoamLeg::Resting,
                }
            } else {
                stroll_to(
                    rng, creature, commons, occupied, roam, doorways, settled, clear,
                )
            }
        }
    };
    roam.insert(creature.id, next);
    (next.target, pace(next.leg))
}

/// Where the next stroll goes: somewhere along the commons at least a step or two from home,
/// clear of doorways and of anybody standing or headed there if such a place can be found.
#[allow(clippy::too_many_arguments)]
fn stroll_to(
    rng: &mut ChaCha12Rng,
    creature: &Creature,
    commons: HomeCommons,
    occupied: &[(CreatureId, Point)],
    roam: &BTreeMap<CreatureId, RoamStep>,
    doorways: &[(f32, f32)],
    settled: Point,
    clear: f32,
) -> RoamStep {
    let reach = CREATURE_FRAME_WIDTH * STROLL_REACH_FRAMES * commons.scale;
    let mut best: Option<(Point, u8)> = None;
    for _ in 0..ROAM_TRIES {
        let candidate = commons.along(rng.random_range(0.0..1.0));
        if (candidate.x - settled.x).abs() < reach {
            continue;
        }
        let crowded = occupied
            .iter()
            .any(|(id, point)| *id != creature.id && (point.x - candidate.x).abs() < clear)
            || roam.iter().any(|(id, step)| {
                *id != creature.id && (step.target.x - candidate.x).abs() < clear
            });
        let in_a_doorway = doorways
            .iter()
            .any(|(x, reach)| (x - candidate.x).abs() < *reach);
        let score = u8::from(!crowded) * 2 + u8::from(!in_a_doorway);
        if best.is_none_or(|(_, previous)| score > previous) {
            best = Some((candidate, score));
            if score == 3 {
                break;
            }
        }
    }
    match best {
        Some((target, score)) => RoamStep {
            target,
            dwell: rng.random_range(if score >= 2 {
                STROLL_PAUSE
            } else {
                STROLL_PASSING_PAUSE
            }),
            leg: RoamLeg::Out,
        },
        // A commons too short to go anywhere on: stay home a while and ask again.
        None => RoamStep {
            target: settled,
            dwell: rng.random_range(STROLL_WAIT),
            leg: RoamLeg::Resting,
        },
    }
}

/// A short, quiet thing a resident does at its own door while the home is out. It is a clip and
/// nothing else: no moment emits an `ActionCompleted`, so none of them teaches the creature a
/// tendency, fills a counter, moves a bond or writes a line in the journal.
#[derive(Clone, Copy, Debug)]
pub(super) struct HomeMoment {
    action: ActionKind,
    phase: MomentPhase,
    remaining: f32,
    /// The spot this resident returns to. A village that moves under it ends the moment.
    rest: Point,
    /// The belonging an errand is walking to.
    errand: Option<Point>,
    /// A neighbour waving back, or a moment the person at the desk asked for. Neither counts
    /// against "one resident busy at a time", and the scheduler never displaces either.
    courtesy: bool,
    /// What a walk to a hangout spot is for: done where the walk ends, instead of a poke at a
    /// belonging and a walk back.
    visit: Option<HangoutVisit>,
}

/// A quiet moment at one of the spots the person at the desk put down.
#[derive(Clone, Copy, Debug)]
struct HangoutVisit {
    action: ActionKind,
    seconds: f32,
    /// Which way to face once there: out over the desktop at a lookout, and otherwise however
    /// the walk left it.
    facing_right: Option<bool>,
}

/// The longest a walk across the village to a hangout spot may take before the companion gives
/// up on it and does its thing wherever it has got to.
const HANGOUT_WALK_SECS: f32 = 30.0;

/// Advances one resident's quiet moment. Returns the clip it should show and how much of the
/// moment is left, or `None` once it is over and the resident should stand at its door again.
fn advance_home_moment(
    moment: &mut HomeMoment,
    creature: &mut Creature,
    rest: Point,
    dt: f32,
) -> Option<(ActionKind, f32)> {
    // The ground moved under the village: another display, a narrowed habitat, a new scale.
    if moment.rest.distance(rest) > 0.5 {
        return None;
    }
    moment.remaining -= dt;
    match moment.phase {
        MomentPhase::Hold => {
            if moment.remaining <= 0.0 {
                return None;
            }
        }
        MomentPhase::Away | MomentPhase::Back => {
            let goal = if moment.phase == MomentPhase::Away {
                moment.errand.unwrap_or(rest)
            } else {
                rest
            };
            let start = creature.state.position;
            let distance = start.distance(goal);
            if distance > 0.0 {
                creature.state.facing_right = goal.x >= start.x;
                let speed = 22.0 + creature.personality.activity * 18.0;
                creature.state.position =
                    lerp_point(start, goal, (speed * dt / distance).clamp(0.0, 1.0));
            }
            if let Some(visit) = moment.visit
                && moment.phase == MomentPhase::Away
                && (creature.state.position == goal || moment.remaining <= 0.0)
            {
                // At the hangout spot, or as near as the walk got: its thing, done right here.
                moment.phase = MomentPhase::Hold;
                moment.action = visit.action;
                moment.remaining = visit.seconds;
                moment.rest = creature.state.position;
                moment.visit = None;
                if let Some(facing_right) = visit.facing_right {
                    creature.state.facing_right = facing_right;
                }
            } else if creature.state.position == goal || moment.remaining <= 0.0 {
                creature.state.position = goal;
                if moment.phase == MomentPhase::Away {
                    moment.phase = MomentPhase::Poke;
                    moment.remaining = ERRAND_PAUSE_SECS;
                    moment.action = ActionKind::Idle;
                } else {
                    return None;
                }
            }
        }
        MomentPhase::Poke => {
            if moment.remaining <= 0.0 {
                moment.phase = MomentPhase::Back;
                moment.remaining = ERRAND_WALK_SECS;
                moment.action = ActionKind::Traverse;
            }
        }
    }
    Some((moment.action, moment.remaining))
}

/// Picks one quiet moment for a resident standing at `rest`. Everything it can choose is a clip
/// the colony already has; the choice comes from the colony's own seeded stream, so the same
/// colony always fidgets the same way.
/// A hangout spot free for a quiet moment: what it is, where it stands, and whether the open
/// desktop lies to its right.
#[derive(Clone, Copy, Debug)]
struct FreeHangout {
    kind: HangoutKind,
    at: Point,
    open_right: bool,
}

#[allow(clippy::too_many_arguments)]
fn choose_home_moment(
    rng: &mut ChaCha12Rng,
    creature: &mut Creature,
    rest: Point,
    house: Option<(Point, f32)>,
    neighbour: Option<(CreatureId, Point)>,
    belongings: &[Point],
    hangouts: &[FreeHangout],
    frame: f32,
) -> (HomeMoment, Option<CreatureId>) {
    // Two or three steps for something close by, stopping beside it rather than on top of it,
    // and always back on the same ground line the village stands on.
    let errand = belongings
        .iter()
        .copied()
        .map(|point| ((point.x - rest.x).abs(), point))
        .filter(|(distance, _)| *distance > frame * 0.6 && *distance <= frame * ERRAND_REACH_FRAMES)
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, point)| Point {
            x: point.x - (point.x - rest.x).signum() * frame * 0.35,
            y: rest.y,
        });
    // Weighted so an ordinary visit is mostly nibbling and pottering; a nap or a wave is rarer
    // and worth noticing when it happens.
    let mut choices: Vec<(u8, ActionKind, f32, f32, Option<FreeHangout>)> = vec![
        (4, ActionKind::Eat, 6.0, 12.0, None),
        (4, ActionKind::Drink, 5.0, 10.0, None),
        (4, ActionKind::SoloPlay, 8.0, 16.0, None),
        (3, ActionKind::InspectScreen, 5.0, 9.0, None),
        (2, ActionKind::Sleep, 60.0, 150.0, None),
    ];
    if neighbour.is_some() {
        choices.push((3, ActionKind::Greet, 4.0, 7.0, None));
    }
    if errand.is_some() {
        choices.push((
            3,
            ActionKind::Traverse,
            ERRAND_WALK_SECS,
            ERRAND_WALK_SECS,
            None,
        ));
    }
    // A spot the person at the desk put down draws the moment it is for, a little more so for a
    // companion who feels like it: a sleepy one to the cushion, a hungry one to the blanket, a
    // curious one to the lookout. It is only ever one choice among the rest.
    let drives = &creature.state.drives;
    for hangout in hangouts.iter().copied() {
        let (keen, action, low, high) = match hangout.kind {
            HangoutKind::Cushion => (drives.sleep_pressure > 0.5, ActionKind::Sleep, 60.0, 150.0),
            HangoutKind::Blanket => (drives.energy < 0.45, ActionKind::Eat, 6.0, 12.0),
            HangoutKind::Lookout => (
                creature.personality.curiosity > 0.6,
                ActionKind::InspectScreen,
                5.0,
                9.0,
            ),
        };
        choices.push((2 + 2 * u8::from(keen), action, low, high, Some(hangout)));
    }
    let total: u32 = choices.iter().map(|(weight, ..)| u32::from(*weight)).sum();
    let mut roll = rng.random_range(0..total);
    let (_, action, low, high, hangout) = choices
        .iter()
        .copied()
        .find(|(weight, ..)| {
            let hit = roll < u32::from(*weight);
            roll = roll.saturating_sub(u32::from(*weight));
            hit
        })
        .unwrap_or((1, ActionKind::Eat, 6.0, 12.0, None));
    let seconds = if low < high {
        rng.random_range(low..high)
    } else {
        low
    };

    if let Some(hangout) = hangout {
        // A blanket is for a drink as much as a snack.
        let action = if hangout.kind == HangoutKind::Blanket && rng.random_bool(0.4) {
            ActionKind::Drink
        } else {
            action
        };
        // On the cushion or the blanket; beside the lookout, looking out past it.
        let stand = match hangout.kind {
            HangoutKind::Lookout => Point {
                x: hangout.at.x + if hangout.open_right { -0.45 } else { 0.45 } * frame,
                y: rest.y,
            },
            _ => Point {
                x: hangout.at.x,
                y: rest.y,
            },
        };
        let speed = 22.0 + creature.personality.activity * 18.0;
        let walk = (stand.x - creature.state.position.x).abs() / speed + 2.0;
        return (
            HomeMoment {
                action: ActionKind::Traverse,
                phase: MomentPhase::Away,
                remaining: walk.min(HANGOUT_WALK_SECS),
                rest,
                errand: Some(stand),
                courtesy: false,
                visit: Some(HangoutVisit {
                    action,
                    seconds,
                    facing_right: (hangout.kind == HangoutKind::Lookout)
                        .then_some(hangout.open_right),
                }),
            },
            None,
        );
    }

    let mut answering = None;
    match action {
        ActionKind::Greet => {
            if let Some((id, point)) = neighbour {
                creature.state.facing_right = point.x >= rest.x;
                answering = Some(id);
            }
        }
        ActionKind::InspectScreen => {
            if let Some((point, _)) = house {
                creature.state.facing_right = point.x >= rest.x;
            }
        }
        _ => {}
    }
    let walking = action == ActionKind::Traverse;
    (
        HomeMoment {
            action,
            phase: if walking {
                MomentPhase::Away
            } else {
                MomentPhase::Hold
            },
            remaining: seconds,
            rest,
            errand: walking.then_some(errand).flatten(),
            courtesy: false,
            visit: None,
        },
        answering,
    )
}

impl World {
    pub(super) fn update_home_cycle(&mut self, desktop: &DesktopSnapshot) -> bool {
        let timeline_now = self.save.maximum_seen_utc;
        let ritual_shelter = self
            .colony_plan
            .as_ref()
            .is_some_and(|plan| plan.kind == RitualKind::ShelterGathering);
        if !ritual_shelter
            && self.village_moment.is_none()
            && self.save.companion.quiet_until.is_none()
            && self
                .save
                .home
                .active_since_utc
                .is_some_and(|started| timeline_now - started >= HOME_DURATION)
        {
            self.dismiss_home(timeline_now, false);
        }

        let due = !self.save.home.is_active()
            && self.colony_plan.is_none()
            && self
                .save
                .home
                .last_disappeared_utc
                .is_none_or(|ended| timeline_now - ended >= HOME_COOLDOWN);
        if due && self.interaction.is_none() && self.resolve_home_monitor(desktop).is_some() {
            self.begin_home_visit(timeline_now);
        }

        if ritual_shelter {
            return false;
        }
        // Runs before the paused and hidden branches of the tick take their early exit, so a
        // colony that is put on hold or tucked away settles out of whatever it was doing.
        if !self.save.home.is_active() || self.save.settings.paused || !self.save.settings.visible {
            self.cancel_home_moments();
            self.end_village_moment(false);
        }
        self.save.home.is_active()
    }

    /// The home appears and everyone sets off for it. Shared by the ordinary cycle and by a
    /// gathering the person at the desk asked for.
    fn begin_home_visit(&mut self, now: OffsetDateTime) {
        self.save.home.active_since_utc = Some(now);
        self.home_moments.clear();
        self.home_moment_timers.clear();
        self.home_roam.clear();
        self.clear_runtime_plans();
        Self::emit(&mut self.events, WorldEvent::HomeAppeared);
        self.visitor_home_appeared(now);
    }

    /// Call the whole colony home now, without waiting out the cooldown. It is an ordinary
    /// gathering in every other respect: the same length, the same cooldown afterwards, and the
    /// same ways of ending early.
    pub(super) fn send_home(&mut self, desktop: &DesktopSnapshot) -> bool {
        if self.save.home.is_active() {
            return true;
        }
        if self.save.settings.paused
            || self.interaction.is_some()
            || self.resolve_home_monitor(desktop).is_none()
        {
            return false;
        }
        let now = self.save.maximum_seen_utc;
        self.interrupt_colony_plan(now);
        self.begin_home_visit(now);
        let heading_home: Vec<_> = self
            .save
            .creatures
            .iter()
            .filter(|creature| creature.state.arrival_delay_secs <= 0.0)
            .map(|creature| creature.id)
            .collect();
        for creature_id in heading_home {
            self.show_bubble(creature_id, BubbleIcon::Home);
        }
        true
    }

    fn resolve_home_monitor(&mut self, desktop: &DesktopSnapshot) -> Option<MonitorInfo> {
        let preferred = self.save.home.display;
        let monitor = preferred
            .and_then(|display| {
                desktop.monitors.iter().find(|monitor| {
                    monitor.display_key == display
                        && !accessible_regions(&self.save.settings.habitat, monitor).is_empty()
                })
            })
            .or_else(|| {
                desktop.monitors.iter().find(|monitor| {
                    monitor.primary
                        && !accessible_regions(&self.save.settings.habitat, monitor).is_empty()
                })
            })
            .or_else(|| {
                desktop.monitors.iter().find(|monitor| {
                    !accessible_regions(&self.save.settings.habitat, monitor).is_empty()
                })
            })?
            .clone();
        self.save.home.display = Some(monitor.display_key);
        Some(monitor)
    }

    pub(super) fn tick_homebound_creatures(
        &mut self,
        now: OffsetDateTime,
        dt: f32,
        desktop: &DesktopSnapshot,
    ) {
        let Some(monitor) = self.resolve_home_monitor(desktop) else {
            return;
        };
        let Some(anchor) = resolved_home_anchor(
            &self.save.home,
            &monitor,
            self.save.settings.display_scale,
            &self.save.settings.habitat,
        ) else {
            return;
        };
        let inward = match self.save.home.corner {
            HomeCorner::BottomLeft => 1.0,
            HomeCorner::BottomRight => -1.0,
        };
        // Where the colony settles when it is standing still, laid out by the same walk that
        // places the houses and the belongings, so no two faces end up behind one another.
        let (resting, houses) = self.village_places(desktop);
        // The ground between the two trees, and the doorways on it worth not standing in.
        let cottage_list = colony_cottage_list(&self.save.creatures);
        let commons = home_commons(
            &self.save.home,
            cottage_list.as_slice(),
            &desktop.monitors,
            &self.save.settings.habitat,
            self.save.settings.display_scale,
        );
        let mut doorways = Vec::with_capacity(cottage_list.as_slice().len() + 1);
        if let Some(commons) = commons {
            for slot in 0..=cottage_list.as_slice().len() {
                let Some((_, point)) = home_dwelling_position(
                    &self.save.home,
                    slot,
                    cottage_list.as_slice(),
                    &desktop.monitors,
                    &self.save.settings.habitat,
                    self.save.settings.display_scale,
                ) else {
                    continue;
                };
                // Standing this close to a house's middle puts a frame across its door.
                doorways.push((point.x, CREATURE_FRAME_WIDTH / 2.0 * commons.scale));
            }
        }
        let standing: Vec<(CreatureId, Point)> = self
            .save
            .creatures
            .iter()
            .map(|creature| (creature.id, creature.state.position))
            .collect();
        let belongings = self.village_belongings(desktop);
        let frame = spacing::frame_width(self.save.settings.display_scale, monitor.scale_factor);
        // Where each spot the person at the desk put down stands, and which way the open desktop
        // lies from it, on the display the village is on.
        let middle = monitor.usable_bounds.x + monitor.usable_bounds.width / 2.0;
        let hangouts: Vec<FreeHangout> = home_hangout_positions(
            &self.save.home,
            cottage_list.as_slice(),
            &desktop.monitors,
            &self.save.settings.habitat,
            self.save.settings.display_scale,
        )
        .into_iter()
        .filter(|(_, monitor_id, _)| *monitor_id == monitor.id)
        .map(|(kind, _, at)| FreeHangout {
            kind,
            at,
            open_right: middle >= at.x,
        })
        .collect();
        // Passive moments are decoration. They stop for a still desktop and for a hidden one.
        let quiet = self.save.settings.reduce_motion || !self.save.settings.visible;
        let reduce_motion = self.save.settings.reduce_motion;
        let mut occupied = self.home_moments.values().any(|moment| !moment.courtesy);
        let settled: Vec<(CreatureId, Point)> = self
            .save
            .creatures
            .iter()
            .filter(|creature| {
                creature.state.arrival_delay_secs <= 0.0
                    && creature.state.action == ActionKind::Homebound
                    && !self.home_moments.contains_key(&creature.id)
            })
            .filter_map(|creature| Some((creature.id, *resting.get(&creature.id)?)))
            .collect();
        let mut answering: Vec<(CreatureId, Point, f32)> = Vec::new();
        for creature in &mut self.save.creatures {
            if self
                .interaction
                .as_ref()
                .is_some_and(|interaction| interaction.creature_id == creature.id)
            {
                self.home_moments.remove(&creature.id);
                continue;
            }
            if creature.state.arrival_delay_secs > 0.0 {
                creature.state.arrival_delay_secs =
                    (creature.state.arrival_delay_secs - dt).max(0.0);
                if creature.state.arrival_delay_secs == 0.0 {
                    creature.born_at_utc = now;
                    Self::emit(
                        &mut self.events,
                        WorldEvent::CreatureSpawned {
                            creature_id: creature.id,
                        },
                    );
                }
                continue;
            }
            creature.state.action_elapsed += dt;
            if creature.state.action == ActionKind::PetReaction
                && creature.state.action_elapsed < creature.state.action_duration
            {
                self.home_moments.remove(&creature.id);
                continue;
            }
            let anchored = resting.get(&creature.id).copied().unwrap_or(anchor);
            // Reduced motion keeps the colony where it stands, and so does a hidden one — there
            // is no sense walking a village nobody can see. Otherwise everyone has the run of the
            // ground between the two trees.
            // A moment in progress owns the creature's feet: it is doing its small thing where it
            // was asked, not on the way to somewhere else.
            // A companion in a moment the village was asked to share has its own place in the
            // line, and nothing else to do at its door until the moment is over.
            let shared = self
                .village_moment
                .as_ref()
                .and_then(|plan| plan.place(creature.id));
            if shared.is_some() {
                self.home_moments.remove(&creature.id);
            }
            let (target, pace) = if let Some(place) = shared {
                (place.spot, 1.0)
            } else if let Some(moment) = self.home_moments.get(&creature.id) {
                (moment.rest, 1.0)
            } else {
                match commons.filter(|_| !quiet) {
                    Some(commons) => roam_target(
                        &mut self.home_roam,
                        &mut self.home_roam_rng,
                        creature,
                        commons,
                        houses.get(&creature.id).copied(),
                        &standing,
                        &doorways,
                        anchored,
                        dt,
                    ),
                    None => (anchored, 1.0),
                }
            };

            // A quiet moment in progress holds the creature where it is, or walks the two or
            // three steps of an errand. Anything that moves the village ends it at once.
            let held = self
                .home_moments
                .get_mut(&creature.id)
                .and_then(|moment| advance_home_moment(moment, creature, target, dt));
            if let Some((action, remaining)) = held {
                if creature.state.action != action {
                    creature.state.action = action;
                    creature.state.action_elapsed = 0.0;
                    creature.state.action_duration = remaining.max(dt);
                    // Its own little ways come home with it, but a moment at the door teaches
                    // nothing, so nothing new is picked up here.
                    cue_habit(
                        creature,
                        action,
                        &mut self.habit_rng,
                        false,
                        reduce_motion,
                        &mut self.events,
                    );
                    Self::emit(
                        &mut self.events,
                        WorldEvent::ActionStarted {
                            creature_id: creature.id,
                            action,
                        },
                    );
                }
                creature.state.velocity = Point::default();
                creature.state.drives.comfort =
                    (creature.state.drives.comfort + dt * 0.01).min(1.0);
                creature.state.drives.arousal =
                    (creature.state.drives.arousal - dt * 0.04).max(0.0);
                continue;
            }
            self.home_moments.remove(&creature.id);

            // Come down from a ledge before walking home. Reuse the existing journey so a house
            // dismissal in mid-descent still finishes the landing through the ordinary tick path.
            if !self.window_journeys.contains_key(&creature.id)
                && (creature.state.surface.kind == SurfaceKind::WindowLedge
                    || creature.state.action != ActionKind::Traverse)
                && let Some((monitor_id, floor)) = nearest_habitat_point(
                    &self.save.settings.habitat,
                    &desktop.monitors,
                    creature.state.position,
                )
                && (floor.y - creature.state.position.y).abs() > 0.5
            {
                self.window_journeys.insert(
                    creature.id,
                    WindowJourney::Hop(HopJourney {
                        start: creature.state.position,
                        target: floor,
                        surface: SurfaceAttachment {
                            kind: SurfaceKind::ScreenFloor,
                            monitor_id,
                            window_key: None,
                            relative_x: 0.5,
                        },
                        elapsed: 0.0,
                        duration: (creature.state.position.distance(floor) / 180.0).max(0.1),
                    }),
                );
            }

            let previous = creature.state.position;
            let mut next_action = if let Some(journey) = self.window_journeys.get_mut(&creature.id)
            {
                if !journey.valid(desktop) {
                    self.window_journeys.remove(&creature.id);
                    settle_interrupted_journey(
                        creature,
                        desktop,
                        &self.save.settings.habitat,
                        &mut self.events,
                    );
                    continue;
                }
                let step = journey.advance(dt);
                creature.state.position = step.position;
                if step.complete {
                    creature.state.surface = journey.surface().clone();
                    self.window_journeys.remove(&creature.id);
                }
                ActionKind::Landing
            } else {
                let distance = previous.distance(target);
                let speed = (24.0 + creature.personality.activity * 34.0) * pace;
                if distance > 0.0 {
                    creature.state.facing_right = target.x >= previous.x;
                    creature.state.position =
                        lerp_point(previous, target, (speed * dt / distance).clamp(0.0, 1.0));
                    // A stroll's steps are slower as well as shorter, so its feet keep pace with
                    // the ground: the walk cycle runs at the stroll's own pace.
                    if pace < 1.0 && creature.state.action == ActionKind::Traverse {
                        creature.state.action_elapsed -= dt * (1.0 - pace);
                    }
                }
                if creature.state.position == target
                    && let Some(place) = shared
                    && let Some(plan) = &self.village_moment
                {
                    // In its place: turned to the middle of the line, waiting for the others and
                    // then doing what the moment is for.
                    if (plan.centre.x - target.x).abs() > 0.5 {
                        creature.state.facing_right = plan.centre.x > target.x;
                    }
                    if plan.together {
                        place.action
                    } else {
                        ActionKind::Homebound
                    }
                } else if creature.state.position == target {
                    creature.state.facing_right = inward > 0.0;
                    match self.home_moments.get_mut(&creature.id) {
                        Some(moment) if moment.remaining > 0.0 => {
                            moment.remaining -= dt;
                            moment.action
                        }
                        _ => {
                            self.home_moments.remove(&creature.id);
                            ActionKind::Homebound
                        }
                    }
                } else {
                    ActionKind::Traverse
                }
            };
            // A disconnected/excluded part of the habitat is not a shortcut home. Wait at its
            // boundary; if the habitat itself changed underneath us, use normal support recovery.
            if !desktop.monitors.iter().any(|current| {
                habitat_contains(
                    &self.save.settings.habitat,
                    current,
                    creature.state.position,
                )
            }) {
                creature.state.position = previous;
                self.window_journeys.remove(&creature.id);
                if !desktop
                    .monitors
                    .iter()
                    .any(|current| habitat_contains(&self.save.settings.habitat, current, previous))
                {
                    settle_interrupted_journey(
                        creature,
                        desktop,
                        &self.save.settings.habitat,
                        &mut self.events,
                    );
                    continue;
                }
                next_action = ActionKind::Idle;
            }
            creature.state.velocity = if dt > 0.0 && next_action != ActionKind::Homebound {
                Point {
                    x: (creature.state.position.x - previous.x) / dt,
                    y: (creature.state.position.y - previous.y) / dt,
                }
            } else {
                Point::default()
            };
            // Keep rendering on the source display until the creature actually crosses the seam.
            if let Some(current) = desktop
                .monitors
                .iter()
                .find(|current| current.bounds.contains(creature.state.position))
            {
                creature.state.surface = SurfaceAttachment {
                    kind: SurfaceKind::ScreenFloor,
                    monitor_id: current.id,
                    window_key: None,
                    relative_x: ((creature.state.position.x - current.usable_bounds.x)
                        / current.usable_bounds.width)
                        .clamp(0.0, 1.0),
                };
            }
            if creature.state.action != next_action {
                creature.state.action = next_action;
                creature.state.action_elapsed = 0.0;
                creature.state.action_duration = HOME_DURATION.whole_seconds() as f32;
                Self::emit(
                    &mut self.events,
                    WorldEvent::ActionStarted {
                        creature_id: creature.id,
                        action: next_action,
                    },
                );
                // A moment shared because it was asked for is a real one: done the companion's
                // own way, and somewhere a habit can be picked up.
                if shared.is_some_and(|place| place.action == next_action) {
                    cue_habit(
                        creature,
                        next_action,
                        &mut self.habit_rng,
                        true,
                        reduce_motion,
                        &mut self.events,
                    );
                }
            }
            if let Some(place) = shared
                && next_action == place.action
            {
                creature.state.attention =
                    self.village_moment.as_ref().and_then(|plan| plan.pose());
            }
            creature.state.drives.comfort = (creature.state.drives.comfort + dt * 0.01).min(1.0);
            creature.state.drives.arousal = (creature.state.drives.arousal - dt * 0.04).max(0.0);

            // Standing at its own door with time on its hands: every so often, one small thing.
            // Waiting for the rest of a shared moment to gather is not time on its hands.
            if next_action != ActionKind::Homebound || shared.is_some() {
                self.home_moment_timers.remove(&creature.id);
                continue;
            }
            let due = {
                let remaining = self
                    .home_moment_timers
                    .entry(creature.id)
                    .or_insert_with(|| self.home_moment_rng.random_range(FIRST_MOMENT_SECS));
                *remaining -= dt;
                *remaining <= 0.0
            };
            if !due || quiet || occupied {
                continue;
            }
            let neighbour = settled
                .iter()
                .copied()
                .filter(|(id, _)| *id != creature.id)
                .map(|(id, point)| (target.distance(point), id, point))
                .filter(|(distance, ..)| *distance <= frame * 4.0)
                .min_by(|a, b| a.0.total_cmp(&b.0))
                .map(|(_, id, point)| (id, point));
            // The spots nobody else is standing on right now.
            let free: Vec<FreeHangout> = hangouts
                .iter()
                .copied()
                .filter(|hangout| {
                    !standing.iter().any(|(id, point)| {
                        *id != creature.id && (point.x - hangout.at.x).abs() < frame * 0.6
                    })
                })
                .collect();
            let (moment, answered) = choose_home_moment(
                &mut self.home_moment_rng,
                creature,
                target,
                houses.get(&creature.id).copied(),
                neighbour,
                &belongings,
                &free,
                frame,
            );
            let length = moment.remaining;
            self.home_moments.insert(creature.id, moment);
            self.home_moment_timers.insert(
                creature.id,
                length + self.home_moment_rng.random_range(NEXT_MOMENT_SECS),
            );
            occupied = true;
            if let Some(id) = answered
                && let Some(point) = resting.get(&id).copied()
            {
                answering.push((id, point, length));
            }
        }

        // A neighbour waves back. A courtesy moment never counts against the one-at-a-time rule
        // and is never chosen over: it only ever answers something already happening.
        for (creature_id, rest, seconds) in answering {
            if self.home_moments.contains_key(&creature_id) {
                continue;
            }
            self.home_moments.insert(
                creature_id,
                HomeMoment {
                    action: ActionKind::Greet,
                    phase: MomentPhase::Hold,
                    remaining: seconds,
                    rest,
                    errand: None,
                    courtesy: true,
                    visit: None,
                },
            );
        }
    }

    /// Whether this companion is waiting out a home visit at its own door: standing still there,
    /// or in the middle of one of the small quiet things residents do while the home is out.
    pub fn resting_at_home(&self, creature_id: CreatureId) -> bool {
        self.save.home.is_active()
            && (self.home_moments.contains_key(&creature_id)
                || self.save.creatures.iter().any(|creature| {
                    creature.id == creature_id && creature.state.action == ActionKind::Homebound
                }))
    }

    /// Where every member of the colony waits out a visit, and where its own house stands, in
    /// the stable colony order the village is laid out in.
    pub(super) fn village_places(
        &self,
        desktop: &DesktopSnapshot,
    ) -> (
        BTreeMap<CreatureId, Point>,
        BTreeMap<CreatureId, (Point, f32)>,
    ) {
        let cottages = colony_cottage_list(&self.save.creatures);
        let cottages = cottages.as_slice();
        let scale_factor = desktop
            .monitors
            .iter()
            .find(|monitor| Some(monitor.display_key) == self.save.home.display)
            .or_else(|| desktop.monitors.first())
            .map_or(1.0, |monitor| monitor.scale_factor.max(1.0));
        let mut order: Vec<_> = self
            .save
            .creatures
            .iter()
            .map(|creature| (creature.colony_order, creature.id))
            .collect();
        order.sort_unstable();
        let mut resting = BTreeMap::new();
        let mut houses = BTreeMap::new();
        let residents = order.len().max(1);
        for (slot, (_, creature_id)) in order.into_iter().enumerate() {
            if let Some((_, point)) = home_resting_position(
                &self.save.home,
                slot,
                residents,
                cottages,
                &desktop.monitors,
                &self.save.settings.habitat,
                self.save.settings.display_scale,
            ) {
                resting.insert(creature_id, point);
            }
            // A mini has no house of its own: the one it comes home to is its big version's.
            let house = self
                .save
                .creatures
                .iter()
                .find(|creature| creature.id == creature_id)
                .map(|creature| {
                    house_slot_for(
                        creature,
                        &self.save.creatures,
                        &self.save.home.cottage_order,
                    )
                })
                .unwrap_or(slot);
            if let Some((_, point)) = home_dwelling_position(
                &self.save.home,
                house,
                cottages,
                &desktop.monitors,
                &self.save.settings.habitat,
                self.save.settings.display_scale,
            ) {
                let kind = if house == 0 {
                    DwellingKind::Main
                } else {
                    DwellingKind::Cottage
                };
                let unit = f32::from(self.save.settings.display_scale) / scale_factor;
                houses.insert(creature_id, (point, kind.width() / 2.0 * unit));
            }
        }
        (resting, houses)
    }

    /// The belongings that are actually on the strip right now, for a two-step errand.
    fn village_belongings(&self, desktop: &DesktopSnapshot) -> Vec<Point> {
        let cottages = colony_cottage_list(&self.save.creatures);
        let places = home_object_positions(
            &self.save.home,
            cottages.as_slice(),
            &desktop.monitors,
            &self.save.settings.habitat,
            self.save.settings.display_scale,
        );
        places
            .into_iter()
            .take(self.save.objects.objects.len().min(MAX_COLONY_OBJECTS))
            .flatten()
            .map(|(_, point)| point)
            .collect()
    }

    /// Let one resident do a short thing where it rests — nibble, play, doze — and then settle
    /// again. Refused unless the home is out and the creature has already arrived at its spot.
    // The seam the offers work calls from `world/offers.rs`; nothing on this branch does yet.
    pub(super) fn begin_home_moment(
        &mut self,
        creature_id: CreatureId,
        action: ActionKind,
        seconds: f32,
    ) -> bool {
        if !self.save.home.is_active()
            || self
                .interaction
                .as_ref()
                .is_some_and(|interaction| interaction.creature_id == creature_id)
        {
            return false;
        }
        let resting = self.home_moments.get(&creature_id).copied();
        let Some(creature) = self
            .save
            .creatures
            .iter()
            .find(|creature| creature.id == creature_id)
        else {
            return false;
        };
        // Home and on its own feet. A companion strolling the commons stops where it is and takes
        // what is held out, rather than having to be caught standing still: the walk between two
        // places is most of an afternoon now, and an offer refused because somebody was mid-stroll
        // would read as the offer being broken.
        if creature.state.arrival_delay_secs > 0.0
            || self.window_journeys.contains_key(&creature_id)
            || !matches!(
                creature.state.action,
                ActionKind::Homebound | ActionKind::Traverse | ActionKind::Idle
            ) && resting.is_none()
        {
            return false;
        }
        // An offer outranks an idle fidget, and the scheduler leaves a requested moment alone.
        let rest = resting.map_or(creature.state.position, |moment| moment.rest);
        // Taking what is held out means stepping out of a shared moment, which carries on without
        // it for as long as there are still two.
        if let Some(plan) = &mut self.village_moment {
            plan.places.retain(|place| place.creature_id != creature_id);
        }
        self.home_moments.insert(
            creature_id,
            HomeMoment {
                action,
                phase: MomentPhase::Hold,
                remaining: seconds.max(0.0),
                rest,
                errand: None,
                courtesy: true,
                visit: None,
            },
        );
        let wait = seconds.max(0.0) + self.home_moment_rng.random_range(NEXT_MOMENT_SECS);
        self.home_moment_timers.insert(creature_id, wait);
        true
    }

    /// Ends every quiet moment and settles whoever was in one back into the calm resting pose.
    /// A visit that is over, a paused or hidden colony, and a companion picked up all use this.
    pub(super) fn cancel_home_moments(&mut self) {
        if self.home_moments.is_empty() {
            return;
        }
        let ended: Vec<CreatureId> = self.home_moments.keys().copied().collect();
        self.home_moments.clear();
        for creature in &mut self.save.creatures {
            if !ended.contains(&creature.id) {
                continue;
            }
            creature.state.action = ActionKind::Homebound;
            creature.state.action_elapsed = 0.0;
            creature.state.action_duration = HOME_DURATION.whole_seconds() as f32;
            creature.state.velocity = Point::default();
        }
    }

    pub(super) fn dismiss_home(&mut self, now: OffsetDateTime, interrupted: bool) {
        if interrupted {
            self.save.companion.quiet_until = None;
        }
        if !self.save.home.is_active() {
            return;
        }
        self.save.home.active_since_utc = None;
        self.save.home.last_disappeared_utc = Some(now);
        self.end_village_moment(false);
        self.cancel_home_moments();
        self.home_moment_timers.clear();
        // Every visit starts with the walk home: nobody picks up a stroll where the last one left
        // off.
        self.home_roam.clear();
        self.visitor_home_disappeared(now);
        for creature in &mut self.save.creatures {
            if matches!(
                creature.state.action,
                ActionKind::Homebound | ActionKind::Traverse
            ) {
                creature.state.action = ActionKind::Idle;
                creature.state.action_elapsed = 0.0;
                creature.state.action_duration = 2.5;
                creature.state.velocity = Point::default();
            }
            if creature.state.arrival_delay_secs <= 0.0 {
                self.pending_home_greetings.insert(creature.id);
            }
        }
        Self::emit(
            &mut self.events,
            WorldEvent::HomeDisappeared { interrupted },
        );
    }
}
