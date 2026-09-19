use super::home::HOME_DURATION;
use super::*;

/// How long after the houses appear the visitor turns up. The residents walk home first; a guest
/// arriving with them would read as one of them.
const ARRIVAL_DELAY_SECS: f32 = 12.0;
/// How much of the gathering is left when the visitor starts saying goodbye. Long enough for the
/// wave and the walk back out, so nobody is ever cut off mid-farewell.
const DEPARTURE_LEAD_SECS: f32 = 90.0;
/// The hello, and the wave goodbye.
const GREETING_SECS: f32 = 3.2;
const FAREWELL_SECS: f32 = 2.6;
/// One shared moment beside the houses, and the still stretch between two of them.
const BEAT_SECS: f32 = 6.5;
const STILL_SECS: f32 = 15.0;
/// A walk shorter than this is not a walk: the guest steps out from beside the last house.
const MIN_WALK: f32 = 24.0;
/// And no walk is longer than this, so an arrival reads the same on a laptop and on a wall of
/// displays rather than growing with the desktop.
const MAX_WALK: f32 = 520.0;
/// A walk that has not finished by now was never going to; the guest simply arrives or is gone.
const MAX_WALK_SECS: f32 = 40.0;
/// How long a resident holds its answer to the hello, before its temperament lengthens it.
const ANSWER_HOLD_SECS: f32 = 2.6;
/// A resident still on its way home only notices a guest it is already this close to. Anyone
/// resting at the village is part of the welcome, however long the strip has grown.
const NOTICE_DISTANCE: f32 = 420.0;
/// An invited friend stays for a day.
const INVITED_STAY: Duration = Duration::hours(24);

/// Where a guest stands, and where it walks in from.
struct GuestStage {
    monitor_id: MonitorId,
    spot: Point,
    /// The far side of the village's own accessible region, or none when there is no room to
    /// walk. A doorway is only ever offered inside the region the guest spot itself sits in, so
    /// the walk can never cross an excluded strip.
    doorway: Option<Point>,
    relative_x: f32,
}

impl World {
    /// Whether this is whoever is visiting, rather than a colony member.
    pub(super) fn is_guest(&self, creature_id: CreatureId) -> bool {
        self.save
            .visitors
            .on_stage()
            .is_some_and(|guest| guest.id == creature_id)
    }

    /// A colony member, or the visitor while it is out. Offers and petting treat a guest like
    /// anyone else; everything that belongs to the colony itself looks members up directly.
    pub(super) fn member_or_guest_mut(&mut self, creature_id: CreatureId) -> Option<&mut Creature> {
        if self.is_guest(creature_id) {
            return self
                .save
                .visitors
                .guest
                .as_mut()
                .map(|guest| &mut guest.creature);
        }
        creature_mut(&mut self.save.creatures, creature_id)
    }

