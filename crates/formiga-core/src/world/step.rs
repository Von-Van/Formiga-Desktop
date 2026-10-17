//! One creature's part of a tick, step by step: what holds it still, its drives, a journey across
//! windows, what it can see, choosing what to do next and setting out on it, and doing it.
//! [`World::tick`] runs these for each creature in turn, in this order; each step reads and
//! changes the world exactly as the same lines did inline, so a colony lives the same life.

use super::*;

/// What every creature's step shares within one tick.
#[derive(Clone, Copy)]
pub(super) struct TickShared<'a> {
    pub(super) now: OffsetDateTime,
    pub(super) timeline_now: OffsetDateTime,
    pub(super) dt: f32,
    pub(super) desktop: &'a DesktopSnapshot,
    pub(super) cottages: &'a [DwellingKind],
    pub(super) finds_allowed: bool,
    pub(super) night: bool,
}

/// The next action as it was picked, before the creature sets out on it.
struct Picked {
    old: ActionKind,
    old_elapsed: f32,
    choice: ActionChoice,
    explicit_experience: Option<RelationshipExperience>,
    continuing_plan: bool,
    scheduled_ambient: bool,
}

impl World {
    /// The creature at `index` in the colony, for one tick.
    pub(super) fn step_creature(&mut self, index: usize, shared: &TickShared) {
        if self.held_this_tick(index, shared) {
            return;
        }
        let Some(frame_width) = self.advance_drives(index, shared) else {
            return;
        };
        if self.advance_window_journey(index, shared) {
            return;
        }
        let (context, nearest) = self.behavior_context(index, shared);
        self.react_to_tossed_friend(index, context);
        let creature = &self.save.creatures[index];
        if creature.state.action_elapsed >= creature.state.action_duration {
            let picked = self.pick_next_action(index, shared, context, frame_width);
            self.set_out(index, shared, picked, frame_width);
        }
        self.carry_out_action(index, shared, context, nearest, frame_width);
    }

    /// Whether something other than its own choices has the creature this tick: being handled, a
    /// wonder or a house elsewhere, arriving, or flying through the air after a toss.
    fn held_this_tick(&mut self, index: usize, shared: &TickShared) -> bool {
        let TickShared {
            timeline_now,
            dt,
            desktop,
            ..
        } = *shared;
        let creature = &mut self.save.creatures[index];
        if self
            .interaction
            .as_ref()
            .is_some_and(|interaction| interaction.creature_id == creature.id)
        {
            return true;
        }
        // A wonder's players are its own until it is over, and a house open elsewhere has its
        // household until it closes.
        if self.wonders.owns(creature.id) || self.house_visit.contains(&creature.id) {
            return true;
        }
        if creature.state.arrival_delay_secs > 0.0 {
            let previous_delay = creature.state.arrival_delay_secs;
            creature.state.arrival_delay_secs = (previous_delay - dt).max(0.0);
            if creature.state.arrival_delay_secs == 0.0 {
                creature.born_at_utc = timeline_now;
                Self::emit(
                    &mut self.events,
                    WorldEvent::CreatureSpawned {
                        creature_id: creature.id,
                    },
                );
            }
            return true;
        }
        if self.tosses.contains_key(&creature.id) {
            let landing = {
                let toss = self
                    .tosses
                    .get_mut(&creature.id)
                    .expect("known toss exists");
                advance_toss(
                    creature,
                    toss,
                    dt,
                    desktop,
                    &self.save.settings.habitat,
                    self.save.settings.reduce_motion,
                    self.save.settings.window_ledges,
                    self.save.settings.display_scale,
                )
            };
            if let Some((surface, bounced)) = landing {
                self.tosses.remove(&creature.id);
                bubbles::show(&mut self.bubbles, creature.id, BubbleIcon::Dizzy);
                Self::emit(
                    &mut self.events,
                    WorldEvent::TossLanded {
                        creature_id: creature.id,
                        surface: surface.kind,
                        bounced,
                    },
                );
                Self::emit(
                    &mut self.events,
                    WorldEvent::SurfaceChanged {
                        creature_id: creature.id,
                        kind: surface.kind,
                    },
                );
            }
            return true;
        }
        false
    }

