use super::*;

/// A second offer inside this window is a nudge rather than a new question, and is turned down.
/// It is what keeps a double click from feeding anyone twice.
const PESTER_SECS: i64 = 6;
/// How long a creature stays full after accepting a snack.
const FED_SECS: i64 = 90;
/// How long a creature has had its fill of the toy after accepting one.
const PLAYED_SECS: i64 = 45;
/// Sleep pressure past which nothing held out is interesting.
const TOO_SLEEPY: f32 = 0.82;
/// Sleep pressure below which a sleeping creature has had its rest and can be woken gently.
const RESTED_ENOUGH: f32 = 0.35;
/// Boldness below which a creature takes a beat before it answers.
const TIMID_BOLDNESS: f32 = 0.38;
/// How long that beat lasts.
const THINKING_SECS: f32 = 0.55;
/// A cursor this close is the hand holding the offer out.
const CURSOR_REACH: f32 = 240.0;
/// How many kind and unkind handlings still colour how an offer is received.
const REMEMBERED_HANDLING: u32 = 50;
/// Playfulness below which a toy is simply baffling rather than unwanted.
const BAFFLED_BY_TOYS: f32 = 0.35;

/// The clip a creature is enjoying because it accepted an offer, if any. A visit holds its own
/// timeline still while its guest finishes one, exactly as it does for a pet.
pub(super) fn enjoying(
    offers: &BTreeMap<CreatureId, OfferMemory>,
    creature_id: CreatureId,
) -> Option<ActionKind> {
    offers.get(&creature_id).and_then(|memory| memory.enjoying)
}

/// Everything one creature carries between offers. Runtime-only: none of it is saved, journaled,
/// or counted, and it is dropped the moment that creature is no longer here.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct OfferMemory {
    /// When the last offer was held out, on the colony timeline.
    last_offer_utc: Option<OffsetDateTime>,
    /// When a snack was last accepted.
    fed_utc: Option<OffsetDateTime>,
    /// When a toy was last accepted.
    played_utc: Option<OffsetDateTime>,
    /// How many offers this creature has been made, which walks its decision stream forward so
    /// two offers in the same state are still two separate dice.
    offers: u32,
    /// An answer a timid creature is still working up to.
    pending: Option<PendingAnswer>,
    /// The clip an accepted offer started, held until the creature has finished with it.
    enjoying: Option<ActionKind>,
}

/// An answer already decided, waiting out the beat before it is given.
#[derive(Clone, Copy, Debug)]
struct PendingAnswer {
    kind: OfferKind,
    answer: Answer,
    remaining: f32,
}

/// What the creature makes of it. A decline carries the icon that says why.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Answer {
    Accept,
    Decline(BubbleIcon),
}

impl World {
    /// Hold out a snack to one creature. Returns whether the offer was made at all; whether it is
    /// accepted is the creature's own business.
    pub(super) fn offer_snack(
        &mut self,
        creature_id: CreatureId,
        desktop: &DesktopSnapshot,
    ) -> bool {
        self.hold_out(creature_id, OfferKind::Snack, desktop)
    }

    /// Hold out a toy to one creature.
    pub(super) fn offer_toy(&mut self, creature_id: CreatureId, desktop: &DesktopSnapshot) -> bool {
        self.hold_out(creature_id, OfferKind::Toy, desktop)
    }

    /// Whether anyone is still enjoying something they were given. A colony ritual waits for that
    /// rather than calling the creature away from it mid-mouthful.
    pub(super) fn offer_response_active(&self) -> bool {
        self.offers.values().any(|memory| memory.enjoying.is_some())
    }

    /// Count down the beat a timid creature takes, give the answer when it is up, and let go of a
    /// response the creature has finished with. Runs before the paused and at-home short-circuits
    /// so an offer behaves the same wherever the colony happens to be.
    pub(super) fn tick_offers(&mut self, dt: f32, desktop: &DesktopSnapshot) {
        if self.offers.is_empty() {
            return;
        }
        self.forget_departed_offers();
        let finished: Vec<_> = self
            .offers
            .iter()
            .filter(|(_, memory)| memory.enjoying.is_some())
            .filter(|(creature_id, memory)| {
                self.offer_subject(**creature_id).is_none_or(|creature| {
                    Some(creature.state.action) != memory.enjoying
                        || creature.state.action_elapsed >= creature.state.action_duration
                })
            })
            .map(|(creature_id, _)| *creature_id)
            .collect();
        for creature_id in finished {
            if let Some(memory) = self.offers.get_mut(&creature_id) {
                memory.enjoying = None;
            }
        }
        // A paused colony is not thinking about anything; the beat waits with it.
        if self.save.settings.paused {
            return;
        }
        let mut ready = Vec::new();
        for (creature_id, memory) in &mut self.offers {
            let Some(pending) = &mut memory.pending else {
                continue;
            };
            pending.remaining -= dt.max(0.0);
            if pending.remaining <= 0.0 {
                ready.push((*creature_id, *pending));
            }
        }
        for (creature_id, pending) in ready {
            if let Some(memory) = self.offers.get_mut(&creature_id) {
                memory.pending = None;
            }
            // The moment may have passed while it was thinking. Then there is no answer to give.
            if !self.offer_can_be_made(creature_id) {
                continue;
            }
            self.deliver(creature_id, pending.kind, pending.answer, desktop);
        }
    }

