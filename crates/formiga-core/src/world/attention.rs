//! Short actor/observer sequences, coordinated before ordinary behavior selection.
use super::*;
use crate::attention::{GeometryChange, GeometrySignal};
mod assistance;
mod cursor;
mod dares;
mod displays;
mod games;
mod gaps;
mod geometry_comedy;
mod geometry_games;
mod hesitation;
mod ledges;
mod motion;
mod play;
mod rides;
use super::rides::{RideKind, RidePose};
pub(super) use gaps::gap_step_safe;
mod presentation;
mod spectacle;
pub(super) use displays::DisplayAttention;
use displays::DisplayWalk;
use motion::{MAX_APPROACH_SECONDS, ShortWalk};
use presentation::*;
use spectacle::{Cue, Outcome, Stage};

/// The most creatures one scene gathers: the riders reacting to one window, the reactions
/// running at once, or a play session's members. Scenes stay this small on purpose. Anything that
/// remembers something about each resident — setbacks, refused invitations, supports, display
/// moves, rides — is sized by the colony instead, so the fifth and sixth companions are noticed
/// and remembered exactly like the first four.
const MAX_PARTICIPANTS: usize = 4;
const REACTION_SECONDS: f32 = 3.2;
const REACTION_COOLDOWN: f32 = 12.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Origin {
    Window(u64),
    Cursor(u64),
    Display(u64),
    Surface(u64),
}

