//! Review sheets of how a body moves: the everyday loops, every gesture with the face it is
//! judged with, the activity clips, and the climbing, hanging and finding clips.

use crate::{
    blit_scaled, blit_scaled_square_alpha, draw_rect_alpha, fill_gradient, reference_creatures,
    write_png,
};
use anyhow::Result;
use formiga_art::{
    CreatureRenderer, ExpressionKind, EyelidPose, FRAME_SIZE, FaceRenderState, GazeDirection,
};
use formiga_core::*;
use std::path::PathBuf;
use time::OffsetDateTime;

/// The face a gesture is reviewed with. A pose and its face have to be judged together, since
/// half of what a body says is said by where it is looking.
pub(crate) fn gesture_face(gesture: Gesture) -> FaceRenderState {
    let (expression, eyelids) = match gesture {
        Gesture::Cheer | Gesture::Bop => (ExpressionKind::Joy, EyelidPose::Open),
        Gesture::Gasp => (ExpressionKind::Startled, EyelidPose::Open),
        Gesture::Cover => (ExpressionKind::Worried, EyelidPose::Closed),
        Gesture::Worry | Gesture::Balance => (ExpressionKind::Worried, EyelidPose::Open),
        Gesture::Crouch | Gesture::Heave => (ExpressionKind::Determined, EyelidPose::Open),
        Gesture::Reach | Gesture::Watch => (ExpressionKind::Curious, EyelidPose::Open),
        // Shown as the desktop shows the top of it: eyes screwed shut.
        Gesture::Stretch => (ExpressionKind::Content, EyelidPose::Closed),
        Gesture::Yawn => (ExpressionKind::Yawning, EyelidPose::Closed),
        Gesture::Huff | Gesture::Stomp => (ExpressionKind::Grumpy, EyelidPose::Open),
        Gesture::Swoon => (ExpressionKind::Affectionate, EyelidPose::Closed),
        Gesture::Beg => (ExpressionKind::Pleading, EyelidPose::Open),
        // Pleased with itself: half-lidded and smiling.
        Gesture::Strut => (ExpressionKind::Smug, EyelidPose::Half),
        Gesture::Peek => (ExpressionKind::Worried, EyelidPose::Half),
        Gesture::Sit => (ExpressionKind::Content, EyelidPose::Open),
        Gesture::Scoot => (ExpressionKind::Joy, EyelidPose::Open),
        // Washing with its eyes shut, pleased with itself.
        Gesture::Groom => (ExpressionKind::Content, EyelidPose::Closed),
        Gesture::Shake => (ExpressionKind::Determined, EyelidPose::Closed),
        Gesture::Sniff => (ExpressionKind::Curious, EyelidPose::Half),
        Gesture::Sprawl | Gesture::Loaf => (ExpressionKind::Sleepy, EyelidPose::Closed),
        Gesture::Skip => (ExpressionKind::Joy, EyelidPose::Open),
        Gesture::Trudge => (ExpressionKind::Sleepy, EyelidPose::Half),
        Gesture::Pour | Gesture::Tend => (ExpressionKind::Focused, EyelidPose::Open),
        Gesture::Pluck => (ExpressionKind::Determined, EyelidPose::Open),
        // Eyes shut on a smile under the hand.
        Gesture::Bask => (ExpressionKind::Affectionate, EyelidPose::Closed),
    };
    FaceRenderState {
        expression,
        eyelids,
        // Watching is the one pose whose whole point is that the eyes are on something, so it is
        // reviewed the way the desktop shows it: aimed out ahead and a little down, at a window.
        gaze: if gesture == Gesture::Watch {
            GazeDirection::new(1, 1)
        } else {
            GazeDirection::default()
        },
    }
}

