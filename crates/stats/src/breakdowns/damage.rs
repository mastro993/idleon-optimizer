//! The `damage` row, resolved by `idleon-stats-engine`.
//!
//! The descriptor's value is its maximum damage, `DDL0 × DDL1 × DDL2`, so the
//! row's tree is that product over the three layers. The row is marked
//! partial when the import lacks the companion roster or the server's vote row
//! ([`Metadata`]); a term the engine cannot justify is named too, so the row
//! never shows a lower bound as complete. A dungeon map has no value.

use super::{Inputs, Resolution};
use idleon_common::formula::{FormulaNode, ValueFormat};
use idleon_save::{Roster, Save};
use idleon_stats_engine::{damage_layers, is_dungeon_map};

/// Why the damage is hidden on a dungeon map.
pub const DUNGEON: &str =
    "Unavailable: current-run Dungeon DMG and Mastery are not stored in save JSON.";

/// The saved metadata the damage descriptor reads.
///
/// The companion roster comes from the `it.json` wrapper's `companion.l` list
/// and the active vote from `serverVars.voteCategories[0]`; both default to
/// unavailable, so a plain `save.json` reads as missing both.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Metadata {
    /// Whether the import carried the companion roster.
    pub companions: bool,
    /// Whether it carried the server's active-vote row.
    pub vote: bool,
}

impl Metadata {
    /// Both fields present, for a summary built without a save.
    pub const COMPLETE: Metadata = Metadata {
        companions: true,
        vote: true,
    };

    /// Read the pair from an import, wrapped or flat.
    pub fn read(save: &Save) -> Self {
        Self {
            companions: Roster::read(save.root().get("companion")).available,
            vote: save
                .get("serverVars")
                .and_then(|vars| vars.get("voteCategories").cloned())
                .and_then(|categories| categories.get(0).cloned())
                .and_then(|vote| js_number(&vote))
                .is_some_and(f64::is_finite),
        }
    }

    /// Whether the import carried everything the descriptor reads.
    pub fn complete(&self) -> bool {
        self.companions && self.vote
    }

    /// The descriptor's `missingMetadata` list, in its order.
    pub fn missing(&self) -> Vec<&'static str> {
        let mut missing = Vec::new();
        if !self.companions {
            missing.push("companion ownership");
        }
        if !self.vote {
            missing.push("current server vote");
        }
        missing
    }

    /// The descriptor's `reason`, empty when nothing is missing.
    pub fn reason(&self) -> String {
        let missing = self.missing();
        if missing.is_empty() {
            return String::new();
        }
        format!(
            "Partial total: the imported JSON does not include {} metadata.",
            missing.join(" or ")
        )
    }
}

/// JavaScript's `Number(value)` for the saved vote row: numbers and numeric
/// strings read as numbers, and anything else is not a value.
fn js_number(value: &serde_json::Value) -> Option<f64> {
    match value {
        serde_json::Value::Number(number) => number.as_f64(),
        serde_json::Value::String(text) => text.trim().parse().ok(),
        serde_json::Value::Bool(flag) => Some(if *flag { 1.0 } else { 0.0 }),
        _ => None,
    }
}

/// Resolve the `damage` descriptor, when the id is that one.
pub(super) fn resolve(inputs: &Inputs<'_>, id: &str) -> Option<Resolution> {
    if id != "damage" {
        return None;
    }
    if is_dungeon_map(inputs.map) {
        return Some(Resolution::Unavailable {
            reason: DUNGEON.into(),
        });
    }
    let Some(layers) = damage_layers(inputs.save, inputs.simulation, inputs.character, inputs.map)
    else {
        return Some(Resolution::Unavailable {
            reason: "The imported save does not carry the characters the damage row needs.".into(),
        });
    };
    let mut reasons = vec![Metadata::read(inputs.save).reason()];
    if !layers.is_complete() {
        reasons.push(format!(
            "Lower bound: {} damage term(s) are not modelled ({}).",
            layers.missing.len(),
            layers.missing.join("; ")
        ));
    }
    let reasons: Vec<String> = reasons
        .into_iter()
        .filter(|reason| !reason.is_empty())
        .collect();
    let max = if layers.max.is_finite() {
        layers.max
    } else {
        0.0
    };
    Some(Resolution::Value {
        partial: (!reasons.is_empty()).then(|| reasons.join(" ")),
        tree: FormulaNode::leaf("Max Damage", max, ValueFormat::Raw)
            .with_input("Base × Percentage × Multiplicative")
            .with_source("DamageDealed(\"Max\")")
            .with_children([
                FormulaNode::leaf("Base Damage (with softcap)", layers.base, ValueFormat::Raw)
                    .with_note(format!(
                        "{} before the softcaps",
                        ValueFormat::Raw.format(layers.base_pre_softcap)
                    )),
                FormulaNode::leaf(
                    "Percentage Damage Multiplier",
                    layers.percent,
                    ValueFormat::Multiplier,
                )
                .with_multiplier(layers.percent)
                .with_note(format!(
                    "{} before the softcaps",
                    ValueFormat::Multiplier.format(layers.percent_pre_softcap)
                )),
                FormulaNode::leaf(
                    "Multiplicative Damage Bonuses",
                    layers.multiplier,
                    ValueFormat::Multiplier,
                )
                .with_multiplier(layers.multiplier)
                .with_note(format!(
                    "{} before the softcaps",
                    ValueFormat::Multiplier.format(layers.multiplier_pre_softcap)
                )),
            ]),
    })
}
