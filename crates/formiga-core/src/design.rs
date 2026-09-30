//! Small, bounded construction recipes. Source images never become runtime geometry or textures.
use crate::{Creature, SeedStream};
use rand::Rng;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BodyPlan {
    Round,
    Upright,
    Long,
    Winged,
    /// The original one-piece silhouette: no separate head, a face carried high on a soft
    /// rounded mass, and stubby feet. Kept as a first-class plan because the shape reads as
    /// the friendliest of the set. Its index stays last so earlier recipes decode unchanged.
    Blob,
}

impl BodyPlan {
    pub const ALL: [Self; 5] = [
        Self::Round,
        Self::Upright,
        Self::Long,
        Self::Winged,
        Self::Blob,
    ];
    pub const fn label(self) -> &'static str {
        match self {
            Self::Round => "Round companion",
            Self::Upright => "Upright companion",
            Self::Long => "Four-pawed companion",
            Self::Winged => "Winged companion",
            Self::Blob => "Blob companion",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EarStyle {
    None,
    Round,
    Pointed,
    Long,
    Floppy,
    Tuft,
}

impl EarStyle {
    pub const ALL: [Self; 6] = [
        Self::None,
        Self::Round,
        Self::Pointed,
        Self::Long,
        Self::Floppy,
        Self::Tuft,
    ];
}

/// Parts carried over from the original one-piece companions, each chosen on its own, so a
/// recipe can sit anywhere between the tidy modular look and the older scrappy one. Every part at
/// zero is a plain modular recipe, which is what every recipe written before these parts existed
/// reads as, so none of those changes appearance.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ClassicParts {
    /// 0 soft recipe colours inside a full outline; 1 candy colours, a top edge left in the light
    /// over a heavier line underneath, and a crescent of shade.
    pub coat: u8,
    /// 0 the large modular eyes; 1 a mask, 2 a visor, 3 beads, 4 tall eyes, 5 square eyes.
    pub face: u8,
    /// 0 coat-coloured paws and feet; 1 accent nubs; 2 stick legs on forked accent feet.
    pub limbs: u8,
    /// 0 the recipe's ears; 1 antennae; 2 sprouts.
    pub crown: u8,
    /// 0 the recipe's marking; 1 stripes, 2 spots, 3 patches.
    pub pattern: u8,
    /// 0 the recipe's tail; 1 a curl, 2 a star on a stalk.
    pub tail: u8,
}

impl ClassicParts {
    /// The largest value each part takes, in field order.
    const LIMITS: [u8; 6] = [1, 5, 2, 2, 3, 2];

    pub fn is_modular(&self) -> bool {
        *self == Self::default()
    }

    pub fn bounded(self) -> Self {
        let [coat, face, limbs, crown, pattern, tail] = Self::LIMITS;
        Self {
            coat: self.coat.min(coat),
            face: self.face.min(face),
            limbs: self.limbs.min(limbs),
            crown: self.crown.min(crown),
            pattern: self.pattern.min(pattern),
            tail: self.tail.min(tail),
        }
    }

    /// Two parts to a byte, in the four bytes a version 2 code has always reserved after the
    /// recipe. The last byte stays zero.
    pub fn to_bytes(self) -> [u8; 4] {
        let p = self.bounded();
        [
            p.coat | (p.limbs << 4),
            p.face | (p.crown << 4),
            p.pattern | (p.tail << 4),
            0,
        ]
    }

    pub fn from_bytes(bytes: [u8; 4]) -> Option<Self> {
        let parts = Self {
            coat: bytes[0] & 0x0f,
            limbs: bytes[0] >> 4,
            face: bytes[1] & 0x0f,
            crown: bytes[1] >> 4,
            pattern: bytes[2] & 0x0f,
            tail: bytes[2] >> 4,
        };
        (bytes[3] == 0 && parts == parts.bounded()).then_some(parts)
    }

    /// How far a new companion leans toward the original look is drawn first: a quarter lean
    /// wholly modular, a quarter wholly classic, and the half between take each part on its own
    /// at that lean. Colours, the face, and the limbs follow the lean itself; the crown, pattern,
    /// and tail are extras even a wholly classic companion only sometimes has.
    fn generated(rng: &mut impl Rng) -> Self {
        let lean = [0.0, 0.35, 0.65, 1.0][rng.random_range(0..4)];
        let [coat, face, limbs, crown, pattern, tail] = Self::LIMITS;
        let mut take = |share: f64, limit: u8| {
            if rng.random_bool(share) {
                rng.random_range(1..=limit)
            } else {
                0
            }
        };
        Self {
            coat: take(lean, coat),
            face: take(lean, face),
            limbs: take(lean, limbs),
            crown: take(lean * 0.6, crown),
            pattern: take(lean * 0.7, pattern),
            tail: take(lean * 0.5, tail),
        }
    }

    /// A mini keeps its parent's colouring, face, and build, and usually its crown, pattern, and
    /// tail, the same way it usually keeps the parent's ears.
    fn inherited(self, rng: &mut impl Rng) -> Self {
        let [_, _, _, crown, pattern, tail] = Self::LIMITS;
        let mut keep = |parent: u8, share: f64, limit: u8| {
            if rng.random_bool(share) {
                parent
            } else {
                rng.random_range(0..=limit)
            }
        };
        Self {
            crown: keep(self.crown, 0.8, crown),
            pattern: keep(self.pattern, 0.6, pattern),
            tail: keep(self.tail, 0.75, tail),
            ..self
        }
    }
}

/// Coat and accent pairs from the original palettes. A new companion wearing candy colours starts
/// from one of these, nudged a little, so it keeps their contrast without repeating them exactly.
const CANDY: [([u8; 3], [u8; 3]); 12] = [
    ([0xe9, 0x8a, 0xb5], [0xff, 0xe0, 0x81]),
    ([0x75, 0xc9, 0xc8], [0xff, 0xcf, 0x70]),
    ([0xf2, 0xa6, 0x5a], [0x7d, 0xcf, 0xb6]),
    ([0x9b, 0x8d, 0xe3], [0xff, 0x9f, 0x9f]),
    ([0xa7, 0xcb, 0x65], [0xf3, 0xa8, 0x6b]),
    ([0xe8, 0x83, 0x7b], [0x7e, 0xc8, 0xe3]),
    ([0x65, 0xb5, 0xdf], [0xf5, 0xd7, 0x6e]),
    ([0xca, 0x78, 0xd1], [0x84, 0xd6, 0xb4]),
    ([0xe0, 0xc6, 0x5a], [0xef, 0x7d, 0x77]),
    ([0xaa, 0xb3, 0xb7], [0xf4, 0x9d, 0x6e]),
    ([0xda, 0x78, 0x94], [0x94, 0xd3, 0xac]),
    ([0x70, 0xc4, 0x9b], [0xbe, 0x87, 0xd9]),
];

/// Which generator made a companion. Whatever a generator draws from a seed it goes on drawing
/// exactly as it did, so a companion keeps its looks and its temperament and a friend's code brings
/// back the same companion; a new way of generating is a new edition, never a change to an old one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Edition {
    /// Before 0.62.0: every part and every personality value drawn on its own.
    Original,
    /// Since 0.62.0: a body archetype and an authored face for the recipe, and a temperament
    /// the personality values are drawn from.
    Archetypes,
}

impl Edition {
    /// What a companion made today comes from.
    pub const LATEST: Self = Self::Archetypes;
}

/// The number of authored face layouts a recipe can wear.
pub const FACE_TEMPLATES: u8 = 12;

/// The number of body archetypes a recipe can be drawn from.
pub const BODY_ARCHETYPES: u8 = 8;

fn is_zero(value: &u8) -> bool {
    *value == 0
}

