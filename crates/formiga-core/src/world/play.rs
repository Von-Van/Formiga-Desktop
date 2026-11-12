//! Giving each companion that settles down to play a [`Play`]: something to play with and the
//! moves its shape is played with, carried along while it plays; and an umbrella, when the colony
//! has one, ridden down from a ledge.
//!
//! Like meals, this runs at the end of each tick, rolls dice of its own, and writes nothing the
//! simulation reads.

use super::*;
use crate::play::{Play, Plaything, own_toy};

/// How far down a hop off a window has to go before an umbrella is worth opening for it: a
/// real drop, not a step down or a companion set on the floor.
const UMBRELLA_DROP: f32 = 60.0;

impl World {
    /// Start, carry on or clear everyone's play, the guest on stage included. Called once, at the
    /// end of every tick.
    pub(super) fn carry_play(&mut self, now: OffsetDateTime, dt: f32) {
        // Paused, a game holds where it is, like everything else the colony is doing.
        let dt = if self.save.settings.paused { 0.0 } else { dt };
        let finds: Vec<u8> = self
            .save
            .companion
            .scrapbook
            .iter()
            .map(|record| record.variant)
            .collect();
        // Whatever is already borrowed from the yard stays with whoever has it.
        let mut borrowed: Vec<u64> = self
            .save
            .creatures
            .iter()
            .filter_map(|creature| creature.state.play.and_then(|play| play.borrowed))
            .collect();
        // Only the start of a hop down off a window, far enough to float, is ridden under an
        // umbrella: never a step down, a companion set on the floor, or the end of a toss.
        let descents: Vec<(CreatureId, f32)> = self
            .window_journeys
            .iter()
            .filter_map(|(id, journey)| match journey {
                WindowJourney::Hop(hop)
                    if hop.target.y - hop.start.y >= UMBRELLA_DROP && hop.elapsed <= 0.1 =>
                {
                    Some((*id, hop.duration - hop.elapsed))
                }
                _ => None,
            })
            .collect();
        let streams = SeedStream::new(self.save.colony_seed);
        let objects = &self.save.objects.objects;
        let reduce_motion = self.save.settings.reduce_motion;
        let guest_id = self.save.visitors.on_stage().map(|guest| guest.id);
        let guest = self
            .save
            .visitors
            .guest
            .as_mut()
            .filter(|guest| guest.on_stage)
            .map(|guest| &mut guest.creature);
        for creature in self.save.creatures.iter_mut().chain(guest) {
            let is_guest = Some(creature.id) == guest_id;
            let state = &mut creature.state;
            let playing = matches!(state.action, ActionKind::SoloPlay | ActionKind::Landing);
            if !playing {
                state.play = None;
                continue;
            }
            if let Some(play) = &mut state.play {
                play.elapsed += dt;
                continue;
            }
            // A guest plays with what it brought; the colony's belongings and finds are the
            // colony's own.
            let free: Vec<&ColonyObject> = objects
                .iter()
                .filter(|object| !is_guest && !borrowed.contains(&object.id))
                .collect();
            let mut rng = streams.rng(
                "play",
                creature.id ^ (now.unix_timestamp() as u64).rotate_left(23),
            );
            let left = (state.action_duration - state.action_elapsed).max(0.5);
            let mut play = if state.action == ActionKind::Landing {
                // Down off a ledge under the colony's umbrella, now and then.
                let Some(umbrella) = free
                    .iter()
                    .find(|object| object.kind == ColonyObjectKind::Umbrella)
                else {
                    continue;
                };
                let Some((_, drop)) = descents.iter().find(|(id, _)| *id == creature.id) else {
                    continue;
                };
                if state.action_elapsed > 0.1 || !rng.random_bool(0.5) {
                    continue;
                }
                let mut ride = Play::umbrella_ride(drop.min(left).min(5.0));
                ride.borrowed = Some(umbrella.id);
                ride
            } else {
                let kinds: Vec<ColonyObjectKind> = free.iter().map(|object| object.kind).collect();
                let thing = Play::choose_thing(
                    own_toy(&creature.appearance),
                    &kinds,
                    if is_guest { &[] } else { &finds },
                    &mut rng,
                );
                let mut play = Play::plan(thing, left.min(30.0), &mut rng);
                if let Plaything::Belonging(kind) = thing {
                    play.borrowed = free
                        .iter()
                        .find(|object| object.kind == kind)
                        .map(|object| object.id);
                }
                play
            };
            play.elapsed = dt.min(left);
            // A bow before a game comes first, and the game is picked up after it.
            if !reduce_motion
                && let Some(flourish) = state.flourish
                && flourish.habit == Habit::PlayBows
                && flourish.action == ActionKind::SoloPlay
                && let Some(began) = flourish.started_at
            {
                let behind = state.action_elapsed - play.elapsed;
                let picking_up = play.moves().get(1).map_or(0.0, |(_, start)| *start);
                play.pick_up_until(began + flourish.habit.seconds() - behind + picking_up);
            }
            borrowed.extend(play.borrowed);
            state.play = Some(play);
        }
    }
}
