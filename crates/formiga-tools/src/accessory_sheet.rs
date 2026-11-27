//! Everything a companion can wear, on every kind of body, in the poses it is seen in most.
//!
//!   cargo run -p formiga-tools -- accessory-sheet [--output docs/assets/accessory-sheet.png]
//!
//! One row per accessory, and two rows of keepsakes worn as pins. Along each row, seven bodies —
//! the three original families, a long modular body, a blob, a mini and the Echo Fox — each
//! resting, walking, asleep and cheering, drawn through `BodyPresentation` with the face the
//! desktop resolves, so the sheet shows what the overlay shows; and then walking side-on, walking
//! away and climbing with its back to the screen, so a piece is seen following a body turned.

use crate::{blit_scaled_square_alpha, reference_creatures, write_png};
use anyhow::Result;
use formiga_art::{AccessoryArt, BodyClip, BodyPresentation, CreatureRenderer, FRAME_SIZE};
use formiga_core::*;
use std::path::PathBuf;
use time::OffsetDateTime;

const SCALE: u32 = 2;

/// The poses each body is drawn in.
const POSES: [(ActionKind, f32, Option<Gesture>); 4] = [
    (ActionKind::Idle, 0.0, None),
    (ActionKind::Traverse, 0.25, None),
    (ActionKind::Sleep, 0.0, None),
    (ActionKind::InspectScreen, 0.1, Some(Gesture::Cheer)),
];

/// The turned clips each body is drawn in after its poses: a side-on walk, a walk away, and a
/// climb with its back to the screen. A body nothing draws turned shows what stands in for them.
const TURNED: [(BodyClip, u8); 3] = [
    (BodyClip::Gesture(Gesture::Stride), 1),
    (BodyClip::Gesture(Gesture::Depart), 1),
    (BodyClip::Gesture(Gesture::Scale), 2),
];

pub(crate) fn subjects() -> Vec<Creature> {
    let mut subjects: Vec<Creature> = reference_creatures()
        .into_iter()
        .map(|mut creature| {
            creature.appearance.design = None;
            creature
        })
        .collect();
    let preview = World::preview_adult(
        [31; 32],
        OffsetDateTime::UNIX_EPOCH,
        &DesktopSnapshot::default(),
    );
    for plan in [BodyPlan::Long, BodyPlan::Blob, BodyPlan::Round] {
        let mut creature = preview.clone();
        let mut design = CreatureDesign::modular([31; 32], 0, None);
        design.body = plan;
        creature.appearance.design = Some(design);
        subjects.push(creature);
    }
    // The last one as a mini.
    if let Some(mini) = subjects.last_mut() {
        mini.appearance.logical_size = 26;
    }
    let mut fox = preview;
    fox.appearance.echo_fox = true;
    subjects.push(fox);
    subjects
}

pub(crate) fn posed(
    subject: &Creature,
    (action, at, gesture): (ActionKind, f32, Option<Gesture>),
) -> Creature {
    let mut creature = subject.clone();
    let state = &mut creature.state;
    state.facing_right = true;
    state.flourish = None;
    state.velocity = Point::default();
    state.action = action;
    state.action_elapsed = at;
    state.attention = gesture.map(|gesture| AttentionPose {
        target: state.position,
        emotion: AttentionEmotion::Enjoying,
        hanging: 0.0,
        gesture: Some(gesture),
    });
    creature
}

pub fn run(path: PathBuf) -> Result<()> {
    let subjects = subjects();
    let members: Vec<formiga_art::Palette> = subjects
        .iter()
        .map(|creature| formiga_art::palette_for(&creature.appearance))
        .collect();
    let colony_seed = [61_u8; 32];
    let mut rows: Vec<Option<Accessory>> = vec![None];
    rows.extend(AccessoryKind::ALL.map(|kind| Some(Accessory::Worn(kind))));
    rows.extend([0_u8, 26, 58, 150].map(|variant| Some(Accessory::Pin(variant))));
    let cell = FRAME_SIZE * SCALE;
    let per = POSES.len() + TURNED.len();
    let columns = subjects.len() * per;
    let width = columns as u32 * cell;
    let height = rows.len() as u32 * cell;
    let mut pixels = vec![0_u8; (width * height * 4) as usize];
    for (row, accessory) in rows.iter().enumerate() {
        // Alternate pale and dark bands, so a piece is seen on both.
        let band = if row % 2 == 0 {
            [236, 232, 222, 255]
        } else {
            [34, 38, 46, 255]
        };
        for y in row as u32 * cell..(row as u32 + 1) * cell {
            for x in 0..width {
                let index = ((y * width + x) * 4) as usize;
                pixels[index..index + 4].copy_from_slice(&band);
            }
        }
        let dress =
            accessory.map(|accessory| AccessoryArt::resolve(accessory, colony_seed, &members));
        for (index, subject) in subjects.iter().enumerate() {
            for (pose_index, pose) in POSES.into_iter().enumerate() {
                let creature = posed(subject, pose);
                let body = BodyPresentation::for_creature(&creature);
                let face = CreatureRenderer::resolve_face_state(
                    &creature,
                    CursorSnapshot::default(),
                    false,
                );
                let canvas = CreatureRenderer::render_dressed_composited_frame(
                    &creature.appearance,
                    dress,
                    body.clip,
                    body.frame,
                    body.facing_right,
                    false,
                    face,
                );
                let column = index * per + pose_index;
                blit_scaled_square_alpha(
                    &mut pixels,
                    width,
                    column as u32 * cell,
                    row as u32 * cell,
                    &canvas.rgba_bytes(),
                    FRAME_SIZE,
                    SCALE,
                );
            }
            for (turned_index, (clip, frame)) in TURNED.into_iter().enumerate() {
                let face = formiga_art::FaceRenderState {
                    expression: formiga_art::ExpressionKind::Content,
                    eyelids: formiga_art::EyelidPose::Open,
                    gaze: formiga_art::GazeDirection::default(),
                };
                let canvas = CreatureRenderer::render_dressed_composited_frame(
                    &subject.appearance,
                    dress,
                    clip,
                    frame,
                    true,
                    false,
                    face,
                );
                let column = index * per + POSES.len() + turned_index;
                blit_scaled_square_alpha(
                    &mut pixels,
                    width,
                    column as u32 * cell,
                    row as u32 * cell,
                    &canvas.rgba_bytes(),
                    FRAME_SIZE,
                    SCALE,
                );
            }
        }
    }
    write_png(&path, width, height, &pixels)?;
    println!("wrote {}", path.display());
    Ok(())
}
