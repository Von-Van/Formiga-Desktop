use super::*;

/// A colony of three full-size companions and the minis that turned up in its first hours, with
/// the cottages arranged by hand, every bond warmed a little, and one companion with a memory of
/// its own to recognise it by.
fn colony() -> (World, DesktopSnapshot, OffsetDateTime) {
    let now = datetime!(2026-09-14 12:00 UTC);
    let desktop = desktop();
    let mut world = World::new([7; 32], now, &desktop);
    world.tick(now + Duration::hours(2), 0.05, &desktop);
    for seed in [[41; 32], [42; 32]] {
        world.add_designed_adult(seed, None, now, &desktop).unwrap();
    }
    for bond in &mut world.save.relationships {
        bond.affinity = 40 + (bond.a % 7) as u8 * 10;
        bond.familiarity = 120;
    }
    for creature in &mut world.save.creatures {
        creature.kept = false;
    }
    let adults: Vec<CreatureId> = world
        .save
        .creatures
        .iter()
        .filter(|creature| creature.role.is_adult())
        .map(|creature| creature.id)
        .collect();
    assert_eq!(adults.len(), 3);
    let creatures = world.save.creatures.clone();
    world
        .save
        .home
        .arrange_cottages(vec![adults[2], adults[1]], &creatures);
    assert!(!world.save.home.cottage_order.is_empty());
    let middle = world
        .save
        .creatures
        .iter_mut()
        .find(|creature| creature.id == adults[1])
        .unwrap();
    middle.memory.times_petted = 42;
    (world, desktop, now)
}

fn adults(world: &World) -> Vec<CreatureId> {
    world
        .save
        .creatures
        .iter()
        .filter(|creature| creature.role.is_adult())
        .map(|creature| creature.id)
        .collect()
}

fn bonds_with(world: &World, id: CreatureId) -> Vec<CreatureRelationship> {
    world
        .save
        .relationships
        .iter()
        .filter(|bond| bond.a == id || bond.b == id)
        .cloned()
        .collect()
}

/// A removed companion comes back exactly as it left: the same id in the same place in the
/// colony, its memories, its bonds, its minis and its cottage. Everyone else keeps what they did
/// in the meantime, and there is nothing further to undo.
#[test]
fn a_removed_companion_comes_back_exactly_as_it_left() {
    let (mut world, desktop, now) = colony();
    let target = adults(&world)[1];
    let before = world.save.clone();
    let name = world
        .save
        .creatures
        .iter()
        .find(|c| c.id == target)
        .unwrap()
        .name
        .clone();
    world
        .edit(ColonyEdit::Removed { name: name.clone() }, |world| {
            world.remove_colony_creature(target)
        })
        .unwrap();
    assert!(
        !world
            .save
            .creatures
            .iter()
            .any(|creature| creature.id == target)
    );
    assert!(!world.save.home.cottage_order.contains(&target));
    assert_eq!(
        world.last_edit(),
        Some(&ColonyEdit::Removed { name: name.clone() })
    );
    assert_eq!(
        world.last_edit().unwrap().describe(),
        format!("removing {name}")
    );

    // Life goes on before anyone thinks better of it.
    world.tick(
        now + Duration::hours(2) + Duration::minutes(3),
        0.05,
        &desktop,
    );
    let founder = world.save.creatures[0].id;
    world.save.creatures[0].memory.times_petted = 7;

    assert_eq!(world.undo_last_edit(), Ok(ColonyEdit::Removed { name }));
    // Everyone in the order they stood before, and anyone who arrived on their own since after
    // them.
    let ids = |save: &SaveFile| save.creatures.iter().map(|c| c.id).collect::<Vec<_>>();
    assert_eq!(ids(&world.save)[..before.creatures.len()], ids(&before)[..]);
    let back = world
        .save
        .creatures
        .iter()
        .find(|c| c.id == target)
        .unwrap();
    let was = before.creatures.iter().find(|c| c.id == target).unwrap();
    assert_eq!(back.memory, was.memory);
    assert_eq!(back.memory.times_petted, 42);
    assert_eq!(back.appearance, was.appearance);
    assert_eq!(back.personality, was.personality);
    assert_eq!(
        (back.name.as_str(), back.colony_order),
        (was.name.as_str(), was.colony_order)
    );
    for (now, then) in world.save.creatures.iter().zip(&before.creatures) {
        assert_eq!(
            now.role, then.role,
            "every mini has its own big version back"
        );
    }
    // Its bonds with everyone who was here are what they were; with anyone new, just begun.
    let known = |bond: &CreatureRelationship| {
        [bond.a, bond.b]
            .iter()
            .all(|id| before.creatures.iter().any(|c| c.id == *id))
    };
    let restored: Vec<_> = bonds_with(&world, target)
        .into_iter()
        .filter(known)
        .collect();
    assert_eq!(restored, bonds_with_save(&before, target));
    assert_eq!(world.save.home.cottage_order, before.home.cottage_order);
    let founder = world
        .save
        .creatures
        .iter()
        .find(|c| c.id == founder)
        .unwrap();
    assert_eq!(
        founder.memory.times_petted, 7,
        "the others keep what they did since"
    );

    assert_eq!(world.last_edit(), None);
    assert_eq!(world.undo_last_edit(), Err(UndoError::NothingToUndo));
    world.tick(
        now + Duration::hours(2) + Duration::minutes(6),
        0.05,
        &desktop,
    );
}