    /// Its drives and timers move on. `None` when something it is part of (a beat, a scene, a
    /// tow) has it this tick; otherwise how wide it draws where it stands.
    fn advance_drives(&mut self, index: usize, shared: &TickShared) -> Option<f32> {
        let TickShared { dt, desktop, .. } = *shared;
        let creature = &mut self.save.creatures[index];
        // How wide this creature draws where it stands: every bond mark is measured in it,
        // so companions keep the same distance at every display scale.
        let frame_width =
            spacing::creature_frame_width(creature, self.save.settings.display_scale, desktop);
        update_drives(creature, dt);
        creature.state.cursor_cooldown = (creature.state.cursor_cooldown - dt).max(0.0);
        // A yawn, or a look at a friend yawning, is had standing still, and whatever it was
        // doing waits for it rather than running out underneath it.
        if creature.state.beat.is_some()
            && !self.window_journeys.contains_key(&creature.id)
            && !self.attention.owns(creature.id)
        {
            creature.state.velocity = Point::default();
            return None;
        }
        creature.state.action_elapsed += dt;
        if creature.state.action == ActionKind::Sleep {
            *self.sleep_elapsed.entry(creature.id).or_default() += dt;
        }

        if self.attention.owns(creature.id) {
            if !self.attention.crosses_displays(creature.id) {
                constrain_to_surface(
                    creature,
                    desktop,
                    &self.save.settings.habitat,
                    self.save.settings.display_scale,
                );
            }
            return None;
        }
        // A friend towing a sleeper is walked by the tow until it lets go.
        if self.tows.towing(creature.id) {
            return None;
        }
        Some(frame_width)
    }

    /// One step of a journey across windows, when it is on one. Returns whether it was.
    fn advance_window_journey(&mut self, index: usize, shared: &TickShared) -> bool {
        let TickShared { dt, desktop, .. } = *shared;
        let creature_views = &self.creature_views;
        let creature = &mut self.save.creatures[index];
        if self.window_journeys.contains_key(&creature.id) {
            let valid = self
                .window_journeys
                .get(&creature.id)
                .is_some_and(|journey| journey.valid(desktop));
            if !valid {
                self.window_journeys.remove(&creature.id);
                self.window_routes.remove(&creature.id);
                settle_interrupted_journey(
                    creature,
                    desktop,
                    &self.save.settings.habitat,
                    self.save.settings.display_scale,
                    &mut self.events,
                );
                return true;
            }
            let (step, surface) = {
                let journey = self
                    .window_journeys
                    .get_mut(&creature.id)
                    .expect("validated journey exists");
                (journey.advance(dt), journey.surface().clone())
            };
            let route_point_valid = desktop.monitors.iter().any(|monitor| {
                monitor.bounds.contains(step.position)
                    && habitat_contains(&self.save.settings.habitat, monitor, step.position)
            });
            let gap_step_valid = self
                .window_journeys
                .get(&creature.id)
                .is_none_or(|journey| {
                    attention::gap_step_safe(
                        journey,
                        creature,
                        step.position,
                        desktop,
                        &self.save.settings,
                        creature_views,
                    )
                });
            if !route_point_valid || !gap_step_valid {
                self.window_journeys.remove(&creature.id);
                self.window_routes.remove(&creature.id);
                settle_interrupted_journey(
                    creature,
                    desktop,
                    &self.save.settings.habitat,
                    self.save.settings.display_scale,
                    &mut self.events,
                );
                return true;
            }
            let previous_action = creature.state.action;
            if self.window_journeys.get(&creature.id).is_some_and(|j| {
                matches!(j, WindowJourney::Gap(gap)
                if gap.catch && gap.elapsed >= gap.preparation + gap.hop.duration)
            }) {
                creature.state.surface = surface.clone();
            }
            creature.state.facing_right = step.position.x >= creature.state.position.x;
            creature.state.position = step.position;
            if step.action != previous_action {
                Self::emit(
                    &mut self.events,
                    WorldEvent::ActionCompleted {
                        creature_id: creature.id,
                        action: previous_action,
                    },
                );
                creature.state.action = step.action;
                creature.state.action_elapsed = 0.0;
                creature.state.action_duration = f32::MAX;
                Self::emit(
                    &mut self.events,
                    WorldEvent::ActionStarted {
                        creature_id: creature.id,
                        action: step.action,
                    },
                );
            }
            if step.complete {
                self.window_journeys.remove(&creature.id);
                let completed_action = creature.state.action;
                creature.state.surface = surface.clone();
                Self::emit(
                    &mut self.events,
                    WorldEvent::ActionCompleted {
                        creature_id: creature.id,
                        action: completed_action,
                    },
                );
                Self::emit(
                    &mut self.events,
                    WorldEvent::SurfaceChanged {
                        creature_id: creature.id,
                        kind: surface.kind,
                    },
                );
                let next_hop = self
                    .window_routes
                    .get_mut(&creature.id)
                    .filter(|plan| plan.geometry_hash == self.topology.geometry_hash())
                    .and_then(|plan| plan.remaining.pop_front());
                if let Some(hop) = next_hop {
                    let mut journey = build_route_hop_journey(creature, hop, desktop);
                    // Look at the next step before taking it, so an ascent reads as a staircase.
                    if let WindowJourney::Climb(climb) = &mut journey {
                        climb.elapsed = -0.3;
                    }
                    let next = journey.initial_action();
                    creature.state.action = next;
                    creature.state.action_elapsed = 0.0;
                    creature.state.action_duration = f32::MAX;
                    creature.state.velocity = Point::default();
                    self.window_journeys.insert(creature.id, journey);
                    Self::emit(
                        &mut self.events,
                        WorldEvent::ActionStarted {
                            creature_id: creature.id,
                            action: next,
                        },
                    );
                    return true;
                }
                self.window_routes.remove(&creature.id);
                creature.state.action = if surface.kind == SurfaceKind::ScreenFloor {
                    ActionKind::Idle
                } else {
                    ActionKind::Perch
                };
                creature.state.action_elapsed = 0.0;
                creature.state.action_duration = 3.5;
                creature.state.velocity = Point::default();
                Self::emit(
                    &mut self.events,
                    WorldEvent::ActionStarted {
                        creature_id: creature.id,
                        action: creature.state.action,
                    },
                );
            }
            return true;
        }
        false
    }

