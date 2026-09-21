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

/// How long a guest stays at one stop on its walk around the village, and how much of that is
/// the one small thing it came over to do. The rest of the stay is the same calm moments a visit
/// has always had, only somewhere new each time: near enough a full round of them, so a guest
/// strolls the length of the houses a handful of times in a gathering rather than pacing it.
const TOUR_STAY_SECS: f32 = 72.0;
const TOUR_BEAT_SECS: f32 = 8.0;
/// How much of a gathering is kept back for the walk back to the spot the guest came in at, so
/// the goodbye is said there and the walk out is the walk in run backwards rather than a dash
/// from the far end of the village.
const TOUR_RETURN_SECS: f32 = 45.0;
/// How many spans of ground a fifth body is kept off: a doorway and a resting resident for each
/// of the colony, and the patch each of its things is sitting on.
const MAX_BLOCKED: usize = 2 * MAX_COLONY_CREATURES + MAX_COLONY_OBJECTS;
/// The most answers one visit can be holding: one per resident for the hello, and one more each
/// for the walk round, since a guest only ever goes over to somebody it has not met yet.
const MAX_VISIT_ANSWERS: usize = 2 * MAX_COLONY_CREATURES;

/// The residents a guest can see from where it is standing. On the stack: a colony is never more
/// than four, and this is rebuilt on every tick of a visit.
type Residents = [Option<(CreatureId, Point)>; MAX_COLONY_CREATURES];

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
        // Who the guest can go over to, gathered before it is borrowed: a colony is never more
        // than four, so this costs nothing and never allocates.
        let mut residents: Residents = [None; MAX_COLONY_CREATURES];
        for (slot, entry) in residents.iter_mut().enumerate() {
            *entry = self
                .save
                .creatures
                .get(slot)
                .filter(|creature| creature.state.arrival_delay_secs <= 0.0)
                .map(|creature| (creature.id, creature.state.position));
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
        let roam = remaining > DEPARTURE_LEAD_SECS + TOUR_RETURN_SECS;

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
            guest.visit.stay = MAX_WALK_SECS;
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
                    // And off around the houses, if the village left anywhere to go.
                    guest.visit.stop = u8::from(guest.visit.stops.len() > 1);
                    guest.visit.moment = TourMoment::Walking;
                    guest.visit.stay = MAX_WALK_SECS;
                }
            }
            VisitPhase::Visiting => {
                if remaining <= DEPARTURE_LEAD_SECS {
                    // A tour that has run long simply stops touring.
                    enter(guest, VisitPhase::Farewell, ActionKind::Greet);
                } else if guest.visit.stops.len() > 1 {
                    met = tour(guest, &stage, &residents, seed, dt, reduce_motion, roam);
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
                place(guest, here, &stage);
                act(&mut guest.creature, ActionKind::Greet);
                guest.creature.state.attention =
                    pose(here, (!reduce_motion).then_some(Gesture::Reach));
                if guest.visit.elapsed >= FAREWELL_SECS {
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
        let standing = guest.creature.state.position;

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
            .map(|(index, creature)| resident_answer(creature, index, reduce_motion))
            .collect()
    }

    /// One resident answers a hello said at a stop on the walk round. The colony's own machinery
    /// asked for a single answer rather than a round of them: a turn, a hold, and whatever its
    /// temperament makes of the moment. It never leaves its door, and nothing is written down.
    fn answer_the_greeting(&mut self, creature_id: CreatureId) {
        let reduce_motion = self.save.settings.reduce_motion;
        let Some(answer) = self
            .save
            .creatures
            .iter()
            .find(|creature| creature.id == creature_id)
            .map(|creature| resident_answer(creature, 0, reduce_motion))
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
            region,
        })
    }

    /// Where a guest's walk around the village stops.
    ///
    /// Every measurement here is asked of the village's own layout rather than written down
    /// again, so a tour follows whatever the houses, the porches and the tree's yard do. The
    /// first stop is always the spot the guest walked in to; the rest are found by walking the
    /// ground line from there inward past the houses, taking the first place a body may
    /// legitimately stand and then the next one a clear frame beyond it.
    ///
    /// A stop has to be inside the one accessible region the village stands in, clear of every
    /// doorway by the sliver of wall a resting frame is allowed to reach across, off the things
    /// the colony has set down, and no closer to a resting resident than two residents may
    /// stand to each other. A guest is a fifth body in a village laid out for four, so most of
    /// the strip fails one of those: a stop that cannot keep its distance is dropped rather than
    /// crowding somebody, and a short tour is what a full village gets.
    fn plan_tour(&self, stage: &GuestStage, desktop: &DesktopSnapshot) -> Vec<TourStop> {
        // Wherever else the guest goes, it always has the spot it walked in to to come back to.
        let mut stops = vec![TourStop {
            at: stage.spot,
            look: stage.spot,
            interest: TourInterest::Ground,
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
        // One shelter pixel in desktop points, taken from the frame the spacing rules already
        // measure a standing creature by rather than worked out over again here.
        let scale =
            spacing::frame_width(display_scale, monitor.scale_factor) / CREATURE_FRAME_WIDTH;
        let cottages = colony_cottages(&self.save.creatures);
        let half = CREATURE_FRAME_WIDTH / 2.0 * scale;
        let clear = CREATURE_FRAME_WIDTH * REST_CLEAR_RATIO * scale;
        let underfoot = OBJECT_WIDTH / 2.0 * scale;
        // A display that leaves a creature no width at all leaves nowhere to walk to either.
        if !clear.is_finite() || clear <= 0.0 {
            return stops;
        }

        let mut houses = [None; MAX_COLONY_CREATURES];
        for (slot, house) in houses.iter_mut().enumerate() {
            let Some((_, point)) = home_dwelling_position(
                &self.save.home,
                slot,
                &cottages,
                &desktop.monitors,
                policy,
                display_scale,
            ) else {
                continue;
            };
            let kind = if slot == 0 {
                DwellingKind::Main
            } else {
                match cottages.get(slot - 1) {
                    Some(kind) => *kind,
                    None => continue,
                }
            };
            // As close to a house as a body may come without standing over its door: the
            // outermost sliver of wall a resting frame is allowed to reach across, and no more.
            // It is the same arithmetic that puts a resident's own porch where it is.
            let reach =
                (kind.width() / 2.0 + CREATURE_FRAME_WIDTH / 2.0 - REST_WALL_SLIVER) * scale;
            *house = Some((point, reach));
        }
        let mut resting = [None; MAX_COLONY_CREATURES];
        for (slot, spot) in resting.iter_mut().enumerate() {
            *spot = home_resting_position(
                &self.save.home,
                slot,
                &cottages,
                &desktop.monitors,
                policy,
                display_scale,
            )
            .map(|(_, point)| point);
        }
        let places = home_object_positions(
            &self.save.home,
            &cottages,
            &desktop.monitors,
            policy,
            display_scale,
        );
        let kept = self.save.objects.objects.len().min(MAX_COLONY_OBJECTS);
        let things = &places[..kept];

        // Which way the village lies from the spot the guest walked in to, taken from the lot
        // furthest from it. The tour only ever goes that way: a guest touring the open floor on
        // the other side would be standing to one side again, which is the thing a tour is for.
        let Some(inward) = houses
            .iter()
            .flatten()
            .map(|(point, _)| point.x)
            .chain(resting.iter().flatten().map(|point| point.x))
            .map(|x| x - stage.spot.x)
            .max_by(|a, b| a.abs().total_cmp(&b.abs()))
            .filter(|reach| reach.abs() >= clear)
            .map(f32::signum)
        else {
            return stops;
        };
        // Everywhere a fifth body may not stand, as spans of the ground line: the doorway of
        // each house and the wall either side of it, the elbow room each resting resident
        // keeps, and the patch of ground each of the colony's things sits on.
        let mut blocked = [(0.0, 0.0); MAX_BLOCKED];
        let mut count = 0;
        for (point, reach) in houses.iter().flatten() {
            blocked[count] = (point.x - reach, point.x + reach);
            count += 1;
        }
        for point in resting.iter().flatten() {
            blocked[count] = (point.x - clear, point.x + clear);
            count += 1;
        }
        for (_, point) in things.iter().flatten() {
            blocked[count] = (point.x - underfoot, point.x + underfoot);
            count += 1;
        }
        let blocked = &mut blocked[..count];
        blocked.sort_by(|a, b| a.0.total_cmp(&b.0));

        // The ground the tour runs over: from a clear frame past the spot the guest walked in
        // to, inward past the houses, as far as the village's own region reaches.
        let start = stage.spot.x + inward * clear;
        let far = if inward > 0.0 {
            stage.region.right() - half
        } else {
            stage.region.x + half
        };
        let (low, high) = if inward > 0.0 {
            (start, far)
        } else {
            (far, start)
        };
        // Where in a piece of free ground a guest comes to rest is its own: two guests tour the
        // same village the same way every time, but not quite the same way as each other. It
        // never leans all the way to one end, so a stop is never balanced on the very edge of
        // the ground it was given.
        let lean =
            f32::from(2 + SeedStream::new(self.visit_seed()).bytes("visit-tour-v1", 0)[0] % 13)
                / 16.0;
        let mut found = [0.0; MAX_TOUR_STOPS];
        let mut room = 0;
        let mut cursor = low;
        for (from, to) in blocked.iter().copied().chain([(high, high)]) {
            if to <= cursor {
                continue;
            }
            // A stretch of free ground: one body fits in it, and another every clear frame
            // beyond that. They sit together inside it, leaning the way this guest leans, so
            // nobody ends up standing on the edge of somebody else's elbow room.
            let gap = from.min(high) - cursor;
            if gap > 0.0 && room < MAX_TOUR_STOPS {
                let fit = ((gap / clear).min(MAX_TOUR_STOPS as f32) as usize + 1)
                    .min(MAX_TOUR_STOPS - room);
                let spread = (fit - 1) as f32 * clear;
                let first = cursor + (gap - spread).max(0.0) * lean;
                for step in 0..fit {
                    found[room] = first + step as f32 * clear;
                    room += 1;
                }
            }
            cursor = cursor.max(to);
            if cursor >= high {
                break;
            }
        }

        // In the order the walk meets them, counting outward from the spot the guest came in at.
        let found = &mut found[..room];
        if inward < 0.0 {
            found.reverse();
        }
        for x in found.iter().copied() {
            if stops.len() >= MAX_TOUR_STOPS {
                break;
            }
            let point = Point { x, y: stage.spot.y };
            // Nothing the arithmetic above proposed is taken on trust. Finding the free ground
            // is one thing; the rules a fifth body has to keep are another, and a stop that
            // does not plainly keep them is dropped rather than shaved until it does.
            if !stage.region.contains(point)
                || houses
                    .iter()
                    .flatten()
                    .any(|(house, reach)| (point.x - house.x).abs() < *reach)
                || resting
                    .iter()
                    .flatten()
                    .any(|spot| (point.x - spot.x).abs() < clear)
                || things
                    .iter()
                    .flatten()
                    .any(|(_, thing)| (point.x - thing.x).abs() < underfoot)
            {
                continue;
            }
            // What the guest has come over for: whichever of the colony's things or its houses
            // is nearest the spot the free ground let it stand in.
            let thing = nearest(things.iter().flatten().map(|(_, point)| *point), point);
            let house = nearest(houses.iter().flatten().map(|(point, _)| *point), point);
            let (look, interest) = match (thing, house) {
                (Some(thing), Some(house))
                    if (thing.x - point.x).abs() <= (house.x - point.x).abs() =>
                {
                    (thing, TourInterest::Keepsake)
                }
                (Some(thing), None) => (thing, TourInterest::Keepsake),
                (_, Some(house)) => (house, TourInterest::House),
                (None, None) => (point, TourInterest::Ground),
            };
            stops.push(TourStop {
                at: point,
                look,
                interest,
            });
        }
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

/// The nearest of a handful of places to a point along the ground line.
fn nearest(places: impl Iterator<Item = Point>, to: Point) -> Option<Point> {
    places.min_by(|a, b| (a.x - to.x).abs().total_cmp(&(b.x - to.x).abs()))
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

/// One resident's answer: when it turns, how long it holds, and what its temperament makes of
/// the moment. `index` staggers a round of them, so the colony answers in ones and twos.
fn resident_answer(creature: &Creature, index: usize, reduce_motion: bool) -> ResidentAnswer {
    ResidentAnswer {
        creature_id: creature.id,
        // The sociable ones look up first and the rest follow, a beat apart.
        after: 0.4 + index as f32 * 0.9 + (1.0 - creature.personality.sociability) * 2.2,
        hold: ANSWER_HOLD_SECS + creature.personality.sociability * 1.4,
        // A playful one bounces where it stands, a bold one waves back, and a timid one only
        // looks. Reduced motion leaves everyone looking.
        gesture: if reduce_motion {
            None
        } else if creature.personality.playfulness >= 0.6 {
            Some(Gesture::Bop)
        } else if creature.personality.boldness >= 0.35 {
            Some(Gesture::Reach)
        } else {
            None
        },
    }
}

/// Walk the guest on round the village, or hold it at the stop it has reached. Returns the one
/// resident it has just gone over to greet, on the tick the greeting starts, so the colony can
/// answer it in its own time.
fn tour(
    guest: &mut Visitor,
    stage: &GuestStage,
    residents: &Residents,
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
        guest.visit.stay = MAX_WALK_SECS;
    }
    let index = usize::from(guest.visit.stop).min(guest.visit.stops.len() - 1);
    let stop = guest.visit.stops[index];
    guest.visit.stay -= dt;
    if guest.visit.moment == TourMoment::Walking {
        act(&mut guest.creature, ActionKind::Traverse);
        // The same bounded step the walk in uses, and the same patience with a walk that was
        // never going to finish.
        if !step_toward(&mut guest.creature, stop.at, dt) && guest.visit.stay > 0.0 {
            return None;
        }
        place(guest, stop.at, stage);
        return arrive(guest, stop, residents, seed);
    }
    place(guest, stop.at, stage);
    hold(guest, stop, residents, dt, reduce_motion);
    if guest.visit.stay > 0.0 {
        return None;
    }
    if roam {
        // On to the next: the ring runs out along the houses and back again.
        guest.visit.stop = ((index + 1) % guest.visit.stops.len()) as u8;
        guest.visit.moment = TourMoment::Walking;
        guest.visit.stay = MAX_WALK_SECS;
        guest.creature.state.attention = None;
    } else {
        // Nowhere left to go before the goodbye: the guest settles where it walked in.
        guest.visit.moment = TourMoment::Resting;
        guest.visit.stay = TOUR_STAY_SECS - TOUR_BEAT_SECS;
    }
    None
}

/// The guest has reached a stop. Decide the one small thing it does here.
fn arrive(
    guest: &mut Visitor,
    stop: TourStop,
    residents: &Residents,
    seed: [u8; 32],
) -> Option<CreatureId> {
    guest.visit.stay = TOUR_STAY_SECS;
    guest.creature.state.velocity = Point::default();
    // The nearest resident this guest has not been over to yet, if it can see one from here.
    // Meeting somebody new is the whole point of walking over, so it wins every time.
    let met = residents
        .iter()
        .flatten()
        .filter(|(creature_id, _)| !guest.visit.met.contains(creature_id))
        .filter(|(_, point)| point.distance(stop.at) <= NOTICE_DISTANCE)
        .min_by(|a, b| a.1.distance(stop.at).total_cmp(&b.1.distance(stop.at)))
        .map(|(creature_id, _)| *creature_id);
    if let Some(creature_id) = met
        && guest.visit.met.len() < MAX_COLONY_CREATURES
    {
        guest.visit.met.push(creature_id);
        guest.visit.moment = TourMoment::Greeting(creature_id);
        return Some(creature_id);
    }
    // Otherwise, mostly a proper look at whatever it came over for; now and then a breather.
    let roll = SeedStream::new(seed).bytes("visit-tour-beat-v1", u64::from(guest.visit.beat))[0];
    guest.visit.moment = if stop.interest == TourInterest::Ground || roll.is_multiple_of(4) {
        TourMoment::Resting
    } else {
        TourMoment::Looking
    };
    None
}

/// Hold the guest at a stop: the one small thing it came over to do, and then the same calm
/// moments a visit has always had until it is time to move on.
fn hold(guest: &mut Visitor, stop: TourStop, residents: &Residents, dt: f32, reduce_motion: bool) {
    if guest.visit.stay <= TOUR_STAY_SECS - TOUR_BEAT_SECS {
        calm_moment(guest, stop.at, dt, reduce_motion);
        return;
    }
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
            creature.state.facing_right = stop.look.x >= creature.state.position.x;
            // Up at the house it is standing beside, or down at whatever the colony has left
            // under its tree.
            let (action, gesture) = match stop.interest {
                TourInterest::House => (ActionKind::InspectScreen, None),
                TourInterest::Keepsake => (ActionKind::PresentDiscovery, Some(Gesture::Watch)),
                TourInterest::Ground => (ActionKind::Idle, None),
            };
            act(creature, action);
            creature.state.attention =
                pose(stop.look, (!reduce_motion).then_some(gesture).flatten());
        }
        TourMoment::Resting | TourMoment::Walking => {
            act(creature, ActionKind::Perch);
            creature.state.attention = None;
        }
    }
}

/// One of the calm moments a visit is made of, shown wherever the guest happens to be standing,
/// and the next one along once this one has run its own length.
fn calm_moment(guest: &mut Visitor, at: Point, dt: f32, reduce_motion: bool) {
    guest.visit.beat_remaining -= dt;
    if guest.visit.beat_remaining <= 0.0 {
        guest.visit.beat = (guest.visit.beat + 1) % 8;
        guest.visit.beat_remaining = shared_moment(guest.visit.beat, reduce_motion).1;
    }
    let action = shared_moment(guest.visit.beat, reduce_motion).0;
    act(&mut guest.creature, action);
    // The dance is the one moment with a pose of its own; the rest are still.
    guest.creature.state.attention = (action == ActionKind::SoloPlay)
        .then(|| pose(at, Some(Gesture::Bop)))
        .flatten();
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
