//! Which trinket a creature holds up, and what has to be true for it.
//!
//! The catalogue in `crate::trinkets` says which circumstance each variant belongs to. This module
//! reads whether that circumstance holds at the moment of a find and picks a variant. Everything
//! it reads is state the simulation already keeps — the clock, the ledge under the creature, the
//! window moving beneath it, whoever is standing next to it, what the village is doing — and none
//! of it is written down. The scrapbook keeps what was found, when, and by whom, and never why it
//! qualified.

use super::rides::RideMemory;
use super::surfaces::drop_below;
use super::*;
use crate::trinkets::{TrinketCondition, trinkets_for};

/// "After dark" is wider than the late-night ritual's 22..05. That window is about the person
/// still being awake at an odd hour; this one is about it being dark outside, which begins before
/// the evening is late and lasts past the first hour anybody is up. It also has to be wide enough
/// that the night trinkets are findable by someone who keeps ordinary evening hours.
const EVENING_HOUR: u8 = 20;
const MORNING_HOUR: u8 = 6;
/// The early part of the day, when the morning finds turn up: from first light until ten.
const LATE_MORNING_HOUR: u8 = 10;

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

/// One find in this many is one of the rare ones, wherever and whenever it happens.
const RARE_IN: u32 = 60;

/// The moon's cycle, and one new moon to count it from: 6 January 2000, 18:14 UTC.
const SYNODIC_MONTH_DAYS: f64 = 29.530_588_853;
const KNOWN_NEW_MOON_UNIX: f64 = 947_182_440.0;
/// How near to exactly full the moon has to be to count as full: a day and a half either side,
/// which is as full as it looks to anybody glancing up.
const FULL_MOON_DAYS: f64 = 1.5;

/// How many days either side of the colony's own birthday its birthday finds turn up.
const BIRTHDAY_DAYS: i64 = 3;

/// What is true of a creature at the moment it finds something. Conditional trinkets only turn up
/// when the matching circumstance holds; everything here is read from state the simulation
/// already has, and none of it is stored.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct DiscoveryCircumstances {
    pub(super) night: bool,
    pub(super) high_tier: bool,
    pub(super) mid_ride: bool,
    pub(super) beside_close_friend: bool,
    pub(super) at_home: bool,
    pub(super) in_garden: bool,
    pub(super) morning: bool,
    pub(super) weekend: bool,
    pub(super) after_nap: bool,
    pub(super) on_roof: bool,
    pub(super) with_visitor: bool,
    pub(super) full_moon: bool,
    pub(super) colony_birthday: bool,
}