    /// What the creature can see and feel just now, as its choices read it, and the companion
    /// nearest it.
    fn behavior_context(
        &mut self,
        index: usize,
        shared: &TickShared,
    ) -> (BehaviorContext, Option<(f32, Point, CreatureId)>) {
        let TickShared {
            now,
            desktop,
            cottages,
            night,
            ..
        } = *shared;
        let creature_views = &self.creature_views;
        let relationship_views = &self.relationship_views;
        let creature = &mut self.save.creatures[index];
        let nearest = creature_views
            .iter()
            .filter(|other| other.id != creature.id && other.state.arrival_delay_secs <= 0.0)
            .map(|other| {
                (
                    creature.state.position.distance(other.state.position),
                    other.state.position,
                    other.id,
                )
            })
            .min_by(|a, b| a.0.total_cmp(&b.0));
        let context = BehaviorContext {
            cursor_safe: self.cursor_observer.safe && self.save.settings.cursor_reactions,
            ambience: self
                .geometry_observer
                .ambience(creature.state.surface.monitor_id),
            nearest_creature_distance: nearest.map(|item| item.0),
            nearest_creature_position: nearest.map(|item| item.1),
            nearest_creature_id: nearest.map(|item| item.2),
            bond: preferred_bond_context(creature, creature_views, relationship_views),
            on_window_ledge: creature.state.surface.kind == SurfaceKind::WindowLedge,
            // Filled in where the action is chosen, which is this field's only reader.
            // Searching the desktop for a ledge is the most expensive question in this
            // context, and an action already under way must not pay for it every tick.
            reachable_window_ledge: false,
            // Geometry attention owns interruptions and cooldowns; ordinary utility choices
            // must not independently restart the same event on every window scan.
            window_changed_nearby: false,
            objects: nearby_object_utility(
                creature,
                &self.save.objects.objects,
                cottages,
                desktop,
                &self.save.settings.habitat,
                &self.save.home,
                self.save.settings.display_scale,
            ),
            hour_utc: now.hour(),
            night,
            // Filled in where the action is chosen, like the ledge search: only a homebody
            // setting out reads it.
            home_point: None,
        };
        (context, nearest)
    }

