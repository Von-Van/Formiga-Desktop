//! Review sheets of how a body moves: the everyday loops, every gesture with the face it is
//! judged with, the activity clips, and the climbing, hanging and finding clips.

use crate::{
    blit_scaled, blit_scaled_square_alpha, draw_rect_alpha, fill_gradient, reference_creatures,
    write_png,
};
use anyhow::Result;
use formiga_art::{
    CreatureRenderer, ExpressionKind, EyelidPose, FRAME_SIZE, FaceRenderState, GazeDirection,
    PixelPoint,
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
        Gesture::Crouch | Gesture::Heave | Gesture::Mantle | Gesture::Leap => {
            (ExpressionKind::Determined, EyelidPose::Open)
        }
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
        // Baked with the face they are drawn with (see `BodyClip::face_in_frame`).
        Gesture::Stride | Gesture::Dash | Gesture::Depart => {
            (ExpressionKind::Neutral, EyelidPose::Open)
        }
        Gesture::Pour | Gesture::Tend => (ExpressionKind::Focused, EyelidPose::Open),
        Gesture::Pluck => (ExpressionKind::Determined, EyelidPose::Open),
        // Eyes shut on a smile under the hand.
        Gesture::Bask => (ExpressionKind::Affectionate, EyelidPose::Closed),
        Gesture::Nibble | Gesture::Chomp => (ExpressionKind::Content, EyelidPose::Open),
        Gesture::Chew => (ExpressionKind::Content, EyelidPose::Half),
        Gesture::Gulp => (ExpressionKind::Joy, EyelidPose::Open),
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
/// frame for every body, one row each, at 3x, then being carried, a toss tumbling through the air,
/// a hop: the crouch it springs from, the leap rising and coming down, and the landing, which
/// is played once from the touch to standing, and the shuffle of a squeeze through a narrow gap,
/// pinched with its face. A loop that reads as the same picture twice in a row, or that hitches
/// where it comes round, shows up here and nowhere else.
pub fn motion_sheet(path: PathBuf) -> Result<()> {
    use formiga_art::BodyClip;
    const SCALE: u32 = 3;
    const LOOPS: [BodyClip; 11] = [
        BodyClip::Action(ActionKind::Traverse),
        BodyClip::Action(ActionKind::Sprint),
        BodyClip::Action(ActionKind::Idle),
        BodyClip::Action(ActionKind::Eat),
        BodyClip::Action(ActionKind::Sleep),
        BodyClip::Action(ActionKind::Dragged),
        BodyClip::Action(ActionKind::Tossed),
        BodyClip::Gesture(Gesture::Crouch),
        BodyClip::Gesture(Gesture::Leap),
        BodyClip::Action(ActionKind::Landing),
        BodyClip::Action(ActionKind::SqueezeWindow),
    ];
    let subjects = gesture_subjects();
    let cell = FRAME_SIZE * SCALE;
    let columns: u32 = LOOPS
        .iter()
        .map(|clip| u32::from(formiga_art::AnimationSpec::for_clip(*clip).frames))
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
        for clip in LOOPS {
            let spec = formiga_art::AnimationSpec::for_clip(clip);
            for frame in 0..spec.frames {
                let rendered = match clip {
                    BodyClip::Action(action) => {
                        CreatureRenderer::render_frame(genome, action, frame, true)
                    }
                    // A hop's crouch and leap wear the face the hop does.
                    BodyClip::Gesture(gesture) => CreatureRenderer::render_composited_frame(
                        genome,
                        clip,
                        frame,
                        true,
                        false,
                        gesture_face(gesture),
                    ),
                };
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
/// three ways it walks — plainly, skipping and trudging — and the two ways it sits, on its
/// doorstep and up on a window, breathing. One row per body at 3x, a gap between
/// each clip, so the moods can be told apart at a glance and a pose that reads as something else
/// shows up beside the ones it might be mistaken for.
pub fn repertoire_sheet(path: PathBuf) -> Result<()> {
    const SCALE: u32 = 3;
    let clips: [(formiga_art::BodyClip, FaceRenderState); 14] = {
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
            (G(Gesture::Stride), gesture_face(Gesture::Stride)),
            (G(Gesture::Dash), gesture_face(Gesture::Dash)),
            (G(Gesture::Depart), gesture_face(Gesture::Depart)),
            (
                Action(ActionKind::Homebound),
                face(ExpressionKind::Content, EyelidPose::Open),
            ),
            (
                Action(ActionKind::Perch),
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
/// keeping a toy up, playing with a friend, saying hello, and being petted. The watering can is
/// drawn in its paws as the overlay draws it, tipped and pouring on the frames the pour leans in;
/// what it picks is left to the hold sheet.
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
    let objects = formiga_art::ColonyObjectRenderer::render_atlas([61_u8; 32]);
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
                let mut rendered = CreatureRenderer::render_composited_frame(
                    genome, clip, frame, true, false, face,
                );
                if clip == formiga_art::BodyClip::Gesture(Gesture::Pour) {
                    for (sprite, at) in
                        village_held_cells(genome, VillageProp::WateringCan, clip, frame)
                    {
                        let tile = atlas_cell(
                            &objects,
                            formiga_art::ColonyObjectRenderer::cell_origin(
                                formiga_art::ColonyObjectRenderer::prop_cell(sprite),
                            ),
                            formiga_art::COLONY_OBJECT_SIZE,
                        );
                        let half = formiga_art::COLONY_OBJECT_SIZE as i32 / 2;
                        for y in 0..tile.height() as i32 {
                            for x in 0..tile.width() as i32 {
                                let pixel = tile.get(x, y);
                                if pixel.a > 0 {
                                    rendered.set(at.x - half + x, at.y - half + y, pixel);
                                }
                            }
                        }
                    }
                }
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
/// a rest looked like and what it looks like now, side by side. Under them, one row a body of a
/// long nap turning over, a frame every tenth of a second from just before the turn: the stir,
/// the crouch it gathers itself up in, and the way it settles into. Last, a mini beside the adult
/// it would be with the same identity and temperament, row against row: resting a frame every
/// third of a second, and bopping and washing a frame every tenth, so its quicker, lighter pace
/// can be seen against the grown-up one beat for beat.
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
    let rests = subjects.len() as u32 * (2 * cell + GAP);
    // Under the rests, one row each of a long nap turning over, a tenth of a second a column.
    let naps = rests + subjects.len() as u32 * (cell + GAP);
    // Under the naps, the mini beside its adult: six rows, rest, bop and wash, adult then mini.
    let height = naps + 6 * cell + GAP;
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
                            depth: 0.0,
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
    for (index, subject) in subjects.iter().enumerate() {
        let mut creature = subject.clone();
        creature.state.attention = None;
        creature.state.beat = None;
        creature.state.flourish = None;
        creature.state.facing_right = true;
        creature.state.drives = Drives::default();
        creature.state.action = ActionKind::Sleep;
        creature.state.action_duration = 40.0;
        creature.state.nudge = None;
        let motion = formiga_art::MotionSignature::for_creature(&creature);
        // The first long nap of this companion's that turns over, and when.
        let Some((began, turn_at)) = (0..400).map(|nap| nap as f32 * 37.0).find_map(|began| {
            let first = motion.nap_pose(began, 0.0, 40.0, true);
            (0..400)
                .map(|tenth| tenth as f32 / 10.0)
                .find(|&into| motion.nap_pose(began, into, 40.0, true) != first)
                .map(|at| (began, at))
        }) else {
            continue;
        };
        for column in 0..COLUMNS {
            let into = turn_at - 0.6 + column as f32 / 10.0;
            creature.state.action_elapsed = into;
            creature.state.presence = Presence {
                clock: began + into,
                since: into,
                last: Some(PresenceMark {
                    x: creature.state.position.x,
                    depth: 0.0,
                    window: None,
                }),
                ..Presence::default()
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
                rests + index as u32 * (cell + GAP),
                &rendered.rgba_bytes(),
                SCALE,
            );
        }
    }
    // The mini: the first reference companion again, as a mini of itself at the share minis are
    // drawn at, so the only differences are its size and its pace.
    let adult = subjects[0].clone();
    let mut mini = adult.clone();
    mini.role = CreatureRole::Mini {
        parent_id: adult.id,
    };
    mini.display_scale_percent = 70;
    mini.appearance.logical_size = size_after_parent(&adult, mini.display_scale_percent);
    for (pair, creature) in [adult, mini].into_iter().enumerate() {
        let mut creature = creature;
        creature.state.attention = None;
        creature.state.beat = None;
        creature.state.flourish = None;
        creature.state.facing_right = true;
        creature.state.drives = Drives::default();
        creature.state.action = ActionKind::Idle;
        let motion = formiga_art::MotionSignature::for_creature(&creature);
        for (band, clip) in [None, Some(Gesture::Bop), Some(Gesture::Groom)]
            .into_iter()
            .enumerate()
        {
            for column in 0..COLUMNS {
                let (clip, frame, face) = match clip {
                    None => {
                        let at = column as f32 / 3.0;
                        creature.state.action_elapsed = at;
                        creature.state.presence = Presence {
                            clock: 40.0 + at,
                            since: at,
                            last: Some(PresenceMark {
                                x: creature.state.position.x,
                                depth: 0.0,
                                window: None,
                            }),
                            ..Presence::default()
                        };
                        let body = formiga_art::BodyPresentation::for_creature(&creature);
                        let face = CreatureRenderer::resolve_face_state_among(
                            &creature,
                            &[],
                            CursorSnapshot::default(),
                            false,
                            false,
                        );
                        (body.clip, body.frame, face)
                    }
                    Some(gesture) => {
                        let clip = formiga_art::BodyClip::Gesture(gesture);
                        (
                            clip,
                            motion.frame(clip, column as f32 / 10.0),
                            gesture_face(gesture),
                        )
                    }
                };
                let rendered = CreatureRenderer::render_composited_frame(
                    &creature.appearance,
                    clip,
                    frame,
                    true,
                    false,
                    face,
                );
                blit_scaled(
                    &mut pixels,
                    width,
                    column * cell,
                    naps + (band as u32 * 2 + pair as u32) * cell,
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

/// The gestures the gesture sheet shows frame by frame, one row each for every body, after every
/// pose's own cell: the watching loop, because a held pose is only honest if it keeps moving
/// while it holds, and a climb's mantle, because it is three beats that only read in order.
const GESTURE_LOOPS: [Gesture; 2] = [Gesture::Watch, Gesture::Mantle];

/// How many rows of cells the gesture sheet has, `cols` cells to a row.
fn gesture_sheet_rows(cols: u32) -> u32 {
    let subjects = gesture_subjects().len() as u32;
    let references = reference_creatures().len() as u32;
    let full_rows = ((ActionKind::ALL.len() + Gesture::ALL.len()) as u32).div_ceil(cols);
    let gesture_rows = ((REFERENCE_ACTIONS.len() + Gesture::ALL.len()) as u32).div_ceil(cols);
    // Each loop is followed by the same pose on a mini.
    let loop_rows: u32 = GESTURE_LOOPS
        .iter()
        .map(|gesture| {
            (u32::from(formiga_art::AnimationSpec::for_clip(*gesture).frames) + 1).div_ceil(cols)
        })
        .sum();
    references * full_rows + (subjects - references) * gesture_rows + subjects * loop_rows
}

/// Three bands, all at 3x so a pose is judged near the size a desktop shows it at.
///
/// First every action then every gesture, for each reference creature; then, for each modular body
/// plan, standing about and plain peering followed by every gesture, so a new pose can be
/// compared against both the nine it has to stay distinct from and the two it replaces; then, for
/// each body, a row of each of [`GESTURE_LOOPS`] frame by frame.
pub fn gesture_sheet(path: PathBuf) -> Result<()> {
    const COLS: u32 = 7;
    const SCALE: u32 = 3;
    let subjects = gesture_subjects();
    let references = reference_creatures().len();
    let cell = FRAME_SIZE * SCALE;
    let width = COLS * cell;
    let height = gesture_sheet_rows(COLS) * cell;
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
    for gesture in GESTURE_LOOPS {
        let frames = formiga_art::AnimationSpec::for_clip(gesture).frames;
        for genome in &subjects {
            let mut loop_frames: Vec<_> = (0..frames)
                .map(|frame| {
                    CreatureRenderer::render_composited_frame(
                        genome,
                        gesture,
                        frame,
                        true,
                        false,
                        gesture_face(gesture),
                    )
                })
                .collect();
            // The last column of each loop is the same pose on a mini, which is the smallest body
            // the colony grows and therefore the one a held pose has to survive being drawn at.
            let mut mini = genome.clone();
            mini.logical_size = 21;
            loop_frames.push(CreatureRenderer::render_composited_frame(
                &mini,
                gesture,
                1,
                true,
                false,
                gesture_face(gesture),
            ));
            place(&mut pixels, &mut row_cursor, loop_frames);
        }
    }
    write_png(&path, width, height, &pixels)?;
    println!("wrote {}", path.display());
    Ok(())
}

/// Playing, eating, drinking and sprinting, four frames each, for every reference subject. With
/// `reduced` every frame is drawn as reduced motion draws it, so each clip shows the one frame it
/// holds still on, and a climb up a window is added, being the other clip that holds a beat
/// other than its first.
pub fn activity_sheet(path: PathBuf, reduced: bool) -> Result<()> {
    const COLS: u32 = 8;
    const SCALE: u32 = 3;
    const PREVIEW_FRAMES: u8 = 4;
    const ACTIONS: [ActionKind; 4] = [
        ActionKind::SoloPlay,
        ActionKind::Eat,
        ActionKind::Drink,
        ActionKind::Sprint,
    ];
    let actions: Vec<ActionKind> = if reduced {
        ACTIONS
            .into_iter()
            .chain([ActionKind::ClimbWindow])
            .collect()
    } else {
        ACTIONS.to_vec()
    };
    let creatures = reference_creatures();
    let cell = FRAME_SIZE * SCALE;
    let cells_per_family = actions.len() as u32 * u32::from(PREVIEW_FRAMES);
    let rows_per_family = cells_per_family.div_ceil(COLS);
    let width = COLS * cell;
    let height = creatures.len() as u32 * rows_per_family * cell;
    let mut pixels = vec![0_u8; (width * height * 4) as usize];
    for (family_index, creature) in creatures.iter().enumerate() {
        for (action_index, &action) in actions.iter().enumerate() {
            let spec = formiga_art::AnimationSpec::for_action(action);
            for frame in 0..PREVIEW_FRAMES {
                let cell_index = action_index as u32 * u32::from(PREVIEW_FRAMES) + u32::from(frame);
                let rendered = CreatureRenderer::render_frame_with_options(
                    &creature.appearance,
                    action,
                    frame % spec.frames,
                    true,
                    reduced,
                    0,
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

/// How far below the ledge a companion's contact point is in each frame of its mantle, in art
/// pixels: partway through each frame, as the world hauls it up (`formiga_core::world::journeys`)
/// — the chest over the edge low down, the push nearly up, and the stand on it.
const MANTLE_LEDGE_DROP: [u32; 3] = [9, 1, 0];

pub fn ambient_sheet(path: PathBuf) -> Result<()> {
    use formiga_art::BodyClip;
    const COLS: u32 = 8;
    const SCALE: u32 = 3;
    const CELL_ART_SIZE: u32 = 64;
    const CLIPS: [BodyClip; 5] = [
        BodyClip::Action(ActionKind::ClimbWindow),
        BodyClip::Gesture(Gesture::Mantle),
        BodyClip::Action(ActionKind::Dangle),
        BodyClip::Action(ActionKind::InspectScreen),
        BodyClip::Action(ActionKind::PresentDiscovery),
    ];
    let creatures = reference_creatures();
    let cell = CELL_ART_SIZE * SCALE;
    // Every frame of each clip, a clip starting a new row rather than wrapping onto one: the
    // climb's six on the first row, the mantle at its top and the dangle on the second, the look
    // and the find on the third, and the trinkets under them.
    let mut cells = Vec::new();
    let mut next = 0_u32;
    for clip in CLIPS {
        let frames = u32::from(formiga_art::AnimationSpec::for_clip(clip).frames);
        if next % COLS + frames > COLS {
            next = next.div_ceil(COLS) * COLS;
        }
        cells.push(next);
        next += frames;
    }
    let action_rows = next.div_ceil(COLS);
    let rows_per_family = action_rows + 1;
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
        for (clip_index, clip) in CLIPS.into_iter().enumerate() {
            // A gesture is placed standing, as the overlay places one.
            let action = match clip {
                BodyClip::Action(action) => action,
                BodyClip::Gesture(_) => ActionKind::ClimbWindow,
            };
            for frame in 0..formiga_art::AnimationSpec::for_clip(clip).frames {
                let cell_index = cells[clip_index] + u32::from(frame);
                let column = cell_index % COLS;
                let row = family_row + cell_index / COLS;
                let cell_x = column * cell;
                let cell_y = row * cell;
                let contact_y = if action == ActionKind::Dangle { 8 } else { 56 };
                // The ledge a mantle is topping out onto, where the world has it in each frame.
                let ledge_y = match clip {
                    BodyClip::Gesture(Gesture::Mantle) => {
                        contact_y - MANTLE_LEDGE_DROP[usize::from(frame.min(2))]
                    }
                    _ => contact_y,
                };
                draw_rect_alpha(
                    &mut pixels,
                    width,
                    cell_x + 4 * SCALE,
                    cell_y + ledge_y * SCALE,
                    (CELL_ART_SIZE - 8) * SCALE,
                    SCALE,
                    [118, 155, 157, 210],
                );
                let placement = formiga_art::FramePlacement::for_action(action, baseline);
                let rendered = match clip {
                    BodyClip::Action(action) => {
                        CreatureRenderer::render_frame(&creature.appearance, action, frame, true)
                    }
                    BodyClip::Gesture(gesture) => CreatureRenderer::render_composited_frame(
                        &creature.appearance,
                        gesture,
                        frame,
                        true,
                        false,
                        gesture_face(gesture),
                    ),
                };
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

        let trinket_row = family_row + action_rows;
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

/// The colours the hold sheet marks with: the paw a frame holds things in, and the mouth it eats
/// with. No palette has either, so a mark is never mistaken for a pixel of the art; the sheet's
/// test holds every subject to that.
pub(crate) const HANDS_MARK: [u8; 4] = [255, 0, 255, 255];
pub(crate) const MOUTH_MARK: [u8; 4] = [0, 0, 255, 255];

/// The clips that put something in a body's paws or mouth, with the face each is shown with:
/// eating, drinking, keeping a toy up, holding up a find, and the three village chores.
fn hold_clips() -> [(formiga_art::BodyClip, &'static str, FaceRenderState); 7] {
    use formiga_art::BodyClip::{Action, Gesture as G};
    let face = |expression, eyelids| FaceRenderState {
        expression,
        eyelids,
        gaze: GazeDirection::default(),
    };
    [
        (
            Action(ActionKind::Eat),
            "EAT",
            face(ExpressionKind::Content, EyelidPose::Open),
        ),
        (
            Action(ActionKind::Drink),
            "DRINK",
            face(ExpressionKind::Content, EyelidPose::Half),
        ),
        (
            Action(ActionKind::SoloPlay),
            "SOLO PLAY",
            face(ExpressionKind::Joy, EyelidPose::Open),
        ),
        (
            Action(ActionKind::PresentDiscovery),
            "PRESENT FIND",
            face(ExpressionKind::Joy, EyelidPose::Open),
        ),
        (G(Gesture::Pour), "POUR", gesture_face(Gesture::Pour)),
        (G(Gesture::Pluck), "PLUCK", gesture_face(Gesture::Pluck)),
        (G(Gesture::Tend), "TEND", gesture_face(Gesture::Tend)),
    ]
}

/// Every body the hold sheet shows, named, at the smallest and the largest a companion stands:
/// the sheets' usual subjects — a modular body from each original family, then one of each body
/// plan — and the three original families as a look from before recipes draws them.
pub(crate) fn hold_subjects() -> Vec<(String, AppearanceGenome)> {
    let references = reference_creatures().len();
    let mut named: Vec<(String, AppearanceGenome)> = gesture_subjects()
        .into_iter()
        .enumerate()
        .map(|(index, genome)| {
            let plan = genome.design.map(|design| design.body);
            let name = match plan {
                Some(plan) if index < references => {
                    format!("{:?} FAMILY {:?}", genome.family, plan)
                }
                Some(plan) => format!("PLAN {plan:?}"),
                None => format!("{:?}", genome.family),
            };
            (name.to_uppercase(), genome)
        })
        .collect();
    named.extend(reference_creatures().into_iter().map(|creature| {
        let mut genome = creature.appearance;
        genome.design = None;
        (
            format!("ORIGINAL {:?}", genome.family).to_uppercase(),
            genome,
        )
    }));
    named
        .into_iter()
        .flat_map(|(name, genome)| {
            [STATURE_MIN, STATURE_MAX].map(|stature| {
                let mut sized = genome.clone();
                sized.logical_size = size_for(stature, 100);
                (format!("{name} {stature}"), sized)
            })
        })
        .collect()
}

/// A sheet pixel of a frame's mark, kept inside the frame drawn `scale` times over from
/// `(left, top)`.
fn mark_dot(
    pixels: &mut [u8],
    width: u32,
    (left, top): (u32, u32),
    scale: u32,
    (x, y): (i32, i32),
    color: [u8; 4],
) {
    let side = (FRAME_SIZE * scale) as i32;
    let (dx, dy) = (x - left as i32, y - top as i32);
    if (0..side).contains(&dx) && (0..side).contains(&dy) {
        crate::blend_pixel(pixels, width, x as u32, y as u32, color);
    }
}

/// A crosshair one art pixel wide at most on art pixel `at` of a frame drawn `scale` times over
/// from `origin`: a box round that pixel, so the pixel itself shows through, and arms out from it,
/// two sheet pixels thick so it is seen at a glance.
fn crosshair(
    pixels: &mut [u8],
    width: u32,
    origin: (u32, u32),
    at: PixelPoint,
    scale: u32,
    color: [u8; 4],
) {
    const ARM: i32 = 12;
    const THICK: i32 = 2;
    let scale = scale as i32;
    // The box's inside is the art pixel itself.
    let (x0, y0) = (
        origin.0 as i32 + at.x * scale - THICK,
        origin.1 as i32 + at.y * scale - THICK,
    );
    let outer = scale + 2 * THICK;
    let mut dot = |x: i32, y: i32| mark_dot(pixels, width, origin, scale as u32, (x, y), color);
    for along in 0..outer {
        for across in 0..THICK {
            dot(x0 + along, y0 + across);
            dot(x0 + along, y0 + outer - 1 - across);
            dot(x0 + across, y0 + along);
            dot(x0 + outer - 1 - across, y0 + along);
        }
    }
    let middle = outer / 2 - THICK / 2;
    for step in 1..=ARM {
        for across in 0..THICK {
            dot(x0 - step, y0 + middle + across);
            dot(x0 + outer - 1 + step, y0 + middle + across);
            dot(x0 + middle + across, y0 - step);
            dot(x0 + middle + across, y0 + outer - 1 + step);
        }
    }
}

/// What the village has put in a companion's paws in `frame` of `clip`, as the overlay draws it
/// with the frame facing right: each cell of the colony's object sheet and where in the frame its
/// middle goes — the can (tipped while it pours) or what was picked on the paw, and the water from
/// the spout for as long as the can is tipped.
pub(crate) fn village_held_cells(
    genome: &AppearanceGenome,
    prop: VillageProp,
    clip: formiga_art::BodyClip,
    frame: u8,
) -> Vec<(formiga_art::PropSprite, PixelPoint)> {
    let body = CreatureRenderer::render_body_frame(genome, clip, frame, false);
    let hands = formiga_art::held_up_at(body.hands, body.face_anchor, formiga_art::HELD_UP_REACH);
    let (held, water) = formiga_art::village_held(prop, clip, frame);
    std::iter::once((held, hands))
        .chain(water.map(|(x, y)| {
            (
                formiga_art::PropSprite::Water,
                PixelPoint {
                    x: hands.x + x,
                    y: hands.y + y,
                },
            )
        }))
        .collect()
}

/// A cell `size` pixels square cut from an atlas at `origin`.
fn atlas_cell(atlas: &formiga_art::Canvas, (x0, y0): (u32, u32), size: u32) -> formiga_art::Canvas {
    let mut cell = formiga_art::Canvas::new(size, size);
    for y in 0..size {
        for x in 0..size {
            cell.set(
                x as i32,
                y as i32,
                atlas.get((x0 + x) as i32, (y0 + y) as i32),
            );
        }
    }
    cell
}

/// Where every belonging is held, frame by frame, at 4x. One row per body at the smallest and
/// largest a companion stands, every frame of eating, drinking, play, holding up a find, and the
/// three chores, each with the face it is drawn with and what it holds as the overlay draws it:
/// the snack, cup or toy the frame holds, a find held up, and the watering can or what was picked.
/// Over each frame, a magenta crosshair on the frame's `hands`, the paw it holds things in (or,
/// with no paw free, under the mouth it carries them in), where the overlay centres a find or a
/// village prop too; and a blue one on its `mouth`. A crosshair off the limb or the mouth is a
/// belonging drawn in the wrong place.
pub fn hold_sheet(path: PathBuf) -> Result<()> {
    use formiga_art::{
        AnimationSpec, COLONY_OBJECT_SIZE, ColonyObjectRenderer, PropSprite, TRINKET_CELL,
        TRINKET_FRAME_GLINT, TrinketAtlasRenderer,
    };
    const SCALE: u32 = 4;
    const LABEL: u32 = 150;
    const HEADER: u32 = 12;
    const GAP: u32 = 12;
    const LEGEND: u32 = 24;
    const INK: [u8; 4] = [60, 64, 62, 255];
    let subjects = hold_subjects();
    let clips = hold_clips();
    let cell = FRAME_SIZE * SCALE;
    let frames: u32 = clips
        .iter()
        .map(|(clip, _, _)| u32::from(AnimationSpec::for_clip(*clip).frames))
        .sum();
    let width = LABEL + frames * cell + (clips.len() as u32 - 1) * GAP;
    let row_height = HEADER + cell;
    let height = LEGEND + subjects.len() as u32 * row_height;
    let mut pixels = vec![0_u8; (width * height * 4) as usize];
    for chunk in pixels.chunks_exact_mut(4) {
        chunk.copy_from_slice(&[236, 234, 228, 255]);
    }
    let text = |pixels: &mut Vec<u8>, x: u32, y: u32, words: &str, color: [u8; 4]| {
        crate::social_preview::draw_text(
            pixels,
            width as i32,
            height as i32,
            x as i32,
            y as i32,
            words,
            1,
            color,
        );
    };
    // The key.
    let mut x = 8;
    for (color, words) in [
        (
            HANDS_MARK,
            "HANDS, WHERE A TOY, A FIND OR A VILLAGE PROP IS HELD",
        ),
        (MOUTH_MARK, "MOUTH"),
    ] {
        crate::fill_rect(&mut pixels, width, x, 8, 8, 8, color);
        text(&mut pixels, x + 12, 8, words, INK);
        x += 12 + words.len() as u32 * 6 + 24;
    }
    let creatures = reference_creatures();
    let members: Vec<formiga_art::Palette> = creatures
        .iter()
        .map(|creature| formiga_art::palette_for(&creature.appearance))
        .collect();
    let colony_seed = [61_u8; 32];
    let finds = TrinketAtlasRenderer::render(colony_seed, &members);
    let objects = ColonyObjectRenderer::render_atlas(colony_seed);
    let find = {
        let (x0, y0, size, _) = TrinketAtlasRenderer::cell_rect(3, TRINKET_FRAME_GLINT);
        atlas_cell(&finds, (x0, y0), size.min(TRINKET_CELL))
    };
    let prop = |sprite: PropSprite| {
        atlas_cell(
            &objects,
            ColonyObjectRenderer::cell_origin(ColonyObjectRenderer::prop_cell(sprite)),
            COLONY_OBJECT_SIZE,
        )
    };
    for (row, (name, genome)) in subjects.iter().enumerate() {
        let top = LEGEND + row as u32 * row_height;
        if row % 2 == 1 {
            crate::fill_rect(
                &mut pixels,
                width,
                0,
                top,
                width,
                row_height,
                [226, 224, 216, 255],
            );
        }
        let mut words = name.split(' ').collect::<Vec<_>>();
        let size = words.pop().unwrap_or_default();
        text(&mut pixels, 6, top + HEADER + 4, &words.join(" "), INK);
        text(
            &mut pixels,
            6,
            top + HEADER + 14,
            &format!("SIZE {size}"),
            INK,
        );
        let mut left = LABEL;
        for (clip, label, face) in clips {
            text(&mut pixels, left + 2, top + 2, label, INK);
            for frame in 0..AnimationSpec::for_clip(clip).frames {
                let rendered = CreatureRenderer::render_composited_frame(
                    genome, clip, frame, true, false, face,
                );
                let mut picture = rendered;
                // What the overlay draws over the body in its own quad: the find held up, the
                // watering can and its water, what was picked.
                let body = CreatureRenderer::render_body_frame(genome, clip, frame, false);
                let held_up = formiga_art::held_up_at(
                    body.hands,
                    body.face_anchor,
                    formiga_art::HELD_UP_REACH,
                );
                let mut overlay = |thing: &formiga_art::Canvas, PixelPoint { x: cx, y: cy }| {
                    let half = thing.width() as i32 / 2;
                    for y in 0..thing.height() as i32 {
                        for x in 0..thing.width() as i32 {
                            let pixel = thing.get(x, y);
                            if pixel.a > 0 {
                                picture.set(cx - half + x, cy - half + y, pixel);
                            }
                        }
                    }
                };
                let village = match clip {
                    formiga_art::BodyClip::Action(ActionKind::PresentDiscovery) => {
                        overlay(&find, held_up);
                        None
                    }
                    formiga_art::BodyClip::Gesture(Gesture::Pour) => Some(VillageProp::WateringCan),
                    formiga_art::BodyClip::Gesture(Gesture::Pluck) => {
                        Some(VillageProp::Produce(GardenKind::Vegetables))
                    }
                    _ => None,
                };
                for (sprite, at) in village
                    .map(|held| village_held_cells(genome, held, clip, frame))
                    .unwrap_or_default()
                {
                    overlay(&prop(sprite), at);
                }
                let origin = (left, top + HEADER);
                crate::blit_canvas_scaled(&mut pixels, width, origin.0, origin.1, &picture, SCALE);
                crosshair(&mut pixels, width, origin, body.mouth, SCALE, MOUTH_MARK);
                crosshair(&mut pixels, width, origin, body.hands, SCALE, HANDS_MARK);
                left += cell;
            }
            left += GAP;
        }
    }
    write_png(&path, width, height, &pixels)?;
    println!("wrote {}", path.display());
    Ok(())
}

/// Every food, eaten through a whole meal: a row for each, twelve moments from the first look to
/// the finish, on a different body each row, with its face and what is in its paws as the
/// overlay draws them. The meals are laid out from a fixed seed, so the sheet is the same every
/// time.
pub fn meal_sheet(path: PathBuf) -> Result<()> {
    use formiga_core::meals::{Appetite, Food, Meal};
    use rand::SeedableRng;
    const SCALE: u32 = 2;
    const MOMENTS: u32 = 12;
    const LENGTH: f32 = 9.0;
    let subjects = gesture_subjects();
    let cell = FRAME_SIZE * SCALE;
    let width = MOMENTS * cell;
    let height = Food::ALL.len() as u32 * cell;
    let mut pixels = vec![0_u8; (width * height * 4) as usize];
    for chunk in pixels.chunks_exact_mut(4) {
        chunk.copy_from_slice(&[236, 234, 228, 255]);
    }
    let mut creature = World::preview_adult(
        [29; 32],
        OffsetDateTime::UNIX_EPOCH,
        &DesktopSnapshot::default(),
    );
    creature.state.facing_right = true;
    creature.state.action = ActionKind::Eat;
    creature.state.action_duration = LENGTH;
    for (row, food) in Food::ALL.into_iter().enumerate() {
        creature.appearance = subjects[row % subjects.len()].clone();
        let mut rng = rand_chacha::ChaCha12Rng::seed_from_u64(row as u64);
        let mut meal = Meal::plan(food, LENGTH, Appetite::default(), &mut rng);
        for moment in 0..MOMENTS {
            meal.elapsed = (moment as f32 + 0.5) * LENGTH / MOMENTS as f32;
            creature.state.action_elapsed = meal.elapsed;
            creature.state.meal = Some(meal);
            let body = formiga_art::BodyPresentation::for_creature(&creature);
            let face =
                CreatureRenderer::resolve_face_state(&creature, CursorSnapshot::default(), false);
            let baked = CreatureRenderer::render_body_frame(
                &creature.appearance,
                body.clip,
                body.frame,
                false,
            )
            .held;
            // Painted where the meal puts it, as the overlay draws it: lifted, lowered or set
            // down by the step, not only where the frame holds things.
            let mut rendered = CreatureRenderer::render_composited_frame_holding(
                &creature.appearance,
                None,
                body.clip,
                body.frame,
                true,
                false,
                face,
                formiga_art::Holding::Nothing,
            );
            if let Some(held) = formiga_art::held_now(&creature, body.clip, baked) {
                held.paint(&mut rendered, true);
            }
            blit_scaled(
                &mut pixels,
                width,
                moment * cell,
                row as u32 * cell,
                &rendered.rgba_bytes(),
                SCALE,
            );
        }
    }
    write_png(&path, width, height, &pixels)?;
    println!("wrote {}", path.display());
    Ok(())
}

/// Every shape of plaything played with through a session: a row each, twelve moments from
/// picking it up to putting it down, and a last row riding an umbrella down off a ledge. Each
/// moment has room round the body, so a ball thrown high or a plane gliding away stays in view.
pub fn play_sheet(path: PathBuf) -> Result<()> {
    use formiga_core::play::{Play, Plaything};
    use rand::SeedableRng;
    const SCALE: u32 = 2;
    const MOMENTS: u32 = 12;
    const LENGTH: f32 = 8.0;
    // The body's frame sits this far in from each moment's left and top.
    const PAD: i32 = 24;
    let find = |name: &str| {
        formiga_core::all_trinkets()
            .find(|info| info.name == name)
            .map(|info| info.variant)
            .expect("a find of that name")
    };
    let things = [
        Plaything::Toy(0),
        Plaything::Toy(1),
        Plaything::Toy(2),
        Plaything::Toy(3),
        Plaything::Toy(5),
        Plaything::Belonging(ColonyObjectKind::Umbrella),
        Plaything::Belonging(ColonyObjectKind::Kite),
        Plaything::Belonging(ColonyObjectKind::Paper),
        Plaything::Belonging(ColonyObjectKind::Drum),
        Plaything::Trinket(find("Twig")),
        Plaything::Trinket(find("Walking stick")),
        Plaything::Trinket(find("Paper boat")),
        Plaything::Trinket(find("Marble")),
    ];
    let subjects = gesture_subjects();
    let (cw, ch) = (FRAME_SIZE as i32 + PAD * 3, FRAME_SIZE as i32 + PAD);
    let width = MOMENTS * cw as u32 * SCALE;
    let rows = things.len() as u32 + 1;
    let height = rows * ch as u32 * SCALE;
    let mut pixels = vec![0_u8; (width * height * 4) as usize];
    for chunk in pixels.chunks_exact_mut(4) {
        chunk.copy_from_slice(&[236, 234, 228, 255]);
    }
    let mut creature = World::preview_adult(
        [29; 32],
        OffsetDateTime::UNIX_EPOCH,
        &DesktopSnapshot::default(),
    );
    creature.state.facing_right = true;
    for row in 0..rows as usize {
        creature.appearance = subjects[row % subjects.len()].clone();
        let mut rng = rand_chacha::ChaCha12Rng::seed_from_u64(row as u64 + 3);
        let (mut play, action) = match things.get(row) {
            Some(thing) => (Play::plan(*thing, LENGTH, &mut rng), ActionKind::SoloPlay),
            None => (Play::umbrella_ride(LENGTH), ActionKind::Landing),
        };
        creature.state.action = action;
        creature.state.action_duration = LENGTH;
        for moment in 0..MOMENTS {
            play.elapsed = (moment as f32 + 0.5) * LENGTH / MOMENTS as f32;
            creature.state.action_elapsed = play.elapsed;
            creature.state.play = Some(play);
            let body = formiga_art::BodyPresentation::for_creature(&creature);
            let face =
                CreatureRenderer::resolve_face_state(&creature, CursorSnapshot::default(), false);
            let frame = CreatureRenderer::render_composited_frame_holding(
                &creature.appearance,
                None,
                body.clip,
                body.frame,
                true,
                false,
                face,
                formiga_art::Holding::Nothing,
            );
            let mut cell = formiga_art::Canvas::new(cw as u32, ch as u32);
            for y in 0..FRAME_SIZE as i32 {
                for x in 0..FRAME_SIZE as i32 {
                    let pixel = frame.get(x, y);
                    if pixel.a > 0 {
                        cell.set(x + PAD, y + PAD, pixel);
                    }
                }
            }
            let baked = CreatureRenderer::render_body_frame(
                &creature.appearance,
                body.clip,
                body.frame,
                false,
            )
            .held;
            let mut thing = formiga_art::Canvas::new(cw as u32, ch as u32);
            // A kite's string, behind the kite.
            if let Some((paw, kite)) = formiga_art::held_tether(&creature, body.clip, baked) {
                thing.line(
                    paw.x + PAD,
                    paw.y + PAD,
                    kite.x + PAD,
                    kite.y + PAD,
                    1,
                    formiga_art::KITE_STRING,
                );
            }
            if let Some(held) = formiga_art::held_now(&creature, body.clip, baked) {
                let offset = held.thing.offset();
                formiga_art::HeldPlacement::centred(
                    held.thing,
                    held.at.x + offset.x + PAD,
                    held.at.y + offset.y + PAD,
                )
                .paint(&mut thing, true);
            }
            for y in 0..ch {
                for x in 0..cw {
                    let pixel = thing.get(x, y);
                    if pixel.a > 0 {
                        cell.set(x, y, pixel);
                    }
                }
            }
            let (left, top) = (moment * cw as u32 * SCALE, row as u32 * ch as u32 * SCALE);
            for y in 0..ch as u32 * SCALE {
                for x in 0..cw as u32 * SCALE {
                    let pixel = cell.get((x / SCALE) as i32, (y / SCALE) as i32);
                    if pixel.a > 0 {
                        let at = (((top + y) * width + left + x) * 4) as usize;
                        pixels[at..at + 4].copy_from_slice(&[pixel.r, pixel.g, pixel.b, 255]);
                    }
                }
            }
        }
    }
    write_png(&path, width, height, &pixels)?;
    println!("wrote {}", path.display());
    Ok(())
}

/// Every bubble a companion says things in, over a companion, at the size the desktop shows them
/// at 2x: rows of styles with the topics they are said about, then two companions talking — an
/// invitation and its answer, a meal offered and thanks for it, a goodnight whispered back — and
/// last, every picture there is.
pub fn talk_sheet(path: PathBuf) -> Result<()> {
    use formiga_art::{TALK_DETAIL, TALK_ICON_AT, TalkSheet};
    use formiga_core::talk::{Saying, TalkStyle as S, TalkTopic as T};
    const SCALE: u32 = 2;
    let d = TALK_DETAIL as i32;
    let mut rows: Vec<Vec<(Saying, Option<Saying>)>> = vec![
        vec![
            (Saying::one(S::Chat, T::Wave), None),
            (Saying::one(S::Chat, T::Food), None),
            (Saying::one(S::Chat, T::Home), None),
            (Saying::one(S::Chat, T::No), None),
        ],
        vec![
            (Saying::one(S::Excited, T::Ball), None),
            (Saying::one(S::Excited, T::Sparkle), None),
            (Saying::one(S::Excited, T::Kite), None),
            (Saying::one(S::Excited, T::Fish), None),
        ],
        vec![
            (Saying::one(S::Whisper, T::Moon), None),
            (Saying::one(S::Whisper, T::Blush), None),
            (Saying::one(S::Thought, T::Sleep), None),
            (Saying::one(S::Thought, T::Question), None),
        ],
        vec![
            (Saying::one(S::Song, T::Note), None),
            (Saying::one(S::Tender, T::Heart), None),
            (Saying::one(S::Grumble, T::Storm), None),
            (Saying::one(S::Shout, T::Exclaim), None),
        ],
        vec![
            (
                Saying::one(S::Excited, T::Stick),
                Some(Saying::one(S::Excited, T::Exclaim)),
            ),
            (
                Saying::one(S::Chat, T::Food),
                Some(Saying::one(S::Tender, T::Heart)),
            ),
        ],
        vec![
            (
                Saying::one(S::Whisper, T::Moon),
                Some(Saying::one(S::Whisper, T::Heart)),
            ),
            (
                Saying::one(S::Chat, T::Wave),
                Some(Saying::one(S::Chat, T::Wave)),
            ),
        ],
    ];
    // Every picture, said the way it most often is.
    rows.extend(T::ALL.chunks(4).map(|topics| {
        topics
            .iter()
            .map(|topic| (Saying::one(S::Chat, *topic), None))
            .collect()
    }));
    let sheet = TalkSheet::render();
    let subjects = gesture_subjects();
    // Each companion's place, in texture pixels: room for a double bubble over a body.
    let (cw, ch) = (40 * d, 72 * d);
    let width = (4 * cw) as u32;
    let height = (rows.len() as i32 * ch) as u32;
    let mut canvas = formiga_art::Canvas::new(width, height);
    canvas.fill_rect(
        0,
        0,
        width as i32,
        height as i32,
        formiga_art::Rgba::new(236, 234, 228, 255),
    );
    let put = |canvas: &mut formiga_art::Canvas,
               x0: i32,
               y0: i32,
               subject: usize,
               facing_right: bool,
               saying: Saying| {
        let genome = &subjects[subject % subjects.len()];
        let face = FaceRenderState {
            expression: ExpressionKind::Joy,
            eyelids: EyelidPose::Open,
            gaze: GazeDirection::default(),
        };
        let frame = CreatureRenderer::render_composited_frame(
            genome,
            ActionKind::Idle,
            0,
            facing_right,
            false,
            face,
        );
        let top = frame.alpha_bounds().map_or(0, |(_, t, _, _)| t as i32);
        let fx = x0 + cw / 2 - FRAME_SIZE as i32 / 2 * d;
        let fy = y0 + ch - FRAME_SIZE as i32 * d;
        for y in 0..FRAME_SIZE as i32 {
            for x in 0..FRAME_SIZE as i32 {
                let pixel = frame.get(x, y);
                if pixel.a > 0 {
                    canvas.fill_rect(fx + x * d, fy + y * d, d, d, pixel);
                }
            }
        }
        let head_top = fy + top * d;
        let rect = TalkSheet::frame(saying.style, BubbleGrowth::Full);
        let bx = x0 + cw / 2 - (rect.width as i32 / d / 2) * d;
        let by = head_top - rect.height as i32 - d;
        let mut blit = |rect: formiga_art::SpriteRect, x: i32, y: i32| {
            for sy in 0..rect.height as i32 {
                for sx in 0..rect.width as i32 {
                    let pixel = sheet.get(rect.x as i32 + sx, rect.y as i32 + sy);
                    if pixel.a > 0 {
                        canvas.set(x + sx, y + sy, pixel);
                    }
                }
            }
        };
        blit(rect, bx, by);
        blit(
            TalkSheet::icon(saying.topic),
            bx + TALK_ICON_AT.0 as i32 * d,
            by + TALK_ICON_AT.1 as i32 * d,
        );
    };
    for (row, sayings) in rows.iter().enumerate() {
        let y0 = row as i32 * ch;
        let mut column = 0;
        for (saying, answer) in sayings {
            put(
                &mut canvas,
                column * cw,
                y0,
                row + column as usize,
                true,
                *saying,
            );
            column += 1;
            if let Some(answer) = answer {
                put(&mut canvas, column * cw, y0, row + 3, false, *answer);
                column += 1;
            }
        }
    }
    let mut pixels = vec![0_u8; (width * SCALE * height * SCALE * 4) as usize];
    for y in 0..height * SCALE {
        for x in 0..width * SCALE {
            let pixel = canvas.get((x / SCALE) as i32, (y / SCALE) as i32);
            let at = ((y * width * SCALE + x) * 4) as usize;
            pixels[at..at + 4].copy_from_slice(&[pixel.r, pixel.g, pixel.b, 255]);
        }
    }
    write_png(&path, width * SCALE, height * SCALE, &pixels)?;
    println!("wrote {}", path.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The hold sheet's marks are only worth anything if they can never be a pixel of the art:
    /// no frame of any body it shows has a mark's colour in it, and every point it marks is inside
    /// the frame it is drawn over.
    /// The mantle has a row of its own for every body on the gesture sheet, beside the
    /// watching loop's: one more row per body than the sheet had before it.
    #[test]
    fn the_gesture_sheet_gives_the_mantle_a_row_for_every_body() {
        const COLS: u32 = 7;
        let subjects = gesture_subjects().len() as u32;
        let references = reference_creatures().len() as u32;
        // Ten rows a reference body since the walks turned side-on and away took the sixty-third
        // to sixty-fifth cells, past the end of the ninth row of seven.
        let before = references * 10 + (subjects - references) * 6 + subjects;
        assert_eq!(gesture_sheet_rows(COLS), before + subjects);
    }

    #[test]
    fn the_hold_sheet_marks_are_no_colour_the_art_uses_and_land_in_the_frame() {
        let marks = [HANDS_MARK, MOUTH_MARK].map(|[r, g, b, _]| (r, g, b));
        let inside = |at: PixelPoint| {
            (0..FRAME_SIZE as i32).contains(&at.x) && (0..FRAME_SIZE as i32).contains(&at.y)
        };
        for (name, genome) in hold_subjects() {
            for (clip, label, face) in hold_clips() {
                for frame in 0..formiga_art::AnimationSpec::for_clip(clip).frames {
                    let picture = CreatureRenderer::render_composited_frame(
                        &genome, clip, frame, true, false, face,
                    );
                    for y in 0..FRAME_SIZE as i32 {
                        for x in 0..FRAME_SIZE as i32 {
                            let pixel = picture.get(x, y);
                            assert!(
                                pixel.a == 0 || !marks.contains(&(pixel.r, pixel.g, pixel.b)),
                                "{name} {label} frame {frame} draws a mark's colour at {x},{y}"
                            );
                        }
                    }
                    let hold = CreatureRenderer::hold_points(&genome, clip, frame, false);
                    assert!(
                        inside(hold.hands) && inside(hold.mouth),
                        "{name} {label} frame {frame}"
                    );
                }
            }
        }
    }
}
