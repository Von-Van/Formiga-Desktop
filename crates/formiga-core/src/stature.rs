//! How big a companion is drawn against the others. Since 0.63.1 every adult has a stature of its
//! own, drawn from its seed, and a mini is its parent's size scaled down by the share its
//! generation is drawn at.
use crate::{Creature, SeedStream};
use rand::Rng;

/// The smallest and largest a companion stands, in percent of the average.
pub const STATURE_MIN: u8 = 85;
pub const STATURE_MAX: u8 = 115;

/// The logical size an average adult is drawn at; the renderer's unit of scale.
pub const AVERAGE_SIZE: f32 = 38.0;

/// How tall an adult from `source_seed` stands, from [`STATURE_MIN`] to [`STATURE_MAX`] percent
/// of the average: four in five within five points of it, one in ten small and one in ten large.
/// It comes from a stream of its own, so it is the same for a companion every time, on every
/// computer, and for one brought back from a code.
pub fn stature_percent(source_seed: [u8; 32]) -> u8 {
    let mut rng = SeedStream::new(source_seed).rng("creature-stature-v1", 0);
    let roll: f64 = rng.random();
    if roll < 0.1 {
        rng.random_range(STATURE_MIN..=94)
    } else if roll < 0.2 {
        rng.random_range(106..=STATURE_MAX)
    } else {
        rng.random_range(95..=105)
    }
}

/// The logical size a companion of `stature` is drawn at when its generation is drawn at
/// `scale_percent` of an adult. The adult's size is rounded first and then scaled, the way a mini
/// is scaled from its parent, so a companion comes out the same size however it was reached.
pub fn size_for(stature: u8, scale_percent: u8) -> u8 {
    let adult = (AVERAGE_SIZE * f32::from(stature) / 100.0).round();
    (adult * f32::from(scale_percent) / 100.0).round() as u8
}

/// The size of a companion drawn at `scale_percent` of an adult whose parent is `parent`: the
/// parent's own size, scaled from the share the parent is drawn at to the share the child is.
pub fn size_after_parent(parent: &Creature, scale_percent: u8) -> u8 {
    let parent_scale = f32::from(parent.display_scale_percent.max(1));
    (f32::from(parent.appearance.logical_size) * f32::from(scale_percent) / parent_scale).round()
        as u8
}

/// Give every companion in a colony its size, exactly as it would have been made with it. Each
/// takes the stature of the seed it came from, at the share its generation is drawn at: a mini
/// born in the colony shares its parent's seed, so it is its parent's size scaled down. A mini
/// of a companion adopted from a code was drawn from the colony's own seed instead, so it follows
/// that parent's size directly, as it does when it is born.
pub fn apply_statures(creatures: &mut [Creature]) {
    for creature in creatures.iter_mut() {
        creature.appearance.logical_size = size_for(
            stature_percent(creature.origin.source_colony_seed),
            creature.display_scale_percent,
        );
    }
    let adopted: Vec<Creature> = creatures
        .iter()
        .filter(|creature| is_adopted_root(creature))
        .cloned()
        .collect();
    for creature in creatures.iter_mut() {
        if let crate::CreatureRole::Mini { parent_id } = creature.role
            && let Some(parent) = adopted.iter().find(|parent| parent.id == parent_id)
            && creature.origin.source_colony_seed != parent.origin.source_colony_seed
        {
            creature.appearance.logical_size =
                size_after_parent(parent, creature.display_scale_percent);
        }
    }
}

/// An adult adopted from a code made for a mini: the first of its line in this colony, though
/// its code says it was drawn as a later generation.
pub fn is_adopted_root(creature: &Creature) -> bool {
    creature.role.is_adult() && creature.generation == 0 && creature.origin.source_generation > 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statures_spread_from_85_to_115_with_most_near_the_average() {
        let count = 10_000;
        let mut small = 0;
        let mut large = 0;
        let mut near = 0;
        for index in 0..count {
            let seed = SeedStream::new([71; 32]).bytes("stature", index);
            let stature = stature_percent(seed);
            assert!((STATURE_MIN..=STATURE_MAX).contains(&stature));
            assert_eq!(stature, stature_percent(seed));
            small += u32::from(stature < 95);
            large += u32::from(stature > 105);
            near += u32::from((95..=105).contains(&stature));
        }
        let share = |n: u32| f64::from(n) / count as f64;
        assert!(
            (0.08..=0.12).contains(&share(small)),
            "small {}",
            share(small)
        );
        assert!(
            (0.08..=0.12).contains(&share(large)),
            "large {}",
            share(large)
        );
        assert!((0.76..=0.84).contains(&share(near)), "near {}", share(near));
        assert_eq!(size_for(STATURE_MIN, 100), 32);
        assert_eq!(size_for(100, 100), 38);
        assert_eq!(size_for(STATURE_MAX, 100), 44);
        assert_eq!(size_for(100, 55), 21);
    }
}
