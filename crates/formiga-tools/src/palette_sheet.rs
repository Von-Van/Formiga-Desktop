//! The village in every palette it can be painted in, for each style of house.
//!
//!   cargo run -p formiga-tools -- village-palette-sheet [--output docs/assets/village-palette-sheet.png]
//!
//! One row per style of house: leaf tent, mushroom hut, cushion den and paper house. Along each
//! row, the village in the colours its own seed gave it, then in each named palette in the order
//! the Home page lists them: Meadow, Blossom, Harbour, Autumn, Twilight and Pebble. Each village
//! is its keepsake tree, the colony house and one cottage hung with its keeper's curtain, cut
//! from the same atlas the desktop samples, so a palette is judged on the houses it paints.

use crate::{blit_scaled_square_alpha, fill_gradient, fixture_desktop, write_png};
use anyhow::Result;
use formiga_art::{Canvas, ResidentMark, SHELTER_SIZE, ShelterRenderer, VillageCell};
use formiga_core::*;
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use time::OffsetDateTime;

const SCALE: u32 = 2;
/// How far apart the tree, the colony house and the cottage stand within one village, in
/// shelter pixels.
const STEP: u32 = 40;
const VILLAGE_WIDTH: u32 = SHELTER_SIZE + STEP * 2;
const GAP: u32 = 8;

pub fn run(path: PathBuf) -> Result<()> {
    let palettes: Vec<Option<VillagePalette>> = std::iter::once(None)
        .chain(VillagePalette::ALL.map(Some))
        .collect();
    let width = (GAP + (VILLAGE_WIDTH + GAP) * palettes.len() as u32) * SCALE;
    let row_height = SHELTER_SIZE + GAP;
    let height = (GAP + row_height * 4) * SCALE;
    let mut pixels = vec![0_u8; (width * height * 4) as usize];
    fill_gradient(
        &mut pixels,
        width,
        height,
        [237, 234, 224, 255],
        [209, 226, 219, 255],
    );
    for style in 0..4_u32 {
        let mut seed = [0_u8; 32];
        seed.copy_from_slice(&Sha256::digest(format!("formiga-shelter-{style}")));
        seed[1] = style as u8;
        let mut home = ColonyHome::from_seed(seed, None, None, None);
        let resident = World::preview_adult(
            SeedStream::new(seed).bytes("palette-sheet-resident", 0),
            OffsetDateTime::UNIX_EPOCH,
            &fixture_desktop(),
        );
        let marks = [None, Some(ResidentMark::of(&resident))];
        for (column, palette) in palettes.iter().enumerate() {
            home.palette = *palette;
            let village =
                ShelterRenderer::render_village(&home.drawn_shelter(), &[], &marks, &[], false);
            let left = GAP + column as u32 * (VILLAGE_WIDTH + GAP);
            let top = GAP + style * row_height;
            for (offset, cell) in [
                (0, VillageCell::Tree),
                (
                    STEP,
                    VillageCell::House {
                        slot: 0,
                        lit: false,
                        occupied: false,
                    },
                ),
                (
                    STEP * 2,
                    VillageCell::House {
                        slot: 1,
                        lit: false,
                        occupied: false,
                    },
                ),
            ] {
                blit_scaled_square_alpha(
                    &mut pixels,
                    width,
                    (left + offset) * SCALE,
                    top * SCALE,
                    &cut(&village, cell).rgba_bytes(),
                    SHELTER_SIZE,
                    SCALE,
                );
            }
        }
    }
    write_png(&path, width, height, &pixels)?;
    println!("wrote {}", path.display());
    Ok(())
}

/// One cell of the village atlas as its own square.
fn cut(village: &Canvas, cell: VillageCell) -> Canvas {
    let (x, y) = ShelterRenderer::village_cell(cell);
    let mut tile = Canvas::new(SHELTER_SIZE, SHELTER_SIZE);
    for ty in 0..SHELTER_SIZE as i32 {
        for tx in 0..SHELTER_SIZE as i32 {
            tile.set(tx, ty, village.get(x as i32 + tx, y as i32 + ty));
        }
    }
    tile
}
