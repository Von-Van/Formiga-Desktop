//! `formiga-tools turn-sheet`: companions turned side-on and turned away, beside themselves as
//! the app draws them, to judge whether a turned body still reads as the same companion.
//!
//! One band a companion. Its first row is the turnaround — facing the screen, side-on, turned
//! away, side-on the other way — and then the side-on walk; its second is the walk facing the
//! screen and then the walk away; its third the run facing the screen and then side-on. The
//! walks and the run are the clips the app shows; the standing views are drawn only here.

use crate::cast;
use crate::fixtures::reference_creatures;
use crate::pixels::{blit_canvas_scaled, fill_gradient, fill_rect, write_png};
use anyhow::Result;
use formiga_art::{
    AccessoryArt, AnimationSpec, BodyClip, CreatureRenderer, ExpressionKind, EyelidPose,
    FRAME_SIZE, FaceRenderState, GazeDirection, TURNED_STRIDE_FRAMES, View,
};
use formiga_core::{
    Accessory, AccessoryKind, ActionKind, Anatomy, BodyPlan, Creature, CreatureDesign, EarStyle,
    Gesture, apply_creature_design,
};
use std::path::PathBuf;

const SCALE: u32 = 4;
const CELL: u32 = FRAME_SIZE * SCALE;
/// The four standing views and the six frames of a turned walk.
const COLUMNS: u32 = 4 + TURNED_STRIDE_FRAMES as u32;
const ROWS: u32 = 3;

pub fn run(output: PathBuf, args: &[String]) -> Result<()> {
    let mut companions = cast::colony_argument(args)?;
    companions.extend(cast::cast_argument(args)?);
    if companions.is_empty() {
        companions = reference_creatures();
    }
    // `--fox`: the same companions with Echo Fox Mode on, the fox standing in for each of them.
    if args.iter().any(|arg| arg == "--fox") {
        companions.truncate(1);
        for companion in &mut companions {
            companion.appearance.echo_fox = true;
        }
    }
    let (width, height) = (CELL * COLUMNS, CELL * ROWS * companions.len() as u32);
    let mut pixels = vec![0; (width * height * 4) as usize];
    fill_gradient(
        &mut pixels,
        width,
        height,
        [240, 236, 226, 255],
        [214, 228, 221, 255],
    );
    for (index, companion) in companions.iter().enumerate() {
        band(&mut pixels, width, index as u32 * CELL * ROWS, companion);
    }
    write_png(&output, width, height, &pixels)?;
    println!("wrote {}", output.display());
    Ok(())
}

fn band(pixels: &mut [u8], width: u32, top: u32, companion: &Creature) {
    let face = FaceRenderState {
        expression: ExpressionKind::Neutral,
        eyelids: EyelidPose::Open,
        gaze: GazeDirection::default(),
    };
    let genome = &companion.appearance;
    // The ground both rows stand on, and a rule between the standing views and the walks.
    for row in 0..ROWS {
        fill_rect(
            pixels,
            width,
            0,
            top + row * CELL + 41 * SCALE,
            width,
            1,
            [120, 130, 120, 70],
        );
    }
    fill_rect(
        pixels,
        width,
        4 * CELL,
        top,
        1,
        CELL * ROWS,
        [120, 130, 120, 70],
    );
    let mut put = |column: u32, row: u32, view: View, step: Option<u8>, right: bool| {
        if let Some(frame) = CreatureRenderer::render_turned_frame(genome, view, step, right, face)
        {
            blit_canvas_scaled(
                pixels,
                width,
                column * CELL,
                top + row * CELL,
                &frame,
                SCALE,
            );
        }
    };
    put(0, 0, View::Front, None, true);
    put(1, 0, View::Side, None, true);
    put(2, 0, View::Back, None, true);
    put(3, 0, View::Side, None, false);
    // The clips as the app draws them: whole frames, face and all.
    let mut clip = |column: u32, row: u32, clip: BodyClip, frame: u8| {
        let frame =
            CreatureRenderer::render_composited_frame(genome, clip, frame, true, false, face);
        blit_canvas_scaled(
            pixels,
            width,
            column * CELL,
            top + row * CELL,
            &frame,
            SCALE,
        );
    };
    let rows = [
        (ActionKind::Traverse, Gesture::Stride),
        (ActionKind::Traverse, Gesture::Depart),
        (ActionKind::Sprint, Gesture::Dash),
    ];
    for (row, (front, turned)) in rows.into_iter().enumerate() {
        let row = row as u32;
        // Facing the screen, to set the turned one against; the turnaround has the first row.
        if row > 0 {
            for frame in 0..AnimationSpec::for_clip(front).frames.min(4) {
                clip(u32::from(frame), row, front.into(), frame);
            }
        }
        for frame in 0..TURNED_STRIDE_FRAMES {
            clip(4 + u32::from(frame), row, turned.into(), frame);
        }
    }
}

