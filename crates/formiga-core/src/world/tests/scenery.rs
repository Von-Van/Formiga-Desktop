use super::*;
use crate::world::home::HOME_DURATION;

const CREATED: OffsetDateTime = datetime!(2026-01-01 0:00 UTC);

/// A colony of `members` laid out on the pond, walked home and standing still.
fn colony_on_the_pond(seed: [u8; 32], members: usize, desktop: &DesktopSnapshot) -> World {
    let mut world = World::new_original(seed, CREATED, desktop);
    world.save.home.scenery = Some(VillageScenery::Pond);
    while world.save.creatures.len() < members {
        let generation = world.save.creatures.len() as u8;
        let mut grown = world.save.creatures[0].clone();
        grown.generation = generation;
        grown.colony_order = generation;
        grown.id = u64::from(generation) + 100;
        world.save.creatures.push(grown);
    }
    world.tick(CREATED, 0.05, desktop);
    assert!(world.save.home.is_active());
    finish_home_approach_still(&mut world, CREATED, desktop);
    world
}

fn commons(world: &World, desktop: &DesktopSnapshot) -> Option<HomeCommons> {
    home_commons(
        &world.save.home,
        colony_cottage_list(&world.save.creatures).as_slice(),
        &desktop.monitors,
        &world.save.settings.habitat,
        world.save.settings.display_scale,
    )
}

fn placement(world: &World, desktop: &DesktopSnapshot) -> SceneryPlacement {
    commons(world, desktop)
        .and_then(|commons| commons.scenery)
        .expect("the pond fits this display")
}

/// Every house stands on its own spot in the picture, in the order they are given out, the two
/// trees on theirs, and the picture itself on the floor in the home's corner.
#[test]
fn every_house_and_tree_stands_on_its_spot_in_the_pond() {
    let desktop = desktop();
    let world = colony_on_the_pond([81; 32], 6, &desktop);
    let scenery = placement(&world, &desktop);
    let cottages = colony_cottage_list(&world.save.creatures);
    let policy = &world.save.settings.habitat;
    let scale = world.save.settings.display_scale;
    for slot in 0..MAX_COLONY_CREATURES {
        let (_, at) = home_dwelling_position(
            &world.save.home,
            slot,
            cottages.as_slice(),
            &desktop.monitors,
            policy,
            scale,
        )
        .expect("every house shows");
        assert_eq!(Some(at), scenery.house(slot), "house {slot}");
    }
    for end in TreeEnd::BOTH {
        let (_, at) = home_tree_position(
            &world.save.home,
            end,
            cottages.as_slice(),
            &desktop.monitors,
            policy,
            scale,
        )
        .expect("both trees show");
        assert_eq!(at, scenery.tree(end));
    }
    let usable = desktop.monitors[0].usable_bounds;
    let bottom = scenery.origin.y + scenery.map().height as f32 * scenery.scale;
    assert!((bottom - (usable.bottom() - 2.0)).abs() < 0.01);
    let corner_side = match world.save.home.corner {
        HomeCorner::BottomLeft => scenery.origin.x - usable.x,
        HomeCorner::BottomRight => {
            usable.right() - (scenery.origin.x + scenery.map().width as f32 * scenery.scale)
        }
    };
    assert!((corner_side - 8.0).abs() < 0.01);
}

/// Walking home from the floor, everybody hops up onto the path along the front of the pond and
/// goes the rest of the way over the picture's own ground, to a place of its own on the trail.
#[test]
fn the_colony_walks_home_up_onto_the_pond() {
    let desktop = desktop();
    let world = colony_on_the_pond([82; 32], 4, &desktop);
    let scenery = placement(&world, &desktop);
    for creature in &world.save.creatures {
        assert!(
            scenery.on_map(creature.state.position),
            "{} is at {:?}, off the pond",
            creature.name,
            creature.state.position
        );
    }
}