    /// A friend who has just been tossed is gone to see about.
    fn react_to_tossed_friend(&mut self, index: usize, context: BehaviorContext) {
        let creature = &mut self.save.creatures[index];
        if let Some(bond) = context.bond
            && bond.target_action == ActionKind::Tossed
            && bond.relationship.affinity >= 128
            && bond.relationship.avoidance < 192
            && self
                .reacted_to_toss
                .insert((creature.id, bond.target_creature))
        {
            let interrupted = creature.state.action;
            if interrupted == ActionKind::Sleep {
                let elapsed_seconds = self
                    .sleep_elapsed
                    .remove(&creature.id)
                    .unwrap_or(creature.state.action_elapsed)
                    .max(0.0) as u32;
                Self::emit(
                    &mut self.events,
                    WorldEvent::SleepInterrupted {
                        creature_id: creature.id,
                        elapsed_seconds,
                    },
                );
            }
            Self::emit(
                &mut self.events,
                WorldEvent::ActionCompleted {
                    creature_id: creature.id,
                    action: interrupted,
                },
            );
            creature.state.action = ActionKind::ReactToWindow;
            creature.state.action_elapsed = 0.0;
            creature.state.action_duration = 2.2;
            creature.state.drives.arousal = (creature.state.drives.arousal + 0.25).min(1.0);
            self.action_choices.insert(
                creature.id,
                ActionChoice {
                    action: ActionKind::ReactToWindow,
                    target_creature: Some(bond.target_creature),
                    target_point: Some(bond.target_position),
                },
            );
            self.bond_plans.insert(
                creature.id,
                BondPlan {
                    target: bond.target_creature,
                    final_action: ActionKind::ReactToWindow,
                    experience: RelationshipExperience::ConcernedAfterToss,
                    approaching: false,
                },
            );
            Self::emit(
                &mut self.events,
                WorldEvent::ActionStarted {
                    creature_id: creature.id,
                    action: ActionKind::ReactToWindow,
                },
            );
        }
    }

