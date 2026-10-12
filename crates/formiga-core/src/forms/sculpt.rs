//! A sculpted form: a body plan, its proportions, the parts fitted to it, its coat and its
//! markings. Every value is a small bounded step, named for what it does, never a pixel or a
//! bone: the renderer decides what each step means for each plan, so a form keeps its meaning as
//! the drawing improves.

use super::FormError;
use serde::{Deserialize, Serialize};

/// The most a proportion or a part's size can be. Every slider runs from 0 to this, and the
/// middle is the plan's own default.
pub const STEPS: u8 = 10;

/// The middle of every slider.
pub const MIDDLE: u8 = STEPS / 2;

/// The furthest a part can be tilted or lifted from where its slot holds it, either way.
pub const NUDGE: i8 = 3;

/// How many markings a coat can carry at once.
pub const MAX_MARKINGS: usize = 4;

/// The broad body a sculpted form is built on. A plan decides how the form stands, moves, rests
/// and is held, and which slots it has; everything else is the owner's to choose.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Plan {
    /// Four legs under a rounded body: a bear, a cat, a dog.
    CompactQuadruped,
    /// Four sturdy legs under a heavy body: an elephant, a rhinoceros.
    LargeQuadruped,
    /// Four long legs and a long neck: a giraffe.
    TallQuadruped,
    /// Sitting or standing up, with arms: a gorilla, a koala sat up.
    Upright,
    /// No legs at all. Floats a little above the ground and bobs: a whale, a dolphin.
    Floater,
    /// Low to the ground under a shell, on four short limbs: a turtle.
    Crawler,
    /// Two legs and a pair of wings: a bird.
    Percher,
}

impl Plan {
    pub const ALL: [Self; 7] = [
        Self::CompactQuadruped,
        Self::LargeQuadruped,
        Self::TallQuadruped,
        Self::Upright,
        Self::Floater,
        Self::Crawler,
        Self::Percher,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::CompactQuadruped => "Compact four-legged",
            Self::LargeQuadruped => "Large four-legged",
            Self::TallQuadruped => "Tall four-legged",
            Self::Upright => "Upright",
            Self::Floater => "Floater",
            Self::Crawler => "Crawler",
            Self::Percher => "Percher",
        }
    }

    /// How the plan gets about, for the editor and for anyone placing it in a scene.
    pub const fn locomotion(self) -> Locomotion {
        match self {
            Self::CompactQuadruped | Self::LargeQuadruped | Self::TallQuadruped => Locomotion::Walk,
            Self::Upright => Locomotion::Amble,
            Self::Floater => Locomotion::Float,
            Self::Crawler => Locomotion::Crawl,
            Self::Percher => Locomotion::Hop,
        }
    }

    /// Whether a part may be fitted in `slot` on this plan.
    pub const fn has(self, slot: Slot) -> bool {
        use Slot as S;
        match self {
            Self::CompactQuadruped | Self::LargeQuadruped | Self::TallQuadruped | Self::Upright => {
                !matches!(slot, S::Fins | S::Shell)
            }
            Self::Floater => !matches!(slot, S::Feet | S::Wings | S::Shell),
            Self::Crawler => !matches!(slot, S::Wings | S::Tusks),
            Self::Percher => !matches!(slot, S::Fins | S::Shell | S::Tusks),
        }
    }

    /// Whether a proportion means anything on this plan. The rest are kept, unused, so that a
    /// form taken to another plan and back keeps them.
    pub const fn uses(self, dimension: Dimension) -> bool {
        use Dimension as D;
        match self {
            Self::Upright => true,
            Self::Floater => !matches!(dimension, D::Legs | D::Neck | D::Arms),
            Self::Crawler => !matches!(dimension, D::Neck | D::Arms),
            Self::Percher => !matches!(dimension, D::Girth | D::Arms),
            _ => !matches!(dimension, D::Arms),
        }
    }
}

/// How a plan moves, which every app maps to its own animation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Locomotion {
    /// Steps on four legs.
    Walk,
    /// Steps upright, rocking side to side.
    Amble,
    /// Hovers a few pixels up and bobs along. Never walks on legs it does not have.
    Float,
    /// Paddles low along the ground.
    Crawl,
    /// Hops on two feet, with a flap now and then.
    Hop,
}

/// One proportion of a form.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Dimension {
    Head,
    Length,
    Depth,
    Neck,
    Legs,
    Arms,
    Girth,
    Tail,
}

