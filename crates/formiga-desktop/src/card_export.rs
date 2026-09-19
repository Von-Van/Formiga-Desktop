use anyhow::{Context as _, Result};
use formiga_art::{
    CARD_HEIGHT, CARD_WIDTH, COLONY_CARD_HEIGHT, COLONY_CARD_WIDTH, ColonyCardRenderer,
    CreatureCardRenderer,
};
use formiga_core::{Creature, SaveFile};
use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};

pub fn choose_card_destination(creature: &Creature) -> Option<PathBuf> {
    rfd::FileDialog::new()
        .add_filter("PNG image", &["png"])
        .set_file_name(default_card_filename(&creature.name))
        .save_file()
}

/// The whole-colony portrait keeps one fixed name: it is a picture of the colony, not of any one
/// companion, and the colony's own name is not something Formiga holds.
const COLONY_CARD_FILENAME: &str = "Formiga-colony.png";

pub fn choose_colony_card_destination() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .add_filter("PNG image", &["png"])
        .set_file_name(COLONY_CARD_FILENAME)
        .save_file()
}

pub fn export_colony_card_to_selected_destination(
    save: &SaveFile,
    selected: Option<PathBuf>,
) -> Result<Option<PathBuf>> {
    let Some(path) = selected else {
        return Ok(None);
    };
    let path = with_png_extension(path);
    write_colony_card_png(save, &path)?;
    Ok(Some(path))
}

fn write_colony_card_png(save: &SaveFile, path: &Path) -> Result<()> {
    // Render only after a destination exists, exactly as the creature card does. The portrait
    // canvas, font atlas, and PNG buffers are scoped to this call.
    let card = ColonyCardRenderer::render(save);
    write_png(
        path,
        COLONY_CARD_WIDTH,
        COLONY_CARD_HEIGHT,
        &card.rgba_bytes(),
        "colony portrait",
    )
}

pub fn export_to_selected_destination(
    creature: &Creature,
    selected: Option<PathBuf>,
) -> Result<Option<PathBuf>> {
    let Some(path) = selected else {
        return Ok(None);
    };
    let path = with_png_extension(path);
    write_card_png(creature, &path)?;
    Ok(Some(path))
}

fn write_card_png(creature: &Creature, path: &Path) -> Result<()> {
    // Render only after a destination exists. The card canvas, font atlas, and PNG buffers are all
    // scoped to this call and are released immediately after the on-demand export.
    let card = CreatureCardRenderer::render(creature);
    write_png(
        path,
        CARD_WIDTH,
        CARD_HEIGHT,
        &card.rgba_bytes(),
        "creature card",
    )
}

/// Straight RGBA, eight bits, and no ancillary chunks at all: no text, no timestamp, no profile.
/// Both cards go out through here so neither can grow hidden metadata the other does not have.
fn write_png(path: &Path, width: u32, height: u32, pixels: &[u8], what: &str) -> Result<()> {
    let file =
        File::create(path).with_context(|| format!("create {what} at {}", path.display()))?;
    let mut encoder = png::Encoder::new(BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder
        .write_header()
        .with_context(|| format!("write {what} header"))?;
    writer
        .write_image_data(pixels)
        .with_context(|| format!("write {what} pixels"))?;
    writer.finish().with_context(|| format!("finish {what}"))?;
    Ok(())
}

fn default_card_filename(name: &str) -> String {
    let mut stem = String::new();
    let mut last_was_separator = false;
    for character in name.chars() {
        if character.is_alphanumeric() {
            stem.push(character);
            last_was_separator = false;
        } else if !last_was_separator && !stem.is_empty() {
            stem.push('-');
            last_was_separator = true;
        }
    }
    while stem.ends_with('-') {
        stem.pop();
    }
    if stem.is_empty() {
        stem.push_str("Formiga");
    }
    format!("{stem}-creature-card.png")
}

fn with_png_extension(mut path: PathBuf) -> PathBuf {
    if !path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("png"))
    {
        path.set_extension("png");
    }
    path
}

#[cfg(test)]
mod tests {
    use super::*;
    use formiga_core::{DesktopRect, DesktopSnapshot, DisplayKey, MonitorInfo, World};
    use time::macros::datetime;

    fn creature() -> Creature {
        let desktop = DesktopSnapshot {
            monitors: vec![MonitorInfo {
                id: 1,
                display_key: DisplayKey([1; 16]),
                bounds: DesktopRect {
                    x: 0.0,
                    y: 0.0,
                    width: 1280.0,
                    height: 800.0,
                },
                usable_bounds: DesktopRect {
                    x: 0.0,
                    y: 24.0,
                    width: 1280.0,
                    height: 776.0,
                },
                scale_factor: 1.0,
                primary: true,
            }],
            ..DesktopSnapshot::default()
        };
        World::new([11; 32], datetime!(2026-08-14 12:30 UTC), &desktop)
            .save
            .creatures
            .remove(0)
    }