/// The broad kind of body a recipe drawn since 0.62.0 starts from. An archetype is a set of odds
/// over the parts every recipe already has — which body plan, which ears and how big, which tail,
/// how it is marked and which face it wears — not a species: the parts are the same parts, only
/// drawn from odds that suit one another.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BodyArchetype {
    /// Round, with tiny limbs and simple ears.
    Mochi,
    /// Four legs, ears and a tail.
    Critter,
    /// Upright, with an oversized head and stubby limbs.
    Bean,
    /// Round and fluffy.
    Puff,
    /// Upright, crowned with antennae or sprouts.
    Sprite,
    /// Four legs and a tail with some presence to it.
    Whelp,
    /// Wings, a round body and tiny feet.
    Birb,
    /// A blob, and strange on purpose.
    Goober,
}

impl BodyArchetype {
    pub const ALL: [Self; BODY_ARCHETYPES as usize] = [
        Self::Mochi,
        Self::Critter,
        Self::Bean,
        Self::Puff,
        Self::Sprite,
        Self::Whelp,
        Self::Birb,
        Self::Goober,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Mochi => "Mochi",
            Self::Critter => "Critter",
            Self::Bean => "Bean",
            Self::Puff => "Puff",
            Self::Sprite => "Sprite",
            Self::Whelp => "Whelp",
            Self::Birb => "Birb",
            Self::Goober => "Goober",
        }
    }

    /// Its number in a recipe, from 1; 0 is a recipe drawn before archetypes existed.
    pub const fn number(self) -> u8 {
        self as u8 + 1
    }

    pub fn from_number(number: u8) -> Option<Self> {
        Self::ALL.get(usize::from(number).checked_sub(1)?).copied()
    }

    /// How often a new companion starts from this archetype, out of a hundred. Goobers are the
    /// rarest on purpose.
    const fn weight(self) -> f32 {
        match self {
            Self::Mochi => 16.0,
            Self::Critter | Self::Bean => 14.0,
            Self::Puff | Self::Sprite | Self::Whelp | Self::Birb => 12.0,
            Self::Goober => 8.0,
        }
    }

    /// Whether a recipe of this archetype can have this body plan.
    pub const fn allows(self, body: BodyPlan) -> bool {
        matches!(
            (self, body),
            (Self::Mochi | Self::Puff, BodyPlan::Round | BodyPlan::Blob)
                | (Self::Critter | Self::Whelp, BodyPlan::Long)
                | (Self::Bean, BodyPlan::Upright)
                | (Self::Sprite, BodyPlan::Upright | BodyPlan::Round)
                | (Self::Birb, BodyPlan::Winged)
                | (Self::Goober, BodyPlan::Blob | BodyPlan::Round)
        )
    }

    /// The archetype a body plan belongs to when the plan was chosen some other way, from a
    /// picture say. `lean` picks between the two a plan can belong to.
    pub const fn for_body(body: BodyPlan, lean: bool) -> Self {
        match body {
            BodyPlan::Round => {
                if lean {
                    Self::Puff
                } else {
                    Self::Mochi
                }
            }
            BodyPlan::Upright => {
                if lean {
                    Self::Sprite
                } else {
                    Self::Bean
                }
            }
            BodyPlan::Long => {
                if lean {
                    Self::Whelp
                } else {
                    Self::Critter
                }
            }
            BodyPlan::Winged => Self::Birb,
            BodyPlan::Blob => {
                if lean {
                    Self::Goober
                } else {
                    Self::Mochi
                }
            }
        }
    }

    /// Which faces suit it, and how often, as `(template, weight)`.
    const fn faces(self) -> &'static [(u8, f32)] {
        match self {
            Self::Mochi => &[(1, 3.0), (2, 3.0), (3, 3.0), (4, 1.0), (5, 2.0), (7, 1.0)],
            Self::Critter => &[(1, 3.0), (2, 2.0), (4, 2.0), (5, 2.0), (7, 2.0), (8, 1.0)],
            Self::Bean => &[(1, 3.0), (2, 2.0), (3, 2.0), (4, 2.0), (7, 2.0), (10, 0.5)],
            Self::Puff => &[(1, 2.0), (2, 3.0), (3, 3.0), (5, 2.0), (6, 1.0)],
            Self::Sprite => &[(1, 2.0), (4, 2.0), (7, 2.0), (8, 2.0), (9, 1.0), (10, 0.5)],
            Self::Whelp => &[(1, 3.0), (2, 2.0), (3, 2.0), (4, 2.0), (7, 1.0)],
            Self::Birb => &[(12, 5.0), (1, 1.0), (2, 1.0), (5, 1.0)],
            Self::Goober => &[
                (6, 2.0),
                (9, 2.0),
                (10, 2.0),
                (11, 2.0),
                (3, 1.0),
                (5, 1.0),
                (8, 1.0),
            ],
        }
    }
}

/// How the generator came to choose a recipe: one of the cutest of several, the cutest of several
/// that each break a rule of their archetype, or a genuine oddball.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Strangeness {
    Cute,
    WeirdCute,
    Oddball,
}

impl Strangeness {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Cute => "cute",
            Self::WeirdCute => "weird-cute",
            Self::Oddball => "oddball",
        }
    }
}

/// How many recipes a new companion's seed draws before the generator chooses one.
const CANDIDATES: u64 = 6;

/// One choice from weighted options. The weights need not add up to anything; a choice weighted
/// at nothing is never made unless every weight is nothing, when the last is.
fn weighted<T: Copy>(rng: &mut impl Rng, choices: &[(T, f32)]) -> T {
    let total: f32 = choices.iter().map(|(_, weight)| weight.max(0.0)).sum();
    if total > 0.0 {
        let mut roll = rng.random_range(0.0..total);
        for (value, weight) in choices {
            let weight = weight.max(0.0);
            if roll < weight {
                return *value;
            }
            roll -= weight;
        }
    }
    choices[choices.len() - 1].0
}

/// A colour from hue in degrees, saturation and lightness.
fn hsl(hue: f32, saturation: f32, lightness: f32) -> [u8; 3] {
    let hue = hue.rem_euclid(360.0) / 360.0;
    let (s, l) = (saturation.clamp(0.0, 1.0), lightness.clamp(0.0, 1.0));
    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let p = 2.0 * l - q;
    let channel = |t: f32| {
        let t = t.rem_euclid(1.0);
        let value = if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 0.5 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        };
        (value * 255.0).round().clamp(0.0, 255.0) as u8
    };
    [
        channel(hue + 1.0 / 3.0),
        channel(hue),
        channel(hue - 1.0 / 3.0),
    ]
}

/// Hue in degrees, saturation and lightness of a colour.
fn to_hsl(rgb: [u8; 3]) -> (f32, f32, f32) {
    let [r, g, b] = rgb.map(|channel| f32::from(channel) / 255.0);
    let (max, min) = (r.max(g).max(b), r.min(g).min(b));
    let lightness = (max + min) / 2.0;
    if max - min < 1e-6 {
        return (0.0, 0.0, lightness);
    }
    let delta = max - min;
    let saturation = if lightness > 0.5 {
        delta / (2.0 - max - min)
    } else {
        delta / (max + min)
    };
    let hue = if max == r {
        (g - b) / delta + if g < b { 6.0 } else { 0.0 }
    } else if max == g {
        (b - r) / delta + 2.0
    } else {
        (r - g) / delta + 4.0
    };
    (hue * 60.0, saturation, lightness)
}

/// How far apart two hues are around the wheel, from 0 to 180 degrees.
fn hue_gap(a: f32, b: f32) -> f32 {
    let gap = (a - b).rem_euclid(360.0);
    gap.min(360.0 - gap)
}

/// A soft coat, and an accent that belongs with it: near it around the colour wheel, across from
/// it, or a third of the way round.
fn harmonious(rng: &mut impl Rng) -> ([u8; 3], [u8; 3]) {
    let hue: f32 = rng.random_range(0.0..360.0);
    let saturation: f32 = rng.random_range(0.28..0.6);
    let lightness: f32 = rng.random_range(0.62..0.8);
    let side = if rng.random_bool(0.5) { 1.0 } else { -1.0 };
    let accent_hue = match weighted(rng, &[(0_u8, 5.0), (1, 3.5), (2, 1.5)]) {
        0 => hue + side * rng.random_range(25.0..50.0),
        1 => hue + 180.0 + rng.random_range(-20.0..20.0),
        _ => hue + side * 120.0,
    };
    let accent_saturation = (saturation + rng.random_range(0.05..0.25)).min(0.72);
    let accent_lightness = (lightness + rng.random_range(-0.12..0.06)).clamp(0.5, 0.85);
    (
        hsl(hue, saturation, lightness),
        hsl(accent_hue, accent_saturation, accent_lightness),
    )
}