impl Dimension {
    pub const ALL: [Self; 8] = [
        Self::Head,
        Self::Length,
        Self::Depth,
        Self::Neck,
        Self::Legs,
        Self::Arms,
        Self::Girth,
        Self::Tail,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Head => "Head size",
            Self::Length => "Body length",
            Self::Depth => "Body depth",
            Self::Neck => "Neck length",
            Self::Legs => "Leg length",
            Self::Arms => "Arm length",
            Self::Girth => "Limb thickness",
            Self::Tail => "Tail length",
        }
    }
}

/// The proportions of a form, each from 0 to [`STEPS`], with [`MIDDLE`] the plan's own.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Shape {
    pub head: u8,
    pub length: u8,
    pub depth: u8,
    pub neck: u8,
    pub legs: u8,
    pub arms: u8,
    pub girth: u8,
    pub tail: u8,
}

impl Default for Shape {
    fn default() -> Self {
        Self {
            head: MIDDLE,
            length: MIDDLE,
            depth: MIDDLE,
            neck: MIDDLE,
            legs: MIDDLE,
            arms: MIDDLE,
            girth: MIDDLE,
            tail: MIDDLE,
        }
    }
}

impl Shape {
    pub fn get(&self, dimension: Dimension) -> u8 {
        match dimension {
            Dimension::Head => self.head,
            Dimension::Length => self.length,
            Dimension::Depth => self.depth,
            Dimension::Neck => self.neck,
            Dimension::Legs => self.legs,
            Dimension::Arms => self.arms,
            Dimension::Girth => self.girth,
            Dimension::Tail => self.tail,
        }
    }

    pub fn set(&mut self, dimension: Dimension, value: u8) {
        let value = value.min(STEPS);
        match dimension {
            Dimension::Head => self.head = value,
            Dimension::Length => self.length = value,
            Dimension::Depth => self.depth = value,
            Dimension::Neck => self.neck = value,
            Dimension::Legs => self.legs = value,
            Dimension::Arms => self.arms = value,
            Dimension::Girth => self.girth = value,
            Dimension::Tail => self.tail = value,
        }
    }
}

/// Where a part is fitted.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Slot {
    Ears,
    Snout,
    Horns,
    Tusks,
    Mane,
    Tail,
    Feet,
    Fins,
    Back,
    Wings,
    Shell,
}

impl Slot {
    pub const ALL: [Self; 11] = [
        Self::Ears,
        Self::Snout,
        Self::Horns,
        Self::Tusks,
        Self::Mane,
        Self::Tail,
        Self::Feet,
        Self::Fins,
        Self::Back,
        Self::Wings,
        Self::Shell,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Ears => "Ears",
            Self::Snout => "Snout",
            Self::Horns => "Horns",
            Self::Tusks => "Tusks",
            Self::Mane => "Mane",
            Self::Tail => "Tail",
            Self::Feet => "Feet",
            Self::Fins => "Fins",
            Self::Back => "Back",
            Self::Wings => "Wings",
            Self::Shell => "Shell",
        }
    }

    /// Whether the slot holds a pair, one each side, which can be set uneven.
    pub const fn paired(self) -> bool {
        matches!(
            self,
            Self::Ears | Self::Horns | Self::Tusks | Self::Fins | Self::Wings
        )
    }

    /// Every part that fits the slot.
    pub fn kinds(self) -> impl Iterator<Item = PartKind> {
        PartKind::ALL
            .into_iter()
            .filter(move |kind| kind.slot() == self)
    }
}

