//! Formiga Hill's souvenirs, drawn the way Formiga Hill draws them in its station's display case.
//!
//! A port of Formiga Hill's own pictures, row for row and ink for ink, so a souvenir looks the same
//! in the journal as it did at the Hill. Each is seven pixels square in that souvenir's own fixed
//! colours: a souvenir is one particular thing, not something drawn in a colony's inks. A change
//! to any picture here is a change Formiga Hill should hear about, and the other way round.

use crate::paint::mix;
use crate::{Canvas, Rgba};
use formiga_core::Souvenir;

/// The width and height of every souvenir's picture.
pub const SOUVENIR_ICON: u32 = 7;

/// The width and height of a souvenir shown on its own: its picture with a pixel of velvet all
/// round.
pub const SOUVENIR_TILE: u32 = SOUVENIR_ICON + 2;

/// The velvet lining Formiga Hill's display case, from the top of the case to the foot of it.
const VELVET: [u32; 2] = [0x7a2c3a, 0x3e1620];

/// Draws `souvenir` with its top-left corner at `(x, y)`.
pub fn draw_souvenir(canvas: &mut Canvas, souvenir: Souvenir, x: i32, y: i32) {
    let (rows, inks) = picture(souvenir);
    for (dy, row) in rows.iter().enumerate() {
        for (dx, code) in row.bytes().enumerate() {
            let ink = match code {
                b'#' => inks[0],
                b'o' => inks[1],
                b'*' => inks[2],
                b'x' => inks[3],
                _ => continue,
            };
            canvas.set(x + dx as i32, y + dy as i32, rgb(ink));
        }
    }
}

/// Every souvenir side by side in [`Souvenir::ALL`]'s order, one tile wide each, each set on the
/// velvet that lines Formiga Hill's display case. The pictures were made to be seen against it,
/// so a pale one still reads on a pale page.
pub fn souvenir_strip() -> Canvas {
    let mut strip = Canvas::new(SOUVENIR_TILE * Souvenir::ALL.len() as u32, SOUVENIR_TILE);
    for (index, souvenir) in Souvenir::ALL.into_iter().enumerate() {
        let left = (index as u32 * SOUVENIR_TILE) as i32;
        for y in 0..SOUVENIR_TILE as i32 {
            // The stretch of the case a souvenir sits in: lighter above, darker below.
            let along = 0.1 + 0.6 * y as f32 / (SOUVENIR_TILE - 1) as f32;
            let velvet = mix(rgb(VELVET[0]), rgb(VELVET[1]), along);
            for x in 0..SOUVENIR_TILE as i32 {
                strip.set(left + x, y, velvet);
            }
        }
        draw_souvenir(&mut strip, souvenir, left + 1, 1);
    }
    strip
}

const fn rgb(hex: u32) -> Rgba {
    Rgba::new((hex >> 16) as u8, (hex >> 8) as u8, hex as u8, 255)
}