    fn hold_out(
        &mut self,
        creature_id: CreatureId,
        kind: OfferKind,
        desktop: &DesktopSnapshot,
    ) -> bool {
        if !self.offer_can_be_made(creature_id) {
            return false;
        }
        self.forget_departed_offers();
        let now = self.save.maximum_seen_utc;
        let previous = self.offers.get(&creature_id).copied().unwrap_or_default();
        let answer = self.offer_answer(creature_id, kind, now, previous);
        let hesitates = previous.pending.is_none()
            && !self.save.settings.reduce_motion
            && self
                .offer_subject(creature_id)
                .is_some_and(|creature| creature.personality.boldness < TIMID_BOLDNESS);
        let memory = self.offers.entry(creature_id).or_default();
        memory.offers = memory.offers.saturating_add(1);
        memory.last_offer_utc = Some(now);
        memory.pending = hesitates.then_some(PendingAnswer {
            kind,
            answer,
            remaining: THINKING_SECS,
        });
        if hesitates {
            self.show_bubble(creature_id, BubbleIcon::Ellipsis);
            return true;
        }
        self.deliver(creature_id, kind, answer, desktop);
        true
    }

    /// Whether an offer can be held out to this creature at all. When it cannot, nothing is shown
    /// and nothing changes: the menu item does nothing rather than cutting something short.
    fn offer_can_be_made(&self, creature_id: CreatureId) -> bool {
        if !self.save.settings.visible || self.save.settings.paused {
            return false;
        }
        let Some(creature) = self.offer_subject(creature_id) else {
            return false;
        };
        if creature.state.arrival_delay_secs > 0.0 {
            return false;
        }
        // In someone's hand, in the air, part-way through a climb or a squeeze, or hanging off an
        // edge: all of them are the wrong moment, and none of them is safe to cut short.
        if matches!(
            creature.state.action,
            ActionKind::Dragged
                | ActionKind::Tossed
                | ActionKind::ClimbWindow
                | ActionKind::SqueezeWindow
                | ActionKind::Dangle
        ) {
            return false;
        }
        if self
            .interaction
            .as_ref()
            .is_some_and(|interaction| interaction.creature_id == creature_id)
            || self.tosses.contains_key(&creature_id)
            || self.window_journeys.contains_key(&creature_id)
            || self.window_routes.contains_key(&creature_id)
        {
            return false;
        }
        // A colony ritual is the colony's own moment. It is left alone.
        if self.colony_plan.as_ref().is_some_and(|plan| {
            plan.participants
                .iter()
                .any(|participant| participant.creature_id == creature_id)
        }) {
            return false;
        }
        // While the home is out a resident is either walking to its spot or resting at it, and
        // only the second of those is somewhere a snack can be enjoyed. Resting includes the small
        // things a resident does at its door: an offer simply takes over from a fidget. A guest
        // has no walk.
        if self.save.home.is_active()
            && !self.is_guest(creature_id)
            && !self.resting_at_home(creature_id)
        {
            return false;
        }
        true
    }

