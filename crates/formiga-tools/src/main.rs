mod accessory_sheet;
mod app_icon;
mod colony_card;
mod creature_sheets;
mod cuteness_sheet;
mod decoration_sheet;
mod demo_images;
mod dev;
mod dev_art;
mod dev_catalog;
mod dev_save;
mod fixtures;
mod habit_sheet;
mod home_yard_sheet;
mod palette_sheet;
mod pixels;
mod pose_sheets;
mod postcard;
mod prop_sheet;
mod scenery_sheet;
mod shelter_sheet;
mod simulate;
mod soak;
mod social_preview;
mod sticker;
mod tick_bench;
mod train_sheet;
mod ui_sheet;
mod village_life_sheet;
mod wonder_sheet;

use anyhow::Result;
use app_icon::app_icon;
use creature_sheets::{
    animation_preview, classic_sheet, contact_sheet, creature_card, expression_sheet,
    generation_sheet,
};
use demo_images::{demo_animation, hero_image};
use fixtures::{fixture_desktop, reference_creatures};
use home_yard_sheet::home_yard_sheet;
use pixels::{
    SCENE_CELL, blend_pixel, blit_canvas_scaled, blit_scaled, blit_scaled_anchor,
    blit_scaled_square_alpha, draw_rect_alpha, fill_gradient, fill_rect, write_png,
};
use pose_sheets::{activity_sheet, ambient_sheet, gesture_face, gesture_sheet, motion_sheet};
use shelter_sheet::shelter_sheet;
use simulate::simulate;
use std::path::PathBuf;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("contact-sheet") => contact_sheet(output_argument(&args)),
        Some("generation-sheet") => generation_sheet(output_argument_with_default(
            &args,
            "docs/assets/generation-sheet.png",
        )),
        Some("classic-sheet") => classic_sheet(output_argument_with_default(
            &args,
            "docs/assets/classic-sheet.png",
        )),
        Some("face-sheet") => cuteness_sheet::face_sheet(output_argument_with_default(
            &args,
            "docs/assets/face-sheet.png",
        )),
        Some("temperament-sheet") => cuteness_sheet::temperament_sheet(
            output_argument_with_default(&args, "docs/assets/temperament-sheet.png"),
        ),
        Some("cuteness-sheet") => cuteness_sheet::run(
            output_argument_with_default(&args, "cuteness-sheet.png"),
            cuteness_sheet::options(&args)?,
        ),
        Some("home-yard-sheet") => home_yard_sheet(output_argument_with_default(
            &args,
            "docs/assets/home-yard-sheet.png",
        )),
        Some("scenery-sheet") => scenery_sheet::scenery_sheet(output_argument_with_default(
            &args,
            "docs/assets/scenery-sheet.png",
        )),
        Some("animation-preview") => animation_preview(
            output_argument_with_default(&args, "animation-preview.png"),
            seed_argument(&args),
        ),
        Some("expression-sheet") => expression_sheet(output_argument_with_default(
            &args,
            "docs/assets/expression-sheet.png",
        )),
        Some("motion-sheet") => motion_sheet(output_argument_with_default(
            &args,
            "docs/assets/motion-sheet.png",
        )),
        Some("gesture-sheet") => gesture_sheet(output_argument_with_default(
            &args,
            "docs/assets/gesture-sheet.png",
        )),
        Some("decoration-sheet") => decoration_sheet::run(output_argument_with_default(
            &args,
            "docs/assets/decoration-sheet.png",
        )),
        Some("village-life-sheet") => village_life_sheet::run(output_argument_with_default(
            &args,
            "docs/assets/village-life-sheet.png",
        )),
        Some("accessory-sheet") => accessory_sheet::run(output_argument_with_default(
            &args,
            "docs/assets/accessory-sheet.png",
        )),
        Some("habit-sheet") => habit_sheet::run(output_argument_with_default(
            &args,
            "docs/assets/habit-sheet.png",
        )),
        Some("activity-sheet") => activity_sheet(output_argument_with_default(
            &args,
            "docs/assets/activity-sheet.png",
        )),
        Some("ambient-sheet") => ambient_sheet(output_argument_with_default(
            &args,
            "docs/assets/ambient-sheet.png",
        )),
        Some("ui-sheet") => ui_sheet::run(output_argument_with_default(
            &args,
            "docs/assets/ui-sheet.png",
        )),
        Some("hero-image") => {
            hero_image(output_argument_with_default(&args, "docs/assets/hero.png"))
        }
        Some("demo-animation") => demo_animation(output_argument_with_default(
            &args,
            "docs/assets/formiga-demo.gif",
        )),
        Some("social-preview") => social_preview::social_preview(output_argument_with_default(
            &args,
            "docs/assets/social-preview.png",
        )),
        Some("itch-cover") => social_preview::itch_cover(output_argument_with_default(
            &args,
            "packaging/itch/cover.png",
        )),
        Some("app-icon") => app_icon(
            output_argument_with_default(&args, "packaging/shared"),
            source_argument(&args),
        ),
        Some("train-sheet") => train_sheet::run(output_argument_with_default(
            &args,
            "docs/assets/train-sheet.png",
        )),
        Some("wonder-sheet") => wonder_sheet::run(output_argument_with_default(
            &args,
            "docs/assets/wonder-sheet.png",
        )),
        Some("prop-sheet") => prop_sheet::run(output_argument_with_default(
            &args,
            "docs/assets/prop-sheet.png",
        )),
        Some("shelter-sheet") => shelter_sheet(output_argument_with_default(
            &args,
            "docs/assets/shelter-sheet.png",
        )),
        Some("village-palette-sheet") => palette_sheet::run(output_argument_with_default(
            &args,
            "docs/assets/village-palette-sheet.png",
        )),
        Some("creature-card") => creature_card(output_argument_with_default(
            &args,
            "docs/assets/creature-card.png",
        )),
        Some("sticker") => sticker::run(
            output_argument_with_default(&args, "docs/assets/sticker-wave.gif"),
            sticker::clip_argument(&args)?,
            sticker::scale_argument(&args)?,
            sticker::seed_argument(&args)?,
        ),
        Some("colony-card") => colony_card::run(output_argument_with_default(
            &args,
            "docs/assets/colony-card.png",
        )),
        Some("postcard") => postcard::run(
            output_argument_with_default(&args, "docs/assets/postcard.png"),
            postcard::scene_argument(&args)?,
            &postcard::caption_argument(&args),
        ),
        Some("postcard-sheet") => postcard::sheet(output_argument_with_default(
            &args,
            "docs/assets/postcards.png",
        )),
        Some("tick-bench") => tick_bench::run(&args[2..]),
        Some("soak") => soak::run(&args[2..]),
        Some("dev") => dev::run(&args[2..]),
        Some("simulate") => simulate(
            args.get(2)
                .and_then(|value| value.parse().ok())
                .unwrap_or(181),
        ),
        _ => {
            eprintln!(
                "usage:\n  formiga-tools contact-sheet [--output PATH]\n  formiga-tools generation-sheet [--output PATH]\n  formiga-tools classic-sheet [--output PATH]\n  formiga-tools face-sheet [--output PATH]\n  formiga-tools temperament-sheet [--output PATH]\n  formiga-tools cuteness-sheet [--count N] [--seed NUMBER] [--edition details|archetypes|original] [--ratings FILE] [--output PATH]\n  formiga-tools home-yard-sheet [--output PATH]\n  formiga-tools scenery-sheet [--output PATH]\n  formiga-tools animation-preview [--seed NUMBER] [--output PATH]\n  formiga-tools expression-sheet [--output PATH]\n  formiga-tools gesture-sheet [--output PATH]\n  formiga-tools motion-sheet [--output PATH]\n  formiga-tools habit-sheet [--output PATH]\n  formiga-tools activity-sheet [--output PATH]\n  formiga-tools ambient-sheet [--output PATH]\n  formiga-tools prop-sheet [--output PATH]\n  formiga-tools wonder-sheet [--output PATH]\n  formiga-tools train-sheet [--output PATH]\n  formiga-tools ui-sheet [--output PATH]\n  formiga-tools social-preview [--output PATH]\n  formiga-tools itch-cover [--output PATH]\n  formiga-tools hero-image [--output PATH]\n  formiga-tools demo-animation [--output PATH]\n  formiga-tools app-icon [--source PNG] [--output DIRECTORY]\n  formiga-tools shelter-sheet [--output PATH]\n  formiga-tools decoration-sheet [--output PATH]\n  formiga-tools village-palette-sheet [--output PATH]\n  formiga-tools creature-card [--output PATH]\n  formiga-tools sticker [--seed NUMBER] [--clip NAME] [--scale 4|8] [--output PATH]\n  formiga-tools colony-card [--output PATH]\n  formiga-tools postcard [--scene nap|picnic|play|dusk] [--caption TEXT] [--output PATH]\n  formiga-tools postcard-sheet [--output PATH]\n  formiga-tools simulate [DAYS]\n  formiga-tools tick-bench [--ticks N] [--warmup N] [FILTER]\n  formiga-tools soak [--colonies N] [--days N] [--seed N] [--threads N] [--damage N] [--only N] [--out DIR]\n  formiga-tools dev fixtures | fixture NAME --out DIR [--now-unix SECONDS] | check-fixtures | capture NAME --out PNG"
            );
            Ok(())
        }
    }
}

fn output_argument(args: &[String]) -> PathBuf {
    output_argument_with_default(args, "contact-sheet.png")
}

fn output_argument_with_default(args: &[String], default: &str) -> PathBuf {
    args.windows(2)
        .find(|window| window[0] == "--output")
        .map(|window| PathBuf::from(&window[1]))
        .unwrap_or_else(|| PathBuf::from(default))
}

fn seed_argument(args: &[String]) -> u64 {
    args.windows(2)
        .find(|window| window[0] == "--seed")
        .and_then(|window| window[1].parse().ok())
        .unwrap_or(17)
}

fn source_argument(args: &[String]) -> PathBuf {
    args.windows(2)
        .find(|window| window[0] == "--source")
        .map(|window| PathBuf::from(&window[1]))
        .unwrap_or_else(|| PathBuf::from("packaging/shared/Formiga-mascot-master.png"))
}
