//! The village laid out on its scenery, from a colony actually living on it: the pond with a full
//! colony of six walked home, the same colony a little later with somebody hand over hand on a
//! cliff, and the pond's walk map drawn over the picture.

use crate::{blit_canvas_scaled, draw_rect_alpha, fill_rect, write_png};
use anyhow::Result;
use formiga_art::{Canvas, CreatureRenderer, FRAME_SIZE, SHELTER_SIZE, ShelterRenderer};
use formiga_core::*;
use std::path::PathBuf;
use time::OffsetDateTime;

/// One display just big enough for the pond at one point per shelter pixel, drawn twice over.
const DISPLAY: (u32, u32) = (800, 500);
const SCALE: u32 = 2;
const GAP: u32 = 16;

/// The house types the picture was drawn with first — a tent, a mushroom and a pillow house —
/// and then the rest.
const STYLES: [ShelterStyle; MAX_COLONY_CREATURES] = [
    ShelterStyle::Tent,
    ShelterStyle::Mushroom,
    ShelterStyle::PillowFort,
    ShelterStyle::LeafHouse,
    ShelterStyle::Tent,
    ShelterStyle::Mushroom,
];

pub fn scenery_sheet(path: PathBuf) -> Result<()> {
    let desktop = desktop();
    let mut world = colony_on_the_pond(&desktop);
    let settled = world.save.creatures.clone();
    // A little later, as soon as somebody is on one of the cliffs.
    let mut now = OffsetDateTime::UNIX_EPOCH;
    for _ in 0..12_000 {
        now += time::Duration::milliseconds(50);
        world.tick(now, 0.05, &desktop);
        world.drain_events().for_each(drop);
        if world
            .save
            .creatures
            .iter()
            .any(|creature| creature.state.action == ActionKind::ClimbWindow)
        {
            break;
        }
    }
    let later = world.save.creatures.clone();

    let (panel_width, panel_height) = (DISPLAY.0 * SCALE, DISPLAY.1 * SCALE);
    let width = panel_width + GAP * 2;
    let height = (panel_height + GAP) * 3 + GAP;
    let mut pixels = vec![0u8; (width * height * 4) as usize];
    fill_rect(
        &mut pixels,
        width,
        0,
        0,
        width,
        height,
        [222, 226, 220, 255],
    );
    for (row, creatures) in [Some(&settled), Some(&later), None].into_iter().enumerate() {
        let (x, y) = (GAP, GAP + row as u32 * (panel_height + GAP));
        // The rest of the display the corner is in.
        fill_rect(
            &mut pixels,
            width,
            x,
            y,
            panel_width,
            panel_height,
            [244, 246, 250, 255],
        );
        let panel = draw_panel(&world, &desktop, creatures.map(Vec::as_slice));
        blit_canvas_scaled(&mut pixels, width, x, y, &panel, SCALE);
        if creatures.is_none() {
            draw_walk_map(&mut pixels, width, x, y, &world, &desktop);
        }
    }
    write_png(&path, width, height, &pixels)?;
    println!("wrote {}", path.display());
    Ok(())
}

fn desktop() -> DesktopSnapshot {
    let bounds = DesktopRect {
        x: 0.0,
        y: 0.0,
        width: DISPLAY.0 as f32,
        height: DISPLAY.1 as f32,
    };
    DesktopSnapshot {
        monitors: vec![MonitorInfo {
            id: 1,
            display_key: DisplayKey([1; 16]),
            bounds,
            usable_bounds: bounds,
            scale_factor: 1.0,
            primary: true,
        }],
        ..DesktopSnapshot::default()
    }
}

