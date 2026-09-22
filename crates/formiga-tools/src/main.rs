use anyhow::{Context, Result, bail};
use formiga_art::{
    BodyClip, CARD_HEIGHT, CARD_WIDTH, CreatureCardRenderer, CreatureRenderer, ExpressionKind,
    EyelidPose, FRAME_SIZE, FaceRenderState, GazeDirection, SHELTER_SIZE, ShelterRenderer,
};
use formiga_core::*;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use time::OffsetDateTime;

mod colony_card;
mod habit_sheet;
mod palette_sheet;
mod postcard;
mod prop_sheet;
mod social_preview;
mod sticker;
mod tick_bench;
mod ui_sheet;

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
        Some("home-yard-sheet") => home_yard_sheet(output_argument_with_default(
            &args,
            "docs/assets/home-yard-sheet.png",
        )),
        Some("animation-preview") => animation_preview(
            output_argument_with_default(&args, "animation-preview.png"),
            seed_argument(&args),
        ),
        Some("expression-sheet") => expression_sheet(output_argument_with_default(
            &args,
            "docs/assets/expression-sheet.png",
        )),
        Some("gesture-sheet") => gesture_sheet(output_argument_with_default(
            &args,
            "docs/assets/gesture-sheet.png",
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
        Some("simulate") => simulate(
            args.get(2)
                .and_then(|value| value.parse().ok())
                .unwrap_or(181),
        ),
        _ => {
            eprintln!(
                "usage:\n  formiga-tools contact-sheet [--output PATH]\n  formiga-tools generation-sheet [--output PATH]\n  formiga-tools classic-sheet [--output PATH]\n  formiga-tools home-yard-sheet [--output PATH]\n  formiga-tools animation-preview [--seed NUMBER] [--output PATH]\n  formiga-tools expression-sheet [--output PATH]\n  formiga-tools gesture-sheet [--output PATH]\n  formiga-tools habit-sheet [--output PATH]\n  formiga-tools activity-sheet [--output PATH]\n  formiga-tools ambient-sheet [--output PATH]\n  formiga-tools prop-sheet [--output PATH]\n  formiga-tools ui-sheet [--output PATH]\n  formiga-tools social-preview [--output PATH]\n  formiga-tools itch-cover [--output PATH]\n  formiga-tools hero-image [--output PATH]\n  formiga-tools demo-animation [--output PATH]\n  formiga-tools app-icon [--source PNG] [--output DIRECTORY]\n  formiga-tools shelter-sheet [--output PATH]\n  formiga-tools village-palette-sheet [--output PATH]\n  formiga-tools creature-card [--output PATH]\n  formiga-tools sticker [--seed NUMBER] [--clip NAME] [--scale 4|8] [--output PATH]\n  formiga-tools colony-card [--output PATH]\n  formiga-tools postcard [--scene nap|picnic|play|dusk] [--caption TEXT] [--output PATH]\n  formiga-tools postcard-sheet [--output PATH]\n  formiga-tools simulate [DAYS]\n  formiga-tools tick-bench [--ticks N] [--warmup N] [FILTER]"
            );
            Ok(())
        }
    }
}

fn creature_card(path: PathBuf) -> Result<()> {
    let mut seed = [0_u8; 32];
    seed.copy_from_slice(&Sha256::digest(b"formiga-creature-card-preview"));
    let mut world = World::new(seed, OffsetDateTime::UNIX_EPOCH, &fixture_desktop());
    let creature = &mut world.save.creatures[0];
    creature.name = "Mallow".into();
    creature.born_at_utc = time::macros::datetime!(2026-08-14 12:30 UTC);
    creature.tendencies.climbing = 62;
    creature.tendencies.exploration = 53;
    creature.tendencies.sociability = 41;
    formiga_core::update_descriptor_flags(&mut creature.memory, creature.tendencies);
    let card = CreatureCardRenderer::render(creature);
    write_png(&path, CARD_WIDTH, CARD_HEIGHT, &card.rgba_bytes())?;
    println!("wrote {}", path.display());
    Ok(())
}

fn generation_sheet(path: PathBuf) -> Result<()> {
    let scale = 4;
    let cell = FRAME_SIZE * scale;
    let (width, height) = (
        cell * EarStyle::ALL.len() as u32,
        cell * BodyPlan::ALL.len() as u32,
    );
    let mut pixels = vec![0; (width * height * 4) as usize];
    fill_gradient(
        &mut pixels,
        width,
        height,
        [237, 234, 224, 255],
        [209, 226, 219, 255],
    );
    for (row, body) in BodyPlan::ALL.into_iter().enumerate() {
        for (column, ears) in EarStyle::ALL.into_iter().enumerate() {
            let seed =
                SeedStream::new([55; 32]).bytes("generation-sheet", (row * 6 + column) as u64);
            let mut creature =
                World::preview_adult(seed, OffsetDateTime::UNIX_EPOCH, &fixture_desktop());
            // The grid is of modular parts, so it starts from the modular recipe alone.
            let mut design = CreatureDesign::modular(seed, 0, None);
            design.body = body;
            design.ears = ears;
            apply_creature_design(&mut creature, Some(design));
            let canvas =
                CreatureRenderer::render_frame(&creature.appearance, ActionKind::Idle, 0, true);
            blit_scaled_square_alpha(
                &mut pixels,
                width,
                column as u32 * cell,
                row as u32 * cell,
                &canvas.rgba_bytes(),
                FRAME_SIZE,
                scale,
            );
        }
    }
    write_png(&path, width, height, &pixels)?;
    println!("wrote {}", path.display());
    Ok(())
}

/// Each classic part alone on every body plan, then wholly classic companions in motion.
///
/// The first band starts every row from the same plain modular recipe and changes one part per
/// column: candy colours, the five eye arrangements, nubs and stick legs, antennae and sprouts,
/// the three patterns, and the two tails. The second band gives each plan every classic part at
/// once and runs it through standing, walking, greeting, hanging, sleeping, and five gestures,
/// which is where a nub, a stick leg, or an antenna would come loose if it were going to.
fn classic_sheet(path: PathBuf) -> Result<()> {
    const SCALE: u32 = 3;
    let parts = |edit: fn(&mut ClassicParts)| {
        let mut parts = ClassicParts::default();
        edit(&mut parts);
        parts
    };
    let singles = [
        parts(|_| {}),
        parts(|p| p.coat = 1),
        parts(|p| p.face = 1),
        parts(|p| p.face = 2),
        parts(|p| p.face = 3),
        parts(|p| p.face = 4),
        parts(|p| p.face = 5),
        parts(|p| p.limbs = 1),
        parts(|p| p.limbs = 2),
        parts(|p| p.crown = 1),
        parts(|p| p.crown = 2),
        parts(|p| p.pattern = 1),
        parts(|p| p.pattern = 2),
        parts(|p| p.pattern = 3),
        parts(|p| p.tail = 1),
        parts(|p| p.tail = 2),
    ];
    let motion: [(BodyClip, u8); 10] = [
        (BodyClip::Action(ActionKind::Idle), 0),
        (BodyClip::Action(ActionKind::Traverse), 1),
        (BodyClip::Action(ActionKind::Greet), 1),
        (BodyClip::Action(ActionKind::Dangle), 0),
        (BodyClip::Action(ActionKind::Sleep), 0),
        (BodyClip::Gesture(Gesture::Cheer), 1),
        (BodyClip::Gesture(Gesture::Gasp), 0),
        (BodyClip::Gesture(Gesture::Cover), 0),
        (BodyClip::Gesture(Gesture::Reach), 1),
        (BodyClip::Gesture(Gesture::Watch), 0),
    ];
    let cell = FRAME_SIZE * SCALE;
    let rows = BodyPlan::ALL.len() as u32;
    let (width, height) = (cell * singles.len() as u32, cell * rows * 2);
    let mut pixels = vec![0; (width * height * 4) as usize];
    fill_gradient(
        &mut pixels,
        width,
        height,
        [237, 234, 224, 255],
        [209, 226, 219, 255],
    );
    let mut creature =
        World::preview_adult([13; 32], OffsetDateTime::UNIX_EPOCH, &fixture_desktop());
    // A coat and accent far enough apart that every accent-coloured part shows against the body.
    let base = CreatureDesign {
        classic: ClassicParts::default(),
        coat: [118, 172, 196],
        accent: [242, 168, 88],
        ..creature
            .appearance
            .design
            .expect("a generated companion carries its recipe")
    };
    let mut draw = |creature: &Creature, clip: BodyClip, frame: u8, column: u32, row: u32| {
        let state = FaceRenderState {
            expression: ExpressionKind::Neutral,
            eyelids: if clip == BodyClip::Action(ActionKind::Sleep) {
                EyelidPose::Closed
            } else {
                EyelidPose::Open
            },
            gaze: GazeDirection::default(),
        };
        let canvas = CreatureRenderer::render_composited_frame(
            &creature.appearance,
            clip,
            frame,
            true,
            false,
            state,
        );
        blit_scaled_square_alpha(
            &mut pixels,
            width,
            column * cell,
            row * cell,
            &canvas.rgba_bytes(),
            FRAME_SIZE,
            SCALE,
        );
    };
    for (row, body) in BodyPlan::ALL.into_iter().enumerate() {
        for (column, classic) in singles.iter().enumerate() {
            apply_creature_design(
                &mut creature,
                Some(CreatureDesign {
                    body,
                    classic: *classic,
                    ..base
                }),
            );
            let idle = BodyClip::Action(ActionKind::Idle);
            draw(&creature, idle, 0, column as u32, row as u32);
        }
        // Every part at once, with a different eye arrangement, crown, and tail on each row.
        let index = row as u8;
        apply_creature_design(
            &mut creature,
            Some(CreatureDesign {
                body,
                classic: ClassicParts {
                    coat: 1,
                    face: 1 + index % 5,
                    limbs: 1 + index % 2,
                    crown: 1 + index % 2,
                    pattern: 1 + index % 3,
                    tail: 1 + index % 2,
                },
                coat: [0xe9, 0x8a, 0xb5],
                accent: [0xff, 0xe0, 0x81],
                ..base
            }),
        );
        for (column, (clip, frame)) in motion.into_iter().enumerate() {
            draw(&creature, clip, frame, column as u32, rows + row as u32);
        }
    }
    write_png(&path, width, height, &pixels)?;
    println!("wrote {}", path.display());
    Ok(())
}

