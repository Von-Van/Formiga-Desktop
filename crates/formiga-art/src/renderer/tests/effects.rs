//! The small marks drawn around a body as it acts, and the startle's two halves: a greeting's
//! motif that drifts off and fades, crumbs that fall to the ground, a cheek filled by a chew, a
//! jump at a window that gathers itself first, and a shy away from the cursor that leans.
use super::*;
use formiga_core::{DesktopSnapshot, Presence, PresenceMark, World};

/// The drawn pixels of a canvas: how many, the middle of them, and the most opaque.
fn marks(canvas: &Canvas) -> (usize, (f32, f32), u8) {
    let mut drawn = Vec::new();
    for y in 0..canvas.height() as i32 {
        for x in 0..canvas.width() as i32 {
            let pixel = canvas.get(x, y);
            if pixel.a > 0 {
                drawn.push((x, y, pixel.a));
            }
        }
    }
    let count = drawn.len().max(1) as f32;
    let middle = (
        drawn.iter().map(|(x, _, _)| *x as f32).sum::<f32>() / count,
        drawn.iter().map(|(_, y, _)| *y as f32).sum::<f32>() / count,
    );
    let opacity = drawn.iter().map(|(_, _, a)| *a).max().unwrap_or(0);
    (drawn.len(), middle, opacity)
}

/// A greeting's, a game's or a cheer's motif rises off the head and away from it over the clip's
/// four frames, the same shape all the way, and fades on the last rather than hovering in place.
#[test]
fn a_motif_drifts_up_and_out_and_fades_on_the_last_frame() {
    let color = Rgba::new(120, 200, 160, 255);
    let drawn: Vec<_> = (0..4)
        .map(|frame| {
            let mut canvas = Canvas::new(FRAME_SIZE, FRAME_SIZE);
            draw_drifting_motif(&mut canvas, EffectMotif::Spark, (30, 20), frame, color);
            marks(&canvas)
        })
        .collect();
    for pair in drawn.windows(2) {
        let ((count, before, _), (next_count, after, _)) = (pair[0], pair[1]);
        assert_eq!(count, next_count, "the motif keeps its shape as it drifts");
        assert!(after.1 < before.1, "it rises: {before:?} to {after:?}");
        assert!(
            after.0 > before.0,
            "and drifts out: {before:?} to {after:?}"
        );
    }
    for (frame, (_, _, opacity)) in drawn.iter().enumerate() {
        if frame < 3 {
            assert_eq!(*opacity, 255, "frame {frame} is drawn whole");
        } else {
            assert!(*opacity < 200, "the last frame fades ({opacity})");
        }
    }
    // The first frame is where the motif always sat, which is also what reduced motion holds.
    let mut first = Canvas::new(FRAME_SIZE, FRAME_SIZE);
    draw_drifting_motif(&mut first, EffectMotif::Spark, (30, 20), 0, color);
    let mut sat = Canvas::new(FRAME_SIZE, FRAME_SIZE);
    draw_motif(&mut sat, EffectMotif::Spark, 30, 20, color);
    assert_eq!(first, sat);
    // Near the top of the frame it stops rising rather than pushing the body down its frame.
    let mut high = Canvas::new(FRAME_SIZE, FRAME_SIZE);
    draw_drifting_motif(&mut high, EffectMotif::Spark, (30, 4), 3, color);
    let (_, top, _, _) = high.alpha_bounds().expect("drawn");
    assert!(top >= 1);
}

/// The greeting, the game with a friend and the cheer all drift; reduced motion draws each of
/// them on its first frame, as it always has.
#[test]
fn greetings_games_and_cheers_draw_their_motif_drifting() {
    let mut genome = genome(BodyFamily::Hopper);
    genome.effect_motif = EffectMotif::Star;
    for clip in [
        BodyClip::Action(ActionKind::Greet),
        BodyClip::Action(ActionKind::SocialPlay),
        BodyClip::Gesture(Gesture::Cheer),
    ] {
        let frames: Vec<_> = (0..4)
            .map(|frame| CreatureRenderer::render_body_frame(&genome, clip, frame, false))
            .collect();
        let faded = frames[3]
            .canvas
            .pixels()
            .iter()
            .any(|pixel| pixel.a > 0 && pixel.a < 255);
        assert!(faded, "{clip:?} fades its motif on the last frame");
        assert!(
            frames[..3].iter().all(|frame| frame
                .canvas
                .pixels()
                .iter()
                .all(|p| p.a == 0 || p.a == 255)),
            "{clip:?} draws its motif whole before the last frame"
        );
        assert_eq!(
            CreatureRenderer::render_body_frame(&genome, clip, 3, true),
            CreatureRenderer::render_body_frame(&genome, clip, 0, true),
        );
    }
}

