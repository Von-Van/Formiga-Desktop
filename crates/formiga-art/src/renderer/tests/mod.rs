//! The renderer's tests, one file per topic. `genome` is the plain, mid-sized creature of each
//! original family that most of them start from and vary one gene at a time.
use super::*;

mod belongings;
mod faces;
mod frames;
mod poses;

fn genome(family: BodyFamily) -> AppearanceGenome {
    AppearanceGenome {
        design: None,
        sculpt: None,
        family,
        logical_size: 38,
        body_width: 22,
        body_height: 18,
        head_ratio: 0.8,
        roundness: 0.8,
        leg_length: 6,
        foot_size: 3,
        head_appendages: formiga_core::HeadAppendageGenome {
            style: HeadAppendageStyle::Pointed,
            size: 5,
        },
        tail_style: TailStyle::Curl,
        tail_length: 8,
        face: formiga_core::FaceGenome {
            eye_shape: EyeShape::Round,
            eye_size: 1,
            eye_spacing: 5,
            vertical_offset: 0,
            pupil_style: PupilStyle::Dot,
            highlight_style: HighlightStyle::Single,
            brow_style: BrowStyle::Soft,
            mouth_style: MouthStyle::Smile,
            cheek_style: CheekStyle::Dots,
        },
        forelimbs: formiga_core::ForelimbGenome {
            style: match family {
                BodyFamily::Blob => ForelimbStyle::Pseudopod,
                BodyFamily::Hopper => ForelimbStyle::MittenArm,
                BodyFamily::SoftQuadruped => ForelimbStyle::FrontPaw,
            },
            length: 5,
            thickness: 1,
            tip_style: match family {
                BodyFamily::Blob => LimbTipStyle::Round,
                BodyFamily::Hopper => LimbTipStyle::Mitten,
                BodyFamily::SoftQuadruped => LimbTipStyle::Paw,
            },
            rest_pose: RestPose::AtSides,
        },
        effect_motif: EffectMotif::Spark,
        palette_index: 2,
        pattern: PatternKind::Spots,
        pattern_density: 0.5,
        marking_seed: 42,
        gait_bob: 0.6,
        face_signature: 7,
        echo_fox: false,
    }
}