/// Every body the art can draw: the three reference creatures — a plain modular body from each of
/// the blob, hopper, and soft-quadruped families — then the five modular body plans on one shared
/// look, so a pose is judged across every plan.
fn gesture_subjects() -> Vec<AppearanceGenome> {
    let mut subjects: Vec<_> = reference_creatures()
        .into_iter()
        .map(|creature| creature.appearance)
        .collect();
    let preview = World::preview_adult(
        [29; 32],
        OffsetDateTime::UNIX_EPOCH,
        &DesktopSnapshot::default(),
    );
    for plan in BodyPlan::ALL {
        let mut appearance = preview.appearance.clone();
        // Classic limbs make their own gestures, reviewed on the classic sheet.
        let mut design = CreatureDesign::modular([29; 32], 0, None);
        design.body = plan;
        appearance.design = Some(design);
        subjects.push(appearance);
    }
    subjects
}

/// The loops a companion is seen in most — walking, running, resting, eating, sleeping — frame by
/// frame for every body, one row each, at 3x. A loop that reads as the same picture twice in a
/// row, or that hitches where it comes round, shows up here and nowhere else.
pub fn motion_sheet(path: PathBuf) -> Result<()> {
    const SCALE: u32 = 3;
    const LOOPS: [ActionKind; 5] = [
        ActionKind::Traverse,
        ActionKind::Sprint,
        ActionKind::Idle,
        ActionKind::Eat,
        ActionKind::Sleep,
    ];
    let subjects = gesture_subjects();
    let cell = FRAME_SIZE * SCALE;
    let columns: u32 = LOOPS
        .iter()
        .map(|action| u32::from(formiga_art::AnimationSpec::for_action(*action).frames))
        .sum::<u32>()
        + LOOPS.len() as u32
        - 1;
    let width = columns * cell;
    let height = subjects.len() as u32 * cell;
    let mut pixels = vec![0_u8; (width * height * 4) as usize];
    for chunk in pixels.chunks_exact_mut(4) {
        chunk.copy_from_slice(&[236, 234, 228, 255]);
    }
    for (row, genome) in subjects.iter().enumerate() {
        let mut column = 0_u32;
        for action in LOOPS {
            let spec = formiga_art::AnimationSpec::for_action(action);
            for frame in 0..spec.frames {
                let rendered = CreatureRenderer::render_frame(genome, action, frame, true);
                blit_scaled(
                    &mut pixels,
                    width,
                    column * cell,
                    row as u32 * cell,
                    &rendered.rgba_bytes(),
                    SCALE,
                );
                column += 1;
            }
            column += 1;
        }
    }
    write_png(&path, width, height, &pixels)?;
    println!("wrote {}", path.display());
    Ok(())
}

/// What every body does with itself, frame by frame with the face it is drawn with: washing,
/// shaking, sniffing the air, the three ways it sleeps — curled, sprawled and loafed — and the
/// three ways it walks — plainly, skipping and trudging. One row per body at 3x, a gap between
/// each clip, so the moods can be told apart at a glance and a pose that reads as something else
/// shows up beside the ones it might be mistaken for.
pub fn repertoire_sheet(path: PathBuf) -> Result<()> {
    const SCALE: u32 = 3;
    let clips: [(formiga_art::BodyClip, FaceRenderState); 10] = {
        use formiga_art::BodyClip::{Action, Gesture as G};
        let face = |expression, eyelids| FaceRenderState {
            expression,
            eyelids,
            gaze: GazeDirection::default(),
        };
        [
            (G(Gesture::Groom), gesture_face(Gesture::Groom)),
            (G(Gesture::Shake), gesture_face(Gesture::Shake)),
            (G(Gesture::Sniff), gesture_face(Gesture::Sniff)),
            (
                Action(ActionKind::Sleep),
                face(ExpressionKind::Sleepy, EyelidPose::Closed),
            ),
            (G(Gesture::Sprawl), gesture_face(Gesture::Sprawl)),
            (G(Gesture::Loaf), gesture_face(Gesture::Loaf)),
            (
                Action(ActionKind::Traverse),
                face(ExpressionKind::Content, EyelidPose::Open),
            ),
            (G(Gesture::Skip), gesture_face(Gesture::Skip)),
            (G(Gesture::Trudge), gesture_face(Gesture::Trudge)),
            (
                Action(ActionKind::Homebound),
                face(ExpressionKind::Content, EyelidPose::Open),
            ),
        ]
    };
    let subjects = gesture_subjects();
    let cell = FRAME_SIZE * SCALE;
    let columns: u32 = clips
        .iter()
        .map(|(clip, _)| u32::from(formiga_art::AnimationSpec::for_clip(*clip).frames))
        .sum::<u32>()
        + clips.len() as u32
        - 1;
    let width = columns * cell;
    let height = subjects.len() as u32 * cell;
    let mut pixels = vec![0_u8; (width * height * 4) as usize];
    for chunk in pixels.chunks_exact_mut(4) {
        chunk.copy_from_slice(&[236, 234, 228, 255]);
    }
    for (row, genome) in subjects.iter().enumerate() {
        let mut column = 0_u32;
        for (clip, face) in clips {
            for frame in 0..formiga_art::AnimationSpec::for_clip(clip).frames {
                let rendered = CreatureRenderer::render_composited_frame(
                    genome, clip, frame, true, false, face,
                );
                blit_scaled(
                    &mut pixels,
                    width,
                    column * cell,
                    row as u32 * cell,
                    &rendered.rgba_bytes(),
                    SCALE,
                );
                column += 1;
            }
            column += 1;
        }
    }
    write_png(&path, width, height, &pixels)?;
    println!("wrote {}", path.display());
    Ok(())
}

