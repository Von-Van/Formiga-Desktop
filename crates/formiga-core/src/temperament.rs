//! Who a companion is: nine plain-language axes, the broad kind of companion it started out as,
//! now and then a tension between two things that do not usually go together, and the three
//! traits that read out of all of that.
//!
//! A companion made since 0.62.0 is given a temperament when it is made. Its kind is drawn first,
//! its axes are drawn around that kind — a kind is where a companion starts, not a package it
//! comes in — and the behaviour values the rest of the simulation reads are then taken from the
//! axes, so a grump really does keep to itself rather than only being called a grump.
//!
//! Every older companion reads its temperament out of the values it already had. Nothing is
//! drawn for it: its sociability, energy, boldness, playfulness and curiosity are its own, and the
//! four sides it never had — feistiness, impulsiveness, suspicion and affection — read as
//! middling, which is also where they leave its behaviour exactly as it was.

use crate::{PersonalityGenome, SeedStream};
use rand::Rng;
use serde::{Deserialize, Serialize};

/// Where a companion sits between two ends of nine plain-language scales, each from 0 to 1 with
/// 0.5 in the middle. The name of each field is the end that 1 stands for.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Axes {
    /// Solitary to social: how much it wants other companions about.
    pub social: f32,
    /// Calm to energetic.
    pub energy: f32,
    /// Cautious to bold.
    pub boldness: f32,
    /// Serious to playful.
    pub playfulness: f32,
    /// Incurious to curious.
    pub curiosity: f32,
    /// Gentle to feisty.
    pub feistiness: f32,
    /// Patient to impulsive.
    pub impulsiveness: f32,
    /// Trusting to suspicious.
    pub suspicion: f32,
    /// Independent to affectionate: how much it wants the person at the desk.
    pub affection: f32,
}

impl Axes {
    /// Every axis at its middle.
    pub const MIDDLING: Self = Self {
        social: 0.5,
        energy: 0.5,
        boldness: 0.5,
        playfulness: 0.5,
        curiosity: 0.5,
        feistiness: 0.5,
        impulsiveness: 0.5,
        suspicion: 0.5,
        affection: 0.5,
    };

    const fn from_array(values: [f32; 9]) -> Self {
        Self {
            social: values[0],
            energy: values[1],
            boldness: values[2],
            playfulness: values[3],
            curiosity: values[4],
            feistiness: values[5],
            impulsiveness: values[6],
            suspicion: values[7],
            affection: values[8],
        }
    }

    fn to_array(self) -> [f32; 9] {
        [
            self.social,
            self.energy,
            self.boldness,
            self.playfulness,
            self.curiosity,
            self.feistiness,
            self.impulsiveness,
            self.suspicion,
            self.affection,
        ]
    }

    fn map(self, mut f: impl FnMut(usize, f32) -> f32) -> Self {
        let mut values = self.to_array();
        for (index, value) in values.iter_mut().enumerate() {
            *value = f(index, *value);
        }
        Self::from_array(values)
    }

    /// Every value kept on its scale.
    pub fn bounded(self) -> Self {
        self.map(|_, value| {
            if value.is_finite() {
                value.clamp(0.0, 1.0)
            } else {
                0.5
            }
        })
    }

    /// How far from the middle the companion sits, on average: 0 for a thoroughly middling one,
    /// 1 for one at an end of every scale.
    pub fn extremeness(self) -> f32 {
        let values = self.to_array();
        values
            .iter()
            .map(|value| (value - 0.5).abs() * 2.0)
            .sum::<f32>()
            / values.len() as f32
    }
}

/// The broad kind of companion a temperament starts from. It sets where the axes are drawn from
/// and which traits come most readily, and it names the companion on its profile, but the axes are
/// drawn loosely around it, so two grumps are never the same grump.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TemperamentKind {
    Sweetheart,
    Troublemaker,
    Grump,
    Explorer,
    Wallflower,
    Showoff,
    Scholar,
    Oddball,
    Lazybones,
    Guardian,
}