/// Every part in the catalogue, by its catalogue identifier. A form names its parts by these and
/// nothing else, so a reader that does not know one refuses the form rather than guessing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum PartKind {
    #[serde(rename = "ears.round")]
    EarsRound,
    #[serde(rename = "ears.pointed")]
    EarsPointed,
    #[serde(rename = "ears.long")]
    EarsLong,
    #[serde(rename = "ears.floppy")]
    EarsFloppy,
    #[serde(rename = "ears.tufted")]
    EarsTufted,
    #[serde(rename = "ears.fan")]
    EarsFan,
    #[serde(rename = "ears.small")]
    EarsSmall,
    #[serde(rename = "snout.button")]
    SnoutButton,
    #[serde(rename = "snout.muzzle")]
    SnoutMuzzle,
    #[serde(rename = "snout.long")]
    SnoutLong,
    #[serde(rename = "snout.broad")]
    SnoutBroad,
    #[serde(rename = "snout.trunk")]
    SnoutTrunk,
    #[serde(rename = "snout.beak")]
    SnoutBeak,
    #[serde(rename = "snout.hooked_beak")]
    SnoutHookedBeak,
    #[serde(rename = "snout.rostrum")]
    SnoutRostrum,
    #[serde(rename = "horns.nubs")]
    HornsNubs,
    #[serde(rename = "horns.swept")]
    HornsSwept,
    #[serde(rename = "horns.nose")]
    HornsNose,
    #[serde(rename = "horns.double_nose")]
    HornsDoubleNose,
    #[serde(rename = "horns.ossicones")]
    HornsOssicones,
    #[serde(rename = "horns.antlers")]
    HornsAntlers,
    #[serde(rename = "tusks.short")]
    TusksShort,
    #[serde(rename = "tusks.long")]
    TusksLong,
    #[serde(rename = "mane.ruff")]
    ManeRuff,
    #[serde(rename = "mane.crest")]
    ManeCrest,
    #[serde(rename = "mane.shaggy")]
    ManeShaggy,
    #[serde(rename = "mane.tuft")]
    ManeTuft,
    #[serde(rename = "tail.stub")]
    TailStub,
    #[serde(rename = "tail.taper")]
    TailTaper,
    #[serde(rename = "tail.tufted")]
    TailTufted,
    #[serde(rename = "tail.plume")]
    TailPlume,
    #[serde(rename = "tail.fluffy")]
    TailFluffy,
    #[serde(rename = "tail.curl")]
    TailCurl,
    #[serde(rename = "tail.fluke")]
    TailFluke,
    #[serde(rename = "tail.fan")]
    TailFan,
    #[serde(rename = "feet.paws")]
    FeetPaws,
    #[serde(rename = "feet.big_paws")]
    FeetBigPaws,
    #[serde(rename = "feet.hooves")]
    FeetHooves,
    #[serde(rename = "feet.pads")]
    FeetPads,
    #[serde(rename = "feet.hands")]
    FeetHands,
    #[serde(rename = "feet.talons")]
    FeetTalons,
    #[serde(rename = "fins.pectoral")]
    FinsPectoral,
    #[serde(rename = "fins.long")]
    FinsLong,
    #[serde(rename = "fins.flippers")]
    FinsFlippers,
    #[serde(rename = "back.dorsal")]
    BackDorsal,
    #[serde(rename = "back.tall_dorsal")]
    BackTallDorsal,
    #[serde(rename = "back.tiny_dorsal")]
    BackTinyDorsal,
    #[serde(rename = "back.hump")]
    BackHump,
    #[serde(rename = "back.ridge")]
    BackRidge,
    #[serde(rename = "wings.folded")]
    WingsFolded,
    #[serde(rename = "wings.spread")]
    WingsSpread,
    #[serde(rename = "wings.tiny")]
    WingsTiny,
    #[serde(rename = "shell.dome")]
    ShellDome,
    #[serde(rename = "shell.flat")]
    ShellFlat,
}

impl PartKind {
    pub const ALL: [Self; 54] = [
        Self::EarsRound,
        Self::EarsPointed,
        Self::EarsLong,
        Self::EarsFloppy,
        Self::EarsTufted,
        Self::EarsFan,
        Self::EarsSmall,
        Self::SnoutButton,
        Self::SnoutMuzzle,
        Self::SnoutLong,
        Self::SnoutBroad,
        Self::SnoutTrunk,
        Self::SnoutBeak,
        Self::SnoutHookedBeak,
        Self::SnoutRostrum,
        Self::HornsNubs,
        Self::HornsSwept,
        Self::HornsNose,
        Self::HornsDoubleNose,
        Self::HornsOssicones,
        Self::HornsAntlers,
        Self::TusksShort,
        Self::TusksLong,
        Self::ManeRuff,
        Self::ManeCrest,
        Self::ManeShaggy,
        Self::ManeTuft,
        Self::TailStub,
        Self::TailTaper,
        Self::TailTufted,
        Self::TailPlume,
        Self::TailFluffy,
        Self::TailCurl,
        Self::TailFluke,
        Self::TailFan,
        Self::FeetPaws,
        Self::FeetBigPaws,
        Self::FeetHooves,
        Self::FeetPads,
        Self::FeetHands,
        Self::FeetTalons,
        Self::FinsPectoral,
        Self::FinsLong,
        Self::FinsFlippers,
        Self::BackDorsal,
        Self::BackTallDorsal,
        Self::BackTinyDorsal,
        Self::BackHump,
        Self::BackRidge,
        Self::WingsFolded,
        Self::WingsSpread,
        Self::WingsTiny,
        Self::ShellDome,
        Self::ShellFlat,
    ];

