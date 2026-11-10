//! The research side of a save: grid levels, observations, magnifiers, shapes,
//! and the research level and EXP the run starts from.

use super::Character;
use super::json;
use crate::data::game;
use crate::model::{GridState, Mag, MagKind, Observations, Placement};
use serde_json::{Map, Value};

/// A magnifier's `(type, slot)` from the save.
type MagEntry = (Option<f64>, Option<f64>);

/// A shape's `(x, y, rotation)` from the save.
type Position = (Option<f64>, Option<f64>, Option<f64>);

/// The research rows: `Research` is a JSON string in the game's save, but the
/// app's own state exports use a plain array.
pub(crate) fn research_rows(
    save: &Map<String, Value>,
    warnings: &mut Vec<String>,
) -> Vec<Vec<Value>> {
    let Some(field) = save.get("Research") else {
        return Vec::new();
    };
    let parsed = match field {
        Value::String(text) => match serde_json::from_str::<Value>(text) {
            Ok(parsed) => parsed,
            Err(error) => {
                warnings.push(format!(
                    "Research: the research string is not JSON ({error})"
                ));
                return Vec::new();
            }
        },
        other => other.clone(),
    };
    let Value::Array(rows) = parsed else {
        warnings.push("Research: not an array of rows".into());
        return Vec::new();
    };
    rows.into_iter()
        .map(|row| match row {
            Value::Array(cells) => cells,
            _ => Vec::new(),
        })
        .collect()
}

/// Research level: the decoded field when present, else the highest character
/// level, as the upstream decoder reads it.
pub(crate) fn research_level(save: &Map<String, Value>, characters: &[Character]) -> Option<u32> {
    save.get("researchLevel")
        .and_then(json::number)
        .map(|level| level.max(0.0).round() as u32)
        .or_else(|| {
            characters
                .iter()
                .filter_map(|character| character.research_level)
                .max()
        })
}

/// Accumulated research EXP: the decoded field, else the EXP of a character at
/// the imported research level.
pub(crate) fn research_exp(
    save: &Map<String, Value>,
    characters: &[Character],
    research_level: u32,
) -> f64 {
    save.get("researchExp")
        .and_then(json::number)
        .or_else(|| save.get("cachedResearchExp").and_then(json::number))
        .or_else(|| {
            characters
                .iter()
                .filter(|character| character.research_level == Some(research_level))
                .filter_map(|character| character.research_exp)
                .max_by(f64::total_cmp)
        })
        .unwrap_or(0.0)
}

/// Per-square levels: the decoded `gridLevels`, else row 0 of `Research`.
pub(crate) fn grid_levels(
    save: &Map<String, Value>,
    rows: &[Vec<Value>],
) -> Option<Vec<Option<f64>>> {
    field_array(save, "gridLevels", rows, 0)
}

/// Build the grid, padding short arrays and trimming long ones.
pub(crate) fn grid(levels: &[Option<f64>], warnings: &mut Vec<String>) -> GridState {
    let cells = game().cells();
    let mut grid = GridState::empty();
    for (cell, level) in levels.iter().take(cells).enumerate() {
        if let Some(level) = level {
            grid.set(cell, level.max(0.0).round() as u32);
        }
    }
    if levels.len() < cells {
        warnings.push(format!(
            "gridLevels: {} of {cells} squares; the rest stay at level 0",
            levels.len()
        ));
    } else if levels.len() > cells {
        warnings.push(format!(
            "gridLevels: {} entries, only the first {cells} squares are used",
            levels.len()
        ));
    }
    grid
}

/// Observation found flags, insight levels and insight progress.
pub(crate) fn observations(
    save: &Map<String, Value>,
    rows: &[Vec<Value>],
    warnings: &mut Vec<String>,
) -> Observations {
    let mut observations = Observations::default();
    let count = observations.found.len();

    match field_array(save, "occFound", rows, 2) {
        Some(values) => {
            for (obs, value) in values.iter().take(count).enumerate() {
                observations.found[obs] = value.is_some_and(|value| value >= 1.0);
            }
        }
        None => warnings.push("occFound: not in the save; no observation is found".into()),
    }
    match field_array(save, "insightLvs", rows, 4) {
        Some(values) => {
            for (obs, value) in values.iter().take(count).enumerate() {
                if let Some(value) = value {
                    observations.insight_level[obs] = value.max(0.0).round() as u32;
                }
            }
        }
        None => warnings.push("insightLvs: not in the save; insight stays at level 0".into()),
    }
    match field_array(save, "insightProgress", rows, 3) {
        Some(values) => {
            for (obs, value) in values.iter().take(count).enumerate() {
                if let Some(value) = value {
                    observations.insight_exp[obs] = value.max(0.0);
                }
            }
        }
        None => warnings.push("insightProgress: not in the save; insight EXP stays at 0".into()),
    }
    observations
}

