//! Belongings: the props a companion plays with, carries and is given, and whatever it wears,
//! drawn on its body and on their own.
use super::*;
use formiga_core::{DesktopSnapshot, World};
use sha2::{Digest, Sha256};

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
    if let Some(held) = body.held {
        held.paint(&mut canvas, true);
    }
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
                let Some(bounds) = prop.alpha_bounds() else {
                    // A mouthful is chewed with nothing in the paws.
                    assert!(
                        action == ActionKind::Eat
                            && MOUTHFUL[usize::from(frame % 4)] == Mouthful::Chewing,
                        "{family:?} {action:?} {frame} draws nothing"
                    );
                    continue;
                };
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
    // A mini, small enough to wear the speck drawings, checked in the pieces drawn that way.
    let mut mini = bodies.last().expect("a modular body").1.clone();
    mini.logical_size = accessories::SPECK_SIZE - 2;
    bodies.push(("mini".to_owned(), mini));
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
                    if body == "mini"
                        && !matches!(
                            dress.accessory,
                            Accessory::Worn(
                                AccessoryKind::MoonClip
                                    | AccessoryKind::ShellNecklace
                                    | AccessoryKind::SnailPack
                            )
                        )
                    {
                        continue;
                    }
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

/// A companion of [`accessories::SPECK_SIZE`] or smaller draws the moon clip, the shell necklace and
/// the snail pack in their plainer speck shapes; one pixel of size larger, and every piece, those
/// three included, is drawn exactly as before.
#[test]
fn only_a_speck_sized_companion_wears_the_speck_drawings() {
    use formiga_core::{Accessory, AccessoryKind};
    let preview = World::preview_adult(
        [29; 32],
        time::OffsetDateTime::UNIX_EPOCH,
        &DesktopSnapshot::default(),
    );
    let members = [crate::palette_for(&preview.appearance)];
    let figure = Figure {
        face: PixelPoint { x: 24, y: 26 },
        crown: PixelPoint { x: 24, y: 18 },
        head_half: 7,
        neck: PixelPoint { x: 24, y: 30 },
        neck_half: 5,
        chest: PixelPoint { x: 25, y: 33 },
        hip: PixelPoint { x: 18, y: 34 },
        back: PixelPoint { x: 18, y: 30 },
        floor: 42,
        hand: None,
    };
    let speck_pieces = [
        AccessoryKind::MoonClip,
        AccessoryKind::ShellNecklace,
        AccessoryKind::SnailPack,
    ];
    for kind in AccessoryKind::ALL {
        let art = AccessoryArt::resolve(Accessory::Worn(kind), [29; 32], &members);
        let worn = |logical_size| {
            let mut canvas = Canvas::new(FRAME_SIZE, FRAME_SIZE);
            accessories::draw_accessory(&mut canvas, art, figure, logical_size);
            canvas
        };
        let speck = worn(accessories::SPECK_SIZE);
        let full = worn(accessories::SPECK_SIZE + 1);
        assert_eq!(
            full,
            worn(u8::MAX),
            "{kind:?} draws the same at every size above a speck"
        );
        assert!(speck.alpha_bounds().is_some(), "{kind:?} shows on a speck");
        if speck_pieces.contains(&kind) {
            assert_ne!(speck, full, "{kind:?} has a speck drawing");
        } else {
            assert_eq!(speck, full, "{kind:?} has no speck drawing");
        }
    }
}

/// Every piece, pictured on its own as it sits on a companion, is the piece and nothing else: it
/// is there, it is smaller than the companion's frame, and it has a clear pixel all round.
#[test]
fn every_piece_can_be_pictured_on_its_own() {
    use formiga_core::{Accessory, AccessoryKind};
    let preview = World::preview_adult(
        [29; 32],
        time::OffsetDateTime::UNIX_EPOCH,
        &DesktopSnapshot::default(),
    );
    let members = [crate::palette_for(&preview.appearance)];
    for family in [
        BodyFamily::Blob,
        BodyFamily::Hopper,
        BodyFamily::SoftQuadruped,
    ] {
        let genome = genome(family);
        for accessory in AccessoryKind::ALL
            .into_iter()
            .map(Accessory::Worn)
            .chain([Accessory::Pin(16)])
        {
            let art = AccessoryArt::resolve(accessory, [29; 32], &members);
            let piece = CreatureRenderer::render_worn_piece(&genome, art)
                .unwrap_or_else(|| panic!("{family:?}: {accessory:?} pictures as nothing"));
            assert!(piece.width() < FRAME_SIZE && piece.height() < FRAME_SIZE);
            let (left, top, right, bottom) = piece.alpha_bounds().expect("something drawn");
            assert_eq!((left, top), (1, 1), "{family:?}: {accessory:?}");
            assert_eq!(
                (right + 2, bottom + 2),
                (piece.width(), piece.height()),
                "{family:?}: {accessory:?}"
            );
        }
    }
}

