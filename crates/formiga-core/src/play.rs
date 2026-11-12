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
    /// Whether this is a paper plane, which is thrown once and lands, never flown on a string.
    pub fn is_plane(self) -> bool {
        match self {
            Self::Belonging(kind) => kind == ColonyObjectKind::Paper,
            Self::Trinket(variant) => {
                trinket_info(variant).is_some_and(|info| info.name == "Paper plane")
            }
            Self::Toy(_) => false,
        }
    }

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
    /// Where something thrown came down, watched for a moment from where it was thrown: how a
    /// paper plane's flight ends, rather than with it back in the paws.
    Landed,
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

/// The most moves a session can hold: a pick-up, two towers built and knocked down, and a
/// put-down, with room to spare.
const MAX_MOVES: usize = 12;

/// How much longer than its time a session may be at its natural pace before a whole round of it
/// is left out, rather than everything in it hurried.
const MOST_HURRIED: f32 = 1.3;

/// A session of play under way.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Play {
    pub thing: Plaything,
    pub shape: PlayShape,
    /// Seconds since it began.
    pub elapsed: f32,
    pub length: f32,
    /// The colony belonging this is, by its id, while it is borrowed from the yard: the yard
    /// leaves it out until it is put back, so it is only ever in one place.
    pub borrowed: Option<u64>,
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

    /// Lays out a session with `thing` that fills `length` seconds: picked up, one to three rounds
    /// of its shape's moves, and put down. Every round is played whole — a tower is built up and
    /// knocked down, a line cast and reeled in — and a round that does not fit is left out rather
    /// than cut short. Something thrown is thrown last and watched where it lands.
    pub fn plan<R: Rng + ?Sized>(thing: Plaything, length: f32, rng: &mut R) -> Self {
        let shape = thing.shape().unwrap_or(PlayShape::Round);
        let opening = (PlayMove::PickUp, 0.8);
        let mut rounds: Vec<Vec<(PlayMove, f32)>> = Vec::new();
        let wanted = if length > 6.0 { 3 } else { 2 };
        // A paper plane is only ever thrown, and anything else flown may be thrown at the end.
        let thrown = shape == PlayShape::Flying && (thing.is_plane() || rng.random_bool(0.5));
        let choices: &[PlayMove] = if shape == PlayShape::Flying {
            &[PlayMove::Fly]
        } else {
            moves_of(shape)
        };
        let middle = if thing.is_plane() {
            0
        } else {
            wanted - usize::from(thrown)
        };
        let mut last = None;
        for _ in 0..middle {
            let mut pick = choices[rng.random_range(0..choices.len())];
            if Some(pick) == last && choices.len() > 1 {
                pick = choices
                    [(choices.iter().position(|m| *m == pick).unwrap_or(0) + 1) % choices.len()];
            }
            last = Some(pick);
            rounds.push(match pick {
                // A line is cast and waited on, and a bite comes, or does not.
                PlayMove::Fish => {
                    let mut round = vec![(PlayMove::Fish, rng.random_range(1.8..2.8))];
                    if rng.random_bool(0.6) {
                        round.push((PlayMove::Catch, 1.2));
                    }
                    round
                }
                // A tower goes up a block at a time, and comes down.
                PlayMove::Stack => vec![
                    (PlayMove::Stack, 0.8),
                    (PlayMove::Stack, 0.8),
                    (PlayMove::Stack, 0.8),
                    (PlayMove::Topple, 1.0),
                ],
                // Something set spinning, or a kite up, is watched a while.
                PlayMove::Spin | PlayMove::Fly | PlayMove::Sail => {
                    vec![(pick, rng.random_range(2.0..3.0))]
                }
                _ => vec![(pick, rng.random_range(1.4..2.2))],
            });
        }
        let ending = if thrown {
            vec![
                (PlayMove::Throw, rng.random_range(1.6..2.2)),
                (PlayMove::Landed, 1.0),
            ]
        } else {
            vec![(PlayMove::PutDown, 0.8)]
        };
        // Whole rounds come off the end until what is left fits the table and, at its own pace,
        // nearly fits the time; the pace then takes up the rest.
        let size = |rounds: &[Vec<(PlayMove, f32)>]| {
            1 + rounds.iter().map(Vec::len).sum::<usize>() + ending.len()
        };
        let natural = |rounds: &[Vec<(PlayMove, f32)>]| {
            opening.1
                + rounds.iter().flatten().map(|(_, secs)| secs).sum::<f32>()
                + ending.iter().map(|(_, secs)| secs).sum::<f32>()
        };
        while rounds.len() > 1
            && (size(&rounds) > MAX_MOVES || natural(&rounds) > length * MOST_HURRIED)
        {
            rounds.pop();
        }
        let kept: Vec<(PlayMove, f32)> = std::iter::once(opening)
            .chain(rounds.into_iter().flatten())
            .chain(ending)
            .take(MAX_MOVES)
            .collect();
        let total: f32 = kept.iter().map(|(_, secs)| secs).sum();
        let scale = length.max(0.5) / total.max(0.01);
        let mut laid = [(PlayMove::PickUp, 0.0); MAX_MOVES];
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
            borrowed: None,
            moves: laid,
            count: kept.len() as u8,
        }
    }

    /// Keeps the picking up going until `until` seconds in, fitting the rest of the game into the
    /// time left: for a companion that bows before a game, so the bow comes first and the game
    /// after it, rather than over the top of its opening.
    pub fn pick_up_until(&mut self, until: f32) {
        let count = usize::from(self.count);
        if count < 2 || self.moves[0].0 != PlayMove::PickUp {
            return;
        }
        let old = self.moves[1].1;
        let until = until.min(self.length * 0.5);
        if until <= old || self.elapsed >= old {
            return;
        }
        let scale = (self.length - until) / (self.length - old).max(0.01);
        for (_, start) in &mut self.moves[1..count] {
            *start = until + (*start - old) * scale;
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
            borrowed: None,
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

    /// How long the move under way lasts, in seconds.
    pub fn move_length(&self) -> f32 {
        let moves = self.moves();
        let index = moves
            .iter()
            .rposition(|(_, start)| *start <= self.elapsed)
            .unwrap_or(0);
        let end = moves.get(index + 1).map_or(self.length, |(_, next)| *next);
        (end - moves[index].1).max(0.01)
    }

    /// How many blocks are up, while a tower is being built: counted from the last time it came
    /// down, so a second tower goes up from one again.
    pub fn tower(&self) -> u8 {
        let mut blocks = 0_u8;
        for (step, _) in self
            .moves()
            .iter()
            .filter(|(_, start)| *start <= self.elapsed)
        {
            match step {
                PlayMove::Stack => blocks = blocks.saturating_add(1),
                PlayMove::Topple => blocks = 0,
                _ => {}
            }
        }
        blocks.min(3)
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
            let last = moves.last().unwrap().0;
            if thing.shape() == Some(PlayShape::Flying) {
                assert!(matches!(last, PlayMove::PutDown | PlayMove::Landed));
            } else {
                assert_eq!(last, PlayMove::PutDown);
            }
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

    /// A tower is built up whole and knocked down whole, and a second one goes up from one block
    /// again: never a tower of four, and never one left standing half-built at the put-down.
    #[test]
    fn every_tower_goes_up_one_two_three_and_comes_down() {
        let mut seconds = 0;
        for seed in 0..80_u64 {
            let length = 4.0 + (seed % 9) as f32;
            let mut play = Play::plan(
                Plaything::Toy(3),
                length,
                &mut ChaCha12Rng::seed_from_u64(seed),
            );
            let moves: Vec<PlayMove> = play.moves().iter().map(|(step, _)| *step).collect();
            let middle = &moves[1..moves.len() - 1];
            assert_eq!(middle.len() % 4, 0, "{moves:?}");
            for round in middle.chunks(4) {
                assert_eq!(
                    round,
                    [
                        PlayMove::Stack,
                        PlayMove::Stack,
                        PlayMove::Stack,
                        PlayMove::Topple
                    ]
                );
            }
            if middle.len() == 8 {
                seconds += 1;
            }
            let mut heights = Vec::new();
            while play.elapsed < play.length {
                if play.now().0 == PlayMove::Stack {
                    heights.push(play.tower());
                }
                play.elapsed += 0.05;
            }
            assert!(heights.iter().all(|h| (1..=3).contains(h)), "{heights:?}");
            // The put-down is a moment, not most of the game.
            let (_, put_down) = *play.moves().last().unwrap();
            assert!(play.length - put_down < length * 0.25, "{:?}", play.moves());
        }
        assert!(seconds > 0, "no session ever built a second tower");
    }

    /// A paper plane is picked up, thrown once and watched where it lands; it never comes back to
    /// the paws.
    #[test]
    fn a_paper_plane_is_thrown_once_and_lands() {
        for seed in 0..40_u64 {
            let play = Play::plan(
                Plaything::Belonging(ColonyObjectKind::Paper),
                3.0 + (seed % 8) as f32,
                &mut ChaCha12Rng::seed_from_u64(seed),
            );
            let moves: Vec<PlayMove> = play.moves().iter().map(|(step, _)| *step).collect();
            assert_eq!(
                moves,
                [PlayMove::PickUp, PlayMove::Throw, PlayMove::Landed],
                "seed {seed}"
            );
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
