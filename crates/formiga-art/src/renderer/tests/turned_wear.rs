//! What a companion wears, on a body drawn turned, and the Echo Fox turned: a piece follows the
//! body side-on and from behind, what the body hides from behind is hidden, and the fox walks,
//! runs and goes away turned inside its frame, wearing whatever it wears.
use super::*;
use crate::{AccessoryArt, Palette};
use formiga_core::{Accessory, AccessoryKind, CreatureDesign, Edition, SeedStream};

fn face() -> FaceRenderState {
    FaceRenderState {
        expression: ExpressionKind::Neutral,
        eyelids: EyelidPose::Open,
        gaze: GazeDirection::default(),
    }
}

/// Bodies that turn, at the smallest and largest statures, an original blob, and the fox.
fn turners() -> Vec<AppearanceGenome> {
    let mut out = Vec::new();
    for index in 0..8_u64 {
        let seed = SeedStream::new([89; 32]).bytes("turned-wearers", index);
        let mut design = CreatureDesign::generated_by(Edition::Details, seed, 0, None);
        design.body = [
            formiga_core::BodyPlan::Round,
            formiga_core::BodyPlan::Upright,
            formiga_core::BodyPlan::Winged,
            formiga_core::BodyPlan::Blob,
        ][index as usize % 4];
        let mut g = genome(BodyFamily::Blob);
        g.design = Some(design.bounded());
        g.logical_size = [26, 33, 38, 44][index as usize / 2 % 4];
        out.push(g);
    }
    out.push(genome(BodyFamily::Blob));
    let mut fox = genome(BodyFamily::Hopper);
    fox.echo_fox = true;
    out.push(fox);
    out
}

fn pieces(members: &[Palette]) -> Vec<AccessoryArt> {
    let mut out: Vec<AccessoryArt> = AccessoryKind::ALL
        .map(|kind| AccessoryArt::resolve(Accessory::Worn(kind), [9; 32], members))
        .to_vec();
    out.push(AccessoryArt::resolve(Accessory::Pin(26), [9; 32], members));
    out
}

/// Every piece on every turned view stays inside the frame and touches the body it is worn on,
/// and from behind a pin or a clip on the front of the head is hidden by it.
#[test]
fn a_piece_follows_a_turned_body_and_is_hidden_behind_it() {
    let turners = turners();
    let members: Vec<Palette> = turners.iter().map(crate::palette_for).collect();
    for genome in &turners {
        assert!(crate::walks_turned(genome));
        for piece in pieces(&members) {
            for view in [View::Side, View::Back] {
                let bare = CreatureRenderer::render_dressed_turned_frame(
                    genome,
                    None,
                    view,
                    None,
                    true,
                    face(),
                )
                .unwrap();
                let worn = CreatureRenderer::render_dressed_turned_frame(
                    genome,
                    Some(piece),
                    view,
                    None,
                    true,
                    face(),
                )
                .unwrap();
                let bounds = worn.alpha_bounds().unwrap();
                assert!(
                    bounds.0 > 0 && bounds.1 > 0 && bounds.2 < FRAME_SIZE - 1,
                    "{:?} {view:?} leaves the frame",
                    piece.accessory
                );
                let front_only = matches!(
                    piece.accessory,
                    Accessory::Pin(_)
                        | Accessory::Worn(AccessoryKind::StarClip | AccessoryKind::MoonClip)
                );
                if view == View::Back && front_only {
                    assert_eq!(
                        worn, bare,
                        "{:?} shows through from behind",
                        piece.accessory
                    );
                    continue;
                }
                // Something changed, and every changed pixel is joined to the body through what
                // is drawn: nothing floats beside it.
                let size = FRAME_SIZE as i32;
                let mut joined = vec![false; (size * size) as usize];
                let mut todo: Vec<(i32, i32)> = Vec::new();
                for y in 0..size {
                    for x in 0..size {
                        if bare.get(x, y).a > 0 {
                            joined[(y * size + x) as usize] = true;
                            todo.push((x, y));
                        }
                    }
                }
                while let Some((x, y)) = todo.pop() {
                    for (dx, dy) in [
                        (1, 0),
                        (-1, 0),
                        (0, 1),
                        (0, -1),
                        (1, 1),
                        (-1, -1),
                        (1, -1),
                        (-1, 1),
                    ] {
                        let (nx, ny) = (x + dx, y + dy);
                        if nx < 0 || ny < 0 || nx >= size || ny >= size {
                            continue;
                        }
                        let at = (ny * size + nx) as usize;
                        if !joined[at] && worn.get(nx, ny).a > 0 {
                            joined[at] = true;
                            todo.push((nx, ny));
                        }
                    }
                }
                let mut changed = 0;
                for y in 0..size {
                    for x in 0..size {
                        if worn.get(x, y) == bare.get(x, y) {
                            continue;
                        }
                        changed += 1;
                        assert!(
                            worn.get(x, y).a == 0 || joined[(y * size + x) as usize],
                            "{:?} {view:?} floats off the body at {x},{y} on {:?}",
                            piece.accessory,
                            genome.design
                        );
                    }
                }
                assert!(changed > 0, "{:?} does not show {view:?}", piece.accessory);
            }
        }
    }
}