/// Every kind of body a belonging is held by, at the smallest and largest a companion stands:
/// the three original families, a recipe of each body plan, the Echo Fox, and a sculpted form of
/// each plan.
fn holding_subjects() -> Vec<(String, AppearanceGenome)> {
    use formiga_core::forms::{Design, Form, Plan, Sculpt, plain_face};
    let preview = World::preview_adult(
        [29; 32],
        time::OffsetDateTime::UNIX_EPOCH,
        &DesktopSnapshot::default(),
    )
    .appearance;
    let mut kinds: Vec<(String, AppearanceGenome)> = [
        BodyFamily::Blob,
        BodyFamily::Hopper,
        BodyFamily::SoftQuadruped,
    ]
    .into_iter()
    .map(|family| (format!("{family:?}"), genome(family)))
    .collect();
    for plan in formiga_core::BodyPlan::ALL {
        let mut design = formiga_core::CreatureDesign::modular([29; 32], 0, None);
        design.body = plan;
        for limbs in 0..=2 {
            design.classic.limbs = limbs;
            let mut appearance = preview.clone();
            appearance.design = Some(design);
            kinds.push((format!("{plan:?} limbs {limbs}"), appearance));
        }
    }
    kinds.push((
        "Echo Fox".into(),
        AppearanceGenome {
            echo_fox: true,
            ..genome(BodyFamily::Hopper)
        },
    ));
    for plan in Plan::ALL {
        let design = Design {
            form: Form::Sculpted {
                sculpt: Sculpt::starter(plan),
            },
            face: plain_face(),
        };
        kinds.push((format!("sculpted {plan:?}"), design.genome(&preview)));
    }
    kinds
        .into_iter()
        .flat_map(|(name, genome)| {
            [formiga_core::STATURE_MIN, formiga_core::STATURE_MAX].map(|stature| {
                let mut sized = genome.clone();
                sized.logical_size = formiga_core::size_for(stature, 100);
                (format!("{name} at {stature}"), sized)
            })
        })
        .collect()
}

/// Every clip whose own frames carry no marks of what is held renders exactly as it did before
/// belongings were placed from each frame's paw and mouth: holding something moves the thing,
/// never the body. Eating, play and drinking are left out: each moves its body in time with
/// what it holds (the head dips to a mouthful and tips back with a cup, a paw keeps a toy up),
/// and eating and play leave crumbs and a trail in the body's own frame. A change that means to
/// redraw one of the others records the new digest here.
#[test]
fn clips_that_hold_nothing_of_their_own_draw_as_they_did() {
    let mut digest = Sha256::new();
    for (_, genome) in holding_subjects() {
        for clip in BodyClip::baked() {
            if matches!(
                clip,
                BodyClip::Action(ActionKind::Eat | ActionKind::SoloPlay | ActionKind::Drink)
            ) {
                continue;
            }
            for frame in 0..AnimationSpec::for_clip(clip).frames {
                let body = CreatureRenderer::render_body_frame(&genome, clip, frame, false);
                digest.update(body.canvas.rgba_bytes());
                digest.update([body.face_anchor.x as u8, body.face_anchor.y as u8]);
            }
        }
    }
    let hex: String = digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    assert_eq!(
        hex, "f63b3d82a2d2768e00b1abcb528d9bee35a91c1c85c6d9632edc750df0587acc",
        "a clip that holds nothing of its own drew differently"
    );
}