#[derive(Clone, Copy, Debug)]
enum Role {
    Actor {
        window: WindowKey,
        vanished: bool,
        /// Another rider sharing this window, watched instead of the desktop behind it.
        companion: Option<CreatureId>,
    },
    Observer {
        actor: CreatureId,
    },
    Cursor {
        investigate: bool,
        anchor: Point,
        /// Keeping pace with a cursor that rushed past, for a moment.
        racing: bool,
    },
    Display {
        discovery: bool,
    },
    Ledge {
        window: WindowKey,
        bounds: DesktopRect,
        drop: f32,
        resting: bool,
        declined: bool,
        commute: bool,
    },
    /// `None` targets are habitat floor. An escape is startled throughout and never rewarding.
    Journey {
        target_window: Option<WindowKey>,
        target_bounds: Option<DesktopRect>,
        stage: Stage,
        hanging: f32,
        rewarding: bool,
        escape: bool,
        /// Plan time at which `stage` began.
        since: f32,
        /// The attempt ended hanging from the far edge and had to pull itself up.
        caught: bool,
    },
    /// A borderline gap: look, back up, come forward, reconsider; then commit or chicken out.
    Hesitate {
        source: WindowKey,
        source_bounds: DesktopRect,
        target_window: WindowKey,
        target_bounds: DesktopRect,
        edge: Point,
        runup: Point,
        landing: Point,
        drop: f32,
        attempt: u8,
        phase: HesitatePhase,
        phase_elapsed: f32,
        /// Decided privately at the start: one more try before the final choice, and that choice.
        retry: bool,
        commit: bool,
    },
    Helper {
        actor: CreatureId,
        window: WindowKey,
        bounds: DesktopRect,
    },
    Tumble {
        stage: Stage,
    },
    Play {
        stage: Stage,
        gesture: ActionKind,
        bounds: Option<DesktopRect>,
        /// A leap in progress: the journey owns contact and action until it lands.
        hopping: bool,
        /// A body pose the game wants struck over `gesture` this tick. Games restate it every
        /// tick, and presentation still decides whether the body is free to show it.
        pose: Option<Gesture>,
    },
    /// Invited to try the gap a companion just cleared: walk to the edge, then answer.
    Dare {
        source: WindowKey,
        source_bounds: DesktopRect,
        target_window: WindowKey,
        target_bounds: DesktopRect,
        landing: Point,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HesitatePhase {
    Look,
    BackUp,
    Approach,
    Reconsider,
    Retreat,
}

/// A recent slip, fall, interrupted attempt, or chicken-out. Runtime-only and never saved.
#[derive(Clone, Copy, Debug)]
struct Setback {
    creature: CreatureId,
    remaining: f32,
    count: u8,
    route: Option<(WindowKey, WindowKey)>,
}

const SETBACK_SECONDS: f32 = 90.0;

#[derive(Clone, Copy, Debug)]
struct Reaction {
    origin: Origin,
    role: Role,
    target: Point,
    emotion: AttentionEmotion,
    elapsed: f32,
    seconds: f32,
    delay: f32,
    travel_elapsed: f32,
    walk: Option<ShortWalk>,
    display_walk: Option<DisplayWalk>,
    surface: (MonitorId, Option<WindowKey>),
    action: ActionKind,
    cue: Option<Cue>,
    ride: Option<RidePose>,
}

#[derive(Default)]
pub(super) struct AttentionRuntime {
    plans: BTreeMap<CreatureId, Reaction>,
    cooldowns: BTreeMap<CreatureId, f32>,
    colony_cooldown: f32,
    // Last actual support, retained briefly through attachment recovery; never saved.
    recent_supports: BTreeMap<CreatureId, (WindowKey, f32, Option<u64>)>,
    play: play::PlayRuntime,
    setbacks: [Option<Setback>; MAX_COLONY_CREATURES],
}

impl AttentionRuntime {
    /// How much of the moment is being held onto: live plans, and creatures still cooling down.
    /// Both are keyed by creature, so both are bounded by the colony; this is how that is checked.
    #[cfg(test)]
    pub(super) fn held(&self) -> (usize, usize) {
        (self.plans.len(), self.cooldowns.len())
    }

    /// Remember a failure for risk decisions. One record per creature; the oldest is replaced.
    fn record_setback(&mut self, creature: CreatureId, route: Option<(WindowKey, WindowKey)>) {
        let slot = self
            .setbacks
            .iter()
            .position(|s| s.is_some_and(|s| s.creature == creature))
            .or_else(|| self.setbacks.iter().position(Option::is_none))
            .unwrap_or_else(|| {
                (0..MAX_COLONY_CREATURES)
                    .min_by(|&a, &b| {
                        let remaining = |i: usize| self.setbacks[i].map_or(0.0, |s| s.remaining);
                        remaining(a).total_cmp(&remaining(b))
                    })
                    .unwrap()
            });
        let count = self.setbacks[slot]
            .filter(|s| s.creature == creature)
            .map_or(0, |s| s.count);
        self.setbacks[slot] = Some(Setback {
            creature,
            remaining: SETBACK_SECONDS,
            count: (count + 1).min(3),
            route: route.or(self.setbacks[slot]
                .filter(|s| s.creature == creature)
                .and_then(|s| s.route)),
        });
    }

    fn setback(&self, creature: CreatureId) -> Option<Setback> {
        self.setbacks
            .iter()
            .flatten()
            .find(|s| s.creature == creature)
            .copied()
    }

    fn decay_setbacks(&mut self, dt: f32) {
        for slot in &mut self.setbacks {
            if let Some(s) = slot {
                s.remaining -= dt;
                if s.remaining <= 0.0 {
                    *slot = None;
                }
            }
        }
    }
}

impl AttentionRuntime {
    pub fn owns(&self, id: CreatureId) -> bool {
        self.plans.get(&id).is_some_and(|p| {
            !matches!(
                p.role,
                Role::Journey {
                    stage: Stage::Notice | Stage::Prepare | Stage::Act | Stage::Catch,
                    ..
                } | Role::Play { hopping: true, .. }
            )
        })
    }
    pub fn crosses_displays(&self, id: CreatureId) -> bool {
        self.plans
            .get(&id)
            .is_some_and(|plan| plan.display_walk.is_some())
    }
    pub fn crossing_ids(&self) -> impl Iterator<Item = CreatureId> + '_ {
        self.plans
            .iter()
            .filter_map(|(&id, plan)| plan.display_walk.is_some().then_some(id))
    }

    /// Off the ground, with the leap or crossing owning contact until it lands. Overlap handling
    /// leaves these alone: nothing can step aside mid-air.
    pub(super) fn airborne(&self, id: CreatureId) -> bool {
        self.plans.get(&id).is_some_and(|plan| {
            matches!(
                plan.role,
                Role::Play { hopping: true, .. } | Role::Journey { .. } | Role::Hesitate { .. }
            )
        })
    }

    /// How long a game still making contact between two creatures has left to run. Games are
    /// bounded, so the contact a game is deliberately making carries its own deadline to end by;
    /// everything else a plan does is beside a creature, not through it, and earns no grace.
    pub(super) fn contact_scene_remaining(&self, id: CreatureId) -> Option<f32> {
        self.plans
            .get(&id)
            .filter(|plan| matches!(plan.role, Role::Play { .. }))
            .map(|plan| (plan.seconds + plan.delay + plan.travel_elapsed - plan.elapsed).max(0.0))
    }

    /// Spots companions have already set off for, which nothing else may claim.
    pub(super) fn reserved_spots(&self) -> impl Iterator<Item = (CreatureId, Point)> + '_ {
        self.plans
            .iter()
            .filter_map(|(&id, plan)| plan.walk.map(|walk| (id, walk.destination)))
    }
}

impl World {
    pub(super) fn clear_attention(&mut self) {
        for creature in &mut self.save.creatures {
            if let Some(plan) = self.attention.plans.get(&creature.id) {
                release(creature, *plan);
            }
            creature.state.attention = None;
        }
        self.attention.plans.clear();
        self.attention.cooldowns.clear();
        self.attention.colony_cooldown = 0.0;
        self.attention.recent_supports.clear();
        self.attention.play.reset();
    }