    /// Advance whoever is visiting. Called on every tick of a home gathering, after the colony
    /// itself has been moved, and nowhere else: a visitor only exists at the houses.
    pub(super) fn tick_visitor(&mut self, now: OffsetDateTime, dt: f32, desktop: &DesktopSnapshot) {
        let Some(guest_id) = self
            .save
            .visitors
            .guest
            .as_ref()
            .map(|guest| guest.creature.id)
        else {
            return;
        };
        // The person at the desk has hold of the guest: it answers them and nothing else.
        if self
            .interaction
            .as_ref()
            .is_some_and(|interaction| interaction.creature_id == guest_id)
        {
            return;
        }
        // The village has nowhere to stand any more — a display went away, or the habitat was
        // redrawn under it. The guest leaves the desktop at once rather than standing outside.
        let Some(stage) = self.guest_stage(desktop) else {
            self.sign_if_visited(now);
            self.close_visit();
            return;
        };
        let reduce_motion = self.save.settings.reduce_motion;
        // How long is left of the gathering, on the same timeline the houses themselves use.
        let remaining = self.save.home.active_since_utc.map_or(0.0, |started| {
            (HOME_DURATION - (now - started)).as_seconds_f32()
        });
        let policy = self.save.settings.habitat.clone();
        let monitor = desktop
            .monitors
            .iter()
            .find(|monitor| monitor.id == stage.monitor_id)
            .cloned();

        let guest = self
            .save
            .visitors
            .guest
            .as_mut()
            .expect("a guest was found above");
        guest.visit.elapsed += dt;
        guest.visit.since_home += dt;
        if guest.visit.greeted {
            guest.visit.since_hello += dt;
        }
        guest.creature.state.action_elapsed += dt;
        // Petting answers first, exactly as it does for a homebound resident: the rest of the
        // visit waits where it stands until the guest has finished enjoying it.
        if guest.creature.state.action == ActionKind::PetReaction
            && guest.creature.state.action_elapsed < guest.creature.state.action_duration
        {
            return;
        }
        // So does a snack or a toy it accepted from the person at the desk.
        if offers::enjoying(&self.offers, guest_id) == Some(guest.creature.state.action)
            && guest.creature.state.action_elapsed < guest.creature.state.action_duration
        {
            return;
        }

        let mut hello = false;
        match guest.visit.phase {
            VisitPhase::Waiting => {
                guest.on_stage = false;
                // Turning up with only the goodbye left would be a walk-past, not a visit.
                if guest.visit.since_home < ARRIVAL_DELAY_SECS
                    || remaining <= DEPARTURE_LEAD_SECS + GREETING_SECS
                {
                    return;
                }
                // Reduced motion asks for no walk-in: the guest is simply there, saying hello.
                guest.visit.doorway = if reduce_motion { None } else { stage.doorway };
                let start = guest.visit.doorway.unwrap_or(stage.spot);
                stand(
                    &mut guest.creature,
                    start,
                    stage.monitor_id,
                    stage.relative_x,
                );
                guest.creature.state.facing_right = stage.spot.x >= start.x;
                guest.on_stage = true;
                if guest.visit.doorway.is_some() {
                    enter(guest, VisitPhase::ArrivingWalk, ActionKind::Traverse);
                } else {
                    enter(guest, VisitPhase::Greeting, ActionKind::Greet);
                    hello = true;
                }
            }
            VisitPhase::ArrivingWalk => {
                act(&mut guest.creature, ActionKind::Traverse);
                if step_toward(&mut guest.creature, stage.spot, dt)
                    || guest.visit.elapsed >= MAX_WALK_SECS
                {
                    stand(
                        &mut guest.creature,
                        stage.spot,
                        stage.monitor_id,
                        stage.relative_x,
                    );
                    enter(guest, VisitPhase::Greeting, ActionKind::Greet);
                    hello = true;
                }
            }
            VisitPhase::Greeting => {
                stand_at(guest, &stage);
                act(&mut guest.creature, ActionKind::Greet);
                if guest.visit.elapsed >= GREETING_SECS {
                    enter(guest, VisitPhase::Visiting, ActionKind::Idle);
                    guest.visit.beat = 0;
                    guest.visit.beat_remaining = STILL_SECS;
                }
            }
            VisitPhase::Visiting => {
                stand_at(guest, &stage);
                guest.visit.beat_remaining -= dt;
                if remaining <= DEPARTURE_LEAD_SECS {
                    enter(guest, VisitPhase::Farewell, ActionKind::Greet);
                } else {
                    if guest.visit.beat_remaining <= 0.0 {
                        guest.visit.beat = (guest.visit.beat + 1) % 8;
                        guest.visit.beat_remaining =
                            shared_moment(guest.visit.beat, reduce_motion).1;
                    }
                    let action = shared_moment(guest.visit.beat, reduce_motion).0;
                    act(&mut guest.creature, action);
                    // The dance is the one moment with a pose of its own; the rest are still.
                    guest.creature.state.attention = (action == ActionKind::SoloPlay)
                        .then(|| pose(stage.spot, Some(Gesture::Bop)))
                        .flatten();
                }
            }
            VisitPhase::Farewell => {
                stand_at(guest, &stage);
                act(&mut guest.creature, ActionKind::Greet);
                guest.creature.state.attention =
                    pose(stage.spot, (!reduce_motion).then_some(Gesture::Reach));
                if guest.visit.elapsed >= FAREWELL_SECS {
                    match guest.visit.doorway {
                        Some(doorway) if !reduce_motion => {
                            guest.creature.state.facing_right = doorway.x >= stage.spot.x;
                            enter(guest, VisitPhase::LeavingWalk, ActionKind::Traverse);
                        }
                        _ => enter(guest, VisitPhase::Gone, ActionKind::Idle),
                    }
                }
            }
            VisitPhase::LeavingWalk => {
                act(&mut guest.creature, ActionKind::Traverse);
                let doorway = guest.visit.doorway.unwrap_or(stage.spot);
                if step_toward(&mut guest.creature, doorway, dt)
                    || guest.visit.elapsed >= MAX_WALK_SECS
                {
                    enter(guest, VisitPhase::Gone, ActionKind::Idle);
                }
            }
            VisitPhase::Gone => {}
        }
        // A gathering that has run out entirely — quiet mode holding the houses open, a clock
        // jump — ends the visit wherever it had got to, rather than leaving a guest standing.
        if remaining <= 0.0 && guest.visit.phase != VisitPhase::Gone {
            enter(guest, VisitPhase::Gone, ActionKind::Idle);
        }
        if guest.visit.phase == VisitPhase::Gone {
            guest.on_stage = false;
            guest.creature.state.attention = None;
            guest.creature.state.velocity = Point::default();
        }
        // Whatever the scene asked for, the guest ends its tick standing somewhere real.
        if let Some(monitor) = &monitor
            && !habitat_contains(&policy, monitor, guest.creature.state.position)
        {
            stand_at(guest, &stage);
        }
        let left = guest.visit.phase == VisitPhase::Gone;

        if hello {
            self.say_hello(now, stage.spot);
        }
        self.answer_the_hello(stage.spot);
        // A wanderer's line in the book is written on its way out, once it really has been here.
        if left {
            self.sign_if_visited(now);
        }
    }

