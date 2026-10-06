//! The boundary between a colony file and a running colony.
//!
//! A [`SaveFile`] is only the shape the file has: it can hold anything that shape can spell —
//! eighty habitat zones, a pair of companions bonded twice, a companion wearing a trinket the
//! colony never found. A [`ValidatedSave`] is a `SaveFile` that has been brought inside every
//! limit and made to agree with itself, and it can only be made here. A [`World`](crate::World)
//! is built from nothing else, so nothing in the running colony has to check again.
//!
//! The colony's own file is repaired wherever it can be — a colony is never lost to a recovery
//! screen over something that can be put right. A snapshot the person chose to restore from is
//! refused instead when it is not a whole colony, since they can choose another.
//!
//! [`violations`] lists what a validated colony never holds. Validation guarantees none of them,
//! and the long simulated runs check that a colony living for days never comes to hold one.

use crate::{
    CreatureRole, MAX_COLONY_CREATURES, MAX_COLONY_OBJECTS, MAX_FAVORITE_VISITORS, MAX_GARDENS,
    MAX_HABITAT_ZONES, MAX_HANGOUTS, MAX_HOUSE_DECORATIONS, MAX_JOURNAL_ENTRIES,
    MAX_NAME_CHARACTERS, MAX_ORNAMENTS, MAX_PINNED_ENTRIES, MAX_RELATIONSHIPS, MAX_ROUTINES,
    MAX_SCHEDULED_TRANSITIONS, SaveFile, TRINKET_VARIANTS,
};
use std::collections::BTreeSet;
use time::OffsetDateTime;

/// Farther from the origin, in points, than any arrangement of displays reaches.
const FARTHEST_PLACE: f32 = 1_000_000.0;

/// A colony brought inside every limit and made to agree with itself. Made only by validation;
/// read through `Deref`, and taken apart with [`ValidatedSave::into_inner`].
#[derive(Clone, Debug, PartialEq)]
pub struct ValidatedSave(SaveFile);

/// Why a snapshot is not a whole colony.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ImportRefusal {
    #[error("it has no companions")]
    Empty,
    #[error("it has more companions than a colony can hold")]
    Crowded,
    #[error("two of its companions are the same companion")]
    SharedIdentity,
    #[error("one of its companions has a name that cannot be used")]
    Name,
    #[error("it has no grown-up companion")]
    NoAdult,
    #[error("its habitat has more areas than a colony can keep")]
    Habitat,
}