    pub(super) fn remember_attention_supports(&mut self, dt: f32) {
        self.attention
            .recent_supports
            .retain(|id, (_, remaining, _)| {
                *remaining -= dt;
                *remaining > 0.0 && self.save.creatures.iter().any(|c| c.id == *id)
            });
        for creature in self.save.creatures.iter().take(MAX_COLONY_CREATURES) {
            if let Some(key) = creature.state.surface.window_key {
                let support = self
                    .attention
                    .recent_supports
                    .entry(creature.id)
                    .or_insert((key, 8.0, None));
                if support.0 != key {
                    *support = (key, 8.0, None);
                }
                support.1 = 8.0;
            }
        }
    }

    fn familiar_loss(&self, creature: &Creature, signal: GeometrySignal) -> bool {
        signal.kind == GeometryChange::Disappeared
            && self
                .attention
                .recent_supports
                .get(&creature.id)
                .is_some_and(|(window, _, origin)| {
                    *window == signal.window && *origin != Some(signal.origin)
                })
    }

    pub(super) fn advance_attention(&mut self, desktop: &DesktopSnapshot, dt: f32, ready: bool) {
        self.attention.decay_setbacks(dt);
        if !ready || self.colony_plan.is_some() {
            self.cancel_gap_journeys(desktop);
            self.clear_attention();
            return;
        }
        self.update_gap_attention(desktop);
        self.update_hesitation(desktop, dt);
        self.update_dares(desktop);
        self.update_cursor_races(desktop);
        self.update_tumble_attention();
        self.try_assistance(desktop);
        self.advance_play(desktop, dt);
        self.observe_play(desktop, dt);
        self.attention.colony_cooldown = (self.attention.colony_cooldown - dt).max(0.0);
        self.attention.cooldowns.retain(|id, remaining| {
            *remaining -= dt;
            *remaining > 0.0 && self.save.creatures.iter().any(|c| c.id == *id)
        });
        if self.attention.plans.is_empty()
            && self.geometry_observer.signals().next().is_none()
            && self.cursor_observer.cue().is_none()
            && !self.display_attention.has_pending()
            && self.surface_memory.inspect_in > 0.0
            && self.window_journeys.is_empty()
            && !self.attention.play.invitation_ready()
            && !self
                .save
                .creatures
                .iter()
                .any(|c| self.ride_memory.dizzy(c.id))
        {
            return;
        }

        // Decide cancellations from one snapshot so iteration order cannot let an observer keep
        // watching an interrupted actor for an extra tick. This pass is bounded by four creatures.
        let invalid: Vec<_> = self
            .attention
            .plans
            .iter()
            .filter_map(|(&id, plan)| (!self.reaction_valid(id, plan, desktop)).then_some(id))
            .collect();
        let observers_of_invalid: Vec<_> = self
            .attention
            .plans
            .iter()
            .filter_map(|(&id, plan)| match plan.role {
                Role::Observer { actor } | Role::Helper { actor, .. }
                    if invalid.contains(&actor) =>
                {
                    Some(id)
                }
                _ => None,
            })
            .collect();
        for &id in &invalid {
            // An attempt that becomes impossible mid-scene is a setback, never a success.
            if self.attention.plans.get(&id).is_some_and(|p| {
                matches!(
                    p.role,
                    Role::Journey {
                        stage: Stage::Prepare | Stage::Act | Stage::Catch,
                        escape: false,
                        ..
                    }
                )
            }) {
                let route = match self.window_journeys.get(&id) {
                    Some(WindowJourney::Gap(gap)) => gap
                        .hop
                        .surface
                        .window_key
                        .map(|target| (gap.source, target)),
                    _ => None,
                };
                self.attention.record_setback(id, route);
            }
        }
        for id in invalid.into_iter().chain(observers_of_invalid) {
            if matches!(self.window_journeys.get(&id), Some(WindowJourney::Gap(_))) {
                self.window_journeys.remove(&id);
                if let Some(creature) = creature_mut(&mut self.save.creatures, id) {
                    settle_interrupted_journey(
                        creature,
                        desktop,
                        &self.save.settings.habitat,
                        &mut self.events,
                    );
                }
            }
            if let Some(plan) = self.attention.plans.remove(&id)
                && let Some(creature) = creature_mut(&mut self.save.creatures, id)
            {
                release(creature, plan);
            }
        }
        let cues: Vec<_> = self
            .attention
            .plans
            .iter()
            .filter_map(|(&id, plan)| spectacle::cue(plan).map(|cue| (id, cue)))
            .collect();
        // Normal completion lets companions finish with a brief recovery. Safety interruptions
        // above still release the whole audience immediately.
        let completed: Vec<_> = self
            .attention
            .plans
            .iter()
            .filter_map(|(&id, plan)| {
                (plan.elapsed + dt >= plan.seconds + plan.delay + plan.travel_elapsed).then_some(id)
            })
            .collect();
        for plan in self.attention.plans.values_mut() {
            if let Role::Observer { actor } | Role::Helper { actor, .. } = plan.role
                && completed.contains(&actor)
            {
                plan.walk = None;
                plan.elapsed = plan.elapsed.max(plan.delay + plan.travel_elapsed + 2.15);
                // However late a watcher was, it still sees how the scene ended.
                plan.cue = cues
                    .iter()
                    .find(|(id, _)| *id == actor)
                    .map(|(_, cue)| *cue)
                    .or(plan.cue);
            }
        }
        for id in completed {
            if let Some(plan) = self.attention.plans.remove(&id)
                && let Some(creature) = creature_mut(&mut self.save.creatures, id)
            {
                release(creature, plan);
            }
        }
        self.advance_riding_attention(desktop);
        self.try_accidental_launch(desktop, dt);
        self.try_jump_ship(desktop, dt);
        let neighbors: Vec<_> = self
            .save
            .creatures
            .iter()
            .take(MAX_COLONY_CREATURES)
            .filter(|c| c.state.arrival_delay_secs <= 0.0)
            .map(|c| (c.id, c.state.surface.clone(), c.state.position))
            .chain(self.window_journeys.iter().map(|(&id, journey)| {
                (
                    id,
                    journey.surface().clone(),
                    motion::landing_point(journey),
                )
            }))
            .collect();
        // Snapshot only positions; do not clone genomes or retain the desktop in a plan.
        let positions: Vec<_> = self
            .save
            .creatures
            .iter()
            .take(MAX_COLONY_CREATURES)
            .map(|c| {
                (
                    c.id,
                    head_point(c, &self.save.settings, desktop),
                    c.state.attention.map(|pose| pose.emotion),
                )
            })
            .collect();
        for (&id, plan) in &mut self.attention.plans {
            plan.elapsed += dt;
            if let Role::Cursor {
                investigate: false, ..
            } = plan.role
            {
                plan.target = desktop.cursor.position;
            } else if let Role::Observer { actor } | Role::Helper { actor, .. } = plan.role {
                // Watchers register a new stage in their own time, so a colony does not gasp
                // in unison. Until then they keep playing out the stage they already saw.
                let lag = (plan.delay - 0.25).clamp(0.0, 0.3);
                match cues.iter().find(|(id, _)| *id == actor) {
                    Some((_, fresh))
                        if plan.cue.is_none_or(|old| old.stage != fresh.stage)
                            && fresh.since < lag =>
                    {
                        if let Some(old) = &mut plan.cue {
                            old.since += dt;
                        }
                    }
                    Some((_, fresh)) => {
                        plan.cue = Some(Cue {
                            since: fresh.since - lag,
                            ..*fresh
                        });
                    }
                    None => {}
                }
                if let Some((_, point, _)) = positions.iter().find(|(other, _, _)| *other == actor)
                {
                    plan.target = *point;
                }
            } else if let Role::Actor {
                window,
                vanished: false,
                companion,
            } = plan.role
                && let Some(bounds) = desktop
                    .windows
                    .iter()
                    .find(|w| w.key == window)
                    .map(|w| w.bounds)
                && let Some(creature) = self.save.creatures.iter().find(|c| c.id == id)
            {
                plan.target = companion
                    .and_then(|other| {
                        positions
                            .iter()
                            .find(|(id, _, _)| *id == other)
                            .map(|(_, point, _)| *point)
                    })
                    .unwrap_or_else(|| window_gaze(bounds, creature.state.position));
                if plan
                    .walk
                    .is_some_and(|walk| walk.watched_bounds.is_some_and(|old| old != bounds))
                {
                    plan.walk = None;
                }
                if let Some(signal) = self
                    .geometry_observer
                    .signals()
                    .find(|s| Origin::Window(s.origin) == plan.origin)
                {
                    plan.emotion = actor_emotion(creature, signal);
                }
            }
            // A creature on its own journey, or in a toss, is moved again after presentation. A
            // pose may only show when this tick's step leaves it standing exactly as presented.
            let held = !self.tosses.contains_key(&id)
                && self.window_journeys.get(&id).is_none_or(|journey| {
                    self.save
                        .creatures
                        .iter()
                        .find(|c| c.id == id)
                        .is_some_and(|c| journey_holds_still(journey, c, dt))
                });
            if let Some(creature) = creature_mut(&mut self.save.creatures, id) {
                if self.save.settings.reduce_motion {
                    plan.walk = None;
                    if let Some(walk) = plan.display_walk.take() {
                        walk.settle(creature, desktop);
                    }
                }
                if plan.elapsed - plan.delay >= 0.45
                    && let Some(walk) = &mut plan.display_walk
                {
                    plan.travel_elapsed += dt;
                    if plan.travel_elapsed >= 10.0
                        || !walk.step(creature, &neighbors, desktop, &self.save.settings, dt)
                    {
                        walk.settle(creature, desktop);
                        plan.display_walk = None;
                    }
                    plan.surface = (
                        creature.state.surface.monitor_id,
                        creature.state.surface.window_key,
                    );
                }
                if plan.elapsed - plan.delay
                    >= if matches!(plan.role, Role::Ledge { commute: true, .. }) {
                        1.0
                    } else {
                        0.45
                    }
                    && let Some(walk) = plan.walk
                {
                    plan.travel_elapsed += dt;
                    let travel_cap = if matches!(plan.role, Role::Play { .. }) {
                        motion::MAX_PLAY_TRAVEL
                    } else {
                        MAX_APPROACH_SECONDS
                    };
                    if plan.travel_elapsed >= travel_cap
                        || !motion::step(
                            creature,
                            walk,
                            &neighbors,
                            desktop,
                            &self.save.settings,
                            dt,
                        )
                    {
                        plan.walk = None;
                    }
                }
                present(creature, plan, self.save.settings.reduce_motion, held);
            }
        }
        // A success still on screen can be offered to a watcher before the scene closes.
        self.try_dare(desktop);
        if !self.attention.plans.is_empty() {
            return;
        }

        if self.try_display_attention(desktop) {
            return;
        }
        if self.try_dizzy_attention(desktop) {
            return;
        }
        if self.try_crowding_attention(desktop) {
            return;
        }
        let candidate = self
            .geometry_observer
            .signals()
            .filter_map(|signal| {
                self.save
                    .creatures
                    .iter()
                    .filter(|c| {
                        let familiar = self.familiar_loss(c, signal);
                        (self.attention.colony_cooldown <= 0.0 || familiar)
                            && self.attention_eligible_with_cooldown(c, desktop, familiar)
                    })
                    .filter_map(|creature| {
                        let familiar = self.familiar_loss(creature, signal);
                        let rider = creature.state.surface.window_key == Some(signal.window);
                        let distance = distance_to_rect(creature.state.position, signal.bounds);
                        if !rider
                            && !familiar
                            && (distance > 220.0
                                || !point_exposed(
                                    signal.bounds.clamp(creature.state.position),
                                    Some(signal.window),
                                    desktop,
                                ))
                        {
                            return None;
                        }
                        let p = &creature.personality;
                        if !rider && !familiar && p.curiosity < 0.3 && signal.strength < 0.8 {
                            return None;
                        }
                        Some((
                            f32::from(rider || familiar) * 3.0 + signal.strength + p.curiosity
                                - distance / 300.0,
                            creature.id,
                            signal,
                        ))
                    })
                    .max_by(|a, b| a.0.total_cmp(&b.0))
            })
            .max_by(|a, b| a.0.total_cmp(&b.0));
        let Some((_, actor, signal)) = candidate else {
            self.try_cursor_attention(desktop);
            if self.attention.plans.is_empty() {
                self.promote_window_journey(desktop);
            }
            if self.attention.plans.is_empty() {
                self.try_ledge_attention(desktop);
            }
            if self.attention.plans.is_empty() {
                self.try_play(desktop);
            }
            return;
        };
        self.start_actor(actor, signal, desktop);
        // Two riders may react to the same physical event, but observers never publish a new
        // origin. Everyone else responds to the chosen actor, not independently to the window.
        let riders: Vec<_> = self
            .save
            .creatures
            .iter()
            .filter(|c| {
                c.id != actor
                    && (c.state.surface.window_key == Some(signal.window)
                        || self.familiar_loss(c, signal))
            })
            .filter(|c| {
                self.attention_eligible_with_cooldown(c, desktop, self.familiar_loss(c, signal))
            })
            .take(MAX_PARTICIPANTS - 1)
            .map(|c| c.id)
            .collect();
        for id in riders {
            self.start_actor(id, signal, desktop);
        }
        self.recruit_attention_observers(actor, Origin::Window(signal.origin), desktop);
        self.attention.colony_cooldown = 7.0;
    }

