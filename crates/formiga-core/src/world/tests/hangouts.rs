use super::home::settled_colony;
use super::*;

/// One spot of each kind at most, somewhere on the ground, and picking one up takes it away.
#[test]
fn a_village_keeps_one_spot_of_each_kind_on_its_ground() {
    let mut home = ColonyHome::default();
    assert!(home.set_hangout(HangoutKind::Cushion, Some(0.25)));
    assert!(home.set_hangout(HangoutKind::Cushion, Some(1.7)));
    assert!(home.set_hangout(HangoutKind::Lookout, Some(-3.0)));
    assert!(!home.set_hangout(HangoutKind::Blanket, Some(f32::NAN)));
    assert_eq!(
        home.hangouts,
        [
            HangoutSpot {
                kind: HangoutKind::Cushion,
                along: 1.0
            },
            HangoutSpot {
                kind: HangoutKind::Lookout,
                along: 0.0
            },
        ]
    );
    assert!(home.set_hangout(HangoutKind::Cushion, None));
    assert_eq!(home.hangout(HangoutKind::Cushion), None);
    assert!(home.hangout(HangoutKind::Lookout).is_some());
    // A file that says otherwise is put right when the colony opens.
    home.hangouts = vec![
        HangoutSpot {
            kind: HangoutKind::Blanket,
            along: 0.5,
        },
        HangoutSpot {
            kind: HangoutKind::Blanket,
            along: 0.9,
        },
        HangoutSpot {
            kind: HangoutKind::Cushion,
            along: f32::INFINITY,
        },
    ];
    home.normalize_hangouts();
    assert_eq!(
        home.hangouts,
        [HangoutSpot {
            kind: HangoutKind::Blanket,
            along: 0.5
        }]
    );
}

/// Every spot stands on the ground a companion may stand on, and never on top of another, however
/// close together they were put down.
#[test]
fn spots_stand_on_the_ground_and_apart() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    let world = settled_colony([71; 32], 4, created, &desktop);
    let cottages = colony_cottage_list(&world.save.creatures);
    let commons = home_commons(
        &world.save.home,
        cottages.as_slice(),
        &desktop.monitors,
        &world.save.settings.habitat,
        world.save.settings.display_scale,
    )
    .unwrap();
    let (low, high) = commons.standing_span();
    let gap = (HANGOUT_WIDTH + 2.0) * commons.scale;
    for placements in [
        [0.5, 0.5, 0.5],
        [0.0, 0.01, 1.0],
        [1.0, 1.0, 1.0],
        [0.9, 0.1, 0.5],
    ] {
        let mut home = world.save.home.clone();
        for (kind, along) in HangoutKind::ALL.into_iter().zip(placements) {
            home.set_hangout(kind, Some(along));
        }
        let spots = home_hangout_positions(
            &home,
            cottages.as_slice(),
            &desktop.monitors,
            &world.save.settings.habitat,
            world.save.settings.display_scale,
        );
        assert_eq!(spots.len(), 3, "{placements:?}");
        let mut xs: Vec<f32> = spots.iter().map(|(_, _, at)| at.x).collect();
        for (_, monitor_id, at) in &spots {
            assert_eq!(*monitor_id, commons.monitor_id);
            assert_eq!(at.y, commons.ground_y);
            assert!(
                at.x >= low - 0.01 && at.x <= high + 0.01,
                "{placements:?}: {} is off the ground {low}..{high}",
                at.x
            );
        }
        xs.sort_by(f32::total_cmp);
        for pair in xs.windows(2) {
            assert!(pair[1] - pair[0] >= gap - 0.01, "{placements:?}: {xs:?}");
        }
    }
    // Nothing put down, nothing to place.
    assert!(
        home_hangout_positions(
            &world.save.home,
            cottages.as_slice(),
            &desktop.monitors,
            &world.save.settings.habitat,
            world.save.settings.display_scale,
        )
        .is_empty()
    );
}

