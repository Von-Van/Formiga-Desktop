//! How a traveler looks: everything the art crate needs to draw it exactly as Desktop does, and
//! nothing about how Desktop caches or bakes that drawing.
//!
//! Every type here is the travel format's own. Each enum mirrors one in `formiga-core` variant
//! for variant, through conversions that match exhaustively, so a variant added to the core
//! fails to build here until the travel format is deliberately extended (and versioned).

use crate::document::{TravelError, hex, unhex};
use formiga_core as core;
use serde::{Deserialize, Serialize};

/// An enum of the travel format's own, written in `snake_case`, with conversions to and from
/// the core enum it mirrors.
macro_rules! mirror {
    ($(#[$meta:meta])* $name:ident = $core:ty { $($variant:ident),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(rename_all = "snake_case")]
        pub enum $name {
            $($variant),+
        }

        impl From<$core> for $name {
            fn from(value: $core) -> Self {
                match value {
                    $(<$core>::$variant => Self::$variant),+
                }
            }
        }

        impl From<$name> for $core {
            fn from(value: $name) -> Self {
                match value {
                    $($name::$variant => Self::$variant),+
                }
            }
        }
    };
}
pub(crate) use mirror;

mirror!(
    BodyFamily = core::BodyFamily {
        Blob,
        Hopper,
        SoftQuadruped
    }
);
mirror!(
    PatternKind = core::PatternKind {
        Solid,
        Patches,
        Spots,
        Stripes,
        Mask,
        Socks,
        Tips
    }
);
mirror!(
    HeadAppendageStyle = core::HeadAppendageStyle {
        None,
        Round,
        Pointed,
        Leaf,
        Droop,
        Antenna
    }
);
mirror!(
    EyeShape = core::EyeShape {
        Round,
        Tall,
        SoftSquare
    }
);
mirror!(PupilStyle = core::PupilStyle { Dot, Wide, Spark });
mirror!(
    HighlightStyle = core::HighlightStyle {
        Single,
        Double,
        Diagonal
    }
);
mirror!(BrowStyle = core::BrowStyle { None, Soft, Bold });
mirror!(
    MouthStyle = core::MouthStyle {
        Tiny,
        Smile,
        Cat,
        Beak
    }
);
mirror!(CheekStyle = core::CheekStyle { None, Dots, Blush });
mirror!(
    ForelimbStyle = core::ForelimbStyle {
        SoftNub,
        Pseudopod,
        MittenArm,
        FrontPaw
    }
);
mirror!(LimbTipStyle = core::LimbTipStyle { Round, Mitten, Paw });
mirror!(
    RestPose = core::RestPose {
        AtSides,
        Folded,
        Together
    }
);
mirror!(
    EffectMotif = core::EffectMotif {
        None,
        Dot,
        Star,
        Heart,
        Leaf,
        Spark
    }
);
mirror!(
    TailStyle = core::TailStyle {
        None,
        Stub,
        Taper,
        Tuft,
        Curl
    }
);

/// A companion's recipe, in the byte form its share code carries it: the twenty bytes of parts,
/// proportions, colour, archetype and face, and the eight bytes of details when it has any.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesignRecipe {
    /// 40 lowercase hex digits.
    pub parts: String,
    /// 16 lowercase hex digits, absent for a recipe without details.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
}

impl From<core::CreatureDesign> for DesignRecipe {
    fn from(design: core::CreatureDesign) -> Self {
        Self {
            parts: hex(&design.to_bytes()),
            details: (!design.details.is_none()).then(|| hex(&design.details.to_bytes())),
        }
    }
}

impl TryFrom<&DesignRecipe> for core::CreatureDesign {
    type Error = TravelError;

