use super::*;
use formiga_core::{DesktopRect, DesktopSnapshot, MonitorInfo, World};
use sha2::{Digest, Sha256};

fn genome(family: BodyFamily) -> AppearanceGenome {
    AppearanceGenome {
        design: None,
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
    }
}

#[test]
fn every_family_renders_inside_frame() {
    for family in [
        BodyFamily::Blob,
        BodyFamily::Hopper,
        BodyFamily::SoftQuadruped,
    ] {
        for action in ActionKind::ALL {
            let atlas = CreatureRenderer::render_atlas(&genome(family), action, true);
            assert!(!atlas.frames.is_empty());
            for frame in atlas.frames {
                let bounds = frame.alpha_bounds().expect("creature is visible");
                assert!(
                    bounds.0 > 0
                        && bounds.1 > 0
                        && bounds.2 < FRAME_SIZE - 1
                        && bounds.3 < FRAME_SIZE - 1,
                    "{family:?} {action:?}: {bounds:?}"
                );
            }
        }
    }
}

#[test]
fn atlas_is_deterministic() {
    let first =
        CreatureRenderer::render_frame(&genome(BodyFamily::Blob), ActionKind::Idle, 0, true);
    let second =
        CreatureRenderer::render_frame(&genome(BodyFamily::Blob), ActionKind::Idle, 0, true);
    assert_eq!(
        Sha256::digest(first.rgba_bytes()),
        Sha256::digest(second.rgba_bytes())
    );
}

#[test]
fn generated_activity_props_are_deterministic_distinct_and_opaque() {
    let palette = crate::prop_palette(PALETTES[2], 17);
    let mut hashes = std::collections::BTreeSet::new();
    for variant in 0..TOY_KINDS {
        let mut first = Canvas::new(FRAME_SIZE, FRAME_SIZE);
        let mut second = Canvas::new(FRAME_SIZE, FRAME_SIZE);
        draw_generated_toy(&mut first, palette, EffectMotif::Spark, variant, 24, 24, 0);
        draw_generated_toy(&mut second, palette, EffectMotif::Spark, variant, 24, 24, 0);
        assert_eq!(first, second);
        // Every toy is big enough to be recognised, not a speck beside a paw.
        let opaque = first.pixels().iter().filter(|pixel| pixel.a > 0).count();
        assert!(opaque >= 40, "toy {variant} covers only {opaque} pixels");
        let (min_x, min_y, max_x, max_y) = first.alpha_bounds().expect("a toy is visible");
        assert!(
            max_x - min_x >= 6 && max_y - min_y >= 6,
            "toy {variant} is {}x{} and would vanish at 2x",
            max_x - min_x + 1,
            max_y - min_y + 1
        );
        hashes.insert(Sha256::digest(first.rgba_bytes()).to_vec());
    }
    assert_eq!(hashes.len(), usize::from(TOY_KINDS));

    // A toy that turns looks different as it turns.
    let turned = |spin| {
        let mut canvas = Canvas::new(FRAME_SIZE, FRAME_SIZE);
        draw_generated_toy(&mut canvas, palette, EffectMotif::Spark, 0, 24, 24, spin);
        canvas
    };
    assert_ne!(turned(0), turned(2));

    let mut snacks = std::collections::BTreeSet::new();
    for variant in 0..SNACK_KINDS {
        let mut snack = Canvas::new(FRAME_SIZE, FRAME_SIZE);
        draw_generated_snack(&mut snack, palette, variant, 24, 24, 0);
        assert!(snack.alpha_bounds().is_some());
        // A mouthful visibly goes out of it.
        let mut later = Canvas::new(FRAME_SIZE, FRAME_SIZE);
        draw_generated_snack(&mut later, palette, variant, 24, 24, 2);
        let count = |canvas: &Canvas| canvas.pixels().iter().filter(|p| p.a > 0).count();
        assert!(
            count(&later) < count(&snack),
            "snack {variant} is never actually eaten"
        );
        snacks.insert(Sha256::digest(snack.rgba_bytes()).to_vec());
    }
    assert_eq!(snacks.len(), usize::from(SNACK_KINDS));

    let mut cups = std::collections::BTreeSet::new();
    for variant in 0..DRINK_KINDS {
        let mut cup = Canvas::new(FRAME_SIZE, FRAME_SIZE);
        draw_generated_drinkware(&mut cup, palette, variant, 24, 24, false, 0);
        assert!(cup.alpha_bounds().is_some());
        let mut tipped = Canvas::new(FRAME_SIZE, FRAME_SIZE);
        draw_generated_drinkware(&mut tipped, palette, variant, 24, 24, true, 1);
        assert_ne!(cup, tipped, "cup {variant} never tips toward the mouth");
        cups.insert(Sha256::digest(cup.rgba_bytes()).to_vec());
    }
    assert_eq!(cups.len(), usize::from(DRINK_KINDS));
}

/// Exactly the pixels one belonging adds to a frame, drawn on their own so they can be
/// measured against the body that is using them.
fn prop_only(genome: &AppearanceGenome, action: ActionKind, frame: u8) -> (Canvas, PixelPoint) {
    let body = CreatureRenderer::render_body_frame(genome, action, frame, false);
    let mut canvas = Canvas::new(FRAME_SIZE, FRAME_SIZE);
    draw_activity_prop(
        &mut canvas,
        genome,
        crate::palette_for(genome),
        body.face_anchor,
        Pose::new(genome, BodyClip::Action(action), frame, false),
        action,
        frame,
        false,
    );
    (canvas, body.face_anchor)
}

/// A toy is something the creature is playing with, not something hanging near it: it stays in
/// the frame, keeps off the face, travels over the round, and comes back to the body.
#[test]
fn a_toy_is_carried_through_the_paws_clear_of_the_face_and_inside_the_frame() {
    for family in [
        BodyFamily::Blob,
        BodyFamily::Hopper,
        BodyFamily::SoftQuadruped,
    ] {
        let genome = genome(family);
        for action in [ActionKind::SoloPlay, ActionKind::Eat, ActionKind::Drink] {
            let mut places = std::collections::BTreeSet::new();
            let mut touching = 0;
            for frame in 0..4 {
                let (prop, anchor) = prop_only(&genome, action, frame);
                let bounds = prop
                    .alpha_bounds()
                    .unwrap_or_else(|| panic!("{family:?} {action:?} {frame} draws nothing"));
                assert!(
                    bounds.0 > 0
                        && bounds.1 > 0
                        && bounds.2 < FRAME_SIZE - 1
                        && bounds.3 < FRAME_SIZE - 1,
                    "{family:?} {action:?} {frame}: a prop at {bounds:?} leaves the frame"
                );
                // The eyes the layered face draws over this body: anything on top of them is
                // worn rather than held.
                for dx in -4..=4 {
                    for dy in -3..=0 {
                        assert_eq!(
                            prop.get(anchor.x + dx, anchor.y + dy).a,
                            0,
                            "{family:?} {action:?} {frame} draws a prop across the face"
                        );
                    }
                }
                places.insert((bounds.0, bounds.1));
                // Contact: the prop's own pixels sit against the body actually drawn.
                let body = CreatureRenderer::render_body_frame(&genome, action, frame, false);
                let close = (0..FRAME_SIZE as i32).any(|y| {
                    (0..FRAME_SIZE as i32).any(|x| {
                        prop.get(x, y).a > 0
                            && (-2..=2).any(|dx| {
                                (-2..=2).any(|dy| {
                                    prop.get(x + dx, y + dy).a == 0
                                        && body.canvas.get(x + dx, y + dy).a > 0
                                })
                            })
                    })
                });
                touching += usize::from(close);
            }
            assert!(
                touching >= 2,
                "{family:?} never touches what it is {action:?}-ing"
            );
            if action == ActionKind::SoloPlay {
                assert!(
                    places.len() >= 3,
                    "{family:?} holds its toy still instead of playing with it"
                );
            }
        }
    }
}