    pub const fn slot(self) -> Slot {
        use PartKind as K;
        match self {
            K::EarsRound
            | K::EarsPointed
            | K::EarsLong
            | K::EarsFloppy
            | K::EarsTufted
            | K::EarsFan
            | K::EarsSmall => Slot::Ears,
            K::SnoutButton
            | K::SnoutMuzzle
            | K::SnoutLong
            | K::SnoutBroad
            | K::SnoutTrunk
            | K::SnoutBeak
            | K::SnoutHookedBeak
            | K::SnoutRostrum => Slot::Snout,
            K::HornsNubs
            | K::HornsSwept
            | K::HornsNose
            | K::HornsDoubleNose
            | K::HornsOssicones
            | K::HornsAntlers => Slot::Horns,
            K::TusksShort | K::TusksLong => Slot::Tusks,
            K::ManeRuff | K::ManeCrest | K::ManeShaggy | K::ManeTuft => Slot::Mane,
            K::TailStub
            | K::TailTaper
            | K::TailTufted
            | K::TailPlume
            | K::TailFluffy
            | K::TailCurl
            | K::TailFluke
            | K::TailFan => Slot::Tail,
            K::FeetPaws
            | K::FeetBigPaws
            | K::FeetHooves
            | K::FeetPads
            | K::FeetHands
            | K::FeetTalons => Slot::Feet,
            K::FinsPectoral | K::FinsLong | K::FinsFlippers => Slot::Fins,
            K::BackDorsal | K::BackTallDorsal | K::BackTinyDorsal | K::BackHump | K::BackRidge => {
                Slot::Back
            }
            K::WingsFolded | K::WingsSpread | K::WingsTiny => Slot::Wings,
            K::ShellDome | K::ShellFlat => Slot::Shell,
        }
    }

    pub const fn label(self) -> &'static str {
        use PartKind as K;
        match self {
            K::EarsRound => "Round",
            K::EarsPointed => "Pointed",
            K::EarsLong => "Long",
            K::EarsFloppy => "Floppy",
            K::EarsTufted => "Tufted",
            K::EarsFan => "Great fans",
            K::EarsSmall => "Small",
            K::SnoutButton => "Button nose",
            K::SnoutMuzzle => "Muzzle",
            K::SnoutLong => "Long muzzle",
            K::SnoutBroad => "Broad muzzle",
            K::SnoutTrunk => "Trunk",
            K::SnoutBeak => "Beak",
            K::SnoutHookedBeak => "Hooked beak",
            K::SnoutRostrum => "Rostrum",
            K::HornsNubs => "Nubs",
            K::HornsSwept => "Swept back",
            K::HornsNose => "Nose horn",
            K::HornsDoubleNose => "Two nose horns",
            K::HornsOssicones => "Ossicones",
            K::HornsAntlers => "Antlers",
            K::TusksShort => "Short",
            K::TusksLong => "Long",
            K::ManeRuff => "Ruff",
            K::ManeCrest => "Crest",
            K::ManeShaggy => "Shaggy",
            K::ManeTuft => "Tuft",
            K::TailStub => "Stub",
            K::TailTaper => "Taper",
            K::TailTufted => "Tufted",
            K::TailPlume => "Plume",
            K::TailFluffy => "Fluffy",
            K::TailCurl => "Curl",
            K::TailFluke => "Fluke",
            K::TailFan => "Feather fan",
            K::FeetPaws => "Paws",
            K::FeetBigPaws => "Big paws",
            K::FeetHooves => "Hooves",
            K::FeetPads => "Round pads",
            K::FeetHands => "Hands",
            K::FeetTalons => "Talons",
            K::FinsPectoral => "Pectoral",
            K::FinsLong => "Long",
            K::FinsFlippers => "Flippers",
            K::BackDorsal => "Dorsal fin",
            K::BackTallDorsal => "Tall dorsal fin",
            K::BackTinyDorsal => "Tiny dorsal fin",
            K::BackHump => "Hump",
            K::BackRidge => "Ridge",
            K::WingsFolded => "Folded",
            K::WingsSpread => "Spread",
            K::WingsTiny => "Tiny",
            K::ShellDome => "Dome",
            K::ShellFlat => "Flat",
        }
    }

    /// The catalogue identifier, as a form is written with it.
    pub fn id(self) -> String {
        serde_json::to_value(self)
            .ok()
            .and_then(|value| value.as_str().map(str::to_owned))
            .unwrap_or_default()
    }
}

