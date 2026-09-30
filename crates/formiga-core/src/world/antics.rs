//! A companion being itself: the little moments a temperament makes of what goes on around it.
//!
//! A jealous one huffs when another is petted, and a grump grumbles at a pet before it gives in to
//! it. A dramatic one swoons at a fright and a jumpy one hides behind its paws. A food-lover begs
//! when somebody nearby is eating, a show-off strikes a pose after a climb or a find, and one that
//! embarrasses easily blushes behind its paws after a mishap. Now and then a troublemaker pounces
//! on a friend who is resting, a stubborn one stamps its foot when it will not join in, and an
//! impatient one left standing about taps its foot.
//!
//! Each is a beat: it holds the companion where it is for a moment while the art shows a pose, a
//! face and a bubble, and then lets go. Nothing here is saved, journaled or counted, a companion
//! has a while to itself after each one, and every choice comes from a seeded stream of its own,
//! so none of it shifts any other choice the colony makes.

use super::*;

/// How long a companion has to itself after one of these moments before it can have another.
const COOLDOWN_SECS: std::ops::Range<f32> = 45.0..100.0;
/// How near another companion has to be, in creature widths, to react to what happens to it.
const REACH_FRAMES: f32 = 5.0;
/// How long a cue waits for somebody to be free to react to it before it is let go.
const CUE_PATIENCE_SECS: f32 = 3.0;
/// How long between one prank somewhere in the colony and the next, in visible seconds.
const PRANK_INTERVAL_SECS: std::ops::Range<f32> = 150.0..360.0;
/// How long an impatient companion stands about before it starts tapping its foot.
const IMPATIENT_AFTER_SECS: f32 = 16.0;

/// Something that happened that a companion's temperament might make something of.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Cue {
    /// Somebody was petted.
    Petted(CreatureId),
    /// Somebody sat down to eat.
    Eating(CreatureId),
    /// Somebody got a fright.
    Startled(CreatureId),
    /// Somebody climbed up onto a window.
    Climbed(CreatureId),
    /// Somebody found something and held it up.
    Found(CreatureId),
    /// Somebody had a small mishap.
    Mishap(CreatureId),
    /// Somebody said no to joining in.
    Declined(CreatureId),
}

/// Something that follows a moment a beat later: a bubble, or a beat of its own.
#[derive(Clone, Copy, Debug)]
enum Later {
    Bubble(BubbleIcon),
    Beat(BeatKind, f32, Option<Point>),
}

/// The colony's antics under way and waiting: cues not yet answered, who is having a while to
/// itself, what follows a moment a beat later, who is in the middle of a mishap, and when the
/// next prank is due.
#[derive(Clone, Debug)]
pub(super) struct Antics {
    cues: Vec<(Cue, f32)>,
    cooldowns: BTreeMap<CreatureId, f32>,
    later: Vec<(CreatureId, Later, f32)>,
    mishaps: BTreeSet<CreatureId>,
    /// Who is a troublemaker and who is impatient, worked out once per companion and kept with
    /// the temperament it was worked out from, since the checks that ask run every tick.
    everyday: BTreeMap<CreatureId, (Option<Temperament>, (bool, bool))>,
    prank_in: f32,
    rng: ChaCha12Rng,
}

impl Antics {
    pub(super) fn new(streams: &SeedStream) -> Self {
        let mut rng = streams.rng("antics", 0);
        Self {
            cues: Vec::new(),
            cooldowns: BTreeMap::new(),
            later: Vec::new(),
            mishaps: BTreeSet::new(),
            everyday: BTreeMap::new(),
            prank_in: rng.random_range(PRANK_INTERVAL_SECS),
            rng,
        }
    }

    /// Note something that happened, for the next tick to answer. A handful at most wait at
    /// once; a busy moment does not need every one of them answered.
    pub(super) fn cue(&mut self, cue: Cue) {
        if self.cues.len() < 12 && !self.cues.iter().any(|(queued, _)| *queued == cue) {
            self.cues.push((cue, 0.0));
        }
    }

    /// Show `icon` over a companion `after` seconds from now.
    pub(super) fn bubble_later(&mut self, creature: CreatureId, icon: BubbleIcon, after: f32) {
        self.later.push((creature, Later::Bubble(icon), after));
    }

    #[cfg(test)]
    pub(super) fn prank_now(&mut self) {
        self.prank_in = 0.0;
    }

    #[cfg(test)]
    pub(super) fn waiting(&self) -> usize {
        self.cues.len()
    }
}

