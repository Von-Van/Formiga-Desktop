use super::*;

#[test]
fn shared_adoption_preserves_colony_and_exact_legacy_origins() {
    let now = datetime!(2026-09-14 12:00 UTC);
    let desktop = desktop();
    for generation in 0..=3 {
        let shared = SharedCreatureSeed {
            source_colony_seed: [88; 32],
            source_generation: generation,
            design: None,
        };
        let expected = World::from_shared_creature(shared, now, &desktop)
            .save
            .creatures
            .remove(0);
        let mut world = World::new([7; 32], now, &desktop);
        world.save.creatures[0].memory.times_petted = 123;
        let original = world.save.creatures[0].clone();
        let home = world.save.home.clone();
        let id = world
            .adopt_shared_creature(shared, None, now, &desktop)
            .unwrap();
        let adopted = world.save.creatures.iter().find(|c| c.id == id).unwrap();
        assert_eq!(adopted.appearance, expected.appearance);
        assert_eq!(adopted.personality, expected.personality);
        assert_eq!(adopted.origin, expected.origin);
        assert_eq!(adopted.memory.times_petted, 0);
        assert_eq!(world.save.creatures[0], original);
        assert_eq!(world.save.home, home);
        let before = world.save.clone();
        assert_eq!(
            world.adopt_shared_creature(shared, None, now, &desktop),
            Err(ColonyManagementError::DuplicateIdentity)
        );
        assert_eq!(world.save, before);
    }
}

#[test]
fn shared_replacement_respects_keep_and_reparents_minis() {
    let now = datetime!(2026-09-14 12:00 UTC);
    let desktop = desktop();
    let mut world = World::new([7; 32], now, &desktop);
    world.tick(now + Duration::hours(2), 0.05, &desktop);
    let old_id = world.save.creatures[0].id;
    let shared = SharedCreatureSeed {
        source_colony_seed: [81; 32],
        source_generation: 2,
        design: Some(CreatureDesign::generated([81; 32], 2, None)),
    };
    world.set_creature_kept(old_id, true).unwrap();
    let before = world.save.clone();
    assert_eq!(
        world.adopt_shared_creature(shared, Some(old_id), now, &desktop),
        Err(ColonyManagementError::CreatureKept)
    );
    assert_eq!(world.save, before);
    world.set_creature_kept(old_id, false).unwrap();
    let mini_ids: Vec<_> = world
        .save
        .creatures
        .iter()
        .filter(|c| c.role.parent_id() == Some(old_id))
        .map(|c| c.id)
        .collect();
    assert!(!mini_ids.is_empty());
    let id = world
        .adopt_shared_creature(shared, Some(old_id), now, &desktop)
        .unwrap();
    for mini in world
        .save
        .creatures
        .iter()
        .filter(|c| mini_ids.contains(&c.id))
    {
        assert_eq!(mini.role.parent_id(), Some(id));
    }
    assert_eq!(
        world
            .save
            .creatures
            .iter()
            .find(|c| c.id == id)
            .unwrap()
            .origin
            .design,
        shared.design
    );
}

#[test]
fn shared_adoption_refuses_full_colony_without_mutation() {
    let now = datetime!(2026-09-14 12:00 UTC);
    let desktop = desktop();
    let mut world = World::new([7; 32], now, &desktop);
    world.tick(now + Duration::days(40), 0.05, &desktop);
    assert_eq!(world.save.creatures.len(), 4);
    let before = world.save.clone();
    let shared = SharedCreatureSeed {
        source_colony_seed: [81; 32],
        source_generation: 0,
        design: None,
    };
    assert_eq!(
        world.adopt_shared_creature(shared, None, now, &desktop),
        Err(ColonyManagementError::ColonyFull)
    );
    assert_eq!(world.save, before);
}

#[test]
fn generated_colonies_enforce_total_adult_and_mini_caps() {
    let now = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    let mut world = World::new([120; 32], now, &desktop);
    world.add_generated_adult([121; 32], now, &desktop).unwrap();
    world.add_generated_adult([122; 32], now, &desktop).unwrap();
    assert_eq!(adult_count(&world.save.creatures), MAX_ADULT_CREATURES);
    assert_eq!(
        world.add_generated_adult([123; 32], now, &desktop),
        Err(ColonyManagementError::AdultLimit)
    );
    world.tick(now + Duration::hours(1), 0.05, &desktop);
    assert_eq!(world.save.creatures.len(), MAX_COLONY_CREATURES);
    assert!(
        world
            .save
            .creatures
            .iter()
            .any(|creature| { !creature.role.is_adult() && creature.display_scale_percent < 100 })
    );
    assert_eq!(
        world.add_generated_adult([124; 32], now, &desktop),
        Err(ColonyManagementError::ColonyFull)
    );
    for adult in world
        .save
        .creatures
        .iter()
        .filter(|creature| creature.role.is_adult())
    {
        assert!(mini_count_for_parent(&world.save.creatures, adult.id) <= MAX_MINIS_PER_ADULT);
    }
}

