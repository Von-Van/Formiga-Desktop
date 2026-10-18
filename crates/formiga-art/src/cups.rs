//! Formiga Battle's keepsakes, a trophy and a medal for each of its four cups, drawn the way
//! Formiga Home draws them on its shelves.
//!
//! A port of Formiga Home's own pictures, pixel for pixel and ramp for ramp, so a cup looks the
//! same in the journal as it does in the house. Every trophy is the same little brass cup, stood on
//! what its cup's arena has to hand: a wooden plinth from the Backyard, a mossy stump from Woodland
//! Glade, a millstone from the Millpond and a piece of driftwood from Sandy Cove. Every medal is
//! the same brass disc, hung on its cup's ribbon: green, russet, blue and sandy yellow, as
//! `BattleKeepsake::description` says. A change to any picture here is a change Formiga Home
//! should hear about, and the other way round.

use crate::paint::{self, Ramp, mix, rgb};
use crate::{Canvas, Rgba};
use formiga_core::BattleKeepsake;

/// The width and height of a keepsake shown on its own: a stretch of the shelf it stands on or
/// hangs above, with room round the tallest of the pictures.
pub const CUP_TILE: u32 = 18;

/// Formiga Home's ramps for what the keepsakes are made of.
const BRASS: Ramp = Ramp::new(0x6a4e20, 0x9c7a36, 0xc7a14d, 0xe2c477, 0xf6e3a8);
const OAK: Ramp = Ramp::new(0x6b3f24, 0x94603a, 0xb67d4c, 0xd29e69, 0xe9c18f);
const BARK: Ramp = Ramp::new(0x36261b, 0x553b2a, 0x70503a, 0x8b6a50, 0xa88a6c);
const MOSS: Ramp = Ramp::new(0x334d24, 0x4a6b31, 0x62873e, 0x7fa352, 0xa6c47a);
const DRIFTWOOD: Ramp = Ramp::new(0x5e5a52, 0x8c867a, 0xb1aa9b, 0xcfc8b9, 0xe8e3d6);
const FERN: Ramp = Ramp::new(0x2c5530, 0x3f7a42, 0x569a52, 0x78b86a, 0xa5d48e);
const CLAY: Ramp = Ramp::new(0x763823, 0xa9563a, 0xc9744f, 0xe39672, 0xf2b997);
const CORNFLOWER: Ramp = Ramp::new(0x46597f, 0x6a80a8, 0x8aa3c9, 0xaec2e0, 0xd0def0);
const WICKER: Ramp = Ramp::new(0x775428, 0xa77d42, 0xc9a063, 0xe0bf86, 0xf0d9a8);

/// The back of the cabinet the keepsakes are kept in, from its top to the shelf: a deep green
/// baize, which every one of them reads against.
const BAIZE: [u32; 2] = [0x24493c, 0x122a21];
/// The shelf itself, its lit edge and its front.
const SHELF: [u32; 2] = [0x8a6145, 0x4a2e1e];
/// The row the shelf's lit edge is on: a trophy stands on it.
const SHELF_TOP: i32 = CUP_TILE as i32 - 2;

/// `keepsake`'s picture: 11 by 13 pixels for a trophy, 9 by 13 for a medal.
pub fn battle_keepsake_picture(keepsake: BattleKeepsake) -> Canvas {
    use BattleKeepsake::*;
    match keepsake {
        BackyardCupTrophy => trophy(Stand::Plinth),
        WoodlandCupTrophy => trophy(Stand::Stump),
        MillpondCupTrophy => trophy(Stand::Millstone),
        CoveCupTrophy => trophy(Stand::Driftwood),
        BackyardCupMedal => medal(FERN),
        WoodlandCupMedal => medal(CLAY),
        MillpondCupMedal => medal(CORNFLOWER),
        CoveCupMedal => medal(WICKER),
    }
}

/// Whether `keepsake` is a trophy, which stands, rather than a medal, which hangs.
const fn stands(keepsake: BattleKeepsake) -> bool {
    use BattleKeepsake::*;
    matches!(
        keepsake,
        BackyardCupTrophy | WoodlandCupTrophy | MillpondCupTrophy | CoveCupTrophy
    )
}

/// Every keepsake side by side in [`BattleKeepsake::ALL`]'s order, one tile wide each, each kept on
/// its own stretch of a cabinet's shelf: a trophy stood on the shelf, a medal hung by its ribbon
/// from a brass rail above it.
pub fn cup_strip() -> Canvas {
    let mut strip = Canvas::new(CUP_TILE * BattleKeepsake::ALL.len() as u32, CUP_TILE);
    for (index, keepsake) in BattleKeepsake::ALL.into_iter().enumerate() {
        let left = (index as u32 * CUP_TILE) as i32;
        for y in 0..CUP_TILE as i32 {
            let color = match y - SHELF_TOP {
                0 => rgb(SHELF[0]),
                1 => rgb(SHELF[1]),
                // The baize: lighter above, darker toward the shelf.
                _ => mix(
                    rgb(BAIZE[0]),
                    rgb(BAIZE[1]),
                    y as f32 / (SHELF_TOP - 1) as f32,
                ),
            };
            paint::hline(&mut strip, left, y, CUP_TILE as i32, color);
        }
        let picture = battle_keepsake_picture(keepsake);
        let (width, height) = (picture.width() as i32, picture.height() as i32);
        let x = left + (CUP_TILE as i32 - width) / 2;
        if stands(keepsake) {
            paint::blit(&mut strip, &picture, x, SHELF_TOP - height);
        } else {
            // The rail, a pixel above the ribbon's top, the width of the ribbon.
            paint::hline(&mut strip, x, 1, width, BRASS.shadow);
            paint::put(&mut strip, x + 1, 1, BRASS.light);
            paint::blit(&mut strip, &picture, x, 2);
        }
    }
    strip
}

