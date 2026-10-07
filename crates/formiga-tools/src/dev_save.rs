//! `formiga-tools dev save FILE`: a colony file, read the way Desktop reads one.
//!
//!   cargo run -p formiga-tools -- dev save path/to/colony.json
//!
//! One JSON document saying what Desktop would make of the file: the version it names and the
//! upgrades that bring it forward, every rule the file breaks before it is put right, each change
//! the repair makes, whether it would be accepted as a snapshot to restore from, what Desktop
//! would open if this were its colony file (it falls back to the backup beside it), and a summary
//! of the colony itself. Everything is done by `formiga-core`'s own reading, upgrading, checking
//! and repairing; nothing here decides anything about a colony.

use anyhow::Result;
use formiga_core::*;
use serde_json::{Value, json};
use std::path::Path;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

/// How many differences between the file and its repaired colony are listed; the count is
/// always given.
const REPAIRS_SHOWN: usize = 200;

pub fn inspect(path: &Path) -> Result<Value> {
    let bytes = std::fs::read(path)?;
    let named = named_version(&bytes);
    let mut report = json!({
        "file": path.display().to_string(),
        "bytes": bytes.len(),
        "version": {
            "named": named,
            "current": SAVE_VERSION,
            "upgrades": (named.max(1)..SAVE_VERSION).collect::<Vec<_>>(),
        },
        "snapshot": snapshot(path),
        "opens": opens(path),
    });
    let unrepaired = match decode_unvalidated(&bytes) {
        Ok(save) => save,
        Err(error) => {
            report["success"] = json!(false);
            report["error"] = json!(error.to_string());
            return Ok(report);
        }
    };
    let broken = violations(&unrepaired);
    let before = serde_json::to_value(&unrepaired)?;
    let repaired = ValidatedSave::from(unrepaired).into_inner();
    let after = serde_json::to_value(&repaired)?;
    let mut repairs = Vec::new();
    differences("", &before, &after, &mut repairs);
    // Validation promises a colony with none of these; one left is Formiga's mistake, not the
    // file's.
    let left = violations(&repaired);
    report["success"] = json!(true);
    report["broken"] = json!(broken);
    report["repairs"] = json!({
        "count": repairs.len(),
        "shown": repairs.iter().take(REPAIRS_SHOWN).collect::<Vec<_>>(),
    });
    report["after_repair"] = json!(left);
    report["colony"] = summary(&repaired);
    Ok(report)
}

/// Whether the file would be accepted as a snapshot to restore from, which is never repaired.
fn snapshot(path: &Path) -> Value {
    match SaveStore::read_snapshot(path) {
        Ok(_) => json!({ "accepted": true }),
        Err(error) => json!({ "accepted": false, "why": error.to_string() }),
    }
}

/// What Desktop would open if this were its colony file: the file, the backup kept beside it, or
/// nothing (a new colony).
fn opens(path: &Path) -> Value {
    let backup = path.with_extension("json.bak");
    let store = SaveStore::new(path);
    let read = |path: &Path| std::fs::read(path).ok().and_then(|b| decode(&b).ok());
    match store.load() {
        Ok(None) => json!({ "opens": "new colony" }),
        Ok(Some(save)) => {
            let which = if read(path).as_ref() == Some(&save) {
                "file"
            } else {
                "backup"
            };
            json!({ "opens": which, "backup": backup.display().to_string() })
        }
        Err(error) => json!({ "opens": "nothing", "why": error.to_string() }),
    }
}