impl TemperamentKind {
    pub const ALL: [Self; 10] = [
        Self::Sweetheart,
        Self::Troublemaker,
        Self::Grump,
        Self::Explorer,
        Self::Wallflower,
        Self::Showoff,
        Self::Scholar,
        Self::Oddball,
        Self::Lazybones,
        Self::Guardian,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Sweetheart => "Sweetheart",
            Self::Troublemaker => "Troublemaker",
            Self::Grump => "Grump",
            Self::Explorer => "Explorer",
            Self::Wallflower => "Wallflower",
            Self::Showoff => "Showoff",
            Self::Scholar => "Scholar",
            Self::Oddball => "Oddball",
            Self::Lazybones => "Lazybones",
            Self::Guardian => "Guardian",
        }
    }

    /// How often a new companion starts from this kind, out of a hundred. Oddballs are rarer on
    /// purpose; the rest are close to even, so no one kind of companion fills a colony.
    const fn weight(self) -> u32 {
        match self {
            Self::Oddball => 6,
            Self::Scholar => 9,
            Self::Grump | Self::Wallflower | Self::Showoff => 10,
            Self::Sweetheart
            | Self::Troublemaker
            | Self::Explorer
            | Self::Lazybones
            | Self::Guardian => 11,
        }
    }

    /// Where the axes of a companion of this kind are drawn around, in `Axes` field order:
    /// social, energy, boldness, playfulness, curiosity, feistiness, impulsiveness, suspicion,
    /// affection. An oddball has no middle of its own; see `Temperament::draw_axes`.
    const fn centre(self) -> [f32; 9] {
        match self {
            Self::Sweetheart => [0.72, 0.50, 0.42, 0.62, 0.50, 0.14, 0.40, 0.15, 0.90],
            Self::Troublemaker => [0.66, 0.82, 0.80, 0.86, 0.60, 0.62, 0.86, 0.30, 0.50],
            Self::Grump => [0.24, 0.35, 0.60, 0.16, 0.35, 0.86, 0.40, 0.66, 0.48],
            Self::Explorer => [0.50, 0.80, 0.80, 0.55, 0.92, 0.40, 0.66, 0.25, 0.38],
            Self::Wallflower => [0.16, 0.30, 0.15, 0.42, 0.56, 0.20, 0.25, 0.62, 0.66],
            Self::Showoff => [0.84, 0.76, 0.82, 0.62, 0.40, 0.62, 0.62, 0.24, 0.34],
            Self::Scholar => [0.34, 0.28, 0.45, 0.18, 0.92, 0.40, 0.14, 0.45, 0.44],
            Self::Oddball => [0.5; 9],
            Self::Lazybones => [0.50, 0.08, 0.40, 0.34, 0.24, 0.45, 0.30, 0.30, 0.64],
            Self::Guardian => [0.40, 0.52, 0.78, 0.24, 0.45, 0.62, 0.24, 0.82, 0.78],
        }
    }

    /// The axes that make a companion this kind, and which way, as `(axis index, direction)`.
    /// Reading an older companion's kind looks only at these.
    const fn signature(self) -> &'static [(usize, f32)] {
        match self {
            Self::Sweetheart => &[(8, 1.0), (5, -1.0), (0, 1.0), (7, -1.0)],
            Self::Troublemaker => &[(3, 1.0), (1, 1.0), (6, 1.0), (2, 1.0)],
            Self::Grump => &[(5, 1.0), (3, -1.0), (0, -1.0), (7, 1.0)],
            Self::Explorer => &[(4, 1.0), (1, 1.0), (2, 1.0)],
            Self::Wallflower => &[(0, -1.0), (2, -1.0), (1, -1.0)],
            Self::Showoff => &[(0, 1.0), (2, 1.0), (1, 1.0), (8, -1.0)],
            Self::Scholar => &[(4, 1.0), (3, -1.0), (1, -1.0), (6, -1.0)],
            Self::Oddball => &[],
            Self::Lazybones => &[(1, -1.0), (4, -1.0), (3, -1.0)],
            Self::Guardian => &[(7, 1.0), (8, 1.0), (2, 1.0), (3, -1.0)],
        }
    }

    /// The traits that come most readily to this kind.
    const fn pool(self) -> &'static [Trait] {
        use Trait::*;
        match self {
            Self::Sweetheart => &[Affectionate, Gentle, Trusting, Nurturing, Sweet, Clingy],
            Self::Troublemaker => &[
                Mischievous,
                Impulsive,
                Excitable,
                Reckless,
                Messy,
                Spontaneous,
            ],
            Self::Grump => &[Stubborn, Irritable, Grumpy, Suspicious, Aloof, Picky],
            Self::Explorer => &[Curious, Energetic, Brave, Distractible, Restless, Nosy],
            Self::Wallflower => &[Shy, Observant, Cautious, Loyal, Nervous, Wary],
            Self::Showoff => &[
                Confident,
                Dramatic,
                Competitive,
                AttentionSeeking,
                Vain,
                Bossy,
            ],
            Self::Scholar => &[Curious, Patient, Serious, Particular, Observant, Obsessive],
            Self::Oddball => &[Eccentric, Unpredictable, Obsessive, Independent, Dramatic],
            Self::Lazybones => &[Sleepy, Mellow, FoodMotivated, Stubborn, Lazy, Unbothered],
            Self::Guardian => &[Protective, Serious, Loyal, Suspicious, Watchful, Bossy],
        }
    }

    /// What a companion of this kind can be called on its profile, each with the traits it only
    /// suits: a glutton has to be food-motivated, and a diva vain, dramatic or after attention. A
    /// name with none listed suits anyone of the kind. One is chosen for each companion and kept
    /// for good.
    const fn nouns(self) -> &'static [(&'static str, &'static [Trait])] {
        use Trait::*;
        match self {
            Self::Sweetheart => &[
                ("sweetheart", &[]),
                ("softie", &[]),
                ("cuddlebug", &[Affectionate, Clingy, AttentionSeeking]),
            ],
            Self::Troublemaker => &[("troublemaker", &[]), ("scamp", &[]), ("rascal", &[])],
            Self::Grump => &[
                ("grump", &[]),
                ("grouch", &[]),
                ("curmudgeon", &[Stubborn, Picky, Stoic]),
            ],
            Self::Explorer => &[
                ("explorer", &[]),
                ("adventurer", &[Brave, Reckless, Energetic]),
                ("wanderer", &[Restless, Distractible, Independent]),
            ],
            Self::Wallflower => &[
                ("wallflower", &[]),
                ("worrier", &[Nervous, Wary, Cautious, Cowardly]),
            ],
            Self::Showoff => &[
                ("showoff", &[]),
                ("star", &[Confident, Outgoing]),
                ("diva", &[Vain, Dramatic, AttentionSeeking]),
            ],
            Self::Scholar => &[
                ("scholar", &[]),
                ("thinker", &[]),
                ("professor", &[Particular, Serious, Obsessive]),
            ],
            Self::Oddball => &[
                ("oddball", &[]),
                (
                    "gremlin",
                    &[Mischievous, Dramatic, Impulsive, Messy, Grumpy, Restless],
                ),
                ("mystery", &[Aloof, Independent, Loner, Wary, Suspicious]),
            ],
            Self::Lazybones => &[
                ("lazybones", &[]),
                ("napper", &[Sleepy, Mellow, Lazy]),
                ("glutton", &[FoodMotivated]),
            ],
            Self::Guardian => &[
                ("guardian", &[]),
                ("watchdog", &[Suspicious, Watchful, Wary]),
                ("sentry", &[Serious, Stoic, Watchful]),
            ],
        }
    }

    /// How likely a companion of this kind is to carry a tension, and which ones suit it.
    const fn tensions(self) -> (f64, &'static [Tension]) {
        use Tension::*;
        match self {
            Self::Sweetheart => (0.18, &[GentleButStubborn, AffectionateButIndependent]),
            Self::Troublemaker => (0.18, &[DramaticButCowardly, PlayfulButShy]),
            Self::Grump => (
                0.5,
                &[
                    GrumpyButAffectionate,
                    GrumpyButAffectionate,
                    GentleButStubborn,
                ],
            ),
            Self::Explorer => (0.2, &[BraveButNervous, SeriousButCurious]),
            Self::Wallflower => (0.24, &[PlayfulButShy, SuspiciousButLoyal]),
            Self::Showoff => (0.28, &[ConfidentButEasilyEmbarrassed, DramaticButCowardly]),
            Self::Scholar => (0.2, &[SeriousButCurious, AffectionateButIndependent]),
            Self::Oddball => (0.34, &Tension::ALL),
            Self::Lazybones => (0.26, &[LazyButCompetitive]),
            Self::Guardian => (0.3, &[SuspiciousButLoyal, BraveButNervous]),
        }
    }
}

/// Two sides of a companion that do not usually go together. A tension is drawn on purpose, now
/// and then, and it changes what the companion does as well as how it reads: a brave but nervous
/// one walks straight up to things and then jumps at them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Tension {
    BraveButNervous,
    AffectionateButIndependent,
    PlayfulButShy,
    GentleButStubborn,
    SeriousButCurious,
    ConfidentButEasilyEmbarrassed,
    LazyButCompetitive,
    SuspiciousButLoyal,
    DramaticButCowardly,
    GrumpyButAffectionate,
}

impl Tension {
    pub const ALL: [Self; 10] = [
        Self::BraveButNervous,
        Self::AffectionateButIndependent,
        Self::PlayfulButShy,
        Self::GentleButStubborn,
        Self::SeriousButCurious,
        Self::ConfidentButEasilyEmbarrassed,
        Self::LazyButCompetitive,
        Self::SuspiciousButLoyal,
        Self::DramaticButCowardly,
        Self::GrumpyButAffectionate,
    ];

    /// The two traits it reads as, the surprising one second.
    pub const fn traits(self) -> [Trait; 2] {
        use Trait::*;
        match self {
            Self::BraveButNervous => [Brave, Nervous],
            Self::AffectionateButIndependent => [Affectionate, Independent],
            Self::PlayfulButShy => [Playful, Shy],
            Self::GentleButStubborn => [Gentle, Stubborn],
            Self::SeriousButCurious => [Serious, Curious],
            Self::ConfidentButEasilyEmbarrassed => [Confident, EasilyEmbarrassed],
            Self::LazyButCompetitive => [Lazy, Competitive],
            Self::SuspiciousButLoyal => [Suspicious, DeeplyLoyal],
            Self::DramaticButCowardly => [Dramatic, Cowardly],
            Self::GrumpyButAffectionate => [Grumpy, SecretlyAffectionate],
        }
    }

    /// The ranges its axes are held to, so the companion behaves as both halves say, as
    /// `(axis index, low, high)`.
    const fn holds(self) -> &'static [(usize, f32, f32)] {
        match self {
            Self::BraveButNervous => &[(2, 0.72, 1.0), (1, 0.6, 1.0)],
            Self::AffectionateButIndependent => &[(8, 0.72, 1.0), (0, 0.0, 0.34)],
            Self::PlayfulButShy => &[(3, 0.72, 1.0), (2, 0.0, 0.34), (0, 0.0, 0.46)],
            Self::GentleButStubborn => &[(5, 0.0, 0.32), (6, 0.0, 0.3)],
            Self::SeriousButCurious => &[(3, 0.0, 0.28), (4, 0.76, 1.0)],
            Self::ConfidentButEasilyEmbarrassed => &[(2, 0.72, 1.0), (0, 0.6, 1.0)],
            Self::LazyButCompetitive => &[(1, 0.0, 0.24), (5, 0.62, 1.0)],
            Self::SuspiciousButLoyal => &[(7, 0.72, 1.0), (8, 0.72, 1.0)],
            Self::DramaticButCowardly => &[(2, 0.0, 0.24), (1, 0.6, 1.0), (0, 0.56, 1.0)],
            Self::GrumpyButAffectionate => &[(5, 0.7, 1.0), (3, 0.0, 0.32), (8, 0.68, 1.0)],
        }
    }
}