    fn recruit_attention_observers(
        &mut self,
        actor: CreatureId,
        origin: Origin,
        desktop: &DesktopSnapshot,
    ) {
        let actor_creature = self.save.creatures.iter().find(|c| c.id == actor).unwrap();
        let actor_point = head_point(actor_creature, &self.save.settings, desktop);
        let actor_monitor = actor_creature.state.surface.monitor_id;
        let actor_emotion = self.attention.plans[&actor].emotion;
        let mut observers: Vec<_> = self
            .save
            .creatures
            .iter()
            .filter(|c| c.id != actor && self.attention_eligible(c, desktop))
            .filter(|c| c.state.surface.monitor_id == actor_monitor)
            .map(|c| {
                let distance = c.state.position.distance(actor_creature.state.position);
                let bond = self.save.relationships.iter().find(|bond| {
                    (bond.a == actor && bond.b == c.id) || (bond.b == actor && bond.a == c.id)
                });
                // A companion facing away is slower to look up, and is rarely the first to.
                let facing =
                    c.state.facing_right == (actor_creature.state.position.x > c.state.position.x);
                (c.id, distance, bond.copied(), facing)
            })
            .filter(|(_, distance, bond, _)| {
                *distance <= 420.0 && !bond.is_some_and(|b| b.avoidance >= 192)
            })
            .filter(|(id, _, bond, _)| {
                self.save
                    .creatures
                    .iter()
                    .find(|c| c.id == *id)
                    .is_some_and(|c| {
                        c.personality.sociability + c.personality.curiosity >= 0.65
                            || bond.is_some_and(|b| b.affinity >= 128)
                    })
            })
            .collect::<Vec<_>>()
            .into_iter()
            .map(|(id, distance, bond, facing)| {
                // A little variation in who looks up first keeps repeated scenes from lining up.
                let jitter = self.ambient_rng.random_range(0.0..40.0);
                (
                    distance - bond.map_or(0.0, |b| f32::from(b.affinity) * 0.3)
                        + f32::from(!facing) * 60.0
                        + jitter,
                    id,
                    facing,
                )
            })
            .collect();
        observers.sort_by(|a, b| a.0.total_cmp(&b.0));
        for (index, (_, id, facing)) in observers.into_iter().enumerate() {
            if self.attention.plans.len() >= MAX_PARTICIPANTS {
                break;
            }
            let creature = self.save.creatures.iter().find(|c| c.id == id).unwrap();
            let delay = 0.25
                + index as f32 * 0.22
                + (1.0 - creature.personality.curiosity) * 0.25
                + f32::from(!facing) * 0.3;
            let reaction = Reaction {
                origin,
                role: Role::Observer { actor },
                target: actor_point,
                emotion: if actor_emotion == AttentionEmotion::Startled {
                    AttentionEmotion::Concerned
                } else {
                    AttentionEmotion::Curious
                },
                elapsed: 0.0,
                seconds: REACTION_SECONDS,
                display_walk: None,
                delay,
                travel_elapsed: 0.0,
                walk: motion::observer_walk(
                    self,
                    creature,
                    self.save.creatures.iter().find(|c| c.id == actor).unwrap(),
                    desktop,
                ),
                surface: (
                    creature.state.surface.monitor_id,
                    creature.state.surface.window_key,
                ),
                action: ActionKind::InspectScreen,
                cue: None,
                ride: None,
            };
            self.begin_attention(id, reaction);
        }
    }

