use super::home::settled_colony;
use super::*;
use rand::{Rng, SeedableRng};

/// A colony of six: four full-size companions in four houses and two minis, one each for the
/// first two.
fn crowded_colony(seed: u8) -> (World, DesktopSnapshot, OffsetDateTime) {
    let created = datetime!(2026-03-01 10:00 UTC);
    let desktop = desktop();
    let mut world = settled_colony([seed; 32], 6, created, &desktop);
    let ids: Vec<CreatureId> = world.save.creatures.iter().map(|c| c.id).collect();
    world.save.creatures[4].role = CreatureRole::Mini { parent_id: ids[0] };
    world.save.creatures[5].role = CreatureRole::Mini { parent_id: ids[1] };
    // Each companion its own name, so a moved companion is told apart in a failure.
    for (index, creature) in world.save.creatures.iter_mut().enumerate() {
        creature.name = format!("C{index}");
    }
    (world, desktop, created)
}

/// Everything about a companion that is not where it lives or what it is doing this moment:
/// who it is, what it remembers, what it has learned, what it wears.
fn identity(creature: &Creature) -> String {
    format!(
        "{} {:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?}",
        creature.name,
        creature.role,
        creature.origin,
        creature.appearance,
        creature.personality,
        creature.temperament,
        creature.memory,
        creature.tendencies,
        creature.routines,
        creature.accessory,
        creature.born_at_utc,
        creature.kept,
    )
}

fn identities(world: &World) -> Vec<String> {
    world.save.creatures.iter().map(identity).collect()
}

/// Every companion lives in a house the village has, the village has at least one house, and the
/// colony holds nothing a validated colony never does.
fn assert_everyone_housed(world: &World, context: &str) {
    let creatures = &world.save.creatures;
    let owners = house_owners(creatures, &world.save.home.cottage_order);
    assert!(!owners.as_slice().is_empty(), "{context}: no house");
    assert_eq!(
        owners.as_slice().len(),
        colony_cottage_list(creatures).as_slice().len() + 1,
        "{context}: the houses drawn are the houses kept"
    );
    for creature in creatures {
        let keeper = house_keeper_of(creature, creatures)
            .unwrap_or_else(|| panic!("{context}: {} lives nowhere", creature.name));
        assert!(
            owners.as_slice().contains(&keeper),
            "{context}: {} lives in a house that is not in the village",
            creature.name
        );
        let slot = house_slot_for(creature, creatures, &world.save.home.cottage_order);
        assert_eq!(
            owners.as_slice()[slot],
            keeper,
            "{context}: {}",
            creature.name
        );
    }
    let found = violations(&world.save);
    assert!(found.is_empty(), "{context}: {found:?}");
}

/// Who lives in which house, as the companions' own names, house by house in row order.
fn arrangement(world: &World) -> Vec<Vec<String>> {
    let creatures = &world.save.creatures;
    house_owners(creatures, &world.save.home.cottage_order)
        .as_slice()
        .iter()
        .map(|keeper| {
            house_residents(creatures, *keeper)
                .into_iter()
                .map(|id| creatures.iter().find(|c| c.id == id).unwrap().name.clone())
                .collect()
        })
        .collect()
}