#[test]
fn minis_are_balanced_across_two_adults_and_prefer_the_oldest_of_three() {
    let now = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    let mut two_adults = World::new([125; 32], now, &desktop);
    let second = two_adults
        .add_generated_adult([126; 32], now + Duration::seconds(1), &desktop)
        .unwrap();
    let first = two_adults.save.creatures[0].id;
    two_adults.set_creature_kept(second, false).unwrap();
    two_adults.tick(
        now + Duration::hours(1) + Duration::seconds(1),
        0.05,
        &desktop,
    );
    assert_eq!(two_adults.save.creatures.len(), 4);
    assert_eq!(mini_count_for_parent(&two_adults.save.creatures, first), 1);
    assert_eq!(mini_count_for_parent(&two_adults.save.creatures, second), 1);

    let mut three_adults = World::new([127; 32], now, &desktop);
    let oldest = three_adults.save.creatures[0].id;
    three_adults
        .add_generated_adult([128; 32], now + Duration::seconds(1), &desktop)
        .unwrap();
    three_adults
        .add_generated_adult([129; 32], now + Duration::seconds(2), &desktop)
        .unwrap();
    three_adults.tick(now + Duration::hours(1), 0.05, &desktop);
    assert_eq!(three_adults.save.creatures.len(), 4);
    assert_eq!(
        mini_count_for_parent(&three_adults.save.creatures, oldest),
        1
    );
}

#[test]
fn keep_protects_replacement_and_bulk_regeneration_preserves_kept_creatures() {
    let now = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    let mut world = World::new([131; 32], now, &desktop);
    let protected = world.save.creatures[0].id;
    assert_eq!(
        world.replace_creature_with_adult(protected, [132; 32], now, &desktop),
        Err(ColonyManagementError::CreatureKept)
    );
    let replaceable = world.add_generated_adult([133; 32], now, &desktop).unwrap();
    world.tick(now + Duration::hours(1), 0.05, &desktop);
    let removable_mini = world
        .save
        .creatures
        .iter()
        .find(|creature| !creature.role.is_adult())
        .unwrap()
        .id;
    world.set_creature_kept(replaceable, false).unwrap();
    world.set_creature_kept(removable_mini, false).unwrap();
    assert_eq!(world.regenerate_unkept(&[[134; 32]], now, &desktop), 2);
    assert!(
        world
            .save
            .creatures
            .iter()
            .any(|creature| creature.id == protected)
    );
    assert!(
        !world
            .save
            .creatures
            .iter()
            .any(|creature| creature.id == replaceable || creature.id == removable_mini)
    );
    assert!(world.save.creatures.iter().all(|creature| creature.kept));
}

#[test]
fn removing_an_adult_reparents_minis_but_never_removes_the_last_adult() {
    let now = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    let mut world = World::new([135; 32], now, &desktop);
    let first = world.save.creatures[0].id;
    assert_eq!(
        world.remove_colony_creature(first),
        Err(ColonyManagementError::LastAdult)
    );
    let second = world
        .add_generated_adult([136; 32], now + Duration::seconds(1), &desktop)
        .unwrap();
    world.tick(
        now + Duration::hours(1) + Duration::seconds(1),
        0.05,
        &desktop,
    );
    world.remove_colony_creature(first).unwrap();
    assert!(
        world.save.creatures.iter().all(|creature| {
            creature.role.is_adult() || creature.role.parent_id() == Some(second)
        })
    );
}