    pub(super) fn cancel_creature_attention(&mut self, id: CreatureId) {
        self.attention.recent_supports.remove(&id);
        let affected: Vec<_> = self
            .attention
            .plans
            .iter()
            .filter_map(|(&other, plan)| {
                (other == id || matches!(plan.role, Role::Observer { actor } | Role::Helper { actor, .. } if actor == id))
                    .then_some(other)
            })
            .collect();
        for other in affected {
            if let Some(plan) = self.attention.plans.remove(&other)
                && let Some(creature) = creature_mut(&mut self.save.creatures, other)
            {
                release(creature, plan);
            }
        }
    }

    fn attention_eligible(&self, creature: &Creature, desktop: &DesktopSnapshot) -> bool {
        self.attention_eligible_with_cooldown(creature, desktop, false)
    }

    fn attention_eligible_with_cooldown(
        &self,
        creature: &Creature,
        desktop: &DesktopSnapshot,
        familiar_loss: bool,
    ) -> bool {
        creature.state.arrival_delay_secs <= 0.0
            && creature.state.drives.energy > 0.15
            && creature.state.drives.sleep_pressure < 0.85
            && matches!(
                creature.state.action,
                ActionKind::Idle
                    | ActionKind::Perch
                    | ActionKind::Traverse
                    | ActionKind::InspectScreen
                    | ActionKind::ReactToWindow
                    | ActionKind::RideWindow
            )
            && !self.attention.plans.contains_key(&creature.id)
            && (familiar_loss || !self.attention.cooldowns.contains_key(&creature.id))
            && !self.window_journeys.contains_key(&creature.id)
            && !self.tosses.contains_key(&creature.id)
            && !self.bond_plans.contains_key(&creature.id)
            && !self
                .interaction
                .as_ref()
                .is_some_and(|i| i.creature_id == creature.id)
            && point_exposed(
                head_point(creature, &self.save.settings, desktop),
                creature.state.surface.window_key,
                desktop,
            )
            && desktop.monitors.iter().any(|m| {
                m.id == creature.state.surface.monitor_id
                    && habitat_contains(&self.save.settings.habitat, m, creature.state.position)
            })
    }