/// While the houses are out, a resident on its own feet keeps to the walk map, and the way
/// down off the top terrace is hand over hand down its cliff.
#[test]
fn residents_keep_to_the_walk_map_and_climb_down_its_cliff() {
    let desktop = desktop();
    let mut world = colony_on_the_pond([83; 32], 6, &desktop);
    let scenery = placement(&world, &desktop);
    // Whoever rests furthest up on the top terrace strolls down to the second house.
    let top = world
        .save
        .creatures
        .iter()
        .min_by(|a, b| a.state.position.y.total_cmp(&b.state.position.y))
        .map(|creature| creature.id)
        .unwrap();
    let below = commons(&world, &desktop)
        .unwrap()
        .at(scenery.house(1).unwrap().x);
    world.send_strolling(top, below);
    let mut now = CREATED;
    let mut climbed = false;
    for _ in 0..4_000 {
        now += time::Duration::milliseconds(50);
        world.tick(now, 0.05, &desktop);
        for creature in &world.save.creatures {
            let walking = matches!(
                creature.state.action,
                ActionKind::Traverse | ActionKind::ClimbWindow
            );
            if walking && !world.window_journeys.contains_key(&creature.id) {
                assert!(
                    scenery.on_map(creature.state.position),
                    "{} walked off the pond at {:?}",
                    creature.name,
                    creature.state.position
                );
            }
        }
        let walker = world
            .save
            .creatures
            .iter()
            .find(|creature| creature.id == top)
            .unwrap();
        climbed |= walker.state.action == ActionKind::ClimbWindow;
        if walker.state.position == below {
            break;
        }
    }
    assert!(
        climbed,
        "the trail comes down off the top terrace by its cliff"
    );
    let walker = world.save.creatures.iter().find(|c| c.id == top).unwrap();
    assert_eq!(walker.state.position, below);
}

/// The scenery goes with the houses, and whoever was up in it hops down to the floor.
#[test]
fn when_the_houses_go_everyone_hops_down_to_the_floor() {
    let desktop = desktop();
    let mut world = colony_on_the_pond([84; 32], 4, &desktop);
    let floor = desktop.monitors[0].usable_bounds.bottom() - 4.0;
    assert!(
        world
            .save
            .creatures
            .iter()
            .any(|creature| creature.state.position.y < floor - 100.0),
        "somebody is up in the picture"
    );
    let mut now = CREATED + HOME_DURATION + time::Duration::seconds(1);
    world.save.maximum_seen_utc = now;
    for _ in 0..80 {
        now += time::Duration::milliseconds(50);
        world.tick(now, 0.05, &desktop);
    }
    assert!(!world.save.home.is_active());
    for creature in &world.save.creatures {
        assert!(
            (creature.state.position.y - floor).abs() <= 0.5,
            "{} is still at {:?}",
            creature.name,
            creature.state.position
        );
    }
}

/// A display with no room for the whole picture at this size keeps the village on its strip,
/// exactly where it would have stood without the scenery.
#[test]
fn a_display_too_small_for_the_pond_keeps_the_village_on_its_strip() {
    let mut desktop = desktop();
    desktop.monitors[0].scale_factor = 1.0;
    let mut world = World::new_original([85; 32], CREATED, &desktop);
    let cottages = colony_cottage_list(&world.save.creatures);
    let strip = home_dwelling_position(
        &world.save.home,
        0,
        cottages.as_slice(),
        &desktop.monitors,
        &world.save.settings.habitat,
        world.save.settings.display_scale,
    );
    world.save.home.scenery = Some(VillageScenery::Pond);
    assert!(
        commons(&world, &desktop).is_some_and(|commons| commons.scenery.is_none()),
        "no scenery at twice the size"
    );
    assert_eq!(
        home_dwelling_position(
            &world.save.home,
            0,
            cottages.as_slice(),
            &desktop.monitors,
            &world.save.settings.habitat,
            world.save.settings.display_scale,
        ),
        strip
    );
}

/// A whole visit of ordinary village life on the pond — strolls, chores, the garden, quiet
/// moments — and nobody walking anywhere ever leaves the picture's own ground.
#[test]
fn a_whole_visit_on_the_pond_keeps_every_walk_on_its_ground() {
    let desktop = desktop();
    let mut world = colony_on_the_pond([86; 32], 6, &desktop);
    world
        .save
        .home
        .set_garden(GardenKind::Flowers, Some(0.4), CREATED);
    let scenery = placement(&world, &desktop);
    let mut now = CREATED;
    let mut walked = 0;
    while world.save.home.is_active() {
        now += time::Duration::milliseconds(50);
        world.tick(now, 0.05, &desktop);
        for creature in &world.save.creatures {
            if matches!(
                creature.state.action,
                ActionKind::Traverse | ActionKind::ClimbWindow
            ) && !world.window_journeys.contains_key(&creature.id)
            {
                walked += 1;
                assert!(
                    scenery.on_map(creature.state.position),
                    "{} walked off the pond at {:?} doing {:?}",
                    creature.name,
                    creature.state.position,
                    world.village_life.get(&creature.id).map(|life| &life.plan)
                );
            }
        }
    }
    assert!(walked > 1_000, "the colony got about: {walked}");
}