impl From<SaveFile> for ValidatedSave {
    /// Repair a colony: everything the file holds is kept that can be, and only what cannot be
    /// true of a colony is put right.
    fn from(mut save: SaveFile) -> Self {
        save.save_version = crate::SAVE_VERSION;
        // Each companion once, and no more than a colony can hold. Only a damaged file has
        // either, and the first of each is the one the file meant.
        let mut seen = BTreeSet::new();
        save.creatures.retain(|creature| seen.insert(creature.id));
        save.creatures.truncate(MAX_COLONY_CREATURES);
        // A companion with no birth time — anyone from before they were kept — was born when its
        // generation implies. Before roles, which order minis by birth.
        for creature in &mut save.creatures {
            if creature.born_at_utc == OffsetDateTime::UNIX_EPOCH {
                creature.born_at_utc = legacy_birth_at(save.created_at_utc, creature.generation);
            }
        }
        // A name keeps as much of itself as a name can have; one with nothing left is given the
        // name a new companion of its generation would get.
        let mut names: Vec<String> = Vec::with_capacity(save.creatures.len());
        for creature in &mut save.creatures {
            let kept: String = creature
                .name
                .chars()
                .filter(|c| !c.is_control())
                .collect::<String>()
                .trim()
                .chars()
                .take(MAX_NAME_CHARACTERS)
                .collect();
            creature.name = match crate::validate_creature_name(&kept) {
                Ok(name) => name,
                Err(_) => {
                    crate::default_creature_name(save.colony_seed, creature.generation, &names)
                }
            };
            names.push(creature.name.clone());
        }
        save.companion.normalize();
        save.visitors.normalize();
        crate::world::normalize_colony_roles(&mut save);
        crate::world::normalize_relationships(&mut save);
        save.objects.objects.truncate(MAX_COLONY_OBJECTS);
        save.home.normalize_village();
        // A trip that cannot have happened is forgotten, though it is still counted.
        if save
            .trips
            .last
            .as_ref()
            .is_some_and(|trip| !trip_holds(trip))
        {
            save.trips.last = None;
        }
        if save.trips.last.is_some() {
            save.trips.count = save.trips.count.max(1);
        }
        // A souvenir kept twice is the one that came home first.
        let mut kept = Vec::new();
        save.trips.souvenirs.retain(|record| {
            let first = !kept.contains(&record.souvenir);
            kept.push(record.souvenir);
            first
        });
        // Anything that was never scheduled is scheduled from the colony's seed, as a new colony's
        // would be.
        if save.ritual.next_at_utc == OffsetDateTime::UNIX_EPOCH {
            save.ritual.next_at_utc = crate::world::scheduled_ritual_at(
                save.colony_seed,
                save.ritual.ordinal,
                save.maximum_seen_utc,
            );
        }
        if save.objects.next_at_utc == OffsetDateTime::UNIX_EPOCH {
            save.objects.next_at_utc = crate::world::scheduled_colony_object_at(
                save.colony_seed,
                save.objects.ordinal,
                save.maximum_seen_utc,
            );
        }
        if save.home.unlocks.next_at_utc == OffsetDateTime::UNIX_EPOCH {
            save.home.unlocks.next_at_utc = crate::world::scheduled_village_unlock_at(
                save.colony_seed,
                save.home.unlocks.ordinal,
                save.maximum_seen_utc,
            );
        }
        for creature in &mut save.creatures {
            // A sculpted form is kept as the design model draws it, and always beside a recipe,
            // which anything that cannot draw sculpts draws instead.
            if let Some(sculpt) = creature.appearance.sculpt.take() {
                let design = crate::forms::Design {
                    form: crate::forms::Form::Sculpted {
                        sculpt: sculpt.normalized(),
                    },
                    face: creature.appearance.face,
                };
                creature.appearance.design = creature
                    .appearance
                    .design
                    .or_else(|| design.fallback_recipe(None));
                creature.appearance.sculpt = design.form.sculpt().cloned();
            }
            creature.appearance.design = creature
                .appearance
                .design
                .map(crate::CreatureDesign::bounded);
            creature.origin.design = creature.appearance.design;
            creature.routines.len = creature.routines.len.min(MAX_ROUTINES as u8);
            // Somewhere no desktop reaches is nowhere: the first tick places it on the ground.
            let position = creature.state.position;
            if position.x.abs() > FARTHEST_PLACE || position.y.abs() > FARTHEST_PLACE {
                creature.state.position = crate::Point::default();
            }
            // Only something the colony has found can be worn: a file that says otherwise wears
            // nothing rather than something from nowhere.
            if creature
                .accessory
                .is_some_and(|accessory| !accessory.available(&save.companion.scrapbook))
            {
                creature.accessory = None;
            }
        }
        Self(save)
    }
}

impl ValidatedSave {
    /// Check a snapshot the person chose to restore from, refusing one that is not a whole colony,
    /// then repair it like any other.
    pub(super) fn import(save: SaveFile) -> Result<Self, ImportRefusal> {
        if save.creatures.is_empty() {
            return Err(ImportRefusal::Empty);
        }
        if save.creatures.len() > MAX_COLONY_CREATURES {
            return Err(ImportRefusal::Crowded);
        }
        let mut ids = BTreeSet::new();
        if !save
            .creatures
            .iter()
            .all(|creature| ids.insert(creature.id))
        {
            return Err(ImportRefusal::SharedIdentity);
        }
        if save
            .creatures
            .iter()
            .any(|creature| crate::validate_creature_name(&creature.name).is_err())
        {
            return Err(ImportRefusal::Name);
        }
        if !save
            .creatures
            .iter()
            .any(|creature| creature.role.is_adult())
        {
            return Err(ImportRefusal::NoAdult);
        }
        if save.settings.habitat.zones.len() > MAX_HABITAT_ZONES {
            return Err(ImportRefusal::Habitat);
        }
        Ok(Self::from(save))
    }

    /// The colony, to be run or changed. Whatever changes it next is the running colony's
    /// business; the next time it is read back, it is validated again.
    pub fn into_inner(self) -> SaveFile {
        self.0
    }
}

impl std::ops::Deref for ValidatedSave {
    type Target = SaveFile;

    fn deref(&self) -> &SaveFile {
        &self.0
    }
}

