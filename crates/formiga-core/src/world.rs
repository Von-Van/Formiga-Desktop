use crate::behavior::{BehaviorContext, choose_action};
use crate::rng::SeedStream;
use crate::*;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha12Rng;
use std::collections::BTreeMap;
use time::{Date, Duration, Month, OffsetDateTime, PrimitiveDateTime, UtcOffset};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ArrivalMilestone {
    Hours(i64),
    Days(i64),
    CalendarMonths(u8),
}

const ARRIVAL_MILESTONES: [ArrivalMilestone; 3] = [
    ArrivalMilestone::Hours(1),
    ArrivalMilestone::Days(7),
    ArrivalMilestone::CalendarMonths(1),
];
/// Width of a creature's art frame, matching `formiga_art::FRAME_SIZE`. The simulation crate
/// cannot depend on the art crate, so shelter layout mirrors the constant the way `home_anchor`
/// already mirrors the shelter's own half-width.
const CREATURE_ART_WIDTH: f32 = 48.0;
/// How far apart homebound creatures sit, as a fraction of their drawn width. Below roughly half
/// they overlap enough to hide each other; much above it they stop reading as a colony at home.
const HOME_SPACING_RATIO: f32 = 0.55;
const HOME_DURATION: time::Duration = time::Duration::minutes(15);
const HOME_COOLDOWN: time::Duration = time::Duration::minutes(15);
const INSPECT_INTERVAL_SECS: std::ops::Range<f32> = 120.0..240.0;
const DANGLE_INTERVAL_SECS: std::ops::Range<f32> = 240.0..480.0;
const DISCOVERY_INTERVAL_SECS: std::ops::Range<f32> = 600.0..1_200.0;
const INSPECTION_RADIUS: f32 = 12.0;
const TOSS_SPEED_THRESHOLD: f32 = 220.0;
const TOSS_VELOCITY_SCALE: f32 = 0.65;
const TOSS_MAX_SPEED: f32 = 900.0;
const TOSS_GRAVITY: f32 = 1_200.0;
const TOSS_HORIZONTAL_DRAG: f32 = 1.6;
const TOSS_BOUNCE_RESTITUTION: f32 = 0.28;
const TOSS_BOUNCE_HORIZONTAL_RETENTION: f32 = 0.65;
const TOSS_MIN_BOUNCE_SPEED: f32 = 140.0;
const TOSS_MAX_DURATION: f32 = 3.0;
const DRAG_THRESHOLD: f32 = 6.0;
const OBSERVATION_INTERVAL_SECS: f32 = 60.0;
const MANTLE_LIFT_POINTS: f32 = 10.0;

pub struct World {
    pub save: SaveFile,
    rngs: BTreeMap<CreatureId, ChaCha12Rng>,
    events: Vec<WorldEvent>,
    last_windows: BTreeMap<WindowKey, DesktopRect>,
    interaction: Option<InteractionSession>,
    window_journeys: BTreeMap<CreatureId, WindowJourney>,
    ambient_rng: ChaCha12Rng,
    ambient_timers: BTreeMap<CreatureId, AmbientTimers>,
    discovery_remaining: f32,
    tosses: BTreeMap<CreatureId, TossState>,
    observation_elapsed: f32,
    projected_events: usize,
    sleep_elapsed: BTreeMap<CreatureId, f32>,
    action_choices: BTreeMap<CreatureId, ActionChoice>,
}

#[derive(Clone)]
struct InteractionSession {
    creature_id: CreatureId,
    press_cursor: Point,
    max_excursion: f32,
    dragging: bool,
    grab_offset: Point,
    original_position: Point,
    original_surface: SurfaceAttachment,
    original_action: ActionKind,
    velocity_samples: [Point; 3],
    velocity_sample_count: u8,
    next_velocity_sample: u8,
}

impl InteractionSession {
    fn record_velocity(&mut self, velocity: Point) {
        let index = usize::from(self.next_velocity_sample % 3);
        self.velocity_samples[index] = velocity;
        self.next_velocity_sample = (self.next_velocity_sample + 1) % 3;
        self.velocity_sample_count = self.velocity_sample_count.saturating_add(1).min(3);
    }

    fn release_velocity(&self) -> Point {
        let count = usize::from(self.velocity_sample_count);
        if count == 0 {
            return Point::default();
        }
        let sum = self
            .velocity_samples
            .iter()
            .take(count)
            .fold(Point::default(), |sum, sample| Point {
                x: sum.x + sample.x,
                y: sum.y + sample.y,
            });
        Point {
            x: sum.x / count as f32,
            y: sum.y / count as f32,
        }
    }
}

#[derive(Clone)]
struct TossState {
    elapsed: f32,
    bounces: u8,
    last_safe_position: Point,
    last_safe_surface: SurfaceAttachment,
}

#[derive(Clone)]
struct HopJourney {
    start: Point,
    target: Point,
    surface: SurfaceAttachment,
    elapsed: f32,
    duration: f32,
}

#[derive(Clone)]
struct ClimbJourney {
    target_window: WindowKey,
    target_bounds: DesktopRect,
    start: Point,
    approach: Point,
    climb_end: Point,
    target: Point,
    surface: SurfaceAttachment,
    elapsed: f32,
    approach_duration: f32,
    climb_duration: f32,
    mantle_duration: f32,
}

#[derive(Clone)]
enum WindowJourney {
    Hop(HopJourney),
    Climb(ClimbJourney),
}

#[derive(Clone, Copy)]
struct JourneyStep {
    position: Point,
    action: ActionKind,
    complete: bool,
}

#[derive(Clone, Copy)]
struct AmbientTimers {
    inspect_remaining: f32,
    dangle_remaining: f32,
}

impl WindowJourney {
    fn initial_action(&self) -> ActionKind {
        match self {
            Self::Hop(_) => ActionKind::Landing,
            Self::Climb(journey) if journey.approach_duration > 0.05 => ActionKind::Traverse,
            Self::Climb(_) => ActionKind::ClimbWindow,
        }
    }

    fn surface(&self) -> &SurfaceAttachment {
        match self {
            Self::Hop(journey) => &journey.surface,
            Self::Climb(journey) => &journey.surface,
        }
    }

    fn valid(&self, desktop: &DesktopSnapshot) -> bool {
        match self {
            Self::Hop(journey) => journey.surface.window_key.is_some_and(|key| {
                desktop
                    .windows
                    .iter()
                    .any(|window| window.key == key && window.visible && !window.minimized)
            }),
            Self::Climb(journey) => desktop.windows.iter().any(|window| {
                window.key == journey.target_window
                    && window.visible
                    && !window.minimized
                    && window.bounds == journey.target_bounds
            }),
        }
    }

    fn advance(&mut self, dt: f32) -> JourneyStep {
        match self {
            Self::Hop(journey) => {
                journey.elapsed += dt;
                let progress = (journey.elapsed / journey.duration).clamp(0.0, 1.0);
                let arc = (progress * std::f32::consts::PI).sin()
                    * journey
                        .start
                        .distance(journey.target)
                        .mul_add(0.12, 24.0)
                        .min(90.0);
                JourneyStep {
                    position: Point {
                        x: lerp(journey.start.x, journey.target.x, progress),
                        y: lerp(journey.start.y, journey.target.y, progress) - arc,
                    },
                    action: ActionKind::Landing,
                    complete: progress >= 1.0,
                }
            }
            Self::Climb(journey) => {
                journey.elapsed += dt;
                let approach_end = journey.approach_duration;
                let climb_end = approach_end + journey.climb_duration;
                let total = climb_end + journey.mantle_duration;
                if journey.elapsed < approach_end {
                    let progress = (journey.elapsed / approach_end.max(0.001)).clamp(0.0, 1.0);
                    JourneyStep {
                        position: lerp_point(journey.start, journey.approach, smoothstep(progress)),
                        action: ActionKind::Traverse,
                        complete: false,
                    }
                } else if journey.elapsed < climb_end {
                    let progress = ((journey.elapsed - approach_end)
                        / journey.climb_duration.max(0.001))
                    .clamp(0.0, 1.0);
                    JourneyStep {
                        position: lerp_point(journey.approach, journey.climb_end, progress),
                        action: ActionKind::ClimbWindow,
                        complete: false,
                    }
                } else {
                    let progress = ((journey.elapsed - climb_end)
                        / journey.mantle_duration.max(0.001))
                    .clamp(0.0, 1.0);
                    JourneyStep {
                        position: lerp_point(
                            journey.climb_end,
                            journey.target,
                            smoothstep(progress),
                        ),
                        // Keep the climbing pose attached through the whole pull-up. Switching to
                        // the landing pose at the start of this short segment made the body appear
                        // to pause and then snap above the ledge before settling.
                        action: ActionKind::ClimbWindow,
                        complete: journey.elapsed >= total,
                    }
                }
            }
        }
    }
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

fn arrival_due_at(created_at_utc: OffsetDateTime, milestone: ArrivalMilestone) -> OffsetDateTime {
    match milestone {
        ArrivalMilestone::Hours(hours) => created_at_utc + Duration::hours(hours),
        ArrivalMilestone::Days(days) => created_at_utc + Duration::days(days),
        ArrivalMilestone::CalendarMonths(months) => add_calendar_months_utc(created_at_utc, months),
    }
}

fn add_calendar_months_utc(value: OffsetDateTime, months: u8) -> OffsetDateTime {
    let value = value.to_offset(UtcOffset::UTC);
    let month_index = i64::from(value.year()) * 12 + i64::from(u8::from(value.month()) - 1);
    let destination = month_index + i64::from(months);
    let year = i32::try_from(destination.div_euclid(12)).expect("calendar year remains in range");
    let month = Month::try_from((destination.rem_euclid(12) + 1) as u8)
        .expect("calendar month is always in range");
    let day = value.day().min(month.length(year));
    let date = Date::from_calendar_date(year, month, day).expect("clamped calendar date is valid");
    PrimitiveDateTime::new(date, value.time()).assume_utc()
}

impl World {
    /// Queues one ephemeral world event. Publicly observable events are projected into compact
    /// state before `tick`, `handle_command`, or `drain_events` returns.
    fn emit(events: &mut Vec<WorldEvent>, event: WorldEvent) {
        events.push(event);
    }

    pub fn new(colony_seed: [u8; 32], now: OffsetDateTime, desktop: &DesktopSnapshot) -> Self {
        let streams = SeedStream::new(colony_seed);
        let creature = generate_creature(&streams, colony_seed, 0, now, desktop, &[], None);
        let home_display = desktop
            .monitors
            .iter()
            .find(|monitor| monitor.primary)
            .or_else(|| desktop.monitors.first())
            .map(|monitor| monitor.display_key);
        let save = SaveFile {
            save_version: crate::SAVE_VERSION,
            colony_seed,
            created_at_utc: now,
            maximum_seen_utc: now,
            arrival_state: ArrivalState::default(),
            home: ColonyHome::from_seed(colony_seed, home_display, Some(now), None),
            settings: Settings::default(),
            creatures: vec![creature],
        };
        Self::from_save(save)
    }

    pub fn from_save(mut save: SaveFile) -> Self {
        // Identity and durable drives survive relaunch, but interrupted locomotion and reactions do
        // not. Surface attachments are validated against the first desktop snapshot on the next
        // tick, while every creature resumes from a stable pose.
        for creature in &mut save.creatures {
            creature.state.action = ActionKind::Idle;
            creature.state.action_elapsed = 0.0;
            creature.state.action_duration = 2.5;
            creature.state.velocity = Point::default();
            creature.state.activity_variant = 0;
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
            ambient_rng,
            ambient_timers,
            discovery_remaining,
            tosses: BTreeMap::new(),
            observation_elapsed: 0.0,
            projected_events: 0,
            sleep_elapsed: BTreeMap::new(),
            action_choices: BTreeMap::new(),
        }
    }

