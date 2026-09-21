use crate::behavior::{BehaviorContext, choose_action};
use crate::rng::SeedStream;
use crate::*;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha12Rng;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use time::{Duration, OffsetDateTime, UtcOffset};

mod arrivals;
mod attention;
mod bonds;
mod bubbles;
mod colony;
mod discovery;
mod experience;
mod generation;
mod home;
mod interaction;
mod journeys;
mod movement;
mod objects;
mod offers;
mod rides;
mod rituals;
mod routine;
mod spacing;
mod surfaces;
mod visitors;
use attention::{AttentionRuntime, DisplayAttention};
use bonds::*;
pub use bubbles::{BubbleGrowth, ThoughtBubble};
use colony::*;
use generation::*;
use interaction::*;
use journeys::*;
use movement::*;
use objects::*;
pub(crate) use objects::{scheduled_colony_object_at, scheduled_shelter_decoration_at};
pub(crate) use rituals::scheduled_ritual_at;
use rituals::*;
use surfaces::SurfaceMemory;

/// Width of a creature's art frame, matching `formiga_art::FRAME_SIZE`. The simulation crate
/// cannot depend on the art crate, so shelter layout mirrors the constant the way `home_anchor`
/// already mirrors the shelter's own half-width.
const CREATURE_ART_WIDTH: f32 = crate::CREATURE_FRAME_WIDTH;
const INSPECT_INTERVAL_SECS: std::ops::Range<f32> = 120.0..240.0;
const DANGLE_INTERVAL_SECS: std::ops::Range<f32> = 240.0..480.0;
const DISCOVERY_INTERVAL_SECS: std::ops::Range<f32> = 600.0..1_200.0;

pub struct World {
    pub save: SaveFile,
    rngs: BTreeMap<CreatureId, ChaCha12Rng>,
    events: Vec<WorldEvent>,
    last_windows: BTreeMap<WindowKey, DesktopRect>,
    interaction: Option<InteractionSession>,
    window_journeys: BTreeMap<CreatureId, WindowJourney>,
    window_routes: BTreeMap<CreatureId, WindowRoutePlan>,
    ambient_rng: ChaCha12Rng,
    ambient_timers: BTreeMap<CreatureId, AmbientTimers>,
    discovery_remaining: f32,
    tosses: BTreeMap<CreatureId, TossState>,
    observation_elapsed: f32,
    projected_events: usize,
    sleep_elapsed: BTreeMap<CreatureId, f32>,
    action_choices: BTreeMap<CreatureId, ActionChoice>,
    bond_plans: BTreeMap<CreatureId, BondPlan>,
    calm_proximity_seconds: BTreeMap<(CreatureId, CreatureId), u16>,
    reacted_to_toss: BTreeSet<(CreatureId, CreatureId)>,
    watched_climb: BTreeSet<(CreatureId, CreatureId)>,
    pending_home_greetings: BTreeSet<CreatureId>,
    /// The short, quiet things residents are doing at their own doors right now. Runtime only:
    /// a passive moment is a clip and nothing else, and never outlives a home visit.
    home_moments: BTreeMap<CreatureId, home::HomeMoment>,
    /// Seconds until each resident's next passive moment, staggered per creature.
    home_moment_timers: BTreeMap<CreatureId, f32>,
    home_moment_rng: ChaCha12Rng,
    colony_plan: Option<ColonyPlan>,
    topology: DesktopTopology,
    geometry_observer: crate::attention::GeometryObserver,
    attention: AttentionRuntime,
    cursor_observer: crate::cursor::CursorObserver,
    display_attention: DisplayAttention,
    surface_memory: SurfaceMemory,
    ride_memory: rides::RideMemory,
    bubbles: Vec<ThoughtBubble>,
    /// What each creature carries between offers: recent answers, a beat it is taking, and the
    /// clip it is still enjoying. Runtime-only, and never wider than whoever is actually here.
    offers: BTreeMap<CreatureId, offers::OfferMemory>,
    overlaps: spacing::OverlapWatch,
    /// The colony exactly as it stood when the current tick began. Each creature answers for
    /// itself against the same picture of the others, rather than against neighbours the same
    /// loop has already moved, and the borrow checker will not lend out the colony twice. The
    /// two lists are kept between ticks so that taking the picture reuses their storage.
    creature_views: Vec<Creature>,
    relationship_views: Vec<CreatureRelationship>,
}

