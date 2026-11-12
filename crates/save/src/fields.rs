//! Reading save fields the account decoder needs.
//!
//! The game writes most account rows as JSON strings and spreads some tables
//! across `<name>_<char>` keys; these helpers mirror the upstream
//! `parseSaveKey`/array access so every system reads the same shape.

use crate::json;
use serde_json::{Map, Value};

/// A decoded save field, accepting the JSON-string form.
pub fn field(save: &Map<String, Value>, key: &str) -> Option<Value> {
    json::parsed_field(save, key)
}

/// A numeric row of a decoded field, as many save rows are arrays of numbers.
pub fn numbers(save: &Map<String, Value>, key: &str) -> Option<Vec<Option<f64>>> {
    let value = field(save, key)?;
    values(&value)
}

pub fn values(value: &Value) -> Option<Vec<Option<f64>>> {
    value
        .as_array()
        .map(|items| items.iter().map(json::number).collect())
}

/// A nested number at `path`, e.g. `Holes[31][2][16]`.
pub fn at(save: &Map<String, Value>, key: &str, path: &[usize]) -> Option<f64> {
    let value = field(save, key)?;
    json::index(&value, path).and_then(json::number)
}

/// A nested row as numbers.
pub fn row(save: &Map<String, Value>, key: &str, path: &[usize]) -> Option<Vec<Option<f64>>> {
    let value = field(save, key)?;
    values(json::index(&value, path)?)
}

/// A per-character field: `<key>_<char>`, decoded.
pub fn char_field(save: &Map<String, Value>, key: &str, char: usize) -> Option<Value> {
    field(save, &format!("{key}_{char}"))
}

/// The row at `index` of a nested array field, accepting object rows with
/// numeric string keys (older exports write sparse rows this way).
pub fn object_row(value: &Value, index: usize) -> Option<Vec<Option<f64>>> {
    let row = match value {
        Value::Array(rows) => rows.get(index)?,
        Value::Object(rows) => rows.get(&index.to_string())?,
        _ => return None,
    };
    if row.is_array() {
        let numbers = values(row);
        if numbers
            .as_ref()
            .is_some_and(|cells| cells.iter().any(Option::is_some))
        {
            return numbers;
        }
    }
    let fields = row.as_object()?;
    let mut cells: Vec<Option<f64>> = Vec::new();
    // A sparse row serialises as `{ length, 0: .., 1: .. }`; `length` is the
    // array's own length, not one of its values, so it stays out of the row.
    let mut keys: Vec<(usize, &String)> = fields
        .keys()
        .filter_map(|key| key.parse::<usize>().ok().map(|index| (index, key)))
        .collect();
    keys.sort_by_key(|(index, _)| *index);
    for (index, key) in keys {
        if cells.len() <= index {
            cells.resize(index + 1, None);
        }
        cells[index] = fields.get(key).and_then(json::number);
    }
    Some(cells)
}

/// A row of string entries, as gallery hats and capes are stored.
pub fn strings(save: &Map<String, Value>, key: &str, path: &[usize]) -> Option<Vec<String>> {
    let value = field(save, key)?;
    let row = json::index(&value, path)?.as_array()?;
    Some(
        row.iter()
            .map(|entry| entry.as_str().map(str::to_owned).unwrap_or_default())
            .collect(),
    )
}

/// A decoded JSON object field, accepting the string form.
pub fn object(save: &Map<String, Value>, key: &str) -> Option<Map<String, Value>> {
    match field(save, key)? {
        Value::Object(fields) => Some(fields),
        _ => None,
    }
}

/// `Number(value) || 0`, the upstream coercion.
pub fn number_at(values: &[Option<f64>], index: usize) -> f64 {
    values.get(index).copied().flatten().unwrap_or(0.0)
}

/// A non-negative length: `arr ? arr.length : 0`.
pub fn len_of(save: &Map<String, Value>, key: &str) -> usize {
    field(save, key)
        .and_then(|value| value.as_array().map(Vec::len))
        .unwrap_or(0)
}