/// Whatever a frame holds is held by the body drawn in it: at least one of the thing's pixels
/// meets the paw the frame drew (on it, or beside it along a row or a column) or comes within
/// two pixels of the mouth the face is drawn with, on every body, at both ends of the size
/// range, in every frame of every clip that holds something.
///
/// Keep-up is the one exception, because its toy is in the air for three beats of four and never
/// covers the face; it has a test of its own, which holds it to the paw on the beat it is caught
/// (see `keep_ups_toy_never_crosses_the_face_and_is_caught_in_the_paw`).
#[test]
fn every_belonging_is_held_in_a_drawn_paw_or_at_the_mouth() {
    let mut misses = Vec::new();
    for (name, genome) in holding_subjects() {
        for clip in BodyClip::baked() {
            if clip == BodyClip::Action(ActionKind::SoloPlay) {
                continue;
            }
            for frame in 0..AnimationSpec::for_clip(clip).frames {
                let body = CreatureRenderer::render_body_frame(&genome, clip, frame, false);
                let Some(held) = body.held else {
                    continue;
                };
                let mut prop = Canvas::new(FRAME_SIZE, FRAME_SIZE);
                held.paint(&mut prop, true);
                let solid = |canvas: &Canvas, x: i32, y: i32| canvas.get(x, y).a > 0;
                let things: Vec<(i32, i32)> = (0..FRAME_SIZE as i32)
                    .flat_map(|y| (0..FRAME_SIZE as i32).map(move |x| (x, y)))
                    .filter(|&(x, y)| solid(&prop, x, y))
                    .collect();
                // The paw: the body's own pixels within a pixel of where the frame says it holds
                // things, which has to be on the drawing itself.
                let hands = body.hands;
                let paw: Vec<(i32, i32)> = (hands.y - 1..=hands.y + 1)
                    .flat_map(|y| (hands.x - 1..=hands.x + 1).map(move |x| (x, y)))
                    .filter(|&(x, y)| solid(&body.canvas, x, y))
                    .collect();
                let touching = |(x, y): (i32, i32)| {
                    paw.iter()
                        .any(|&(px, py)| (x - px).abs() + (y - py).abs() <= 1)
                };
                let in_paw =
                    solid(&body.canvas, hands.x, hands.y) && things.iter().any(|&at| touching(at));
                let mouth = body.mouth;
                let at_mouth = things
                    .iter()
                    .any(|&(x, y)| (x - mouth.x).abs() <= 2 && (y - mouth.y).abs() <= 2);
                if !in_paw && !at_mouth {
                    misses.push(format!("{name} {clip:?} frame {frame}"));
                }
            }
        }
    }
    assert!(
        misses.is_empty(),
        "{} frames hold something away from both paw and mouth:\n{}",
        misses.len(),
        misses.join("\n")
    );
}

/// The mouth a frame carries mouthfuls to is the mouth its face draws: an inked pixel of the
/// face is on it or right beside it, on every body and in every clip.
#[test]
fn a_frames_mouth_is_where_its_face_draws_one() {
    let state = FaceRenderState {
        expression: ExpressionKind::Neutral,
        eyelids: EyelidPose::Open,
        gaze: GazeDirection::default(),
    };
    for (name, genome) in holding_subjects() {
        let face = CreatureRenderer::render_face_frame(&genome, state);
        for clip in BodyClip::baked() {
            for frame in 0..AnimationSpec::for_clip(clip).frames {
                let body = CreatureRenderer::render_body_frame(&genome, clip, frame, false);
                let half = FACE_FRAME_SIZE as i32 / 2;
                let x = body.mouth.x - body.face_anchor.x + half;
                let y = body.mouth.y - body.face_anchor.y + half;
                assert!(
                    (-1..=1).any(|dy| (-1..=1).any(|dx| face.get(x + dx, y + dy).a > 0)),
                    "{name} {clip:?} frame {frame}: no mouth drawn at {:?}",
                    body.mouth
                );
            }
        }
    }
}

/// The pixels the face drawn over a frame covers, in the expressions play and a meal wear, as
/// the overlay places it: centred on the frame's face anchor.
fn face_pixels(genome: &AppearanceGenome, anchor: PixelPoint) -> Vec<(i32, i32)> {
    let half = FACE_FRAME_SIZE as i32 / 2;
    let mut pixels = Vec::new();
    for expression in [ExpressionKind::Joy, ExpressionKind::Neutral] {
        let face = CreatureRenderer::render_face_frame(
            genome,
            FaceRenderState {
                expression,
                eyelids: EyelidPose::Open,
                gaze: GazeDirection::default(),
            },
        );
        for y in 0..FACE_FRAME_SIZE as i32 {
            for x in 0..FACE_FRAME_SIZE as i32 {
                if face.get(x, y).a > 0 {
                    pixels.push((anchor.x - half + x, anchor.y - half + y));
                }
            }
        }
    }
    pixels
}

