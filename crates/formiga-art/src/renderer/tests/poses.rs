//! Clips and poses: how animations play, and which gesture, rest, cheer, habit or bedtime a
//! body is drawn in, and when.
use super::*;
use formiga_core::{DesktopRect, DesktopSnapshot, MonitorInfo, World};

/// A loop keeps turning however long it has been playing, long past the 255 frames a smaller
/// count would stop at, and a held clip stays on its last frame.
#[test]
fn a_clip_asked_for_long_after_it_began_still_plays() {
    let clips = ActionKind::ALL
        .into_iter()
        .map(BodyClip::Action)
        .chain(Gesture::ALL.into_iter().map(BodyClip::Gesture));
    for clip in clips {
        let spec = AnimationSpec::for_clip(clip);
        // Ten minutes in, and a day in: both far past 255 frames at any rate a clip plays at.
        for start in [600.0, 86_400.0] {
            let seen: std::collections::BTreeSet<u8> = (0..u32::from(spec.frames) * 4)
                .map(|step| spec.frame_at(start + step as f32 / f32::from(spec.fps) / 2.0))
                .collect();
            match spec.playback {
                PlaybackMode::Loop => assert_eq!(
                    seen.len(),
                    usize::from(spec.frames),
                    "{clip:?} stands still {start} seconds in"
                ),
                PlaybackMode::Hold => assert_eq!(
                    seen.into_iter().collect::<Vec<_>>(),
                    vec![spec.frames - 1],
                    "{clip:?}"
                ),
            }
        }
    }
}