    /// What this creature makes of what is being held out: its temperament, how it has been
    /// treated, what it needs, what it is in the middle of, and a private roll of its own dice.
    fn offer_answer(
        &self,
        creature_id: CreatureId,
        kind: OfferKind,
        now: OffsetDateTime,
        memory: OfferMemory,
    ) -> Answer {
        let Some(creature) = self.offer_subject(creature_id) else {
            return Answer::Decline(BubbleIcon::Decline);
        };
        let personality = &creature.personality;
        let drives = &creature.state.drives;
        let asleep = creature.state.action == ActionKind::Sleep;
        // Too tired to take an interest, or still sleeping off the day: a creature that needs its
        // rest sleeps through the whole thing.
        if drives.sleep_pressure >= TOO_SLEEPY || (asleep && drives.sleep_pressure > RESTED_ENOUGH)
        {
            return Answer::Decline(BubbleIcon::Sleepy);
        }
        let no_thanks = || {
            Answer::Decline(if asleep {
                BubbleIcon::Sleepy
            } else if kind == OfferKind::Toy && personality.playfulness < BAFFLED_BY_TOYS {
                BubbleIcon::Question
            } else {
                BubbleIcon::Decline
            })
        };
        // Asked again before the last answer has finished settling.
        if memory
            .last_offer_utc
            .is_some_and(|last| now - last < Duration::seconds(PESTER_SECS))
        {
            return no_thanks();
        }
        let recently = |at: Option<OffsetDateTime>, window: i64| {
            at.is_some_and(|at| now - at < Duration::seconds(window))
        };
        // Just had one, or in the middle of one. Politely, no.
        match kind {
            OfferKind::Snack
                if creature.state.action == ActionKind::Eat
                    || recently(memory.fed_utc, FED_SECS) =>
            {
                return no_thanks();
            }
            OfferKind::Toy
                if matches!(
                    creature.state.action,
                    ActionKind::SoloPlay | ActionKind::SocialPlay
                ) || recently(memory.played_utc, PLAYED_SECS) =>
            {
                return no_thanks();
            }
            _ => {}
        }

        // How much of this it actually wants.
        let want = match kind {
            OfferKind::Snack => 0.10 + (1.0 - drives.energy) * 1.25 + drives.boredom * 0.10,
            OfferKind::Toy => {
                0.05 + drives.boredom * 1.10
                    + personality.playfulness * 0.70
                    + drives.arousal * 0.10
                    - (1.0 - drives.energy) * 0.25
            }
        };
        // How it has been treated: the learned tendencies, plus the handling it remembers.
        let handled = (creature.memory.times_petted.min(REMEMBERED_HANDLING) as f32
            - creature.memory.times_tossed.min(REMEMBERED_HANDLING) as f32)
            / REMEMBERED_HANDLING as f32;
        let trust = LearnedTendencies::utility(creature.tendencies.cursor_trust)
            + LearnedTendencies::utility(creature.tendencies.sociability) * 0.5
            + handled * 0.18;
        // Taking something out of a hand is a sociable, bold, curious thing to do.
        let manner = personality.sociability * 0.30
            + personality.boldness * 0.26
            + personality.curiosity * 0.12;
        let tiredness = drives.sleep_pressure * 0.75 + (1.0 - drives.comfort) * 0.05;
        let score = want + trust + manner - tiredness;
        // Nothing is ever certain in either direction: every invitation can be declined.
        let chance = (1.0 / (1.0 + (-(score - 0.55) * 2.6).exp())).clamp(0.03, 0.97);

        let mut rng = SeedStream::new(creature.behavior_seed).rng(
            "offer",
            u64::from(memory.offers) * 2 + u64::from(kind == OfferKind::Toy),
        );
        if rng.random::<f32>() < chance {
            Answer::Accept
        } else {
            no_thanks()
        }
    }

    fn deliver(
        &mut self,
        creature_id: CreatureId,
        kind: OfferKind,
        answer: Answer,
        desktop: &DesktopSnapshot,
    ) {
        match answer {
            Answer::Decline(icon) => {
                self.show_bubble(creature_id, icon);
                // A small look away, and nothing else. No penalty, no sulking, nothing learned
                // against the person who asked.
                if !self.save.settings.reduce_motion && !self.attention.owns(creature_id) {
                    self.look_away(creature_id, desktop);
                }
                self.record_offer(creature_id, kind, false);
            }
            Answer::Accept => {
                let (action, icon) = match kind {
                    OfferKind::Snack => (ActionKind::Eat, BubbleIcon::Snack),
                    OfferKind::Toy => (ActionKind::SoloPlay, BubbleIcon::Toy),
                };
                let seconds = {
                    let mut rng = SeedStream::new(self.save.colony_seed).rng(
                        "offer-clip",
                        creature_id.wrapping_mul(2) + u64::from(kind == OfferKind::Toy),
                    );
                    action_duration(action, &mut rng)
                };
                if !self.begin_offer_clip(creature_id, kind, action, seconds, desktop) {
                    return;
                }
                self.show_bubble(creature_id, icon);
                self.record_offer(creature_id, kind, true);
            }
        }
    }

