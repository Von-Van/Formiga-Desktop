use super::*;

#[test]
fn colony_object_schedule_is_deterministic_and_between_three_and_seven_days() {
    let now = datetime!(2026-01-01 0:00 UTC);
    for ordinal in 0..64 {
        let first = scheduled_colony_object_at([101; 32], ordinal, now);
        let second = scheduled_colony_object_at([101; 32], ordinal, now);
        assert_eq!(first, second);
        assert!(first - now >= Duration::days(3));
        assert!(first - now <= Duration::days(7));
    }
}

#[test]
fn overdue_colony_objects_add_one_without_a_catch_up_flood() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let now = created + Duration::days(40);
    let desktop = desktop();
    let mut world = World::new([102; 32], created, &desktop);
    world.save.objects.next_at_utc = created + Duration::days(3);
    world.tick(now, 0.05, &desktop);
    assert_eq!(world.save.objects.objects.len(), 1);
    assert_eq!(world.save.objects.ordinal, 1);
    assert!(world.save.objects.next_at_utc >= now + Duration::days(3));
    assert!(world.save.objects.next_at_utc <= now + Duration::days(7));
    world.tick(now, 0.05, &desktop);
    assert_eq!(world.save.objects.objects.len(), 1);
    assert_eq!(
        world
            .drain_events()
            .filter(|event| matches!(event, WorldEvent::ColonyObjectAdded { .. }))
            .count(),
        1
    );
}

#[test]
fn colony_objects_cap_at_eight_and_invalid_positions_recover() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    let mut world = World::new([103; 32], created, &desktop);
    for index in 0..MAX_COLONY_OBJECTS + 3 {
        world.save.objects.next_at_utc = created;
        world.tick(created + Duration::days(index as i64 + 1), 0.05, &desktop);
    }
    assert_eq!(world.save.objects.objects.len(), MAX_COLONY_OBJECTS);

    let object = &mut world.save.objects.objects[0];
    object.display = DisplayKey([255; 16]);
    object.normalized_position = Point { x: -50.0, y: 50.0 };
    world.reconcile_colony_objects(&desktop);
    let object = &world.save.objects.objects[0];
    assert_eq!(object.display, desktop.monitors[0].display_key);
    assert!((0.0..=1.0).contains(&object.normalized_position.x));
    assert!((0.0..=1.0).contains(&object.normalized_position.y));
    assert!(
        resolved_colony_object_position(object, &desktop.monitors, &world.save.settings.habitat)
            .is_some()
    );
}

#[test]
fn the_village_gains_something_every_day_or_two_on_a_deterministic_schedule() {
    let now = datetime!(2026-01-01 0:00 UTC);
    for ordinal in 0..64 {
        let first = scheduled_village_unlock_at([104; 32], ordinal, now);
        let second = scheduled_village_unlock_at([104; 32], ordinal, now);
        assert_eq!(first, second);
        assert!(first - now >= Duration::hours(24));
        assert!(first - now <= Duration::hours(48));
    }
}

#[test]
fn the_village_gains_one_thing_after_downtime_and_in_time_everything() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    let mut world = World::new([105; 32], created, &desktop);
    let starting = world.save.home.unlocks.clone();
    assert_eq!(
        starting.remaining().count(),
        VillageItem::all().count() - 12
    );
    let overdue = created + Duration::days(40);
    world.save.home.unlocks.next_at_utc = created + Duration::days(1);
    world.tick(overdue, 0.05, &desktop);
    assert_eq!(
        world.save.home.unlocks.remaining().count(),
        starting.remaining().count() - 1,
        "a long absence brings one thing, not a flood"
    );
    assert!(world.save.home.unlocks.next_at_utc >= overdue + Duration::hours(24));
    assert!(world.save.home.unlocks.next_at_utc <= overdue + Duration::hours(48));
    world.tick(overdue, 0.05, &desktop);
    let unlocked: Vec<VillageItem> = world
        .drain_events()
        .filter_map(|event| match event {
            WorldEvent::VillageUnlocked { item } => Some(item),
            _ => None,
        })
        .collect();
    assert_eq!(unlocked.len(), 1);
    assert!(
        world
            .save
            .companion
            .journal
            .iter()
            .any(|entry| entry.moment == JournalMoment::Unlocked(unlocked[0])),
        "the journal says what arrived"
    );

    let total = VillageItem::all().count();
    for day in 1..=total as i64 + 3 {
        world.save.home.unlocks.next_at_utc = overdue;
        world.tick(overdue + Duration::days(day), 0.05, &desktop);
    }
    assert_eq!(world.save.home.unlocks.remaining().count(), 0);
    // Categories take turns rather than one filling up before the next begins.
    let unlocks = &world.save.home.unlocks;
    assert_eq!(unlocks.decorations.len(), ShelterDecorationKind::ALL.len());
    assert_eq!(unlocks.ornaments.len(), OrnamentKind::ALL.len());
}

