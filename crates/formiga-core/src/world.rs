use crate::behavior::{BehaviorContext, choose_action};
use crate::rng::SeedStream;
use crate::tuning::{FINDS, ROAMING};
use crate::*;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha12Rng;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use time::{Duration, OffsetDateTime, UtcOffset};

mod antics;
mod arrivals;
mod attention;
mod battle;
mod beats;
mod bonds;
mod bubbles;
mod colony;
mod discovery;
mod echo_fox;
mod experience;
mod generation;
mod habits;
mod home;
mod house_visits;
mod interaction;
mod journeys;
mod moments;
mod movement;
mod new_looks;
mod objects;
mod offers;
mod presence;
mod rides;
mod rituals;
mod routine;
mod spacing;
mod step;
mod surfaces;
mod tows;
mod trips;
mod undo;
mod village_life;
mod visitors;
mod wonders;
use attention::{AttentionRuntime, DisplayAttention};
pub(crate) use bonds::normalize_relationships;
use bonds::*;
pub use bubbles::{BubbleGrowth, ThoughtBubble};
pub(crate) use colony::normalize_colony_roles;
use colony::*;
use generation::*;
use habits::*;
use interaction::*;
use journeys::*;
use movement::*;
use objects::*;
pub(crate) use objects::{scheduled_colony_object_at, scheduled_village_unlock_at};
pub(crate) use rituals::scheduled_ritual_at;
use rituals::*;
use surfaces::SurfaceMemory;
pub use undo::{ColonyEdit, UndoError};
pub use wonders::{WonderPose, wonder_length, wonder_motion, wonder_poses};

/// Width of a creature's art frame, matching `formiga_art::FRAME_SIZE`. The simulation crate
/// cannot depend on the art crate, so shelter layout mirrors the constant the way `home_anchor`
/// already mirrors the shelter's own half-width.
const CREATURE_ART_WIDTH: f32 = crate::CREATURE_FRAME_WIDTH;
/// How long a companion stays down on the floor before it thinks of climbing again.
fn climb_rest_secs(leaning: RoamingLeaning) -> std::ops::Range<f32> {
    match leaning {
        RoamingLeaning::Climber => ROAMING.climber_rest_secs,
        RoamingLeaning::Anywhere => ROAMING.anywhere_rest_secs,
        RoamingLeaning::Homebody => ROAMING.homebody_rest_secs,
        RoamingLeaning::FloorDweller => ROAMING.floor_dweller_rest_secs,
    }
}

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
    /// Where each resident is strolling to while the colony is home, and its own seeded stream
    /// for choosing. Runtime-only: a relaunch simply sends everybody wandering again.
    home_roam: BTreeMap<CreatureId, home::RoamStep>,
    home_roam_rng: ChaCha12Rng,
    /// Whether a moment becomes a habit, and whether a habit is done this time. A stream of its
    /// own, so habits never shift any other choice the colony makes.
    habit_rng: ChaCha12Rng,
    /// A picnic, dance or nap the village was asked to share and is sharing now.
    village_moment: Option<moments::VillageMomentPlan>,
    /// Who joins a moment the village is asked to share. A stream of its own, like the habits'.
    moment_rng: ChaCha12Rng,
    /// The changes that can still be taken back, oldest first. Never saved: they last as long as
    /// the app runs.
    undo_history: VecDeque<undo::UndoPoint>,
    /// When the last tick was, by the clock, and the seconds with the houses out since the day
    /// book was last written to. Runtime only.
    day_clock: Option<OffsetDateTime>,
    home_seconds_pending: f32,
    /// Sleepers being towed out of somebody's way by a friend on a little rope.
    tows: tows::TowTable,
    /// When the colony next yawns, and the yawns and looks still waiting to start.
    beats: beats::Beats,
    /// The little moments a temperament makes of what goes on: who is waiting to react to what,
    /// who is having a while to itself, and when the next prank is due. Runtime only.
    antics: antics::Antics,
    /// What residents are doing about the village beyond strolling and the quiet moments at their
    /// doors: tending the gardens, seeing to their houses, indoors, up on a roof, or in the middle
    /// of a small mishap. Runtime only, like the moments.
    village_life: BTreeMap<CreatureId, village_life::VillageActivity>,
    /// Which of those each resident does, and whether it turns something up. A stream of its own,
    /// like the moments'.
    village_life_rng: ChaCha12Rng,
    colony_plan: Option<ColonyPlan>,
    /// A wonder out somewhere for a companion or two to play on, and when the next is due.
    /// Runtime only: the notebook's page of them is all a save keeps.
    wonders: wonders::Wonders,
    /// Who is away in a house open in Formiga Home: indoors, out of every plan, and left alone
    /// until the house closes. Runtime only: a visit never outlives the app.
    house_visit: BTreeSet<CreatureId>,
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
    /// Which generator new companions come from: always the latest in the app. Only tests ask for
    /// an older one, to keep a scene that was tuned on the companions an older generator made.
    generator: Edition,
    /// Whether Echo Fox Mode is on. Runtime only: a relaunch brings everybody back as themselves.
    echo_fox: bool,
}

