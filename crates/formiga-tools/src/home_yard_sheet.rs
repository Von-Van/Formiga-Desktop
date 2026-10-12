//! The village as the layout functions place it, panel by panel: every house style, colonies of
//! one to six, the trees filling with finds, hangout spots, gardens, palettes, and after dark.

use crate::{blit_scaled_square_alpha, fill_gradient, fill_rect, fixture_desktop, write_png};
use anyhow::Result;
use formiga_art::{CreatureRenderer, FRAME_SIZE, SHELTER_SIZE, ShelterRenderer};
use formiga_core::*;
use std::path::PathBuf;
use time::OffsetDateTime;

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

/// Shelter pixels of ground each panel shows, how tall it is, and the blit scale: a village cell
/// tall and a little more, and a little wider than the widest village there is.
const YARD_STRIP: u32 = 600;
const YARD_HEIGHT: u32 = SHELTER_SIZE + 8;
const YARD_SCALE: u32 = 2;

pub fn home_yard_sheet(path: PathBuf) -> Result<()> {
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
                y + panel_height - (YARD_HEIGHT - SHELTER_SIZE) * YARD_SCALE,
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
        home.set_garden(kind, along, OffsetDateTime::UNIX_EPOCH);
    }
    // Grown, as a village's gardens always looked on this sheet.
    for patch in &mut home.gardens {
        patch.planted_at_utc =
            Some(OffsetDateTime::UNIX_EPOCH - time::Duration::hours(patch.kind.stage_hours() * 2));
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
    // The colony house wearing the first six decorations, as it always has on this sheet.
    let village = ShelterRenderer::render_village(
        &home.drawn_shelter(),
        &[ShelterDecorationKind::ALL[..6].to_vec()],
        &marks,
        &panel.styles,
        true,
    );

    // Everything is blitted by the top-left of its own square, so a quad hung in a tree lands
    // by its anchor and a house lands by its footprint's middle on the ground line.
    let mut corner_of = |canvas: &formiga_art::Canvas, size: u32, x: i32, y: i32| {
        // Whatever reaches past the panel's left or top edge is cut off there, the way the
        // display's own edge cuts off the empty margin of a tree's cell standing close to it.
        let (skip_x, skip_y) = ((-x).max(0), (-y).max(0));
        let mut visible = formiga_art::Canvas::new(size, size);
        for row in 0..size as i32 - skip_y {
            for column in 0..size as i32 - skip_x {
                visible.set(column, row, canvas.get(column + skip_x, row + skip_y));
            }
        }
        blit_scaled_square_alpha(
            pixels,
            width,
            base_x + (x + skip_x) as u32 * YARD_SCALE,
            base_y + (y + skip_y) as u32 * YARD_SCALE,
            &visible.rgba_bytes(),
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

    for slot in 0..=cottages.len() {
        let Some((_, p)) = home_dwelling_position(&home, slot, cottages, monitors, &policy, 1)
        else {
            continue;
        };
        let (cell_x, cell_y) = ShelterRenderer::village_cell(formiga_art::VillageCell::House {
            slot,
            lit: panel.night,
            occupied: false,
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

    // The two trees in front of the houses they reach in over, and then whatever the scrapbook
    // holds hung between them: one 16x16 quad from the colony's own trinket sheet at the anchor
    // for its slot, exactly as the overlay draws them. The inward tree is the same atlas cell
    // read the other way round.
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
        // With nothing chosen, the first sixteen finds each hang on the hook numbered after them.
        for variant in 0..panel.found as u8 {
            let Some((hangs_in, anchor)) = formiga_art::hook_place(usize::from(variant)) else {
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
            let (inset_x, inset_y) = formiga_art::TREE_INSET;
            corner_of(
                &quad,
                formiga_art::TRINKET_CELL,
                left + inset_x + anchor.x - half,
                top + inset_y + anchor.y - half,
            );
        }
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
        let (u, v) = formiga_art::ColonyObjectRenderer::cell_origin(slot as u32);
        let mut tile = formiga_art::Canvas::new(16, 16);
        for y in 0..16 {
            for x in 0..16 {
                tile.set(x, y, atlas.get(u as i32 + x, v as i32 + y));
            }
        }
        let (x, y) = standing(16, p);
        corner_of(&tile, 16, x, y);
    }

    // The spots put down and the patches planted on the ground, the lookout turned out over the
    // rest of the display.
    let middle = monitor.usable_bounds.x + monitor.usable_bounds.width / 2.0;
    for (item, _, p) in home_ground_positions(&home, cottages, monitors, &policy, 1) {
        let cell = formiga_art::ColonyObjectRenderer::ground_cell(item, GardenStage::Grown);
        let (u, v) = formiga_art::ColonyObjectRenderer::cell_origin(cell);
        let mirrored = formiga_art::ColonyObjectRenderer::ground_mirrored(item, p.x, middle);
        let mut tile = formiga_art::Canvas::new(16, 16);
        for y in 0..16 {
            for x in 0..16 {
                let read = if mirrored { 15 - x } else { x };
                tile.set(x, y, atlas.get(u as i32 + read, v as i32 + y));
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