/// Whether a trait is a warm one, a plain one, or one of the mild flaws that make a companion
/// somebody in particular rather than another nice one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Valence {
    Warm,
    Plain,
    Flaw,
}

/// The side of a companion a trait is about. A profile shows at most one trait from each, so
/// three traits say three different things.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Family {
    Social,
    Energy,
    Boldness,
    Play,
    Feisty,
    Impulse,
    Trust,
    Affection,
    Curiosity,
    /// A trait of its own, which can sit beside anything it does not contradict.
    Flavour,
}

/// Everything a companion's profile can call it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Trait {
    Outgoing,
    Chatty,
    Clingy,
    Loner,
    Energetic,
    Excitable,
    Restless,
    Mellow,
    Sleepy,
    Lazy,
    Brave,
    Confident,
    Reckless,
    Cautious,
    Shy,
    Nervous,
    Cowardly,
    Playful,
    Mischievous,
    Silly,
    Serious,
    Stoic,
    Grumpy,
    Feisty,
    Stubborn,
    Bossy,
    Irritable,
    Competitive,
    Gentle,
    Sweet,
    Easygoing,
    Impulsive,
    Impatient,
    Distractible,
    Spontaneous,
    Patient,
    Steady,
    Particular,
    Picky,
    Suspicious,
    Wary,
    Watchful,
    Trusting,
    Friendly,
    Affectionate,
    Loyal,
    Jealous,
    Protective,
    Nurturing,
    Independent,
    Aloof,
    Curious,
    Nosy,
    Obsessive,
    Observant,
    Unbothered,
    Dramatic,
    Vain,
    AttentionSeeking,
    FoodMotivated,
    Messy,
    Eccentric,
    Unpredictable,
    NightOwl,
    EarlyRiser,
    EasilyEmbarrassed,
    SecretlyAffectionate,
    DeeplyLoyal,
}

/// How strongly the upper end of an axis shows: nothing at the middle, 1 at the very end.
fn hi(value: f32) -> f32 {
    ((value - 0.5) * 2.0).max(0.0)
}

/// How strongly the lower end of an axis shows.
fn lo(value: f32) -> f32 {
    ((0.5 - value) * 2.0).max(0.0)
}

fn all(values: &[f32]) -> f32 {
    values.iter().copied().fold(1.0, f32::min)
}