    #[test]
    fn cancelled_save_does_not_render_or_create_anything() {
        assert_eq!(
            export_to_selected_destination(&creature(), None).unwrap(),
            None
        );
    }

    #[test]
    fn filenames_handle_unicode_and_path_punctuation() {
        assert_eq!(
            default_card_filename("Mochi 雪 / très doux"),
            "Mochi-雪-très-doux-creature-card.png"
        );
        assert_eq!(default_card_filename("///"), "Formiga-creature-card.png");
    }

    #[test]
    fn selection_is_normalized_to_png() {
        assert_eq!(
            with_png_extension(PathBuf::from("Mallow.card")),
            PathBuf::from("Mallow.png")
        );
        assert_eq!(
            with_png_extension(PathBuf::from("Mallow.PNG")),
            PathBuf::from("Mallow.PNG")
        );
    }

    #[test]
    fn written_png_has_fixed_dimensions_and_no_hidden_metadata() {
        let path = std::env::temp_dir().join(format!(
            "formiga-card-export-test-{}.png",
            std::process::id()
        ));
        write_card_png(&creature(), &path).unwrap();
        assert_no_hidden_metadata(&path, CARD_WIDTH, CARD_HEIGHT);
        std::fs::remove_file(path).unwrap();
    }

    fn assert_no_hidden_metadata(path: &Path, width: u32, height: u32) {
        let bytes = std::fs::read(path).unwrap();
        let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
        let mut reader = decoder.read_info().unwrap();
        let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
        reader.next_frame(&mut pixels).unwrap();
        let info = reader.info();
        assert_eq!((info.width, info.height), (width, height));
        assert_eq!(info.color_type, png::ColorType::Rgba);
        assert_eq!(info.bit_depth, png::BitDepth::Eight);
        assert!(info.uncompressed_latin1_text.is_empty());
        assert!(info.compressed_latin1_text.is_empty());
        assert!(info.utf8_text.is_empty());
        assert!(info.exif_metadata.is_none());
        assert!(info.icc_profile.is_none());
    }

    fn colony() -> SaveFile {
        let desktop = DesktopSnapshot {
            monitors: vec![MonitorInfo {
                id: 1,
                display_key: DisplayKey([1; 16]),
                bounds: DesktopRect {
                    x: 0.0,
                    y: 0.0,
                    width: 1280.0,
                    height: 800.0,
                },
                usable_bounds: DesktopRect {
                    x: 0.0,
                    y: 24.0,
                    width: 1280.0,
                    height: 776.0,
                },
                scale_factor: 1.0,
                primary: true,
            }],
            ..DesktopSnapshot::default()
        };
        let created = datetime!(2026-05-02 08:00 UTC);
        let mut world = World::new([19; 32], created, &desktop);
        world.tick(created + time::Duration::days(120), 0.05, &desktop);
        world.save
    }

    #[test]
    fn cancelled_colony_portrait_does_not_render_or_create_anything() {
        assert_eq!(
            export_colony_card_to_selected_destination(&colony(), None).unwrap(),
            None
        );
    }

    #[test]
    fn colony_portrait_is_written_at_its_fixed_size_with_no_hidden_metadata() {
        let path = std::env::temp_dir().join(format!(
            "formiga-colony-card-export-test-{}.png",
            std::process::id()
        ));
        let save = colony();
        write_colony_card_png(&save, &path).unwrap();
        assert_no_hidden_metadata(&path, COLONY_CARD_WIDTH, COLONY_CARD_HEIGHT);

        // Nothing but pixels: no name, no share code, no display key anywhere in the file.
        let bytes = std::fs::read(&path).unwrap();
        let contains = |needle: &[u8]| {
            !needle.is_empty() && bytes.windows(needle.len()).any(|window| window == needle)
        };
        for creature in &save.creatures {
            assert!(!contains(creature.name.as_bytes()));
            assert!(!contains(
                formiga_core::encode_creature_seed(creature.origin).as_bytes()
            ));
        }
        assert!(!contains(&save.colony_seed));

        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn the_colony_portrait_offers_one_name_that_says_nothing_about_who_lives_here() {
        assert_eq!(COLONY_CARD_FILENAME, "Formiga-colony.png");
        assert_eq!(
            with_png_extension(PathBuf::from(COLONY_CARD_FILENAME)),
            PathBuf::from(COLONY_CARD_FILENAME)
        );
        for creature in &colony().creatures {
            assert!(!COLONY_CARD_FILENAME.contains(&creature.name));
        }
    }
}