/// A chance, with a share kept between never and always.
fn chance(rng: &mut impl Rng, share: f64) -> bool {
    rng.random_bool(share.clamp(0.0, 1.0))
}

/// How far a pair of ears reaches out of the outline, from nothing to about 1.4.
fn ear_weight(ears: EarStyle, size: u8) -> f32 {
    let shape = match ears {
        EarStyle::None => 0.0,
        EarStyle::Round => 0.4,
        EarStyle::Tuft => 0.5,
        EarStyle::Pointed => 0.7,
        EarStyle::Floppy => 0.8,
        EarStyle::Long => 1.0,
    };
    shape * f32::from(size) / 5.0
}

/// How much a tail adds to the outline, from nothing to 1.
fn tail_weight(tail: u8, classic: u8) -> f32 {
    if classic > 0 {
        return 0.9;
    }
    [0.0, 0.3, 0.6, 0.8, 1.0][usize::from(tail.min(4))]
}

/// How busy the markings across the body are, from nothing to 1.
fn marking_weight(marking: u8, pattern: u8) -> f32 {
    if pattern > 0 {
        return [0.0, 0.9, 0.8, 1.0][usize::from(pattern.min(3))];
    }
    match marking {
        0 => 0.0,
        1 => 0.2,
        6 => 0.3,
        5 => 0.4,
        4 => 0.5,
        _ => 0.7,
    }
}

/// Parts, proportions, and color: sixteen bytes of modular recipe, six classic parts, and since
/// 0.62.0 the archetype it was drawn from and the face it wears. Every combination has a connected
/// rounded body, a reserved large face, paired limbs, and bounded appendages inside the same 48px
/// atlas.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CreatureDesign {
    pub body: BodyPlan,
    pub ears: EarStyle,
    pub tail: u8,
    pub width: u8,
    pub height: u8,
    pub head: u8,
    pub ear_size: u8,
    pub legs: u8,
    pub muzzle: u8,
    pub marking: u8,
    pub coat: [u8; 3],
    pub accent: [u8; 3],
    /// Absent from every recipe saved before classic parts existed, which reads as all modular.
    #[serde(default, skip_serializing_if = "ClassicParts::is_modular")]
    pub classic: ClassicParts,
    /// The body archetype the recipe was drawn from, 1 to [`BODY_ARCHETYPES`], or 0 for a recipe
    /// drawn before archetypes existed. Absent from the file while it is 0.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub archetype: u8,
    /// The authored face layout it wears, 1 to [`FACE_TEMPLATES`], or 0 for the face every recipe
    /// before 0.62.0 wears. A recipe has both or neither. Absent from the file while it is 0.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub face_template: u8,
}

impl CreatureDesign {
    pub fn bounded(mut self) -> Self {
        self.tail = self.tail.min(4);
        self.width = self.width.clamp(8, 12);
        self.height = self.height.clamp(7, 11);
        self.head = self.head.clamp(7, 9);
        self.ear_size = self.ear_size.clamp(3, 7);
        self.legs = self.legs.clamp(3, 6);
        self.muzzle = self.muzzle.min(2);
        self.marking = self.marking.min(6);
        self.classic = self.classic.bounded();
        self.archetype = self.archetype.min(BODY_ARCHETYPES);
        self.face_template = self.face_template.min(FACE_TEMPLATES);
        // Both or neither, so every recipe is one a code can carry.
        if self.archetype == 0 || self.face_template == 0 {
            self.archetype = 0;
            self.face_template = 0;
        }
        // A blob carries its face high on its one mass, and at a mini's size the layouts that set
        // the eyes high reach past the top of it, so a blob wears the same layout a row lower.
        if self.body == BodyPlan::Blob {
            self.face_template = match self.face_template {
                4 | 12 => 1,
                7 => 8,
                11 => 9,
                other => other,
            };
        }
        self
    }

    /// Which generator drew this recipe.
    pub const fn edition(&self) -> Edition {
        if self.archetype == 0 {
            Edition::Original
        } else {
            Edition::Archetypes
        }
    }

    /// The archetype a recipe was drawn from, for a recipe drawn since 0.62.0.
    pub fn body_archetype(&self) -> Option<BodyArchetype> {
        BodyArchetype::from_number(self.archetype)
    }

    /// A new companion's recipe from `generator`, or a mini's from its parent's generator, so a
    /// family that began before archetypes existed goes on as it began.
    pub fn generated_by(
        generator: Edition,
        seed: [u8; 32],
        generation: u8,
        parent: Option<Self>,
    ) -> Self {
        match (parent.map_or(generator, |parent| parent.edition()), parent) {
            (Edition::Original, _) => Self::generated_original(seed, generation, parent),
            (Edition::Archetypes, Some(parent)) => Self::mini_of(parent, seed, generation),
            (Edition::Archetypes, None) => Self::drawn(seed, generation).0,
        }
    }

    /// A new companion's recipe as the current generator makes it, and how it came to be chosen.
    ///
    /// Its archetype is drawn first, then how strange it is to be: seven in ten are the cutest of
    /// six recipes drawn from the archetype's own odds, two in ten are the cutest of six that each
    /// break one of its rules, and one in ten is a genuine oddball, the least coherent of six drawn
    /// strange on purpose. Everything comes from named streams of the seed, so the same seed always
    /// makes the same companion.
    pub fn drawn(seed: [u8; 32], generation: u8) -> (Self, Strangeness) {
        let streams = SeedStream::new(seed);
        let mut choice = streams.rng("creature-archetype-v1", u64::from(generation));
        let archetype = weighted(
            &mut choice,
            &BodyArchetype::ALL.map(|archetype| (archetype, archetype.weight())),
        );
        let roll: f64 = choice.random();
        let strangeness = if roll < 0.7 {
            Strangeness::Cute
        } else if roll < 0.9 {
            Strangeness::WeirdCute
        } else {
            Strangeness::Oddball
        };
        let candidates: Vec<Self> = (0..CANDIDATES)
            .map(|index| {
                let mut rng = streams.rng("creature-recipe-v1", u64::from(generation) * 64 + index);
                let mut design = Self::draw(archetype, &mut rng);
                match strangeness {
                    Strangeness::Cute => {}
                    Strangeness::WeirdCute => design.mutate(&mut rng, 1),
                    Strangeness::Oddball => {
                        if rng.random_bool(0.4) {
                            design = Self::draw(BodyArchetype::Goober, &mut rng);
                        }
                        let count = rng.random_range(2..=3);
                        design.mutate(&mut rng, count);
                    }
                }
                design.bounded()
            })
            .collect();
        let score = |design: &Self| design.coherence();
        let chosen = match strangeness {
            Strangeness::Cute | Strangeness::WeirdCute => {
                candidates.iter().copied().reduce(|best, next| {
                    if score(&next) > score(&best) {
                        next
                    } else {
                        best
                    }
                })
            }
            Strangeness::Oddball => candidates.iter().copied().reduce(|least, next| {
                if score(&next) < score(&least) {
                    next
                } else {
                    least
                }
            }),
        }
        .expect("there are candidates");
        (chosen, strangeness)
    }

    /// A mini of a parent drawn since 0.62.0: its parent's archetype, body, face and accent, usually
    /// its ears, a coat a few shades off its parent's, and proportions of its own from the
    /// archetype's odds.
    fn mini_of(parent: Self, seed: [u8; 32], generation: u8) -> Self {
        let mut rng = SeedStream::new(seed).rng("creature-mini-v1", u64::from(generation));
        let archetype = parent.body_archetype().unwrap_or(BodyArchetype::Mochi);
        let mut design = Self::draw(archetype, &mut rng);
        design.body = parent.body;
        design.face_template = parent.face_template;
        design.archetype = parent.archetype;
        if rng.random_bool(0.8) {
            design.ears = parent.ears;
            design.ear_size = parent.ear_size;
        }
        design.accent = parent.accent;
        design.coat = parent
            .coat
            .map(|channel| (i16::from(channel) + rng.random_range(-18..=18)).clamp(0, 255) as u8);
        design.classic = parent.classic.inherited(&mut rng);
        design.bounded()
    }