/// Six companions, each a different one, laid out on the pond at one point to the pixel and
/// walked home from wherever they started.
fn colony_on_the_pond(desktop: &DesktopSnapshot) -> World {
    let created = OffsetDateTime::UNIX_EPOCH;
    let seed = [61; 32];
    let mut world = World::new(seed, created, desktop);
    world.save.settings.display_scale = 1;
    world.save.home.corner = HomeCorner::BottomLeft;
    world.save.home.scenery = Some(VillageScenery::Pond);
    while world.save.creatures.len() < MAX_COLONY_CREATURES {
        let order = world.save.creatures.len() as u8;
        let mut companion = World::preview_adult(
            SeedStream::new(seed).bytes("pond-resident", u64::from(order)),
            created,
            desktop,
        );
        companion.id = 200 + u64::from(order);
        companion.colony_order = order;
        companion.generation = order;
        companion.state.position = Point {
            x: 60.0 + f32::from(order) * 120.0,
            y: DISPLAY.1 as f32 - 4.0,
        };
        world.save.creatures.push(companion);
    }
    let mut now = created;
    for _ in 0..6_000 {
        world.tick(now, 0.05, desktop);
        world.drain_events().for_each(drop);
        if world
            .save
            .creatures
            .iter()
            .all(|creature| world.resting_at_home(creature.id))
        {
            break;
        }
        now += time::Duration::milliseconds(50);
    }
    world
}

fn placement(world: &World, desktop: &DesktopSnapshot) -> Option<SceneryPlacement> {
    home_commons(
        &world.save.home,
        colony_cottage_list(&world.save.creatures).as_slice(),
        &desktop.monitors,
        &world.save.settings.habitat,
        world.save.settings.display_scale,
    )
    .and_then(|commons| commons.scenery)
}

/// The display at one point to the pixel: the picture, the houses and trees on their spots with
/// sixteen finds hung in the trees, the belongings in the yards, and — when there are any —
/// the companions where they stand, each in the clip it is showing.
fn draw_panel(world: &World, desktop: &DesktopSnapshot, creatures: Option<&[Creature]>) -> Canvas {
    let mut canvas = Canvas::new(DISPLAY.0, DISPLAY.1);
    let home = &world.save.home;
    let policy = &world.save.settings.habitat;
    let monitors = &desktop.monitors;
    let cottages = colony_cottage_list(&world.save.creatures);
    let cottages = cottages.as_slice();
    let paste = |canvas: &mut Canvas, source: &Canvas, x: i32, y: i32, mirror: bool| {
        for row in 0..source.height() as i32 {
            for column in 0..source.width() as i32 {
                let read = if mirror {
                    source.width() as i32 - 1 - column
                } else {
                    column
                };
                let pixel = source.get(read, row);
                if pixel.a > 0 {
                    canvas.set(x + column, y + row, pixel);
                }
            }
        }
    };
    let cell = |atlas: &Canvas, (cx, cy): (u32, u32), size: u32| {
        let mut cell = Canvas::new(size, size);
        for y in 0..size as i32 {
            for x in 0..size as i32 {
                cell.set(x, y, atlas.get(cx as i32 + x, cy as i32 + y));
            }
        }
        cell
    };
    let standing = |size: u32, point: Point| {
        (
            point.x.round() as i32 - size as i32 / 2,
            point.y.round() as i32 - size as i32,
        )
    };

    if let Some(scenery) = placement(world, desktop) {
        let picture = formiga_art::render_scenery(scenery.scenery);
        paste(
            &mut canvas,
            &picture,
            scenery.origin.x.round() as i32,
            scenery.origin.y.round() as i32,
            false,
        );
    }

    let marks: Vec<Option<formiga_art::ResidentMark>> = world
        .save
        .creatures
        .iter()
        .map(|resident| Some(formiga_art::ResidentMark::of(resident)))
        .collect();
    let village =
        ShelterRenderer::render_village(&home.drawn_shelter(), &[], &marks, &STYLES, false);
    for slot in 0..=cottages.len() {
        let Some((_, point)) = home_dwelling_position(home, slot, cottages, monitors, policy, 1)
        else {
            continue;
        };
        let house = cell(
            &village,
            ShelterRenderer::village_cell(formiga_art::VillageCell::House {
                slot,
                lit: false,
                occupied: false,
            }),
            SHELTER_SIZE,
        );
        let (x, y) = standing(SHELTER_SIZE, point);
        paste(&mut canvas, &house, x, y, false);
    }

    let members: Vec<formiga_art::Palette> = world
        .save
        .creatures
        .iter()
        .map(|creature| formiga_art::palette_for(&creature.appearance))
        .collect();
    let trinkets = formiga_art::TrinketAtlasRenderer::render(world.save.colony_seed, &members);
    let tree = cell(
        &village,
        ShelterRenderer::village_cell(formiga_art::VillageCell::Tree),
        SHELTER_SIZE,
    );
    for end in TreeEnd::BOTH {
        let Some((_, point)) = home_tree_position(home, end, cottages, monitors, policy, 1) else {
            continue;
        };
        let (left, top) = standing(SHELTER_SIZE, point);
        paste(&mut canvas, &tree, left, top, end == TreeEnd::Inward);
        for variant in 0..TREE_HOOKS as u8 {
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
            let quad = cell(&trinkets, (sx, sy), formiga_art::TRINKET_CELL);
            let half = formiga_art::TRINKET_CELL as i32 / 2;
            let (inset_x, inset_y) = formiga_art::TREE_INSET;
            paste(
                &mut canvas,
                &quad,
                left + inset_x + anchor.x - half,
                top + inset_y + anchor.y - half,
                false,
            );
        }
    }

    let objects = formiga_art::ColonyObjectRenderer::render_atlas(world.save.colony_seed);
    for (slot, place) in home_object_positions(home, cottages, monitors, policy, 1)
        .iter()
        .enumerate()
    {
        let Some((_, point)) = place else {
            continue;
        };
        let tile = cell(
            &objects,
            formiga_art::ColonyObjectRenderer::cell_origin(slot as u32),
            16,
        );
        let (x, y) = standing(16, *point);
        paste(&mut canvas, &tile, x, y, false);
    }

    for creature in creatures.into_iter().flatten() {
        if creature.state.indoors {
            continue;
        }
        let frame = CreatureRenderer::render_frame(
            &creature.appearance,
            creature.state.action,
            0,
            creature.state.facing_right,
        );
        let (x, y) = standing(FRAME_SIZE, creature.state.position);
        paste(&mut canvas, &frame, x, y, false);
    }
    canvas
}