/// Over many colonies and long runs of random rearranging — houses taken down, companions moved
/// in with others and given their own houses back, households swapped, and changes undone — no
/// companion is ever lost or changed: the colony holds the same companions with the same
/// memories, bonds, tallies, looks and belongings, everybody always lives in a house the village
/// has, and the arrangement survives being written and read back exactly.
#[test]
fn rearranging_who_lives_where_never_loses_a_companion_or_anything_it_has() {
    for seed in [11_u8, 37, 62, 90, 141, 203] {
        let (mut world, desktop, created) = crowded_colony(seed);
        let before = identities(&world);
        let bonds = world.save.relationships.clone();
        let tallies = world.save.tallies.clone();
        let objects = world.save.objects.objects.clone();
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(u64::from(seed));
        let mut now = created;
        for step in 0..120 {
            let ids: Vec<CreatureId> = world.save.creatures.iter().map(|c| c.id).collect();
            let keepers: Vec<CreatureId> =
                house_owners(&world.save.creatures, &world.save.home.cottage_order)
                    .as_slice()
                    .to_vec();
            let pick = |rng: &mut rand_chacha::ChaCha8Rng, from: &[CreatureId]| {
                from[rng.random_range(0..from.len())]
            };
            let op = rng.random_range(0..5);
            let label = match op {
                0 => {
                    let keeper = pick(&mut rng, &keepers);
                    let result = world.edit(ColonyEdit::TookDownHouse { name: "x".into() }, |w| {
                        w.take_down_house(keeper)
                    });
                    if keepers.len() == 1 {
                        assert_eq!(result, Err(HousingError::LastHouse));
                    } else {
                        assert!(result.is_ok(), "seed {seed} step {step}: {result:?}");
                    }
                    "take down"
                }
                1 => {
                    let (resident, keeper) = (pick(&mut rng, &ids), pick(&mut rng, &keepers));
                    let _ = world.edit(ColonyEdit::MovedIn { name: "x".into() }, |w| {
                        w.move_into_house(resident, keeper)
                    });
                    "move in"
                }
                2 => {
                    let resident = pick(&mut rng, &ids);
                    let _ = world.edit(ColonyEdit::MovedIn { name: "x".into() }, |w| {
                        w.give_own_house(resident)
                    });
                    "own house"
                }
                3 => {
                    let (a, b) = (pick(&mut rng, &keepers), pick(&mut rng, &keepers));
                    let _ = world.edit(ColonyEdit::SwappedHouses, |w| w.swap_households(a, b));
                    "swap"
                }
                _ => {
                    let _ = world.undo_last_edit();
                    "undo"
                }
            };
            let context = format!("seed {seed} step {step} ({label})");
            assert_eq!(identities(&world), before, "{context}");
            assert_eq!(world.save.relationships, bonds, "{context}");
            assert_eq!(world.save.tallies, tallies, "{context}");
            assert_eq!(world.save.objects.objects, objects, "{context}");
            assert_everyone_housed(&world, &context);
            // A few ticks between changes, so the village is living in whatever it was left.
            for _ in 0..4 {
                now += Duration::milliseconds(50);
                world.tick(now, 0.05, &desktop);
            }
            if step % 20 == 19 {
                let bytes = serde_json::to_vec(&world.save).unwrap();
                let reopened = World::from_save(crate::persistence::decode(&bytes).unwrap());
                assert_eq!(arrangement(&reopened), arrangement(&world), "{context}");
                assert_eq!(identities(&reopened), before, "{context}");
            }
        }
    }
}

/// Taking down a house moves its whole household together into the house with the most room,
/// keeps everyone's house look keyed to them so it comes back as it was, and can be undone back
/// to exactly the village it was.
#[test]
fn a_house_taken_down_rehouses_its_household_together_and_undoes() {
    let (mut world, _, _) = crowded_colony(23);
    let ids: Vec<CreatureId> = world.save.creatures.iter().map(|c| c.id).collect();
    world
        .save
        .home
        .set_house_style(ids[1], Some(ShelterStyle::Mushroom));
    let home = world.save.home.clone();
    let layout = arrangement(&world);
    assert_eq!(layout.len(), 4);
    // C1 keeps a cottage with its mini C5. The emptiest houses are C2's and C3's, each alone;
    // C2's is nearer.
    let host = world
        .edit(ColonyEdit::TookDownHouse { name: "C1".into() }, |w| {
            w.take_down_house(ids[1])
        })
        .unwrap();
    assert_eq!(host, ids[2]);
    assert_eq!(
        arrangement(&world),
        [vec!["C0", "C4"], vec!["C2", "C1", "C5"], vec!["C3"]]
    );
    assert_eq!(world.save.creatures[1].lodges_with, Some(ids[2]));
    assert_eq!(world.save.creatures[5].lodges_with, None, "a mini follows");
    // Its own house's look is kept for it.
    assert_eq!(
        world.save.home.house_style(ids[1]),
        Some(ShelterStyle::Mushroom)
    );
    assert_eq!(
        world.last_edit(),
        Some(&ColonyEdit::TookDownHouse { name: "C1".into() })
    );
    world.undo_last_edit().unwrap();
    assert_eq!(arrangement(&world), layout);
    assert_eq!(world.save.home, home);
    assert!(world.save.creatures.iter().all(|c| c.lodges_with.is_none()));

    // Taken down and put up again, it stands as it stood and looks as it looked.
    world.take_down_house(ids[1]).unwrap();
    world.give_own_house(ids[1]).unwrap();
    assert_eq!(arrangement(&world), layout);
    assert_eq!(
        world.save.home.house_style(ids[1]),
        Some(ShelterStyle::Mushroom)
    );
}

