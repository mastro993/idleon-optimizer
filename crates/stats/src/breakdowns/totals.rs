//! The four `total-str`/`total-agi`/`total-wis`/`total-luk` rows, ported from
//! `js/stats/systems/common/stats.js` by `idleon-stats-engine`.
//!
//! The engine resolves the pools it models and names every term it leaves out,
//! so the row shows a real total with a partial reason rather than a dash.

use super::{Inputs, Resolution};
use idleon_common::formula::ValueFormat;
use idleon_stats_engine::{Stat, total_stat};

/// Resolve one base-stat descriptor, when the id is one of the four.
pub(super) fn resolve(inputs: &Inputs<'_>, id: &str) -> Option<Resolution> {
    let stat = match id {
        "total-str" => Stat::Str,
        "total-agi" => Stat::Agi,
        "total-wis" => Stat::Wis,
        "total-luk" => Stat::Luk,
        _ => return None,
    };
    Some(
        match total_stat(inputs.save, inputs.simulation, inputs.character, stat) {
            Some(total) => Resolution::Value {
                partial: (!total.is_complete()).then(|| {
                    format!(
                        "Lower bound: {} of the upstream terms are not modelled ({}).",
                        total.missing.len(),
                        total.missing.join("; ")
                    )
                }),
                tree: total
                    .tree
                    .with_format(ValueFormat::Count)
                    .with_source("js/stats/systems/common/stats.js · computeTotalStat"),
            },
            None => Resolution::Unavailable {
                reason: format!(
                    "The imported save does not carry the characters {} needs.",
                    stat.id()
                ),
            },
        },
    )
}