/// One souvenir's rows, `#` `o` `*` `x` naming its four inks and `.` left clear.
fn picture(souvenir: Souvenir) -> ([&'static str; 7], [u32; 4]) {
    match souvenir {
        // A bow of red gingham.
        Souvenir::PicnicRibbon => (
            [
                ".......", "##...##", "#o#.#o#", "#oo#oo#", "#o#.#o#", "##.#.##", "..#.#..",
            ],
            [0x6e2a2a, 0xd0574a, 0xf3e9cf, 0xffffff],
        ),
        // A daisy, pressed flat.
        Souvenir::PressedDaisy => (
            [
                "...o...", ".o.o.o.", "..ooo..", "oo*#*oo", "..ooo..", ".o.o.o.", "...o...",
            ],
            [0xe0a82a, 0xfbf6ee, 0xf5d25e, 0xffffff],
        ),
        // A copper penny, catching the light.
        Souvenir::WellPenny => (
            [
                ".......", "..###..", ".#ooo#.", ".#o*o#.", ".#ooo#.", "..###..", ".......",
            ],
            [0x7a3e1e, 0xc0743a, 0xf2b07a, 0xffffff],
        ),
        // An acorn in its cap.
        Souvenir::OakAcorn => (
            [
                "...#...", "..###..", ".#####.", ".ooooo.", ".o*ooo.", "..ooo..", "...o...",
            ],
            [0x5a4232, 0x9a6a38, 0xd2a060, 0xffffff],
        ),
        // A soft grey feather.
        Souvenir::SwingFeather => (
            [
                ".....##", "....#o#", "...#o*.", "..#oo..", ".#oo...", "#o.....", "#......",
            ],
            [0x7a8590, 0xd8dde2, 0xffffff, 0xffffff],
        ),
        // A glass marble with an amber twist through it.
        Souvenir::ChestMarble => (
            [
                "..###..", ".#*oo#.", "#*oxxo#", "#oxooo#", "#ooxxo#", ".#ooo#.", "..###..",
            ],
            [0x2c5a86, 0x5c9bd2, 0xe6f4ff, 0xf0b44c],
        ),
        // A ticket stub, torn along its perforations.
        Souvenir::FairTicket => (
            [
                ".......", "#######", "#o*o*o#", "#ooooo#", "#o*o*o#", "#######", "x.x.x.x",
            ],
            [0x8a2a30, 0xf3d8a0, 0xd0584c, 0xc9b080],
        ),
        // A rosette of pleated blue ribbon round a gilt button, its two tails hanging below.
        Souvenir::RaceRosette => (
            [
                ".#o#o#.", "#o*xxo#", "ooxxxoo", "#oxxxo#", ".#o#o#.", ".#o.o#.", ".#...#.",
            ],
            [0x1e2c58, 0x5a7ec4, 0xfff2bc, 0xe4c06c],
        ),
        // A little brass bell, lit from the left, its clapper showing under its rim.
        Souvenir::StrikerBell => (
            [
                "...#...", "..#*#..", ".#*ox#.", ".#*ox#.", "#*ooox#", "#######", "...x...",
            ],
            [0x6b4a24, 0xd8b058, 0xfff0b8, 0x9a7434],
        ),
        // A little teddy's face from the hoopla stall's shelf: round ears, bright eyes, and a
        // pale muzzle with its nose in the middle.
        Souvenir::HooplaTeddy => (
            [
                "##...##", "#x###x#", "#ooooo#", "#o*o*o#", "#ox#xo#", ".#xxx#.", "..###..",
            ],
            [0x5a3420, 0xb07a4a, 0x2e1a12, 0xe8c494],
        ),
        // A length of the tug-of-war's rope, its twist catching the light and its ends frayed,
        // with the red ribbon from its middle still tied on.
        Souvenir::TugRope => (
            [
                "#*.....", "*o*....", ".*o*.x.", "..xox..", ".x.*o*.", "....*o*", ".....*#",
            ],
            [0x6a5030, 0xb8955a, 0xe2c58c, 0xc8303e],
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pixels_of(souvenir: Souvenir) -> Canvas {
        let mut canvas = Canvas::new(SOUVENIR_ICON, SOUVENIR_ICON);
        draw_souvenir(&mut canvas, souvenir, 0, 0);
        canvas
    }

    #[test]
    fn every_souvenir_has_a_picture_of_its_own() {
        let mut seen = Vec::new();
        for souvenir in Souvenir::ALL {
            let canvas = pixels_of(souvenir);
            let drawn = canvas.pixels().iter().filter(|pixel| pixel.a > 0).count();
            assert!(drawn >= 12, "{souvenir:?} is barely there");
            assert!(
                canvas
                    .pixels()
                    .iter()
                    .all(|pixel| pixel.a == 0 || pixel.a == 255)
            );
            assert!(!seen.contains(&canvas), "{souvenir:?} looks like another");
            seen.push(canvas);
        }
    }

    /// The pictures as Formiga Hill draws them, by a digest of their pixels. Changing one means
    /// the two apps no longer show the same souvenir: change both, together.
    #[test]
    fn every_souvenir_is_drawn_as_formiga_hill_draws_it() {
        let digest = |canvas: &Canvas| {
            canvas
                .rgba_bytes()
                .iter()
                .fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
                    (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
                })
        };
        let digests: Vec<_> = Souvenir::ALL
            .map(|souvenir| (souvenir, digest(&pixels_of(souvenir))))
            .to_vec();
        assert_eq!(digests, AS_FORMIGA_HILL_DRAWS_THEM.to_vec());
    }

    #[test]
    fn the_strip_sets_each_picture_on_velvet_where_its_place_in_the_list_says() {
        let strip = souvenir_strip();
        assert_eq!(
            (strip.width(), strip.height()),
            (SOUVENIR_TILE * Souvenir::ALL.len() as u32, SOUVENIR_TILE)
        );
        assert!(
            strip.pixels().iter().all(|pixel| pixel.a == 255),
            "every tile is velvet where the picture is clear"
        );
        for (index, souvenir) in Souvenir::ALL.into_iter().enumerate() {
            let alone = pixels_of(souvenir);
            let left = index as i32 * SOUVENIR_TILE as i32;
            for y in 0..SOUVENIR_ICON as i32 {
                for x in 0..SOUVENIR_ICON as i32 {
                    let drawn = alone.get(x, y);
                    if drawn.a > 0 {
                        let at = strip.get(left + 1 + x, 1 + y);
                        assert_eq!(at, drawn, "{souvenir:?} at ({x}, {y})");
                    }
                }
            }
            // The velvet's own edge, which no picture reaches.
            for y in 0..SOUVENIR_TILE as i32 {
                let row = strip.get(left, y);
                assert!(row.r > row.g && row.r > row.b, "velvet at ({left}, {y})");
            }
        }
    }

    const AS_FORMIGA_HILL_DRAWS_THEM: [(Souvenir, u64); 11] = [
        (Souvenir::PicnicRibbon, 0xc5b5_90f5_5e52_7385),
        (Souvenir::PressedDaisy, 0x2246_823e_0c34_fe8e),
        (Souvenir::WellPenny, 0x6bfb_e90a_f30d_3fe8),
        (Souvenir::OakAcorn, 0x5aac_cdde_787a_4224),
        (Souvenir::SwingFeather, 0x91bc_2b2d_a35b_6255),
        (Souvenir::ChestMarble, 0x3e6e_accf_5294_5074),
        (Souvenir::FairTicket, 0xf5d5_ebbd_8dc4_e04b),
        (Souvenir::RaceRosette, 0xfdc9_d920_d187_2f01),
        (Souvenir::StrikerBell, 0x10b1_0400_cd72_6378),
        (Souvenir::HooplaTeddy, 0x2c30_80ab_8f48_f4f3),
        (Souvenir::TugRope, 0x3f1f_7c2b_e610_9a49),
    ];
}
