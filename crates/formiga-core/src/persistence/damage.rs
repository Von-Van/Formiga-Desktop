//! Damage to a colony file, for the tests and the long simulated runs that check every damaged
//! file is either refused or read back whole. Nothing in the app damages anything.

/// Every place in a JSON document, as pointers.
fn json_pointers(value: &serde_json::Value, at: String, pointers: &mut Vec<String>) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, child) in map {
                json_pointers(child, format!("{at}/{key}"), pointers);
            }
        }
        serde_json::Value::Array(items) => {
            for (index, child) in items.iter().enumerate() {
                json_pointers(child, format!("{at}/{index}"), pointers);
            }
        }
        _ => {}
    }
    pointers.push(at);
}

/// Damage one place in a colony file the way a bad copy, a hand edit or a stray tool might:
/// mostly keeping each value's kind, so the damage reaches what validation repairs, and now and
/// then not, so it reaches what parsing refuses.
pub fn damage(value: &mut serde_json::Value, rng: &mut impl rand::Rng) {
    use serde_json::{Value, json};
    let mut pointers = Vec::new();
    json_pointers(value, String::new(), &mut pointers);
    let at = &pointers[rng.random_range(0..pointers.len())];
    let Some(place) = value.pointer_mut(at) else {
        return;
    };
    let choice = rng.random_range(0..8);
    *place = match place.take() {
        Value::Array(mut items) => {
            match choice {
                0..=2 if !items.is_empty() => {
                    let item = items[rng.random_range(0..items.len())].clone();
                    items.extend(std::iter::repeat_n(item, rng.random_range(1..40)));
                }
                3 => items.clear(),
                4 => items.reverse(),
                _ => items.truncate(items.len() / 2),
            }
            Value::Array(items)
        }
        Value::Object(mut map) if choice < 6 && !map.is_empty() => {
            let key = map.keys().nth(rng.random_range(0..map.len())).cloned();
            if let Some(key) = key {
                map.remove(&key);
            }
            Value::Object(map)
        }
        Value::Number(number) => match choice {
            0 => json!(0),
            1 => json!(255),
            2 if number.is_f64() => json!(f64::MAX),
            3 if number.is_f64() => json!(-1.5),
            4 => json!(u32::MAX),
            5 => json!(-1),
            _ => json!(number.as_u64().map_or(0, |n| n.wrapping_mul(7) % 256)),
        },
        Value::String(text) => match choice {
            0..=2 => json!(""),
            3 => json!(text.repeat(20)),
            4 => json!("\u{7}\u{0}"),
            _ => json!(format!("  {text}  ")),
        },
        Value::Bool(flag) => json!(!flag),
        _ => json!(null),
    };
}
