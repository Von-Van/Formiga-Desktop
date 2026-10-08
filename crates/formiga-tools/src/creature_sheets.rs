//! Review sheets of companions themselves: a contact sheet of fresh ones, the modular parts on
//! every body plan, classic parts, one companion's every animation, its expressions, and the
//! card a companion is shared on.

use crate::{
    SCENE_CELL, blit_canvas_scaled, blit_scaled, blit_scaled_square_alpha, fill_gradient,
    fixture_desktop, reference_creatures, write_png,
};
use anyhow::Result;
use formiga_art::{
    BodyClip, CARD_HEIGHT, CARD_WIDTH, CreatureCardRenderer, CreatureRenderer, ExpressionKind,
    EyelidPose, FRAME_SIZE, FaceRenderState, GazeDirection,
};
use formiga_core::*;
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use time::OffsetDateTime;

pub fn creature_card(path: PathBuf) -> Result<()> {
    let mut seed = [0_u8; 32];
    seed.copy_from_slice(&Sha256::digest(b"formiga-creature-card-preview"));
    let mut world = World::new(seed, OffsetDateTime::UNIX_EPOCH, &fixture_desktop());
    let creature = &mut world.save.creatures[0];
    creature.name = "Mallow".into();
    creature.born_at_utc = time::macros::datetime!(2026-08-14 12:30 UTC);
    creature.tendencies.climbing = 62.0;
    creature.tendencies.exploration = 53.0;
    creature.tendencies.sociability = 41.0;
    formiga_core::update_descriptor_flags(&mut creature.memory, creature.tendencies);
    let card = CreatureCardRenderer::render(creature);
    write_png(&path, CARD_WIDTH, CARD_HEIGHT, &card.rgba_bytes())?;
    println!("wrote {}", path.display());
    Ok(())
}

pub fn generation_sheet(path: PathBuf) -> Result<()> {
    let scale = 4;
    // Each companion stands beside the same village tree, so their sizes read against it.
    let (cell_width, cell_height) = (SCENE_CELL.0 * scale, SCENE_CELL.1 * scale);
    let (width, height) = (
        cell_width * EarStyle::ALL.len() as u32,
        cell_height * BodyPlan::ALL.len() as u32,
    );
    let mut pixels = vec![0; (width * height * 4) as usize];
    fill_gradient(
        &mut pixels,
        width,
        height,
        [237, 234, 224, 255],
        [209, 226, 219, 255],
    );
    for (row, body) in BodyPlan::ALL.into_iter().enumerate() {
        for (column, ears) in EarStyle::ALL.into_iter().enumerate() {
            let seed =
                SeedStream::new([55; 32]).bytes("generation-sheet", (row * 6 + column) as u64);
            let mut creature =
                World::preview_adult(seed, OffsetDateTime::UNIX_EPOCH, &fixture_desktop());
            // The grid is of modular parts, so it starts from the modular recipe alone.
            let mut design = CreatureDesign::modular(seed, 0, None);
            design.body = body;
            design.ears = ears;
            apply_creature_design(&mut creature, Some(design));
            let canvas =
                CreatureRenderer::render_frame(&creature.appearance, ActionKind::Idle, 0, true);
            let scene = formiga_art::beside_tree(&canvas, formiga_art::standing_row(&canvas));
            blit_canvas_scaled(
                &mut pixels,
                width,
                column as u32 * cell_width,
                row as u32 * cell_height,
                &scene,
                scale,
            );
        }
    }
    write_png(&path, width, height, &pixels)?;
    println!("wrote {}", path.display());
    Ok(())
}