    fn try_from(recipe: &DesignRecipe) -> Result<Self, TravelError> {
        let refused = || TravelError::invalid("a recipe this build cannot draw");
        let parts = unhex::<20>(&recipe.parts).ok_or_else(refused)?;
        let mut design = core::CreatureDesign::from_bytes(&parts).ok_or_else(refused)?;
        if let Some(details) = &recipe.details {
            let bytes = unhex::<8>(details).ok_or_else(refused)?;
            design.details = core::DetailParts::from_bytes(&bytes).ok_or_else(refused)?;
            // Only details written the one way a recipe carries them, as a share code insists.
            if design.details.is_none() || design != design.bounded() {
                return Err(refused());
            }
        }
        Ok(design)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TravelHeadAppendages {
    pub style: HeadAppendageStyle,
    pub size: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TravelTail {
    pub style: TailStyle,
    pub length: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TravelFace {
    pub eye_shape: EyeShape,
    pub eye_size: u8,
    pub eye_spacing: u8,
    pub vertical_offset: i8,
    pub pupil: PupilStyle,
    pub highlight: HighlightStyle,
    pub brow: BrowStyle,
    pub mouth: MouthStyle,
    pub cheek: CheekStyle,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TravelForelimbs {
    pub style: ForelimbStyle,
    pub length: u8,
    pub thickness: u8,
    pub tip: LimbTipStyle,
    pub rest_pose: RestPose,
}

/// The resolved look of one companion, as drawn: its recipe when it has one, and every
/// proportion, part and colour choice the renderer reads.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TravelAppearance {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub design: Option<DesignRecipe>,
    pub family: BodyFamily,
    /// The size it is drawn at, in art pixels, with its stature and its share of an adult's size
    /// already applied.
    pub logical_size: u8,
    pub body_width: u8,
    pub body_height: u8,
    pub head_ratio: f32,
    pub roundness: f32,
    pub leg_length: u8,
    pub foot_size: u8,
    pub head_appendages: TravelHeadAppendages,
    pub tail: TravelTail,
    pub face: TravelFace,
    pub forelimbs: TravelForelimbs,
    pub effect_motif: EffectMotif,
    pub palette_index: u8,
    pub pattern: PatternKind,
    pub pattern_density: f32,
    /// 16 lowercase hex digits.
    pub marking_seed: String,
    pub gait_bob: f32,
    pub face_signature: u32,
}

impl From<&core::AppearanceGenome> for TravelAppearance {
    fn from(genome: &core::AppearanceGenome) -> Self {
        Self {
            design: genome.design.map(DesignRecipe::from),
            family: genome.family.into(),
            logical_size: genome.logical_size,
            body_width: genome.body_width,
            body_height: genome.body_height,
            head_ratio: genome.head_ratio,
            roundness: genome.roundness,
            leg_length: genome.leg_length,
            foot_size: genome.foot_size,
            head_appendages: TravelHeadAppendages {
                style: genome.head_appendages.style.into(),
                size: genome.head_appendages.size,
            },
            tail: TravelTail {
                style: genome.tail_style.into(),
                length: genome.tail_length,
            },
            face: TravelFace {
                eye_shape: genome.face.eye_shape.into(),
                eye_size: genome.face.eye_size,
                eye_spacing: genome.face.eye_spacing,
                vertical_offset: genome.face.vertical_offset,
                pupil: genome.face.pupil_style.into(),
                highlight: genome.face.highlight_style.into(),
                brow: genome.face.brow_style.into(),
                mouth: genome.face.mouth_style.into(),
                cheek: genome.face.cheek_style.into(),
            },
            forelimbs: TravelForelimbs {
                style: genome.forelimbs.style.into(),
                length: genome.forelimbs.length,
                thickness: genome.forelimbs.thickness,
                tip: genome.forelimbs.tip_style.into(),
                rest_pose: genome.forelimbs.rest_pose.into(),
            },
            effect_motif: genome.effect_motif.into(),
            palette_index: genome.palette_index,
            pattern: genome.pattern.into(),
            pattern_density: genome.pattern_density,
            marking_seed: hex(&genome.marking_seed.to_be_bytes()),
            gait_bob: genome.gait_bob,
            face_signature: genome.face_signature,
        }
    }
}

impl TravelAppearance {
    /// The genome the art crate draws from, exactly as Desktop held it.
    pub fn to_genome(&self) -> Result<core::AppearanceGenome, TravelError> {
        self.validate()?;
        Ok(core::AppearanceGenome {
            design: self
                .design
                .as_ref()
                .map(core::CreatureDesign::try_from)
                .transpose()?,
            family: self.family.into(),
            logical_size: self.logical_size,
            body_width: self.body_width,
            body_height: self.body_height,
            head_ratio: self.head_ratio,
            roundness: self.roundness,
            leg_length: self.leg_length,
            foot_size: self.foot_size,
            head_appendages: core::HeadAppendageGenome {
                style: self.head_appendages.style.into(),
                size: self.head_appendages.size,
            },
            tail_style: self.tail.style.into(),
            tail_length: self.tail.length,
            face: core::FaceGenome {
                eye_shape: self.face.eye_shape.into(),
                eye_size: self.face.eye_size,
                eye_spacing: self.face.eye_spacing,
                vertical_offset: self.face.vertical_offset,
                pupil_style: self.face.pupil.into(),
                highlight_style: self.face.highlight.into(),
                brow_style: self.face.brow.into(),
                mouth_style: self.face.mouth.into(),
                cheek_style: self.face.cheek.into(),
            },
            forelimbs: core::ForelimbGenome {
                style: self.forelimbs.style.into(),
                length: self.forelimbs.length,
                thickness: self.forelimbs.thickness,
                tip_style: self.forelimbs.tip.into(),
                rest_pose: self.forelimbs.rest_pose.into(),
            },
            effect_motif: self.effect_motif.into(),
            palette_index: self.palette_index,
            pattern: self.pattern.into(),
            pattern_density: self.pattern_density,
            marking_seed: unhex::<8>(&self.marking_seed)
                .map(u64::from_be_bytes)
                .ok_or_else(|| TravelError::invalid("a marking seed is 16 hex digits"))?,
            gait_bob: self.gait_bob,
            face_signature: self.face_signature,
        })
    }

    /// The bounds every look keeps: a recipe that decodes, a size the atlas can hold, and finite
    /// proportions.
    pub(crate) fn validate(&self) -> Result<(), TravelError> {
        if let Some(recipe) = &self.design {
            core::CreatureDesign::try_from(recipe)?;
        }
        // A frame is 48 art pixels square; nothing drawn into one is larger.
        let fits = |value: u8| value <= formiga_art::FRAME_SIZE as u8;
        if self.logical_size == 0
            || ![
                self.logical_size,
                self.body_width,
                self.body_height,
                self.leg_length,
                self.foot_size,
            ]
            .into_iter()
            .all(fits)
        {
            return Err(TravelError::invalid("a look larger than a frame"));
        }
        if ![
            self.head_ratio,
            self.roundness,
            self.pattern_density,
            self.gait_bob,
        ]
        .into_iter()
        .all(|value| value.is_finite() && (-4.0..=4.0).contains(&value))
        {
            return Err(TravelError::invalid("a proportion out of range"));
        }
        if unhex::<8>(&self.marking_seed).is_none() {
            return Err(TravelError::invalid("a marking seed is 16 hex digits"));
        }
        Ok(())
    }
}