impl PartialEq<SaveFile> for ValidatedSave {
    fn eq(&self, other: &SaveFile) -> bool {
        self.0 == *other
    }
}

/// When a companion from before birth times were kept was born: the first at the colony's
/// founding, and each later generation as long after it as the old arrival schedule said.
fn legacy_birth_at(created_at_utc: OffsetDateTime, generation: u8) -> OffsetDateTime {
    const LEGACY_ARRIVAL_DAYS: [i64; 3] = [30, 90, 180];
    generation
        .checked_sub(1)
        .and_then(|index| LEGACY_ARRIVAL_DAYS.get(index as usize))
        .map_or(created_at_utc, |days| {
            created_at_utc + time::Duration::days(*days)
        })
}

/// Everything about `save` that a validated colony never holds, each said in a line. Empty for
/// any colony fresh from validation, and for a running colony at any moment it could be saved.
pub fn violations(save: &SaveFile) -> Vec<String> {
    let mut found = Vec::new();
    let mut check = |holds: bool, what: &dyn Fn() -> String| {
        if !holds {
            found.push(what());
        }
    };

    check(save.save_version == crate::SAVE_VERSION, &|| {
        format!("save version {} is not the current one", save.save_version)
    });

    // Who lives here.
    let ids: BTreeSet<_> = save.creatures.iter().map(|creature| creature.id).collect();
    check(save.creatures.len() <= MAX_COLONY_CREATURES, &|| {
        format!("{} companions", save.creatures.len())
    });
    check(ids.len() == save.creatures.len(), &|| {
        "two companions share an id".to_owned()
    });
    check(
        save.creatures.is_empty() || save.creatures.iter().any(|c| c.role.is_adult()),
        &|| "no companion is grown up".to_owned(),
    );
    for creature in &save.creatures {
        let name = &creature.name;
        if let CreatureRole::Mini { parent_id } = creature.role {
            check(
                save.creatures
                    .iter()
                    .any(|parent| parent.id == parent_id && parent.role.is_adult()),
                &|| format!("{name} is a mini with no grown-up parent here"),
            );
        }
        check(crate::validate_creature_name(name).is_ok(), &|| {
            format!("{name:?} is not a name a companion can have")
        });
        let state = &creature.state;
        check(
            [state.position.x, state.position.y]
                .iter()
                .all(|value| value.abs() <= FARTHEST_PLACE)
                && [state.velocity.x, state.velocity.y]
                    .iter()
                    .all(|value| value.is_finite()),
            &|| format!("{name} is somewhere that is not a place"),
        );
        check(
            creature
                .accessory
                .is_none_or(|accessory| accessory.available(&save.companion.scrapbook)),
            &|| format!("{name} wears something the colony never found"),
        );
        check(
            creature.appearance.design == creature.appearance.design.map(|d| d.bounded()),
            &|| format!("{name}'s recipe is out of bounds"),
        );
        check(
            creature.appearance.sculpt.as_ref().is_none_or(|sculpt| {
                *sculpt == sculpt.normalized() && creature.appearance.design.is_some()
            }),
            &|| format!("{name}'s sculpted form is out of bounds, or has no recipe beside it"),
        );
        check(creature.routines.len as usize <= MAX_ROUTINES, &|| {
            format!("{name} keeps {} routines", creature.routines.len)
        });
    }

    // Who knows whom.
    let mut pairs = BTreeSet::new();
    for relationship in &save.relationships {
        let (a, b) = (relationship.a, relationship.b);
        check(a < b && ids.contains(&a) && ids.contains(&b), &|| {
            format!("a bond between {a} and {b}, who are not a pair here")
        });
        check(pairs.insert((a, b)), &|| {
            format!("{a} and {b} are bonded twice")
        });
    }
    check(save.relationships.len() <= MAX_RELATIONSHIPS, &|| {
        format!("{} bonds", save.relationships.len())
    });
    let mut tallied = BTreeSet::new();
    for pair in &save.tallies {
        let (a, b) = (pair.a, pair.b);
        check(a < b && ids.contains(&a) && ids.contains(&b), &|| {
            format!("a tally for {a} and {b}, who are not a pair here")
        });
        check(tallied.insert((a, b)), &|| {
            format!("{a} and {b} are tallied twice")
        });
    }

    // What the notebook keeps.
    let companion = &save.companion;
    check(companion.journal.len() <= MAX_JOURNAL_ENTRIES, &|| {
        format!("{} journal moments", companion.journal.len())
    });
    check(companion.pins.len() <= MAX_PINNED_ENTRIES, &|| {
        format!("{} pins", companion.pins.len())
    });
    check(
        companion
            .scrapbook
            .windows(2)
            .all(|pair| pair[0].variant < pair[1].variant)
            && companion
                .scrapbook
                .iter()
                .all(|record| record.variant < TRINKET_VARIANTS),
        &|| "the scrapbook is not one record per find, in order".to_owned(),
    );
    let schedule = &companion.schedule.transitions;
    check(
        schedule.len() <= MAX_SCHEDULED_TRANSITIONS
            && schedule
                .iter()
                .all(|row| row.minute < 1440 && row.preset < 2 && row.days != 0),
        &|| "the weekly routine has a change that cannot happen".to_owned(),
    );
    let text_scale = companion.appearance.text_scale;
    check(
        (100..=150).contains(&text_scale) && text_scale.is_multiple_of(10),
        &|| format!("text at {text_scale}%"),
    );
    check(
        std::iter::once(&save.settings.habitat)
            .chain(companion.modes.iter().flatten().map(|mode| &mode.habitat))
            .all(|habitat| habitat.zones.len() <= MAX_HABITAT_ZONES),
        &|| "a habitat with more areas than a colony keeps".to_owned(),
    );
    check(
        save.visitors.favorites.len() <= MAX_FAVORITE_VISITORS,
        &|| format!("{} favourite visitors", save.visitors.favorites.len()),
    );
    check(
        save.trips.last.as_ref().is_none_or(trip_holds)
            && (save.trips.last.is_none() || save.trips.count >= 1),
        &|| "a trip that cannot have happened".to_owned(),
    );
    check(souvenirs_once(&save.trips.souvenirs), &|| {
        "a souvenir kept twice".to_owned()
    });

    // The village.
    check(save.objects.objects.len() <= MAX_COLONY_OBJECTS, &|| {
        format!("{} objects", save.objects.objects.len())
    });
    let home = &save.home;
    check(
        home.gardens.len() <= MAX_GARDENS
            && placed(home.gardens.iter().map(|patch| patch.along))
            && home
                .gardens
                .iter()
                .all(|patch| home.unlocks.gardens.contains(&patch.kind)),
        &|| "a garden that is not planted anywhere it can be".to_owned(),
    );
    check(
        home.hangouts.len() <= MAX_HANGOUTS
            && placed(home.hangouts.iter().map(|spot| spot.along))
            && home
                .hangouts
                .iter()
                .all(|spot| home.unlocks.hangouts.contains(&spot.kind)),
        &|| "a hangout spot that is not anywhere it can be".to_owned(),
    );
    check(
        home.ornaments.len() <= MAX_ORNAMENTS
            && placed(home.ornaments.iter().map(|spot| spot.along))
            && home
                .ornaments
                .iter()
                .all(|spot| home.unlocks.ornaments.contains(&spot.kind)),
        &|| "an ornament that is not anywhere it can be".to_owned(),
    );
    check(
        home.cottage_order.len() <= MAX_COLONY_CREATURES
            && home.cottage_order.iter().collect::<BTreeSet<_>>().len() == home.cottage_order.len(),
        &|| "a cottage stands twice".to_owned(),
    );
    check(
        home.dressing.len() <= MAX_COLONY_CREATURES
            && home.dressing.iter().all(|dressing| {
                dressing.decorations.len() <= MAX_HOUSE_DECORATIONS
                    && dressing
                        .decorations
                        .iter()
                        .all(|kind| home.unlocks.decorations.contains(kind))
            }),
        &|| "a house hangs decorations it cannot".to_owned(),
    );
    found.extend(
        save.day_book
            .violations(&ids.iter().copied().collect::<Vec<_>>()),
    );
    found
}