/// Every place `after` differs from `before`, by JSON path.
fn differences(at: &str, before: &Value, after: &Value, found: &mut Vec<Value>) {
    match (before, after) {
        (Value::Object(a), Value::Object(b)) => {
            for (key, value) in a {
                let path = format!("{at}.{key}");
                match b.get(key) {
                    Some(other) => differences(&path, value, other, found),
                    None => found.push(json!({ "path": path, "was": value, "now": null })),
                }
            }
            for (key, value) in b {
                if !a.contains_key(key) {
                    found.push(json!({ "path": format!("{at}.{key}"), "was": null, "now": value }));
                }
            }
        }
        // Companions, objects and the like are matched by their id, so one dropped from the
        // middle is reported as itself and not as a change to everything after it.
        (Value::Array(a), Value::Array(b)) if identified(a) && identified(b) => {
            let mut matched = Vec::new();
            for (index, item) in a.iter().enumerate() {
                let id = &item["id"];
                let path = format!("{at}[id={id}]");
                let first = a.iter().position(|other| other["id"] == *id) == Some(index);
                match b.iter().find(|other| other["id"] == *id) {
                    Some(other) if first => {
                        matched.push(id);
                        differences(&path, item, other, found);
                    }
                    Some(_) => {
                        found.push(json!({ "path": path, "was": "a second copy", "now": null }))
                    }
                    None => found.push(json!({ "path": path, "was": "present", "now": null })),
                }
            }
            for item in b {
                if !matched.contains(&&item["id"]) {
                    let path = format!("{at}[id={}]", item["id"]);
                    found.push(json!({ "path": path, "was": null, "now": item }));
                }
            }
        }
        (Value::Array(a), Value::Array(b)) => {
            for (index, (x, y)) in a.iter().zip(b).enumerate() {
                differences(&format!("{at}[{index}]"), x, y, found);
            }
            if a.len() != b.len() {
                found.push(json!({
                    "path": at,
                    "was": format!("{} items", a.len()),
                    "now": format!("{} items", b.len()),
                }));
            }
        }
        _ if before != after => found.push(json!({ "path": at, "was": before, "now": after })),
        _ => {}
    }
}

/// Whether every item of a list is a record with an id.
fn identified(items: &[Value]) -> bool {
    !items.is_empty() && items.iter().all(|item| item.get("id").is_some())
}

fn when(at: OffsetDateTime) -> String {
    at.format(&Rfc3339).unwrap_or_default()
}

/// The JSON name a value is saved under: the variant of an enum, as the file spells it.
fn saved_name(value: impl serde::Serialize) -> String {
    match serde_json::to_value(value) {
        Ok(Value::String(name)) => name,
        Ok(Value::Object(map)) => map.keys().next().cloned().unwrap_or_default(),
        Ok(other) => other.to_string(),
        Err(_) => String::new(),
    }
}

