use super::*;

/// Echo Fox Mode draws everybody as the fox, newcomers too, and never reaches the save: the
/// colony written while it is on is the colony written without it, and switching it off, or
/// loading that save again, brings everybody back as themselves.
#[test]
fn echo_fox_mode_reaches_everybody_and_never_the_save() {
    let now = datetime!(2026-09-14 12:00 UTC);
    let desktop = desktop();
    let mut world = World::new([7; 32], now, &desktop);
    world.tick(now, 0.05, &desktop);
    let before = serde_json::to_string(&world.save).unwrap();
    let looks: Vec<AppearanceGenome> = world
        .save
        .creatures
        .iter()
        .map(|creature| creature.appearance.clone())
        .collect();

    world.set_echo_fox(true);
    assert!(world.echo_fox());
    assert!(world.save.creatures.iter().all(|c| c.appearance.echo_fox));
    world
        .add_designed_adult([41; 32], None, now, &desktop)
        .unwrap();
    world.tick(now + Duration::seconds(1), 0.05, &desktop);
    assert!(world.save.creatures.iter().all(|c| c.appearance.echo_fox));

    let during = serde_json::to_string(&world.save).unwrap();
    assert!(!during.contains("echo_fox"));
    let reloaded: SaveFile = serde_json::from_str(&during).unwrap();
    assert!(reloaded.creatures.iter().all(|c| !c.appearance.echo_fox));

    world.set_echo_fox(false);
    assert!(world.save.creatures.iter().all(|c| !c.appearance.echo_fox));
    for (creature, look) in world.save.creatures.iter().zip(&looks) {
        assert_eq!(&creature.appearance, look);
    }
    assert!(before.len() < during.len(), "the newcomer is kept");
}
