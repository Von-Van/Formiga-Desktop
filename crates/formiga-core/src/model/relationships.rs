//! The bond between each pair of companions and the tally of what they have done together.

use super::{CreatureId, MAX_COLONY_CREATURES};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

pub const MAX_RELATIONSHIPS: usize = MAX_COLONY_CREATURES * (MAX_COLONY_CREATURES - 1) / 2;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreatureRelationship {
    pub a: CreatureId,
    pub b: CreatureId,
    pub affinity: u8,
    pub familiarity: u8,
    pub playfulness: u8,
    pub avoidance: u8,
}

/// What one pair has actually been seen doing together, kept beside the pair's bond scores rather
/// than inside them: the scores are read by every companion's every decision, and copied with them,
/// and the tally is read only by the notebook. Never filled in for time before it was kept: a
/// colony from an earlier release starts every pair at nothing, however close they already are.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PairTally {
    pub a: CreatureId,
    pub b: CreatureId,
    #[serde(default)]
    pub tally: RelationshipTally,
}

/// The tally of what two companions have done together, if anything has been counted.
pub fn tally_between(tallies: &[PairTally], a: CreatureId, b: CreatureId) -> Option<&PairTally> {
    let (a, b) = canonical_creature_pair(a, b)?;
    tallies.iter().find(|tally| tally.a == a && tally.b == b)
}

/// The tally for two companions, started at nothing if this is the first thing counted, with
/// the pair in its canonical order. `None` for a companion and itself, or once every pair a full
/// colony can have is already counted.
pub fn tally_mut_or_insert(
    tallies: &mut Vec<PairTally>,
    a: CreatureId,
    b: CreatureId,
) -> Option<&mut PairTally> {
    let (a, b) = canonical_creature_pair(a, b)?;
    // Kept in the pairs' own order, which is the order a colony read back from disk has them in.
    match tallies.binary_search_by_key(&(a, b), |t| (t.a, t.b)) {
        Ok(index) => tallies.get_mut(index),
        Err(_) if tallies.len() >= MAX_RELATIONSHIPS => None,
        Err(index) => {
            tallies.insert(
                index,
                PairTally {
                    a,
                    b,
                    tally: RelationshipTally::default(),
                },
            );
            tallies.get_mut(index)
        }
    }
}

/// The kinds of moment a pair's tally counts, and the one its memory names.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SharedMomentKind {
    /// Five calm minutes spent near one another.
    Calm,
    Greeting,
    /// A nap side by side.
    Rest,
    Play,
    /// A found treasure brought over to the other.
    Gift,
    Squabble,
}

/// The last warm thing a pair shared, and when: the moment a relationship memory names. Only ever
/// written from a moment that happened, so a memory can never say something the colony did not do.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PairMemory {
    #[serde(with = "time::serde::rfc3339")]
    pub at: OffsetDateTime,
    pub kind: SharedMomentKind,
}

/// Counts of what one pair has done together, each held at its ceiling rather than wrapping. The
/// numbers an observation quotes as its evidence, so they only ever go up by a moment that
/// happened.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct RelationshipTally {
    pub calm_spells: u16,
    pub greetings: u16,
    pub shared_rests: u16,
    pub plays: u16,
    pub gifts: u16,
    pub squabbles: u16,
    /// How often each went looking for the other on its own — followed it, went to greet it
    /// coming home, or brought it a find — as `[a sought b, b sought a]`. A greeting a ritual
    /// asked of everyone is not counted here, since nobody chose it.
    pub sought: [u16; 2],
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory: Option<PairMemory>,
}

impl RelationshipTally {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    /// Every moment counted, of whatever kind.
    pub fn total(&self) -> u32 {
        [
            self.calm_spells,
            self.greetings,
            self.shared_rests,
            self.plays,
            self.gifts,
            self.squabbles,
        ]
        .into_iter()
        .map(u32::from)
        .sum()
    }

    /// Count one experience between `a` (who began it, where anyone did) and `b`, at `now`.
    pub fn count(
        &mut self,
        a_began: Option<bool>,
        experience: RelationshipExperience,
        now: OffsetDateTime,
    ) {
        let kind = experience.shared_kind();
        let field = match kind {
            SharedMomentKind::Calm => &mut self.calm_spells,
            SharedMomentKind::Greeting => &mut self.greetings,
            SharedMomentKind::Rest => &mut self.shared_rests,
            SharedMomentKind::Play => &mut self.plays,
            SharedMomentKind::Gift => &mut self.gifts,
            SharedMomentKind::Squabble => &mut self.squabbles,
        };
        *field = field.saturating_add(1);
        if experience.sought_out()
            && let Some(a_began) = a_began
        {
            let side = &mut self.sought[usize::from(!a_began)];
            *side = side.saturating_add(1);
        }
        if !matches!(kind, SharedMomentKind::Calm | SharedMomentKind::Squabble) {
            self.memory = Some(PairMemory { at: now, kind });
        }
    }
}

