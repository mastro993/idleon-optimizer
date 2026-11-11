//! The coral god rank planner's save-derived inputs.
//!
//! Mirrors `coralGodRankInputs` in
//! `js/stats/systems/w5/coral-god-rank.js`: each planner value comes from the
//! decoded save, and a missing field is named instead of guessed.

use super::Character;
use super::json;
use crate::coral::{SaveInputs, upgrade_slot, y4_base};
use serde_json::{Map, Value};

/// The Y4 bubble (`GIFTS_ABOUND`) sits at `AlchemyDescription[3][23]`.
const Y4_CAULDRON: usize = 3;
const Y4_BUBBLE: usize = 23;
/// Vault upgrade 76 (`Big Blessings`) grants its per-level bonus with no
/// mastery multiplier (`VAULT_NO_MASTERY` contains 76); the per-level value is 1.
const VAULT_76_SLOT: usize = 76;
const VAULT_76_PER_LEVEL: f64 = 1.0;
/// The cauldron letters the Prisma list in `OptLacc[384]` uses (`N2L`).
const CAULDRON_LETTERS: [char; 4] = ['_', 'a', 'b', 'c'];

/// Read every planner input the save carries, naming what it does not.
pub(crate) fn inputs(
    save: &Map<String, Value>,
    root: &Map<String, Value>,
    characters: &[Character],
    rows: &[Vec<Value>],
) -> SaveInputs {
    let mut inputs = SaveInputs::default();
    let missing = &mut inputs.missing;

    let divinity = field(save, "divinityData", "Divinity");
    match &divinity {
        Some(divinity) => {
            inputs.budget = cell(divinity, 24).and_then(json::number);
            inputs.god_rank = cell(divinity, 25).and_then(json::number);
            inputs.tier = cell(divinity, 26).and_then(json::number);
            for (value, name) in [
                (inputs.budget, "Divinity[24] budget"),
                (inputs.god_rank, "Divinity[25] god rank"),
                (inputs.tier, "Divinity[26] offer tier"),
            ] {
                if value.is_none() {
                    missing.push(name.into());
                }
            }
        }
        None => missing.push("Divinity row (budget, god rank, tier)".into()),
    }

    let options = field(save, "optionsListData", "OptLacc");
    match &options {
        Some(options) => {
            inputs.levels = std::array::from_fn(|index| {
                cell(options, upgrade_slot(index))
                    .and_then(json::number)
                    .map(level)
            });
            inputs.singularity_levels = std::array::from_fn(|index| {
                cell(options, upgrade_slot(index + 6))
                    .and_then(json::number)
                    .map(level)
            });
            if inputs.levels.iter().all(Option::is_none) {
                missing.push("Coral Kid levels (OptLacc 427-432)".into());
            }
            if inputs.singularity_levels.iter().all(Option::is_none) {
                missing.push("Singularity levels (OptLacc 616-621)".into());
            }
        }
        None => missing.push("Coral Kid and Singularity levels (OptLacc)".into()),
    }

    let spelunk = field(save, "spelunkData", "Spelunk");
    inputs.reef_upgrade_count = options
        .as_ref()
        .and_then(|options| named(options, "reefUpgradeCount"))
        .and_then(json::number)
        .or_else(|| {
            let row = cell(spelunk.as_ref()?, 13)?;
            let total: f64 = (0..6)
                .filter_map(|slot| cell(row, slot).and_then(json::number))
                .map(|value| value.max(0.0))
                .sum();
            Some(total)
        });
    if inputs.reef_upgrade_count.is_none() {
        missing.push("Reef upgrades (OptLacc.reefUpgradeCount or Spelunk[13])".into());
    }

    inputs.mine_floor = save
        .get("stateR7")
        .and_then(|row| cell(row, 4))
        .or_else(|| rows.get(7).and_then(|row| row.get(4)))
        .and_then(json::number)
        .map(|floor| floor.max(0.0));
    if inputs.mine_floor.is_none() {
        missing.push("Minehead floor (Research row 7)".into());
    }

    inputs.vault76_bonus = field(save, "vaultData", "UpgVault")
        .as_ref()
        .and_then(|vault| cell(vault, VAULT_76_SLOT))
        .and_then(json::number)
        .map(|level| level.max(0.0) * VAULT_76_PER_LEVEL);
    if inputs.vault76_bonus.is_none() {
        missing.push("Vault 76 bonus (UpgVault)".into());
    }

    let cauldron = field(save, "cauldronInfoData", "CauldronInfo");
    let y4_level = cauldron
        .as_ref()
        .and_then(|info| cell(info, Y4_CAULDRON))
        .and_then(|row| cell(row, Y4_BUBBLE))
        .and_then(json::number);
    match y4_level {
        Some(_) if prismad(options.as_ref(), Y4_CAULDRON, Y4_BUBBLE) => {
            missing.push("Y4 is Prisma-boosted; enter the value from the alchemy panel".into())
        }
        Some(level) => inputs.y4 = Some(y4_base(level)),
        None => missing.push("Y4 (CauldronInfo[3][23])".into()),
    }

    inputs.div_cost_after3 = save
        .get("serverVarDivCostAfter3")
        .and_then(json::number)
        .or_else(|| {
            root.get("serverVars")
                .and_then(|vars| vars.get("DivCostAfter3"))
                .and_then(json::number)
        });
    if inputs.div_cost_after3.is_none() {
        missing.push("DivCostAfter3 (serverVars)".into());
    }

    let divinity_levels: Vec<u32> = characters
        .iter()
        .filter_map(|character| character.divinity())
        .collect();
    if divinity_levels.is_empty() {
        missing.push("Total Divinity levels (Lv0_<n>[14])".into());
    } else {
        inputs.total_divinity_levels = Some(divinity_levels.iter().sum());
    }

    inputs
}

/// A decoded field (`divinityData`) or the raw save name (`Divinity`).
fn field(save: &Map<String, Value>, decoded: &str, raw: &str) -> Option<Value> {
    save.get(decoded)
        .cloned()
        .or_else(|| json::parsed_field(save, raw))
}

/// An array element or a numeric-keyed object entry, as the decoded saves use.
fn cell(value: &Value, index: usize) -> Option<&Value> {
    match value {
        Value::Array(rows) => rows.get(index),
        Value::Object(map) => map.get(&index.to_string()),
        _ => None,
    }
}

/// A named property, as app state exports `reefUpgradeCount` on the options row.
fn named<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    value.as_object()?.get(key)
}

/// A level as a non-negative integer, matching `Math.max(0, Math.floor(...))`.
fn level(value: f64) -> u32 {
    value.max(0.0).floor() as u32
}

/// Whether the save's Prisma list (`OptLacc[384]`) marks this bubble.
fn prismad(options: Option<&Value>, cauldron: usize, bubble: usize) -> bool {
    let Some(list) = options
        .and_then(|options| cell(options, 384))
        .and_then(Value::as_str)
    else {
        return false;
    };
    let letter = CAULDRON_LETTERS.get(cauldron).copied().unwrap_or('_');
    list.contains(&format!("{letter}{bubble},"))
}
