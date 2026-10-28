use super::home::settled_colony;
use super::*;
use crate::world::village_life::{Plan, VillageActivity};
use crate::world::village_paths::{Door, PATH_DEPTH, VillagePaths};

/// Every house's door along the village, and how far back the house stands.
fn doors(world: &World, desktop: &DesktopSnapshot) -> Vec<(f32, f32)> {
    let cottages = colony_cottage_list(&world.save.creatures);
    let depths = world.save.home.house_depth_list(&world.save.creatures);
    (0..=cottages.as_slice().len())
        .filter_map(|slot| {
            let (_, point) = home_dwelling_position(
                &world.save.home,
                slot,
                cottages.as_slice(),
                &desktop.monitors,
                &world.save.settings.habitat,
                world.save.settings.display_scale,
            )?;
            Some((point.x, f32::from(depths[slot])))
        })
        .collect()
}

/// A long afternoon at home: residents go up onto the trodden path and onto doorsteps set back
/// from the front, and come back down, and never change how far back they stand anywhere but on
/// a path running back at least as far as where they are going — which runs in front of its own
/// house and nobody else's, so nobody is ever drawn walking through a wall.
#[test]
fn residents_go_into_the_village_only_by_its_paths() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    let mut world = settled_colony([88; 32], 4, created, &desktop);
    world.wonders.hold_off();
    let doors = doors(&world, &desktop);
    let mut before: Vec<(CreatureId, f32, f32)> = world
        .save
        .creatures
        .iter()
        .map(|creature| (creature.id, creature.state.position.x, creature.state.depth))
        .collect();
    let (mut on_path, mut on_doorstep, mut at_front) = (0, 0, 0);
    for _ in 0..40_000 {
        world.tick(created, 0.05, &desktop);
        world.save.home.active_since_utc = Some(created);
        for creature in &world.save.creatures {
            let depth = creature.state.depth;
            assert!(
                (0.0..=f32::from(MAX_HOUSE_DEPTH)).contains(&depth),
                "{depth} back is off the grass"
            );
            let (_, x, was) = *before.iter().find(|(id, _, _)| *id == creature.id).unwrap();
            if (depth - was).abs() > 1e-4 && !creature.state.indoors {
                let reach = depth.max(was) - 0.02;
                assert!(
                    doors.iter().any(|(door, back)| (door - x).abs() <= 0.6
                        && (door - creature.state.position.x).abs() <= 0.6
                        && *back >= reach),
                    "went from {was} to {depth} back at {x}, on no path that far back"
                );
            }
            if (depth - PATH_DEPTH).abs() < 0.01 {
                on_path += 1;
            } else if depth >= 3.0 && doors.iter().any(|(_, back)| (back - depth).abs() < 0.01) {
                on_doorstep += 1;
            } else if depth == 0.0 {
                at_front += 1;
            }
        }
        before = world
            .save
            .creatures
            .iter()
            .map(|creature| (creature.id, creature.state.position.x, creature.state.depth))
            .collect();
    }
    eprintln!("front {at_front}, path {on_path}, doorsteps {on_doorstep}");
    assert!(on_path > 0, "nobody walked the path along the middle");
    assert!(on_doorstep > 0, "nobody went up to a doorstep");
    // The front of the grass is still where the village mostly lives.
    assert!(
        at_front > (on_path + on_doorstep) * 2,
        "front {at_front}, path {on_path}, doorsteps {on_doorstep}"
    );
}

/// Going indoors, a resident whose house is set back walks up the path to its door before it
/// goes in, and comes out onto its doorstep and back down the same path.
#[test]
fn a_resident_goes_up_its_own_path_to_go_indoors() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    let mut world = settled_colony([89; 32], 2, created, &desktop);
    let doors = doors(&world, &desktop);
    let (door_x, back) = doors[1];
    assert!(back >= 3.0, "the cottage stands at the front");
    let id = world.save.creatures[1].id;
    let ground = world.save.creatures[1].state.position.y;
    world.village_life.insert(
        id,
        VillageActivity::new(Plan::Indoors {
            slot: 1,
            door: Point {
                x: door_x,
                y: ground,
            },
            depth: back,
            stay: 2.0,
            nap: false,
        }),
    );
    let mut went_in_from = None;
    let mut came_down = false;
    for _ in 0..1_200 {
        world.tick(created, 0.05, &desktop);
        world.save.home.active_since_utc = Some(created);
        let creature = &world.save.creatures[1];
        if creature.state.indoors && went_in_from.is_none() {
            went_in_from = Some((creature.state.position.x, creature.state.depth));
        }
        if went_in_from.is_some() && !creature.state.indoors && creature.state.depth == 0.0 {
            came_down = true;
            break;
        }
    }
    assert_eq!(went_in_from, Some((door_x, back)), "went in at its door");
    assert!(came_down, "came back down to the front");
}

