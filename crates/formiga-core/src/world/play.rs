//! Giving each companion that settles down to play a [`Play`]: something to play with and the
//! moves its shape is played with, carried along while it plays; and an umbrella, when the colony
//! has one, ridden down from a ledge.
//!
//! Like meals, this runs at the end of each tick, rolls dice of its own, and writes nothing the
//! simulation reads.

use super::*;
use crate::play::{Play, own_toy};

impl World {
    /// Start, carry on or clear everyone's play. Called once, at the end of every tick.
    pub(super) fn carry_play(&mut self, now: OffsetDateTime, dt: f32) {
        let belongings: Vec<ColonyObjectKind> = self
            .save
            .objects
            .objects
            .iter()
            .map(|object| object.kind)
            .collect();
        let umbrella = belongings.contains(&ColonyObjectKind::Umbrella);
        let finds: Vec<u8> = self
            .save
            .companion
            .scrapbook
            .iter()
            .map(|record| record.variant)
            .collect();
        let streams = SeedStream::new(self.save.colony_seed);
        for creature in &mut self.save.creatures {
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
            let mut rng = streams.rng(
                "play",
                creature.id ^ (now.unix_timestamp() as u64).rotate_left(23),
            );
            let left = (state.action_duration - state.action_elapsed).max(0.5);
            let mut play = if state.action == ActionKind::Landing {
                // Down off a ledge under the colony's umbrella, now and then.
                if !umbrella || state.action_elapsed > 0.1 || !rng.random_bool(0.5) {
                    continue;
                }
                Play::umbrella_ride(left.min(5.0))
            } else {
                let thing = Play::choose_thing(
                    own_toy(&creature.appearance),
                    &belongings,
                    &finds,
                    &mut rng,
                );
                Play::plan(thing, left.min(30.0), &mut rng)
            };
            play.elapsed = dt.min(left);
            state.play = Some(play);
        }
    }
}