#[derive(Clone, Copy)]
struct AmbientTimers {
    inspect_remaining: f32,
    dangle_remaining: f32,
}

fn lerp(a: f32, b: f32, progress: f32) -> f32 {
    a + (b - a) * progress
}

fn lerp_point(a: Point, b: Point, progress: f32) -> Point {
    Point {
        x: lerp(a.x, b.x, progress),
        y: lerp(a.y, b.y, progress),
    }
}

fn smoothstep(progress: f32) -> f32 {
    progress * progress * (3.0 - 2.0 * progress)
}

fn creature_mut(creatures: &mut [Creature], creature_id: CreatureId) -> Option<&mut Creature> {
    creatures
        .iter_mut()
        .find(|creature| creature.id == creature_id)
}

fn display_region(
    desktop: &DesktopSnapshot,
    monitor_id: MonitorId,
    point: Point,
) -> Option<(DisplayKey, u8)> {
    let monitor = desktop
        .monitors
        .iter()
        .find(|monitor| monitor.id == monitor_id)
        .or_else(|| {
            desktop
                .monitors
                .iter()
                .find(|monitor| monitor.bounds.contains(point))
        })?;
    let bounds = monitor.usable_bounds;
    if bounds.width <= 0.0 || bounds.height <= 0.0 {
        return None;
    }
    let column = (((point.x - bounds.x) / bounds.width).clamp(0.0, 0.999) * 3.0) as u8;
    let row = (((point.y - bounds.y) / bounds.height).clamp(0.0, 0.999) * 3.0) as u8;
    Some((monitor.display_key, row * 3 + column))
}

fn local_time_or_utc(now: OffsetDateTime) -> OffsetDateTime {
    let offset = UtcOffset::local_offset_at(now).unwrap_or(UtcOffset::UTC);
    now.to_offset(offset)
}

impl World {
    /// Queues one ephemeral world event. Publicly observable events are projected into compact
    /// state before `tick`, `handle_command`, or `drain_events` returns.
    fn emit(events: &mut Vec<WorldEvent>, event: WorldEvent) {
        events.push(event);
    }

    pub fn new(colony_seed: [u8; 32], now: OffsetDateTime, desktop: &DesktopSnapshot) -> Self {
        let streams = SeedStream::new(colony_seed);
        let creature = generate_new_creature(&streams, colony_seed, 0, now, desktop, &[], None);
        let home_display = desktop
            .monitors
            .iter()
            .find(|monitor| monitor.primary)
            .or_else(|| desktop.monitors.first())
            .map(|monitor| monitor.display_key);
        let mut home = ColonyHome::from_seed(colony_seed, home_display, Some(now), None);
        home.decorations.next_at_utc = scheduled_shelter_decoration_at(colony_seed, 0, now);
        let save = SaveFile {
            companion: crate::CompanionState {
                onboarding_complete: false,
                journal: vec![crate::JournalEntry {
                    at: now,
                    creature: Some(creature.id),
                    moment: crate::JournalMoment::Arrival,
                }],
                ..Default::default()
            },
            save_version: crate::SAVE_VERSION,
            colony_seed,
            created_at_utc: now,
            maximum_seen_utc: now,
            arrival_state: ArrivalState::default(),
            home,
            settings: Settings::default(),
            creatures: vec![creature],
            relationships: Vec::new(),
            ritual: RitualState {
                next_at_utc: scheduled_ritual_at(colony_seed, 0, now),
                ..RitualState::default()
            },
            objects: ColonyObjectState {
                next_at_utc: scheduled_colony_object_at(colony_seed, 0, now),
                ..ColonyObjectState::default()
            },
            visitors: VisitorState::default(),
        };
        Self::from_save(save)
    }

