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

/// Sixteen bytes of parts, proportions, and color. Every combination has a connected rounded
/// body, a reserved large face, paired limbs, and bounded appendages inside the same 48px atlas.
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
        self
    }

    pub fn generated(seed: [u8; 32], generation: u8, parent: Option<Self>) -> Self {
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

    pub fn to_bytes(self) -> [u8; 16] {
        let d = self.bounded();
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
        ]
    }

    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != 16 {
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
        assert_eq!(std::mem::size_of::<CreatureDesign>(), 16);
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
        for index in 0..10 {
            let mut bad = valid;
            bad[index] = 255;
            assert!(CreatureDesign::from_bytes(&bad).is_none());
        }
        assert!(CreatureDesign::from_bytes(&valid[..15]).is_none());
    }
}