/// Which of the coat's colours a part is drawn in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Ink {
    #[default]
    Primary,
    Secondary,
    Underside,
    Accent,
    Feature,
}

impl Ink {
    pub const ALL: [Self; 5] = [
        Self::Primary,
        Self::Secondary,
        Self::Underside,
        Self::Accent,
        Self::Feature,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Primary => "Primary",
            Self::Secondary => "Secondary",
            Self::Underside => "Underside",
            Self::Accent => "Accent",
            Self::Feature => "Feature",
        }
    }
}

/// One part fitted to a form.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Part {
    pub kind: PartKind,
    /// From 0 to [`STEPS`]; [`MIDDLE`] is the part as authored.
    pub size: u8,
    /// Turned forward (positive) or back, within [`NUDGE`].
    #[serde(default)]
    pub tilt: i8,
    /// Raised (positive) or lowered along its slot, within [`NUDGE`].
    #[serde(default)]
    pub lift: i8,
    /// For a pair, how much further the far one is tilted than the near one, within [`NUDGE`]:
    /// one ear flopped, one horn askew. Zero is a matched pair.
    #[serde(default)]
    pub uneven: i8,
    pub ink: Ink,
}

impl Part {
    pub fn new(kind: PartKind, ink: Ink) -> Self {
        Self {
            kind,
            size: MIDDLE,
            tilt: 0,
            lift: 0,
            uneven: 0,
            ink,
        }
    }

    pub fn sized(self, size: u8) -> Self {
        Self { size, ..self }
    }

    fn bounded(self) -> Self {
        let nudge = |value: i8| value.clamp(-NUDGE, NUDGE);
        Self {
            size: self.size.min(STEPS),
            tilt: nudge(self.tilt),
            lift: nudge(self.lift),
            uneven: if self.kind.slot().paired() {
                nudge(self.uneven)
            } else {
                0
            },
            ..self
        }
    }
}

/// What the coat is like to look at. Each is a way of shading, never a texture from a file.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Treatment {
    /// A soft edge, ruffled here and there.
    #[default]
    Fur,
    /// Long fur, ruffled all round.
    Shaggy,
    /// Smooth skin with a shine to it.
    Smooth,
    /// Rows of soft feathers.
    Feathers,
    /// Thick skin with a few folds in it.
    Plated,
}

impl Treatment {
    pub const ALL: [Self; 5] = [
        Self::Fur,
        Self::Shaggy,
        Self::Smooth,
        Self::Feathers,
        Self::Plated,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Fur => "Fur",
            Self::Shaggy => "Shaggy fur",
            Self::Smooth => "Smooth skin",
            Self::Feathers => "Feathers",
            Self::Plated => "Thick skin",
        }
    }
}

/// The coat: how it is treated and its colours.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Coat {
    pub treatment: Treatment,
    pub primary: [u8; 3],
    pub secondary: [u8; 3],
    /// The belly, chest and muzzle.
    pub underside: [u8; 3],
    pub accent: [u8; 3],
    /// Horns, tusks, beaks, hooves and claws.
    pub feature: [u8; 3],
    pub eyes: [u8; 3],
    /// How far the underside colour reaches, from none to the whole front.
    pub underside_reach: u8,
}

impl Default for Coat {
    fn default() -> Self {
        Self {
            treatment: Treatment::Fur,
            primary: [0xd9, 0xa5, 0x7b],
            secondary: [0x9a, 0x6a, 0x4b],
            underside: [0xf4, 0xe3, 0xc8],
            accent: [0xe8, 0x8a, 0x8a],
            feature: [0xf1, 0xe6, 0xd2],
            eyes: [0x20, 0x1b, 0x29],
            underside_reach: MIDDLE,
        }
    }
}

