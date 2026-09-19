//! `formiga-tools sticker`: one creature's clip as a transparent animated GIF, for the docs.

use crate::fixture_desktop;
use anyhow::{Context, Result, bail};
use formiga_art::{DEFAULT_STICKER_SCALE, STICKER_SCALES, StickerClip, StickerRenderer};
use formiga_core::*;
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use time::OffsetDateTime;

pub fn clip_argument(args: &[String]) -> Result<StickerClip> {
    let Some(window) = args.windows(2).find(|window| window[0] == "--clip") else {
        return Ok(StickerClip::default());
    };
    StickerClip::from_slug(&window[1]).with_context(|| {
        let names: Vec<&str> = StickerClip::ALL.iter().map(|clip| clip.slug()).collect();
        format!(
            "unknown clip {:?}; try one of {}",
            window[1],
            names.join(", ")
        )
    })
}

pub fn scale_argument(args: &[String]) -> Result<u32> {
    let Some(window) = args.windows(2).find(|window| window[0] == "--scale") else {
        return Ok(DEFAULT_STICKER_SCALE);
    };
    let scale: u32 = window[1]
        .parse()
        .with_context(|| format!("scale {:?} is not a number", window[1]))?;
    if !STICKER_SCALES.contains(&scale) {
        bail!("scale must be one of {STICKER_SCALES:?}");
    }
    Ok(scale)
}

/// A numeric seed if one was asked for; otherwise none, and the sample creature is used.
pub fn seed_argument(args: &[String]) -> Result<Option<u64>> {
    let Some(window) = args.windows(2).find(|window| window[0] == "--seed") else {
        return Ok(None);
    };
    window[1]
        .parse()
        .map(Some)
        .with_context(|| format!("seed {:?} is not a number", window[1]))
}

/// With no seed of its own, the sticker poses the same creature the creature card does, so the
/// two docs assets read as one set.
pub fn run(path: PathBuf, clip: StickerClip, scale: u32, seed_number: Option<u64>) -> Result<()> {
    let mut seed = [0_u8; 32];
    seed.copy_from_slice(&match seed_number {
        Some(number) => Sha256::digest(number.to_le_bytes()),
        None => Sha256::digest(b"formiga-creature-card-preview"),
    });
    let mut world = World::new(seed, OffsetDateTime::UNIX_EPOCH, &fixture_desktop());
    let creature = &mut world.save.creatures[0];
    creature.name = "Mallow".into();

    let sticker = StickerRenderer::render(creature, clip, scale);
    let bytes = sticker
        .encode_gif()
        .with_context(|| format!("encode the {} sticker", clip.slug()))?;
    std::fs::write(&path, &bytes).with_context(|| format!("create {}", path.display()))?;
    println!(
        "wrote {} ({}x{}, {} frames, {} bytes)",
        path.display(),
        sticker.width(),
        sticker.height(),
        sticker.frames().len(),
        bytes.len()
    );
    Ok(())
}