/// Over an afternoon at home, a cushion put down draws some of the colony's naps to it, and the
/// nap is had on the cushion; the colony still does everything else it did before.
#[test]
fn a_cushion_draws_naps_to_it_without_taking_over_the_afternoon() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    let mut world = settled_colony([72; 32], 3, created, &desktop);
    world.save.home.set_hangout(HangoutKind::Cushion, Some(0.5));
    let cottages = colony_cottage_list(&world.save.creatures);
    let (_, _, cushion) = home_hangout_positions(
        &world.save.home,
        cottages.as_slice(),
        &desktop.monitors,
        &world.save.settings.habitat,
        world.save.settings.display_scale,
    )[0];
    let mut on_cushion = 0;
    let mut elsewhere = 0;
    let mut other_moments = 0;
    for _ in 0..40_000 {
        world.tick(created, 0.05, &desktop);
        for creature in &world.save.creatures {
            match creature.state.action {
                ActionKind::Sleep if (creature.state.position.x - cushion.x).abs() < 0.5 => {
                    on_cushion += 1
                }
                ActionKind::Sleep => elsewhere += 1,
                ActionKind::Eat
                | ActionKind::Drink
                | ActionKind::SoloPlay
                | ActionKind::InspectScreen
                | ActionKind::Greet => other_moments += 1,
                _ => {}
            }
        }
        // Keep the visit going for the whole afternoon.
        world.save.home.active_since_utc = Some(created);
    }
    assert!(on_cushion > 0, "nobody ever napped on the cushion");
    assert!(
        other_moments > 0,
        "the cushion took over every quiet moment"
    );
    eprintln!("naps on the cushion {on_cushion}, elsewhere {elsewhere}, other {other_moments}");
}

/// At the lookout a companion stands beside it, looking out over the desktop past it.
#[test]
fn a_lookout_is_looked_out_of_toward_the_open_desktop() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    let mut world = settled_colony([73; 32], 3, created, &desktop);
    // A curious colony, so the lookout is somewhere they feel like going.
    for creature in &mut world.save.creatures {
        creature.personality.curiosity = 1.0;
    }
    world.save.home.set_hangout(HangoutKind::Lookout, Some(0.5));
    let cottages = colony_cottage_list(&world.save.creatures);
    let (_, _, lookout) = home_hangout_positions(
        &world.save.home,
        cottages.as_slice(),
        &desktop.monitors,
        &world.save.settings.habitat,
        world.save.settings.display_scale,
    )[0];
    let middle =
        desktop.monitors[0].usable_bounds.x + desktop.monitors[0].usable_bounds.width / 2.0;
    // The village has more to do than stand at the lookout — its gardens, its houses, its roofs
    // — so it is given a long afternoon to get round to it.
    let mut looked = 0;
    for _ in 0..80_000 {
        world.tick(created, 0.05, &desktop);
        world.save.home.active_since_utc = Some(created);
        for creature in &world.save.creatures {
            let beside = (creature.state.position.x - lookout.x).abs();
            if creature.state.action == ActionKind::InspectScreen && beside > 1.0 && beside < 40.0 {
                looked += 1;
                assert_eq!(
                    creature.state.facing_right,
                    middle >= lookout.x,
                    "looking out over the desktop"
                );
                assert_eq!(
                    creature.state.position.x < lookout.x,
                    middle >= lookout.x,
                    "from the near side of the lookout"
                );
            }
        }
    }
    assert!(looked > 0, "nobody ever went to the lookout");
}

/// A picnic the village is asked for gathers around the picnic blanket.
#[test]
fn an_invited_picnic_gathers_at_the_blanket() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    let mut world = settled_colony([74; 32], 3, created, &desktop);
    for creature in &mut world.save.creatures {
        creature.personality.sociability = 1.0;
        creature.state.drives.energy = 0.5;
    }
    world.save.home.set_hangout(HangoutKind::Blanket, Some(0.8));
    let cottages = colony_cottage_list(&world.save.creatures);
    let (_, _, blanket) = home_hangout_positions(
        &world.save.home,
        cottages.as_slice(),
        &desktop.monitors,
        &world.save.settings.habitat,
        world.save.settings.display_scale,
    )[0];
    let host = world.save.creatures[0].id;
    assert!(world.handle_command(
        WorldCommand::InviteVillageMoment {
            creature_id: host,
            moment: VillageMoment::Picnic,
        },
        &desktop,
    ));
    let plan = world.village_moment.as_ref().unwrap();
    let first = plan.places.first().unwrap().spot.x;
    let last = plan.places.last().unwrap().spot.x;
    assert!(
        first - 0.5 <= blanket.x && blanket.x <= last + 0.5,
        "the line {first}..{last} should gather around the blanket at {}",
        blanket.x
    );
}

/// Spots are kept with the village and come back exactly, and a village without any writes none.
#[test]
fn spots_are_kept_with_the_village() {
    let mut home = ColonyHome::default();
    let empty = serde_json::to_value(&home).unwrap();
    assert!(
        empty.get("hangouts").is_none(),
        "absent until one is put down"
    );
    home.set_hangout(HangoutKind::Blanket, Some(0.3));
    home.set_hangout(HangoutKind::Lookout, Some(0.75));
    let text = serde_json::to_string(&home).unwrap();
    let back: ColonyHome = serde_json::from_str(&text).unwrap();
    assert_eq!(back.hangouts, home.hangouts);
}
