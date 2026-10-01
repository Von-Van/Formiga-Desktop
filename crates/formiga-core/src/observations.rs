//! What the notebook can honestly say it has noticed about the colony: a companion that keeps to
//! high places, two that keep seeking one another out. Every observation is read from counts the
//! colony keeps of things that really happened, and carries those counts as its evidence, so the
//! notebook can always show why it says what it says. Nothing is inferred from a creature's
//! nature alone, nothing is estimated for time before a count was kept, and below a threshold
//! there is simply nothing to say.
//!
//! This reads the save and nothing else. It is not stored: the same colony always gives the same
//! observations, in the same order.
use crate::{CreatureId, DisplayKey, SaveFile};

/// One thing noticed, and the evidence for it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Observation {
    /// Spends more time up on window ledges than anyone else here, and climbs to get there.
    HighPlaces {
        creature: CreatureId,
        ledge_seconds: u32,
        climbs: u32,
    },
    /// Rides along on moving windows more than anyone else here.
    WindowRider {
        creature: CreatureId,
        ride_seconds: u32,
    },
    /// The first to find more of the scrapbook than anyone else here.
    Finder {
        creature: CreatureId,
        firsts: u32,
        scrapbook: u32,
    },
    /// Has slept a long stretch without stirring.
    SoundSleeper {
        creature: CreatureId,
        longest_seconds: u32,
    },
    /// Keeps to one part of one display.
    KeepsToAPlace {
        creature: CreatureId,
        display: DisplayKey,
        cell: u8,
        confidence: u8,
    },
    /// Plays more than anyone else here.
    Playful { creature: CreatureId, sessions: u32 },
    /// Each has gone looking for the other, again and again.
    SeekEachOther {
        a: CreatureId,
        b: CreatureId,
        a_sought: u16,
        b_sought: u16,
    },
    /// One goes looking for the other far more than the other way round.
    FollowsAround {
        seeker: CreatureId,
        sought: CreatureId,
        times: u16,
        returned: u16,
    },
    /// Nap side by side.
    NapTogether {
        a: CreatureId,
        b: CreatureId,
        naps: u16,
    },
    /// Play together.
    PlayTogether {
        a: CreatureId,
        b: CreatureId,
        games: u16,
    },
    /// Bring each other what they find.
    ShareFinds {
        a: CreatureId,
        b: CreatureId,
        gifts: u16,
    },
    /// Have their squabbles, more than they play.
    Squabble {
        a: CreatureId,
        b: CreatureId,
        squabbles: u16,
        games: u16,
    },
    /// A visitor the guest book has seen more than once.
    RegularVisitor { name: String, visits: u16 },
}

impl Observation {
    /// Who the observation is about.
    pub fn subjects(&self) -> Vec<CreatureId> {
        match *self {
            Self::HighPlaces { creature, .. }
            | Self::WindowRider { creature, .. }
            | Self::Finder { creature, .. }
            | Self::SoundSleeper { creature, .. }
            | Self::KeepsToAPlace { creature, .. }
            | Self::Playful { creature, .. } => vec![creature],
            Self::SeekEachOther { a, b, .. }
            | Self::NapTogether { a, b, .. }
            | Self::PlayTogether { a, b, .. }
            | Self::ShareFinds { a, b, .. }
            | Self::Squabble { a, b, .. } => vec![a, b],
            Self::FollowsAround { seeker, sought, .. } => vec![seeker, sought],
            Self::RegularVisitor { .. } => Vec::new(),
        }
    }

    /// Whether this is about one particular companion.
    pub fn concerns(&self, creature: CreatureId) -> bool {
        self.subjects().contains(&creature)
    }
}

/// Seconds on ledges before keeping to high places is worth remarking on: an hour, with at least
/// ten climbs of its own to get there.
pub const HIGH_PLACES_SECONDS: u32 = 60 * 60;
pub const HIGH_PLACES_CLIMBS: u32 = 10;
/// Half an hour riding windows.
pub const WINDOW_RIDER_SECONDS: u32 = 30 * 60;
/// First to find at least five things in the scrapbook.
pub const FINDER_FIRSTS: u32 = 5;
/// Two hours asleep without being disturbed.
pub const SOUND_SLEEPER_SECONDS: u32 = 2 * 60 * 60;
/// How sure the colony has to be of a companion's usual spot before saying so.
pub const PLACE_CONFIDENCE: u8 = 24;
/// Twenty games.
pub const PLAYFUL_SESSIONS: u32 = 20;
/// Each sought the other out at least five times, sixteen between them.
pub const SEEK_EACH: u16 = 5;
pub const SEEK_TOTAL: u16 = 16;
/// Sought out twelve times, at least three times as often as it went the other way.
pub const FOLLOW_TIMES: u16 = 12;
pub const NAPS: u16 = 10;
pub const GAMES: u16 = 30;
pub const GIFTS: u16 = 4;
pub const SQUABBLES: u16 = 8;
/// A visitor seen this many times.
pub const REGULAR_VISITS: usize = 3;

