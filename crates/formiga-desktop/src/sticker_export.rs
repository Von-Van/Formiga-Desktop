//! Animated sticker export, on the same contract as the creature card: the native save dialog
//! runs before anything is rendered, a cancelled dialog allocates nothing, and every buffer the
//! export makes is released as soon as the file is written. Nothing reaches the overlay GPU, and
//! the colony save is never touched.

use anyhow::{Context as _, Result};
use formiga_art::{StickerClip, StickerRenderer};
use formiga_core::Creature;
use std::path::PathBuf;

pub fn choose_sticker_destination(creature: &Creature, clip: StickerClip) -> Option<PathBuf> {
    rfd::FileDialog::new()
        .add_filter("Animated GIF", &["gif"])
        .set_file_name(default_sticker_filename(&creature.name, clip))
        .save_file()
}

pub fn export_to_selected_destination(
    creature: &Creature,
    clip: StickerClip,
    scale: u32,
    selected: Option<PathBuf>,
) -> Result<Option<PathBuf>> {
    let Some(path) = selected else {
        return Ok(None);
    };
    let path = with_gif_extension(path);
    write_sticker_gif(creature, clip, scale, &path)?;
    Ok(Some(path))
}

fn write_sticker_gif(
    creature: &Creature,
    clip: StickerClip,
    scale: u32,
    path: &std::path::Path,
) -> Result<()> {
    // Render only after a destination exists. The frames and the encoded bytes are scoped to this
    // call and are released immediately after the on-demand export.
    let bytes = StickerRenderer::render(creature, clip, scale)
        .encode_gif()
        .with_context(|| format!("encode the {} sticker", clip.slug()))?;
    std::fs::write(path, bytes).with_context(|| format!("create sticker at {}", path.display()))
}

/// `<Name>-<clip>.gif`, with everything that is not a letter or a digit folded into single
/// hyphens. Counted in characters rather than bytes, so a name in any script survives.
fn default_sticker_filename(name: &str, clip: StickerClip) -> String {
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
    format!("{stem}-{}.gif", clip.slug())
}

fn with_gif_extension(mut path: PathBuf) -> PathBuf {
    if !path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("gif"))
    {
        path.set_extension("gif");
    }
    path
}

#[cfg(test)]
mod tests {
    use super::*;
    use formiga_art::{DEFAULT_STICKER_SCALE, STICKER_SCALES};
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
            export_to_selected_destination(
                &creature(),
                StickerClip::Wave,
                DEFAULT_STICKER_SCALE,
                None
            )
            .unwrap(),
            None
        );
    }

    #[test]
    fn filenames_handle_unicode_and_path_punctuation() {
        assert_eq!(
            default_sticker_filename("Mochi 雪 / très doux", StickerClip::Dance),
            "Mochi-雪-très-doux-dance.gif"
        );
        assert_eq!(
            default_sticker_filename("///", StickerClip::Wave),
            "Formiga-wave.gif"
        );
    }

    #[test]
    fn selection_is_normalized_to_gif() {
        assert_eq!(
            with_gif_extension(PathBuf::from("Mallow.sticker")),
            PathBuf::from("Mallow.gif")
        );
        assert_eq!(
            with_gif_extension(PathBuf::from("Mallow.GIF")),
            PathBuf::from("Mallow.GIF")
        );
    }

    #[test]
    fn every_offered_clip_and_scale_writes_the_shared_encoder_output_and_nothing_else() {
        let mut creature = creature();
        creature.name = "Mochi 雪".into();
        for clip in StickerClip::ALL {
            for scale in STICKER_SCALES {
                let path = std::env::temp_dir().join(format!(
                    "formiga-sticker-export-test-{}-{}-{scale}.gif",
                    std::process::id(),
                    clip.slug()
                ));
                write_sticker_gif(&creature, clip, scale, &path).unwrap();
                let bytes = std::fs::read(&path).unwrap();

                // Exactly what the one shared encoder produced, byte for byte: the desktop adds
                // nothing of its own to the file.
                assert_eq!(
                    bytes,
                    StickerRenderer::render(&creature, clip, scale)
                        .encode_gif()
                        .unwrap()
                );
                assert!(bytes.starts_with(b"GIF89a"));
                // What the file may and may not carry is checked block by block where the encoder
                // lives; here the contract is that the export adds nothing of its own on the way
                // to disk, and that nothing about this creature is written beside the pixels.
                assert!(!contains(&bytes, creature.name.as_bytes()));
                assert!(!contains(
                    &bytes,
                    formiga_core::encode_creature_seed(creature.origin).as_bytes()
                ));

                std::fs::remove_file(path).unwrap();
            }
        }
    }

    fn contains(haystack: &[u8], needle: &[u8]) -> bool {
        !needle.is_empty()
            && haystack
                .windows(needle.len())
                .any(|window| window == needle)
    }
}
