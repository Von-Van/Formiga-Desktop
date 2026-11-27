//! Every companion that already exists draws pixel for pixel as it did when 0.69.0 shipped.
//!
//! New parts, new clips drawn for a body that had none, and pieces worn turned are added beside
//! what was drawn, never in place of it: an original, a recipe of every edition before them, a
//! mini, a sculpted form and the fox each hash exactly as they did, in every clip a creature is
//! baked with and every face. The digests were taken from 0.69.0 itself. What has changed on
//! purpose since is left out by name: the fox's walk and run turned side-on and away (it walked
//! facing the screen in every one), and a piece worn on a body drawn turned, which sat on it as
//! though it faced the screen.
use super::*;
use crate::{AccessoryArt, Palette};
use formiga_core::{Accessory, AccessoryKind, CreatureDesign, Edition, SeedStream};
use sha2::{Digest, Sha256};

/// The clips nothing changes for any of these companions.
fn unchanged_for(genome: &AppearanceGenome, dressed: bool) -> Vec<BodyClip> {
    BodyClip::baked()
        .filter(|clip| {
            let turned = matches!(
                clip,
                BodyClip::Gesture(Gesture::Stride | Gesture::Dash | Gesture::Depart)
            );
            let climbs = matches!(
                clip,
                BodyClip::Action(ActionKind::ClimbWindow)
                    | BodyClip::Gesture(Gesture::Mantle | Gesture::Scale | Gesture::Crest)
            );
            !(turned && genome.echo_fox) && !(dressed && (turned || climbs))
        })
        .collect()
}

fn digest_of(genome: &AppearanceGenome, dress: Option<AccessoryArt>, hash: &mut Sha256) {
    for clip in unchanged_for(genome, dress.is_some()) {
        for frame in 0..AnimationSpec::for_clip(clip).frames {
            for reduce_motion in [false, true] {
                if reduce_motion && frame > 0 {
                    continue;
                }
                let rendered = CreatureRenderer::render_dressed_body_frame(
                    genome,
                    dress,
                    clip,
                    frame,
                    reduce_motion,
                );
                hash.update(rendered.canvas.rgba_bytes());
                for point in [rendered.face_anchor, rendered.hands, rendered.mouth] {
                    hash.update(point.x.to_le_bytes());
                    hash.update(point.y.to_le_bytes());
                }
            }
        }
    }
    if dress.is_none() {
        for expression in ExpressionKind::ALL {
            for eyelids in [EyelidPose::Open, EyelidPose::Half, EyelidPose::Closed] {
                let face = CreatureRenderer::render_face_frame(
                    genome,
                    FaceRenderState {
                        expression,
                        eyelids,
                        gaze: GazeDirection::new(1, 0),
                    },
                );
                hash.update(face.rgba_bytes());
            }
        }
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Recipes from every generator before 0.69.1, at three statures, a mini of each, and the
/// originals they bridge to.
fn existing_companions() -> Vec<(String, AppearanceGenome)> {
    let mut out = Vec::new();
    for family in [
        BodyFamily::Blob,
        BodyFamily::Hopper,
        BodyFamily::SoftQuadruped,
    ] {
        out.push((format!("{family:?}"), genome(family)));
    }
    let mut fox = genome(BodyFamily::Hopper);
    fox.echo_fox = true;
    out.push(("fox".to_owned(), fox));
    for plan in [
        formiga_core::forms::Plan::CompactQuadruped,
        formiga_core::forms::Plan::Upright,
    ] {
        let mut sculpted = genome(BodyFamily::Blob);
        sculpted.sculpt = Some(formiga_core::forms::Sculpt::starter(plan));
        out.push((format!("{plan:?}"), sculpted));
    }
    for index in 0..18_u64 {
        let seed = SeedStream::new([71; 32]).bytes("pixel-identity", index);
        let edition =
            [Edition::Original, Edition::Archetypes, Edition::Details][index as usize % 3];
        let parent = CreatureDesign::generated_by(edition, seed, 0, None);
        let mini = CreatureDesign::generated_by(edition, seed, 1, Some(parent));
        for (label, design, size) in [
            ("adult", parent, [33, 38, 44][index as usize % 3]),
            ("mini", mini, 26),
        ] {
            let mut g = genome(BodyFamily::Blob);
            g.design = Some(design);
            g.logical_size = size;
            out.push((format!("{edition:?} {label} {index}"), g));
        }
    }
    out
}

#[test]
fn every_existing_companion_draws_exactly_as_it_did() {
    let mut bare = Sha256::new();
    let companions = existing_companions();
    for (label, genome) in &companions {
        bare.update(label.as_bytes());
        digest_of(genome, None, &mut bare);
    }
    let mut dressed = Sha256::new();
    let members: Vec<Palette> = companions
        .iter()
        .take(6)
        .map(|(_, genome)| crate::palette_for(genome))
        .collect();
    for (index, (label, genome)) in companions.iter().enumerate().step_by(3) {
        let kind = AccessoryKind::ALL[index % AccessoryKind::ALL.len()];
        let dress = AccessoryArt::resolve(Accessory::Worn(kind), [5; 32], &members);
        dressed.update(label.as_bytes());
        digest_of(genome, Some(dress), &mut dressed);
    }
    let bare = hex(&bare.finalize());
    let dressed = hex(&dressed.finalize());
    assert_eq!(
        (bare.as_str(), dressed.as_str()),
        (BARE_0_69_0, DRESSED_0_69_0),
        "an existing companion no longer draws as it did in 0.69.0"
    );
}

const BARE_0_69_0: &str = "c663dd85ae490a187b37a0d6756de79ef4fa1f298d8ac4a74c5d48e3d8bac9a7";
const DRESSED_0_69_0: &str = "608d085e167c4389239a7d4bb3bc93b71b1a9a471b5af5a66b5730d2ceec6835";
