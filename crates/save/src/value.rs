//! The decoded save, read with the upstream `readRawKey` semantics.
//!
//! The game writes some fields as JSON strings and others as one character per
//! key (`{"0":"{","1":"["…}`). Upstream's `js/stats/systems/common/combat-sim-setup.js`
//! decodes both on every read, and the Killroy planner depends on that. This
//! wrapper keeps the parsed object and offers the same lookups without the
//! section modules re-implementing the codec.

use serde_json::{Map, Value};

/// A save's own field map and the wrapper object beside it.
pub type Parts<'a> = (&'a Map<String, Value>, &'a Map<String, Value>);

/// Top-level keys that live beside `data` in an `it.json` wrapper.
const WRAPPER_KEYS: [&str; 3] = ["companion", "serverVars", "charNames"];

/// A parsed save object, wrapped or flat.
#[derive(Clone, Debug)]
pub struct Save {
    root: Value,
}

impl std::ops::Deref for Save {
    type Target = Value;

    /// The save's own object, without the `it.json` wrapper, for the readers
    /// that expect the decoded field map.
    fn deref(&self) -> &Value {
        match self.root.as_object().and_then(|root| root.get("data")) {
            Some(fields @ Value::Object(_)) => fields,
            _ => &self.root,
        }
    }
}

impl Save {
    /// Wrap the parsed text's top-level value.
    pub fn new(root: Value) -> Self {
        Self { root }
    }

    /// The top-level value: the `it.json` wrapper, or the flat save.
    pub fn root(&self) -> &Value {
        &self.root
    }

    /// Read one key, decoding it the way upstream does.
    pub fn get(&self, key: &str) -> Option<Value> {
        let root = self.root.as_object()?;
        let source = if WRAPPER_KEYS.contains(&key) {
            root
        } else {
            self.fields()?
        };
        source.get(key).map(decode)
    }

    /// The object holding the save's own keys: `data` in an `it.json`, else the root.
    fn fields(&self) -> Option<&Map<String, Value>> {
        let root = self.root.as_object()?;
        match root.get("data") {
            Some(Value::Object(fields)) => Some(fields),
            _ => Some(root),
        }
    }

    /// The `charNames` roster beside the save, when the wrapper carries one.
    fn char_names(&self) -> Option<&Vec<Value>> {
        self.root.get("charNames").and_then(Value::as_array)
    }

    /// Every character index the save carries.
    pub fn character_count(&self) -> usize {
        let mut count = self.char_names().map_or(0, Vec::len);
        let Some(fields) = self.fields() else {
            return count;
        };
        for key in fields.keys() {
            for prefix in ["Lv0_", "CharacterClass_", "EquipOrder_", "SL_"] {
                let Some(index) = key
                    .strip_prefix(prefix)
                    .filter(|rest| !rest.is_empty() && rest.chars().all(|ch| ch.is_ascii_digit()))
                    .and_then(|rest| rest.parse::<usize>().ok())
                else {
                    continue;
                };
                count = count.max(index + 1);
            }
        }
        count
    }

    /// The save's own field map and the wrapper object beside it.
    ///
    /// The account decoder reads some fields from the wrapper (`companion`)
    /// and the rest from the save; this hands both over without cloning.
    pub fn parts(&self) -> Option<Parts<'_>> {
        let root = self.root.as_object()?;
        Some((self.fields().unwrap_or(root), root))
    }

    /// A character's name beside its index, when the save carries one.
    pub fn character_name(&self, ci: usize) -> String {
        let name = self
            .char_names()
            .and_then(|names| names.get(ci))
            .and_then(Value::as_str)
            .filter(|name| !name.is_empty());
        name.map(str::to_string)
            .unwrap_or_else(|| format!("Character {}", ci + 1))
    }
}

/// One stored value decoded to its game value.
fn decode(stored: &Value) -> Value {
    match stored {
        Value::String(text) => serde_json::from_str::<Value>(text)
            .ok()
            .filter(|parsed| parsed.is_object() || parsed.is_array())
            .unwrap_or_else(|| stored.clone()),
        Value::Object(map) if is_char_object(map) => {
            let mut chars: Vec<(usize, &str)> = map
                .iter()
                .filter_map(|(key, value)| Some((numeric_key(key)?, value.as_str()?)))
                .collect();
            chars.sort_unstable_by_key(|(index, _)| *index);
            let text: String = chars.into_iter().map(|(_, ch)| ch).collect();
            serde_json::from_str(&text).unwrap_or_else(|_| stored.clone())
        }
        _ => stored.clone(),
    }
}

