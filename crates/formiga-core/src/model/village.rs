//! What the village ground holds and gains over time: gardens, hangout spots, ornaments, the
//! unlocks that bring new ones, and the keepsakes hung in the trees.

use super::ShelterDecorationKind;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

/// The most garden patches planted at once, each of a different kind.
pub const MAX_GARDENS: usize = 4;

/// A little patch planted on the village ground. It grows by itself through four stages and back
/// round again, and the colony tends it now and then.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GardenKind {
    Flowers,
    Vegetables,
    Herbs,
    Sunflowers,
    Pumpkins,
    Strawberries,
    MushroomRing,
    BerryBush,
    Tulips,
    Cactus,
    PeaTrellis,
    Tomatoes,
}

impl GardenKind {
    pub const ALL: [Self; 12] = [
        Self::Flowers,
        Self::Vegetables,
        Self::Herbs,
        Self::Sunflowers,
        Self::Pumpkins,
        Self::Strawberries,
        Self::MushroomRing,
        Self::BerryBush,
        Self::Tulips,
        Self::Cactus,
        Self::PeaTrellis,
        Self::Tomatoes,
    ];

    /// The gardens a new colony can plant from its first day.
    pub const STARTING: [Self; 3] = [Self::Flowers, Self::Vegetables, Self::Herbs];

    pub const fn index(self) -> u8 {
        self as u8
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Flowers => "Flower bed",
            Self::Vegetables => "Vegetable patch",
            Self::Herbs => "Herb box",
            Self::Sunflowers => "Sunflowers",
            Self::Pumpkins => "Pumpkin patch",
            Self::Strawberries => "Strawberry planter",
            Self::MushroomRing => "Mushroom ring",
            Self::BerryBush => "Berry bush",
            Self::Tulips => "Tulip row",
            Self::Cactus => "Cactus pots",
            Self::PeaTrellis => "Pea trellis",
            Self::Tomatoes => "Tomato cane",
        }
    }

    pub const fn description(self) -> &'static str {
        match self {
            Self::Flowers => "Three flowers in the colony's own colours.",
            Self::Vegetables => "A cabbage, a carrot and a pumpkin coming along.",
            Self::Herbs => "Rosemary, basil and lavender in a planter box.",
            Self::Sunflowers => "Two tall sunflowers that turn to follow the light.",
            Self::Pumpkins => "A trailing vine with one pumpkin growing fat on it.",
            Self::Strawberries => "A strawberry pot with runners over the rim.",
            Self::MushroomRing => "A little ring of mushrooms that came up by itself.",
            Self::BerryBush => "A round bush that fills with berries.",
            Self::Tulips => "A row of tulips standing to attention.",
            Self::Cactus => "Two small cacti in clay pots, one of them flowering.",
            Self::PeaTrellis => "Peas climbing a little lattice of sticks.",
            Self::Tomatoes => "A tomato plant tied to a cane.",
        }
    }

    /// Whether what grows here is something a companion might pick and eat.
    pub const fn edible(self) -> bool {
        matches!(
            self,
            Self::Vegetables
                | Self::Herbs
                | Self::Pumpkins
                | Self::Strawberries
                | Self::BerryBush
                | Self::PeaTrellis
                | Self::Tomatoes
        )
    }

    /// Whether this is a patch someone would proudly hold something up from.
    pub const fn harvest(self) -> bool {
        matches!(
            self,
            Self::Vegetables | Self::Pumpkins | Self::PeaTrellis | Self::Tomatoes
        )
    }

    /// How long each stage of growing lasts, in hours: quick herbs, slow pumpkins.
    pub const fn stage_hours(self) -> i64 {
        match self {
            Self::Herbs | Self::MushroomRing => 5,
            Self::Flowers | Self::Tulips | Self::Strawberries => 6,
            Self::Vegetables | Self::PeaTrellis | Self::Tomatoes | Self::BerryBush => 8,
            Self::Sunflowers | Self::Cactus => 10,
            Self::Pumpkins => 12,
        }
    }
}

/// How far along a garden patch is. A patch goes round these by itself — no watering needed, and
/// nothing wilts for want of it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GardenStage {
    /// Just a few green shoots.
    Sprout,
    /// Leafy, with nothing on it yet.
    Growing,
    /// Flowering or fruiting: how a patch looked before it grew.
    Grown,
    /// At its fullest, before it goes back to seed and starts over.
    Bounty,
}

impl GardenStage {
    pub const ALL: [Self; 4] = [Self::Sprout, Self::Growing, Self::Grown, Self::Bounty];

    pub const fn index(self) -> u8 {
        self as u8
    }
}