impl Coat {
    pub fn ink(&self, ink: Ink) -> [u8; 3] {
        match ink {
            Ink::Primary => self.primary,
            Ink::Secondary => self.secondary,
            Ink::Underside => self.underside,
            Ink::Accent => self.accent,
            Ink::Feature => self.feature,
        }
    }

    pub fn set_ink(&mut self, ink: Ink, rgb: [u8; 3]) {
        match ink {
            Ink::Primary => self.primary = rgb,
            Ink::Secondary => self.secondary = rgb,
            Ink::Underside => self.underside = rgb,
            Ink::Accent => self.accent = rgb,
            Ink::Feature => self.feature = rgb,
        }
    }
}

/// A way of marking a coat. Each is laid out over the regions of the body it belongs to, so it
/// follows the body whatever shape it is given: stripes run round the torso and down the legs,
/// eye patches sit on the face, socks stop where the legs do.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MarkingKind {
    Stripes,
    Spots,
    Rosettes,
    /// Large patches with thin lines of the coat between them.
    Patches,
    /// Soft blotches all over.
    Mottle,
    EyePatches,
    /// The lower face and muzzle.
    Mask,
    Socks,
    /// A band across the back.
    Saddle,
    /// The whole head.
    Cap,
    /// A band round the shoulders and forelegs.
    Shoulders,
}

impl MarkingKind {
    pub const ALL: [Self; 11] = [
        Self::Stripes,
        Self::Spots,
        Self::Rosettes,
        Self::Patches,
        Self::Mottle,
        Self::EyePatches,
        Self::Mask,
        Self::Socks,
        Self::Saddle,
        Self::Cap,
        Self::Shoulders,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Stripes => "Stripes",
            Self::Spots => "Spots",
            Self::Rosettes => "Rosettes",
            Self::Patches => "Patches",
            Self::Mottle => "Mottle",
            Self::EyePatches => "Eye patches",
            Self::Mask => "Mask",
            Self::Socks => "Socks",
            Self::Saddle => "Saddle",
            Self::Cap => "Cap",
            Self::Shoulders => "Shoulders",
        }
    }
}

/// One marking on the coat.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Marking {
    pub kind: MarkingKind,
    pub color: [u8; 3],
    /// How much of the coat it covers, from 0 to [`STEPS`].
    pub amount: u8,
    /// How large each mark is, from 0 to [`STEPS`].
    pub size: u8,
    /// Which of the many layouts of the same marking: a resolved value, never a promise about
    /// what some future randomiser would do.
    pub layout: u8,
}

impl Marking {
    pub fn new(kind: MarkingKind, color: [u8; 3]) -> Self {
        Self {
            kind,
            color,
            amount: MIDDLE,
            size: MIDDLE,
            layout: 0,
        }
    }

    fn bounded(self) -> Self {
        Self {
            amount: self.amount.min(STEPS),
            size: self.size.min(STEPS),
            ..self
        }
    }
}

/// A sculpted form: everything about how it looks but its face's features, which every form
/// shares (see [`super::Design`]).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Sculpt {
    pub plan: Plan,
    pub shape: Shape,
    /// At most one part in each slot, in slot order.
    pub parts: Vec<Part>,
    pub coat: Coat,
    /// At most [`MAX_MARKINGS`], drawn in order, so a later one goes over an earlier.
    #[serde(default)]
    pub markings: Vec<Marking>,
    /// Which of Formiga's authored face layouts it wears, 1 to
    /// [`crate::FACE_TEMPLATES`].
    pub face_template: u8,
}