    /// Its action is over: what it does next, by the priority list a finished action is weighed
    /// against, from a bond plan under way down to its own choice.
    fn pick_next_action(
        &mut self,
        index: usize,
        shared: &TickShared,
        context: BehaviorContext,
        frame_width: f32,
    ) -> Picked {
        let TickShared {
            now,
            desktop,
            cottages,
            finds_allowed,
            ..
        } = *shared;
        let creature_views = &self.creature_views;
        let relationship_views = &self.relationship_views;
        let creature = &mut self.save.creatures[index];
        let old = creature.state.action;
        let old_elapsed = creature.state.action_elapsed;
        if old == ActionKind::InvestigateCursor {
            creature.state.cursor_cooldown = creature.state.cursor_cooldown.max(5.0);
        }
        Self::emit(
            &mut self.events,
            WorldEvent::ActionCompleted {
                creature_id: creature.id,
                action: old,
            },
        );
        let mut selected_choice = None;
        let mut explicit_experience = None;
        let mut continuing_plan = false;
        if let Some(plan) = self.bond_plans.get(&creature.id).copied() {
            if plan.approaching && old == ActionKind::Follow {
                if let Some(target_point) = bond_target_point(
                    creature,
                    creature_views,
                    plan.target,
                    plan.final_action,
                    frame_width,
                ) {
                    selected_choice = Some(ActionChoice {
                        action: plan.final_action,
                        target_creature: Some(plan.target),
                        target_point: Some(target_point),
                    });
                    self.bond_plans.insert(
                        creature.id,
                        BondPlan {
                            approaching: false,
                            ..plan
                        },
                    );
                    continuing_plan = true;
                } else {
                    self.bond_plans.remove(&creature.id);
                }
            } else if !plan.approaching && old == plan.final_action {
                Self::emit(
                    &mut self.events,
                    WorldEvent::BondInteraction {
                        a: creature.id,
                        b: plan.target,
                        experience: plan.experience,
                    },
                );
                self.bond_plans.remove(&creature.id);
            } else {
                self.bond_plans.remove(&creature.id);
            }
        }
        let rng = self
            .rngs
            .get_mut(&creature.id)
            .expect("creature RNG exists");
        let discovery_available = self.save.settings.visible
            && finds_allowed
            && self.discovery_remaining <= 0.0
            && !creature_views.iter().any(|other| {
                other.id != creature.id && other.state.action == ActionKind::PresentDiscovery
            });
        let dangle_available = self.save.settings.visible
            && context.on_window_ledge
            && self
                .ambient_timers
                .get(&creature.id)
                .is_some_and(|timers| timers.dangle_remaining <= 0.0);
        let mut scheduled_ambient = continuing_plan;
        if selected_choice.is_none()
            && self.pending_home_greetings.remove(&creature.id)
            && let Some(bond) = context.bond
            && bond.relationship.avoidance < 160
            && bond_target_point(
                creature,
                creature_views,
                bond.target_creature,
                ActionKind::Greet,
                frame_width,
            )
            .is_some()
        {
            selected_choice = Some(ActionChoice {
                action: ActionKind::Greet,
                target_creature: Some(bond.target_creature),
                target_point: Some(bond.target_position),
            });
            explicit_experience = Some(RelationshipExperience::HomecomingGreeting);
        }
        if selected_choice.is_none()
            && let Some(bond) = context.bond
            && bond.target_action == ActionKind::ClimbWindow
            && bond.relationship.familiarity >= 64
            && bond.relationship.avoidance < 160
            && rng.random_ratio(1, 2)
            && self
                .watched_climb
                .insert((creature.id, bond.target_creature))
        {
            selected_choice = Some(ActionChoice {
                action: ActionKind::InspectScreen,
                target_creature: Some(bond.target_creature),
                target_point: Some(bond.target_position),
            });
            explicit_experience = Some(RelationshipExperience::WatchedClimb);
        }
        if selected_choice.is_none()
            && let Some(bond) = context.bond
            && matches!(
                bond.target_action,
                ActionKind::SoloPlay | ActionKind::SocialPlay
            )
            && bond.relationship.playfulness >= 96
            && bond.relationship.avoidance < 192
            && rng.random_ratio(1, 3)
        {
            selected_choice = Some(ActionChoice {
                action: ActionKind::SocialPlay,
                target_creature: Some(bond.target_creature),
                target_point: Some(bond.target_position),
            });
            explicit_experience = Some(RelationshipExperience::StoleToy);
        }
        if selected_choice.is_none()
            && let Some(invitation) = self.topology.invitation()
            && cursor_invitation_eligible(creature, invitation)
            && rng.random_ratio(1, 2)
        {
            if creature.state.surface.window_key == Some(invitation.window_key) {
                selected_choice = Some(ActionChoice {
                    action: ActionKind::InspectScreen,
                    target_creature: None,
                    target_point: Some(invitation.point),
                });
            } else if creature.state.position.distance(invitation.point) <= 480.0 {
                selected_choice = Some(ActionChoice {
                    action: ActionKind::Perch,
                    target_creature: None,
                    target_point: Some(invitation.point),
                });
            }
            if selected_choice.is_some() {
                scheduled_ambient = true;
            }
        }
        if selected_choice.is_none()
            && let Some(bond) = context.bond
            && bond.distance < 180.0
            && bond.relationship.avoidance >= 96
            && bond.relationship.playfulness >= 48
            && rng.random_ratio(1, 48)
        {
            selected_choice = Some(ActionChoice {
                action: ActionKind::SocialPlay,
                target_creature: Some(bond.target_creature),
                target_point: Some(bond.target_position),
            });
            explicit_experience = Some(RelationshipExperience::Squabble);
        }
        // Waking from a nap is a fine moment to notice something lying beside the pillow;
        // being startled by a window is not.
        if selected_choice.is_none() && discovery_available && old != ActionKind::ReactToWindow {
            creature.state.activity_variant = discovery::choose_trinket_variant(
                &mut self.ambient_rng,
                discovery::circumstances_of(
                    creature,
                    old,
                    now,
                    self.save.created_at_utc,
                    desktop,
                    &self.save.settings,
                    &self.ride_memory,
                    discovery::ColonyView {
                        creatures: creature_views,
                        relationships: relationship_views,
                    },
                ),
                &self.save.companion.scrapbook,
            );
            self.discovery_remaining = self.ambient_rng.random_range(FINDS.desktop_interval_secs);
            let bond = context.bond.filter(|bond| {
                bond.relationship.affinity >= 96
                    && bond.relationship.familiarity >= 48
                    && bond.relationship.avoidance < 160
            });
            selected_choice = Some(ActionChoice {
                action: ActionKind::PresentDiscovery,
                target_creature: bond.map(|bond| bond.target_creature),
                target_point: bond.map(|bond| bond.target_position),
            });
            explicit_experience = bond.map(|_| RelationshipExperience::BroughtDiscovery);
            scheduled_ambient = true;
        }
        if selected_choice.is_none()
            && dangle_available
            && !matches!(old, ActionKind::ReactToWindow | ActionKind::RideWindow)
        {
            if let Some(timers) = self.ambient_timers.get_mut(&creature.id) {
                timers.dangle_remaining =
                    self.ambient_rng.random_range(ROAMING.dangle_interval_secs);
            }
            selected_choice = Some(ActionChoice {
                action: ActionKind::Dangle,
                target_creature: None,
                target_point: None,
            });
            scheduled_ambient = true;
        }
        if selected_choice.is_none()
            && self.save.settings.visible
            && let Some(window_key) = creature.state.surface.window_key
            && self
                .ambient_timers
                .get(&creature.id)
                .is_some_and(|timers| timers.inspect_remaining <= 0.0)
            && let Some(corner) = self
                .topology
                .nearest_corner(window_key, creature.state.position)
        {
            selected_choice = Some(ActionChoice {
                action: ActionKind::InspectScreen,
                target_creature: None,
                target_point: Some(corner),
            });
            if let Some(timers) = self.ambient_timers.get_mut(&creature.id) {
                timers.inspect_remaining =
                    self.ambient_rng.random_range(ROAMING.inspect_interval_secs);
            }
            scheduled_ambient = true;
        }
        if selected_choice.is_none() {
            // A find reaches the scrapbook from the `ActionCompleted` above, and that is
            // projected after this loop has already moved the creature on to something
            // else. Clearing the trinket here would hand the scrapbook variant 0 whatever
            // was actually found, so what was held up outlives the pose by a moment. No
            // other action draws it, and the next thing to show one sets it again.
            if old != ActionKind::PresentDiscovery {
                creature.state.activity_variant = 0;
            }
            // A ledge is a destination, not a one-time upgrade from the desktop floor.
            // Continuing to search while perched lets creatures climb between stacked
            // application windows and later descend when the desktop arrangement changes.
            // Turning window ledges off is a request to stay on the floor, so a ledge
            // simply stops being somewhere a creature can think of going.
            let resting_from_climbing = self
                .ambient_timers
                .get(&creature.id)
                .is_some_and(|timers| timers.climb_rest > 0.0);
            let context = BehaviorContext {
                reachable_window_ledge: self.save.settings.window_ledges
                    && !resting_from_climbing
                    && find_nearby_ledge(
                        creature,
                        desktop,
                        &self.save.settings.habitat,
                        &self.topology,
                        self.save.settings.display_scale,
                    )
                    .is_some(),
                home_point: (creature.leaning == RoamingLeaning::Homebody)
                    .then(|| {
                        desktop.monitors.iter().find(|monitor| {
                            monitor.id == creature.state.surface.monitor_id
                                && self.save.home.display == Some(monitor.display_key)
                        })
                    })
                    .flatten()
                    .and_then(|monitor| {
                        resolved_home_anchor(
                            &self.save.home,
                            cottages,
                            monitor,
                            self.save.settings.display_scale,
                            &self.save.settings.habitat,
                        )
                    }),
                ..context
            };
            selected_choice = Some(choose_action(creature, desktop, context, rng));
        }
        Picked {
            old,
            old_elapsed,
            choice: selected_choice.expect("an action is always selected"),
            explicit_experience,
            continuing_plan,
            scheduled_ambient,
        }
    }