/// Crumbs off a mouthful fall: under the mouth as the bite knocks them off, then on the ground
/// row the body stands on, and nowhere on the other frames.
#[test]
fn eat_crumbs_fall_to_the_ground_over_two_frames() {
    let genome = genome(BodyFamily::SoftQuadruped);
    let palette = crate::prop_palette(crate::palette_for(&genome), prop_signature(&genome));
    let hold = PropHold {
        hands: PixelPoint { x: 30, y: 30 },
        mouth: PixelPoint { x: 32, y: 24 },
        floor: 42,
        ground: 45,
        forward: 1,
        face: PixelPoint { x: 30, y: 22 },
        crown: PixelPoint { x: 30, y: 14 },
        paw: false,
    };
    let crumbs = |frame: u8| {
        let mut canvas = Canvas::new(FRAME_SIZE, FRAME_SIZE);
        draw_falling_crumbs(
            &mut canvas,
            &genome,
            crate::palette_for(&genome),
            hold,
            frame,
        );
        let mut at = Vec::new();
        for y in 0..FRAME_SIZE as i32 {
            for x in 0..FRAME_SIZE as i32 {
                if canvas.get(x, y).a > 0 {
                    at.push((x, y, canvas.get(x, y)));
                }
            }
        }
        at
    };
    assert_eq!(CRUMB_FRAMES.len(), 2);
    for frame in 0..AnimationSpec::for_action(ActionKind::Eat).frames {
        let drawn = crumbs(frame);
        match CRUMB_FRAMES.iter().position(|at| *at == frame) {
            None => assert!(drawn.is_empty(), "frame {frame} has no crumbs"),
            Some(0) => {
                assert_eq!(drawn.len(), 2);
                assert!(
                    drawn
                        .iter()
                        .all(|(_, y, _)| *y > hold.mouth.y && *y < hold.ground),
                    "falling, just under the mouth: {drawn:?}"
                );
            }
            Some(_) => {
                assert_eq!(drawn.len(), 2);
                assert!(
                    drawn.iter().all(|(_, y, _)| *y == hold.ground),
                    "landed on the ground row: {drawn:?}"
                );
                assert!(drawn.iter().any(|(_, _, c)| *c == palette.shadow));
                assert!(drawn.iter().any(|(_, _, c)| *c == palette.highlight));
            }
        }
    }
    // Every body drops them on its own ground, inside its frame.
    for family in [
        BodyFamily::Blob,
        BodyFamily::Hopper,
        BodyFamily::SoftQuadruped,
    ] {
        let genome = self::genome(family);
        let clip = BodyClip::Action(ActionKind::Eat);
        let frame = CRUMB_FRAMES[1];
        let rendered = CreatureRenderer::render_body_frame(&genome, clip, frame, false);
        let (_, _, _, bottom) = rendered.canvas.alpha_bounds().expect("drawn");
        assert!(bottom < FRAME_SIZE - 1, "{family:?}");
    }
}

