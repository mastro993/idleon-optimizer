//! The decoded save.
//!
//! The game writes some fields as JSON strings and others as one character per
//! key (`{"0":"{","1":"["…}`). Both are decoded once per save, the first
//! time a reader asks for the field map: the readers then look a field up and
//! never re-parse it. See [`decoded`] for why that matters.

use serde_json::{Map, Value};
use std::sync::{Arc, OnceLock};

/// A save's own field map and the wrapper object beside it.
pub type Parts<'a> = (&'a Map<String, Value>, &'a Map<String, Value>);

/// Top-level keys that live beside `data` in an `it.json` wrapper.
const WRAPPER_KEYS: [&str; 3] = ["companion", "serverVars", "charNames"];

/// A parsed save object, wrapped or flat.
///
/// The tree is shared: a clone is a reference-count bump, so a section, a
/// sandbox and a background run can each hold the save without copying it.
#[derive(Clone, Debug)]
pub struct Save {
    /// The top-level value as the import read it, `it.json` wrapper included:
    /// an export writes it back unchanged.
    root: Arc<Value>,
    /// The save's own fields with every encoded field already decoded, so a
    /// reader pays for the parse once per save instead of once per cell. Shared
    /// by every clone, and built on the first read: most saves in a run are
    /// built, handed on and never read.
    fields: Arc<OnceLock<Value>>,
}

impl std::ops::Deref for Save {
    type Target = Value;

    /// The save's own object, without the `it.json` wrapper, for the readers
    /// that expect the decoded field map.
    fn deref(&self) -> &Value {
        self.fields.get_or_init(|| decoded(&self.root))
    }
}

impl Save {
    /// Wrap the parsed text's top-level value.
    pub fn new(root: Value) -> Self {
        Self {
            fields: Arc::new(OnceLock::new()),
            root: Arc::new(root),
        }
    }

    /// The top-level value: the `it.json` wrapper, or the flat save.
    pub fn root(&self) -> &Value {
        &self.root
    }

    /// Read one key, decoding either encoding.
    pub fn get(&self, key: &str) -> Option<Value> {
        let root = self.root.as_object()?;
        if WRAPPER_KEYS.contains(&key) {
            return root.get(key).map(decode);
        }
        self.fields().and_then(|fields| fields.get(key)).cloned()
    }

    /// The object holding the save's own keys: `data` in an `it.json`, else the root.
    fn fields(&self) -> Option<&Map<String, Value>> {
        match &**self {
            Value::Object(fields) => Some(fields),
            _ => None,
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
                    .filter(|index| *index < crate::MAX_CHARACTERS)
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
        Some((self.fields()?, root))
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
///
/// A save field costs a JSON parse of its whole text, and the decoders read the
/// same field once per cell (thousands of times for `Breeding` or `Lab`), so
/// leaving the encoding in place made an import re-parse hundreds of megabytes.
/// [`decoded`] runs this over every field once per save, and hands the readers
/// that map.
pub(crate) fn decode(stored: &Value) -> Value {
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

/// A save's own field map with every encoded field decoded once.
///
/// The map `Deref` and [`Save::parts`] hand out: the readers below it call
/// [`crate::fields`] per cell, and each of those decodes the field it reads.
fn decoded(root: &Value) -> Value {
    let fields = match root.as_object().and_then(|root| root.get("data")) {
        Some(Value::Object(fields)) => fields,
        _ => match root.as_object() {
            Some(fields) => fields,
            None => return root.clone(),
        },
    };
    Value::Object(
        fields
            .iter()
            .map(|(key, stored)| (key.clone(), decode(stored)))
            .collect(),
    )
}

/// Whether a value is an object holding a JSON text one character per numeric key.
pub(crate) fn is_char_object_value(value: &Value) -> bool {
    value.as_object().is_some_and(is_char_object)
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

/// A number: a missing, blank or non-finite value is `None`.
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

/// A row's cells: arrays as-is, objects by their
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
                        .filter(|index| *index < crate::MAX_ROW_CELLS)
                        .max()
                        .map_or(0, |max| max + 1)
                },
                |length| (length.max(0.0) as usize).min(crate::MAX_ROW_CELLS),
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
    fn readers_get_the_decoded_fields_while_an_export_keeps_the_text() {
        let save = Save::new(json!({ "data": { "Lab": "[[1,2],[3,4]]" } }));
        // The per-cell readers below `parts` read the field already decoded…
        let (fields, _) = save.parts().expect("the save has fields");
        assert_eq!(fields["Lab"], json!([[1, 2], [3, 4]]));
        // …while a project export writes the save back as the game wrote it.
        assert_eq!(save.root()["data"]["Lab"], json!("[[1,2],[3,4]]"));
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

    #[test]
    fn an_absurd_character_suffix_is_ignored() {
        let save = Save::new(json!({
            "Lv0_0": "[]",
            "Lv0_99999999999": "[]",
            "SL_18446744073709551615": "[]",
        }));
        assert_eq!(save.character_count(), 1);
    }

    #[test]
    fn an_absurd_row_length_is_capped() {
        assert!(row_values(&json!({ "length": 1e15 })).len() <= crate::MAX_ROW_CELLS);
        assert!(row_values(&json!({ "999999999999": 1 })).is_empty());
        assert_eq!(row_values(&json!({ "0": 1, "2": 3, "length": 3 })).len(), 3);
    }

    #[test]
    fn a_cloned_save_shares_its_tree() {
        let save = Save::new(json!({ "Lv0_0": "[]" }));
        let copy = save.clone();
        assert!(std::ptr::eq(save.root(), copy.root()));
    }
}