#[test]
fn a_new_decoration_goes_up_on_the_colony_house_when_its_place_there_is_free() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    let mut world = World::new([106; 32], created, &desktop);
    let founder = world.save.creatures[0].id;
    assert!(world.save.home.decorations_of(founder).is_empty());
    let mut hung = 0;
    for day in 1..=90 {
        world.save.home.unlocks.next_at_utc = created;
        world.tick(created + Duration::days(day), 0.05, &desktop);
        let events: Vec<WorldEvent> = world.drain_events().collect();
        for event in events {
            if let WorldEvent::VillageUnlocked {
                item: VillageItem::Decoration(kind),
            } = event
            {
                let showing = world.save.home.decoration_in(founder, kind.slot());
                assert!(showing.is_some(), "{kind:?}'s slot is filled");
                hung += usize::from(showing == Some(kind));
            }
        }
    }
    // One for each slot the house had free; everything later waits to be chosen.
    assert_eq!(hung, DecorationSlot::ALL.len());
    assert_eq!(
        world.save.home.decorations_of(founder).len(),
        DecorationSlot::ALL.len()
    );
}

#[test]
fn what_the_village_gains_reflects_what_the_colony_has_been_doing() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    let first_decoration = |world: &mut World| {
        // Only the decorations are left to come, so the theme alone decides between them.
        let unlocks = &mut world.save.home.unlocks;
        for item in VillageItem::all() {
            if !matches!(item, VillageItem::Decoration(_)) {
                unlocks.grant(item);
            }
        }
        preferred_village_unlock(&world.save)
    };

    let mut climbers = World::new([106; 32], created, &desktop);
    climbers.save.creatures[0].memory.window_climbs = u32::MAX;
    let Some(VillageItem::Decoration(kind)) = first_decoration(&mut climbers) else {
        panic!("a decoration was left to come");
    };
    assert!(
        matches!(
            kind,
            ShelterDecorationKind::RoofOrnament
                | ShelterDecorationKind::WeatherVane
                | ShelterDecorationKind::PerchedBird
                | ShelterDecorationKind::Pinwheel
                | ShelterDecorationKind::WindChime
        ),
        "a colony of climbers gets something for the sky, not {kind:?}"
    );

    let mut friends = two_creature_world([107; 32], created);
    friends.save.relationships[0].affinity = u8::MAX;
    friends.save.relationships[0].familiarity = u8::MAX;
    let Some(VillageItem::Decoration(kind)) = first_decoration(&mut friends) else {
        panic!("a decoration was left to come");
    };
    assert!(
        matches!(
            kind,
            ShelterDecorationKind::Pennant
                | ShelterDecorationKind::PaperLanterns
                | ShelterDecorationKind::FairyLights
        ),
        "close friends get something for a party, not {kind:?}"
    );
}

