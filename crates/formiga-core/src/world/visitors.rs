use super::home::HOME_DURATION;
use super::*;
use crate::tuning::VISITS;

/// The most answers one visit can be holding: one per resident for the hello, and one more each
/// for the walk round, since a guest only ever goes over to somebody it has not met yet.
const MAX_VISIT_ANSWERS: usize = 2 * MAX_COLONY_CREATURES;

/// How far beside a garden patch, an ornament or a belonging a guest stands to look at it, in
/// creature frames: the same step a resident's errand to a belonging stops short by, and a
/// little more, since a patch is wider than a belonging.
const BESIDE_REACH: f32 = 0.45;

/// How many places along the middle path a guest considers before giving it up as all behind
/// houses.
const PATH_TRIES: usize = 7;

/// The residents a guest can see from where it is standing. On the stack: a colony is never more
/// than six, and this is rebuilt on every tick of a visit.
type Residents = [Option<(CreatureId, Point)>; MAX_COLONY_CREATURES];

/// Everywhere a resident is standing about, headed, or busy, which a guest choosing where to go
/// next keeps out of: one place each for where it stands and where its stroll is taking it, and
/// up to two more for a moment and a plan of its own. A resident resting at its own place is
/// not in it: it makes room for a guest come to stand beside it, as for a busy neighbour. On the
/// stack, like [`Residents`].
type Crowd = [Option<Point>; 4 * MAX_COLONY_CREATURES];

/// Where a guest stands, and where it walks in from.
struct GuestStage {
    monitor_id: MonitorId,
    spot: Point,
    /// The far side of the village's own accessible region, or none when there is no room to
    /// walk. A doorway is only ever offered inside the region the guest spot itself sits in, so
    /// the walk can never cross an excluded strip.
    doorway: Option<Point>,
    relative_x: f32,
    /// The one accessible region the village stands in. Every stop on the tour has to be inside
    /// it, so a guest can never walk round into an excluded strip.
    region: DesktopRect,
    /// The paths in from the front of the grass to every door, which a guest goes up and down by
    /// exactly as a resident does.
    paths: village_paths::VillagePaths,
    /// How wide a creature is drawn here, in points, and how close another may come to its face.
    frame: f32,
    clear: f32,
}

impl World {
    /// Whether this is whoever is visiting, rather than a colony member.
    pub(super) fn is_guest(&self, creature_id: CreatureId) -> bool {
        self.save
            .visitors
            .on_stage()
            .is_some_and(|guest| guest.id == creature_id)
    }