#[test]
fn shared_seed_import_reproduces_all_four_source_generations_byte_for_byte() {
    let original_birth = datetime!(2024-01-02 3:04 UTC);
    let imported_birth = datetime!(2026-09-02 5:06 UTC);
    let desktop = desktop();
    for source_generation in 0_u8..=3 {
        let shared = SharedCreatureSeed {
            source_colony_seed: [source_generation.wrapping_mul(41).wrapping_add(17); 32],
            source_generation,
            design: None,
        };
        let expected = generate_source_creature(shared, original_birth, &desktop);
        let imported = World::from_shared_creature(shared, imported_birth, &desktop);
        let actual = &imported.save.creatures[0];
        assert_eq!(actual.id, expected.id);
        assert_eq!(actual.origin, CreatureOrigin::from(shared));
        assert_eq!(actual.appearance, expected.appearance);
        assert_eq!(actual.personality, expected.personality);
        assert_eq!(actual.display_scale_percent, expected.display_scale_percent);
        assert_eq!(actual.behavior_seed, expected.behavior_seed);
        assert_eq!(actual.name, expected.name);
        assert_eq!(actual.generation, 0);
        assert_eq!(actual.colony_order, 0);
        assert_eq!(actual.born_at_utc, imported_birth);
        assert_eq!(actual.memory, CreatureMemory::default());
        assert_eq!(actual.tendencies, LearnedTendencies::default());
        assert_eq!(actual.routines, RoutineTable::default());
        assert!(imported.save.relationships.is_empty());
        assert_ne!(imported.save.colony_seed, shared.source_colony_seed);
    }
}

#[test]
fn accepted_design_survives_add_replace_save_and_sharing_with_related_minis() {
    let now = datetime!(2026-09-12 0:00 UTC);
    let desktop = desktop();
    let mut world = World::new([6; 32], now, &desktop);
    let design = crate::CreatureDesign::generated([88; 32], 0, None);
    let id = world
        .add_designed_adult([77; 32], Some(design), now, &desktop)
        .unwrap();
    let added = world.save.creatures.iter().find(|c| c.id == id).unwrap();
    assert_eq!(added.appearance.design, Some(design));
    let code = crate::encode_creature_seed(added.origin);
    let imported =
        World::from_shared_creature(crate::decode_creature_seed(&code).unwrap(), now, &desktop);
    assert_eq!(imported.save.creatures[0].appearance, added.appearance);
    assert_eq!(imported.save.creatures[0].personality, added.personality);
    let serialized = serde_json::to_vec(&world.save).unwrap();
    let mut restored = World::from_save(serde_json::from_slice(&serialized).unwrap());
    assert_eq!(
        restored
            .save
            .creatures
            .iter()
            .find(|c| c.id == id)
            .unwrap()
            .appearance
            .design,
        Some(design)
    );
    restored
        .save
        .creatures
        .iter_mut()
        .find(|c| c.id == id)
        .unwrap()
        .kept = false;
    restored
        .replace_creature_with_design(id, [78; 32], Some(design), now, &desktop)
        .unwrap();
    assert!(
        restored
            .save
            .creatures
            .iter()
            .any(|c| c.appearance.design == Some(design))
    );
    world.tick(now + Duration::hours(1), 0.05, &desktop);
    for mini in world.save.creatures.iter().filter(|c| !c.role.is_adult()) {
        let parent = world
            .save
            .creatures
            .iter()
            .find(|c| Some(c.id) == mini.role.parent_id())
            .unwrap();
        assert_eq!(
            mini.appearance.design.unwrap().body,
            parent.appearance.design.unwrap().body
        );
        let imported = World::from_shared_creature(
            crate::decode_creature_seed(&crate::encode_creature_seed(mini.origin)).unwrap(),
            now,
            &desktop,
        );
        assert_eq!(imported.save.creatures[0].appearance, mini.appearance);
    }
}

#[test]
fn imported_creature_gets_a_distinct_companion_lineage() {
    let now = datetime!(2026-09-02 5:06 UTC);
    let desktop = desktop();
    let shared = SharedCreatureSeed {
        source_colony_seed: [211; 32],
        source_generation: 1,
        design: None,
    };
    let mut imported = World::from_shared_creature(shared, now, &desktop);
    imported.tick(now + Duration::hours(1), 0.05, &desktop);
    assert_eq!(imported.save.creatures.len(), 2);
    assert_eq!(
        imported.save.creatures[0].origin,
        CreatureOrigin::from(shared)
    );
    assert_eq!(
        imported.save.creatures[1].origin.source_colony_seed,
        imported.save.colony_seed
    );
    assert_ne!(
        imported.save.creatures[1].origin.source_colony_seed,
        shared.source_colony_seed
    );
}
