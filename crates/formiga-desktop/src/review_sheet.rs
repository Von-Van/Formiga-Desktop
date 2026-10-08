//! What the test-only review sheets share: laying a translucent pixel over the sheet, and writing
//! the sheet out when the person running the test asked for it.

use formiga_art::{Canvas, Rgba};

/// Source-over, because `Canvas::set` replaces rather than blends: a soft outline or a bubble's
/// translucent paper has to read against the sheet the way it reads against a desktop.
pub(crate) fn over(source: Rgba, under: Rgba) -> Rgba {
    let alpha = u32::from(source.a);
    let mix = |s: u8, u: u8| ((u32::from(s) * alpha + u32::from(u) * (255 - alpha)) / 255) as u8;
    Rgba::new(
        mix(source.r, under.r),
        mix(source.g, under.g),
        mix(source.b, under.b),
        under.a.max(source.a),
    )
}

/// Write `sheet` as `filename` into the directory the environment variable `directory_variable`
/// names. Without the variable nothing is written.
pub(crate) fn save(directory_variable: &str, filename: &str, sheet: &Canvas) {
    if let Some(directory) = std::env::var_os(directory_variable) {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let file = std::fs::File::create(directory.join(filename)).unwrap();
        formiga_art::write_png(file, sheet.width(), sheet.height(), &sheet.rgba_bytes()).unwrap();
    }
}