/// The colony house can come down too: the next house along becomes the colony house, and the
/// founder and its mini move in with whoever has room. The last house cannot come down.
#[test]
fn the_colony_house_passes_on_and_the_last_house_stays() {
    let (mut world, _, _) = crowded_colony(48);
    let ids: Vec<CreatureId> = world.save.creatures.iter().map(|c| c.id).collect();
    world.take_down_house(ids[0]).unwrap();
    let owners = house_owners(&world.save.creatures, &world.save.home.cottage_order);
    assert_eq!(owners.as_slice()[0], ids[1], "the next house along");
    // The next along is also the first to arrive of those left, so nothing needs writing down.
    assert!(!world.save.creatures[1].keeps_colony_house);
    assert_everyone_housed(&world, "colony house down");
    world.take_down_house(ids[1]).unwrap();
    world.take_down_house(ids[2]).unwrap();
    assert_eq!(world.take_down_house(ids[3]), Err(HousingError::LastHouse));
    assert_eq!(arrangement(&world).len(), 1);
    assert_eq!(arrangement(&world)[0].len(), 6, "all six live in one house");
    assert_everyone_housed(&world, "one house");
}

/// Swapping two households moves the people, never the houses: each house keeps its type,
/// depth, decorations and place, and the colony house stays the colony house.
#[test]
fn swapping_households_moves_people_not_houses() {
    let (mut world, _, _) = crowded_colony(77);
    let ids: Vec<CreatureId> = world.save.creatures.iter().map(|c| c.id).collect();
    world.save.home.set_house_depth(ids[2], Some(1));
    let styles = world.save.home.house_style_list(&world.save.creatures);
    let depths = world.save.home.house_depth_list(&world.save.creatures);
    let decorations = world.save.home.house_decoration_list(&world.save.creatures);
    world.swap_households(ids[0], ids[2]).unwrap();
    assert_eq!(
        arrangement(&world),
        [vec!["C2"], vec!["C1", "C5"], vec!["C0", "C4"], vec!["C3"]]
    );
    assert_eq!(
        world.save.home.house_style_list(&world.save.creatures),
        styles
    );
    assert_eq!(
        world.save.home.house_depth_list(&world.save.creatures),
        depths
    );
    assert_eq!(
        world.save.home.house_decoration_list(&world.save.creatures),
        decorations
    );
    assert_everyone_housed(&world, "swapped");
    // And back again leaves nothing written down that was not before.
    world.swap_households(ids[0], ids[2]).unwrap();
    assert!(world.save.creatures.iter().all(|c| !c.keeps_colony_house));
    assert_eq!(
        world.save.home.house_style_list(&world.save.creatures),
        styles
    );
}