/// One garden patch: what is growing in it, how far along the village ground it is, as a
/// fraction from its left end to its right, and when it was planted.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct GardenPatch {
    pub kind: GardenKind,
    pub along: f32,
    /// Absent for a patch planted before gardens grew; such a patch is taken to have been planted
    /// long ago, part way round its cycle.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "time::serde::rfc3339::option"
    )]
    pub planted_at_utc: Option<OffsetDateTime>,
}

impl GardenPatch {
    /// How far along this patch is at `now`. It starts as a sprout when it is planted, and then
    /// goes round sprout, growing, grown and bounty for as long as it stays in the ground. A clock
    /// set back never un-plants it.
    pub fn stage(&self, now: OffsetDateTime) -> GardenStage {
        let hours = match self.planted_at_utc {
            Some(planted) => (now - planted).whole_hours().max(0),
            // An old patch: somewhere round its cycle, the same place for the same kind, and
            // moving on from there with the clock like any other.
            None => now.unix_timestamp() / 3600 + i64::from(self.kind.index()) * 7,
        };
        let stage = (hours / self.kind.stage_hours()).rem_euclid(4);
        GardenStage::ALL[stage as usize]
    }
}

/// The most hangout spots put down at once, each of a different kind.
pub const MAX_HANGOUTS: usize = 4;

/// Something the person at the desk can put down on the village ground for the colony to gather
/// at. Each gently draws one kind of quiet moment at home to it; a companion is free to do
/// something else instead.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HangoutKind {
    /// A plump floor cushion: somewhere to nap.
    Cushion,
    /// A picnic blanket spread out: somewhere to snack and sip.
    Blanket,
    /// A little spyglass on a stand: somewhere to stand and look out.
    Lookout,
    Hammock,
    Swing,
    TeaTable,
    BookNook,
    Campfire,
    Sandbox,
    Puddle,
    Bench,
    DrumStump,
    BirdFeeder,
    StargazingMat,
    SunnyRock,
}

impl HangoutKind {
    pub const ALL: [Self; 15] = [
        Self::Cushion,
        Self::Blanket,
        Self::Lookout,
        Self::Hammock,
        Self::Swing,
        Self::TeaTable,
        Self::BookNook,
        Self::Campfire,
        Self::Sandbox,
        Self::Puddle,
        Self::Bench,
        Self::DrumStump,
        Self::BirdFeeder,
        Self::StargazingMat,
        Self::SunnyRock,
    ];

    /// The spots a new colony can put down from its first day.
    pub const STARTING: [Self; 3] = [Self::Cushion, Self::Blanket, Self::Lookout];

    pub const fn index(self) -> u8 {
        self as u8
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Cushion => "Nap cushion",
            Self::Blanket => "Picnic blanket",
            Self::Lookout => "Lookout",
            Self::Hammock => "Hammock",
            Self::Swing => "Swing",
            Self::TeaTable => "Tea table",
            Self::BookNook => "Book nook",
            Self::Campfire => "Campfire",
            Self::Sandbox => "Sandbox",
            Self::Puddle => "Splash puddle",
            Self::Bench => "Bench",
            Self::DrumStump => "Drum stump",
            Self::BirdFeeder => "Bird feeder",
            Self::StargazingMat => "Stargazing mat",
            Self::SunnyRock => "Sunny rock",
        }
    }

    pub const fn description(self) -> &'static str {
        match self {
            Self::Cushion => "Somewhere soft for a nap at home.",
            Self::Blanket => "Somewhere to snack and sip, and where a picnic gathers.",
            Self::Lookout => "Somewhere to stand and look out over the desktop.",
            Self::Hammock => "Slung between two posts, for a swaying nap.",
            Self::Swing => "A plank on two ropes, for swinging on.",
            Self::TeaTable => "A little table set for tea, for a sip and a sit.",
            Self::BookNook => "A stack of books to sit by and read.",
            Self::Campfire => "A ring of stones and a small fire to warm paws at.",
            Self::Sandbox => "A box of sand, for digging in.",
            Self::Puddle => "A puddle kept on purpose, for splashing.",
            Self::Bench => "A bench for sitting and watching the village.",
            Self::DrumStump => "A hollow stump that makes a good drum.",
            Self::BirdFeeder => "A feeder on a pole, and birds to watch at it.",
            Self::StargazingMat => "A mat to lie back on and look up from.",
            Self::SunnyRock => "A flat rock that holds the warmth, for basking.",
        }
    }
}

/// The most ornaments set out at once, each of a different kind.
pub const MAX_ORNAMENTS: usize = 4;

