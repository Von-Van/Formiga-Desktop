//! The blink-and-change between expressions, and the mouths that tell the look-alike expressions
//! apart.
use super::*;
use crate::renderer::face::{FaceDrawing, draw_layout_mouth, draw_mouth, face_drawing};
use crate::renderer::face_change::{blinks_between, eyes, mouth};
use formiga_core::{DesktopSnapshot, World};

const MOUTHS: [MouthStyle; 4] = [
    MouthStyle::Tiny,
    MouthStyle::Smile,
    MouthStyle::Cat,
    MouthStyle::Beak,
];

/// The four expressions that drew one mouth between two of them — rest and sleepy, a question
/// and a fright — and so told themselves apart only above it.
const LOOK_ALIKES: [ExpressionKind; 4] = [
    ExpressionKind::Neutral,
    ExpressionKind::Sleepy,
    ExpressionKind::Curious,
    ExpressionKind::Startled,
];

/// A classic face of each family with `mouth` and `brows`, cheeks that are always drawn.
fn classic(family: BodyFamily, mouth: MouthStyle, brows: BrowStyle) -> AppearanceGenome {
    let mut genome = genome(family);
    genome.face.mouth_style = mouth;
    genome.face.brow_style = brows;
    genome
}

/// A companion's own recipe worn with every authored layout and every mouth gene, on a round
/// body and on a blob, which wears some layouts snug or a row lower.
fn layouts() -> Vec<AppearanceGenome> {
    let mut genome = World::preview_adult(
        [23; 32],
        time::OffsetDateTime::UNIX_EPOCH,
        &DesktopSnapshot::default(),
    )
    .appearance;
    let base = genome.design.expect("a new companion has a recipe");
    let mut all = Vec::new();
    for body in [formiga_core::BodyPlan::Round, formiga_core::BodyPlan::Blob] {
        for template in 1..=formiga_core::FACE_TEMPLATES {
            for mouth in MOUTHS {
                genome.design = Some(formiga_core::CreatureDesign {
                    face_template: template,
                    body,
                    ..base
                });
                genome.face.mouth_style = mouth;
                all.push(genome.clone());
            }
        }
    }
    assert!(all.iter().any(beaked), "a beak among the layouts");
    all
}

fn classics() -> Vec<AppearanceGenome> {
    let mut all = Vec::new();
    for family in [
        BodyFamily::Blob,
        BodyFamily::Hopper,
        BodyFamily::SoftQuadruped,
    ] {
        for mouth in MOUTHS {
            for brows in [BrowStyle::None, BrowStyle::Soft, BrowStyle::Bold] {
                all.push(classic(family, mouth, brows));
            }
        }
    }
    all
}

fn face(genome: &AppearanceGenome, expression: ExpressionKind) -> Canvas {
    CreatureRenderer::render_face_frame(
        genome,
        FaceRenderState {
            expression,
            eyelids: EyelidPose::Open,
            gaze: GazeDirection::default(),
        },
    )
}

/// The mouth alone that `genome`'s face draws for `expression`, round the middle of a blank
/// face-sized canvas, by the drawing that face is drawn with.
fn mouth_alone(genome: &AppearanceGenome, expression: ExpressionKind) -> Canvas {
    let mut canvas = Canvas::new(FACE_FRAME_SIZE, FACE_FRAME_SIZE);
    let palette = crate::palette_for(genome);
    let center = FACE_FRAME_SIZE as i32 / 2;
    let mut face = genome.face;
    match face_drawing(genome) {
        FaceDrawing::Layout { mouth } => {
            face.mouth_style = mouth;
            draw_layout_mouth(&mut canvas, face, palette, center, center, expression);
        }
        FaceDrawing::Classic { .. } => {
            draw_mouth(&mut canvas, face, palette, center, center, expression);
        }
        FaceDrawing::Fox => unreachable!("no fox here"),
    }
    canvas
}

/// Every offset at which `mouth`, drawn round the middle of a blank canvas, is drawn in `face`:
/// where every pixel of it is a pixel of the face.
fn offsets(face: &Canvas, mouth: &Canvas) -> Vec<(i32, i32)> {
    let size = FACE_FRAME_SIZE as i32;
    let drawn: Vec<(i32, i32, Rgba)> = (0..size)
        .flat_map(|y| (0..size).map(move |x| (x, y)))
        .filter_map(|(x, y)| {
            let pixel = mouth.pixels()[(y * size + x) as usize];
            (pixel.a > 0).then_some((x, y, pixel))
        })
        .collect();
    (-size..size)
        .flat_map(|dy| (-size..size).map(move |dx| (dx, dy)))
        .filter(|&(dx, dy)| {
            drawn.iter().all(|&(x, y, pixel)| {
                let (fx, fy) = (x + dx, y + dy);
                (0..size).contains(&fx)
                    && (0..size).contains(&fy)
                    && face.pixels()[(fy * size + fx) as usize] == pixel
            })
        })
        .collect()
}

