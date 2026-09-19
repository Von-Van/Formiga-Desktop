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
fn shelter_decoration_schedule_is_deterministic_and_between_four_and_nine_days() {
    let now = datetime!(2026-01-01 0:00 UTC);
    for ordinal in 0..64 {
        let first = scheduled_shelter_decoration_at([104; 32], ordinal, now);
        let second = scheduled_shelter_decoration_at([104; 32], ordinal, now);
        assert_eq!(first, second);
        assert!(first - now >= Duration::days(4));
        assert!(first - now <= Duration::days(9));
    }
}

#[test]
fn shelter_decorations_add_one_after_downtime_and_cap_at_six_unique_kinds() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    let mut world = World::new([105; 32], created, &desktop);
    let overdue = created + Duration::days(40);
    world.save.home.decorations.next_at_utc = created + Duration::days(4);
    world.tick(overdue, 0.05, &desktop);
    assert_eq!(world.save.home.decorations.decorations.len(), 1);
    assert!(world.save.home.decorations.next_at_utc >= overdue + Duration::days(4));
    assert!(world.save.home.decorations.next_at_utc <= overdue + Duration::days(9));
    world.tick(overdue, 0.05, &desktop);
    assert_eq!(world.save.home.decorations.decorations.len(), 1);
    assert_eq!(
        world
            .drain_events()
            .filter(|event| matches!(event, WorldEvent::ShelterDecorationAdded { .. }))
            .count(),
        1
    );

    for day in 1..=MAX_SHELTER_DECORATIONS + 3 {
        world.save.home.decorations.next_at_utc = overdue;
        world.tick(overdue + Duration::days(day as i64), 0.05, &desktop);
    }
    let decorations = &world.save.home.decorations.decorations;
    assert_eq!(decorations.len(), MAX_SHELTER_DECORATIONS);
    assert_eq!(
        decorations.iter().copied().collect::<BTreeSet<_>>().len(),
        decorations.len()
    );
}

#[test]
fn shelter_decoration_choice_reflects_memories_bonds_rituals_and_objects() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();

    let mut memories = World::new([106; 32], created, &desktop);
    memories.save.creatures[0].memory.ledge_seconds = u32::MAX;
    assert_eq!(
        preferred_shelter_decoration(&memories.save),
        Some(ShelterDecorationKind::Leaf)
    );

    let mut bonds = two_creature_world([107; 32], created);
    bonds.save.relationships[0].affinity = u8::MAX;
    bonds.save.relationships[0].familiarity = u8::MAX;
    assert_eq!(
        preferred_shelter_decoration(&bonds.save),
        Some(ShelterDecorationKind::Banner)
    );

    let mut ritual = World::new([108; 32], created, &desktop);
    ritual.save.ritual.last_kind = Some(RitualKind::Picnic);
    assert_eq!(
        preferred_shelter_decoration(&ritual.save),
        Some(ShelterDecorationKind::Flower)
    );

    let mut objects = World::new([109; 32], created, &desktop);
    objects.save.objects.objects.push(ColonyObject {
        id: 1,
        kind: ColonyObjectKind::Pebble,
        display: desktop.monitors[0].display_key,
        normalized_position: Point { x: 0.5, y: 0.9 },
        role: ColonyObjectRole::Curiosity,
    });
    assert_eq!(
        preferred_shelter_decoration(&objects.save),
        Some(ShelterDecorationKind::Stone)
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
            DwellingKind::MiniCottage,
            DwellingKind::MiniCottage,
        ],
    ];
    for cottages in &villages {
        for scale in 1..=4 {
            for corner in [HomeCorner::BottomLeft, HomeCorner::BottomRight] {
                home.corner = corner;
                let anchor = resolved_home_anchor(&home, monitor, scale, &policy).unwrap();
                let unit = f32::from(scale) / monitor.scale_factor.max(1.0);
                // Houses, porches and belongings share one walk, so no two footprints overlap.
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

                    // Its resident's porch is a lot of the same walk, beside the door.
                    let (rest_id, rest) = home_resting_position(
                        &home,
                        slot,
                        cottages,
                        &desktop.monitors,
                        &policy,
                        scale,
                    )
                    .unwrap();
                    assert_eq!(rest_id, monitor.id);
                    assert!((rest.y - anchor.y).abs() < 0.01);
                    assert!(
                        (rest.x - anchor.x)
                            * if corner == HomeCorner::BottomLeft {
                                1.0
                            } else {
                                -1.0
                            }
                            > (p.x - anchor.x).abs(),
                        "a resident stands further out than its own house"
                    );
                    push(rest.x, PORCH_WIDTH, &mut spans);
                }
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
                    // Belongings rest on the ground rather than stacking upward.
                    assert!((p.y - anchor.y).abs() <= 1.0 * unit + 0.01);
                    assert!(if corner == HomeCorner::BottomLeft {
                        p.x > anchor.x
                    } else {
                        p.x < anchor.x
                    });
                    push(p.x, 16.0, &mut spans);
                }
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
        vec![
            DwellingKind::Cottage,
            DwellingKind::MiniCottage,
            DwellingKind::MiniCottage
        ],
        "adults get a full cottage and minis a matching smaller one"
    );
}
