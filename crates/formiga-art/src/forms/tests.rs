//! A sculpted form, held to what drawing a companion asks of it: it draws every clip Desktop
//! plays inside its frame, standing (or hovering) where it should, wearing a companion's faces
//! and whatever its owner chose; and a creature that keeps its body draws exactly as before.

use super::*;
use crate::{AccessoryArt, AnimationSpec, ExpressionKind, EyelidPose, GazeDirection, palette_for};
use formiga_core::forms::{Form, plain_face};
use formiga_core::{Accessory, AccessoryKind, DesktopSnapshot, World};
use time::OffsetDateTime;

fn face() -> FaceRenderState {
    FaceRenderState {
        expression: ExpressionKind::Content,
        eyelids: EyelidPose::Open,
        gaze: GazeDirection::new(0, 0),
    }
}

fn sculpted(plan: Plan) -> Design {
    Design {
        form: Form::Sculpted {
            sculpt: Sculpt::starter(plan),
        },
        face: plain_face(),
    }
}

/// A companion as Desktop makes one today, to draw a design over.
fn base() -> AppearanceGenome {
    World::preview_adult(
        [61; 32],
        OffsetDateTime::UNIX_EPOCH,
        &DesktopSnapshot::default(),
    )
    .appearance
}

fn clips() -> Vec<BodyClip> {
    BodyClip::baked().collect()
}

/// Every creature of a colony, and one from before recipes, draws pixel for pixel as it always
/// has, in every clip, both ways round: taking a creature's design and putting it back changes
/// nothing about how it looks.
#[test]
fn a_creature_that_keeps_its_body_draws_exactly_as_it_always_has() {
    let desktop = DesktopSnapshot::default();
    let world = World::new([3; 32], OffsetDateTime::UNIX_EPOCH, &desktop);
    let mut original = World::preview_adult([4; 32], OffsetDateTime::UNIX_EPOCH, &desktop);
    formiga_core::apply_creature_design(&mut original, None);
    for creature in world.save.creatures.iter().chain([&original]) {
        let genome = &creature.appearance;
        let design = Design::of(genome);
        assert_eq!(design.genome(genome), *genome, "{}", creature.name);
        for clip in clips() {
            for frame in [0, 1] {
                for facing_right in [true, false] {
                    assert_eq!(
                        DesignRenderer::frame(
                            &design,
                            genome,
                            clip,
                            frame,
                            facing_right,
                            false,
                            face()
                        ),
                        CreatureRenderer::render_composited_frame(
                            genome,
                            clip,
                            frame,
                            facing_right,
                            false,
                            face()
                        ),
                        "{} {clip:?} {frame}",
                        creature.name
                    );
                }
            }
        }
    }
}

/// A sculpted creature is drawn as its sculpt by the very renderer every companion is drawn by,
/// not as the recipe it keeps for readers that cannot draw sculpts.
#[test]
fn a_sculpted_creature_is_drawn_as_its_sculpt_wherever_a_creature_is_drawn() {
    let base = base();
    for plan in Plan::ALL {
        let genome = sculpted(plan).genome(&base);
        let recipe_only = AppearanceGenome {
            sculpt: None,
            ..genome.clone()
        };
        let drawn = CreatureRenderer::render_composited_frame(
            &genome,
            ActionKind::Idle,
            0,
            true,
            false,
            face(),
        );
        assert_ne!(
            drawn,
            CreatureRenderer::render_composited_frame(
                &recipe_only,
                ActionKind::Idle,
                0,
                true,
                false,
                face()
            ),
            "{plan:?} is drawn as its recipe"
        );
        assert_eq!(
            drawn,
            DesignRenderer::frame(
                &sculpted(plan),
                &base,
                ActionKind::Idle,
                0,
                true,
                false,
                face()
            )
        );
    }
}