    fn reaction_valid(&self, id: CreatureId, plan: &Reaction, desktop: &DesktopSnapshot) -> bool {
        let Some(creature) = self.save.creatures.iter().find(|c| c.id == id) else {
            return false;
        };
        if (!matches!(
            plan.role,
            Role::Journey { .. } | Role::Tumble { .. } | Role::Play { hopping: true, .. }
        ) && (creature.state.action != plan.action
            || (
                creature.state.surface.monitor_id,
                creature.state.surface.window_key,
            ) != plan.surface))
            || self
                .interaction
                .as_ref()
                .is_some_and(|i| i.creature_id == id)
            || (self.tosses.contains_key(&id) && !matches!(plan.role, Role::Tumble { .. }))
            || !point_exposed(
                head_point(creature, &self.save.settings, desktop),
                creature.state.surface.window_key,
                desktop,
            )
            || !desktop.monitors.iter().any(|m| {
                m.id == plan.surface.0
                    && habitat_contains(&self.save.settings.habitat, m, creature.state.position)
            })
        {
            return false;
        }
        match plan.role {
            Role::Play {
                bounds, hopping, ..
            } => {
                creature.state.drives.energy > 0.2
                    && creature.state.drives.sleep_pressure < 0.8
                    && !self.bond_plans.contains_key(&id)
                    && (hopping || !self.window_journeys.contains_key(&id))
                    && self
                        .window_journeys
                        .get(&id)
                        .is_none_or(|journey| journey.valid(desktop))
                    && match (plan.surface.1, bounds) {
                        (Some(key), Some(bounds)) => desktop.windows.iter().any(|w| {
                            w.key == key && w.bounds == bounds && w.visible && !w.minimized
                        }),
                        // A ride is expected to move; only the surface itself must survive.
                        (Some(key), None) => desktop
                            .windows
                            .iter()
                            .any(|w| w.key == key && w.visible && !w.minimized),
                        (None, _) => true,
                    }
            }
            Role::Tumble { stage } => {
                self.tosses.contains_key(&id)
                    || (matches!(stage, Stage::Recover(_))
                        && matches!(
                            creature.state.action,
                            ActionKind::Idle | ActionKind::Perch | ActionKind::Landing
                        ))
            }
            Role::Journey {
                target_window,
                target_bounds,
                stage,
                ..
            } => {
                let target_present = target_unchanged(target_window, target_bounds, desktop);
                if let Some(journey) = self.window_journeys.get(&id) {
                    journey.valid(desktop)
                        && !self.save.settings.reduce_motion
                        && journey.surface().window_key == target_window
                        && target_present
                } else {
                    matches!(stage, Stage::Recover(_))
                        && creature.state.surface.window_key == target_window
                        && target_present
                        && matches!(
                            creature.state.action,
                            ActionKind::Perch | ActionKind::Greet | ActionKind::Idle
                        )
                }
            }
            Role::Helper {
                actor,
                window,
                bounds,
            } => {
                desktop
                    .windows
                    .iter()
                    .any(|w| w.key == window && w.bounds == bounds && w.visible && !w.minimized)
                    && self
                        .save
                        .creatures
                        .iter()
                        .find(|c| c.id == actor)
                        .is_some_and(|c| {
                            c.state.surface.window_key == Some(window)
                                && c.state.position.distance(creature.state.position) < 180.0
                        })
                    && self
                        .attention
                        .plans
                        .get(&actor)
                        .is_some_and(|p| matches!(p.role, Role::Journey { .. }))
            }
            Role::Dare {
                source,
                source_bounds,
                target_window,
                target_bounds,
                ..
            }
            | Role::Hesitate {
                source,
                source_bounds,
                target_window,
                target_bounds,
                ..
            } => {
                !self.save.settings.reduce_motion
                    && creature.state.surface.window_key == Some(source)
                    && target_unchanged(Some(source), Some(source_bounds), desktop)
                    && target_unchanged(Some(target_window), Some(target_bounds), desktop)
            }
            Role::Ledge { window, bounds, .. } => desktop
                .windows
                .iter()
                .any(|w| w.key == window && w.visible && !w.minimized && w.bounds == bounds),
            Role::Cursor {
                investigate,
                anchor,
                ..
            } => self.cursor_reaction_valid(creature, investigate, anchor, desktop),
            Role::Display { .. } => plan
                .display_walk
                .is_none_or(|walk| walk.valid(desktop, &self.save.settings)),
            Role::Observer { actor } => {
                (plan.elapsed - plan.delay - plan.travel_elapsed >= 2.15
                    || self.attention.plans.get(&actor).is_some_and(|p| {
                        p.origin == plan.origin && !matches!(p.role, Role::Observer { .. })
                    }))
                    && self
                        .save
                        .creatures
                        .iter()
                        .find(|c| c.id == actor)
                        .is_some_and(|c| {
                            c.state.surface.monitor_id == plan.surface.0
                                && c.state.position.distance(creature.state.position) <= 480.0
                                && point_exposed(
                                    head_point(c, &self.save.settings, desktop),
                                    c.state.surface.window_key,
                                    desktop,
                                )
                        })
            }
            Role::Actor {
                window, vanished, ..
            } => {
                vanished
                    || desktop
                        .windows
                        .iter()
                        .any(|w| w.key == window && w.visible && !w.minimized)
            }
        }
    }