/// `turn-sheet --animate`: the same companions moving, a lane each, as a looping GIF. Each walks
/// along side-on, stops and faces the screen, runs back, walks away up the lane and comes back
/// down it facing the screen: the clips the app shows, in the order a stroll about the village
/// would show them.
pub fn animate(output: PathBuf, args: &[String]) -> Result<()> {
    /// Art pixels a lane is wide and tall, and the row its floor is on.
    const LANE: (u32, u32) = (140, 62);
    const RISE: i32 = 12;
    const FPS: u16 = 10;
    const SCALE: u32 = 3;
    let mut companions = cast::colony_argument(args)?;
    companions.extend(cast::cast_argument(args)?);
    if companions.is_empty() {
        companions = reference_creatures();
    }
    let face = FaceRenderState {
        expression: ExpressionKind::Neutral,
        eyelids: EyelidPose::Open,
        gaze: GazeDirection::default(),
    };
    let (width, height) = (LANE.0 * SCALE, LANE.1 * SCALE * companions.len() as u32);
    let file = std::fs::File::create(&output)?;
    let mut encoder = gif::Encoder::new(
        std::io::BufWriter::new(file),
        width as u16,
        height as u16,
        &[],
    )?;
    encoder.set_repeat(gif::Repeat::Infinite)?;
    // What it shows on each tenth of a second: the clip, its frame, which way it faces, and
    // where it is along the lane and up it.
    let mut script: Vec<(BodyClip, u8, bool, i32, i32)> = Vec::new();
    let idle = BodyClip::Action(ActionKind::Idle);
    let (left, right) = (4, 84);
    for step in 0..40 {
        script.push((
            Gesture::Stride.into(),
            (step % 6) as u8,
            true,
            left + step * 2,
            0,
        ));
    }
    script.extend((0..6).map(|_| (idle, 0, true, right, 0)));
    for step in 0..16 {
        script.push((
            Gesture::Dash.into(),
            (step % 6) as u8,
            false,
            right - step * 5,
            0,
        ));
    }
    script.extend((0..6).map(|_| (idle, 0, false, left, 0)));
    for step in 0..18 {
        script.push((
            Gesture::Depart.into(),
            (step % 6) as u8,
            true,
            left,
            step * RISE / 18,
        ));
    }
    script.extend((0..6).map(|_| (idle, 0, true, left, RISE)));
    for step in 0..18 {
        let frame = (step % i32::from(AnimationSpec::for_clip(ActionKind::Traverse).frames)) as u8;
        script.push((
            ActionKind::Traverse.into(),
            frame,
            true,
            left,
            RISE - step * RISE / 18,
        ));
    }
    script.extend((0..6).map(|_| (idle, 0, true, left, 0)));
    for (clip, frame, facing_right, along, up) in script {
        let mut pixels = vec![0; (width * height * 4) as usize];
        fill_gradient(
            &mut pixels,
            width,
            height,
            [240, 236, 226, 255],
            [214, 228, 221, 255],
        );
        for (lane, companion) in companions.iter().enumerate() {
            let top = lane as u32 * LANE.1 * SCALE;
            // The front of the grass and the back of it.
            for row in [41 + RISE as u32, 41] {
                fill_rect(
                    &mut pixels,
                    width,
                    0,
                    top + row * SCALE,
                    width,
                    1,
                    [120, 130, 120, 70],
                );
            }
            let drawn = CreatureRenderer::render_composited_frame(
                &companion.appearance,
                clip,
                frame,
                facing_right,
                false,
                face,
            );
            blit_canvas_scaled(
                &mut pixels,
                width,
                along as u32 * SCALE,
                top + (RISE - up) as u32 * SCALE,
                &drawn,
                SCALE,
            );
        }
        let mut gif_frame =
            gif::Frame::from_rgba_speed(width as u16, height as u16, pixels.as_mut_slice(), 20);
        gif_frame.delay = 100 / FPS;
        encoder.write_frame(&gif_frame)?;
    }
    println!("wrote {}", output.display());
    Ok(())
}

/// The bodies a turn-sheet shows each part of a companion's anatomy on, in the order of their
/// plans, as a companion of each plan the generator draws.
fn subject(plan: BodyPlan, ears: EarStyle) -> CreatureDesign {
    let design = (0_u8..=255)
        .map(|index| CreatureDesign::drawn([index; 32], 0).0)
        .find(|design| design.body == plan && design.classic.is_modular())
        .unwrap_or_else(|| CreatureDesign::drawn([0; 32], 0).0);
    CreatureDesign {
        body: plan,
        ears,
        ear_size: design.ear_size.max(5),
        details: Default::default(),
        ..design
    }
    .bounded()
}

