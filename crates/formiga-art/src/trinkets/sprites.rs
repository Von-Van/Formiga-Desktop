//! Every find, drawn as a small grid of letters.
//!
//! Each grid is the 12x12 interior of a 16x16 cell, placed two pixels in from its top-left corner
//! so the outline pass has room for its edge. A sprite names its own inks: each is five letters,
//! one for each of the ink's tones darkest first — edge, shadow, base, light, shine — with `.`
//! where a tone goes unused. `k` is always the shared outline colour, for a line drawn inside a
//! shape, and `.` in the grid is empty. Only the interior is written: the outline is drawn round
//! it afterwards.
//!
//! What a find is made of decides its inks (see `materials`): an acorn is an acorn's brown in
//! every colony, and only what is dyed or painted is drawn in the colony's own colour.
//!
//! The top two rows keep to the middle four columns and the third row to the middle six, because
//! the corners of a held keepsake's cell land on its holder's eyes and are always cleared.

mod everyday;
mod found_out;
mod home_and_garden;
mod special;

use super::TrinketInk;
use super::materials::Ink;
use crate::Rgba;

/// One find's drawing and what it is drawn in.
pub(super) struct Sprite {
    pub(super) rows: [&'static str; 12],
    pub(super) inks: &'static [(&'static str, Ink)],
}

impl Sprite {
    /// The colour one letter paints, and which of its ink's five tones it is, or `None` for an
    /// empty pixel. A letter no ink names paints nothing.
    pub(super) fn paint(&self, letter: char, ink: TrinketInk) -> Option<(Rgba, usize)> {
        if letter == '.' {
            return None;
        }
        if letter == 'k' {
            return Some((ink.outline, 0));
        }
        self.inks.iter().find_map(|(letters, material)| {
            letters
                .chars()
                .position(|named| named == letter)
                .map(|tone| (material.tones(ink)[tone], tone))
        })
    }
}

/// The drawing for one variant.
pub(super) fn sprite(variant: u8) -> &'static Sprite {
    let index = usize::from(variant % formiga_core::TRINKET_VARIANTS);
    match index {
        0..40 => &everyday::SPRITES[index],
        40..80 => &found_out::SPRITES[index - 40],
        80..120 => &home_and_garden::SPRITES[index - 80],
        _ => &special::SPRITES[index - 120],
    }
}

/// Where the glint goes on a find: the first light or shining pixel far enough inside the cell for
/// its rays to stay in it, reading the grid top to bottom, or its first painted pixel there if it
/// has none.
pub(super) fn glint_spot(sprite: &Sprite, ink: TrinketInk) -> (i32, i32) {
    let mut first_opaque = None;
    for (row, line) in sprite.rows.iter().enumerate() {
        for (column, letter) in line.chars().enumerate() {
            let (x, y) = (column as i32 + 2, row as i32 + 2);
            let Some((_, tone)) = sprite.paint(letter, ink) else {
                continue;
            };
            if first_opaque.is_none() && (4..=11).contains(&x) && y >= 4 {
                first_opaque = Some((x, y));
            }
            if tone >= 3 && letter != 'k' && (4..=11).contains(&x) && (4..=11).contains(&y) {
                return (x, y);
            }
        }
    }
    first_opaque.unwrap_or((8, 8))
}
