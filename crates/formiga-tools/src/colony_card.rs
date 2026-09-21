//! `formiga-tools colony-card`: the whole-colony portrait, for the docs.

use crate::{fixture_desktop, write_png};
use anyhow::Result;
use formiga_art::{COLONY_CARD_HEIGHT, COLONY_CARD_WIDTH, ColonyCardRenderer};
use formiga_core::*;
use sha2::{Digest, Sha256};
use std::path::PathBuf;

pub fn run(path: PathBuf) -> Result<()> {
    let card = ColonyCardRenderer::render(&sample_colony());
    write_png(
        &path,
        COLONY_CARD_WIDTH,
        COLONY_CARD_HEIGHT,
        &card.rgba_bytes(),
    )?;
    println!("wrote {}", path.display());
    Ok(())
}

/// A colony grown the way a real one grows: seeded, then left to live for a season so it has
/// companions, a decorated house, and belongings on the ground line.
pub fn sample_colony() -> SaveFile {
    let mut seed = [0_u8; 32];
    seed.copy_from_slice(&Sha256::digest(b"formiga-colony-card-preview"));
    let desktop = fixture_desktop();
    let created = time::macros::datetime!(2026-03-08 09:15 UTC);
    let mut world = World::new(seed, created, &desktop);
    world.tick(created + time::Duration::days(210), 0.05, &desktop);
    for (creature, name) in world
        .save
        .creatures
        .iter_mut()
        .zip(["Mallow", "Juniper", "Pebble", "Tofu"])
    {
        creature.name = name.into();
    }
    world.save.home.decorations.decorations = vec![
        ShelterDecorationKind::Banner,
        ShelterDecorationKind::Lamp,
        ShelterDecorationKind::Flower,
        ShelterDecorationKind::Leaf,
    ];
    world.save.objects.objects = [
        ColonyObjectKind::Lamp,
        ColonyObjectKind::Plant,
        ColonyObjectKind::Toy,
        ColonyObjectKind::Cup,
    ]
    .into_iter()
    .enumerate()
    .map(|(index, kind)| ColonyObject {
        id: index as u64,
        kind,
        role: kind.default_role(),
        ..Default::default()
    })
    .collect();
    world.save
}