    pub fn from_save(mut save: SaveFile) -> Self {
        save.companion.normalize();
        save.visitors.normalize();
        normalize_colony_roles(&mut save);
        normalize_relationships(&mut save);
        if save.ritual.next_at_utc == OffsetDateTime::UNIX_EPOCH {
            save.ritual.next_at_utc =
                scheduled_ritual_at(save.colony_seed, save.ritual.ordinal, save.maximum_seen_utc);
        }
        save.objects.objects.truncate(MAX_COLONY_OBJECTS);
        if save.objects.next_at_utc == OffsetDateTime::UNIX_EPOCH {
            save.objects.next_at_utc = scheduled_colony_object_at(
                save.colony_seed,
                save.objects.ordinal,
                save.maximum_seen_utc,
            );
        }
        let mut seen_decorations = BTreeSet::new();
        save.home
            .decorations
            .decorations
            .retain(|kind| seen_decorations.insert(*kind));
        save.home
            .decorations
            .decorations
            .truncate(MAX_SHELTER_DECORATIONS);
        if save.home.decorations.next_at_utc == OffsetDateTime::UNIX_EPOCH {
            save.home.decorations.next_at_utc = scheduled_shelter_decoration_at(
                save.colony_seed,
                save.home.decorations.ordinal,
                save.maximum_seen_utc,
            );
        }
        // Identity and durable drives survive relaunch, but interrupted locomotion and reactions do
        // not. Surface attachments are validated against the first desktop snapshot on the next
        // tick, while every creature resumes from a stable pose.
        for creature in &mut save.creatures {
            creature.appearance.design = creature
                .appearance
                .design
                .map(crate::CreatureDesign::bounded);
            creature.origin.design = creature.appearance.design;
            creature.state.action = ActionKind::Idle;
            creature.state.action_elapsed = 0.0;
            creature.state.action_duration = 2.5;
            creature.state.velocity = Point::default();
            creature.state.activity_variant = 0;
            creature.state.attention = None;
        }
        let rngs = save
            .creatures
            .iter()
            .map(|creature| (creature.id, ChaCha12Rng::from_seed(creature.behavior_seed)))
            .collect();
        let streams = SeedStream::new(save.colony_seed);
        let mut ambient_rng = streams.rng("ambient-runtime", 0);
        let ambient_timers = save
            .creatures
            .iter()
            .map(|creature| {
                (
                    creature.id,
                    AmbientTimers {
                        inspect_remaining: ambient_rng.random_range(INSPECT_INTERVAL_SECS),
                        dangle_remaining: ambient_rng.random_range(DANGLE_INTERVAL_SECS),
                    },
                )
            })
            .collect();
        let discovery_remaining = ambient_rng.random_range(DISCOVERY_INTERVAL_SECS);
        Self {
            save,
            rngs,
            events: Vec::new(),
            last_windows: BTreeMap::new(),
            interaction: None,
            window_journeys: BTreeMap::new(),
            surface_memory: SurfaceMemory::default(),
            ride_memory: rides::RideMemory::default(),
            bubbles: Vec::new(),
            offers: BTreeMap::new(),
            overlaps: spacing::OverlapWatch::default(),
            window_routes: BTreeMap::new(),
            ambient_rng,
            ambient_timers,
            discovery_remaining,
            tosses: BTreeMap::new(),
            observation_elapsed: 0.0,
            projected_events: 0,
            sleep_elapsed: BTreeMap::new(),
            action_choices: BTreeMap::new(),
            bond_plans: BTreeMap::new(),
            calm_proximity_seconds: BTreeMap::new(),
            reacted_to_toss: BTreeSet::new(),
            watched_climb: BTreeSet::new(),
            pending_home_greetings: BTreeSet::new(),
            home_moments: BTreeMap::new(),
            home_moment_timers: BTreeMap::new(),
            home_moment_rng: streams.rng("home-moments", 0),
            colony_plan: None,
            creature_views: Vec::new(),
            relationship_views: Vec::new(),
            topology: DesktopTopology::default(),
            geometry_observer: crate::attention::GeometryObserver::default(),
            attention: AttentionRuntime::default(),
            cursor_observer: crate::cursor::CursorObserver::default(),
            display_attention: DisplayAttention::default(),
        }
    }