/// Every body going about the village's business, frame by frame with its face, at 2x — the
/// size a desktop at display scale 2 draws a companion — so what reads and what doesn't is judged
/// where it will be seen: watering, picking, tending, holding up what it grew, eating, drinking,
/// keeping a toy up, playing with a friend, saying hello, and being petted. What the village puts
/// in its hands, a watering can or what it picked, is drawn over the body by the overlay and is
/// not drawn here.
pub fn doings_sheet(path: PathBuf) -> Result<()> {
    const SCALE: u32 = 2;
    let clips: [(formiga_art::BodyClip, FaceRenderState); 10] = {
        use formiga_art::BodyClip::{Action, Gesture as G};
        let face = |expression, eyelids| FaceRenderState {
            expression,
            eyelids,
            gaze: GazeDirection::default(),
        };
        [
            (G(Gesture::Pour), gesture_face(Gesture::Pour)),
            (G(Gesture::Pluck), gesture_face(Gesture::Pluck)),
            (G(Gesture::Tend), gesture_face(Gesture::Tend)),
            (
                Action(ActionKind::PresentDiscovery),
                face(ExpressionKind::Joy, EyelidPose::Open),
            ),
            (
                Action(ActionKind::Eat),
                face(ExpressionKind::Content, EyelidPose::Open),
            ),
            (
                Action(ActionKind::Drink),
                face(ExpressionKind::Content, EyelidPose::Half),
            ),
            (
                Action(ActionKind::SoloPlay),
                face(ExpressionKind::Joy, EyelidPose::Open),
            ),
            (
                Action(ActionKind::SocialPlay),
                face(ExpressionKind::Joy, EyelidPose::Open),
            ),
            (
                Action(ActionKind::Greet),
                face(ExpressionKind::Affectionate, EyelidPose::Open),
            ),
            (G(Gesture::Bask), gesture_face(Gesture::Bask)),
        ]
    };
    let subjects = gesture_subjects();
    let cell = FRAME_SIZE * SCALE;
    let columns: u32 = clips
        .iter()
        .map(|(clip, _)| u32::from(formiga_art::AnimationSpec::for_clip(*clip).frames))
        .sum::<u32>()
        + clips.len() as u32
        - 1;
    let width = columns * cell;
    let height = subjects.len() as u32 * cell;
    let mut pixels = vec![0_u8; (width * height * 4) as usize];
    for chunk in pixels.chunks_exact_mut(4) {
        chunk.copy_from_slice(&[236, 234, 228, 255]);
    }
    for (row, genome) in subjects.iter().enumerate() {
        let mut column = 0_u32;
        for (clip, face) in clips {
            for frame in 0..formiga_art::AnimationSpec::for_clip(clip).frames {
                let rendered = CreatureRenderer::render_composited_frame(
                    genome, clip, frame, true, false, face,
                );
                blit_scaled(
                    &mut pixels,
                    width,
                    column * cell,
                    row as u32 * cell,
                    &rendered.rgba_bytes(),
                    SCALE,
                );
                column += 1;
            }
            column += 1;
        }
    }
    write_png(&path, width, height, &pixels)?;
    println!("wrote {}", path.display());
    Ok(())
}

