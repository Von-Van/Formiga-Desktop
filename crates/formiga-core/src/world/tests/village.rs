use super::home::settled_colony;
use super::*;

/// The founder keeps the colony house whatever order is asked for. The cottages stand in the
/// order arranged, anyone left out stands after them in the order they arrived, a mini comes
/// home to its big version's house wherever that now stands, and an order that is only the
/// order everyone arrived in is not written down.
#[test]
fn cottages_stand_in_the_order_arranged_and_the_founder_stays_first() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    let mut world = settled_colony([72; 32], 4, created, &desktop);
    let creatures = world.save.creatures.clone();
    let ids: Vec<CreatureId> = creatures.iter().map(|creature| creature.id).collect();
    assert_eq!(house_owners(&creatures, &[]).as_slice(), ids);

    // The last to arrive moved to the front; the founder, a stranger and a repeat are ignored,
    // and the one left out keeps its place after the others.
    let asked = vec![ids[3], ids[0], 999, ids[3], ids[1]];
    world.save.home.arrange_cottages(asked, &creatures);
    assert_eq!(world.save.home.cottage_order, [ids[3], ids[1], ids[2]]);
    let order = world.save.home.cottage_order.clone();
    assert_eq!(
        house_owners(&creatures, &order).as_slice(),
        [ids[0], ids[3], ids[1], ids[2]]
    );
    for (slot, id) in [(0, ids[0]), (1, ids[3]), (2, ids[1]), (3, ids[2])] {
        let creature = creatures.iter().find(|creature| creature.id == id).unwrap();
        assert_eq!(house_slot_for(creature, &creatures, &order), slot);
    }
    let mut mini = creatures[1].clone();
    mini.id = 500;
    mini.colony_order = 9;
    mini.role = CreatureRole::Mini { parent_id: ids[3] };
    let mut with_mini = creatures.clone();
    with_mini.push(mini.clone());
    assert_eq!(house_slot_for(&mini, &with_mini, &order), 1);

    // Everyone walks home to the house that is now theirs.
    let (_, houses) = world.village_places(&desktop);
    let cottages = colony_cottage_list(&creatures);
    for (slot, id) in [(1, ids[3]), (2, ids[1]), (3, ids[2])] {
        let (_, door) = home_dwelling_position(
            &world.save.home,
            slot,
            cottages.as_slice(),
            &desktop.monitors,
            &world.save.settings.habitat,
            world.save.settings.display_scale,
        )
        .unwrap();
        assert_eq!(houses[&id].0, door, "cottage {slot}");
    }

    world
        .save
        .home
        .arrange_cottages(vec![ids[1], ids[2], ids[3]], &creatures);
    assert!(world.save.home.cottage_order.is_empty());
}

/// A companion replaced from the Colony page hands its cottage to the newcomer where it stood,
/// and one who leaves takes its place in the order with it.
#[test]
fn a_replacement_keeps_the_cottage_and_a_departure_leaves_the_order() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    let mut world = settled_colony([73; 32], 4, created, &desktop);
    let ids: Vec<CreatureId> = world
        .save
        .creatures
        .iter()
        .map(|creature| creature.id)
        .collect();
    let creatures = world.save.creatures.clone();
    world
        .save
        .home
        .arrange_cottages(vec![ids[3], ids[1], ids[2]], &creatures);
    for creature in &mut world.save.creatures {
        creature.kept = false;
    }
    let newcomer = world
        .replace_creature_with_adult(ids[3], [9; 32], created, &desktop)
        .unwrap();
    assert_eq!(world.save.home.cottage_order, [newcomer, ids[1], ids[2]]);
    world.remove_colony_creature(ids[1]).unwrap();
    assert_eq!(world.save.home.cottage_order, [newcomer, ids[2]]);
    assert_eq!(
        house_owners(&world.save.creatures, &world.save.home.cottage_order).as_slice(),
        [ids[0], newcomer, ids[2]]
    );
}

/// A named palette repaints the village and touches nothing else about it: the style, the size
/// and every detail of the house are the colony's own, and taking the palette away gives back
/// exactly the colours it was generated with.
#[test]
fn a_palette_repaints_the_village_and_nothing_else() {
    let mut home = ColonyHome::from_seed([5; 32], None, None, None);
    assert_eq!(home.drawn_shelter(), home.shelter);
    let mut pairings = std::collections::BTreeSet::new();
    for palette in VillagePalette::ALL {
        home.palette = Some(palette);
        let drawn = home.drawn_shelter();
        let (main, accent) = palette.palettes();
        assert!(main < 12 && accent < 12 && main != accent, "{palette:?}");
        assert_eq!((drawn.palette_index, drawn.accent_index), (main, accent));
        assert_eq!(
            ShelterGenome {
                palette_index: home.shelter.palette_index,
                accent_index: home.shelter.accent_index,
                ..drawn
            },
            home.shelter
        );
        pairings.insert((main, accent));
    }
    assert_eq!(pairings.len(), VillagePalette::ALL.len());
    home.palette = None;
    assert_eq!(home.drawn_shelter(), home.shelter);
}