    /// Where a guest out on its walk round the village is standing, or on its way to stand, so
    /// the residents leave it room there as they do one another.
    pub(super) fn guest_spot(&self) -> Option<(CreatureId, Point)> {
        let guest = self.save.visitors.guest.as_ref().filter(|guest| {
            guest.on_stage
                && guest.visit.phase == VisitPhase::Visiting
                && guest.visit.stops.len() > 1
        })?;
        let stop = guest.visit.stops.get(usize::from(guest.visit.stop))?;
        Some((guest.creature.id, stop.at))
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
        // Who the guest can go over to, gathered before it is borrowed: a colony is never more
        // than six, so this costs nothing and never allocates.
        // Anyone sharing a moment with the rest of the village is busy, and is left to it.
        let mut residents: Residents = [None; MAX_COLONY_CREATURES];
        for (slot, entry) in residents.iter_mut().enumerate() {
            *entry = self
                .save
                .creatures
                .get(slot)
                .filter(|creature| {
                    creature.state.arrival_delay_secs <= 0.0 && !self.in_village_moment(creature.id)
                })
                .map(|creature| (creature.id, creature.state.position));
        }
        // Where the colony is, or is about to be: a guest picks somewhere out of everybody's way
        // to go next, the way a resident picks the far end of a stroll.
        let mut crowd: Crowd = [None; 4 * MAX_COLONY_CREATURES];
        for (slot, point) in crowd.iter_mut().zip(
            self.standing_about()
                .chain(self.stroll_targets())
                .chain(self.busy_spots())
                .map(|(_, point)| point),
        ) {
            *slot = Some(point);
        }
        let seed = self.visit_seed();
        // The walk round is worked out once a visit, and again whenever the village has moved
        // underneath the guest. Reduced motion asks for no walking at all, so it has no tour.
        let replan = (!reduce_motion
            && self
                .save
                .visitors
                .guest
                .as_ref()
                .is_some_and(|guest| guest.visit.planned != Some(stage.spot)))
        .then(|| self.plan_tour(&stage, desktop));
        // Whether there is still time to walk somewhere new before the goodbye.
        let roam = remaining > VISITS.departure_lead_secs + VISITS.tour_return_secs;

        let guest = self
            .save
            .visitors
            .guest
            .as_mut()
            .expect("a guest was found above");
        if let Some(stops) = replan {
            guest.visit.stops = stops;
            guest.visit.planned = Some(stage.spot);
            // A village that moved takes the guest back to the spot it walked in to.
            guest.visit.stop = 0;
            guest.visit.moment = TourMoment::Walking;
            guest.visit.stay = VISITS.max_walk_secs;
        }
        guest.visit.elapsed += dt;
        guest.visit.since_home += dt;
        if guest.visit.greeted {
            guest.visit.since_hello += dt;
        }
        guest.creature.state.action_elapsed += dt;
        // Petting answers first, exactly as it does for a homebound resident: the rest of the
        // visit waits where it stands until the guest has finished enjoying it. A guest caught
        // mid-stroll stops walking for it, and picks the walk up again afterwards.
        if guest.creature.state.action == ActionKind::PetReaction
            && guest.creature.state.action_elapsed < guest.creature.state.action_duration
        {
            guest.creature.state.velocity = Point::default();
            return;
        }
        // So does a snack or a toy it accepted from the person at the desk.
        if offers::enjoying(&self.offers, guest_id) == Some(guest.creature.state.action)
            && guest.creature.state.action_elapsed < guest.creature.state.action_duration
        {
            guest.creature.state.velocity = Point::default();
            return;
        }

        let mut hello = false;
        let mut met = None;
        match guest.visit.phase {
            VisitPhase::Waiting => {
                guest.on_stage = false;
                // Turning up with only the goodbye left would be a walk-past, not a visit.
                if guest.visit.since_home < VISITS.arrival_delay_secs
                    || remaining <= VISITS.departure_lead_secs + VISITS.greeting_secs
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
                guest.creature.state.depth = 0.0;
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
                    || guest.visit.elapsed >= VISITS.max_walk_secs
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
                if guest.visit.elapsed >= VISITS.greeting_secs {
                    enter(guest, VisitPhase::Visiting, ActionKind::Idle);
                    guest.visit.beat = 0;
                    guest.visit.beat_remaining = VISITS.still_secs;
                    // And off into the village, if it left anywhere to go: the tour picks the
                    // first place out of everybody's way on the guest's next step.
                    guest.visit.stop = 0;
                    guest.visit.moment = TourMoment::Resting;
                    guest.visit.stay = 0.0;
                }
            }
            VisitPhase::Visiting => {
                if remaining <= VISITS.departure_lead_secs {
                    // A tour that has run long simply stops touring.
                    enter(guest, VisitPhase::Farewell, ActionKind::Greet);
                } else if guest.visit.stops.len() > 1 {
                    met = tour(
                        guest,
                        &stage,
                        (&residents, &crowd),
                        seed,
                        dt,
                        reduce_motion,
                        roam,
                    );
                } else {
                    // Reduced motion, or a village with no ground to spare: the still visit,
                    // exactly as it was, beside the houses on the one spot.
                    stand_at(guest, &stage);
                    calm_moment(guest, stage.spot, dt, reduce_motion);
                }
            }
            VisitPhase::Farewell => {
                // Wherever the walk round had got to: a guest waves goodbye from where it is
                // standing rather than stepping back to the spot it came in at to do it.
                let here = guest.creature.state.position;
                let depth = guest.creature.state.depth;
                place(guest, here, &stage);
                guest.creature.state.depth = depth;
                act(&mut guest.creature, ActionKind::Greet);
                guest.creature.state.attention =
                    pose(here, (!reduce_motion).then_some(Gesture::Reach));
                if guest.visit.elapsed >= VISITS.farewell_secs {
                    match guest.visit.doorway {
                        Some(doorway) if !reduce_motion => {
                            guest.creature.state.facing_right = doorway.x >= here.x;
                            enter(guest, VisitPhase::LeavingWalk, ActionKind::Traverse);
                        }
                        _ => enter(guest, VisitPhase::Gone, ActionKind::Idle),
                    }
                }
            }
            VisitPhase::LeavingWalk => {
                act(&mut guest.creature, ActionKind::Traverse);
                let doorway = guest.visit.doorway.unwrap_or(stage.spot);
                // Down to the front of the grass first, by a path, if the goodbye was said
                // somewhere back in the village.
                if walk(&mut guest.creature, &stage.paths, doorway, 0.0, dt)
                    || guest.visit.elapsed >= VISITS.max_walk_secs
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
        let standing = guest.creature.state.position;
        // Out of sight behind a house nearer the front, a guest is out of reach of the pointer
        // as a resident is.
        if !left
            && stage.paths.hidden(
                standing.x,
                guest.creature.state.depth,
                RESTING_WIDTH / 2.0 * stage.frame / CREATURE_FRAME_WIDTH,
            )
            && !self.behind_houses.contains(&guest_id)
        {
            self.behind_houses.push(guest_id);
        }

        if hello {
            self.say_hello(now, stage.spot);
        }
        // One resident has just been walked over to and greeted: it answers in its own time.
        if let Some(creature_id) = met {
            self.answer_the_greeting(creature_id);
        }
        // The colony looks at where the guest actually is, so a resident that turned to answer
        // the hello follows it round the houses rather than watching the spot it came in at.
        self.answer_the_hello(standing);
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
        let until = self.save.maximum_seen_utc.max(now) + VISITS.invited_stay;
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
            if self.in_village_moment(answer.creature_id) {
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
            // Said once, as it turns.
            if let Some(icon) = answer.bubble
                && since < answer.after + 0.06
            {
                self.show_bubble(answer.creature_id, icon);
            }
        }
    }

    fn plan_answers(&self, spot: Point) -> Vec<ResidentAnswer> {
        let reduce_motion = self.save.settings.reduce_motion;
        self.save
            .creatures
            .iter()
            .filter(|creature| {
                creature.state.arrival_delay_secs <= 0.0 && !self.in_village_moment(creature.id)
            })
            .filter(|creature| {
                self.resting_at_home(creature.id)
                    || creature.state.position.distance(spot) <= VISITS.notice_distance
            })
            .enumerate()
            .map(|(index, creature)| self.answer_of(creature, index, reduce_motion))
            .collect()
    }

    /// How one resident answers the guest: as it would a stranger, or, for a guest it met on an
    /// earlier visit, as a familiar face. Who it met is read from the guest book and when the
    /// resident arrived; a companion who came to live here since the last visit never met them.
    fn answer_of(&self, creature: &Creature, index: usize, reduce_motion: bool) -> ResidentAnswer {
        let answer = resident_answer(creature, index, reduce_motion);
        let last_visit = self.save.visitors.earlier_visits().map(|(_, at)| at);
        if last_visit.is_some_and(|at| creature.born_at_utc <= at) {
            recognised(answer, reduce_motion)
        } else {
            answer
        }
    }

    /// One resident answers a hello said at a stop on the walk round. The colony's own machinery
    /// asked for a single answer rather than a round of them: a turn, a hold, and whatever its
    /// temperament makes of the moment. It never leaves its door, and nothing is written down.
    fn answer_the_greeting(&mut self, creature_id: CreatureId) {
        if self.in_village_moment(creature_id) {
            return;
        }
        let reduce_motion = self.save.settings.reduce_motion;
        let Some(answer) = self
            .save
            .creatures
            .iter()
            .find(|creature| creature.id == creature_id)
            .map(|creature| self.answer_of(creature, 0, reduce_motion))
        else {
            return;
        };
        let Some(guest) = &mut self.save.visitors.guest else {
            return;
        };
        if guest.visit.answers.len() >= MAX_VISIT_ANSWERS {
            return;
        }
        // Timed off the hello, like every other answer, so one clock carries a whole visit.
        guest.visit.answers.push(ResidentAnswer {
            after: guest.visit.since_hello + answer.after,
            ..answer
        });
    }

    /// Write down a visit that really happened: the guest was here, and said hello.
    pub(super) fn sign_if_visited(&mut self, now: OffsetDateTime) {
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
        // A guest the book already knows is written down as coming back, and which visit this is.
        // The book only keeps its last two dozen, so a visitor it has forgotten is new again.
        let before = self.save.visitors.visits_of(&origin);
        self.save.visitors.sign(GuestBookEntry {
            visited_at_utc: now,
            name: name.clone(),
            origin,
            source,
        });
        let moment = if before == 0 {
            JournalMoment::Visit(name)
        } else {
            JournalMoment::Revisit(name, u16::try_from(before + 1).unwrap_or(u16::MAX))
        };
        self.save.companion.remember(None, moment, now);
    }

    /// The guest is off the desktop, wherever the visit had got to.
    pub(super) fn close_visit(&mut self) {
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
                design: Some(CreatureDesign::generated_by(
                    self.generator,
                    seed,
                    generation,
                    None,
                )),
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
        let (low, high) = standing_span(
            region.x,
            region.right(),
            body_half_width(monitor, self.save.settings.display_scale),
        );
        let edge = if outward { high } else { low };
        let reach = (edge - spot.x).abs().min(VISITS.max_walk);
        let frame = spacing::frame_width(self.save.settings.display_scale, monitor.scale_factor);
        Some(GuestStage {
            monitor_id,
            spot,
            doorway: (reach >= VISITS.min_walk).then(|| Point {
                x: spot.x + (edge - spot.x).signum() * reach,
                y: spot.y,
            }),
            relative_x: ((spot.x - region.x) / region.width.max(1.0)).clamp(0.0, 1.0),
            region,
            paths: self.village_paths(desktop),
            frame,
            clear: frame * spacing::FACE_CLEAR_RATIO,
        })
    }

    /// Where a guest's walk around the village stops.
    ///
    /// Every measurement here is asked of the village's own layout rather than written down
    /// again, so a tour follows wherever the houses, their paths, the spots the person at the desk
    /// put down, the garden and the trees' yards have ended up. The first stop is always the spot
    /// the guest walked in to, at the outer end of the village, and it is only come back to for
    /// the goodbye. The rest are places in the village a resident would go to: one of the spots,
    /// used the way it is for; a doorstep up the path to a house set back from the front; the
    /// trodden path along the middle; a garden patch; and a keepsake, a belonging in a tree's
    /// yard or an ornament on the grass. Which spot, doorstep, patch and keepsake is the guest's
    /// own choice, from its seed, so two guests see the same village differently; in the order the
    /// walk meets them going away from the spot it came in at.
    ///
    /// A stop has to be inside the one accessible region the village stands in, no further back
    /// than the grass goes there without a house in front of it, off the things on the ground it
    /// has not come to use, and a face-clear step from every other stop. Who is standing where is
    /// left until the guest sets off for one, when it picks the next one out of everybody's way.
    fn plan_tour(&self, stage: &GuestStage, desktop: &DesktopSnapshot) -> Vec<TourStop> {
        // Wherever else the guest goes, it always has the spot it walked in to to come back to.
        let mut stops = vec![TourStop {
            at: stage.spot,
            look: stage.spot,
            interest: TourInterest::Ground,
            depth: 0.0,
        }];
        let Some(monitor) = desktop
            .monitors
            .iter()
            .find(|monitor| monitor.id == stage.monitor_id)
        else {
            return stops;
        };
        let policy = &self.save.settings.habitat;
        let display_scale = self.save.settings.display_scale;
        let frame = stage.frame;
        // One shelter pixel in desktop points, taken from the frame the spacing rules already
        // measure a standing creature by rather than worked out over again here.
        let scale = frame / CREATURE_FRAME_WIDTH;
        let clear = stage.clear;
        let half = RESTING_WIDTH / 2.0 * scale;
        let underfoot = OBJECT_WIDTH / 2.0 * scale;
        // A display that leaves a creature no width at all leaves nowhere to walk to either.
        if !clear.is_finite() || clear <= 0.0 {
            return stops;
        }
        let ground_y = stage.spot.y;
        let on_ground = |x: f32| Point { x, y: ground_y };
        // Beside something, on the side the guest comes from, `reach` frames off it.
        let beside = |at: Point, reach: f32| {
            let side = if stage.spot.x >= at.x { 1.0 } else { -1.0 };
            on_ground(at.x + side * reach * frame)
        };

        let cottages = colony_cottages(&self.save.creatures);
        let kept = self.save.objects.objects.len().min(MAX_COLONY_OBJECTS);
        let belongings: Vec<Point> = home_object_positions(
            &self.save.home,
            &cottages,
            &desktop.monitors,
            policy,
            display_scale,
        )[..kept]
            .iter()
            .flatten()
            .filter(|(monitor_id, _)| *monitor_id == monitor.id)
            .map(|(_, point)| *point)
            .collect();
        let ground_items: Vec<(GroundItem, Point)> = home_ground_positions(
            &self.save.home,
            &cottages,
            &desktop.monitors,
            policy,
            display_scale,
        )
        .into_iter()
        .filter(|(_, monitor_id, _)| *monitor_id == monitor.id)
        .map(|(item, _, at)| (item, at))
        .collect();
        let middle = monitor.usable_bounds.x + monitor.usable_bounds.width / 2.0;

        // Everything a guest could go to, a kind of place to each list.
        let mut spots = Vec::new();
        let mut gardens = Vec::new();
        let mut keepsakes = Vec::new();
        for (item, at) in ground_items.iter().copied() {
            match item {
                GroundItem::Hangout(kind) => {
                    let open_right = middle >= at.x;
                    let (x, facing_right) = home::hangout_stand(
                        home::FreeHangout {
                            kind,
                            at,
                            open_right,
                        },
                        stage.spot,
                        frame,
                    );
                    // A lookout is looked out from, over the open desktop; anything stood at
                    // is turned to; a cushion or a blanket is simply sat on.
                    let look = match facing_right {
                        Some(right) if kind == HangoutKind::Lookout => {
                            on_ground(x + if right { frame } else { -frame })
                        }
                        Some(_) => at,
                        None => on_ground(x),
                    };
                    spots.push(TourStop {
                        at: on_ground(x),
                        look,
                        interest: TourInterest::Hangout(kind),
                        depth: 0.0,
                    });
                }
                GroundItem::Garden(_) => gardens.push(TourStop {
                    at: beside(at, BESIDE_REACH),
                    look: at,
                    interest: TourInterest::Garden,
                    depth: 0.0,
                }),
                GroundItem::Ornament(_) => keepsakes.push(TourStop {
                    at: beside(at, BESIDE_REACH),
                    look: at,
                    interest: TourInterest::Keepsake,
                    depth: 0.0,
                }),
            }
        }
        for at in belongings.iter().copied() {
            keepsakes.push(TourStop {
                at: beside(at, BESIDE_REACH),
                look: at,
                interest: TourInterest::Keepsake,
                depth: 0.0,
            });
        }
        // Up the path to a door set back from the front, to stand on the step and look up.
        let doorsteps: Vec<TourStop> = stage
            .paths
            .doorsteps()
            .map(|door| TourStop {
                at: on_ground(door.x),
                look: on_ground(door.x),
                interest: TourInterest::House,
                depth: door.depth,
            })
            .collect();
        // Somewhere along the trodden path in the middle that no house stands in front of,
        // leaning the way this guest leans.
        let lean = f32::from(SeedStream::new(self.visit_seed()).bytes("visit-tour-v2", 0)[0])
            / f32::from(u8::MAX);
        let path: Vec<TourStop> = stage
            .paths
            .path_span()
            .into_iter()
            .flat_map(|(low, high)| {
                (0..PATH_TRIES).map(move |step| {
                    let along = (lean + step as f32 / PATH_TRIES as f32).fract();
                    low + (high - low) * along
                })
            })
            .filter(|x| !stage.paths.hidden(*x, village_paths::PATH_DEPTH, half))
            .take(1)
            .map(|x| TourStop {
                at: on_ground(x),
                look: on_ground(x),
                interest: TourInterest::Path,
                depth: village_paths::PATH_DEPTH,
            })
            .collect();

        // Nothing proposed above is taken on trust: a stop that does not plainly keep the rules a
        // body standing in the village keeps is dropped rather than shaved until it does.
        let (low, high) = (stage.region.x + half, stage.region.right() - half);
        let fits = |stop: &TourStop| {
            // The one spot a guest has come to use is the one thing on the ground it may stand on.
            let using = |item: GroundItem| {
                matches!((stop.interest, item),
                    (TourInterest::Hangout(a), GroundItem::Hangout(b)) if a == b)
            };
            let off = |at: Point| (stop.at.x - at.x).abs() >= underfoot;
            stage.region.contains(stop.at)
                && (low..=high).contains(&stop.at.x)
                && stop.depth <= stage.paths.open_depth(stop.at.x, half) + 0.01
                && (stop.depth > 0.0
                    || (belongings.iter().copied().all(off)
                        && ground_items
                            .iter()
                            .all(|(item, at)| using(*item) || off(*at))))
        };

        // One of each kind of place, in the guest's own order within each kind, then a second
        // round for whatever kinds have more, until the tour is full.
        let mut kinds = [spots, doorsteps, path, gardens, keepsakes];
        for (index, kind) in kinds.iter_mut().enumerate() {
            kind.retain(fits);
            let stream = SeedStream::new(self.visit_seed());
            kind.sort_by_key(|stop| {
                stream.bytes(
                    "visit-tour-order-v1",
                    ((index as u64) << 32) | stop.at.x.to_bits() as u64,
                )[0]
            });
        }
        let mut chosen: Vec<TourStop> = Vec::with_capacity(MAX_TOUR_STOPS - 1);
        let deepest = kinds.iter().map(Vec::len).max().unwrap_or(0);
        'rounds: for round in 0..deepest {
            for kind in &kinds {
                if chosen.len() + 1 >= MAX_TOUR_STOPS {
                    break 'rounds;
                }
                let Some(stop) = kind.get(round) else {
                    continue;
                };
                // Two stops a face-clear step apart at most would be one place twice.
                if stops
                    .iter()
                    .chain(&chosen)
                    .any(|other| spacing::closer_than(other.at.x - stop.at.x, clear))
                {
                    continue;
                }
                chosen.push(*stop);
            }
        }
        // In the order the walk meets them, going away from the spot the guest came in at.
        chosen.sort_by(|a, b| {
            (a.at.x - stage.spot.x)
                .abs()
                .total_cmp(&(b.at.x - stage.spot.x).abs())
        });
        stops.extend(chosen);
        stops
    }

    /// The seed a visit is authored from: the guest's own, so one guest tours a village the same
    /// way every gathering of its stay and no two guests tour it alike.
    fn visit_seed(&self) -> [u8; 32] {
        self.save
            .visitors
            .guest
            .as_ref()
            .map_or([0; 32], |guest| guest.creature.origin.source_colony_seed)
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
        1 => (ActionKind::Eat, VISITS.beat_secs),
        3 => (ActionKind::PresentDiscovery, VISITS.beat_secs),
        5 if !reduce_motion => (ActionKind::SoloPlay, VISITS.beat_secs),
        7 => (ActionKind::Perch, VISITS.beat_secs),
        _ => (ActionKind::Idle, VISITS.still_secs),
    }
}

/// One resident's answer: when it turns, how long it holds, and what its temperament makes of
/// the moment. `index` staggers a round of them, so the colony answers in ones and twos.
pub(super) fn resident_answer(
    creature: &Creature,
    index: usize,
    reduce_motion: bool,
) -> ResidentAnswer {
    let traits = creature.traits();
    let kind = creature.temperament().kind;
    let has = |any: &[Trait]| traits.iter().any(|t| any.contains(t));
    // A guardian keeps an eye on the stranger, a shy one hides behind its paws, a show-off shows
    // off and a grump huffs. Otherwise a playful one bounces where it stands, a bold one waves
    // back, and a timid one only looks. Reduced motion leaves everyone looking, bubbles and all.
    let (gesture, bubble) = if has(&[Trait::Suspicious, Trait::Watchful, Trait::Protective])
        || kind == TemperamentKind::Guardian
    {
        (Some(Gesture::Watch), Some(BubbleIcon::Watching))
    } else if has(&[Trait::Shy]) || kind == TemperamentKind::Wallflower {
        (Some(Gesture::Peek), Some(BubbleIcon::Blush))
    } else if has(&[Trait::Vain, Trait::AttentionSeeking]) || kind == TemperamentKind::Showoff {
        (Some(Gesture::Strut), Some(BubbleIcon::Sparkle))
    } else if has(&[Trait::Grumpy, Trait::Irritable]) || kind == TemperamentKind::Grump {
        (Some(Gesture::Huff), Some(BubbleIcon::Grumble))
    } else if creature.personality.playfulness >= 0.6 {
        (Some(Gesture::Bop), None)
    } else if creature.personality.boldness >= 0.35 {
        (Some(Gesture::Reach), None)
    } else {
        (None, None)
    };
    ResidentAnswer {
        creature_id: creature.id,
        // The sociable ones look up first and the rest follow, a beat apart.
        after: 0.4 + index as f32 * 0.9 + (1.0 - creature.personality.sociability) * 2.2,
        hold: VISITS.answer_hold_secs + creature.personality.sociability * 1.4,
        gesture: gesture.filter(|_| !reduce_motion),
        bubble,
    }
}

/// A resident's answer to a guest it has met before: a familiar face is waved to rather than
/// watched, huffed at or hidden from, and greeted with a hello. Its timing is its own, as ever.
pub(super) fn recognised(answer: ResidentAnswer, reduce_motion: bool) -> ResidentAnswer {
    let gesture = match answer.gesture {
        Some(Gesture::Watch | Gesture::Huff | Gesture::Peek) | None => Some(Gesture::Reach),
        other => other,
    };
    ResidentAnswer {
        gesture: gesture.filter(|_| !reduce_motion),
        bubble: Some(BubbleIcon::Hello),
        ..answer
    }
}

/// Walk the guest on round the village, or hold it at the stop it has reached. Returns the one
/// resident it has just gone over to greet, on the tick the greeting starts, so the colony can
/// answer it in its own time.
fn tour(
    guest: &mut Visitor,
    stage: &GuestStage,
    (residents, crowd): (&Residents, &Crowd),
    seed: [u8; 32],
    dt: f32,
    reduce_motion: bool,
    roam: bool,
) -> Option<CreatureId> {
    // Time to be heading back: the goodbye is said where the guest walked in, so the walk out
    // is the walk in run backwards rather than a dash from the far end of the village.
    if !roam && guest.visit.stop != 0 {
        guest.visit.stop = 0;
        guest.visit.moment = TourMoment::Walking;
        guest.visit.stay = VISITS.max_walk_secs;
    }
    let index = usize::from(guest.visit.stop).min(guest.visit.stops.len() - 1);
    let stop = guest.visit.stops[index];
    guest.visit.stay -= dt;
    if guest.visit.moment == TourMoment::Walking {
        act(&mut guest.creature, ActionKind::Traverse);
        // Along the strip and up or down the paths to the doors, as a resident goes, with the
        // same patience with a walk that was never going to finish.
        if !walk(&mut guest.creature, &stage.paths, stop.at, stop.depth, dt)
            && guest.visit.stay > 0.0
        {
            return None;
        }
        place(guest, stop.at, stage);
        guest.creature.state.depth = stop.depth;
        return arrive(guest, stop, residents, seed);
    }
    place(guest, stop.at, stage);
    guest.creature.state.depth = stop.depth;
    hold(guest, stop, residents, stage.frame, dt, reduce_motion);
    if guest.visit.stay > 0.0 {
        return None;
    }
    guest.creature.state.attention = None;
    if roam {
        // On to the next place in the village that nobody is standing about at, busy at or
        // headed for. With all of them spoken for just now, a little longer here, or wherever
        // the walk in left it, and then ask again.
        match next_stop(&guest.visit.stops, index, crowd, stage.clear) {
            Some(next) => {
                guest.visit.stop = next as u8;
                guest.visit.moment = TourMoment::Walking;
                guest.visit.stay = VISITS.max_walk_secs;
            }
            None => {
                guest.visit.moment = TourMoment::Resting;
                guest.visit.stay = VISITS.tour_beat_secs;
            }
        }
    } else {
        // Nowhere left to go before the goodbye: the guest settles where it walked in.
        guest.visit.moment = TourMoment::Resting;
        guest.visit.stay = VISITS.tour_stay_secs - VISITS.tour_beat_secs;
    }
    None
}

/// Which stop out in the village the guest goes on to from stop `index`: the next one round the
/// ring that nobody is standing about at, strolling to or busy at. The spot it walked in to is not
/// on the ring, since that is for the goodbye, and nor is the stop it is at: with nowhere else
/// free just now it stays a little longer and asks again.
fn next_stop(stops: &[TourStop], index: usize, crowd: &Crowd, clear: f32) -> Option<usize> {
    let ring = stops.len().checked_sub(1).filter(|ring| *ring > 0)?;
    let from = if index == 0 { 0 } else { index % ring };
    (0..ring)
        .map(|step| 1 + (from + step) % ring)
        .filter(|candidate| *candidate != index)
        .find(|candidate| {
            let at = stops[*candidate].at;
            !crowd
                .iter()
                .flatten()
                .any(|point| spacing::closer_than(point.x - at.x, clear))
        })
}

/// The guest has reached a stop. Decide the one small thing it does here.
fn arrive(
    guest: &mut Visitor,
    stop: TourStop,
    residents: &Residents,
    seed: [u8; 32],
) -> Option<CreatureId> {
    guest.visit.stay = VISITS.tour_stay_secs;
    guest.creature.state.velocity = Point::default();
    // The nearest resident this guest has not been over to yet, if it can see one from here.
    // Meeting somebody new is the whole point of walking over, so it wins every time.
    let met = residents
        .iter()
        .flatten()
        .filter(|(creature_id, _)| !guest.visit.met.contains(creature_id))
        .filter(|(_, point)| point.distance(stop.at) <= VISITS.notice_distance)
        .min_by(|a, b| a.1.distance(stop.at).total_cmp(&b.1.distance(stop.at)))
        .map(|(creature_id, _)| *creature_id);
    if let Some(creature_id) = met
        && guest.visit.met.len() < MAX_COLONY_CREATURES
    {
        guest.visit.met.push(creature_id);
        guest.visit.moment = TourMoment::Greeting(creature_id);
        return Some(creature_id);
    }
    // Otherwise a spot is always used, since that is what it came over for; anything else is
    // mostly a proper look, and now and then a breather.
    let roll = SeedStream::new(seed).bytes("visit-tour-beat-v1", u64::from(guest.visit.beat))[0];
    guest.visit.moment = match stop.interest {
        TourInterest::Ground => TourMoment::Resting,
        TourInterest::Hangout(_) => TourMoment::Looking,
        _ if roll.is_multiple_of(4) => TourMoment::Resting,
        _ => TourMoment::Looking,
    };
    None
}

/// How long the one small thing a guest came over to a stop for lasts, before the calm moments
/// a visit has always had: a spot is used for a good while, anything else is a look.
fn thing_secs(interest: TourInterest) -> f32 {
    match interest {
        TourInterest::Hangout(_) => VISITS.tour_use_secs,
        _ => VISITS.tour_beat_secs,
    }
}

/// Hold the guest at a stop: the one small thing it came over to do, and then the same calm
/// moments a visit has always had until it is time to move on.
fn hold(
    guest: &mut Visitor,
    stop: TourStop,
    residents: &Residents,
    frame: f32,
    dt: f32,
    reduce_motion: bool,
) {
    // Hello said to whoever was here, a spot is still used: the stay starts over for it.
    if matches!(guest.visit.moment, TourMoment::Greeting(_))
        && matches!(stop.interest, TourInterest::Hangout(_))
        && guest.visit.stay <= VISITS.tour_stay_secs - VISITS.tour_beat_secs
    {
        guest.visit.moment = TourMoment::Looking;
        guest.visit.stay = VISITS.tour_stay_secs;
    }
    let thing = thing_secs(stop.interest);
    if guest.visit.stay <= VISITS.tour_stay_secs - thing {
        calm_moment(guest, stop.at, dt, reduce_motion);
        return;
    }
    let held = VISITS.tour_stay_secs - guest.visit.stay;
    let remaining = guest.visit.stay - (VISITS.tour_stay_secs - thing);
    let creature = &mut guest.creature;
    match guest.visit.moment {
        TourMoment::Greeting(creature_id) => {
            let target = residents
                .iter()
                .flatten()
                .find(|(resident, _)| *resident == creature_id)
                .map_or(stop.look, |(_, point)| *point);
            creature.state.facing_right = target.x >= creature.state.position.x;
            act(creature, ActionKind::Greet);
            creature.state.attention = pose(target, None);
        }
        TourMoment::Looking => {
            if stop.look.x != stop.at.x {
                creature.state.facing_right = stop.look.x >= creature.state.position.x;
            }
            match stop.interest {
                // Used the way a resident uses it: a nap on the cushion, a snack on the blanket,
                // a look out from the lookout, a seat on the bench.
                TourInterest::Hangout(kind) => {
                    let (_, action, _, _) = home::hangout_moment(kind, creature);
                    let action = if reduce_motion && action == ActionKind::SoloPlay {
                        ActionKind::Idle
                    } else {
                        action
                    };
                    if home::seated_at(kind) {
                        let stance =
                            home::sit_at(creature, kind, stop.at.y, held, remaining, frame);
                        act(creature, stance.unwrap_or(action));
                    } else {
                        act(creature, action);
                        creature.state.attention = (action == ActionKind::InspectScreen)
                            .then(|| pose(stop.look, None))
                            .flatten();
                    }
                }
                // Up at the house whose step it is standing on, in at a garden patch, or down at
                // whatever the colony has left on the grass or under its tree.
                interest => {
                    let (action, gesture) = match interest {
                        TourInterest::House | TourInterest::Garden => {
                            (ActionKind::InspectScreen, None)
                        }
                        TourInterest::Keepsake => {
                            (ActionKind::PresentDiscovery, Some(Gesture::Watch))
                        }
                        _ => (ActionKind::Idle, None),
                    };
                    act(creature, action);
                    creature.state.attention =
                        pose(stop.look, (!reduce_motion).then_some(gesture).flatten());
                }
            }
        }
        TourMoment::Resting | TourMoment::Walking => {
            act(creature, ActionKind::Perch);
            creature.state.attention = None;
        }
    }
}

/// One of the calm moments a visit is made of, shown wherever the guest happens to be standing,
/// and the next one along once this one has run its own length.
fn calm_moment(guest: &mut Visitor, _at: Point, dt: f32, reduce_motion: bool) {
    guest.visit.beat_remaining -= dt;
    if guest.visit.beat_remaining <= 0.0 {
        guest.visit.beat = (guest.visit.beat + 1) % 8;
        guest.visit.beat_remaining = shared_moment(guest.visit.beat, reduce_motion).1;
    }
    let action = shared_moment(guest.visit.beat, reduce_motion).0;
    act(&mut guest.creature, action);
    // None of them has a pose of its own: the snack is a real meal and the dance a real game,
    // which a pose over the top would hide.
    guest.creature.state.attention = None;
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
    guest.creature.state.depth = 0.0;
}

/// Stand the guest somewhere else on the village ground line, holding on to the floor exactly
/// the way the guest spot itself does.
fn place(guest: &mut Visitor, at: Point, stage: &GuestStage) {
    let relative_x = ((at.x - stage.region.x) / stage.region.width.max(1.0)).clamp(0.0, 1.0);
    stand(&mut guest.creature, at, stage.monitor_id, relative_x);
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

/// How fast a guest walks, in points a second: a little brisker than a resident's stroll, since
/// it has a whole village to see in one visit.
fn pace(creature: &Creature) -> f32 {
    24.0 + creature.personality.activity * 34.0
}

/// One step of a walk round the village to `goal`, `depth` back: along the strip, and up or down
/// the paths to the doors where the depth changes, exactly as a resident goes. Returns whether
/// the walk is finished.
fn walk(
    creature: &mut Creature,
    paths: &village_paths::VillagePaths,
    goal: Point,
    depth: f32,
    dt: f32,
) -> bool {
    let previous = creature.state.position;
    let there = paths.walk_to(creature, goal, depth, pace(creature), dt);
    creature.state.velocity = if dt > 0.0 && !there {
        Point {
            x: (creature.state.position.x - previous.x) / dt,
            y: (creature.state.position.y - previous.y) / dt,
        }
    } else {
        Point::default()
    };
    there
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
    let speed = pace(creature);
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