    /// The home has just appeared. Decide whether anyone comes by this time.
    pub(super) fn visitor_home_appeared(&mut self, now: OffsetDateTime) {
        let ordinal = self.save.visitors.gatherings;
        self.save.visitors.gatherings = ordinal.saturating_add(1);
        // An invited friend whose day is up simply does not come back.
        if self
            .save
            .visitors
            .guest
            .as_ref()
            .is_some_and(|guest| guest.outstayed(now))
        {
            self.save.visitors.guest = None;
        }
        if let Some(guest) = &mut self.save.visitors.guest {
            // An invited guest is at the houses for every gathering of its stay, and arrives the
            // same way each time.
            guest.visit = VisitProgress::default();
            return;
        }
        if !wanderer_due(self.save.colony_seed, ordinal) {
            return;
        }
        if let Some(creature) = self.generate_wanderer(ordinal, now) {
            self.save.visitors.guest = Some(Visitor::new(creature, VisitorSource::Wanderer, None));
        }
    }

    /// The home has gone. Whoever was visiting leaves the desktop with it.
    pub(super) fn visitor_home_disappeared(&mut self, now: OffsetDateTime) {
        // A wanderer that really was here signs on its way out, even when the houses are
        // dismissed out from under it; one that never appeared leaves no trace at all.
        self.sign_if_visited(now);
        self.close_visit();
        let leaving =
            self.save.visitors.guest.as_ref().is_some_and(|guest| {
                guest.source == VisitorSource::Wanderer || guest.outstayed(now)
            });
        if leaving {
            self.save.visitors.guest = None;
        }
    }

    /// Invite a friend's creature to stay for a day. The code is read offline, like every other
    /// seed code: nothing is fetched, and nothing about the friend's colony is learned.
    pub fn invite_visitor(
        &mut self,
        shared: SharedCreatureSeed,
        now: OffsetDateTime,
        desktop: &DesktopSnapshot,
    ) -> Result<(), VisitorError> {
        if self.save.visitors.guest.is_some() {
            return Err(VisitorError::GuestPresent);
        }
        let mut creature = generate_source_creature(shared, now, desktop);
        if self
            .save
            .creatures
            .iter()
            .any(|member| member.id == creature.id)
        {
            return Err(VisitorError::AlreadyHome);
        }
        creature.name = self.guest_name(shared.source_colony_seed, shared.source_generation);
        creature.state.arrival_delay_secs = 0.0;
        // A day on the colony's own timeline, which never runs backwards, so winding the clock
        // back can never lengthen a stay.
        let until = self.save.maximum_seen_utc.max(now) + INVITED_STAY;
        self.save.visitors.guest =
            Some(Visitor::new(creature, VisitorSource::Invited, Some(until)));
        // Call everyone home, so a friend invited now turns up now rather than in half an hour.
        self.send_home(desktop);
        Ok(())
    }