impl Trait {
    pub const ALL: [Self; 68] = [
        Self::Outgoing,
        Self::Chatty,
        Self::Clingy,
        Self::Loner,
        Self::Energetic,
        Self::Excitable,
        Self::Restless,
        Self::Mellow,
        Self::Sleepy,
        Self::Lazy,
        Self::Brave,
        Self::Confident,
        Self::Reckless,
        Self::Cautious,
        Self::Shy,
        Self::Nervous,
        Self::Cowardly,
        Self::Playful,
        Self::Mischievous,
        Self::Silly,
        Self::Serious,
        Self::Stoic,
        Self::Grumpy,
        Self::Feisty,
        Self::Stubborn,
        Self::Bossy,
        Self::Irritable,
        Self::Competitive,
        Self::Gentle,
        Self::Sweet,
        Self::Easygoing,
        Self::Impulsive,
        Self::Impatient,
        Self::Distractible,
        Self::Spontaneous,
        Self::Patient,
        Self::Steady,
        Self::Particular,
        Self::Picky,
        Self::Suspicious,
        Self::Wary,
        Self::Watchful,
        Self::Trusting,
        Self::Friendly,
        Self::Affectionate,
        Self::Loyal,
        Self::Jealous,
        Self::Protective,
        Self::Nurturing,
        Self::Independent,
        Self::Aloof,
        Self::Curious,
        Self::Nosy,
        Self::Obsessive,
        Self::Observant,
        Self::Unbothered,
        Self::Dramatic,
        Self::Vain,
        Self::AttentionSeeking,
        Self::FoodMotivated,
        Self::Messy,
        Self::Eccentric,
        Self::Unpredictable,
        Self::NightOwl,
        Self::EarlyRiser,
        Self::EasilyEmbarrassed,
        Self::SecretlyAffectionate,
        Self::DeeplyLoyal,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Outgoing => "Outgoing",
            Self::Chatty => "Chatty",
            Self::Clingy => "Clingy",
            Self::Loner => "Loner",
            Self::Energetic => "Energetic",
            Self::Excitable => "Excitable",
            Self::Restless => "Restless",
            Self::Mellow => "Mellow",
            Self::Sleepy => "Sleepy",
            Self::Lazy => "Lazy",
            Self::Brave => "Brave",
            Self::Confident => "Confident",
            Self::Reckless => "Reckless",
            Self::Cautious => "Cautious",
            Self::Shy => "Shy",
            Self::Nervous => "Nervous",
            Self::Cowardly => "Cowardly",
            Self::Playful => "Playful",
            Self::Mischievous => "Mischievous",
            Self::Silly => "Silly",
            Self::Serious => "Serious",
            Self::Stoic => "Stoic",
            Self::Grumpy => "Grumpy",
            Self::Feisty => "Feisty",
            Self::Stubborn => "Stubborn",
            Self::Bossy => "Bossy",
            Self::Irritable => "Irritable",
            Self::Competitive => "Competitive",
            Self::Gentle => "Gentle",
            Self::Sweet => "Sweet",
            Self::Easygoing => "Easygoing",
            Self::Impulsive => "Impulsive",
            Self::Impatient => "Impatient",
            Self::Distractible => "Distractible",
            Self::Spontaneous => "Spontaneous",
            Self::Patient => "Patient",
            Self::Steady => "Steady",
            Self::Particular => "Particular",
            Self::Picky => "Picky",
            Self::Suspicious => "Suspicious",
            Self::Wary => "Wary",
            Self::Watchful => "Watchful",
            Self::Trusting => "Trusting",
            Self::Friendly => "Friendly",
            Self::Affectionate => "Affectionate",
            Self::Loyal => "Loyal",
            Self::Jealous => "Jealous",
            Self::Protective => "Protective",
            Self::Nurturing => "Nurturing",
            Self::Independent => "Independent",
            Self::Aloof => "Aloof",
            Self::Curious => "Curious",
            Self::Nosy => "Nosy",
            Self::Obsessive => "Obsessive",
            Self::Observant => "Observant",
            Self::Unbothered => "Unbothered",
            Self::Dramatic => "Dramatic",
            Self::Vain => "Vain",
            Self::AttentionSeeking => "Attention-seeking",
            Self::FoodMotivated => "Food-motivated",
            Self::Messy => "Messy",
            Self::Eccentric => "Eccentric",
            Self::Unpredictable => "Unpredictable",
            Self::NightOwl => "Night owl",
            Self::EarlyRiser => "Early riser",
            Self::EasilyEmbarrassed => "Easily embarrassed",
            Self::SecretlyAffectionate => "Secretly affectionate",
            Self::DeeplyLoyal => "Deeply loyal",
        }
    }

    pub const fn valence(self) -> Valence {
        use Valence::*;
        match self {
            Self::Clingy
            | Self::Restless
            | Self::Lazy
            | Self::Reckless
            | Self::Nervous
            | Self::Cowardly
            | Self::Grumpy
            | Self::Stubborn
            | Self::Bossy
            | Self::Irritable
            | Self::Competitive
            | Self::Impulsive
            | Self::Impatient
            | Self::Distractible
            | Self::Picky
            | Self::Suspicious
            | Self::Jealous
            | Self::Aloof
            | Self::Nosy
            | Self::Dramatic
            | Self::Vain
            | Self::AttentionSeeking
            | Self::Messy
            | Self::EasilyEmbarrassed => Flaw,
            Self::Outgoing
            | Self::Energetic
            | Self::Mellow
            | Self::Brave
            | Self::Confident
            | Self::Playful
            | Self::Silly
            | Self::Gentle
            | Self::Sweet
            | Self::Easygoing
            | Self::Patient
            | Self::Steady
            | Self::Trusting
            | Self::Friendly
            | Self::Affectionate
            | Self::Loyal
            | Self::Protective
            | Self::Nurturing
            | Self::Curious
            | Self::Observant
            | Self::SecretlyAffectionate
            | Self::DeeplyLoyal => Warm,
            _ => Plain,
        }
    }

    const fn family(self) -> Family {
        use Family::*;
        match self {
            Self::Outgoing | Self::Chatty | Self::Loner => Social,
            Self::Energetic | Self::Excitable | Self::Restless | Self::Mellow | Self::Sleepy => {
                Energy
            }
            Self::Lazy => Energy,
            Self::Brave
            | Self::Confident
            | Self::Reckless
            | Self::Cautious
            | Self::Shy
            | Self::Nervous
            | Self::Cowardly => Boldness,
            Self::Playful
            | Self::Mischievous
            | Self::Silly
            | Self::Serious
            | Self::Stoic
            | Self::Grumpy => Play,
            Self::Feisty
            | Self::Stubborn
            | Self::Bossy
            | Self::Irritable
            | Self::Competitive
            | Self::Gentle
            | Self::Sweet
            | Self::Easygoing => Feisty,
            Self::Impulsive
            | Self::Impatient
            | Self::Distractible
            | Self::Spontaneous
            | Self::Patient
            | Self::Steady
            | Self::Particular
            | Self::Picky => Impulse,
            Self::Suspicious | Self::Wary | Self::Watchful | Self::Trusting | Self::Friendly => {
                Trust
            }
            Self::Affectionate
            | Self::Loyal
            | Self::Jealous
            | Self::Protective
            | Self::Nurturing
            | Self::Independent
            | Self::Aloof
            | Self::Clingy => Affection,
            Self::Curious | Self::Nosy | Self::Obsessive | Self::Observant | Self::Unbothered => {
                Curiosity
            }
            _ => Flavour,
        }
    }

    /// Which way each axis leans for this trait, as `(axis index, sign)`: two traits that lean
    /// opposite ways on any axis contradict each other, and only a tension puts them together.
    const fn leans(self) -> &'static [(usize, i8)] {
        match self {
            Self::Outgoing => &[(0, 1)],
            Self::Chatty => &[(0, 1), (1, 1)],
            Self::Clingy => &[(8, 1), (0, 1)],
            Self::Loner => &[(0, -1)],
            Self::Energetic => &[(1, 1)],
            Self::Excitable => &[(1, 1), (6, 1)],
            Self::Restless => &[(1, 1), (6, 1)],
            Self::Mellow => &[(1, -1), (5, -1)],
            Self::Sleepy => &[(1, -1)],
            Self::Lazy => &[(1, -1), (3, -1)],
            Self::Brave => &[(2, 1)],
            Self::Confident => &[(2, 1), (0, 1)],
            Self::Reckless => &[(2, 1), (6, 1)],
            Self::Cautious => &[(2, -1)],
            Self::Shy => &[(2, -1), (0, -1)],
            Self::Nervous => &[(2, -1), (1, 1)],
            Self::Cowardly => &[(2, -1)],
            Self::Playful => &[(3, 1)],
            Self::Mischievous => &[(3, 1), (6, 1)],
            Self::Silly => &[(3, 1), (5, -1)],
            Self::Serious => &[(3, -1)],
            Self::Stoic => &[(3, -1), (1, -1)],
            Self::Grumpy => &[(3, -1), (5, 1)],
            Self::Feisty => &[(5, 1)],
            Self::Stubborn => &[(5, 1), (6, -1)],
            Self::Bossy => &[(5, 1), (0, 1)],
            Self::Irritable => &[(5, 1), (0, -1)],
            Self::Competitive => &[(5, 1), (1, 1)],
            Self::Gentle => &[(5, -1)],
            Self::Sweet => &[(5, -1), (8, 1)],
            Self::Easygoing => &[(5, -1), (6, -1)],
            Self::Impulsive => &[(6, 1)],
            Self::Impatient => &[(6, 1), (1, 1)],
            Self::Distractible => &[(6, 1), (4, 1)],
            Self::Spontaneous => &[(6, 1), (3, 1)],
            Self::Patient => &[(6, -1)],
            Self::Steady => &[(6, -1), (1, -1)],
            Self::Particular => &[(6, -1)],
            Self::Picky => &[(6, -1), (5, 1)],
            Self::Suspicious => &[(7, 1)],
            Self::Wary => &[(7, 1), (2, -1)],
            Self::Watchful => &[(7, 1), (2, 1)],
            Self::Trusting => &[(7, -1)],
            Self::Friendly => &[(7, -1), (0, 1)],
            Self::Affectionate => &[(8, 1)],
            Self::Loyal => &[(8, 1), (6, -1)],
            Self::Jealous => &[(8, 1), (5, 1)],
            Self::Protective => &[(8, 1), (2, 1)],
            Self::Nurturing => &[(8, 1), (5, -1)],
            Self::Independent => &[(8, -1)],
            Self::Aloof => &[(8, -1), (0, -1)],
            Self::Curious => &[(4, 1)],
            Self::Nosy => &[(4, 1), (7, -1)],
            Self::Obsessive => &[(4, 1), (6, -1)],
            Self::Observant => &[(4, 1), (1, -1)],
            Self::Unbothered => &[(4, -1), (7, -1)],
            Self::Dramatic => &[(1, 1), (0, 1)],
            Self::Vain => &[(0, 1), (8, -1)],
            Self::AttentionSeeking => &[(0, 1), (8, 1)],
            Self::FoodMotivated => &[(1, -1)],
            Self::Messy => &[(6, 1), (3, 1)],
            Self::Eccentric | Self::NightOwl | Self::EarlyRiser => &[],
            Self::Unpredictable => &[(6, 1)],
            Self::EasilyEmbarrassed => &[(2, -1)],
            Self::SecretlyAffectionate => &[(8, 1)],
            Self::DeeplyLoyal => &[(8, 1)],
        }
    }

    /// How strongly a companion shows this trait, from nothing to 1. `sleep` is its sleep timing,
    /// which is not one of the axes but says whether it keeps late or early hours.
    fn strength(self, a: Axes, sleep: f32) -> f32 {
        let Axes {
            social,
            energy,
            boldness,
            playfulness,
            curiosity,
            feistiness,
            impulsiveness,
            suspicion,
            affection,
        } = a;
        match self {
            Self::Outgoing => hi(social),
            Self::Chatty => all(&[hi(social), hi(energy)]),
            Self::Clingy => all(&[hi(affection), hi(social), lo(suspicion)]) * 1.3,
            Self::Loner => lo(social) * lo(social) * 1.1,
            Self::Energetic => hi(energy),
            Self::Excitable => all(&[hi(energy), hi(impulsiveness)]) * 1.15,
            Self::Restless => all(&[hi(energy), hi(impulsiveness), lo(playfulness)]) * 1.3,
            Self::Mellow => all(&[lo(energy), lo(feistiness)]) * 1.15,
            Self::Sleepy => lo(energy),
            Self::Lazy => all(&[lo(energy), lo(playfulness)]) * 1.2,
            Self::Brave => hi(boldness),
            Self::Confident => all(&[hi(boldness), hi(social)]) * 1.15,
            Self::Reckless => all(&[hi(boldness), hi(impulsiveness)]) * 1.1,
            Self::Cautious => lo(boldness) * 0.9,
            Self::Shy => all(&[lo(boldness), lo(social)]) * 1.25,
            Self::Nervous => all(&[lo(boldness), hi(energy)]) * 1.25,
            Self::Cowardly => (lo(boldness) - 0.45).max(0.0) * 1.6,
            Self::Playful => hi(playfulness),
            Self::Mischievous => all(&[hi(playfulness), hi(impulsiveness)]) * 1.2,
            Self::Silly => all(&[hi(playfulness), lo(feistiness)]) * 1.1,
            Self::Serious => lo(playfulness),
            Self::Stoic => all(&[lo(playfulness), lo(energy)]) * 1.1,
            Self::Grumpy => all(&[lo(playfulness), hi(feistiness)]) * 1.3,
            Self::Feisty => hi(feistiness) * 0.9,
            Self::Stubborn => all(&[hi(feistiness), lo(impulsiveness)]) * 1.3,
            Self::Bossy => all(&[hi(feistiness), hi(social), hi(boldness)]) * 1.4,
            Self::Irritable => all(&[hi(feistiness), lo(social)]) * 1.25,
            Self::Competitive => all(&[hi(feistiness), hi(energy)]) * 1.2,
            Self::Gentle => lo(feistiness),
            Self::Sweet => all(&[lo(feistiness), hi(affection)]) * 1.15,
            Self::Easygoing => all(&[lo(feistiness), lo(impulsiveness)]) * 1.1,
            Self::Impulsive => hi(impulsiveness),
            Self::Impatient => all(&[hi(impulsiveness), hi(energy)]) * 1.15,
            Self::Distractible => all(&[hi(impulsiveness), hi(curiosity)]) * 1.2,
            Self::Spontaneous => all(&[hi(impulsiveness), hi(playfulness)]) * 1.05,
            Self::Patient => lo(impulsiveness),
            Self::Steady => all(&[lo(impulsiveness), lo(energy)]) * 1.1,
            Self::Particular => all(&[lo(impulsiveness), lo(playfulness)]) * 1.05,
            Self::Picky => all(&[lo(impulsiveness), hi(feistiness)]) * 1.25,
            Self::Suspicious => hi(suspicion),
            Self::Wary => all(&[hi(suspicion), lo(boldness)]) * 1.15,
            Self::Watchful => all(&[hi(suspicion), hi(boldness)]) * 1.15,
            Self::Trusting => lo(suspicion),
            Self::Friendly => all(&[lo(suspicion), hi(social)]) * 1.1,
            Self::Affectionate => hi(affection),
            Self::Loyal => all(&[hi(affection), lo(impulsiveness)]) * 1.15,
            Self::Jealous => all(&[hi(affection), hi(feistiness)]) * 1.3,
            Self::Protective => all(&[hi(affection), hi(boldness), hi(suspicion)]) * 1.4,
            Self::Nurturing => all(&[hi(affection), lo(feistiness), hi(social)]) * 1.3,
            Self::Independent => lo(affection),
            Self::Aloof => all(&[lo(affection), lo(social)]) * 1.2,
            Self::Curious => hi(curiosity),
            Self::Nosy => all(&[hi(curiosity), hi(social), lo(suspicion)]) * 1.35,
            Self::Obsessive => all(&[hi(curiosity), lo(impulsiveness), lo(playfulness)]) * 1.3,
            Self::Observant => all(&[hi(curiosity), lo(energy)]) * 1.15,
            Self::Unbothered => all(&[lo(curiosity), lo(suspicion)]) * 1.1,
            Self::Dramatic => all(&[hi(energy), hi(social), hi(feistiness).max(0.3)]) * 1.2,
            Self::Vain => all(&[hi(social), hi(boldness), lo(affection)]) * 1.35,
            Self::AttentionSeeking => all(&[hi(social), hi(affection), hi(energy)]) * 1.3,
            Self::FoodMotivated => lo(energy) * 0.85,
            Self::Messy => all(&[hi(impulsiveness), hi(playfulness)]) * 0.95,
            Self::Eccentric => ((a.extremeness() - 0.55) * 2.6).max(0.0),
            Self::Unpredictable => {
                all(&[hi(impulsiveness), (a.extremeness() - 0.45).max(0.0) * 3.0])
            }
            Self::NightOwl => hi(sleep) * 0.7,
            Self::EarlyRiser => lo(sleep) * 0.7,
            Self::EasilyEmbarrassed | Self::SecretlyAffectionate | Self::DeeplyLoyal => 0.0,
        }
    }

    /// Whether a companion can be both this and `other` without a tension to explain it.
    fn contradicts(self, other: Self) -> bool {
        self.leans().iter().any(|(axis, sign)| {
            other
                .leans()
                .iter()
                .any(|(other_axis, other_sign)| axis == other_axis && sign != other_sign)
        })
    }

    /// Whether the word reads naturally in "a … guardian".
    const fn is_adjective(self) -> bool {
        !matches!(self, Self::Loner | Self::NightOwl | Self::EarlyRiser)
    }
}