    pub fn tick(&mut self, now: OffsetDateTime, dt: f32, desktop: &DesktopSnapshot) {
        if now > self.save.maximum_seen_utc {
            self.save.maximum_seen_utc = now;
        }
        let timeline_now = self.save.maximum_seen_utc;
        if self
            .save
            .companion
            .quiet_until
            .is_some_and(|until| now >= until)
        {
            self.save.companion.quiet_until = None;
            self.dismiss_home(timeline_now, false);
        }
        self.apply_routine_schedule(now);
        self.process_arrivals(timeline_now, desktop);
        self.process_colony_objects(timeline_now, desktop);
        self.process_shelter_decorations(timeline_now);
        self.reconcile_colony_objects(desktop);
        // Capture display loss before route recovery can replace a creature's old attachment.
        let display_attention_active = self.save.settings.visible
            && !self.save.settings.paused
            && !self.save.home.is_active()
            && self.save.companion.quiet_until.is_none();
        self.prepare_display_attention(desktop, dt, display_attention_active);
        let topology_changed = self
            .topology
            .rebuild_if_changed(desktop, &self.last_windows);
        if topology_changed && !self.window_routes.is_empty() {
            for (creature_id, mut previous_plan) in std::mem::take(&mut self.window_routes) {
                let unaffected = self
                    .window_journeys
                    .get(&creature_id)
                    .is_some_and(|j| j.valid(desktop))
                    && previous_plan.remaining.iter().all(|hop| {
                        [
                            (hop.from_window, hop.from_bounds),
                            (hop.to_window, hop.to_bounds),
                        ]
                        .into_iter()
                        .all(|(key, bounds)| {
                            desktop.windows.iter().any(|w| {
                                w.key == key && w.bounds == bounds && w.visible && !w.minimized
                            })
                        })
                    });
                if unaffected {
                    previous_plan.geometry_hash = self.topology.geometry_hash();
                    self.window_routes.insert(creature_id, previous_plan);
                    continue;
                }
                self.window_journeys.remove(&creature_id);
                if let Some(creature) = creature_mut(&mut self.save.creatures, creature_id) {
                    settle_interrupted_journey(
                        creature,
                        desktop,
                        &self.save.settings.habitat,
                        &mut self.events,
                    );
                    if !previous_plan.repaired
                        && display_attention_active
                        && self.save.settings.window_ledges
                        && !self.save.settings.reduce_motion
                        && creature.state.surface.window_key.is_some()
                    {
                        let mut route = planned_window_route(
                            creature,
                            desktop,
                            &self.save.settings.habitat,
                            &self.topology,
                            self.surface_memory.destination(creature, desktop),
                        );
                        if !route.is_empty() {
                            let first = route.remove(0);
                            let mut journey = build_route_hop_journey(creature, first, desktop);
                            if let WindowJourney::Climb(climb) = &mut journey {
                                climb.elapsed = -0.7;
                            }
                            creature.state.action = ActionKind::InspectScreen;
                            creature.state.action_elapsed = 0.0;
                            creature.state.action_duration = f32::MAX;
                            self.window_journeys.insert(creature_id, journey);
                            self.window_routes.insert(
                                creature_id,
                                WindowRoutePlan {
                                    geometry_hash: self.topology.geometry_hash(),
                                    remaining: route.into(),
                                    repaired: true,
                                },
                            );
                        }
                    }
                }
            }
        }
        if self.colony_plan.is_some()
            && (!self.save.settings.visible
                || self.save.settings.paused
                || self.interaction.is_some()
                || !self.tosses.is_empty())
        {
            self.interrupt_colony_plan(timeline_now);
        }
        let home_active = self.update_home_cycle(desktop);
        let attention_active = self.save.settings.visible
            && !self.save.settings.paused
            && !home_active
            && self.save.companion.quiet_until.is_none();
        self.cursor_observer.update(
            desktop,
            dt,
            attention_active
                && self.save.settings.cursor_reactions
                && desktop.window_sample.is_none_or(|sample| sample.reliable),
        );
        if self.cursor_observer.safe && attention_active && self.save.settings.cursor_reactions {
            self.topology.update_cursor_invitation(desktop, dt);
        } else {
            self.topology.clear_invitation();
        }
        let observations_ready = self.geometry_observer.update(desktop, dt, attention_active);
        self.ride_memory
            .update(&self.save.creatures, desktop, dt, observations_ready);
        if !observations_ready {
            self.surface_memory
                .update(&self.save.creatures, desktop, dt, false);
            self.cancel_gap_journeys(desktop);
            self.clear_attention();
        }
        self.tick_bubbles(dt);
        self.tick_offers(dt, desktop);
        if self.save.settings.paused {
            self.settle_active_tosses(desktop);
            self.project_events(timeline_now);
            return;
        }
        if home_active {
            self.tick_homebound_creatures(timeline_now, dt, desktop);
            self.tick_visitor(timeline_now, dt, desktop);
            self.sample_observations(dt, desktop);
            self.last_windows = desktop
                .windows
                .iter()
                .map(|window| (window.key, window.bounds))
                .collect();
            self.project_events(timeline_now);
            return;
        }
        if self.save.settings.visible {
            for timers in self.ambient_timers.values_mut() {
                timers.inspect_remaining = (timers.inspect_remaining - dt).max(0.0);
                timers.dangle_remaining = (timers.dangle_remaining - dt).max(0.0);
            }
            self.discovery_remaining = (self.discovery_remaining - dt).max(0.0);
        }

        if observations_ready {
            self.remember_attention_supports(dt);
        }
        update_surface_attachments(
            &mut self.save.creatures,
            desktop,
            &self.last_windows,
            &self.topology,
            observations_ready,
            &self.window_journeys,
            &mut self.events,
        );
        if self
            .colony_plan
            .as_ref()
            .is_some_and(|plan| !plan.geometry_is_valid(desktop, &self.save.settings.habitat))
        {
            self.interrupt_colony_plan(timeline_now);
        }
        self.advance_colony_plan(timeline_now, dt);
        let at_selection_boundary = self.save.creatures.iter().any(|creature| {
            creature.state.arrival_delay_secs <= 0.0
                && creature.state.action_elapsed + dt >= creature.state.action_duration
        });
        if self.colony_plan.is_none()
            && self.save.ritual.next_at_utc <= timeline_now
            && at_selection_boundary
            // Nobody is called away from something they were just given.
            && !self.offer_response_active()
        {
            self.try_start_colony_plan(timeline_now, desktop);
        }
        self.surface_memory
            .update(&self.save.creatures, desktop, dt, observations_ready);
        self.advance_attention(desktop, dt, observations_ready);
        self.creature_views.clear();
        self.creature_views.extend_from_slice(&self.save.creatures);
        self.relationship_views.clear();
        self.relationship_views
            .extend_from_slice(&self.save.relationships);
        let creature_views = &self.creature_views;
        let relationship_views = &self.relationship_views;
        self.reacted_to_toss.retain(|(_, target)| {
            creature_views.iter().any(|creature| {
                creature.id == *target && creature.state.action == ActionKind::Tossed
            })
        });
        self.watched_climb.retain(|(_, target)| {
            creature_views.iter().any(|creature| {
                creature.id == *target && creature.state.action == ActionKind::ClimbWindow
            })
        });
        let cottages = colony_cottages(&self.save.creatures);
        for creature in &mut self.save.creatures {
            if self
                .interaction
                .as_ref()
                .is_some_and(|interaction| interaction.creature_id == creature.id)
            {
                continue;
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
                continue;
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
                continue;
            }
            // How wide this creature draws where it stands: every bond mark is measured in it,
            // so companions keep the same distance at every display scale.
            let frame_width =
                spacing::creature_frame_width(creature, self.save.settings.display_scale, desktop);
            update_drives(creature, dt);
            creature.state.cursor_cooldown = (creature.state.cursor_cooldown - dt).max(0.0);
            creature.state.action_elapsed += dt;
            if creature.state.action == ActionKind::Sleep {
                *self.sleep_elapsed.entry(creature.id).or_default() += dt;
            }

            if self.attention.owns(creature.id) {
                if !self.attention.crosses_displays(creature.id) {
                    constrain_to_surface(creature, desktop, &self.save.settings.habitat);
                }
                continue;
            }

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
                        &mut self.events,
                    );
                    continue;
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
                        &mut self.events,
                    );
                    continue;
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
                        continue;
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
                continue;
            }

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
                    &cottages,
                    desktop,
                    &self.save.settings.habitat,
                    &self.save.home,
                    self.save.settings.display_scale,
                ),
                hour_utc: now.hour(),
            };

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

            if creature.state.action_elapsed >= creature.state.action_duration {
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
                    && self.discovery_remaining <= 0.0
                    && !creature_views.iter().any(|other| {
                        other.id != creature.id
                            && other.state.action == ActionKind::PresentDiscovery
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
                if selected_choice.is_none()
                    && discovery_available
                    && !matches!(old, ActionKind::Sleep | ActionKind::ReactToWindow)
                {
                    creature.state.activity_variant = discovery::choose_trinket_variant(
                        &mut self.ambient_rng,
                        discovery::circumstances_of(
                            creature,
                            old,
                            now,
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
                    self.discovery_remaining =
                        self.ambient_rng.random_range(DISCOVERY_INTERVAL_SECS);
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
                            self.ambient_rng.random_range(DANGLE_INTERVAL_SECS);
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
                            self.ambient_rng.random_range(INSPECT_INTERVAL_SECS);
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
                    let context = BehaviorContext {
                        reachable_window_ledge: self.save.settings.window_ledges
                            && find_nearby_ledge(
                                creature,
                                desktop,
                                &self.save.settings.habitat,
                                &self.topology,
                            )
                            .is_some(),
                        ..context
                    };
                    selected_choice = Some(choose_action(creature, desktop, context, rng));
                }
                let mut choice = selected_choice.expect("an action is always selected");
                let selected = choice.action;
                if !continuing_plan
                    && let Some(target) = choice.target_creature
                    && let Some(experience) = explicit_experience
                        .or_else(|| relationship_experience_for_action(choice.action))
                {
                    let target_point = bond_target_point(
                        creature,
                        creature_views,
                        target,
                        choice.action,
                        frame_width,
                    );
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
                            )
                        })
                    {
                        let journey = build_window_journey(creature, target, surface, desktop);
                        next = journey.initial_action();
                        self.window_journeys.insert(creature.id, journey);
                    }
                }
                creature.state.action = next;
                creature.state.action_elapsed = 0.0;
                creature.state.action_duration = action_duration(next, rng);
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
            if constrain_to_surface(creature, desktop, &self.save.settings.habitat)
                && matches!(
                    creature.state.action,
                    ActionKind::Traverse | ActionKind::Sprint
                )
                && let Some(choice) = self.action_choices.get_mut(&creature.id)
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
                    timers.inspect_remaining = self.ambient_rng.random_range(INSPECT_INTERVAL_SECS);
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
        let unconstrained: Vec<_> = self
            .interaction
            .as_ref()
            .map(|interaction| interaction.creature_id)
            .into_iter()
            .chain(self.window_journeys.keys().copied())
            .chain(self.tosses.keys().copied())
            .chain(self.attention.crossing_ids())
            .collect();
        keep_creatures_in_habitat(
            &mut self.save.creatures,
            desktop,
            &self.save.settings.habitat,
            &unconstrained,
        );
        self.resolve_overlaps(dt, desktop);
        self.last_windows = desktop
            .windows
            .iter()
            .map(|window| (window.key, window.bounds))
            .collect();
        self.sample_observations(dt, desktop);
        self.project_events(timeline_now);
    }

    pub fn drain_events(&mut self) -> impl Iterator<Item = WorldEvent> + '_ {
        self.project_events(self.save.maximum_seen_utc);
        self.projected_events = 0;
        self.events.drain(..)
    }

    pub fn is_dragging(&self) -> bool {
        self.interaction
            .as_ref()
            .is_some_and(|interaction| interaction.dragging)
    }

    pub fn is_interacting(&self) -> bool {
        self.interaction.is_some()
    }

    pub fn reset(&mut self, colony_seed: [u8; 32], now: OffsetDateTime, desktop: &DesktopSnapshot) {
        *self = Self::new(colony_seed, now, desktop);
    }
}

fn distance_to_interval(value: f32, interval: (f32, f32)) -> f32 {
    if value < interval.0 {
        interval.0 - value
    } else if value > interval.1 {
        value - interval.1
    } else {
        0.0
    }
}

fn distance_to_rect(point: Point, rect: DesktopRect) -> f32 {
    let dx = if point.x < rect.x {
        rect.x - point.x
    } else if point.x > rect.right() {
        point.x - rect.right()
    } else {
        0.0
    };
    let dy = if point.y < rect.y {
        rect.y - point.y
    } else if point.y > rect.bottom() {
        point.y - rect.bottom()
    } else {
        0.0
    };
    (dx * dx + dy * dy).sqrt()
}

#[cfg(test)]
mod tests;