/// A standing ornament for the village ground: something to look at and to wander over and
/// inspect, rather than somewhere to spend a quiet moment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OrnamentKind {
    LampPost,
    BirdBath,
    Signpost,
    WishingWell,
    PicketFence,
    SteppingStones,
    Scarecrow,
    WindSpinner,
    Wheelbarrow,
    Beehive,
    StoneCairn,
    LilyPond,
    MailboxPost,
    FlagPole,
    LogStool,
}

impl OrnamentKind {
    pub const ALL: [Self; 15] = [
        Self::LampPost,
        Self::BirdBath,
        Self::Signpost,
        Self::WishingWell,
        Self::PicketFence,
        Self::SteppingStones,
        Self::Scarecrow,
        Self::WindSpinner,
        Self::Wheelbarrow,
        Self::Beehive,
        Self::StoneCairn,
        Self::LilyPond,
        Self::MailboxPost,
        Self::FlagPole,
        Self::LogStool,
    ];

    /// The ornaments a new colony can set out from its first day.
    pub const STARTING: [Self; 3] = [Self::LampPost, Self::BirdBath, Self::Signpost];

    pub const fn index(self) -> u8 {
        self as u8
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::LampPost => "Lamp post",
            Self::BirdBath => "Bird bath",
            Self::Signpost => "Signpost",
            Self::WishingWell => "Wishing well",
            Self::PicketFence => "Picket fence",
            Self::SteppingStones => "Stepping stones",
            Self::Scarecrow => "Scarecrow",
            Self::WindSpinner => "Wind spinner",
            Self::Wheelbarrow => "Wheelbarrow",
            Self::Beehive => "Beehive",
            Self::StoneCairn => "Stone stack",
            Self::LilyPond => "Lily pond",
            Self::MailboxPost => "Post box",
            Self::FlagPole => "Flag pole",
            Self::LogStool => "Log stool",
        }
    }

    pub const fn description(self) -> &'static str {
        match self {
            Self::LampPost => "A lamp on a post that lights up after dark.",
            Self::BirdBath => "A stone bowl of water for passing birds.",
            Self::Signpost => "Two arrows pointing to places nobody has been.",
            Self::WishingWell => "A little well with a roof and a bucket.",
            Self::PicketFence => "A short run of white fence.",
            Self::SteppingStones => "Flat stones set in the grass.",
            Self::Scarecrow => "A scarecrow that scares nothing at all.",
            Self::WindSpinner => "A spinner on a pole that turns in the slightest breeze.",
            Self::Wheelbarrow => "A wheelbarrow, parked with a few things in it.",
            Self::Beehive => "A round straw hive and its busy bees.",
            Self::StoneCairn => "Stones balanced one on another.",
            Self::LilyPond => "A tiny pond with a lily pad on it.",
            Self::MailboxPost => "A post box on a pole, waiting for letters.",
            Self::FlagPole => "A tall pole with the colony's own flag.",
            Self::LogStool => "A round of log to sit on.",
        }
    }
}

/// One ornament: what it is, and how far along the village ground it stands.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct OrnamentSpot {
    pub kind: OrnamentKind,
    pub along: f32,
}

/// One hangout spot: what it is, and how far along the village ground it stands, as a fraction
/// from its left end to its right, so it keeps its place on the ground as the village grows,
/// shrinks, or moves.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct HangoutSpot {
    pub kind: HangoutKind,
    pub along: f32,
}

/// Something the village can gain over time: a decoration for its houses, a hangout spot, a
/// garden, or an ornament for its ground.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VillageItem {
    Decoration(ShelterDecorationKind),
    Hangout(HangoutKind),
    Garden(GardenKind),
    Ornament(OrnamentKind),
}

impl VillageItem {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Decoration(kind) => kind.label(),
            Self::Hangout(kind) => kind.label(),
            Self::Garden(kind) => kind.label(),
            Self::Ornament(kind) => kind.label(),
        }
    }

    /// Everything the village can gain, in the order the catalogues list them.
    pub fn all() -> impl Iterator<Item = Self> {
        ShelterDecorationKind::ALL
            .into_iter()
            .map(Self::Decoration)
            .chain(HangoutKind::ALL.into_iter().map(Self::Hangout))
            .chain(GardenKind::ALL.into_iter().map(Self::Garden))
            .chain(OrnamentKind::ALL.into_iter().map(Self::Ornament))
    }
}

/// What the village has to choose from so far, and when the next thing arrives. Every category
/// starts with three, and one more arrives every day or two for as long as there is anything left.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct VillageUnlocks {
    pub decorations: Vec<ShelterDecorationKind>,
    pub hangouts: Vec<HangoutKind>,
    pub gardens: Vec<GardenKind>,
    pub ornaments: Vec<OrnamentKind>,
    #[serde(with = "time::serde::rfc3339")]
    pub next_at_utc: OffsetDateTime,
    pub ordinal: u32,
}