/// A companion's temperament: the kind it started from, where it sits on the nine axes, and the
/// tension it carries, if any.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Temperament {
    pub kind: TemperamentKind,
    pub axes: Axes,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tension: Option<Tension>,
}

/// How the named stream a new companion's temperament is drawn from is labelled. Changing what
/// it draws changes every companion made from then on, so it gets a new label instead.
const STREAM: &str = "temperament-v1";

/// Below this a trait is not shown at all.
const SHOWN: f32 = 0.2;

impl Temperament {
    /// A new companion's temperament and the behaviour values drawn from it. A mini starts from
    /// its parent: it keeps the parent's kind a little over half the time, and wherever it lands
    /// it is pulled a third of the way back toward the parent's axes.
    pub fn generated(
        seed: [u8; 32],
        generation: u8,
        parent: Option<&Temperament>,
    ) -> (Self, PersonalityGenome) {
        let mut rng = SeedStream::new(seed).rng(STREAM, u64::from(generation));
        let kind = match parent {
            Some(parent) if rng.random_bool(0.55) => parent.kind,
            _ => {
                let total: u32 = TemperamentKind::ALL.iter().map(|kind| kind.weight()).sum();
                let mut roll = rng.random_range(0..total);
                *TemperamentKind::ALL
                    .iter()
                    .find(|kind| {
                        let hit = roll < kind.weight();
                        roll = roll.saturating_sub(kind.weight());
                        hit
                    })
                    .expect("the weights cover the roll")
            }
        };
        let mut axes = Self::draw_axes(kind, &mut rng);
        if let Some(parent) = parent {
            axes = axes.map(|index, value| value + (parent.axes.to_array()[index] - value) * 0.35);
        }
        let (chance, suited) = kind.tensions();
        let inherited = parent
            .and_then(|parent| parent.tension)
            .filter(|_| rng.random_bool(0.4));
        let tension = inherited.or_else(|| {
            rng.random_bool(chance)
                .then(|| suited[rng.random_range(0..suited.len())])
        });
        if let Some(tension) = tension {
            axes = axes.map(|index, value| {
                tension
                    .holds()
                    .iter()
                    .find(|(axis, ..)| *axis == index)
                    .map_or(value, |(_, low, high)| value.clamp(*low, *high))
            });
        }
        let axes = axes.bounded();
        // A kind is where a companion starts. Now and then its axes wander far enough from it
        // that another kind fits them clearly better, and then it is that kind: a wallflower that
        // came out brave and energetic is not called a wallflower. An oddball stays an oddball.
        let fits = |kind: TemperamentKind| Self::fit(kind, axes);
        let best = Self::closest_kind(axes);
        let kind = if kind != TemperamentKind::Oddball
            && best != TemperamentKind::Oddball
            && fits(best) > fits(kind) + 0.2
        {
            best
        } else {
            kind
        };
        let temperament = Self {
            kind,
            axes,
            tension,
        };
        let sleep_timing = rng.random_range(0.2..0.9);
        (temperament, temperament.genome(sleep_timing))
    }