    /// Whether the colony has room to keep whoever is visiting.
    pub fn visitor_can_stay(&self) -> bool {
        self.save.visitors.can_stay(&self.save.creatures)
    }

    /// Ask the visitor to stay for good. It joins through the ordinary adoption path — the exact
    /// appearance and temperament its code carries, with a fresh history of its own — and stands
    /// where it already was rather than appearing somewhere new.
    pub fn ask_visitor_to_stay(
        &mut self,
        now: OffsetDateTime,
        desktop: &DesktopSnapshot,
    ) -> Result<CreatureId, ColonyManagementError> {
        let Some(guest) = &self.save.visitors.guest else {
            return Err(ColonyManagementError::CreatureNotFound);
        };
        let shared = SharedCreatureSeed::from(guest.creature.origin);
        let position = guest.creature.state.position;
        let surface = guest.creature.state.surface.clone();
        let creature_id = self.adopt_shared_creature(shared, None, now, desktop)?;
        if let Some(member) = creature_mut(&mut self.save.creatures, creature_id) {
            member.state.position = position;
            member.state.surface = surface;
        }
        self.sign_guest_book(now);
        self.save.visitors.guest = None;
        self.show_bubble(creature_id, BubbleIcon::Stay);
        Ok(creature_id)
    }

    /// The visitor's own seed code, to keep or to pass on.
    pub fn visitor_share_code(&self) -> Option<String> {
        self.save
            .visitors
            .guest
            .as_ref()
            .map(|guest| encode_creature_seed(guest.creature.origin))
    }

    /// Say hello: one bubble, and the colony's own answers, planned here so each resident turns
    /// in its own time.
    fn say_hello(&mut self, now: OffsetDateTime, spot: Point) {
        let answers = self.plan_answers(spot);
        let Some(guest) = &mut self.save.visitors.guest else {
            return;
        };
        let (guest_id, invited) = (guest.creature.id, guest.source == VisitorSource::Invited);
        guest.visit.greeted = true;
        guest.visit.since_hello = 0.0;
        guest.visit.answers = answers;
        guest.creature.state.attention = pose(spot, None);
        self.show_bubble(guest_id, BubbleIcon::Hello);
        // A day-long stay is one visit, written down the first time the friend says hello.
        if invited {
            self.sign_guest_book(now);
        }
    }

    /// Nearby residents turn and answer, staggered by how sociable each one is. Nobody leaves
    /// their place at the houses: an answer is a turn of the head and a pose, nothing more.
    fn answer_the_hello(&mut self, spot: Point) {
        let Some(guest) = &self.save.visitors.guest else {
            return;
        };
        if !guest.visit.greeted
            || !matches!(
                guest.visit.phase,
                VisitPhase::Greeting | VisitPhase::Visiting
            )
        {
            return;
        }
        let since = guest.visit.since_hello;
        let answers = guest.visit.answers.clone();
        for answer in answers {
            if since < answer.after || since >= answer.after + answer.hold {
                continue;
            }
            let Some(creature) = creature_mut(&mut self.save.creatures, answer.creature_id) else {
                continue;
            };
            creature.state.facing_right = spot.x >= creature.state.position.x;
            creature.state.attention = Some(AttentionPose {
                target: spot,
                emotion: AttentionEmotion::Enjoying,
                hanging: 0.0,
                gesture: answer.gesture,
            });
        }
    }