#[derive(Clone, Copy)]
struct AmbientTimers {
    inspect_remaining: f32,
    dangle_remaining: f32,
    /// Seconds before a companion that has just come down off a ledge thinks of climbing again:
    /// a spell pottering about the floor between one climb and the next.
    climb_rest: f32,
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

/// `now` by the desk's own clock, or by UTC where the system will not say its offset.
///
/// The crate's own tests read UTC as the local clock wherever they run, so a scenario that ticks
/// through a given hour plays out the same on a laptop in New York as on CI.
fn local_time_or_utc(now: OffsetDateTime) -> OffsetDateTime {
    #[cfg(not(test))]
    let offset = UtcOffset::local_offset_at(now).unwrap_or(UtcOffset::UTC);
    #[cfg(test)]
    let offset = UtcOffset::UTC;
    now.to_offset(offset)
}

/// Whether an hour of the local clock is at night, when a creature that keeps to the clock is
/// drawn to sleep.
fn is_night(local_hour: u8) -> bool {
    !(6..22).contains(&local_hour)
}

/// The local day `now` falls on, as a Julian day number: the day the day book counts it under.
fn local_day(now: OffsetDateTime) -> i32 {
    local_time_or_utc(now).date().to_julian_day()
}

impl World {
    /// Counts the time the houses are out in the day book, by the clock rather than by ticks, a
    /// few seconds at a time. A gap of more than a few seconds — the machine asleep, the app
    /// stopped — is not time at home, and is not counted.
    fn count_home_time(&mut self, now: OffsetDateTime) {
        const LONGEST_STEP: f32 = 5.0;
        const WRITE_EVERY: f32 = 10.0;
        let step = self.day_clock.map_or(0.0, |last| {
            (now - last).as_seconds_f32().clamp(0.0, LONGEST_STEP)
        });
        self.day_clock = Some(now);
        if !self.save.home.is_active() {
            return;
        }
        self.home_seconds_pending += step;
        if self.home_seconds_pending >= WRITE_EVERY {
            let whole = self.home_seconds_pending.floor();
            self.home_seconds_pending -= whole;
            self.save.day_book.count_home(local_day(now), whole as u32);
        }
    }

    /// Queues one ephemeral world event. Publicly observable events are projected into compact
    /// state before `tick`, `handle_command`, or `drain_events` returns.
    fn emit(events: &mut Vec<WorldEvent>, event: WorldEvent) {
        events.push(event);
    }

    pub fn new(colony_seed: [u8; 32], now: OffsetDateTime, desktop: &DesktopSnapshot) -> Self {
        Self::new_from(colony_seed, now, desktop, Edition::LATEST)
    }

    /// A new colony whose companions all come from the original generator: for the scenes tests
    /// tuned on the companions it made.
    #[cfg(test)]
    pub(crate) fn new_original(
        colony_seed: [u8; 32],
        now: OffsetDateTime,
        desktop: &DesktopSnapshot,
    ) -> Self {
        Self::new_from(colony_seed, now, desktop, Edition::Original)
    }

    /// A new colony from whichever generator a test compares.
    #[cfg(test)]
    pub(crate) fn new_from_generator(
        colony_seed: [u8; 32],
        now: OffsetDateTime,
        desktop: &DesktopSnapshot,
        generator: Edition,
    ) -> Self {
        Self::new_from(colony_seed, now, desktop, generator)
    }