    /// Put the creature on the clip it just said yes to, wherever it happens to be standing.
    fn begin_offer_clip(
        &mut self,
        creature_id: CreatureId,
        kind: OfferKind,
        action: ActionKind,
        seconds: f32,
        desktop: &DesktopSnapshot,
    ) -> bool {
        // While the home is out the tick short-circuits and residents rest at their spots, so the
        // snack is enjoyed on the doorstep instead.
        if self.begin_home_moment(creature_id, action, seconds) {
            return true;
        }
        let guest = self.is_guest(creature_id);
        if self.save.home.is_active() && !guest {
            return false;
        }
        if !guest {
            // Whatever it was attending to gives way through the same door a grab uses.
            self.cancel_creature_attention(creature_id);
            self.action_choices.remove(&creature_id);
            self.bond_plans.remove(&creature_id);
        }
        let reduce_motion = self.save.settings.reduce_motion;
        let cursor = (desktop.cursor.available && kind == OfferKind::Snack && !reduce_motion)
            .then_some(desktop.cursor.position);
        let slept_for = self
            .offer_subject(creature_id)
            .filter(|creature| creature.state.action == ActionKind::Sleep)
            .map(|creature| {
                self.sleep_elapsed
                    .get(&creature_id)
                    .copied()
                    .unwrap_or(creature.state.action_elapsed)
                    .max(0.0) as u32
            });
        let Some(creature) = self.member_or_guest_mut(creature_id) else {
            return false;
        };
        // Turn towards the hand, if it is close enough to be the hand.
        if let Some(point) = cursor
            && creature.state.position.distance(point) <= CURSOR_REACH
        {
            creature.state.facing_right = point.x >= creature.state.position.x;
        }
        creature.state.action = action;
        creature.state.action_elapsed = 0.0;
        creature.state.action_duration = seconds;
        creature.state.velocity = Point::default();
        creature.state.activity_variant = 0;
        if let Some(elapsed_seconds) = slept_for {
            self.sleep_elapsed.remove(&creature_id);
            Self::emit(
                &mut self.events,
                WorldEvent::SleepInterrupted {
                    creature_id,
                    elapsed_seconds,
                },
            );
            Self::emit(&mut self.events, WorldEvent::CreatureWoke { creature_id });
        }
        Self::emit(
            &mut self.events,
            WorldEvent::ActionStarted {
                creature_id,
                action,
            },
        );
        self.offers.entry(creature_id).or_default().enjoying = Some(action);
        true
    }

    /// Face away from the hand that was held out. A sleeping creature is left as it is.
    fn look_away(&mut self, creature_id: CreatureId, desktop: &DesktopSnapshot) {
        let cursor = desktop.cursor.available.then_some(desktop.cursor.position);
        let Some(creature) = self.member_or_guest_mut(creature_id) else {
            return;
        };
        if creature.state.action == ActionKind::Sleep {
            return;
        }
        creature.state.facing_right = match cursor {
            Some(point) if creature.state.position.distance(point) <= CURSOR_REACH => {
                point.x <= creature.state.position.x
            }
            _ => !creature.state.facing_right,
        };
    }

    fn record_offer(&mut self, creature_id: CreatureId, kind: OfferKind, accepted: bool) {
        let now = self.save.maximum_seen_utc;
        let memory = self.offers.entry(creature_id).or_default();
        if accepted {
            match kind {
                OfferKind::Snack => memory.fed_utc = Some(now),
                OfferKind::Toy => memory.played_utc = Some(now),
            }
        }
        // A guest answers like anyone else and nothing about it is kept: no tendency moves, no
        // counter turns, and the colony learns nothing from a stranger's visit.
        if self.is_guest(creature_id) {
            return;
        }
        Self::emit(
            &mut self.events,
            WorldEvent::OfferAnswered {
                creature_id,
                kind,
                accepted,
            },
        );
    }

    /// A colony member, or whoever is visiting while it is out on the desktop.
    fn offer_subject(&self, creature_id: CreatureId) -> Option<&Creature> {
        self.save
            .creatures
            .iter()
            .find(|creature| creature.id == creature_id)
            .or_else(|| {
                self.save
                    .visitors
                    .on_stage()
                    .filter(|guest| guest.id == creature_id)
            })
    }

    fn forget_departed_offers(&mut self) {
        if self.offers.is_empty() {
            return;
        }
        let guest = self.save.visitors.on_stage().map(|guest| guest.id);
        let creatures = &self.save.creatures;
        self.offers.retain(|creature_id, _| {
            creatures.iter().any(|creature| creature.id == *creature_id)
                || guest == Some(*creature_id)
        });
    }
}