/// One patch of each kind at most, somewhere on the ground. Putting the village back as it grew
/// digs them up with the palette and the cottage order, and leaves the hangout spots alone.
#[test]
fn a_village_keeps_one_patch_of_each_kind_and_puts_back_what_was_arranged() {
    let mut home = ColonyHome::default();
    assert!(home.set_garden(GardenKind::Herbs, Some(0.4)));
    assert!(home.set_garden(GardenKind::Herbs, Some(2.0)));
    assert!(home.set_garden(GardenKind::Flowers, Some(0.1)));
    assert!(!home.set_garden(GardenKind::Vegetables, Some(f32::NAN)));
    assert_eq!(
        home.gardens,
        [
            GardenPatch {
                kind: GardenKind::Flowers,
                along: 0.1
            },
            GardenPatch {
                kind: GardenKind::Herbs,
                along: 1.0
            },
        ]
    );
    assert!(home.set_garden(GardenKind::Flowers, None));
    assert_eq!(home.garden(GardenKind::Flowers), None);
    home.gardens.push(GardenPatch {
        kind: GardenKind::Herbs,
        along: 0.2,
    });
    home.gardens.push(GardenPatch {
        kind: GardenKind::Vegetables,
        along: f32::NEG_INFINITY,
    });
    home.cottage_order = vec![4, 4, 3];
    home.normalize_village();
    assert_eq!(home.gardens.len(), 1);
    assert_eq!(home.cottage_order, [4, 3]);

    home.set_hangout(HangoutKind::Blanket, Some(0.5));
    home.palette = Some(VillagePalette::Autumn);
    home.reset_arrangement();
    assert!(home.gardens.is_empty());
    assert!(home.cottage_order.is_empty());
    assert_eq!(home.palette, None);
    assert!(home.hangout(HangoutKind::Blanket).is_some());
}

/// Garden patches and hangout spots share the ground: every one of them stands on ground a
/// companion may stand on, none on top of another however they were placed, and the spots are
/// where the colony goes to use them.
#[test]
fn gardens_and_spots_share_the_ground_without_touching() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    let world = settled_colony([74; 32], 4, created, &desktop);
    let cottages = colony_cottage_list(&world.save.creatures);
    let policy = &world.save.settings.habitat;
    let scale = world.save.settings.display_scale;
    let commons = home_commons(
        &world.save.home,
        cottages.as_slice(),
        &desktop.monitors,
        policy,
        scale,
    )
    .unwrap();
    let (low, high) = commons.standing_span();
    let gap = (HANGOUT_WIDTH + 2.0) * commons.scale;
    for placements in [
        [0.5; 6],
        [0.0, 0.01, 1.0, 0.99, 0.5, 0.49],
        [1.0; 6],
        [0.1, 0.9, 0.3, 0.7, 0.5, 0.2],
    ] {
        let mut home = world.save.home.clone();
        for (kind, along) in HangoutKind::ALL.into_iter().zip(&placements[..3]) {
            home.set_hangout(kind, Some(*along));
        }
        for (kind, along) in GardenKind::ALL.into_iter().zip(&placements[3..]) {
            home.set_garden(kind, Some(*along));
        }
        let ground =
            home_ground_positions(&home, cottages.as_slice(), &desktop.monitors, policy, scale);
        assert_eq!(ground.len(), 6, "{placements:?}");
        let mut xs: Vec<f32> = ground.iter().map(|(_, _, at)| at.x).collect();
        for (_, _, at) in &ground {
            assert_eq!(at.y, commons.ground_y);
            assert!(at.x >= low - 0.01 && at.x <= high + 0.01, "{placements:?}");
        }
        xs.sort_by(f32::total_cmp);
        for pair in xs.windows(2) {
            assert!(pair[1] - pair[0] >= gap - 0.01, "{placements:?}: {xs:?}");
        }
        let spots =
            home_hangout_positions(&home, cottages.as_slice(), &desktop.monitors, policy, scale);
        let from_ground: Vec<_> = ground
            .iter()
            .filter_map(|(item, monitor_id, at)| match item {
                GroundItem::Hangout(kind) => Some((*kind, *monitor_id, *at)),
                GroundItem::Garden(_) => None,
            })
            .collect();
        assert_eq!(spots, from_ground);
    }
}

/// The arrangement is kept with the village and comes back exactly, and a village that was never
/// arranged writes none of it.
#[test]
fn an_arranged_village_is_kept_and_an_unarranged_one_writes_nothing() {
    let mut home = ColonyHome::default();
    let empty = serde_json::to_value(&home).unwrap();
    for field in ["cottage_order", "palette", "gardens"] {
        assert!(empty.get(field).is_none(), "{field} is absent until chosen");
    }
    home.cottage_order = vec![11, 7];
    home.palette = Some(VillagePalette::Pebble);
    home.set_garden(GardenKind::Vegetables, Some(0.75));
    let back: ColonyHome = serde_json::from_str(&serde_json::to_string(&home).unwrap()).unwrap();
    assert_eq!(back, home);
}
