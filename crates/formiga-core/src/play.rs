//! Play: what a companion plays with, what shape it is, and how a thing of that shape is played
//! with, laid out from picking it up to putting it down.
//!
//! Every plaything has a shape, and the shape decides the game: something round is tossed and
//! caught, bounced and balanced; a stick is swished like a sword, leaned on like a cane or fished
//! with; an umbrella is twirled, sat in like a boat, or ridden down from a ledge; a kite is flown,
//! a paper plane thrown, a top spun, a drum drummed, a plush hugged, a tower of blocks built and
//! knocked down, a paper boat sailed. A companion plays with its own toy, with one of the colony's
//! belongings, or with something the colony has found, borrowed from the scrapbook for a while.
//! Runtime only, like a meal: nothing here is saved, and nothing in the simulation reads it.

use crate::{ColonyObjectKind, trinket_info};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha12Rng;

/// What shape a plaything is, which is how it is played with.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PlayShape {
    Round,
    Stick,
    Umbrella,
    Flying,
    Spinning,
    Noisy,
    Soft,
    Stacking,
    Floating,
}

/// Which of the eight toys a companion was born with, from the genes its looks are drawn from:
/// the toy the art has always shown it playing with.
pub fn own_toy(appearance: &crate::AppearanceGenome) -> u8 {
    ((appearance.marking_seed ^ u64::from(appearance.face_signature)) % 8) as u8
}

/// Something to play with.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Plaything {
    /// The companion's own toy, by its kind: a ball, a top, a plush, a block, a yo-yo, a rattle,
    /// a pinwheel or a hoop.
    Toy(u8),
    /// One of the colony's belongings.
    Belonging(ColonyObjectKind),
    /// Something the colony has found, by its variant.
    Trinket(u8),
}

impl Plaything {
    /// Its shape, if it is something to play with at all.
    pub fn shape(self) -> Option<PlayShape> {
        use PlayShape::*;
        match self {
            Self::Toy(kind) => Some(match kind % 8 {
                0 | 7 => Round,
                1 | 4 | 6 => Spinning,
                2 => Soft,
                3 => Stacking,
                _ => Noisy,
            }),
            Self::Belonging(kind) => match kind {
                ColonyObjectKind::Toy | ColonyObjectKind::YarnBall | ColonyObjectKind::Pebble => {
                    Some(Round)
                }
                ColonyObjectKind::Umbrella => Some(Umbrella),
                ColonyObjectKind::Kite | ColonyObjectKind::Paper => Some(Flying),
                ColonyObjectKind::SpinningTop => Some(Spinning),
                ColonyObjectKind::Drum | ColonyObjectKind::MusicBox => Some(Noisy),
                ColonyObjectKind::Pillow | ColonyObjectKind::Blanket => Some(Soft),
                _ => None,
            },
            Self::Trinket(variant) => trinket_shape(variant),
        }
    }
}

/// The shape of a found thing, read from what it is. A find that is not for playing with — a
/// stamp, a key, a gem — has none.
pub fn trinket_shape(variant: u8) -> Option<PlayShape> {
    use PlayShape::*;
    let name = trinket_info(variant)?.name;
    Some(match name {
        "Marble" | "Rainbow marble" | "Acorn" | "Golden acorn" | "Walnut" | "Pinecone"
        | "Glass bead" | "Holey pebble" | "Heart pebble" | "Button" | "Pyjama button" => Round,
        "Twig" | "Long feather" | "Owl feather" | "Crystal feather" | "Knitting needle"
        | "Walking stick" | "Pencil stub" | "Crayon" | "Chalk stub" | "Little broom"
        | "Bubble wand" | "Nest twig" | "Weathervane arrow" | "Pearl hat pin" => Stick,
        "Leaf" => Umbrella,
        "Paper plane" | "Kite tail" | "Diamond kite" | "Dandelion clock" | "Balloon string"
        | "Party streamer" | "Ribbon" | "Seed wing" | "Little pinwheel" => Flying,
        "Yo-yo" | "Spool" | "Toy wheel" => Spinning,
        "Tiny bell" | "Silver bell" | "Tin whistle" | "Chime bar" => Noisy,
        "Toy sheep" | "Tiny mitten" | "Striped sock" | "Blanket fluff" | "Scrap of wool"
        | "Patchwork square" | "Slipper" => Soft,
        "Toy block" | "Domino" | "Die" | "Little brick" | "Chess pawn" => Stacking,
        "Paper boat" | "Cork" => Floating,
        _ => return None,
    })
}

