//! Magic fruit out on the desktop: when one is ripe, who finds it in their meal, and the spell it
//! leaves them under until it wears off or something else comes along.
//!
//! A fruit ripens after a few days of the colony being out on the desktop, visible and unpaused —
//! a clock that only runs while somebody could see it happen. The next companion to sit down to
//! eat out there finds a glowing star fruit in its paws in place of whatever it was having, with a
//! start. If it eats most of it, a spell comes over it as the meal ends: a burst of lights, a
//! cheer, and then a minute or two of floating, bouncing, zoomies or shimmering, whichever its
//! temperament leans to (`magic::leanings`).
//!
//! A spell is only ever drawn: a floating companion's feet stay where the simulation has them,
//! and the spell goes to nothing whenever something else wants its body — a scene, a climb, a
//! wonder, a pet — and comes back once it is free again. Being picked up or thrown, falling
//! asleep, going home, the colony leaving on a trip or into a house, a ritual, quiet mode, hiding
//! or pausing end it outright, and a relaunch never knew of it. With reduced motion it is the
//! still kind: a few lights that stay where they are. Nothing about it is saved, counted or
//! written anywhere, and every choice comes from a stream of its own, so none of it shifts any
//! other choice the colony makes.

use super::*;
use crate::magic::{MagicKind, Spell, leanings};
use crate::meals::{Food, Meal};
use crate::tuning::MAGIC;

/// When the next magic fruit is ripe, and who is eating one now.
#[derive(Clone, Debug)]
pub(super) struct MagicFruit {
    /// Seconds out on the desktop until the next one is ripe.
    pub(super) ripe_in: f32,
    /// Who is eating a magic fruit now, and how far through it they had got by the last tick.
    eating: Option<(CreatureId, f32)>,
    rng: ChaCha12Rng,
}

impl MagicFruit {
    pub(super) fn new(streams: &SeedStream) -> Self {
        let mut rng = streams.rng("magic-fruit", 0);
        Self {
            ripe_in: rng.random_range(MAGIC.ripe_after_secs),
            eating: None,
            rng,
        }
    }

    #[cfg(test)]
    pub(super) fn ripen_now(&mut self) {
        self.ripe_in = 0.0;
    }

    #[cfg(test)]
    pub(super) fn eating(&self) -> Option<CreatureId> {
        self.eating.map(|(id, _)| id)
    }
}

/// Whether a companion doing this could be drawn under a spell: on its feet, or sat, doing its
/// own ordinary things. Anything that hangs, climbs, rides, falls or is handled has its body.
fn shows_magic(action: ActionKind) -> bool {
    matches!(
        action,
        ActionKind::Idle
            | ActionKind::Traverse
            | ActionKind::Sprint
            | ActionKind::Follow
            | ActionKind::Greet
            | ActionKind::SocialPlay
            | ActionKind::InvestigateCursor
            | ActionKind::AvoidCursor
            | ActionKind::InspectScreen
            | ActionKind::Perch
            | ActionKind::SoloPlay
            | ActionKind::Eat
            | ActionKind::Drink
            | ActionKind::PresentDiscovery
    )
}

impl World {
    /// Whether a magic fruit could be found, or a spell kept, just now: out on the desktop with
    /// the colony visible, unpaused, not in quiet mode and not gathered for a ritual.
    fn magic_lively(&self) -> bool {
        self.save.settings.visible
            && !self.save.settings.paused
            && self.save.companion.quiet_until.is_none()
            && !self.save.home.is_active()
            && self.colony_plan.is_none()
    }

    /// Ripens magic fruit, puts one in a companion's paws, starts the spell when it has eaten
    /// it, and carries every spell along. Called once, at the end of every tick, after the meals.
    pub(super) fn carry_magic(&mut self, dt: f32) {
        let lively = self.magic_lively();
        if lively {
            self.magic.ripe_in = (self.magic.ripe_in - dt).max(0.0);
        }
        self.follow_magic_meal(lively);
        if lively && self.magic.ripe_in <= 0.0 && self.magic.eating.is_none() {
            self.find_magic_fruit();
        }
        self.carry_spells(dt, lively);
    }

    /// Lets go of every spell and any magic meal: the colony is being settled. The fruit being
    /// eaten is simply a meal like any other from here on.
    pub(super) fn end_all_magic(&mut self) {
        if self.magic.eating.take().is_some() {
            self.magic.ripe_in = self.magic.rng.random_range(MAGIC.retry_secs);
        }
        for creature in &mut self.save.creatures {
            creature.state.magic = None;
        }
    }

    /// The next companion out on the desktop to sit down to eat finds its food is a star fruit.
    fn find_magic_fruit(&mut self) {
        let held = self.interaction.as_ref().map(|i| i.creature_id);
        let finder = self.save.creatures.iter().position(|creature| {
            let state = &creature.state;
            state.action == ActionKind::Eat
                && state.arrival_delay_secs <= 0.0
                && state.magic.is_none()
                && !state.indoors
                && held != Some(creature.id)
                && !self.house_visit.contains(&creature.id)
                && state
                    .meal
                    .is_some_and(|meal| meal.elapsed < 0.6 && meal.food != Food::Starfruit)
        });
        let Some(index) = finder else {
            return;
        };
        let creature = &self.save.creatures[index];
        let id = creature.id;
        let appetite = meals::appetite(creature);
        let Some(old) = creature.state.meal else {
            return;
        };
        let mut meal = Meal::plan(Food::Starfruit, old.length, appetite, &mut self.magic.rng);
        meal.elapsed = old.elapsed;
        meal.action_mark = old.action_mark;
        self.save.creatures[index].state.meal = Some(meal);
        self.magic.eating = Some((id, 0.0));
        self.show_bubble(id, BubbleIcon::Surprise);
    }