/// Everything the colony's own records support saying, companions first in colony order, then
/// pairs, then visitors. Deterministic: the same save gives the same list.
pub fn observe(save: &SaveFile) -> Vec<Observation> {
    let creatures = &save.creatures;
    let mut found = Vec::new();
    // "More than anyone else here" means strictly the most: a tie says nothing about either.
    let unique_top = |value: &dyn Fn(&crate::Creature) -> u32, id: CreatureId| {
        let mine = creatures.iter().find(|c| c.id == id).map_or(0, value);
        creatures.iter().all(|c| c.id == id || value(c) < mine)
    };
    for creature in creatures {
        let memory = &creature.memory;
        if memory.ledge_seconds >= HIGH_PLACES_SECONDS
            && memory.window_climbs >= HIGH_PLACES_CLIMBS
            && unique_top(&|c| c.memory.ledge_seconds, creature.id)
        {
            found.push(Observation::HighPlaces {
                creature: creature.id,
                ledge_seconds: memory.ledge_seconds,
                climbs: memory.window_climbs,
            });
        }
        if memory.window_ride_seconds >= WINDOW_RIDER_SECONDS
            && unique_top(&|c| c.memory.window_ride_seconds, creature.id)
        {
            found.push(Observation::WindowRider {
                creature: creature.id,
                ride_seconds: memory.window_ride_seconds,
            });
        }
        let firsts_of = |id: CreatureId| {
            save.companion
                .scrapbook
                .iter()
                .filter(|record| record.finder == Some(id))
                .count() as u32
        };
        let firsts = firsts_of(creature.id);
        if firsts >= FINDER_FIRSTS && unique_top(&|c| firsts_of(c.id), creature.id) {
            found.push(Observation::Finder {
                creature: creature.id,
                firsts,
                scrapbook: save.companion.scrapbook.len() as u32,
            });
        }
        if memory.longest_sleep_seconds >= SOUND_SLEEPER_SECONDS {
            found.push(Observation::SoundSleeper {
                creature: creature.id,
                longest_seconds: memory.longest_sleep_seconds,
            });
        }
        if let Some(place) = memory.preferred_region
            && place.confidence >= PLACE_CONFIDENCE
        {
            found.push(Observation::KeepsToAPlace {
                creature: creature.id,
                display: place.display,
                cell: place.cell,
                confidence: place.confidence,
            });
        }
        if memory.play_sessions >= PLAYFUL_SESSIONS
            && unique_top(&|c| c.memory.play_sessions, creature.id)
        {
            found.push(Observation::Playful {
                creature: creature.id,
                sessions: memory.play_sessions,
            });
        }
    }
    // Pairs, and only pairs who both still live here. Of each kind of thing a pair can be noticed
    // for, only the colony's standout pair is: "the two who nap together most", not every pair
    // past a line. A tie for the most says nothing about either pair.
    let here = |id: CreatureId| creatures.iter().any(|c| c.id == id);
    let mut pairs: Vec<_> = save
        .tallies
        .iter()
        .filter(|r| here(r.a) && here(r.b))
        .collect();
    pairs.sort_by_key(|r| (r.a, r.b));
    let standout = |score: &dyn Fn(&crate::RelationshipTally) -> Option<u32>| {
        let scored: Vec<(u32, &crate::PairTally)> = pairs
            .iter()
            .filter_map(|r| score(&r.tally).map(|value| (value, *r)))
            .collect();
        let best = scored.iter().map(|(value, _)| *value).max()?;
        let mut top = scored.into_iter().filter(|(value, _)| *value == best);
        let first = top.next()?;
        top.next().is_none().then_some(first.1)
    };
    if let Some(r) = standout(&|t| {
        let [a, b] = t.sought;
        (a >= SEEK_EACH && b >= SEEK_EACH && a.saturating_add(b) >= SEEK_TOTAL)
            .then(|| u32::from(a.min(b)) * 1000 + u32::from(a.max(b)))
    }) {
        found.push(Observation::SeekEachOther {
            a: r.a,
            b: r.b,
            a_sought: r.tally.sought[0],
            b_sought: r.tally.sought[1],
        });
    }
    // One-sided: whichever goes looking far more often than it is looked for, the strongest one.
    let one_sided = |t: &crate::RelationshipTally| {
        let [a, b] = t.sought;
        let (times, returned) = (a.max(b), a.min(b));
        (times >= FOLLOW_TIMES && times >= returned.saturating_mul(3)).then_some(u32::from(times))
    };
    if let Some(r) = standout(&one_sided) {
        let [a_sought, b_sought] = r.tally.sought;
        let (seeker, sought, times, returned) = if a_sought >= b_sought {
            (r.a, r.b, a_sought, b_sought)
        } else {
            (r.b, r.a, b_sought, a_sought)
        };
        found.push(Observation::FollowsAround {
            seeker,
            sought,
            times,
            returned,
        });
    }
    if let Some(r) = standout(&|t| (t.shared_rests >= NAPS).then_some(u32::from(t.shared_rests))) {
        found.push(Observation::NapTogether {
            a: r.a,
            b: r.b,
            naps: r.tally.shared_rests,
        });
    }
    if let Some(r) =
        standout(&|t| (t.plays >= GAMES && t.squabbles <= t.plays).then_some(u32::from(t.plays)))
    {
        found.push(Observation::PlayTogether {
            a: r.a,
            b: r.b,
            games: r.tally.plays,
        });
    }
    if let Some(r) = standout(&|t| {
        (t.squabbles >= SQUABBLES && t.squabbles > t.plays).then_some(u32::from(t.squabbles))
    }) {
        found.push(Observation::Squabble {
            a: r.a,
            b: r.b,
            squabbles: r.tally.squabbles,
            games: r.tally.plays,
        });
    }
    if let Some(r) = standout(&|t| (t.gifts >= GIFTS).then_some(u32::from(t.gifts))) {
        found.push(Observation::ShareFinds {
            a: r.a,
            b: r.b,
            gifts: r.tally.gifts,
        });
    }
    // Regular visitors, by the name they last went by, most visits first.
    let mut visitors: Vec<(String, u16, crate::CreatureOrigin)> = Vec::new();
    for entry in save.visitors.guest_book.iter().rev() {
        if visitors
            .iter()
            .any(|(_, _, origin)| *origin == entry.origin)
        {
            continue;
        }
        let visits = save.visitors.visits_of(&entry.origin);
        if visits >= REGULAR_VISITS {
            visitors.push((
                entry.name.clone(),
                u16::try_from(visits).unwrap_or(u16::MAX),
                entry.origin,
            ));
        }
    }
    visitors.sort_by(|x, y| y.1.cmp(&x.1).then_with(|| x.0.cmp(&y.0)));
    found.extend(
        visitors
            .into_iter()
            .map(|(name, visits, _)| Observation::RegularVisitor { name, visits }),
    );
    found
}