    fn new_from(
        colony_seed: [u8; 32],
        now: OffsetDateTime,
        desktop: &DesktopSnapshot,
        generator: Edition,
    ) -> Self {
        let streams = SeedStream::new(colony_seed);
        let creature =
            generate_new_creature(&streams, colony_seed, 0, now, desktop, &[], None, generator);
        let home_display = desktop
            .monitors
            .iter()
            .find(|monitor| monitor.primary)
            .or_else(|| desktop.monitors.first())
            .map(|monitor| monitor.display_key);
        let mut home = ColonyHome::from_seed(colony_seed, home_display, Some(now), None);
        home.unlocks.next_at_utc = scheduled_village_unlock_at(colony_seed, 0, now);
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
            tallies: Vec::new(),
            ritual: RitualState {
                next_at_utc: scheduled_ritual_at(colony_seed, 0, now),
                ..RitualState::default()
            },
            objects: ColonyObjectState {
                next_at_utc: scheduled_colony_object_at(colony_seed, 0, now),
                ..ColonyObjectState::default()
            },
            visitors: VisitorState::default(),
            finds_today: FindsToday::default(),
            day_book: DayBook::default(),
            trips: TripLog::default(),
            battle: crate::BattleLog::default(),
        };
        let mut world = Self::from_save(save);
        world.generator = generator;
        world
    }