    fn start_actor(&mut self, id: CreatureId, signal: GeometrySignal, desktop: &DesktopSnapshot) {
        let creature = self.save.creatures.iter().find(|c| c.id == id).unwrap();
        let rider = creature.state.surface.window_key == Some(signal.window);
        let emotion = actor_emotion(creature, signal);
        let reaction = Reaction {
            origin: Origin::Window(signal.origin),
            role: Role::Actor {
                window: signal.window,
                vanished: signal.kind == GeometryChange::Disappeared,
                companion: None,
            },
            target: window_gaze(signal.bounds, creature.state.position),
            emotion,
            elapsed: 0.0,
            seconds: REACTION_SECONDS,
            display_walk: None,
            delay: 0.0,
            travel_elapsed: 0.0,
            walk: motion::actor_walk(self, creature, signal, desktop),
            surface: (
                creature.state.surface.monitor_id,
                creature.state.surface.window_key,
            ),
            action: if rider {
                creature.state.action
            } else {
                ActionKind::InspectScreen
            },
            cue: None,
            ride: if rider && signal.kind == GeometryChange::Moved {
                self.ride_memory.pose(id)
            } else {
                None
            },
        };
        self.begin_attention(id, reaction);
    }

    fn begin_attention(&mut self, id: CreatureId, mut reaction: Reaction) {
        self.action_choices.remove(&id);
        // A journey or a toss begun alongside this plan moves the creature later in the tick.
        let held = !self.window_journeys.contains_key(&id) && !self.tosses.contains_key(&id);
        let creature = creature_mut(&mut self.save.creatures, id).unwrap();
        // Interruption intentionally emits no ActionCompleted or learned achievement.
        let original = creature.state.action;
        present(
            creature,
            &mut reaction,
            self.save.settings.reduce_motion,
            held,
        );
        if original != creature.state.action {
            Self::emit(
                &mut self.events,
                WorldEvent::ActionStarted {
                    creature_id: id,
                    action: creature.state.action,
                },
            );
        }
        if matches!(reaction.role, Role::Actor { vanished: true, .. })
            && let Some(support) = self.attention.recent_supports.get_mut(&id)
            && let Origin::Window(origin) = reaction.origin
        {
            support.2 = Some(origin);
        }
        self.attention.plans.insert(id, reaction);
        self.attention.cooldowns.insert(id, REACTION_COOLDOWN);
    }
}

