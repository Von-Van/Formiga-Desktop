//! The one-tick bridge between clips, tick by tick: every body walking, startled in mid-stride,
//! walking on and coming to rest, a frame every twentieth of a second as the overlay presents it.

use crate::{blit_scaled_square_alpha, fill_rect, reference_creatures, write_png};
use anyhow::Result;
use formiga_art::{
    BodyClip, BodyPresentation, ClipBridge, CreatureRenderer, FRAME_SIZE, FrameShape,
};
use formiga_core::*;
use std::collections::HashMap;
use std::path::PathBuf;
use time::OffsetDateTime;

/// Walking, a fright, walking on and a rest, each this many ticks.
const STRETCH: u32 = 8;
const COLUMNS: u32 = 4 * STRETCH;

/// Each body has two rows: on top, the cut the overlay made before the bridge, every tick as
/// the body's clip says; under it, what the overlay draws now, with each tick that passes through
/// the settle on a pale amber ground. Beneath every column but the first, a mark where the clip
/// changed. A change from or to the plain settle, or one that moves the outline less than
/// `formiga_art::BRIDGE_ROWS` rows, cuts as before.
pub fn run(path: PathBuf) -> Result<()> {
    const SCALE: u32 = 2;
    const GAP: u32 = 16;
    let preview = World::preview_adult(
        [29; 32],
        OffsetDateTime::UNIX_EPOCH,
        &DesktopSnapshot::default(),
    );
    let mut subjects = reference_creatures();
    for (index, plan) in BodyPlan::ALL.into_iter().enumerate() {
        let mut creature = preview.clone();
        let mut design = CreatureDesign::modular([29; 32], 0, None);
        design.body = plan;
        creature.appearance.design = Some(design);
        creature.id = preview.id.wrapping_add(index as u64 + 1);
        subjects.push(creature);
    }
    let cell = FRAME_SIZE * SCALE;
    let width = COLUMNS * cell;
    let block = 2 * cell + GAP;
    let height = subjects.len() as u32 * block;
    let mut pixels = vec![0_u8; (width * height * 4) as usize];
    for chunk in pixels.chunks_exact_mut(4) {
        chunk.copy_from_slice(&[236, 234, 228, 255]);
    }
    let mut bridges = 0;
    for (index, subject) in subjects.iter().enumerate() {
        let top = index as u32 * block;
        let mut previous: Option<BodyClip> = None;
        for (column, (creature, body, shown)) in ticks(subject).into_iter().enumerate() {
            let column = column as u32;
            let face = CreatureRenderer::resolve_face_state_among(
                &creature,
                &[],
                CursorSnapshot::default(),
                false,
                false,
            );
            if shown != body {
                bridges += 1;
                fill_rect(
                    &mut pixels,
                    width,
                    column * cell,
                    top + cell,
                    cell,
                    cell,
                    [246, 214, 140, 255],
                );
            }
            if previous.is_some_and(|clip| clip != body.clip) {
                fill_rect(
                    &mut pixels,
                    width,
                    column * cell,
                    top + 2 * cell + 2,
                    2,
                    GAP - 6,
                    [120, 116, 108, 255],
                );
            }
            previous = Some(body.clip);
            for (row, drawn) in [body, shown].into_iter().enumerate() {
                let rendered = CreatureRenderer::render_composited_frame(
                    &creature.appearance,
                    drawn.clip,
                    drawn.frame,
                    drawn.facing_right,
                    false,
                    face,
                );
                blit_scaled_square_alpha(
                    &mut pixels,
                    width,
                    column * cell,
                    top + row as u32 * cell,
                    &rendered.rgba_bytes(),
                    FRAME_SIZE,
                    SCALE,
                );
            }
        }
    }
    write_png(&path, width, height, &pixels)?;
    println!("wrote {} ({bridges} bridged ticks)", path.display());
    Ok(())
}

/// The scene's ticks for one subject: the creature as the world has it, the body its clip says,
/// and what the overlay draws through a [`ClipBridge`], the outline of each frame read from the
/// frame as the atlas bakes it.
fn ticks(subject: &Creature) -> Vec<(Creature, BodyPresentation, BodyPresentation)> {
    let mut creature = subject.clone();
    creature.state.attention = None;
    creature.state.beat = None;
    creature.state.flourish = None;
    creature.state.facing_right = true;
    creature.state.drives = Drives::default();
    let mark = PresenceMark {
        x: creature.state.position.x,
        depth: 0.0,
        window: None,
    };
    let mut outlines = HashMap::new();
    let mut bridge = ClipBridge::default();
    let mut since = 0.0;
    let mut stride = 0.0;
    (0..COLUMNS)
        .map(|tick| {
            let walking = tick < 3 * STRETCH;
            if tick == 3 * STRETCH {
                since = 0.0;
            }
            creature.state.attention =
                (STRETCH..2 * STRETCH)
                    .contains(&tick)
                    .then_some(AttentionPose {
                        target: creature.state.position,
                        emotion: AttentionEmotion::Startled,
                        hanging: 0.0,
                        gesture: Some(Gesture::Gasp),
                    });
            creature.state.action = if walking {
                ActionKind::Traverse
            } else {
                ActionKind::Idle
            };
            creature.state.action_elapsed = since;
            creature.state.presence = Presence {
                clock: 40.0 + tick as f32 * 0.05,
                since,
                stepping: walking,
                stride,
                last: Some(mark),
                ..Presence::default()
            };
            let body = BodyPresentation::for_creature(&creature);
            let (top, bottom, baked) =
                *outlines.entry((body.clip, body.frame)).or_insert_with(|| {
                    let rendered = CreatureRenderer::render_body_frame(
                        &creature.appearance,
                        body.clip,
                        body.frame,
                        false,
                    );
                    let (top, bottom) = rendered
                        .canvas
                        .alpha_bounds()
                        .map_or((0, FRAME_SIZE as u8), |(_, top, _, bottom)| {
                            (top as u8, (bottom + 1) as u8)
                        });
                    (top, bottom, rendered.held)
                });
            let shape = FrameShape {
                top,
                bottom,
                holding: formiga_art::held_now(&creature, body.clip, baked).is_some(),
            };
            bridge.advance(body, Some(shape), false);
            let shown = bridge.shown(body);
            since += 0.05;
            if walking {
                stride += 0.4;
            }
            (creature.clone(), body, shown)
        })
        .collect()
}
