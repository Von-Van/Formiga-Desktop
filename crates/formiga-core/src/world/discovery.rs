//! Which trinket a creature holds up, and what has to be true for it.
//!
//! The catalogue in `crate::trinkets` says which circumstance each variant belongs to. This module
//! reads whether that circumstance holds at the moment of a find and picks a variant. Everything
//! it reads is state the simulation already keeps — the clock, the ledge under the creature, the
//! window moving beneath it, whoever is standing next to it — and none of it is written down. The
//! scrapbook keeps what was found, when, and by whom, and never why it qualified.

use super::rides::RideMemory;
use super::surfaces::drop_below;
use super::*;
use crate::trinkets::{TrinketCondition, trinkets_for};

/// The everyday trinkets sit at the front of the catalogue and need no circumstance at all. A find
/// with nothing behind it is always one of these, and so is a toy carried through a game.
const EVERYDAY_TRINKETS: u8 = 8;

/// "After dark" is wider than the late-night ritual's 22..05. That window is about the person
/// still being awake at an odd hour; this one is about it being dark outside, which begins before
/// the evening is late and lasts past the first hour anybody is up. It also has to be wide enough
/// that the two night trinkets are findable by someone who keeps ordinary evening hours.
const EVENING_HOUR: u8 = 20;
const MORNING_HOUR: u8 = 6;

/// How far down the nearest thing that would catch a fall has to be, in logical desktop points,
/// before a ledge counts as high. The floor-is-lava game calls 90 points far enough to be worth
/// not falling into; something found high up asks for more than that — about a window's worth of
/// clear air, and well past the 12 points a ledge hop treats as a drop at all.
const HIGH_DROP: f32 = 140.0;

/// The affinity at which the journal writes down a new close friendship. A trinket found beside
/// a close friend means the word the journal means, so `world/experience.rs` reads this too.
pub(super) const CLOSE_FRIENDSHIP_AFFINITY: u8 = 112;

/// Two creature-widths: near enough to be standing together rather than merely sharing a ledge.
const FRIEND_REACH: f32 = CREATURE_ART_WIDTH * 2.0;

/// About half of the finds that could be conditional are, so a circumstance makes a keepsake
/// likely without ever promising one.
const CONDITIONAL_IN: u32 = 2;

/// Three times in four the pick prefers something the scrapbook has not seen yet, so an empty
/// slot fills before a duplicate turns up again.
const PREFER_UNDISCOVERED_IN: u32 = 4;

/// What is true of a creature at the moment it finds something. Conditional trinkets only turn up
/// when the matching circumstance holds; everything here is read from state the simulation
/// already has, and none of it is stored.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct DiscoveryCircumstances {
    pub(super) night: bool,
    pub(super) high_tier: bool,
    pub(super) mid_ride: bool,
    pub(super) beside_close_friend: bool,
}

impl DiscoveryCircumstances {
    /// Whether the circumstance a catalogue entry belongs to holds right now. Everyday trinkets
    /// have no circumstance, so they are always available and are never chosen from here.
    fn holds(self, condition: TrinketCondition) -> bool {
        match condition {
            TrinketCondition::Anywhere => true,
            TrinketCondition::Night => self.night,
            TrinketCondition::HighTier => self.high_tier,
            TrinketCondition::MidRide => self.mid_ride,
            TrinketCondition::BesideCloseFriend => self.beside_close_friend,
        }
    }
}

/// The colony as the find sees it: the snapshots `tick` takes before the creature loop, so reading
/// them never collides with the creature currently being moved.
#[derive(Clone, Copy)]
pub(super) struct ColonyView<'a> {
    pub(super) creatures: &'a [Creature],
    pub(super) relationships: &'a [CreatureRelationship],
}

/// Dark outside, by the clock on the user's own wall. The caller converts with
/// `local_time_or_utc`, so a machine that will not give up its offset behaves one consistent way.
pub(super) fn after_dark(local: OffsetDateTime) -> bool {
    !(MORNING_HOUR..EVENING_HOUR).contains(&local.hour())
}

/// High up: on a window ledge with a long way down to the nearest thing below it. This is the same
/// relative height a hop across a gap and a step off a ledge are weighed against, so "high" means
/// one thing everywhere, and it carries no absolute screen position with it.
pub(super) fn high_up(creature: &Creature, desktop: &DesktopSnapshot, settings: &Settings) -> bool {
    creature.state.surface.kind == SurfaceKind::WindowLedge
        && drop_below(creature, creature.state.position.x, desktop, settings) >= HIGH_DROP
}

/// Partway along a ride: the window is moving under the creature right now, or the action that
/// just ended was the ride itself. A find is chosen at an action boundary, and a ride is an
/// ordinary action that ends at one, so a ride settling is exactly when this is true.
pub(super) fn mid_ride(
    creature: &Creature,
    previous_action: ActionKind,
    rides: &RideMemory,
) -> bool {
    previous_action == ActionKind::RideWindow || rides.riding(creature.id)
}