/// A captured destination is still the same visible rectangle; habitat floor has no rectangle.
fn target_unchanged(
    window: Option<WindowKey>,
    bounds: Option<DesktopRect>,
    desktop: &DesktopSnapshot,
) -> bool {
    match (window, bounds) {
        (Some(key), Some(bounds)) => desktop
            .windows
            .iter()
            .any(|w| w.key == key && w.bounds == bounds && w.visible && !w.minimized),
        (None, None) => true,
        _ => false,
    }
}

/// The point on a window a creature reacting to it is actually looking at.
///
/// The obvious answer — the window rectangle clamped to the creature — is the point *closest* to
/// it rather than the point worth looking at, and it made two very visible lies. A creature
/// standing beside a window got a target at its own feet's height, so however tall the window was
/// it looked at the floor next to its toes; and a creature standing on the window that moved got
/// its own position back, so it stared straight down with no direction at all and never even
/// turned to face what it was reacting to.
///
/// So: the near edge, halfway down the window, for a creature beside it, which is the bit of the
/// frame a companion on the desktop can see; and the window's own centre for one standing over or
/// under it, which sends the look down and along the surface it is riding. Both are points on the
/// window rather than points on the creature, which is the whole of the difference.
fn window_gaze(bounds: DesktopRect, at: Point) -> Point {
    Point {
        x: if (bounds.x..=bounds.right()).contains(&at.x) {
            bounds.x + bounds.width * 0.5
        } else if at.x < bounds.x {
            bounds.x
        } else {
            bounds.right()
        },
        y: bounds.y + bounds.height * 0.5,
    }
}

/// Creature-scale unit: logical desktop points per art pixel on the creature's display.
fn creature_unit(creature: &Creature, settings: &Settings, desktop: &DesktopSnapshot) -> f32 {
    desktop
        .monitors
        .iter()
        .find(|m| m.id == creature.state.surface.monitor_id)
        .map_or(1.0, |m| {
            (f32::from(settings.display_scale) / m.scale_factor).clamp(0.5, 2.0)
        })
}

fn head_point(creature: &Creature, settings: &Settings, desktop: &DesktopSnapshot) -> Point {
    let scale = desktop
        .monitors
        .iter()
        .find(|m| m.id == creature.state.surface.monitor_id)
        .map_or(1.0, |m| m.scale_factor.max(0.5));
    Point {
        x: creature.state.position.x,
        y: creature.state.position.y
            + (-24.0 + creature.state.attention.map_or(0.0, |p| p.hanging) * 36.0)
                * f32::from(settings.display_scale)
                / scale,
    }
}

pub(super) fn point_exposed(
    point: Point,
    supporting: Option<WindowKey>,
    desktop: &DesktopSnapshot,
) -> bool {
    let support_z = supporting.and_then(|key| {
        desktop
            .windows
            .iter()
            .find(|w| w.key == key && w.visible && !w.minimized)
            .map(|w| w.z_order)
    });
    !desktop.windows.iter().take(MAX_TOPOLOGY_WINDOWS).any(|w| {
        w.visible
            && !w.minimized
            && Some(w.key) != supporting
            && support_z.is_none_or(|z| w.z_order < z)
            && w.bounds.contains(point)
    })
}

#[cfg(test)]
mod tests;
