use crate::behavior::{BehaviorContext, choose_action};
use crate::rng::SeedStream;
use crate::*;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha12Rng;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
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
const RITUAL_APPROACH_SECS: f32 = 8.0;
const RITUAL_MIN_CREATURES: usize = 2;

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
    colony_plan: Option<ColonyPlan>,
    topology: DesktopTopology,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct BondPlan {
    target: CreatureId,
    final_action: ActionKind,
    experience: RelationshipExperience,
    approaching: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RitualPhase {
    Approach,
    Ceremony,
}

#[derive(Clone, Copy, Debug)]
struct RitualParticipant {
    creature_id: CreatureId,
    approach_target: Point,
    ceremony_target: Point,
    ceremony_action: ActionKind,
}

#[derive(Clone, Debug)]
struct ColonyPlan {
    kind: RitualKind,
    monitor_id: MonitorId,
    usable_bounds: DesktopRect,
    participants: Vec<RitualParticipant>,
    phase: RitualPhase,
    remaining_secs: f32,
}

impl ColonyPlan {
    fn geometry_is_valid(&self, desktop: &DesktopSnapshot, policy: &HabitatPolicy) -> bool {
        let Some(monitor) = desktop
            .monitors
            .iter()
            .find(|monitor| monitor.id == self.monitor_id)
        else {
            return false;
        };
        monitor.usable_bounds == self.usable_bounds
            && self.participants.iter().all(|participant| {
                monitor.bounds.contains(participant.approach_target)
                    && monitor.bounds.contains(participant.ceremony_target)
                    && habitat_contains(policy, monitor, participant.approach_target)
                    && habitat_contains(policy, monitor, participant.ceremony_target)
            })
    }
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
struct SqueezeJourney {
    from_window: WindowKey,
    from_bounds: DesktopRect,
    target_window: WindowKey,
    target_bounds: DesktopRect,
    start: Point,
    target: Point,
    surface: SurfaceAttachment,
    elapsed: f32,
    duration: f32,
}

#[derive(Clone)]
struct WindowRoutePlan {
    geometry_hash: u64,
    remaining: VecDeque<TopologyRouteHop>,
}

#[derive(Clone)]
enum WindowJourney {
    Hop(HopJourney),
    Climb(ClimbJourney),
    Squeeze(SqueezeJourney),
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
            Self::Squeeze(_) => ActionKind::SqueezeWindow,
        }
    }

    fn surface(&self) -> &SurfaceAttachment {
        match self {
            Self::Hop(journey) => &journey.surface,
            Self::Climb(journey) => &journey.surface,
            Self::Squeeze(journey) => &journey.surface,
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
            Self::Squeeze(journey) => {
                let from_valid = desktop.windows.iter().any(|window| {
                    window.key == journey.from_window
                        && window.visible
                        && !window.minimized
                        && window.bounds == journey.from_bounds
                });
                let target_valid = desktop.windows.iter().any(|window| {
                    window.key == journey.target_window
                        && window.visible
                        && !window.minimized
                        && window.bounds == journey.target_bounds
                });
                from_valid && target_valid
            }
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
            Self::Squeeze(journey) => {
                journey.elapsed += dt;
                let progress = (journey.elapsed / journey.duration).clamp(0.0, 1.0);
                JourneyStep {
                    position: lerp_point(journey.start, journey.target, smoothstep(progress)),
                    action: ActionKind::SqueezeWindow,
                    complete: progress >= 1.0,
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

pub(crate) fn scheduled_ritual_at(
    colony_seed: [u8; 32],
    ordinal: u32,
    from: OffsetDateTime,
) -> OffsetDateTime {
    let streams = SeedStream::new(colony_seed);
    let mut rng = streams.rng("ritual-schedule", u64::from(ordinal));
    from + Duration::minutes(rng.random_range(12 * 60..=48 * 60))
}

fn interrupted_ritual_at(
    colony_seed: [u8; 32],
    ordinal: u32,
    from: OffsetDateTime,
) -> OffsetDateTime {
    let streams = SeedStream::new(colony_seed);
    let mut rng = streams.rng("ritual-interruption", u64::from(ordinal));
    from + Duration::minutes(rng.random_range(2 * 60..=6 * 60))
}

pub(crate) fn scheduled_colony_object_at(
    colony_seed: [u8; 32],
    ordinal: u32,
    from: OffsetDateTime,
) -> OffsetDateTime {
    let streams = SeedStream::new(colony_seed);
    let mut rng = streams.rng("colony-object-schedule", u64::from(ordinal));
    from + Duration::days(rng.random_range(3..=7))
}

pub(crate) fn scheduled_shelter_decoration_at(
    colony_seed: [u8; 32],
    ordinal: u32,
    from: OffsetDateTime,
) -> OffsetDateTime {
    let streams = SeedStream::new(colony_seed);
    let mut rng = streams.rng("shelter-decoration-schedule", u64::from(ordinal));
    from + Duration::days(rng.random_range(4..=9))
}

fn local_time_or_utc(now: OffsetDateTime) -> OffsetDateTime {
    let offset = UtcOffset::local_offset_at(now).unwrap_or(UtcOffset::UTC);
    now.to_offset(offset)
}

fn ritual_ceremony_duration(kind: RitualKind) -> f32 {
    match kind {
        RitualKind::GroupNap => 30.0,
        RitualKind::LateNightSleepPile => 45.0,
        RitualKind::QuietDayHuddle => 18.0,
        RitualKind::Picnic | RitualKind::ShelterGathering => 14.0,
        RitualKind::FloorRace
        | RitualKind::Catch
        | RitualKind::GroupPresentation
        | RitualKind::HatchDay => 10.0,
    }
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
        let mut home = ColonyHome::from_seed(colony_seed, home_display, Some(now), None);
        home.decorations.next_at_utc = scheduled_shelter_decoration_at(colony_seed, 0, now);
        let save = SaveFile {
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
        };
        Self::from_save(save)
    }

    pub fn from_save(mut save: SaveFile) -> Self {
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
            colony_plan: None,
            topology: DesktopTopology::default(),
        }
    }

    pub fn tick(&mut self, now: OffsetDateTime, dt: f32, desktop: &DesktopSnapshot) {
        if now > self.save.maximum_seen_utc {
            self.save.maximum_seen_utc = now;
        }
        let timeline_now = self.save.maximum_seen_utc;
        self.process_arrivals(timeline_now, desktop);
        self.process_colony_objects(timeline_now, desktop);
        self.process_shelter_decorations(timeline_now);
        self.reconcile_colony_objects(desktop);
        let topology_changed = self
            .topology
            .rebuild_if_changed(desktop, &self.last_windows);
        if topology_changed && !self.window_routes.is_empty() {
            let interrupted: Vec<_> = self.window_routes.keys().copied().collect();
            self.window_routes.clear();
            for creature_id in interrupted {
                self.window_journeys.remove(&creature_id);
                if let Some(creature) = creature_mut(&mut self.save.creatures, creature_id) {
                    settle_interrupted_journey(
                        creature,
                        desktop,
                        &self.save.settings.habitat,
                        &mut self.events,
                    );
                }
            }
        }
        if self.save.settings.visible && !self.save.settings.paused {
            self.topology.update_cursor_invitation(desktop, dt);
        } else {
            self.topology.clear_invitation();
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
            &self.topology,
            &mut self.events,
        );
        if self
            .colony_plan
            .as_ref()
            .is_some_and(|plan| !plan.geometry_is_valid(desktop, &self.save.settings.habitat))
        {
            self.interrupt_colony_plan(timeline_now);
        }
        self.advance_colony_plan(timeline_now, dt, desktop);
        let at_selection_boundary = self.save.creatures.iter().any(|creature| {
            creature.state.arrival_delay_secs <= 0.0
                && creature.state.action_elapsed + dt >= creature.state.action_duration
        });
        if self.colony_plan.is_none()
            && self.save.ritual.next_at_utc <= timeline_now
            && at_selection_boundary
        {
            self.try_start_colony_plan(timeline_now, desktop);
        }
        let creature_views = self.save.creatures.clone();
        let relationship_views = self.save.relationships.clone();
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
                if !route_point_valid {
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
                            kind: SurfaceKind::WindowLedge,
                        },
                    );
                    let next_hop = self
                        .window_routes
                        .get_mut(&creature.id)
                        .filter(|plan| plan.geometry_hash == self.topology.geometry_hash())
                        .and_then(|plan| plan.remaining.pop_front());
                    if let Some(hop) = next_hop {
                        let journey = build_route_hop_journey(creature, hop, desktop);
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
                    creature.state.action = ActionKind::Perch;
                    creature.state.action_elapsed = 0.0;
                    creature.state.action_duration = 3.5;
                    creature.state.velocity = Point::default();
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
                bond: preferred_bond_context(creature, &creature_views, &relationship_views),
                on_window_ledge: creature.state.surface.kind == SurfaceKind::WindowLedge,
                // A ledge is a destination, not a one-time upgrade from the desktop floor.
                // Continuing to search while perched lets creatures climb between stacked
                // application windows and later descend when the desktop arrangement changes.
                reachable_window_ledge: find_nearby_ledge(
                    creature,
                    desktop,
                    &self.save.settings.habitat,
                    &self.topology,
                )
                .is_some(),
                window_changed_nearby: window_changed.contains(&creature.id),
                objects: nearby_object_utility(
                    creature,
                    &self.save.objects.objects,
                    desktop,
                    &self.save.settings.habitat,
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

            if self.colony_plan.is_none()
                && context.window_changed_nearby
                && creature.state.action != ActionKind::ReactToWindow
                && creature.state.action_elapsed >= 0.25
            {
                self.action_choices.remove(&creature.id);
                self.bond_plans.remove(&creature.id);
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
                            &creature_views,
                            plan.target,
                            plan.final_action,
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
                        &creature_views,
                        bond.target_creature,
                        ActionKind::Greet,
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
                    && rng.random_ratio(1, 3)
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
                    && rng.random_ratio(1, 4)
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
                    && rng.random_ratio(1, 64)
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
                    creature.state.activity_variant = self.ambient_rng.random_range(0..8);
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
                    creature.state.activity_variant = 0;
                    selected_choice = Some(choose_action(creature, desktop, context, rng));
                }
                let mut choice = selected_choice.expect("an action is always selected");
                let selected = choice.action;
                if !continuing_plan
                    && let Some(target) = choice.target_creature
                    && let Some(experience) = explicit_experience
                        .or_else(|| relationship_experience_for_action(choice.action))
                {
                    let target_point =
                        bond_target_point(creature, &creature_views, target, choice.action);
                    if let Some(target_point) = target_point {
                        let final_action = choice.action;
                        let approaching = bond_approach_required(
                            creature.state.position,
                            target_point,
                            final_action,
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
                if selected == ActionKind::Perch {
                    let mut route = planned_window_route(
                        creature,
                        desktop,
                        &self.save.settings.habitat,
                        &self.topology,
                    );
                    if !route.is_empty() {
                        let first = route.remove(0);
                        let journey = build_route_hop_journey(creature, first, desktop);
                        next = journey.initial_action();
                        self.window_routes.insert(
                            creature.id,
                            WindowRoutePlan {
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
                    match bond_target_point(creature, &creature_views, target, action) {
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
            constrain_to_surface(creature, desktop, &self.save.settings.habitat);
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

        let mut calm_pairs = BTreeSet::new();
        for (index, first) in views.iter().enumerate() {
            for second in views.iter().skip(index + 1) {
                if first.state.arrival_delay_secs > 0.0
                    || second.state.arrival_delay_secs > 0.0
                    || first.state.surface.monitor_id != second.state.surface.monitor_id
                    || first.state.position.distance(second.state.position) > 120.0
                    || !calm_for_proximity(first.state.action)
                    || !calm_for_proximity(second.state.action)
                {
                    continue;
                }
                let pair = canonical_creature_pair(first.id, second.id).expect("distinct pair");
                calm_pairs.insert(pair);
                let elapsed = self.calm_proximity_seconds.entry(pair).or_default();
                *elapsed = elapsed.saturating_add(OBSERVATION_INTERVAL_SECS as u16);
                if *elapsed >= 5 * 60 {
                    *elapsed -= 5 * 60;
                    Self::emit(
                        &mut self.events,
                        WorldEvent::BondInteraction {
                            a: pair.0,
                            b: pair.1,
                            experience: RelationshipExperience::CalmProximity,
                        },
                    );
                }
            }
        }
        self.calm_proximity_seconds
            .retain(|pair, _| calm_pairs.contains(pair));
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
                WorldEvent::BondInteraction { a, b, experience } => {
                    if let Some(relationship) =
                        relationship_mut_or_insert(&mut self.save.relationships, a, b)
                    {
                        relationship.apply(experience);
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

    fn eligible_ritual_kinds(
        &self,
        now: OffsetDateTime,
        desktop: &DesktopSnapshot,
        shelter_available: bool,
    ) -> Vec<RitualKind> {
        let local_now = local_time_or_utc(now);
        let local_created = self.save.created_at_utc.to_offset(local_now.offset());
        let hatch_day_due = local_now.year() > local_created.year()
            && local_now.month() == local_created.month()
            && local_now.day() == local_created.day()
            && self.save.ritual.hatch_day_acknowledged_year != Some(local_now.year());
        if hatch_day_due {
            return vec![RitualKind::HatchDay];
        }

        let late_night = local_now.hour() >= 22 || local_now.hour() < 5;
        let current_windows: BTreeMap<_, _> = desktop
            .windows
            .iter()
            .map(|window| (window.key, window.bounds))
            .collect();
        let quiet_day = desktop.idle_duration >= std::time::Duration::from_secs(10 * 60)
            && current_windows == self.last_windows;
        RitualKind::ALL
            .into_iter()
            .filter(|kind| match kind {
                RitualKind::FloorRace => !self.save.settings.reduce_motion,
                RitualKind::ShelterGathering => shelter_available,
                RitualKind::HatchDay => false,
                RitualKind::QuietDayHuddle => quiet_day,
                RitualKind::LateNightSleepPile => late_night,
                _ => true,
            })
            .collect()
    }

    fn choose_ritual_kind(
        &self,
        now: OffsetDateTime,
        desktop: &DesktopSnapshot,
        shelter_available: bool,
    ) -> Option<RitualKind> {
        let mut eligible = self.eligible_ritual_kinds(now, desktop, shelter_available);
        if eligible.len() > 1
            && let Some(previous) = self.save.ritual.last_kind
        {
            eligible.retain(|kind| *kind != previous);
        }
        if eligible.is_empty() {
            return None;
        }
        let streams = SeedStream::new(self.save.colony_seed);
        let mut rng = streams.rng("ritual-kind", u64::from(self.save.ritual.ordinal));
        let index = rng.random_range(0..eligible.len());
        eligible.get(index).copied()
    }

    fn try_start_colony_plan(&mut self, now: OffsetDateTime, desktop: &DesktopSnapshot) -> bool {
        if !self.save.settings.visible
            || self.save.settings.paused
            || self.save.home.is_active()
            || self.interaction.is_some()
            || !self.tosses.is_empty()
            || !self.window_journeys.is_empty()
        {
            return false;
        }
        let revealed_count = self
            .save
            .creatures
            .iter()
            .filter(|creature| creature.state.arrival_delay_secs <= 0.0)
            .count();
        let mut available: Vec<_> = self
            .save
            .creatures
            .iter()
            .filter(|creature| creature.state.arrival_delay_secs <= 0.0)
            .filter(|creature| {
                creature.state.surface.kind == SurfaceKind::ScreenFloor
                    && !matches!(
                        creature.state.action,
                        ActionKind::Dragged | ActionKind::Tossed | ActionKind::Homebound
                    )
            })
            .map(|creature| {
                (
                    creature.colony_order,
                    creature.id,
                    creature.state.surface.monitor_id,
                )
            })
            .collect();
        if available.len() < RITUAL_MIN_CREATURES || available.len() != revealed_count {
            return false;
        }
        available.sort_unstable();
        let monitor_id = available[0].2;
        if available.iter().any(|(_, _, id)| *id != monitor_id) {
            return false;
        }
        let Some(monitor) = desktop
            .monitors
            .iter()
            .find(|monitor| monitor.id == monitor_id)
            .cloned()
        else {
            return false;
        };
        let mut regions = accessible_regions(&self.save.settings.habitat, &monitor);
        regions.sort_by(|a, b| {
            (b.width * b.height)
                .total_cmp(&(a.width * a.height))
                .then_with(|| a.x.total_cmp(&b.x))
        });
        let Some(region) = regions.first().copied() else {
            return false;
        };
        let shelter_anchor = (self.save.home.display == Some(monitor.display_key))
            .then(|| {
                resolved_home_anchor(
                    &self.save.home,
                    &monitor,
                    self.save.settings.display_scale,
                    &self.save.settings.habitat,
                )
            })
            .flatten();
        let kind = match self.choose_ritual_kind(now, desktop, shelter_anchor.is_some()) {
            Some(kind) => kind,
            None => return false,
        };
        if kind == RitualKind::Catch {
            available.truncate(2);
        }

        let count = available.len();
        let creature_width = CREATURE_ART_WIDTH * f32::from(self.save.settings.display_scale)
            / monitor.scale_factor.max(1.0);
        let spacing = (creature_width * 0.72).max(22.0);
        let total_width = spacing * (count.saturating_sub(1)) as f32;
        let region_margin = (creature_width * 0.55).max(12.0);
        if region.width < total_width + region_margin * 2.0 {
            return false;
        }
        let average_x = available
            .iter()
            .filter_map(|(_, creature_id, _)| {
                self.save
                    .creatures
                    .iter()
                    .find(|creature| creature.id == *creature_id)
                    .map(|creature| creature.state.position.x)
            })
            .sum::<f32>()
            / count as f32;
        let default_center = average_x.clamp(
            region.x + region_margin + total_width * 0.5,
            region.right() - region_margin - total_width * 0.5,
        );
        let center = if kind == RitualKind::ShelterGathering {
            shelter_anchor
                .expect("shelter ritual has a resolved anchor")
                .x
                .clamp(
                    region.x + region_margin + total_width * 0.5,
                    region.right() - region_margin - total_width * 0.5,
                )
        } else {
            default_center
        };
        let floor_y = region.bottom() - 4.0;
        let start_x = center - total_width * 0.5;
        let race_start = region.x + region_margin;
        let race_finish = region.right() - region_margin;
        let mut participants = Vec::with_capacity(count);
        for (index, (_, creature_id, _)) in available.iter().enumerate() {
            let lineup = Point {
                x: start_x + index as f32 * spacing,
                y: floor_y,
            };
            let (approach_target, ceremony_target, ceremony_action) = match kind {
                RitualKind::Picnic => (
                    lineup,
                    lineup,
                    if index % 2 == 0 {
                        ActionKind::Eat
                    } else {
                        ActionKind::Drink
                    },
                ),
                RitualKind::GroupNap | RitualKind::LateNightSleepPile => {
                    (lineup, lineup, ActionKind::Sleep)
                }
                RitualKind::FloorRace => (
                    Point {
                        x: race_start + index as f32 * spacing * 0.3,
                        y: floor_y,
                    },
                    Point {
                        x: race_finish - index as f32 * spacing * 0.2,
                        y: floor_y,
                    },
                    ActionKind::Sprint,
                ),
                RitualKind::ShelterGathering => (lineup, lineup, ActionKind::Homebound),
                RitualKind::Catch => (lineup, lineup, ActionKind::SocialPlay),
                RitualKind::GroupPresentation => (
                    lineup,
                    Point {
                        x: center,
                        y: floor_y,
                    },
                    if index == 0 {
                        ActionKind::PresentDiscovery
                    } else {
                        ActionKind::InspectScreen
                    },
                ),
                RitualKind::HatchDay => (lineup, lineup, ActionKind::Greet),
                RitualKind::QuietDayHuddle => (lineup, lineup, ActionKind::Idle),
            };
            participants.push(RitualParticipant {
                creature_id: *creature_id,
                approach_target,
                ceremony_target,
                ceremony_action,
            });
        }

        self.action_choices.clear();
        self.bond_plans.clear();
        self.pending_home_greetings.clear();
        for participant in &participants {
            let Some(creature) = creature_mut(&mut self.save.creatures, participant.creature_id)
            else {
                return false;
            };
            let old = creature.state.action;
            if old == ActionKind::Sleep {
                let elapsed = self
                    .sleep_elapsed
                    .remove(&creature.id)
                    .unwrap_or(creature.state.action_elapsed)
                    .max(0.0) as u32;
                Self::emit(
                    &mut self.events,
                    WorldEvent::CreatureRested {
                        creature_id: creature.id,
                        uninterrupted_seconds: elapsed,
                    },
                );
                Self::emit(
                    &mut self.events,
                    WorldEvent::CreatureWoke {
                        creature_id: creature.id,
                    },
                );
            }
            Self::emit(
                &mut self.events,
                WorldEvent::ActionCompleted {
                    creature_id: creature.id,
                    action: old,
                },
            );
            creature.state.action = ActionKind::Traverse;
            creature.state.action_elapsed = 0.0;
            creature.state.action_duration = f32::MAX;
            creature.state.velocity = Point::default();
            creature.state.surface.window_key = None;
            creature.state.surface.kind = SurfaceKind::ScreenFloor;
            creature.state.surface.monitor_id = monitor_id;
            self.action_choices.insert(
                creature.id,
                ActionChoice {
                    action: ActionKind::Traverse,
                    target_creature: None,
                    target_point: Some(participant.approach_target),
                },
            );
            Self::emit(
                &mut self.events,
                WorldEvent::ActionStarted {
                    creature_id: creature.id,
                    action: ActionKind::Traverse,
                },
            );
        }
        if kind == RitualKind::ShelterGathering {
            self.save.home.active_since_utc = Some(now);
            Self::emit(&mut self.events, WorldEvent::HomeAppeared);
        }
        let local_now = local_time_or_utc(now);
        if kind == RitualKind::HatchDay {
            self.save.ritual.hatch_day_acknowledged_year = Some(local_now.year());
        }
        self.save.ritual.last_kind = Some(kind);
        self.save.ritual.ordinal = self.save.ritual.ordinal.saturating_add(1);
        self.save.ritual.next_at_utc =
            scheduled_ritual_at(self.save.colony_seed, self.save.ritual.ordinal, now);
        self.colony_plan = Some(ColonyPlan {
            kind,
            monitor_id,
            usable_bounds: monitor.usable_bounds,
            participants,
            phase: RitualPhase::Approach,
            remaining_secs: RITUAL_APPROACH_SECS,
        });
        Self::emit(&mut self.events, WorldEvent::RitualStarted { kind });
        true
    }

    fn advance_colony_plan(&mut self, now: OffsetDateTime, dt: f32, _desktop: &DesktopSnapshot) {
        let Some(plan) = &mut self.colony_plan else {
            return;
        };
        plan.remaining_secs = (plan.remaining_secs - dt.max(0.0)).max(0.0);
        let gathered = plan.phase == RitualPhase::Approach
            && plan.participants.iter().all(|participant| {
                self.save
                    .creatures
                    .iter()
                    .find(|creature| creature.id == participant.creature_id)
                    .is_some_and(|creature| {
                        (creature.state.position.x - participant.approach_target.x).abs() <= 8.0
                    })
            });
        if plan.phase == RitualPhase::Approach && (gathered || plan.remaining_secs <= 0.0) {
            let kind = plan.kind;
            let participants = plan.participants.clone();
            plan.phase = RitualPhase::Ceremony;
            plan.remaining_secs = ritual_ceremony_duration(kind);
            for participant in participants {
                let Some(creature) =
                    creature_mut(&mut self.save.creatures, participant.creature_id)
                else {
                    self.interrupt_colony_plan(now);
                    return;
                };
                let old = creature.state.action;
                Self::emit(
                    &mut self.events,
                    WorldEvent::ActionCompleted {
                        creature_id: creature.id,
                        action: old,
                    },
                );
                creature.state.action = participant.ceremony_action;
                creature.state.action_elapsed = 0.0;
                creature.state.action_duration = f32::MAX;
                creature.state.velocity = Point::default();
                if participant.ceremony_action == ActionKind::PresentDiscovery {
                    creature.state.activity_variant =
                        (self.save.ritual.ordinal as u8).wrapping_sub(1) % 8;
                }
                self.action_choices.insert(
                    creature.id,
                    ActionChoice {
                        action: participant.ceremony_action,
                        target_creature: None,
                        target_point: Some(participant.ceremony_target),
                    },
                );
                Self::emit(
                    &mut self.events,
                    WorldEvent::ActionStarted {
                        creature_id: creature.id,
                        action: participant.ceremony_action,
                    },
                );
                if participant.ceremony_action == ActionKind::Sleep {
                    Self::emit(
                        &mut self.events,
                        WorldEvent::CreatureSlept {
                            creature_id: creature.id,
                        },
                    );
                }
            }
            return;
        }
        if plan.phase != RitualPhase::Ceremony || plan.remaining_secs > 0.0 {
            return;
        }

        let plan = self.colony_plan.take().expect("active ritual exists");
        let ids: Vec<_> = plan
            .participants
            .iter()
            .map(|participant| participant.creature_id)
            .collect();
        for participant in &plan.participants {
            self.action_choices.remove(&participant.creature_id);
            if let Some(creature) = creature_mut(&mut self.save.creatures, participant.creature_id)
            {
                let old = creature.state.action;
                let old_elapsed = creature.state.action_elapsed;
                Self::emit(
                    &mut self.events,
                    WorldEvent::ActionCompleted {
                        creature_id: creature.id,
                        action: old,
                    },
                );
                if old == ActionKind::Sleep {
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
                }
                creature.state.action = ActionKind::Idle;
                creature.state.action_elapsed = 0.0;
                creature.state.action_duration = 2.5;
                creature.state.velocity = Point::default();
                creature.state.activity_variant = 0;
                Self::emit(
                    &mut self.events,
                    WorldEvent::ActionStarted {
                        creature_id: creature.id,
                        action: ActionKind::Idle,
                    },
                );
            }
        }
        let experience = match plan.kind {
            RitualKind::GroupNap | RitualKind::QuietDayHuddle | RitualKind::LateNightSleepPile => {
                RelationshipExperience::SharedRest
            }
            RitualKind::FloorRace | RitualKind::Catch | RitualKind::HatchDay => {
                RelationshipExperience::PositivePlay
            }
            RitualKind::Picnic | RitualKind::ShelterGathering | RitualKind::GroupPresentation => {
                RelationshipExperience::Greeting
            }
        };
        for (index, first) in ids.iter().copied().enumerate() {
            for second in ids.iter().copied().skip(index + 1) {
                Self::emit(
                    &mut self.events,
                    WorldEvent::BondInteraction {
                        a: first,
                        b: second,
                        experience,
                    },
                );
            }
        }
        if plan.kind == RitualKind::ShelterGathering {
            self.dismiss_home(now, false);
        }
        Self::emit(
            &mut self.events,
            WorldEvent::RitualCompleted { kind: plan.kind },
        );
    }

    fn interrupt_colony_plan(&mut self, now: OffsetDateTime) {
        let Some(plan) = self.colony_plan.take() else {
            return;
        };
        for participant in &plan.participants {
            self.action_choices.remove(&participant.creature_id);
            self.bond_plans.remove(&participant.creature_id);
            if let Some(creature) = creature_mut(&mut self.save.creatures, participant.creature_id)
                && !matches!(
                    creature.state.action,
                    ActionKind::Dragged | ActionKind::Tossed
                )
            {
                creature.state.action = ActionKind::Idle;
                creature.state.action_elapsed = 0.0;
                creature.state.action_duration = 2.5;
                creature.state.velocity = Point::default();
                creature.state.activity_variant = 0;
            }
        }
        if plan.kind == RitualKind::ShelterGathering && self.save.home.is_active() {
            self.dismiss_home(now, true);
        }
        self.save.ritual.next_at_utc =
            interrupted_ritual_at(self.save.colony_seed, self.save.ritual.ordinal, now);
        Self::emit(
            &mut self.events,
            WorldEvent::RitualInterrupted { kind: plan.kind },
        );
    }

    fn update_home_cycle(&mut self, desktop: &DesktopSnapshot) -> bool {
        let timeline_now = self.save.maximum_seen_utc;
        let ritual_shelter = self
            .colony_plan
            .as_ref()
            .is_some_and(|plan| plan.kind == RitualKind::ShelterGathering);
        if !ritual_shelter
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
            self.save.home.active_since_utc = Some(timeline_now);
            self.window_journeys.clear();
            self.window_routes.clear();
            self.tosses.clear();
            self.action_choices.clear();
            self.bond_plans.clear();
            Self::emit(&mut self.events, WorldEvent::HomeAppeared);
        }

        if ritual_shelter {
            return false;
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
            if creature.state.arrival_delay_secs <= 0.0 {
                self.pending_home_greetings.insert(creature.id);
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
                self.interrupt_colony_plan(self.save.maximum_seen_utc);
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
        self.window_routes.remove(&creature_id);
        let interrupted_toss = self.tosses.remove(&creature_id);
        self.action_choices.remove(&creature_id);
        self.bond_plans.remove(&creature_id);
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
        if self.colony_plan.is_some() {
            self.interrupt_colony_plan(self.save.maximum_seen_utc);
        }
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
        self.window_routes.clear();
        self.tosses.clear();
        self.action_choices.clear();
        self.bond_plans.clear();
        self.pending_home_greetings.clear();
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
                add_arrival_relationships(
                    &mut self.save.relationships,
                    &self.save.creatures,
                    creature.id,
                    primary.as_ref().map(|parent| parent.id),
                );
                self.save.creatures.push(creature);
                self.save.arrival_state.arrived[index] = true;
                arrivals_this_tick += 1;
            }
        }
    }

    fn process_colony_objects(&mut self, now: OffsetDateTime, desktop: &DesktopSnapshot) {
        if self.save.objects.objects.len() >= MAX_COLONY_OBJECTS
            || self.save.objects.next_at_utc > now
        {
            return;
        }
        let mut monitors: Vec<_> = desktop
            .monitors
            .iter()
            .filter(|monitor| !accessible_regions(&self.save.settings.habitat, monitor).is_empty())
            .collect();
        monitors.sort_by_key(|monitor| {
            (
                self.save.home.display != Some(monitor.display_key),
                !monitor.primary,
                monitor.id,
            )
        });
        let Some(monitor) = monitors.first().copied() else {
            return;
        };
        let mut regions = accessible_regions(&self.save.settings.habitat, monitor);
        regions.sort_by(|a, b| {
            (b.width * b.height)
                .total_cmp(&(a.width * a.height))
                .then_with(|| a.x.total_cmp(&b.x))
        });
        let Some(region) = regions.first().copied() else {
            return;
        };
        let streams = SeedStream::new(self.save.colony_seed);
        let mut rng = streams.rng("colony-object", u64::from(self.save.objects.ordinal));
        let kind = ColonyObjectKind::ALL[rng.random_range(0..ColonyObjectKind::ALL.len())];
        let point = Point {
            x: rng.random_range(region.x + 12.0..=region.right() - 12.0),
            y: region.bottom() - 4.0,
        };
        let mut id = rng.random::<u64>();
        while self
            .save
            .objects
            .objects
            .iter()
            .any(|object| object.id == id)
        {
            id = id.wrapping_add(1);
        }
        let object = ColonyObject {
            id,
            kind,
            display: monitor.display_key,
            normalized_position: Point {
                x: ((point.x - monitor.usable_bounds.x) / monitor.usable_bounds.width)
                    .clamp(0.0, 1.0),
                y: ((point.y - monitor.usable_bounds.y) / monitor.usable_bounds.height)
                    .clamp(0.0, 1.0),
            },
            role: kind.default_role(),
        };
        self.save.objects.objects.push(object);
        self.save.objects.ordinal = self.save.objects.ordinal.saturating_add(1);
        self.save.objects.next_at_utc =
            scheduled_colony_object_at(self.save.colony_seed, self.save.objects.ordinal, now);
        Self::emit(
            &mut self.events,
            WorldEvent::ColonyObjectAdded {
                object_id: id,
                kind,
            },
        );
    }

    fn process_shelter_decorations(&mut self, now: OffsetDateTime) {
        if self.save.home.decorations.decorations.len() >= MAX_SHELTER_DECORATIONS
            || self.save.home.decorations.next_at_utc > now
        {
            return;
        }
        let Some(kind) = preferred_shelter_decoration(&self.save) else {
            return;
        };
        self.save.home.decorations.decorations.push(kind);
        self.save.home.decorations.ordinal = self.save.home.decorations.ordinal.saturating_add(1);
        self.save.home.decorations.next_at_utc = scheduled_shelter_decoration_at(
            self.save.colony_seed,
            self.save.home.decorations.ordinal,
            now,
        );
        Self::emit(
            &mut self.events,
            WorldEvent::ShelterDecorationAdded { kind },
        );
    }

    fn reconcile_colony_objects(&mut self, desktop: &DesktopSnapshot) {
        for object in &mut self.save.objects.objects {
            let Some((monitor_id, point)) = resolved_colony_object_position(
                object,
                &desktop.monitors,
                &self.save.settings.habitat,
            ) else {
                continue;
            };
            let Some(monitor) = desktop
                .monitors
                .iter()
                .find(|monitor| monitor.id == monitor_id)
            else {
                continue;
            };
            object.display = monitor.display_key;
            object.normalized_position = Point {
                x: ((point.x - monitor.usable_bounds.x) / monitor.usable_bounds.width)
                    .clamp(0.0, 1.0),
                y: ((point.y - monitor.usable_bounds.y) / monitor.usable_bounds.height)
                    .clamp(0.0, 1.0),
            };
        }
    }
}

fn normalize_relationships(save: &mut SaveFile) {
    let creature_ids: BTreeSet<_> = save.creatures.iter().map(|creature| creature.id).collect();
    let mut canonical = BTreeMap::new();
    for mut relationship in save.relationships.drain(..) {
        let Some((a, b)) = canonical_creature_pair(relationship.a, relationship.b) else {
            continue;
        };
        if !creature_ids.contains(&a) || !creature_ids.contains(&b) {
            continue;
        }
        relationship.a = a;
        relationship.b = b;
        canonical
            .entry((a, b))
            .and_modify(|existing: &mut CreatureRelationship| {
                existing.affinity = existing.affinity.max(relationship.affinity);
                existing.familiarity = existing.familiarity.max(relationship.familiarity);
                existing.playfulness = existing.playfulness.max(relationship.playfulness);
                existing.avoidance = existing.avoidance.max(relationship.avoidance);
            })
            .or_insert(relationship);
    }
    let ids: Vec<_> = creature_ids.into_iter().collect();
    for (index, a) in ids.iter().copied().enumerate() {
        for b in ids.iter().copied().skip(index + 1) {
            canonical
                .entry((a, b))
                .or_insert_with(|| CreatureRelationship::new(a, b).expect("distinct pair"));
        }
    }
    save.relationships = canonical.into_values().take(MAX_RELATIONSHIPS).collect();
}

fn relationship_mut_or_insert(
    relationships: &mut Vec<CreatureRelationship>,
    a: CreatureId,
    b: CreatureId,
) -> Option<&mut CreatureRelationship> {
    let pair = canonical_creature_pair(a, b)?;
    if let Some(index) = relationships
        .iter()
        .position(|relationship| relationship.a == pair.0 && relationship.b == pair.1)
    {
        return relationships.get_mut(index);
    }
    if relationships.len() >= MAX_RELATIONSHIPS {
        return None;
    }
    relationships.push(CreatureRelationship::new(pair.0, pair.1)?);
    relationships.last_mut()
}

fn calm_for_proximity(action: ActionKind) -> bool {
    !matches!(
        action,
        ActionKind::Dragged
            | ActionKind::Tossed
            | ActionKind::AvoidCursor
            | ActionKind::ReactToWindow
            | ActionKind::Sprint
            | ActionKind::ClimbWindow
            | ActionKind::Dangle
    )
}

fn add_arrival_relationships(
    relationships: &mut Vec<CreatureRelationship>,
    creatures: &[Creature],
    arriving: CreatureId,
    parent: Option<CreatureId>,
) {
    for creature in creatures {
        if relationships.len() >= MAX_RELATIONSHIPS {
            break;
        }
        let Some(mut relationship) = CreatureRelationship::new(creature.id, arriving) else {
            continue;
        };
        if parent == Some(creature.id) {
            relationship.affinity = 217;
            relationship.familiarity = 48;
            relationship.playfulness = 64;
        }
        relationships.push(relationship);
    }
    relationships.sort_by_key(|relationship| (relationship.a, relationship.b));
    relationships.dedup_by_key(|relationship| (relationship.a, relationship.b));
}

fn preferred_bond_context(
    creature: &Creature,
    creatures: &[Creature],
    relationships: &[CreatureRelationship],
) -> Option<BondContext> {
    relationships
        .iter()
        .copied()
        .filter_map(|relationship| {
            let target_id = relationship.other(creature.id)?;
            let target = creatures
                .iter()
                .find(|target| target.id == target_id && target.state.arrival_delay_secs <= 0.0)?;
            let distance = creature.state.position.distance(target.state.position);
            let same_monitor = creature.state.surface.monitor_id == target.state.surface.monitor_id;
            let score = relationship.closeness()
                + i16::from(relationship.playfulness) / 2
                + if same_monitor { 32 } else { -128 };
            Some((
                score,
                std::cmp::Reverse(target.id),
                target,
                relationship,
                distance,
            ))
        })
        .max_by_key(|(score, target_id, ..)| (*score, *target_id))
        .map(|(_, _, target, relationship, distance)| BondContext {
            target_creature: target.id,
            target_position: target.state.position,
            distance,
            relationship,
            target_action: target.state.action,
            target_surface: target.state.surface.kind,
        })
}

fn relationship_experience_for_action(action: ActionKind) -> Option<RelationshipExperience> {
    match action {
        ActionKind::Follow => Some(RelationshipExperience::Followed),
        ActionKind::Greet => Some(RelationshipExperience::Greeting),
        ActionKind::Sleep => Some(RelationshipExperience::SharedRest),
        ActionKind::SocialPlay => Some(RelationshipExperience::PositivePlay),
        _ => None,
    }
}

fn bond_approach_required(actor: Point, target: Point, final_action: ActionKind) -> bool {
    let threshold = match final_action {
        ActionKind::Sleep => 58.0,
        ActionKind::Greet | ActionKind::SocialPlay => 72.0,
        ActionKind::PresentDiscovery => 96.0,
        _ => return false,
    };
    actor.distance(target) > threshold
}

fn bond_target_point(
    actor: &Creature,
    creatures: &[Creature],
    target_id: CreatureId,
    action: ActionKind,
) -> Option<Point> {
    let target = creatures.iter().find(|target| {
        target.id == target_id
            && target.state.arrival_delay_secs <= 0.0
            && target.state.surface.monitor_id == actor.state.surface.monitor_id
            && (action == ActionKind::ReactToWindow
                || target.state.surface.kind == actor.state.surface.kind)
    })?;
    let allowed = match action {
        ActionKind::ReactToWindow => !matches!(
            target.state.action,
            ActionKind::Dragged | ActionKind::Homebound
        ),
        ActionKind::InspectScreen => matches!(
            target.state.action,
            ActionKind::ClimbWindow | ActionKind::Perch
        ),
        ActionKind::Sleep => !matches!(
            target.state.action,
            ActionKind::Dragged | ActionKind::Tossed | ActionKind::Homebound
        ),
        ActionKind::Greet | ActionKind::SocialPlay | ActionKind::PresentDiscovery => !matches!(
            target.state.action,
            ActionKind::Sleep | ActionKind::Dragged | ActionKind::Tossed | ActionKind::Homebound
        ),
        ActionKind::Follow => !matches!(
            target.state.action,
            ActionKind::Sleep | ActionKind::Dragged | ActionKind::Tossed | ActionKind::Homebound
        ),
        _ => true,
    };
    if !allowed {
        return None;
    }
    if action == ActionKind::Sleep {
        let side = if actor.state.position.x <= target.state.position.x {
            -1.0
        } else {
            1.0
        };
        Some(Point {
            x: target.state.position.x + side * 30.0,
            y: target.state.position.y,
        })
    } else {
        Some(target.state.position)
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
    let mut target_stop_distance = 0.0;
    let mut target_speed_multiplier = 1.0;
    match creature.state.action {
        ActionKind::Traverse if selected_target.is_some() => {
            target_x = selected_target.map(|target| target.x);
        }
        ActionKind::Sprint if selected_target.is_some() => {
            target_x = selected_target.map(|target| target.x);
            target_speed_multiplier = 2.35;
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
            target_x = selected_target
                .map(|target| target.x)
                .or_else(|| nearest.map(|item| item.1.x));
            target_stop_distance = if creature.state.action == ActionKind::Follow {
                42.0
            } else {
                30.0
            };
            creature.state.drives.social_need =
                (creature.state.drives.social_need - dt * 0.08).max(0.0);
        }
        ActionKind::Sleep if selected_target.is_some() => {
            target_x = selected_target.map(|target| target.x);
            target_stop_distance = 5.0;
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
            if let Some(target) = selected_target {
                creature.state.facing_right = target.x >= creature.state.position.x;
            }
            creature.state.drives.curiosity_satisfaction =
                (creature.state.drives.curiosity_satisfaction + dt * 0.055).min(1.0);
            creature.state.drives.boredom = (creature.state.drives.boredom - dt * 0.035).max(0.0);
        }
        ActionKind::PresentDiscovery => {
            creature.state.drives.curiosity_satisfaction =
                (creature.state.drives.curiosity_satisfaction + dt * 0.035).min(1.0);
            creature.state.drives.comfort = (creature.state.drives.comfort + dt * 0.012).min(1.0);
            if let Some(target) = selected_target {
                creature.state.facing_right = target.x >= creature.state.position.x;
            }
        }
        ActionKind::ReactToWindow => {
            if let Some(target) = selected_target {
                creature.state.facing_right = target.x >= creature.state.position.x;
                creature.state.velocity.x = 0.0;
            } else {
                creature.state.velocity.x = if creature.state.facing_right {
                    speed * 1.4
                } else {
                    -speed * 1.4
                };
            }
            creature.state.drives.arousal = (creature.state.drives.arousal + dt * 0.5).min(1.0);
        }
        _ => creature.state.velocity.x *= (1.0 - dt * 8.0).max(0.0),
    }
    if let Some(target) = target_x {
        let dx = target - creature.state.position.x;
        creature.state.facing_right = dx >= 0.0;
        creature.state.velocity.x = if dx.abs() <= target_stop_distance {
            0.0
        } else {
            dx.signum() * speed * target_speed_multiplier
        };
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
            .or_else(|| {
                desktop
                    .monitors
                    .iter()
                    .find(|monitor| monitor.bounds.contains(creature.state.position))
            })
            .or(primary);
        if let Some(monitor) = monitor {
            // Native display identifiers can change after sleep, hot-plugging, or a display-mode
            // transition. Rendering filters by the current identifier, so retaining a stale ID
            // leaves an otherwise valid creature alive in the simulation but absent from every
            // overlay until restart. Rebind it to the monitor that actually contains its point.
            creature.state.surface.monitor_id = monitor.id;
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
    topology: &DesktopTopology,
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
                    let calm_platform = topology.is_slow_platform(key);
                    creature.state.action = if !calm_platform
                        && rapid
                        && creature.personality.window_tolerance < 0.72
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

fn build_route_hop_journey(
    creature: &Creature,
    hop: TopologyRouteHop,
    desktop: &DesktopSnapshot,
) -> WindowJourney {
    let surface = SurfaceAttachment {
        kind: SurfaceKind::WindowLedge,
        monitor_id: hop.monitor_id,
        window_key: Some(hop.to_window),
        relative_x: ((hop.target.x - hop.to_bounds.x) / hop.to_bounds.width).clamp(0.05, 0.95),
    };
    match hop.kind {
        RouteHopKind::WindowTier => build_window_journey(creature, hop.target, surface, desktop),
        RouteHopKind::NarrowGap => WindowJourney::Squeeze(SqueezeJourney {
            from_window: hop.from_window,
            from_bounds: hop.from_bounds,
            target_window: hop.to_window,
            target_bounds: hop.to_bounds,
            start: creature.state.position,
            target: hop.target,
            surface,
            elapsed: 0.0,
            duration: (creature.state.position.distance(hop.target) / 52.0).clamp(0.7, 4.0),
        }),
    }
}

fn planned_window_route(
    creature: &Creature,
    desktop: &DesktopSnapshot,
    policy: &HabitatPolicy,
    topology: &DesktopTopology,
) -> Vec<TopologyRouteHop> {
    let Some(start_window) = creature.state.surface.window_key else {
        return Vec::new();
    };
    let preferred = creature.memory.preferred_region.and_then(|preferred| {
        let monitor = desktop
            .monitors
            .iter()
            .find(|monitor| monitor.display_key == preferred.display)?;
        let column = f32::from(preferred.cell.min(8) % 3);
        let row = f32::from(preferred.cell.min(8) / 3);
        Some(Point {
            x: monitor.usable_bounds.x + monitor.usable_bounds.width * ((column + 0.5) / 3.0),
            y: monitor.usable_bounds.y + monitor.usable_bounds.height * ((row + 0.5) / 3.0),
        })
    });
    let target_hint = topology
        .invitation()
        .filter(|invitation| cursor_invitation_eligible(creature, *invitation))
        .map(|invitation| invitation.point)
        .or(preferred);
    let route = topology.plan_route(
        start_window,
        RoutePreferences {
            climbing: creature.tendencies.climbing,
            exploration: creature.tendencies.exploration,
            cursor_trust: creature.tendencies.cursor_trust,
            target_hint,
        },
    );
    if route.iter().all(|hop| {
        desktop
            .monitors
            .iter()
            .find(|monitor| monitor.id == hop.monitor_id)
            .is_some_and(|monitor| habitat_contains(policy, monitor, hop.target))
    }) {
        route
    } else {
        Vec::new()
    }
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
    topology: &DesktopTopology,
) -> Option<(Point, SurfaceAttachment)> {
    let current_window = creature.state.surface.window_key;
    let candidate = topology
        .windows()
        .iter()
        .filter(|window| window.bounds.width >= 120.0)
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
            let reachable = dx <= 360.0
                && (36.0..=640.0).contains(&dy)
                && habitat_contains(
                    policy,
                    monitor,
                    Point {
                        x: ledge_x,
                        y: window.bounds.y,
                    },
                );
            let island_bonus = if topology.island_windows().any(|key| key == window.key) {
                42.0 + f32::from(creature.tendencies.exploration.max(0)) * 0.2
            } else {
                0.0
            };
            // Nearby intermediate ledges remain easiest. Isolated window islands become slightly
            // more attractive to curious creatures without bypassing reachability or habitat.
            reachable.then_some((
                dx * 0.65 + dy * 0.12 - island_bonus,
                window,
                ledge_x,
                monitor.id,
            ))
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

fn topology_ledge_at_target(
    topology: &DesktopTopology,
    target: Point,
    desktop: &DesktopSnapshot,
    policy: &HabitatPolicy,
) -> Option<(Point, SurfaceAttachment)> {
    let window = topology
        .windows()
        .iter()
        .filter(|window| {
            target.x >= window.bounds.x + 12.0
                && target.x <= window.bounds.right() - 12.0
                && (target.y - window.bounds.y).abs() <= 24.0
        })
        .min_by(|a, b| {
            (target.y - a.bounds.y)
                .abs()
                .total_cmp(&(target.y - b.bounds.y).abs())
        })?;
    let point = Point {
        x: target
            .x
            .clamp(window.bounds.x + 12.0, window.bounds.right() - 12.0),
        y: window.bounds.y,
    };
    let monitor = desktop
        .monitors
        .iter()
        .find(|monitor| monitor.id == window.monitor_id)?;
    habitat_contains(policy, monitor, point).then_some((
        point,
        SurfaceAttachment {
            kind: SurfaceKind::WindowLedge,
            monitor_id: window.monitor_id,
            window_key: Some(window.key),
            relative_x: ((point.x - window.bounds.x) / window.bounds.width).clamp(0.05, 0.95),
        },
    ))
}

fn cursor_invitation_eligible(creature: &Creature, invitation: CursorInvitation) -> bool {
    creature.state.surface.monitor_id == invitation.monitor_id
        && creature.state.cursor_cooldown <= 0.0
        && creature.tendencies.cursor_trust >= -20
        && (creature.tendencies.cursor_trust >= 10
            || creature.personality.cursor_interest + creature.personality.boldness >= 1.15)
}

fn nearby_object_utility(
    creature: &Creature,
    objects: &[ColonyObject],
    desktop: &DesktopSnapshot,
    policy: &HabitatPolicy,
) -> ObjectUtility {
    let mut utility = ObjectUtility::default();
    for object in objects.iter().take(MAX_COLONY_OBJECTS) {
        let Some((monitor_id, point)) =
            resolved_colony_object_position(object, &desktop.monitors, policy)
        else {
            continue;
        };
        if monitor_id != creature.state.surface.monitor_id {
            continue;
        }
        let distance = creature.state.position.distance(point);
        if distance > 160.0 {
            continue;
        }
        utility.add(object.role, 0.08 * (1.0 - distance / 160.0));
    }
    utility
}

fn preferred_shelter_decoration(save: &SaveFile) -> Option<ShelterDecorationKind> {
    let mut scores = [0_u64; MAX_SHELTER_DECORATIONS];
    for creature in &save.creatures {
        let memory = &creature.memory;
        scores[ShelterDecorationKind::Leaf.index()] = scores[ShelterDecorationKind::Leaf.index()]
            .saturating_add(u64::from(memory.ledge_seconds / 60))
            .saturating_add(u64::from(memory.window_climbs).saturating_mul(20));
        scores[ShelterDecorationKind::Banner.index()] = scores
            [ShelterDecorationKind::Banner.index()]
        .saturating_add(u64::from(memory.times_petted).saturating_mul(3))
        .saturating_add(u64::from(memory.play_sessions).saturating_mul(2));
        scores[ShelterDecorationKind::Stone.index()] = scores[ShelterDecorationKind::Stone.index()]
            .saturating_add(u64::from(memory.placements).saturating_mul(4))
            .saturating_add(u64::from(memory.home_visits).saturating_mul(6));
        scores[ShelterDecorationKind::Flower.index()] = scores
            [ShelterDecorationKind::Flower.index()]
        .saturating_add(u64::from(memory.discoveries_found).saturating_mul(4))
        .saturating_add(u64::from(memory.times_petted));
        scores[ShelterDecorationKind::Lamp.index()] = scores[ShelterDecorationKind::Lamp.index()]
            .saturating_add(u64::from(memory.longest_sleep_seconds / 60))
            .saturating_add(u64::from(memory.home_visits).saturating_mul(8));
        scores[ShelterDecorationKind::RoofOrnament.index()] = scores
            [ShelterDecorationKind::RoofOrnament.index()]
        .saturating_add(u64::from(memory.window_climbs).saturating_mul(20))
        .saturating_add(u64::from(memory.window_ride_seconds / 60))
        .saturating_add(u64::from(memory.discoveries_found).saturating_mul(3));
    }
    for relationship in &save.relationships {
        scores[ShelterDecorationKind::Banner.index()] = scores
            [ShelterDecorationKind::Banner.index()]
        .saturating_add(u64::from(relationship.affinity))
        .saturating_add(u64::from(relationship.familiarity));
        scores[ShelterDecorationKind::Flower.index()] = scores
            [ShelterDecorationKind::Flower.index()]
        .saturating_add(u64::from(relationship.playfulness));
        scores[ShelterDecorationKind::Stone.index()] = scores[ShelterDecorationKind::Stone.index()]
            .saturating_add(u64::from(relationship.avoidance));
    }
    if let Some(kind) = save.ritual.last_kind {
        let decoration = match kind {
            RitualKind::Picnic => ShelterDecorationKind::Flower,
            RitualKind::GroupNap | RitualKind::LateNightSleepPile => ShelterDecorationKind::Lamp,
            RitualKind::FloorRace => ShelterDecorationKind::RoofOrnament,
            RitualKind::ShelterGathering | RitualKind::QuietDayHuddle => {
                ShelterDecorationKind::Leaf
            }
            RitualKind::Catch | RitualKind::GroupPresentation | RitualKind::HatchDay => {
                ShelterDecorationKind::Banner
            }
        };
        scores[decoration.index()] = scores[decoration.index()].saturating_add(256);
    }
    for object in &save.objects.objects {
        let decoration = match object.kind {
            ColonyObjectKind::Pillow | ColonyObjectKind::Blanket | ColonyObjectKind::Lamp => {
                ShelterDecorationKind::Lamp
            }
            ColonyObjectKind::Toy | ColonyObjectKind::Cup => ShelterDecorationKind::Banner,
            ColonyObjectKind::Plant => ShelterDecorationKind::Flower,
            ColonyObjectKind::Paper => ShelterDecorationKind::Leaf,
            ColonyObjectKind::Pebble => ShelterDecorationKind::Stone,
        };
        scores[decoration.index()] = scores[decoration.index()].saturating_add(128);
    }

    let streams = SeedStream::new(save.colony_seed);
    let mut rng = streams.rng(
        "shelter-decoration-choice",
        u64::from(save.home.decorations.ordinal),
    );
    ShelterDecorationKind::ALL
        .into_iter()
        .filter(|kind| !save.home.decorations.decorations.contains(kind))
        .map(|kind| (scores[kind.index()], rng.random::<u16>(), kind))
        .max()
        .map(|(_, _, kind)| kind)
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

    fn two_creature_world(seed: [u8; 32], created: OffsetDateTime) -> World {
        let desktop = desktop();
        let mut world = World::new(seed, created, &desktop);
        let now = created + Duration::hours(1);
        world.tick(now, 0.05, &desktop);
        let_colony_wander(&mut world, now);
        world.pending_home_greetings.clear();
        assert_eq!(world.save.creatures.len(), 2);
        for (index, creature) in world.save.creatures.iter_mut().enumerate() {
            creature.state.position = Point {
                x: 500.0 + index as f32 * 60.0,
                y: 846.0,
            };
            creature.state.surface = SurfaceAttachment {
                kind: SurfaceKind::ScreenFloor,
                monitor_id: 1,
                window_key: None,
                relative_x: 0.5,
            };
            creature.state.action = ActionKind::Idle;
            creature.state.action_elapsed = 0.0;
            creature.state.action_duration = 100.0;
        }
        world
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
    fn narrow_gap_squeeze_is_runtime_only_and_cancels_on_geometry_change() {
        let created = datetime!(2026-01-01 0:00 UTC);
        let mut desktop = desktop();
        desktop.windows = vec![
            DesktopWindow {
                key: 301,
                bounds: DesktopRect {
                    x: 100.0,
                    y: 300.0,
                    width: 240.0,
                    height: 220.0,
                },
                z_order: 0,
                visible: true,
                minimized: false,
                application: None,
                application_name: None,
            },
            DesktopWindow {
                key: 302,
                bounds: DesktopRect {
                    x: 360.0,
                    y: 310.0,
                    width: 240.0,
                    height: 220.0,
                },
                z_order: 1,
                visible: true,
                minimized: false,
                application: None,
                application_name: None,
            },
        ];
        let mut world = World::new([103; 32], created, &desktop);
        let_colony_wander(&mut world, created);
        world.save.creatures[0].state.position = Point { x: 320.0, y: 300.0 };
        world.save.creatures[0].state.surface = SurfaceAttachment {
            kind: SurfaceKind::WindowLedge,
            monitor_id: 1,
            window_key: Some(301),
            relative_x: 0.9,
        };
        world.tick(created, 0.05, &desktop);

        let route = world.topology.plan_route(301, RoutePreferences::default());
        assert_eq!(route.len(), 1);
        assert_eq!(route[0].kind, RouteHopKind::NarrowGap);
        let creature_id = world.save.creatures[0].id;
        let journey = build_route_hop_journey(&world.save.creatures[0], route[0], &desktop);
        world.save.creatures[0].state.action = journey.initial_action();
        world.save.creatures[0].state.action_duration = f32::MAX;
        world.window_journeys.insert(creature_id, journey);
        world.window_routes.insert(
            creature_id,
            WindowRoutePlan {
                geometry_hash: world.topology.geometry_hash(),
                remaining: VecDeque::new(),
            },
        );
        world.tick(created, 0.1, &desktop);
        assert_eq!(
            world.save.creatures[0].state.action,
            ActionKind::SqueezeWindow
        );
        assert!(world.window_routes.contains_key(&creature_id));

        desktop.windows[1].bounds.x += 1.0;
        world.tick(created, 0.05, &desktop);
        assert!(!world.window_routes.contains_key(&creature_id));
        assert!(!world.window_journeys.contains_key(&creature_id));
        assert!(matches!(
            world.save.creatures[0].state.action,
            ActionKind::ReactToWindow | ActionKind::RideWindow
        ));
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
            &{
                let mut topology = DesktopTopology::default();
                topology.rebuild_if_changed(&desktop, &BTreeMap::new());
                topology
            },
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
        world.save.ritual.next_at_utc = now + Duration::hours(12);
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

        let mut topology = DesktopTopology::default();
        topology.rebuild_if_changed(&desktop, &BTreeMap::new());
        let (target, surface) =
            find_nearby_ledge(creature, &desktop, &world.save.settings.habitat, &topology)
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
    fn four_creatures_have_exactly_six_canonical_bond_records() {
        let created = datetime!(2026-01-01 0:00 UTC);
        let desktop = desktop();
        let mut world = World::new([54; 32], created, &desktop);
        world.tick(created + Duration::days(31), 0.05, &desktop);

        assert_eq!(world.save.creatures.len(), 4);
        assert_eq!(world.save.relationships.len(), MAX_RELATIONSHIPS);
        let pairs: BTreeSet<_> = world
            .save
            .relationships
            .iter()
            .map(|relationship| (relationship.a, relationship.b))
            .collect();
        assert_eq!(pairs.len(), MAX_RELATIONSHIPS);
        assert!(pairs.iter().all(|(a, b)| a < b));
        for creature in &world.save.creatures {
            assert_eq!(
                pairs
                    .iter()
                    .filter(|(a, b)| *a == creature.id || *b == creature.id)
                    .count(),
                3
            );
        }
    }

    #[test]
    fn five_calm_minutes_project_into_one_compact_bond_update() {
        let created = datetime!(2026-01-01 0:00 UTC);
        let desktop = desktop();
        let mut world = two_creature_world([55; 32], created);
        let a = world.save.creatures[0].id;
        let b = world.save.creatures[1].id;
        let relationship = relationship_mut_or_insert(&mut world.save.relationships, a, b).unwrap();
        relationship.avoidance = 3;
        let before = *relationship;

        for _ in 0..5 {
            world.sample_observations(OBSERVATION_INTERVAL_SECS, &desktop);
            world.project_events(created + Duration::hours(1));
        }

        let after = *relationship_between(&world.save.relationships, a, b).unwrap();
        assert_eq!(after.familiarity, before.familiarity + 1);
        assert_eq!(after.avoidance, before.avoidance - 1);
        assert!(world.drain_events().any(|event| matches!(
            event,
            WorldEvent::BondInteraction {
                experience: RelationshipExperience::CalmProximity,
                ..
            }
        )));
        assert_eq!(
            world.calm_proximity_seconds.get(&(a.min(b), a.max(b))),
            Some(&0)
        );
    }

    #[test]
    fn every_targeted_sequence_handles_moved_and_unavailable_companions() {
        let created = datetime!(2026-01-01 0:00 UTC);
        let world = two_creature_world([56; 32], created);
        let actor = world.save.creatures[0].clone();
        let mut target = world.save.creatures[1].clone();
        let target_id = target.id;
        let social_actions = [
            ActionKind::Follow,
            ActionKind::Sleep,
            ActionKind::PresentDiscovery,
            ActionKind::SocialPlay,
            ActionKind::Greet,
            ActionKind::InspectScreen,
            ActionKind::ReactToWindow,
        ];

        target.state.position = Point { x: 710.0, y: 846.0 };
        let moved = vec![actor.clone(), target.clone()];
        assert_eq!(
            bond_target_point(&actor, &moved, target_id, ActionKind::Greet),
            Some(target.state.position)
        );

        target.state.action = ActionKind::Sleep;
        let sleeping = vec![actor.clone(), target.clone()];
        assert!(bond_target_point(&actor, &sleeping, target_id, ActionKind::Sleep).is_some());
        for action in [
            ActionKind::Follow,
            ActionKind::PresentDiscovery,
            ActionKind::SocialPlay,
            ActionKind::Greet,
            ActionKind::InspectScreen,
        ] {
            assert_eq!(
                bond_target_point(&actor, &sleeping, target_id, action),
                None
            );
        }

        target.state.action = ActionKind::Homebound;
        let homebound = vec![actor.clone(), target.clone()];
        for action in social_actions {
            assert_eq!(
                bond_target_point(&actor, &homebound, target_id, action),
                None
            );
        }

        target.state.action = ActionKind::Tossed;
        let tossed = vec![actor.clone(), target.clone()];
        assert!(bond_target_point(&actor, &tossed, target_id, ActionKind::ReactToWindow).is_some());
        for action in social_actions
            .into_iter()
            .filter(|action| *action != ActionKind::ReactToWindow)
        {
            assert_eq!(bond_target_point(&actor, &tossed, target_id, action), None);
        }

        target.state.action = ActionKind::ClimbWindow;
        let climbing = vec![actor.clone(), target.clone()];
        assert_eq!(
            bond_target_point(&actor, &climbing, target_id, ActionKind::InspectScreen),
            Some(target.state.position)
        );

        let removed = vec![actor.clone()];
        for action in social_actions {
            assert_eq!(bond_target_point(&actor, &removed, target_id, action), None);
            assert_eq!(bond_target_point(&actor, &removed, u64::MAX, action), None);
        }
    }

    #[test]
    fn a_bond_plan_tracks_motion_then_cancels_if_the_target_cannot_participate() {
        let created = datetime!(2026-01-01 0:00 UTC);
        let desktop = desktop();
        let mut moving = two_creature_world([57; 32], created);
        let actor = moving.save.creatures[0].id;
        let target = moving.save.creatures[1].id;
        moving.save.creatures[1].state.position = Point { x: 820.0, y: 846.0 };
        moving.save.creatures[0].state.action = ActionKind::Follow;
        moving.action_choices.insert(
            actor,
            ActionChoice {
                action: ActionKind::Follow,
                target_creature: Some(target),
                target_point: Some(Point { x: 600.0, y: 846.0 }),
            },
        );
        moving.bond_plans.insert(
            actor,
            BondPlan {
                target,
                final_action: ActionKind::Greet,
                experience: RelationshipExperience::Greeting,
                approaching: true,
            },
        );
        moving.tick(created + Duration::hours(1), 0.05, &desktop);
        assert_eq!(
            moving.action_choices[&actor].target_point,
            Some(Point { x: 820.0, y: 846.0 })
        );

        moving.save.creatures[1].state.action = ActionKind::Sleep;
        moving.tick(created + Duration::hours(1), 0.05, &desktop);
        assert_eq!(moving.save.creatures[0].state.action, ActionKind::Idle);
        assert!(!moving.action_choices.contains_key(&actor));
        assert!(!moving.bond_plans.contains_key(&actor));

        let mut removed = two_creature_world([58; 32], created);
        let actor = removed.save.creatures[0].id;
        let target = removed.save.creatures[1].id;
        removed.save.creatures[0].state.action = ActionKind::Greet;
        removed.action_choices.insert(
            actor,
            ActionChoice {
                action: ActionKind::Greet,
                target_creature: Some(target),
                target_point: Some(removed.save.creatures[1].state.position),
            },
        );
        removed.bond_plans.insert(
            actor,
            BondPlan {
                target,
                final_action: ActionKind::Greet,
                experience: RelationshipExperience::Greeting,
                approaching: false,
            },
        );
        removed.save.creatures.remove(1);
        removed.tick(created + Duration::hours(1), 0.05, &desktop);
        assert_eq!(removed.save.creatures[0].state.action, ActionKind::Idle);
        assert!(!removed.action_choices.contains_key(&actor));
        assert!(!removed.bond_plans.contains_key(&actor));
    }

    #[test]
    fn follow_then_greet_updates_the_pair_only_after_the_sequence_completes() {
        let created = datetime!(2026-01-01 0:00 UTC);
        let desktop = desktop();
        let mut world = two_creature_world([59; 32], created);
        let actor = world.save.creatures[0].id;
        let target = world.save.creatures[1].id;
        world.save.creatures[0].state.position = Point { x: 300.0, y: 846.0 };
        world.save.creatures[1].state.position = Point { x: 600.0, y: 846.0 };
        let before = *relationship_between(&world.save.relationships, actor, target).unwrap();
        world.save.creatures[0].state.action = ActionKind::Follow;
        world.save.creatures[0].state.action_elapsed = 1.0;
        world.save.creatures[0].state.action_duration = 1.0;
        world.action_choices.insert(
            actor,
            ActionChoice {
                action: ActionKind::Follow,
                target_creature: Some(target),
                target_point: Some(world.save.creatures[1].state.position),
            },
        );
        world.bond_plans.insert(
            actor,
            BondPlan {
                target,
                final_action: ActionKind::Greet,
                experience: RelationshipExperience::Greeting,
                approaching: true,
            },
        );

        world.tick(created + Duration::hours(1), 0.01, &desktop);
        assert_eq!(world.save.creatures[0].state.action, ActionKind::Greet);
        assert!(!world.bond_plans[&actor].approaching);
        assert_eq!(
            relationship_between(&world.save.relationships, actor, target),
            Some(&before)
        );

        let duration = world.save.creatures[0].state.action_duration;
        world.save.creatures[0].state.action_elapsed = duration;
        world.tick(created + Duration::hours(1), 0.01, &desktop);
        let after = relationship_between(&world.save.relationships, actor, target).unwrap();
        assert_eq!(after.affinity, before.affinity.saturating_add(2));
        assert_eq!(after.familiarity, before.familiarity.saturating_add(1));
        assert!(!world.bond_plans.contains_key(&actor));
    }

    #[test]
    fn a_tossed_preferred_companion_prompts_a_concerned_reaction() {
        let created = datetime!(2026-01-01 0:00 UTC);
        let desktop = desktop();
        let mut world = two_creature_world([60; 32], created);
        let actor = world.save.creatures[0].id;
        let target = world.save.creatures[1].id;
        let before = *relationship_between(&world.save.relationships, actor, target).unwrap();
        world.save.creatures[0].state.action = ActionKind::Sleep;
        world.save.creatures[0].state.action_elapsed = 30.0;
        world.save.creatures[0].state.action_duration = 300.0;
        world.save.creatures[1].state.action = ActionKind::Tossed;
        world.save.creatures[1].state.action_duration = 100.0;

        world.tick(created + Duration::hours(1), 0.01, &desktop);
        assert_eq!(
            world.save.creatures[0].state.action,
            ActionKind::ReactToWindow
        );
        assert_eq!(
            world.bond_plans[&actor].experience,
            RelationshipExperience::ConcernedAfterToss
        );
        assert!(world.drain_events().any(|event| matches!(
            event,
            WorldEvent::SleepInterrupted { creature_id, .. } if creature_id == actor
        )));

        world.save.creatures[1].state.action = ActionKind::Landing;
        let duration = world.save.creatures[0].state.action_duration;
        world.save.creatures[0].state.action_elapsed = duration;
        world.tick(created + Duration::hours(1), 0.01, &desktop);
        let after = relationship_between(&world.save.relationships, actor, target).unwrap();
        assert_eq!(after.affinity, before.affinity.saturating_add(2));
        assert_eq!(after.familiarity, before.familiarity.saturating_add(1));
    }

    #[test]
    fn shelter_return_reuses_greeting_actions_and_adds_no_body_clip() {
        let created = datetime!(2026-01-01 0:00 UTC);
        let desktop = desktop();
        let mut world = two_creature_world([62; 32], created);
        let now = created + Duration::hours(1);
        world.save.home.active_since_utc = Some(now);
        for creature in &mut world.save.creatures {
            creature.state.action = ActionKind::Homebound;
        }
        world.dismiss_home(now, false);
        for creature in &mut world.save.creatures {
            creature.state.action_elapsed = creature.state.action_duration;
        }

        world.tick(now, 0.01, &desktop);
        assert!(world.bond_plans.values().any(|plan| {
            plan.final_action == ActionKind::Greet
                && plan.experience == RelationshipExperience::HomecomingGreeting
        }));
        assert!(world.save.creatures.iter().any(|creature| {
            matches!(
                creature.state.action,
                ActionKind::Greet | ActionKind::Follow
            )
        }));
        assert_eq!(ActionKind::ALL.len(), 25);
        assert_eq!(ActionKind::BODY_CLIPS.len(), 22);
    }

    #[test]
    fn discovery_watch_steal_and_squabble_schedule_existing_targeted_actions() {
        let created = datetime!(2026-01-01 0:00 UTC);
        let desktop = desktop();
        let now = created + Duration::hours(1);

        let mut gift = two_creature_world([63; 32], created);
        let gift_actor = gift.save.creatures[0].id;
        let gift_target = gift.save.creatures[1].id;
        let relationship =
            relationship_mut_or_insert(&mut gift.save.relationships, gift_actor, gift_target)
                .unwrap();
        relationship.affinity = 200;
        relationship.familiarity = 200;
        relationship.avoidance = 0;
        gift.discovery_remaining = 0.0;
        gift.save.creatures[0].state.action_elapsed = 1.0;
        gift.save.creatures[0].state.action_duration = 1.0;
        gift.tick(now, 0.01, &desktop);
        let gift_plan = gift.bond_plans[&gift_actor];
        assert_eq!(gift_plan.target, gift_target);
        assert_eq!(gift_plan.final_action, ActionKind::PresentDiscovery);
        assert_eq!(
            gift_plan.experience,
            RelationshipExperience::BroughtDiscovery
        );

        for (seed, target_action, scores, expected, expected_action, attempts) in [
            (
                64,
                ActionKind::ClimbWindow,
                (100, 100, 0, 0),
                RelationshipExperience::WatchedClimb,
                ActionKind::InspectScreen,
                64,
            ),
            (
                65,
                ActionKind::SoloPlay,
                (100, 100, 200, 0),
                RelationshipExperience::StoleToy,
                ActionKind::SocialPlay,
                64,
            ),
            (
                67,
                ActionKind::Idle,
                (20, 100, 100, 120),
                RelationshipExperience::Squabble,
                ActionKind::SocialPlay,
                1_024,
            ),
        ] {
            let mut world = two_creature_world([seed; 32], created);
            let actor = world.save.creatures[0].id;
            let target = world.save.creatures[1].id;
            let relationship =
                relationship_mut_or_insert(&mut world.save.relationships, actor, target).unwrap();
            relationship.affinity = scores.0;
            relationship.familiarity = scores.1;
            relationship.playfulness = scores.2;
            relationship.avoidance = scores.3;
            world.discovery_remaining = f32::MAX;

            let mut scheduled = None;
            for _ in 0..attempts {
                world.action_choices.clear();
                world.bond_plans.clear();
                world.save.creatures[0].state.action = ActionKind::Idle;
                world.save.creatures[0].state.action_elapsed = 1.0;
                world.save.creatures[0].state.action_duration = 1.0;
                world.save.creatures[1].state.action = target_action;
                world.save.creatures[1].state.action_elapsed = 0.0;
                world.save.creatures[1].state.action_duration = 100.0;
                world.tick(now, 0.01, &desktop);
                scheduled = world
                    .bond_plans
                    .get(&actor)
                    .copied()
                    .filter(|plan| plan.experience == expected);
                if scheduled.is_some() {
                    break;
                }
            }
            let plan = scheduled.expect("seeded scheduler should eventually choose the rare bond");
            assert_eq!(plan.target, target);
            assert_eq!(plan.final_action, expected_action);
            assert_eq!(world.action_choices[&actor].target_creature, Some(target));
        }
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
            bond: None,
            on_window_ledge: false,
            reachable_window_ledge: false,
            window_changed_nearby: false,
            objects: ObjectUtility::default(),
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

    #[test]
    fn ritual_schedule_is_deterministic_and_stays_between_twelve_and_forty_eight_hours() {
        let now = datetime!(2026-01-01 0:00 UTC);
        for ordinal in 0..64 {
            let first = scheduled_ritual_at([91; 32], ordinal, now);
            let second = scheduled_ritual_at([91; 32], ordinal, now);
            assert_eq!(first, second);
            assert!(first - now >= Duration::hours(12));
            assert!(first - now <= Duration::hours(48));
        }
    }

    #[test]
    fn every_ritual_uses_a_bounded_runtime_plan_and_existing_actions() {
        use std::collections::HashSet;

        let created = datetime!(2026-01-01 12:00 UTC);
        let mut now = created + Duration::days(2);
        while !(local_time_or_utc(now).hour() >= 22 || local_time_or_utc(now).hour() < 5) {
            now += Duration::hours(1);
        }
        let mut quiet_desktop = desktop();
        quiet_desktop.idle_duration = std::time::Duration::from_secs(20 * 60);
        let mut seen = HashSet::new();
        for ordinal in 0..512 {
            let mut world = two_creature_world([92; 32], created);
            world.save.ritual.ordinal = ordinal;
            assert!(world.try_start_colony_plan(now, &quiet_desktop));
            let plan = world.colony_plan.as_ref().expect("ritual plan starts");
            assert!(plan.participants.len() <= 4);
            if plan.kind == RitualKind::Catch {
                assert_eq!(plan.participants.len(), 2);
            }
            assert!(
                plan.participants
                    .iter()
                    .all(|participant| ActionKind::ALL.contains(&participant.ceremony_action))
            );
            seen.insert(plan.kind);
            if seen.len() == RitualKind::ALL.len() - 1 {
                break;
            }
        }
        for expected in RitualKind::ALL {
            if expected != RitualKind::HatchDay {
                assert!(seen.contains(&expected), "did not schedule {expected:?}");
            }
        }
    }

    #[test]
    fn reduced_motion_excludes_races_and_interruption_reschedules_without_replay() {
        let created = datetime!(2026-01-01 12:00 UTC);
        let now = created + Duration::days(2);
        let mut desktop = desktop();
        desktop.idle_duration = std::time::Duration::from_secs(20 * 60);
        for ordinal in 0..128 {
            let mut world = two_creature_world([93; 32], created);
            world.save.settings.reduce_motion = true;
            world.save.ritual.ordinal = ordinal;
            assert!(world.try_start_colony_plan(now, &desktop));
            assert_ne!(
                world.colony_plan.as_ref().unwrap().kind,
                RitualKind::FloorRace
            );
        }

        let mut world = two_creature_world([94; 32], created);
        assert!(world.try_start_colony_plan(now, &desktop));
        let kind = world.colony_plan.as_ref().unwrap().kind;
        let ordinal = world.save.ritual.ordinal;
        world.interrupt_colony_plan(now);
        assert!(world.colony_plan.is_none());
        assert_eq!(world.save.ritual.ordinal, ordinal);
        assert!(world.save.ritual.next_at_utc - now >= Duration::hours(2));
        assert!(world.save.ritual.next_at_utc - now <= Duration::hours(6));
        assert!(world.drain_events().any(|event| matches!(
            event,
            WorldEvent::RitualInterrupted { kind: interrupted } if interrupted == kind
        )));
    }

    #[test]
    fn overdue_downtime_runs_at_most_one_ritual_and_schedules_from_now() {
        let created = datetime!(2026-01-01 12:00 UTC);
        let now = created + Duration::days(10);
        let desktop = desktop();
        let mut world = two_creature_world([95; 32], created);
        world.save.home.last_disappeared_utc = Some(now);
        world.save.ritual.next_at_utc = created + Duration::hours(12);
        for creature in &mut world.save.creatures {
            creature.state.action_duration = 0.0;
        }
        world.tick(now, 0.05, &desktop);
        assert_eq!(world.save.ritual.ordinal, 1);
        assert!(world.colony_plan.is_some());
        assert!(world.save.ritual.next_at_utc >= now + Duration::hours(12));
        world.advance_colony_plan(now, RITUAL_APPROACH_SECS + 0.1, &desktop);
        world.advance_colony_plan(now, 60.0, &desktop);
        assert!(world.colony_plan.is_none());
        world.tick(now, 0.05, &desktop);
        assert_eq!(
            world.save.ritual.ordinal, 1,
            "missed rituals must not replay"
        );
    }

    #[test]
    fn hatch_day_is_local_deduplicated_and_reduced_motion_safe() {
        let created = datetime!(2025-06-15 16:00 UTC);
        let now = datetime!(2026-06-15 16:00 UTC);
        let desktop = desktop();
        let mut world = two_creature_world([96; 32], created);
        world.save.settings.reduce_motion = true;
        assert!(world.try_start_colony_plan(now, &desktop));
        assert_eq!(
            world.colony_plan.as_ref().unwrap().kind,
            RitualKind::HatchDay
        );
        assert_eq!(
            world.save.ritual.hatch_day_acknowledged_year,
            Some(local_time_or_utc(now).year())
        );
        world.interrupt_colony_plan(now);
        assert!(
            !world
                .eligible_ritual_kinds(now, &desktop, true)
                .contains(&RitualKind::HatchDay)
        );
    }

    #[test]
    fn stale_monitor_ids_rebind_all_arrived_creatures_instead_of_hiding_them() {
        let created = datetime!(2026-01-01 0:00 UTC);
        let desktop = desktop();
        let mut world = two_creature_world([97; 32], created);
        for creature in &mut world.save.creatures {
            creature.state.surface.monitor_id = u64::MAX;
        }
        keep_creatures_in_habitat(
            &mut world.save.creatures,
            &desktop,
            &world.save.settings.habitat,
            &[],
        );
        assert_eq!(world.save.creatures.len(), 2);
        assert!(
            world
                .save
                .creatures
                .iter()
                .all(|creature| creature.state.surface.monitor_id == desktop.monitors[0].id)
        );
    }

    #[test]
    fn rituals_cancel_safely_when_hidden_paused_dragged_or_geometry_changes() {
        let created = datetime!(2026-01-01 12:00 UTC);
        let now = created + Duration::days(2);
        let desktop = desktop();

        for pause_instead_of_hide in [false, true] {
            let mut world = two_creature_world([98; 32], created);
            assert!(world.try_start_colony_plan(now, &desktop));
            if pause_instead_of_hide {
                world.save.settings.paused = true;
            } else {
                world.save.settings.visible = false;
            }
            world.tick(now, 0.05, &desktop);
            assert!(world.colony_plan.is_none());
        }

        let mut changed = two_creature_world([99; 32], created);
        assert!(changed.try_start_colony_plan(now, &desktop));
        let mut changed_desktop = desktop.clone();
        changed_desktop.monitors[0].usable_bounds.width -= 40.0;
        changed.tick(now, 0.05, &changed_desktop);
        assert!(changed.colony_plan.is_none());

        let mut dragged = two_creature_world([100; 32], created);
        assert!(dragged.try_start_colony_plan(now, &desktop));
        let creature_id = dragged.save.creatures[0].id;
        let cursor = dragged.save.creatures[0].state.position;
        assert!(dragged.handle_command(
            WorldCommand::BeginInteraction {
                creature_id,
                cursor,
            },
            &desktop,
        ));
        assert!(dragged.handle_command(
            WorldCommand::UpdateInteraction {
                cursor: Point {
                    x: cursor.x + DRAG_THRESHOLD + 1.0,
                    y: cursor.y,
                },
                velocity: Point::default(),
            },
            &desktop,
        ));
        assert!(dragged.colony_plan.is_none());
        assert!(dragged.is_dragging());
    }

    #[test]
    fn colony_object_schedule_is_deterministic_and_between_three_and_seven_days() {
        let now = datetime!(2026-01-01 0:00 UTC);
        for ordinal in 0..64 {
            let first = scheduled_colony_object_at([101; 32], ordinal, now);
            let second = scheduled_colony_object_at([101; 32], ordinal, now);
            assert_eq!(first, second);
            assert!(first - now >= Duration::days(3));
            assert!(first - now <= Duration::days(7));
        }
    }

    #[test]
    fn overdue_colony_objects_add_one_without_a_catch_up_flood() {
        let created = datetime!(2026-01-01 0:00 UTC);
        let now = created + Duration::days(40);
        let desktop = desktop();
        let mut world = World::new([102; 32], created, &desktop);
        world.save.objects.next_at_utc = created + Duration::days(3);
        world.tick(now, 0.05, &desktop);
        assert_eq!(world.save.objects.objects.len(), 1);
        assert_eq!(world.save.objects.ordinal, 1);
        assert!(world.save.objects.next_at_utc >= now + Duration::days(3));
        assert!(world.save.objects.next_at_utc <= now + Duration::days(7));
        world.tick(now, 0.05, &desktop);
        assert_eq!(world.save.objects.objects.len(), 1);
        assert_eq!(
            world
                .drain_events()
                .filter(|event| matches!(event, WorldEvent::ColonyObjectAdded { .. }))
                .count(),
            1
        );
    }

    #[test]
    fn colony_objects_cap_at_eight_and_invalid_positions_recover() {
        let created = datetime!(2026-01-01 0:00 UTC);
        let desktop = desktop();
        let mut world = World::new([103; 32], created, &desktop);
        for index in 0..MAX_COLONY_OBJECTS + 3 {
            world.save.objects.next_at_utc = created;
            world.tick(created + Duration::days(index as i64 + 1), 0.05, &desktop);
        }
        assert_eq!(world.save.objects.objects.len(), MAX_COLONY_OBJECTS);

        let object = &mut world.save.objects.objects[0];
        object.display = DisplayKey([255; 16]);
        object.normalized_position = Point { x: -50.0, y: 50.0 };
        world.reconcile_colony_objects(&desktop);
        let object = &world.save.objects.objects[0];
        assert_eq!(object.display, desktop.monitors[0].display_key);
        assert!((0.0..=1.0).contains(&object.normalized_position.x));
        assert!((0.0..=1.0).contains(&object.normalized_position.y));
        assert!(
            resolved_colony_object_position(
                object,
                &desktop.monitors,
                &world.save.settings.habitat
            )
            .is_some()
        );
    }

    #[test]
    fn shelter_decoration_schedule_is_deterministic_and_between_four_and_nine_days() {
        let now = datetime!(2026-01-01 0:00 UTC);
        for ordinal in 0..64 {
            let first = scheduled_shelter_decoration_at([104; 32], ordinal, now);
            let second = scheduled_shelter_decoration_at([104; 32], ordinal, now);
            assert_eq!(first, second);
            assert!(first - now >= Duration::days(4));
            assert!(first - now <= Duration::days(9));
        }
    }

    #[test]
    fn shelter_decorations_add_one_after_downtime_and_cap_at_six_unique_kinds() {
        let created = datetime!(2026-01-01 0:00 UTC);
        let desktop = desktop();
        let mut world = World::new([105; 32], created, &desktop);
        let overdue = created + Duration::days(40);
        world.save.home.decorations.next_at_utc = created + Duration::days(4);
        world.tick(overdue, 0.05, &desktop);
        assert_eq!(world.save.home.decorations.decorations.len(), 1);
        assert!(world.save.home.decorations.next_at_utc >= overdue + Duration::days(4));
        assert!(world.save.home.decorations.next_at_utc <= overdue + Duration::days(9));
        world.tick(overdue, 0.05, &desktop);
        assert_eq!(world.save.home.decorations.decorations.len(), 1);
        assert_eq!(
            world
                .drain_events()
                .filter(|event| matches!(event, WorldEvent::ShelterDecorationAdded { .. }))
                .count(),
            1
        );

        for day in 1..=MAX_SHELTER_DECORATIONS + 3 {
            world.save.home.decorations.next_at_utc = overdue;
            world.tick(overdue + Duration::days(day as i64), 0.05, &desktop);
        }
        let decorations = &world.save.home.decorations.decorations;
        assert_eq!(decorations.len(), MAX_SHELTER_DECORATIONS);
        assert_eq!(
            decorations.iter().copied().collect::<BTreeSet<_>>().len(),
            decorations.len()
        );
    }

    #[test]
    fn shelter_decoration_choice_reflects_memories_bonds_rituals_and_objects() {
        let created = datetime!(2026-01-01 0:00 UTC);
        let desktop = desktop();

        let mut memories = World::new([106; 32], created, &desktop);
        memories.save.creatures[0].memory.ledge_seconds = u32::MAX;
        assert_eq!(
            preferred_shelter_decoration(&memories.save),
            Some(ShelterDecorationKind::Leaf)
        );

        let mut bonds = two_creature_world([107; 32], created);
        bonds.save.relationships[0].affinity = u8::MAX;
        bonds.save.relationships[0].familiarity = u8::MAX;
        assert_eq!(
            preferred_shelter_decoration(&bonds.save),
            Some(ShelterDecorationKind::Banner)
        );

        let mut ritual = World::new([108; 32], created, &desktop);
        ritual.save.ritual.last_kind = Some(RitualKind::Picnic);
        assert_eq!(
            preferred_shelter_decoration(&ritual.save),
            Some(ShelterDecorationKind::Flower)
        );

        let mut objects = World::new([109; 32], created, &desktop);
        objects.save.objects.objects.push(ColonyObject {
            id: 1,
            kind: ColonyObjectKind::Pebble,
            display: desktop.monitors[0].display_key,
            normalized_position: Point { x: 0.5, y: 0.9 },
            role: ColonyObjectRole::Curiosity,
        });
        assert_eq!(
            preferred_shelter_decoration(&objects.save),
            Some(ShelterDecorationKind::Stone)
        );
    }
}
