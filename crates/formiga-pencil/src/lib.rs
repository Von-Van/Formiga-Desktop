//! The pencil hand every Formiga notebook is written in: Kalam, drawn onto a pixel grid with no
//! anti-aliasing, so what the researcher or the Handler writes reads as pixel-art pencil, the same
//! family as the notebooks' stepped edges and painted pictures. Printed things (brass plates, tab
//! numbers, codes) stay in egui's monospace.
//!
//! egui lays the hand out like any other font, so wrapping, measuring, hit-testing and screen
//! readers are egui's own. Only how its glyphs are drawn changes: [`Pencil`], an egui plugin, finds
//! each piece of text set in the hand in the frame's output and draws its glyphs from an atlas of
//! its own, rasterised once per size with a hard threshold and sampled nearest-neighbour. Writing
//! under 20 points is drawn a point to a texel; headings two points to a texel, as a sprite is
//! drawn at twice its size. Tilted writing is always drawn the finer way.
//!
//! Install it once per context with [`install`], then write with [`hand`] or [`hand_bold`], or
//! everywhere at once by installing it with [`Everywhere::Yes`].

mod atlas;
mod plugin;

use egui::{FontData, FontDefinitions, FontFamily, FontId};
use std::sync::Arc;

pub use plugin::Pencil;

/// The family the hand is written in, and its bold.
pub const HAND: &str = "hand";
pub const HAND_BOLD: &str = "hand-bold";

/// Kalam, by the Indian Type Foundry, under the SIL Open Font License (`fonts/OFL.txt`).
pub(crate) static REGULAR: &[u8] = include_bytes!("../fonts/Kalam-Regular.ttf");
pub(crate) static BOLD: &[u8] = include_bytes!("../fonts/Kalam-Bold.ttf");

/// Writing at or above this size, in points, is a heading, drawn two points to a texel, unless
/// [`set_heading_size`] says otherwise.
pub const HEADING_SIZE: f32 = 20.0;

fn heading_id() -> egui::Id {
    egui::Id::new("formiga-pencil-heading-size")
}

/// Writing at or above `size` points is drawn as a heading from now on: for a window whose text
/// is scaled up, so its body text is not mistaken for headings.
pub fn set_heading_size(ctx: &egui::Context, size: f32) {
    ctx.data_mut(|data| data.insert_temp(heading_id(), size));
}

pub(crate) fn heading_size(ctx: &egui::Context) -> f32 {
    ctx.data(|data| data.get_temp(heading_id()))
        .unwrap_or(HEADING_SIZE)
}

/// The hand at `size` points.
pub fn hand(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(HAND.into()))
}

/// The hand's bold at `size` points.
pub fn hand_bold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(HAND_BOLD.into()))
}

/// Whether ordinary labels are written in the hand too, or only text set in it on purpose.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Everywhere {
    /// The proportional family begins with the hand, so every label, button and field is
    /// written in it unless it asks for monospace.
    Yes,
    /// Only text set in [`hand`] or [`hand_bold`].
    No,
}

/// Adds the hand to `fonts`: its two families, each falling back to the proportional fonts for
/// anything Kalam has no glyph for, and, with [`Everywhere::Yes`], the proportional family too.
pub fn add_to(fonts: &mut FontDefinitions, everywhere: Everywhere) {
    fonts
        .font_data
        .insert("Kalam".into(), Arc::new(FontData::from_static(REGULAR)));
    fonts
        .font_data
        .insert("Kalam-Bold".into(), Arc::new(FontData::from_static(BOLD)));
    let fallback = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    for (family, face) in [(HAND, "Kalam"), (HAND_BOLD, "Kalam-Bold")] {
        let mut faces = vec![face.to_owned()];
        faces.extend(fallback.iter().cloned());
        fonts
            .families
            .insert(FontFamily::Name(family.into()), faces);
    }
    if everywhere == Everywhere::Yes {
        fonts
            .families
            .entry(FontFamily::Proportional)
            .or_default()
            .insert(0, "Kalam".into());
    }
}

/// Sets `ctx`'s fonts to egui's own with the hand added, and draws the hand in pencil from now on.
/// Call once, before the first frame.
pub fn install(ctx: &egui::Context, everywhere: Everywhere) {
    let mut fonts = FontDefinitions::default();
    add_to(&mut fonts, everywhere);
    ctx.set_fonts(fonts);
    ctx.add_plugin(Pencil::new(everywhere));
}