/// Whether an object holds a JSON text one character per numeric key.
fn is_char_object(map: &Map<String, Value>) -> bool {
    if map.is_empty() || map.len() != map.keys().filter_map(|key| numeric_key(key)).count() {
        return false;
    }
    let mut keys: Vec<usize> = map.keys().filter_map(|key| numeric_key(key)).collect();
    keys.sort_unstable();
    keys.iter().copied().eq(0..map.len())
        && map
            .values()
            .all(|value| value.as_str().is_some_and(|text| text.chars().count() == 1))
        && matches!(map.get("0").and_then(Value::as_str), Some("{") | Some("["))
}

fn numeric_key(key: &str) -> Option<usize> {
    key.parse::<usize>().ok()
}

/// The upstream `_number`: a missing, blank or non-finite value is `None`.
pub fn number(value: &Value) -> Option<f64> {
    match value {
        Value::Null => None,
        Value::String(text) if text.is_empty() => None,
        Value::String(text) => text.parse::<f64>().ok().filter(|n| n.is_finite()),
        Value::Number(_) => value.as_f64().filter(|n| n.is_finite()),
        _ => None,
    }
}

/// A row cell, by index, whichever way the row is stored.
pub fn at(value: &Value, index: usize) -> Option<&Value> {
    match value {
        Value::Array(cells) => cells.get(index),
        Value::Object(cells) => cells.get(&index.to_string()),
        _ => None,
    }
}

/// A row as upstream `rowValues` reads it: arrays as-is, objects by their
/// numeric keys, and holes as `Null`.
pub fn row_values(value: &Value) -> Vec<Value> {
    match value {
        Value::Array(cells) => cells.clone(),
        Value::Object(cells) => {
            let length = cells.get("length").and_then(number).map_or_else(
                || {
                    cells
                        .keys()
                        .filter_map(|key| key.parse::<usize>().ok())
                        .max()
                        .map_or(0, |max| max + 1)
                },
                |length| length.max(0.0) as usize,
            );
            (0..length)
                .map(|index| {
                    cells
                        .get(&index.to_string())
                        .cloned()
                        .unwrap_or(Value::Null)
                })
                .collect()
        }
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_json_string_decodes_to_its_object() {
        let save = Save::new(json!({ "TimeAway": "{\"GlobalTime\":42}" }));
        assert_eq!(save.get("TimeAway"), Some(json!({ "GlobalTime": 42 })));
    }

    #[test]
    fn a_character_object_decodes_to_its_json_text() {
        let chars: Map<String, Value> = "{\"a\":1}"
            .chars()
            .enumerate()
            .map(|(index, ch)| (index.to_string(), json!(ch.to_string())))
            .collect();
        let save = Save::new(json!({ "KRbest": chars }));
        assert_eq!(save.get("KRbest"), Some(json!({ "a": 1 })));
    }

    #[test]
    fn wrapper_keys_read_beside_data() {
        let save = Save::new(json!({
            "data": { "Lv0_0": "[]" },
            "serverVars": { "KillroySwap": 7 },
            "charNames": ["Ana"],
        }));
        assert_eq!(save.get("KillroySwap"), None);
        assert_eq!(
            save.get("serverVars")
                .and_then(|vars| vars.get("KillroySwap").cloned()),
            Some(json!(7))
        );
        assert_eq!(save.character_count(), 1);
        assert_eq!(save.character_name(0), "Ana");
        assert_eq!(save.character_name(1), "Character 2");
    }

    #[test]
    fn rows_read_arrays_and_numeric_objects() {
        assert_eq!(row_values(&json!([1, 2])), vec![json!(1), json!(2)]);
        assert_eq!(
            row_values(&json!({ "0": 1, "2": 3, "length": 3 })),
            vec![json!(1), Value::Null, json!(3)]
        );
        assert_eq!(at(&json!({ "3": "x" }), 3), Some(&json!("x")));
    }
}
