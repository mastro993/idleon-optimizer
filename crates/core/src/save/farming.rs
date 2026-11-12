//! The farming side of a save: plots, crop quantities, Land Rank and market
//! levels, as the upstream decoder reads `FarmPlot`, `FarmCrop`, `FarmRank` and
//! `FarmUpg`.

use super::json;
use crate::farming::{Farm, PLOTS, Plot, RANK_UPGRADES};
use serde_json::{Map, Value};

/// Top-level names that mark an object as carrying farming state, whichever
/// shape the object uses (`FarmPlot` rows, or the decoded names of a state
/// export).
pub(crate) const FARMING_FIELDS: [&str; 9] = [
    "FarmPlot",
    "FarmRank",
    "FarmUpg",
    "FarmCrop",
    "farmPlotData",
    "farmRankData",
    "farmUpgData",
    "farmCropData",
    "farmCropCount",
];

/// The raw save names; the decoded ones mark a state export instead.
pub(crate) const RAW_FARMING_FIELDS: [&str; 4] = ["FarmPlot", "FarmRank", "FarmUpg", "FarmCrop"];

/// Whether the save carries any farming field.
pub(crate) fn has_farming(save: &Map<String, Value>) -> bool {
    FARMING_FIELDS.iter().any(|field| save.contains_key(*field))
}