impl CreatureRelationship {
    pub fn new(a: CreatureId, b: CreatureId) -> Option<Self> {
        let (a, b) = canonical_creature_pair(a, b)?;
        Some(Self {
            a,
            b,
            ..Self::default()
        })
    }

    pub fn contains(self, creature_id: CreatureId) -> bool {
        self.a == creature_id || self.b == creature_id
    }

    pub fn other(self, creature_id: CreatureId) -> Option<CreatureId> {
        if self.a == creature_id {
            Some(self.b)
        } else if self.b == creature_id {
            Some(self.a)
        } else {
            None
        }
    }

    pub fn closeness(self) -> i16 {
        i16::from(self.affinity) * 2 + i16::from(self.familiarity) - i16::from(self.avoidance) * 2
    }

    pub fn apply(&mut self, experience: RelationshipExperience) {
        let (affinity, familiarity, playfulness, avoidance) = match experience {
            RelationshipExperience::CalmProximity => (0, 1, 0, -1),
            RelationshipExperience::Followed => (0, 1, 0, -1),
            RelationshipExperience::Greeting => (2, 1, 0, -1),
            RelationshipExperience::SharedRest => (2, 2, 0, -2),
            RelationshipExperience::PositivePlay => (1, 1, 3, -1),
            RelationshipExperience::BroughtDiscovery => (3, 1, 1, -1),
            RelationshipExperience::StoleToy => (-1, 1, 3, 2),
            RelationshipExperience::HomecomingGreeting => (3, 2, 0, -2),
            RelationshipExperience::WatchedClimb => (1, 1, 0, -1),
            RelationshipExperience::ConcernedAfterToss => (2, 1, 0, 0),
            RelationshipExperience::Squabble => (-2, 1, 1, 5),
        };
        adjust_relationship_score(&mut self.affinity, affinity);
        adjust_relationship_score(&mut self.familiarity, familiarity);
        adjust_relationship_score(&mut self.playfulness, playfulness);
        adjust_relationship_score(&mut self.avoidance, avoidance);
    }
}

fn adjust_relationship_score(score: &mut u8, delta: i16) {
    *score = (i16::from(*score) + delta).clamp(0, i16::from(u8::MAX)) as u8;
}

pub fn canonical_creature_pair(a: CreatureId, b: CreatureId) -> Option<(CreatureId, CreatureId)> {
    (a != b).then_some(if a < b { (a, b) } else { (b, a) })
}

pub fn relationship_between(
    relationships: &[CreatureRelationship],
    a: CreatureId,
    b: CreatureId,
) -> Option<&CreatureRelationship> {
    let (a, b) = canonical_creature_pair(a, b)?;
    relationships
        .iter()
        .find(|relationship| relationship.a == a && relationship.b == b)
}

pub fn closest_companion(
    relationships: &[CreatureRelationship],
    creature_id: CreatureId,
) -> Option<CreatureId> {
    relationships
        .iter()
        .copied()
        .filter(|relationship| relationship.contains(creature_id))
        .max_by_key(|relationship| {
            (
                relationship.closeness(),
                std::cmp::Reverse(relationship.other(creature_id)),
            )
        })
        .and_then(|relationship| relationship.other(creature_id))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RelationshipExperience {
    CalmProximity,
    Followed,
    Greeting,
    SharedRest,
    PositivePlay,
    BroughtDiscovery,
    StoleToy,
    HomecomingGreeting,
    WatchedClimb,
    ConcernedAfterToss,
    Squabble,
}

impl RelationshipExperience {
    /// Which count in a pair's tally this experience adds to.
    pub const fn shared_kind(self) -> SharedMomentKind {
        match self {
            Self::CalmProximity | Self::Followed | Self::WatchedClimb => SharedMomentKind::Calm,
            Self::Greeting | Self::HomecomingGreeting | Self::ConcernedAfterToss => {
                SharedMomentKind::Greeting
            }
            Self::SharedRest => SharedMomentKind::Rest,
            Self::PositivePlay | Self::StoleToy => SharedMomentKind::Play,
            Self::BroughtDiscovery => SharedMomentKind::Gift,
            Self::Squabble => SharedMomentKind::Squabble,
        }
    }

    /// Whether the one who began it went looking for the other of its own accord.
    pub const fn sought_out(self) -> bool {
        matches!(
            self,
            Self::Followed | Self::HomecomingGreeting | Self::BroughtDiscovery
        )
    }
}
