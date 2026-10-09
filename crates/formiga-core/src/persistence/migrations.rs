//! Bringing an older colony file forward to the current version, one version at a time.
//!
//! Every version has exactly one [`Step`] here, which reads a file of that version and leaves one
//! of the next. A step has two halves:
//!
//! - `raw` changes the file's JSON while it is still in the older shape, which is the only time a
//!   field that has since been removed or reshaped can be read;
//! - `settle` finishes what can only be done once the file is in the current shape — anything that
//!   needs the colony's own rules, such as drawing a home from its seed.
//!
//! Every `raw` half runs first, oldest version first, then the file is parsed once into the
//! current shape, and then every `settle` half runs in the same order. Most versions only added
//! something an older colony simply did not have, and their steps do nothing: they are listed
//! anyway, so the table is the whole history and each version says what it changed.
//!
//! What a step never does is bring a colony inside its limits or repair a reference. That is
//! validation's job, and it is done for every file, of any version, after the last step.

use super::PersistenceError;
use crate::SaveFile;
use serde_json::{Map, Value, json};

/// The oldest colony file that can be read.
const FIRST_VERSION: u32 = 1;

/// One version's change to the colony file.
struct Step {
    /// The version the step reads. It leaves the file at the next one.
    from: u32,
    /// The change to the file's JSON, in the older shape.
    raw: fn(&mut Value, &mut Carried),
    /// What is finished once the file is in the current shape.
    settle: fn(&mut SaveFile, &Carried),
}

impl Step {
    /// A step that only changes the file's JSON.
    const fn raw(from: u32, raw: fn(&mut Value, &mut Carried)) -> Self {
        Self {
            from,
            raw,
            settle: |_, _| {},
        }
    }

    /// A step that changes nothing in the file and finishes in the current shape.
    const fn settle(from: u32, settle: fn(&mut SaveFile, &Carried)) -> Self {
        Self {
            from,
            raw: |_, _| {},
            settle,
        }
    }

    /// A version that only added something an older colony never had.
    const fn adds_only(from: u32) -> Self {
        Self {
            from,
            raw: |_, _| {},
            settle: |_, _| {},
        }
    }
}

/// What a step reads from the older file that only its `settle` half, after parsing, can use.
#[derive(Default)]
struct Carried {
    /// A version-1 colony kept to its primary display.
    primary_display_only: bool,
    /// The decorations a colony before version 19 had earned, and which of them it had hidden.
    legacy_decorations: (Vec<crate::ShelterDecorationKind>, u8),
}