impl DiscoveryCircumstances {
    /// Whether the circumstance a catalogue entry belongs to holds right now. Everyday trinkets
    /// have no circumstance, so they are always available and are never chosen from here, and
    /// a rare one is never a matter of circumstance at all.
    fn holds(self, condition: TrinketCondition) -> bool {
        match condition {
            TrinketCondition::Anywhere | TrinketCondition::Rare => false,
            TrinketCondition::Night => self.night,
            TrinketCondition::HighTier => self.high_tier,
            TrinketCondition::MidRide => self.mid_ride,
            TrinketCondition::BesideCloseFriend => self.beside_close_friend,
            TrinketCondition::AtHome => self.at_home,
            TrinketCondition::InGarden => self.in_garden,
            TrinketCondition::Morning => self.morning,
            TrinketCondition::Weekend => self.weekend,
            TrinketCondition::AfterNap => self.after_nap,
            TrinketCondition::OnRoof => self.on_roof,
            TrinketCondition::WithVisitor => self.with_visitor,
            TrinketCondition::FullMoon => self.full_moon,
            TrinketCondition::ColonyBirthday => self.colony_birthday,
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

/// The first hours of the day, by the same wall clock.
pub(super) fn early_morning(local: OffsetDateTime) -> bool {
    (MORNING_HOUR..LATE_MORNING_HOUR).contains(&local.hour())
}

/// Saturday or Sunday, by the same wall clock.
pub(super) fn weekend(local: OffsetDateTime) -> bool {
    matches!(
        local.weekday(),
        time::Weekday::Saturday | time::Weekday::Sunday
    )
}

/// How many days into its cycle the moon is, from new at 0 to full at about 14.8. The moon is the
/// same moon for everybody, so this reads universal time.
pub(super) fn moon_age_days(now: OffsetDateTime) -> f64 {
    let days = (now.unix_timestamp() as f64 - KNOWN_NEW_MOON_UNIX) / 86_400.0;
    days.rem_euclid(SYNODIC_MONTH_DAYS)
}

/// Whether tonight's moon is full, or near enough that nobody looking up would say otherwise.
pub(super) fn full_moon(now: OffsetDateTime) -> bool {
    (moon_age_days(now) - SYNODIC_MONTH_DAYS / 2.0).abs() <= FULL_MOON_DAYS
}

/// Within a few days of the colony's own birthday, once it has had one: the same anniversary the
/// hatch-day ritual keeps, a little wider, and never in the year the colony began.
pub(super) fn colony_birthday(local: OffsetDateTime, created_utc: OffsetDateTime) -> bool {
    let created = created_utc.to_offset(local.offset()).date();
    let today = local.date();
    [today.year() - 1, today.year(), today.year() + 1]
        .into_iter()
        .filter(|year| *year > created.year())
        .filter_map(|year| {
            // A colony begun on the 29th of February keeps its birthday on the 28th in other
            // years, as people do.
            time::Date::from_calendar_date(year, created.month(), created.day())
                .or_else(|_| time::Date::from_calendar_date(year, created.month(), 28))
                .ok()
        })
        .any(|birthday| (today - birthday).whole_days().abs() <= BIRTHDAY_DAYS)
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

/// The circumstances every find shares, whoever makes it: the time of day, the day of the week,
/// the moon, and the colony's birthday.
pub(super) fn calendar_circumstances(
    now: OffsetDateTime,
    created_utc: OffsetDateTime,
) -> DiscoveryCircumstances {
    let local = local_time_or_utc(now);
    let night = after_dark(local);
    DiscoveryCircumstances {
        night,
        morning: early_morning(local),
        weekend: weekend(local),
        full_moon: night && full_moon(now),
        colony_birthday: colony_birthday(local, created_utc),
        ..DiscoveryCircumstances::default()
    }
}

/// Everything that is true of this creature at the moment it finds something out on the desktop.
#[allow(clippy::too_many_arguments)]
pub(super) fn circumstances_of(
    creature: &Creature,
    previous_action: ActionKind,
    now: OffsetDateTime,
    created_utc: OffsetDateTime,
    desktop: &DesktopSnapshot,
    settings: &Settings,
    rides: &RideMemory,
    colony: ColonyView<'_>,
) -> DiscoveryCircumstances {
    DiscoveryCircumstances {
        high_tier: high_up(creature, desktop, settings),
        mid_ride: mid_ride(creature, previous_action, rides),
        beside_close_friend: beside_close_friend(creature, colony),
        after_nap: previous_action == ActionKind::Sleep,
        ..calendar_circumstances(now, created_utc)
    }
}

/// A private stream for the conditional decisions, keyed from the ambient stream's own bytes
/// without advancing it. Keying a fresh generator rather than cloning outright keeps these
/// decisions independent of the draws the ambient stream has still to make.
fn fork(rng: &ChaCha12Rng) -> ChaCha12Rng {
    ChaCha12Rng::from_rng(&mut rng.clone())
}

/// The catalogue variants of one circumstance, in a stack array so the choice allocates nothing.
fn variants_where(
    mut keep: impl FnMut(TrinketCondition) -> bool,
) -> ([u8; TRINKET_VARIANTS as usize], usize) {
    let mut out = [0_u8; TRINKET_VARIANTS as usize];
    let mut count = 0;
    for condition in TrinketCondition::ALL {
        if !keep(condition) {
            continue;
        }
        for info in trinkets_for(condition) {
            out[count] = info.variant;
            count += 1;
        }
    }
    (out, count)
}

/// One of `pool`, preferring three times in four something the scrapbook has not seen yet.
fn pick_preferring_unseen(side: &mut ChaCha12Rng, pool: &[u8], found: &[ScrapbookRecord]) -> u8 {
    let mut unseen = [0_u8; TRINKET_VARIANTS as usize];
    let mut unseen_count = 0;
    for variant in pool {
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
        pool
    };
    pool[side.random_range(0..pool.len())]
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
    let (everyday, everyday_count) =
        variants_where(|condition| condition == TrinketCondition::Anywhere);
    let everyday = &everyday[..everyday_count];
    // The one draw, taken first and unconditionally, so the stream is left exactly where a bare
    // `random_range` would leave it whatever happens next.
    let plain = everyday[rng.random_range(0..everyday.len())];
    let mut side = fork(rng);
    // Once in a long while, anywhere at all.
    if side.random_ratio(1, RARE_IN) {
        let (rare, count) = variants_where(|condition| condition == TrinketCondition::Rare);
        return pick_preferring_unseen(&mut side, &rare[..count], found);
    }
    let (qualifying, count) = variants_where(|condition| circumstances.holds(condition));
    if count > 0 && side.random_ratio(1, CONDITIONAL_IN) {
        return pick_preferring_unseen(&mut side, &qualifying[..count], found);
    }
    // An everyday find. It prefers one the scrapbook is still missing, like everything else,
    // and otherwise is the plain draw.
    if side.random_ratio(PREFER_UNDISCOVERED_IN - 1, PREFER_UNDISCOVERED_IN) {
        let missing = everyday
            .iter()
            .filter(|variant| !found.iter().any(|record| record.variant == **variant))
            .count();
        if missing > 0 {
            let nth = side.random_range(0..missing);
            if let Some(variant) = everyday
                .iter()
                .filter(|variant| !found.iter().any(|record| record.variant == **variant))
                .nth(nth)
            {
                return *variant;
            }
        }
    }
    plain
}

/// The everyday trinket a play scene's toy is, taken from the scene's own seed. A game borrows
/// `PresentDiscovery` to hold a toy up, so the variant has to be a deliberate choice: left alone
/// it would be whatever that creature last actually found, and a keepsake that only turns up after
/// dark would read as a fresh find in the middle of a game of keep-away. Only the original eight
/// everyday finds are playthings, so a game of catch looks the way it always has.
pub(super) fn plaything_variant(seed: u64) -> u8 {
    (seed % 8) as u8
}

/// Whether a variant is one of the everyday finds.
#[cfg(test)]
pub(super) fn is_everyday(variant: u8) -> bool {
    crate::trinkets::trinket_info(variant)
        .is_some_and(|info| info.condition == TrinketCondition::Anywhere)
}
