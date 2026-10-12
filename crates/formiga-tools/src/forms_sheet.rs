//! Every body plan Formiga Farm sculpts, in the poses it is seen in most, bare and dressed.
//!
//!   cargo run -p formiga-tools -- forms-sheet [--output docs/assets/forms-sheet.png]
//!
//! One band per plan, its starting form as Farm offers it, first bare and then wearing a hat, a
//! scarf, a satchel and a snail pack. Along each row it rests, walks, eats, drinks, sleeps,
//! cheers, plays and is carried, drawn through `BodyPresentation` with the face the desktop
//! resolves, so the sheet shows what the overlay shows.

use crate::{blit_scaled_square_alpha, write_png};
use anyhow::Result;
use formiga_art::{AccessoryArt, BodyPresentation, CreatureRenderer, FRAME_SIZE};
use formiga_core::forms::{Design, Form, Plan, Sculpt};
use formiga_core::*;
use std::path::PathBuf;
use time::OffsetDateTime;

const SCALE: u32 = 2;

/// The poses each form is drawn in.
const POSES: [(ActionKind, f32, Option<Gesture>); 8] = [
    (ActionKind::Idle, 0.0, None),
    (ActionKind::Traverse, 0.25, None),
    (ActionKind::Eat, 0.3, None),
    (ActionKind::Drink, 0.3, None),
    (ActionKind::Sleep, 0.0, None),
    (ActionKind::InspectScreen, 0.1, Some(Gesture::Cheer)),
    (ActionKind::SoloPlay, 0.4, None),
    (ActionKind::Dragged, 0.0, None),
];

/// What each form is drawn wearing, a row each.
const DRESSES: [Option<AccessoryKind>; 5] = [
    None,
    Some(AccessoryKind::LeafHat),
    Some(AccessoryKind::KnittedScarf),
    Some(AccessoryKind::TinySatchel),
    Some(AccessoryKind::SnailPack),
];

fn posed(
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
    let preview = World::preview_adult(
        [31; 32],
        OffsetDateTime::UNIX_EPOCH,
        &DesktopSnapshot::default(),
    );
    let subjects: Vec<Creature> = Plan::ALL
        .into_iter()
        .map(|plan| {
            let mut creature = preview.clone();
            creature.appearance = Design {
                form: Form::Sculpted {
                    sculpt: Sculpt::starter(plan),
                },
                face: creature.appearance.face,
            }
            .genome(&creature.appearance);
            creature
        })
        .collect();
    let members: Vec<formiga_art::Palette> = subjects
        .iter()
        .map(|creature| formiga_art::palette_for(&creature.appearance))
        .collect();
    let colony_seed = [61_u8; 32];
    let cell = FRAME_SIZE * SCALE;
    let width = POSES.len() as u32 * cell;
    let height = (subjects.len() * DRESSES.len()) as u32 * cell;
    let mut pixels = vec![0_u8; (width * height * 4) as usize];
    for (index, subject) in subjects.iter().enumerate() {
        for (dress_index, dress) in DRESSES.into_iter().enumerate() {
            let row = (index * DRESSES.len() + dress_index) as u32;
            // Alternate pale and dark bands, a plan to each, so a form is seen on both.
            let band = if index % 2 == 0 {
                [236, 232, 222, 255]
            } else {
                [34, 38, 46, 255]
            };
            for y in row * cell..(row + 1) * cell {
                for x in 0..width {
                    let at = ((y * width + x) * 4) as usize;
                    pixels[at..at + 4].copy_from_slice(&band);
                }
            }
            let dress = dress
                .map(|kind| AccessoryArt::resolve(Accessory::Worn(kind), colony_seed, &members));
            for (column, pose) in POSES.into_iter().enumerate() {
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
                blit_scaled_square_alpha(
                    &mut pixels,
                    width,
                    column as u32 * cell,
                    row * cell,
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