/// One move in a session of play.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PlayMove {
    /// Picked up and looked over.
    PickUp,
    /// Knocked up off the paw and caught again, over and over.
    KeepUp,
    /// Thrown high, watched, and caught.
    TossHigh,
    /// Bounced on the ground and caught.
    Bounce,
    /// Balanced on the head, wobbling.
    Balance,
    /// Swished about like a sword.
    Swish,
    /// Leaned on like a cane, strutting.
    Cane,
    /// The line cast, and the wait for a bite.
    Fish,
    /// Something on the line, pulled in.
    Catch,
    /// Waved in time like a conductor's baton.
    Conduct,
    /// Held overhead and twirled.
    Twirl,
    /// Turned upside down and sat in like a boat.
    Boat,
    /// Held up high on its string.
    Fly,
    /// Thrown, and watched as it glides away.
    Throw,
    /// Set spinning, and watched.
    Spin,
    /// Drummed, rung or wound, in time.
    Music,
    /// Danced to.
    Dance,
    /// Hugged close.
    Hug,
    /// Stacked a little higher.
    Stack,
    /// Knocked down.
    Topple,
    /// Floated and watched as it bobs.
    Sail,
    /// Put down again with a pleased look.
    PutDown,
    /// Holding an open umbrella overhead on the way down from a ledge, floating.
    Ride,
}

/// The moves each shape is played with: one or two of the middle ones make each session.
fn moves_of(shape: PlayShape) -> &'static [PlayMove] {
    use PlayMove::*;
    match shape {
        PlayShape::Round => &[KeepUp, TossHigh, Bounce, Balance],
        PlayShape::Stick => &[Swish, Cane, Fish, Conduct],
        PlayShape::Umbrella => &[Twirl, Boat],
        PlayShape::Flying => &[Fly, Throw],
        PlayShape::Spinning => &[Spin, KeepUp],
        PlayShape::Noisy => &[Music, Dance],
        PlayShape::Soft => &[Hug, TossHigh],
        PlayShape::Stacking => &[Stack],
        PlayShape::Floating => &[Sail],
    }
}

const MAX_MOVES: usize = 8;

/// A session of play under way.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Play {
    pub thing: Plaything,
    pub shape: PlayShape,
    /// Seconds since it began.
    pub elapsed: f32,
    pub length: f32,
    moves: [(PlayMove, f32); MAX_MOVES],
    count: u8,
}

impl Play {
    /// Chooses what to play with: usually its own toy, sometimes one of the colony's belongings,
    /// sometimes a find borrowed from the scrapbook.
    pub fn choose_thing<R: Rng + ?Sized>(
        own_toy: u8,
        belongings: &[ColonyObjectKind],
        finds: &[u8],
        rng: &mut R,
    ) -> Plaything {
        let belongings: Vec<Plaything> = belongings
            .iter()
            .map(|kind| Plaything::Belonging(*kind))
            .filter(|thing| thing.shape().is_some())
            .collect();
        let finds: Vec<Plaything> = finds
            .iter()
            .map(|variant| Plaything::Trinket(*variant))
            .filter(|thing| thing.shape().is_some())
            .collect();
        let roll: f32 = rng.random();
        if roll < 0.3 && !belongings.is_empty() {
            return belongings[rng.random_range(0..belongings.len())];
        }
        if roll < 0.55 && !finds.is_empty() {
            return finds[rng.random_range(0..finds.len())];
        }
        Plaything::Toy(own_toy)
    }

    /// Lays out a session with `thing` that fills `length` seconds: picked up, one to three of
    /// its shape's moves, and put down.
    pub fn plan<R: Rng + ?Sized>(thing: Plaything, length: f32, rng: &mut R) -> Self {
        let shape = thing.shape().unwrap_or(PlayShape::Round);
        let mut moves: Vec<(PlayMove, f32)> = vec![(PlayMove::PickUp, 0.8)];
        let choices = moves_of(shape);
        let wanted = if length > 6.0 { 3 } else { 2 };
        let mut last = None;
        for _ in 0..wanted {
            let mut pick = choices[rng.random_range(0..choices.len())];
            if Some(pick) == last && choices.len() > 1 {
                pick = choices
                    [(choices.iter().position(|m| *m == pick).unwrap_or(0) + 1) % choices.len()];
            }
            last = Some(pick);
            match pick {
                // A line is cast and waited on, and a bite comes, or does not.
                PlayMove::Fish => {
                    moves.push((PlayMove::Fish, rng.random_range(1.8..2.8)));
                    if rng.random_bool(0.6) {
                        moves.push((PlayMove::Catch, 1.2));
                    }
                }
                // A tower goes up a block at a time, and comes down.
                PlayMove::Stack => {
                    for _ in 0..3 {
                        moves.push((PlayMove::Stack, 0.8));
                    }
                    moves.push((PlayMove::Topple, 1.0));
                }
                // Something set spinning, or a kite up, is watched a while.
                PlayMove::Spin | PlayMove::Fly | PlayMove::Sail => {
                    moves.push((pick, rng.random_range(2.0..3.0)))
                }
                _ => moves.push((pick, rng.random_range(1.4..2.2))),
            }
        }
        moves.push((PlayMove::PutDown, 0.8));
        let total: f32 = moves.iter().map(|(_, secs)| secs).sum();
        let scale = length.max(0.5) / total.max(0.01);
        let mut laid = [(PlayMove::PickUp, 0.0); MAX_MOVES];
        let kept: Vec<(PlayMove, f32)> = if moves.len() > MAX_MOVES {
            let last = moves[moves.len() - 1];
            moves[..MAX_MOVES - 1]
                .iter()
                .copied()
                .chain([last])
                .collect()
        } else {
            moves
        };
        let mut start = 0.0;
        for (slot, (step, secs)) in laid.iter_mut().zip(&kept) {
            *slot = (*step, start);
            start += secs * scale;
        }
        Self {
            thing,
            shape,
            elapsed: 0.0,
            length: length.max(0.5),
            moves: laid,
            count: kept.len() as u8,
        }
    }