    pub fn tick(&mut self, now: OffsetDateTime, dt: f32, desktop: &DesktopSnapshot) {
        if now > self.save.maximum_seen_utc {
            self.save.maximum_seen_utc = now;
        }
        let timeline_now = self.save.maximum_seen_utc;
        self.process_arrivals(timeline_now, desktop);
        let home_active = self.update_home_cycle(desktop);
        if self.save.settings.paused {
            self.settle_active_tosses(desktop);
            self.project_events(timeline_now);
            return;
        }
        if home_active {
            self.tick_homebound_creatures(timeline_now, dt);
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

        let window_changed =
            window_change_near_creatures(&self.last_windows, desktop, &self.save.creatures);
        update_surface_attachments(
            &mut self.save.creatures,
            desktop,
            &self.last_windows,
            &mut self.events,
        );
        let creature_views = self.save.creatures.clone();
        let mut relationship_updates = Vec::new();
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
            update_drives(creature, dt);
            creature.state.cursor_cooldown = (creature.state.cursor_cooldown - dt).max(0.0);
            creature.state.action_elapsed += dt;
            if creature.state.action == ActionKind::Sleep {
                *self.sleep_elapsed.entry(creature.id).or_default() += dt;
            }

            if self.window_journeys.contains_key(&creature.id) {
                let valid = self
                    .window_journeys
                    .get(&creature.id)
                    .is_some_and(|journey| journey.valid(desktop));
                if !valid {
                    self.window_journeys.remove(&creature.id);
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
                if !route_point_valid {
                    self.window_journeys.remove(&creature.id);
                    settle_interrupted_journey(
                        creature,
                        desktop,
                        &self.save.settings.habitat,
                        &mut self.events,
                    );
                    continue;
                }
                let previous_action = creature.state.action;
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
                    creature.state.surface = surface;
                    creature.state.action = ActionKind::Perch;
                    creature.state.action_elapsed = 0.0;
                    creature.state.action_duration = 3.5;
                    creature.state.velocity = Point::default();
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
                            kind: SurfaceKind::WindowLedge,
                        },
                    );
                    Self::emit(
                        &mut self.events,
                        WorldEvent::ActionStarted {
                            creature_id: creature.id,
                            action: ActionKind::Perch,
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
                nearest_creature_distance: nearest.map(|item| item.0),
                nearest_creature_position: nearest.map(|item| item.1),
                nearest_creature_id: nearest.map(|item| item.2),
                on_window_ledge: creature.state.surface.kind == SurfaceKind::WindowLedge,
                // A ledge is a destination, not a one-time upgrade from the desktop floor.
                // Continuing to search while perched lets creatures climb between stacked
                // application windows and later descend when the desktop arrangement changes.
                reachable_window_ledge: find_nearby_ledge(
                    creature,
                    desktop,
                    &self.save.settings.habitat,
                )
                .is_some(),
                window_changed_nearby: window_changed.contains(&creature.id),
                hour_utc: now.hour(),
            };

            if context.window_changed_nearby
                && creature.state.action != ActionKind::ReactToWindow
                && creature.state.action_elapsed >= 0.25
            {
                creature.state.action = ActionKind::ReactToWindow;
                creature.state.action_elapsed = 0.0;
                creature.state.action_duration = 2.2;
                creature.state.drives.arousal = (creature.state.drives.arousal + 0.3).min(1.0);
                Self::emit(
                    &mut self.events,
                    WorldEvent::WindowReaction {
                        creature_id: creature.id,
                        action: ActionKind::ReactToWindow,
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
                let mut selected_choice = None;
                let (selected, scheduled_ambient) = if discovery_available
                    && !matches!(old, ActionKind::Sleep | ActionKind::ReactToWindow)
                {
                    creature.state.activity_variant = self.ambient_rng.random_range(0..8);
                    self.discovery_remaining =
                        self.ambient_rng.random_range(DISCOVERY_INTERVAL_SECS);
                    (ActionKind::PresentDiscovery, true)
                } else if dangle_available
                    && !matches!(old, ActionKind::ReactToWindow | ActionKind::RideWindow)
                {
                    if let Some(timers) = self.ambient_timers.get_mut(&creature.id) {
                        timers.dangle_remaining =
                            self.ambient_rng.random_range(DANGLE_INTERVAL_SECS);
                    }
                    (ActionKind::Dangle, true)
                } else {
                    creature.state.activity_variant = 0;
                    let choice = choose_action(creature, desktop, context, rng);
                    selected_choice = Some(choice);
                    (choice.action, false)
                };
                let mut next = selected;
                if selected == ActionKind::Perch
                    && let Some((target, surface)) =
                        find_nearby_ledge(creature, desktop, &self.save.settings.habitat)
                {
                    let journey = build_window_journey(creature, target, surface, desktop);
                    next = journey.initial_action();
                    self.window_journeys.insert(creature.id, journey);
                }
                creature.state.action = next;
                creature.state.action_elapsed = 0.0;
                creature.state.action_duration = action_duration(next, rng);
                if let Some(mut choice) = selected_choice {
                    choice.action = next;
                    self.action_choices.insert(creature.id, choice);
                } else {
                    self.action_choices.remove(&creature.id);
                }
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
                        .unwrap_or(creature.state.action_elapsed)
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
                if matches!(
                    next,
                    ActionKind::Greet | ActionKind::Follow | ActionKind::SocialPlay
                ) && let Some((_, _, other_id)) = nearest
                {
                    let gain = if next == ActionKind::Follow {
                        0.004
                    } else {
                        0.012
                    };
                    relationship_updates.push((creature.id, other_id, gain));
                    Self::emit(
                        &mut self.events,
                        WorldEvent::SocialInteraction {
                            a: creature.id,
                            b: other_id,
                            action: next,
                        },
                    );
                }
            }

            let previous_position = creature.state.position;
            let target_point = self
                .action_choices
                .get(&creature.id)
                .and_then(|choice| choice.target_point);
            execute_action(creature, desktop, context, dt, nearest, target_point);
            constrain_to_surface(creature, desktop, &self.save.settings.habitat);
            let inspect_ready = self.save.settings.visible
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
        for (a, b, gain) in relationship_updates {
            if let Some(creature) = self
                .save
                .creatures
                .iter_mut()
                .find(|creature| creature.id == a)
            {
                let affinity = creature.state.relationships.entry(b).or_insert(0.25);
                *affinity = (*affinity + gain).min(1.0);
            }
            if let Some(creature) = self
                .save
                .creatures
                .iter_mut()
                .find(|creature| creature.id == b)
            {
                let affinity = creature.state.relationships.entry(a).or_insert(0.25);
                *affinity = (*affinity + gain).min(1.0);
            }
        }
        let unconstrained: Vec<_> = self
            .interaction
            .as_ref()
            .map(|interaction| interaction.creature_id)
            .into_iter()
            .chain(self.window_journeys.keys().copied())
            .chain(self.tosses.keys().copied())
            .collect();
        keep_creatures_in_habitat(
            &mut self.save.creatures,
            desktop,
            &self.save.settings.habitat,
            &unconstrained,
        );
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

    pub fn rename_creature(
        &mut self,
        creature_id: CreatureId,
        name: &str,
    ) -> Result<(), CreatureNameError> {
        let name = validate_creature_name(name)?;
        if let Some(creature) = self
            .save
            .creatures
            .iter_mut()
            .find(|creature| creature.id == creature_id)
        {
            creature.name = name;
        }
        Ok(())
    }

    pub fn mark_profile_viewed(&mut self, creature_id: CreatureId) -> bool {
        if let Some(creature) = self
            .save
            .creatures
            .iter_mut()
            .find(|creature| creature.id == creature_id)
        {
            if creature.memory.viewed_profile_revision == creature.memory.profile_revision {
                return false;
            }
            creature.memory.viewed_profile_revision = creature.memory.profile_revision;
            return true;
        }
        false
    }

    fn sample_observations(&mut self, dt: f32, desktop: &DesktopSnapshot) {
        if !self.save.settings.visible || self.save.settings.paused {
            return;
        }
        self.observation_elapsed += dt.max(0.0);
        if self.observation_elapsed < OBSERVATION_INTERVAL_SECS {
            return;
        }
        self.observation_elapsed %= OBSERVATION_INTERVAL_SECS;
        let views = self.save.creatures.clone();
        for creature in &self.save.creatures {
            if creature.state.arrival_delay_secs > 0.0 {
                continue;
            }
            let Some((display, region)) = display_region(
                desktop,
                creature.state.surface.monitor_id,
                creature.state.position,
            ) else {
                continue;
            };
            let nearby_creature = views
                .iter()
                .filter(|other| other.id != creature.id && other.state.arrival_delay_secs <= 0.0)
                .map(|other| {
                    (
                        creature.state.position.distance(other.state.position),
                        other.id,
                    )
                })
                .filter(|(distance, _)| *distance <= 120.0)
                .min_by(|a, b| a.0.total_cmp(&b.0))
                .map(|(_, id)| id);
            Self::emit(
                &mut self.events,
                WorldEvent::ObservationElapsed {
                    creature_id: creature.id,
                    display,
                    region,
                    on_ledge: creature.state.surface.kind == SurfaceKind::WindowLedge,
                    riding_window: creature.state.action == ActionKind::RideWindow,
                    nearby_creature,
                    active_seconds: OBSERVATION_INTERVAL_SECS as u8,
                },
            );
        }
    }

    fn project_events(&mut self, _now: OffsetDateTime) {
        if self.projected_events >= self.events.len() {
            return;
        }
        let pending = self.events[self.projected_events..].to_vec();
        self.projected_events = self.events.len();
        let mut changed_profiles = Vec::new();

        for event in pending {
            match event {
                WorldEvent::CreaturePetted { creature_id } => {
                    if let Some(creature) = creature_mut(&mut self.save.creatures, creature_id) {
                        creature.memory.times_petted =
                            creature.memory.times_petted.saturating_add(1);
                        LearnedTendencies::adjust(&mut creature.tendencies.cursor_trust, 3);
                        LearnedTendencies::adjust(&mut creature.tendencies.sociability, 2);
                        changed_profiles.push(creature_id);
                    }
                }
                WorldEvent::DragEnded {
                    creature_id,
                    outcome: DragReleaseKind::Tossed { .. },
                } => {
                    if let Some(creature) = creature_mut(&mut self.save.creatures, creature_id) {
                        creature.memory.times_tossed =
                            creature.memory.times_tossed.saturating_add(1);
                        LearnedTendencies::adjust(&mut creature.tendencies.cursor_trust, -8);
                        changed_profiles.push(creature_id);
                    }
                }
                WorldEvent::CreaturePlaced {
                    creature_id,
                    display,
                    region,
                } => {
                    if let Some(creature) = creature_mut(&mut self.save.creatures, creature_id) {
                        creature.memory.placements = creature.memory.placements.saturating_add(1);
                        match &mut creature.memory.preferred_region {
                            Some(preferred)
                                if preferred.display == display && preferred.cell == region =>
                            {
                                preferred.confidence = preferred.confidence.saturating_add(4);
                                LearnedTendencies::adjust(&mut creature.tendencies.routine, 2);
                            }
                            Some(preferred) if preferred.confidence > 2 => {
                                preferred.confidence = preferred.confidence.saturating_sub(2);
                            }
                            preferred => {
                                *preferred = Some(PreferredRegionMemory {
                                    display,
                                    cell: region.min(8),
                                    confidence: 4,
                                });
                            }
                        }
                        changed_profiles.push(creature_id);
                    }
                }
                WorldEvent::SleepInterrupted { creature_id, .. } => {
                    if let Some(creature) = creature_mut(&mut self.save.creatures, creature_id) {
                        creature.memory.sleep_interruptions =
                            creature.memory.sleep_interruptions.saturating_add(1);
                        LearnedTendencies::adjust(&mut creature.tendencies.sleep_security, -6);
                        changed_profiles.push(creature_id);
                    }
                }
                WorldEvent::CreatureRested {
                    creature_id,
                    uninterrupted_seconds,
                } => {
                    if let Some(creature) = creature_mut(&mut self.save.creatures, creature_id) {
                        creature.memory.longest_sleep_seconds = creature
                            .memory
                            .longest_sleep_seconds
                            .max(uninterrupted_seconds);
                        if uninterrupted_seconds >= 15 * 60 {
                            LearnedTendencies::adjust(&mut creature.tendencies.sleep_security, 2);
                            changed_profiles.push(creature_id);
                        }
                    }
                }
                WorldEvent::ActionCompleted {
                    creature_id,
                    action,
                } => {
                    if let Some(creature) = creature_mut(&mut self.save.creatures, creature_id) {
                        match action {
                            ActionKind::ClimbWindow => {
                                creature.memory.window_climbs =
                                    creature.memory.window_climbs.saturating_add(1);
                                LearnedTendencies::adjust(&mut creature.tendencies.climbing, 2);
                                changed_profiles.push(creature_id);
                            }
                            ActionKind::PresentDiscovery => {
                                creature.memory.discoveries_found =
                                    creature.memory.discoveries_found.saturating_add(1);
                                LearnedTendencies::adjust(&mut creature.tendencies.exploration, 3);
                                changed_profiles.push(creature_id);
                            }
                            ActionKind::SoloPlay | ActionKind::SocialPlay => {
                                creature.memory.play_sessions =
                                    creature.memory.play_sessions.saturating_add(1);
                                LearnedTendencies::adjust(&mut creature.tendencies.play, 2);
                                changed_profiles.push(creature_id);
                            }
                            _ => {}
                        }
                    }
                }
                WorldEvent::ObservationElapsed {
                    creature_id,
                    display,
                    on_ledge,
                    riding_window,
                    active_seconds,
                    ..
                } => {
                    if let Some(creature) = creature_mut(&mut self.save.creatures, creature_id) {
                        creature.memory.milestone_cooldown_active_seconds = creature
                            .memory
                            .milestone_cooldown_active_seconds
                            .saturating_add(u32::from(active_seconds));
                        if on_ledge {
                            let previous = creature.memory.ledge_seconds / 300;
                            creature.memory.ledge_seconds = creature
                                .memory
                                .ledge_seconds
                                .saturating_add(u32::from(active_seconds));
                            let earned = creature.memory.ledge_seconds / 300 - previous;
                            for _ in 0..earned {
                                LearnedTendencies::adjust(&mut creature.tendencies.climbing, 1);
                            }
                            if earned > 0 {
                                changed_profiles.push(creature_id);
                            }
                        }
                        if riding_window {
                            creature.memory.window_ride_seconds = creature
                                .memory
                                .window_ride_seconds
                                .saturating_add(u32::from(active_seconds));
                        }
                        match &mut creature.memory.favorite_display {
                            Some(favorite) if favorite.display == display => {
                                favorite.confidence = favorite.confidence.saturating_add(1);
                            }
                            Some(favorite) if favorite.confidence > 0 => {
                                favorite.confidence = favorite.confidence.saturating_sub(1);
                            }
                            favorite => {
                                *favorite = Some(FavoriteDisplayMemory {
                                    display,
                                    confidence: 1,
                                });
                            }
                        }
                    }
                }
                WorldEvent::HomeAppeared => {
                    for creature in self
                        .save
                        .creatures
                        .iter_mut()
                        .filter(|creature| creature.state.arrival_delay_secs <= 0.0)
                    {
                        creature.memory.home_visits = creature.memory.home_visits.saturating_add(1);
                        LearnedTendencies::adjust(&mut creature.tendencies.home_affinity, 2);
                        changed_profiles.push(creature.id);
                    }
                }
                _ => {}
            }
        }

        changed_profiles.sort_unstable();
        changed_profiles.dedup();
        let can_show_milestone = self.save.settings.visible && !self.save.settings.paused;
        for creature_id in changed_profiles {
            let Some(creature) = creature_mut(&mut self.save.creatures, creature_id) else {
                continue;
            };
            let previous_flags = creature.memory.descriptor_flags;
            if !update_descriptor_flags(&mut creature.memory, creature.tendencies) {
                continue;
            }
            let new_descriptor = ProfileDescriptor::ALL.into_iter().find(|descriptor| {
                previous_flags & descriptor.flag() == 0
                    && creature.memory.descriptor_flags & descriptor.flag() != 0
            });
            let bubble_ready = !creature.memory.milestone_bubble_shown
                || creature.memory.milestone_cooldown_active_seconds >= 12 * 60 * 60;
            let show_milestone = can_show_milestone && new_descriptor.is_some() && bubble_ready;
            if show_milestone {
                creature.memory.milestone_bubble_shown = true;
                creature.memory.milestone_cooldown_active_seconds = 0;
            }
            Self::emit(
                &mut self.events,
                WorldEvent::ProfileChanged {
                    creature_id,
                    new_descriptor,
                    show_milestone,
                },
            );
        }
        self.projected_events = self.events.len();
    }

    fn settle_active_tosses(&mut self, desktop: &DesktopSnapshot) {
        let tossed: Vec<_> = self.tosses.keys().copied().collect();
        for creature_id in tossed {
            let Some(toss) = self.tosses.remove(&creature_id) else {
                continue;
            };
            let Some(creature) = self
                .save
                .creatures
                .iter_mut()
                .find(|creature| creature.id == creature_id)
            else {
                continue;
            };
            if let Some((surface, bounced)) = settle_toss(
                creature,
                &toss,
                desktop,
                &self.save.settings.habitat,
                self.save.settings.window_ledges,
            ) {
                Self::emit(
                    &mut self.events,
                    WorldEvent::TossLanded {
                        creature_id,
                        surface: surface.kind,
                        bounced,
                    },
                );
                Self::emit(
                    &mut self.events,
                    WorldEvent::SurfaceChanged {
                        creature_id,
                        kind: surface.kind,
                    },
                );
            }
        }
    }

    fn update_home_cycle(&mut self, desktop: &DesktopSnapshot) -> bool {
        let timeline_now = self.save.maximum_seen_utc;
        if self
            .save
            .home
            .active_since_utc
            .is_some_and(|started| timeline_now - started >= HOME_DURATION)
        {
            self.dismiss_home(timeline_now, false);
        }

        let due = !self.save.home.is_active()
            && self
                .save
                .home
                .last_disappeared_utc
                .is_none_or(|ended| timeline_now - ended >= HOME_COOLDOWN);
        if due && self.interaction.is_none() && self.resolve_home_monitor(desktop).is_some() {
            self.save.home.active_since_utc = Some(timeline_now);
            self.window_journeys.clear();
            self.tosses.clear();
            Self::emit(&mut self.events, WorldEvent::HomeAppeared);
        }

        if self.save.home.is_active() {
            self.place_colony_at_home(desktop);
        }
        self.save.home.is_active()
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

    fn place_colony_at_home(&mut self, desktop: &DesktopSnapshot) {
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
        // Space the colony by a fraction of how wide a creature actually draws. A flat 18 points
        // was narrower than a single creature at every supported scale, so the whole colony piled
        // onto one point at the shelter and only the last creature drawn stayed visible.
        let creature_width = CREATURE_ART_WIDTH * f32::from(self.save.settings.display_scale)
            / monitor.scale_factor.max(1.0);
        let spacing = creature_width * HOME_SPACING_RATIO;
        for creature in &mut self.save.creatures {
            let offset = inward * f32::from(creature.colony_order) * spacing;
            let mut position = Point {
                x: anchor.x + offset,
                y: anchor.y,
            };
            if !habitat_contains(&self.save.settings.habitat, &monitor, position) {
                position = anchor;
            }
            creature.state.position = position;
            creature.state.velocity = Point::default();
            creature.state.facing_right = inward > 0.0;
            creature.state.surface = SurfaceAttachment {
                kind: SurfaceKind::ScreenFloor,
                monitor_id: monitor.id,
                window_key: None,
                relative_x: ((position.x - monitor.usable_bounds.x) / monitor.usable_bounds.width)
                    .clamp(0.0, 1.0),
            };
            if creature.state.action != ActionKind::Homebound {
                creature.state.action = ActionKind::Homebound;
                creature.state.action_elapsed = 0.0;
                creature.state.action_duration = HOME_DURATION.whole_seconds() as f32;
            }
        }
    }

    fn tick_homebound_creatures(&mut self, now: OffsetDateTime, dt: f32) {
        for creature in &mut self.save.creatures {
            if creature.state.arrival_delay_secs > 0.0 {
                let previous_delay = creature.state.arrival_delay_secs;
                creature.state.arrival_delay_secs = (previous_delay - dt).max(0.0);
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
            creature.state.drives.comfort = (creature.state.drives.comfort + dt * 0.01).min(1.0);
            creature.state.drives.arousal = (creature.state.drives.arousal - dt * 0.04).max(0.0);
        }
    }

    fn dismiss_home(&mut self, now: OffsetDateTime, interrupted: bool) {
        if !self.save.home.is_active() {
            return;
        }
        self.save.home.active_since_utc = None;
        self.save.home.last_disappeared_utc = Some(now);
        for creature in &mut self.save.creatures {
            if creature.state.action == ActionKind::Homebound {
                creature.state.action = ActionKind::Idle;
                creature.state.action_elapsed = 0.0;
                creature.state.action_duration = 2.5;
                creature.state.velocity = Point::default();
            }
        }
        Self::emit(
            &mut self.events,
            WorldEvent::HomeDisappeared { interrupted },
        );
    }

    pub fn handle_command(&mut self, command: WorldCommand, desktop: &DesktopSnapshot) -> bool {
        let handled = match command {
            WorldCommand::BeginInteraction {
                creature_id,
                cursor,
            } => self.begin_interaction(creature_id, cursor),
            WorldCommand::UpdateInteraction { cursor, velocity } => {
                self.update_interaction(cursor, velocity, desktop)
            }
            WorldCommand::EndInteraction { cursor, velocity } => {
                self.end_interaction(cursor, velocity, desktop)
            }
            WorldCommand::CancelInteraction => self.cancel_interaction(),
            WorldCommand::GatherCreatures => {
                self.gather_creatures(desktop);
                true
            }
        };
        self.project_events(self.save.maximum_seen_utc);
        handled
    }

    fn begin_interaction(&mut self, creature_id: CreatureId, cursor: Point) -> bool {
        if self.interaction.is_some() || !self.save.settings.direct_manipulation {
            return false;
        }
        let Some(creature_index) = self.save.creatures.iter().position(|creature| {
            creature.id == creature_id && creature.state.arrival_delay_secs <= 0.0
        }) else {
            return false;
        };
        let interrupted_journey = self.window_journeys.remove(&creature_id).is_some();
        let interrupted_toss = self.tosses.remove(&creature_id);
        self.action_choices.remove(&creature_id);
        let creature = &mut self.save.creatures[creature_index];
        let original_position = interrupted_toss
            .as_ref()
            .map_or(creature.state.position, |toss| toss.last_safe_position);
        let original_surface = interrupted_toss.as_ref().map_or_else(
            || creature.state.surface.clone(),
            |toss| toss.last_safe_surface.clone(),
        );
        let original_action = if interrupted_journey || interrupted_toss.is_some() {
            ActionKind::Idle
        } else {
            creature.state.action
        };
        if interrupted_journey || interrupted_toss.is_some() {
            creature.state.position = original_position;
            creature.state.surface = original_surface.clone();
            creature.state.action = ActionKind::Idle;
            creature.state.action_elapsed = 0.0;
            creature.state.action_duration = 2.5;
            creature.state.velocity = Point::default();
        }
        self.interaction = Some(InteractionSession {
            creature_id,
            press_cursor: cursor,
            max_excursion: 0.0,
            dragging: false,
            grab_offset: Point {
                x: cursor.x - creature.state.position.x,
                y: cursor.y - creature.state.position.y,
            },
            original_position,
            original_surface,
            original_action,
            velocity_samples: [Point::default(); 3],
            velocity_sample_count: 0,
            next_velocity_sample: 0,
        });
        true
    }

    fn start_drag_motion(&mut self) {
        let Some(interaction) = self.interaction.as_mut() else {
            return;
        };
        if interaction.dragging {
            return;
        }
        interaction.dragging = true;
        let creature_id = interaction.creature_id;
        let interrupted_sleep = interaction.original_action == ActionKind::Sleep;
        if self.save.home.is_active() {
            self.dismiss_home(self.save.maximum_seen_utc, true);
        }
        let Some(creature) = self
            .save
            .creatures
            .iter_mut()
            .find(|creature| creature.id == creature_id)
        else {
            return;
        };
        if interrupted_sleep {
            let elapsed_seconds = self
                .sleep_elapsed
                .remove(&creature_id)
                .unwrap_or(creature.state.action_elapsed)
                .max(0.0) as u32;
            Self::emit(
                &mut self.events,
                WorldEvent::SleepInterrupted {
                    creature_id,
                    elapsed_seconds,
                },
            );
        }
        creature.state.action = ActionKind::Dragged;
        creature.state.action_elapsed = 0.0;
        creature.state.action_duration = f32::MAX;
        creature.state.velocity = Point::default();
        creature.state.surface.window_key = None;
        Self::emit(&mut self.events, WorldEvent::DragStarted { creature_id });
    }

    fn update_interaction(
        &mut self,
        cursor: Point,
        velocity: Point,
        desktop: &DesktopSnapshot,
    ) -> bool {
        let Some(interaction) = &mut self.interaction else {
            return false;
        };
        interaction.max_excursion = interaction
            .max_excursion
            .max(interaction.press_cursor.distance(cursor));
        if interaction.max_excursion > DRAG_THRESHOLD && !interaction.dragging {
            self.start_drag_motion();
        }
        let Some(interaction) = &mut self.interaction else {
            return false;
        };
        if !interaction.dragging {
            return true;
        }
        interaction.record_velocity(velocity);
        let Some(creature) = self
            .save
            .creatures
            .iter_mut()
            .find(|creature| creature.id == interaction.creature_id)
        else {
            self.interaction = None;
            return false;
        };
        creature.state.position = Point {
            x: cursor.x - interaction.grab_offset.x,
            y: cursor.y - interaction.grab_offset.y,
        };
        if let Some(monitor) = desktop
            .monitors
            .iter()
            .find(|monitor| monitor.bounds.contains(cursor))
        {
            creature.state.surface.monitor_id = monitor.id;
        }
        true
    }

    fn end_interaction(
        &mut self,
        cursor: Point,
        velocity: Point,
        desktop: &DesktopSnapshot,
    ) -> bool {
        let Some(interaction) = &mut self.interaction else {
            return false;
        };
        interaction.max_excursion = interaction
            .max_excursion
            .max(interaction.press_cursor.distance(cursor));
        if interaction.max_excursion > DRAG_THRESHOLD && !interaction.dragging {
            self.start_drag_motion();
        }
        let Some(mut interaction) = self.interaction.take() else {
            return false;
        };
        if !interaction.dragging {
            let interrupted_seconds =
                (interaction.original_action == ActionKind::Sleep).then(|| {
                    self.sleep_elapsed
                        .remove(&interaction.creature_id)
                        .unwrap_or_default()
                        .max(0.0) as u32
                });
            let Some(creature) = self
                .save
                .creatures
                .iter_mut()
                .find(|creature| creature.id == interaction.creature_id)
            else {
                return false;
            };
            if let Some(elapsed_seconds) = interrupted_seconds {
                Self::emit(
                    &mut self.events,
                    WorldEvent::SleepInterrupted {
                        creature_id: creature.id,
                        elapsed_seconds: elapsed_seconds.max(creature.state.action_elapsed as u32),
                    },
                );
            }
            creature.state.action = ActionKind::PetReaction;
            creature.state.action_elapsed = 0.0;
            creature.state.action_duration = 1.4;
            creature.state.velocity = Point::default();
            creature.state.drives.comfort = (creature.state.drives.comfort + 0.12).min(1.0);
            creature.state.drives.arousal = (creature.state.drives.arousal - 0.08).max(0.0);
            Self::emit(
                &mut self.events,
                WorldEvent::CreaturePetted {
                    creature_id: creature.id,
                },
            );
            Self::emit(
                &mut self.events,
                WorldEvent::ActionStarted {
                    creature_id: creature.id,
                    action: ActionKind::PetReaction,
                },
            );
            return true;
        }
        interaction.record_velocity(velocity);
        let release_velocity = interaction.release_velocity();
        let policy = self.save.settings.habitat.clone();
        let Some(creature) = self
            .save
            .creatures
            .iter_mut()
            .find(|creature| creature.id == interaction.creature_id)
        else {
            return false;
        };
        creature.state.position = Point {
            x: cursor.x - interaction.grab_offset.x,
            y: cursor.y - interaction.grab_offset.y,
        };
        let release_speed = release_velocity.distance(Point::default());
        if release_speed >= TOSS_SPEED_THRESHOLD
            && !self.save.settings.paused
            && !self.save.settings.reduce_motion
        {
            let scaled_speed = (release_speed * TOSS_VELOCITY_SCALE).min(TOSS_MAX_SPEED);
            let scale = scaled_speed / release_speed.max(0.001);
            let initial_velocity = Point {
                x: release_velocity.x * scale,
                y: release_velocity.y * scale,
            };
            creature.state.velocity = initial_velocity;
            creature.state.action = ActionKind::Tossed;
            creature.state.action_elapsed = 0.0;
            creature.state.action_duration = f32::MAX;
            creature.state.surface.window_key = None;
            if let Some(monitor) = desktop
                .monitors
                .iter()
                .find(|monitor| monitor.bounds.contains(creature.state.position))
            {
                creature.state.surface.monitor_id = monitor.id;
            }
            creature.state.surface.kind = SurfaceKind::ScreenFloor;
            self.tosses.insert(
                creature.id,
                TossState {
                    elapsed: 0.0,
                    bounces: 0,
                    last_safe_position: interaction.original_position,
                    last_safe_surface: interaction.original_surface,
                },
            );
            creature.state.drives.arousal = (creature.state.drives.arousal + 0.16).min(1.0);
            Self::emit(
                &mut self.events,
                WorldEvent::DragEnded {
                    creature_id: creature.id,
                    outcome: DragReleaseKind::Tossed {
                        velocity: initial_velocity,
                    },
                },
            );
            return true;
        }
        let support = find_drop_support(cursor, desktop, &policy, self.save.settings.window_ledges)
            .or_else(|| {
                nearest_habitat_point(&policy, &desktop.monitors, cursor).map(
                    |(monitor_id, position)| {
                        (
                            position,
                            SurfaceAttachment {
                                kind: SurfaceKind::ScreenFloor,
                                monitor_id,
                                window_key: None,
                                relative_x: 0.5,
                            },
                        )
                    },
                )
            });
        let Some((position, surface)) = support else {
            creature.state.position = interaction.original_position;
            creature.state.surface = interaction.original_surface;
            creature.state.action = interaction.original_action;
            return false;
        };
        creature.state.position = position;
        creature.state.surface = surface.clone();
        creature.state.velocity = Point::default();
        creature.state.action_elapsed = 0.0;
        if self.save.settings.paused || self.save.settings.reduce_motion {
            creature.state.action = ActionKind::Idle;
            creature.state.action_duration = 3.0;
        } else {
            creature.state.action = ActionKind::Landing;
            creature.state.action_duration = 0.55;
        }
        creature.state.drives.arousal = (creature.state.drives.arousal + 0.08).min(1.0);
        Self::emit(
            &mut self.events,
            WorldEvent::DragEnded {
                creature_id: creature.id,
                outcome: DragReleaseKind::Placed(surface.kind),
            },
        );
        Self::emit(
            &mut self.events,
            WorldEvent::SurfaceChanged {
                creature_id: creature.id,
                kind: surface.kind,
            },
        );
        if let Some((display, region)) = display_region(desktop, surface.monitor_id, position) {
            Self::emit(
                &mut self.events,
                WorldEvent::CreaturePlaced {
                    creature_id: creature.id,
                    display,
                    region,
                },
            );
        }
        true
    }

    fn cancel_interaction(&mut self) -> bool {
        let Some(interaction) = self.interaction.take() else {
            return false;
        };
        if let Some(creature) = self
            .save
            .creatures
            .iter_mut()
            .find(|creature| creature.id == interaction.creature_id)
        {
            creature.state.position = interaction.original_position;
            creature.state.surface = interaction.original_surface;
            creature.state.action = interaction.original_action;
            creature.state.action_elapsed = 0.0;
            creature.state.velocity = Point::default();
        }
        true
    }

    fn gather_creatures(&mut self, desktop: &DesktopSnapshot) {
        self.window_journeys.clear();
        self.tosses.clear();
        self.action_choices.clear();
        let policy = self.save.settings.habitat.clone();
        for creature in &mut self.save.creatures {
            if let Some((monitor_id, position)) =
                nearest_habitat_point(&policy, &desktop.monitors, creature.state.position)
            {
                creature.state.position = position;
                creature.state.surface = SurfaceAttachment {
                    kind: SurfaceKind::ScreenFloor,
                    monitor_id,
                    window_key: None,
                    relative_x: 0.5,
                };
                creature.state.action = ActionKind::Idle;
                creature.state.action_elapsed = 0.0;
                creature.state.action_duration = 3.0;
                creature.state.velocity = Point::default();
            }
        }
    }

    pub fn reset(&mut self, colony_seed: [u8; 32], now: OffsetDateTime, desktop: &DesktopSnapshot) {
        *self = Self::new(colony_seed, now, desktop);
    }

    fn process_arrivals(&mut self, now: OffsetDateTime, desktop: &DesktopSnapshot) {
        let streams = SeedStream::new(self.save.colony_seed);
        let mut arrivals_this_tick = 0_u8;
        for (index, milestone) in ARRIVAL_MILESTONES.into_iter().enumerate() {
            let due = arrival_due_at(self.save.created_at_utc, milestone);
            if !self.save.arrival_state.arrived[index] && self.save.maximum_seen_utc >= due {
                let primary = self.save.creatures.first().cloned();
                let existing_names: Vec<_> = self
                    .save
                    .creatures
                    .iter()
                    .map(|creature| creature.name.clone())
                    .collect();
                let generation = index as u8 + 1;
                let mut creature = generate_creature(
                    &streams,
                    self.save.colony_seed,
                    generation,
                    now,
                    desktop,
                    &existing_names,
                    primary.as_ref(),
                );
                creature.state.arrival_delay_secs = f32::from(arrivals_this_tick) * 15.0;
                self.rngs
                    .insert(creature.id, ChaCha12Rng::from_seed(creature.behavior_seed));
                self.ambient_timers.insert(
                    creature.id,
                    AmbientTimers {
                        inspect_remaining: self.ambient_rng.random_range(INSPECT_INTERVAL_SECS),
                        dangle_remaining: self.ambient_rng.random_range(DANGLE_INTERVAL_SECS),
                    },
                );
                if creature.state.arrival_delay_secs == 0.0 {
                    Self::emit(
                        &mut self.events,
                        WorldEvent::CreatureSpawned {
                            creature_id: creature.id,
                        },
                    );
                }
                self.save.creatures.push(creature);
                self.save.arrival_state.arrived[index] = true;
                arrivals_this_tick += 1;
            }
        }
    }
}

fn generate_creature(
    streams: &SeedStream,
    colony_seed: [u8; 32],
    generation: u8,
    born_at_utc: OffsetDateTime,
    desktop: &DesktopSnapshot,
    existing_names: &[String],
    parent: Option<&Creature>,
) -> Creature {
    let mut appearance_rng = streams.rng("appearance", generation as u64);
    let mut personality_rng = streams.rng("personality", generation as u64);
    let family =
        parent
            .map(|value| value.appearance.family)
            .unwrap_or_else(|| match appearance_rng.random_range(0..3) {
                0 => BodyFamily::Blob,
                1 => BodyFamily::Hopper,
                _ => BodyFamily::SoftQuadruped,
            });
    let scale_percent = [100, 70, 62, 55][generation.min(3) as usize];
    let face_signature = parent
        .map(|value| value.appearance.face_signature)
        .unwrap_or_else(|| appearance_rng.random());
    let palette_index = parent
        .map(|value| value.appearance.palette_index)
        .unwrap_or_else(|| appearance_rng.random_range(0..12));
    let appearance = AppearanceGenome {
        family,
        logical_size: ((appearance_rng.random_range(34..=40) as f32) * scale_percent as f32 / 100.0)
            .round() as u8,
        body_width: mutate_parent(
            parent.map(|p| p.appearance.body_width),
            &mut appearance_rng,
            16,
            27,
        ),
        body_height: mutate_parent(
            parent.map(|p| p.appearance.body_height),
            &mut appearance_rng,
            13,
            24,
        ),
        head_ratio: mutate_float(
            parent.map(|p| p.appearance.head_ratio),
            &mut appearance_rng,
            0.55,
            1.05,
        ),
        roundness: mutate_float(
            parent.map(|p| p.appearance.roundness),
            &mut appearance_rng,
            0.35,
            1.0,
        ),
        leg_length: mutate_parent(
            parent.map(|p| p.appearance.leg_length),
            &mut appearance_rng,
            2,
            8,
        ),
        foot_size: mutate_parent(
            parent.map(|p| p.appearance.foot_size),
            &mut appearance_rng,
            2,
            5,
        ),
        head_appendages: HeadAppendageGenome {
            style: if let Some(parent) = parent {
                if appearance_rng.random_bool(0.75) {
                    parent.appearance.head_appendages.style
                } else {
                    random_head_appendage(&mut appearance_rng)
                }
            } else {
                random_head_appendage(&mut appearance_rng)
            },
            size: mutate_parent(
                parent.map(|p| p.appearance.head_appendages.size),
                &mut appearance_rng,
                2,
                7,
            ),
        },
        tail_style: if let Some(parent) = parent {
            if appearance_rng.random_bool(0.75) {
                parent.appearance.tail_style
            } else {
                random_tail(&mut appearance_rng)
            }
        } else {
            random_tail(&mut appearance_rng)
        },
        tail_length: mutate_parent(
            parent.map(|p| p.appearance.tail_length),
            &mut appearance_rng,
            2,
            10,
        ),
        face: generate_face(parent.map(|p| p.appearance.face), &mut appearance_rng),
        forelimbs: generate_forelimbs(
            family,
            parent.map(|p| p.appearance.forelimbs),
            &mut appearance_rng,
        ),
        effect_motif: if let Some(parent) = parent {
            if appearance_rng.random_bool(0.65) {
                parent.appearance.effect_motif
            } else {
                random_effect_motif(&mut appearance_rng)
            }
        } else {
            random_effect_motif(&mut appearance_rng)
        },
        palette_index,
        pattern: if let Some(parent) = parent {
            if appearance_rng.random_bool(0.6) {
                parent.appearance.pattern
            } else {
                random_pattern(&mut appearance_rng)
            }
        } else {
            random_pattern(&mut appearance_rng)
        },
        pattern_density: mutate_float(
            parent.map(|p| p.appearance.pattern_density),
            &mut appearance_rng,
            0.12,
            0.75,
        ),
        marking_seed: appearance_rng.random(),
        gait_bob: mutate_float(
            parent.map(|p| p.appearance.gait_bob),
            &mut appearance_rng,
            0.15,
            0.9,
        ),
        face_signature,
    };
    let personality = PersonalityGenome {
        activity: personality_rng.random_range(0.2..0.95),
        curiosity: personality_rng.random_range(0.15..0.95),
        boldness: personality_rng.random_range(0.1..0.95),
        playfulness: personality_rng.random_range(0.15..0.95),
        sociability: personality_rng.random_range(0.25..0.95),
        routine_affinity: personality_rng.random_range(0.1..0.9),
        sleep_timing: personality_rng.random_range(0.2..0.9),
        window_tolerance: personality_rng.random_range(0.1..0.95),
        cursor_interest: personality_rng.random_range(0.1..0.95),
        decision_temperature: personality_rng.random_range(0.22..0.75),
    };
    let monitor = desktop
        .monitors
        .iter()
        .find(|monitor| monitor.primary)
        .or_else(|| desktop.monitors.first());
    let (position, monitor_id) = monitor
        .map(|monitor| {
            (
                Point {
                    x: monitor.usable_bounds.x
                        + monitor.usable_bounds.width * 0.35
                        + generation as f32 * 42.0,
                    y: monitor.usable_bounds.bottom() - 4.0,
                },
                monitor.id,
            )
        })
        .unwrap_or((Point { x: 320.0, y: 700.0 }, 0));
    Creature {
        id: u64::from_le_bytes(
            streams.bytes("creature-id", generation as u64)[..8]
                .try_into()
                .unwrap(),
        ),
        generation,
        origin: CreatureOrigin {
            source_colony_seed: colony_seed,
            source_generation: generation,
        },
        colony_order: generation,
        name: default_creature_name(colony_seed, generation, existing_names),
        born_at_utc,
        display_scale_percent: scale_percent,
        appearance,
        personality,
        behavior_seed: streams.bytes("behavior", generation as u64),
        memory: CreatureMemory::default(),
        tendencies: LearnedTendencies::default(),
        routines: RoutineTable::default(),
        state: CreatureState {
            position,
            velocity: Point::default(),
            facing_right: true,
            action: ActionKind::Idle,
            action_elapsed: 0.0,
            action_duration: 2.5,
            drives: Drives::default(),
            surface: SurfaceAttachment {
                kind: SurfaceKind::ScreenFloor,
                monitor_id,
                window_key: None,
                relative_x: 0.35,
            },
            relationships: parent
                .map(|parent| BTreeMap::from([(parent.id, 0.85)]))
                .unwrap_or_default(),
            cursor_cooldown: 0.0,
            activity_variant: 0,
            arrival_delay_secs: 0.0,
        },
    }
}

fn mutate_parent<R: Rng + ?Sized>(parent: Option<u8>, rng: &mut R, min: u8, max: u8) -> u8 {
    parent
        .map(|value| (value as i16 + rng.random_range(-2..=2)).clamp(min as i16, max as i16) as u8)
        .unwrap_or_else(|| rng.random_range(min..=max))
}

fn mutate_float<R: Rng + ?Sized>(parent: Option<f32>, rng: &mut R, min: f32, max: f32) -> f32 {
    parent
        .map(|value| (value + rng.random_range(-0.12..0.12)).clamp(min, max))
        .unwrap_or_else(|| rng.random_range(min..max))
}

fn random_head_appendage<R: Rng + ?Sized>(rng: &mut R) -> HeadAppendageStyle {
    match rng.random_range(0..6) {
        0 => HeadAppendageStyle::None,
        1 => HeadAppendageStyle::Round,
        2 => HeadAppendageStyle::Pointed,
        3 => HeadAppendageStyle::Leaf,
        4 => HeadAppendageStyle::Droop,
        _ => HeadAppendageStyle::Antenna,
    }
}

fn generate_face<R: Rng + ?Sized>(parent: Option<FaceGenome>, rng: &mut R) -> FaceGenome {
    if let Some(parent) = parent {
        return FaceGenome {
            // These genes form the inherited face signature shared by the colony.
            eye_shape: parent.eye_shape,
            eye_size: parent.eye_size,
            eye_spacing: parent.eye_spacing,
            vertical_offset: parent.vertical_offset,
            pupil_style: parent.pupil_style,
            highlight_style: parent.highlight_style,
            brow_style: if rng.random_bool(0.28) {
                random_brow(rng)
            } else {
                parent.brow_style
            },
            mouth_style: if rng.random_bool(0.2) {
                random_mouth(rng)
            } else {
                parent.mouth_style
            },
            cheek_style: if rng.random_bool(0.35) {
                random_cheek(rng)
            } else {
                parent.cheek_style
            },
        };
    }
    FaceGenome {
        eye_shape: match rng.random_range(0..3) {
            0 => EyeShape::Round,
            1 => EyeShape::Tall,
            _ => EyeShape::SoftSquare,
        },
        eye_size: rng.random_range(1..=2),
        eye_spacing: rng.random_range(4..=7),
        vertical_offset: rng.random_range(-1..=1),
        pupil_style: match rng.random_range(0..3) {
            0 => PupilStyle::Dot,
            1 => PupilStyle::Wide,
            _ => PupilStyle::Spark,
        },
        highlight_style: match rng.random_range(0..3) {
            0 => HighlightStyle::Single,
            1 => HighlightStyle::Double,
            _ => HighlightStyle::Diagonal,
        },
        brow_style: random_brow(rng),
        mouth_style: random_mouth(rng),
        cheek_style: random_cheek(rng),
    }
}

fn generate_forelimbs<R: Rng + ?Sized>(
    family: BodyFamily,
    parent: Option<ForelimbGenome>,
    rng: &mut R,
) -> ForelimbGenome {
    let (style, tip_style) = match family {
        BodyFamily::Blob => (
            if rng.random_bool(0.5) {
                ForelimbStyle::SoftNub
            } else {
                ForelimbStyle::Pseudopod
            },
            LimbTipStyle::Round,
        ),
        BodyFamily::Hopper => (ForelimbStyle::MittenArm, LimbTipStyle::Mitten),
        BodyFamily::SoftQuadruped => (ForelimbStyle::FrontPaw, LimbTipStyle::Paw),
    };
    ForelimbGenome {
        style: parent.map_or(style, |value| value.style),
        length: mutate_parent(parent.map(|value| value.length), rng, 3, 7),
        thickness: mutate_parent(parent.map(|value| value.thickness), rng, 1, 2),
        tip_style: parent.map_or(tip_style, |value| value.tip_style),
        rest_pose: if let Some(parent) = parent {
            if rng.random_bool(0.3) {
                random_rest_pose(rng)
            } else {
                parent.rest_pose
            }
        } else {
            random_rest_pose(rng)
        },
    }
}

fn random_brow<R: Rng + ?Sized>(rng: &mut R) -> BrowStyle {
    match rng.random_range(0..3) {
        0 => BrowStyle::None,
        1 => BrowStyle::Soft,
        _ => BrowStyle::Bold,
    }
}

fn random_mouth<R: Rng + ?Sized>(rng: &mut R) -> MouthStyle {
    match rng.random_range(0..4) {
        0 => MouthStyle::Tiny,
        1 => MouthStyle::Smile,
        2 => MouthStyle::Cat,
        _ => MouthStyle::Beak,
    }
}

fn random_cheek<R: Rng + ?Sized>(rng: &mut R) -> CheekStyle {
    match rng.random_range(0..3) {
        0 => CheekStyle::None,
        1 => CheekStyle::Dots,
        _ => CheekStyle::Blush,
    }
}

fn random_rest_pose<R: Rng + ?Sized>(rng: &mut R) -> RestPose {
    match rng.random_range(0..3) {
        0 => RestPose::AtSides,
        1 => RestPose::Folded,
        _ => RestPose::Together,
    }
}

fn random_effect_motif<R: Rng + ?Sized>(rng: &mut R) -> EffectMotif {
    match rng.random_range(0..6) {
        0 => EffectMotif::None,
        1 => EffectMotif::Dot,
        2 => EffectMotif::Star,
        3 => EffectMotif::Heart,
        4 => EffectMotif::Leaf,
        _ => EffectMotif::Spark,
    }
}

fn random_tail<R: Rng + ?Sized>(rng: &mut R) -> TailStyle {
    match rng.random_range(0..5) {
        0 => TailStyle::None,
        1 => TailStyle::Stub,
        2 => TailStyle::Taper,
        3 => TailStyle::Tuft,
        _ => TailStyle::Curl,
    }
}

fn random_pattern<R: Rng + ?Sized>(rng: &mut R) -> PatternKind {
    match rng.random_range(0..7) {
        0 => PatternKind::Solid,
        1 => PatternKind::Patches,
        2 => PatternKind::Spots,
        3 => PatternKind::Stripes,
        4 => PatternKind::Mask,
        5 => PatternKind::Socks,
        _ => PatternKind::Tips,
    }
}

fn update_drives(creature: &mut Creature, dt: f32) {
    let moving = matches!(
        creature.state.action,
        ActionKind::Traverse
            | ActionKind::Sprint
            | ActionKind::InvestigateCursor
            | ActionKind::AvoidCursor
            | ActionKind::Follow
            | ActionKind::ClimbWindow
    );
    let sleeping = creature.state.action == ActionKind::Sleep;
    if sleeping {
        creature.state.drives.energy = (creature.state.drives.energy + dt * 0.035).min(1.0);
        creature.state.drives.sleep_pressure =
            (creature.state.drives.sleep_pressure - dt * 0.05).max(0.0);
        creature.state.drives.arousal = (creature.state.drives.arousal - dt * 0.08).max(0.0);
    } else {
        let movement_cost = if creature.state.action == ActionKind::Sprint {
            0.02
        } else if moving {
            0.008
        } else {
            0.002
        };
        creature.state.drives.energy = (creature.state.drives.energy - dt * movement_cost).max(0.0);
        creature.state.drives.sleep_pressure =
            (creature.state.drives.sleep_pressure + dt * 0.0025).min(1.0);
        creature.state.drives.boredom = (creature.state.drives.boredom + dt * 0.004).min(1.0);
        creature.state.drives.social_need =
            (creature.state.drives.social_need + dt * 0.0015).min(1.0);
        creature.state.drives.arousal = (creature.state.drives.arousal - dt * 0.025).max(0.0);
    }
}

fn execute_action(
    creature: &mut Creature,
    desktop: &DesktopSnapshot,
    context: BehaviorContext,
    dt: f32,
    nearest: Option<(f32, Point, CreatureId)>,
    selected_target: Option<Point>,
) {
    let speed = 24.0 + creature.personality.activity * 34.0;
    let mut target_x = None;
    match creature.state.action {
        ActionKind::Traverse if selected_target.is_some() => {
            target_x = selected_target.map(|target| target.x);
        }
        ActionKind::Traverse | ActionKind::Sprint => {
            let direction = if creature.state.facing_right {
                1.0
            } else {
                -1.0
            };
            let multiplier = if creature.state.action == ActionKind::Sprint {
                2.35
            } else {
                1.0
            };
            creature.state.velocity.x = direction * speed * multiplier;
        }
        ActionKind::InvestigateCursor if desktop.cursor.available => {
            target_x = Some(desktop.cursor.position.x)
        }
        ActionKind::AvoidCursor if desktop.cursor.available => {
            target_x = Some(
                creature.state.position.x
                    + (creature.state.position.x - desktop.cursor.position.x).signum() * 180.0,
            );
            creature.state.cursor_cooldown = 4.0;
            creature.state.drives.arousal = (creature.state.drives.arousal + dt * 0.7).min(1.0);
        }
        ActionKind::Follow | ActionKind::Greet | ActionKind::SocialPlay => {
            target_x = nearest.map(|item| item.1.x);
            creature.state.drives.social_need =
                (creature.state.drives.social_need - dt * 0.08).max(0.0);
        }
        ActionKind::SoloPlay => {
            creature.state.drives.boredom = (creature.state.drives.boredom - dt * 0.09).max(0.0);
        }
        ActionKind::Eat => {
            creature.state.drives.energy = (creature.state.drives.energy + dt * 0.025).min(1.0);
            creature.state.drives.comfort = (creature.state.drives.comfort + dt * 0.018).min(1.0);
            creature.state.drives.boredom = (creature.state.drives.boredom - dt * 0.018).max(0.0);
        }
        ActionKind::Drink => {
            creature.state.drives.comfort = (creature.state.drives.comfort + dt * 0.024).min(1.0);
            creature.state.drives.arousal = (creature.state.drives.arousal - dt * 0.04).max(0.0);
            creature.state.drives.curiosity_satisfaction =
                (creature.state.drives.curiosity_satisfaction + dt * 0.012).min(1.0);
        }
        ActionKind::Dangle => {
            creature.state.drives.comfort = (creature.state.drives.comfort + dt * 0.012).min(1.0);
            creature.state.drives.boredom = (creature.state.drives.boredom - dt * 0.025).max(0.0);
        }
        ActionKind::InspectScreen => {
            creature.state.drives.curiosity_satisfaction =
                (creature.state.drives.curiosity_satisfaction + dt * 0.055).min(1.0);
            creature.state.drives.boredom = (creature.state.drives.boredom - dt * 0.035).max(0.0);
        }
        ActionKind::PresentDiscovery => {
            creature.state.drives.curiosity_satisfaction =
                (creature.state.drives.curiosity_satisfaction + dt * 0.035).min(1.0);
            creature.state.drives.comfort = (creature.state.drives.comfort + dt * 0.012).min(1.0);
        }
        ActionKind::ReactToWindow => {
            creature.state.velocity.x = if creature.state.facing_right {
                speed * 1.4
            } else {
                -speed * 1.4
            };
            creature.state.drives.arousal = (creature.state.drives.arousal + dt * 0.5).min(1.0);
        }
        _ => creature.state.velocity.x *= (1.0 - dt * 8.0).max(0.0),
    }
    if let Some(target) = target_x {
        let dx = target - creature.state.position.x;
        creature.state.facing_right = dx >= 0.0;
        creature.state.velocity.x = dx.signum() * speed;
    }
    creature.state.position.x += creature.state.velocity.x * dt;
    if !context.on_window_ledge && creature.state.action == ActionKind::Perch {
        creature.state.velocity.x = 0.0;
    }
}

fn action_duration<R: Rng + ?Sized>(action: ActionKind, rng: &mut R) -> f32 {
    let range = match action {
        ActionKind::Sleep => 18.0..45.0,
        ActionKind::Traverse | ActionKind::Follow => 4.0..11.0,
        ActionKind::Sprint => 2.5..5.5,
        ActionKind::InvestigateCursor | ActionKind::AvoidCursor | ActionKind::ReactToWindow => {
            2.0..5.0
        }
        ActionKind::Greet | ActionKind::SocialPlay | ActionKind::SoloPlay => 3.0..8.0,
        ActionKind::Eat | ActionKind::Drink => 5.0..9.0,
        ActionKind::Dangle => 4.0..7.0,
        ActionKind::InspectScreen => 3.0..5.0,
        ActionKind::PresentDiscovery => 3.8..4.2,
        _ => 3.0..10.0,
    };
    rng.random_range(range)
}

fn reinforce_habit(creature: &mut Creature, action: ActionKind, hour_utc: u8) {
    let key = routine_key(
        creature.state.surface.kind,
        creature.state.surface.relative_x,
        action,
        hour_utc,
    );
    creature.routines.reinforce(key);
}

fn keep_creatures_in_habitat(
    creatures: &mut [Creature],
    desktop: &DesktopSnapshot,
    policy: &HabitatPolicy,
    unconstrained: &[CreatureId],
) {
    let primary = desktop
        .monitors
        .iter()
        .find(|monitor| monitor.primary)
        .or_else(|| desktop.monitors.first());
    for creature in creatures {
        if unconstrained.contains(&creature.id) {
            continue;
        }
        let monitor = desktop
            .monitors
            .iter()
            .find(|monitor| monitor.id == creature.state.surface.monitor_id)
            .or(primary);
        if let Some(monitor) = monitor {
            let regions = accessible_regions(policy, monitor);
            if regions.is_empty() {
                if let Some((monitor_id, position)) =
                    nearest_habitat_point(policy, &desktop.monitors, creature.state.position)
                {
                    creature.state.position = position;
                    creature.state.surface = SurfaceAttachment {
                        kind: SurfaceKind::ScreenFloor,
                        monitor_id,
                        window_key: None,
                        relative_x: 0.5,
                    };
                }
                continue;
            }
            let bounds = monitor.usable_bounds;
            if creature.state.position.x <= bounds.x + 8.0 {
                creature.state.position.x = bounds.x + 8.0;
                creature.state.facing_right = true;
            } else if creature.state.position.x >= bounds.right() - 8.0 {
                creature.state.position.x = bounds.right() - 8.0;
                creature.state.facing_right = false;
            }
            if !regions
                .iter()
                .any(|region| region.contains(creature.state.position))
                && let Some((monitor_id, position)) =
                    nearest_habitat_point(policy, &desktop.monitors, creature.state.position)
            {
                creature.state.position = position;
                creature.state.surface = SurfaceAttachment {
                    kind: SurfaceKind::ScreenFloor,
                    monitor_id,
                    window_key: None,
                    relative_x: 0.5,
                };
            }
        }
    }
}

fn update_surface_attachments(
    creatures: &mut [Creature],
    desktop: &DesktopSnapshot,
    previous: &BTreeMap<WindowKey, DesktopRect>,
    events: &mut Vec<WorldEvent>,
) {
    for creature in creatures {
        let Some(key) = creature.state.surface.window_key else {
            continue;
        };
        let current = desktop
            .windows
            .iter()
            .find(|window| window.key == key && window.visible && !window.minimized);
        match current {
            Some(window) => {
                let old = previous.get(&key).copied();
                let relative = creature.state.surface.relative_x.clamp(0.05, 0.95);
                creature.state.position.x = window.bounds.x + window.bounds.width * relative;
                creature.state.position.y = window.bounds.y;
                if old.is_some_and(|old| old != window.bounds) {
                    let old = old.expect("changed window has previous bounds");
                    let movement = Point {
                        x: window.bounds.x - old.x,
                        y: window.bounds.y - old.y,
                    };
                    let moved_distance = movement.distance(Point::default());
                    let resized = (window.bounds.width - old.width).abs()
                        + (window.bounds.height - old.height).abs();
                    let rapid = moved_distance > 80.0 || resized > 90.0;
                    creature.state.action = if rapid && creature.personality.window_tolerance < 0.72
                    {
                        creature.state.drives.arousal =
                            (creature.state.drives.arousal + 0.35).min(1.0);
                        ActionKind::ReactToWindow
                    } else {
                        ActionKind::RideWindow
                    };
                    creature.state.action_elapsed = 0.0;
                    creature.state.action_duration = 2.5;
                    World::emit(
                        events,
                        WorldEvent::WindowReaction {
                            creature_id: creature.id,
                            action: creature.state.action,
                        },
                    );
                }
            }
            None => {
                let monitor = desktop
                    .monitors
                    .iter()
                    .find(|monitor| monitor.id == creature.state.surface.monitor_id)
                    .or_else(|| desktop.monitors.iter().find(|monitor| monitor.primary))
                    .or_else(|| desktop.monitors.first());
                if let Some(monitor) = monitor {
                    creature.state.position.y = monitor.usable_bounds.bottom() - 4.0;
                    creature.state.position.x = creature.state.position.x.clamp(
                        monitor.usable_bounds.x + 8.0,
                        monitor.usable_bounds.right() - 8.0,
                    );
                    creature.state.surface = SurfaceAttachment {
                        kind: SurfaceKind::ScreenFloor,
                        monitor_id: monitor.id,
                        window_key: None,
                        relative_x: (creature.state.position.x - monitor.usable_bounds.x)
                            / monitor.usable_bounds.width,
                    };
                    creature.state.action = ActionKind::ReactToWindow;
                    creature.state.action_elapsed = 0.0;
                    creature.state.action_duration = 3.0;
                    World::emit(
                        events,
                        WorldEvent::SurfaceChanged {
                            creature_id: creature.id,
                            kind: SurfaceKind::ScreenFloor,
                        },
                    );
                    World::emit(
                        events,
                        WorldEvent::WindowReaction {
                            creature_id: creature.id,
                            action: ActionKind::ReactToWindow,
                        },
                    );
                }
            }
        }
    }
}

fn advance_toss(
    creature: &mut Creature,
    toss: &mut TossState,
    dt: f32,
    desktop: &DesktopSnapshot,
    policy: &HabitatPolicy,
    reduce_motion: bool,
    window_ledges: bool,
) -> Option<(SurfaceAttachment, bool)> {
    toss.elapsed += dt;
    if reduce_motion || toss.elapsed >= TOSS_MAX_DURATION {
        return settle_toss(creature, toss, desktop, policy, window_ledges);
    }

    let previous = creature.state.position;
    creature.state.velocity.y += TOSS_GRAVITY * dt;
    creature.state.velocity.x *= (-TOSS_HORIZONTAL_DRAG * dt).exp();
    let next = Point {
        x: previous.x + creature.state.velocity.x * dt,
        y: previous.y + creature.state.velocity.y * dt,
    };

    if creature.state.velocity.y > 0.0
        && let Some((impact, surface)) =
            find_swept_support(previous, next, desktop, policy, window_ledges)
    {
        creature.state.position = impact;
        if toss.bounces == 0 && creature.state.velocity.y >= TOSS_MIN_BOUNCE_SPEED {
            toss.bounces = 1;
            creature.state.velocity.x *= TOSS_BOUNCE_HORIZONTAL_RETENTION;
            creature.state.velocity.y *= -TOSS_BOUNCE_RESTITUTION;
            return None;
        }
        creature.state.surface = surface.clone();
        creature.state.velocity = Point::default();
        creature.state.action = ActionKind::Landing;
        creature.state.action_elapsed = 0.0;
        creature.state.action_duration = 0.55;
        return Some((surface, toss.bounces > 0));
    }

    creature.state.position = next;
    if let Some(monitor) = desktop
        .monitors
        .iter()
        .find(|monitor| monitor.bounds.contains(next))
    {
        creature.state.surface.monitor_id = monitor.id;
        creature.state.facing_right = creature.state.velocity.x >= 0.0;
        None
    } else {
        settle_toss(creature, toss, desktop, policy, window_ledges)
    }
}

fn settle_toss(
    creature: &mut Creature,
    toss: &TossState,
    desktop: &DesktopSnapshot,
    policy: &HabitatPolicy,
    window_ledges: bool,
) -> Option<(SurfaceAttachment, bool)> {
    let support = find_drop_support(creature.state.position, desktop, policy, window_ledges)
        .or_else(|| {
            nearest_habitat_point(policy, &desktop.monitors, creature.state.position).map(
                |(monitor_id, position)| {
                    (
                        position,
                        SurfaceAttachment {
                            kind: SurfaceKind::ScreenFloor,
                            monitor_id,
                            window_key: None,
                            relative_x: 0.5,
                        },
                    )
                },
            )
        });
    let (position, surface) =
        support.unwrap_or_else(|| (toss.last_safe_position, toss.last_safe_surface.clone()));
    creature.state.position = position;
    creature.state.surface = surface.clone();
    creature.state.velocity = Point::default();
    creature.state.action = ActionKind::Landing;
    creature.state.action_elapsed = 0.0;
    creature.state.action_duration = 0.55;
    Some((surface, toss.bounces > 0))
}

fn find_swept_support(
    previous: Point,
    next: Point,
    desktop: &DesktopSnapshot,
    policy: &HabitatPolicy,
    window_ledges: bool,
) -> Option<(Point, SurfaceAttachment)> {
    let dy = next.y - previous.y;
    if dy <= 0.0 {
        return None;
    }
    let mut candidates = Vec::new();
    for window in desktop
        .windows
        .iter()
        .filter(|window| window_ledges && window.visible && !window.minimized)
    {
        let t = (window.bounds.y - previous.y) / dy;
        if !(0.0..=1.0).contains(&t) {
            continue;
        }
        let x = lerp(previous.x, next.x, t);
        if x < window.bounds.x + 12.0 || x > window.bounds.right() - 12.0 {
            continue;
        }
        let point = Point {
            x,
            y: window.bounds.y,
        };
        let Some(monitor) = desktop
            .monitors
            .iter()
            .find(|monitor| monitor.bounds.contains(point))
        else {
            continue;
        };
        if habitat_contains(policy, monitor, point) {
            candidates.push((
                t,
                point,
                SurfaceAttachment {
                    kind: SurfaceKind::WindowLedge,
                    monitor_id: monitor.id,
                    window_key: Some(window.key),
                    relative_x: ((x - window.bounds.x) / window.bounds.width).clamp(0.05, 0.95),
                },
            ));
        }
    }
    for monitor in &desktop.monitors {
        for region in accessible_regions(policy, monitor) {
            let floor_y = region.bottom() - 4.0;
            let t = (floor_y - previous.y) / dy;
            if !(0.0..=1.0).contains(&t) {
                continue;
            }
            let x = lerp(previous.x, next.x, t);
            if x < region.x + 8.0 || x > region.right() - 8.0 {
                continue;
            }
            candidates.push((
                t,
                Point { x, y: floor_y },
                SurfaceAttachment {
                    kind: SurfaceKind::ScreenFloor,
                    monitor_id: monitor.id,
                    window_key: None,
                    relative_x: ((x - region.x) / region.width).clamp(0.0, 1.0),
                },
            ));
        }
    }
    candidates
        .into_iter()
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, point, surface)| (point, surface))
}

fn build_window_journey(
    creature: &Creature,
    target: Point,
    mut surface: SurfaceAttachment,
    desktop: &DesktopSnapshot,
) -> WindowJourney {
    let start = creature.state.position;
    let upward = target.y < start.y;
    let Some(window) = upward
        .then_some(surface.window_key)
        .flatten()
        .and_then(|key| desktop.windows.iter().find(|window| window.key == key))
    else {
        let distance = start.distance(target);
        return WindowJourney::Hop(HopJourney {
            start,
            target,
            surface,
            elapsed: 0.0,
            duration: (distance / 280.0).clamp(1.0, 2.6),
        });
    };

    let left_track = window.bounds.x + 5.0;
    let right_track = window.bounds.right() - 5.0;
    let use_left = (start.x - left_track).abs() <= (start.x - right_track).abs();
    let track_x = if use_left { left_track } else { right_track };
    let target_x = if use_left {
        window.bounds.x + 18.0
    } else {
        window.bounds.right() - 18.0
    };
    let approach = Point {
        x: track_x,
        y: start.y,
    };
    let climb_end = Point {
        x: track_x,
        // Stop with the body just below its final contact point, then lift and move inward in one
        // smooth mantle. Previously the contact point reached the ledge before the horizontal
        // pull began, producing a visible hold-and-reposition hitch.
        y: window.bounds.y + MANTLE_LIFT_POINTS,
    };
    let target = Point {
        x: target_x,
        y: window.bounds.y,
    };
    surface.relative_x = ((target_x - window.bounds.x) / window.bounds.width).clamp(0.05, 0.95);
    let traverse_speed = 24.0 + creature.personality.activity * 34.0;
    let climb_speed = 44.0 + creature.personality.activity * 18.0;
    WindowJourney::Climb(ClimbJourney {
        target_window: window.key,
        target_bounds: window.bounds,
        start,
        approach,
        climb_end,
        target,
        surface,
        elapsed: 0.0,
        approach_duration: (start.distance(approach) / traverse_speed).clamp(0.0, 6.0),
        climb_duration: (approach.distance(climb_end) / climb_speed).max(0.35),
        mantle_duration: 0.7,
    })
}

fn settle_interrupted_journey(
    creature: &mut Creature,
    desktop: &DesktopSnapshot,
    policy: &HabitatPolicy,
    events: &mut Vec<WorldEvent>,
) {
    let support = find_drop_support(creature.state.position, desktop, policy, true).or_else(|| {
        nearest_habitat_point(policy, &desktop.monitors, creature.state.position).map(
            |(monitor_id, position)| {
                (
                    position,
                    SurfaceAttachment {
                        kind: SurfaceKind::ScreenFloor,
                        monitor_id,
                        window_key: None,
                        relative_x: 0.5,
                    },
                )
            },
        )
    });
    if let Some((position, surface)) = support {
        creature.state.position = position;
        creature.state.surface = surface.clone();
        creature.state.action = ActionKind::ReactToWindow;
        creature.state.action_elapsed = 0.0;
        creature.state.action_duration = 2.2;
        creature.state.velocity = Point::default();
        World::emit(
            events,
            WorldEvent::SurfaceChanged {
                creature_id: creature.id,
                kind: surface.kind,
            },
        );
        World::emit(
            events,
            WorldEvent::WindowReaction {
                creature_id: creature.id,
                action: ActionKind::ReactToWindow,
            },
        );
    }
}

fn crossed_inspection_anchor(
    creature: &Creature,
    previous_x: f32,
    desktop: &DesktopSnapshot,
    policy: &HabitatPolicy,
) -> bool {
    let path_min = previous_x.min(creature.state.position.x) - INSPECTION_RADIUS;
    let path_max = previous_x.max(creature.state.position.x) + INSPECTION_RADIUS;
    if let Some(key) = creature.state.surface.window_key
        && let Some(window) = desktop
            .windows
            .iter()
            .find(|window| window.key == key && window.visible && !window.minimized)
    {
        return [1.0_f32 / 3.0, 2.0 / 3.0]
            .into_iter()
            .map(|fraction| window.bounds.x + window.bounds.width * fraction)
            .any(|anchor| (path_min..=path_max).contains(&anchor));
    }

    let Some(monitor) = desktop
        .monitors
        .iter()
        .find(|monitor| monitor.id == creature.state.surface.monitor_id)
    else {
        return false;
    };
    accessible_regions(policy, monitor)
        .into_iter()
        .filter(|region| {
            (creature.state.position.y - (region.bottom() - 4.0)).abs() <= INSPECTION_RADIUS
        })
        .flat_map(|region| {
            [1.0_f32 / 3.0, 2.0 / 3.0].map(move |fraction| region.x + region.width * fraction)
        })
        .any(|anchor| (path_min..=path_max).contains(&anchor))
}

fn find_nearby_ledge(
    creature: &Creature,
    desktop: &DesktopSnapshot,
    policy: &HabitatPolicy,
) -> Option<(Point, SurfaceAttachment)> {
    let current_window = creature.state.surface.window_key;
    let candidate = desktop
        .windows
        .iter()
        .filter(|window| window.visible && !window.minimized && window.bounds.width >= 120.0)
        .filter(|window| Some(window.key) != current_window)
        .filter_map(|window| {
            let ledge_x = creature
                .state
                .position
                .x
                .clamp(window.bounds.x + 12.0, window.bounds.right() - 12.0);
            let dx = (creature.state.position.x - ledge_x).abs();
            let dy = (creature.state.position.y - window.bounds.y).abs();
            let monitor = desktop.monitors.iter().find(|monitor| {
                monitor.bounds.contains(Point {
                    x: ledge_x,
                    y: window.bounds.y,
                })
            })?;
            (dx <= 360.0
                && (36.0..=640.0).contains(&dy)
                && habitat_contains(
                    policy,
                    monitor,
                    Point {
                        x: ledge_x,
                        y: window.bounds.y,
                    },
                ))
            // Nearby intermediate ledges remain easiest, while the vertical-progress bonus makes
            // a visibly different elevation preferable to another almost-level window.
            .then_some((dx * 0.65 + dy * 0.12, window, ledge_x, monitor.id))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0));
    candidate.map(|(_, window, ledge_x, monitor_id)| {
        (
            Point {
                x: ledge_x,
                y: window.bounds.y,
            },
            SurfaceAttachment {
                kind: SurfaceKind::WindowLedge,
                monitor_id,
                window_key: Some(window.key),
                relative_x: ((ledge_x - window.bounds.x) / window.bounds.width).clamp(0.05, 0.95),
            },
        )
    })
}

fn constrain_to_surface(
    creature: &mut Creature,
    desktop: &DesktopSnapshot,
    policy: &HabitatPolicy,
) {
    if let Some(key) = creature.state.surface.window_key
        && let Some(window) = desktop.windows.iter().find(|window| window.key == key)
    {
        let Some(monitor) = desktop
            .monitors
            .iter()
            .find(|monitor| monitor.id == creature.state.surface.monitor_id)
        else {
            return;
        };
        let intervals: Vec<_> = accessible_regions(policy, monitor)
            .into_iter()
            .filter(|region| window.bounds.y >= region.y && window.bounds.y <= region.bottom())
            .filter_map(|region| {
                let min = (window.bounds.x + 12.0).max(region.x + 8.0);
                let max = (window.bounds.right() - 12.0).min(region.right() - 8.0);
                (max > min).then_some((min, max))
            })
            .collect();
        let Some((min_x, max_x)) = intervals.iter().copied().min_by(|a, b| {
            distance_to_interval(creature.state.position.x, *a)
                .total_cmp(&distance_to_interval(creature.state.position.x, *b))
        }) else {
            return;
        };
        if creature.state.position.x <= min_x {
            creature.state.position.x = min_x;
            creature.state.facing_right = true;
        } else if creature.state.position.x >= max_x {
            creature.state.position.x = max_x;
            creature.state.facing_right = false;
        }
        creature.state.position.y = window.bounds.y;
        creature.state.surface.relative_x =
            ((creature.state.position.x - window.bounds.x) / window.bounds.width).clamp(0.05, 0.95);
        return;
    }
    if creature.state.surface.kind == SurfaceKind::ScreenFloor
        && let Some(monitor) = desktop
            .monitors
            .iter()
            .find(|monitor| monitor.id == creature.state.surface.monitor_id)
    {
        let regions = accessible_regions(policy, monitor);
        if let Some(region) = regions.iter().min_by(|a, b| {
            distance_to_interval(creature.state.position.x, (a.x + 8.0, a.right() - 8.0)).total_cmp(
                &distance_to_interval(creature.state.position.x, (b.x + 8.0, b.right() - 8.0)),
            )
        }) {
            creature.state.position.x = creature
                .state
                .position
                .x
                .clamp(region.x + 8.0, region.right() - 8.0);
            creature.state.position.y = region.bottom() - 4.0;
            creature.state.surface.relative_x =
                ((creature.state.position.x - region.x) / region.width).clamp(0.0, 1.0);
        }
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

fn find_drop_support(
    cursor: Point,
    desktop: &DesktopSnapshot,
    policy: &HabitatPolicy,
    window_ledges: bool,
) -> Option<(Point, SurfaceAttachment)> {
    let monitor = desktop
        .monitors
        .iter()
        .find(|monitor| monitor.bounds.contains(cursor))
        .or_else(|| desktop.monitors.iter().find(|monitor| monitor.primary))
        .or_else(|| desktop.monitors.first())?;
    let regions = accessible_regions(policy, monitor);
    let mut candidates = Vec::new();
    for window in desktop
        .windows
        .iter()
        .filter(|window| window_ledges && window.visible && !window.minimized)
    {
        let x = cursor
            .x
            .clamp(window.bounds.x + 12.0, window.bounds.right() - 12.0);
        let point = Point {
            x,
            y: window.bounds.y,
        };
        if point.y >= cursor.y && regions.iter().any(|region| region.contains(point)) {
            candidates.push((
                cursor.distance(point),
                point,
                SurfaceAttachment {
                    kind: SurfaceKind::WindowLedge,
                    monitor_id: monitor.id,
                    window_key: Some(window.key),
                    relative_x: ((x - window.bounds.x) / window.bounds.width).clamp(0.05, 0.95),
                },
            ));
        }
    }
    for region in regions {
        let point = Point {
            x: cursor.x.clamp(region.x + 8.0, region.right() - 8.0),
            y: region.bottom() - 4.0,
        };
        if point.y >= cursor.y {
            candidates.push((
                cursor.distance(point),
                point,
                SurfaceAttachment {
                    kind: SurfaceKind::ScreenFloor,
                    monitor_id: monitor.id,
                    window_key: None,
                    relative_x: ((point.x - region.x) / region.width).clamp(0.0, 1.0),
                },
            ));
        }
    }
    candidates
        .into_iter()
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, point, surface)| (point, surface))
}

fn window_change_near_creatures(
    previous: &BTreeMap<WindowKey, DesktopRect>,
    desktop: &DesktopSnapshot,
    creatures: &[Creature],
) -> Vec<CreatureId> {
    let mut changed = std::collections::BTreeSet::new();
    for window in &desktop.windows {
        let moved = previous
            .get(&window.key)
            .is_none_or(|old| old != &window.bounds);
        if moved {
            for creature in creatures {
                if distance_to_rect(creature.state.position, window.bounds) <= 260.0 {
                    changed.insert(creature.id);
                }
            }
        }
    }
    for (key, bounds) in previous {
        if !desktop.windows.iter().any(|window| window.key == *key) {
            for creature in creatures {
                if distance_to_rect(creature.state.position, *bounds) <= 260.0 {
                    changed.insert(creature.id);
                }
            }
        }
    }
    changed.into_iter().collect()
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
mod tests {
    use super::*;
    use time::macros::datetime;

    fn desktop() -> DesktopSnapshot {
        DesktopSnapshot {
            monitors: vec![MonitorInfo {
                id: 1,
                display_key: DisplayKey([1; 16]),
                bounds: DesktopRect {
                    x: 0.0,
                    y: 0.0,
                    width: 1440.0,
                    height: 900.0,
                },
                usable_bounds: DesktopRect {
                    x: 0.0,
                    y: 24.0,
                    width: 1440.0,
                    height: 826.0,
                },
                scale_factor: 2.0,
                primary: true,
            }],
            ..DesktopSnapshot::default()
        }
    }

    fn let_colony_wander(world: &mut World, now: OffsetDateTime) {
        world.save.home.active_since_utc = None;
        world.save.home.last_disappeared_utc = Some(now);
    }

    #[test]
    fn colony_arrives_on_calendar_thresholds() {
        let created = datetime!(2026-01-01 0:00 UTC);
        let mut world = World::new([9; 32], created, &desktop());
        world.tick(
            created + time::Duration::hours(1) - time::Duration::seconds(1),
            0.05,
            &desktop(),
        );
        assert_eq!(world.save.creatures.len(), 1);
        world.tick(created + time::Duration::hours(1), 0.05, &desktop());
        assert_eq!(world.save.creatures.len(), 2);
        assert_eq!(
            world.save.creatures[1].born_at_utc,
            created + time::Duration::hours(1)
        );
        world.tick(
            created + time::Duration::days(7) - time::Duration::seconds(1),
            0.05,
            &desktop(),
        );
        assert_eq!(world.save.creatures.len(), 2);
        world.tick(created + time::Duration::days(7), 0.05, &desktop());
        assert_eq!(world.save.creatures.len(), 3);
        world.tick(datetime!(2026-01-31 23:59:59 UTC), 0.05, &desktop());
        assert_eq!(world.save.creatures.len(), 3);
        world.tick(datetime!(2026-02-01 0:00 UTC), 0.05, &desktop());
        assert_eq!(world.save.creatures.len(), 4);
    }

    #[test]
    fn calendar_month_arrival_clamps_to_the_destination_month() {
        assert_eq!(
            add_calendar_months_utc(datetime!(2026-01-31 14:05:06 UTC), 1),
            datetime!(2026-02-28 14:05:06 UTC)
        );
        assert_eq!(
            add_calendar_months_utc(datetime!(2028-01-31 14:05:06 UTC), 1),
            datetime!(2028-02-29 14:05:06 UTC)
        );
        assert_eq!(
            add_calendar_months_utc(datetime!(2026-12-31 14:05:06 UTC), 1),
            datetime!(2027-01-31 14:05:06 UTC)
        );
    }

    #[test]
    fn clock_rollback_does_not_remove_arrivals() {
        let created = datetime!(2026-01-01 0:00 UTC);
        let mut world = World::new([2; 32], created, &desktop());
        world.tick(created + time::Duration::days(180), 0.05, &desktop());
        world.tick(created + time::Duration::days(3), 0.05, &desktop());
        assert_eq!(world.save.creatures.len(), 4);
    }

    #[test]
    fn overdue_arrivals_are_present_but_revealed_fifteen_seconds_apart() {
        let created = datetime!(2026-01-01 0:00 UTC);
        let mut world = World::new([12; 32], created, &desktop());
        world.tick(created + time::Duration::days(181), 0.05, &desktop());
        assert_eq!(world.save.creatures.len(), 4);
        assert_eq!(world.save.creatures[1].state.arrival_delay_secs, 0.0);
        assert!(world.save.creatures[2].state.arrival_delay_secs > 14.0);
        assert!(world.save.creatures[3].state.arrival_delay_secs > 29.0);
        world.tick(created + time::Duration::days(181), 15.0, &desktop());
        assert_eq!(world.save.creatures[2].state.arrival_delay_secs, 0.0);
        assert!(world.save.creatures[3].state.arrival_delay_secs > 14.0);
    }

    #[test]
    fn mini_is_related_but_not_identical() {
        let created = datetime!(2026-01-01 0:00 UTC);
        let mut world = World::new([4; 32], created, &desktop());
        world.tick(created + time::Duration::days(30), 0.05, &desktop());
        let parent = &world.save.creatures[0];
        let mini = &world.save.creatures[1];
        assert_eq!(parent.appearance.family, mini.appearance.family);
        assert_eq!(
            parent.appearance.face_signature,
            mini.appearance.face_signature
        );
        assert_eq!(
            parent.appearance.palette_index,
            mini.appearance.palette_index
        );
        assert_eq!(
            parent.appearance.face.eye_shape,
            mini.appearance.face.eye_shape
        );
        assert_eq!(
            parent.appearance.face.eye_size,
            mini.appearance.face.eye_size
        );
        assert_eq!(
            parent.appearance.face.eye_spacing,
            mini.appearance.face.eye_spacing
        );
        assert_eq!(
            parent.appearance.face.pupil_style,
            mini.appearance.face.pupil_style
        );
        assert_eq!(
            parent.appearance.face.highlight_style,
            mini.appearance.face.highlight_style
        );
        assert_ne!(parent.appearance.marking_seed, mini.appearance.marking_seed);
        assert_ne!(parent.personality, mini.personality);
    }

    #[test]
    fn perched_creature_rides_and_falls_from_window() {
        let created = datetime!(2026-01-01 0:00 UTC);
        let mut desktop = desktop();
        desktop.windows.push(DesktopWindow {
            key: 44,
            bounds: DesktopRect {
                x: 200.0,
                y: 300.0,
                width: 600.0,
                height: 400.0,
            },
            z_order: 0,
            visible: true,
            minimized: false,
            application: None,
            application_name: None,
        });
        let mut world = World::new([8; 32], created, &desktop);
        let_colony_wander(&mut world, created);
        let creature = &mut world.save.creatures[0];
        creature.state.surface = SurfaceAttachment {
            kind: SurfaceKind::WindowLedge,
            monitor_id: 1,
            window_key: Some(44),
            relative_x: 0.5,
        };
        world.tick(created, 0.05, &desktop);
        assert_eq!(
            world.save.creatures[0].state.position,
            Point { x: 500.0, y: 300.0 }
        );
        desktop.windows[0].bounds.x = 240.0;
        desktop.windows[0].bounds.y = 280.0;
        world.tick(created, 0.05, &desktop);
        assert_eq!(world.save.creatures[0].state.position.y, 280.0);
        assert_eq!(world.save.creatures[0].state.action, ActionKind::RideWindow);
        world.save.creatures[0].personality.window_tolerance = 0.0;
        desktop.windows[0].bounds.x = 500.0;
        world.tick(created, 0.05, &desktop);
        assert_eq!(
            world.save.creatures[0].state.action,
            ActionKind::ReactToWindow
        );
        desktop.windows.clear();
        world.tick(created, 0.05, &desktop);
        assert_eq!(
            world.save.creatures[0].state.surface.kind,
            SurfaceKind::ScreenFloor
        );
        assert_eq!(
            world.save.creatures[0].state.action,
            ActionKind::ReactToWindow
        );
    }

    #[test]
    fn drag_release_lands_inside_the_desktop_habitat() {
        let created = datetime!(2026-01-01 0:00 UTC);
        let desktop = desktop();
        let mut world = World::new([21; 32], created, &desktop);
        let creature_id = world.save.creatures[0].id;
        let original = world.save.creatures[0].state.position;
        assert!(world.handle_command(
            WorldCommand::BeginInteraction {
                creature_id,
                cursor: original,
            },
            &desktop,
        ));
        assert_eq!(world.save.creatures[0].state.action, ActionKind::Idle);
        assert!(world.handle_command(
            WorldCommand::UpdateInteraction {
                cursor: Point { x: 900.0, y: 300.0 },
                velocity: Point::default(),
            },
            &desktop,
        ));
        assert_eq!(world.save.creatures[0].state.action, ActionKind::Dragged);
        assert!(world.handle_command(
            WorldCommand::EndInteraction {
                cursor: Point { x: 900.0, y: 300.0 },
                velocity: Point::default(),
            },
            &desktop,
        ));
        let creature = &world.save.creatures[0];
        assert_eq!(creature.state.action, ActionKind::Landing);
        assert!(
            desktop.monitors[0]
                .usable_bounds
                .contains(creature.state.position)
        );
        assert_eq!(creature.state.surface.kind, SurfaceKind::ScreenFloor);
    }

    #[test]
    fn click_without_drag_pets_and_does_not_dismiss_the_shelter() {
        let created = datetime!(2026-01-01 0:00 UTC);
        let desktop = desktop();
        let mut world = World::new([81; 32], created, &desktop);
        world.tick(created, 0.05, &desktop);
        let creature_id = world.save.creatures[0].id;
        let position = world.save.creatures[0].state.position;
        assert!(world.save.home.is_active());
        world.handle_command(
            WorldCommand::BeginInteraction {
                creature_id,
                cursor: position,
            },
            &desktop,
        );
        world.handle_command(
            WorldCommand::EndInteraction {
                cursor: Point {
                    x: position.x + DRAG_THRESHOLD,
                    y: position.y,
                },
                velocity: Point::default(),
            },
            &desktop,
        );
        let creature = &world.save.creatures[0];
        assert!(world.save.home.is_active());
        assert_eq!(creature.state.action, ActionKind::PetReaction);
        assert_eq!(creature.memory.times_petted, 1);
        assert_eq!(creature.tendencies.cursor_trust, 3);
        assert_eq!(creature.tendencies.sociability, 2);
    }

    #[test]
    fn drag_out_and_back_uses_maximum_excursion_instead_of_release_position() {
        let created = datetime!(2026-01-01 0:00 UTC);
        let desktop = desktop();
        let mut world = World::new([82; 32], created, &desktop);
        let creature_id = world.save.creatures[0].id;
        let start = world.save.creatures[0].state.position;
        world.handle_command(
            WorldCommand::BeginInteraction {
                creature_id,
                cursor: start,
            },
            &desktop,
        );
        world.handle_command(
            WorldCommand::UpdateInteraction {
                cursor: Point {
                    x: start.x + DRAG_THRESHOLD + 0.1,
                    y: start.y,
                },
                velocity: Point::default(),
            },
            &desktop,
        );
        world.handle_command(
            WorldCommand::UpdateInteraction {
                cursor: start,
                velocity: Point::default(),
            },
            &desktop,
        );
        world.handle_command(
            WorldCommand::EndInteraction {
                cursor: start,
                velocity: Point::default(),
            },
            &desktop,
        );
        assert_eq!(world.save.creatures[0].memory.times_petted, 0);
        assert_eq!(world.save.creatures[0].state.action, ActionKind::Landing);
    }

    #[test]
    fn minute_observations_pause_while_hidden_or_paused() {
        let created = datetime!(2026-01-01 0:00 UTC);
        let desktop = desktop();
        let mut active = World::new([83; 32], created, &desktop);
        active.tick(created, OBSERVATION_INTERVAL_SECS, &desktop);
        assert_eq!(
            active.save.creatures[0]
                .memory
                .favorite_display
                .map(|favorite| favorite.confidence),
            Some(1)
        );

        for (visible, paused) in [(false, false), (true, true)] {
            let mut inactive = World::new([84; 32], created, &desktop);
            inactive.save.settings.visible = visible;
            inactive.save.settings.paused = paused;
            inactive.tick(created, OBSERVATION_INTERVAL_SECS, &desktop);
            assert!(inactive.save.creatures[0].memory.favorite_display.is_none());
        }
    }

    #[test]
    fn contrary_experiences_reverse_tendencies_and_badges_persist_until_viewed() {
        let created = datetime!(2026-01-01 0:00 UTC);
        let desktop = desktop();
        let mut world = World::new([85; 32], created, &desktop);
        let creature_id = world.save.creatures[0].id;
        for _ in 0..12 {
            world
                .events
                .push(WorldEvent::CreaturePetted { creature_id });
        }
        world.project_events(created);
        assert_eq!(world.save.creatures[0].tendencies.cursor_trust, 36);
        assert!(world.save.creatures[0].memory.profile_revision > 0);
        assert!(world.save.creatures[0].memory.viewed_profile_revision == 0);
        assert!(world.drain_events().any(|event| matches!(
            event,
            WorldEvent::ProfileChanged {
                new_descriptor: Some(ProfileDescriptor::Trusting),
                show_milestone: true,
                ..
            }
        )));
        for _ in 0..10 {
            world.events.push(WorldEvent::DragEnded {
                creature_id,
                outcome: DragReleaseKind::Tossed {
                    velocity: Point::default(),
                },
            });
        }
        world.project_events(created + Duration::hours(1));
        assert!(world.save.creatures[0].tendencies.cursor_trust < 0);
        assert!(world.drain_events().any(|event| matches!(
            event,
            WorldEvent::ProfileChanged {
                new_descriptor: Some(ProfileDescriptor::Wary),
                show_milestone: false,
                ..
            }
        )));

        world.save.creatures[0]
            .memory
            .milestone_cooldown_active_seconds = 12 * 60 * 60 - 60;
        world.events.push(WorldEvent::ObservationElapsed {
            creature_id,
            display: desktop.monitors[0].display_key,
            region: 0,
            on_ledge: false,
            riding_window: false,
            nearby_creature: None,
            active_seconds: 60,
        });
        world.project_events(created + Duration::days(2));
        world.drain_events().for_each(drop);
        for _ in 0..18 {
            world
                .events
                .push(WorldEvent::CreaturePetted { creature_id });
        }
        world.project_events(created + Duration::days(2));
        assert!(world.drain_events().any(|event| matches!(
            event,
            WorldEvent::ProfileChanged {
                new_descriptor: Some(ProfileDescriptor::Social),
                show_milestone: true,
                ..
            }
        )));
        assert!(world.mark_profile_viewed(creature_id));
        assert_eq!(
            world.save.creatures[0].memory.viewed_profile_revision,
            world.save.creatures[0].memory.profile_revision
        );
    }

    #[test]
    fn cancelled_drag_restores_the_last_safe_state() {
        let created = datetime!(2026-01-01 0:00 UTC);
        let desktop = desktop();
        let mut world = World::new([22; 32], created, &desktop);
        let creature_id = world.save.creatures[0].id;
        let original_position = world.save.creatures[0].state.position;
        let original_surface = world.save.creatures[0].state.surface.clone();
        let original_action = world.save.creatures[0].state.action;
        assert!(world.handle_command(
            WorldCommand::BeginInteraction {
                creature_id,
                cursor: original_position,
            },
            &desktop,
        ));
        world.handle_command(
            WorldCommand::UpdateInteraction {
                cursor: Point {
                    x: -500.0,
                    y: -500.0,
                },
                velocity: Point::default(),
            },
            &desktop,
        );
        assert!(world.handle_command(WorldCommand::CancelInteraction, &desktop));
        let creature = &world.save.creatures[0];
        assert_eq!(creature.state.position, original_position);
        assert_eq!(creature.state.surface, original_surface);
        assert_eq!(creature.state.action, original_action);
    }

    #[test]
    fn nearby_window_change_interrupts_a_long_running_action() {
        let created = datetime!(2026-01-01 0:00 UTC);
        let mut desktop = desktop();
        let mut world = World::new([31; 32], created, &desktop);
        let_colony_wander(&mut world, created);
        world.tick(created, 0.05, &desktop);
        let position = world.save.creatures[0].state.position;
        world.save.creatures[0].state.action = ActionKind::Sleep;
        world.save.creatures[0].state.action_elapsed = 5.0;
        world.save.creatures[0].state.action_duration = 100.0;
        desktop.windows.push(DesktopWindow {
            key: 91,
            bounds: DesktopRect {
                x: position.x - 180.0,
                y: position.y - 260.0,
                width: 360.0,
                height: 120.0,
            },
            z_order: 0,
            visible: true,
            minimized: false,
            application: None,
            application_name: None,
        });
        world.tick(created, 0.05, &desktop);
        assert_eq!(
            world.save.creatures[0].state.action,
            ActionKind::ReactToWindow
        );
        assert!(world.drain_events().any(|event| matches!(
            event,
            WorldEvent::WindowReaction {
                action: ActionKind::ReactToWindow,
                ..
            }
        )));
    }

    #[test]
    fn ledge_journey_visibly_moves_before_attaching() {
        let created = datetime!(2026-01-01 0:00 UTC);
        let mut desktop = desktop();
        let mut world = World::new([32; 32], created, &desktop);
        let_colony_wander(&mut world, created);
        let start = world.save.creatures[0].state.position;
        desktop.windows.push(DesktopWindow {
            key: 92,
            bounds: DesktopRect {
                x: start.x - 100.0,
                y: start.y - 300.0,
                width: 300.0,
                height: 220.0,
            },
            z_order: 0,
            visible: true,
            minimized: false,
            application: None,
            application_name: None,
        });
        let (target, surface) = find_nearby_ledge(
            &world.save.creatures[0],
            &desktop,
            &world.save.settings.habitat,
        )
        .expect("test window should expose a reachable ledge");
        let creature_id = world.save.creatures[0].id;
        world.save.creatures[0].state.action = ActionKind::Landing;
        world.window_journeys.insert(
            creature_id,
            WindowJourney::Hop(HopJourney {
                start,
                target,
                surface,
                elapsed: 0.0,
                duration: 1.0,
            }),
        );
        world.tick(created, 0.4, &desktop);
        let midway = world.save.creatures[0].state.position;
        assert_ne!(midway, start);
        assert_ne!(midway, target);
        world.tick(created, 0.7, &desktop);
        let creature = &world.save.creatures[0];
        assert_eq!(creature.state.position, target);
        assert_eq!(creature.state.surface.kind, SurfaceKind::WindowLedge);
        assert_eq!(creature.state.action, ActionKind::Perch);
    }

    #[test]
    fn upward_window_routes_stage_traverse_climb_mantle_and_perch() {
        let created = datetime!(2026-01-01 0:00 UTC);
        let mut desktop = desktop();
        let world = World::new([33; 32], created, &desktop);
        let creature = &world.save.creatures[0];
        let start = creature.state.position;
        let bounds = DesktopRect {
            x: start.x - 110.0,
            y: start.y - 360.0,
            width: 320.0,
            height: 260.0,
        };
        desktop.windows.push(DesktopWindow {
            key: 93,
            bounds,
            z_order: 0,
            visible: true,
            minimized: false,
            application: None,
            application_name: None,
        });
        let surface = SurfaceAttachment {
            kind: SurfaceKind::WindowLedge,
            monitor_id: 1,
            window_key: Some(93),
            relative_x: 0.5,
        };
        let target = Point {
            x: start.x,
            y: bounds.y,
        };
        let mut journey = build_window_journey(creature, target, surface, &desktop);
        assert_eq!(journey.initial_action(), ActionKind::Traverse);
        let WindowJourney::Climb(climb) = &journey else {
            panic!("upward transfer should climb");
        };
        let climb_speed = climb.approach.distance(climb.climb_end) / climb.climb_duration;
        assert!((44.0..=62.0).contains(&climb_speed));
        assert_eq!(climb.climb_end.y, bounds.y + MANTLE_LIFT_POINTS);
        assert_eq!(climb.target.y, bounds.y);
        assert_eq!(climb.mantle_duration, 0.7);

        let mut stages = Vec::new();
        let mut last = JourneyStep {
            position: start,
            action: ActionKind::Traverse,
            complete: false,
        };
        for _ in 0..400 {
            last = journey.advance(0.05);
            if stages.last() != Some(&last.action) {
                stages.push(last.action);
            }
            if last.complete {
                break;
            }
        }
        assert!(last.complete);
        assert_eq!(stages, vec![ActionKind::Traverse, ActionKind::ClimbWindow]);
        assert_eq!(last.position.y, bounds.y);
        assert!(last.position.x == bounds.x + 18.0 || last.position.x == bounds.right() - 18.0);
    }

    #[test]
    fn downward_window_routes_keep_the_existing_hop() {
        let created = datetime!(2026-01-01 0:00 UTC);
        let mut desktop = desktop();
        desktop.windows.push(DesktopWindow {
            key: 94,
            bounds: DesktopRect {
                x: 300.0,
                y: 620.0,
                width: 420.0,
                height: 200.0,
            },
            z_order: 0,
            visible: true,
            minimized: false,
            application: None,
            application_name: None,
        });
        let mut creature = World::new([34; 32], created, &desktop)
            .save
            .creatures
            .remove(0);
        creature.state.position = Point { x: 500.0, y: 280.0 };
        let target = Point { x: 510.0, y: 620.0 };
        let surface = SurfaceAttachment {
            kind: SurfaceKind::WindowLedge,
            monitor_id: 1,
            window_key: Some(94),
            relative_x: 0.5,
        };
        assert!(matches!(
            build_window_journey(&creature, target, surface, &desktop),
            WindowJourney::Hop(_)
        ));
    }

    #[test]
    fn climb_cancels_when_its_path_leaves_the_habitat() {
        let created = datetime!(2026-01-01 0:00 UTC);
        let mut desktop = desktop();
        let window = DesktopWindow {
            key: 96,
            bounds: DesktopRect {
                x: 320.0,
                y: 360.0,
                width: 420.0,
                height: 360.0,
            },
            z_order: 0,
            visible: true,
            minimized: false,
            application: None,
            application_name: None,
        };
        desktop.windows.push(window.clone());
        let mut world = World::new([42; 32], created, &desktop);
        let_colony_wander(&mut world, created);
        world.save.settings.habitat.zones.push(HabitatZone {
            id: 1,
            display: DisplayKey([1; 16]),
            normalized_bounds: DesktopRect {
                x: 0.14,
                y: 0.0,
                width: 0.06,
                height: 1.0,
            },
            kind: HabitatZoneKind::Excluded,
            enabled: true,
        });
        let creature = &mut world.save.creatures[0];
        creature.state.position = Point { x: 100.0, y: 846.0 };
        creature.state.surface = SurfaceAttachment {
            kind: SurfaceKind::ScreenFloor,
            monitor_id: 1,
            window_key: None,
            relative_x: 0.07,
        };
        let surface = SurfaceAttachment {
            kind: SurfaceKind::WindowLedge,
            monitor_id: 1,
            window_key: Some(96),
            relative_x: 0.5,
        };
        let journey = build_window_journey(
            creature,
            Point {
                x: 500.0,
                y: window.bounds.y,
            },
            surface,
            &desktop,
        );
        let creature_id = creature.id;
        creature.state.action = journey.initial_action();
        creature.state.action_duration = f32::MAX;
        world.window_journeys.insert(creature_id, journey);
        for _ in 0..120 {
            world.tick(created, 0.05, &desktop);
            if !world.window_journeys.contains_key(&creature_id) {
                break;
            }
        }
        assert!(!world.window_journeys.contains_key(&creature_id));
        assert_eq!(
            world.save.creatures[0].state.action,
            ActionKind::ReactToWindow
        );
        assert!(habitat_contains(
            &world.save.settings.habitat,
            &desktop.monitors[0],
            world.save.creatures[0].state.position,
        ));
    }

    #[test]
    fn ambient_cadence_is_deterministic_bounded_and_suspended() {
        let created = datetime!(2026-01-01 0:00 UTC);
        let desktop = desktop();
        let mut first = World::new([35; 32], created, &desktop);
        let second = World::new([35; 32], created, &desktop);
        let id = first.save.creatures[0].id;
        let first_timers = first.ambient_timers[&id];
        let second_timers = second.ambient_timers[&id];
        assert_eq!(
            first_timers.inspect_remaining,
            second_timers.inspect_remaining
        );
        assert_eq!(
            first_timers.dangle_remaining,
            second_timers.dangle_remaining
        );
        assert_eq!(first.discovery_remaining, second.discovery_remaining);
        assert!((120.0..240.0).contains(&first_timers.inspect_remaining));
        assert!((240.0..480.0).contains(&first_timers.dangle_remaining));
        assert!((600.0..1_200.0).contains(&first.discovery_remaining));

        let_colony_wander(&mut first, created);
        first.save.settings.visible = false;
        first.ambient_timers.get_mut(&id).unwrap().inspect_remaining = 0.0;
        first.ambient_timers.get_mut(&id).unwrap().dangle_remaining = 0.0;
        first.discovery_remaining = 0.0;
        first.save.creatures[0].state.action = ActionKind::Idle;
        first.save.creatures[0].state.action_elapsed = 4.0;
        first.save.creatures[0].state.action_duration = 3.0;
        first.tick(created, 1.0, &desktop);
        assert_eq!(first.ambient_timers[&id].inspect_remaining, 0.0);
        assert_eq!(first.ambient_timers[&id].dangle_remaining, 0.0);
        assert_eq!(first.discovery_remaining, 0.0);
        assert!(!matches!(
            first.save.creatures[0].state.action,
            ActionKind::InspectScreen | ActionKind::Dangle | ActionKind::PresentDiscovery
        ));

        first.save.settings.paused = true;
        first.save.settings.visible = true;
        first.tick(created, 1.0, &desktop);
        assert_eq!(first.discovery_remaining, 0.0);
        assert!(!matches!(
            first.save.creatures[0].state.action,
            ActionKind::InspectScreen | ActionKind::Dangle | ActionKind::PresentDiscovery
        ));
    }

    #[test]
    fn only_one_creature_begins_a_colony_discovery() {
        let created = datetime!(2026-01-01 0:00 UTC);
        let desktop = desktop();
        let now = created + time::Duration::days(180);
        let mut world = World::new([43; 32], created, &desktop);
        world.tick(now, 0.05, &desktop);
        for creature in &mut world.save.creatures {
            creature.state.arrival_delay_secs = 0.0;
            creature.state.action = ActionKind::Idle;
            creature.state.action_elapsed = 4.0;
            creature.state.action_duration = 3.0;
        }
        world.discovery_remaining = 0.0;
        world.tick(now, 0.05, &desktop);
        assert_eq!(
            world
                .save
                .creatures
                .iter()
                .filter(|creature| creature.state.action == ActionKind::PresentDiscovery)
                .count(),
            1
        );
        assert!((600.0..1_200.0).contains(&world.discovery_remaining));
    }

    #[test]
    fn inspection_landmarks_are_geometry_only_screen_thirds() {
        let created = datetime!(2026-01-01 0:00 UTC);
        let desktop = desktop();
        let mut world = World::new([36; 32], created, &desktop);
        let creature = &mut world.save.creatures[0];
        creature.state.surface = SurfaceAttachment {
            kind: SurfaceKind::ScreenFloor,
            monitor_id: 1,
            window_key: None,
            relative_x: 1.0 / 3.0,
        };
        creature.state.position = Point {
            x: 490.0,
            y: desktop.monitors[0].usable_bounds.bottom() - 4.0,
        };
        assert!(crossed_inspection_anchor(
            creature,
            470.0,
            &desktop,
            &world.save.settings.habitat,
        ));
        creature.state.position.x = 760.0;
        assert!(!crossed_inspection_anchor(
            creature,
            730.0,
            &desktop,
            &world.save.settings.habitat,
        ));
    }

    #[test]
    fn toss_release_threshold_and_reduced_motion_preserve_precise_placement() {
        let created = datetime!(2026-01-01 0:00 UTC);
        let desktop = desktop();
        let mut world = World::new([37; 32], created, &desktop);
        let creature_id = world.save.creatures[0].id;
        let start = world.save.creatures[0].state.position;
        assert!(world.handle_command(
            WorldCommand::BeginInteraction {
                creature_id,
                cursor: start,
            },
            &desktop,
        ));
        assert!(world.handle_command(
            WorldCommand::UpdateInteraction {
                cursor: Point {
                    x: start.x + 12.0,
                    y: start.y,
                },
                velocity: Point { x: 300.0, y: 0.0 },
            },
            &desktop,
        ));
        assert!(world.handle_command(
            WorldCommand::EndInteraction {
                cursor: start,
                velocity: Point { x: 300.0, y: 0.0 },
            },
            &desktop,
        ));
        assert_eq!(world.save.creatures[0].state.action, ActionKind::Tossed);
        assert!(world.tosses.contains_key(&creature_id));
        assert_eq!(world.save.creatures[0].state.velocity.x, 195.0);
        assert!(world.drain_events().any(|event| matches!(
            event,
            WorldEvent::DragEnded {
                outcome: DragReleaseKind::Tossed { .. },
                ..
            }
        )));

        world.save.settings.paused = true;
        world.tick(created, 0.05, &desktop);
        assert!(world.tosses.is_empty());
        assert_eq!(world.save.creatures[0].state.action, ActionKind::Landing);
        assert!(world.drain_events().any(|event| matches!(
            event,
            WorldEvent::TossLanded {
                creature_id: landed_id,
                surface: SurfaceKind::ScreenFloor,
                ..
            } if landed_id == creature_id
        )));

        let mut below_threshold = World::new([45; 32], created, &desktop);
        let creature_id = below_threshold.save.creatures[0].id;
        let start = below_threshold.save.creatures[0].state.position;
        below_threshold.handle_command(
            WorldCommand::BeginInteraction {
                creature_id,
                cursor: start,
            },
            &desktop,
        );
        below_threshold.handle_command(
            WorldCommand::EndInteraction {
                cursor: start,
                velocity: Point { x: 219.0, y: 0.0 },
            },
            &desktop,
        );
        assert_eq!(
            below_threshold.save.creatures[0].state.action,
            ActionKind::PetReaction
        );
        assert_eq!(below_threshold.save.creatures[0].memory.times_petted, 1);
        assert!(below_threshold.tosses.is_empty());

        let mut reduced = World::new([38; 32], created, &desktop);
        reduced.save.settings.reduce_motion = true;
        let creature_id = reduced.save.creatures[0].id;
        let start = reduced.save.creatures[0].state.position;
        reduced.handle_command(
            WorldCommand::BeginInteraction {
                creature_id,
                cursor: start,
            },
            &desktop,
        );
        reduced.handle_command(
            WorldCommand::UpdateInteraction {
                cursor: Point {
                    x: start.x + 12.0,
                    y: start.y,
                },
                velocity: Point {
                    x: 900.0,
                    y: -100.0,
                },
            },
            &desktop,
        );
        reduced.handle_command(
            WorldCommand::EndInteraction {
                cursor: start,
                velocity: Point {
                    x: 900.0,
                    y: -100.0,
                },
            },
            &desktop,
        );
        assert_eq!(reduced.save.creatures[0].state.action, ActionKind::Idle);
        assert!(reduced.tosses.is_empty());
    }

    #[test]
    fn drag_velocity_history_is_fixed_capacity_and_caps_launch_speed() {
        let mut drag = InteractionSession {
            press_cursor: Point::default(),
            max_excursion: 0.0,
            dragging: true,
            creature_id: 1,
            grab_offset: Point::default(),
            original_position: Point::default(),
            original_surface: SurfaceAttachment {
                kind: SurfaceKind::ScreenFloor,
                monitor_id: 1,
                window_key: None,
                relative_x: 0.5,
            },
            original_action: ActionKind::Idle,
            velocity_samples: [Point::default(); 3],
            velocity_sample_count: 0,
            next_velocity_sample: 0,
        };
        for x in [100.0, 200.0, 300.0, 400.0] {
            drag.record_velocity(Point { x, y: 0.0 });
        }
        assert_eq!(drag.velocity_sample_count, 3);
        assert_eq!(drag.release_velocity(), Point { x: 300.0, y: 0.0 });

        let created = datetime!(2026-01-01 0:00 UTC);
        let desktop = desktop();
        let mut world = World::new([44; 32], created, &desktop);
        let creature_id = world.save.creatures[0].id;
        let start = world.save.creatures[0].state.position;
        world.handle_command(
            WorldCommand::BeginInteraction {
                creature_id,
                cursor: start,
            },
            &desktop,
        );
        world.handle_command(
            WorldCommand::UpdateInteraction {
                cursor: Point {
                    x: start.x + 12.0,
                    y: start.y,
                },
                velocity: Point { x: 2_000.0, y: 0.0 },
            },
            &desktop,
        );
        world.handle_command(
            WorldCommand::EndInteraction {
                cursor: start,
                velocity: Point { x: 2_000.0, y: 0.0 },
            },
            &desktop,
        );
        assert_eq!(world.save.creatures[0].state.velocity.x, TOSS_MAX_SPEED);
    }

    #[test]
    fn swept_toss_lands_on_ledges_bounces_once_and_settles() {
        let created = datetime!(2026-01-01 0:00 UTC);
        let mut desktop = desktop();
        desktop.windows.push(DesktopWindow {
            key: 95,
            bounds: DesktopRect {
                x: 300.0,
                y: 400.0,
                width: 500.0,
                height: 300.0,
            },
            z_order: 0,
            visible: true,
            minimized: false,
            application: None,
            application_name: None,
        });
        let swept = find_swept_support(
            Point { x: 500.0, y: 100.0 },
            Point { x: 500.0, y: 900.0 },
            &desktop,
            &HabitatPolicy::default(),
            true,
        )
        .expect("sweep should find the window before the floor");
        assert_eq!(swept.1.kind, SurfaceKind::WindowLedge);
        assert_eq!(swept.1.window_key, Some(95));
        let floor_only = find_swept_support(
            Point { x: 500.0, y: 100.0 },
            Point { x: 500.0, y: 900.0 },
            &desktop,
            &HabitatPolicy::default(),
            false,
        )
        .expect("disabled ledges should still leave a floor");
        assert_eq!(floor_only.1.kind, SurfaceKind::ScreenFloor);

        desktop.windows.clear();
        let mut creature = World::new([39; 32], created, &desktop)
            .save
            .creatures
            .remove(0);
        creature.state.position = Point { x: 500.0, y: 200.0 };
        creature.state.velocity = Point {
            x: 260.0,
            y: -160.0,
        };
        creature.state.action = ActionKind::Tossed;
        let mut toss = TossState {
            elapsed: 0.0,
            bounces: 0,
            last_safe_position: Point { x: 500.0, y: 846.0 },
            last_safe_surface: SurfaceAttachment {
                kind: SurfaceKind::ScreenFloor,
                monitor_id: 1,
                window_key: None,
                relative_x: 0.5,
            },
        };
        let mut landed = None;
        for _ in 0..60 {
            landed = advance_toss(
                &mut creature,
                &mut toss,
                0.05,
                &desktop,
                &HabitatPolicy::default(),
                false,
                true,
            );
            if landed.is_some() {
                break;
            }
        }
        let (surface, bounced) = landed.expect("toss should settle before its timeout");
        assert!(bounced);
        assert_eq!(toss.bounces, 1);
        assert_eq!(surface.kind, SurfaceKind::ScreenFloor);
        assert_eq!(creature.state.action, ActionKind::Landing);
        assert_eq!(creature.state.velocity, Point::default());

        creature.state.position = Point { x: 500.0, y: 845.0 };
        creature.state.velocity = Point::default();
        creature.state.action = ActionKind::Tossed;
        let mut low_energy = TossState {
            elapsed: 0.0,
            bounces: 0,
            last_safe_position: Point { x: 500.0, y: 846.0 },
            last_safe_surface: toss.last_safe_surface.clone(),
        };
        let first_impact = advance_toss(
            &mut creature,
            &mut low_energy,
            0.05,
            &desktop,
            &HabitatPolicy::default(),
            false,
            true,
        )
        .expect("low-energy impact should settle immediately");
        assert!(!first_impact.1);
        assert_eq!(low_energy.bounces, 0);

        creature.state.position = Point {
            x: 2_000.0,
            y: 200.0,
        };
        creature.state.velocity = Point { x: 400.0, y: 0.0 };
        creature.state.action = ActionKind::Tossed;
        let mut timed_out = TossState {
            elapsed: TOSS_MAX_DURATION - 0.01,
            bounces: 0,
            last_safe_position: Point { x: 500.0, y: 846.0 },
            last_safe_surface: toss.last_safe_surface,
        };
        assert!(
            advance_toss(
                &mut creature,
                &mut timed_out,
                0.05,
                &desktop,
                &HabitatPolicy::default(),
                false,
                true,
            )
            .is_some()
        );
        assert!(
            desktop.monitors[0]
                .usable_bounds
                .contains(creature.state.position)
        );
    }

    #[test]
    fn grabbing_a_toss_in_flight_cancels_back_to_its_last_safe_state() {
        let created = datetime!(2026-01-01 0:00 UTC);
        let desktop = desktop();
        let mut world = World::new([40; 32], created, &desktop);
        let creature_id = world.save.creatures[0].id;
        let original = world.save.creatures[0].state.position;
        world.handle_command(
            WorldCommand::BeginInteraction {
                creature_id,
                cursor: original,
            },
            &desktop,
        );
        world.handle_command(
            WorldCommand::EndInteraction {
                cursor: Point {
                    x: original.x + 80.0,
                    y: original.y - 80.0,
                },
                velocity: Point {
                    x: 700.0,
                    y: -500.0,
                },
            },
            &desktop,
        );
        let airborne = world.save.creatures[0].state.position;
        assert!(world.handle_command(
            WorldCommand::BeginInteraction {
                creature_id,
                cursor: airborne,
            },
            &desktop,
        ));
        assert!(world.tosses.is_empty());
        assert!(world.handle_command(WorldCommand::CancelInteraction, &desktop));
        assert_eq!(world.save.creatures[0].state.position, original);
        assert_eq!(world.save.creatures[0].state.action, ActionKind::Idle);
    }

    #[test]
    fn most_creatures_discover_a_reachable_window_within_one_minute() {
        let created = datetime!(2026-01-01 0:00 UTC);
        let mut discovered = 0;
        for seed_byte in 0_u8..20 {
            let mut desktop = desktop();
            let mut world = World::new([seed_byte; 32], created, &desktop);
            let_colony_wander(&mut world, created);
            let start = world.save.creatures[0].state.position;
            desktop.windows.push(DesktopWindow {
                key: 100 + u64::from(seed_byte),
                bounds: DesktopRect {
                    x: start.x - 180.0,
                    y: start.y - 320.0,
                    width: 420.0,
                    height: 250.0,
                },
                z_order: 0,
                visible: true,
                minimized: false,
                application: None,
                application_name: None,
            });
            for _ in 0..1_200 {
                world.tick(created, 0.05, &desktop);
                if world.save.creatures[0].state.surface.kind == SurfaceKind::WindowLedge {
                    discovered += 1;
                    break;
                }
            }
        }
        assert!(
            discovered >= 16,
            "only {discovered}/20 creatures found the ledge"
        );
    }

    #[test]
    fn perched_creature_can_choose_a_different_window_height() {
        let created = datetime!(2026-01-01 0:00 UTC);
        let mut desktop = desktop();
        desktop.windows.extend([
            DesktopWindow {
                key: 201,
                bounds: DesktopRect {
                    x: 360.0,
                    y: 610.0,
                    width: 460.0,
                    height: 210.0,
                },
                z_order: 1,
                visible: true,
                minimized: false,
                application: None,
                application_name: None,
            },
            DesktopWindow {
                key: 202,
                bounds: DesktopRect {
                    x: 470.0,
                    y: 330.0,
                    width: 520.0,
                    height: 300.0,
                },
                z_order: 0,
                visible: true,
                minimized: false,
                application: None,
                application_name: None,
            },
        ]);
        let mut world = World::new([41; 32], created, &desktop);
        let_colony_wander(&mut world, created);
        let creature = &mut world.save.creatures[0];
        creature.state.position = Point { x: 590.0, y: 610.0 };
        creature.state.surface = SurfaceAttachment {
            kind: SurfaceKind::WindowLedge,
            monitor_id: 1,
            window_key: Some(201),
            relative_x: 0.5,
        };

        let (target, surface) = find_nearby_ledge(creature, &desktop, &world.save.settings.habitat)
            .expect("the upper window should be a reachable transfer");
        assert_eq!(surface.window_key, Some(202));
        assert_eq!(target.y, 330.0);
    }

    #[test]
    fn creatures_explore_multiple_window_levels() {
        let created = datetime!(2026-01-01 0:00 UTC);
        let mut explorers = 0;
        for seed_byte in 0_u8..20 {
            let mut desktop = desktop();
            let mut world = World::new([seed_byte; 32], created, &desktop);
            let_colony_wander(&mut world, created);
            let start = world.save.creatures[0].state.position;
            desktop.windows.extend([
                DesktopWindow {
                    key: 300 + u64::from(seed_byte) * 3,
                    bounds: DesktopRect {
                        x: start.x - 180.0,
                        y: start.y - 190.0,
                        width: 410.0,
                        height: 150.0,
                    },
                    z_order: 2,
                    visible: true,
                    minimized: false,
                    application: None,
                    application_name: None,
                },
                DesktopWindow {
                    key: 301 + u64::from(seed_byte) * 3,
                    bounds: DesktopRect {
                        x: start.x - 90.0,
                        y: start.y - 390.0,
                        width: 440.0,
                        height: 180.0,
                    },
                    z_order: 1,
                    visible: true,
                    minimized: false,
                    application: None,
                    application_name: None,
                },
                DesktopWindow {
                    key: 302 + u64::from(seed_byte) * 3,
                    bounds: DesktopRect {
                        x: start.x - 210.0,
                        y: start.y - 590.0,
                        width: 520.0,
                        height: 190.0,
                    },
                    z_order: 0,
                    visible: true,
                    minimized: false,
                    application: None,
                    application_name: None,
                },
            ]);
            let mut visited = std::collections::BTreeSet::new();
            for _ in 0..2_400 {
                world.tick(created, 0.05, &desktop);
                if let Some(key) = world.save.creatures[0].state.surface.window_key {
                    visited.insert(key);
                }
                if visited.len() >= 2 {
                    explorers += 1;
                    break;
                }
            }
        }
        assert!(
            explorers >= 16,
            "only {explorers}/20 creatures explored more than one window level"
        );
    }

    #[test]
    fn home_appears_for_fifteen_minutes_then_observes_its_cooldown() {
        let created = datetime!(2026-01-01 0:00 UTC);
        let desktop = desktop();
        let mut world = World::new([52; 32], created, &desktop);
        world.tick(created, 0.05, &desktop);
        assert!(world.save.home.is_active());
        assert_eq!(world.save.creatures[0].state.action, ActionKind::Homebound);
        assert_eq!(world.save.home.display, Some(DisplayKey([1; 16])));

        world.tick(
            created + time::Duration::minutes(14) + time::Duration::seconds(59),
            0.05,
            &desktop,
        );
        assert!(world.save.home.is_active());
        world.tick(created + time::Duration::minutes(15), 0.05, &desktop);
        assert!(!world.save.home.is_active());
        assert_eq!(
            world.save.home.last_disappeared_utc,
            Some(created + time::Duration::minutes(15))
        );

        world.tick(
            created + time::Duration::minutes(29) + time::Duration::seconds(59),
            0.05,
            &desktop,
        );
        assert!(!world.save.home.is_active());
        world.tick(created + time::Duration::minutes(30), 0.05, &desktop);
        assert!(world.save.home.is_active());
    }

    #[test]
    fn homebound_colony_is_spaced_by_how_wide_a_creature_draws() {
        let created = datetime!(2026-01-01 0:00 UTC);
        let desktop = desktop();
        let monitor = &desktop.monitors[0];

        for display_scale in [1_u8, 3, 5] {
            let mut world = World::new([53; 32], created, &desktop);
            world.save.settings.display_scale = display_scale;
            // Give the colony four generations without waiting out the real arrival schedule.
            while world.save.creatures.len() < 4 {
                let generation = world.save.creatures.len() as u8;
                let mut grown = world.save.creatures[0].clone();
                grown.generation = generation;
                grown.colony_order = generation;
                grown.id = u64::from(generation) + 100;
                world.save.creatures.push(grown);
            }
            world.tick(created, 0.05, &desktop);
            assert!(world.save.home.is_active());

            let creature_width =
                CREATURE_ART_WIDTH * f32::from(display_scale) / monitor.scale_factor.max(1.0);
            let mut xs: Vec<_> = world
                .save
                .creatures
                .iter()
                .map(|creature| creature.state.position.x)
                .collect();
            xs.sort_by(f32::total_cmp);
            for pair in xs.windows(2) {
                let gap = pair[1] - pair[0];
                assert!(
                    gap >= creature_width * 0.5,
                    "scale {display_scale}: creatures {gap} apart but draw {creature_width} wide",
                );
            }
        }
    }

    #[test]
    fn dragging_a_homebound_creature_dismisses_the_shelter_and_starts_cooldown() {
        let created = datetime!(2026-01-01 0:00 UTC);
        let desktop = desktop();
        let mut world = World::new([53; 32], created, &desktop);
        world.tick(created, 0.05, &desktop);
        let creature_id = world.save.creatures[0].id;
        let position = world.save.creatures[0].state.position;
        assert!(world.handle_command(
            WorldCommand::BeginInteraction {
                creature_id,
                cursor: position,
            },
            &desktop,
        ));
        assert!(world.save.home.is_active());
        assert!(world.handle_command(
            WorldCommand::UpdateInteraction {
                cursor: Point {
                    x: position.x + 12.0,
                    y: position.y,
                },
                velocity: Point::default(),
            },
            &desktop,
        ));
        assert!(!world.save.home.is_active());
        assert_eq!(world.save.home.last_disappeared_utc, Some(created));
        assert!(
            world
                .drain_events()
                .any(|event| matches!(event, WorldEvent::HomeDisappeared { interrupted: true }))
        );

        assert!(world.handle_command(
            WorldCommand::EndInteraction {
                cursor: Point { x: 720.0, y: 500.0 },
                velocity: Point::default(),
            },
            &desktop,
        ));
        world.tick(created + time::Duration::minutes(14), 0.05, &desktop);
        assert!(!world.save.home.is_active());
        world.tick(created + time::Duration::minutes(15), 0.05, &desktop);
        assert!(world.save.home.is_active());
    }

    #[test]
    fn passive_activity_actions_have_distinct_state_outcomes() {
        let desktop = desktop();
        let creature = World::new([61; 32], datetime!(2026-01-01 0:00 UTC), &desktop)
            .save
            .creatures
            .remove(0);
        let context = BehaviorContext {
            nearest_creature_distance: None,
            nearest_creature_position: None,
            nearest_creature_id: None,
            on_window_ledge: false,
            reachable_window_ledge: false,
            window_changed_nearby: false,
            hour_utc: 12,
        };

        let mut eating = creature.clone();
        eating.state.action = ActionKind::Eat;
        eating.state.drives.energy = 0.25;
        let energy_before = eating.state.drives.energy;
        update_drives(&mut eating, 1.0);
        execute_action(&mut eating, &desktop, context, 1.0, None, None);
        assert!(eating.state.drives.energy > energy_before);

        let mut drinking = creature.clone();
        drinking.state.action = ActionKind::Drink;
        drinking.state.drives.comfort = 0.2;
        drinking.state.drives.arousal = 0.8;
        execute_action(&mut drinking, &desktop, context, 1.0, None, None);
        assert!(drinking.state.drives.comfort > 0.2);
        assert!(drinking.state.drives.arousal < 0.8);

        let mut sprinting = creature;
        sprinting.state.action = ActionKind::Sprint;
        sprinting.state.facing_right = true;
        let start_x = sprinting.state.position.x;
        let walking_speed = 24.0 + sprinting.personality.activity * 34.0;
        execute_action(&mut sprinting, &desktop, context, 0.5, None, None);
        assert!(sprinting.state.position.x - start_x > walking_speed * 0.5 * 2.0);
    }
}