/// One stretch of village ground, drawn exactly where the layout functions put everything: the
/// two keepsake trees bookending the strip with whatever the scrapbook holds hung between them,
/// the colony house, one cottage per later member, the belongings in the yard at each trunk, and
/// every resident out on the shared ground in front of the row.
struct YardPanel {
    cottages: Vec<DwellingKind>,
    objects: usize,
    /// Hangout spots put down on the ground, as fractions along it: cushion, blanket, lookout.
    hangouts: [Option<f32>; 3],
    /// Garden patches planted along the ground: flowers, vegetables, herbs.
    gardens: [Option<f32>; 3],
    /// The palette the village is painted in, if not its own colours.
    palette: Option<VillagePalette>,
    /// The type each house is built as, slot by slot; empty for the colony's own type throughout.
    styles: Vec<ShelterStyle>,
    /// After dark: the houses lit from inside, on a night sky.
    night: bool,
    /// How many of the sixteen trinkets this colony has found, so the trees can be judged bare,
    /// part-filled and full.
    found: usize,
    style_seed: u8,
}

/// Shelter pixels of ground each panel shows, how tall it is, and the blit scale.
const YARD_STRIP: u32 = 560;
const YARD_HEIGHT: u32 = 88;
const YARD_SCALE: u32 = 2;

fn home_yard_sheet(path: PathBuf) -> Result<()> {
    // Four shelter styles with a full colony of six and the two trees filling up as the scrapbook
    // does — the outward one takes the first eight finds, the inward one the rest — then a colony
    // of one to six, so the rule that nobody stands in front of a house can be checked at every
    // size and both corners, and so can the widest village there is: six full-size companions,
    // eight belongings split between the yards, and two bare trees. Last, the four styles again
    // after dark, lit from inside, each door hung with its own resident's curtain. Along the way,
    // hangout spots and garden patches spread out, bunched up and mixed together, and villages
    // painted in a named palette rather than their own colours.
    let full = vec![DwellingKind::Cottage; MAX_COLONY_CREATURES - 1];
    let mut panels: Vec<YardPanel> = (0..4)
        .map(|style| YardPanel {
            cottages: full.clone(),
            objects: MAX_COLONY_OBJECTS,
            // Two of the grown villages with spots put down, spread out and bunched up; one with
            // its three gardens planted; and one with spots and gardens mixed along the ground.
            hangouts: match style {
                0 => [Some(0.15), Some(0.5), Some(0.9)],
                1 => [Some(0.6), Some(0.6), Some(0.6)],
                3 => [Some(0.2), None, Some(0.7)],
                _ => [None; 3],
            },
            gardens: match style {
                2 => [Some(0.1), Some(0.45), Some(0.85)],
                3 => [Some(0.35), Some(0.5), Some(0.95)],
                _ => [None; 3],
            },
            palette: match style {
                2 => Some(VillagePalette::Meadow),
                3 => Some(VillagePalette::Twilight),
                _ => None,
            },
            // A mixed village: each house a different type, starting from the colony's own.
            styles: (0..MAX_COLONY_CREATURES)
                .map(|slot| {
                    ShelterStyle::ALL[(usize::from(style) + slot) % ShelterStyle::ALL.len()]
                })
                .collect(),
            found: [0, 5, 11, 16][style as usize],
            style_seed: style,
            night: false,
        })
        .collect();
    for members in 1..=MAX_COLONY_CREATURES {
        panels.push(YardPanel {
            cottages: full[..members - 1].to_vec(),
            objects: MAX_COLONY_OBJECTS,
            hangouts: [None; 3],
            gardens: [None; 3],
            palette: None,
            styles: Vec::new(),
            found: [16, 12, 8, 5, 3, 0][members - 1],
            style_seed: 3,
            night: false,
        });
    }
    for style in 0..4 {
        panels.push(YardPanel {
            cottages: full.clone(),
            objects: MAX_COLONY_OBJECTS,
            hangouts: if style == 1 {
                [Some(0.3), Some(0.55), Some(0.8)]
            } else {
                [None; 3]
            },
            gardens: if style == 2 {
                [Some(0.2), Some(0.5), Some(0.8)]
            } else {
                [None; 3]
            },
            palette: (style == 2).then_some(VillagePalette::Harbour),
            styles: (0..MAX_COLONY_CREATURES)
                .map(|slot| {
                    ShelterStyle::ALL[(usize::from(style) + slot) % ShelterStyle::ALL.len()]
                })
                .collect(),
            found: 8,
            style_seed: style,
            night: true,
        });
    }

    let gap = 10;
    let panel_width = YARD_STRIP * YARD_SCALE;
    let panel_height = YARD_HEIGHT * YARD_SCALE;
    let width = panel_width * 2 + gap * 3;
    let height = (panel_height + gap) * panels.len() as u32 + gap;
    let mut pixels = vec![0; (width * height * 4) as usize];
    fill_gradient(
        &mut pixels,
        width,
        height,
        [237, 234, 224, 255],
        [209, 226, 219, 255],
    );
    for (row, panel) in panels.iter().enumerate() {
        for (column, corner) in [HomeCorner::BottomRight, HomeCorner::BottomLeft]
            .into_iter()
            .enumerate()
        {
            let (x, y) = (
                gap + column as u32 * (panel_width + gap),
                gap + row as u32 * (panel_height + gap),
            );
            // Each panel is one corner of a desktop, so the empty half is real: it is the rest
            // of the display the village is tucked into.
            fill_rect(
                &mut pixels,
                width,
                x,
                y,
                panel_width,
                panel_height,
                if panel.night {
                    // A desktop wallpaper after dark.
                    [28, 34, 58, 235]
                } else {
                    [255, 255, 255, 90]
                },
            );
            fill_rect(
                &mut pixels,
                width,
                x,
                y + panel_height - (YARD_HEIGHT - 80) * YARD_SCALE,
                panel_width,
                YARD_SCALE,
                [120, 134, 128, 140],
            );
            draw_yard_panel(&mut pixels, width, x, y, panel, corner);
        }
    }
    write_png(&path, width, height, &pixels)?;
    println!("wrote {}", path.display());
    Ok(())
}