    /// Keeps up with whoever is eating a magic fruit, and when the meal is over, starts the spell
    /// if enough of it was eaten and nothing has taken the companion away from it.
    fn follow_magic_meal(&mut self, lively: bool) {
        let Some((id, eaten)) = self.magic.eating else {
            return;
        };
        let Some(index) = self.save.creatures.iter().position(|c| c.id == id) else {
            self.magic.eating = None;
            return;
        };
        let state = &self.save.creatures[index].state;
        if state.action == ActionKind::Eat
            && let Some(meal) = state.meal
            && meal.food == Food::Starfruit
        {
            let progress = meal.elapsed / meal.length.max(0.01);
            self.magic.eating = Some((id, eaten.max(progress)));
            return;
        }
        self.magic.eating = None;
        let held = self
            .interaction
            .as_ref()
            .is_some_and(|i| i.creature_id == id);
        let finished = eaten >= MAGIC.eaten_enough
            && lively
            && !held
            && !matches!(
                state.action,
                ActionKind::Dragged | ActionKind::Tossed | ActionKind::Sleep
            );
        if !finished {
            self.magic.ripe_in = self.magic.rng.random_range(MAGIC.retry_secs);
            return;
        }
        self.magic.ripe_in = self.magic.rng.random_range(MAGIC.ripe_after_secs);
        let creature = &self.save.creatures[index];
        let weights = leanings(&creature.personality, &creature.traits());
        let total: f32 = weights.iter().sum();
        let mut roll = self.magic.rng.random_range(0.0..total);
        let mut kind = MagicKind::Shimmer;
        for (candidate, weight) in MagicKind::ALL.into_iter().zip(weights) {
            if roll < weight {
                kind = candidate;
                break;
            }
            roll -= weight;
        }
        let length = self.magic.rng.random_range(MAGIC.length_secs);
        let phase = self.magic.rng.random_range(0.0..10.0);
        self.cast_spell(id, kind, length, phase);
    }

    /// Puts a companion under a spell, with a burst of lights and a moment of delight that is its
    /// own: a show-off strikes a pose, and a shy one hides a blush behind its paws.
    pub(super) fn cast_spell(&mut self, id: CreatureId, kind: MagicKind, length: f32, phase: f32) {
        let still = self.save.settings.reduce_motion;
        let Some(index) = self.save.creatures.iter().position(|c| c.id == id) else {
            return;
        };
        let creature = &self.save.creatures[index];
        let traits = creature.traits();
        let has = |any: &[Trait]| traits.iter().any(|t| any.contains(t));
        let (beat, bubble) = if has(&[Trait::Shy, Trait::EasilyEmbarrassed]) {
            (BeatKind::Peek, BubbleIcon::Blush)
        } else if has(&[Trait::Vain, Trait::AttentionSeeking, Trait::Confident]) {
            (BeatKind::Strut, BubbleIcon::Sparkle)
        } else {
            (BeatKind::Delight, BubbleIcon::Sparkle)
        };
        let free = beats::free_for_a_beat(self, creature);
        self.show_bubble(id, bubble);
        let state = &mut self.save.creatures[index].state;
        state.magic = Some(Spell::new(kind, length, phase, still));
        if free && !still {
            state.beat = Some(Beat::new(beat, 1.8));
        }
    }

    /// Carries every spell along: ends one that something has ended, turns one still for reduced
    /// motion, eases how much of it shows toward what the companion is doing, and works out how
    /// far up it is drawn.
    fn carry_spells(&mut self, dt: f32, lively: bool) {
        let held = self.interaction.as_ref().map(|i| i.creature_id);
        let reduce_motion = self.save.settings.reduce_motion;
        for index in 0..self.save.creatures.len() {
            let creature = &self.save.creatures[index];
            let Some(mut spell) = creature.state.magic else {
                continue;
            };
            let id = creature.id;
            let state = &creature.state;
            let ended = !lively
                || held == Some(id)
                || state.indoors
                || self.house_visit.contains(&id)
                || matches!(
                    state.action,
                    ActionKind::Dragged | ActionKind::Tossed | ActionKind::Sleep
                )
                || spell.over();
            if ended {
                self.save.creatures[index].state.magic = None;
                continue;
            }
            if reduce_motion && !spell.still {
                spell = Spell {
                    kind: MagicKind::Shimmer,
                    still: true,
                    lift: 0.0,
                    ..spell
                };
            }
            let busy = self.attention.owns(id)
                || state.attention.is_some()
                || self.window_journeys.contains_key(&id)
                || self.tosses.contains_key(&id)
                || self.wonders.owns(id)
                || state.flourish.is_some()
                || state.nudge.is_some()
                || !shows_magic(state.action);
            let target = if busy { 0.0 } else { spell.envelope() };
            let step = MAGIC.settle_per_sec * dt;
            spell.strength = if spell.strength < target {
                (spell.strength + step).min(target)
            } else {
                (spell.strength - step).max(target)
            };
            spell.elapsed += dt;
            // Something that wants its body has it on its feet at once, not floating down.
            if busy {
                spell.strength = 0.0;
            }
            let presence = &state.presence;
            spell.lift = spell.shape_lift(presence.stepping, presence.stride) * spell.strength;
            self.save.creatures[index].state.magic = Some(spell);
        }
    }
}
