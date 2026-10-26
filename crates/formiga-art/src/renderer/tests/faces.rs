//! Faces: every expression, layout, gaze and runtime face state keeps two readable eyes and
//! sits on its head.
use super::*;
use formiga_core::{DesktopRect, DesktopSnapshot, MonitorInfo, World};
use sha2::{Digest, Sha256};

#[test]
fn every_expression_keeps_two_readable_eyes_and_a_distinct_silhouette() {
    use std::collections::BTreeSet;

    for family in [
        BodyFamily::Blob,
        BodyFamily::Hopper,
        BodyFamily::SoftQuadruped,
    ] {
        let genome = genome(family);
        let palette = crate::palette_for(&genome);
        let mut hashes = BTreeSet::new();
        for expression in ExpressionKind::ALL {
            let face = CreatureRenderer::render_face_frame(
                &genome,
                FaceRenderState {
                    expression,
                    eyelids: EyelidPose::Open,
                    gaze: GazeDirection::default(),
                },
            );
            let left_eye = face.pixels().iter().enumerate().any(|(index, pixel)| {
                index as u32 % FACE_FRAME_SIZE < FACE_FRAME_SIZE / 2 && *pixel == palette.eye
            });
            let right_eye = face.pixels().iter().enumerate().any(|(index, pixel)| {
                index as u32 % FACE_FRAME_SIZE >= FACE_FRAME_SIZE / 2 && *pixel == palette.eye
            });
            assert!(left_eye && right_eye, "{family:?} {expression:?}");
            hashes.insert(Sha256::digest(face.rgba_bytes()).to_vec());
        }
        assert_eq!(hashes.len(), ExpressionKind::ALL.len(), "{family:?}");
    }
}

/// Masks and visors run the two eyes together, so this is where the two-eye grammar is most
/// at risk: every arrangement still shows an eye on each side in every expression, and no two
/// arrangements draw the same face.
#[test]
fn every_classic_face_keeps_two_readable_eyes_and_its_own_arrangement() {
    use std::collections::BTreeSet;

    let mut genome = World::preview_adult(
        [23; 32],
        time::OffsetDateTime::UNIX_EPOCH,
        &DesktopSnapshot::default(),
    )
    .appearance;
    let base = formiga_core::CreatureDesign::modular([23; 32], 0, None);
    let mut neutral = BTreeSet::new();
    for face in 0..=5 {
        genome.design = Some(formiga_core::CreatureDesign {
            classic: formiga_core::ClassicParts {
                face,
                ..Default::default()
            },
            ..base
        });
        let palette = crate::palette_for(&genome);
        for expression in ExpressionKind::ALL {
            for eyelids in EyelidPose::ALL {
                let rendered = CreatureRenderer::render_face_frame(
                    &genome,
                    FaceRenderState {
                        expression,
                        eyelids,
                        gaze: GazeDirection::default(),
                    },
                );
                let side = |left: bool| {
                    rendered.pixels().iter().enumerate().any(|(index, pixel)| {
                        (index as u32 % FACE_FRAME_SIZE < FACE_FRAME_SIZE / 2) == left
                            && *pixel == palette.eye
                    })
                };
                assert!(
                    side(true) && side(false),
                    "face {face} {expression:?} {eyelids:?}"
                );
                if expression == ExpressionKind::Neutral && eyelids == EyelidPose::Open {
                    neutral.insert(Sha256::digest(rendered.rgba_bytes()).to_vec());
                }
            }
        }
    }
    assert_eq!(neutral.len(), 6, "every eye arrangement is its own");
}