    fn plan_answers(&self, spot: Point) -> Vec<ResidentAnswer> {
        let reduce_motion = self.save.settings.reduce_motion;
        self.save
            .creatures
            .iter()
            .filter(|creature| creature.state.arrival_delay_secs <= 0.0)
            .filter(|creature| {
                self.resting_at_home(creature.id)
                    || creature.state.position.distance(spot) <= NOTICE_DISTANCE
            })
            .enumerate()
            .map(|(index, creature)| ResidentAnswer {
                creature_id: creature.id,
                // The sociable ones look up first and the rest follow, a beat apart.
                after: 0.4 + index as f32 * 0.9 + (1.0 - creature.personality.sociability) * 2.2,
                hold: ANSWER_HOLD_SECS + creature.personality.sociability * 1.4,
                // A playful one bounces where it stands, a bold one waves back, and a timid one
                // only looks. Reduced motion leaves everyone looking.
                gesture: if reduce_motion {
                    None
                } else if creature.personality.playfulness >= 0.6 {
                    Some(Gesture::Bop)
                } else if creature.personality.boldness >= 0.35 {
                    Some(Gesture::Reach)
                } else {
                    None
                },
            })
            .collect()
    }

    /// Write down a visit that really happened: the guest was here, and said hello.
    fn sign_if_visited(&mut self, now: OffsetDateTime) {
        if self
            .save
            .visitors
            .guest
            .as_ref()
            .is_some_and(|guest| guest.visit.greeted)
        {
            self.sign_guest_book(now);
        }
    }

    /// One line of the guest book and one journal moment, unless this visit is already in them.
    fn sign_guest_book(&mut self, now: OffsetDateTime) {
        let Some(guest) = &mut self.save.visitors.guest else {
            return;
        };
        if guest.signed {
            return;
        }
        guest.signed = true;
        let (name, origin, source) = (
            guest.creature.name.clone(),
            guest.creature.origin,
            guest.source,
        );
        self.save.visitors.sign(GuestBookEntry {
            visited_at_utc: now,
            name: name.clone(),
            origin,
            source,
        });
        self.save
            .companion
            .remember(None, JournalMoment::Visit(name), now);
    }

    /// The guest is off the desktop, wherever the visit had got to.
    fn close_visit(&mut self) {
        if let Some(guest) = &mut self.save.visitors.guest {
            guest.on_stage = false;
            guest.visit = VisitProgress::default();
            guest.creature.state.attention = None;
            guest.creature.state.velocity = Point::default();
        }
    }

    /// A cosy name of its own, never one a resident already answers to.
    fn guest_name(&self, seed: [u8; 32], generation: u8) -> String {
        let taken: Vec<_> = self
            .save
            .creatures
            .iter()
            .map(|creature| creature.name.clone())
            .collect();
        default_creature_name(seed, generation, &taken)
    }

    /// Someone new, built from the colony's own seed and the ordinal of this gathering, so one
    /// colony always meets the same visitors in the same order and no two colonies share them.
    /// A visitor never has a colony member's identity.
    fn generate_wanderer(&self, ordinal: u32, now: OffsetDateTime) -> Option<Creature> {
        let streams = SeedStream::new(self.save.colony_seed);
        let desktop = DesktopSnapshot::default();
        for attempt in 0..8_u64 {
            let index = u64::from(ordinal) * 8 + attempt;
            let seed = streams.bytes("visitor-seed-v1", index);
            // Mostly grown-up visitors; now and then a small one comes by instead.
            let generation = u8::from(streams.bytes("visitor-kind-v1", index)[0].is_multiple_of(4));
            let shared = SharedCreatureSeed {
                source_colony_seed: seed,
                source_generation: generation,
                design: Some(CreatureDesign::generated(seed, generation, None)),
            };
            let mut creature = generate_source_creature(shared, now, &desktop);
            if self
                .save
                .creatures
                .iter()
                .any(|member| member.id == creature.id)
            {
                continue;
            }
            creature.name = self.guest_name(seed, generation);
            creature.state.arrival_delay_secs = 0.0;
            return Some(creature);
        }
        None
    }

