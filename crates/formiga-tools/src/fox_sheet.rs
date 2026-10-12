//! The Echo Fox through every clip, and every face it makes.
//!
//!   cargo run -p formiga-tools -- fox-sheet [--output docs/assets/fox-sheet.png]
//!
//! One row per clip a companion's body has, actions and then gestures, each frame left to right
//! with the face the desktop shows for that action, on a pale ground so the outline reads. The
//! last rows are the face alone: every expression with its eyes open, then the three eyelids
//! and the eight ways it looks about.

use crate::{blit_scaled, fill_rect, write_png};
use anyhow::Result;
use formiga_art::{
    AnimationSpec, BodyClip, CreatureRenderer, ExpressionKind, EyelidPose, FRAME_SIZE,
    FaceRenderState, GazeDirection,
};
use formiga_core::{ActionKind, World};
use std::path::PathBuf;
use time::OffsetDateTime;

const SCALE: u32 = 3;
const GROUND: [u8; 4] = [236, 240, 228, 255];

pub fn run(path: PathBuf) -> Result<()> {
    let world = World::new(
        [7; 32],
        OffsetDateTime::UNIX_EPOCH,
        &crate::fixture_desktop(),
    );
    let mut genome = world.save.creatures[0].appearance.clone();
    genome.echo_fox = true;

    let clips: Vec<BodyClip> = BodyClip::baked().collect();
    let cols = clips
        .iter()
        .map(|clip| u32::from(AnimationSpec::for_clip(*clip).frames))
        .max()
        .unwrap_or(1)
        .max(8);
    let cell = FRAME_SIZE * SCALE;
    let faces = faces();
    let rows = clips.len() as u32 + (faces.len() as u32).div_ceil(cols);
    let (width, height) = (cols * cell, rows * cell);
    let mut pixels = vec![0_u8; (width * height * 4) as usize];
    fill_rect(&mut pixels, width, 0, 0, width, height, GROUND);

    for (row, clip) in clips.iter().enumerate() {
        for frame in 0..AnimationSpec::for_clip(*clip).frames {
            let canvas = match clip {
                BodyClip::Action(action) => {
                    CreatureRenderer::render_frame(&genome, *action, frame, true)
                }
                BodyClip::Gesture(_) => CreatureRenderer::render_composited_frame(
                    &genome,
                    *clip,
                    frame,
                    true,
                    false,
                    FaceRenderState {
                        expression: ExpressionKind::Content,
                        eyelids: EyelidPose::Open,
                        gaze: GazeDirection::default(),
                    },
                ),
            };
            blit_scaled(
                &mut pixels,
                width,
                u32::from(frame) * cell,
                row as u32 * cell,
                &canvas.rgba_bytes(),
                SCALE,
            );
        }
    }

    // The faces, each over an idle body so it is seen where it sits.
    for (index, face) in faces.into_iter().enumerate() {
        let index = index as u32;
        let canvas = CreatureRenderer::render_composited_frame(
            &genome,
            ActionKind::Idle,
            0,
            true,
            false,
            face,
        );
        blit_scaled(
            &mut pixels,
            width,
            (index % cols) * cell,
            (clips.len() as u32 + index / cols) * cell,
            &canvas.rgba_bytes(),
            SCALE,
        );
    }

    write_png(&path, width, height, &pixels)?;
    println!("wrote {}", path.display());
    Ok(())
}

/// Every expression with its eyes open, then each eyelid, then each way it looks.
fn faces() -> Vec<FaceRenderState> {
    ExpressionKind::ALL
        .into_iter()
        .map(|expression| FaceRenderState {
            expression,
            eyelids: EyelidPose::Open,
            gaze: GazeDirection::default(),
        })
        .chain(EyelidPose::ALL.into_iter().map(|eyelids| FaceRenderState {
            expression: ExpressionKind::Neutral,
            eyelids,
            gaze: GazeDirection::default(),
        }))
        .chain(
            [
                (-1, -1),
                (0, -1),
                (1, -1),
                (-1, 0),
                (1, 0),
                (-1, 1),
                (0, 1),
                (1, 1),
            ]
            .map(|(x, y)| FaceRenderState {
                expression: ExpressionKind::Neutral,
                eyelids: EyelidPose::Open,
                gaze: GazeDirection::new(x, y),
            }),
        )
        .collect()
}