/// A mini can move in with another household and back; a keeper who moves brings its household
/// and its house comes down; a mini never keeps a house.
#[test]
fn companions_move_between_houses_without_being_recreated() {
    let (mut world, _, _) = crowded_colony(99);
    let ids: Vec<CreatureId> = world.save.creatures.iter().map(|c| c.id).collect();
    world.move_into_house(ids[4], ids[3]).unwrap();
    assert_eq!(arrangement(&world)[3], ["C3", "C4"]);
    assert_eq!(
        world.move_into_house(ids[4], ids[3]),
        Err(HousingError::AlreadyThere)
    );
    assert_eq!(world.give_own_house(ids[4]), Err(HousingError::NotFullSize));
    world.move_into_house(ids[4], ids[0]).unwrap();
    assert_eq!(
        world.save.creatures[4].lodges_with, None,
        "home with its big version"
    );
    world.move_into_house(ids[1], ids[3]).unwrap();
    assert_eq!(
        arrangement(&world),
        [vec!["C0", "C4"], vec!["C2"], vec!["C3", "C1", "C5"]]
    );
    assert_eq!(
        world.move_into_house(ids[1], ids[1]),
        Err(HousingError::NoHouse)
    );
    world.give_own_house(ids[1]).unwrap();
    assert_eq!(arrangement(&world).len(), 4);
    assert_everyone_housed(&world, "moved back");
}

/// Nothing moves while a house is open in Formiga Home, and a resident leaving the colony
/// leaves its lodgers housed rather than given houses of their own.
#[test]
fn a_house_open_elsewhere_holds_everyone_and_a_departure_rehouses_its_lodgers() {
    let (mut world, desktop, _) = crowded_colony(130);
    let ids: Vec<CreatureId> = world.save.creatures.iter().map(|c| c.id).collect();
    world.move_into_house(ids[3], ids[1]).unwrap();
    let went = world.begin_house_visit(&[ids[1], ids[3], ids[5]], &desktop);
    assert!(!went.is_empty());
    let layout = arrangement(&world);
    assert_eq!(world.take_down_house(ids[2]), Err(HousingError::HouseOpen));
    assert_eq!(
        world.move_into_house(ids[4], ids[2]),
        Err(HousingError::HouseOpen)
    );
    assert_eq!(
        world.swap_households(ids[0], ids[2]),
        Err(HousingError::HouseOpen)
    );
    assert_eq!(arrangement(&world), layout);
    world.end_house_visit();

    world.remove_colony_creature(ids[1]).unwrap();
    assert_everyone_housed(&world, "host left");
    let lodger = world
        .save
        .creatures
        .iter()
        .find(|c| c.id == ids[3])
        .unwrap();
    assert!(
        lodger.lodges_with.is_some(),
        "the lodger moved in elsewhere rather than building a house"
    );
}

/// Whatever the village was in the middle of ends when the houses change hands: plans next door
/// and up to doorsteps are let go, a guest works its walk round out again, and everyone then
/// walks home to the house that is theirs now.
#[test]
fn plans_in_flight_resolve_when_a_house_comes_down() {
    for seed in [5_u8, 66, 180] {
        let (mut world, desktop, created) = crowded_colony(seed);
        let ids: Vec<CreatureId> = world.save.creatures.iter().map(|c| c.id).collect();
        let mut now = created;
        // Let the village get going.
        for _ in 0..600 {
            now += Duration::milliseconds(50);
            world.tick(now, 0.05, &desktop);
        }
        world.take_down_house(ids[3]).unwrap();
        assert!(world.village_life.is_empty(), "seed {seed}: plans let go");
        for _ in 0..1200 {
            now += Duration::milliseconds(50);
            world.tick(now, 0.05, &desktop);
        }
        assert_everyone_housed(&world, &format!("seed {seed}"));
        let (_, houses) = world.village_places(&desktop);
        let host = house_keeper_of(
            world
                .save
                .creatures
                .iter()
                .find(|c| c.id == ids[3])
                .unwrap(),
            &world.save.creatures,
        )
        .unwrap();
        assert_eq!(
            houses[&ids[3]].0, houses[&host].0,
            "seed {seed}: comes home to its host's door"
        );
    }
}