/// Every plan draws every clip Desktop bakes, every frame of it, inside the frame with a pixel
/// to spare, as the atlas needs, and with whatever it is wearing.
#[test]
fn every_plan_draws_every_clip_inside_its_frame() {
    let base = base();
    let palettes = [palette_for(&base)];
    let dresses = [
        None,
        Some(AccessoryArt::resolve(
            Accessory::Worn(AccessoryKind::PartyHat),
            [5; 32],
            &palettes,
        )),
        Some(AccessoryArt::resolve(
            Accessory::Worn(AccessoryKind::SnailPack),
            [5; 32],
            &palettes,
        )),
    ];
    for plan in Plan::ALL {
        let genome = sculpted(plan).genome(&base);
        for dress in dresses {
            for clip in clips() {
                for frame in 0..AnimationSpec::for_clip(clip).frames {
                    let body = CreatureRenderer::render_dressed_body_frame(
                        &genome, dress, clip, frame, false,
                    );
                    let (left, top, right, bottom) = body
                        .canvas
                        .alpha_bounds()
                        .unwrap_or_else(|| panic!("{plan:?} {clip:?} {frame} draws nothing"));
                    assert!(
                        left >= 1
                            && top >= 1
                            && right <= FRAME_SIZE - 2
                            && bottom <= FRAME_SIZE - 2,
                        "{plan:?} {clip:?} {frame} reaches the edge: {left} {top} {right} {bottom}"
                    );
                    let face = body.face_anchor;
                    assert!(
                        (8..=40).contains(&face.x) && (8..=40).contains(&face.y),
                        "{plan:?} {clip:?} the face would leave the frame at {face:?}"
                    );
                }
            }
        }
    }
}

/// What a companion wears is drawn on a sculpted form too, so choosing it in the notebook shows.
#[test]
fn a_sculpted_form_wears_what_its_owner_chose() {
    let base = base();
    let hat = AccessoryArt::resolve(
        Accessory::Worn(AccessoryKind::LeafHat),
        [5; 32],
        &[palette_for(&base)],
    );
    for plan in Plan::ALL {
        let genome = sculpted(plan).genome(&base);
        let bare = CreatureRenderer::render_body_frame(&genome, ActionKind::Idle, 0, false);
        let dressed = CreatureRenderer::render_dressed_body_frame(
            &genome,
            Some(hat),
            ActionKind::Idle,
            0,
            false,
        );
        assert_ne!(bare.canvas, dressed.canvas, "{plan:?} shows no hat");
    }
}

/// A sculpted form wears a companion's faces, every expression of them, in its own eye colour.
#[test]
fn a_sculpted_face_keeps_every_expression_in_its_own_eye_colour() {
    let base = base();
    let mut design = sculpted(Plan::Upright);
    if let Form::Sculpted { sculpt } = &mut design.form {
        sculpt.coat.eyes = [0x2a, 0x6b, 0x3c];
    }
    let genome = design.genome(&base);
    let mut faces = Vec::new();
    for expression in ExpressionKind::ALL {
        let face = CreatureRenderer::render_face_frame(
            &genome,
            FaceRenderState {
                expression,
                ..face()
            },
        );
        assert!(
            face.alpha_bounds().is_some(),
            "{expression:?} draws no face"
        );
        faces.push(face);
    }
    faces.dedup();
    assert!(faces.len() > 1, "every expression draws the same face");
    let eyes = Rgba::new(0x2a, 0x6b, 0x3c, 255);
    let face = &faces[0];
    let mut found = false;
    for y in 0..face.height() as i32 {
        for x in 0..face.width() as i32 {
            let pixel = face.get(x, y);
            found |= (pixel.r, pixel.g, pixel.b) == (eyes.r, eyes.g, eyes.b);
        }
    }
    assert!(found, "the eyes are not in the form's own colour");
}