impl Default for VillageUnlocks {
    fn default() -> Self {
        Self::starting()
    }
}

impl VillageUnlocks {
    /// A new colony's village: three of everything.
    pub fn starting() -> Self {
        Self {
            decorations: ShelterDecorationKind::STARTING.to_vec(),
            hangouts: HangoutKind::STARTING.to_vec(),
            gardens: GardenKind::STARTING.to_vec(),
            ornaments: OrnamentKind::STARTING.to_vec(),
            next_at_utc: OffsetDateTime::UNIX_EPOCH,
            ordinal: 0,
        }
    }

    /// Whether the village has this to choose from yet.
    pub fn has(&self, item: VillageItem) -> bool {
        match item {
            VillageItem::Decoration(kind) => self.decorations.contains(&kind),
            VillageItem::Hangout(kind) => self.hangouts.contains(&kind),
            VillageItem::Garden(kind) => self.gardens.contains(&kind),
            VillageItem::Ornament(kind) => self.ornaments.contains(&kind),
        }
    }

    /// Add something to choose from. Returns whether it was new.
    pub fn grant(&mut self, item: VillageItem) -> bool {
        if self.has(item) {
            return false;
        }
        match item {
            VillageItem::Decoration(kind) => self.decorations.push(kind),
            VillageItem::Hangout(kind) => self.hangouts.push(kind),
            VillageItem::Garden(kind) => self.gardens.push(kind),
            VillageItem::Ornament(kind) => self.ornaments.push(kind),
        }
        true
    }

    /// Everything still to come.
    pub fn remaining(&self) -> impl Iterator<Item = VillageItem> + '_ {
        VillageItem::all().filter(|item| !self.has(*item))
    }

    /// Each kind once, in the order it arrived, and every category topped up to its first three.
    pub fn normalize(&mut self) {
        fn dedup<T: PartialEq + Copy>(list: &mut Vec<T>, starting: &[T]) {
            let mut seen: Vec<T> = Vec::with_capacity(list.len());
            list.retain(|item| {
                let fresh = !seen.contains(item);
                seen.push(*item);
                fresh
            });
            for item in starting {
                if !list.contains(item) {
                    list.push(*item);
                }
            }
        }
        dedup(&mut self.decorations, &ShelterDecorationKind::STARTING);
        dedup(&mut self.hangouts, &HangoutKind::STARTING);
        dedup(&mut self.gardens, &GardenKind::STARTING);
        dedup(&mut self.ornaments, &OrnamentKind::STARTING);
    }
}

/// The number of hooks across the two keepsake trees: eight on each.
pub const TREE_HOOKS: usize = 16;

/// Which keepsakes hang on the trees' hooks, as the person at the desk chose them: a variant or
/// nothing for each hook, the outward tree's eight first. Absent until anything is chosen, in
/// which case the trees fill themselves as finds come in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TreeKeepsakes {
    pub hooks: [Option<u8>; TREE_HOOKS],
}

impl TreeKeepsakes {
    /// How many hooks hold something.
    pub fn hung(&self) -> usize {
        self.hooks.iter().flatten().count()
    }
}

/// What hangs on each of the sixteen hooks. With nothing chosen, the original sixteen finds hang
/// on the hooks they have always hung on — a hook for each of them — and anything found since
/// fills the hooks still empty, in the order it was found. With a choice, exactly what was chosen,
/// less anything the scrapbook does not hold.
pub fn hung_keepsakes(
    chosen: Option<&TreeKeepsakes>,
    scrapbook: &[crate::ScrapbookRecord],
) -> [Option<u8>; TREE_HOOKS] {
    let found = |variant: u8| {
        variant < crate::TRINKET_VARIANTS
            && scrapbook.iter().any(|record| record.variant == variant)
    };
    if let Some(chosen) = chosen {
        return chosen
            .hooks
            .map(|hook| hook.filter(|variant| found(*variant)));
    }
    let mut hooks = [None; TREE_HOOKS];
    for record in scrapbook {
        if usize::from(record.variant) < TREE_HOOKS {
            hooks[usize::from(record.variant)] = Some(record.variant);
        }
    }
    let mut later: Vec<&crate::ScrapbookRecord> = scrapbook
        .iter()
        .filter(|record| usize::from(record.variant) >= TREE_HOOKS && found(record.variant))
        .collect();
    later.sort_by_key(|record| (record.first_at, record.variant));
    let mut later = later.into_iter();
    for hook in &mut hooks {
        if hook.is_none() {
            *hook = later.next().map(|record| record.variant);
        }
    }
    hooks
}