    /// Set out on what was picked: a bond plan, a route up the windows or down off a ledge, and
    /// everything the change of action says.
    fn set_out(&mut self, index: usize, shared: &TickShared, picked: Picked, frame_width: f32) {
        let TickShared { now, desktop, .. } = *shared;
        let creature_views = &self.creature_views;
        let creature = &mut self.save.creatures[index];
        let Picked {
            old,
            old_elapsed,
            mut choice,
            explicit_experience,
            continuing_plan,
            scheduled_ambient,
        } = picked;
        let rng = self
            .rngs
            .get_mut(&creature.id)
            .expect("creature RNG exists");
        let selected = choice.action;
        if !continuing_plan
            && let Some(target) = choice.target_creature
            && let Some(experience) =
                explicit_experience.or_else(|| relationship_experience_for_action(choice.action))
        {
            let target_point =
                bond_target_point(creature, creature_views, target, choice.action, frame_width);
            if let Some(target_point) = target_point {
                let final_action = choice.action;
                let approaching = bond_approach_required(
                    creature.state.position,
                    target_point,
                    final_action,
                    frame_width,
                );
                self.bond_plans.insert(
                    creature.id,
                    BondPlan {
                        target,
                        final_action,
                        experience,
                        approaching,
                    },
                );
                if approaching {
                    choice.action = ActionKind::Follow;
                    choice.target_point = Some(target_point);
                }
            } else {
                choice.target_creature = None;
                choice.target_point = None;
            }
        }
        let mut next = choice.action;
        if selected == ActionKind::Perch && self.save.settings.window_ledges {
            let mut route = planned_window_route(
                creature,
                desktop,
                &self.save.settings.habitat,
                &self.topology,
                self.surface_memory.destination(creature, desktop),
            );
            if !route.is_empty() {
                let first = route.remove(0);
                let journey = build_route_hop_journey(creature, first, desktop);
                next = journey.initial_action();
                self.window_routes.insert(
                    creature.id,
                    WindowRoutePlan {
                        repaired: false,
                        geometry_hash: self.topology.geometry_hash(),
                        remaining: route.into(),
                    },
                );
                self.window_journeys.insert(creature.id, journey);
            } else if let Some((target, surface)) = choice
                .target_point
                .and_then(|target| {
                    topology_ledge_at_target(
                        &self.topology,
                        target,
                        desktop,
                        &self.save.settings.habitat,
                    )
                })
                .or_else(|| {
                    find_nearby_ledge(
                        creature,
                        desktop,
                        &self.save.settings.habitat,
                        &self.topology,
                        self.save.settings.display_scale,
                    )
                })
            {
                let journey = build_window_journey(creature, target, surface, desktop);
                next = journey.initial_action();
                self.window_journeys.insert(creature.id, journey);
            }
        }
        // A companion its owner would rather keep lower down comes off a ledge to do
        // whatever it has chosen next, the way the colony comes down to walk home: the
        // same hop to the floor below, and the chosen action once it has landed.
        // Everybody comes down now and then, so a colony that can climb spends its time
        // about the whole screen rather than settling up on the windows for good: about
        // half of it up high for a companion that likes to be anywhere, more for a
        // climber, and less for the others.
        let comes_down = match creature.leaning {
            RoamingLeaning::FloorDweller => 0.7,
            RoamingLeaning::Homebody => 0.4,
            RoamingLeaning::Anywhere => ROAMING.anywhere_comes_down,
            RoamingLeaning::Climber => ROAMING.climber_comes_down,
        };
        if comes_down > 0.0
            && creature.state.surface.kind == SurfaceKind::WindowLedge
            && !matches!(selected, ActionKind::Perch | ActionKind::RideWindow)
            && !self.window_journeys.contains_key(&creature.id)
            && rng.random_bool(comes_down)
            && let Some((monitor_id, floor)) = nearest_habitat_point(
                &self.save.settings.habitat,
                &desktop.monitors,
                creature.state.position,
                self.save.settings.display_scale,
            )
            && (floor.y - creature.state.position.y).abs() > 0.5
        {
            let journey = WindowJourney::Hop(HopJourney {
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
                lift: 0.0,
            });
            next = journey.initial_action();
            self.window_journeys.insert(creature.id, journey);
            if let Some(timers) = self.ambient_timers.get_mut(&creature.id) {
                timers.climb_rest = rng.random_range(climb_rest_secs(creature.leaning));
            }
        }
        creature.state.action = next;
        creature.state.action_elapsed = 0.0;
        creature.state.action_duration = action_duration(next, rng);
        cue_habit(
            creature,
            next,
            &mut self.habit_rng,
            true,
            self.save.settings.reduce_motion,
            &mut self.events,
        );
        choice.action = next;
        self.action_choices.insert(creature.id, choice);
        if !scheduled_ambient {
            reinforce_habit(creature, selected, now.hour());
        }
        Self::emit(
            &mut self.events,
            WorldEvent::ActionStarted {
                creature_id: creature.id,
                action: next,
            },
        );
        if old == ActionKind::Sleep && next != ActionKind::Sleep {
            let uninterrupted_seconds = self
                .sleep_elapsed
                .remove(&creature.id)
                .unwrap_or(old_elapsed)
                .max(0.0) as u32;
            Self::emit(
                &mut self.events,
                WorldEvent::CreatureRested {
                    creature_id: creature.id,
                    uninterrupted_seconds,
                },
            );
            Self::emit(
                &mut self.events,
                WorldEvent::CreatureWoke {
                    creature_id: creature.id,
                },
            );
        } else if old != ActionKind::Sleep && next == ActionKind::Sleep {
            Self::emit(
                &mut self.events,
                WorldEvent::CreatureSlept {
                    creature_id: creature.id,
                },
            );
        }
        if matches!(
            next,
            ActionKind::InvestigateCursor | ActionKind::AvoidCursor
        ) {
            Self::emit(
                &mut self.events,
                WorldEvent::CursorReaction {
                    creature_id: creature.id,
                    action: next,
                },
            );
        }
        if matches!(next, ActionKind::ReactToWindow | ActionKind::RideWindow) {
            Self::emit(
                &mut self.events,
                WorldEvent::WindowReaction {
                    creature_id: creature.id,
                    action: next,
                },
            );
        }
    }