/// What a companion's temperament makes of things, read from its traits, its tension and the
/// sides of it those rest on.
struct Character {
    traits: [Trait; 3],
    temperament: Temperament,
}

impl Character {
    fn of(creature: &Creature) -> Self {
        Self {
            traits: creature.traits(),
            temperament: creature.temperament(),
        }
    }

    fn has(&self, any: &[Trait]) -> bool {
        self.traits.iter().any(|t| any.contains(t))
    }

    fn tension(&self, tension: Tension) -> bool {
        self.temperament.tension == Some(tension)
    }

    fn jealous(&self) -> bool {
        let a = self.temperament.axes;
        self.has(&[Trait::Jealous, Trait::Clingy, Trait::AttentionSeeking])
            || (a.affection >= 0.78 && a.feistiness >= 0.6)
    }

    fn dramatic(&self) -> bool {
        self.has(&[Trait::Dramatic]) || self.tension(Tension::DramaticButCowardly)
    }

    fn jumpy(&self) -> bool {
        self.has(&[Trait::Nervous, Trait::Cowardly]) || self.tension(Tension::BraveButNervous)
    }

    fn food_lover(&self) -> bool {
        self.has(&[Trait::FoodMotivated])
    }

    fn show_off(&self) -> bool {
        self.has(&[Trait::Vain, Trait::AttentionSeeking, Trait::Confident])
            || self.temperament.kind == TemperamentKind::Showoff
    }

    fn embarrassable(&self) -> bool {
        self.tension(Tension::ConfidentButEasilyEmbarrassed)
            || self.has(&[Trait::Shy, Trait::EasilyEmbarrassed])
    }

    fn troublemaker(&self) -> bool {
        self.has(&[Trait::Mischievous]) || self.temperament.kind == TemperamentKind::Troublemaker
    }

    fn stubborn(&self) -> bool {
        self.has(&[Trait::Stubborn, Trait::Picky]) || self.tension(Tension::GentleButStubborn)
    }

    fn impatient(&self) -> bool {
        self.has(&[Trait::Impatient, Trait::Restless])
    }
}

/// What a companion's bubble says when it is petted: a heart, or for a grump a grumble, and
/// whether a secretly affectionate grump's heart follows it a moment later.
pub(super) fn pet_bubble(creature: &Creature) -> (BubbleIcon, bool) {
    let character = Character::of(creature);
    let grump = character.has(&[Trait::Grumpy, Trait::Irritable])
        || character.temperament.kind == TemperamentKind::Grump;
    if !grump {
        return (BubbleIcon::Heart, false);
    }
    let soft = character.tension(Tension::GrumpyButAffectionate)
        || character.has(&[Trait::SecretlyAffectionate])
        || character.temperament.axes.affection >= 0.62;
    (BubbleIcon::Grumble, soft)
}

impl World {
    /// Moves the colony's antics along: answers what happened since the last tick, lets bubbles
    /// that follow a moment show, and now and then starts a prank or a tapping foot.
    pub(super) fn advance_antics(&mut self, dt: f32) {
        for cooldown in self.antics.cooldowns.values_mut() {
            *cooldown -= dt;
        }
        self.antics
            .cooldowns
            .retain(|_, remaining| *remaining > 0.0);
        let lively =
            self.save.settings.visible && !self.save.settings.paused && self.colony_plan.is_none();
        if !lively {
            self.antics.cues.clear();
            self.antics.later.clear();
            self.antics.mishaps.clear();
            return;
        }
        let mut due = Vec::new();
        self.antics.later.retain_mut(|(id, later, after)| {
            *after -= dt;
            if *after <= 0.0 {
                due.push((*id, *later));
                false
            } else {
                true
            }
        });
        for (id, later) in due {
            match later {
                Later::Bubble(icon) => self.show_bubble(id, icon),
                Later::Beat(kind, length, look) => {
                    if !self.save.settings.reduce_motion
                        && let Some(creature) = self.save.creatures.iter().find(|c| c.id == id)
                        && beats::free_for_a_beat(self, creature)
                        && let Some(creature) = creature_mut(&mut self.save.creatures, id)
                    {
                        creature.state.beat = Some(Beat {
                            look,
                            ..Beat::new(kind, length)
                        });
                    }
                }
            }
        }
        // A mishap is answered once it is over, so the blush follows the fright.
        let in_mishap: BTreeSet<CreatureId> = self
            .save
            .creatures
            .iter()
            .filter(|c| {
                c.state.beat.is_some_and(|beat| {
                    matches!(
                        beat.kind,
                        BeatKind::LeafOnFace | BeatKind::DroppedSnack | BeatKind::MissedCushion
                    )
                })
            })
            .map(|c| c.id)
            .collect();
        let over: Vec<CreatureId> = self
            .antics
            .mishaps
            .difference(&in_mishap)
            .copied()
            .collect();
        self.antics.mishaps = in_mishap;
        for id in over {
            self.antics.cue(Cue::Mishap(id));
        }
        let cues = std::mem::take(&mut self.antics.cues);
        for (cue, waited) in cues {
            if !self.answer(cue) && waited + dt < CUE_PATIENCE_SECS {
                self.antics.cues.push((cue, waited + dt));
            }
        }
        // Worked out once for each companion, and forgotten when it leaves.
        let creatures = &self.save.creatures;
        self.antics
            .everyday
            .retain(|id, _| creatures.iter().any(|c| c.id == *id));
        for creature in creatures {
            let known = self.antics.everyday.get(&creature.id);
            if known.is_none_or(|(temperament, _)| *temperament != creature.temperament) {
                let character = Character::of(creature);
                self.antics.everyday.insert(
                    creature.id,
                    (
                        creature.temperament,
                        (character.troublemaker(), character.impatient()),
                    ),
                );
            }
        }
        self.antics.prank_in -= dt;
        if self.antics.prank_in <= 0.0 {
            self.antics.prank_in = self.antics.rng.random_range(PRANK_INTERVAL_SECS);
            self.start_prank();
        }
        self.tap_a_foot();
    }