/// Each classic part alone on every body plan, then wholly classic companions in motion.
///
/// The first band starts every row from the same plain modular recipe and changes one part per
/// column: candy colours, the five eye arrangements, nubs and stick legs, antennae and sprouts,
/// the three patterns, and the two tails. The second band gives each plan every classic part at
/// once and runs it through standing, walking, greeting, hanging, sleeping, and five gestures,
/// which is where a nub, a stick leg, or an antenna would come loose if it were going to.
pub fn classic_sheet(path: PathBuf) -> Result<()> {
    const SCALE: u32 = 3;
    let parts = |edit: fn(&mut ClassicParts)| {
        let mut parts = ClassicParts::default();
        edit(&mut parts);
        parts
    };
    let singles = [
        parts(|_| {}),
        parts(|p| p.coat = 1),
        parts(|p| p.face = 1),
        parts(|p| p.face = 2),
        parts(|p| p.face = 3),
        parts(|p| p.face = 4),
        parts(|p| p.face = 5),
        parts(|p| p.limbs = 1),
        parts(|p| p.limbs = 2),
        parts(|p| p.crown = 1),
        parts(|p| p.crown = 2),
        parts(|p| p.pattern = 1),
        parts(|p| p.pattern = 2),
        parts(|p| p.pattern = 3),
        parts(|p| p.tail = 1),
        parts(|p| p.tail = 2),
    ];
    let motion: [(BodyClip, u8); 10] = [
        (BodyClip::Action(ActionKind::Idle), 0),
        (BodyClip::Action(ActionKind::Traverse), 1),
        (BodyClip::Action(ActionKind::Greet), 1),
        (BodyClip::Action(ActionKind::Dangle), 0),
        (BodyClip::Action(ActionKind::Sleep), 0),
        (BodyClip::Gesture(Gesture::Cheer), 1),
        (BodyClip::Gesture(Gesture::Gasp), 0),
        (BodyClip::Gesture(Gesture::Cover), 0),
        (BodyClip::Gesture(Gesture::Reach), 1),
        (BodyClip::Gesture(Gesture::Watch), 0),
    ];
    let cell = FRAME_SIZE * SCALE;
    let rows = BodyPlan::ALL.len() as u32;
    let (width, height) = (cell * singles.len() as u32, cell * rows * 2);
    let mut pixels = vec![0; (width * height * 4) as usize];
    fill_gradient(
        &mut pixels,
        width,
        height,
        [237, 234, 224, 255],
        [209, 226, 219, 255],
    );
    let mut creature =
        World::preview_adult([13; 32], OffsetDateTime::UNIX_EPOCH, &fixture_desktop());
    // A coat and accent far enough apart that every accent-coloured part shows against the body,
    // on the plain modular recipe the original generator draws from the same seed: classic parts
    // belong to that generator, and a layout would put its eyes over the arrangements this sheet
    // is here to show.
    let base = CreatureDesign {
        classic: ClassicParts::default(),
        coat: [118, 172, 196],
        accent: [242, 168, 88],
        ..CreatureDesign::generated_by(Edition::Original, [13; 32], 0, None)
    };
    let mut draw = |creature: &Creature, clip: BodyClip, frame: u8, column: u32, row: u32| {
        let state = FaceRenderState {
            expression: ExpressionKind::Neutral,
            eyelids: if clip == BodyClip::Action(ActionKind::Sleep) {
                EyelidPose::Closed
            } else {
                EyelidPose::Open
            },
            gaze: GazeDirection::default(),
        };
        let canvas = CreatureRenderer::render_composited_frame(
            &creature.appearance,
            clip,
            frame,
            true,
            false,
            state,
        );
        blit_scaled_square_alpha(
            &mut pixels,
            width,
            column * cell,
            row * cell,
            &canvas.rgba_bytes(),
            FRAME_SIZE,
            SCALE,
        );
    };
    for (row, body) in BodyPlan::ALL.into_iter().enumerate() {
        for (column, classic) in singles.iter().enumerate() {
            apply_creature_design(
                &mut creature,
                Some(CreatureDesign {
                    body,
                    classic: *classic,
                    ..base
                }),
            );
            let idle = BodyClip::Action(ActionKind::Idle);
            draw(&creature, idle, 0, column as u32, row as u32);
        }
        // Every part at once, with a different eye arrangement, crown, and tail on each row.
        let index = row as u8;
        apply_creature_design(
            &mut creature,
            Some(CreatureDesign {
                body,
                classic: ClassicParts {
                    coat: 1,
                    face: 1 + index % 5,
                    limbs: 1 + index % 2,
                    crown: 1 + index % 2,
                    pattern: 1 + index % 3,
                    tail: 1 + index % 2,
                },
                coat: [0xe9, 0x8a, 0xb5],
                accent: [0xff, 0xe0, 0x81],
                ..base
            }),
        );
        for (column, (clip, frame)) in motion.into_iter().enumerate() {
            draw(&creature, clip, frame, column as u32, rows + row as u32);
        }
    }
    write_png(&path, width, height, &pixels)?;
    println!("wrote {}", path.display());
    Ok(())
}

