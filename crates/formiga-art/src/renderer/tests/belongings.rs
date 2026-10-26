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