    /// One recipe from an archetype's odds. Each part is drawn after the ones it has to suit, and
    /// its odds lean away from what would clash with them: big ears make a big tail less likely,
    /// wings keep the ears and tail simple, a tiny body keeps everything that sticks out small, a
    /// busy outline keeps the markings plain, a bright coat keeps the accent colour to a few
    /// places, and busy markings keep the face plain. Nothing is ruled out; the odds only lean.
    fn draw(archetype: BodyArchetype, rng: &mut impl Rng) -> Self {
        use BodyArchetype as A;
        use BodyPlan as P;
        use EarStyle as E;
        let body = match archetype {
            A::Mochi => weighted(rng, &[(P::Round, 6.0), (P::Blob, 4.0)]),
            A::Critter | A::Whelp => P::Long,
            A::Bean => P::Upright,
            A::Puff => weighted(rng, &[(P::Round, 5.0), (P::Blob, 5.0)]),
            A::Sprite => weighted(rng, &[(P::Upright, 7.0), (P::Round, 3.0)]),
            A::Birb => P::Winged,
            A::Goober => weighted(rng, &[(P::Blob, 8.0), (P::Round, 2.0)]),
        };
        let mut span = |low: u8, high: u8| rng.random_range(low..=high);
        let (width, height, head, legs) = match archetype {
            A::Mochi => (span(10, 12), span(9, 11), 9, span(3, 4)),
            A::Critter => (span(9, 11), span(7, 9), span(8, 9), span(3, 5)),
            A::Bean => (span(8, 9), span(8, 10), 9, 3),
            A::Puff => (span(11, 12), span(10, 11), span(8, 9), span(3, 4)),
            A::Sprite => (span(8, 10), span(8, 10), span(8, 9), span(3, 5)),
            A::Whelp => (span(10, 12), span(8, 10), span(8, 9), span(4, 6)),
            A::Birb => (span(10, 12), span(9, 11), span(8, 9), 3),
            A::Goober => (span(8, 12), span(7, 11), span(7, 9), span(3, 6)),
        };
        let tiny = width <= 9 && height <= 8;
        let winged = body == P::Winged;
        let ear_odds: [(EarStyle, f32); 6] = match archetype {
            A::Mochi => [
                (E::None, 3.0),
                (E::Round, 5.0),
                (E::Tuft, 1.0),
                (E::Floppy, 1.5),
                (E::Pointed, 0.5),
                (E::Long, 0.2),
            ],
            A::Critter => [
                (E::None, 0.3),
                (E::Round, 2.5),
                (E::Pointed, 3.5),
                (E::Tuft, 2.0),
                (E::Floppy, 1.0),
                (E::Long, 1.0),
            ],
            A::Bean => [
                (E::None, 2.0),
                (E::Round, 3.0),
                (E::Pointed, 2.0),
                (E::Tuft, 1.0),
                (E::Floppy, 1.0),
                (E::Long, 0.5),
            ],
            A::Puff => [
                (E::None, 2.0),
                (E::Round, 3.0),
                (E::Tuft, 4.0),
                (E::Floppy, 1.0),
                (E::Pointed, 0.5),
                (E::Long, 0.2),
            ],
            A::Sprite => [
                (E::None, 3.0),
                (E::Round, 2.0),
                (E::Pointed, 2.0),
                (E::Tuft, 1.0),
                (E::Floppy, 0.5),
                (E::Long, 1.0),
            ],
            A::Whelp => [
                (E::None, 0.2),
                (E::Round, 2.0),
                (E::Pointed, 2.5),
                (E::Tuft, 1.0),
                (E::Floppy, 3.0),
                (E::Long, 1.5),
            ],
            A::Birb => [
                (E::None, 4.0),
                (E::Tuft, 4.0),
                (E::Round, 1.5),
                (E::Pointed, 0.5),
                (E::Floppy, 0.2),
                (E::Long, 0.1),
            ],
            A::Goober => [
                (E::None, 1.0),
                (E::Round, 1.0),
                (E::Pointed, 1.0),
                (E::Tuft, 1.0),
                (E::Floppy, 1.0),
                (E::Long, 1.0),
            ],
        };
        // Wings and tiny bodies keep the ears small and simple.
        let ear_odds = ear_odds.map(|(ears, odds)| {
            let busy = matches!(ears, E::Long | E::Floppy | E::Pointed);
            let lean = if busy && (winged || tiny) { 0.35 } else { 1.0 };
            (ears, odds * lean)
        });
        let ears = weighted(rng, &ear_odds);
        let (low, high) = match archetype {
            A::Mochi | A::Puff | A::Birb => (3, 4),
            A::Bean | A::Sprite => (3, 5),
            A::Critter | A::Whelp => (4, 6),
            A::Goober => (3, 7),
        };
        let mut ear_size = rng.random_range(low..=high);
        if tiny || winged {
            ear_size = ear_size.min(4);
        }
        let big_ears = ear_size >= 6 && matches!(ears, E::Pointed | E::Long | E::Floppy);
        // Tails 0 none, 1 a puff, 2 a taper, 3 a tufted taper, 4 a big round tail.
        let tail_odds: [f32; 5] = match archetype {
            A::Mochi => [4.0, 4.0, 0.5, 0.3, 1.0],
            A::Critter => [0.5, 1.5, 3.0, 3.0, 1.5],
            A::Bean => [4.0, 3.0, 0.5, 0.5, 0.5],
            A::Puff => [1.0, 4.0, 0.2, 0.5, 3.0],
            A::Sprite => [3.0, 2.0, 1.0, 1.0, 0.5],
            A::Whelp => [0.2, 0.5, 1.5, 4.0, 4.0],
            A::Birb => [4.0, 3.0, 0.3, 0.3, 0.5],
            A::Goober => [1.0, 1.0, 1.0, 1.0, 1.0],
        };
        let tail = weighted(
            rng,
            &[0_u8, 1, 2, 3, 4].map(|tail| {
                let mut odds = tail_odds[usize::from(tail)];
                if tail >= 3 && big_ears {
                    odds *= 0.3;
                }
                if tail >= 2 && (tiny || winged) {
                    odds *= 0.4;
                }
                (tail, odds)
            }),
        );
        let muzzle = match archetype {
            A::Critter | A::Whelp => weighted(rng, &[(0, 1.0), (1, 2.0), (2, 1.5)]),
            A::Goober => rng.random_range(0..=2),
            _ => weighted(rng, &[(0, 5.0), (1, 1.5), (2, 0.3)]),
        };
        // The originals' parts, each at the archetype's own odds.
        let mut classic = ClassicParts::default();
        let candy = chance(
            rng,
            match archetype {
                A::Goober => 0.5,
                A::Sprite => 0.45,
                _ => 0.3,
            },
        );
        classic.coat = u8::from(candy);
        let (nubs, sticks) = match archetype {
            A::Mochi | A::Puff => (0.3, 0.0),
            A::Bean => (0.25, 0.03),
            A::Sprite => (0.35, 0.2),
            A::Birb => (0.2, 0.0),
            A::Critter | A::Whelp => (0.1, 0.05),
            A::Goober => (0.3, 0.3),
        };
        let limbs_roll: f64 = rng.random();
        classic.limbs = if limbs_roll < sticks {
            2
        } else if limbs_roll < sticks + nubs {
            1
        } else {
            0
        };
        let crown = match archetype {
            A::Sprite => 0.85,
            A::Goober => 0.35,
            A::Bean => 0.1,
            A::Mochi => 0.05,
            _ => 0.02,
        };
        if chance(rng, crown) {
            classic.crown = weighted(rng, &[(1, 5.5), (2, 4.5)]);
        }
        let curl = match archetype {
            A::Whelp => 0.25,
            A::Sprite => 0.2,
            A::Goober => 0.3,
            _ => 0.03,
        };
        if chance(rng, curl) {
            classic.tail = match archetype {
                A::Sprite => 2,
                A::Whelp => weighted(rng, &[(1, 4.0), (2, 1.0)]),
                _ => rng.random_range(1..=2),
            };
        }
        // Colours next, so a bright coat can keep the accent to a few places.
        let (coat, accent) = if candy {
            let (coat, accent) = CANDY[rng.random_range(0..CANDY.len())];
            let mut nudge = |rgb: [u8; 3]| {
                rgb.map(|channel| {
                    (i16::from(channel) + rng.random_range(-12..=12)).clamp(0, 255) as u8
                })
            };
            (nudge(coat), nudge(accent))
        } else {
            harmonious(rng)
        };
        let bright = to_hsl(coat).1 > 0.5;
        // How much already sticks out of the outline.
        let outline = ear_weight(ears, ear_size)
            + tail_weight(tail, classic.tail)
            + if classic.crown > 0 { 1.0 } else { 0.0 }
            + if winged { 1.0 } else { 0.0 };
        let busy = 1.0 / (1.0 + outline * 0.8);
        let accented = if bright { 0.45 } else { 1.0 };
        // 0 none, 1 a pale belly, 6 two pale spots, 5 a band, 4 a patch, 2 stripes, 3 spots; the
        // last four are drawn in the accent colour.
        let marking = weighted(
            rng,
            &[
                (0, 3.0),
                (1, 3.0),
                (6, 2.0),
                (5, 1.5 * busy * accented),
                (4, 1.0 * busy * accented),
                (2, 1.0 * busy * accented),
                (3, 1.0 * busy * accented),
            ],
        );
        let pattern = match archetype {
            A::Goober => 0.4,
            A::Puff => 0.15,
            _ => 0.1,
        } * f64::from(busy * accented);
        if chance(rng, pattern) {
            classic.pattern = rng.random_range(1..=3);
        }
        let face_template = weighted(rng, archetype.faces());
        Self {
            body,
            ears,
            tail,
            width,
            height,
            head,
            ear_size,
            legs,
            muzzle,
            marking,
            coat,
            accent,
            classic,
            archetype: archetype.number(),
            face_template,
        }
        .bounded()
    }