fn draw_yard_panel(
    pixels: &mut [u8],
    width: u32,
    base_x: u32,
    base_y: u32,
    panel: &YardPanel,
    corner: HomeCorner,
) {
    let mut seed = [55; 32];
    seed[1] = panel.style_seed;
    let mut monitor = fixture_desktop().monitors.remove(0);
    monitor.bounds = DesktopRect {
        x: 0.0,
        y: 0.0,
        width: YARD_STRIP as f32,
        height: YARD_HEIGHT as f32,
    };
    monitor.usable_bounds = monitor.bounds;
    monitor.scale_factor = 1.0;
    let mut home = ColonyHome::from_seed(
        seed,
        Some(monitor.display_key),
        Some(OffsetDateTime::UNIX_EPOCH),
        None,
    );
    home.corner = corner;
    home.palette = panel.palette;
    for (kind, along) in HangoutKind::ALL.into_iter().zip(panel.hangouts) {
        home.set_hangout(kind, along);
    }
    for (kind, along) in GardenKind::ALL.into_iter().zip(panel.gardens) {
        home.set_garden(kind, along);
    }
    let policy = HabitatPolicy::default();
    let monitors = std::slice::from_ref(&monitor);
    let cottages = &panel.cottages;
    let residents: Vec<Creature> = (0..=cottages.len())
        .map(|slot| {
            World::preview_adult(
                SeedStream::new(seed).bytes("yard-resident", slot as u64),
                OffsetDateTime::UNIX_EPOCH,
                &fixture_desktop(),
            )
        })
        .collect();
    let marks: Vec<Option<formiga_art::ResidentMark>> = residents
        .iter()
        .map(|resident| Some(formiga_art::ResidentMark::of(resident)))
        .collect();
    let village = ShelterRenderer::render_village(
        &home.drawn_shelter(),
        &ShelterDecorationKind::ALL,
        &marks,
        &panel.styles,
        true,
    );

    // Everything is blitted by the top-left of its own square, so a quad hung in a tree lands
    // by its anchor and a house lands by its footprint's middle on the ground line.
    let mut corner_of = |canvas: &formiga_art::Canvas, size: u32, x: i32, y: i32| {
        if x < 0 || y < 0 {
            return;
        }
        blit_scaled_square_alpha(
            pixels,
            width,
            base_x + x as u32 * YARD_SCALE,
            base_y + y as u32 * YARD_SCALE,
            &canvas.rgba_bytes(),
            size,
            YARD_SCALE,
        );
    };
    let standing = |size: u32, point: Point| {
        (
            point.x.round() as i32 - size as i32 / 2,
            point.y.round() as i32 - size as i32,
        )
    };

    // The two trees first, and then whatever the scrapbook holds hung between them: one 16x16
    // quad from the colony's own trinket sheet at the anchor for its slot, exactly as the overlay
    // draws them. The inward tree is the same atlas cell read the other way round.
    let members: Vec<formiga_art::Palette> = (0..=cottages.len())
        .map(|slot| {
            formiga_art::palette_for(
                &World::preview_adult(
                    SeedStream::new(seed).bytes("yard-resident", slot as u64),
                    OffsetDateTime::UNIX_EPOCH,
                    &fixture_desktop(),
                )
                .appearance,
            )
        })
        .collect();
    let sheet = formiga_art::TrinketAtlasRenderer::render(seed, &members);
    for end in TreeEnd::BOTH {
        let Some((_, tree)) = home_tree_position(&home, end, cottages, monitors, &policy, 1) else {
            continue;
        };
        let (tree_x, tree_y) = ShelterRenderer::village_cell(formiga_art::VillageCell::Tree);
        let mut cell = formiga_art::Canvas::new(SHELTER_SIZE, SHELTER_SIZE);
        for y in 0..SHELTER_SIZE as i32 {
            for x in 0..SHELTER_SIZE as i32 {
                let read = if end == TreeEnd::Inward {
                    SHELTER_SIZE as i32 - 1 - x
                } else {
                    x
                };
                cell.set(x, y, village.get(tree_x as i32 + read, tree_y as i32 + y));
            }
        }
        let (left, top) = standing(SHELTER_SIZE, tree);
        corner_of(&cell, SHELTER_SIZE, left, top);
        for variant in 0..panel.found as u8 {
            let Some((hangs_in, anchor)) = formiga_art::trinket_place(variant) else {
                continue;
            };
            if hangs_in != end {
                continue;
            }
            let (sx, sy, _, _) = formiga_art::TrinketAtlasRenderer::cell_rect(
                variant,
                formiga_art::TRINKET_FRAME_REST,
            );
            let mut quad =
                formiga_art::Canvas::new(formiga_art::TRINKET_CELL, formiga_art::TRINKET_CELL);
            for y in 0..formiga_art::TRINKET_CELL as i32 {
                for x in 0..formiga_art::TRINKET_CELL as i32 {
                    quad.set(x, y, sheet.get(sx as i32 + x, sy as i32 + y));
                }
            }
            let half = formiga_art::TRINKET_CELL as i32 / 2;
            corner_of(
                &quad,
                formiga_art::TRINKET_CELL,
                left + anchor.x - half,
                top + anchor.y - half,
            );
        }
    }

    for slot in 0..=cottages.len() {
        let Some((_, p)) = home_dwelling_position(&home, slot, cottages, monitors, &policy, 1)
        else {
            continue;
        };
        let (cell_x, cell_y) = ShelterRenderer::village_cell(formiga_art::VillageCell::House {
            slot,
            lit: panel.night,
        });
        let mut cell = formiga_art::Canvas::new(SHELTER_SIZE, SHELTER_SIZE);
        for y in 0..SHELTER_SIZE as i32 {
            for x in 0..SHELTER_SIZE as i32 {
                cell.set(x, y, village.get(cell_x as i32 + x, cell_y as i32 + y));
            }
        }
        let (x, y) = standing(SHELTER_SIZE, p);
        corner_of(&cell, SHELTER_SIZE, x, y);
    }

    // The colony's belongings, four over each tree's roots. They are drawn after the trees, so
    // they stand in front of a trunk, and back to front so a yard reads as having depth.
    let atlas = formiga_art::ColonyObjectRenderer::render_atlas(seed);
    let places = home_object_positions(&home, cottages, monitors, &policy, 1);
    let mut yard: Vec<(usize, Point)> = (0..panel.objects)
        .filter_map(|slot| places[slot].map(|(_, point)| (slot, point)))
        .collect();
    yard.sort_by(|a, b| a.1.y.total_cmp(&b.1.y));
    for (slot, p) in yard {
        let mut tile = formiga_art::Canvas::new(16, 16);
        for y in 0..16 {
            for x in 0..16 {
                tile.set(x, y, atlas.get(slot as i32 * 16 + x, y));
            }
        }
        let (x, y) = standing(16, p);
        corner_of(&tile, 16, x, y);
    }

    // The spots put down and the patches planted on the ground, the lookout turned out over the
    // rest of the display.
    let middle = monitor.usable_bounds.x + monitor.usable_bounds.width / 2.0;
    for (item, _, p) in home_ground_positions(&home, cottages, monitors, &policy, 1) {
        let cell = formiga_art::ColonyObjectRenderer::ground_cell(item) as i32;
        let mirrored = formiga_art::ColonyObjectRenderer::ground_mirrored(item, p.x, middle);
        let mut tile = formiga_art::Canvas::new(16, 16);
        for y in 0..16 {
            for x in 0..16 {
                let read = if mirrored { 15 - x } else { x };
                tile.set(x, y, atlas.get(cell * 16 + read, y));
            }
        }
        let (x, y) = standing(16, p);
        corner_of(&tile, 16, x, y);
    }

    // The residents themselves, spread along the ground in front of the row and facing the strip.
    let facing = corner == HomeCorner::BottomLeft;
    for (slot, member) in residents.iter().enumerate() {
        let Some((_, p)) = home_resting_position(
            &home,
            slot,
            cottages.len() + 1,
            cottages,
            monitors,
            &policy,
            1,
        ) else {
            continue;
        };
        let frame =
            CreatureRenderer::render_frame(&member.appearance, ActionKind::Homebound, 0, facing);
        let (x, y) = standing(FRAME_SIZE, p);
        corner_of(&frame, FRAME_SIZE, x, y);
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

fn animation_preview(path: PathBuf, seed_number: u64) -> Result<()> {
    const SCALE: u32 = 3;
    // One column per frame of the longest clip, so a clip that grows stays on its own row.
    let cols = ActionKind::ALL
        .into_iter()
        .map(|action| u32::from(formiga_art::AnimationSpec::for_action(action).frames))
        .max()
        .unwrap_or(1);
    let rows = ActionKind::ALL.len() as u32;
    let cell = FRAME_SIZE * SCALE;
    let width = cols * cell;
    let height = rows * cell;
    let mut pixels = vec![0_u8; (width * height * 4) as usize];
    let mut seed = [0_u8; 32];
    seed.copy_from_slice(&Sha256::digest(seed_number.to_le_bytes()));
    let world = World::new(seed, OffsetDateTime::UNIX_EPOCH, &fixture_desktop());
    let creature = &world.save.creatures[0];
    for (row, action) in ActionKind::ALL.into_iter().enumerate() {
        let spec = formiga_art::AnimationSpec::for_action(action);
        for frame in 0..spec.frames {
            let rendered =
                CreatureRenderer::render_frame(&creature.appearance, action, frame, true);
            blit_scaled(
                &mut pixels,
                width,
                u32::from(frame) * cell,
                row as u32 * cell,
                &rendered.rgba_bytes(),
                SCALE,
            );
        }
    }
    write_png(&path, width, height, &pixels)?;
    println!(
        "wrote {} for seed {} ({:?})",
        path.display(),
        seed_number,
        creature.appearance.family
    );
    Ok(())
}

fn contact_sheet(path: PathBuf) -> Result<()> {
    const COLS: u32 = 10;
    const ROWS: u32 = 10;
    const SCALE: u32 = 3;
    let cell = FRAME_SIZE * SCALE;
    let mut pixels = vec![0_u8; (COLS * cell * ROWS * cell * 4) as usize];
    let desktop = fixture_desktop();
    for index in 0..COLS * ROWS {
        let mut root = [0_u8; 32];
        root.copy_from_slice(&Sha256::digest(format!("formiga-contact-{index}")));
        let world = World::new(root, OffsetDateTime::UNIX_EPOCH, &desktop);
        let creature = &world.save.creatures[0];
        let frame = CreatureRenderer::render_frame(
            &creature.appearance,
            ActionKind::Idle,
            (index % 4) as u8,
            true,
        );
        blit_scaled(
            &mut pixels,
            COLS * cell,
            index % COLS * cell,
            index / COLS * cell,
            &frame.rgba_bytes(),
            SCALE,
        );
    }
    write_png(&path, COLS * cell, ROWS * cell, &pixels)?;
    println!("wrote {}", path.display());
    Ok(())
}

fn expression_sheet(path: PathBuf) -> Result<()> {
    const SCALE: u32 = 3;
    let creatures = reference_creatures();
    let cell = FRAME_SIZE * SCALE;
    let width = ExpressionKind::ALL.len() as u32 * cell;
    let height = creatures.len() as u32 * cell;
    let mut pixels = vec![0_u8; (width * height * 4) as usize];
    for (row, creature) in creatures.iter().enumerate() {
        for (column, expression) in ExpressionKind::ALL.into_iter().enumerate() {
            let rendered = CreatureRenderer::render_composited_frame(
                &creature.appearance,
                ActionKind::Idle,
                0,
                true,
                false,
                FaceRenderState {
                    expression,
                    eyelids: EyelidPose::Open,
                    gaze: GazeDirection::default(),
                },
            );
            blit_scaled(
                &mut pixels,
                width,
                column as u32 * cell,
                row as u32 * cell,
                &rendered.rgba_bytes(),
                SCALE,
            );
        }
    }
    write_png(&path, width, height, &pixels)?;
    println!("wrote {}", path.display());
    Ok(())
}

/// The face a gesture is reviewed with. A pose and its face have to be judged together, since
/// half of what a body says is said by where it is looking.
fn gesture_face(gesture: Gesture) -> FaceRenderState {
    let (expression, eyelids) = match gesture {
        Gesture::Cheer | Gesture::Bop => (ExpressionKind::Joy, EyelidPose::Open),
        Gesture::Gasp => (ExpressionKind::Startled, EyelidPose::Open),
        Gesture::Cover => (ExpressionKind::Worried, EyelidPose::Closed),
        Gesture::Worry | Gesture::Balance => (ExpressionKind::Worried, EyelidPose::Open),
        Gesture::Crouch | Gesture::Heave => (ExpressionKind::Determined, EyelidPose::Open),
        Gesture::Reach | Gesture::Watch => (ExpressionKind::Curious, EyelidPose::Open),
        // Shown as the desktop shows the top of it: eyes screwed shut.
        Gesture::Stretch => (ExpressionKind::Content, EyelidPose::Closed),
    };
    FaceRenderState {
        expression,
        eyelids,
        // Watching is the one pose whose whole point is that the eyes are on something, so it is
        // reviewed the way the desktop shows it: aimed out ahead and a little down, at a window.
        gaze: if gesture == Gesture::Watch {
            GazeDirection::new(1, 1)
        } else {
            GazeDirection::default()
        },
    }
}

/// Every body the art can draw: the three reference creatures — a plain modular body from each of
/// the blob, hopper, and soft-quadruped families — then the five modular body plans on one shared
/// look, so a pose is judged across every plan.
fn gesture_subjects() -> Vec<AppearanceGenome> {
    let mut subjects: Vec<_> = reference_creatures()
        .into_iter()
        .map(|creature| creature.appearance)
        .collect();
    let preview = World::preview_adult(
        [29; 32],
        OffsetDateTime::UNIX_EPOCH,
        &DesktopSnapshot::default(),
    );
    for plan in BodyPlan::ALL {
        let mut appearance = preview.appearance.clone();
        // Classic limbs make their own gestures, reviewed on the classic sheet.
        let mut design = CreatureDesign::modular([29; 32], 0, None);
        design.body = plan;
        appearance.design = Some(design);
        subjects.push(appearance);
    }
    subjects
}

/// The two poses a body pose is most easily mistaken for: standing about, and the plain peer a
/// gesture usually stands in for. Every body plan is shown holding both, right beside its
/// gestures, so "does this look like anything in particular?" can be answered by looking.
const REFERENCE_ACTIONS: [ActionKind; 2] = [ActionKind::Idle, ActionKind::InspectScreen];

/// Three bands, all at 3x so a pose is judged near the size a desktop shows it at.
///
/// First every action then every gesture, for each reference creature; then, for each modular body
/// plan, standing about and plain peering followed by every gesture, so a new pose can be
/// compared against both the nine it has to stay distinct from and the two it replaces; then one
/// band per body of the whole watching loop, frame by frame, because a held pose is only honest
/// if it keeps moving while it holds.
fn gesture_sheet(path: PathBuf) -> Result<()> {
    const COLS: u32 = 7;
    const SCALE: u32 = 3;
    let subjects = gesture_subjects();
    let references = reference_creatures().len();
    let cell = FRAME_SIZE * SCALE;
    let watch_frames = formiga_art::AnimationSpec::for_clip(Gesture::Watch).frames;
    let full_rows = ((ActionKind::ALL.len() + Gesture::ALL.len()) as u32).div_ceil(COLS);
    let gesture_rows = ((REFERENCE_ACTIONS.len() + Gesture::ALL.len()) as u32).div_ceil(COLS);
    let width = COLS * cell;
    let bands = references as u32 * full_rows
        + (subjects.len() - references) as u32 * gesture_rows
        + subjects.len() as u32 * u32::from(watch_frames).div_ceil(COLS);
    let height = bands * cell;
    let mut pixels = vec![0_u8; (width * height * 4) as usize];
    let mut row_cursor = 0_u32;
    let place = |pixels: &mut Vec<u8>, row_cursor: &mut u32, frames: Vec<formiga_art::Canvas>| {
        for (index, rendered) in frames.iter().enumerate() {
            blit_scaled(
                pixels,
                width,
                index as u32 % COLS * cell,
                (*row_cursor + index as u32 / COLS) * cell,
                &rendered.rgba_bytes(),
                SCALE,
            );
        }
        *row_cursor += (frames.len() as u32).div_ceil(COLS);
    };
    for (index, genome) in subjects.iter().enumerate() {
        let mut frames = Vec::new();
        let actions: Vec<_> = if index < references {
            ActionKind::ALL.to_vec()
        } else {
            REFERENCE_ACTIONS.to_vec()
        };
        frames.extend(actions.into_iter().enumerate().map(|(step, action)| {
            let spec = formiga_art::AnimationSpec::for_action(action);
            CreatureRenderer::render_frame(genome, action, (step as u8 + 1) % spec.frames, true)
        }));
        frames.extend(Gesture::ALL.into_iter().map(|gesture| {
            let spec = formiga_art::AnimationSpec::for_clip(gesture);
            CreatureRenderer::render_composited_frame(
                genome,
                gesture,
                1 % spec.frames,
                true,
                false,
                gesture_face(gesture),
            )
        }));
        place(&mut pixels, &mut row_cursor, frames);
    }
    for genome in &subjects {
        let mut loop_frames: Vec<_> = (0..watch_frames)
            .map(|frame| {
                CreatureRenderer::render_composited_frame(
                    genome,
                    Gesture::Watch,
                    frame,
                    true,
                    false,
                    gesture_face(Gesture::Watch),
                )
            })
            .collect();
        // The seventh column of each loop is the same pose on a mini, which is the smallest body
        // the colony grows and therefore the one a held pose has to survive being drawn at.
        let mut mini = genome.clone();
        mini.logical_size = 21;
        loop_frames.push(CreatureRenderer::render_composited_frame(
            &mini,
            Gesture::Watch,
            1,
            true,
            false,
            gesture_face(Gesture::Watch),
        ));
        place(&mut pixels, &mut row_cursor, loop_frames);
    }
    write_png(&path, width, height, &pixels)?;
    println!("wrote {}", path.display());
    Ok(())
}

fn activity_sheet(path: PathBuf) -> Result<()> {
    const COLS: u32 = 8;
    const SCALE: u32 = 3;
    const PREVIEW_FRAMES: u8 = 4;
    const ACTIONS: [ActionKind; 4] = [
        ActionKind::SoloPlay,
        ActionKind::Eat,
        ActionKind::Drink,
        ActionKind::Sprint,
    ];
    let creatures = reference_creatures();
    let cell = FRAME_SIZE * SCALE;
    let cells_per_family = ACTIONS.len() as u32 * u32::from(PREVIEW_FRAMES);
    let rows_per_family = cells_per_family.div_ceil(COLS);
    let width = COLS * cell;
    let height = creatures.len() as u32 * rows_per_family * cell;
    let mut pixels = vec![0_u8; (width * height * 4) as usize];
    for (family_index, creature) in creatures.iter().enumerate() {
        for (action_index, action) in ACTIONS.into_iter().enumerate() {
            let spec = formiga_art::AnimationSpec::for_action(action);
            for frame in 0..PREVIEW_FRAMES {
                let cell_index = action_index as u32 * u32::from(PREVIEW_FRAMES) + u32::from(frame);
                let rendered = CreatureRenderer::render_frame(
                    &creature.appearance,
                    action,
                    frame % spec.frames,
                    true,
                );
                let row = family_index as u32 * rows_per_family + cell_index / COLS;
                blit_scaled(
                    &mut pixels,
                    width,
                    cell_index % COLS * cell,
                    row * cell,
                    &rendered.rgba_bytes(),
                    SCALE,
                );
            }
        }
    }
    write_png(&path, width, height, &pixels)?;
    println!("wrote {}", path.display());
    Ok(())
}

fn ambient_sheet(path: PathBuf) -> Result<()> {
    const COLS: u32 = 8;
    const SCALE: u32 = 3;
    const CELL_ART_SIZE: u32 = 64;
    const ACTIONS: [ActionKind; 4] = [
        ActionKind::ClimbWindow,
        ActionKind::Dangle,
        ActionKind::InspectScreen,
        ActionKind::PresentDiscovery,
    ];
    let creatures = reference_creatures();
    let cell = CELL_ART_SIZE * SCALE;
    let rows_per_family = 3;
    let width = COLS * cell;
    let height = creatures.len() as u32 * rows_per_family * cell;
    let mut pixels = vec![0_u8; (width * height * 4) as usize];
    fill_gradient(
        &mut pixels,
        width,
        height,
        [18, 27, 33, 255],
        [29, 43, 47, 255],
    );

    for (family_index, creature) in creatures.iter().enumerate() {
        let family_row = family_index as u32 * rows_per_family;
        let baseline = CreatureRenderer::resting_baseline(&creature.appearance, false);
        for (action_index, action) in ACTIONS.into_iter().enumerate() {
            for frame in 0..4_u8 {
                let cell_index = action_index as u32 * 4 + u32::from(frame);
                let column = cell_index % COLS;
                let row = family_row + cell_index / COLS;
                let cell_x = column * cell;
                let cell_y = row * cell;
                let contact_y = if action == ActionKind::Dangle { 8 } else { 56 };
                draw_rect_alpha(
                    &mut pixels,
                    width,
                    cell_x + 4 * SCALE,
                    cell_y + contact_y * SCALE,
                    (CELL_ART_SIZE - 8) * SCALE,
                    SCALE,
                    [118, 155, 157, 210],
                );
                let placement = formiga_art::FramePlacement::for_action(action, baseline);
                let rendered =
                    CreatureRenderer::render_frame(&creature.appearance, action, frame, true);
                blit_scaled_square_alpha(
                    &mut pixels,
                    width,
                    cell_x + 8 * SCALE,
                    cell_y + (contact_y as i32 + placement.origin_y) as u32 * SCALE,
                    &rendered.rgba_bytes(),
                    FRAME_SIZE,
                    SCALE,
                );
            }
        }

        let trinket_row = family_row + 2;
        for variant in 0..8_u8 {
            let cell_x = u32::from(variant) * cell;
            let cell_y = trinket_row * cell;
            let trinket = CreatureRenderer::render_trinket(&creature.appearance, variant);
            blit_scaled_square_alpha(
                &mut pixels,
                width,
                cell_x + 16 * SCALE,
                cell_y + 16 * SCALE,
                &trinket.rgba_bytes(),
                16,
                6,
            );
        }
    }

    write_png(&path, width, height, &pixels)?;
    println!("wrote {}", path.display());
    Ok(())
}

/// Three reference creatures, one from each of the blob, hopper, and soft-quadruped families:
/// the first seed of each that grows into that family, on a plain modular recipe, so the review
/// sheets that compare poses keep the same three subjects whatever the generator now mixes in.
pub(crate) fn reference_creatures() -> Vec<Creature> {
    let desktop = fixture_desktop();
    [
        BodyFamily::Blob,
        BodyFamily::Hopper,
        BodyFamily::SoftQuadruped,
    ]
    .into_iter()
    .map(|family| {
        (0_u64..1_000)
            .find_map(|index| {
                let mut seed = [0_u8; 32];
                seed.copy_from_slice(&Sha256::digest(format!(
                    "formiga-reference-{family:?}-{index}"
                )));
                let mut creature = World::new(seed, OffsetDateTime::UNIX_EPOCH, &desktop)
                    .save
                    .creatures
                    .remove(0);
                // Classic parts have their own sheet; these subjects stay as they have been.
                apply_creature_design(&mut creature, Some(CreatureDesign::modular(seed, 0, None)));
                (creature.appearance.family == family).then_some(creature)
            })
            .expect("a deterministic reference seed exists for every family")
    })
    .collect()
}

fn hero_image(path: PathBuf) -> Result<()> {
    const WIDTH: u32 = 1200;
    const HEIGHT: u32 = 630;
    let mut pixels = vec![0_u8; (WIDTH * HEIGHT * 4) as usize];
    fill_gradient(
        &mut pixels,
        WIDTH,
        HEIGHT,
        [14, 23, 28, 255],
        [24, 42, 42, 255],
    );
    draw_window(&mut pixels, WIDTH, 75, 90, 640, 390, [45, 65, 72, 255]);
    draw_window(&mut pixels, WIDTH, 650, 190, 450, 325, [53, 57, 78, 255]);
    draw_rect_alpha(&mut pixels, WIDTH, 740, 380, 330, 180, [61, 185, 125, 48]);
    let creatures = demo_colony();
    for (index, (x, y, action, scale)) in [
        (360, 480, ActionKind::Idle, 4),
        (795, 190, ActionKind::Perch, 3),
        (940, 515, ActionKind::SoloPlay, 3),
        (150, 480, ActionKind::Sleep, 3),
    ]
    .into_iter()
    .enumerate()
    {
        let creature = &creatures[index.min(creatures.len() - 1)];
        let spec = formiga_art::AnimationSpec::for_action(action);
        let rendered = CreatureRenderer::render_frame(
            &creature.appearance,
            action,
            (index as u8) % spec.frames,
            true,
        );
        blit_scaled_anchor(
            &mut pixels,
            WIDTH,
            HEIGHT,
            x,
            y,
            &rendered.rgba_bytes(),
            scale,
            None,
        );
    }
    write_png(&path, WIDTH, HEIGHT, &pixels)?;
    println!("wrote {}", path.display());
    Ok(())
}

fn demo_animation(path: PathBuf) -> Result<()> {
    const WIDTH: u32 = 480;
    const HEIGHT: u32 = 270;
    const FPS: u32 = 10;
    const FRAMES: u32 = 20 * FPS;
    let file = File::create(&path).with_context(|| format!("create {}", path.display()))?;
    let mut encoder = gif::Encoder::new(BufWriter::new(file), WIDTH as u16, HEIGHT as u16, &[])?;
    encoder.set_repeat(gif::Repeat::Infinite)?;
    let creatures = demo_colony();
    for frame_index in 0..FRAMES {
        let mut pixels = vec![0_u8; (WIDTH * HEIGHT * 4) as usize];
        fill_gradient(
            &mut pixels,
            WIDTH,
            HEIGHT,
            [13, 21, 27, 255],
            [23, 38, 40, 255],
        );
        let scene = frame_index / (4 * FPS);
        let phase = (frame_index % (4 * FPS)) as f32 / (4 * FPS - 1) as f32;
        draw_window(&mut pixels, WIDTH, 22, 32, 255, 150, [43, 63, 72, 255]);
        draw_window(&mut pixels, WIDTH, 285, 72, 170, 125, [55, 57, 78, 255]);
        match scene {
            0 => {
                let x = 70 + (phase * 150.0) as u32;
                draw_demo_creature(
                    &mut pixels,
                    WIDTH,
                    HEIGHT,
                    &creatures[0],
                    ActionKind::Traverse,
                    frame_index,
                    x,
                    245,
                    3,
                    None,
                );
            }
            1 => {
                let x = (150.0 + phase * 230.0) as u32;
                let y = (240.0 - (phase * std::f32::consts::PI).sin() * 100.0) as u32;
                draw_demo_creature(
                    &mut pixels,
                    WIDTH,
                    HEIGHT,
                    &creatures[0],
                    ActionKind::Dragged,
                    frame_index,
                    x,
                    y,
                    3,
                    None,
                );
                draw_cursor(&mut pixels, WIDTH, x + 8, y.saturating_sub(52));
            }
            2 => {
                draw_rect_alpha(&mut pixels, WIDTH, 250, 160, 215, 95, [54, 204, 132, 74]);
                draw_rect_alpha(&mut pixels, WIDTH, 20, 185, 150, 70, [226, 71, 69, 70]);
                let x = (285.0 + phase * 110.0) as u32;
                draw_demo_creature(
                    &mut pixels,
                    WIDTH,
                    HEIGHT,
                    &creatures[0],
                    ActionKind::Traverse,
                    frame_index,
                    x,
                    245,
                    3,
                    None,
                );
            }
            3 => {
                let selected_window = (150, 120, 250, 115);
                draw_window(
                    &mut pixels,
                    WIDTH,
                    selected_window.0,
                    selected_window.1,
                    selected_window.2,
                    selected_window.3,
                    [71, 61, 82, 255],
                );
                let x = (125.0 + phase * 160.0) as u32;
                draw_demo_creature(
                    &mut pixels,
                    WIDTH,
                    HEIGHT,
                    &creatures[0],
                    ActionKind::Traverse,
                    frame_index,
                    x,
                    210,
                    3,
                    Some(selected_window),
                );
            }
            _ => {
                for (index, (x, y, action, scale)) in [
                    (80, 245, ActionKind::Idle, 3),
                    (235, 182, ActionKind::Perch, 2),
                    (340, 245, ActionKind::SoloPlay, 2),
                    (430, 245, ActionKind::Sleep, 2),
                ]
                .into_iter()
                .enumerate()
                {
                    draw_demo_creature(
                        &mut pixels,
                        WIDTH,
                        HEIGHT,
                        &creatures[index],
                        action,
                        frame_index,
                        x,
                        y,
                        scale,
                        None,
                    );
                }
            }
        }
        let mut gif_frame =
            gif::Frame::from_rgba_speed(WIDTH as u16, HEIGHT as u16, pixels.as_mut_slice(), 20);
        gif_frame.delay = (100 / FPS) as u16;
        encoder.write_frame(&gif_frame)?;
    }
    println!("wrote {}", path.display());
    Ok(())
}

fn demo_colony() -> Vec<Creature> {
    let desktop = fixture_desktop();
    let created = OffsetDateTime::UNIX_EPOCH;
    let mut world = World::new([42; 32], created, &desktop);
    world.tick(created + time::Duration::days(181), 0.05, &desktop);
    world.save.creatures
}

fn app_icon(directory: PathBuf, source: PathBuf) -> Result<()> {
    std::fs::create_dir_all(&directory)
        .with_context(|| format!("create {}", directory.display()))?;
    let png_path = directory.join("Formiga.png");
    let ico_path = directory.join("Formiga.ico");
    let icns_path = directory.join("Formiga.icns");

    let (source_width, source_height, source_pixels) = read_rgba_png(&source)?;
    anyhow::ensure!(
        source_width == source_height,
        "icon source must be square, got {source_width}x{source_height}"
    );
    let mac_pixels = resize_rgba_square(&source_pixels, source_width, 1024);
    write_png(&png_path, 1024, 1024, &mac_pixels)?;

    let icon_sizes = [
        (*b"ic10", 1024_u32),
        (*b"ic09", 512),
        (*b"ic08", 256),
        (*b"ic07", 128),
        (*b"icp5", 32),
        (*b"icp4", 16),
    ];
    let mut icns_chunks = Vec::new();
    for (kind, size) in icon_sizes {
        let pixels = resize_rgba_square(&source_pixels, source_width, size);
        icns_chunks.push((kind, encode_png(size, size, &pixels)?));
    }
    let icns_length = 8_usize
        + icns_chunks
            .iter()
            .map(|(_, png)| 8 + png.len())
            .sum::<usize>();
    let mut icns = BufWriter::new(
        File::create(&icns_path).with_context(|| format!("create {}", icns_path.display()))?,
    );
    icns.write_all(b"icns")?;
    icns.write_all(&(icns_length as u32).to_be_bytes())?;
    for (kind, png) in icns_chunks {
        icns.write_all(&kind)?;
        icns.write_all(&((png.len() + 8) as u32).to_be_bytes())?;
        icns.write_all(&png)?;
    }
    icns.flush()?;

    // Modern Windows icon resources can contain a PNG-compressed 256 px image. Writing the tiny
    // ICO container here keeps packaging deterministic and avoids an image-conversion dependency.
    let windows_pixels = resize_rgba_square(&source_pixels, source_width, 256);
    let png_bytes = encode_png(256, 256, &windows_pixels)?;
    let mut ico = BufWriter::new(
        File::create(&ico_path).with_context(|| format!("create {}", ico_path.display()))?,
    );
    ico.write_all(&0_u16.to_le_bytes())?; // reserved
    ico.write_all(&1_u16.to_le_bytes())?; // image
    ico.write_all(&1_u16.to_le_bytes())?; // one entry
    ico.write_all(&[0, 0, 0, 0])?; // 256x256, true color, reserved
    ico.write_all(&1_u16.to_le_bytes())?; // color planes
    ico.write_all(&32_u16.to_le_bytes())?;
    ico.write_all(&(png_bytes.len() as u32).to_le_bytes())?;
    ico.write_all(&22_u32.to_le_bytes())?;
    ico.write_all(&png_bytes)?;
    ico.flush()?;

    println!(
        "wrote {}, {}, and {}",
        png_path.display(),
        icns_path.display(),
        ico_path.display()
    );
    Ok(())
}

fn encode_png(width: u32, height: u32, pixels: &[u8]) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header()?;
        writer.write_image_data(pixels)?;
    }
    Ok(bytes)
}

