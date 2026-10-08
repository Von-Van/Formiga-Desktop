//! What a companion looks like and its innate personality: the genomes it is generated with.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BodyFamily {
    Blob,
    Hopper,
    SoftQuadruped,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PatternKind {
    Solid,
    Patches,
    Spots,
    Stripes,
    Mask,
    Socks,
    Tips,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HeadAppendageStyle {
    None,
    Round,
    Pointed,
    Leaf,
    Droop,
    Antenna,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct HeadAppendageGenome {
    pub style: HeadAppendageStyle,
    pub size: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EyeShape {
    Round,
    Tall,
    SoftSquare,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PupilStyle {
    Dot,
    Wide,
    Spark,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HighlightStyle {
    Single,
    Double,
    Diagonal,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BrowStyle {
    None,
    Soft,
    Bold,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MouthStyle {
    Tiny,
    Smile,
    Cat,
    Beak,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CheekStyle {
    None,
    Dots,
    Blush,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FaceGenome {
    pub eye_shape: EyeShape,
    pub eye_size: u8,
    pub eye_spacing: u8,
    pub vertical_offset: i8,
    pub pupil_style: PupilStyle,
    pub highlight_style: HighlightStyle,
    pub brow_style: BrowStyle,
    pub mouth_style: MouthStyle,
    pub cheek_style: CheekStyle,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ForelimbStyle {
    SoftNub,
    Pseudopod,
    MittenArm,
    FrontPaw,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LimbTipStyle {
    Round,
    Mitten,
    Paw,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RestPose {
    AtSides,
    Folded,
    Together,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ForelimbGenome {
    pub style: ForelimbStyle,
    pub length: u8,
    pub thickness: u8,
    pub tip_style: LimbTipStyle,
    pub rest_pose: RestPose,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EffectMotif {
    None,
    Dot,
    Star,
    Heart,
    Leaf,
    Spark,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TailStyle {
    None,
    Stub,
    Taper,
    Tuft,
    Curl,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AppearanceGenome {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub design: Option<crate::CreatureDesign>,
    /// A form sculpted in Formiga Farm on one of the newer body plans, drawn in place of the
    /// recipe, which is kept as the companion plan nearest it for anything that cannot draw
    /// sculpts. Absent from the file for every creature drawn from its recipe or its genes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sculpt: Option<crate::forms::Sculpt>,
    pub family: BodyFamily,
    pub logical_size: u8,
    pub body_width: u8,
    pub body_height: u8,
    pub head_ratio: f32,
    pub roundness: f32,
    pub leg_length: u8,
    pub foot_size: u8,
    pub head_appendages: HeadAppendageGenome,
    pub tail_style: TailStyle,
    pub tail_length: u8,
    pub face: FaceGenome,
    pub forelimbs: ForelimbGenome,
    pub effect_motif: EffectMotif,
    pub palette_index: u8,
    pub pattern: PatternKind,
    pub pattern_density: f32,
    pub marking_seed: u64,
    pub gait_bob: f32,
    pub face_signature: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PersonalityGenome {
    pub activity: f32,
    pub curiosity: f32,
    pub boldness: f32,
    pub playfulness: f32,
    pub sociability: f32,
    pub routine_affinity: f32,
    pub sleep_timing: f32,
    pub window_tolerance: f32,
    pub cursor_interest: f32,
    pub decision_temperature: f32,
}
