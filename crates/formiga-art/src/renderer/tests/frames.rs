//! Where a body sits in its frame: inside it at every size and in every clip, on one ground
//! row, mirrored exactly, with its ears, tails and paws where they belong, and inside the boxes
//! `world::spacing` spaces companions by.
use super::*;
use formiga_core::{DesktopRect, DesktopSnapshot, MonitorInfo, World};
use sha2::{Digest, Sha256};

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