fn bonds_with_save(save: &SaveFile, id: CreatureId) -> Vec<CreatureRelationship> {
    save.relationships
        .iter()
        .filter(|bond| bond.a == id || bond.b == id)
        .cloned()
        .collect()
}

/// A replaced companion comes back in its own place, with its own id, bonds, minis and cottage,
/// and the replacement goes again without a trace in the colony.
#[test]
fn a_replaced_companion_comes_back_and_the_replacement_goes() {
    let (mut world, desktop, now) = colony();
    let target = adults(&world)[1];
    let before = world.save.clone();
    let newcomer = world
        .edit(
            ColonyEdit::Replaced {
                name: "Poppy".into(),
            },
            |world| world.replace_creature_with_adult(target, [77; 32], now, &desktop),
        )
        .unwrap();
    assert_eq!(world.save.home.cottage_order[1], newcomer);
    assert!(world.undo_last_edit().is_ok());
    assert!(
        !world
            .save
            .creatures
            .iter()
            .any(|creature| creature.id == newcomer)
    );
    assert!(
        !world
            .save
            .relationships
            .iter()
            .any(|bond| bond.a == newcomer || bond.b == newcomer)
    );
    let index = before
        .creatures
        .iter()
        .position(|c| c.id == target)
        .unwrap();
    assert_eq!(world.save.creatures[index].id, target);
    assert_eq!(
        world.save.creatures[index].memory,
        before.creatures[index].memory
    );
    assert_eq!(bonds_with(&world, target), bonds_with_save(&before, target));
    for (now, then) in world.save.creatures.iter().zip(&before.creatures) {
        assert_eq!(now.role, then.role);
    }
    assert_eq!(world.save.home.cottage_order, before.home.cottage_order);
    world.tick(now + Duration::minutes(1), 0.05, &desktop);
}

