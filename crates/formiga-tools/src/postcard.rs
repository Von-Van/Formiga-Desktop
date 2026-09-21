//! `formiga-tools postcard`: one postcard of a sample colony, or every scene on one sheet.
//!
//!   cargo run -p formiga-tools -- postcard [--scene nap|picnic|play|dusk] [--caption TEXT] [--output PATH]
//!   cargo run -p formiga-tools -- postcard-sheet [--output docs/assets/postcards.png]
//!
//! The colony is the one the colony portrait is drawn from, grown for a season, so the postcards
//! and the portrait in the docs show the same companions and the same village.

use crate::write_png;
use anyhow::{Result, bail};
use formiga_art::{POSTCARD_HEIGHT, POSTCARD_WIDTH, PostcardRenderer, PostcardScene};
use std::path::PathBuf;

pub fn run(path: PathBuf, scene: PostcardScene, caption: &str) -> Result<()> {
    let save = crate::colony_card::sample_colony();
    let card = PostcardRenderer::render(&save, scene, caption);
    write_png(&path, POSTCARD_WIDTH, POSTCARD_HEIGHT, &card.rgba_bytes())?;
    println!("wrote {}", path.display());
    Ok(())
}

/// Every scene at half size, two across: nap and picnic, then play and dusk, each with a caption
/// but the last, which shows a card sent without one.
pub fn sheet(path: PathBuf) -> Result<()> {
    let save = crate::colony_card::sample_colony();
    let captions = [
        "Everyone fell asleep in the sun",
        "Snacks for all, and a cup each",
        "Nobody has caught the ball yet",
        "",
    ];
    let (half_width, half_height) = (POSTCARD_WIDTH / 2, POSTCARD_HEIGHT / 2);
    let width = half_width * 2;
    let height = half_height * 2;
    let mut pixels = vec![0_u8; (width * height * 4) as usize];
    for (index, (scene, caption)) in PostcardScene::ALL.into_iter().zip(captions).enumerate() {
        let card = PostcardRenderer::render(&save, scene, caption);
        let (left, top) = (
            (index as u32 % 2) * half_width,
            (index as u32 / 2) * half_height,
        );
        // Each half-size pixel is the average of the four it stands for.
        for y in 0..half_height {
            for x in 0..half_width {
                let mut sum = [0_u32; 4];
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let pixel = card.get((x * 2 + dx) as i32, (y * 2 + dy) as i32);
                    for (total, value) in sum.iter_mut().zip([pixel.r, pixel.g, pixel.b, pixel.a]) {
                        *total += u32::from(value);
                    }
                }
                let index = (((top + y) * width + left + x) * 4) as usize;
                for (channel, total) in sum.iter().enumerate() {
                    pixels[index + channel] = (total / 4) as u8;
                }
            }
        }
    }
    write_png(&path, width, height, &pixels)?;
    println!("wrote {}", path.display());
    Ok(())
}

pub fn scene_argument(args: &[String]) -> Result<PostcardScene> {
    match args.iter().position(|arg| arg == "--scene") {
        None => Ok(PostcardScene::default()),
        Some(index) => {
            let Some(value) = args.get(index + 1) else {
                bail!("--scene needs one of nap, picnic, play, dusk");
            };
            PostcardScene::from_slug(value).ok_or_else(|| {
                anyhow::anyhow!("unknown scene {value:?}; try nap, picnic, play, or dusk")
            })
        }
    }
}

pub fn caption_argument(args: &[String]) -> String {
    args.iter()
        .position(|arg| arg == "--caption")
        .and_then(|index| args.get(index + 1))
        .cloned()
        .unwrap_or_default()
}