    /// Riding an umbrella down from a ledge, for as long as the drop takes.
    pub fn umbrella_ride(length: f32) -> Self {
        let mut moves = [(PlayMove::PickUp, 0.0); MAX_MOVES];
        moves[0] = (PlayMove::Ride, 0.0);
        Self {
            thing: Plaything::Belonging(ColonyObjectKind::Umbrella),
            shape: PlayShape::Umbrella,
            elapsed: 0.0,
            length: length.max(0.1),
            moves,
            count: 1,
        }
    }

    /// [`Self::choose_thing`] and [`Self::plan`] in one, from a seed.
    pub fn seeded(
        own_toy: u8,
        belongings: &[ColonyObjectKind],
        finds: &[u8],
        length: f32,
        seed: u64,
    ) -> Self {
        let mut rng = ChaCha12Rng::seed_from_u64(seed);
        let thing = Self::choose_thing(own_toy, belongings, finds, &mut rng);
        Self::plan(thing, length, &mut rng)
    }

    /// The move under way, how many seconds into it, and how far through it from 0 to 1.
    pub fn now(&self) -> (PlayMove, f32, f32) {
        let moves = self.moves();
        let index = moves
            .iter()
            .rposition(|(_, start)| *start <= self.elapsed)
            .unwrap_or(0);
        let (step, start) = moves[index];
        let end = moves.get(index + 1).map_or(self.length, |(_, next)| *next);
        let into = (self.elapsed - start).max(0.0);
        (step, into, (into / (end - start).max(0.01)).clamp(0.0, 1.0))
    }

    /// How many blocks are up, while a tower is being built.
    pub fn tower(&self) -> u8 {
        self.moves()
            .iter()
            .filter(|(step, start)| *step == PlayMove::Stack && *start <= self.elapsed)
            .count() as u8
    }

    pub fn moves(&self) -> &[(PlayMove, f32)] {
        &self.moves[..usize::from(self.count.max(1))]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_toy_and_most_finds_have_a_shape_and_some_finds_do_not() {
        for kind in 0..8 {
            assert!(Plaything::Toy(kind).shape().is_some());
        }
        let shaped = (0..crate::TRINKET_VARIANTS)
            .filter(|variant| trinket_shape(*variant).is_some())
            .count();
        assert!(shaped >= 50, "only {shaped} finds can be played with");
        assert_eq!(trinket_shape(0), None, "a gem is not a toy");
        let named = |name: &str| {
            crate::all_trinkets()
                .find(|info| info.name == name)
                .map(|info| info.variant)
                .unwrap()
        };
        assert_eq!(trinket_shape(named("Marble")), Some(PlayShape::Round));
        assert_eq!(trinket_shape(named("Twig")), Some(PlayShape::Stick));
        assert_eq!(trinket_shape(named("Paper plane")), Some(PlayShape::Flying));
        assert_eq!(
            Plaything::Belonging(ColonyObjectKind::Umbrella).shape(),
            Some(PlayShape::Umbrella)
        );
    }

    #[test]
    fn a_session_is_picked_up_played_with_by_shape_and_put_down() {
        for seed in 0..120_u64 {
            let mut rng = ChaCha12Rng::seed_from_u64(seed);
            let thing = [
                Plaything::Toy(0),
                Plaything::Toy(3),
                Plaything::Belonging(ColonyObjectKind::Umbrella),
                Plaything::Belonging(ColonyObjectKind::Kite),
            ][seed as usize % 4];
            let length = rng.random_range(3.0..8.0);
            let play = Play::plan(thing, length, &mut rng);
            let moves = play.moves();
            assert_eq!(moves[0].0, PlayMove::PickUp);
            assert_eq!(moves.last().unwrap().0, PlayMove::PutDown);
            assert!(moves.windows(2).all(|pair| pair[0].1 < pair[1].1));
            assert!(moves.last().unwrap().1 < length);
            let shape = thing.shape().unwrap();
            for (step, _) in &moves[1..moves.len() - 1] {
                assert!(
                    moves_of(shape).contains(step)
                        || matches!(step, PlayMove::Catch | PlayMove::Topple),
                    "{shape:?} played with by {step:?}"
                );
            }
        }
    }

    #[test]
    fn belongings_and_finds_get_played_with_too() {
        let mut rng = ChaCha12Rng::seed_from_u64(9);
        let mut seen = std::collections::BTreeSet::new();
        for _ in 0..200 {
            let thing = Play::choose_thing(2, &[ColonyObjectKind::Kite], &[16], &mut rng);
            seen.insert(format!("{thing:?}"));
        }
        assert_eq!(seen.len(), 3, "{seen:?}");
    }
}