fn read_rgba_png(path: &Path) -> Result<(u32, u32, Vec<u8>)> {
    let file = File::open(path).with_context(|| format!("open {}", path.display()))?;
    let mut decoder = png::Decoder::new(BufReader::new(file));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info()?;
    let size = reader
        .output_buffer_size()
        .context("decoded icon is too large")?;
    let mut pixels = vec![0; size];
    let info = reader.next_frame(&mut pixels)?;
    pixels.truncate(info.buffer_size());
    anyhow::ensure!(
        info.color_type == png::ColorType::Rgba && info.bit_depth == png::BitDepth::Eight,
        "icon source must decode to 8-bit RGBA"
    );
    Ok((info.width, info.height, pixels))
}

fn resize_rgba_square(source: &[u8], source_size: u32, target_size: u32) -> Vec<u8> {
    let mut output = vec![0; (target_size * target_size * 4) as usize];
    for y in 0..target_size {
        let source_y = y * source_size / target_size;
        for x in 0..target_size {
            let source_x = x * source_size / target_size;
            let source_index = ((source_y * source_size + source_x) * 4) as usize;
            let target_index = ((y * target_size + x) * 4) as usize;
            output[target_index..target_index + 4]
                .copy_from_slice(&source[source_index..source_index + 4]);
        }
    }
    output
}