/// Whoever is standing back in the village when the visit ends steps forward to the front of
/// the grass as it sets off, and nobody is left standing back out on the desktop.
#[test]
fn everybody_steps_forward_when_the_visit_ends() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    let mut world = settled_colony([90; 32], 3, created, &desktop);
    world.save.creatures[1].state.depth = PATH_DEPTH;
    world.dismiss_home(created, false);
    for _ in 0..40 {
        world.tick(created, 0.05, &desktop);
    }
    assert!(
        world
            .save
            .creatures
            .iter()
            .all(|creature| creature.state.depth == 0.0)
    );
}

/// Picked up from the path along the middle, a companion stays where it was drawn, under the
/// hand that took hold of it, and is carried at the front.
#[test]
fn picked_up_from_the_path_it_stays_under_the_hand() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    let mut world = settled_colony([91; 32], 3, created, &desktop);
    let index = 1;
    world.save.creatures[index].state.depth = PATH_DEPTH;
    let points_per_pixel = f32::from(world.save.settings.display_scale) / 2.0;
    let creature = &world.save.creatures[index];
    let drawn = creature.state.drawn_position(points_per_pixel);
    let creature_id = creature.id;
    assert!(world.handle_command(
        WorldCommand::BeginInteraction {
            creature_id,
            cursor: drawn,
        },
        &desktop,
    ));
    let held = Point {
        x: drawn.x + 40.0,
        y: drawn.y - 30.0,
    };
    assert!(world.handle_command(
        WorldCommand::UpdateInteraction {
            cursor: held,
            velocity: Point::default(),
        },
        &desktop,
    ));
    let creature = &world.save.creatures[index];
    assert_eq!(creature.state.action, ActionKind::Dragged);
    assert_eq!(creature.state.depth, 0.0);
    assert!(creature.state.position.distance(held) < 0.01);
}

/// The way from one depth to another is by the nearest path that runs back far enough, and
/// nobody is out of sight but behind a house nearer the front.
#[test]
fn the_way_in_is_by_a_path_far_enough_back() {
    let door = |x: f32, depth: f32| Door {
        slot: 0,
        x,
        half: 30.0,
        depth,
    };
    let paths = VillagePaths::new(
        vec![
            door(100.0, 0.0),
            door(170.0, 7.0),
            door(240.0, 2.0),
            door(310.0, 8.0),
        ],
        2.0,
    );
    let at = Point { x: 230.0, y: 800.0 };
    let goal = Point { x: 250.0, y: 800.0 };
    // Onto the path: by the door at 170, the nearest that runs back past it.
    let way = paths.toward(at, 0.0, goal, PATH_DEPTH);
    assert_eq!((way.point.x, way.depth), (170.0, 0.0));
    let foot = Point { x: 170.0, y: 800.0 };
    let way = paths.toward(foot, 0.0, goal, PATH_DEPTH);
    assert_eq!((way.point.x, way.depth), (170.0, PATH_DEPTH));
    // Along it, as far back as it is.
    let up = paths.toward(foot, PATH_DEPTH, goal, PATH_DEPTH);
    assert_eq!((up.point, up.depth), (goal, PATH_DEPTH));
    // Up to the furthest doorstep only by its own path.
    let way = paths.toward(foot, PATH_DEPTH, Point { x: 310.0, y: 800.0 }, 8.0);
    assert_eq!((way.point.x, way.depth), (310.0, PATH_DEPTH));
    assert_eq!(paths.path_span(), Some((100.0, 310.0)));
    // On the path behind the house at the front, out of sight; in front of one set back, not.
    assert!(paths.hidden(100.0, PATH_DEPTH, 15.0));
    assert!(!paths.hidden(170.0, PATH_DEPTH, 15.0));
    assert!(!paths.hidden(100.0, 0.0, 15.0));
    // A village whose houses all stand near the front has nowhere to walk the path from.
    let front = VillagePaths::new(vec![door(100.0, 0.0), door(170.0, 2.0)], 2.0);
    assert_eq!(front.path_span(), None);
}
