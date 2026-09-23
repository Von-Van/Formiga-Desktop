//! Small moments a companion has between everything else, and the yawns that pass from one to
//! the next.
//!
//! A `Beat` on a creature's state holds it where it is for a few seconds while the art shows a
//! pose, a face and anything held. This module moves every beat on, lets one go when it is over
//! or when something bigger comes along, and starts the queued ones — which is how a yawn is
//! caught: one companion yawns, a friend nearby looks over and, a moment later, yawns too; now and
//! then it reaches a third, who holds out for a second or two before giving in.
//!
//! Nothing here is saved, journaled or counted. Every choice comes from a seeded stream of its
//! own, so yawning never shifts any other choice the colony makes.

use super::*;

/// How long between one yawn somewhere in the colony and the next, in visible seconds. Sleepier
/// colonies yawn more: see `YAWN_SLEEPY_BIAS`.
const YAWN_INTERVAL_SECS: std::ops::Range<f32> = 150.0..360.0;
/// How near a friend has to be to catch a yawn, in creature widths.
const YAWN_REACH_FRAMES: f32 = 3.5;
/// The chance the nearest friend catches a yawn, and the chance it goes on to a third.
const CATCH_CHANCE: f64 = 0.8;
const THIRD_CHANCE: f64 = 0.4;
/// How long a yawn lasts, from breathing in to settling.
pub(super) const YAWN_SECS: f32 = 2.2;
/// How long a friend looks over before its own yawn starts.
const NOTICE_SECS: std::ops::Range<f32> = 0.7..1.4;
/// How long the last in a chain holds out.
const RESIST_SECS: std::ops::Range<f32> = 1.3..2.1;

/// One beat waiting to start. Links of one chain share a `chain` number, so a friend who has
/// wandered off takes everyone after it out of the chain too.
#[derive(Clone, Copy, Debug)]
pub(super) struct QueuedBeat {
    creature: CreatureId,
    start_in: f32,
    beat: Beat,
    /// Whose yawn this one answers, so the beat looks at them.
    source: Option<CreatureId>,
    chain: u32,
}

/// The colony's yawning: when the next one is due, the beats waiting to start, and which chain
/// each beat under way belongs to, so the next link of a chain can follow straight on from the
/// last without the two having to line up to the tick.
#[derive(Clone, Debug)]
pub(super) struct Beats {
    yawn_in: f32,
    queue: Vec<QueuedBeat>,
    links: Vec<(CreatureId, u32)>,
    next_chain: u32,
    rng: ChaCha12Rng,
}

impl Beats {
    pub(super) fn new(streams: &SeedStream) -> Self {
        let mut rng = streams.rng("beats", 0);
        Self {
            yawn_in: rng.random_range(YAWN_INTERVAL_SECS),
            queue: Vec::new(),
            links: Vec::new(),
            next_chain: 0,
            rng,
        }
    }

    /// Forget every beat still waiting to start. Beats already under way are the creatures' own
    /// and end on their own terms.
    pub(super) fn clear_queue(&mut self) {
        self.queue.clear();
        self.links.clear();
    }

    #[cfg(test)]
    pub(super) fn queued(&self) -> usize {
        self.queue.len()
    }

    #[cfg(test)]
    pub(super) fn yawn_now(&mut self) {
        self.yawn_in = 0.0;
    }
}

/// Whether a creature is free to yawn, or to stop and look at somebody yawning: arrived, awake,
/// standing about or ambling somewhere, and doing nothing that owns its body — no scene, no
/// journey, no small thing at its door, no beat of its own under way. Somebody walking stops
/// where it is for the yawn and then walks on.
fn free_for_a_beat(world: &World, creature: &Creature) -> bool {
    let state = &creature.state;
    state.arrival_delay_secs <= 0.0
        && state.beat.is_none()
        && !state.indoors
        && state.attention.is_none()
        && state.flourish.is_none()
        && matches!(
            state.action,
            ActionKind::Idle | ActionKind::Perch | ActionKind::Homebound | ActionKind::Traverse
        )
        && !world.attention.owns(creature.id)
        && !world.window_journeys.contains_key(&creature.id)
        && !world.home_moments.contains_key(&creature.id)
        && !world.village_life.contains_key(&creature.id)
        && !world
            .interaction
            .as_ref()
            .is_some_and(|interaction| interaction.creature_id == creature.id)
}