/// `turn-sheet --anatomy`: each part of the anatomy added in 0.69.1 on every body it fits, a row
/// each — standing facing the screen, side-on and turned away; the side-on walk and run and the
/// walk away; the walk facing the screen; and a mini of it side-on — so a part can be judged in
/// every view a companion wearing it is drawn in.
pub fn anatomy(output: PathBuf) -> Result<()> {
    const COLUMNS: u32 = 8;
    let face = FaceRenderState {
        expression: ExpressionKind::Neutral,
        eyelids: EyelidPose::Open,
        gaze: GazeDirection::default(),
    };
    let base = reference_creatures().remove(0);
    let mut rows = Vec::new();
    for part in Anatomy::ALL {
        for plan in BodyPlan::ALL {
            let ears = if part == Anatomy::EarTufts {
                [EarStyle::Pointed, EarStyle::Long][rows.len() % 2]
            } else {
                EarStyle::Round
            };
            let design = subject(plan, ears);
            if !part.fits(&design) {
                continue;
            }
            let color = match part {
                Anatomy::Ruff => [246, 232, 204],
                Anatomy::Brush => [252, 248, 238],
                _ => [255, 196, 120],
            };
            rows.push(part.put_on(design, color));
        }
    }
    let (width, height) = (CELL * COLUMNS, CELL * rows.len() as u32);
    let mut pixels = vec![0; (width * height * 4) as usize];
    fill_gradient(
        &mut pixels,
        width,
        height,
        [240, 236, 226, 255],
        [214, 228, 221, 255],
    );
    for (row, design) in rows.into_iter().enumerate() {
        let mut creature = base.clone();
        apply_creature_design(&mut creature, Some(design));
        let genome = &creature.appearance;
        let mut mini = genome.clone();
        mini.logical_size = 26;
        let top = row as u32 * CELL;
        fill_rect(
            &mut pixels,
            width,
            0,
            top + 41 * SCALE,
            width,
            1,
            [120, 130, 120, 70],
        );
        let mut put = |column: u32, canvas: Option<formiga_art::Canvas>| {
            if let Some(canvas) = canvas {
                blit_canvas_scaled(&mut pixels, width, column * CELL, top, &canvas, SCALE);
            }
        };
        put(
            0,
            CreatureRenderer::render_turned_frame(genome, View::Front, None, true, face),
        );
        put(
            1,
            CreatureRenderer::render_turned_frame(genome, View::Side, None, true, face),
        );
        put(
            2,
            CreatureRenderer::render_turned_frame(genome, View::Back, None, true, face),
        );
        for (column, clip, frame) in [
            (3, BodyClip::Gesture(Gesture::Stride), 1),
            (4, BodyClip::Gesture(Gesture::Dash), 2),
            (5, BodyClip::Gesture(Gesture::Depart), 4),
            (6, BodyClip::Action(ActionKind::Traverse), 1),
        ] {
            put(
                column,
                Some(CreatureRenderer::render_composited_frame(
                    genome, clip, frame, true, false, face,
                )),
            );
        }
        put(
            7,
            CreatureRenderer::render_turned_frame(&mini, View::Side, None, true, face),
        );
    }
    write_png(&output, width, height, &pixels)?;
    println!("wrote {}", output.display());
    Ok(())
}