impl Sculpt {
    /// A plain form on `plan`, with the parts that make the plan read as itself.
    pub fn starter(plan: Plan) -> Self {
        use Ink::*;
        use PartKind as K;
        let parts = match plan {
            Plan::CompactQuadruped => vec![
                Part::new(K::EarsRound, Primary),
                Part::new(K::SnoutMuzzle, Underside),
                Part::new(K::TailTaper, Primary),
                Part::new(K::FeetPaws, Primary),
            ],
            Plan::LargeQuadruped => vec![
                Part::new(K::EarsSmall, Primary),
                Part::new(K::SnoutBroad, Primary),
                Part::new(K::TailStub, Primary),
                Part::new(K::FeetPads, Primary),
            ],
            Plan::TallQuadruped => vec![
                Part::new(K::EarsPointed, Primary),
                Part::new(K::SnoutMuzzle, Underside),
                Part::new(K::TailTufted, Primary),
                Part::new(K::FeetHooves, Feature),
            ],
            Plan::Upright => vec![
                Part::new(K::EarsRound, Primary),
                Part::new(K::SnoutButton, Underside),
                Part::new(K::FeetHands, Primary),
            ],
            Plan::Floater => vec![
                Part::new(K::SnoutButton, Primary),
                Part::new(K::TailFluke, Primary),
                Part::new(K::FinsPectoral, Primary),
                Part::new(K::BackDorsal, Primary),
            ],
            Plan::Crawler => vec![
                Part::new(K::TailStub, Primary),
                Part::new(K::FinsFlippers, Primary),
                Part::new(K::ShellDome, Secondary),
            ],
            Plan::Percher => vec![
                Part::new(K::SnoutBeak, Feature),
                Part::new(K::TailFan, Primary),
                Part::new(K::FeetTalons, Feature),
                Part::new(K::WingsFolded, Primary),
            ],
        };
        let treatment = match plan {
            Plan::Floater => Treatment::Smooth,
            Plan::Crawler => Treatment::Smooth,
            Plan::Percher => Treatment::Feathers,
            Plan::LargeQuadruped => Treatment::Plated,
            _ => Treatment::Fur,
        };
        Self {
            plan,
            shape: Shape::default(),
            parts,
            coat: Coat {
                treatment,
                ..Coat::default()
            },
            markings: Vec::new(),
            face_template: 1,
        }
        .normalized()
    }

    /// The part in `slot`, if there is one.
    pub fn part(&self, slot: Slot) -> Option<&Part> {
        self.parts.iter().find(|part| part.kind.slot() == slot)
    }

    pub fn part_mut(&mut self, slot: Slot) -> Option<&mut Part> {
        self.parts.iter_mut().find(|part| part.kind.slot() == slot)
    }

    /// Fit `part` in its slot, in place of whatever was there.
    pub fn fit(&mut self, part: Part) {
        self.parts
            .retain(|fitted| fitted.kind.slot() != part.kind.slot());
        self.parts.push(part);
        self.parts.sort_by_key(|part| part.kind.slot());
    }

    /// Take whatever is in `slot` off, and return it.
    pub fn remove(&mut self, slot: Slot) -> Option<Part> {
        let index = self
            .parts
            .iter()
            .position(|part| part.kind.slot() == slot)?;
        Some(self.parts.remove(index))
    }

    /// The parts the plan has no slot for, which [`Self::normalized`] would take off.
    pub fn unfitted(&self) -> impl Iterator<Item = &Part> {
        self.parts
            .iter()
            .filter(|part| !self.plan.has(part.kind.slot()))
    }

    /// The same form with every value in range, the parts the plan has no slot for taken off,
    /// one part to a slot and in slot order, and markings to the limit. Whatever the renderer is
    /// given, it draws as this.
    pub fn normalized(&self) -> Self {
        let mut parts: Vec<Part> = Vec::with_capacity(self.parts.len());
        for part in &self.parts {
            if self.plan.has(part.kind.slot())
                && !parts
                    .iter()
                    .any(|fitted| fitted.kind.slot() == part.kind.slot())
            {
                parts.push(part.bounded());
            }
        }
        parts.sort_by_key(|part| part.kind.slot());
        let shape = Shape {
            head: self.shape.head.min(STEPS),
            length: self.shape.length.min(STEPS),
            depth: self.shape.depth.min(STEPS),
            neck: self.shape.neck.min(STEPS),
            legs: self.shape.legs.min(STEPS),
            arms: self.shape.arms.min(STEPS),
            girth: self.shape.girth.min(STEPS),
            tail: self.shape.tail.min(STEPS),
        };
        Self {
            plan: self.plan,
            shape,
            parts,
            coat: Coat {
                underside_reach: self.coat.underside_reach.min(STEPS),
                ..self.coat
            },
            markings: self
                .markings
                .iter()
                .take(MAX_MARKINGS)
                .map(|marking| marking.bounded())
                .collect(),
            face_template: self.face_template.clamp(1, crate::FACE_TEMPLATES),
        }
    }