    /// Where the guest stands this tick, and where it walks in from.
    fn guest_stage(&self, desktop: &DesktopSnapshot) -> Option<GuestStage> {
        let policy = &self.save.settings.habitat;
        let cottages = colony_cottages(&self.save.creatures);
        let (monitor_id, spot) = home_guest_position(
            &self.save.home,
            &cottages,
            self.save.objects.objects.len(),
            &desktop.monitors,
            policy,
            self.save.settings.display_scale,
        )?;
        let monitor = desktop
            .monitors
            .iter()
            .find(|monitor| monitor.id == monitor_id)?;
        let region = accessible_regions(policy, monitor)
            .into_iter()
            .find(|region| region.contains(spot))?;
        let outward = self.save.home.corner == HomeCorner::BottomLeft;
        let edge = if outward {
            region.right() - 8.0
        } else {
            region.x + 8.0
        };
        let reach = (edge - spot.x).abs().min(MAX_WALK);
        Some(GuestStage {
            monitor_id,
            spot,
            doorway: (reach >= MIN_WALK).then(|| Point {
                x: spot.x + (edge - spot.x).signum() * reach,
                y: spot.y,
            }),
            relative_x: ((spot.x - region.x) / region.width.max(1.0)).clamp(0.0, 1.0),
        })
    }
}

/// Whether a wanderer turns up at gathering `ordinal`. Exactly one gathering in every block of
/// four brings someone by, at a place in the block taken from the colony's own seed: about one
/// gathering in four, with no long drought and no run of visitors either.
pub(super) fn wanderer_due(colony_seed: [u8; 32], ordinal: u32) -> bool {
    let block = u64::from(ordinal / 4);
    let slot = u32::from(SeedStream::new(colony_seed).bytes("visitor-cadence-v1", block)[0] % 4);
    slot == ordinal % 4
}

/// The calm moments of a visit, in order: mostly standing about, with a snack shared, a trinket
/// shown, a little dance, and a sit-down between the still stretches.
fn shared_moment(beat: u8, reduce_motion: bool) -> (ActionKind, f32) {
    match beat {
        1 => (ActionKind::Eat, BEAT_SECS),
        3 => (ActionKind::PresentDiscovery, BEAT_SECS),
        5 if !reduce_motion => (ActionKind::SoloPlay, BEAT_SECS),
        7 => (ActionKind::Perch, BEAT_SECS),
        _ => (ActionKind::Idle, STILL_SECS),
    }
}

fn pose(target: Point, gesture: Option<Gesture>) -> Option<AttentionPose> {
    Some(AttentionPose {
        target,
        emotion: AttentionEmotion::Enjoying,
        hanging: 0.0,
        gesture,
    })
}

fn enter(guest: &mut Visitor, phase: VisitPhase, action: ActionKind) {
    guest.visit.phase = phase;
    guest.visit.elapsed = 0.0;
    guest.creature.state.attention = None;
    guest.creature.state.velocity = Point::default();
    act(&mut guest.creature, action);
}

fn act(creature: &mut Creature, action: ActionKind) {
    if creature.state.action != action {
        creature.state.action = action;
        creature.state.action_elapsed = 0.0;
    }
    creature.state.action_duration = f32::MAX;
}

fn stand_at(guest: &mut Visitor, stage: &GuestStage) {
    stand(
        &mut guest.creature,
        stage.spot,
        stage.monitor_id,
        stage.relative_x,
    );
}

fn stand(creature: &mut Creature, at: Point, monitor_id: MonitorId, relative_x: f32) {
    creature.state.position = at;
    creature.state.velocity = Point::default();
    creature.state.surface = SurfaceAttachment {
        kind: SurfaceKind::ScreenFloor,
        monitor_id,
        window_key: None,
        relative_x,
    };
}

/// One step of a walk along the village ground line. Returns whether the walk is finished.
fn step_toward(creature: &mut Creature, target: Point, dt: f32) -> bool {
    let previous = creature.state.position;
    let distance = previous.distance(target);
    if distance <= 0.0 {
        creature.state.velocity = Point::default();
        return true;
    }
    creature.state.facing_right = target.x >= previous.x;
    let speed = 24.0 + creature.personality.activity * 34.0;
    let progress = (speed * dt / distance).clamp(0.0, 1.0);
    creature.state.position = if progress >= 1.0 {
        target
    } else {
        lerp_point(previous, target, progress)
    };
    creature.state.velocity = if dt > 0.0 {
        Point {
            x: (creature.state.position.x - previous.x) / dt,
            y: (creature.state.position.y - previous.y) / dt,
        }
    } else {
        Point::default()
    };
    creature.state.position == target
}