/// `turn-sheet --wearables`: everything a companion can wear on every turned view, a row a
/// piece. Along each row, bodies of each plan that turns at three statures, an original blob, a
/// mini and the Echo Fox, each standing facing the screen, side-on and turned away, so a piece
/// can be judged following the body round.
pub fn wearables(output: PathBuf) -> Result<()> {
    const SCALE: u32 = 2;
    let cell = FRAME_SIZE * SCALE;
    let face = FaceRenderState {
        expression: ExpressionKind::Neutral,
        eyelids: EyelidPose::Open,
        gaze: GazeDirection::default(),
    };
    let base = reference_creatures().remove(0);
    let mut subjects = Vec::new();
    for (plan, size) in [
        (BodyPlan::Round, 44),
        (BodyPlan::Upright, 33),
        (BodyPlan::Winged, 38),
        (BodyPlan::Blob, 38),
        (BodyPlan::Round, 26),
    ] {
        let mut creature = base.clone();
        apply_creature_design(&mut creature, Some(subject(plan, EarStyle::Round)));
        creature.appearance.logical_size = size;
        subjects.push(creature.appearance);
    }
    let mut original = base.appearance.clone();
    original.design = None;
    subjects.push(original);
    let mut fox = base.appearance.clone();
    fox.echo_fox = true;
    subjects.push(fox);
    let members: Vec<formiga_art::Palette> =
        subjects.iter().map(formiga_art::palette_for).collect();
    let mut rows: Vec<Option<Accessory>> = vec![None];
    rows.extend(AccessoryKind::ALL.map(|kind| Some(Accessory::Worn(kind))));
    rows.extend([0_u8, 58].map(|variant| Some(Accessory::Pin(variant))));
    let views = [View::Front, View::Side, View::Back];
    let width = cell * (subjects.len() * views.len()) as u32;
    let height = cell * rows.len() as u32;
    let mut pixels = vec![0; (width * height * 4) as usize];
    for (row, accessory) in rows.iter().enumerate() {
        let band = if row % 2 == 0 {
            [236, 232, 222, 255]
        } else {
            [196, 206, 200, 255]
        };
        fill_rect(&mut pixels, width, 0, row as u32 * cell, width, cell, band);
        let dress = accessory.map(|accessory| AccessoryArt::resolve(accessory, [61; 32], &members));
        for (index, genome) in subjects.iter().enumerate() {
            for (turn, view) in views.into_iter().enumerate() {
                let Some(canvas) = CreatureRenderer::render_dressed_turned_frame(
                    genome, dress, view, None, true, face,
                ) else {
                    continue;
                };
                let column = (index * views.len() + turn) as u32;
                blit_canvas_scaled(
                    &mut pixels,
                    width,
                    column * cell,
                    row as u32 * cell,
                    &canvas,
                    SCALE,
                );
            }
        }
    }
    write_png(&output, width, height, &pixels)?;
    println!("wrote {}", output.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use formiga_core::BodyFamily;

    fn face() -> FaceRenderState {
        FaceRenderState {
            expression: ExpressionKind::Neutral,
            eyelids: EyelidPose::Open,
            gaze: GazeDirection::default(),
        }
    }

    #[test]
    fn a_turned_body_stays_in_its_frame_and_mirrors_with_its_facing() {
        let blob = reference_creatures()
            .into_iter()
            .find(|creature| creature.appearance.family == BodyFamily::Blob)
            .unwrap();
        for view in [View::Side, View::Back] {
            let standing = (None, 1);
            let walking = (0..TURNED_STRIDE_FRAMES).map(|step| (Some(step), 1));
            let mut frames = Vec::new();
            for (step, _) in std::iter::once(standing).chain(walking) {
                let right = CreatureRenderer::render_turned_frame(
                    &blob.appearance,
                    view,
                    step,
                    true,
                    face(),
                )
                .expect("a blob turns");
                let (left, top, right_edge, bottom) = right.alpha_bounds().expect("drawn");
                assert!(left >= 1 && top >= 1, "{view:?} {step:?} touches the frame");
                assert!(right_edge <= FRAME_SIZE - 2 && bottom <= FRAME_SIZE - 2);
                let mut mirrored = CreatureRenderer::render_turned_frame(
                    &blob.appearance,
                    view,
                    step,
                    false,
                    face(),
                )
                .unwrap();
                mirrored.mirror_horizontal();
                assert_eq!(mirrored.rgba_bytes(), right.rgba_bytes());
                frames.push(right.rgba_bytes());
            }
            // A walk moves: not every frame of it is the standing one.
            assert!(frames[1..].iter().any(|frame| *frame != frames[0]));
        }
    }

    #[test]
    fn turned_away_shows_no_face_and_side_on_is_not_the_front() {
        let blob = reference_creatures()
            .into_iter()
            .find(|creature| creature.appearance.family == BodyFamily::Blob)
            .unwrap();
        let genome = &blob.appearance;
        let eye = formiga_art::palette_for(genome).eye;
        let shows_eye = |view: View| {
            let frame =
                CreatureRenderer::render_turned_frame(genome, view, None, true, face()).unwrap();
            frame.pixels().iter().filter(|pixel| **pixel == eye).count()
        };
        assert_eq!(shows_eye(View::Back), 0);
        assert!(shows_eye(View::Side) > 0);
        assert!(shows_eye(View::Side) < shows_eye(View::Front));
    }

    #[test]
    fn a_body_nothing_draws_turned_is_left_out() {
        let four_pawed = cast::four_pawed(0);
        for view in [View::Side, View::Back] {
            assert!(
                CreatureRenderer::render_turned_frame(
                    &four_pawed.appearance,
                    view,
                    None,
                    true,
                    face()
                )
                .is_none()
            );
        }
        assert!(
            CreatureRenderer::render_turned_frame(
                &four_pawed.appearance,
                View::Front,
                None,
                true,
                face()
            )
            .is_some()
        );
    }
}