/// A close friend standing within reach on the same ledge of the same display. "Close" is the
/// affinity the journal calls a friendship; a well-known acquaintance standing just as near is
/// not the same thing.
pub(super) fn beside_close_friend(creature: &Creature, colony: ColonyView<'_>) -> bool {
    colony.relationships.iter().any(|relationship| {
        relationship.affinity >= CLOSE_FRIENDSHIP_AFFINITY
            && relationship
                .other(creature.id)
                .is_some_and(|friend| beside(creature, friend, colony.creatures))
    })
}

/// Whether `friend` is here, on this creature's own surface, close enough to reach.
fn beside(creature: &Creature, friend: CreatureId, creatures: &[Creature]) -> bool {
    creatures.iter().any(|other| {
        other.id == friend
            && other.state.arrival_delay_secs <= 0.0
            && other.state.surface.monitor_id == creature.state.surface.monitor_id
            && other.state.surface.kind == creature.state.surface.kind
            && other.state.surface.window_key == creature.state.surface.window_key
            && creature.state.position.distance(other.state.position) <= FRIEND_REACH
    })
}

/// Everything that is true of this creature at the moment it finds something.
pub(super) fn circumstances_of(
    creature: &Creature,
    previous_action: ActionKind,
    now: OffsetDateTime,
    desktop: &DesktopSnapshot,
    settings: &Settings,
    rides: &RideMemory,
    colony: ColonyView<'_>,
) -> DiscoveryCircumstances {
    DiscoveryCircumstances {
        night: after_dark(local_time_or_utc(now)),
        high_tier: high_up(creature, desktop, settings),
        mid_ride: mid_ride(creature, previous_action, rides),
        beside_close_friend: beside_close_friend(creature, colony),
    }
}

/// A private stream for the conditional decisions, keyed from the ambient stream's own bytes
/// without advancing it. Keying a fresh generator rather than cloning outright keeps these
/// decisions independent of the draws the ambient stream has still to make.
fn fork(rng: &ChaCha12Rng) -> ChaCha12Rng {
    ChaCha12Rng::from_rng(&mut rng.clone())
}

/// Which trinket a creature holds up. Takes the ambient stream directly so it can be called from
/// inside the creature loop, where `World` is already partly borrowed, and `found` is the
/// scrapbook so an empty slot fills before a duplicate turns up again.
///
/// Exactly one draw comes off the ambient stream, on every path. Whether a circumstance holds
/// depends on the clock and on where the windows happen to be, so letting it decide how far the
/// stream moves would quietly hand those things a say in every ambient choice afterwards.
pub(super) fn choose_trinket_variant(
    rng: &mut ChaCha12Rng,
    circumstances: DiscoveryCircumstances,
    found: &[ScrapbookRecord],
) -> u8 {
    // The one draw, taken first and unconditionally, so an ordinary find is the same trinket it
    // has always been and the stream is left exactly where a bare `random_range` would leave it.
    let everyday = rng.random_range(0..EVERYDAY_TRINKETS);
    let mut candidates = [0_u8; TRINKET_VARIANTS as usize];
    let mut count = 0;
    for condition in TrinketCondition::ALL {
        if condition == TrinketCondition::Anywhere || !circumstances.holds(condition) {
            continue;
        }
        for info in trinkets_for(condition) {
            candidates[count] = info.variant;
            count += 1;
        }
    }
    let qualifying = &candidates[..count];
    if qualifying.is_empty() {
        return everyday;
    }
    let mut side = fork(rng);
    if !side.random_ratio(1, CONDITIONAL_IN) {
        return everyday;
    }
    let mut unseen = [0_u8; TRINKET_VARIANTS as usize];
    let mut unseen_count = 0;
    for variant in qualifying {
        if !found.iter().any(|record| record.variant == *variant) {
            unseen[unseen_count] = *variant;
            unseen_count += 1;
        }
    }
    let pool = if unseen_count > 0
        && side.random_ratio(PREFER_UNDISCOVERED_IN - 1, PREFER_UNDISCOVERED_IN)
    {
        &unseen[..unseen_count]
    } else {
        qualifying
    };
    pool[side.random_range(0..pool.len())]
}

/// The everyday trinket a play scene's toy is, taken from the scene's own seed. A game borrows
/// `PresentDiscovery` to hold a toy up, so the variant has to be a deliberate choice: left alone
/// it would be whatever that creature last actually found, and a keepsake that only turns up after
/// dark would read as a fresh find in the middle of a game of keep-away.
pub(super) fn plaything_variant(seed: u64) -> u8 {
    (seed % u64::from(EVERYDAY_TRINKETS)) as u8
}