#[test]
fn village_lots_are_mirrored_scaled_separated_and_do_not_escape_restrictions() {
    let desktop = desktop();
    let monitor = &desktop.monitors[0];
    let mut home = ColonyHome::from_seed([0; 32], Some(monitor.display_key), None, None);
    let policy = HabitatPolicy::default();
    let villages = [
        Vec::new(),
        vec![DwellingKind::Cottage],
        vec![
            DwellingKind::Cottage,
            DwellingKind::Cottage,
            DwellingKind::Cottage,
        ],
    ];
    for cottages in &villages {
        for scale in 1..=4 {
            for corner in [HomeCorner::BottomLeft, HomeCorner::BottomRight] {
                home.corner = corner;
                let anchor = resolved_home_anchor(&home, monitor, scale, &policy).unwrap();
                let unit = f32::from(scale) / monitor.scale_factor.max(1.0);
                // Houses and trees share one walk, so no two houses' footprints overlap, and a
                // tree reaches in over its end house by `TREE_OVERLAP` and no further.
                let mut spans: Vec<(f32, f32)> = Vec::new();
                let push = |centre: f32, width: f32, spans: &mut Vec<(f32, f32)>| {
                    let half = width * unit / 2.0;
                    for (start, end) in spans.iter() {
                        assert!(
                            centre + half <= *start + 0.01 || centre - half >= *end - 0.01,
                            "overlapping lot at {centre}"
                        );
                    }
                    spans.push((centre - half, centre + half));
                };
                for slot in 0..=cottages.len() {
                    let (id, p) = home_dwelling_position(
                        &home,
                        slot,
                        cottages,
                        &desktop.monitors,
                        &policy,
                        scale,
                    )
                    .unwrap();
                    assert_eq!(id, monitor.id);
                    // Every dwelling stands on the shared ground line.
                    assert!((p.y - anchor.y).abs() < 0.01);
                    let kind = if slot == 0 {
                        DwellingKind::Main
                    } else {
                        cottages[slot - 1]
                    };
                    push(p.x, kind.width(), &mut spans);

                    // The commons in front of the houses is where its resident settles.
                    let (rest_id, rest) = home_resting_position(
                        &home,
                        slot,
                        cottages.len() + 1,
                        cottages,
                        &desktop.monitors,
                        &policy,
                        scale,
                    )
                    .unwrap();
                    assert_eq!(rest_id, monitor.id);
                    assert!((rest.y - anchor.y).abs() < 0.01);
                    // A companion stands on the ground in front of the houses rather than on a
                    // lot of its own, so its place is not compared with the lots on the strip —
                    // only with the other companions', which the caller checks.
                }
                // The colony's belongings are not lots on the strip: they are scattered over the
                // roots of the two trees that bookend it, four at each end.
                let mut trees = Vec::new();
                for end in TreeEnd::BOTH {
                    let (tree_id, tree) =
                        home_tree_position(&home, end, cottages, &desktop.monitors, &policy, scale)
                            .unwrap();
                    assert_eq!(tree_id, monitor.id);
                    assert!((tree.y - anchor.y).abs() < 0.01);
                    push(tree.x, TREE_WIDTH - 2.0 * TREE_OVERLAP, &mut spans);
                    trees.push(tree);
                }
                // The outward tree is the one nearest the corner; the inward one is past
                // everything else on the strip.
                let outward = (trees[0].x - anchor.x)
                    * if corner == HomeCorner::BottomLeft {
                        1.0
                    } else {
                        -1.0
                    };
                assert!(outward < 0.0, "the outward tree left the corner");
                let mut counted = [0; 2];
                for slot in 0..MAX_COLONY_OBJECTS {
                    let (id, p) = home_object_position(
                        &home,
                        slot,
                        cottages,
                        &desktop.monitors,
                        &policy,
                        scale,
                    )
                    .unwrap();
                    assert_eq!(id, monitor.id);
                    // Scattered around one tree's own ground, in depth as well as sideways.
                    let (yard, tree) = trees
                        .iter()
                        .enumerate()
                        .min_by(|a, b| (a.1.x - p.x).abs().total_cmp(&(b.1.x - p.x).abs()))
                        .unwrap();
                    assert!((p.y - tree.y).abs() <= BELONGING_DEPTH * unit + 0.01);
                    assert!((p.x - tree.x).abs() <= TREE_WIDTH / 2.0 * unit);
                    counted[yard] += 1;
                }
                assert_eq!(
                    counted,
                    [MAX_COLONY_OBJECTS / 2; 2],
                    "the colony's things did not split evenly between the two yards"
                );
            }
        }
    }
    let mut narrow = desktop.clone();
    narrow.monitors[0].usable_bounds.width = 80.0;
    home.corner = HomeCorner::BottomLeft;
    assert!(home_object_position(&home, 7, &[], &narrow.monitors, &policy, 3).is_none());
    assert!(home_object_position(&home, 8, &[], &desktop.monitors, &policy, 1).is_none());
    assert!(home_object_position(&home, 0, &[], &[], &policy, 1).is_none());
    assert!(
        home_dwelling_position(&home, 2, &[], &desktop.monitors, &policy, 1).is_none(),
        "a colony without that companion has no lot for it"
    );
}

#[test]
fn every_colony_member_after_the_first_gets_a_matching_house() {
    let now = datetime!(2026-09-12 9:30 UTC);
    let desktop = desktop();
    let mut world = World::new([31; 32], now, &desktop);
    assert!(
        colony_cottages(&world.save.creatures).is_empty(),
        "the founder shares the colony house"
    );
    let parent = world.save.creatures[0].id;
    for index in 0..3 {
        let mut extra = world.save.creatures[0].clone();
        extra.id = 900 + index as CreatureId;
        extra.colony_order = (index + 1) as u8;
        extra.role = if index == 0 {
            CreatureRole::Adult
        } else {
            CreatureRole::Mini { parent_id: parent }
        };
        world.save.creatures.push(extra);
    }
    assert_eq!(
        colony_cottages(&world.save.creatures),
        vec![DwellingKind::Cottage],
        "a house belongs to a full-size companion; a mini lives in its big version's"
    );
}