/// The keepers' names along the row, the colony house first.
fn row(world: &World) -> Vec<String> {
    arrangement(world)
        .into_iter()
        .map(|house| house[0].clone())
        .collect()
}

/// A house taken down and put up again stands where it stood in a row that had been rearranged,
/// whichever house it was, and however the houses left standing were shuffled in between.
#[test]
fn a_house_put_up_again_stands_where_it_stood_in_a_rearranged_row() {
    for taken in 1..=3 {
        let (mut world, _, _) = crowded_colony(40 + taken as u8);
        let ids: Vec<CreatureId> = world.save.creatures.iter().map(|c| c.id).collect();
        let creatures = world.save.creatures.clone();
        world
            .save
            .home
            .arrange_cottages(vec![ids[3], ids[1], ids[2]], &creatures);
        assert_eq!(row(&world), ["C0", "C3", "C1", "C2"]);
        world.take_down_house(ids[taken]).unwrap();
        assert_eq!(row(&world).len(), 3);
        world.give_own_house(ids[taken]).unwrap();
        assert_eq!(row(&world), ["C0", "C3", "C1", "C2"], "C{taken} put up");
        assert_everyone_housed(&world, "put up again");
    }

    // The houses left standing rearranged while one is down: it comes back after the house it
    // stood behind.
    let (mut world, _, _) = crowded_colony(44);
    let ids: Vec<CreatureId> = world.save.creatures.iter().map(|c| c.id).collect();
    let creatures = world.save.creatures.clone();
    world
        .save
        .home
        .arrange_cottages(vec![ids[3], ids[1], ids[2]], &creatures);
    world.take_down_house(ids[1]).unwrap();
    let creatures = world.save.creatures.clone();
    world
        .save
        .home
        .arrange_cottages(vec![ids[2], ids[3]], &creatures);
    assert_eq!(row(&world), ["C0", "C2", "C3"]);
    world.give_own_house(ids[1]).unwrap();
    assert_eq!(row(&world), ["C0", "C2", "C3", "C1"]);

    // A row nobody arranged is still not written down, a house down or not.
    let (mut world, _, _) = crowded_colony(45);
    let ids: Vec<CreatureId> = world.save.creatures.iter().map(|c| c.id).collect();
    world.take_down_house(ids[2]).unwrap();
    assert!(world.save.home.cottage_order.is_empty());
    world.give_own_house(ids[2]).unwrap();
    assert!(world.save.home.cottage_order.is_empty());
    assert_eq!(row(&world), ["C0", "C1", "C2", "C3"]);
}

/// Taking back a change to who lives where waits while a house is open in Formiga Home, and is
/// still there to take back once the household is home.
#[test]
fn a_rehousing_is_not_undone_while_a_house_is_open() {
    let (mut world, desktop, _) = crowded_colony(131);
    let ids: Vec<CreatureId> = world.save.creatures.iter().map(|c| c.id).collect();
    let before = arrangement(&world);
    world
        .edit(ColonyEdit::MovedIn { name: "C3".into() }, |w| {
            w.move_into_house(ids[3], ids[1])
        })
        .unwrap();
    let moved = arrangement(&world);
    assert_ne!(moved, before);
    let went = world.begin_house_visit(&[ids[1], ids[3], ids[5]], &desktop);
    assert!(!went.is_empty());
    assert_eq!(world.undo_last_edit(), Err(UndoError::HouseOpen));
    assert_eq!(arrangement(&world), moved);
    assert!(world.last_edit().is_some());
    world.end_house_visit();
    assert!(matches!(
        world.undo_last_edit(),
        Ok(ColonyEdit::MovedIn { .. })
    ));
    assert_eq!(arrangement(&world), before);
    assert_everyone_housed(&world, "undone at home");
}