fn shelter_sheet(path: PathBuf) -> Result<()> {
    const SCALE: u32 = 3;
    const MARGIN: u32 = 8;
    const LANE_GAP: u32 = 10;
    // One row per style, all on one ground line: the colony house plain, the same house carrying
    // every decoration it can earn, a companion's cottage hung with that companion's curtain, the
    // same cottage lit from inside after dark, the keepsake tree the colony hangs its finds on,
    // and the companion itself drawn at the very same scale — so the decoration anchors, the way
    // a cottage reads beside the one who lives in it, and the village atlas's day and night cells
    // stay reviewable in one glance.
    let footprints = [
        DwellingKind::Main.width() as u32,
        DwellingKind::Main.width() as u32,
        DwellingKind::Cottage.width() as u32,
        DwellingKind::Cottage.width() as u32,
        TREE_WIDTH as u32,
        FRAME_SIZE,
    ];
    let lane: u32 = footprints.iter().sum::<u32>() + LANE_GAP * (footprints.len() as u32 - 1);
    let width = (lane + MARGIN * 2) * SCALE;
    let row_height = SHELTER_SIZE * SCALE + MARGIN * SCALE;
    let height = row_height * 4 + MARGIN * SCALE;
    let mut pixels = vec![0_u8; (width * height * 4) as usize];
    fill_gradient(
        &mut pixels,
        width,
        height,
        [18, 29, 34, 255],
        [35, 55, 54, 255],
    );
    for style in 0..4_u32 {
        let mut seed = [0_u8; 32];
        seed.copy_from_slice(&Sha256::digest(format!("formiga-shelter-{style}")));
        seed[1] = style as u8;
        let home = ColonyHome::from_seed(seed, None, None, None);
        let resident = World::preview_adult(
            SeedStream::new(seed).bytes("shelter-sheet-resident", 0),
            OffsetDateTime::UNIX_EPOCH,
            &fixture_desktop(),
        );
        let marks = [None, Some(formiga_art::ResidentMark::of(&resident))];
        let village = ShelterRenderer::render_village(&home.shelter, &[], &marks, &[], true);
        let cell = |cell: formiga_art::VillageCell| {
            let (x, y) = ShelterRenderer::village_cell(cell);
            let mut tile = formiga_art::Canvas::new(SHELTER_SIZE, SHELTER_SIZE);
            for ty in 0..SHELTER_SIZE as i32 {
                for tx in 0..SHELTER_SIZE as i32 {
                    tile.set(tx, ty, village.get(x as i32 + tx, y as i32 + ty));
                }
            }
            tile
        };
        let cottage = |lit| formiga_art::VillageCell::House { slot: 1, lit };
        let lane_items = [
            (ShelterRenderer::render(&home.shelter), SHELTER_SIZE),
            (
                ShelterRenderer::render_with_decorations(
                    &home.shelter,
                    &formiga_core::ShelterDecorationKind::ALL,
                ),
                SHELTER_SIZE,
            ),
            (cell(cottage(false)), SHELTER_SIZE),
            (cell(cottage(true)), SHELTER_SIZE),
            (cell(formiga_art::VillageCell::Tree), SHELTER_SIZE),
            (
                CreatureRenderer::render_frame(
                    &resident.appearance,
                    ActionKind::Homebound,
                    0,
                    true,
                ),
                FRAME_SIZE,
            ),
        ];
        // Every dwelling cell stands on row 61 of its own 64px cell; a creature frame stands on
        // its own last row. Lining those two up puts everything on one ground line.
        let baseline = style * row_height + (MARGIN + SHELTER_SIZE - 3) * SCALE;
        let mut pen = MARGIN;
        for ((canvas, size), footprint) in lane_items.into_iter().zip(footprints) {
            let centre = pen + footprint / 2;
            let foot = if size == FRAME_SIZE { 0 } else { 3 };
            blit_scaled_square_alpha(
                &mut pixels,
                width,
                (centre.saturating_sub(size / 2)) * SCALE,
                baseline + foot * SCALE - size * SCALE,
                &canvas.rgba_bytes(),
                size,
                SCALE,
            );
            pen = centre + footprint / 2 + LANE_GAP;
        }
    }
    write_png(&path, width, height, &pixels)?;
    println!("wrote {}", path.display());
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn blit_scaled_square_alpha(
    target: &mut [u8],
    target_width: u32,
    origin_x: u32,
    origin_y: u32,
    source: &[u8],
    source_size: u32,
    scale: u32,
) {
    for sy in 0..source_size {
        for sx in 0..source_size {
            let source_index = ((sy * source_size + sx) * 4) as usize;
            let color = [
                source[source_index],
                source[source_index + 1],
                source[source_index + 2],
                source[source_index + 3],
            ];
            if color[3] == 0 {
                continue;
            }
            for oy in 0..scale {
                for ox in 0..scale {
                    blend_pixel(
                        target,
                        target_width,
                        origin_x + sx * scale + ox,
                        origin_y + sy * scale + oy,
                        color,
                    );
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_demo_creature(
    target: &mut [u8],
    width: u32,
    height: u32,
    creature: &Creature,
    action: ActionKind,
    frame_index: u32,
    x: u32,
    y: u32,
    scale: u32,
    clip: Option<(u32, u32, u32, u32)>,
) {
    let spec = formiga_art::AnimationSpec::for_action(action);
    let rendered = CreatureRenderer::render_frame(
        &creature.appearance,
        action,
        (frame_index as u8) % spec.frames,
        true,
    );
    blit_scaled_anchor(
        target,
        width,
        height,
        x,
        y,
        &rendered.rgba_bytes(),
        scale,
        clip,
    );
}

fn simulate(days: i64) -> Result<()> {
    if days < 0 {
        bail!("days must be non-negative");
    }
    let desktop = fixture_desktop();
    let created = OffsetDateTime::UNIX_EPOCH;
    let mut world = World::new([17; 32], created, &desktop);
    for day in 0..=days {
        let now = created + time::Duration::days(day);
        for _ in 0..1_200 {
            world.tick(now, 0.05, &desktop);
            world.drain_events().for_each(drop);
        }
    }
    println!(
        "simulated {days} days; colony contains {} creature(s)",
        world.save.creatures.len()
    );
    for creature in &world.save.creatures {
        // Report the modular body plan when there is one; legacy creatures report their family.
        let shape = creature.appearance.design.map_or_else(
            || format!("{:?}", creature.appearance.family),
            |design| format!("{:?}", design.body),
        );
        println!(
            "- {}: {shape}, {:?}, generation {}, {:?}",
            creature.id, creature.role, creature.generation, creature.state.action
        );
    }
    println!(
        "village: colony house plus {:?}",
        colony_cottages(&world.save.creatures)
    );
    Ok(())
}

pub(crate) fn fixture_desktop() -> DesktopSnapshot {
    DesktopSnapshot {
        monitors: vec![MonitorInfo {
            id: 1,
            display_key: DisplayKey([1; 16]),
            bounds: DesktopRect {
                x: 0.0,
                y: 0.0,
                width: 1440.0,
                height: 900.0,
            },
            usable_bounds: DesktopRect {
                x: 0.0,
                y: 24.0,
                width: 1440.0,
                height: 826.0,
            },
            scale_factor: 2.0,
            primary: true,
        }],
        cursor: CursorSnapshot {
            position: Point { x: 720.0, y: 420.0 },
            velocity: Point::default(),
            available: true,
        },
        ..DesktopSnapshot::default()
    }
}

fn blit_scaled(
    target: &mut [u8],
    target_width: u32,
    origin_x: u32,
    origin_y: u32,
    source: &[u8],
    scale: u32,
) {
    blit_scaled_square(
        target,
        target_width,
        origin_x,
        origin_y,
        source,
        FRAME_SIZE,
        scale,
    );
}

#[allow(clippy::too_many_arguments)]
fn blit_scaled_square(
    target: &mut [u8],
    target_width: u32,
    origin_x: u32,
    origin_y: u32,
    source: &[u8],
    source_size: u32,
    scale: u32,
) {
    for sy in 0..source_size {
        for sx in 0..source_size {
            let source_index = ((sy * source_size + sx) * 4) as usize;
            for oy in 0..scale {
                for ox in 0..scale {
                    let x = origin_x + sx * scale + ox;
                    let y = origin_y + sy * scale + oy;
                    let target_index = ((y * target_width + x) * 4) as usize;
                    target[target_index..target_index + 4]
                        .copy_from_slice(&source[source_index..source_index + 4]);
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn blit_scaled_anchor(
    target: &mut [u8],
    target_width: u32,
    target_height: u32,
    anchor_x: u32,
    anchor_y: u32,
    source: &[u8],
    scale: u32,
    clip: Option<(u32, u32, u32, u32)>,
) {
    let size = FRAME_SIZE * scale;
    let origin_x = anchor_x as i32 - size as i32 / 2;
    let origin_y = anchor_y as i32 - size as i32;
    for sy in 0..FRAME_SIZE {
        for sx in 0..FRAME_SIZE {
            let source_index = ((sy * FRAME_SIZE + sx) * 4) as usize;
            let color = [
                source[source_index],
                source[source_index + 1],
                source[source_index + 2],
                source[source_index + 3],
            ];
            if color[3] == 0 {
                continue;
            }
            for oy in 0..scale {
                for ox in 0..scale {
                    let x = origin_x + (sx * scale + ox) as i32;
                    let y = origin_y + (sy * scale + oy) as i32;
                    if x < 0 || y < 0 || x >= target_width as i32 || y >= target_height as i32 {
                        continue;
                    }
                    let x = x as u32;
                    let y = y as u32;
                    if clip.is_some_and(|(cx, cy, width, height)| {
                        x >= cx && x < cx + width && y >= cy && y < cy + height
                    }) {
                        continue;
                    }
                    blend_pixel(target, target_width, x, y, color);
                }
            }
        }
    }
}

fn fill_gradient(target: &mut [u8], width: u32, height: u32, top: [u8; 4], bottom: [u8; 4]) {
    for y in 0..height {
        let mix = y as f32 / height.max(1) as f32;
        let color = [
            (top[0] as f32 * (1.0 - mix) + bottom[0] as f32 * mix) as u8,
            (top[1] as f32 * (1.0 - mix) + bottom[1] as f32 * mix) as u8,
            (top[2] as f32 * (1.0 - mix) + bottom[2] as f32 * mix) as u8,
            255,
        ];
        for x in 0..width {
            let index = ((y * width + x) * 4) as usize;
            target[index..index + 4].copy_from_slice(&color);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_window(
    target: &mut [u8],
    target_width: u32,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    body: [u8; 4],
) {
    draw_rect_alpha(
        target,
        target_width,
        x + 8,
        y + 10,
        width,
        height,
        [0, 0, 0, 70],
    );
    draw_rect_alpha(target, target_width, x, y, width, height, body);
    draw_rect_alpha(
        target,
        target_width,
        x,
        y,
        width,
        24.min(height),
        [25, 36, 42, 255],
    );
    for (offset, color) in [
        (10, [233, 108, 101, 255]),
        (26, [235, 190, 91, 255]),
        (42, [102, 194, 128, 255]),
    ] {
        draw_rect_alpha(target, target_width, x + offset, y + 8, 7, 7, color);
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_rect_alpha(
    target: &mut [u8],
    target_width: u32,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    color: [u8; 4],
) {
    let target_height = (target.len() as u32 / 4) / target_width;
    for py in y..y.saturating_add(height).min(target_height) {
        for px in x..x.saturating_add(width).min(target_width) {
            blend_pixel(target, target_width, px, py, color);
        }
    }
}

fn draw_cursor(target: &mut [u8], target_width: u32, x: u32, y: u32) {
    for row in 0..18_u32 {
        for column in 0..=(row / 2) {
            blend_pixel(
                target,
                target_width,
                x + column,
                y + row,
                [248, 250, 247, 255],
            );
        }
    }
    draw_rect_alpha(
        target,
        target_width,
        x + 5,
        y + 13,
        5,
        10,
        [248, 250, 247, 255],
    );
}

fn fill_rect(
    target: &mut [u8],
    target_width: u32,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    color: [u8; 4],
) {
    for row in y..y + height {
        for column in x..x + width {
            if column >= target_width {
                continue;
            }
            blend_pixel(target, target_width, column, row, color);
        }
    }
}

pub(crate) fn blend_pixel(target: &mut [u8], width: u32, x: u32, y: u32, source: [u8; 4]) {
    let index = ((y * width + x) * 4) as usize;
    if index + 4 > target.len() {
        return;
    }
    let alpha = source[3] as f32 / 255.0;
    for channel in 0..3 {
        target[index + channel] =
            (source[channel] as f32 * alpha + target[index + channel] as f32 * (1.0 - alpha)) as u8;
    }
    target[index + 3] = 255;
}

pub(crate) fn write_png(path: &Path, width: u32, height: u32, pixels: &[u8]) -> Result<()> {
    let file = File::create(path).with_context(|| format!("create {}", path.display()))?;
    let mut encoder = png::Encoder::new(BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(pixels)?;
    Ok(())
}