/// A whale does not walk on invisible legs: at rest and moving it hovers above the ground it
/// stands for, and it only comes down to rest.
#[test]
fn a_floater_hovers_and_never_walks_on_legs_it_does_not_have() {
    let base = base();
    let design = sculpted(Plan::Floater);
    let ground = (FRAME_SIZE - 1 - DesignRenderer::resting_baseline(&design, &base, false)) as i32;
    for action in [ActionKind::Idle, ActionKind::Traverse, ActionKind::Follow] {
        for frame in 0..AnimationSpec::for_action(action).frames {
            let body = DesignRenderer::body_frame(&design, &base, action, frame, false);
            let bottom = body.canvas.alpha_bounds().unwrap().3 as i32;
            assert!(
                bottom < ground - 1,
                "{action:?} {frame} touches down at {bottom}, ground {ground}"
            );
        }
    }
    let resting = DesignRenderer::body_frame(&design, &base, ActionKind::Sleep, 0, false);
    assert!(
        resting.canvas.alpha_bounds().unwrap().3 as i32 >= ground - 1,
        "it settles to rest"
    );
}

/// A form on legs stands with its feet on the ground row, so Desktop can stand it on a window or
/// the dock exactly as it stands a companion.
#[test]
fn walkers_stand_on_their_ground_row() {
    let base = base();
    for plan in [
        Plan::CompactQuadruped,
        Plan::LargeQuadruped,
        Plan::TallQuadruped,
        Plan::Upright,
        Plan::Crawler,
        Plan::Percher,
    ] {
        let design = sculpted(plan);
        let ground = FRAME_SIZE - 1 - DesignRenderer::resting_baseline(&design, &base, false);
        let body = DesignRenderer::body_frame(&design, &base, ActionKind::Idle, 0, false);
        let bottom = body.canvas.alpha_bounds().unwrap().3;
        assert!(
            bottom.abs_diff(ground) <= 1,
            "{plan:?}: feet at {bottom}, ground {ground}"
        );
    }
}

/// The same design always draws the same frame, and reduced motion holds every clip still.
#[test]
fn drawing_is_a_pure_function_of_the_design_and_reduced_motion_holds_still() {
    let base = base();
    for plan in Plan::ALL {
        let design = sculpted(plan);
        let a = DesignRenderer::frame(&design, &base, ActionKind::Traverse, 3, true, false, face());
        let b = DesignRenderer::frame(&design, &base, ActionKind::Traverse, 3, true, false, face());
        assert_eq!(a, b);
        let still: Vec<_> = (0..6)
            .map(|frame| {
                DesignRenderer::frame(
                    &design,
                    &base,
                    ActionKind::Traverse,
                    frame,
                    true,
                    true,
                    face(),
                )
            })
            .collect();
        assert!(
            still.windows(2).all(|pair| pair[0] == pair[1]),
            "{plan:?} moves under reduced motion"
        );
    }
}

/// Facing left is facing right in a mirror, face and all, as a companion is drawn.
#[test]
fn facing_left_is_the_mirror_of_facing_right() {
    let base = base();
    let design = sculpted(Plan::Percher);
    let right = DesignRenderer::frame(&design, &base, ActionKind::Idle, 0, true, false, face());
    let mut left = DesignRenderer::frame(&design, &base, ActionKind::Idle, 0, false, false, face());
    left.mirror_horizontal();
    assert_eq!(left, right);
}

/// Different plans make different silhouettes, not one body in different paint.
#[test]
fn every_plan_has_a_silhouette_of_its_own() {
    let base = base();
    let silhouettes: Vec<Vec<bool>> = Plan::ALL
        .into_iter()
        .map(|plan| {
            DesignRenderer::body_frame(&sculpted(plan), &base, ActionKind::Idle, 0, true)
                .alpha_mask
                .pixels
        })
        .collect();
    for (i, a) in silhouettes.iter().enumerate() {
        for (j, b) in silhouettes.iter().enumerate().skip(i + 1) {
            let differ = a.iter().zip(b).filter(|(x, y)| x != y).count();
            assert!(
                differ > 120,
                "{:?} and {:?} differ by only {differ} pixels",
                Plan::ALL[i],
                Plan::ALL[j]
            );
        }
    }
}