    fn draw_axes(kind: TemperamentKind, rng: &mut impl Rng) -> Axes {
        let centre = kind.centre();
        let mut values = [0.0; 9];
        for (index, value) in values.iter_mut().enumerate() {
            *value = if kind == TemperamentKind::Oddball {
                // An oddball is out near an end of most scales, which end it is being anyone's
                // guess, and it is rarely patient about it.
                if index == 6 {
                    rng.random_range(0.55..0.98)
                } else if rng.random_bool(0.7) {
                    if rng.random_bool(0.5) {
                        rng.random_range(0.02..0.24)
                    } else {
                        rng.random_range(0.76..0.98)
                    }
                } else {
                    rng.random_range(0.0..1.0)
                }
            } else if rng.random_bool(0.1) {
                // Now and then one axis ignores the kind altogether, so a kind is a place to
                // start rather than a package.
                rng.random_range(0.0..1.0)
            } else {
                // Roughly bell-shaped around the kind's middle.
                let spread: f32 = (0..3).map(|_| rng.random_range(-1.0..1.0_f32)).sum::<f32>();
                centre[index] + spread * 0.085
            };
        }
        Axes::from_array(values).bounded()
    }

    /// The temperament of a companion made before temperaments existed, read from the values it
    /// already has. Nothing is drawn: feistiness, impulsiveness, suspicion and affection read as
    /// middling, and its kind is whichever kind its own values fit best.
    pub fn read(genome: &PersonalityGenome) -> Self {
        let unit = |value: f32, low: f32, high: f32| ((value - low) / (high - low)).clamp(0.0, 1.0);
        let axes = Axes {
            social: unit(genome.sociability, 0.25, 0.95),
            energy: unit(genome.activity, 0.2, 0.95),
            boldness: unit(genome.boldness, 0.1, 0.95),
            playfulness: unit(genome.playfulness, 0.15, 0.95),
            curiosity: unit(genome.curiosity, 0.15, 0.95),
            ..Axes::MIDDLING
        }
        .bounded();
        Self {
            kind: Self::closest_kind(axes),
            axes,
            tension: None,
        }
    }

    /// The kind a set of axes fits best: how far, on average, they lean the way each kind's own
    /// axes do. An axis at its middle says nothing either way, so a kind that rests on sides a
    /// companion never had is rarely the best fit for it. An oddball is out near the ends of
    /// everything.
    pub fn closest_kind(axes: Axes) -> TemperamentKind {
        TemperamentKind::ALL
            .into_iter()
            .max_by(|a, b| Self::fit(*a, axes).total_cmp(&Self::fit(*b, axes)))
            .expect("there are kinds")
    }

    /// How well a set of axes fits a kind, from -1 to 1.
    fn fit(kind: TemperamentKind, axes: Axes) -> f32 {
        if kind == TemperamentKind::Oddball {
            return (axes.extremeness() - 0.62) * 2.0;
        }
        let values = axes.to_array();
        let signature = kind.signature();
        signature
            .iter()
            .map(|(axis, direction)| (values[*axis] - 0.5) * 2.0 * direction)
            .sum::<f32>()
            / signature.len() as f32
    }

    /// The behaviour values the rest of the simulation reads, drawn from the axes over the same
    /// ranges companions have always had, so everything tuned against them still holds.
    pub fn genome(&self, sleep_timing: f32) -> PersonalityGenome {
        let a = self.axes.bounded();
        let lerp = |low: f32, high: f32, t: f32| low + (high - low) * t.clamp(0.0, 1.0);
        PersonalityGenome {
            activity: lerp(0.2, 0.95, a.energy),
            curiosity: lerp(0.15, 0.95, a.curiosity),
            boldness: lerp(0.1, 0.95, a.boldness),
            playfulness: lerp(0.15, 0.95, a.playfulness),
            sociability: lerp(0.25, 0.95, a.social),
            routine_affinity: lerp(0.1, 0.9, 1.0 - a.impulsiveness),
            sleep_timing: sleep_timing.clamp(0.2, 0.9),
            window_tolerance: lerp(0.1, 0.95, a.boldness * 0.6 + (1.0 - a.suspicion) * 0.4),
            cursor_interest: lerp(
                0.1,
                0.95,
                a.curiosity * 0.4 + a.affection * 0.35 + (1.0 - a.suspicion) * 0.25,
            ),
            decision_temperature: lerp(0.22, 0.75, a.impulsiveness),
        }
    }

    /// The three traits the profile shows, strongest first: a tension's two, then whatever else
    /// stands out most. At most one comes from each side of a companion, none contradicts another,
    /// at most two are flaws, and a kind's own traits come a little more readily.
    pub fn traits(&self, sleep_timing: f32) -> [Trait; 3] {
        let axes = self.axes.bounded();
        let pool = self.kind.pool();
        let mut ranked: Vec<(Trait, f32)> = Trait::ALL
            .into_iter()
            .map(|t| {
                let bonus = if pool.contains(&t) { 0.16 } else { 0.0 };
                // A word about one end of one scale is the easiest to earn, so it is held back a
                // little, and a flaw is let forward a little: otherwise the plainest words fill
                // every profile and nobody is ever stubborn.
                let plain = if t.leans().len() == 1 { 0.85 } else { 1.0 };
                let flaw = if t.valence() == Valence::Flaw {
                    1.12
                } else {
                    1.0
                };
                let strength = t.strength(axes, sleep_timing) * plain * flaw;
                (
                    t,
                    if strength > 0.0 {
                        strength + bonus
                    } else {
                        0.0
                    },
                )
            })
            .collect();
        // Stable order for equal strengths: the catalogue's own.
        ranked.sort_by(|a, b| b.1.total_cmp(&a.1));
        let mut chosen: Vec<Trait> = self
            .tension
            .map(|tension| tension.traits().to_vec())
            .unwrap_or_default();
        let fits = |chosen: &[Trait], candidate: Trait| {
            let flaws = chosen
                .iter()
                .filter(|t| t.valence() == Valence::Flaw)
                .count();
            !chosen.contains(&candidate)
                && (candidate.family() == Family::Flavour
                    || chosen.iter().all(|t| t.family() != candidate.family()))
                && chosen.iter().all(|t| !t.contradicts(candidate))
                && !(candidate.valence() == Valence::Flaw && flaws >= 2)
        };
        for (candidate, strength) in &ranked {
            if chosen.len() == 3 {
                break;
            }
            if *strength >= SHOWN && fits(&chosen, *candidate) {
                chosen.push(*candidate);
            }
        }
        // A thoroughly middling companion still reads as somebody: the plainest words that fit,
        // and never one that leans against its own kind — no easygoing grouch.
        let signature = self.kind.signature();
        let suits_kind = |t: Trait| {
            t.leans().iter().all(|(axis, sign)| {
                signature.iter().all(|(kind_axis, direction)| {
                    kind_axis != axis || *direction * f32::from(*sign) > 0.0
                })
            })
        };
        for fallback in [
            Trait::Easygoing,
            Trait::Steady,
            Trait::Friendly,
            Trait::Observant,
            Trait::Patient,
            Trait::Unbothered,
        ] {
            if chosen.len() == 3 {
                break;
            }
            if fits(&chosen, fallback) && suits_kind(fallback) {
                chosen.push(fallback);
            }
        }
        // Three is always reachable: the fallbacks alone contradict nothing each other says.
        while chosen.len() < 3 {
            chosen.push(Trait::Easygoing);
        }
        // At least one warm or plain word, however prickly the rest.
        if chosen.iter().all(|t| t.valence() == Valence::Flaw)
            && let Some(warm) = ranked.iter().find(|(t, s)| {
                *s > 0.0 && t.valence() != Valence::Flaw && {
                    let others: Vec<_> = chosen[..2].to_vec();
                    fits(&others, *t)
                }
            })
        {
            chosen[2] = warm.0;
        }
        [chosen[0], chosen[1], chosen[2]]
    }