/// Every version's step, oldest first: `STEPS[n]` reads version `n + 1`.
const STEPS: &[Step] = &[
    // 2: the "primary display only" switch became the habitat's Primary Display preset.
    Step {
        from: 1,
        raw: |value, carried| {
            carried.primary_display_only = value
                .pointer("/settings/primary_display_only")
                .and_then(Value::as_bool)
                .unwrap_or(false);
        },
        settle: |save, carried| {
            if carried.primary_display_only {
                save.settings.habitat.preset = crate::HabitatPreset::PrimaryDisplay;
            }
        },
    },
    // 3: each creature's art genes were reshaped into head appendages, a face, forelimbs and an
    // effect motif, resolved from the face signature it already had.
    Step::raw(2, |value, _| {
        for creature in creatures_mut(value) {
            if let Some(appearance) = creature
                .get_mut("appearance")
                .and_then(Value::as_object_mut)
            {
                reshape_appearance(appearance);
            }
        }
    }),
    // 4: the colony gained a home, drawn from its seed.
    Step::settle(3, |save, _| {
        save.home =
            crate::ColonyHome::from_seed(save.colony_seed, None, None, Some(save.maximum_seen_utc));
    }),
    // 5: every creature gained a birth time. Validation gives one with none the time its
    // generation implies, for this and any later file.
    Step::adds_only(4),
    // 6: names, origins, memories and learned tendencies, and the twelve strongest string-keyed
    // habits kept as a fixed routine table.
    Step::raw(5, |value, _| lived_experience(value)),
    // 7: each creature's own map of feelings became one shared record per pair.
    Step::raw(6, |value, _| shared_relationships(value)),
    // 8: rituals, scheduled by validation from the colony's seed.
    Step::adds_only(7),
    // 9: the colony's objects, scheduled the same way.
    Step::adds_only(8),
    // 10: the colony house's decorations.
    Step::adds_only(9),
    // 11: the first creature became the adult and every later one a mini in its care, all kept.
    Step::raw(10, |value, _| colony_roles(value)),
    // 12: recipes. A colony without them is drawn exactly as it was.
    Step::adds_only(11),
    // 13: the companion's own records: the journal, and whether the introduction was seen. An
    // older colony counts as having seen it.
    Step::adds_only(12),
    // 14: pins, the scrapbook, appearance preferences and weekly routines. Each starts empty on
    // an older colony and is never invented from an earlier count, so anything in the file that
    // only happens to share one of their names is dropped.
    Step::raw(13, |value, _| {
        if let Some(companion) = value.get_mut("companion").and_then(Value::as_object_mut) {
            for field in ["pins", "scrapbook", "schedule", "appearance"] {
                companion.remove(field);
            }
        }
    }),
    // 15: visitors and their guest book. An older colony has had neither, so both start empty
    // rather than being read from a field that only happens to share the name.
    Step::raw(14, |value, _| {
        if let Some(root) = value.as_object_mut() {
            root.remove("visitors");
        }
    }),
    // 16: room for a sixth companion. Nothing changed in the file; the version moved so an older
    // build refuses a colony of six rather than quietly dropping one.
    Step::adds_only(15),
    // 17: classic parts in recipes, favourite visitors and roaming leanings. A recipe without
    // classic parts is a plain modular one, an older colony has no favourites, and a missing
    // leaning is `Anywhere`.
    Step::adds_only(16),
    // 18: a house type chosen by hand. An older colony has chosen none.
    Step::adds_only(17),
    // 19: the colony house's earned decorations became what the whole village chooses from. The
    // fields that held them are gone, so they are read here and hung again once the file is in
    // the current shape.
    Step {
        from: 18,
        raw: |value, carried| {
            carried.legacy_decorations = take_legacy_decorations(value).unwrap_or_default();
        },
        settle: |save, carried| village_unlocks(save, &carried.legacy_decorations),
    },
    // 20: temperaments, archetype recipes with authored faces, and learned leanings as fractions.
    // A companion without a temperament reads one from its own values, and whole-number leanings
    // read as the same values.
    Step::adds_only(19),
    // 21: details in recipes. A recipe without them has none.
    Step::adds_only(20),
    // 22: every companion has a stature: the size its own seed gives it, and a mini its parent's
    // scaled down, exactly as a new one would.
    Step::settle(21, |save, _| crate::apply_statures(&mut save.creatures)),
    // 23: a tally of what each pair has done together, and the moment the journal was last
    // read. Nothing was counted before, so every tally starts at nothing; the journal counts as
    // read up to its newest moment, so an older colony is not shown news it has already seen.
    Step::settle(22, |save, _| {
        save.tallies.clear();
        save.companion.journal_seen_until = save.companion.journal.iter().map(|e| e.at).max();
    }),
    // 24: the notebook's page of wonders, and the trinkets found today. An older colony has
    // found none of either.
    Step::adds_only(23),
    // 25: the day book, for the Today page to compare one day with another. An older colony has
    // counted nothing, and the book only says anything about days it counted.
    Step::adds_only(24),
    // 26: trips away on the train, and the journal moment for coming home from one. An older
    // colony has never been anywhere.
    Step::adds_only(25),
    // 27: the souvenirs a colony has brought home from Formiga Hill. An older colony has brought
    // none home.
    Step::adds_only(26),
    // 28: four more of Formiga Hill's souvenirs, and the journal moment for time spent inside a
    // house in Formiga Home. An older colony has neither.
    Step::adds_only(27),
    // 29: a picture the village can be laid out on instead of its strip. An older colony keeps
    // to the strip.
    Step::adds_only(28),
];

/// The version a raw file says it is, or 0 when it names none.
pub(super) fn version_of(value: &Value) -> u32 {
    value
        .get("save_version")
        .and_then(Value::as_u64)
        .and_then(|version| u32::try_from(version).ok())
        .unwrap_or_default()
}

/// Bring a raw colony file forward to the current version and parse it into the current shape.
/// The result is not yet validated.
pub(super) fn upgrade(mut value: Value) -> Result<SaveFile, PersistenceError> {
    let from = version_of(&value);
    if from == crate::SAVE_VERSION {
        return Ok(serde_json::from_value(value)?);
    }
    if !(FIRST_VERSION..crate::SAVE_VERSION).contains(&from) {
        return Err(PersistenceError::UnsupportedVersion(from));
    }
    let steps = &STEPS[(from - FIRST_VERSION) as usize..];
    let mut carried = Carried::default();
    for step in steps {
        (step.raw)(&mut value, &mut carried);
        value["save_version"] = Value::from(step.from + 1);
    }
    let mut save: SaveFile = serde_json::from_value(value)?;
    for step in steps {
        (step.settle)(&mut save, &carried);
    }
    Ok(save)
}