/// A chew fills the cheek out: the head's edge beside the mouth pushed out one pixel on the side
/// the face looks toward, two rows deep, and nothing else touched.
#[test]
fn a_chew_pushes_the_cheek_out_a_pixel() {
    let outline = Rgba::new(30, 30, 40, 255);
    let fill = Rgba::new(200, 160, 120, 255);
    let mut head = Canvas::new(FRAME_SIZE, FRAME_SIZE);
    head.fill_circle(20, 20, 7, outline);
    head.fill_circle(20, 20, 6, fill);
    let face = PixelPoint { x: 20, y: 18 };
    let right_edge = |canvas: &Canvas, y: i32| {
        (0..FRAME_SIZE as i32)
            .rev()
            .find(|x| canvas.get(*x, y).a > 0)
            .unwrap()
    };
    let mut chewing = head.clone();
    draw_full_cheek(&mut chewing, face);
    for y in 0..FRAME_SIZE as i32 {
        if y == face.y + 2 || y == face.y + 3 {
            let edge = right_edge(&head, y);
            assert_eq!(right_edge(&chewing, y), edge + 1, "row {y} bulges a pixel");
            assert_eq!(chewing.get(edge + 1, y), outline, "still outlined");
            assert_eq!(chewing.get(edge, y), fill, "filled behind its outline");
        } else {
            for x in 0..FRAME_SIZE as i32 {
                assert_eq!(chewing.get(x, y), head.get(x, y), "row {y} is untouched");
            }
        }
    }
    // Only the chewing frame of a meal has it.
    assert!(FULL_CHEEK_FRAME < AnimationSpec::for_action(ActionKind::Eat).frames);
    assert!(CRUMB_FRAMES.contains(&FULL_CHEEK_FRAME));
}

/// Shying away from the cursor carries the head forward, and the world has turned the creature
/// to face away from the cursor, so it leans away from it; a jump at a window does not lean.
#[test]
fn a_creature_leans_away_from_the_cursor_and_not_from_a_window() {
    for family in [
        BodyFamily::Blob,
        BodyFamily::Hopper,
        BodyFamily::SoftQuadruped,
    ] {
        let genome = genome(family);
        for frame in 0..4 {
            let avoid = Pose::new(
                &genome,
                BodyClip::Action(ActionKind::AvoidCursor),
                frame,
                false,
            );
            let react = Pose::new(
                &genome,
                BodyClip::Action(ActionKind::ReactToWindow),
                frame,
                false,
            );
            assert_eq!(avoid.lean, AVOID_LEAN, "{family:?} frame {frame}");
            assert_eq!(react.lean, 0, "{family:?} frame {frame}");
            assert_eq!(avoid.step_a, react.step_a, "the same scramble underneath");
            assert_eq!(avoid.bob, react.bob);
        }
    }
}

fn startled(seed: u8, action: ActionKind, elapsed: f32, reduced: bool) -> BodyPresentation {
    let mut creature = World::preview_adult(
        [seed; 32],
        time::OffsetDateTime::UNIX_EPOCH,
        &DesktopSnapshot::default(),
    );
    creature.state.attention = None;
    creature.state.beat = None;
    creature.state.flourish = None;
    creature.state.presence = Presence {
        since: 30.0,
        reduced,
        last: Some(PresenceMark {
            x: creature.state.position.x,
            depth: 0.0,
            window: None,
        }),
        ..Presence::default()
    };
    creature.state.action = action;
    creature.state.action_elapsed = elapsed;
    BodyPresentation::for_creature(&creature)
}

/// A window that jumps is met with a moment drawn down into a crouch, the crouch gesture's own
/// frame, before the scramble; a cursor coming too close is shied from at once, and with motion
/// reduced there is no crouch first.
#[test]
fn a_jump_at_a_window_gathers_itself_in_a_crouch_first() {
    for seed in [3, 17, 41] {
        let gathering = startled(seed, ActionKind::ReactToWindow, 0.05, false);
        assert_eq!(gathering.clip, BodyClip::Gesture(Gesture::Crouch));
        assert_eq!(gathering.frame, STARTLE_CROUCH_FRAME);
        assert!(STARTLE_CROUCH_FRAME < AnimationSpec::for_clip(Gesture::Crouch).frames);
        assert_eq!(
            startled(
                seed,
                ActionKind::ReactToWindow,
                STARTLE_GATHER_SECS + 0.05,
                false
            )
            .clip,
            BodyClip::Action(ActionKind::ReactToWindow),
            "then the scramble"
        );
        assert_eq!(
            startled(seed, ActionKind::AvoidCursor, 0.05, false).clip,
            BodyClip::Action(ActionKind::AvoidCursor),
            "a cursor is shied from straight away"
        );
        assert_eq!(
            startled(seed, ActionKind::ReactToWindow, 0.05, true).clip,
            BodyClip::Action(ActionKind::ReactToWindow),
            "no crouch first with motion reduced"
        );
    }
}