/// The authored faces a recipe drawn since 0.62.0 wears: every layout shows an eye on each side in
/// every expression, eyelid pose and gaze; no two layouts draw the same face, and no two
/// expressions the same face on one layout; and on every body plan, at every head and width, grown
/// up and as small as a mini is drawn, through every clip, every pixel any expression draws lands
/// on the body under it, so no eye, brow or mouth hangs off the edge of a head.
#[test]
fn every_face_layout_keeps_two_eyes_and_sits_on_its_head() {
    use std::collections::{BTreeMap, BTreeSet};

    let mut genome = World::preview_adult(
        [23; 32],
        time::OffsetDateTime::UNIX_EPOCH,
        &DesktopSnapshot::default(),
    )
    .appearance;
    let base = genome.design.expect("a new companion has a recipe");
    assert_ne!(base.face_template, 0);
    let mut neutral = BTreeSet::new();
    // Every pixel a layout draws in any expression, eyelid pose and gaze, on a blob — which wears
    // its layout snug — and on every other body.
    let mut drawn = BTreeMap::new();
    for template in 1..=formiga_core::FACE_TEMPLATES {
        for body in [formiga_core::BodyPlan::Round, formiga_core::BodyPlan::Blob] {
            genome.design = Some(formiga_core::CreatureDesign {
                face_template: template,
                body,
                ..base
            });
            genome.logical_size = 38;
            let palette = crate::palette_for(&genome);
            let mut pixels = vec![false; (FACE_FRAME_SIZE * FACE_FRAME_SIZE) as usize];
            let mut expressions = BTreeSet::new();
            for expression in ExpressionKind::ALL {
                for eyelids in EyelidPose::ALL {
                    for (gaze_x, gaze_y) in [(0, 0), (-1, -1), (1, 1), (1, -1), (-1, 1)] {
                        let rendered = CreatureRenderer::render_face_frame(
                            &genome,
                            FaceRenderState {
                                expression,
                                eyelids,
                                gaze: GazeDirection::new(gaze_x, gaze_y),
                            },
                        );
                        let side = |left: bool| {
                            rendered.pixels().iter().enumerate().any(|(index, pixel)| {
                                (index as u32 % FACE_FRAME_SIZE < FACE_FRAME_SIZE / 2) == left
                                    && *pixel == palette.eye
                            })
                        };
                        assert!(
                            side(true) && side(false),
                            "layout {template} on {body:?} {expression:?} {eyelids:?}"
                        );
                        for (drawn, pixel) in pixels.iter_mut().zip(rendered.pixels()) {
                            *drawn |= pixel.a > 0;
                        }
                        if eyelids == EyelidPose::Open && (gaze_x, gaze_y) == (0, 0) {
                            let hash = Sha256::digest(rendered.rgba_bytes()).to_vec();
                            if expression == ExpressionKind::Neutral
                                && body == formiga_core::BodyPlan::Round
                            {
                                neutral.insert(hash.clone());
                            }
                            expressions.insert(hash);
                        }
                    }
                }
            }
            assert_eq!(
                expressions.len(),
                ExpressionKind::ALL.len(),
                "layout {template} on {body:?}: every expression is its own"
            );
            drawn.insert((template, body == formiga_core::BodyPlan::Blob), pixels);
        }
    }
    assert_eq!(
        neutral.len(),
        usize::from(formiga_core::FACE_TEMPLATES),
        "every layout is its own"
    );
    let clips: Vec<BodyClip> = [ActionKind::Idle, ActionKind::Traverse, ActionKind::Sleep]
        .into_iter()
        .map(BodyClip::Action)
        .chain(Gesture::ALL.into_iter().map(BodyClip::Gesture))
        .collect();
    for body in formiga_core::BodyPlan::ALL {
        for head in 7..=9 {
            for width in [8, 10, 12] {
                for size in [18, 19, 21, 22, 24, 25, 28, 32, 34, 40, 44] {
                    genome.logical_size = size;
                    genome.design = Some(formiga_core::CreatureDesign {
                        body,
                        head,
                        width,
                        ..base
                    });
                    for &clip in &clips {
                        for frame in 0..AnimationSpec::for_clip(clip).frames {
                            let body_frame =
                                CreatureRenderer::render_body_frame(&genome, clip, frame, false);
                            let origin_x = body_frame.face_anchor.x - FACE_FRAME_SIZE as i32 / 2;
                            let origin_y = body_frame.face_anchor.y - FACE_FRAME_SIZE as i32 / 2;
                            for template in 1..=formiga_core::FACE_TEMPLATES {
                                let pixels =
                                    &drawn[&(template, body == formiga_core::BodyPlan::Blob)];
                                for (index, _) in pixels.iter().enumerate().filter(|(_, on)| **on) {
                                    let x = origin_x + (index as u32 % FACE_FRAME_SIZE) as i32;
                                    let y = origin_y + (index as u32 / FACE_FRAME_SIZE) as i32;
                                    let inside = (0..FRAME_SIZE as i32).contains(&x)
                                        && (0..FRAME_SIZE as i32).contains(&y);
                                    assert!(
                                        inside && body_frame.canvas.get(x, y).a > 0,
                                        "layout {template} on {body:?} head {head} width {width} size {size} {clip:?} frame {frame}: a face pixel at {x},{y} hangs off the body"
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn all_body_anchors_keep_the_layered_face_inside_the_sprite() {
    let half_face = FACE_FRAME_SIZE as i32 / 2;
    for family in [
        BodyFamily::Blob,
        BodyFamily::Hopper,
        BodyFamily::SoftQuadruped,
    ] {
        let genome = genome(family);
        let clips = ActionKind::ALL
            .into_iter()
            .map(BodyClip::Action)
            .chain(Gesture::ALL.into_iter().map(BodyClip::Gesture));
        for action in clips {
            let spec = AnimationSpec::for_clip(action);
            for frame in 0..spec.frames {
                let rendered = CreatureRenderer::render_body_frame(&genome, action, frame, false);
                assert!(
                    rendered.face_anchor.x - half_face >= 0,
                    "{family:?} {action:?}"
                );
                assert!(
                    rendered.face_anchor.y - half_face >= 0,
                    "{family:?} {action:?}"
                );
                assert!(
                    rendered.face_anchor.x + half_face < FRAME_SIZE as i32,
                    "{family:?} {action:?}"
                );
                assert!(
                    rendered.face_anchor.y + half_face < FRAME_SIZE as i32,
                    "{family:?} {action:?}"
                );
                assert_eq!(
                    rendered.alpha_mask.pixels.len(),
                    (FRAME_SIZE * FRAME_SIZE) as usize
                );
            }
        }
    }
}

#[test]
fn gaze_supports_all_nine_directions() {
    let genome = genome(BodyFamily::Blob);
    let mut hashes = std::collections::BTreeSet::new();
    for y in -1..=1 {
        for x in -1..=1 {
            let face = CreatureRenderer::render_face_frame(
                &genome,
                FaceRenderState {
                    expression: ExpressionKind::Neutral,
                    eyelids: EyelidPose::Open,
                    gaze: GazeDirection::new(x, y),
                },
            );
            hashes.insert(Sha256::digest(face.rgba_bytes()).to_vec());
        }
    }
    assert_eq!(hashes.len(), 9);
}

#[test]
fn runtime_face_state_combines_drives_activity_cursor_and_seeded_blinks() {
    let desktop = DesktopSnapshot {
        monitors: vec![MonitorInfo {
            id: 1,
            display_key: formiga_core::DisplayKey([2; 16]),
            bounds: DesktopRect {
                x: 0.0,
                y: 0.0,
                width: 800.0,
                height: 600.0,
            },
            usable_bounds: DesktopRect {
                x: 0.0,
                y: 24.0,
                width: 800.0,
                height: 536.0,
            },
            scale_factor: 1.0,
            primary: true,
        }],
        ..DesktopSnapshot::default()
    };
    let mut world = World::new([23; 32], time::OffsetDateTime::UNIX_EPOCH, &desktop);
    let creature = &mut world.save.creatures[0];
    creature.state.action = ActionKind::Idle;
    creature.state.drives.boredom = 0.9;
    creature.state.drives.comfort = 0.2;
    creature.state.drives.arousal = 0.1;
    let cursor = CursorSnapshot {
        position: formiga_core::Point {
            x: creature.state.position.x + 100.0,
            y: creature.state.position.y - 128.0,
        },
        available: true,
        ..CursorSnapshot::default()
    };
    let state = CreatureRenderer::resolve_face_state(creature, cursor, true);
    assert_eq!(state.expression, ExpressionKind::Bored);
    assert_eq!(state.gaze, GazeDirection::new(1, -1));

    let mut saw_blink = false;
    for step in 0..500 {
        creature.state.action_elapsed = step as f32 / 50.0;
        let first = CreatureRenderer::resolve_face_state(creature, cursor, true);
        let second = CreatureRenderer::resolve_face_state(creature, cursor, true);
        assert_eq!(first, second);
        saw_blink |= first.eyelids != EyelidPose::Open;
    }
    assert!(saw_blink);
}