/// Ten seconds of every body at rest, a frame every third of a second — the rate a resting colony
/// is drawn at — with its face as the overlay resolves it. Each body has two rows: the first as its
/// action alone draws it, the six-beat rest going round and a blink on the beat; the second as it
/// carries itself on the desktop, coming to rest with a settle, holding still, shifting its weight
/// or looking off when it does, glancing about and blinking at its own pace. The two rows are what
/// a rest looked like and what it looks like now, side by side.
pub fn rest_sheet(path: PathBuf) -> Result<()> {
    const SCALE: u32 = 2;
    const COLUMNS: u32 = 30;
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
        // A companion of its own, so each rests and looks about in its own way.
        creature.id = preview.id.wrapping_add(index as u64 + 1);
        creature.behavior_seed[0] = creature.behavior_seed[0].wrapping_add(index as u8 + 1);
        subjects.push(creature);
    }
    let cell = FRAME_SIZE * SCALE;
    let width = COLUMNS * cell;
    let height = subjects.len() as u32 * (2 * cell + GAP);
    let mut pixels = vec![0_u8; (width * height * 4) as usize];
    for chunk in pixels.chunks_exact_mut(4) {
        chunk.copy_from_slice(&[236, 234, 228, 255]);
    }
    for (index, subject) in subjects.iter().enumerate() {
        let mut creature = subject.clone();
        creature.state.action = ActionKind::Idle;
        creature.state.attention = None;
        creature.state.beat = None;
        creature.state.flourish = None;
        creature.state.facing_right = true;
        creature.state.drives = Drives::default();
        for (row, measured) in [false, true].into_iter().enumerate() {
            for column in 0..COLUMNS {
                let at = column as f32 / 3.0;
                creature.state.action_elapsed = at;
                creature.state.presence = if measured {
                    Presence {
                        clock: 40.0 + at,
                        since: at,
                        last: Some(PresenceMark {
                            x: creature.state.position.x,
                            window: None,
                        }),
                        ..Presence::default()
                    }
                } else {
                    Presence::default()
                };
                let body = formiga_art::BodyPresentation::for_creature(&creature);
                let face = CreatureRenderer::resolve_face_state_among(
                    &creature,
                    &[],
                    CursorSnapshot::default(),
                    false,
                    false,
                );
                let rendered = CreatureRenderer::render_composited_frame(
                    &creature.appearance,
                    body.clip,
                    body.frame,
                    body.facing_right,
                    false,
                    face,
                );
                blit_scaled(
                    &mut pixels,
                    width,
                    column * cell,
                    index as u32 * (2 * cell + GAP) + row as u32 * cell,
                    &rendered.rgba_bytes(),
                    SCALE,
                );
            }
        }
    }
    write_png(&path, width, height, &pixels)?;
    println!("wrote {}", path.display());
    Ok(())
}

/// The two poses a body pose is most easily mistaken for: standing about, and the plain peer a
/// gesture usually stands in for. Every body plan is shown holding both, right beside its
/// gestures, so "does this look like anything in particular?" can be answered by looking.
const REFERENCE_ACTIONS: [ActionKind; 2] = [ActionKind::Idle, ActionKind::InspectScreen];