pub fn animation_preview(path: PathBuf, seed_number: u64) -> Result<()> {
    const SCALE: u32 = 3;
    // One column per frame of the longest clip, so a clip that grows stays on its own row.
    let cols = ActionKind::ALL
        .into_iter()
        .map(|action| u32::from(formiga_art::AnimationSpec::for_action(action).frames))
        .max()
        .unwrap_or(1);
    let rows = ActionKind::ALL.len() as u32;
    let cell = FRAME_SIZE * SCALE;
    let width = cols * cell;
    let height = rows * cell;
    let mut pixels = vec![0_u8; (width * height * 4) as usize];
    let mut seed = [0_u8; 32];
    seed.copy_from_slice(&Sha256::digest(seed_number.to_le_bytes()));
    let world = World::new(seed, OffsetDateTime::UNIX_EPOCH, &fixture_desktop());
    let creature = &world.save.creatures[0];
    for (row, action) in ActionKind::ALL.into_iter().enumerate() {
        let spec = formiga_art::AnimationSpec::for_action(action);
        for frame in 0..spec.frames {
            let rendered =
                CreatureRenderer::render_frame(&creature.appearance, action, frame, true);
            blit_scaled(
                &mut pixels,
                width,
                u32::from(frame) * cell,
                row as u32 * cell,
                &rendered.rgba_bytes(),
                SCALE,
            );
        }
    }
    write_png(&path, width, height, &pixels)?;
    println!(
        "wrote {} for seed {} ({:?})",
        path.display(),
        seed_number,
        creature.appearance.family
    );
    Ok(())
}

pub fn contact_sheet(path: PathBuf) -> Result<()> {
    const COLS: u32 = 10;
    const ROWS: u32 = 10;
    const SCALE: u32 = 3;
    let cell = FRAME_SIZE * SCALE;
    let mut pixels = vec![0_u8; (COLS * cell * ROWS * cell * 4) as usize];
    let desktop = fixture_desktop();
    for index in 0..COLS * ROWS {
        let mut root = [0_u8; 32];
        root.copy_from_slice(&Sha256::digest(format!("formiga-contact-{index}")));
        let world = World::new(root, OffsetDateTime::UNIX_EPOCH, &desktop);
        let creature = &world.save.creatures[0];
        let frame = CreatureRenderer::render_frame(
            &creature.appearance,
            ActionKind::Idle,
            (index % 4) as u8,
            true,
        );
        blit_scaled(
            &mut pixels,
            COLS * cell,
            index % COLS * cell,
            index / COLS * cell,
            &frame.rgba_bytes(),
            SCALE,
        );
    }
    write_png(&path, COLS * cell, ROWS * cell, &pixels)?;
    println!("wrote {}", path.display());
    Ok(())
}

pub fn expression_sheet(path: PathBuf) -> Result<()> {
    const SCALE: u32 = 3;
    let creatures = reference_creatures();
    let cell = FRAME_SIZE * SCALE;
    let width = ExpressionKind::ALL.len() as u32 * cell;
    let height = creatures.len() as u32 * cell;
    let mut pixels = vec![0_u8; (width * height * 4) as usize];
    for (row, creature) in creatures.iter().enumerate() {
        for (column, expression) in ExpressionKind::ALL.into_iter().enumerate() {
            let rendered = CreatureRenderer::render_composited_frame(
                &creature.appearance,
                ActionKind::Idle,
                0,
                true,
                false,
                FaceRenderState {
                    expression,
                    eyelids: EyelidPose::Open,
                    gaze: GazeDirection::default(),
                },
            );
            blit_scaled(
                &mut pixels,
                width,
                column as u32 * cell,
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
