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
mod spectacle;
pub(super) use displays::DisplayAttention;
use displays::DisplayWalk;
use motion::{MAX_APPROACH_SECONDS, ShortWalk};
use spectacle::{Cue, Outcome, Stage};

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
    setbacks: [Option<Setback>; MAX_PARTICIPANTS],
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
                (0..MAX_PARTICIPANTS)
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
        for creature in self.save.creatures.iter().take(MAX_PARTICIPANTS) {
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
            .take(MAX_PARTICIPANTS)
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
            .take(MAX_PARTICIPANTS)
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
                    .unwrap_or_else(|| bounds.clamp(creature.state.position));
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
            target: signal.bounds.clamp(creature.state.position),
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

/// How long a creature that caught the far edge teeters there once it has pulled itself up.
const CATCH_WOBBLE_SECONDS: f32 = 0.5;
/// How close a stationary investigator must be to the cursor to reach out for it.
const CURSOR_REACH: f32 = 120.0;

/// Present one plan for this tick: the action, where the creature looks and how it feels, and any
/// body pose struck over that action. `held` is false when something after presentation still
/// moves the creature this tick — its own journey stepping on, or a toss — so a pose chosen now
/// could not be trusted to match what is drawn.
fn present(creature: &mut Creature, plan: &mut Reaction, reduced_motion: bool, held: bool) {
    present_role(creature, plan, reduced_motion);
    // Every role proposes its own pose. This is the one place that decides whether the body is
    // actually free to show it, so no role can put a pose over travel by mistake.
    if !body_free(creature, plan, reduced_motion, held)
        && let Some(pose) = &mut creature.state.attention
    {
        pose.gesture = None;
    }
}

/// Whether a gesture may stand in for the action's own clip. Only a planted presentation gives
/// its body over to a pose: never with reduced motion, never while walking, hopping, carried by a
/// journey or a toss, or hanging by the hands, and never over an action whose clip is itself the
/// point, such as a sprint, a squeeze, a meal, a ride, a nap, or a toy being shown off.
///
/// What is actually moving the creature decides this, rather than its velocity: an approach that
/// has just handed over to a journey leaves the last stride on the books for a while, and a
/// creature standing perfectly still at the edge of a gap is not travelling anywhere.
fn body_free(creature: &Creature, plan: &Reaction, reduced_motion: bool, held: bool) -> bool {
    let planted = matches!(
        creature.state.action,
        ActionKind::Idle
            | ActionKind::Perch
            | ActionKind::InspectScreen
            | ActionKind::Greet
            | ActionKind::SocialPlay
            | ActionKind::SoloPlay
            | ActionKind::ReactToWindow
            | ActionKind::InvestigateCursor
    );
    planted
        && held
        && !reduced_motion
        && plan.walk.is_none()
        && plan.display_walk.is_none()
        && !matches!(plan.role, Role::Play { hopping: true, .. })
        && creature
            .state
            .attention
            .is_some_and(|pose| pose.hanging <= 0.0)
}

/// Whether this tick's step of a creature's own journey leaves it exactly where, and as, it was
/// just presented. The journey moves the creature after presentation, so a pose chosen during a
/// pause must not survive into the step that sets it going again.
fn journey_holds_still(journey: &WindowJourney, creature: &Creature, dt: f32) -> bool {
    let step = journey.clone().advance(dt);
    !step.complete
        && step.action == creature.state.action
        && step.position == creature.state.position
}

/// A helper hauls on its companion while it hangs, and cheers only once the companion is up.
fn helper_pose(plan: &Reaction) -> Option<Gesture> {
    let rescued = plan
        .cue
        .is_some_and(|actor| actor.stage == Stage::Recover(Outcome::Completed));
    match spectacle::cue(plan)?.stage {
        Stage::Catch | Stage::Recover(_) if rescued => Some(Gesture::Cheer),
        Stage::Catch => Some(Gesture::Heave),
        _ => None,
    }
}

fn present_role(creature: &mut Creature, plan: &mut Reaction, reduced_motion: bool) {
    if let Role::Play {
        gesture,
        hopping,
        pose,
        ..
    } = plan.role
    {
        if hopping {
            // A leap in progress owns its own contact, action, and timing.
            plan.action = creature.state.action;
            creature.state.attention = Some(AttentionPose {
                target: plan.target,
                emotion: plan.emotion,
                hanging: 0.0,
                gesture: None,
            });
            return;
        }
        let walking = plan.walk.is_some() && !reduced_motion;
        let action = if reduced_motion {
            ActionKind::InspectScreen
        } else {
            gesture
        };
        if creature.state.action != action {
            creature.state.action_elapsed = 0.0;
        }
        creature.state.action = action;
        creature.state.action_duration = f32::MAX;
        // A travelling player is steered by its own walk, which sets facing and velocity.
        if !walking {
            creature.state.velocity = Point::default();
            creature.state.facing_right = plan.target.x >= creature.state.position.x;
        }
        creature.state.attention = Some(AttentionPose {
            target: plan.target,
            emotion: plan.emotion,
            hanging: 0.0,
            gesture: pose,
        });
        plan.action = action;
        return;
    }
    if let Role::Tumble { stage } = plan.role {
        if matches!(stage, Stage::Recover(_)) {
            creature.state.action = if creature.state.surface.window_key.is_some() {
                ActionKind::Perch
            } else {
                ActionKind::Idle
            };
            creature.state.velocity = Point::default();
            creature.state.action_duration = f32::MAX;
        }
        plan.action = creature.state.action;
        let recovering = matches!(stage, Stage::Recover(_));
        creature.state.attention = Some(AttentionPose {
            target: plan.target,
            emotion: if recovering {
                AttentionEmotion::Relieved
            } else {
                AttentionEmotion::Startled
            },
            hanging: 0.0,
            // A tumble is tossed or landing until it recovers, and both keep the body, so the
            // fall itself has no pose to strike.
            gesture: None,
        });
        return;
    }
    if let Role::Dare { landing, .. } = plan.role {
        let walking = plan.walk.is_some() && !reduced_motion;
        let action = if walking {
            ActionKind::Traverse
        } else {
            ActionKind::InspectScreen
        };
        if creature.state.action != action {
            creature.state.action_elapsed = 0.0;
        }
        creature.state.action = action;
        creature.state.action_duration = f32::MAX;
        if !walking {
            creature.state.velocity = Point::default();
            creature.state.facing_right = landing.x > creature.state.position.x;
        }
        plan.action = action;
        creature.state.attention = Some(AttentionPose {
            target: landing,
            emotion: plan.emotion,
            hanging: 0.0,
            // Facing the gap it was dared to try: a bold creature squares up to it, and a timid
            // one frets.
            gesture: Some(if creature.personality.boldness >= 0.5 {
                Gesture::Crouch
            } else {
                Gesture::Worry
            }),
        });
        return;
    }
    if let Role::Hesitate {
        phase,
        phase_elapsed,
        landing,
        edge,
        drop,
        ..
    } = plan.role
    {
        let walking = plan.walk.is_some() && !reduced_motion;
        let below = Point {
            x: edge.x + if landing.x >= edge.x { 20.0 } else { -20.0 },
            y: edge.y + drop.clamp(12.0, 180.0),
        };
        let (action, target, emotion, gesture) = match phase {
            HesitatePhase::Look if phase_elapsed < 0.45 => (
                ActionKind::InspectScreen,
                landing,
                AttentionEmotion::Curious,
                None,
            ),
            HesitatePhase::Look => (
                ActionKind::InspectScreen,
                below,
                AttentionEmotion::Concerned,
                Some(Gesture::Worry),
            ),
            HesitatePhase::BackUp | HesitatePhase::Approach if walking => (
                ActionKind::Traverse,
                landing,
                AttentionEmotion::Concerned,
                None,
            ),
            HesitatePhase::BackUp | HesitatePhase::Approach => (
                ActionKind::InspectScreen,
                landing,
                AttentionEmotion::Concerned,
                Some(Gesture::Worry),
            ),
            // Leaning over the edge: alternate between the landing and the drop below it.
            HesitatePhase::Reconsider => (
                ActionKind::InspectScreen,
                if ((phase_elapsed / 0.5) as u32).is_multiple_of(2) {
                    below
                } else {
                    landing
                },
                AttentionEmotion::Concerned,
                Some(Gesture::Balance),
            ),
            HesitatePhase::Retreat if walking => (
                ActionKind::Traverse,
                landing,
                AttentionEmotion::Relieved,
                None,
            ),
            HesitatePhase::Retreat => {
                (ActionKind::Perch, landing, AttentionEmotion::Relieved, None)
            }
        };
        if creature.state.action != action {
            creature.state.action = action;
            creature.state.action_elapsed = 0.0;
        }
        plan.action = action;
        creature.state.action_duration = f32::MAX;
        if !walking {
            creature.state.velocity = Point::default();
            if (landing.x - creature.state.position.x).abs() > 8.0 {
                creature.state.facing_right = landing.x > creature.state.position.x;
            }
        }
        creature.state.attention = Some(AttentionPose {
            target,
            emotion,
            hanging: 0.0,
            gesture,
        });
        return;
    }
    if let Role::Journey {
        stage,
        hanging,
        rewarding,
        escape,
        since,
        caught,
        ..
    } = plan.role
    {
        if matches!(stage, Stage::Recover(_)) {
            creature.state.action = if !reduced_motion && rewarding {
                ActionKind::Greet
            } else if creature.state.surface.window_key.is_some() {
                ActionKind::Perch
            } else {
                ActionKind::Idle
            };
            creature.state.velocity = Point::default();
        }
        plan.action = creature.state.action;
        let in_stage = plan.elapsed - since;
        let gesture = match stage {
            // Pulled back up after catching the far edge: a wobble at the brink, then delight.
            Stage::Recover(Outcome::Completed) if caught && in_stage < CATCH_WOBBLE_SECONDS => {
                Some(Gesture::Balance)
            }
            Stage::Recover(Outcome::Completed) if rewarding => Some(Gesture::Cheer),
            Stage::Recover(_) => None,
            // Squaring up to a real leap. A gap's run-up is travel, so the still moment at the
            // edge before it sets off is the leap's only wind-up a pose can show over.
            Stage::Notice if rewarding && in_stage >= 0.15 => Some(Gesture::Crouch),
            // The pause before a climb, or before slipping through a squeeze.
            Stage::Prepare => Some(Gesture::Crouch),
            _ => None,
        };
        creature.state.attention = Some(AttentionPose {
            target: plan.target,
            emotion: match stage {
                Stage::Recover(Outcome::Completed) => AttentionEmotion::Enjoying,
                Stage::Recover(_) => AttentionEmotion::Relieved,
                _ if escape => AttentionEmotion::Startled,
                Stage::Act => AttentionEmotion::Concerned,
                Stage::Catch => AttentionEmotion::Startled,
                _ => AttentionEmotion::Curious,
            },
            hanging,
            gesture,
        });
        return;
    }
    let elapsed = plan.elapsed - plan.delay - plan.travel_elapsed;
    let walking = (plan.walk.is_some() || plan.display_walk.is_some())
        && plan.elapsed - plan.delay >= 0.45
        && !reduced_motion;
    let mut emotion = if elapsed < 0.0 {
        None
    } else if elapsed < 0.45 && !walking {
        Some(AttentionEmotion::Curious)
    } else if elapsed < 2.15 {
        Some(plan.emotion)
    } else {
        Some(match plan.emotion {
            AttentionEmotion::Startled | AttentionEmotion::Concerned => AttentionEmotion::Relieved,
            _ => AttentionEmotion::Curious,
        })
    };
    let rider = matches!(plan.role, Role::Actor { window, .. } if Some(window) == creature.state.surface.window_key);
    let mut action = if walking {
        if matches!(plan.role, Role::Cursor { .. }) && plan.emotion == AttentionEmotion::Concerned {
            ActionKind::AvoidCursor
        } else if matches!(
            plan.role,
            Role::Cursor {
                investigate: true,
                ..
            } | Role::Cursor { racing: true, .. }
        ) {
            ActionKind::InvestigateCursor
        } else if plan.emotion == AttentionEmotion::Startled {
            ActionKind::ReactToWindow
        } else {
            ActionKind::Traverse
        }
    } else if rider && !reduced_motion {
        if elapsed >= 2.15 {
            ActionKind::Perch
        } else if plan.emotion == AttentionEmotion::Startled {
            ActionKind::ReactToWindow
        } else {
            ActionKind::RideWindow
        }
    } else if matches!(plan.role, Role::Display { discovery: true }) && elapsed >= 2.15 {
        ActionKind::Perch
    } else {
        ActionKind::InspectScreen
    };
    if matches!(plan.role, Role::Ledge { .. }) && !walking && elapsed >= 2.15 {
        action = ActionKind::Perch;
    }
    if matches!(plan.role, Role::Helper { .. }) && !walking && !reduced_motion {
        action = ActionKind::SocialPlay;
    }
    let mut target = plan.target;
    if matches!(plan.role, Role::Ledge { declined: true, .. })
        && ((0.85..1.15).contains(&elapsed) || (1.65..1.95).contains(&elapsed))
    {
        action = ActionKind::Perch;
        target = Point {
            x: creature.state.position.x,
            y: creature.state.position.y + 40.0,
        };
    }
    let mut watched = None;
    if let Some(cue) = plan.cue
        && elapsed >= 0.0
    {
        spectacle::present_observer(
            creature,
            cue,
            &mut spectacle::Watching {
                reduced: reduced_motion,
                walking,
                watching_for: elapsed,
                action: &mut action,
                emotion: &mut emotion,
                target: &mut target,
                gesture: &mut watched,
            },
        );
    }
    let hanging = if matches!(plan.role, Role::Ledge { commute: true, .. }) && !reduced_motion {
        if elapsed < 0.45 {
            0.0
        } else if elapsed < 1.45 {
            action = ActionKind::Dangle;
            ((elapsed - 0.45) / 0.45).clamp(0.0, 1.0)
        } else if elapsed < 2.15 {
            action = ActionKind::ClimbWindow;
            (1.0 - (elapsed - 1.45) / 0.7).clamp(0.0, 1.0)
        } else {
            0.0
        }
    } else if rider && let Some(ride) = plan.ride {
        rides::present_ride(
            creature,
            plan,
            ride,
            reduced_motion,
            walking,
            &mut action,
            &mut target,
        )
    } else {
        0.0
    };
    if action != creature.state.action {
        creature.state.action = action;
        creature.state.action_elapsed = 0.0;
    }
    plan.action = action;
    creature.state.action_duration = f32::MAX;
    if !walking {
        creature.state.velocity = Point::default();
    }
    if matches!(plan.role, Role::Actor { vanished: true, .. }) && (0.45..2.15).contains(&elapsed) {
        target.x += if (((elapsed - 0.45) / 0.55) as u32).is_multiple_of(2) {
            -48.0
        } else {
            48.0
        };
    }
    let gesture = match plan.role {
        Role::Observer { .. } => watched,
        Role::Helper { .. } => helper_pose(plan),
        // The support is simply gone: a gasp, then a look around for where it went.
        Role::Actor { vanished: true, .. } => (elapsed < 0.45).then_some(Gesture::Gasp),
        // A window lurching at a creature standing beside it. A rider's pose belongs to the ride.
        Role::Actor { .. } => {
            (!rider && emotion == Some(AttentionEmotion::Startled)).then_some(Gesture::Gasp)
        }
        // Close enough to the cursor to reach for it, once it has had a look.
        Role::Cursor {
            investigate: true, ..
        } => (plan.emotion == AttentionEmotion::Curious
            && elapsed >= 0.45
            && (plan.target.x - creature.state.position.x).abs() > 8.0
            && plan.target.distance(creature.state.position) <= CURSOR_REACH)
            .then_some(Gesture::Reach),
        // Looking over a long way down. A commute's hang and climb keep the body.
        Role::Ledge { commute: false, .. } => {
            (emotion == Some(AttentionEmotion::Concerned)).then_some(Gesture::Worry)
        }
        _ => None,
    };
    creature.state.attention = emotion.map(|emotion| AttentionPose {
        target,
        emotion,
        hanging,
        gesture,
    });
    if !walking && elapsed >= 0.0 && (plan.target.x - creature.state.position.x).abs() > 8.0 {
        creature.state.facing_right = plan.target.x > creature.state.position.x;
    }
}

fn actor_emotion(creature: &Creature, signal: GeometrySignal) -> AttentionEmotion {
    let tolerance =
        creature.personality.window_tolerance * 0.65 + creature.personality.boldness * 0.35;
    if creature.state.surface.window_key == Some(signal.window) && tolerance >= 0.65 {
        AttentionEmotion::Enjoying
    } else if matches!(
        signal.kind,
        GeometryChange::Expanded | GeometryChange::Moved
    ) && signal.strength > 0.5
        && tolerance < 0.65
    {
        AttentionEmotion::Startled
    } else {
        AttentionEmotion::Curious
    }
}

fn release(creature: &mut Creature, plan: Reaction) {
    creature.state.attention = None;
    if matches!(
        plan.role,
        Role::Journey {
            stage: Stage::Notice | Stage::Prepare | Stage::Act | Stage::Catch,
            ..
        }
    ) {
        return;
    }
    if matches!(plan.role, Role::Tumble { stage: Stage::Act })
        && creature.state.action == ActionKind::Tossed
    {
        return;
    }
    // A leap already in the air lands on its own terms.
    if matches!(plan.role, Role::Play { hopping: true, .. }) {
        return;
    }
    if creature.state.action == plan.action
        && (
            creature.state.surface.monitor_id,
            creature.state.surface.window_key,
        ) == plan.surface
    {
        creature.state.action = if creature.state.surface.kind == SurfaceKind::WindowLedge {
            ActionKind::Perch
        } else {
            ActionKind::Idle
        };
        creature.state.action_elapsed = 0.0;
        creature.state.action_duration = 2.5;
        creature.state.velocity = Point::default();
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
mod tests {
    use super::*;
    use time::macros::datetime;

    pub(super) fn scene() -> (World, DesktopSnapshot, OffsetDateTime) {
        let created = datetime!(2026-01-01 0:00 UTC);
        let now = created + Duration::days(40);
        let mut desktop = super::super::tests::desktop();
        desktop.window_sample = Some(WindowSample {
            monotonic_millis: 0,
            reliable: true,
        });
        desktop.windows.push(DesktopWindow {
            key: 701,
            bounds: DesktopRect {
                x: 200.0,
                y: 600.0,
                width: 600.0,
                height: 200.0,
            },
            z_order: 0,
            visible: true,
            minimized: false,
            application: None,
            application_name: None,
        });
        let mut world = World::new([92; 32], created, &desktop);
        world.tick(now, 0.05, &desktop);
        super::super::tests::let_colony_wander(&mut world, now);
        world.save.ritual.next_at_utc = now + Duration::days(1);
        for (index, creature) in world.save.creatures.iter_mut().enumerate() {
            creature.state.arrival_delay_secs = 0.0;
            creature.state.action = ActionKind::Idle;
            creature.state.action_elapsed = 0.0;
            creature.state.action_duration = 100.0;
            creature.state.drives = Drives::default();
            creature.personality.curiosity = 1.0;
            creature.personality.sociability = 1.0;
            creature.personality.boldness = if index == 1 { 1.0 } else { 0.0 };
            creature.personality.window_tolerance = if index == 1 { 1.0 } else { 0.0 };
            if index == 1 {
                creature.personality.curiosity = 0.6;
            }
            if index < 2 {
                creature.state.surface = SurfaceAttachment {
                    kind: SurfaceKind::WindowLedge,
                    monitor_id: 1,
                    window_key: Some(701),
                    relative_x: 0.2 + index as f32 * 0.4,
                };
                creature.state.position = Point {
                    x: 320.0 + index as f32 * 240.0,
                    y: 600.0,
                };
            } else {
                creature.state.surface = SurfaceAttachment {
                    kind: SurfaceKind::ScreenFloor,
                    monitor_id: 1,
                    window_key: None,
                    relative_x: 0.5,
                };
                creature.state.position = Point {
                    x: 600.0 + (index - 2) as f32 * 100.0,
                    y: 846.0,
                };
            }
        }
        world.tick(now, 0.05, &desktop);
        world.drain_events().for_each(drop);
        (world, desktop, now)
    }

    fn start(world: &mut World, desktop: &mut DesktopSnapshot, now: OffsetDateTime) {
        desktop.window_sample.as_mut().unwrap().monotonic_millis = 250;
        desktop.windows[0].bounds.x += 120.0;
        world.tick(now + Duration::milliseconds(250), 0.05, desktop);
    }

    /// Actions whose own clip is the whole point of them. A pose never stands in for one.
    const BODY_OWNING: [ActionKind; 16] = [
        ActionKind::Traverse,
        ActionKind::Sprint,
        ActionKind::Follow,
        ActionKind::SqueezeWindow,
        ActionKind::Landing,
        ActionKind::Dragged,
        ActionKind::Tossed,
        ActionKind::Dangle,
        ActionKind::ClimbWindow,
        ActionKind::Sleep,
        ActionKind::RideWindow,
        ActionKind::Eat,
        ActionKind::Drink,
        ActionKind::Homebound,
        ActionKind::AvoidCursor,
        ActionKind::PresentDiscovery,
    ];

    /// Every body pose a run of ticks left on screen, checked tick by tick against the rules for
    /// when one may replace the action's own clip: never with reduced motion, never while
    /// walking, hopping, hanging, tossed, or carried by a journey, only over a planted
    /// presentation, and a reach always toward what the creature is looking at. Scenes read their
    /// poses through this, so "the colony struck a pose" means the same thing in every test.
    #[derive(Default, Debug)]
    pub(super) struct Poses {
        /// Each creature's poses in the order it struck them, with unbroken repeats collapsed.
        pub(super) struck: BTreeMap<CreatureId, Vec<Gesture>>,
        /// How many ticks each creature spent showing each pose.
        pub(super) held: BTreeMap<CreatureId, Vec<(Gesture, usize)>>,
        last: BTreeMap<CreatureId, Gesture>,
    }

    impl Poses {
        /// Run one tick, then check and write down every pose it left on screen.
        pub(super) fn tick(&mut self, world: &mut World, tick: impl FnOnce(&mut World)) {
            let before: Vec<(CreatureId, Point)> = world
                .save
                .creatures
                .iter()
                .map(|c| (c.id, c.state.position))
                .collect();
            tick(world);
            self.note(world, &before);
        }

        fn note(&mut self, world: &World, before: &[(CreatureId, Point)]) {
            for c in &world.save.creatures {
                let shown = c.state.attention.and_then(|pose| pose.gesture);
                let Some(gesture) = shown else {
                    self.last.remove(&c.id);
                    continue;
                };
                let pose = c.state.attention.unwrap();
                let label = format!(
                    "{gesture:?} over {:?} by creature {} at {:?}",
                    c.state.action, c.id, c.state.position
                );
                assert!(
                    !world.save.settings.reduce_motion,
                    "reduced motion: {label}"
                );
                assert!(!BODY_OWNING.contains(&c.state.action), "{label}");
                assert!(
                    matches!(
                        c.state.action,
                        ActionKind::Idle
                            | ActionKind::Perch
                            | ActionKind::InspectScreen
                            | ActionKind::Greet
                            | ActionKind::SocialPlay
                            | ActionKind::SoloPlay
                            | ActionKind::ReactToWindow
                            | ActionKind::InvestigateCursor
                    ),
                    "not a planted presentation: {label}"
                );
                assert_eq!(pose.hanging, 0.0, "hanging: {label}");
                assert!(!world.tosses.contains_key(&c.id), "tossed: {label}");
                let plan = world
                    .attention
                    .plans
                    .get(&c.id)
                    .unwrap_or_else(|| panic!("a pose outlived its scene: {label}"));
                assert!(
                    plan.walk.is_none() && plan.display_walk.is_none(),
                    "walking: {label}"
                );
                assert!(
                    !matches!(plan.role, Role::Play { hopping: true, .. }),
                    "hopping: {label}"
                );
                if world.window_journeys.contains_key(&c.id) {
                    let was = before.iter().find(|(id, _)| *id == c.id).map(|(_, at)| *at);
                    assert_eq!(was, Some(c.state.position), "carried: {label}");
                }
                if gesture == Gesture::Reach && (pose.target.x - c.state.position.x).abs() > 1.0 {
                    assert_eq!(
                        c.state.facing_right,
                        pose.target.x > c.state.position.x,
                        "reaching away from {:?}: {label}",
                        pose.target
                    );
                }
                if self.last.insert(c.id, gesture) != Some(gesture) {
                    self.struck.entry(c.id).or_default().push(gesture);
                }
                let held = self.held.entry(c.id).or_default();
                match held.iter_mut().find(|(g, _)| *g == gesture) {
                    Some((_, ticks)) => *ticks += 1,
                    None => held.push((gesture, 1)),
                }
            }
        }

        /// Whether anyone struck this pose.
        pub(super) fn showed(&self, gesture: Gesture) -> bool {
            self.struck.values().any(|poses| poses.contains(&gesture))
        }

        /// The poses one creature struck, in order.
        pub(super) fn by(&self, id: CreatureId) -> &[Gesture] {
            self.struck.get(&id).map_or(&[], Vec::as_slice)
        }

        /// How many ticks one creature held one pose, all told.
        pub(super) fn ticks(&self, id: CreatureId, gesture: Gesture) -> usize {
            self.held
                .get(&id)
                .and_then(|held| held.iter().find(|(g, _)| *g == gesture))
                .map_or(0, |(_, ticks)| *ticks)
        }

        /// Every pose anyone struck.
        pub(super) fn all(&self) -> Vec<Gesture> {
            let mut all = Vec::new();
            for gesture in self.struck.values().flatten() {
                if !all.contains(gesture) {
                    all.push(*gesture);
                }
            }
            all
        }
    }

    /// What a run of scenes asked of the creatures' bodies, so a sweep for poses that never
    /// appear where the body is busy can show that the body really was busy.
    #[derive(Default, Debug)]
    struct Busy {
        actions: Vec<ActionKind>,
        hung: bool,
        hopped: bool,
        tossed: bool,
    }

    /// Play a scene out, recording its poses and what its bodies were doing meanwhile.
    fn watch(
        world: &mut World,
        desktop: &mut DesktopSnapshot,
        now: OffsetDateTime,
        steps: std::ops::Range<u64>,
        poses: &mut Poses,
        busy: &mut Busy,
    ) {
        for step in steps {
            play_out(world, desktop, now, step..step + 1, poses);
            for creature in &world.save.creatures {
                if BODY_OWNING.contains(&creature.state.action)
                    && !busy.actions.contains(&creature.state.action)
                {
                    busy.actions.push(creature.state.action);
                }
                busy.hung |= creature
                    .state
                    .attention
                    .is_some_and(|pose| pose.hanging > 0.0);
            }
            busy.hopped |= world.attention.plans.values().any(|plan| {
                matches!(
                    plan.role,
                    Role::Play { hopping: true, .. } | Role::Journey { .. }
                )
            });
            busy.tossed |= !world.tosses.is_empty();
        }
    }

    /// Play a scene out tick by tick, writing down every pose it shows.
    fn play_out(
        world: &mut World,
        desktop: &mut DesktopSnapshot,
        now: OffsetDateTime,
        steps: std::ops::Range<u64>,
        poses: &mut Poses,
    ) {
        for step in steps {
            poses.tick(world, |world| {
                desktop.window_sample.as_mut().unwrap().monotonic_millis = step * 50;
                world.tick(
                    now + Duration::milliseconds(step as i64 * 50),
                    0.05,
                    desktop,
                );
            });
        }
    }

    /// Nine poses, and for each of them a scene the colony plays out by itself to strike it. This
    /// is what keeps the vocabulary honest: a pose nothing ever reaches is a pose nobody will see.
    /// Every tick of every scene here is also checked against the rules for showing one at all.
    #[test]
    fn every_pose_in_the_vocabulary_has_a_scene_that_strikes_it() {
        let mut poses = Poses::default();
        // A bold leap over a gap, in front of the colony: the jumper squares up and celebrates,
        // a timid watcher hides its eyes, a bolder one frets through it.
        let (mut world, mut desktop, now) = super::ledges::tests::edge_scene(true, true);
        play_out(&mut world, &mut desktop, now, 3..240, &mut poses);
        // A leap that only just makes it: the colony gasps, a companion hauls it up, and the
        // jumper wobbles on the edge before it is pleased with itself.
        let (mut world, mut desktop, now) = super::ledges::tests::marginal_scene(true);
        play_out(&mut world, &mut desktop, now, 3..240, &mut poses);
        // A circle of dancers, each on its own beat.
        let (mut world, mut desktop, now) = super::games::tests::dance_scene();
        play_out(&mut world, &mut desktop, now, 1..260, &mut poses);
        // A toy nobody else can have.
        let (mut world, mut desktop, now) = super::games::tests::keep_away_scene();
        play_out(&mut world, &mut desktop, now, 1..300, &mut poses);
        for gesture in Gesture::ALL {
            assert!(
                poses.showed(gesture),
                "no scene ever struck {gesture:?}; between them these showed {:?}",
                poses.all()
            );
        }
    }

    /// Reduced motion is a promise that nothing will move about on its own, and a body pose is
    /// movement. The same scenes that are full of poses show none of them with the setting on.
    #[test]
    fn reduced_motion_strikes_no_pose_at_all() {
        for reduced in [false, true] {
            let mut poses = Poses::default();
            // A leap, a catch, and a companion going to help.
            let (mut world, mut desktop, now) = super::ledges::tests::marginal_scene(true);
            world.save.settings.reduce_motion = reduced;
            play_out(&mut world, &mut desktop, now, 3..200, &mut poses);
            // A copy chain, which is one of the two scenes reduced motion still allows.
            let (mut world, mut desktop, now) = super::play::tests::scene(true);
            world.save.settings.reduce_motion = reduced;
            play_out(&mut world, &mut desktop, now, 1..200, &mut poses);
            // A peek over a long drop.
            let (mut world, mut desktop, now) = super::ledges::tests::edge_scene(false, false);
            world.save.settings.reduce_motion = reduced;
            play_out(&mut world, &mut desktop, now, 3..80, &mut poses);
            assert_eq!(
                poses.all().is_empty(),
                reduced,
                "reduced motion {reduced} showed {:?}",
                poses.all()
            );
        }
    }

    /// A pose only ever stands in for a body with nothing else to do. These scenes spend most of
    /// their time travelling, leaping, hanging by the hands and falling; the recorder checks every
    /// tick of every one of them, and the tally afterwards proves they really did put the body to
    /// work rather than standing about being easy to satisfy.
    #[test]
    fn a_pose_never_stands_in_for_a_body_its_action_is_already_using() {
        let mut poses = Poses::default();
        let mut busy = Busy::default();
        // A marginal leap with a helper, where the attempt slips and both of them come off the
        // ledge: run-ups, flight, hanging by the hands, a rescue, and a fall.
        let (mut world, mut desktop, now) = super::ledges::tests::marginal_scene(true);
        let actor = world.save.creatures[0].id;
        for step in 3..230 {
            if let Some(WindowJourney::Gap(gap)) = world.window_journeys.get_mut(&actor) {
                gap.assistance_slip = true;
            }
            watch(
                &mut world,
                &mut desktop,
                now,
                step..step + 1,
                &mut poses,
                &mut busy,
            );
        }
        // Companions vaulting over one another the length of a ledge.
        let (mut world, mut desktop, now) = super::games::tests::leapfrog_scene();
        watch(&mut world, &mut desktop, now, 1..300, &mut poses, &mut busy);
        // A race across the desktop, which travels and leaps the whole way.
        let (mut world, mut desktop, now) = super::geometry_games::tests::race_scene(2);
        watch(
            &mut world,
            &mut desktop,
            now,
            80..700,
            &mut poses,
            &mut busy,
        );
        assert!(
            busy.actions.len() >= 5 && busy.hung && busy.hopped && busy.tossed,
            "the scenes were never busy enough to mean anything: {busy:?}"
        );
        assert!(!poses.all().is_empty(), "and nothing was ever posed at all");
    }

    /// A support that is simply not there any more is a fright first and a puzzle second: the
    /// creature gasps where it stood, and then gets on with looking for where the window went
    /// rather than wearing the gasp for the whole search.
    #[test]
    fn a_vanished_support_gasps_once_and_then_only_looks_around() {
        let (mut world, mut desktop, now) = scene();
        start(&mut world, &mut desktop, now);
        desktop.windows.clear();
        let mut poses = Poses::default();
        play_out(&mut world, &mut desktop, now, 11..40, &mut poses);
        let searcher = world.save.creatures[0].id;
        assert_eq!(poses.by(searcher), [Gesture::Gasp], "{poses:?}");
        assert!(
            poses.ticks(searcher, Gesture::Gasp) <= 12,
            "a gasp is a moment, not a mood: {poses:?}"
        );
        assert!(matches!(
            world.attention.plans[&searcher].role,
            Role::Actor { vanished: true, .. }
        ));
        assert_eq!(
            world.save.creatures[0]
                .state
                .attention
                .expect("still searching")
                .gesture,
            None
        );
    }

    /// Several windows shoved about at once is one piece of news, not one piece per window. The
    /// colony notices the rearrangement, reacts to it once, and settles again; nobody is
    /// interrupted afresh by each window that moved.
    #[test]
    fn a_flurry_of_window_moves_is_one_piece_of_news_rather_than_one_per_window() {
        let (mut world, mut desktop, now) = scene();
        // Three more windows beside the first, close enough together to read as one cluster.
        for (index, x) in [820.0, 900.0, 980.0].into_iter().enumerate() {
            let mut window = desktop.windows[0].clone();
            window.key = 710 + index as u64;
            window.bounds = DesktopRect {
                x,
                y: 560.0,
                width: 70.0,
                height: 240.0,
            };
            window.z_order = 1 + index as u32;
            desktop.windows.push(window);
        }
        let tick = |world: &mut World, desktop: &mut DesktopSnapshot, step: i64| {
            desktop.window_sample.as_mut().unwrap().monotonic_millis = step as u64 * 50;
            world.tick(now + Duration::milliseconds(step * 50), 0.05, desktop);
        };
        // Let the new windows stop being news before the flurry itself begins.
        for step in 1..=60 {
            tick(&mut world, &mut desktop, step);
        }
        world.clear_attention();
        let mut origins: Vec<Origin> = Vec::new();
        let mut busiest = 0;
        for step in 61..=260 {
            if (61..=76).contains(&step) {
                for window in desktop.windows.iter_mut().skip(1) {
                    window.bounds.x += 12.0;
                }
            }
            tick(&mut world, &mut desktop, step);
            for plan in world.attention.plans.values() {
                if !origins.contains(&plan.origin) {
                    origins.push(plan.origin);
                }
            }
            busiest = busiest.max(world.attention.plans.len());
        }
        assert!(!origins.is_empty(), "a rearrangement is worth noticing");
        assert!(
            origins.len() <= 2,
            "one flurry, not a scene per window that moved: {origins:?}"
        );
        assert!(
            busiest <= world.save.creatures.len(),
            "nobody is recruited twice over"
        );
        assert!(
            world.attention.plans.is_empty(),
            "and the colony settles once the desktop does"
        );
    }

    #[test]
    fn one_window_produces_distinct_riders_and_a_delayed_audience_with_one_origin() {
        let (mut world, mut desktop, now) = scene();
        start(&mut world, &mut desktop, now);
        assert_eq!(world.attention.plans.len(), 4);
        let timid = world.save.creatures[0].id;
        let bold = world.save.creatures[1].id;
        assert_eq!(
            world.attention.plans[&timid].emotion,
            AttentionEmotion::Startled
        );
        assert_eq!(
            world.attention.plans[&bold].emotion,
            AttentionEmotion::Enjoying
        );
        for creature in &world.save.creatures[2..] {
            let plan = world.attention.plans[&creature.id];
            assert!(matches!(plan.role, Role::Observer { actor } if actor == timid));
            assert_eq!(plan.origin, world.attention.plans[&timid].origin);
            assert!(creature.state.attention.is_none());
        }
        for step in 1..=24 {
            world.tick(
                now + Duration::milliseconds(250 + step * 50),
                0.05,
                &desktop,
            );
        }
        assert_eq!(
            world.save.creatures[0].state.attention.unwrap().emotion,
            AttentionEmotion::Startled
        );
        assert_eq!(
            world.save.creatures[1].state.attention.unwrap().emotion,
            AttentionEmotion::Enjoying
        );
        for creature in &world.save.creatures[2..] {
            let pose = creature.state.attention.unwrap();
            assert_eq!(pose.emotion, AttentionEmotion::Concerned);
            assert_eq!(
                pose.target,
                head_point(&world.save.creatures[0], &world.save.settings, &desktop)
            );
        }
    }

    #[test]
    fn continuous_motion_does_not_restart_reactions_and_the_audience_returns_to_life() {
        let (mut world, mut desktop, now) = scene();
        start(&mut world, &mut desktop, now);
        let origins: Vec<_> = world.attention.plans.values().map(|p| p.origin).collect();
        for step in 1..=90 {
            if step % 5 == 0 {
                desktop.window_sample.as_mut().unwrap().monotonic_millis = 250 + step * 50;
                desktop.windows[0].bounds.x += 8.0;
            }
            world.tick(
                now + Duration::milliseconds((250 + step * 50) as i64),
                0.05,
                &desktop,
            );
            if step == 20 {
                assert!(world.attention.plans.values().all(|p| p.elapsed >= 0.99));
                assert_eq!(
                    world
                        .attention
                        .plans
                        .values()
                        .map(|p| p.origin)
                        .collect::<Vec<_>>(),
                    origins
                );
            }
        }
        assert!(world.attention.plans.is_empty());
        assert!(
            world
                .save
                .creatures
                .iter()
                .all(|c| c.state.attention.is_none())
        );
        assert!(
            world
                .attention
                .cooldowns
                .values()
                .all(|remaining| *remaining < REACTION_COOLDOWN)
        );
        assert!(!world.drain_events().any(|event| matches!(
            event,
            WorldEvent::ActionCompleted {
                action: ActionKind::InspectScreen,
                ..
            }
        )));
    }

    #[test]
    fn interaction_cancels_actor_and_its_observers_before_the_next_tick() {
        let (mut world, mut desktop, now) = scene();
        start(&mut world, &mut desktop, now);
        let id = world.save.creatures[0].id;
        let cursor = world.save.creatures[0].state.position;
        assert!(world.handle_command(
            WorldCommand::BeginInteraction {
                creature_id: id,
                cursor
            },
            &desktop
        ));
        assert!(!world.attention.owns(id));
        assert!(world.save.creatures[0].state.attention.is_none());
        assert!(
            world.save.creatures[2..]
                .iter()
                .all(|c| !world.attention.owns(c.id))
        );
        assert!(world.handle_command(WorldCommand::CancelInteraction, &desktop));
        assert_ne!(world.save.creatures[0].state.action_duration, f32::MAX);
    }

    #[test]
    fn pause_hide_home_and_unreliable_scans_cancel_without_a_replay() {
        for reason in 0..4 {
            let (mut world, mut desktop, now) = scene();
            start(&mut world, &mut desktop, now);
            match reason {
                0 => world.save.settings.paused = true,
                1 => world.save.settings.visible = false,
                2 => world.set_quiet_mode(15, now),
                _ => desktop.window_sample.as_mut().unwrap().reliable = false,
            }
            world.tick(now + Duration::milliseconds(300), 0.05, &desktop);
            assert!(world.attention.plans.is_empty());
            assert!(
                world
                    .save
                    .creatures
                    .iter()
                    .all(|c| c.state.attention.is_none())
            );
            world.save.settings.paused = false;
            world.save.settings.visible = true;
            world.save.companion.quiet_until = None;
            super::super::tests::let_colony_wander(&mut world, now);
            desktop.window_sample = Some(WindowSample {
                monotonic_millis: 500,
                reliable: true,
            });
            world.tick(now + Duration::milliseconds(500), 0.05, &desktop);
            assert!(world.attention.plans.is_empty());
        }
    }

    #[test]
    fn vanished_support_recovers_safely_and_cancels_watchers() {
        let (mut world, mut desktop, now) = scene();
        start(&mut world, &mut desktop, now);
        desktop.windows.clear();
        desktop.window_sample.as_mut().unwrap().monotonic_millis = 500;
        world.tick(now + Duration::milliseconds(500), 0.05, &desktop);
        assert!(world.attention.plans.is_empty());
        assert!(
            world
                .save
                .creatures
                .iter()
                .all(|c| c.state.attention.is_none())
        );
        for c in &world.save.creatures[..2] {
            assert_eq!(c.state.surface.kind, SurfaceKind::ScreenFloor);
            assert_eq!(c.state.action, ActionKind::ReactToWindow);
            assert!(habitat_contains(
                &world.save.settings.habitat,
                &desktop.monitors[0],
                c.state.position
            ));
        }
    }

    #[test]
    fn poses_are_not_saved_and_reduced_motion_keeps_calm_stationary_gaze() {
        let (mut world, mut desktop, now) = scene();
        world.save.settings.reduce_motion = true;
        start(&mut world, &mut desktop, now);
        for step in 1..=24 {
            world.tick(now + Duration::milliseconds(step * 50), 0.05, &desktop);
        }
        assert!(
            world
                .save
                .creatures
                .iter()
                .all(|c| c.state.action == ActionKind::InspectScreen)
        );
        assert!(
            world
                .save
                .creatures
                .iter()
                .all(|c| c.state.attention.is_some())
        );
        let json = serde_json::to_string(&world.save).unwrap();
        assert!(!json.contains("attention"));
        assert!(!json.contains("\"target\""));
        assert!(
            !serde_json::to_string(&desktop)
                .unwrap()
                .contains("window_sample")
        );
        assert!(
            !serde_json::to_string(&desktop)
                .unwrap()
                .contains("cursor_sample_millis")
        );
        let restored = World::from_save(serde_json::from_str(&json).unwrap());
        assert!(
            restored
                .save
                .creatures
                .iter()
                .all(|c| c.state.attention.is_none())
        );
        assert!(restored.attention.plans.is_empty());
    }

    #[test]
    fn sleeping_distant_occluded_and_uninterested_companions_are_not_recruited() {
        for reason in 0..4 {
            let (mut world, mut desktop, now) = scene();
            let observer = world.save.creatures[2].id;
            match reason {
                0 => world.save.creatures[2].state.action = ActionKind::Sleep,
                1 => world.save.creatures[2].state.position.x = 1_350.0,
                2 => {
                    desktop.windows[0].z_order = 1;
                    desktop.windows.push(DesktopWindow {
                        key: 702,
                        bounds: DesktopRect {
                            x: 560.0,
                            y: 780.0,
                            width: 140.0,
                            height: 100.0,
                        },
                        z_order: 0,
                        visible: true,
                        minimized: false,
                        application: None,
                        application_name: None,
                    });
                }
                _ => {
                    world.save.creatures[2].personality.curiosity = 0.0;
                    world.save.creatures[2].personality.sociability = 0.0;
                    for bond in &mut world.save.relationships {
                        if bond.a == observer || bond.b == observer {
                            bond.affinity = 0;
                        }
                    }
                }
            }
            start(&mut world, &mut desktop, now);
            assert!(
                !world.attention.owns(observer),
                "ineligible observer, reason {reason}"
            );
            assert!(
                world
                    .attention
                    .plans
                    .values()
                    .any(|plan| matches!(plan.role, Role::Actor { .. }))
            );
        }
    }
    pub(super) fn open_scene() -> (World, DesktopSnapshot, OffsetDateTime) {
        let (mut world, mut desktop, now) = scene();
        desktop.windows.clear();
        world.clear_attention();
        world.geometry_observer = crate::attention::GeometryObserver::default();
        world.geometry_observer.update(&desktop, 0.05, true);
        for (index, creature) in world.save.creatures.iter_mut().enumerate() {
            creature.state.surface = SurfaceAttachment {
                kind: SurfaceKind::ScreenFloor,
                monitor_id: 1,
                window_key: None,
                relative_x: 0.5,
            };
            creature.state.position = Point {
                x: 480.0 - index as f32 * 65.0,
                y: 846.0,
            };
            creature.personality.activity = 0.7;
            creature.state.action = ActionKind::Idle;
            creature.state.action_duration = 100.0;
            if index > 0 {
                creature.personality.curiosity = 0.7;
            }
        }
        (world, desktop, now)
    }

    fn new_window(world: &mut World, desktop: &mut DesktopSnapshot, now: OffsetDateTime) {
        desktop.windows.push(DesktopWindow {
            key: 702,
            bounds: DesktopRect {
                x: 580.0,
                y: 650.0,
                width: 240.0,
                height: 150.0,
            },
            z_order: 0,
            visible: true,
            minimized: false,
            application: None,
            application_name: None,
        });
        desktop.window_sample.as_mut().unwrap().monotonic_millis = 250;
        world.tick(now + Duration::milliseconds(250), 0.05, desktop);
    }

    #[test]
    fn new_edge_gets_a_short_approach_and_companions_reserve_distinct_viewing_spots() {
        let (mut world, mut desktop, now) = open_scene();
        new_window(&mut world, &mut desktop, now);
        let actor = world.save.creatures[0].id;
        let observer = world.save.creatures[1].id;
        let destination = world.attention.plans[&actor].walk.unwrap().destination;
        let viewing_spot = world.attention.plans[&observer].walk.unwrap().destination;
        assert!(destination.x > 480.0 && destination.x < 580.0);
        assert!(viewing_spot.x > 415.0 && viewing_spot.x < destination.x - 24.0);
        assert_eq!(world.save.creatures[0].state.position.x, 480.0); // Notice first.
        for step in 1..=120 {
            world.tick(
                now + Duration::milliseconds(250 + step * 50),
                0.05,
                &desktop,
            );
            let a = &world.save.creatures[0];
            let b = &world.save.creatures[1];
            assert!(a.state.position.distance(b.state.position) >= 24.0);
            assert_eq!(a.state.surface.window_key, None);
            if step == 20 {
                assert!(a.state.position.x > 480.0);
                assert_eq!(a.state.action, ActionKind::Traverse);
                assert!(a.state.velocity.x > 0.0);
                assert!(b.state.position.x > 415.0);
                assert!(
                    matches!(world.attention.plans[&observer].role, Role::Observer { actor: id } if id == actor)
                );
            }
            if step == 65 {
                assert!((a.state.position.x - destination.x).abs() < 1.0);
                assert_eq!(a.state.action, ActionKind::InspectScreen);
                assert!(a.state.attention.is_some());
            }
        }
        assert!(world.attention.plans.is_empty());
        assert!(
            world
                .save
                .creatures
                .iter()
                .all(|c| c.state.attention.is_none())
        );
    }

    #[test]
    fn changed_geometry_or_a_blocking_companion_ends_an_approach_at_a_safe_point() {
        for blocked_by_creature in [false, true] {
            let (mut world, mut desktop, now) = open_scene();
            new_window(&mut world, &mut desktop, now);
            for step in 1..=20 {
                world.tick(
                    now + Duration::milliseconds(250 + step * 50),
                    0.05,
                    &desktop,
                );
            }
            let id = world.save.creatures[0].id;
            let before = world.save.creatures[0].state.position;
            if blocked_by_creature {
                let other = world.save.creatures[1].id;
                world.cancel_creature_attention(other);
                world.save.creatures[1].state.position.x = before.x + 24.0;
                world.save.creatures[1].state.action = ActionKind::Sleep;
                world.save.creatures[1].state.action_duration = 100.0;
            } else {
                desktop.windows[0].bounds.x += 60.0;
                desktop.window_sample.as_mut().unwrap().monotonic_millis = 1_500;
            }
            world.tick(now + Duration::milliseconds(1_500), 0.05, &desktop);
            assert!(world.attention.plans[&id].walk.is_none());
            assert_eq!(world.save.creatures[0].state.position, before);
            assert_eq!(world.save.creatures[0].state.velocity, Point::default());
            assert_eq!(
                world.save.creatures[0].state.action,
                ActionKind::InspectScreen
            );
        }
    }

    #[test]
    fn approaches_respect_excluded_strips_and_reduced_motion() {
        for reduced_motion in [false, true] {
            let (mut world, mut desktop, now) = open_scene();
            world.save.settings.reduce_motion = reduced_motion;
            if !reduced_motion {
                world.save.settings.habitat.zones.push(HabitatZone {
                    id: 991,
                    display: desktop.monitors[0].display_key,
                    kind: HabitatZoneKind::Excluded,
                    enabled: true,
                    normalized_bounds: DesktopRect {
                        x: 510.0 / 1440.0,
                        y: 0.0,
                        width: 24.0 / 1440.0,
                        height: 1.0,
                    },
                });
            }
            new_window(&mut world, &mut desktop, now);
            let actor = world.save.creatures[0].id;
            assert!(world.attention.plans[&actor].walk.is_none());
            for step in 1..=20 {
                world.tick(
                    now + Duration::milliseconds(250 + step * 50),
                    0.05,
                    &desktop,
                );
            }
            assert_eq!(world.save.creatures[0].state.position.x, 480.0);
            assert!(world.save.creatures[0].state.attention.is_some());
        }
    }

    #[test]
    fn growth_makes_a_timid_neighbor_retreat_while_a_bold_one_inspects() {
        for bold in [false, true] {
            let (mut world, mut desktop, now) = open_scene();
            new_window(&mut world, &mut desktop, now);
            world.clear_attention();
            world.save.creatures.truncate(1);
            let creature = &mut world.save.creatures[0];
            creature.personality.boldness = f32::from(bold);
            creature.personality.window_tolerance = f32::from(bold);
            creature.state.position.x = 500.0;
            desktop.windows[0].bounds.width += 180.0;
            desktop.window_sample.as_mut().unwrap().monotonic_millis = 500;
            world.tick(now + Duration::milliseconds(500), 0.05, &desktop);
            let id = world.save.creatures[0].id;
            assert_eq!(
                world.attention.plans[&id].emotion,
                if bold {
                    AttentionEmotion::Curious
                } else {
                    AttentionEmotion::Startled
                }
            );
            for step in 1..=25 {
                world.tick(
                    now + Duration::milliseconds(500 + step * 50),
                    0.05,
                    &desktop,
                );
            }
            let creature = &world.save.creatures[0];
            if bold {
                assert_eq!(creature.state.position.x, 500.0);
            } else {
                assert!(creature.state.position.x < 480.0);
            }
            assert_eq!(creature.state.surface.kind, SurfaceKind::ScreenFloor);
        }
    }

    #[test]
    fn recently_lost_support_prompts_one_search_even_during_the_ride_cooldown() {
        let (mut world, mut desktop, now) = scene();
        start(&mut world, &mut desktop, now);
        let actor = world.save.creatures[0].id;
        let old_origin = world.attention.plans[&actor].origin;
        desktop.windows.clear();
        desktop.window_sample.as_mut().unwrap().monotonic_millis = 500;
        world.tick(now + Duration::milliseconds(500), 0.05, &desktop);
        assert!(!world.attention.owns(actor));
        desktop.window_sample.as_mut().unwrap().monotonic_millis = 750;
        world.tick(now + Duration::milliseconds(750), 0.05, &desktop);
        assert!(world.attention.cooldowns.contains_key(&actor));
        assert!(matches!(
            world.attention.plans[&actor].role,
            Role::Actor { vanished: true, .. }
        ));
        assert_ne!(world.attention.plans[&actor].origin, old_origin);
        let mut glances = Vec::new();
        for step in 1..=80 {
            world.tick(
                now + Duration::milliseconds(750 + step * 50),
                0.05,
                &desktop,
            );
            if [12, 25, 38].contains(&step) {
                glances.push(world.save.creatures[0].state.attention.unwrap().target.x);
            }
        }
        assert_ne!(glances[0], glances[1]);
        assert_eq!(glances[0], glances[2]);
        assert!(world.attention.plans.is_empty());
        assert!(world.save.creatures[0].state.attention.is_none());
    }
    /// S10: the same scene with two, three, and four creatures, through both outcomes.
    #[test]
    fn spectator_sequences_stay_readable_at_every_colony_size_and_outcome() {
        for colony in 2..=4 {
            for bold in [false, true] {
                let (mut world, mut desktop, now) = super::ledges::tests::edge_scene(true, bold);
                world.save.creatures.truncate(colony);
                let actor = world.save.creatures[0].id;
                let mut seen: BTreeMap<CreatureId, Vec<AttentionEmotion>> = BTreeMap::new();
                let mut celebrated = false;
                for step in 3..200 {
                    desktop.window_sample.as_mut().unwrap().monotonic_millis = step * 50;
                    world.tick(
                        now + Duration::milliseconds(step as i64 * 50),
                        0.05,
                        &desktop,
                    );
                    for creature in world.save.creatures.iter().skip(1) {
                        let watching = world
                            .attention
                            .plans
                            .get(&creature.id)
                            .is_some_and(|p| matches!(p.role, Role::Observer { .. }));
                        if let Some(pose) = creature.state.attention.filter(|_| watching) {
                            let log = seen.entry(creature.id).or_default();
                            if log.last() != Some(&pose.emotion) {
                                log.push(pose.emotion);
                            }
                            celebrated |= creature.state.action == ActionKind::Greet;
                        }
                    }
                }
                let label = format!("colony {colony}, bold {bold}");
                assert!(!seen.is_empty(), "somebody watches: {label}");
                for log in seen.values() {
                    assert_eq!(log[0], AttentionEmotion::Curious, "notice first: {label}");
                    assert!(log.len() >= 2, "a response follows: {label}");
                    assert!(
                        log.iter().any(|e| matches!(
                            e,
                            AttentionEmotion::Concerned
                                | AttentionEmotion::Averting
                                | AttentionEmotion::Startled
                                | AttentionEmotion::Enjoying
                        )),
                        "a felt response: {label}"
                    );
                    // Everyone ends calm; only a playful watcher turns that into delight.
                    assert!(
                        matches!(
                            log.last(),
                            Some(AttentionEmotion::Relieved | AttentionEmotion::Enjoying)
                        ),
                        "settles at the end: {label}"
                    );
                }
                let delighted = seen.iter().any(|(id, log)| {
                    log.last() == Some(&AttentionEmotion::Enjoying)
                        && world
                            .save
                            .creatures
                            .iter()
                            .any(|c| c.id == *id && c.personality.playfulness > 0.65)
                });
                assert_eq!(delighted, bold, "delight follows a real success: {label}");
                // Only a real success is celebrated, and nobody keeps watching forever.
                assert_eq!(celebrated, bold, "celebration matches outcome: {label}");
                assert!(world.attention.plans.is_empty(), "{label}");
                assert!(
                    world
                        .save
                        .creatures
                        .iter()
                        .all(|c| c.state.attention.is_none()),
                    "{label}"
                );
                assert_eq!(
                    world.save.creatures[0].state.surface.window_key,
                    Some(if bold { 702 } else { 701 }),
                    "{label}"
                );
                let _ = actor;
            }
        }
    }

    #[test]
    fn an_actor_that_becomes_hidden_loses_its_audience() {
        let (mut world, mut desktop, now) = super::ledges::tests::edge_scene(true, true);
        let actor = world.save.creatures[0].id;
        let mut watched = false;
        for step in 3..40 {
            desktop.window_sample.as_mut().unwrap().monotonic_millis = step * 50;
            world.tick(
                now + Duration::milliseconds(step as i64 * 50),
                0.05,
                &desktop,
            );
            watched |=
                world.attention.plans.values().any(
                    |p| matches!(p.role, Role::Observer { actor: watched } if watched == actor),
                );
            if watched {
                break;
            }
        }
        assert!(watched, "the attempt gathers an audience first");
        // A window slides in front of the actor: its watchers cannot see it any more.
        desktop.windows[0].z_order = 3;
        desktop.windows[1].z_order = 4;
        desktop.windows.push(DesktopWindow {
            key: 703,
            bounds: DesktopRect {
                x: 700.0,
                y: 500.0,
                width: 240.0,
                height: 220.0,
            },
            z_order: 0,
            visible: true,
            minimized: false,
            application: None,
            application_name: None,
        });
        desktop.window_sample.as_mut().unwrap().monotonic_millis = 2_000;
        world.tick(now + Duration::milliseconds(2_000), 0.05, &desktop);
        assert!(
            !world
                .attention
                .plans
                .values()
                .any(|p| matches!(p.role, Role::Observer { .. })),
            "a hidden actor is not watched"
        );
        assert!(world.window_journeys.is_empty());
    }

    #[test]
    fn attention_walks_keep_clear_of_an_existing_landing_reservation() {
        let (mut world, mut desktop, now) = open_scene();
        let jumper = world.save.creatures[3].id;
        world.window_journeys.insert(
            jumper,
            WindowJourney::Hop(HopJourney {
                start: Point { x: 100.0, y: 600.0 },
                target: Point { x: 556.0, y: 846.0 },
                surface: world.save.creatures[0].state.surface.clone(),
                elapsed: 0.0,
                duration: 3.0,
            }),
        );
        new_window(&mut world, &mut desktop, now);
        let id = world.save.creatures[0].id;
        assert!(world.attention.plans[&id].walk.is_none());
        assert_eq!(world.save.creatures[0].state.position.x, 480.0);
    }

    #[test]
    fn an_approach_on_a_ledge_cancels_immediately_when_that_support_vanishes() {
        let (mut world, mut desktop, now) = open_scene();
        desktop.windows.push(DesktopWindow {
            key: 701,
            bounds: DesktopRect {
                x: 200.0,
                y: 600.0,
                width: 600.0,
                height: 200.0,
            },
            z_order: 1,
            visible: true,
            minimized: false,
            application: None,
            application_name: None,
        });
        let creature = &mut world.save.creatures[0];
        creature.state.surface = SurfaceAttachment {
            monitor_id: 1,
            window_key: Some(701),
            kind: SurfaceKind::WindowLedge,
            relative_x: (480.0 - 200.0) / 600.0,
        };
        creature.state.position.y = 600.0;
        world.geometry_observer = crate::attention::GeometryObserver::default();
        world.geometry_observer.update(&desktop, 0.05, true);
        new_window(&mut world, &mut desktop, now);
        let id = world.save.creatures[0].id;
        assert!(world.attention.plans[&id].walk.is_some());
        for step in 1..=20 {
            world.tick(
                now + Duration::milliseconds(250 + step * 50),
                0.05,
                &desktop,
            );
        }
        assert!(world.save.creatures[0].state.position.x > 480.0);
        desktop.windows.retain(|w| w.key != 701);
        desktop.window_sample.as_mut().unwrap().monotonic_millis = 1_500;
        world.tick(now + Duration::milliseconds(1_500), 0.05, &desktop);
        assert!(!world.attention.owns(id));
        assert_eq!(
            world.save.creatures[0].state.surface.kind,
            SurfaceKind::ScreenFloor
        );
        assert!(world.save.creatures[0].state.attention.is_none());
    }
}