/// What a trophy stands on.
#[derive(Clone, Copy)]
enum Stand {
    Plinth,
    Stump,
    Millstone,
    Driftwood,
}

/// A little brass cup with two handles on a short stem, on its stand.
fn trophy(stand: Stand) -> Canvas {
    let mut canvas = Canvas::new(11, 13);
    // The bowl, widest at the rim, lit from the upper left.
    for (y, (x0, width)) in [(2, 7), (2, 7), (3, 5), (3, 5), (4, 3)]
        .into_iter()
        .enumerate()
    {
        let y = y as i32 + 1;
        paint::hline(&mut canvas, x0, y, width, BRASS.base);
        paint::put(&mut canvas, x0, y, BRASS.light);
        paint::put(&mut canvas, x0 + width - 1, y, BRASS.shadow);
    }
    paint::hline(&mut canvas, 2, 0, 7, BRASS.light);
    paint::put(&mut canvas, 3, 2, BRASS.shine);
    // The handles.
    for x in [0, 10] {
        paint::vline(&mut canvas, x, 1, 2, BRASS.shadow);
    }
    paint::put(&mut canvas, 1, 3, BRASS.shadow);
    paint::put(&mut canvas, 9, 3, BRASS.shadow);
    // The stem and its foot.
    paint::vline(&mut canvas, 5, 6, 2, BRASS.shadow);
    paint::hline(&mut canvas, 4, 8, 3, BRASS.base);
    paint::put(&mut canvas, 4, 8, BRASS.light);
    match stand {
        Stand::Plinth => {
            paint::rect(&mut canvas, 2, 9, 7, 4, OAK.base);
            paint::hline(&mut canvas, 2, 9, 7, OAK.light);
            paint::vline(&mut canvas, 8, 10, 3, OAK.shadow);
            paint::hline(&mut canvas, 4, 11, 3, BRASS.light);
        }
        Stand::Stump => {
            paint::rect(&mut canvas, 2, 10, 7, 3, BARK.base);
            paint::vline(&mut canvas, 4, 10, 3, BARK.shadow);
            paint::vline(&mut canvas, 7, 11, 2, BARK.shadow);
            paint::hline(&mut canvas, 2, 9, 7, MOSS.base);
            paint::put(&mut canvas, 3, 9, MOSS.light);
            paint::put(&mut canvas, 1, 12, MOSS.shadow);
            paint::put(&mut canvas, 9, 12, MOSS.shadow);
        }
        Stand::Millstone => {
            // Millstone grit: the driftwood's grey, a good deal darker.
            let stone = |color| paint::darker(color, 0.25);
            paint::rect(&mut canvas, 1, 9, 9, 4, stone(DRIFTWOOD.base));
            paint::hline(&mut canvas, 1, 9, 9, stone(DRIFTWOOD.light));
            paint::hline(&mut canvas, 1, 12, 9, stone(DRIFTWOOD.shadow));
            // The grooves of its dressing.
            for x in [3, 7] {
                paint::put(&mut canvas, x, 10, stone(DRIFTWOOD.shadow));
                paint::put(&mut canvas, x - 1, 11, stone(DRIFTWOOD.shadow));
            }
        }
        Stand::Driftwood => {
            paint::hline(&mut canvas, 1, 9, 8, DRIFTWOOD.light);
            paint::rect(&mut canvas, 0, 10, 11, 2, DRIFTWOOD.base);
            paint::hline(&mut canvas, 2, 12, 8, DRIFTWOOD.shadow);
            paint::put(&mut canvas, 3, 10, DRIFTWOOD.shine);
            paint::put(&mut canvas, 8, 11, DRIFTWOOD.shadow);
        }
    }
    edged(&mut canvas);
    canvas
}

/// A brass disc on a ribbon of its cup's colour, the ribbon's two halves crossing above it.
fn medal(ribbon: Ramp) -> Canvas {
    let mut canvas = Canvas::new(9, 13);
    for y in 0..6 {
        let step = y / 2;
        paint::put(&mut canvas, 1 + step, y, ribbon.light);
        paint::put(&mut canvas, 2 + step, y, ribbon.base);
        paint::put(&mut canvas, 6 - step, y, ribbon.base);
        paint::put(&mut canvas, 7 - step, y, ribbon.shadow);
    }
    paint::ellipse(&mut canvas, 4, 9, 3, 3, BRASS.base);
    for (x, y) in [(3, 7), (2, 7), (2, 8), (1, 9)] {
        paint::put(&mut canvas, x, y, BRASS.light);
    }
    for (x, y) in [(7, 9), (6, 10), (6, 11), (5, 11), (4, 12)] {
        paint::put(&mut canvas, x, y, BRASS.shadow);
    }
    paint::put(&mut canvas, 3, 8, BRASS.shine);
    paint::put(&mut canvas, 4, 9, BRASS.light);
    edged(&mut canvas);
    canvas
}