fn appears(face: &Canvas, mouth: &Canvas) -> bool {
    !offsets(face, mouth).is_empty()
}

/// Whether `genome`'s faces for `a` and `b` are the same everywhere but where either mouth is
/// drawn: found by where a yawn's mouth, which no other expression draws, sits in a yawn.
fn same_above_the_mouth(genome: &AppearanceGenome, a: ExpressionKind, b: ExpressionKind) -> bool {
    let yawn = ExpressionKind::Yawning;
    let [(dx, dy)] = offsets(&face(genome, yawn), &mouth_alone(genome, yawn))[..] else {
        panic!("a yawn's mouth sits in one place in a yawn");
    };
    let size = FACE_FRAME_SIZE as i32;
    let (mouth_a, mouth_b) = (mouth_alone(genome, a), mouth_alone(genome, b));
    let (face_a, face_b) = (face(genome, a), face(genome, b));
    (0..size)
        .flat_map(|y| (0..size).map(move |x| (x, y)))
        .all(|(x, y)| {
            let (mx, my) = (x - dx, y - dy);
            let under_mouth = (0..size).contains(&mx)
                && (0..size).contains(&my)
                && (mouth_a.pixels()[(my * size + mx) as usize].a > 0
                    || mouth_b.pixels()[(my * size + mx) as usize].a > 0);
            under_mouth
                || face_a.pixels()[(y * size + x) as usize]
                    == face_b.pixels()[(y * size + x) as usize]
        })
}

fn beaked(genome: &AppearanceGenome) -> bool {
    face_drawing(genome)
        == (FaceDrawing::Layout {
            mouth: MouthStyle::Beak,
        })
}

#[test]
fn the_four_look_alike_expressions_draw_pairwise_distinct_mouths() {
    for genome in classics().iter().chain(layouts().iter()) {
        // A beak on an authored layout stays a beak, which has only its few shapes.
        if beaked(genome) {
            continue;
        }
        let mouths: Vec<_> = LOOK_ALIKES
            .iter()
            .map(|&expression| mouth_alone(genome, expression).rgba_bytes())
            .collect();
        for (a, first) in mouths.iter().enumerate() {
            for (b, second) in mouths.iter().enumerate().skip(a + 1) {
                assert_ne!(
                    first,
                    second,
                    "{:?} and {:?} share a mouth on {:?}",
                    LOOK_ALIKES[a],
                    LOOK_ALIKES[b],
                    face_drawing(genome),
                );
            }
        }
    }
}

/// The blink-and-change's two keys say what the drawing draws: two expressions share a mouth key
/// exactly where they draw the same mouth, and an eyes key exactly where the rest of the face is
/// the same.
#[test]
fn face_change_keys_follow_the_drawing() {
    for genome in classics().iter().chain(layouts().iter()) {
        let drawing = face_drawing(genome);
        for (index, &a) in ExpressionKind::ALL.iter().enumerate() {
            for &b in &ExpressionKind::ALL[index + 1..] {
                let same_mouth =
                    mouth_alone(genome, a).rgba_bytes() == mouth_alone(genome, b).rgba_bytes();
                assert_eq!(
                    mouth(drawing, a) == mouth(drawing, b),
                    same_mouth,
                    "{a:?} and {b:?} mouths on {drawing:?}"
                );
                assert_eq!(
                    eyes(drawing, a) == eyes(drawing, b),
                    same_above_the_mouth(genome, a, b),
                    "{a:?} and {b:?} eyes on {drawing:?}"
                );
            }
        }
    }
}

/// The face every look-alike pair used to share is told apart now in the frame as drawn, not only
/// in the mouth drawn alone.
#[test]
fn sleepy_and_startled_faces_differ_from_their_old_twins_at_the_mouth() {
    for genome in classics() {
        for (old_twin, changed) in [
            (ExpressionKind::Neutral, ExpressionKind::Sleepy),
            (ExpressionKind::Curious, ExpressionKind::Startled),
        ] {
            let mouth = mouth_alone(&genome, changed);
            let face = face(&genome, changed);
            assert!(appears(&face, &mouth), "{changed:?} draws its mouth");
            assert!(
                !appears(&self::face(&genome, old_twin), &mouth),
                "{changed:?}'s mouth is not {old_twin:?}'s"
            );
        }
    }
}