/// Every creature in a raw file, as JSON objects.
fn creatures_mut(value: &mut Value) -> impl Iterator<Item = &mut Map<String, Value>> {
    value
        .get_mut("creatures")
        .and_then(Value::as_array_mut)
        .into_iter()
        .flatten()
        .filter_map(Value::as_object_mut)
}

/// The decorations an older colony had earned, and which of them it had hidden, taken out of the
/// file. `None` when it had never earned any.
fn take_legacy_decorations(value: &mut Value) -> Option<(Vec<crate::ShelterDecorationKind>, u8)> {
    let home = value.get_mut("home")?.as_object_mut()?;
    let hidden = home
        .remove("hidden_decorations")
        .and_then(|hidden| hidden.as_u64())
        .and_then(|hidden| u8::try_from(hidden).ok())
        .unwrap_or(0);
    let earned = home
        .remove("decorations")
        .and_then(|state| state.get("decorations").cloned())
        .and_then(|list| serde_json::from_value::<Vec<crate::ShelterDecorationKind>>(list).ok())
        .unwrap_or_default();
    Some((earned, hidden))
}

/// What a colony from before 0.60.0 has to choose from. Everything it earned stays earned, the
/// three hangout spots and three gardens every village already had stay, and every category is
/// topped up to its first three, so an older colony gains three ornaments. The decorations that
/// were showing on the colony house go on hanging there; the hidden ones are simply not hung.
fn village_unlocks(
    save: &mut SaveFile,
    (earned, hidden): &(Vec<crate::ShelterDecorationKind>, u8),
) {
    let home = &mut save.home;
    home.unlocks = crate::VillageUnlocks {
        decorations: earned.clone(),
        hangouts: crate::HangoutKind::STARTING.to_vec(),
        gardens: crate::GardenKind::STARTING.to_vec(),
        ornaments: crate::OrnamentKind::STARTING.to_vec(),
        next_at_utc: crate::world::scheduled_village_unlock_at(
            save.colony_seed,
            0,
            save.maximum_seen_utc,
        ),
        ordinal: 0,
    };
    home.unlocks.normalize();
    let showing: Vec<crate::ShelterDecorationKind> = earned
        .iter()
        .copied()
        .filter(|kind| kind.index() >= 8 || hidden & (1 << kind.index()) == 0)
        .collect();
    let owners = crate::house_owners(&save.creatures, &home.cottage_order);
    if let Some(keeper) = owners.as_slice().first().copied() {
        for kind in showing {
            home.set_decoration(keeper, kind.slot(), Some(kind));
        }
    }
    home.normalize_village();
}

fn colony_roles(value: &mut Value) {
    let Some(creatures) = value.get_mut("creatures").and_then(Value::as_array_mut) else {
        return;
    };
    let parent_id = creatures
        .first()
        .and_then(|creature| creature.get("id"))
        .and_then(Value::as_u64)
        .unwrap_or_default();
    for (index, creature) in creatures.iter_mut().enumerate() {
        let Some(creature) = creature.as_object_mut() else {
            continue;
        };
        let role = if index == 0 {
            json!({ "kind": "adult" })
        } else {
            json!({ "kind": "mini", "parent_id": parent_id })
        };
        creature.insert("role".into(), role);
        creature.insert("kept".into(), Value::Bool(true));
        creature.insert(
            "mini_arrivals".into(),
            json!({ "enabled": false, "arrived": [false, false] }),
        );
    }
}