/// The walk map over the picture: walked runs in red, climbs in magenta, every point on it, and
/// the trail along the top of the picture's levels in a heavier line.
fn draw_walk_map(
    pixels: &mut [u8],
    width: u32,
    base_x: u32,
    base_y: u32,
    world: &World,
    desktop: &DesktopSnapshot,
) {
    let Some(scenery) = placement(world, desktop) else {
        return;
    };
    let map = scenery.map();
    let dot = |pixels: &mut [u8], point: Point, size: u32, colour: [u8; 4]| {
        let (x, y) = (
            base_x as f32 + point.x * SCALE as f32,
            base_y as f32 + point.y * SCALE as f32,
        );
        draw_rect_alpha(
            pixels,
            width,
            (x - size as f32 / 2.0).max(0.0) as u32,
            (y - size as f32 / 2.0).max(0.0) as u32,
            size,
            size,
            colour,
        );
    };
    let on_trail = |a: u8, b: u8| {
        map.trail()
            .windows(2)
            .any(|pair| (pair[0], pair[1]) == (a, b) || (pair[1], pair[0]) == (a, b))
    };
    for &(a, b, footing) in map.edges() {
        let (from, to) = (
            scenery.point(map.nodes()[usize::from(a)]),
            scenery.point(map.nodes()[usize::from(b)]),
        );
        let colour = match footing {
            Footing::Walk => [220, 40, 40, 230],
            Footing::Climb => [220, 0, 220, 230],
        };
        let thick = if on_trail(a, b) { 6 } else { 3 };
        let steps = (from.distance(to) * 2.0).ceil().max(1.0) as u32;
        for step in 0..=steps {
            let t = step as f32 / steps as f32;
            let point = Point {
                x: from.x + (to.x - from.x) * t,
                y: from.y + (to.y - from.y) * t,
            };
            dot(pixels, point, thick, colour);
        }
    }
    for &node in map.nodes() {
        dot(pixels, scenery.point(node), 9, [250, 210, 0, 255]);
    }
    for slot in 0..MAX_COLONY_CREATURES {
        if let Some(house) = scenery.house(slot) {
            dot(pixels, house, 14, [30, 60, 220, 255]);
        }
    }
    for end in TreeEnd::BOTH {
        dot(pixels, scenery.tree(end), 14, [20, 150, 40, 255]);
    }
}
