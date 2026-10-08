//! Every house style plain, dressed in every decoration, as a cottage and after dark, with its
//! resident beside it and up on the roof.

use crate::{blit_scaled_square_alpha, fill_gradient, fixture_desktop, write_png};
use anyhow::Result;
use formiga_art::{CreatureRenderer, FRAME_SIZE, SHELTER_SIZE, ShelterRenderer};
use formiga_core::*;
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use time::OffsetDateTime;

pub fn shelter_sheet(path: PathBuf) -> Result<()> {
    const SCALE: u32 = 3;
    const MARGIN: u32 = 8;
    const LANE_GAP: u32 = 10;
    // One row per style, all on one ground line: the colony house plain; the same house dressed
    // twice over, with something in each of its six places — two of the five kinds each place
    // takes, so the four rows between them show every decoration there is; a companion's cottage
    // hung with that companion's curtain, the same cottage with its resident at home behind the
    // drawn curtain, and lit from inside after dark; the colony house with its resident sitting
    // up on the roof, at the height the simulation seats it; the keepsake tree; and the companion
    // itself at the very same scale.
    let footprints = [
        DwellingKind::Main.width() as u32,
        DwellingKind::Main.width() as u32,
        DwellingKind::Main.width() as u32,
        DwellingKind::Cottage.width() as u32,
        DwellingKind::Cottage.width() as u32,
        DwellingKind::Cottage.width() as u32,
        DwellingKind::Main.width() as u32,
        TREE_WIDTH as u32,
        FRAME_SIZE,
    ];
    let lane: u32 = footprints.iter().sum::<u32>() + LANE_GAP * (footprints.len() as u32 - 1);
    let width = (lane + MARGIN * 2) * SCALE;
    let row_height = (SHELTER_SIZE + 20) * SCALE + MARGIN * SCALE;
    let height = row_height * 4 + MARGIN * SCALE;
    let mut pixels = vec![0_u8; (width * height * 4) as usize];
    fill_gradient(
        &mut pixels,
        width,
        height,
        [18, 29, 34, 255],
        [35, 55, 54, 255],
    );
    // Every decoration, by the place on a house it goes in.
    let in_place = |place: DecorationSlot| -> Vec<ShelterDecorationKind> {
        ShelterDecorationKind::ALL
            .into_iter()
            .filter(|kind| kind.slot() == place)
            .collect()
    };
    for style in 0..4_u32 {
        let mut seed = [0_u8; 32];
        seed.copy_from_slice(&Sha256::digest(format!("formiga-shelter-{style}")));
        seed[1] = style as u8;
        let home = ColonyHome::from_seed(seed, None, None, None);
        let resident = World::preview_adult(
            SeedStream::new(seed).bytes("shelter-sheet-resident", 0),
            OffsetDateTime::UNIX_EPOCH,
            &fixture_desktop(),
        );
        let marks = [None, Some(formiga_art::ResidentMark::of(&resident))];
        let village = ShelterRenderer::render_village(&home.shelter, &[], &marks, &[], true);
        let cell = |cell: formiga_art::VillageCell| {
            let (x, y) = ShelterRenderer::village_cell(cell);
            let mut tile = formiga_art::Canvas::new(SHELTER_SIZE, SHELTER_SIZE);
            for ty in 0..SHELTER_SIZE as i32 {
                for tx in 0..SHELTER_SIZE as i32 {
                    tile.set(tx, ty, village.get(x as i32 + tx, y as i32 + ty));
                }
            }
            tile
        };
        let cottage = |lit, occupied| formiga_art::VillageCell::House {
            slot: 1,
            lit,
            occupied,
        };
        let dressed = |pick: usize| {
            let kinds: Vec<ShelterDecorationKind> = DecorationSlot::ALL
                .into_iter()
                .map(|place| {
                    let choices = in_place(place);
                    choices[pick % choices.len()]
                })
                .collect();
            ShelterRenderer::render_with_decorations(&home.shelter, &kinds)
        };
        let pick = style as usize * 2;
        let lane_items = [
            (ShelterRenderer::render(&home.shelter), SHELTER_SIZE),
            (dressed(pick), SHELTER_SIZE),
            (dressed(pick + 1), SHELTER_SIZE),
            (cell(cottage(false, false)), SHELTER_SIZE),
            (cell(cottage(false, true)), SHELTER_SIZE),
            (cell(cottage(true, true)), SHELTER_SIZE),
            (ShelterRenderer::render(&home.shelter), SHELTER_SIZE),
            (cell(formiga_art::VillageCell::Tree), SHELTER_SIZE),
            (
                CreatureRenderer::render_frame(
                    &resident.appearance,
                    ActionKind::Homebound,
                    0,
                    true,
                ),
                FRAME_SIZE,
            ),
        ];
        // Every dwelling cell stands three rows above the foot of its own cell; a creature frame
        // stands on its own last row. Lining those two up puts everything on one ground line.
        let baseline = style * row_height + (MARGIN + 20 + SHELTER_SIZE - 3) * SCALE;
        let mut pen = MARGIN;
        for (index, ((canvas, size), footprint)) in
            lane_items.into_iter().zip(footprints).enumerate()
        {
            let centre = pen + footprint / 2;
            let foot = if size == FRAME_SIZE { 0 } else { 3 };
            blit_scaled_square_alpha(
                &mut pixels,
                width,
                (centre.saturating_sub(size / 2)) * SCALE,
                baseline + foot * SCALE - size * SCALE,
                &canvas.rgba_bytes(),
                size,
                SCALE,
            );
            // Up on the roof: the resident's perched frame placed exactly as the overlay places
            // it, its lowest resting pixel on the point the simulation seats it at — the ground
            // line at the foot of the house's cell, less the house's own roof height.
            if index == 6 {
                let roof = formiga_core::house_roof_height(&home.shelter, home.shelter.style, true)
                    .round() as u32;
                let sitter = CreatureRenderer::render_frame(
                    &resident.appearance,
                    ActionKind::Perch,
                    0,
                    true,
                );
                let ground = baseline + 3 * SCALE;
                let contact = ground - roof * SCALE;
                let placement = formiga_art::FramePlacement::for_action(
                    ActionKind::Perch,
                    CreatureRenderer::resting_baseline(&resident.appearance, false),
                );
                let top = (contact as i32 + placement.origin_y * SCALE as i32) as u32;
                blit_scaled_square_alpha(
                    &mut pixels,
                    width,
                    (centre.saturating_sub(FRAME_SIZE / 2)) * SCALE,
                    top,
                    &sitter.rgba_bytes(),
                    FRAME_SIZE,
                    SCALE,
                );
            }
            pen = centre + footprint / 2 + LANE_GAP;
        }
    }
    write_png(&path, width, height, &pixels)?;
    println!("wrote {}", path.display());
    Ok(())
}