impl World {
    /// Lets go of every beat, under way or waiting: the colony is being settled and moved about.
    pub(super) fn clear_beats(&mut self) {
        self.beats.clear_queue();
        for creature in &mut self.save.creatures {
            creature.state.beat = None;
        }
    }

    /// Moves every beat on by `dt`, lets go of the ones that are over or that something bigger
    /// has taken over from, starts the queued ones that are due, and now and then starts a yawn.
    pub(super) fn advance_beats(&mut self, dt: f32, desktop: &DesktopSnapshot) {
        let lively = self.save.settings.visible
            && !self.save.settings.paused
            && !self.save.settings.reduce_motion
            && self.colony_plan.is_none();
        let held = self
            .interaction
            .as_ref()
            .map(|interaction| interaction.creature_id);
        for creature in &mut self.save.creatures {
            let Some(beat) = &mut creature.state.beat else {
                continue;
            };
            beat.elapsed += dt;
            let taken_over = held == Some(creature.id)
                || self.attention.owns(creature.id)
                || creature.state.attention.is_some()
                || matches!(
                    creature.state.action,
                    ActionKind::Dragged | ActionKind::Tossed | ActionKind::PetReaction
                );
            if beat.finished() || taken_over || !lively {
                creature.state.beat = None;
            }
        }
        if !lively {
            self.beats.clear_queue();
            return;
        }
        let creatures = &self.save.creatures;
        self.beats.links.retain(|(id, _)| {
            creatures
                .iter()
                .any(|creature| creature.id == *id && creature.state.beat.is_some())
        });
        self.start_queued_beats(dt);
        self.beats.yawn_in -= dt;
        if self.beats.yawn_in <= 0.0 {
            self.beats.yawn_in = self.beats.rng.random_range(YAWN_INTERVAL_SECS);
            self.start_yawn_chain(desktop);
        }
    }

    fn start_queued_beats(&mut self, dt: f32) {
        if self.beats.queue.is_empty() {
            return;
        }
        let mut due = Vec::new();
        self.beats.queue.retain_mut(|queued| {
            queued.start_in -= dt;
            if queued.start_in <= 0.0 {
                due.push(*queued);
                false
            } else {
                true
            }
        });
        for queued in due {
            let Some(index) = self
                .save
                .creatures
                .iter()
                .position(|creature| creature.id == queued.creature)
            else {
                self.drop_chain(queued.chain);
                continue;
            };
            // The next beat of the same chain follows straight on from the last, so a creature
            // that is only busy with its own previous link still counts as free.
            let creature = &self.save.creatures[index];
            let continuing = creature.state.beat.is_some()
                && self.beats.links.contains(&(queued.creature, queued.chain));
            let free = if continuing {
                let mut probe = creature.clone();
                probe.state.beat = None;
                free_for_a_beat(self, &probe)
            } else {
                free_for_a_beat(self, creature)
            };
            if !free {
                self.drop_chain(queued.chain);
                continue;
            }
            let look = queued.source.and_then(|source| {
                self.save
                    .creatures
                    .iter()
                    .find(|creature| creature.id == source)
                    .map(|creature| creature.state.position)
            });
            let creature = &mut self.save.creatures[index];
            let mut beat = queued.beat;
            beat.look = look;
            if let Some(look) = look
                && (look.x - creature.state.position.x).abs() > 1.0
            {
                creature.state.facing_right = look.x > creature.state.position.x;
            }
            creature.state.beat = Some(beat);
            self.beats.links.retain(|(id, _)| *id != queued.creature);
            self.beats.links.push((queued.creature, queued.chain));
        }
    }

    /// Takes every link of one chain still waiting out of the queue.
    fn drop_chain(&mut self, chain: u32) {
        self.beats.queue.retain(|queued| queued.chain != chain);
        self.beats.links.retain(|(_, link)| *link != chain);
    }