    /// "A suspicious guardian", "An excitable little troublemaker": the kind's name for the
    /// companion with the first of its traits that reads as a describing word and says something
    /// the name does not already.
    pub fn phrase(&self, traits: [Trait; 3], mini: bool) -> String {
        let nouns: Vec<&str> = self
            .kind
            .nouns()
            .iter()
            .filter(|(_, suits)| suits.is_empty() || suits.iter().any(|t| traits.contains(t)))
            .map(|(noun, _)| *noun)
            .collect();
        // Chosen from the axes, which never change, so a companion keeps its name for good.
        let pick = self.axes.to_array().iter().fold(0_u32, |hash, value| {
            hash.wrapping_mul(31).wrapping_add(value.to_bits() >> 8)
        });
        let noun = nouns[pick as usize % nouns.len()];
        let echoes = |t: Trait| {
            matches!(
                (self.kind, t),
                (TemperamentKind::Grump, Trait::Grumpy | Trait::Irritable)
                    | (TemperamentKind::Lazybones, Trait::Lazy | Trait::Sleepy)
                    | (TemperamentKind::Sweetheart, Trait::Sweet)
                    | (TemperamentKind::Troublemaker, Trait::Mischievous)
                    | (
                        TemperamentKind::Guardian,
                        Trait::Protective | Trait::Watchful
                    )
                    | (TemperamentKind::Oddball, Trait::Eccentric)
                    | (TemperamentKind::Showoff, Trait::AttentionSeeking)
                    | (TemperamentKind::Explorer, Trait::Restless)
            ) || (noun == "glutton" && t == Trait::FoodMotivated)
                || (noun == "wanderer" && t == Trait::Curious)
        };
        let adjective = traits
            .into_iter()
            .find(|t| t.is_adjective() && !echoes(*t))
            .map(|t| t.label().to_lowercase());
        let words = [adjective.as_deref(), mini.then_some("little"), Some(noun)]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" ");
        let article = if words.starts_with(['a', 'e', 'i', 'o', 'u']) {
            "An"
        } else {
            "A"
        };
        format!("{article} {words}")
    }
}

impl crate::Creature {
    /// Which generator made it: the current one for anyone given a temperament, the original one
    /// for everyone older and for the minis their families go on having.
    pub fn edition(&self) -> crate::Edition {
        if self.temperament.is_some() {
            crate::Edition::Archetypes
        } else {
            crate::Edition::Original
        }
    }

    /// Its temperament: the one it was given, or for a companion made before temperaments
    /// existed, the one its own values read as.
    pub fn temperament(&self) -> Temperament {
        self.temperament
            .unwrap_or_else(|| Temperament::read(&self.personality))
    }

    /// The three traits its profile shows.
    pub fn traits(&self) -> [Trait; 3] {
        self.temperament().traits(self.personality.sleep_timing)
    }

    /// The line its profile leads with: "A suspicious guardian".
    pub fn temperament_phrase(&self) -> String {
        let temperament = self.temperament();
        temperament.phrase(
            temperament.traits(self.personality.sleep_timing),
            self.role.parent_id().is_some(),
        )
    }