    /// Break some of the archetype's rules on purpose: each break redraws one part from
    /// everything any recipe can have.
    fn mutate(&mut self, rng: &mut impl Rng, breaks: u8) {
        for _ in 0..breaks {
            match rng.random_range(0..9) {
                0 => self.ears = EarStyle::ALL[rng.random_range(0..EarStyle::ALL.len())],
                1 => self.tail = rng.random_range(0..=4),
                2 => self.ear_size = rng.random_range(3..=7),
                3 => self.marking = rng.random_range(0..=6),
                4 => self.legs = rng.random_range(3..=6),
                5 => self.classic.crown = rng.random_range(0..=2),
                6 => self.classic.pattern = rng.random_range(0..=3),
                7 => self.face_template = rng.random_range(1..=FACE_TEMPLATES),
                _ => {
                    self.width = rng.random_range(8..=12);
                    self.height = rng.random_range(7..=11);
                }
            }
        }
        *self = self.bounded();
    }

    /// How cute and coherent a recipe reads, from 0 to 1. It weighs what the brief for 0.62.0
    /// asked it to: a big head on a small body, a plain outline, plain markings, colours that
    /// belong together, parts that suit one another, and a face that fits the head it is on.
    /// It is a heuristic for choosing between recipes, not a verdict on any one companion.
    pub fn coherence(&self) -> f32 {
        let d = self.bounded();
        let winged = d.body == BodyPlan::Winged;
        let blob = d.body == BodyPlan::Blob;
        // A head as wide as the body or wider reads young; a blob is all head.
        let head = if blob {
            0.85
        } else {
            ((f32::from(d.head) / f32::from(d.width) - 0.58) / (1.125 - 0.58)).clamp(0.0, 1.0)
        };
        let outline_load = ear_weight(d.ears, d.ear_size)
            + tail_weight(d.tail, d.classic.tail)
            + if d.classic.crown > 0 { 1.0 } else { 0.0 }
            + if winged { 0.8 } else { 0.0 }
            + if d.classic.limbs == 2 { 0.6 } else { 0.0 };
        let outline = 1.0 - (outline_load / 3.2).min(1.0);
        let marking_load = marking_weight(d.marking, d.classic.pattern)
            + f32::from(d.muzzle) * 0.15
            + if d.classic.limbs == 1 { 0.2 } else { 0.0 };
        let markings = 1.0 - (marking_load / 1.4).min(1.0);
        // Colours: a coat neither murky nor glaring, and an accent that belongs with it without
        // vanishing into it.
        let (coat_hue, coat_saturation, coat_lightness) = to_hsl(d.coat);
        let (accent_hue, accent_saturation, accent_lightness) = to_hsl(d.accent);
        let lightness = 1.0 - ((coat_lightness - 0.7).abs() - 0.1).max(0.0) * 4.0;
        let saturation = 1.0 - ((coat_saturation - 0.45).abs() - 0.2).max(0.0) * 3.0;
        let gap = hue_gap(coat_hue, accent_hue);
        let related =
            if gap <= 55.0 || (150.0..=180.0).contains(&gap) || (105.0..=135.0).contains(&gap) {
                1.0
            } else {
                0.55
            };
        let apart = ((gap / 40.0)
            + (coat_lightness - accent_lightness).abs() * 4.0
            + (coat_saturation - accent_saturation).abs() * 2.0)
            .min(1.0);
        let palette = (lightness.clamp(0.0, 1.0) * 0.35
            + saturation.clamp(0.0, 1.0) * 0.25
            + related * 0.25
            + apart * 0.15)
            .clamp(0.0, 1.0);
        // Parts that do not suit one another.
        let mut clashes = 0.0;
        let big_ears = d.ear_size >= 6
            && matches!(
                d.ears,
                EarStyle::Pointed | EarStyle::Long | EarStyle::Floppy
            );
        let big_tail = d.tail >= 3 || d.classic.tail > 0;
        if big_ears && big_tail {
            clashes += 1.0;
        }
        if winged && (big_ears || big_tail) {
            clashes += 1.0;
        }
        if d.width <= 9 && d.height <= 8 && outline_load > 1.6 {
            clashes += 1.0;
        }
        if blob && d.legs >= 5 {
            clashes += 0.7;
        }
        if marking_load > 0.9 && outline_load > 1.8 {
            clashes += 1.0;
        }
        if coat_saturation > 0.55 && marking_weight(d.marking, d.classic.pattern) > 0.6 {
            clashes += 0.5;
        }
        let balance = 1.0 - (clashes / 3.0_f32).min(1.0);
        // A wide face wants a big head.
        let face = match d.face_template {
            2 | 3 | 5 | 6 | 8 if d.head < 8 && !blob => 0.5,
            9..=11 => 0.7,
            _ => 1.0,
        };
        (head * 0.18
            + outline * 0.2
            + markings * 0.14
            + palette * 0.2
            + balance * 0.18
            + face * 0.1)
            .clamp(0.0, 1.0)
    }

    /// A recipe as the modular stream alone draws it, with no classic parts: exactly what every
    /// seed generated before classic parts existed. Review sheets and tests that are about the
    /// modular parts start from this.
    pub fn modular(seed: [u8; 32], generation: u8, parent: Option<Self>) -> Self {
        let mut rng = SeedStream::new(seed).rng("modular-design-v1", u64::from(generation));
        let mut design = Self {
            body: BodyPlan::ALL[rng.random_range(0..BodyPlan::ALL.len())],
            ears: EarStyle::ALL[rng.random_range(0..6)],
            tail: rng.random_range(0..5),
            width: rng.random_range(8..=12),
            height: rng.random_range(7..=11),
            head: rng.random_range(7..=9),
            ear_size: rng.random_range(3..=7),
            legs: rng.random_range(3..=6),
            muzzle: rng.random_range(0..=2),
            marking: rng.random_range(0..7),
            coat: [
                rng.random_range(70..=240),
                rng.random_range(70..=240),
                rng.random_range(70..=240),
            ],
            accent: [
                rng.random_range(70..=240),
                rng.random_range(70..=240),
                rng.random_range(70..=240),
            ],
            classic: ClassicParts::default(),
            archetype: 0,
            face_template: 0,
        };
        if let Some(parent) = parent {
            design.body = parent.body;
            if rng.random_bool(0.8) {
                design.ears = parent.ears;
            }
            design.coat = parent.coat.map(|channel| {
                (i16::from(channel) + rng.random_range(-18..=18)).clamp(0, 255) as u8
            });
            design.accent = parent.accent;
        }
        design.bounded()
    }