    /// Whether a companion can have one of these moments now: free for a beat, and not still
    /// having a while to itself after the last one.
    fn antic_ready(&self, creature: &Creature) -> bool {
        !self.antics.cooldowns.contains_key(&creature.id) && beats::free_for_a_beat(self, creature)
    }

    /// Holds a companion in a beat, turned toward `look` if it names a place, with a bubble over
    /// it, and gives it a while to itself afterwards. Reduced motion keeps the bubble and leaves
    /// out the pose.
    fn play_antic(
        &mut self,
        id: CreatureId,
        kind: BeatKind,
        length: f32,
        look: Option<Point>,
        bubble: Option<BubbleIcon>,
    ) {
        let cooldown = self.antics.rng.random_range(COOLDOWN_SECS);
        self.antics.cooldowns.insert(id, cooldown);
        if let Some(icon) = bubble {
            self.show_bubble(id, icon);
        }
        if self.save.settings.reduce_motion {
            return;
        }
        let Some(creature) = creature_mut(&mut self.save.creatures, id) else {
            return;
        };
        if let Some(look) = look
            && (look.x - creature.state.position.x).abs() > 1.0
        {
            creature.state.facing_right = look.x > creature.state.position.x;
        }
        creature.state.beat = Some(Beat {
            look,
            ..Beat::new(kind, length)
        });
    }

    /// Everyone near `around`, on its display, who is free for a moment of their own.
    fn nearby_ready(&self, around: CreatureId) -> Vec<CreatureId> {
        let Some(centre) = self.save.creatures.iter().find(|c| c.id == around) else {
            return Vec::new();
        };
        let reach = CREATURE_ART_WIDTH * REACH_FRAMES;
        self.save
            .creatures
            .iter()
            .filter(|other| {
                other.id != around
                    && other.state.surface.monitor_id == centre.state.surface.monitor_id
                    && other.state.position.distance(centre.state.position) <= reach
                    && self.antic_ready(other)
            })
            .map(|other| other.id)
            .collect()
    }

    fn position_of(&self, id: CreatureId) -> Option<Point> {
        self.save
            .creatures
            .iter()
            .find(|c| c.id == id)
            .map(|c| c.state.position)
    }