/// Three bands, all at 3x so a pose is judged near the size a desktop shows it at.
///
/// First every action then every gesture, for each reference creature; then, for each modular body
/// plan, standing about and plain peering followed by every gesture, so a new pose can be
/// compared against both the nine it has to stay distinct from and the two it replaces; then one
/// band per body of the whole watching loop, frame by frame, because a held pose is only honest
/// if it keeps moving while it holds.
pub fn gesture_sheet(path: PathBuf) -> Result<()> {
    const COLS: u32 = 7;
    const SCALE: u32 = 3;
    let subjects = gesture_subjects();
    let references = reference_creatures().len();
    let cell = FRAME_SIZE * SCALE;
    let watch_frames = formiga_art::AnimationSpec::for_clip(Gesture::Watch).frames;
    let full_rows = ((ActionKind::ALL.len() + Gesture::ALL.len()) as u32).div_ceil(COLS);
    let gesture_rows = ((REFERENCE_ACTIONS.len() + Gesture::ALL.len()) as u32).div_ceil(COLS);
    let width = COLS * cell;
    let bands = references as u32 * full_rows
        + (subjects.len() - references) as u32 * gesture_rows
        + subjects.len() as u32 * u32::from(watch_frames).div_ceil(COLS);
    let height = bands * cell;
    let mut pixels = vec![0_u8; (width * height * 4) as usize];
    let mut row_cursor = 0_u32;
    let place = |pixels: &mut Vec<u8>, row_cursor: &mut u32, frames: Vec<formiga_art::Canvas>| {
        for (index, rendered) in frames.iter().enumerate() {
            blit_scaled(
                pixels,
                width,
                index as u32 % COLS * cell,
                (*row_cursor + index as u32 / COLS) * cell,
                &rendered.rgba_bytes(),
                SCALE,
            );
        }
        *row_cursor += (frames.len() as u32).div_ceil(COLS);
    };
    for (index, genome) in subjects.iter().enumerate() {
        let mut frames = Vec::new();
        let actions: Vec<_> = if index < references {
            ActionKind::ALL.to_vec()
        } else {
            REFERENCE_ACTIONS.to_vec()
        };
        frames.extend(actions.into_iter().enumerate().map(|(step, action)| {
            let spec = formiga_art::AnimationSpec::for_action(action);
            CreatureRenderer::render_frame(genome, action, (step as u8 + 1) % spec.frames, true)
        }));
        frames.extend(Gesture::ALL.into_iter().map(|gesture| {
            let spec = formiga_art::AnimationSpec::for_clip(gesture);
            CreatureRenderer::render_composited_frame(
                genome,
                gesture,
                1 % spec.frames,
                true,
                false,
                gesture_face(gesture),
            )
        }));
        place(&mut pixels, &mut row_cursor, frames);
    }
    for genome in &subjects {
        let mut loop_frames: Vec<_> = (0..watch_frames)
            .map(|frame| {
                CreatureRenderer::render_composited_frame(
                    genome,
                    Gesture::Watch,
                    frame,
                    true,
                    false,
                    gesture_face(Gesture::Watch),
                )
            })
            .collect();
        // The seventh column of each loop is the same pose on a mini, which is the smallest body
        // the colony grows and therefore the one a held pose has to survive being drawn at.
        let mut mini = genome.clone();
        mini.logical_size = 21;
        loop_frames.push(CreatureRenderer::render_composited_frame(
            &mini,
            Gesture::Watch,
            1,
            true,
            false,
            gesture_face(Gesture::Watch),
        ));
        place(&mut pixels, &mut row_cursor, loop_frames);
    }
    write_png(&path, width, height, &pixels)?;
    println!("wrote {}", path.display());
    Ok(())
}