/// Magnifiers with the observation they sit on. The save's type codes are
/// `0` magnifying glass, `1` optical monocle, `2` kaleidoscope.
pub(crate) fn mags(
    save: &Map<String, Value>,
    rows: &[Vec<Value>],
    warnings: &mut Vec<String>,
) -> Vec<Mag> {
    let entries = save.get("magData").and_then(mag_entries).or_else(|| {
        rows.get(5)
            .filter(|flat| !flat.is_empty())
            .and_then(|flat| mag_entries(&Value::Array(flat.clone())))
    });
    let entries = match entries {
        Some(entries) => entries,
        None => {
            warnings.push("magData: not in the save; no magnifier imported".into());
            return Vec::new();
        }
    };

    let slots = game().observations.len();
    let mut mags = Vec::with_capacity(entries.len());
    let mut reported: Vec<usize> = Vec::new();
    for (kind, slot) in entries {
        let kind = match kind.map(|kind| kind.round() as u32) {
            Some(0) => MagKind::Regular,
            Some(1) => MagKind::Monocle,
            Some(2) => MagKind::Kaleidoscope,
            Some(other) => {
                warnings.push(format!("magData: unknown magnifier type {other}; skipped"));
                continue;
            }
            None => {
                warnings.push("magData: an entry carries no type; skipped".into());
                continue;
            }
        };
        let slot = slot.and_then(|slot| (slot >= 0.0).then(|| slot.round() as usize));
        match slot {
            Some(slot) if slot >= slots => {
                if !reported.contains(&slot) {
                    reported.push(slot);
                    warnings.push(format!(
                        "magData: a magnifier sits on observation slot {slot}, outside the {slots} observations modelled; imported unplaced"
                    ));
                }
                mags.push(Mag::new(kind, None));
            }
            _ => mags.push(Mag::new(kind, slot)),
        }
    }
    mags
}

/// Magnifier `(type, slot)` pairs from either the decoded objects or the
/// save's flat groups of four numbers `[x, y, slot, type]`.
fn mag_entries(value: &Value) -> Option<Vec<MagEntry>> {
    let items = value.as_array()?;
    if items.iter().all(Value::is_object) {
        return Some(
            items
                .iter()
                .map(|item| {
                    (
                        item.get("type").and_then(json::number),
                        item.get("slot").and_then(json::number),
                    )
                })
                .collect(),
        );
    }
    Some(
        items
            .chunks(4)
            .map(|group| {
                (
                    group.get(3).and_then(json::number),
                    group.get(2).and_then(json::number),
                )
            })
            .collect(),
    )
}

/// Shape placements. The save's `shapePositions` are groups of four numbers
/// `[x, y, rotation, unknown]`, indexed by shape.
pub(crate) fn placements(
    save: &Map<String, Value>,
    rows: &[Vec<Value>],
    warnings: &mut Vec<String>,
) -> Vec<Placement> {
    let positions = save
        .get("shapePositions")
        .and_then(position_entries)
        .or_else(|| {
            rows.get(6)
                .filter(|flat| !flat.is_empty())
                .and_then(|flat| position_entries(&Value::Array(flat.clone())))
        });
    let positions = match positions {
        Some(positions) => positions,
        None => {
            if save.contains_key("shapeOverlay") {
                warnings.push(
                    "shapeOverlay: present, but the placements behind it cannot be rebuilt; no shape is placed"
                        .into(),
                );
            } else {
                warnings.push("shapePositions: not in the save; no shape is placed".into());
            }
            return Vec::new();
        }
    };

    let shapes = game().shapes.len();
    let mut placements = Vec::new();
    for (shape, (x, y, rot)) in positions.iter().enumerate() {
        if shape >= shapes {
            warnings.push(format!(
                "shapePositions: {} entries, only {shapes} shapes exist; the rest are ignored",
                positions.len()
            ));
            break;
        }
        let (Some(x), Some(y)) = (x, y) else {
            continue;
        };
        if !x.is_finite() || !y.is_finite() {
            continue;
        }
        placements.push(Placement {
            shape,
            x: *x,
            y: *y,
            rot: rot.unwrap_or(0.0),
        });
    }
    placements
}

/// Shape `(x, y, rotation)` triples from either the decoded objects or the
/// save's flat groups of four numbers `[x, y, rotation, unknown]`.
fn position_entries(value: &Value) -> Option<Vec<Position>> {
    let items = value.as_array()?;
    if items.iter().all(Value::is_object) {
        return Some(
            items
                .iter()
                .map(|item| {
                    (
                        item.get("x").and_then(json::number),
                        item.get("y").and_then(json::number),
                        item.get("rot").and_then(json::number),
                    )
                })
                .collect(),
        );
    }
    Some(
        items
            .chunks(4)
            .map(|group| {
                (
                    group.first().and_then(json::number),
                    group.get(1).and_then(json::number),
                    group.get(2).and_then(json::number),
                )
            })
            .collect(),
    )
}

/// A grid-shaped array of optional numbers.
fn levels(value: &Value) -> Option<Vec<Option<f64>>> {
    value
        .as_array()
        .map(|items| items.iter().map(json::number).collect())
}

/// A decoded field when present, else the matching `Research` row.
fn field_array(
    save: &Map<String, Value>,
    key: &str,
    rows: &[Vec<Value>],
    row: usize,
) -> Option<Vec<Option<f64>>> {
    let values = save.get(key).and_then(levels).or_else(|| {
        rows.get(row)
            .filter(|cells| !cells.is_empty())
            .map(|cells| cells.iter().map(json::number).collect())
    })?;
    (!values.is_empty()).then_some(values)
}