    /// Whether it carries a trait, among the three its profile shows.
    pub fn shows(&self, t: Trait) -> bool {
        self.traits().contains(&t)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn population(count: u64) -> Vec<(Temperament, PersonalityGenome)> {
        (0..count)
            .map(|index| {
                let seed = SeedStream::new([29; 32]).bytes("temperament-population", index);
                Temperament::generated(seed, 0, None)
            })
            .collect()
    }

    /// The old generator, exactly as `generate_creature` has always drawn personality.
    fn legacy_genome(seed: [u8; 32]) -> PersonalityGenome {
        let mut rng = SeedStream::new(seed).rng("personality", 0);
        PersonalityGenome {
            activity: rng.random_range(0.2..0.95),
            curiosity: rng.random_range(0.15..0.95),
            boldness: rng.random_range(0.1..0.95),
            playfulness: rng.random_range(0.15..0.95),
            sociability: rng.random_range(0.25..0.95),
            routine_affinity: rng.random_range(0.1..0.9),
            sleep_timing: rng.random_range(0.2..0.9),
            window_tolerance: rng.random_range(0.1..0.95),
            cursor_interest: rng.random_range(0.1..0.95),
            decision_temperature: rng.random_range(0.22..0.75),
        }
    }

    #[test]
    fn a_temperament_is_a_pure_function_of_its_seed() {
        for index in 0..64 {
            let seed = SeedStream::new([3; 32]).bytes("temperament-determinism", index);
            let (a, genome_a) = Temperament::generated(seed, 0, None);
            let (b, genome_b) = Temperament::generated(seed, 0, None);
            assert_eq!(a, b);
            assert_eq!(genome_a, genome_b);
            assert_eq!(
                a.traits(genome_a.sleep_timing),
                b.traits(genome_b.sleep_timing)
            );
            let json = serde_json::to_string(&a).unwrap();
            assert_eq!(serde_json::from_str::<Temperament>(&json).unwrap(), a);
        }
    }

    const GOLDEN_TEMPERAMENTS: [&str; 4] = [
        "Scholar [\"Curious\", \"Lazy\", \"Particular\"] A curious professor | 0.576 0.222 0.457 0.321 0.979 0.426 0.329 0.402 0.674 | None",
        "Troublemaker [\"Reckless\", \"Mischievous\", \"Excitable\"] A reckless scamp | 0.684 0.880 0.837 0.808 0.454 0.658 0.767 0.393 0.563 | None",
        "Showoff [\"Confident\", \"Vain\", \"Outgoing\"] A confident star | 0.916 0.756 0.830 0.571 0.387 0.506 0.494 0.272 0.290 | None",
        "Explorer [\"Brave\", \"Energetic\", \"Patient\"] A brave explorer | 0.223 0.898 0.996 0.255 0.661 0.290 0.209 0.632 0.686 | None",
    ];

    /// Temperaments exactly as 0.62.0 draws them. A change here changes every companion made from
    /// then on, and every one a friend's code brings; it needs a new stream label, not an edit.
    #[test]
    fn the_first_temperaments_are_drawn_exactly_as_they_always_were() {
        let seen: Vec<String> = [[23_u8; 32], [55; 32], [61; 32], [7; 32]]
            .into_iter()
            .map(|seed| {
                let (temperament, genome) = Temperament::generated(seed, 0, None);
                let traits = temperament.traits(genome.sleep_timing);
                let axes = temperament
                    .axes
                    .to_array()
                    .map(|value| format!("{value:.3}"))
                    .join(" ");
                format!(
                    "{:?} {:?} {} | {axes} | {:?}",
                    temperament.kind,
                    traits.map(Trait::label),
                    temperament.phrase(traits, false),
                    temperament.tension
                )
            })
            .collect();
        assert_eq!(seen, GOLDEN_TEMPERAMENTS);
    }

    #[test]
    fn behaviour_values_stay_in_the_ranges_companions_have_always_had() {
        for (_, genome) in population(2_000) {
            assert!((0.2..=0.95).contains(&genome.activity), "{genome:?}");
            assert!((0.15..=0.95).contains(&genome.curiosity), "{genome:?}");
            assert!((0.1..=0.95).contains(&genome.boldness), "{genome:?}");
            assert!((0.15..=0.95).contains(&genome.playfulness), "{genome:?}");
            assert!((0.25..=0.95).contains(&genome.sociability), "{genome:?}");
            assert!((0.1..=0.9).contains(&genome.routine_affinity), "{genome:?}");
            assert!((0.2..=0.9).contains(&genome.sleep_timing), "{genome:?}");
            assert!(
                (0.1..=0.95).contains(&genome.window_tolerance),
                "{genome:?}"
            );
            assert!((0.1..=0.95).contains(&genome.cursor_interest), "{genome:?}");
            assert!(
                (0.22..=0.75).contains(&genome.decision_temperature),
                "{genome:?}"
            );
        }
    }

    #[test]
    fn an_older_companion_reads_its_own_values_back_and_nothing_else() {
        let genome = legacy_genome([41; 32]);
        let read = Temperament::read(&genome);
        assert_eq!(read, Temperament::read(&genome));
        assert_eq!(read.tension, None);
        assert_eq!(read.axes.feistiness, 0.5);
        assert_eq!(read.axes.impulsiveness, 0.5);
        assert_eq!(read.axes.suspicion, 0.5);
        assert_eq!(read.axes.affection, 0.5);
        // The five it had come back out of the values they came from.
        let back = read.genome(genome.sleep_timing);
        for (a, b) in [
            (back.activity, genome.activity),
            (back.curiosity, genome.curiosity),
            (back.boldness, genome.boldness),
            (back.playfulness, genome.playfulness),
            (back.sociability, genome.sociability),
        ] {
            assert!((a - b).abs() < 1e-5, "{a} {b}");
        }
        // A new companion's temperament survives the same trip for the axes a genome carries.
        let (temperament, genome) = Temperament::generated([8; 32], 0, None);
        let read = Temperament::read(&genome);
        for (a, b) in [
            (read.axes.social, temperament.axes.social),
            (read.axes.energy, temperament.axes.energy),
            (read.axes.boldness, temperament.axes.boldness),
            (read.axes.playfulness, temperament.axes.playfulness),
            (read.axes.curiosity, temperament.axes.curiosity),
        ] {
            assert!((a - b).abs() < 1e-5, "{a} {b}");
        }
    }

    #[test]
    fn traits_never_contradict_repeat_or_pile_up_flaws() {
        for (temperament, genome) in population(3_000) {
            let traits = temperament.traits(genome.sleep_timing);
            for (index, a) in traits.iter().enumerate() {
                for b in &traits[index + 1..] {
                    assert_ne!(a, b, "{traits:?}");
                    let tension_pair = temperament
                        .tension
                        .is_some_and(|t| t.traits().contains(a) && t.traits().contains(b));
                    if !tension_pair {
                        assert!(!a.contradicts(*b), "{traits:?} {temperament:?}");
                        if a.family() != Family::Flavour {
                            assert_ne!(a.family(), b.family(), "{traits:?}");
                        }
                    }
                }
            }
            assert!(
                traits.iter().any(|t| t.valence() != Valence::Flaw),
                "{traits:?}"
            );
            if let Some(tension) = temperament.tension {
                assert_eq!(traits[..2], tension.traits(), "{temperament:?}");
            }
        }
    }

    #[test]
    fn a_mini_takes_after_its_parent_without_being_a_copy() {
        let mut same_kind = 0;
        let mut identical = 0;
        let count = 400;
        for index in 0..count {
            let seed = SeedStream::new([13; 32]).bytes("temperament-minis", index);
            let (parent, _) = Temperament::generated(seed, 0, None);
            let (mini, _) = Temperament::generated(seed, 1, Some(&parent));
            same_kind += usize::from(mini.kind == parent.kind);
            identical += usize::from(mini == parent);
        }
        assert_eq!(identical, 0);
        assert!(
            (count as usize * 50 / 100..count as usize * 75 / 100).contains(&same_kind),
            "{same_kind} of {count} minis kept their parent's kind"
        );
    }

    #[test]
    fn phrases_read_as_plain_english() {
        for (temperament, genome) in population(1_000) {
            for mini in [false, true] {
                let traits = temperament.traits(genome.sleep_timing);
                let phrase = temperament.phrase(traits, mini);
                assert!(
                    phrase.starts_with("A ") || phrase.starts_with("An "),
                    "{phrase}"
                );
                let second = phrase.split(' ').nth(1).unwrap();
                let vowel = second.starts_with(['a', 'e', 'i', 'o', 'u']);
                assert_eq!(phrase.starts_with("An "), vowel, "{phrase}");
                assert_eq!(phrase.contains(" little "), mini, "{phrase}");
                assert!(!phrase.contains("  "), "{phrase}");
            }
        }
    }

    /// The audit the 0.62.0 brief asked for: a thousand-odd companions from each generator, their
    /// kinds and traits counted. Run with `--nocapture` to see the tables.
    #[test]
    fn personalities_spread_across_kinds_and_traits_rather_than_converging() {
        let count = 2_000;
        let report = |name: &str, temperaments: &[(Temperament, f32)]| {
            let mut kinds: HashMap<TemperamentKind, usize> = HashMap::new();
            let mut traits: HashMap<Trait, usize> = HashMap::new();
            let mut flaws = 0;
            let mut tensions = 0;
            for (temperament, sleep) in temperaments {
                *kinds.entry(temperament.kind).or_default() += 1;
                tensions += usize::from(temperament.tension.is_some());
                for t in temperament.traits(*sleep) {
                    *traits.entry(t).or_default() += 1;
                    flaws += usize::from(t.valence() == Valence::Flaw);
                }
            }
            let mut kinds: Vec<_> = kinds.into_iter().collect();
            kinds.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
            let mut traits: Vec<_> = traits.into_iter().collect();
            traits.sort_by_key(|(t, n)| (std::cmp::Reverse(*n), t.label()));
            let total = temperaments.len() as f32;
            eprintln!("\n{name}: {} companions", temperaments.len());
            for (kind, n) in &kinds {
                eprintln!("  {:<13}{:>5.1}%", kind.label(), *n as f32 / total * 100.0);
            }
            eprintln!(
                "  flaws {:.1}% of traits shown, tensions {:.1}% of companions",
                flaws as f32 / (total * 3.0) * 100.0,
                tensions as f32 / total * 100.0
            );
            for (t, n) in traits.iter().take(20) {
                eprintln!("  {:<22}{:>5.1}%", t.label(), *n as f32 / total * 100.0);
            }
            (
                kinds,
                traits,
                flaws as f32 / (total * 3.0),
                tensions as f32 / total,
            )
        };

        let new: Vec<_> = population(count)
            .into_iter()
            .map(|(temperament, genome)| (temperament, genome.sleep_timing))
            .collect();
        let (kinds, traits, flaw_share, tension_share) = report("new companions", &new);
        assert_eq!(kinds.len(), TemperamentKind::ALL.len());
        for (kind, n) in &kinds {
            let share = *n as f32 / count as f32;
            assert!((0.04..0.16).contains(&share), "{kind:?} {share}");
        }
        // No trait is on more than one companion in five, and the three commonest together fill
        // well under a quarter of the words shown.
        let slots = count as f32 * 3.0;
        assert!(traits[0].1 as f32 / count as f32 <= 0.2, "{:?}", traits[0]);
        let top_three: usize = traits.iter().take(3).map(|(_, n)| n).sum();
        assert!(top_three as f32 / slots <= 0.2, "{top_three}");
        assert!(
            traits.len() >= 50,
            "only {} traits ever shown",
            traits.len()
        );
        // Likable does not have to mean nice: a good share of what shows is a flaw, but never
        // most of it, and the nice words are not what everyone gets.
        assert!((0.25..0.45).contains(&flaw_share), "flaws {flaw_share}");
        assert!(
            (0.15..0.35).contains(&tension_share),
            "tensions {tension_share}"
        );
        for nice in [
            Trait::Gentle,
            Trait::Playful,
            Trait::Sweet,
            Trait::Affectionate,
        ] {
            let n = traits
                .iter()
                .find(|(t, _)| *t == nice)
                .map_or(0, |(_, n)| *n);
            assert!(n as f32 / (count as f32) < 0.12, "{nice:?} on {n}");
        }

        // Older companions read from what they already have spread out too.
        let old: Vec<_> = (0..count)
            .map(|index| {
                let genome =
                    legacy_genome(SeedStream::new([31; 32]).bytes("legacy-population", index));
                (Temperament::read(&genome), genome.sleep_timing)
            })
            .collect();
        let (kinds, traits, _, tension_share) = report("older companions, read", &old);
        assert_eq!(tension_share, 0.0);
        assert!(kinds.len() >= 7, "{kinds:?}");
        assert!(kinds[0].1 as f32 / (count as f32) < 0.3, "{:?}", kinds[0]);
        assert!(traits[0].1 as f32 / (count as f32) < 0.3, "{:?}", traits[0]);
    }
}