fn shared_relationships(value: &mut Value) {
    let Some(creatures) = value.get_mut("creatures").and_then(Value::as_array_mut) else {
        value["relationships"] = Value::Array(Vec::new());
        return;
    };
    let creature_ids: Vec<_> = creatures
        .iter()
        .filter_map(|creature| creature.get("id").and_then(Value::as_u64))
        .collect();
    let valid_ids: std::collections::BTreeSet<_> = creature_ids.iter().copied().collect();
    let mut legacy_scores: std::collections::BTreeMap<(u64, u64), Vec<f64>> =
        std::collections::BTreeMap::new();
    for creature in creatures.iter_mut() {
        let Some(source) = creature.get("id").and_then(Value::as_u64) else {
            continue;
        };
        let relationships = creature
            .pointer_mut("/state/relationships")
            .map(Value::take)
            .and_then(|value| value.as_object().cloned())
            .unwrap_or_else(Map::new);
        if let Some(state) = creature.get_mut("state").and_then(Value::as_object_mut) {
            state.remove("relationships");
        }
        for (target, score) in relationships {
            let Ok(target) = target.parse::<u64>() else {
                continue;
            };
            let Some(pair) = crate::canonical_creature_pair(source, target) else {
                continue;
            };
            if valid_ids.contains(&target)
                && let Some(score) = score.as_f64()
            {
                legacy_scores
                    .entry(pair)
                    .or_default()
                    .push(score.clamp(0.0, 1.0));
            }
        }
    }

    let mut relationships = Vec::new();
    for (index, a) in creature_ids.iter().copied().enumerate() {
        for b in creature_ids.iter().copied().skip(index + 1) {
            let Some(pair) = crate::canonical_creature_pair(a, b) else {
                continue;
            };
            let scores = legacy_scores.get(&pair).map(Vec::as_slice).unwrap_or(&[]);
            let legacy_affinity = if scores.is_empty() {
                0.0
            } else {
                scores.iter().sum::<f64>() / scores.len() as f64
            };
            relationships.push(json!({
                "a": pair.0,
                "b": pair.1,
                "affinity": (legacy_affinity * 255.0).round() as u8,
                "familiarity": (legacy_affinity * 64.0).round() as u8,
                "playfulness": 0,
                "avoidance": 0,
            }));
        }
    }
    relationships.truncate(crate::MAX_RELATIONSHIPS);
    value["relationships"] = Value::Array(relationships);
}

fn lived_experience(value: &mut Value) {
    let colony_seed: [u8; 32] = value
        .get("colony_seed")
        .and_then(Value::as_array)
        .and_then(|bytes| {
            let bytes: Vec<u8> = bytes
                .iter()
                .map(Value::as_u64)
                .collect::<Option<Vec<_>>>()?
                .into_iter()
                .map(|byte| u8::try_from(byte).ok())
                .collect::<Option<Vec<_>>>()?;
            bytes.try_into().ok()
        })
        .unwrap_or_default();
    let Some(creatures) = value.get_mut("creatures").and_then(Value::as_array_mut) else {
        return;
    };
    let mut names = Vec::with_capacity(creatures.len());
    for (colony_order, creature) in creatures.iter_mut().enumerate() {
        let generation = creature
            .get("generation")
            .and_then(Value::as_u64)
            .and_then(|value| u8::try_from(value).ok())
            .unwrap_or(colony_order as u8);
        let name = crate::default_creature_name(colony_seed, generation, &names);
        names.push(name.clone());
        let routines = creature
            .pointer_mut("/state/habits")
            .map(Value::take)
            .and_then(|habits| routines_from_habits(&habits))
            .unwrap_or_default();
        if let Some(state) = creature.get_mut("state").and_then(Value::as_object_mut) {
            state.remove("habits");
        }
        let Some(creature) = creature.as_object_mut() else {
            continue;
        };
        creature.insert(
            "origin".into(),
            json!({
                "source_colony_seed": colony_seed,
                "source_generation": generation,
            }),
        );
        creature.insert("colony_order".into(), Value::from(colony_order as u64));
        creature.insert("name".into(), Value::String(name));
        creature.insert("memory".into(), json!(crate::CreatureMemory::default()));
        creature.insert(
            "tendencies".into(),
            json!(crate::LearnedTendencies::default()),
        );
        creature.insert("routines".into(), json!(routines));
    }
}

fn routines_from_habits(value: &Value) -> Option<crate::RoutineTable> {
    let habits = value.as_object()?;
    let entries = habits
        .iter()
        .filter_map(|(legacy_key, strength)| {
            let mut parts = legacy_key.split(':');
            let time_bucket = parts.next()?.parse::<u8>().ok()?.min(3);
            let region = parts.next()?.parse::<u8>().ok()?.min(2);
            let surface = match parts.next()? {
                "ScreenFloor" => crate::SurfaceKind::ScreenFloor,
                "WindowLedge" => crate::SurfaceKind::WindowLedge,
                _ => return None,
            };
            let action = crate::ActionKind::from_legacy_name(parts.next()?)?;
            let relative_x = (f32::from(region) + 0.5) / 3.0;
            let key = crate::routine_key(surface, relative_x, action, time_bucket * 6);
            Some((key, strength.as_f64()? as f32))
        })
        .collect();
    Some(crate::RoutineTable::from_ranked(entries))
}