#[test]
fn ambient_animation_specs_and_shared_handhold_placement_are_exact() {
    for (action, frames, fps) in [
        // A stroke with each paw, at the rate the world raises a climber by.
        (
            ActionKind::ClimbWindow,
            6,
            formiga_core::CLIMB_FRAMES_PER_SECOND,
        ),
        (ActionKind::Dangle, 4, 3),
        (ActionKind::InspectScreen, 4, 4),
        (ActionKind::PresentDiscovery, 4, 2),
    ] {
        let spec = AnimationSpec::for_action(action);
        assert_eq!(spec.frames, frames);
        assert_eq!(spec.fps, fps);
    }
    assert_eq!(formiga_core::CLIMB_FRAMES_PER_SECOND, 10);
    assert_eq!(
        AnimationSpec::for_action(ActionKind::ClimbWindow).frames,
        2 * formiga_core::CLIMB_STROKE_FRAMES
    );
    let discovery = AnimationSpec::for_action(ActionKind::PresentDiscovery);
    assert_eq!(discovery.playback, PlaybackMode::Hold);
    assert_eq!(discovery.frame_at(20.0), 3);
    assert_eq!(
        AnimationSpec::body_action(ActionKind::Tossed),
        ActionKind::Tossed
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

/// Every companion climbs on the same beat, the world's: its own tempo and phase would put its
/// pulls out of step with the rise the world gives it on them.
#[test]
fn a_climb_is_drawn_on_the_clock_the_world_raises_it_by() {
    let spec = AnimationSpec::for_action(ActionKind::ClimbWindow);
    let world = World::new(
        [61; 32],
        time::OffsetDateTime::UNIX_EPOCH,
        &DesktopSnapshot::default(),
    );
    for index in 0..40_u64 {
        let mut creature = world.save.creatures[0].clone();
        creature.id ^= index.wrapping_mul(0x9e37_79b9);
        creature.personality.activity = index as f32 / 40.0;
        let signature = MotionSignature::for_creature(&creature);
        for step in 0..200 {
            let elapsed = step as f32 / 20.0;
            assert_eq!(
                signature.frame(ActionKind::ClimbWindow, elapsed),
                spec.frame_at(elapsed)
            );
        }
    }
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
            // A scene holds a pose for as long as the moment lasts, and so does a temperament's.
            // A habit's stretch, a yawn, a swoon and a climb's mantle are done once and held at
            // the end.
            let playback = if matches!(
                gesture,
                Gesture::Stretch | Gesture::Yawn | Gesture::Swoon | Gesture::Mantle
            ) {
                PlaybackMode::Hold
            } else {
                PlaybackMode::Loop
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
        // Covering the face shuts the eyes behind the paws, and a temperament's own poses wear
        // their own faces; nothing else changes the face.
        let own = match gesture {
            Gesture::Huff | Gesture::Stomp => Some(ExpressionKind::Grumpy),
            Gesture::Peek => Some(ExpressionKind::Worried),
            Gesture::Strut => Some(ExpressionKind::Smug),
            Gesture::Beg => Some(ExpressionKind::Pleading),
            Gesture::Swoon => Some(ExpressionKind::Affectionate),
            // Topping out a climb keeps the climb's effort on its face.
            Gesture::Mantle => Some(ExpressionKind::Determined),
            _ => None,
        };
        if gesture == Gesture::Cover {
            assert_eq!(face.eyelids, EyelidPose::Closed);
        } else if let Some(expression) = own {
            assert_eq!(face.expression, expression, "{gesture:?}");
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

/// A climb tops out in a mantle of five frames played once, at the rate the world brings the
/// body up by: hung from the ledge and pulled up on it, the chest over the edge and the push up
/// while it hauls, and the stand as it steps in. With reduced motion it holds still on the push
/// up, which says what it is doing better than any other moment of it, and every other clip
/// holds still on its first frame as before.
#[test]
fn a_mantle_plays_once_at_the_worlds_rate_and_holds_its_push_with_motion_reduced() {
    let spec = AnimationSpec::for_clip(Gesture::Mantle);
    assert_eq!(spec.frames, 5);
    assert_eq!(spec.fps, formiga_core::MANTLE_FRAMES_PER_SECOND);
    assert_eq!(spec.playback, PlaybackMode::Hold);
    let frame_secs = 1.0 / f32::from(formiga_core::MANTLE_FRAMES_PER_SECOND);
    let pull_secs = f32::from(formiga_core::MANTLE_PULL_FRAMES) * frame_secs;
    let haul_secs = f32::from(formiga_core::MANTLE_HAUL_FRAMES) * frame_secs;
    assert_eq!(spec.frame_at(0.0), 0);
    assert_eq!(spec.frame_at(pull_secs * 0.75), 1);
    assert_eq!(spec.frame_at(pull_secs + 0.01), 2);
    assert_eq!(spec.frame_at(pull_secs + haul_secs * 0.75), 3);
    assert_eq!(spec.frame_at(pull_secs + haul_secs + 0.01), 4);
    assert_eq!(spec.frame_at(30.0), 4);
    assert_eq!(AnimationSpec::still_frame(Gesture::Mantle), 3);
    // Eating and drinking hold their own still frames (see `AnimationSpec::still_frame`); every
    // other clip holds its first, which for a climb is the catch, both paws on the wall.
    assert_eq!(AnimationSpec::still_frame(ActionKind::Eat), 1);
    assert_eq!(AnimationSpec::still_frame(ActionKind::Drink), 2);
    assert_eq!(AnimationSpec::still_frame(ActionKind::ClimbWindow), 0);
    for clip in BodyClip::baked().filter(|clip| {
        !matches!(
            clip,
            BodyClip::Gesture(Gesture::Mantle)
                | BodyClip::Action(ActionKind::Eat | ActionKind::Drink)
        )
    }) {
        assert_eq!(AnimationSpec::still_frame(clip), 0, "{clip:?}");
    }
    for family in [
        BodyFamily::Blob,
        BodyFamily::Hopper,
        BodyFamily::SoftQuadruped,
    ] {
        let genome = genome(family);
        let held = CreatureRenderer::render_body_frame(&genome, Gesture::Mantle, 0, true);
        for frame in 1..spec.frames {
            assert_eq!(
                held,
                CreatureRenderer::render_body_frame(&genome, Gesture::Mantle, frame, true),
                "{family:?} mantle frame {frame} moves with motion reduced"
            );
        }
        // Not the hang from the ledge, which is the frame anything else would hold.
        let hang = CreatureRenderer::render_body_frame(&genome, Gesture::Mantle, 0, false);
        assert_ne!(held.canvas, hang.canvas, "{family:?}");
    }
}

/// Hanging from a ledge, a body swings its legs slowly from side to side across the dangle's
/// loop while its paws stay where they hold the ledge.
#[test]
fn a_dangling_body_swings_its_legs_and_keeps_its_grip() {
    let preview = World::preview_adult(
        [29; 32],
        time::OffsetDateTime::UNIX_EPOCH,
        &DesktopSnapshot::default(),
    );
    let mut bodies = vec![genome(BodyFamily::Hopper)];
    for plan in formiga_core::BodyPlan::ALL {
        let mut appearance = preview.appearance.clone();
        let mut design = formiga_core::CreatureDesign::modular([29; 32], 0, None);
        design.body = plan;
        appearance.design = Some(design);
        bodies.push(appearance);
    }
    for genome in &bodies {
        let frames: Vec<_> = (0..AnimationSpec::for_action(ActionKind::Dangle).frames)
            .map(|frame| {
                CreatureRenderer::render_body_frame(genome, ActionKind::Dangle, frame, false)
            })
            .collect();
        // Out to one side and out to the other.
        assert_ne!(frames[1].canvas, frames[3].canvas, "{:?}", genome.design);
        // The paws on the ledge, at the top of the frame, never move.
        for frame in &frames[1..] {
            for y in 0..8 {
                for x in 0..FRAME_SIZE as i32 {
                    assert_eq!(
                        frame.canvas.get(x, y),
                        frames[0].canvas.get(x, y),
                        "{:?} lets go of the ledge",
                        genome.design
                    );
                }
            }
        }
    }
}

/// A companion hanging from a ledge glances down at the drop once each swing of its legs, and
/// for longer when the hang has it worried, and looks where its attention is the rest of the
/// time.
#[test]
fn a_dangling_companion_glances_down_at_the_drop() {
    let mut creature = World::preview_adult(
        [31; 32],
        time::OffsetDateTime::UNIX_EPOCH,
        &DesktopSnapshot::default(),
    );
    creature.state.action = ActionKind::Dangle;
    let ahead = formiga_core::Point {
        x: creature.state.position.x + 48.0,
        y: creature.state.position.y - 28.0,
    };
    let spec = AnimationSpec::for_action(ActionKind::Dangle);
    let mut glances = Vec::new();
    for emotion in [
        formiga_core::AttentionEmotion::Curious,
        formiga_core::AttentionEmotion::Concerned,
    ] {
        creature.state.attention = Some(formiga_core::AttentionPose {
            target: ahead,
            emotion,
            hanging: 1.0,
            gesture: None,
        });
        let mut down = 0;
        let mut ahead_of_it = 0;
        // Two whole loops, sampled in the middle of each frame.
        for step in 0..u32::from(spec.frames) * 2 * 4 {
            creature.state.action_elapsed = (step as f32 + 0.5) / f32::from(spec.fps) / 4.0;
            let gaze =
                CreatureRenderer::resolve_face_state(&creature, CursorSnapshot::default(), false)
                    .gaze;
            if gaze == GazeDirection::new(0, 1) {
                down += 1;
            } else {
                assert_eq!(gaze, GazeDirection::new(1, 0), "{emotion:?}");
                ahead_of_it += 1;
            }
        }
        assert!(
            down > 0 && ahead_of_it >= down,
            "{emotion:?}: {down} {ahead_of_it}"
        );
        glances.push(down);
    }
    assert!(glances[1] > glances[0], "{glances:?}");
}
