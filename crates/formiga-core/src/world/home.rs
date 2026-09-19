use super::*;

pub(super) const HOME_DURATION: time::Duration = time::Duration::minutes(15);
const HOME_COOLDOWN: time::Duration = time::Duration::minutes(15);

/// The village lays its porches out with its own copy of the colony-wide spacing rule, because
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
}

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
            if creature.state.position == goal || moment.remaining <= 0.0 {
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
fn choose_home_moment(
    rng: &mut ChaCha12Rng,
    creature: &mut Creature,
    rest: Point,
    house: Option<Point>,
    neighbour: Option<(CreatureId, Point)>,
    belongings: &[Point],
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
    let mut choices: Vec<(u8, ActionKind, f32, f32)> = vec![
        (4, ActionKind::Eat, 6.0, 12.0),
        (4, ActionKind::Drink, 5.0, 10.0),
        (4, ActionKind::SoloPlay, 8.0, 16.0),
        (3, ActionKind::InspectScreen, 5.0, 9.0),
        (2, ActionKind::Sleep, 60.0, 150.0),
    ];
    if neighbour.is_some() {
        choices.push((3, ActionKind::Greet, 4.0, 7.0));
    }
    if errand.is_some() {
        choices.push((3, ActionKind::Traverse, ERRAND_WALK_SECS, ERRAND_WALK_SECS));
    }
    let total: u32 = choices.iter().map(|(weight, ..)| u32::from(*weight)).sum();
    let mut roll = rng.random_range(0..total);
    let (_, action, low, high) = choices
        .iter()
        .copied()
        .find(|(weight, ..)| {
            let hit = roll < u32::from(*weight);
            roll = roll.saturating_sub(u32::from(*weight));
            hit
        })
        .unwrap_or((1, ActionKind::Eat, 6.0, 12.0));
    let seconds = if low < high {
        rng.random_range(low..high)
    } else {
        low
    };

    let mut answering = None;
    match action {
        ActionKind::Greet => {
            if let Some((id, point)) = neighbour {
                creature.state.facing_right = point.x >= rest.x;
                answering = Some(id);
            }
        }
        ActionKind::InspectScreen => {
            if let Some(point) = house {
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
        }
        self.save.home.is_active()
    }

    /// The home appears and everyone sets off for it. Shared by the ordinary cycle and by a
    /// gathering the person at the desk asked for.
    fn begin_home_visit(&mut self, now: OffsetDateTime) {
        self.save.home.active_since_utc = Some(now);
        self.home_moments.clear();
        self.home_moment_timers.clear();
        self.window_journeys.clear();
        self.window_routes.clear();
        self.tosses.clear();
        self.action_choices.clear();
        self.bond_plans.clear();
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
        // Everyone waits out a visit beside its own door, laid out by the same walk that places
        // the houses and the belongings, so nobody stands in front of a wall and no two faces
        // end up behind one another.
        let (resting, houses) = self.village_places(desktop);
        let belongings = self.village_belongings(desktop);
        let frame = spacing::frame_width(self.save.settings.display_scale, monitor.scale_factor);
        // Passive moments are decoration. They stop for a still desktop and for a hidden one.
        let quiet = self.save.settings.reduce_motion || !self.save.settings.visible;
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
            let target = resting.get(&creature.id).copied().unwrap_or(anchor);

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
                let speed = 24.0 + creature.personality.activity * 34.0;
                if distance > 0.0 {
                    creature.state.facing_right = target.x >= previous.x;
                    creature.state.position =
                        lerp_point(previous, target, (speed * dt / distance).clamp(0.0, 1.0));
                }
                if creature.state.position == target {
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
            }
            creature.state.drives.comfort = (creature.state.drives.comfort + dt * 0.01).min(1.0);
            creature.state.drives.arousal = (creature.state.drives.arousal - dt * 0.04).max(0.0);

            // Standing at its own door with time on its hands: every so often, one small thing.
            if next_action != ActionKind::Homebound {
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
            let (moment, answered) = choose_home_moment(
                &mut self.home_moment_rng,
                creature,
                target,
                houses.get(&creature.id).copied(),
                neighbour,
                &belongings,
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
    fn village_places(
        &self,
        desktop: &DesktopSnapshot,
    ) -> (BTreeMap<CreatureId, Point>, BTreeMap<CreatureId, Point>) {
        let cottages = colony_cottages(&self.save.creatures);
        let mut order: Vec<_> = self
            .save
            .creatures
            .iter()
            .map(|creature| (creature.colony_order, creature.id))
            .collect();
        order.sort_unstable();
        let mut resting = BTreeMap::new();
        let mut houses = BTreeMap::new();
        for (slot, (_, creature_id)) in order.into_iter().enumerate() {
            if let Some((_, point)) = home_resting_position(
                &self.save.home,
                slot,
                &cottages,
                &desktop.monitors,
                &self.save.settings.habitat,
                self.save.settings.display_scale,
            ) {
                resting.insert(creature_id, point);
            }
            if let Some((_, point)) = home_dwelling_position(
                &self.save.home,
                slot,
                &cottages,
                &desktop.monitors,
                &self.save.settings.habitat,
                self.save.settings.display_scale,
            ) {
                houses.insert(creature_id, point);
            }
        }
        (resting, houses)
    }

    /// The belongings that are actually on the strip right now, for a two-step errand.
    fn village_belongings(&self, desktop: &DesktopSnapshot) -> Vec<Point> {
        let cottages = colony_cottages(&self.save.creatures);
        (0..self.save.objects.objects.len().min(MAX_COLONY_OBJECTS))
            .filter_map(|slot| {
                home_object_position(
                    &self.save.home,
                    slot,
                    &cottages,
                    &desktop.monitors,
                    &self.save.settings.habitat,
                    self.save.settings.display_scale,
                )
                .map(|(_, point)| point)
            })
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
        // Arrived and standing still: either already settled at its door, or mid-moment there.
        if creature.state.arrival_delay_secs > 0.0
            || (resting.is_none() && creature.state.action != ActionKind::Homebound)
        {
            return false;
        }
        // An offer outranks an idle fidget, and the scheduler leaves a requested moment alone.
        let rest = resting.map_or(creature.state.position, |moment| moment.rest);
        self.home_moments.insert(
            creature_id,
            HomeMoment {
                action,
                phase: MomentPhase::Hold,
                remaining: seconds.max(0.0),
                rest,
                errand: None,
                courtesy: true,
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
        self.cancel_home_moments();
        self.home_moment_timers.clear();
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