/// The observations about one companion, in the order [`observe`] gives them.
pub fn observations_of(save: &SaveFile, creature: CreatureId) -> Vec<Observation> {
    observe(save)
        .into_iter()
        .filter(|observation| observation.concerns(creature))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DesktopSnapshot, RelationshipExperience, World};
    use time::macros::datetime;

    fn colony() -> World {
        let now = datetime!(2026-09-30 12:00 UTC);
        let desktop = DesktopSnapshot::default();
        let mut world = World::new([88; 32], now, &desktop);
        world.tick(now + time::Duration::days(20), 0.05, &desktop);
        assert!(world.save.creatures.len() >= 3);
        world
    }

    #[test]
    fn a_colony_with_no_evidence_has_nothing_to_say() {
        let mut world = colony();
        // However close the pair's scores, scores are not evidence of anything in particular.
        for relationship in &mut world.save.relationships {
            relationship.affinity = 255;
            relationship.familiarity = 255;
            relationship.playfulness = 255;
        }
        assert!(world.save.tallies.is_empty());
        assert_eq!(observe(&world.save), Vec::new());
    }

    #[test]
    fn each_observation_appears_only_once_its_evidence_reaches_the_threshold() {
        let mut world = colony();
        let first = world.save.creatures[0].id;
        let second = world.save.creatures[1].id;
        world.save.creatures[0].memory.ledge_seconds = HIGH_PLACES_SECONDS - 1;
        world.save.creatures[0].memory.window_climbs = HIGH_PLACES_CLIMBS;
        assert!(observe(&world.save).is_empty());
        world.save.creatures[0].memory.ledge_seconds = HIGH_PLACES_SECONDS;
        assert_eq!(
            observe(&world.save),
            vec![Observation::HighPlaces {
                creature: first,
                ledge_seconds: HIGH_PLACES_SECONDS,
                climbs: HIGH_PLACES_CLIMBS,
            }]
        );
        // A tie for the most says nothing about either of them.
        world.save.creatures[1].memory.ledge_seconds = HIGH_PLACES_SECONDS;
        assert!(observe(&world.save).is_empty());
        world.save.creatures[1].memory.ledge_seconds = 0;

        let at = datetime!(2026-10-01 9:00 UTC);
        let pair = crate::tally_mut_or_insert(&mut world.save.tallies, first, second).unwrap();
        let first_is_a = pair.a == first;
        for _ in 0..SEEK_TOTAL / 2 - 1 {
            pair.tally
                .count(Some(true), RelationshipExperience::Followed, at);
            pair.tally
                .count(Some(false), RelationshipExperience::Followed, at);
        }
        // Seven each, fourteen between them: not yet "again and again".
        assert!(
            !observe(&world.save)
                .iter()
                .any(|o| matches!(o, Observation::SeekEachOther { .. }))
        );
        let pair = crate::tally_mut_or_insert(&mut world.save.tallies, first, second).unwrap();
        pair.tally
            .count(Some(first_is_a), RelationshipExperience::Followed, at);
        pair.tally
            .count(Some(!first_is_a), RelationshipExperience::Followed, at);
        let seen = observations_of(&world.save, second);
        let each = SEEK_TOTAL / 2;
        assert!(
            seen.iter().any(|o| matches!(
                o,
                Observation::SeekEachOther { a_sought, b_sought, .. }
                    if *a_sought == each && *b_sought == each
            )),
            "{seen:?}"
        );
        // Another pair just as close is a tie, and a tie says nothing about either pair.
        let third = world.save.creatures[2].id;
        let other = crate::tally_mut_or_insert(&mut world.save.tallies, first, third).unwrap();
        other.tally.sought = [each, each];
        assert!(
            !observe(&world.save)
                .iter()
                .any(|o| matches!(o, Observation::SeekEachOther { .. }))
        );
    }

    #[test]
    fn one_sided_seeking_is_told_the_right_way_round() {
        let mut world = colony();
        let (first, second) = (world.save.creatures[0].id, world.save.creatures[1].id);
        let pair = crate::tally_mut_or_insert(&mut world.save.tallies, first, second).unwrap();
        let (a, b) = (pair.a, pair.b);
        let at = datetime!(2026-10-01 9:00 UTC);
        for _ in 0..FOLLOW_TIMES {
            pair.tally
                .count(Some(false), RelationshipExperience::Followed, at);
        }
        pair.tally
            .count(Some(true), RelationshipExperience::Followed, at);
        // Six times one way and once the other: the second goes looking for the first.
        assert_eq!(
            observe(&world.save),
            vec![Observation::FollowsAround {
                seeker: b,
                sought: a,
                times: FOLLOW_TIMES,
                returned: 1,
            }]
        );
        // Nearer even, it is no longer one-sided; and enough both ways, it is mutual.
        world.save.tallies[0].tally.sought = [SEEK_EACH, FOLLOW_TIMES + 2];
        assert!(matches!(
            observe(&world.save)[..],
            [Observation::SeekEachOther { .. }]
        ));
        world.save.tallies[0].tally.sought = [SEEK_EACH - 1, FOLLOW_TIMES - 1];
        assert!(observe(&world.save).is_empty());
    }

    #[test]
    fn departed_companions_and_ties_are_never_observed() {
        let mut world = colony();
        let gone = world.save.creatures.pop().unwrap();
        let pair = crate::tally_mut_or_insert(
            &mut world.save.tallies,
            world.save.creatures[0].id,
            gone.id,
        )
        .unwrap();
        pair.tally.shared_rests = 99;
        assert!(observe(&world.save).is_empty());
    }
}