    /// Do what it is doing: keep to a companion it is going to, move, stay on its surface, and
    /// stop to look at the screen on the way past.
    fn carry_out_action(
        &mut self,
        index: usize,
        shared: &TickShared,
        context: BehaviorContext,
        nearest: Option<(f32, Point, CreatureId)>,
        frame_width: f32,
    ) {
        let TickShared { dt, desktop, .. } = *shared;
        let creature_views = &self.creature_views;
        let creature = &mut self.save.creatures[index];
        let previous_position = creature.state.position;
        let invalid_bond_target = self
            .action_choices
            .get(&creature.id)
            .and_then(|choice| choice.target_creature.map(|target| (choice.action, target)))
            .is_some_and(|(action, target)| {
                match bond_target_point(creature, creature_views, target, action, frame_width) {
                    Some(point) => {
                        if let Some(choice) = self.action_choices.get_mut(&creature.id) {
                            choice.target_point = Some(point);
                        }
                        false
                    }
                    None => true,
                }
            });
        if invalid_bond_target {
            self.action_choices.remove(&creature.id);
            self.bond_plans.remove(&creature.id);
            let interrupted = creature.state.action;
            let interrupted_elapsed = creature.state.action_elapsed;
            if interrupted == ActionKind::Sleep {
                let uninterrupted_seconds = self
                    .sleep_elapsed
                    .remove(&creature.id)
                    .unwrap_or(interrupted_elapsed)
                    .max(0.0) as u32;
                Self::emit(
                    &mut self.events,
                    WorldEvent::CreatureRested {
                        creature_id: creature.id,
                        uninterrupted_seconds,
                    },
                );
                Self::emit(
                    &mut self.events,
                    WorldEvent::CreatureWoke {
                        creature_id: creature.id,
                    },
                );
            }
            creature.state.action = ActionKind::Idle;
            creature.state.action_elapsed = 0.0;
            creature.state.action_duration = 2.5;
            creature.state.velocity = Point::default();
            Self::emit(
                &mut self.events,
                WorldEvent::ActionStarted {
                    creature_id: creature.id,
                    action: ActionKind::Idle,
                },
            );
        }
        let target_point = self
            .action_choices
            .get(&creature.id)
            .and_then(|choice| choice.target_point);
        execute_action(creature, desktop, context, dt, nearest, target_point);
        // Walking into the end of a ledge, or of the ground a creature is allowed on, is
        // arriving: the constraint turns it round, and letting go of the target it cannot
        // reach is what stops it stepping straight back out and being put back every tick.
        // Without this a creature spends the rest of its walk shivering against the wall.
        if constrain_to_surface(
            creature,
            desktop,
            &self.save.settings.habitat,
            self.save.settings.display_scale,
        ) && matches!(
            creature.state.action,
            ActionKind::Traverse | ActionKind::Sprint
        ) && let Some(choice) = self.action_choices.get_mut(&creature.id)
        {
            choice.target_point = None;
        }
        let inspect_ready = self.colony_plan.is_none()
            && self.save.settings.visible
            && creature.state.action == ActionKind::Traverse
            && self
                .ambient_timers
                .get(&creature.id)
                .is_some_and(|timers| timers.inspect_remaining <= 0.0)
            && crossed_inspection_anchor(
                creature,
                previous_position.x,
                desktop,
                &self.save.settings.habitat,
            );
        if inspect_ready {
            Self::emit(
                &mut self.events,
                WorldEvent::ActionCompleted {
                    creature_id: creature.id,
                    action: ActionKind::Traverse,
                },
            );
            creature.state.action = ActionKind::InspectScreen;
            creature.state.action_elapsed = 0.0;
            creature.state.action_duration = self.ambient_rng.random_range(3.0..5.0);
            creature.state.velocity = Point::default();
            if let Some(timers) = self.ambient_timers.get_mut(&creature.id) {
                timers.inspect_remaining =
                    self.ambient_rng.random_range(ROAMING.inspect_interval_secs);
            }
            Self::emit(
                &mut self.events,
                WorldEvent::ActionStarted {
                    creature_id: creature.id,
                    action: ActionKind::InspectScreen,
                },
            );
        }
    }
}
