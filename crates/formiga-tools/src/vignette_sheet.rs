//! Every vignette the notebook tapes in beside a moment, drawn for review.
//!
//!   cargo run -p formiga-tools -- vignette-sheet [--output docs/assets/vignette-sheet.png]
//!
//! One row per cast of reference creatures, each starting with a different one, and along each
//! row every kind of vignette: an arrival, a friendship, a first find (with the find drawn over
//! the finder's paws, as the notebook draws it), five rituals standing for every pose a ritual
//! is drawn in, and a visitor at the houses. Drawn three screen pixels to an art pixel.

use crate::{reference_creatures, write_png};
use anyhow::Result;
use formiga_art::{
    Canvas, TRINKET_FRAME_REST, TrinketAtlasRenderer, VIGNETTE_HEIGHT, VIGNETTE_WIDTH, Vignette,
    render_vignette,
};
use formiga_core::AppearanceGenome;
use std::path::PathBuf;

const SCALE: u32 = 3;
const GAP: u32 = 4;

/// The find a review vignette holds up: the key, from the reference colony's sheet.
fn find(vignette: Vignette, picture: &mut Canvas, atlas: &Canvas) {
    let Some((left, top)) = vignette.find_spot() else {
        return;
    };
    let (x, y, width, height) = TrinketAtlasRenderer::cell_rect(1, TRINKET_FRAME_REST);
    for row in 0..height as i32 {
        for column in 0..width as i32 {
            let pixel = atlas.get(x as i32 + column, y as i32 + row);
            if pixel.a > 0 {
                picture.set(left + column, top + row, pixel);
            }
        }
    }
}

pub fn run(path: PathBuf) -> Result<()> {
    let creatures = reference_creatures();
    let genomes: Vec<AppearanceGenome> = creatures.iter().map(|c| c.appearance.clone()).collect();
    let palettes: Vec<_> = genomes.iter().map(formiga_art::palette_for).collect();
    let atlas = TrinketAtlasRenderer::render_resting([25; 32], &palettes);
    let rows = genomes.len().min(4);
    let (cell_w, cell_h) = (VIGNETTE_WIDTH * SCALE + GAP, VIGNETTE_HEIGHT * SCALE + GAP);
    let width = Vignette::ALL.len() as u32 * cell_w;
    let height = rows as u32 * cell_h;
    let mut pixels = vec![0_u8; (width * height * 4) as usize];
    for row in 0..rows {
        let cast: Vec<&AppearanceGenome> = (0..4)
            .map(|i| &genomes[(row + i) % genomes.len()])
            .collect();
        for (column, vignette) in Vignette::ALL.into_iter().enumerate() {
            let mut picture = render_vignette(vignette, &cast);
            find(vignette, &mut picture, &atlas);
            let (ox, oy) = (column as u32 * cell_w, row as u32 * cell_h);
            for y in 0..VIGNETTE_HEIGHT * SCALE {
                for x in 0..VIGNETTE_WIDTH * SCALE {
                    let pixel = picture.get((x / SCALE) as i32, (y / SCALE) as i32);
                    let at = (((oy + y) * width + ox + x) * 4) as usize;
                    pixels[at..at + 4].copy_from_slice(&[pixel.r, pixel.g, pixel.b, pixel.a]);
                }
            }
        }
    }
    write_png(&path, width, height, &pixels)?;
    println!("wrote {}", path.display());
    Ok(())
}
