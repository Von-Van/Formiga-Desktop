//! The anatomy added in 0.69.1, on every body it fits: drawn in every view a companion is drawn
//! in, and inside its frame in every clip at the largest and the smallest size.
use super::*;
use formiga_core::{Anatomy, CreatureDesign, EarStyle, Edition, SeedStream};

/// A recipe of each plan with each part it fits, and the same recipe without it.
fn wearers() -> Vec<(Anatomy, AppearanceGenome, AppearanceGenome)> {
    let mut out = Vec::new();
    for index in 0..40_u64 {
        let seed = SeedStream::new([83; 32]).bytes("anatomy-wearers", index);
        let mut design = CreatureDesign::generated_by(Edition::Archetypes, seed, 0, None);
        design.body = formiga_core::BodyPlan::ALL[index as usize % 5];
        design.ears = [EarStyle::Pointed, EarStyle::Long, EarStyle::Tuft][index as usize % 3];
        design.classic = Default::default();
        let design = design.bounded();
        let part = Anatomy::ALL[index as usize / 8 % 5];
        if !part.fits(&design) {
            continue;
        }
        let worn = part.put_on(design, [250, 236, 210]);
        let mut bare = genome(BodyFamily::Blob);
        // The tail a brush or a plume brings, so only the part differs.
        bare.design = Some(CreatureDesign {
            tail: worn.tail,
            ..design
        });
        bare.logical_size = [32, 38, 44][index as usize % 3];
        let mut wearing = bare.clone();
        wearing.design = Some(worn);
        out.push((part, bare, wearing));
    }
    out
}

#[test]
fn every_part_shows_in_every_view_it_is_drawn_in() {
    let face = FaceRenderState {
        expression: ExpressionKind::Neutral,
        eyelids: EyelidPose::Open,
        gaze: GazeDirection::default(),
    };
    let wearers = wearers();
    for part in Anatomy::ALL {
        assert!(wearers.iter().any(|(p, _, _)| *p == part), "{part:?}");
    }
    for (part, bare, wearing) in &wearers {
        for view in [View::Front, View::Side, View::Back] {
            let a = CreatureRenderer::render_turned_frame(bare, view, None, true, face);
            let b = CreatureRenderer::render_turned_frame(wearing, view, None, true, face);
            let (Some(a), Some(b)) = (a, b) else {
                // A four-pawed body is drawn facing the screen only.
                assert!(view != View::Front);
                continue;
            };
            assert_ne!(
                a, b,
                "{part:?} does not show {view:?} on {:?}",
                wearing.design
            );
        }
    }
}

#[test]
fn every_part_stays_in_its_frame_in_every_clip() {
    let largest = formiga_core::size_for(formiga_core::STATURE_MAX, 100);
    for (part, _, wearing) in wearers() {
        for size in [largest, 24] {
            let mut g = wearing.clone();
            g.logical_size = size;
            for clip in BodyClip::baked() {
                for frame in 0..AnimationSpec::for_clip(clip).frames {
                    let rendered = CreatureRenderer::render_body_frame(&g, clip, frame, false);
                    let bounds = rendered.canvas.alpha_bounds().expect("a body is drawn");
                    assert!(
                        bounds.0 > 0
                            && bounds.1 > 0
                            && bounds.2 < FRAME_SIZE - 1
                            && bounds.3 < FRAME_SIZE - 1,
                        "{part:?} size {size} {clip:?} {frame}: {bounds:?}"
                    );
                }
            }
        }
    }
}