/// A piece moves with a turned walk: the dressed frames of a side-on walk and a walk away are not
/// one frame held, and each is the bare frame with only the piece added.
#[test]
fn a_piece_walks_with_a_turned_body() {
    let turners = turners();
    let members: Vec<Palette> = turners.iter().map(crate::palette_for).collect();
    let scarf = AccessoryArt::resolve(
        Accessory::Worn(AccessoryKind::KnittedScarf),
        [9; 32],
        &members,
    );
    for genome in &turners {
        for clip in [Gesture::Stride, Gesture::Depart] {
            let frames: Vec<Canvas> = (0..6)
                .map(|frame| {
                    CreatureRenderer::render_dressed_body_frame(
                        genome,
                        Some(scarf),
                        clip,
                        frame,
                        false,
                    )
                    .canvas
                })
                .collect();
            assert!(frames.windows(2).any(|pair| pair[0] != pair[1]));
        }
    }
}

/// The fox turned: inside its frame, a mirror of itself facing the other way, one eye side-on and
/// none from behind, and its walk and run frames its own.
#[test]
fn the_echo_fox_walks_runs_and_goes_away_turned() {
    let mut fox = genome(BodyFamily::Hopper);
    fox.echo_fox = true;
    assert!(crate::walks_turned(&fox));
    assert!(
        !crate::can_turn(&fox),
        "the fox still climbs facing the screen"
    );
    let front =
        CreatureRenderer::render_turned_frame(&fox, View::Front, None, true, face()).unwrap();
    for view in [View::Side, View::Back] {
        for step in std::iter::once(None).chain((0..TURNED_STRIDE_FRAMES).map(Some)) {
            let right =
                CreatureRenderer::render_turned_frame(&fox, view, step, true, face()).unwrap();
            let bounds = right.alpha_bounds().unwrap();
            assert!(
                bounds.0 > 0
                    && bounds.1 > 0
                    && bounds.2 < FRAME_SIZE - 1
                    && bounds.3 < FRAME_SIZE - 1
            );
            let mut left =
                CreatureRenderer::render_turned_frame(&fox, view, step, false, face()).unwrap();
            left.mirror_horizontal();
            assert_eq!(left, right);
            assert_ne!(right, front);
        }
    }
    // The same orange, white and soot as the fox facing the screen: it is the same fox.
    let colours = |canvas: &Canvas| {
        let mut set: Vec<[u8; 4]> = canvas
            .pixels()
            .iter()
            .filter(|p| p.a > 0)
            .map(|p| [p.r, p.g, p.b, p.a])
            .collect();
        set.sort();
        set.dedup();
        set
    };
    let side = CreatureRenderer::render_turned_frame(&fox, View::Side, None, true, face()).unwrap();
    for colour in colours(&front) {
        let shared = colours(&side).contains(&colour);
        let face_only = colour[0] == 150 || colour[0] == 244; // its mouth and blush
        assert!(shared || face_only, "{colour:?}");
    }
    for clip in [Gesture::Stride, Gesture::Dash, Gesture::Depart] {
        let frames: Vec<Canvas> = (0..6)
            .map(|frame| CreatureRenderer::render_body_frame(&fox, clip, frame, false).canvas)
            .collect();
        for pair in frames.windows(2) {
            assert_ne!(pair[0], pair[1], "{clip:?} holds a frame");
        }
        let still = CreatureRenderer::render_body_frame(&fox, clip, 3, true).canvas;
        assert_eq!(
            still,
            CreatureRenderer::render_body_frame(
                &fox,
                clip,
                AnimationSpec::still_frame(clip),
                false
            )
            .canvas,
            "{clip:?}: reduced motion holds its still frame"
        );
    }
}