    /// A new companion's recipe as the current generator makes it, or a mini's from its parent's.
    pub fn generated(seed: [u8; 32], generation: u8, parent: Option<Self>) -> Self {
        Self::generated_by(Edition::LATEST, seed, generation, parent)
    }

    /// A recipe as the original generator makes it: the modular one, then classic parts drawn
    /// from a stream of their own, so the modular stream still draws exactly what it always drew.
    /// A candy coat then replaces the modular colours with one of the original pairs.
    fn generated_original(seed: [u8; 32], generation: u8, parent: Option<Self>) -> Self {
        let mut design = Self::modular(seed, generation, parent);
        let mut classic = SeedStream::new(seed).rng("classic-parts-v1", u64::from(generation));
        design.classic = match parent {
            Some(parent) => parent.classic.inherited(&mut classic),
            None => ClassicParts::generated(&mut classic),
        };
        if parent.is_none() && design.classic.coat > 0 {
            let (coat, accent) = CANDY[classic.random_range(0..CANDY.len())];
            let mut nudge = |rgb: [u8; 3]| {
                rgb.map(|channel| {
                    (i16::from(channel) + classic.random_range(-12..=12)).clamp(0, 255) as u8
                })
            };
            design.coat = nudge(coat);
            design.accent = nudge(accent);
        }
        design.bounded()
    }

    /// The recipe as a shared code carries it: the sixteen modular bytes, then the four classic
    /// ones, which are all zero for a plain modular recipe. The last of those, which classic parts
    /// leave at zero, holds the archetype and the face template of a recipe drawn since 0.62.0.
    pub fn to_bytes(self) -> [u8; 20] {
        let d = self.bounded();
        let mut classic = d.classic.to_bytes();
        classic[3] = (d.archetype << 4) | d.face_template;
        [
            d.body as u8,
            d.ears as u8,
            d.tail,
            d.width,
            d.height,
            d.head,
            d.ear_size,
            d.legs,
            d.muzzle,
            d.marking,
            d.coat[0],
            d.coat[1],
            d.coat[2],
            d.accent[0],
            d.accent[1],
            d.accent[2],
            classic[0],
            classic[1],
            classic[2],
            classic[3],
        ]
    }

    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != 20 {
            return None;
        }
        let d = Self {
            body: *BodyPlan::ALL.get(bytes[0] as usize)?,
            ears: *EarStyle::ALL.get(bytes[1] as usize)?,
            tail: bytes[2],
            width: bytes[3],
            height: bytes[4],
            head: bytes[5],
            ear_size: bytes[6],
            legs: bytes[7],
            muzzle: bytes[8],
            marking: bytes[9],
            coat: bytes[10..13].try_into().ok()?,
            accent: bytes[13..16].try_into().ok()?,
            classic: ClassicParts::from_bytes([bytes[16], bytes[17], bytes[18], 0])?,
            archetype: bytes[19] >> 4,
            face_template: bytes[19] & 0x0f,
        };
        (d == d.bounded()).then_some(d)
    }
}

pub fn apply_creature_design(creature: &mut Creature, design: Option<CreatureDesign>) {
    let design = design.map(CreatureDesign::bounded);
    creature.appearance.design = design;
    creature.origin.design = design;
}

#[cfg(test)]
mod tests {
    use super::*;
    fn original(seed: [u8; 32], generation: u8, parent: Option<CreatureDesign>) -> CreatureDesign {
        CreatureDesign::generated_by(Edition::Original, seed, generation, parent)
    }

    #[test]
    fn recipes_are_compact_bounded_deterministic_and_varied() {
        // Sixteen modular bytes, six classic parts, an archetype and a face template, which a
        // shared code packs into four.
        assert_eq!(std::mem::size_of::<CreatureDesign>(), 24);
        let mut recipes = std::collections::HashSet::new();
        for index in 0..1024_u64 {
            let seed = SeedStream::new([23; 32]).bytes("test-design", index);
            let d = original(seed, 0, None);
            assert_eq!(d, d.bounded());
            assert_eq!(d, original(seed, 0, None));
            assert_eq!(Some(d), CreatureDesign::from_bytes(&d.to_bytes()));
            recipes.insert(d);
        }
        assert_eq!(recipes.len(), 1024);
        for body in BodyPlan::ALL {
            for ears in EarStyle::ALL {
                assert!(recipes.iter().any(|d| d.body == body && d.ears == ears));
            }
        }
        let parent = original([9; 32], 0, None);
        let mini = original([9; 32], 1, Some(parent));
        assert_eq!(mini.body, parent.body);
        assert_eq!(mini.accent, parent.accent);
        assert_ne!(mini, parent);
    }
    #[test]
    fn the_blob_plan_is_last_so_earlier_recipes_keep_their_appearance() {
        for (index, body) in BodyPlan::ALL.into_iter().enumerate() {
            assert_eq!(
                body as usize, index,
                "body indices are part of the share code"
            );
        }
        assert_eq!(BodyPlan::ALL[4], BodyPlan::Blob);
        let mut design = CreatureDesign::generated([7; 32], 0, None);
        design.body = BodyPlan::Blob;
        let bytes = design.to_bytes();
        assert_eq!(bytes[0], 4);
        assert_eq!(CreatureDesign::from_bytes(&bytes), Some(design));
        // Blobs stay reachable from ordinary generation.
        assert!(
            (0..256_u64)
                .map(|index| CreatureDesign::generated(
                    SeedStream::new([11; 32]).bytes("blob-reach", index),
                    0,
                    None
                ))
                .any(|design| design.body == BodyPlan::Blob)
        );
    }

    #[test]
    fn invalid_parts_are_rejected_by_the_share_decoder() {
        let valid = CreatureDesign::generated([4; 32], 0, None).to_bytes();
        for index in (0..10).chain(16..20) {
            let mut bad = valid;
            bad[index] = 255;
            assert!(CreatureDesign::from_bytes(&bad).is_none());
        }
        assert!(CreatureDesign::from_bytes(&valid[..15]).is_none());
        assert!(CreatureDesign::from_bytes(&valid[..16]).is_none());
    }

    /// Recipes exactly as v0.58.9 generated them, before classic parts existed. The modular
    /// stream still draws every one of these bytes; a candy coat may only repaint the colours.
    #[test]
    fn classic_parts_leave_the_modular_stream_drawing_what_it_always_drew() {
        let golden: [([u8; 32], u8, [u8; 16]); 6] = [
            (
                [23; 32],
                0,
                [0, 5, 1, 9, 7, 7, 4, 4, 2, 3, 163, 217, 219, 79, 98, 237],
            ),
            (
                [55; 32],
                0,
                [3, 5, 0, 12, 8, 8, 5, 4, 1, 3, 218, 193, 83, 135, 176, 80],
            ),
            (
                [61; 32],
                0,
                [2, 0, 2, 12, 8, 8, 4, 5, 1, 3, 114, 143, 217, 114, 184, 135],
            ),
            (
                [7; 32],
                0,
                [3, 5, 3, 8, 9, 8, 5, 3, 2, 2, 184, 176, 215, 202, 217, 171],
            ),
            (
                [11; 32],
                2,
                [0, 1, 4, 11, 10, 7, 4, 5, 2, 1, 235, 99, 128, 77, 181, 178],
            ),
            (
                [90; 32],
                3,
                [0, 5, 3, 8, 11, 9, 7, 4, 1, 0, 155, 112, 181, 169, 85, 167],
            ),
        ];
        let mut repainted = 0;
        for (seed, generation, before) in golden {
            let modular = CreatureDesign::modular(seed, generation, None);
            assert_eq!(modular.to_bytes()[..16], before, "{seed:?}");
            assert!(modular.classic.is_modular());
            let design = original(seed, generation, None);
            let bytes = design.to_bytes();
            assert_eq!(bytes[..10], before[..10], "{seed:?} changed shape");
            if design.classic.coat == 0 {
                assert_eq!(bytes[..16], before, "{seed:?} changed colour");
            } else {
                repainted += 1;
            }
        }
        assert!(
            (1..6).contains(&repainted),
            "the golden set covers both coats"
        );
        // A mini of a modular parent, as v0.58.9 drew it.
        let parent = CreatureDesign::modular([9; 32], 0, None);
        assert_eq!(
            CreatureDesign::modular([9; 32], 1, Some(parent)).to_bytes()[..16],
            [2, 4, 3, 11, 11, 9, 3, 5, 0, 6, 226, 177, 103, 121, 122, 72]
        );
    }