    /// Open a colony. Only a validated one can be opened: a `SaveFile` passed here is validated
    /// first, and one read from disk already has been.
    pub fn from_save(save: impl Into<crate::ValidatedSave>) -> Self {
        let mut save = save.into().into_inner();
        save.visitors.settle_after_opening();
        // Identity and durable drives survive relaunch, but interrupted locomotion and reactions do
        // not. Surface attachments are validated against the first desktop snapshot on the next
        // tick, while every creature resumes from a stable pose.
        for creature in &mut save.creatures {
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
                        inspect_remaining: ambient_rng.random_range(ROAMING.inspect_interval_secs),
                        dangle_remaining: ambient_rng.random_range(ROAMING.dangle_interval_secs),
                        climb_rest: 0.0,
                    },
                )
            })
            .collect();
        let discovery_remaining = ambient_rng.random_range(FINDS.desktop_interval_secs);
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
            home_roam: BTreeMap::new(),
            home_roam_rng: streams.rng("home-roaming", 0),
            habit_rng: streams.rng("habits", 0),
            village_moment: None,
            undo_history: VecDeque::new(),
            day_clock: None,
            home_seconds_pending: 0.0,
            tows: tows::TowTable::default(),
            beats: beats::Beats::new(&streams),
            antics: antics::Antics::new(&streams),
            village_life: BTreeMap::new(),
            village_life_rng: streams.rng("village-life", 0),
            moment_rng: streams.rng("village-moments", 0),
            colony_plan: None,
            wonders: wonders::Wonders::new(&streams),
            house_visit: BTreeSet::new(),
            creature_views: Vec::new(),
            relationship_views: Vec::new(),
            generator: Edition::LATEST,
            echo_fox: false,
            topology: DesktopTopology::default(),
            geometry_observer: crate::attention::GeometryObserver::default(),
            attention: AttentionRuntime::default(),
            cursor_observer: crate::cursor::CursorObserver::default(),
            display_attention: DisplayAttention::default(),
        }
    }

    pub fn tick(&mut self, now: OffsetDateTime, dt: f32, desktop: &DesktopSnapshot) {
        self.tick_colony(now, dt, desktop);
        // Last, once everyone has got wherever this tick takes them.
        self.carry_presence(dt, desktop);
    }

    fn tick_colony(&mut self, now: OffsetDateTime, dt: f32, desktop: &DesktopSnapshot) {
        if now > self.save.maximum_seen_utc {
            self.save.maximum_seen_utc = now;
        }
        let timeline_now = self.save.maximum_seen_utc;
        self.keep_echo_fox();
        // Where this tick's own events start, so the pass at the end of it can tell what happened
        // just now from whatever the desk has not collected yet.
        let events_before = self.events.len();
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
        self.count_home_time(now);
        self.process_arrivals(timeline_now, desktop);
        self.process_colony_objects(timeline_now, desktop);
        self.process_village_unlocks(timeline_now);
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
                        self.save.settings.display_scale,
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
        self.advance_beats(dt, desktop);
        self.advance_antics(dt);
        if self.save.settings.paused {
            self.settle_active_tosses(desktop);
            self.project_events(timeline_now);
            return;
        }
        if home_active {
            self.advance_village_moment(dt);
            self.advance_wonders(dt, desktop, true);
            self.tick_homebound_creatures(timeline_now, dt, desktop);
            self.tick_visitor(timeline_now, dt, desktop);
            self.advance_flourishes();
            self.sample_observations(dt, desktop);
            self.last_windows = desktop
                .windows
                .iter()
                .map(|window| (window.key, window.bounds))
                .collect();
            self.signal_dropping_off(events_before);
            self.project_events(timeline_now);
            return;
        }
        if self.save.settings.visible {
            for timers in self.ambient_timers.values_mut() {
                timers.inspect_remaining = (timers.inspect_remaining - dt).max(0.0);
                timers.dangle_remaining = (timers.dangle_remaining - dt).max(0.0);
                timers.climb_rest = (timers.climb_rest - dt).max(0.0);
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
        self.advance_wonders(dt, desktop, false);
        self.creature_views.clear();
        self.creature_views.extend_from_slice(&self.save.creatures);
        self.relationship_views.clear();
        self.relationship_views
            .extend_from_slice(&self.save.relationships);
        let creature_views = &self.creature_views;
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
        let finds_allowed = self.trinket_find_allowed(timeline_now);
        let night = is_night(local_time_or_utc(now).hour());
        let shared = step::TickShared {
            now,
            timeline_now,
            dt,
            desktop,
            cottages: &cottages,
            finds_allowed,
            night,
        };
        for index in 0..self.save.creatures.len() {
            self.step_creature(index, &shared);
        }
        let unconstrained: Vec<_> = self
            .interaction
            .as_ref()
            .map(|interaction| interaction.creature_id)
            .into_iter()
            .chain(self.window_journeys.keys().copied())
            .chain(self.tosses.keys().copied())
            .chain(self.attention.crossing_ids())
            .chain(self.wonders.players())
            .collect();
        keep_creatures_in_habitat(
            &mut self.save.creatures,
            desktop,
            &self.save.settings.habitat,
            &unconstrained,
            self.save.settings.display_scale,
        );
        self.resolve_overlaps(dt, desktop);
        self.advance_flourishes();
        self.last_windows = desktop
            .windows
            .iter()
            .map(|window| (window.key, window.bounds))
            .collect();
        self.sample_observations(dt, desktop);
        self.signal_dropping_off(events_before);
        self.project_events(timeline_now);
    }

    /// A companion that has just dropped off says so, once, with the same bubble it uses to turn
    /// down a snack it is too sleepy for. Every way into a nap ends in one `ActionStarted`, so
    /// reading this tick's own events catches the lot — the quiet moment at a village door, the
    /// cushion somebody put down, and an ordinary sleep out on the desktop — without each of
    /// those paths having to remember to say it.
    fn signal_dropping_off(&mut self, events_before: usize) {
        if !self.save.settings.visible {
            return;
        }
        let sleepers: Vec<CreatureId> = self.events[events_before..]
            .iter()
            .filter_map(|event| match event {
                WorldEvent::ActionStarted {
                    creature_id,
                    action: ActionKind::Sleep,
                } => Some(*creature_id),
                _ => None,
            })
            .collect();
        for creature_id in sleepers {
            self.show_bubble(creature_id, BubbleIcon::Sleepy);
        }
    }

    /// Moves every habit being done along: a waiting one starts once its companion has stopped,
    /// and a finished one lets the action carry on as usual.
    fn advance_flourishes(&mut self) {
        let guest = self
            .save
            .visitors
            .guest
            .as_mut()
            .map(|guest| &mut guest.creature);
        for creature in self.save.creatures.iter_mut().chain(guest) {
            advance_flourish(creature);
        }
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