    /// Answers one cue. False while nobody it concerns is free yet, so it can wait a little.
    fn answer(&mut self, cue: Cue) -> bool {
        match cue {
            // Somebody else got the pet: a jealous one nearby huffs about it.
            Cue::Petted(petted) => {
                let look = self.position_of(petted);
                let jealous = self
                    .nearby_ready(petted)
                    .into_iter()
                    .find(|id| self.character_of(*id).is_some_and(|c| c.jealous()));
                if let Some(id) = jealous
                    && self.antics.rng.random_bool(0.75)
                {
                    self.play_antic(id, BeatKind::Huff, 2.6, look, Some(BubbleIcon::Jealous));
                }
                true
            }
            // Somebody is eating: a food-lover nearby sits up and begs.
            Cue::Eating(eater) => {
                let look = self.position_of(eater);
                let hungry = self
                    .nearby_ready(eater)
                    .into_iter()
                    .find(|id| self.character_of(*id).is_some_and(|c| c.food_lover()));
                if let Some(id) = hungry
                    && self.antics.rng.random_bool(0.65)
                {
                    self.play_antic(id, BeatKind::Beg, 2.8, look, Some(BubbleIcon::Snack));
                }
                true
            }
            // A fright: a dramatic one swoons, a jumpy one hides behind its paws.
            Cue::Startled(id) => self.in_character(id, |world, character| {
                if character.dramatic() {
                    world.play_antic(id, BeatKind::Swoon, 2.8, None, Some(BubbleIcon::Swoon));
                } else if character.jumpy() {
                    world.play_antic(id, BeatKind::Peek, 2.0, None, Some(BubbleIcon::Surprise));
                }
            }),
            // Up on a window, or a find held up: a show-off strikes a pose for whoever is looking.
            Cue::Climbed(id) | Cue::Found(id) => self.in_character(id, |world, character| {
                if character.show_off() && world.antics.rng.random_bool(0.7) {
                    world.play_antic(id, BeatKind::Strut, 2.2, None, Some(BubbleIcon::Sparkle));
                }
            }),
            // A mishap: one that embarrasses easily hides behind its paws.
            Cue::Mishap(id) => self.in_character(id, |world, character| {
                if character.embarrassable() {
                    world.play_antic(id, BeatKind::Peek, 2.0, None, Some(BubbleIcon::Blush));
                }
            }),
            // It said no, and a stubborn one means it.
            Cue::Declined(id) => self.in_character(id, |world, character| {
                if character.stubborn() {
                    world.play_antic(id, BeatKind::Stomp, 1.8, None, None);
                }
            }),
        }
    }

    fn character_of(&self, id: CreatureId) -> Option<Character> {
        self.save
            .creatures
            .iter()
            .find(|c| c.id == id)
            .map(Character::of)
    }

    /// Runs `react` for a companion once it is free, and says whether it has been.
    fn in_character(&mut self, id: CreatureId, react: impl FnOnce(&mut Self, &Character)) -> bool {
        let Some(creature) = self.save.creatures.iter().find(|c| c.id == id) else {
            return true;
        };
        if self.antics.cooldowns.contains_key(&id) {
            return true;
        }
        if !beats::free_for_a_beat(self, creature) {
            return false;
        }
        let character = Character::of(creature);
        react(self, &character);
        true
    }

    /// A troublemaker creeps up on a friend who is resting nearby and pounces; the friend jumps.
    fn start_prank(&mut self) {
        let pranksters: Vec<CreatureId> = self
            .save
            .creatures
            .iter()
            .filter(|c| {
                self.antics
                    .everyday
                    .get(&c.id)
                    .is_some_and(|(_, flags)| flags.0)
            })
            .filter(|c| self.antic_ready(c))
            .map(|c| c.id)
            .collect();
        for prankster in pranksters {
            let target = self.nearby_ready(prankster).into_iter().find(|id| {
                self.save.creatures.iter().any(|c| {
                    c.id == *id && matches!(c.state.action, ActionKind::Idle | ActionKind::Perch)
                })
            });
            let Some(target) = target else {
                continue;
            };
            let (at_prankster, at_target) = (self.position_of(prankster), self.position_of(target));
            self.play_antic(
                prankster,
                BeatKind::Pounce,
                1.8,
                at_target,
                Some(BubbleIcon::Music),
            );
            // The friend has no idea until the pounce lands, and then it jumps.
            let cooldown = self.antics.rng.random_range(COOLDOWN_SECS);
            self.antics.cooldowns.insert(target, cooldown);
            self.antics.later.push((
                target,
                Later::Beat(BeatKind::Startle, 1.0, at_prankster),
                1.0,
            ));
            self.antics.bubble_later(target, BubbleIcon::Surprise, 1.0);
            return;
        }
    }

    /// An impatient companion that has stood about for a while taps its foot.
    fn tap_a_foot(&mut self) {
        let waiting = self.save.creatures.iter().find(|c| {
            c.state.action == ActionKind::Idle
                && c.state.action_elapsed >= IMPATIENT_AFTER_SECS
                && self
                    .antics
                    .everyday
                    .get(&c.id)
                    .is_some_and(|(_, flags)| flags.1)
                && self.antic_ready(c)
        });
        if let Some(id) = waiting.map(|c| c.id)
            && self.antics.rng.random_bool(0.02)
        {
            self.play_antic(id, BeatKind::Stomp, 1.8, None, Some(BubbleIcon::Ellipsis));
        }
    }
}