/// Every edge a shade darker, as Formiga Home's own keepsakes are.
fn edged(canvas: &mut Canvas) {
    paint::outline_inside(canvas, |color: Rgba| {
        if color.a < 200 {
            color
        } else {
            paint::darker(color, 0.6)
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_keepsake_has_a_picture_of_its_own() {
        let pictures: Vec<Canvas> = BattleKeepsake::ALL
            .into_iter()
            .map(battle_keepsake_picture)
            .collect();
        for (index, keepsake) in BattleKeepsake::ALL.into_iter().enumerate() {
            let canvas = &pictures[index];
            let drawn = canvas.pixels().iter().filter(|pixel| pixel.a > 0).count();
            assert!(drawn >= 40, "{keepsake:?} is barely there");
            assert!(
                canvas.width() < CUP_TILE && canvas.height() < CUP_TILE - 3,
                "{keepsake:?} fits its stretch of the shelf"
            );
            assert!(
                !pictures[..index].contains(canvas),
                "{keepsake:?} looks like another"
            );
        }
    }

    /// The pictures as Formiga Home 0.67.14 draws them (`art::battle`, the same code over the same
    /// ramps and paint), by a digest of their pixels. Changing one means
    /// the two apps no longer show the same keepsake: change both, together.
    #[test]
    fn every_keepsake_is_drawn_as_formiga_home_draws_it() {
        let digest = |canvas: &Canvas| {
            canvas
                .rgba_bytes()
                .iter()
                .fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
                    (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
                })
        };
        let digests: Vec<_> = BattleKeepsake::ALL
            .map(|keepsake| (keepsake, digest(&battle_keepsake_picture(keepsake))))
            .to_vec();
        assert_eq!(digests, AS_FORMIGA_HOME_DRAWS_THEM.to_vec());
    }

    #[test]
    fn a_trophy_stands_on_the_shelf_and_a_medal_hangs_above_it() {
        let strip = cup_strip();
        assert_eq!(
            (strip.width(), strip.height()),
            (CUP_TILE * BattleKeepsake::ALL.len() as u32, CUP_TILE)
        );
        assert!(
            strip.pixels().iter().all(|pixel| pixel.a == 255),
            "every tile is cabinet where the picture is clear"
        );
        for (index, keepsake) in BattleKeepsake::ALL.into_iter().enumerate() {
            let picture = battle_keepsake_picture(keepsake);
            let (width, height) = (picture.width() as i32, picture.height() as i32);
            let left = index as i32 * CUP_TILE as i32 + (CUP_TILE as i32 - width) / 2;
            let top = if stands(keepsake) {
                SHELF_TOP - height
            } else {
                2
            };
            for y in 0..height {
                for x in 0..width {
                    let drawn = picture.get(x, y);
                    if drawn.a > 0 {
                        assert_eq!(strip.get(left + x, top + y), drawn, "{keepsake:?}");
                    }
                }
            }
            let lowest = (0..height)
                .rev()
                .find(|y| (0..width).any(|x| picture.get(x, *y).a > 0))
                .unwrap();
            if stands(keepsake) {
                assert_eq!(top + lowest + 1, SHELF_TOP, "{keepsake:?} stands on it");
            } else {
                assert!(top + lowest + 1 < SHELF_TOP, "{keepsake:?} hangs clear");
                let rail = strip.get(left + width / 2, 1);
                assert!(rail.r > rail.b, "{keepsake:?} hangs from the rail");
            }
            // The shelf runs the whole width of every tile.
            let shelf = strip.get(index as i32 * CUP_TILE as i32, SHELF_TOP);
            assert_eq!(shelf, rgb(SHELF[0]));
        }
    }

    const AS_FORMIGA_HOME_DRAWS_THEM: [(BattleKeepsake, u64); 8] = [
        (BattleKeepsake::BackyardCupTrophy, 1_261_485_052_478_463_201),
        (BattleKeepsake::BackyardCupMedal, 6_908_729_493_675_662_633),
        (BattleKeepsake::WoodlandCupTrophy, 2_458_070_615_304_024_362),
        (BattleKeepsake::WoodlandCupMedal, 15_794_219_028_949_516_138),
        (BattleKeepsake::MillpondCupTrophy, 395_804_718_017_853_621),
        (BattleKeepsake::MillpondCupMedal, 6_177_061_029_006_315_589),
        (BattleKeepsake::CoveCupTrophy, 6_167_753_583_130_946_011),
        (BattleKeepsake::CoveCupMedal, 16_268_790_334_508_205_035),
    ];
}