    /// Somebody yawns — likelier the sleepier they are — and the nearest friend, and now and then
    /// a friend of that friend, catch it.
    fn start_yawn_chain(&mut self, desktop: &DesktopSnapshot) {
        let candidates: Vec<(CreatureId, f32)> = self
            .save
            .creatures
            .iter()
            .filter(|creature| free_for_a_beat(self, creature))
            .map(|creature| (creature.id, creature.state.drives.sleep_pressure + 0.25))
            .collect();
        let total: f32 = candidates.iter().map(|(_, weight)| weight).sum();
        if candidates.is_empty() || total <= 0.0 {
            return;
        }
        let mut roll = self.beats.rng.random_range(0.0..total);
        let first = candidates
            .iter()
            .find(|(_, weight)| {
                let hit = roll < *weight;
                roll -= weight;
                hit
            })
            .map_or(candidates[0].0, |(id, _)| *id);
        let chain = self.beats.next_chain;
        self.beats.next_chain = self.beats.next_chain.wrapping_add(1);
        if let Some(creature) = creature_mut(&mut self.save.creatures, first) {
            creature.state.beat = Some(Beat::new(BeatKind::Yawn, YAWN_SECS));
        }
        // The nearest friend, if it is near enough and free.
        let Some(second) = self.nearest_free_friend(first, &[first], desktop) else {
            return;
        };
        if !self.beats.rng.random_bool(CATCH_CHANCE) {
            return;
        }
        let notice_at = self.beats.rng.random_range(0.4..0.9);
        let notice = self.beats.rng.random_range(NOTICE_SECS);
        self.beats.queue.push(QueuedBeat {
            creature: second,
            start_in: notice_at,
            beat: Beat::new(BeatKind::Notice, notice),
            source: Some(first),
            chain,
        });
        let second_yawn = notice_at + notice;
        self.beats.queue.push(QueuedBeat {
            creature: second,
            start_in: second_yawn,
            beat: Beat::new(BeatKind::Yawn, YAWN_SECS),
            source: Some(first),
            chain,
        });
        // Now and then it travels once more, to somebody who puts up a fight first.
        if !self.beats.rng.random_bool(THIRD_CHANCE) {
            return;
        }
        let Some(third) = self.nearest_free_friend(second, &[first, second], desktop) else {
            return;
        };
        let notice_at = second_yawn + self.beats.rng.random_range(0.4..0.8);
        let notice = self.beats.rng.random_range(NOTICE_SECS);
        let resist = self.beats.rng.random_range(RESIST_SECS);
        self.beats.queue.push(QueuedBeat {
            creature: third,
            start_in: notice_at,
            beat: Beat::new(BeatKind::Notice, notice),
            source: Some(second),
            chain,
        });
        self.beats.queue.push(QueuedBeat {
            creature: third,
            start_in: notice_at + notice,
            beat: Beat::new(BeatKind::ResistYawn, resist),
            source: Some(second),
            chain,
        });
        self.beats.queue.push(QueuedBeat {
            creature: third,
            start_in: notice_at + notice + resist,
            beat: Beat::new(BeatKind::Yawn, YAWN_SECS),
            source: Some(second),
            chain,
        });
    }

    /// The nearest companion to `of` that is free, on the same kind of ground and the same
    /// display, within yawning distance, and none of `besides`.
    fn nearest_free_friend(
        &self,
        of: CreatureId,
        besides: &[CreatureId],
        desktop: &DesktopSnapshot,
    ) -> Option<CreatureId> {
        let source = self
            .save
            .creatures
            .iter()
            .find(|creature| creature.id == of)?;
        let reach =
            spacing::creature_frame_width(source, self.save.settings.display_scale, desktop)
                * YAWN_REACH_FRAMES;
        self.save
            .creatures
            .iter()
            .filter(|creature| !besides.contains(&creature.id))
            .filter(|creature| {
                creature.state.surface.monitor_id == source.state.surface.monitor_id
                    && creature.state.surface.window_key == source.state.surface.window_key
            })
            .filter(|creature| free_for_a_beat(self, creature))
            .map(|creature| {
                (
                    creature.state.position.distance(source.state.position),
                    creature.id,
                )
            })
            .filter(|(distance, _)| *distance <= reach)
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, id)| id)
    }
}