    #[test]
    fn classic_parts_pack_into_four_bytes_and_reject_anything_out_of_range() {
        let [coat, face, limbs, crown, pattern, tail] = ClassicParts::LIMITS;
        let mut seen = std::collections::HashSet::new();
        for coat in 0..=coat {
            for face in 0..=face {
                for limbs in 0..=limbs {
                    for crown in 0..=crown {
                        for pattern in 0..=pattern {
                            for tail in 0..=tail {
                                let parts = ClassicParts {
                                    coat,
                                    face,
                                    limbs,
                                    crown,
                                    pattern,
                                    tail,
                                };
                                let bytes = parts.to_bytes();
                                assert_eq!(bytes[3], 0);
                                assert_eq!(ClassicParts::from_bytes(bytes), Some(parts));
                                assert!(seen.insert(bytes), "{parts:?} packs like another");
                            }
                        }
                    }
                }
            }
        }
        assert_eq!(ClassicParts::default().to_bytes(), [0; 4]);
        assert_eq!(ClassicParts::from_bytes([0, 0, 0, 1]), None);
        for (byte, value) in [
            (0, 0x02),
            (0, 0x30),
            (1, 0x06),
            (1, 0x30),
            (2, 0x04),
            (2, 0x30),
        ] {
            let mut bytes = [0; 4];
            bytes[byte] = value;
            assert_eq!(ClassicParts::from_bytes(bytes), None, "{bytes:?}");
        }
    }

    /// A quarter lean wholly modular and a quarter wholly classic, and the half between mix the two
    /// a part at a time. A mixed lean can still draw no part, or all three main ones, so about 29%
    /// come out plainly modular and about 33% with classic colours, face, and limbs together.
    #[test]
    fn new_recipes_span_the_whole_way_between_the_two_looks() {
        let designs: Vec<_> = (0..4096_u64)
            .map(|index| {
                original(
                    SeedStream::new([37; 32]).bytes("classic-spread", index),
                    0,
                    None,
                )
            })
            .collect();
        let share = |test: &dyn Fn(&ClassicParts) -> bool| {
            designs.iter().filter(|d| test(&d.classic)).count() as f64 / designs.len() as f64
        };
        let modular = share(&|parts| parts.is_modular());
        let classic = share(&|parts| parts.coat > 0 && parts.face > 0 && parts.limbs > 0);
        let mixed = 1.0 - modular - classic;
        assert!((0.25..0.33).contains(&modular), "plainly modular {modular}");
        assert!((0.29..0.37).contains(&classic), "wholly classic {classic}");
        assert!(mixed > 0.33, "mixed {mixed}");
        let reads: [fn(&ClassicParts) -> u8; 6] = [
            |p| p.coat,
            |p| p.face,
            |p| p.limbs,
            |p| p.crown,
            |p| p.pattern,
            |p| p.tail,
        ];
        let names = ["coat", "face", "limbs", "crown", "pattern", "tail"];
        for ((part, limit), name) in reads.into_iter().zip(ClassicParts::LIMITS).zip(names) {
            for value in 0..=limit {
                assert!(
                    designs.iter().any(|d| part(&d.classic) == value),
                    "{name} {value} is never drawn"
                );
            }
        }
        // Candy coats start from the original palettes' pairs, nudged by a few steps at most.
        for design in designs.iter().filter(|d| d.classic.coat > 0) {
            let near = |a: [u8; 3], b: [u8; 3]| a.iter().zip(b).all(|(x, y)| x.abs_diff(y) <= 12);
            assert!(
                CANDY
                    .iter()
                    .any(|(coat, accent)| near(design.coat, *coat) && near(design.accent, *accent)),
                "{design:?}"
            );
        }
    }

    #[test]
    fn a_mini_keeps_its_parents_colouring_face_and_build() {
        let mut kept = [0; 3];
        let mut parents = 0;
        for index in 0..512_u64 {
            let seed = SeedStream::new([41; 32]).bytes("classic-minis", index);
            let parent = CreatureDesign::generated(seed, 0, None);
            if parent.classic.is_modular() {
                continue;
            }
            parents += 1;
            let mini = CreatureDesign::generated(seed, 1, Some(parent));
            assert_eq!(mini.classic.coat, parent.classic.coat);
            assert_eq!(mini.classic.face, parent.classic.face);
            assert_eq!(mini.classic.limbs, parent.classic.limbs);
            for (count, (a, b)) in kept.iter_mut().zip([
                (mini.classic.crown, parent.classic.crown),
                (mini.classic.pattern, parent.classic.pattern),
                (mini.classic.tail, parent.classic.tail),
            ]) {
                *count += usize::from(a == b);
            }
        }
        // Crown, pattern, and tail are kept most of the time, not always.
        for count in kept {
            assert!(
                count * 2 > parents && count < parents,
                "{count} of {parents}"
            );
        }
    }

    #[test]
    fn a_recipe_saved_before_classic_parts_loads_modular_and_saves_the_same_way() {
        let design = CreatureDesign {
            classic: ClassicParts::default(),
            ..CreatureDesign::generated([19; 32], 0, None)
        };
        let json = serde_json::to_string(&design).unwrap();
        assert!(!json.contains("classic"), "{json}");
        assert_eq!(
            serde_json::from_str::<CreatureDesign>(&json).unwrap(),
            design
        );
        let classic = CreatureDesign {
            classic: ClassicParts {
                face: 2,
                tail: 1,
                ..ClassicParts::default()
            },
            ..design
        };
        let json = serde_json::to_string(&classic).unwrap();
        assert!(json.contains("classic"), "{json}");
        assert_eq!(
            serde_json::from_str::<CreatureDesign>(&json).unwrap(),
            classic
        );
    }

    fn drawn_population(count: u64) -> Vec<(CreatureDesign, Strangeness)> {
        (0..count)
            .map(|index| {
                CreatureDesign::drawn(SeedStream::new([43; 32]).bytes("archetypes", index), 0)
            })
            .collect()
    }

