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

/// Parts, proportions, and color: sixteen bytes of modular recipe and six classic parts. Every
/// combination has a connected rounded body, a reserved large face, paired limbs, and bounded
/// appendages inside the same 48px atlas.
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
        self
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

    /// A new companion's recipe: the modular one, then classic parts drawn from a stream of their
    /// own, so the modular stream still draws exactly what it always drew. A candy coat then
    /// replaces the modular colours with one of the original pairs.
    pub fn generated(seed: [u8; 32], generation: u8, parent: Option<Self>) -> Self {
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
    /// ones, which are all zero for a plain modular recipe.
    pub fn to_bytes(self) -> [u8; 20] {
        let d = self.bounded();
        let classic = d.classic.to_bytes();
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
            classic: ClassicParts::from_bytes(bytes[16..20].try_into().ok()?)?,
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
    #[test]
    fn recipes_are_compact_bounded_deterministic_and_varied() {
        // Sixteen modular bytes and six classic parts, which a shared code packs into four.
        assert_eq!(std::mem::size_of::<CreatureDesign>(), 22);
        let mut recipes = std::collections::HashSet::new();
        for index in 0..1024_u64 {
            let seed = SeedStream::new([23; 32]).bytes("test-design", index);
            let d = CreatureDesign::generated(seed, 0, None);
            assert_eq!(d, d.bounded());
            assert_eq!(d, CreatureDesign::generated(seed, 0, None));
            assert_eq!(Some(d), CreatureDesign::from_bytes(&d.to_bytes()));
            recipes.insert(d);
        }
        assert_eq!(recipes.len(), 1024);
        for body in BodyPlan::ALL {
            for ears in EarStyle::ALL {
                assert!(recipes.iter().any(|d| d.body == body && d.ears == ears));
            }
        }
        let parent = CreatureDesign::generated([9; 32], 0, None);
        let mini = CreatureDesign::generated([9; 32], 1, Some(parent));
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
            let design = CreatureDesign::generated(seed, generation, None);
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
                CreatureDesign::generated(
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
}