pub fn activity_sheet(path: PathBuf) -> Result<()> {
    const COLS: u32 = 8;
    const SCALE: u32 = 3;
    const PREVIEW_FRAMES: u8 = 4;
    const ACTIONS: [ActionKind; 4] = [
        ActionKind::SoloPlay,
        ActionKind::Eat,
        ActionKind::Drink,
        ActionKind::Sprint,
    ];
    let creatures = reference_creatures();
    let cell = FRAME_SIZE * SCALE;
    let cells_per_family = ACTIONS.len() as u32 * u32::from(PREVIEW_FRAMES);
    let rows_per_family = cells_per_family.div_ceil(COLS);
    let width = COLS * cell;
    let height = creatures.len() as u32 * rows_per_family * cell;
    let mut pixels = vec![0_u8; (width * height * 4) as usize];
    for (family_index, creature) in creatures.iter().enumerate() {
        for (action_index, action) in ACTIONS.into_iter().enumerate() {
            let spec = formiga_art::AnimationSpec::for_action(action);
            for frame in 0..PREVIEW_FRAMES {
                let cell_index = action_index as u32 * u32::from(PREVIEW_FRAMES) + u32::from(frame);
                let rendered = CreatureRenderer::render_frame(
                    &creature.appearance,
                    action,
                    frame % spec.frames,
                    true,
                );
                let row = family_index as u32 * rows_per_family + cell_index / COLS;
                blit_scaled(
                    &mut pixels,
                    width,
                    cell_index % COLS * cell,
                    row * cell,
                    &rendered.rgba_bytes(),
                    SCALE,
                );
            }
        }
    }
    write_png(&path, width, height, &pixels)?;
    println!("wrote {}", path.display());
    Ok(())
}

pub fn ambient_sheet(path: PathBuf) -> Result<()> {
    const COLS: u32 = 8;
    const SCALE: u32 = 3;
    const CELL_ART_SIZE: u32 = 64;
    const ACTIONS: [ActionKind; 4] = [
        ActionKind::ClimbWindow,
        ActionKind::Dangle,
        ActionKind::InspectScreen,
        ActionKind::PresentDiscovery,
    ];
    let creatures = reference_creatures();
    let cell = CELL_ART_SIZE * SCALE;
    let rows_per_family = 3;
    let width = COLS * cell;
    let height = creatures.len() as u32 * rows_per_family * cell;
    let mut pixels = vec![0_u8; (width * height * 4) as usize];
    fill_gradient(
        &mut pixels,
        width,
        height,
        [18, 27, 33, 255],
        [29, 43, 47, 255],
    );

    for (family_index, creature) in creatures.iter().enumerate() {
        let family_row = family_index as u32 * rows_per_family;
        let baseline = CreatureRenderer::resting_baseline(&creature.appearance, false);
        for (action_index, action) in ACTIONS.into_iter().enumerate() {
            for frame in 0..4_u8 {
                let cell_index = action_index as u32 * 4 + u32::from(frame);
                let column = cell_index % COLS;
                let row = family_row + cell_index / COLS;
                let cell_x = column * cell;
                let cell_y = row * cell;
                let contact_y = if action == ActionKind::Dangle { 8 } else { 56 };
                draw_rect_alpha(
                    &mut pixels,
                    width,
                    cell_x + 4 * SCALE,
                    cell_y + contact_y * SCALE,
                    (CELL_ART_SIZE - 8) * SCALE,
                    SCALE,
                    [118, 155, 157, 210],
                );
                let placement = formiga_art::FramePlacement::for_action(action, baseline);
                let rendered =
                    CreatureRenderer::render_frame(&creature.appearance, action, frame, true);
                blit_scaled_square_alpha(
                    &mut pixels,
                    width,
                    cell_x + 8 * SCALE,
                    cell_y + (contact_y as i32 + placement.origin_y) as u32 * SCALE,
                    &rendered.rgba_bytes(),
                    FRAME_SIZE,
                    SCALE,
                );
            }
        }

        let trinket_row = family_row + 2;
        for variant in 0..8_u8 {
            let cell_x = u32::from(variant) * cell;
            let cell_y = trinket_row * cell;
            let trinket = CreatureRenderer::render_trinket(&creature.appearance, variant);
            blit_scaled_square_alpha(
                &mut pixels,
                width,
                cell_x + 16 * SCALE,
                cell_y + 16 * SCALE,
                &trinket.rgba_bytes(),
                16,
                6,
            );
        }
    }

    write_png(&path, width, height, &pixels)?;
    println!("wrote {}", path.display());
    Ok(())
}