    /// Seven in ten are the cutest of their draws, two in ten weird-cute, one in ten an oddball;
    /// every archetype turns up about as often as its odds say, and every face is worn.
    #[test]
    fn new_recipes_are_mostly_cute_now_and_then_weird_and_rarely_odd() {
        let designs = drawn_population(4000);
        let share = |test: &dyn Fn(&(CreatureDesign, Strangeness)) -> bool| {
            designs.iter().filter(|d| test(d)).count() as f32 / designs.len() as f32
        };
        let cute = share(&|(_, s)| *s == Strangeness::Cute);
        let weird = share(&|(_, s)| *s == Strangeness::WeirdCute);
        let odd = share(&|(_, s)| *s == Strangeness::Oddball);
        assert!((0.67..0.73).contains(&cute), "cute {cute}");
        assert!((0.17..0.23).contains(&weird), "weird-cute {weird}");
        assert!((0.08..0.12).contains(&odd), "oddballs {odd}");
        let total: f32 = BodyArchetype::ALL.iter().map(|a| a.weight()).sum();
        for archetype in BodyArchetype::ALL {
            let seen = share(&|(d, s)| {
                *s != Strangeness::Oddball && d.body_archetype() == Some(archetype)
            }) / (cute + weird);
            let expected = archetype.weight() / total;
            assert!(
                (seen - expected).abs() < 0.035,
                "{archetype:?} {seen} against {expected}"
            );
        }
        for template in 1..=FACE_TEMPLATES {
            assert!(
                designs.iter().any(|(d, _)| d.face_template == template),
                "face {template} is never worn"
            );
        }
        // Oddballs read as the least coherent, the cute ones as the most.
        let mean = |strangeness: Strangeness| {
            let scores: Vec<f32> = designs
                .iter()
                .filter(|(_, s)| *s == strangeness)
                .map(|(d, _)| d.coherence())
                .collect();
            scores.iter().sum::<f32>() / scores.len() as f32
        };
        let (cute, weird, odd) = (
            mean(Strangeness::Cute),
            mean(Strangeness::WeirdCute),
            mean(Strangeness::Oddball),
        );
        eprintln!("coherence: cute {cute:.3}, weird-cute {weird:.3}, oddball {odd:.3}");
        assert!(cute > weird && weird > odd, "{cute} {weird} {odd}");
    }

    /// Recipes exactly as 0.62.0 draws them. A change here changes every companion made from
    /// then on; it needs new stream labels and a new edition, not an edit.
    #[test]
    fn new_recipes_are_drawn_exactly_as_they_always_were() {
        let golden: [([u8; 32], [u8; 20], Strangeness); 4] = [
            (
                [23; 32],
                [
                    4, 0, 2, 11, 10, 9, 4, 3, 0, 0, 229, 201, 156, 153, 203, 235, 0, 0, 0, 66,
                ],
                Strangeness::Cute,
            ),
            (
                [55; 32],
                [
                    2, 5, 3, 9, 7, 9, 4, 5, 1, 1, 167, 214, 95, 249, 176, 107, 1, 0, 0, 36,
                ],
                Strangeness::Cute,
            ),
            (
                [61; 32],
                [
                    3, 1, 0, 10, 10, 8, 5, 3, 0, 6, 123, 208, 118, 126, 229, 169, 0, 0, 0, 114,
                ],
                Strangeness::WeirdCute,
            ),
            (
                [7; 32],
                [
                    4, 5, 3, 11, 11, 9, 3, 3, 1, 1, 97, 180, 223, 234, 205, 122, 1, 0, 0, 133,
                ],
                Strangeness::Cute,
            ),
        ];
        for (seed, bytes, strangeness) in golden {
            let (design, drawn) = CreatureDesign::drawn(seed, 0);
            assert_eq!(design.to_bytes(), bytes, "{seed:?}");
            assert_eq!(drawn, strangeness, "{seed:?}");
        }
    }

    #[test]
    fn a_new_recipe_is_a_pure_function_of_its_seed_and_travels_whole() {
        for (index, (design, strangeness)) in drawn_population(512).into_iter().enumerate() {
            let seed = SeedStream::new([43; 32]).bytes("archetypes", index as u64);
            assert_eq!((design, strangeness), CreatureDesign::drawn(seed, 0));
            assert_eq!(design, CreatureDesign::generated(seed, 0, None));
            assert_eq!(design, design.bounded());
            assert_eq!(design.edition(), Edition::Archetypes);
            assert_eq!(Some(design), CreatureDesign::from_bytes(&design.to_bytes()));
            // A recipe keeps the classic face out of it: its layout places the eyes.
            assert_eq!(design.classic.face, 0);
            let json = serde_json::to_string(&design).unwrap();
            assert!(
                json.contains("archetype") && json.contains("face_template"),
                "{json}"
            );
            assert_eq!(
                serde_json::from_str::<CreatureDesign>(&json).unwrap(),
                design
            );
        }
    }

    /// An archetype sets the body a companion can have, unless one of its rules was broken on
    /// purpose; every body plan is still reachable.
    #[test]
    fn archetypes_keep_to_their_bodies() {
        let designs = drawn_population(2000);
        for (design, strangeness) in &designs {
            let archetype = design.body_archetype().expect("every new recipe has one");
            if *strangeness != Strangeness::Oddball {
                assert!(
                    archetype.allows(design.body),
                    "{archetype:?} {:?}",
                    design.body
                );
            }
        }
        for body in BodyPlan::ALL {
            assert!(designs.iter().any(|(d, _)| d.body == body), "{body:?}");
        }
    }

    /// Parts that clash are drawn together less often than parts that suit each other, though
    /// never never.
    #[test]
    fn clashing_parts_come_together_less_often() {
        let designs: Vec<CreatureDesign> = drawn_population(6000)
            .into_iter()
            .filter(|(_, s)| *s == Strangeness::Cute)
            .map(|(d, _)| d)
            .collect();
        let rate = |given: &dyn Fn(&CreatureDesign) -> bool,
                    then: &dyn Fn(&CreatureDesign) -> bool| {
            let pool: Vec<_> = designs.iter().filter(|d| given(d)).collect();
            pool.iter().filter(|d| then(d)).count() as f32 / pool.len().max(1) as f32
        };
        let big_ears = |d: &CreatureDesign| {
            d.ear_size >= 6
                && matches!(
                    d.ears,
                    EarStyle::Pointed | EarStyle::Long | EarStyle::Floppy
                )
        };
        let big_tail = |d: &CreatureDesign| d.tail >= 3;
        let long_bodies = |d: &CreatureDesign| d.body == BodyPlan::Long;
        assert!(
            rate(&|d| long_bodies(d) && big_ears(d), &big_tail)
                < rate(&|d| long_bodies(d) && !big_ears(d), &big_tail),
            "big ears make a big tail less likely"
        );
        let busy_ears = |d: &CreatureDesign| matches!(d.ears, EarStyle::Long | EarStyle::Floppy);
        assert!(
            rate(&|d| d.body == BodyPlan::Winged, &busy_ears)
                < rate(&|d| d.body != BodyPlan::Winged, &busy_ears),
            "wings keep the ears simple"
        );
        let accented = |d: &CreatureDesign| d.classic.pattern > 0 || matches!(d.marking, 2..=5);
        let bright = |d: &CreatureDesign| to_hsl(d.coat).1 > 0.5 && d.classic.coat == 0;
        assert!(
            rate(&bright, &accented) < rate(&|d| !bright(d) && d.classic.coat == 0, &accented),
            "a bright coat keeps the accent to a few places"
        );
        // Never never: the cutest pick steers round a clash, but a companion that breaks its
        // archetype's rules can still have one.
        assert!(
            drawn_population(6000)
                .iter()
                .any(|(d, _)| big_ears(d) && big_tail(d)),
            "never never"
        );
    }

    #[test]
    fn a_mini_of_a_new_companion_takes_after_it() {
        let mut kept_ears = 0;
        let count = 400;
        for index in 0..count {
            let seed = SeedStream::new([47; 32]).bytes("archetype-minis", index);
            let parent = CreatureDesign::generated(seed, 0, None);
            let mini = CreatureDesign::generated(seed, 1, Some(parent));
            assert_eq!(mini.archetype, parent.archetype);
            assert_eq!(mini.body, parent.body);
            assert_eq!(mini.face_template, parent.face_template);
            assert_eq!(mini.accent, parent.accent);
            assert_eq!(mini.classic.coat, parent.classic.coat);
            assert_ne!(mini, parent);
            kept_ears += usize::from(mini.ears == parent.ears);
        }
        assert!(
            kept_ears * 10 > count as usize * 7,
            "{kept_ears} of {count}"
        );
        // A family that began before archetypes goes on the way it began.
        let parent = original([9; 32], 0, None);
        let mini = CreatureDesign::generated([9; 32], 1, Some(parent));
        assert_eq!(mini.edition(), Edition::Original);
        assert_eq!(mini, original([9; 32], 1, Some(parent)));
    }
}