/// Whether a trip is written as one: its own identifier, and home no earlier than it got there.
fn trip_holds(trip: &crate::Trip) -> bool {
    crate::Trip::is_session(&trip.session) && trip.arrived_at_utc <= trip.left_at_utc
}

/// Whether every souvenir is kept at most once.
fn souvenirs_once(souvenirs: &[crate::SouvenirRecord]) -> bool {
    souvenirs.iter().enumerate().all(|(index, record)| {
        souvenirs[..index]
            .iter()
            .all(|earlier| earlier.souvenir != record.souvenir)
    })
}

/// Whether every one of these places along the village ground is on it.
fn placed(mut alongs: impl Iterator<Item = f32>) -> bool {
    alongs.all(|along| (0.0..=1.0).contains(&along))
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    fn colony() -> SaveFile {
        let desktop = crate::DesktopSnapshot::default();
        let now = datetime!(2026-10-02 9:00 UTC);
        let mut world = crate::World::new([71; 32], now, &desktop);
        world.tick(now + time::Duration::days(3), 0.05, &desktop);
        world.save
    }

    #[test]
    fn a_new_colony_holds_nothing_a_validated_one_never_does() {
        let save = colony();
        assert_eq!(violations(&save), Vec::<String>::new());
        assert_eq!(ValidatedSave::from(save.clone()), save);
    }

    /// Validating twice is validating once.
    #[test]
    fn validation_settles_in_one_pass() {
        let mut save = colony();
        save.companion.appearance.text_scale = 233;
        save.relationships.reverse();
        save.home.cottage_order.extend([1, 1, 1]);
        let once = ValidatedSave::from(save).into_inner();
        let twice = ValidatedSave::from(once.clone());
        assert_eq!(twice, once);
        assert_eq!(violations(&once), Vec::<String>::new());
    }

    /// Each thing a colony cannot hold is noticed, and repairing the colony puts it right.
    #[test]
    fn every_violation_is_noticed_and_repaired() {
        type Breakage = (&'static str, fn(&mut SaveFile));
        let breakages: [Breakage; 10] = [
            ("text", |save| save.companion.appearance.text_scale = 233),
            ("bond", |save| {
                let first = save.relationships[0];
                save.relationships.push(first);
            }),
            ("stranger", |save| {
                let mut stranger = save.relationships[0];
                stranger.b = 999_999;
                save.relationships.push(stranger);
            }),
            ("cottage", |save| {
                let id = save.creatures[0].id;
                save.home.cottage_order.extend([id, id]);
            }),
            ("routine", |save| {
                save.companion
                    .schedule
                    .transitions
                    .push(crate::ScheduledTransition {
                        days: 0,
                        minute: 2000,
                        preset: 7,
                    })
            }),
            ("objects", |save| {
                save.objects.objects = (0..20)
                    .map(|id| crate::ColonyObject {
                        id,
                        ..Default::default()
                    })
                    .collect()
            }),
            ("version", |save| save.save_version = 3),
            ("trip", |save| {
                save.trips.last = Some(crate::Trip {
                    session: "../../colony".to_owned(),
                    arrived_at_utc: save.created_at_utc,
                    left_at_utc: save.created_at_utc,
                })
            }),
            ("souvenir", |save| {
                let twice = crate::SouvenirRecord {
                    souvenir: crate::Souvenir::WellPenny,
                    brought_home_at_utc: save.created_at_utc,
                };
                save.trips.souvenirs = vec![twice, twice];
            }),
            ("orphan", |save| {
                for creature in &mut save.creatures {
                    creature.role = CreatureRole::Mini { parent_id: 404 };
                }
            }),
        ];
        for (what, breaks) in breakages {
            let mut save = colony();
            breaks(&mut save);
            assert!(!violations(&save).is_empty(), "{what} went unnoticed");
            let repaired = ValidatedSave::from(save);
            assert_eq!(violations(&repaired), Vec::<String>::new(), "{what}");
        }
    }

    #[test]
    fn a_snapshot_that_is_not_a_whole_colony_says_why() {
        let whole = colony();
        assert!(ValidatedSave::import(whole.clone()).is_ok());
        let mut empty = whole.clone();
        empty.creatures.clear();
        assert_eq!(ValidatedSave::import(empty), Err(ImportRefusal::Empty));
        let mut twins = whole.clone();
        let first = twins.creatures[0].clone();
        twins.creatures.push(first);
        assert_eq!(
            ValidatedSave::import(twins),
            Err(ImportRefusal::SharedIdentity)
        );
        let mut nameless = whole.clone();
        nameless.creatures[0].name.clear();
        assert_eq!(ValidatedSave::import(nameless), Err(ImportRefusal::Name));
        let mut minis = whole;
        for creature in &mut minis.creatures {
            creature.role = CreatureRole::Mini { parent_id: 1 };
        }
        assert_eq!(ValidatedSave::import(minis), Err(ImportRefusal::NoAdult));
    }
}