/// Taking back a companion welcomed from the studio sends it away again; one that arrived on its
/// own in the meantime stays; and a colony too full to bring someone back says so and keeps the
/// change to undo later.
#[test]
fn only_the_change_itself_is_taken_back() {
    let (mut world, desktop, now) = colony();
    world.remove_colony_creature(adults(&world)[2]).unwrap();
    let welcomed = world
        .edit(ColonyEdit::Welcomed, |world| {
            world.add_designed_adult([51; 32], None, now, &desktop)
        })
        .unwrap();
    let arrived = world
        .add_designed_adult([52; 32], None, now, &desktop)
        .unwrap();
    assert_eq!(world.undo_last_edit(), Ok(ColonyEdit::Welcomed));
    assert!(
        !world
            .save
            .creatures
            .iter()
            .any(|creature| creature.id == welcomed)
    );
    assert!(
        world
            .save
            .creatures
            .iter()
            .any(|creature| creature.id == arrived)
    );

    let target = adults(&world)[1];
    world
        .edit(
            ColonyEdit::Removed {
                name: "Poppy".into(),
            },
            |world| world.remove_colony_creature(target),
        )
        .unwrap();
    let mut seed = 60;
    while world.save.creatures.len() < MAX_COLONY_CREATURES {
        world
            .add_designed_adult([seed; 32], None, now, &desktop)
            .unwrap();
        seed += 1;
    }
    let full = world.save.clone();
    assert_eq!(world.undo_last_edit(), Err(UndoError::NoRoom));
    assert!(world.save == full, "a refused undo changes nothing");
    assert!(world.last_edit().is_some(), "and can be tried again later");
}

/// A layout change puts back exactly what it changed: the palette, a spot, the cottage order, the
/// home's corner, and the keepsakes' order with anything found since after them. A change that
/// fails, or changes nothing, leaves the one before it to undo.
#[test]
fn a_layout_change_puts_back_what_it_changed() {
    let (mut world, _, _) = colony();
    let order = world.save.home.cottage_order.clone();
    world.edit(ColonyEdit::PaintedVillage, |world| {
        world.save.home.palette = Some(VillagePalette::Autumn);
    });
    assert_eq!(world.undo_last_edit(), Ok(ColonyEdit::PaintedVillage));
    assert_eq!(world.save.home.palette, None);

    world.edit(ColonyEdit::Hangout(HangoutKind::Blanket), |world| {
        world.save.home.set_hangout(HangoutKind::Blanket, Some(0.4))
    });
    // Asking to remove the last full-size companion fails, and so is nothing to undo; neither is
    // painting the village the colours it already has.
    let founder = world.save.creatures[0].id;
    let before = world.save.clone();
    for id in adults(&world).into_iter().filter(|id| *id != founder) {
        world.remove_colony_creature(id).unwrap();
    }
    assert!(
        world
            .edit(
                ColonyEdit::Removed {
                    name: "Mallow".into()
                },
                |world| { world.remove_colony_creature(founder) }
            )
            .is_err()
    );
    world.edit(ColonyEdit::PaintedVillage, |_| {});
    assert_eq!(
        world.last_edit(),
        Some(&ColonyEdit::Hangout(HangoutKind::Blanket))
    );
    world.save = before;
    assert_eq!(
        world.undo_last_edit(),
        Ok(ColonyEdit::Hangout(HangoutKind::Blanket))
    );
    assert!(world.save.home.hangouts.is_empty());
    assert_eq!(world.save.home.cottage_order, order);

    world.save.objects.objects = (0..3)
        .map(|id| ColonyObject {
            id,
            kind: ColonyObjectKind::ALL[id as usize],
            role: ColonyObjectKind::ALL[id as usize].default_role(),
            ..Default::default()
        })
        .collect();
    world.edit(ColonyEdit::RearrangedKeepsakes, |world| {
        world.save.objects.objects.swap(0, 2);
        world.save.home.corner = match world.save.home.corner {
            HomeCorner::BottomLeft => HomeCorner::BottomRight,
            HomeCorner::BottomRight => HomeCorner::BottomLeft,
        };
    });
    let corner = world.save.home.corner;
    world.save.objects.objects.push(ColonyObject {
        id: 9,
        kind: ColonyObjectKind::Lamp,
        role: ColonyObjectKind::Lamp.default_role(),
        ..Default::default()
    });
    world.undo_last_edit().unwrap();
    let ids: Vec<u64> = world
        .save
        .objects
        .objects
        .iter()
        .map(|object| object.id)
        .collect();
    assert_eq!(ids, [0, 1, 2, 9]);
    assert_ne!(world.save.home.corner, corner);
}
