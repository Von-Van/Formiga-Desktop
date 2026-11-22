//! `--cast`: draw chosen companions in the storefront pictures instead of the sample ones.
//!
//!   cargo run -p formiga-tools -- postcard-sheet --cast Lumi@colony.json --cast four-pawed:3
//!
//! Each `--cast` names one companion, in the order they take the stage:
//!
//!   NAME@FILE        the companion called NAME in a colony file, read the way Desktop reads one
//!                    (the file is only read, never written)
//!   four-pawed[:N]   the Nth cute four-pawed companion the generator draws (0 when N is left out)
//!
//! The postcards, the social preview, the demo animation, the temperament sheet and the sticker
//! take a cast. With none they draw the sample companions they always have.

use crate::fixture_desktop;
use anyhow::{Context, Result, bail};
use formiga_core::*;
use std::path::Path;
use time::OffsetDateTime;

/// Every `--cast` given, read into companions in the order given; empty when there is none.
pub fn cast_argument(args: &[String]) -> Result<Vec<Creature>> {
    args.windows(2)
        .filter(|window| window[0] == "--cast")
        .map(|window| companion(&window[1]))
        .collect()
}

fn companion(spec: &str) -> Result<Creature> {
    if let Some(rest) = spec.strip_prefix("four-pawed") {
        let nth = match rest.strip_prefix(':') {
            None if rest.is_empty() => 0,
            Some(number) => number
                .parse()
                .with_context(|| format!("{number:?} in --cast {spec:?} is not a number"))?,
            None => bail!("--cast {spec:?}: write four-pawed or four-pawed:N"),
        };
        return Ok(four_pawed(nth));
    }
    let Some((name, file)) = spec.split_once('@') else {
        bail!("--cast {spec:?}: write NAME@colony.json or four-pawed[:N]");
    };
    from_colony(name, Path::new(file))
}

/// The companion called `name` in a colony file. The file is decoded and checked by Desktop's
/// own reading, so a companion comes out exactly as Desktop would draw it.
fn from_colony(name: &str, file: &Path) -> Result<Creature> {
    let bytes = std::fs::read(file).with_context(|| format!("read {}", file.display()))?;
    let save = decode(&bytes)
        .with_context(|| format!("{} does not read as a colony", file.display()))?
        .into_inner();
    let names: Vec<String> = save.creatures.iter().map(|c| c.name.clone()).collect();
    save.creatures
        .into_iter()
        .find(|creature| creature.name.eq_ignore_ascii_case(name))
        .with_context(|| {
            format!(
                "no companion called {name:?} in {}; it has {}",
                file.display(),
                names.join(", ")
            )
        })
}

/// The `nth` cute four-pawed companion drawn from a fixed stream, so the same number always
/// brings the same companion.
pub fn four_pawed(nth: usize) -> Creature {
    let streams = SeedStream::new([47; 32]);
    let (seed, design) = (0_u64..)
        .map(|index| {
            let seed = streams.bytes("cast-four-pawed", index);
            (seed, CreatureDesign::drawn(seed, 0))
        })
        .filter(|(_, (design, strangeness))| {
            design.body == BodyPlan::Long && *strangeness == Strangeness::Cute
        })
        .map(|(seed, (design, _))| (seed, design))
        .nth(nth)
        .expect("the generator draws four-pawed companions");
    let mut creature = World::preview_adult(seed, OffsetDateTime::UNIX_EPOCH, &fixture_desktop());
    apply_creature_design(&mut creature, Some(design));
    creature
}

/// Puts the cast in a colony's place: each of the first companions takes on a cast member's
/// looks, name and temperament (keeping its own place in the colony, so the house, friendships
/// and belongings stay as they were), and any companion past the cast leaves.
pub fn recast(save: &mut SaveFile, cast: &[Creature]) {
    if cast.is_empty() {
        return;
    }
    save.creatures.truncate(cast.len());
    for (stand_in, star) in save.creatures.iter_mut().zip(cast) {
        stand_in.name = star.name.clone();
        stand_in.appearance = star.appearance.clone();
        stand_in.origin = star.origin;
        stand_in.personality = star.personality.clone();
        stand_in.temperament = star.temperament;
        stand_in.display_scale_percent = star.display_scale_percent;
        stand_in.generation = star.generation;
        stand_in.accessory = star.accessory;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn four_pawed_companions_are_four_pawed_and_repeatable() {
        let first = four_pawed(2);
        let again = four_pawed(2);
        assert_eq!(
            first.appearance.design.map(|design| design.body),
            Some(BodyPlan::Long)
        );
        assert_eq!(first.appearance.design, again.appearance.design);
        assert_ne!(four_pawed(0).appearance.design, first.appearance.design);
    }

    #[test]
    fn a_cast_is_read_in_order_and_bad_ones_are_refused() {
        let args: Vec<String> = ["tool", "--cast", "four-pawed", "--cast", "four-pawed:1"]
            .map(String::from)
            .to_vec();
        let cast = cast_argument(&args).unwrap();
        assert_eq!(cast.len(), 2);
        assert_eq!(cast[0].appearance.design, four_pawed(0).appearance.design);
        assert!(companion("four-pawed:x").is_err());
        assert!(companion("Lumi").is_err());
        assert!(companion("Lumi@/no/such/colony.json").is_err());
    }

    #[test]
    fn recasting_keeps_the_colony_and_drops_the_rest() {
        let mut save = crate::colony_card::sample_colony();
        let ids: Vec<_> = save.creatures.iter().map(|c| c.id).collect();
        let cast = vec![four_pawed(0), four_pawed(1)];
        recast(&mut save, &cast);
        assert_eq!(save.creatures.len(), 2);
        assert_eq!(save.creatures[0].id, ids[0]);
        assert_eq!(
            save.creatures[1].appearance.design,
            cast[1].appearance.design
        );
    }
}