#[test]
fn generated_discoveries_have_sixteen_deterministic_opaque_silhouettes() {
    let genome = genome(BodyFamily::Blob);
    let mut hashes = std::collections::BTreeSet::new();
    for variant in 0..formiga_core::TRINKET_VARIANTS {
        let first = CreatureRenderer::render_trinket(&genome, variant);
        let second = CreatureRenderer::render_trinket(&genome, variant);
        assert_eq!(first, second);
        assert!(first.alpha_bounds().is_some(), "variant {variant} is empty");
        assert!(
            first
                .pixels()
                .iter()
                .filter(|pixel| pixel.a > 0)
                .all(|pixel| pixel.a == u8::MAX),
            "variant {variant} contains translucent runtime pixels"
        );
        hashes.insert(Sha256::digest(first.rgba_bytes()).to_vec());
    }
    assert_eq!(hashes.len(), usize::from(formiga_core::TRINKET_VARIANTS));
}

#[test]
fn ambient_animation_specs_and_shared_handhold_placement_are_exact() {
    for (action, fps) in [
        (ActionKind::ClimbWindow, 6),
        (ActionKind::Dangle, 3),
        (ActionKind::InspectScreen, 4),
        (ActionKind::PresentDiscovery, 2),
    ] {
        let spec = AnimationSpec::for_action(action);
        assert_eq!(spec.frames, 4);
        assert_eq!(spec.fps, fps);
    }
    let discovery = AnimationSpec::for_action(ActionKind::PresentDiscovery);
    assert_eq!(discovery.playback, PlaybackMode::Hold);
    assert_eq!(discovery.frame_at(20.0), 3);
    assert_eq!(
        AnimationSpec::body_action(ActionKind::Tossed),
        ActionKind::Dragged
    );
    assert_eq!(
        AnimationSpec::body_action(ActionKind::PetReaction),
        ActionKind::Greet
    );
    assert_eq!(
        FramePlacement::for_action(ActionKind::Dangle, 5).origin_y,
        -7
    );
    assert_eq!(
        FramePlacement::for_action(ActionKind::Idle, 5).origin_y,
        -43
    );
}