#[test]
fn a_change_of_eyes_and_mouth_blinks_for_exactly_one_tick() {
    use ExpressionKind as E;
    let genome = classic(BodyFamily::Hopper, MouthStyle::Smile, BrowStyle::Soft);
    let drawing = face_drawing(&genome);
    assert!(eyes(drawing, E::Neutral) != eyes(drawing, E::Grumpy));
    assert!(mouth(drawing, E::Neutral) != mouth(drawing, E::Grumpy));
    let mut change = FaceChange::default();
    let ticks: Vec<bool> = [E::Neutral, E::Neutral, E::Neutral]
        .into_iter()
        .chain([E::Grumpy; 5])
        .map(|expression| change.advance_to(&genome, expression, false))
        .collect();
    assert_eq!(
        ticks,
        [false, false, false, true, false, false, false, false]
    );

    // On its tick the face shown is the plain one, half-lidded, looking where it was looking.
    let mut change = FaceChange::default();
    change.advance_to(&genome, E::Joy, false);
    assert!(change.advance_to(&genome, E::Worried, false));
    let face = FaceRenderState {
        expression: E::Worried,
        eyelids: EyelidPose::Open,
        gaze: GazeDirection::new(1, -1),
    };
    assert_eq!(
        change.shown(face),
        FaceRenderState {
            expression: FACE_CHANGE_FACE.0,
            eyelids: FACE_CHANGE_FACE.1,
            gaze: GazeDirection::new(1, -1),
        }
    );
    assert!(!change.advance_to(&genome, E::Worried, false));
    assert_eq!(change.shown(face), face);
}

#[test]
fn a_change_of_only_the_eyes_or_only_the_mouth_cuts_straight_across() {
    use ExpressionKind as E;
    let classic = classic(BodyFamily::Blob, MouthStyle::Cat, BrowStyle::Soft);
    let layout = layouts()
        .into_iter()
        .find(|genome| matches!(face_drawing(genome), FaceDrawing::Layout { mouth } if mouth != MouthStyle::Beak))
        .expect("a layout with a mouth");
    for (genome, from, to) in [
        // Classic: the brows and eyes stay, the mouth goes from rest to a smile or a scowl.
        (&classic, E::Neutral, E::Content),
        (&classic, E::Neutral, E::Joy),
        (&classic, E::Focused, E::Determined),
        (&classic, E::Bored, E::Sleepy),
        // Layout: the eyes stay open and plain, or smiling shut, and the mouth changes; or the
        // mouth stays and the eyes narrow.
        (&layout, E::Content, E::Neutral),
        (&layout, E::Focused, E::Determined),
        (&layout, E::Joy, E::Affectionate),
    ] {
        let mut change = FaceChange::default();
        change.advance_to(genome, from, false);
        assert!(
            !change.advance_to(genome, to, false),
            "{from:?} to {to:?} on {:?}",
            face_drawing(genome)
        );
        // And both ways.
        let mut change = FaceChange::default();
        change.advance_to(genome, to, false);
        assert!(!change.advance_to(genome, from, false));
    }
}

#[test]
fn a_blink_and_change_never_comes_into_a_fright_twice_running_or_with_motion_reduced() {
    use ExpressionKind as E;
    let genome = classic(BodyFamily::SoftQuadruped, MouthStyle::Tiny, BrowStyle::Bold);
    let drawing = face_drawing(&genome);
    // A fright lands on its tick.
    assert!(!blinks_between(drawing, E::Joy, E::Startled));
    assert!(blinks_between(drawing, E::Startled, E::Joy));
    // The first face seen, and no change, never blink.
    let mut change = FaceChange::default();
    assert!(!change.advance_to(&genome, E::Grumpy, false));
    assert!(!change.advance_to(&genome, E::Grumpy, false));
    // A change straight after a blink has just been through one.
    assert!(change.advance_to(&genome, E::Joy, false));
    assert!(!change.advance_to(&genome, E::Worried, false));
    assert!(change.advance_to(&genome, E::Grumpy, false));
    // With motion reduced a change is the one swap.
    let mut change = FaceChange::default();
    for expression in [E::Joy, E::Worried, E::Grumpy, E::Joy] {
        assert!(!change.advance_to(&genome, expression, true));
        assert_eq!(change.shown(face_state(expression)), face_state(expression));
    }
}

fn face_state(expression: ExpressionKind) -> FaceRenderState {
    FaceRenderState {
        expression,
        eyelids: EyelidPose::Open,
        gaze: GazeDirection::default(),
    }
}

/// Through a companion as it lives: waking from sleep into the plain face changes the eyes and
/// the mouth, so the first awake tick is the blink.
#[test]
fn a_companion_waking_up_blinks_once_into_its_waking_face() {
    let mut creature = World::preview_adult(
        [23; 32],
        time::OffsetDateTime::UNIX_EPOCH,
        &DesktopSnapshot::default(),
    );
    creature.state.action = ActionKind::Sleep;
    let mut change = FaceChange::default();
    assert!(!change.advance(&creature, false));
    assert!(!change.advance(&creature, false));
    creature.state.action = ActionKind::Idle;
    creature.state.drives.sleep_pressure = 0.0;
    creature.state.drives.energy = 1.0;
    creature.state.drives.boredom = 0.0;
    creature.state.drives.comfort = 0.0;
    creature.state.drives.arousal = 0.0;
    assert_eq!(
        crate::renderer::face::resolve_expression(&creature),
        ExpressionKind::Neutral
    );
    assert!(change.advance(&creature, false));
    assert!(!change.advance(&creature, false));
}