    /// Whether the form is exactly as [`Self::normalized`] would leave it: what a proposal must
    /// be, since a reader checks rather than repairs.
    pub fn validate(&self) -> Result<(), FormError> {
        if let Some(part) = self.unfitted().next() {
            return Err(FormError::Invalid(format!(
                "a {} has no slot for {}",
                self.plan.label().to_lowercase(),
                part.kind.id()
            )));
        }
        for (index, part) in self.parts.iter().enumerate() {
            if self.parts[..index]
                .iter()
                .any(|earlier| earlier.kind.slot() >= part.kind.slot())
            {
                return Err(FormError::Invalid(
                    "parts must be one to a slot, in slot order".into(),
                ));
            }
        }
        if self.markings.len() > MAX_MARKINGS {
            return Err(FormError::Invalid(format!(
                "a coat carries at most {MAX_MARKINGS} markings"
            )));
        }
        if *self != self.normalized() {
            return Err(FormError::Invalid(
                "a value is out of the range the form allows".into(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_starter_is_already_normal_and_fits_its_own_plan() {
        for plan in Plan::ALL {
            let sculpt = Sculpt::starter(plan);
            assert_eq!(sculpt.validate(), Ok(()), "{plan:?}");
            assert!(sculpt.parts.iter().all(|part| plan.has(part.kind.slot())));
        }
    }

    #[test]
    fn every_part_has_one_catalogue_identifier_that_names_its_slot() {
        let mut seen = std::collections::HashSet::new();
        for kind in PartKind::ALL {
            let id = kind.id();
            assert!(seen.insert(id.clone()), "{id} twice");
            let slot = serde_json::to_value(kind.slot()).unwrap();
            assert!(
                id.starts_with(&format!("{}.", slot.as_str().unwrap())),
                "{id}"
            );
            let back: PartKind = serde_json::from_value(serde_json::Value::String(id)).unwrap();
            assert_eq!(back, kind);
        }
        for slot in Slot::ALL {
            assert!(slot.kinds().count() >= 2, "{slot:?} has a real choice");
        }
    }

    #[test]
    fn an_unknown_part_is_refused_rather_than_guessed() {
        let mut value = serde_json::to_value(Sculpt::starter(Plan::Floater)).unwrap();
        value["parts"][0]["kind"] = "snout.laser".into();
        assert!(serde_json::from_value::<Sculpt>(value).is_err());
    }

    #[test]
    fn a_part_without_a_slot_on_the_plan_is_refused_and_normalizing_takes_it_off() {
        let mut sculpt = Sculpt::starter(Plan::Floater);
        sculpt
            .parts
            .push(Part::new(PartKind::FeetHooves, Ink::Feature));
        assert!(sculpt.validate().is_err());
        let normal = sculpt.normalized();
        assert!(normal.part(Slot::Feet).is_none());
        assert_eq!(normal.validate(), Ok(()));
    }

    #[test]
    fn two_parts_in_one_slot_are_refused() {
        let mut sculpt = Sculpt::starter(Plan::CompactQuadruped);
        sculpt
            .parts
            .push(Part::new(PartKind::EarsFan, Ink::Primary));
        assert!(sculpt.validate().is_err());
        let normal = sculpt.normalized();
        assert_eq!(
            normal.part(Slot::Ears).map(|part| part.kind),
            Some(PartKind::EarsRound),
            "the first fitted stays"
        );
    }

    #[test]
    fn values_out_of_range_are_refused_and_normalize_into_range() {
        let mut sculpt = Sculpt::starter(Plan::Upright);
        sculpt.shape.head = 200;
        sculpt.parts[0].tilt = -100;
        sculpt.markings = vec![Marking::new(MarkingKind::Spots, [0, 0, 0]); 9];
        assert!(sculpt.validate().is_err());
        let normal = sculpt.normalized();
        assert_eq!(normal.shape.head, STEPS);
        assert_eq!(normal.parts[0].tilt, -NUDGE);
        assert_eq!(normal.markings.len(), MAX_MARKINGS);
        assert_eq!(normal.validate(), Ok(()));
    }

    #[test]
    fn only_a_pair_can_be_uneven() {
        let mut sculpt = Sculpt::starter(Plan::CompactQuadruped);
        sculpt.part_mut(Slot::Tail).unwrap().uneven = 2;
        sculpt.part_mut(Slot::Ears).unwrap().uneven = 2;
        let normal = sculpt.normalized();
        assert_eq!(normal.part(Slot::Tail).unwrap().uneven, 0);
        assert_eq!(normal.part(Slot::Ears).unwrap().uneven, 2);
    }
}
