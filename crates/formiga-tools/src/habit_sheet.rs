//! Every little habit and way of celebrating, drawn the way the desktop draws it.
//!
//!   cargo run -p formiga-tools -- habit-sheet [--output docs/assets/habit-sheet.png]
//!
//! One row per body: the three original families as a long-lived save still draws them, the five
//! modular plans, and two modular bodies on classic limbs. Along each row, a few moments of each
//! habit and each celebration, left to right:
//!
//! - resting, for reference, then four moments of stretching before a nap;
//! - four moments of turning in circles before a nap;
//! - three moments of looking a snack over, and one of a drink;
//! - two moments of waving hello, and two of a play bow;
//! - two beats each of the three celebrations: a hop, a little dance and a twirl.
//!
//! Every cell is taken through `BodyPresentation` and the face the desktop resolves for the same
//! creature at the same moment, so the sheet shows what a companion does rather than a pose table.

use crate::{blit_scaled, reference_creatures, write_png};
use anyhow::Result;
use formiga_art::{BodyPresentation, CreatureRenderer, FRAME_SIZE};
use formiga_core::*;
use std::path::PathBuf;
use time::OffsetDateTime;

const SCALE: u32 = 3;

/// One cell: what the creature is doing, the habit or celebration it is showing, and how far in.
#[derive(Clone, Copy)]
enum Moment {
    Resting,
    Habit(Habit, ActionKind, f32),
    Celebration(Celebration, f32),
}

fn moments() -> Vec<Moment> {
    let mut moments = vec![Moment::Resting];
    let habit = |habit, action, times: &[f32]| {
        times
            .iter()
            .map(move |at| Moment::Habit(habit, action, *at))
            .collect::<Vec<_>>()
    };
    moments.extend(habit(
        Habit::StretchesBeforeNaps,
        ActionKind::Sleep,
        &[0.1, 0.45, 0.8, 1.3],
    ));
    moments.extend(habit(
        Habit::CirclesBeforeNaps,
        ActionKind::Sleep,
        &[0.1, 0.5, 0.9, 1.3],
    ));
    moments.extend(habit(
        Habit::LooksFoodOver,
        ActionKind::Eat,
        &[0.2, 0.8, 1.3],
    ));
    moments.extend(habit(Habit::LooksFoodOver, ActionKind::Drink, &[0.2]));
    moments.extend(habit(Habit::WavesHello, ActionKind::Greet, &[0.1, 0.4]));
    moments.extend(habit(Habit::PlayBows, ActionKind::SoloPlay, &[0.1, 0.5]));
    for celebration in Celebration::ALL {
        moments.extend([0.05, 0.3].map(|at| Moment::Celebration(celebration, at)));
    }
    moments
}

/// The bodies on the sheet, as creatures rather than bare genomes, since what a creature shows
/// depends on who it is as well as what it looks like.
fn subjects() -> Vec<Creature> {
    let mut subjects: Vec<Creature> = reference_creatures()
        .into_iter()
        .map(|mut creature| {
            // Drawn the way a colony from before the modular bodies still draws them.
            creature.appearance.design = None;
            creature
        })
        .collect();
    let preview = World::preview_adult(
        [29; 32],
        OffsetDateTime::UNIX_EPOCH,
        &DesktopSnapshot::default(),
    );
    let plans = BodyPlan::ALL.map(|plan| (plan, 0));
    let classic = [(BodyPlan::Round, 1), (BodyPlan::Upright, 2)];
    for (plan, limbs) in plans.into_iter().chain(classic) {
        let mut creature = preview.clone();
        let mut design = CreatureDesign::modular([29; 32], 0, None);
        design.body = plan;
        design.classic = ClassicParts {
            limbs,
            ..Default::default()
        };
        creature.appearance.design = Some(design);
        subjects.push(creature);
    }
    subjects
}

/// The same creature, reseeded until it celebrates the way the cell asks for. Only the seed its
/// celebration is read from changes; the body and the face stay the subject's own.
fn celebrating(creature: &Creature, celebration: Celebration) -> Creature {
    let mut reseeded = creature.clone();
    for salt in 0..=u8::MAX {
        reseeded.behavior_seed[31] = salt;
        if Celebration::for_creature(&reseeded) == celebration {
            return reseeded;
        }
    }
    unreachable!("every celebration turns up within 256 seeds")
}

fn posed(subject: &Creature, moment: Moment) -> Creature {
    let mut creature = match moment {
        Moment::Celebration(celebration, _) => celebrating(subject, celebration),
        _ => subject.clone(),
    };
    let state = &mut creature.state;
    state.facing_right = true;
    state.attention = None;
    state.flourish = None;
    state.velocity = Point::default();
    match moment {
        Moment::Resting => {
            state.action = ActionKind::Idle;
            state.action_elapsed = 0.0;
        }
        Moment::Habit(habit, action, at) => {
            state.action = action;
            state.action_elapsed = at;
            state.flourish = Some(Flourish {
                habit,
                action,
                started_at: Some(0.0),
            });
        }
        Moment::Celebration(_, at) => {
            state.action = ActionKind::InspectScreen;
            state.action_elapsed = at;
            state.attention = Some(AttentionPose {
                target: state.position,
                emotion: AttentionEmotion::Enjoying,
                hanging: 0.0,
                gesture: Some(Gesture::Cheer),
            });
        }
    }
    creature
}

pub fn run(path: PathBuf) -> Result<()> {
    let moments = moments();
    let subjects = subjects();
    let cell = FRAME_SIZE * SCALE;
    let width = moments.len() as u32 * cell;
    let height = subjects.len() as u32 * cell;
    let mut pixels = vec![0_u8; (width * height * 4) as usize];
    for (row, subject) in subjects.iter().enumerate() {
        for (column, moment) in moments.iter().enumerate() {
            let creature = posed(subject, *moment);
            let body = BodyPresentation::for_creature(&creature);
            let face =
                CreatureRenderer::resolve_face_state(&creature, CursorSnapshot::default(), false);
            let canvas = CreatureRenderer::render_composited_frame(
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
                column as u32 * cell,
                row as u32 * cell,
                &canvas.rgba_bytes(),
                SCALE,
            );
        }
    }
    write_png(&path, width, height, &pixels)?;
    println!("wrote {}", path.display());
    Ok(())
}