#[test]
fn reduced_motion_ambient_body_variants_are_static() {
    let genome = genome(BodyFamily::SoftQuadruped);
    for action in [
        ActionKind::ClimbWindow,
        ActionKind::Dangle,
        ActionKind::InspectScreen,
        ActionKind::PresentDiscovery,
    ] {
        let first = CreatureRenderer::render_body_frame(&genome, action, 0, true);
        for frame in 1..AnimationSpec::for_action(action).frames {
            assert_eq!(
                first,
                CreatureRenderer::render_body_frame(&genome, action, frame, true),
                "{action:?} frame {frame} moves in reduced-motion mode"
            );
        }
    }
}

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
fn movement_signatures_are_stable_individual_and_stay_inside_their_clips() {
    let desktop = DesktopSnapshot {
        monitors: vec![MonitorInfo {
            id: 1,
            display_key: formiga_core::DisplayKey([3; 16]),
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
    let world = World::new([77; 32], time::OffsetDateTime::UNIX_EPOCH, &desktop);
    let base = world.save.creatures[0].clone();
    let signature = MotionSignature::for_creature(&base);
    assert_eq!(signature, MotionSignature::for_creature(&base));

    let mut neighbor = base.clone();
    neighbor.id = base.id ^ 0x5151;
    assert_ne!(MotionSignature::for_creature(&neighbor), signature);

    // Every frame stays inside its clip, and a loop still visits all of its frames.
    let clips = ActionKind::ALL
        .into_iter()
        .map(BodyClip::Action)
        .chain(Gesture::ALL.into_iter().map(BodyClip::Gesture));
    for action in clips {
        let spec = AnimationSpec::for_clip(action);
        let mut seen = std::collections::BTreeSet::new();
        for step in 0..400 {
            let frame = signature.frame(action, step as f32 / 20.0);
            assert!(frame < spec.frames, "{action:?} frame {frame}");
            seen.insert(frame);
            if spec.playback == PlaybackMode::Hold {
                assert_eq!(frame, spec.frame_at(step as f32 / 20.0));
            }
        }
        if spec.playback == PlaybackMode::Loop {
            assert_eq!(seen.len(), usize::from(spec.frames), "{action:?}");
        }
    }

    // Liveliness and playfulness change cadence without touching stored appearance.
    let appearance = base.appearance.clone();
    let mut lively = base.clone();
    lively.personality.activity = 1.0;
    lively.personality.playfulness = 1.0;
    let mut still = base.clone();
    still.personality.activity = 0.0;
    still.personality.playfulness = 0.0;
    for action in [ActionKind::Traverse, ActionKind::Greet] {
        let (quick, slow) = (
            MotionSignature::for_creature(&lively),
            MotionSignature::for_creature(&still),
        );
        assert!(
            (0..160).any(|step| {
                let elapsed = step as f32 / 20.0;
                quick.frame(action, elapsed) != slow.frame(action, elapsed)
            }),
            "{action:?} should differ with temperament"
        );
    }
    assert_eq!(lively.appearance, appearance);
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

#[test]
fn reduced_motion_preserves_expression_while_softening_body_motion() {
    let genome = genome(BodyFamily::Hopper);
    let active = CreatureRenderer::render_body_frame(&genome, ActionKind::SoloPlay, 1, false);
    let reduced = CreatureRenderer::render_body_frame(&genome, ActionKind::SoloPlay, 1, true);
    assert_ne!(active.canvas, reduced.canvas);
    let state = FaceRenderState {
        expression: ExpressionKind::Joy,
        eyelids: EyelidPose::Open,
        gaze: GazeDirection::default(),
    };
    let joy = CreatureRenderer::render_face_frame(&genome, state);
    let neutral = CreatureRenderer::render_face_frame(
        &genome,
        FaceRenderState {
            expression: ExpressionKind::Neutral,
            ..state
        },
    );
    assert_ne!(joy, neutral);
}

#[test]
fn left_facing_is_exact_mirror() {
    let right =
        CreatureRenderer::render_frame(&genome(BodyFamily::Hopper), ActionKind::Traverse, 2, true);
    let mut expected = right.clone();
    expected.mirror_horizontal();
    let left =
        CreatureRenderer::render_frame(&genome(BodyFamily::Hopper), ActionKind::Traverse, 2, false);
    assert_eq!(expected, left);
}

#[test]
fn cat_gesture_paws_cap_upright_arm_length() {
    let mut compact = genome(BodyFamily::SoftQuadruped);
    compact.forelimbs.length = 4;
    let mut extreme = compact.clone();
    extreme.forelimbs.length = 7;
    for action in [
        ActionKind::Greet,
        ActionKind::SocialPlay,
        ActionKind::PresentDiscovery,
    ] {
        for frame in 0..AnimationSpec::for_action(action).frames {
            assert_eq!(
                CreatureRenderer::render_body_frame(&compact, action, frame, false).canvas,
                CreatureRenderer::render_body_frame(&extreme, action, frame, false).canvas,
                "{action:?} frame {frame}"
            );
        }
    }
}

#[test]
fn one_thousand_generated_genomes_render_every_action() {
    let desktop = DesktopSnapshot {
        monitors: vec![MonitorInfo {
            id: 1,
            display_key: formiga_core::DisplayKey([1; 16]),
            bounds: DesktopRect {
                x: 0.0,
                y: 0.0,
                width: 1440.0,
                height: 900.0,
            },
            usable_bounds: DesktopRect {
                x: 0.0,
                y: 24.0,
                width: 1440.0,
                height: 836.0,
            },
            scale_factor: 2.0,
            primary: true,
        }],
        ..DesktopSnapshot::default()
    };
    for index in 0_u64..1_000 {
        let mut seed = [0_u8; 32];
        seed.copy_from_slice(&Sha256::digest(index.to_le_bytes()));
        let world = World::new(seed, time::OffsetDateTime::UNIX_EPOCH, &desktop);
        let genome = &world.save.creatures[0].appearance;
        assert!(match genome.family {
            BodyFamily::Blob => matches!(
                genome.forelimbs.style,
                ForelimbStyle::SoftNub | ForelimbStyle::Pseudopod
            ),
            BodyFamily::Hopper => genome.forelimbs.style == ForelimbStyle::MittenArm,
            BodyFamily::SoftQuadruped => genome.forelimbs.style == ForelimbStyle::FrontPaw,
        });
        for action in ActionKind::ALL {
            let spec = AnimationSpec::for_action(action);
            let frame = (index as u8) % spec.frames;
            let rendered = CreatureRenderer::render_frame(genome, action, frame, true);
            let bounds = rendered
                .alpha_bounds()
                .expect("generated creature is visible");
            assert!(
                bounds.0 > 0
                    && bounds.1 > 0
                    && bounds.2 < FRAME_SIZE - 1
                    && bounds.3 < FRAME_SIZE - 1,
                "seed {index}, {action:?}: {bounds:?}"
            );
        }
        let palette = crate::palette_for(genome);
        for expression in ExpressionKind::ALL {
            let face = CreatureRenderer::render_face_frame(
                genome,
                FaceRenderState {
                    expression,
                    eyelids: EyelidPose::Open,
                    gaze: GazeDirection::default(),
                },
            );
            let left_eye = face
                .pixels()
                .iter()
                .enumerate()
                .any(|(pixel_index, pixel)| {
                    pixel_index as u32 % FACE_FRAME_SIZE < FACE_FRAME_SIZE / 2
                        && *pixel == palette.eye
                });
            let right_eye = face
                .pixels()
                .iter()
                .enumerate()
                .any(|(pixel_index, pixel)| {
                    pixel_index as u32 % FACE_FRAME_SIZE >= FACE_FRAME_SIZE / 2
                        && *pixel == palette.eye
                });
            assert!(
                left_eye && right_eye,
                "seed {index}, {expression:?} loses its two-eye grammar"
            );
        }
    }
}

/// The simulation crate cannot depend on this one, so `world::spacing` approximates where a
/// face and a body sit inside the 48x48 frame with two fixed boxes, both measured from the
/// frame's centre column: a face box 15 art pixels to either side, and a body box 23. Their
/// sum, 38, is why `FACE_CLEAR_RATIO` is rounded up from 38/48 of how wide a creature draws,
/// and twice the body box is why full separation is 46/48. The watching pose leans a long
/// body's head furthest, and is what sets the face box; every other clip stays within 14.
///
/// This is the evidence for those numbers. If a new body plan, size, or clip ever reached
/// further than the boxes, the simulation would space companions too closely and a face would
/// stay behind a body; this is what catches that here rather than on screen.
#[test]
fn every_face_and_body_stays_inside_the_boxes_the_simulation_spaces_by() {
    /// Half-width of the simulation's face box, in art pixels from the frame centre.
    const FACE_BOX_HALF: i32 = 15;
    /// Half-width of the simulation's body box, in art pixels from the frame centre.
    const BODY_BOX_HALF: i32 = 23;
    const CENTRE: i32 = FRAME_SIZE as i32 / 2;

    let desktop = DesktopSnapshot {
        monitors: vec![MonitorInfo {
            id: 1,
            display_key: formiga_core::DisplayKey([1; 16]),
            bounds: DesktopRect {
                x: 0.0,
                y: 0.0,
                width: 1440.0,
                height: 900.0,
            },
            usable_bounds: DesktopRect {
                x: 0.0,
                y: 24.0,
                width: 1440.0,
                height: 836.0,
            },
            scale_factor: 2.0,
            primary: true,
        }],
        ..DesktopSnapshot::default()
    };
    let mut creature = World::preview_adult([61; 32], time::OffsetDateTime::UNIX_EPOCH, &desktop);
    let base = formiga_core::CreatureDesign::modular([61; 32], 0, None);
    // Plain modular parts, then the classic parts that reach furthest: stick legs, a visor or
    // tall eyes, antennae or sprouts, and both classic tails.
    let classics = [
        formiga_core::ClassicParts::default(),
        formiga_core::ClassicParts {
            coat: 1,
            face: 2,
            limbs: 2,
            crown: 1,
            pattern: 3,
            tail: 1,
        },
        formiga_core::ClassicParts {
            coat: 1,
            face: 4,
            limbs: 1,
            crown: 2,
            pattern: 1,
            tail: 2,
        },
    ];
    for (plan, classic) in formiga_core::BodyPlan::ALL
        .into_iter()
        .flat_map(|plan| classics.map(|classic| (plan, classic)))
    {
        let mut design = base;
        design.body = plan;
        design.classic = classic;
        creature.appearance.design = Some(design);
        // An adult sits at the top of the range and the smallest mini at the bottom, so these
        // four values bracket every `logical_size` a colony can hold.
        for logical_size in [19_u8, 25, 34, 40] {
            creature.appearance.logical_size = logical_size;
            let genome = &creature.appearance;
            // How far the drawn face spreads from the middle of its own 16x16 tile, at its
            // widest across every expression, eyelid, and gaze.
            let mut face_reach = 0;
            for expression in ExpressionKind::ALL {
                for eyelids in EyelidPose::ALL {
                    for gaze in [
                        GazeDirection::default(),
                        GazeDirection { x: 1, y: 1 },
                        GazeDirection { x: -1, y: -1 },
                    ] {
                        let face = CreatureRenderer::render_face_frame(
                            genome,
                            FaceRenderState {
                                expression,
                                eyelids,
                                gaze,
                            },
                        );
                        let (min_x, _, max_x, _) =
                            face.alpha_bounds().expect("a face is never rendered empty");
                        face_reach = face_reach
                            .max(FACE_FRAME_SIZE as i32 / 2 - min_x as i32)
                            .max(max_x as i32 - FACE_FRAME_SIZE as i32 / 2);
                    }
                }
            }
            for clip in BodyClip::baked() {
                for frame in 0..AnimationSpec::for_clip(clip).frames {
                    let rendered = CreatureRenderer::render_body_frame(genome, clip, frame, false);
                    let (min_x, _, max_x, _) = rendered
                        .canvas
                        .alpha_bounds()
                        .expect("a body is never rendered empty");
                    let label =
                        format!("{plan:?} {classic:?} size {logical_size} {clip:?} frame {frame}");
                    // Mirroring sends the anchor to `FRAME_SIZE - anchor.x` and the silhouette
                    // to `FRAME_SIZE - 1 - x`, so both facings are measured together.
                    for anchor_x in [
                        rendered.face_anchor.x,
                        FRAME_SIZE as i32 - rendered.face_anchor.x,
                    ] {
                        let reach =
                            (CENTRE - (anchor_x - face_reach)).max(anchor_x + face_reach - CENTRE);
                        assert!(
                            reach <= FACE_BOX_HALF,
                            "{label}: a face reaches {reach} from the frame centre, past the \
                             {FACE_BOX_HALF} the simulation spaces by"
                        );
                    }
                    for body_x in [
                        min_x as i32,
                        max_x as i32,
                        FRAME_SIZE as i32 - 1 - max_x as i32,
                        FRAME_SIZE as i32 - 1 - min_x as i32,
                    ] {
                        let reach = (CENTRE - body_x).max(body_x - CENTRE);
                        assert!(
                            reach <= BODY_BOX_HALF,
                            "{label}: a body reaches {reach} from the frame centre, past the \
                             {BODY_BOX_HALF} the simulation spaces by"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn resting_baseline_seats_every_family_on_its_contact_point() {
    for family in [
        BodyFamily::Blob,
        BodyFamily::Hopper,
        BodyFamily::SoftQuadruped,
    ] {
        let genome = genome(family);
        let baseline = CreatureRenderer::resting_baseline(&genome, false);

        // Seating by the baseline must not push any resting pose through the surface.
        for action in [ActionKind::Idle, ActionKind::Perch, ActionKind::Homebound] {
            for frame in 0..AnimationSpec::for_action(action).frames {
                let canvas =
                    CreatureRenderer::render_body_frame(&genome, action, frame, false).canvas;
                let clearance = canvas
                    .alpha_bounds()
                    .map(|(_, _, _, max_y)| FRAME_SIZE - 1 - max_y)
                    .expect("resting pose draws pixels");
                assert!(
                    clearance >= baseline,
                    "{family:?} {action:?} frame {frame} would sink {} px below its surface",
                    baseline - clearance,
                );
            }
        }

        // And at least one resting pose has to land exactly on it, or the creature still
        // floats after seating.
        let seated = [ActionKind::Idle, ActionKind::Perch, ActionKind::Homebound]
            .into_iter()
            .flat_map(|action| {
                (0..AnimationSpec::for_action(action).frames).map(move |frame| (action, frame))
            })
            .any(|(action, frame)| {
                CreatureRenderer::render_body_frame(&genome, action, frame, false)
                    .canvas
                    .alpha_bounds()
                    .is_some_and(|(_, _, _, max_y)| FRAME_SIZE - 1 - max_y == baseline)
            });
        assert!(seated, "{family:?} never touches its contact point");
    }
}

const ALL_APPENDAGES: [HeadAppendageStyle; 6] = [
    HeadAppendageStyle::None,
    HeadAppendageStyle::Round,
    HeadAppendageStyle::Pointed,
    HeadAppendageStyle::Leaf,
    HeadAppendageStyle::Droop,
    HeadAppendageStyle::Antenna,
];

const ALL_TAILS: [TailStyle; 5] = [
    TailStyle::None,
    TailStyle::Stub,
    TailStyle::Taper,
    TailStyle::Tuft,
    TailStyle::Curl,
];

fn rest_pose() -> Pose {
    Pose::new(
        &genome(BodyFamily::Blob),
        BodyClip::Action(ActionKind::Idle),
        0,
        true,
    )
}

/// Opaque pixels above `row`, split into those left and right of `center_x`.
fn pixels_above(canvas: &Canvas, row: i32, center_x: i32) -> (usize, usize) {
    let mut left = 0;
    let mut right = 0;
    for y in 0..row.min(canvas.height() as i32) {
        for x in 0..canvas.width() as i32 {
            if canvas.get(x, y).a > 0 {
                if x < center_x {
                    left += 1;
                } else if x > center_x {
                    right += 1;
                }
            }
        }
    }
    (left, right)
}

#[test]
fn every_cat_genome_keeps_a_pair_of_ears_above_the_crown() {
    let palette = PALETTES[2];
    for style in ALL_APPENDAGES {
        let mut genome = genome(BodyFamily::SoftQuadruped);
        genome.head_appendages.style = style;
        for size in 2..=8 {
            let mut canvas = Canvas::new(FRAME_SIZE, FRAME_SIZE);
            draw_cat_ears(&mut canvas, palette, 24, 20, size, &genome, rest_pose());
            let (left, right) = pixels_above(&canvas, 20, 24);
            assert!(
                left > 0 && right > 0,
                "{style:?} size {size} draws {left}/{right} ear pixels above the crown",
            );
            let (min_x, min_y, max_x, max_y) = canvas.alpha_bounds().expect("ears are visible");
            assert!(
                min_x >= 1 && min_y >= 1 && max_x < FRAME_SIZE - 1 && max_y < FRAME_SIZE - 1,
                "{style:?} size {size} ears leave the frame margin",
            );
        }
    }
}

#[test]
fn every_rabbit_genome_keeps_long_ears_inside_the_frame() {
    let palette = PALETTES[2];
    for style in ALL_APPENDAGES {
        let mut genome = genome(BodyFamily::Hopper);
        genome.head_appendages.style = style;
        for size in 2..=8 {
            let mut canvas = Canvas::new(FRAME_SIZE, FRAME_SIZE);
            draw_rabbit_ears(&mut canvas, palette, 24, 20, size, &genome, rest_pose());
            let (min_x, min_y, max_x, max_y) = canvas.alpha_bounds().expect("ears are visible");
            assert!(
                min_x >= 1 && min_y >= 1 && max_x < FRAME_SIZE - 1 && max_y < FRAME_SIZE - 1,
                "{style:?} size {size} ears leave the frame margin",
            );
            if style == HeadAppendageStyle::Droop {
                // A lop set hangs beside the head instead of standing up.
                assert!(max_y as i32 > 20, "{style:?} size {size} does not lop");
                continue;
            }
            let (left, right) = pixels_above(&canvas, 20, 24);
            assert!(
                left > 0 && right > 0,
                "{style:?} size {size} draws {left}/{right} ear pixels above the head",
            );
            let reach = 20 - min_y as i32;
            assert!(
                reach >= 5,
                "{style:?} size {size} ears only reach {reach} px, too short to read as a rabbit",
            );
        }
    }
}

#[test]
fn every_cat_tail_is_carried_above_the_rump() {
    let palette = PALETTES[2];
    for style in ALL_TAILS {
        let mut genome = genome(BodyFamily::SoftQuadruped);
        genome.tail_style = style;
        let mut canvas = Canvas::new(FRAME_SIZE, FRAME_SIZE);
        draw_tail(&mut canvas, &genome, palette, 14, 26, 1.0, rest_pose());
        let (_, min_y, _, _) = canvas.alpha_bounds().expect("every cat carries a tail");
        assert!(
            (26 - min_y as i32) >= 4,
            "{style:?} tail rises only {} px off the rump",
            26 - min_y as i32,
        );
    }
}

#[test]
fn every_rabbit_tail_puffs_behind_the_rump() {
    let palette = PALETTES[2];
    for style in ALL_TAILS {
        let mut genome = genome(BodyFamily::Hopper);
        genome.tail_style = style;
        let mut canvas = Canvas::new(FRAME_SIZE, FRAME_SIZE);
        draw_tail(&mut canvas, &genome, palette, 14, 26, 1.0, rest_pose());
        let (min_x, _, _, _) = canvas.alpha_bounds().expect("every rabbit has a puff");
        assert!(min_x >= 1, "{style:?} puff leaves the frame margin");
        // The body ellipse is painted over everything from its left outline edge inwards, so
        // the puff only reads if it clears that edge by a few columns.
        let clear = (0..13)
            .filter(|x| (0..FRAME_SIZE as i32).any(|y| canvas.get(*x, y).a > 0))
            .count();
        assert!(
            clear >= 3,
            "{style:?} puff leaves only {clear} columns showing behind the rump",
        );
    }
}

#[test]
fn a_resting_cat_plants_all_four_paws_on_its_contact_row() {
    let genome = genome(BodyFamily::SoftQuadruped);
    let walking = CreatureRenderer::render_body_frame(&genome, ActionKind::Traverse, 0, true)
        .canvas
        .alpha_bounds()
        .expect("a walking cat is visible")
        .3;
    for action in [
        ActionKind::Idle,
        ActionKind::Eat,
        ActionKind::Drink,
        ActionKind::ReactToWindow,
        ActionKind::InspectScreen,
    ] {
        let resting = CreatureRenderer::render_body_frame(&genome, action, 0, true)
            .canvas
            .alpha_bounds()
            .expect("a resting cat is visible")
            .3;
        assert_eq!(
            resting, walking,
            "{action:?} does not stand on its legs the way walking does",
        );
    }
}
/// A carried thing rides in front of whoever is holding it. The anchor is one explicit
/// contract shared by the overlay and the review sheets, so a prop changing hands reads as a
/// hand-off rather than two objects swapping places in the air.
#[test]
fn a_carried_prop_rides_on_the_side_its_holder_is_facing_and_stays_inside_the_frame() {
    let facing = PropAnchor::facing(true);
    let away = PropAnchor::facing(false);
    assert_eq!(facing.dx, -away.dx, "the anchor mirrors with facing");
    assert_eq!(
        facing.dy, away.dy,
        "and rides at the same height either way"
    );
    assert!(facing.dx > 0.0, "a held thing is in front, not behind");
    assert!(
        facing.dy > 0.0,
        "and carried at the chest, not floated above the head"
    );
    // The prop is drawn from the face anchor, in a face-sized quad. Both offsets have to keep
    // that quad inside the body frame, or a prop would be clipped differently from its holder.
    let margin = (FRAME_SIZE - FACE_FRAME_SIZE) as f32 / 2.0;
    for anchor in [facing, away] {
        assert!(
            anchor.dx.abs() <= margin,
            "{anchor:?} leaves the frame sideways"
        );
        assert!(
            anchor.dy.abs() <= margin,
            "{anchor:?} leaves the frame vertically"
        );
    }
    // Two creatures facing each other reach toward one another, which is what makes a
    // hand-off read: the gap between their anchors is smaller than the gap between them.
    let mut creature = World::preview_adult(
        [23; 32],
        time::OffsetDateTime::UNIX_EPOCH,
        &DesktopSnapshot::default(),
    );
    creature.state.facing_right = true;
    let giver = PropAnchor::for_creature(&creature);
    creature.state.facing_right = false;
    let taker = PropAnchor::for_creature(&creature);
    assert!(giver.dx > taker.dx);
}
#[test]
fn every_gesture_is_a_distinct_looping_pose_on_every_body() {
    let preview = World::preview_adult(
        [29; 32],
        time::OffsetDateTime::UNIX_EPOCH,
        &DesktopSnapshot::default(),
    );
    let mut bodies: Vec<(String, AppearanceGenome)> = [
        BodyFamily::Blob,
        BodyFamily::Hopper,
        BodyFamily::SoftQuadruped,
    ]
    .into_iter()
    .map(|family| (format!("{family:?}"), genome(family)))
    .collect();
    for limbs in 0..=2 {
        for plan in formiga_core::BodyPlan::ALL {
            let mut appearance = preview.appearance.clone();
            let mut design = formiga_core::CreatureDesign::modular([29; 32], 0, None);
            design.body = plan;
            design.classic = formiga_core::ClassicParts {
                limbs,
                ..Default::default()
            };
            appearance.design = Some(design);
            bodies.push((format!("{plan:?} limbs {limbs}"), appearance));
        }
    }
    for (body, genome) in &bodies {
        // The pose a gesture most often stands in for is the plain inspecting one.
        let inspecting =
            CreatureRenderer::render_body_frame(genome, ActionKind::InspectScreen, 0, false);
        let mut poses: Vec<(Gesture, Canvas)> = Vec::new();
        for gesture in Gesture::ALL {
            let spec = AnimationSpec::for_clip(gesture);
            // A scene holds a pose for as long as the moment lasts. A habit's stretch is done
            // once and held at the top until the nap it opens takes over.
            let playback = if gesture.in_scenes() {
                PlaybackMode::Loop
            } else {
                PlaybackMode::Hold
            };
            assert_eq!(spec.playback, playback, "{gesture:?}");
            assert!(spec.frames >= 2, "{gesture:?} is a pose that moves");
            let frames: Vec<_> = (0..spec.frames)
                .map(|frame| CreatureRenderer::render_body_frame(genome, gesture, frame, false))
                .collect();
            for rendered in &frames {
                let (min_x, min_y, max_x, max_y) = rendered
                    .canvas
                    .alpha_bounds()
                    .expect("a gesture is visible");
                assert!(
                    min_x > 0 && min_y > 0 && max_x < FRAME_SIZE - 1 && max_y < FRAME_SIZE - 1,
                    "{body} {gesture:?} leaves the frame"
                );
            }
            assert!(
                frames
                    .windows(2)
                    .any(|pair| pair[0].canvas != pair[1].canvas),
                "{body} {gesture:?} never moves"
            );
            assert_ne!(
                frames[0].canvas, inspecting.canvas,
                "{body} {gesture:?} looks like plain inspecting"
            );
            for (other, canvas) in &poses {
                assert_ne!(
                    &frames[0].canvas, canvas,
                    "{body}: {gesture:?} and {other:?} share a pose"
                );
            }
            poses.push((gesture, frames[0].canvas.clone()));
        }
    }
}

/// Resting is the clip a companion plays more than any other, so it carries more than a bob.
/// The loop settles, shifts its weight onto one foot, looks off to the side with its ears up,
/// and settles back; which side it settles onto and whether it slumps into the shift are read
/// from the creature's own appearance, so a colony sitting about is not a row of companions
/// doing the same nothing in step.
///
/// The plain square screen-facing settle a companion always held is still the first frame of
/// the loop, and it is the only frame reduced motion draws, so nothing was taken away to make
/// room for the rest of it.
#[test]
fn resting_moves_through_its_own_postures_and_no_two_companions_rest_alike() {
    let spec = AnimationSpec::for_action(ActionKind::Idle);
    assert_eq!(spec.playback, PlaybackMode::Loop);
    // Six is what the creature atlas had left in its last row, so the loop the rest grew
    // into costs no texture; `layered_atlas_matches_the_baked_budget_per_creature` is where
    // that is held to.
    assert_eq!(spec.frames, 6);
    // A still colony presents at the frame rate of whatever it is resting in, so a longer
    // rest must not also be a faster one than the four-a-second clip it replaced.
    assert!(
        spec.fps <= 4,
        "a longer rest redraws more often than it did"
    );

    let preview = World::preview_adult(
        [29; 32],
        time::OffsetDateTime::UNIX_EPOCH,
        &DesktopSnapshot::default(),
    );
    let mut bodies: Vec<(String, AppearanceGenome)> = [
        BodyFamily::Blob,
        BodyFamily::Hopper,
        BodyFamily::SoftQuadruped,
    ]
    .into_iter()
    .map(|family| (format!("{family:?}"), genome(family)))
    .collect();
    for plan in formiga_core::BodyPlan::ALL {
        let mut appearance = preview.appearance.clone();
        let mut design = formiga_core::CreatureDesign::modular([29; 32], 0, None);
        design.body = plan;
        appearance.design = Some(design);
        bodies.push((format!("{plan:?}"), appearance));
    }

    for (body, appearance) in &bodies {
        let frames: Vec<_> = (0..spec.frames)
            .map(|frame| {
                CreatureRenderer::render_body_frame(appearance, ActionKind::Idle, frame, false)
            })
            .collect();
        let mut postures: Vec<&Canvas> = Vec::new();
        for rendered in &frames {
            if !postures.iter().any(|drawn| **drawn == rendered.canvas) {
                postures.push(&rendered.canvas);
            }
        }
        assert!(
            postures.len() >= 4,
            "{body} rests in {} pictures, which is a bob rather than a loop of postures",
            postures.len()
        );
        let still = CreatureRenderer::render_body_frame(appearance, ActionKind::Idle, 0, true);
        for frame in 1..spec.frames {
            assert_eq!(
                CreatureRenderer::render_body_frame(appearance, ActionKind::Idle, frame, true)
                    .canvas,
                still.canvas,
                "{body} moves while resting under reduced motion"
            );
        }
    }

    // The two appearance bytes a rest is read from, set so one companion settles to each side.
    let mut settles_left = bodies[0].1.clone();
    settles_left.marking_seed = 0;
    settles_left.face_signature = 0;
    let mut settles_right = settles_left.clone();
    settles_right.face_signature = 1;
    assert_eq!(resting_manner(&settles_left).0, -1);
    assert_eq!(resting_manner(&settles_right).0, 1);
    let drawn = |appearance: &AppearanceGenome, frame: u8| {
        CreatureRenderer::render_body_frame(appearance, ActionKind::Idle, frame, false).canvas
    };
    assert_eq!(
        drawn(&settles_left, 0),
        drawn(&settles_right, 0),
        "a settled companion is a settled companion, whichever way it rests"
    );
    assert!(
        (1..spec.frames).any(|frame| drawn(&settles_left, frame) != drawn(&settles_right, frame)),
        "two companions come out of the settle the same way"
    );

    // Every way of settling is reached by companions a colony actually generates.
    let manners: std::collections::BTreeSet<_> = (0..32_u8)
        .map(|seed| {
            let creature = World::preview_adult(
                [seed; 32],
                time::OffsetDateTime::UNIX_EPOCH,
                &DesktopSnapshot::default(),
            );
            resting_manner(&creature.appearance)
        })
        .collect();
    assert_eq!(
        manners.len(),
        4,
        "a way of settling nobody is ever born with"
    );
}

#[test]
fn a_body_shows_a_gesture_only_while_its_attention_carries_one() {
    let mut creature = World::preview_adult(
        [31; 32],
        time::OffsetDateTime::UNIX_EPOCH,
        &DesktopSnapshot::default(),
    );
    creature.state.action = ActionKind::InspectScreen;
    creature.state.attention = None;
    assert_eq!(
        BodyClip::for_creature(&creature),
        BodyClip::Action(ActionKind::InspectScreen)
    );
    let mut pose = formiga_core::AttentionPose {
        target: creature.state.position,
        emotion: formiga_core::AttentionEmotion::Curious,
        hanging: 0.0,
        gesture: None,
    };
    creature.state.attention = Some(pose);
    assert_eq!(
        BodyClip::for_creature(&creature),
        BodyClip::Action(ActionKind::InspectScreen)
    );
    let open = CreatureRenderer::resolve_face_state(&creature, CursorSnapshot::default(), false);
    for gesture in Gesture::ALL {
        pose.gesture = Some(gesture);
        creature.state.attention = Some(pose);
        assert_eq!(
            BodyClip::for_creature(&creature),
            BodyClip::Gesture(gesture)
        );
        let face =
            CreatureRenderer::resolve_face_state(&creature, CursorSnapshot::default(), false);
        // Covering the face shuts the eyes behind the paws; nothing else changes the face.
        if gesture == Gesture::Cover {
            assert_eq!(face.eyelids, EyelidPose::Closed);
        } else {
            assert_eq!(face, open, "{gesture:?}");
        }
    }
    // Actions keep their own clips: no action is folded into a gesture or the other way.
    let baked: Vec<_> = BodyClip::baked().collect();
    assert_eq!(
        baked.len(),
        ActionKind::BODY_CLIPS.len() + Gesture::ALL.len()
    );
    for clip in &baked {
        assert_eq!(clip.body(), *clip, "{clip:?} is baked under its own name");
    }
}

fn celebrating(celebration: formiga_core::Celebration) -> Creature {
    let mut creature = World::preview_adult(
        [37; 32],
        time::OffsetDateTime::UNIX_EPOCH,
        &DesktopSnapshot::default(),
    );
    for salt in 0..=u8::MAX {
        creature.behavior_seed[31] = salt;
        if formiga_core::Celebration::for_creature(&creature) == celebration {
            return creature;
        }
    }
    unreachable!("every celebration turns up within 256 seeds")
}

/// A scene asks for a cheer; each creature answers it its own way. Nothing else a scene asks
/// for is changed.
#[test]
fn a_cheer_is_the_creatures_own_celebration() {
    use formiga_core::Celebration;
    for celebration in Celebration::ALL {
        let mut creature = celebrating(celebration);
        creature.state.action = ActionKind::InspectScreen;
        creature.state.facing_right = true;
        let mut pose = formiga_core::AttentionPose {
            target: creature.state.position,
            emotion: formiga_core::AttentionEmotion::Enjoying,
            hanging: 0.0,
            gesture: Some(Gesture::Cheer),
        };
        creature.state.attention = Some(pose);
        let facings: Vec<bool> = [0.1, 0.35, 0.6, 0.85]
            .into_iter()
            .map(|at| {
                creature.state.action_elapsed = at;
                let body = BodyPresentation::for_creature(&creature);
                assert_eq!(body.clip, BodyClip::Gesture(celebration.gesture()));
                body.facing_right
            })
            .collect();
        if celebration == Celebration::Twirl {
            assert_eq!(facings, [true, false, true, false], "a twirl turns");
        } else {
            assert!(facings.iter().all(|facing| *facing), "{celebration:?}");
        }
        pose.gesture = Some(Gesture::Gasp);
        creature.state.attention = Some(pose);
        let body = BodyPresentation::for_creature(&creature);
        assert_eq!(body.clip, BodyClip::Gesture(Gesture::Gasp));
        assert!(body.facing_right);
    }
}

/// A habit is drawn while it is being done and not a moment longer, and never under
/// something the creature's attention is on.
#[test]
fn a_habit_is_drawn_only_while_it_is_being_done() {
    use formiga_core::{Flourish, Habit};
    let mut creature = World::preview_adult(
        [41; 32],
        time::OffsetDateTime::UNIX_EPOCH,
        &DesktopSnapshot::default(),
    );
    creature.state.facing_right = true;
    creature.state.attention = None;
    let doing = |creature: &mut Creature, habit: Habit, action: ActionKind, at: f32| {
        creature.state.action = action;
        creature.state.action_elapsed = at;
        creature.state.flourish = Some(Flourish {
            habit,
            action,
            started_at: Some(0.0),
        });
        BodyPresentation::for_creature(creature)
    };
    // A stretch is played once from its first frame and held at the top.
    let stretch = |creature: &mut Creature, at| {
        doing(creature, Habit::StretchesBeforeNaps, ActionKind::Sleep, at)
    };
    assert_eq!(
        stretch(&mut creature, 0.1).clip,
        BodyClip::Gesture(Gesture::Stretch)
    );
    assert_eq!(stretch(&mut creature, 0.1).frame, 0);
    assert_eq!(stretch(&mut creature, 1.2).frame, 3);
    assert_eq!(stretch(&mut creature, 1.45).frame, 3);
    let face = CreatureRenderer::resolve_face_state(&creature, CursorSnapshot::default(), false);
    assert_eq!(face.eyelids, EyelidPose::Closed, "eyes shut at the top");
    // Then the nap it opened.
    assert_eq!(
        stretch(&mut creature, 1.6).clip,
        BodyClip::Action(ActionKind::Sleep)
    );
    // Turning round steps on the spot and faces each way in turn, eyes open until it lies
    // down.
    let turns: Vec<_> = [0.1, 0.5, 0.9, 1.3]
        .into_iter()
        .map(|at| {
            doing(
                &mut creature,
                Habit::CirclesBeforeNaps,
                ActionKind::Sleep,
                at,
            )
        })
        .collect();
    assert!(
        turns
            .iter()
            .all(|body| body.clip == BodyClip::Action(ActionKind::Traverse))
    );
    assert_eq!(
        turns
            .iter()
            .map(|body| body.facing_right)
            .collect::<Vec<_>>(),
        [true, false, true, false]
    );
    let face = CreatureRenderer::resolve_face_state(&creature, CursorSnapshot::default(), false);
    assert_ne!(face.eyelids, EyelidPose::Closed);
    assert_ne!(face.expression, ExpressionKind::Sleepy);
    // Looking a snack over holds it up in front, with an eye on it.
    for at in [0.1, 0.7, 1.4] {
        let body = doing(&mut creature, Habit::LooksFoodOver, ActionKind::Eat, at);
        assert_eq!(body.clip, BodyClip::Action(ActionKind::Eat));
        assert_eq!(body.frame, 0);
    }
    doing(&mut creature, Habit::LooksFoodOver, ActionKind::Drink, 0.2);
    let face = CreatureRenderer::resolve_face_state(&creature, CursorSnapshot::default(), false);
    assert_eq!(face.expression, ExpressionKind::Curious);
    assert_eq!(face.gaze.y, 1, "looking down at it");
    assert_eq!(
        doing(&mut creature, Habit::WavesHello, ActionKind::Greet, 0.2).clip,
        BodyClip::Gesture(Gesture::Reach)
    );
    assert_eq!(
        doing(&mut creature, Habit::PlayBows, ActionKind::SocialPlay, 0.2).clip,
        BodyClip::Gesture(Gesture::Crouch)
    );
    // Something the creature is paying attention to comes first.
    doing(&mut creature, Habit::PlayBows, ActionKind::SocialPlay, 0.2);
    creature.state.attention = Some(formiga_core::AttentionPose {
        target: creature.state.position,
        emotion: formiga_core::AttentionEmotion::Curious,
        hanging: 0.0,
        gesture: None,
    });
    assert_eq!(
        BodyPresentation::for_creature(&creature).clip,
        BodyClip::Action(ActionKind::SocialPlay)
    );
}

/// A companion on its way to bed walks there with its eyelids heavy, and lies down with its
/// eyes shut only once it has arrived, rather than being drawn asleep while it crosses the
/// floor.
#[test]
fn a_companion_walks_to_bed_and_lies_down_when_it_gets_there() {
    let mut creature = formiga_core::World::preview_adult(
        [23; 32],
        time::OffsetDateTime::UNIX_EPOCH,
        &formiga_core::DesktopSnapshot::default(),
    );
    creature.state.action = ActionKind::Sleep;
    creature.state.action_elapsed = 1.5;
    creature.state.attention = None;
    creature.state.flourish = None;
    creature.state.velocity = formiga_core::Point { x: 34.0, y: 0.0 };
    let walking = BodyPresentation::for_creature(&creature);
    assert_eq!(walking.clip, BodyClip::Action(ActionKind::Traverse));
    let face = CreatureRenderer::resolve_face_state(&creature, CursorSnapshot::default(), false);
    assert_eq!(face.eyelids, EyelidPose::Half);
    assert_eq!(face.expression, ExpressionKind::Sleepy);
    creature.state.velocity = formiga_core::Point::default();
    assert_eq!(
        BodyPresentation::for_creature(&creature).clip,
        BodyClip::Action(ActionKind::Sleep)
    );
    let face = CreatureRenderer::resolve_face_state(&creature, CursorSnapshot::default(), false);
    assert_eq!(face.eyelids, EyelidPose::Closed);
}

/// Every clip a companion stands in puts its feet on the same row. A bob belongs to the body
/// and the floor under it is a constant, so setting off across the village does not lift a
/// resident a pixel or two off the ground it was standing on and stopping does not drop it
/// back down. Hoppers and quadrupeds always drew it this way; blobs and every modular plan
/// used to carry the ground along with the bob.
///
/// Sleeping, crouching, eating and drinking are not standing clips, so they are held to a
/// looser rule in `nothing_held_or_folded_sinks_below_the_feet`: whatever they draw stays on
/// or above this same row.
#[test]
fn standing_clips_share_one_ground_line() {
    const STANDING: [ActionKind; 6] = [
        ActionKind::Idle,
        ActionKind::Perch,
        ActionKind::Homebound,
        ActionKind::Traverse,
        ActionKind::Greet,
        ActionKind::InspectScreen,
    ];
    let mut cases: Vec<(String, AppearanceGenome)> = Vec::new();
    for family in [
        BodyFamily::Blob,
        BodyFamily::Hopper,
        BodyFamily::SoftQuadruped,
    ] {
        cases.push((format!("classic {family:?}"), genome(family)));
    }
    for seed in 0..64u8 {
        let mut modular = genome(BodyFamily::Blob);
        modular.design = Some(formiga_core::CreatureDesign::generated([seed; 32], 0, None));
        cases.push((format!("modular seed {seed}"), modular));
    }
    for (label, appearance) in cases {
        for reduce_motion in [false, true] {
            let ground =
                FRAME_SIZE - 1 - CreatureRenderer::resting_baseline(&appearance, reduce_motion);
            for action in STANDING {
                for frame in 0..AnimationSpec::for_action(action).frames {
                    let (_, _, _, max_y) = CreatureRenderer::render_body_frame(
                        &appearance,
                        action,
                        frame,
                        reduce_motion,
                    )
                    .canvas
                    .alpha_bounds()
                    .expect("a standing clip draws a body");
                    assert_eq!(
                        max_y, ground,
                        "{label} {action:?} frame {frame} stands on row {max_y}, not {ground}"
                    );
                }
            }
        }
    }
}

/// Nothing a companion folds against itself, and no crumb it drops, sinks below its own feet.
/// A folded wing hangs from the shoulder, so a sleeper settling with each breath, or a crouch,
/// used to carry the wingtip a pixel below the ground on some winged bodies; a large head on
/// a body a crouch had squashed flat did the same; and a blob's last mouthful dropped a crumb
/// a row under its feet. All three now stop at the ground.
///
/// Drinking is left out. A cup is held below the mouth, so a body that sits low holds it below
/// its feet, and lifting it clear of the ground would put it across the face instead.
#[test]
fn nothing_held_or_folded_sinks_below_the_feet() {
    const LOW: [BodyClip; 3] = [
        BodyClip::Action(ActionKind::Sleep),
        BodyClip::Action(ActionKind::Eat),
        BodyClip::Gesture(formiga_core::Gesture::Crouch),
    ];
    let mut cases: Vec<(String, AppearanceGenome)> = Vec::new();
    for family in [
        BodyFamily::Blob,
        BodyFamily::Hopper,
        BodyFamily::SoftQuadruped,
    ] {
        cases.push((format!("classic {family:?}"), genome(family)));
    }
    for seed in 0..64u8 {
        let mut modular = genome(BodyFamily::Blob);
        modular.design = Some(formiga_core::CreatureDesign::generated([seed; 32], 0, None));
        cases.push((format!("modular seed {seed}"), modular));
    }
    let mut snacks = std::collections::BTreeSet::new();
    for (label, base) in cases {
        // Each body with every kind of snack, since the snacks are different sizes.
        for kind in 0..SNACK_KINDS {
            let mut appearance = base.clone();
            appearance.marking_seed = (0..)
                .find(|&seed| {
                    appearance.marking_seed = seed;
                    prop_variants(&appearance).1 == kind
                })
                .expect("every snack is somebody's");
            snacks.insert(prop_variants(&appearance).1);
            for reduce_motion in [false, true] {
                let ground =
                    FRAME_SIZE - 1 - CreatureRenderer::resting_baseline(&appearance, reduce_motion);
                for clip in LOW {
                    for frame in 0..AnimationSpec::for_clip(clip).frames {
                        let (_, _, _, max_y) = CreatureRenderer::render_body_frame(
                            &appearance,
                            clip,
                            frame,
                            reduce_motion,
                        )
                        .canvas
                        .alpha_bounds()
                        .expect("the clip draws a body");
                        assert!(
                            max_y <= ground,
                            "{label} snack {kind} {clip:?} frame {frame} reaches row {max_y}, \
                             below the ground at {ground}"
                        );
                    }
                }
            }
        }
    }
    assert_eq!(
        snacks.len(),
        usize::from(SNACK_KINDS),
        "every kind of snack was tried"
    );
}

/// Whatever a companion wears shows on it, on every body plan and in every kind of pose — standing,
/// walking, climbing, up high, eating, asleep, and mid-yawn — and sits on the body: every pixel it
/// adds is within a few pixels of the companion's own drawing, a hat's height above the crown at
/// most, so nothing is ever left floating beside the companion it belongs to.
#[test]
fn whatever_a_companion_wears_shows_on_it_in_every_pose_and_sits_on_its_body() {
    use formiga_core::{Accessory, AccessoryKind};
    let preview = World::preview_adult(
        [29; 32],
        time::OffsetDateTime::UNIX_EPOCH,
        &DesktopSnapshot::default(),
    );
    let mut bodies: Vec<(String, AppearanceGenome)> = [
        BodyFamily::Blob,
        BodyFamily::Hopper,
        BodyFamily::SoftQuadruped,
    ]
    .into_iter()
    .map(|family| (format!("{family:?}"), genome(family)))
    .collect();
    for plan in formiga_core::BodyPlan::ALL {
        let mut appearance = preview.appearance.clone();
        let mut design = formiga_core::CreatureDesign::modular([29; 32], 0, None);
        design.body = plan;
        appearance.design = Some(design);
        bodies.push((format!("{plan:?}"), appearance));
    }
    let members = [crate::palette_for(&preview.appearance)];
    let dresses: Vec<AccessoryArt> = AccessoryKind::ALL
        .into_iter()
        .map(Accessory::Worn)
        .chain([Accessory::Pin(0), Accessory::Pin(3), Accessory::Pin(159)])
        .map(|accessory| AccessoryArt::resolve(accessory, [29; 32], &members))
        .collect();
    let clips: Vec<BodyClip> = [
        ActionKind::Idle,
        ActionKind::Traverse,
        ActionKind::ClimbWindow,
        ActionKind::Perch,
        ActionKind::Eat,
        ActionKind::Sleep,
    ]
    .into_iter()
    .map(BodyClip::from)
    .chain([BodyClip::from(Gesture::Yawn)])
    .collect();
    for (body, genome) in &bodies {
        for clip in &clips {
            for frame in 0..AnimationSpec::for_clip(*clip).frames {
                let bare = CreatureRenderer::render_body_frame(genome, *clip, frame, false);
                let (left, top, right, bottom) = bare
                    .canvas
                    .alpha_bounds()
                    .expect("every frame draws something");
                for dress in &dresses {
                    let dressed = CreatureRenderer::render_dressed_body_frame(
                        genome,
                        Some(*dress),
                        *clip,
                        frame,
                        false,
                    );
                    let mut added = 0;
                    for y in 0..FRAME_SIZE as i32 {
                        for x in 0..FRAME_SIZE as i32 {
                            if dressed.canvas.get(x, y) == bare.canvas.get(x, y) {
                                continue;
                            }
                            added += 1;
                            assert!(
                                x >= left as i32 - 8
                                    && x <= right as i32 + 8
                                    && y >= top as i32 - 12
                                    && y <= bottom as i32 + 2,
                                "{body} {clip:?} frame {frame}: {:?} draws at {x},{y}, away \
                                 from the body at {left},{top}..{right},{bottom}",
                                dress.accessory
                            );
                        }
                    }
                    assert!(
                        added >= 3,
                        "{body} {clip:?} frame {frame}: {:?} does not show",
                        dress.accessory
                    );
                }
            }
        }
    }
}
