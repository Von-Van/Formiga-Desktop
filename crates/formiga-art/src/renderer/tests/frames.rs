//! Where a body sits in its frame: inside it at every size and in every clip, on one ground
//! row, mirrored exactly, with its ears, tails and paws where they belong, and inside the boxes
//! `world::spacing` spaces companions by.
use super::*;
use formiga_core::{DesktopRect, DesktopSnapshot, MonitorInfo, World};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

#[test]
fn every_family_renders_inside_frame() {
    for family in [
        BodyFamily::Blob,
        BodyFamily::Hopper,
        BodyFamily::SoftQuadruped,
    ] {
        for action in ActionKind::ALL {
            let spec = AnimationSpec::for_action(action);
            assert!(spec.frames > 0);
            for frame in (0..spec.frames)
                .map(|frame| CreatureRenderer::render_frame(&genome(family), action, frame, true))
            {
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

/// A companion of the greatest stature still fits its frame: nothing it draws, in any clip, reaches
/// the outermost row or column, where it would be cut off. Every original family at the extremes
/// of its genes, and two thousand recipes from the current generator.
#[test]
fn the_largest_stature_never_reaches_the_edge_of_the_frame() {
    let largest = formiga_core::size_for(formiga_core::STATURE_MAX, 100);
    let check = |genome: &AppearanceGenome, label: &str| {
        for clip in BodyClip::baked() {
            for frame in 0..AnimationSpec::for_clip(clip).frames {
                let rendered = CreatureRenderer::render_body_frame(genome, clip, frame, false);
                let bounds = rendered.canvas.alpha_bounds().expect("a body is drawn");
                assert!(
                    bounds.0 > 0
                        && bounds.1 > 0
                        && bounds.2 < FRAME_SIZE - 1
                        && bounds.3 < FRAME_SIZE - 1,
                    "{label} {clip:?} {frame}: {bounds:?}"
                );
            }
        }
    };
    for family in [
        BodyFamily::Blob,
        BodyFamily::Hopper,
        BodyFamily::SoftQuadruped,
    ] {
        for wide in [false, true] {
            let mut g = genome(family);
            g.logical_size = largest;
            g.body_width = if wide { 27 } else { 16 };
            g.body_height = if wide { 24 } else { 13 };
            g.leg_length = if wide { 8 } else { 2 };
            g.head_appendages.size = 7;
            g.tail_length = 10;
            for style in [
                HeadAppendageStyle::Pointed,
                HeadAppendageStyle::Round,
                HeadAppendageStyle::Droop,
            ] {
                g.head_appendages.style = style;
                check(&g, &format!("{family:?} wide={wide} {style:?}"));
            }
        }
    }
    let mut g = genome(BodyFamily::Blob);
    g.logical_size = largest;
    for index in 0..600_u64 {
        let seed = formiga_core::SeedStream::new([67; 32]).bytes("largest-stature", index);
        g.design = Some(formiga_core::CreatureDesign::generated(seed, 0, None));
        check(&g, &format!("{:?}", g.design));
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
        // The largest adult sits at the top of the range and the smallest mini of the smallest
        // adult at the bottom, so these values bracket every `logical_size` a colony can hold.
        for logical_size in [18_u8, 25, 32, 38, 44] {
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

/// Every companion is the same Echo Fox while the mode is on: whatever it looks like underneath,
/// every clip and every face draws the same fox, inside its frame and with its face on its head.
#[test]
fn every_companion_is_the_same_echo_fox_in_every_clip() {
    let foxes: Vec<AppearanceGenome> = [BodyFamily::Blob, BodyFamily::SoftQuadruped]
        .into_iter()
        .map(|family| AppearanceGenome {
            echo_fox: true,
            ..genome(family)
        })
        .collect();
    for clip in BodyClip::baked() {
        for frame in 0..AnimationSpec::for_clip(clip).frames {
            let drawn: Vec<RenderedBodyFrame> = foxes
                .iter()
                .map(|fox| CreatureRenderer::render_body_frame(fox, clip, frame, false))
                .collect();
            assert_eq!(drawn[0], drawn[1], "{clip:?} frame {frame} is one fox");
            let (min_x, min_y, max_x, max_y) = drawn[0].canvas.alpha_bounds().unwrap();
            assert!(min_x >= 1 && min_y >= 1 && max_x < FRAME_SIZE - 1 && max_y < FRAME_SIZE - 1);
            let face = drawn[0].face_anchor;
            assert!(
                drawn[0].canvas.get(face.x, face.y).a > 0,
                "{clip:?} frame {frame} keeps its face on its head"
            );
        }
    }
    for expression in ExpressionKind::ALL {
        let state = FaceRenderState {
            expression,
            eyelids: EyelidPose::Open,
            gaze: GazeDirection::default(),
        };
        assert_eq!(
            CreatureRenderer::render_face_frame(&foxes[0], state),
            CreatureRenderer::render_face_frame(&foxes[1], state),
        );
    }
    let plain = genome(BodyFamily::Blob);
    assert_ne!(
        CreatureRenderer::render_body_frame(&plain, ActionKind::Idle, 0, false).canvas,
        CreatureRenderer::render_body_frame(&foxes[0], ActionKind::Idle, 0, false).canvas,
    );
}

/// A climb goes up the outside of a window's side with its frame's middle as far out from the edge
/// line as `formiga_core::climb_grip_reach` says its paw on that side reaches, less a grip, so the
/// paw is drawn on the edge and the body beside the window. The simulation cannot see the art, so
/// this holds the estimate to the frames: across generated companions of every kind, the furthest
/// a climb draws on the side it faces — its paws, and for a long body the forepaw out ahead —
/// sits on the estimate for the typical companion of each kind, and within a few pixels of it for
/// nearly all of them, so the paw lands on the edge or a little over it rather than short of it
/// or far across the window.
#[test]
fn a_climbing_paw_reaches_as_far_as_the_simulation_climbs_by() {
    const CENTRE: i32 = FRAME_SIZE as i32 / 2;
    let desktop = DesktopSnapshot::default();
    let spec = AnimationSpec::for_action(ActionKind::ClimbWindow);
    // How far past the middle of its frame a companion's climb draws on the side it faces: the
    // middle one of its frames, so a single frame's ear or flick of a tail does not decide it.
    let reach = |genome: &AppearanceGenome| {
        let mut furthest: Vec<i32> = (0..spec.frames)
            .map(|frame| {
                let body = CreatureRenderer::render_body_frame(
                    genome,
                    ActionKind::ClimbWindow,
                    frame,
                    false,
                );
                let (_, _, right, _) = body.canvas.alpha_bounds().expect("a climber is drawn");
                right as i32 + 1 - CENTRE
            })
            .collect();
        furthest.sort_unstable();
        furthest[furthest.len() / 2]
    };
    let mut by_estimate: BTreeMap<i32, Vec<i32>> = BTreeMap::new();
    for index in 0..120_u8 {
        let mut seed = [index; 32];
        seed[1] = index.wrapping_mul(37);
        let modular =
            World::preview_adult(seed, time::OffsetDateTime::UNIX_EPOCH, &desktop).appearance;
        let mut classic = modular.clone();
        classic.design = None;
        let mut fox = modular.clone();
        fox.echo_fox = true;
        for genome in [modular, classic, fox] {
            let estimate = formiga_core::climb_grip_reach(&genome) as i32;
            by_estimate
                .entry(estimate)
                .or_default()
                .push(reach(&genome));
        }
    }
    for (estimate, mut drawn) in by_estimate {
        drawn.sort_unstable();
        let typical = drawn[drawn.len() / 2];
        let near = drawn.iter().filter(|r| (**r - estimate).abs() <= 3).count();
        assert!(
            (typical - estimate).abs() <= 1,
            "companions climbing at {estimate} pixels draw out to {typical}"
        );
        assert!(
            near * 10 >= drawn.len() * 9,
            "only {near} of {} companions climbing at {estimate} pixels draw within 3 of it: \
             {drawn:?}",
            drawn.len()
        );
    }
}

/// A thrown companion is drawn balled up and tumbling, not as it hangs from a hand. The toss has
/// two frames of its own in the atlas on every kind of body — an original family, each modular
/// plan, the Echo Fox and a sculpted form — unlike any frame of being carried and unlike each
/// other: the second is the first turned, with no colour the first does not have. Reduced motion
/// holds the ball still, and the face drawn with it has its eyes shut.
#[test]
fn a_toss_has_tucked_frames_of_its_own() {
    let tossed = BodyClip::Action(ActionKind::Tossed);
    assert_eq!(tossed.body(), tossed, "a toss is baked under its own name");
    assert_eq!(
        BodyClip::baked().filter(|clip| *clip == tossed).count(),
        1,
        "a toss has one clip in the atlas"
    );
    let spec = AnimationSpec::for_clip(tossed);
    assert_eq!((spec.frames, spec.playback), (2, PlaybackMode::Loop));

    let mut subjects: Vec<(String, AppearanceGenome)> = [
        BodyFamily::Blob,
        BodyFamily::Hopper,
        BodyFamily::SoftQuadruped,
    ]
    .into_iter()
    .map(|family| (format!("{family:?}"), genome(family)))
    .collect();
    for plan in formiga_core::BodyPlan::ALL {
        let mut modular = genome(BodyFamily::Blob);
        let mut design = formiga_core::CreatureDesign::modular([29; 32], 0, None);
        design.body = plan;
        modular.design = Some(design);
        subjects.push((format!("{plan:?}"), modular));
    }
    subjects.push((
        "Echo Fox".into(),
        AppearanceGenome {
            echo_fox: true,
            ..genome(BodyFamily::Hopper)
        },
    ));
    let mut sculpted = genome(BodyFamily::Blob);
    sculpted.sculpt = Some(formiga_core::forms::Sculpt::starter(
        formiga_core::forms::Plan::CompactQuadruped,
    ));
    subjects.push(("sculpted".into(), sculpted));

    let colours = |canvas: &Canvas| {
        canvas
            .pixels()
            .iter()
            .filter(|pixel| pixel.a > 0)
            .map(|pixel| (pixel.r, pixel.g, pixel.b, pixel.a))
            .collect::<std::collections::HashSet<_>>()
    };
    for (label, genome) in &subjects {
        let carried: Vec<Canvas> = (0..AnimationSpec::for_action(ActionKind::Dragged).frames)
            .map(|frame| {
                CreatureRenderer::render_body_frame(genome, ActionKind::Dragged, frame, false)
                    .canvas
            })
            .collect();
        let [ball, turned] = [0, 1]
            .map(|frame| CreatureRenderer::render_body_frame(genome, tossed, frame, false).canvas);
        assert_ne!(
            ball, turned,
            "{label}: the second frame of a toss is turned"
        );
        for frame in [&ball, &turned] {
            assert!(
                !carried.contains(frame),
                "{label}: a toss is drawn as being carried"
            );
        }
        assert!(
            colours(&turned).is_subset(&colours(&ball)),
            "{label}: turning the ball invents a colour"
        );
        assert_eq!(
            CreatureRenderer::render_body_frame(genome, tossed, 1, true).canvas,
            CreatureRenderer::render_body_frame(genome, tossed, 0, true).canvas,
            "{label}: reduced motion holds the ball still"
        );
    }
    for frame in 0..spec.frames {
        assert_eq!(
            default_eyelids(ActionKind::Tossed, frame),
            EyelidPose::Closed
        );
    }
}

/// Landing is played once and held: squashed on the touch, back up, a little past, and standing
/// on the very frame a rest starts from, so the cut to whatever comes next does not show. A leap
/// has two frames of its own, rising and coming down, unlike any frame of standing or landing.
/// Carried, a companion hangs still: no frame of being carried bobs, and the hang sways a pixel
/// over and back at most.
#[test]
fn landing_holds_standing_a_leap_has_its_own_frames_and_a_carry_hangs_still() {
    let landing = AnimationSpec::for_action(ActionKind::Landing);
    assert_eq!((landing.frames, landing.playback), (4, PlaybackMode::Hold));
    assert_eq!(landing.frame_at(0.0), 0);
    assert_eq!(landing.frame_at(0.55), 3);
    assert_eq!(landing.frame_at(30.0), 3, "landing holds its last frame");
    let leap = AnimationSpec::for_clip(Gesture::Leap);
    assert_eq!(leap.frames, 4);
    assert_eq!(
        BodyClip::baked()
            .filter(|clip| *clip == BodyClip::Gesture(Gesture::Leap))
            .count(),
        1
    );
    let mut subjects: Vec<(String, AppearanceGenome)> = [
        BodyFamily::Blob,
        BodyFamily::Hopper,
        BodyFamily::SoftQuadruped,
    ]
    .into_iter()
    .map(|family| (format!("{family:?}"), genome(family)))
    .collect();
    for plan in formiga_core::BodyPlan::ALL {
        let mut modular = genome(BodyFamily::Blob);
        let mut design = formiga_core::CreatureDesign::modular([29; 32], 0, None);
        design.body = plan;
        modular.design = Some(design);
        subjects.push((format!("{plan:?}"), modular));
    }
    let body = |genome: &AppearanceGenome, clip: BodyClip, frame: u8| {
        CreatureRenderer::render_body_frame(genome, clip, frame, false).canvas
    };
    for (label, genome) in &subjects {
        let landed = body(genome, ActionKind::Landing.into(), 3);
        assert_eq!(
            landed,
            body(genome, ActionKind::Idle.into(), 0),
            "{label}: a landing ends standing as a rest begins"
        );
        assert_ne!(body(genome, ActionKind::Landing.into(), 0), landed);
        let leaping = [0, 1, 2, 3].map(|frame| body(genome, Gesture::Leap.into(), frame));
        for (frame, drawn) in leaping.iter().enumerate() {
            assert_ne!(drawn, &landed, "{label}: a leap is not standing");
            for later in &leaping[frame + 1..] {
                assert_ne!(
                    drawn, later,
                    "{label}: every part of a leap's arc is its own"
                );
            }
        }
        let carried: Vec<Canvas> = (0..4)
            .map(|frame| body(genome, ActionKind::Dragged.into(), frame))
            .collect();
        assert_eq!(carried[0], carried[1], "{label}");
        assert_eq!(carried[2], carried[3], "{label}");
        let rows = |canvas: &Canvas| {
            let (_, top, _, bottom) = canvas.alpha_bounds().expect("drawn");
            (top, bottom)
        };
        assert!(
            carried.iter().all(|frame| rows(frame) == rows(&carried[0])),
            "{label}: being carried does not bob"
        );
    }
}

/// The two-tone outline stays in the frame's own silhouette neighbourhood: it never touches a
/// pixel the creature draws, its dark ring lies right against the silhouette, its light rim one
/// edge's width further out and no further, and no rim is ever drawn below the lowest row the
/// creature draws, where it would lighten the shadow under the feet. At a finer detail every
/// distance is that many pixels.
#[test]
fn the_outline_rim_hugs_the_silhouette_and_never_goes_below_the_feet() {
    let check = |plain: &Canvas, edged: &Canvas, detail: i32, what: &str| {
        let (width, height) = (plain.width() as i32, plain.height() as i32);
        let solid = |x: i32, y: i32| {
            (0..width).contains(&x) && (0..height).contains(&y) && plain.get(x, y).a > 16
        };
        // How far, square-wise, the nearest drawn pixel is: 0 on the creature itself.
        let distance = |x: i32, y: i32| {
            (0..=2 * detail + 1)
                .find(|&reach| {
                    (-reach..=reach).any(|dy| (-reach..=reach).any(|dx| solid(x + dx, y + dy)))
                })
                .unwrap_or(i32::MAX)
        };
        let lowest = (0..height)
            .rev()
            .find(|&y| (0..width).any(|x| solid(x, y)))
            .expect("a creature to outline");
        let mut rim = 0;
        for y in 0..height {
            for x in 0..width {
                let (before, after) = (plain.get(x, y), edged.get(x, y));
                if solid(x, y) {
                    assert_eq!(before, after, "{what}: the creature's own pixel at {x},{y}");
                    continue;
                }
                if before == after || after.a == 0 {
                    continue;
                }
                let away = distance(x, y);
                if (after.r, after.g, after.b) == (OUTLINE_RIM.r, OUTLINE_RIM.g, OUTLINE_RIM.b) {
                    rim += 1;
                    assert!(after.a <= OUTLINE_RIM.a, "{what}: a faint rim at {x},{y}");
                    assert!(
                        away > detail && away <= 2 * detail,
                        "{what}: rim {away} px from the silhouette at {x},{y}"
                    );
                    assert!(y <= lowest, "{what}: rim under the feet at {x},{y}");
                } else {
                    assert!(away <= detail, "{what}: ring {away} px out at {x},{y}");
                }
            }
        }
        assert!(rim > 0, "{what}: a rim to see");
    };
    for family in [
        BodyFamily::Blob,
        BodyFamily::Hopper,
        BodyFamily::SoftQuadruped,
    ] {
        for action in ActionKind::ALL {
            for frame in 0..AnimationSpec::for_action(action).frames.min(2) {
                let plain =
                    CreatureRenderer::render_body_frame(&genome(family), action, frame, true)
                        .canvas;
                let mut edged = plain.clone();
                CreatureRenderer::outline_frame(&mut edged);
                check(&plain, &edged, 1, &format!("{family:?} {action:?} {frame}"));
            }
        }
    }
    // A finer bake: the same shape at two pixels to the unit.
    let unit = CreatureRenderer::render_body_frame(
        &genome(BodyFamily::SoftQuadruped),
        ActionKind::Idle,
        0,
        true,
    )
    .canvas;
    let mut fine = Canvas::new(unit.width() * 2, unit.height() * 2);
    for y in 0..fine.height() as i32 {
        for x in 0..fine.width() as i32 {
            fine.set(x, y, unit.get(x / 2, y / 2));
        }
    }
    let mut edged = fine.clone();
    CreatureRenderer::outline_frame_in_detail(&mut edged, 2);
    check(&fine, &edged, 2, "detail 2");
}

/// The bodies the still-frame tests draw: the three original families, a run of modular ones,
/// and the Echo Fox.
fn still_frame_subjects() -> Vec<(String, AppearanceGenome)> {
    let mut cases: Vec<(String, AppearanceGenome)> = [
        BodyFamily::Blob,
        BodyFamily::Hopper,
        BodyFamily::SoftQuadruped,
    ]
    .into_iter()
    .map(|family| (format!("classic {family:?}"), genome(family)))
    .collect();
    for seed in 0..16u8 {
        let mut modular = genome(BodyFamily::Blob);
        modular.design = Some(formiga_core::CreatureDesign::generated([seed; 32], 0, None));
        cases.push((format!("modular seed {seed}"), modular));
    }
    cases.push((
        "echo fox".to_owned(),
        AppearanceGenome {
            echo_fox: true,
            ..genome(BodyFamily::Hopper)
        },
    ));
    cases
}

/// With motion reduced every clip is one picture, whichever of its frames is asked for, and it is
/// the clip's still frame: what it holds is what the moving clip holds on that frame. For eating
/// that is the snack up at the mouth with a bite out of it rather than held away before the
/// first bite, for drinking the cup tipped to the mouth rather than held down, and for play the
/// toy at the paw.
#[test]
fn reduced_motion_holds_each_clip_on_its_still_frame() {
    for (label, appearance) in still_frame_subjects() {
        for clip in BodyClip::baked() {
            let spec = AnimationSpec::for_clip(clip);
            let still = AnimationSpec::still_frame(clip);
            assert!(
                still < spec.frames,
                "{clip:?} holds frame {still} of {}",
                spec.frames
            );
            let held = CreatureRenderer::render_body_frame(&appearance, clip, still, true);
            for frame in 0..spec.frames {
                assert!(
                    CreatureRenderer::render_body_frame(&appearance, clip, frame, true) == held,
                    "{label} {clip:?} frame {frame} is not its still frame {still}"
                );
            }
            let moving = CreatureRenderer::render_body_frame(&appearance, clip, still, false);
            assert_eq!(
                held.held.map(|held| held.thing),
                moving.held.map(|held| held.thing),
                "{label} {clip:?} holds something other than its frame {still} does"
            );
        }
        let held = |action: ActionKind, frame: u8, reduce_motion: bool| {
            CreatureRenderer::render_body_frame(&appearance, action, frame, reduce_motion)
                .held
                .expect("the clip holds something")
        };
        let first = held(ActionKind::Eat, 0, false);
        let mouthful = held(ActionKind::Eat, 0, true);
        assert!(
            matches!(mouthful.thing, HeldThing::Snack { bite, .. } if bite > 0)
                && mouthful.at.y < first.at.y,
            "{label} eats with the snack held away: {mouthful:?} against {first:?}"
        );
        assert!(
            matches!(
                held(ActionKind::Drink, 0, true).thing,
                HeldThing::Cup { tipped: true, .. }
            ),
            "{label} drinks with the cup down"
        );
        assert_eq!(
            held(ActionKind::SoloPlay, 0, true).thing,
            held(ActionKind::SoloPlay, 0, false).thing,
            "{label} plays with the toy away from the paw"
        );
    }
}

/// With motion on, the still frames change nothing: every frame is its own, and a clip that holds
/// still on another frame still opens on its first: the snack held up before the first bite,
/// and the cup down. A climb holds still on its first frame, the catch, with both paws on the
/// wall.
#[test]
fn a_still_frame_changes_nothing_with_motion_on() {
    for (label, appearance) in still_frame_subjects() {
        for action in [ActionKind::Eat, ActionKind::Drink] {
            let still = AnimationSpec::still_frame(action);
            assert_ne!(still, 0, "{action:?}");
            assert!(
                CreatureRenderer::render_body_frame(&appearance, action, 0, false)
                    != CreatureRenderer::render_body_frame(&appearance, action, still, false),
                "{label} {action:?} opens on its still frame with motion on"
            );
        }
        let opening = |action: ActionKind| {
            CreatureRenderer::render_body_frame(&appearance, action, 0, false)
                .held
                .expect("the clip holds something")
                .thing
        };
        assert!(
            matches!(opening(ActionKind::Eat), HeldThing::Snack { bite: 0, .. }),
            "{label}"
        );
        assert!(
            matches!(
                opening(ActionKind::Drink),
                HeldThing::Cup { tipped: false, .. }
            ),
            "{label}"
        );
    }
}

/// A squeeze through a narrow gap is drawn pinched, not narrowed on screen: two frames of its own
/// in the atlas on every kind of body — an original family, each modular plan, the Echo Fox and
/// each sculpted plan — the feet shuffling between them, the whole body drawn about three
/// quarters as wide as it walks (at most 0.82 of its narrowest stride), with the face drawn in
/// and pinched with it so that nothing draws a face over it. Reduced motion holds still.
#[test]
fn a_squeeze_is_drawn_pinched_with_its_face_in() {
    let squeeze = BodyClip::Action(ActionKind::SqueezeWindow);
    assert_eq!(
        squeeze.body(),
        squeeze,
        "a squeeze is baked under its own name"
    );
    assert_eq!(
        BodyClip::baked().filter(|clip| *clip == squeeze).count(),
        1,
        "a squeeze has one clip in the atlas"
    );
    assert_eq!(
        BodyClip::baked()
            .filter(|clip| clip.face_in_frame())
            .collect::<Vec<_>>(),
        vec![squeeze],
        "only a squeeze draws its own face"
    );
    let spec = AnimationSpec::for_clip(squeeze);
    assert_eq!((spec.frames, spec.playback), (2, PlaybackMode::Loop));

    let mut subjects: Vec<(String, AppearanceGenome)> = [
        BodyFamily::Blob,
        BodyFamily::Hopper,
        BodyFamily::SoftQuadruped,
    ]
    .into_iter()
    .map(|family| (format!("{family:?}"), genome(family)))
    .collect();
    for plan in formiga_core::BodyPlan::ALL {
        let mut modular = genome(BodyFamily::Blob);
        let mut design = formiga_core::CreatureDesign::modular([29; 32], 0, None);
        design.body = plan;
        modular.design = Some(design);
        subjects.push((format!("{plan:?}"), modular));
    }
    subjects.push((
        "Echo Fox".into(),
        AppearanceGenome {
            echo_fox: true,
            ..genome(BodyFamily::Hopper)
        },
    ));
    for plan in formiga_core::forms::Plan::ALL {
        let mut sculpted = genome(BodyFamily::Blob);
        sculpted.sculpt = Some(formiga_core::forms::Sculpt::starter(plan));
        subjects.push((format!("sculpted {plan:?}"), sculpted));
    }

    let width = |canvas: &Canvas| {
        let (min_x, _, max_x, _) = canvas.alpha_bounds().expect("drawn");
        (max_x - min_x + 1) as f32
    };
    let looking = FaceRenderState {
        expression: ExpressionKind::Joy,
        eyelids: EyelidPose::Half,
        gaze: GazeDirection::new(-1, 1),
    };
    for (label, genome) in &subjects {
        let walking = (0..AnimationSpec::for_action(ActionKind::Traverse).frames)
            .map(|frame| {
                width(
                    &CreatureRenderer::render_body_frame(
                        genome,
                        ActionKind::Traverse,
                        frame,
                        false,
                    )
                    .canvas,
                )
            })
            .fold(f32::MAX, f32::min);
        let [first, second] =
            [0, 1].map(|frame| CreatureRenderer::render_body_frame(genome, squeeze, frame, false));
        assert_ne!(
            first.canvas, second.canvas,
            "{label}: the feet shuffle between the squeeze's frames"
        );
        for (frame, drawn) in [&first, &second].into_iter().enumerate() {
            assert!(
                width(&drawn.canvas) <= walking * 0.82,
                "{label} frame {frame}: {} wide against a walk {walking} wide",
                width(&drawn.canvas)
            );
            // The face is in the frame already: a whole frame drawn with any face at all is the
            // body frame as it stands.
            assert_eq!(
                CreatureRenderer::render_composited_frame(
                    genome,
                    squeeze,
                    frame as u8,
                    true,
                    false,
                    looking
                ),
                drawn.canvas,
                "{label} frame {frame}: a face is drawn over the squeeze"
            );
        }
        assert_eq!(
            CreatureRenderer::render_body_frame(genome, squeeze, 1, true).canvas,
            CreatureRenderer::render_body_frame(genome, squeeze, 0, true).canvas,
            "{label}: reduced motion holds still"
        );
    }
}