fn reshape_appearance(appearance: &mut Map<String, Value>) {
    let appendage_style = appearance
        .remove("appendage_style")
        .unwrap_or_else(|| Value::String("None".into()));
    let appendage_size = appearance
        .remove("appendage_size")
        .unwrap_or_else(|| Value::from(3));
    appearance.insert(
        "head_appendages".into(),
        json!({
            "style": appendage_style,
            "size": appendage_size,
        }),
    );

    let eye_size = appearance
        .remove("eye_size")
        .and_then(|value| value.as_u64())
        .unwrap_or(1)
        .clamp(1, 2);
    let eye_spacing = appearance
        .remove("eye_spacing")
        .and_then(|value| value.as_u64())
        .unwrap_or(5)
        .clamp(3, 7);
    let vertical_offset = appearance
        .remove("eye_height")
        .and_then(|value| value.as_i64())
        .unwrap_or(0)
        .clamp(-2, 2);
    let signature = appearance
        .get("face_signature")
        .and_then(Value::as_u64)
        .unwrap_or_default();
    let family = appearance
        .get("family")
        .and_then(Value::as_str)
        .unwrap_or("Blob")
        .to_owned();

    appearance.insert(
        "face".into(),
        json!({
            "eye_shape": select_gene(signature, 0, &["Round", "Tall", "SoftSquare"]),
            "eye_size": eye_size,
            "eye_spacing": eye_spacing,
            "vertical_offset": vertical_offset,
            "pupil_style": select_gene(signature, 2, &["Dot", "Wide", "Spark"]),
            "highlight_style": select_gene(signature, 4, &["Single", "Double", "Diagonal"]),
            "brow_style": select_gene(signature, 6, &["None", "Soft", "Bold"]),
            "mouth_style": select_gene(signature, 8, &["Tiny", "Smile", "Cat", "Beak"]),
            "cheek_style": select_gene(signature, 10, &["None", "Dots", "Blush"]),
        }),
    );
    let (style, tip_style) = match family.as_str() {
        "Hopper" => ("MittenArm", "Mitten"),
        "SoftQuadruped" => ("FrontPaw", "Paw"),
        _ if signature & 1 == 0 => ("SoftNub", "Round"),
        _ => ("Pseudopod", "Round"),
    };
    appearance.insert(
        "forelimbs".into(),
        json!({
            "style": style,
            "length": 3 + (signature >> 12) % 5,
            "thickness": 1 + (signature >> 15) % 2,
            "tip_style": tip_style,
            "rest_pose": select_gene(signature, 16, &["AtSides", "Folded", "Together"]),
        }),
    );
    appearance.insert(
        "effect_motif".into(),
        Value::String(
            select_gene(
                signature,
                18,
                &["None", "Dot", "Star", "Heart", "Leaf", "Spark"],
            )
            .into(),
        ),
    );
}

fn select_gene(signature: u64, shift: u32, values: &'static [&'static str]) -> &'static str {
    values[((signature >> shift) as usize) % values.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The table is the whole history: one step for every version before the current one, in
    /// order, the last of them leaving a file at the current version. Moving `SAVE_VERSION`
    /// without adding a step fails here.
    #[test]
    fn every_version_has_exactly_one_step_and_the_last_lands_on_the_current_version() {
        assert_eq!(
            STEPS.len() as u32,
            crate::SAVE_VERSION - FIRST_VERSION,
            "add a step for the new version"
        );
        for (index, step) in STEPS.iter().enumerate() {
            assert_eq!(step.from, FIRST_VERSION + index as u32);
        }
        assert_eq!(STEPS.last().unwrap().from + 1, crate::SAVE_VERSION);
    }

    /// A file with no version, a version 0, or one from a newer build is refused before any step
    /// touches it.
    #[test]
    fn a_version_outside_the_table_is_refused_untouched() {
        for version in [
            Value::Null,
            Value::from(0),
            Value::from(crate::SAVE_VERSION + 1),
        ] {
            let value = json!({ "save_version": version });
            match upgrade(value) {
                Err(PersistenceError::UnsupportedVersion(found)) => {
                    assert_ne!(found, crate::SAVE_VERSION)
                }
                other => panic!("expected a refusal, got {other:?}"),
            }
        }
        assert!(matches!(
            upgrade(json!([1, 2, 3])),
            Err(PersistenceError::UnsupportedVersion(0))
        ));
    }

    /// Each raw step leaves the version it promises, so a file part-way along is always a valid
    /// file of some version.
    #[test]
    fn each_step_moves_the_file_exactly_one_version() {
        let mut value = json!({ "save_version": FIRST_VERSION, "creatures": [] });
        let mut carried = Carried::default();
        for step in STEPS {
            assert_eq!(version_of(&value), step.from);
            (step.raw)(&mut value, &mut carried);
            value["save_version"] = Value::from(step.from + 1);
        }
        assert_eq!(version_of(&value), crate::SAVE_VERSION);
    }
}
