//! Statue levels from a decoded save: one `StatueLevels_<n>` row per character,
//! each holding `[level, exp]` pairs in statue order.
//!
//! The upstream loader (`js/save/loader.js`) reads the same key and stores the
//! rows as `statueLvAllData`; the decoded app-state export is accepted as a
//! fallback. A row the save does not carry stays `None` so the section can show
//! it as unavailable instead of treating it as level zero.

use idleon_save::json;
use idleon_statue::{self as statue, Progress, StatueSave};
use serde_json::{Map, Value};

pub(crate) fn read(
    save: &Map<String, Value>,
    root: &Map<String, Value>,
    warnings: &mut Vec<String>,
) -> StatueSave {
    let names = names(root).or_else(|| names(save)).unwrap_or_default();
    let mut characters: Vec<Vec<Option<Progress>>> = Vec::new();
    let mut found = false;

    for key in save.keys() {
        let Some(index) = key
            .strip_prefix("StatueLevels_")
            .and_then(|tail| tail.parse::<usize>().ok())
        else {
            continue;
        };
        found = true;
        if characters.len() <= index {
            characters.resize(index + 1, empty_rows());
        }
        characters[index] = rows(json::parsed_field(save, key).as_ref());
    }

    if !found && let Some(decoded) = save.get("statueLvAllData").and_then(Value::as_array) {
        found = true;
        characters = decoded.iter().map(|value| rows(Some(value))).collect();
    }

    if !found {
        warnings.push(
            "statues: no StatueLevels_<n> rows in the save; statue levels unavailable".into(),
        );
        return StatueSave::default();
    }

    let imported = StatueSave::new(characters, names);
    if imported.is_empty() {
        warnings
            .push("statues: no readable statue level/EXP rows; statue levels unavailable".into());
        return imported;
    }
    if let Some((character, missing)) = imported.character_indices().find_map(|character| {
        let missing = imported.missing_rows(character);
        (missing > 0).then_some((character, missing))
    }) {
        let carried = statue::count() - missing;
        warnings.push(format!(
            "statues: character {} carries {carried} of {} statue rows; missing rows are unavailable",
            character + 1,
            statue::count()
        ));
    }
    imported
}

/// The `charNames` roster, when the save carries one.
fn names(map: &Map<String, Value>) -> Option<Vec<String>> {
    let value = json::parsed_field(map, "charNames")?;
    Some(
        value
            .as_array()?
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect(),
    )
}

fn empty_rows() -> Vec<Option<Progress>> {
    vec![None; statue::count()]
}

/// One character's rows: every statue slot, with unreadable pairs left empty.
fn rows(value: Option<&Value>) -> Vec<Option<Progress>> {
    let cells = value
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    (0..statue::count())
        .map(|statue| progress(cells.get(statue)))
        .collect()
}

fn progress(value: Option<&Value>) -> Option<Progress> {
    let pair = value?.as_array()?;
    let level = json::number(pair.first()?)?;
    let exp = json::number(pair.get(1)?)?;
    if !level.is_finite()
        || !exp.is_finite()
        || level < 0.0
        || exp < 0.0
        || level.fract() != 0.0
        || level > f64::from(u32::MAX)
    {
        return None;
    }
    Some(Progress {
        level: level as u32,
        exp,
    })
}