/// Read the farming state, or `None` when the save has no farming fields at all.
pub(crate) fn farm(
    save: &Map<String, Value>,
    farm_level: u32,
    warnings: &mut Vec<String>,
) -> Option<Farm> {
    if !has_farming(save) {
        return None;
    }
    let mut farm = Farm {
        farm_level,
        ..Farm::default()
    };

    match field(save, "farmPlotData", "FarmPlot") {
        Some(value) => farm.plots = plot_rows(&value, warnings),
        None => warnings.push("FarmPlot: not in the save; the field stays empty".into()),
    }

    match field(save, "farmRankData", "FarmRank") {
        Some(rank) => {
            let rows = rows(&rank, 3);
            farm.rank_ranks = numbers(rows.first(), PLOTS, "FarmRank[0]", warnings);
            farm.rank_progress = floats(rows.get(1).cloned().unwrap_or_default(), PLOTS);
            farm.rank_levels = numbers(rows.get(2), RANK_UPGRADES, "FarmRank[2]", warnings);
        }
        None => warnings
            .push("FarmRank: not in the save; plot ranks and Land Rank levels stay at 0".into()),
    }

    match field(save, "farmUpgData", "FarmUpg") {
        Some(levels) => farm.market_levels = floats(entries(&levels), usize::MAX),
        None => warnings.push("FarmUpg: not in the save; market levels stay at 0".into()),
    }

    match field(save, "farmCropData", "FarmCrop") {
        Some(crops) => farm.crop_quantities = crop_quantities(&crops),
        None => warnings.push("FarmCrop: not in the save; crop quantities stay empty".into()),
    }
    farm.crop_count = field(save, "farmCropCount", "farmCropCount")
        .and_then(|count| json::number(&count))
        .map(|count| count.max(0.0).round() as u32)
        .unwrap_or(farm.crop_quantities.len() as u32);

    if let Some(spelunk) = field(save, "spelunkData", "Spelunk") {
        farm.organic_exotics = entries(&spelunk)
            .get(50)
            .and_then(Value::as_array)
            .map(|labels| {
                labels
                    .iter()
                    .filter_map(json::number)
                    .map(|label| label.max(0.0) as u32)
                    .collect()
            })
            .unwrap_or_default();
    }
    if let Some(gems) = field(save, "gemItemsData", "GemItemsPurchased") {
        farm.gem_items = floats(entries(&gems), usize::MAX);
    }
    farm.emporium_owned = field(save, "ninjaData", "Ninja")
        .and_then(|ninja| {
            json::index(&ninja, &[102, 9])
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .unwrap_or_default();
    if let Some(grimoire) = field(save, "grimoireData", "Grimoire") {
        let levels = floats(entries(&grimoire), usize::MAX);
        let level = |index: usize| levels.get(index).copied().unwrap_or(0.0);
        farm.grimoire_crop_scientist = level(22) * (1.0 + level(36) / 100.0);
    }
    if let Some(vault) = field(save, "vaultData", "UpgVault") {
        farm.vault_crop_scientist = entries(&vault)
            .get(79)
            .and_then(json::number)
            .unwrap_or(0.0);
    }

    warnings.push(
        "farming multipliers: only market levels, Land Rank, exotics, crop counts, gem items, Spelunk organic labels, the emporium encoding, Grimoire 22/36 and Vault 79 are applied; chances and values are lower bounds".into(),
    );
    Some(farm)
}

/// A decoded field, else its raw save name; either may be a JSON string.
fn field(save: &Map<String, Value>, decoded: &str, raw: &str) -> Option<Value> {
    json::parsed_field(save, decoded).or_else(|| json::parsed_field(save, raw))
}

/// `FarmPlot` rows, padded to the 36-plot field.
fn plot_rows(value: &Value, warnings: &mut Vec<String>) -> Vec<Plot> {
    let Some(rows) = value.as_array() else {
        warnings.push("FarmPlot: not an array of plot rows; the field stays empty".into());
        return vec![Plot::default(); PLOTS];
    };
    let mut plots = vec![Plot::default(); PLOTS];
    for (index, row) in rows.iter().take(PLOTS).enumerate() {
        let Some(cells) = row.as_array() else {
            continue;
        };
        let number = |slot: usize| cells.get(slot).and_then(json::number);
        plots[index] = Plot {
            seed_family: number(0).map(|value| value.round() as i64).unwrap_or(-1),
            growth_progress: number(1).unwrap_or(0.0),
            evolution: number(2).map(|value| value.max(0.0) as u32).unwrap_or(0),
            evolution_blocked: number(3).is_some_and(|value| value == 1.0),
            crops_on_vine: number(4).map(|value| value.max(0.0) as u32).unwrap_or(0),
            overgrowth: number(5).map(|value| value.max(0.0) as u32).unwrap_or(0),
            overflow_progress: number(6).unwrap_or(0.0),
        };
    }
    if rows.len() < PLOTS {
        warnings.push(format!(
            "FarmPlot: {} of {PLOTS} plots; the rest stay empty",
            rows.len()
        ));
    } else if rows.len() > PLOTS {
        warnings.push(format!(
            "FarmPlot: {} plots, only the first {PLOTS} are used",
            rows.len()
        ));
    }
    plots
}

/// `FarmRank` rows: the game writes an object keyed `0`/`1`/`2`, a state
/// export writes the three arrays directly.
fn rows(value: &Value, count: usize) -> Vec<Vec<Value>> {
    let mut rows = vec![Vec::new(); count];
    match value {
        Value::Array(items) => {
            for (index, item) in items.iter().take(count).enumerate() {
                rows[index] = item.as_array().cloned().unwrap_or_default();
            }
        }
        Value::Object(fields) => {
            for (index, row) in rows.iter_mut().enumerate() {
                *row = fields
                    .get(&index.to_string())
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default();
            }
        }
        _ => {}
    }
    rows
}

/// A save field's entries: the array itself, or its object values in key order.
fn entries(value: &Value) -> Vec<Value> {
    match value {
        Value::Array(items) => items.clone(),
        Value::Object(fields) => {
            let mut keys: Vec<&String> = fields.keys().collect();
            keys.sort_by_key(|key| key.parse::<u64>().unwrap_or(u64::MAX));
            keys.into_iter()
                .filter_map(|key| fields.get(key).cloned())
                .collect()
        }
        _ => Vec::new(),
    }
}

/// Whole numbers from one `FarmRank` row, padded to `count`.
fn numbers(
    row: Option<&Vec<Value>>,
    count: usize,
    name: &str,
    warnings: &mut Vec<String>,
) -> Vec<u32> {
    let row = row.map(Vec::as_slice).unwrap_or_default();
    if row.is_empty() {
        warnings.push(format!("{name}: missing from the save; kept at 0"));
    } else if row.len() < count {
        warnings.push(format!(
            "{name}: {} of {count} entries; the rest stay at 0",
            row.len()
        ));
    }
    (0..count)
        .map(|index| {
            row.get(index)
                .and_then(json::number)
                .map(|value| value.max(0.0).round() as u32)
                .unwrap_or(0)
        })
        .collect()
}

fn floats(values: Vec<Value>, count: usize) -> Vec<f64> {
    values
        .iter()
        .take(count)
        .map(|value| json::number(value).unwrap_or(0.0).max(0.0))
        .collect()
}

/// `farmCropData`: an object keyed by crop id, with string or numeric quantities.
fn crop_quantities(value: &Value) -> std::collections::BTreeMap<u32, f64> {
    let mut crops = std::collections::BTreeMap::new();
    let Value::Object(fields) = value else {
        return crops;
    };
    for (key, quantity) in fields {
        let Ok(crop) = key.parse::<u32>() else {
            continue;
        };
        crops.insert(crop, json::number(quantity).unwrap_or(0.0));
    }
    crops
}
