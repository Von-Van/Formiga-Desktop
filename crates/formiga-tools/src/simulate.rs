//! `formiga-tools simulate [DAYS]`: a colony run forward day by day, and who is in it after.

use crate::fixture_desktop;
use anyhow::{Result, bail};
use formiga_core::*;
use time::OffsetDateTime;

pub fn simulate(days: i64) -> Result<()> {
    if days < 0 {
        bail!("days must be non-negative");
    }
    let desktop = fixture_desktop();
    let created = OffsetDateTime::UNIX_EPOCH;
    let mut world = World::new([17; 32], created, &desktop);
    for day in 0..=days {
        let now = created + time::Duration::days(day);
        for _ in 0..1_200 {
            world.tick(now, 0.05, &desktop);
            world.drain_events().for_each(drop);
        }
    }
    println!(
        "simulated {days} days; colony contains {} creature(s)",
        world.save.creatures.len()
    );
    for creature in &world.save.creatures {
        // Report the modular body plan when there is one; legacy creatures report their family.
        let shape = creature.appearance.design.map_or_else(
            || format!("{:?}", creature.appearance.family),
            |design| format!("{:?}", design.body),
        );
        println!(
            "- {}: {shape}, {:?}, generation {}, {:?}",
            creature.id, creature.role, creature.generation, creature.state.action
        );
    }
    println!(
        "village: colony house plus {:?}",
        colony_cottages(&world.save.creatures)
    );
    Ok(())
}
