//! The `it.json` companion roster, shared by the sections that read companions.
//!
//! The wrapper lists one `"<id>,<name>,…,<level-two flag>"` string per owned
//! companion beside the save. A plain `save.json` has no roster, so a section
//! reading a companion bonus reports it as unavailable instead of guessing.

use serde_json::Value;
use std::collections::BTreeSet;

/// The companions an `it.json` wrapper carries.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Roster {
    /// Companion ids owned.
    pub owned: BTreeSet<u32>,
    /// Companion ids at level two, whose bonus differs from level one.
    pub enhanced: BTreeSet<u32>,
    /// Whether the wrapper carried a roster at all.
    pub available: bool,
}

impl Roster {
    /// Whether the save owns a companion.
    pub fn owns(&self, id: u32) -> bool {
        self.owned.contains(&id)
    }

    /// Whether a companion is at level two.
    pub fn is_enhanced(&self, id: u32) -> bool {
        self.enhanced.contains(&id)
    }
}

/// Read the roster beside a save; a missing or unreadable list is unavailable.
pub(crate) fn read(companion: Option<&Value>) -> Roster {
    let Some(list) = companion.and_then(|root| at_key(root, "l")) else {
        return Roster::default();
    };
    let entries: Vec<String> = match list {
        Value::Array(items) => items
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect(),
        Value::Object(fields) => {
            let mut keys: Vec<&String> = fields.keys().collect();
            keys.sort_by_key(|key| key.parse::<u64>().unwrap_or(u64::MAX));
            keys.into_iter()
                .filter_map(|key| fields.get(key).and_then(Value::as_str))
                .map(str::to_owned)
                .collect()
        }
        _ => return Roster::default(),
    };
    let mut roster = Roster {
        available: true,
        ..Roster::default()
    };
    for entry in entries {
        let fields: Vec<&str> = entry.split(',').collect();
        let Some(id) = fields.first().and_then(|id| id.trim().parse::<u32>().ok()) else {
            continue;
        };
        roster.owned.insert(id);
        let level_two = fields
            .get(4)
            .and_then(|flag| flag.trim().parse::<f64>().ok())
            .unwrap_or(0.0);
        if level_two >= 1.0 {
            roster.enhanced.insert(id);
        }
    }
    roster
}

/// Index into an array, or a named field in an object.
fn at_key<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    match value {
        Value::Object(fields) => fields.get(key),
        Value::Array(items) => key.parse::<usize>().ok().and_then(|i| items.get(i)),
        _ => None,
    }
}
