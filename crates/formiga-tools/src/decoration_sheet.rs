//! Every house decoration on its own, so each can be judged at the size it is seen: one column per
//! decoration, grouped by the place on a house it goes, and one row per house type, each worn by a
//! companion's cottage — the narrowest house, where a decoration has least room. Below them, the
//! four that light up, lit after dark.
use anyhow::Result;
use formiga_art::{SHELTER_SIZE, ShelterRenderer, VillageCell};
use formiga_core::{ColonyHome, DecorationSlot, ShelterDecorationKind, ShelterStyle};
use std::path::PathBuf;

const SCALE: u32 = 2;
const GAP: u32 = 6;
/// Ten to a row: the thirty decorations are three blocks of ten, by the slot they go in.
const COLUMNS: u32 = 10;

/// The decorations that glow after dark.
const LIT: [ShelterDecorationKind; 4] = [
    ShelterDecorationKind::Lamp,
    ShelterDecorationKind::FairyLights,
    ShelterDecorationKind::PaperLanterns,
    ShelterDecorationKind::Lantern,
];

/// One cottage of `style` wearing just `kind`, by day or lit after dark.
fn dressed_cottage(
    home: &ColonyHome,
    style: ShelterStyle,
    kind: ShelterDecorationKind,
    lit: bool,
) -> formiga_art::Canvas {
    let village = ShelterRenderer::render_village(
        &home.shelter,
        &[Vec::new(), vec![kind]],
        &[],
        &[home.shelter.style, style],
        lit,
    );
    let (x, y) = ShelterRenderer::village_cell(VillageCell::House {
        slot: 1,
        lit,
        occupied: false,
    });
    let mut tile = formiga_art::Canvas::new(SHELTER_SIZE, SHELTER_SIZE);
    for ty in 0..SHELTER_SIZE as i32 {
        for tx in 0..SHELTER_SIZE as i32 {
            tile.set(tx, ty, village.get(x as i32 + tx, y as i32 + ty));
        }
    }
    tile
}

pub fn run(path: PathBuf) -> Result<()> {
    // In slot order, so each block of the sheet is one place on the house.
    let mut kinds: Vec<ShelterDecorationKind> = Vec::new();
    for place in DecorationSlot::ALL {
        kinds.extend(
            ShelterDecorationKind::ALL
                .into_iter()
                .filter(|kind| kind.slot() == place),
        );
    }
    let home = ColonyHome::from_seed([42; 32], None, None, None);
    let cell = SHELTER_SIZE * SCALE + GAP;
    let blocks = (kinds.len() as u32).div_ceil(COLUMNS);
    let styles = ShelterStyle::ALL.len() as u32;
    // Every decoration on every type, then the lit ones on every type after dark.
    let rows = blocks * styles + styles;
    let width = COLUMNS * cell + GAP;
    let height = rows * cell + GAP * (blocks + 2);
    let mut pixels = vec![0_u8; (width * height * 4) as usize];
    crate::fill_gradient(
        &mut pixels,
        width,
        height,
        [226, 230, 220, 255],
        [196, 210, 202, 255],
    );
    let mut y = GAP;
    for block in 0..blocks {
        for style in ShelterStyle::ALL {
            for column in 0..COLUMNS {
                let Some(kind) = kinds.get((block * COLUMNS + column) as usize) else {
                    continue;
                };
                let tile = dressed_cottage(&home, style, *kind, false);
                crate::blit_canvas_scaled(&mut pixels, width, GAP + column * cell, y, &tile, SCALE);
            }
            y += cell;
        }
        y += GAP;
    }
    // After dark, on a night sky, so the glow can be seen.
    crate::fill_rect(
        &mut pixels,
        width,
        0,
        y - GAP / 2,
        width,
        styles * cell + GAP,
        [30, 36, 60, 255],
    );
    for style in ShelterStyle::ALL {
        for (column, kind) in LIT.into_iter().enumerate() {
            let tile = dressed_cottage(&home, style, kind, true);
            crate::blit_canvas_scaled(
                &mut pixels,
                width,
                GAP + column as u32 * cell,
                y,
                &tile,
                SCALE,
            );
        }
        y += cell;
    }
    crate::write_png(&path, width, height, &pixels)?;
    println!("wrote {}", path.display());
    Ok(())
}