fn summary(save: &SaveFile) -> Value {
    let name_of = |id: CreatureId| {
        save.creatures
            .iter()
            .find(|creature| creature.id == id)
            .map(|creature| creature.name.clone())
    };
    let creatures: Vec<Value> = save
        .creatures
        .iter()
        .map(|creature| {
            let role = match creature.role {
                CreatureRole::Adult => "grown-up".to_owned(),
                CreatureRole::Mini { parent_id } => format!(
                    "mini of {}",
                    name_of(parent_id).unwrap_or_else(|| parent_id.to_string())
                ),
            };
            json!({
                "name": creature.name,
                "id": creature.id,
                "role": role,
                "generation": creature.generation,
                "born": when(creature.born_at_utc),
                "body": saved_name(creature.appearance.family),
                "wears": creature.accessory.map(saved_name),
            })
        })
        .collect();
    let unlocks = &save.home.unlocks;
    let companion = &save.companion;
    let mut moments: Vec<(String, usize)> = Vec::new();
    for entry in &companion.journal {
        let moment = saved_name(&entry.moment);
        match moments.iter_mut().find(|(name, _)| *name == moment) {
            Some((_, count)) => *count += 1,
            None => moments.push((moment, 1)),
        }
    }
    moments.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    json!({
        "seed": save.colony_seed.iter().map(|b| format!("{b:02x}")).collect::<String>(),
        "created": when(save.created_at_utc),
        "last_seen": when(save.maximum_seen_utc),
        "days": (save.maximum_seen_utc - save.created_at_utc).whole_days(),
        "creatures": creatures,
        "relationships": save.relationships.len(),
        "journal": {
            "entries": companion.journal.len(),
            "newest": companion.journal.iter().map(|e| e.at).max().map(when),
            "moments": moments.iter().map(|(name, count)| json!({"moment": name, "count": count}))
                .collect::<Vec<_>>(),
            "pinned": companion.pins.len(),
        },
        "unlocked": {
            "decorations": unlocks.decorations.iter().map(saved_name).collect::<Vec<_>>(),
            "hangouts": unlocks.hangouts.iter().map(saved_name).collect::<Vec<_>>(),
            "gardens": unlocks.gardens.iter().map(saved_name).collect::<Vec<_>>(),
            "ornaments": unlocks.ornaments.iter().map(saved_name).collect::<Vec<_>>(),
            "next": when(unlocks.next_at_utc),
        },
        "placed": {
            "hangouts": save.home.hangouts.len(),
            "gardens": save.home.gardens.len(),
            "ornaments": save.home.ornaments.len(),
            "dressing": save.home.dressing.len(),
        },
        "objects": save.objects.objects.iter().map(|o| saved_name(o.kind)).collect::<Vec<_>>(),
        "finds": companion.scrapbook.iter().map(|r| r.variant).collect::<Vec<_>>(),
        "wonders": companion.wonders.iter().map(|w| saved_name(w.kind)).collect::<Vec<_>>(),
        "trips": save.trips.count,
        "souvenirs": save.trips.souvenirs.iter().map(|r| r.souvenir.id()).collect::<Vec<_>>(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn written(save: &SaveFile, name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("formiga-dev-save-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        std::fs::write(&path, serde_json::to_vec_pretty(save).unwrap()).unwrap();
        path
    }

    fn colony() -> SaveFile {
        let now = time::macros::datetime!(2026-03-02 10:00 UTC);
        World::new([7; 32], now, &crate::fixture_desktop()).save
    }

    #[test]
    fn a_sound_colony_needs_nothing_put_right() {
        let path = written(&colony(), "sound.json");
        let report = inspect(&path).unwrap();
        assert_eq!(report["success"], json!(true));
        assert_eq!(report["broken"], json!([]));
        assert_eq!(report["repairs"]["count"], json!(0), "{report:#}");
        assert_eq!(report["snapshot"]["accepted"], json!(true));
        assert_eq!(report["opens"]["opens"], json!("file"));
    }

    #[test]
    fn a_broken_rule_and_its_repair_are_both_reported() {
        let mut save = colony();
        let twin = save.creatures[0].clone();
        save.creatures.push(twin);
        let path = written(&save, "twins.json");
        let report = inspect(&path).unwrap();
        assert_eq!(report["success"], json!(true));
        let broken = report["broken"].as_array().unwrap();
        assert!(
            broken.contains(&json!("two companions share an id")),
            "{broken:?}"
        );
        let repairs = report["repairs"]["shown"].as_array().unwrap();
        let id = save.creatures[0].id;
        assert_eq!(
            repairs,
            &vec![
                json!({ "path": format!(".creatures[id={id}]"), "was": "a second copy", "now": null })
            ]
        );
        assert_eq!(report["after_repair"], json!([]));
    }

    #[test]
    fn a_file_that_cannot_be_read_says_why() {
        let path = written(&colony(), "cut.json");
        let bytes = std::fs::read(&path).unwrap();
        std::fs::write(&path, &bytes[..bytes.len() / 2]).unwrap();
        let report = inspect(&path).unwrap();
        assert_eq!(report["success"], json!(false));
        assert!(
            report["error"]
                .as_str()
                .unwrap()
                .contains("invalid save file")
        );
        assert_eq!(report["opens"]["opens"], json!("nothing"));
    }
}