/// Keep-up's toy never goes across the face, on any beat, any body or either end of the size
/// range: it is beside the paw, ahead of the face or over the head instead. On the beat it is
/// caught, on the crouch, it touches the paw keeping it up, on it or beside it along a row or a
/// column; a body with no paw free has it on top of its head, touching the head.
#[test]
fn keep_ups_toy_never_crosses_the_face_and_is_caught_in_the_paw() {
    let play = BodyClip::Action(ActionKind::SoloPlay);
    let mut misses = Vec::new();
    for (name, genome) in holding_subjects() {
        for frame in 0..AnimationSpec::for_clip(play).frames {
            let body = CreatureRenderer::render_body_frame(&genome, play, frame, false);
            let held = body.held.expect("keep-up always holds its toy");
            let mut prop = Canvas::new(FRAME_SIZE, FRAME_SIZE);
            held.paint(&mut prop, true);
            let solid = |canvas: &Canvas, x: i32, y: i32| canvas.get(x, y).a > 0;
            let over = face_pixels(&genome, body.face_anchor)
                .into_iter()
                .filter(|&(x, y)| solid(&prop, x, y))
                .count();
            if over > 0 {
                misses.push(format!(
                    "{name} frame {frame}: {over} toy pixels over the face"
                ));
            }
            if frame % 4 != 0 {
                continue;
            }
            let near = |(x, y): (i32, i32)| {
                [(0, 0), (1, 0), (-1, 0), (0, 1), (0, -1)]
                    .into_iter()
                    .any(|(dx, dy)| solid(&prop, x + dx, y + dy))
            };
            let caught = if body.paw {
                let hands = body.hands;
                (hands.y - 1..=hands.y + 1)
                    .flat_map(|y| (hands.x - 1..=hands.x + 1).map(move |x| (x, y)))
                    .filter(|&(x, y)| solid(&body.canvas, x, y))
                    .any(near)
            } else {
                (0..FRAME_SIZE as i32)
                    .flat_map(|y| (0..FRAME_SIZE as i32).map(move |x| (x, y)))
                    .filter(|&(x, y)| solid(&body.canvas, x, y))
                    .any(near)
            };
            if !caught {
                let hands = body.hands;
                let gap = (0..FRAME_SIZE as i32)
                    .flat_map(|y| (0..FRAME_SIZE as i32).map(move |x| (x, y)))
                    .filter(|&(x, y)| solid(&prop, x, y))
                    .map(|(x, y)| (x - hands.x).abs() + (y - hands.y).abs())
                    .min();
                misses.push(format!(
                    "{name} frame {frame}: the toy is caught {gap:?} pixels from the paw"
                ));
            }
        }
    }
    assert!(misses.is_empty(), "{}", misses.join("\n"));
}

/// A mouthful goes into the mouth: on the frame it is bitten some of it is within two pixels of
/// the mouth the face is drawn with, and while it is chewed nothing is held at all, on every body.
#[test]
fn a_mouthful_is_bitten_at_the_mouth_and_chewed_with_nothing_held() {
    let eat = BodyClip::Action(ActionKind::Eat);
    for (name, genome) in holding_subjects() {
        for frame in 0..AnimationSpec::for_clip(eat).frames {
            let body = CreatureRenderer::render_body_frame(&genome, eat, frame, false);
            match MOUTHFUL[usize::from(frame % 4)] {
                Mouthful::Chewing => assert_eq!(
                    body.held, None,
                    "{name} frame {frame}: something is held while it chews"
                ),
                Mouthful::Raised => assert!(body.held.is_some(), "{name} frame {frame}"),
                Mouthful::Bitten => {
                    let held = body.held.expect("a mouthful is held as it is bitten");
                    let mut snack = Canvas::new(FRAME_SIZE, FRAME_SIZE);
                    held.paint(&mut snack, true);
                    let mouth = body.mouth;
                    assert!(
                        (-2..=2).any(
                            |dy| (-2..=2).any(|dx| snack.get(mouth.x + dx, mouth.y + dy).a > 0)
                        ),
                        "{name} frame {frame}: the mouthful misses the mouth at {mouth:?}"
                    );
                }
            }
        }
    }
}
