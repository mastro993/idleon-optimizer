//! The lure/line ranking: every combination of the 15 lures and 15 lines,
//! evaluated for one pool and sorted by the chosen objective.
//!
//! Ported from the upstream `rankFishingToolkits` in
//! `js/stats/systems/w2/fishing.js`. The upstream rebuilds each selection's
//! efficiency and EXP multiplier from the skill systems with the toolkit's `POW`
//! and `EXP`; this port carries both in [`AccountInputs`] and holds them fixed
//! across a ranking, because the skill systems are not decoded yet. Everything
//! else — depth, catch time, the split, ore value, fish and EXP — is resolved
//! per combination through [`evaluate`].

use super::data::data;
use super::{AccountInputs, Evaluation, FishingSave, PoolOutcome, evaluate};

/// What a ranking sorts by, matching the upstream `state.objective`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Objective {
    /// The claim's Fishing EXP.
    #[default]
    Exp,
    /// The claim's credited fish.
    Fish,
}

impl Objective {
    /// The outcome value this objective ranks on.
    pub fn metric(self, outcome: &PoolOutcome) -> f64 {
        match self {
            Objective::Exp => outcome.exp,
            Objective::Fish => outcome.fish_total,
        }
    }

    /// The control label the upstream `renderRanking` shows.
    pub fn label(self) -> &'static str {
        match self {
            Objective::Exp => "Fishing EXP",
            Objective::Fish => "Fish quantity",
        }
    }

    /// What the metric counts, for the table headers.
    pub fn unit(self) -> &'static str {
        match self {
            Objective::Exp => "EXP",
            Objective::Fish => "fish",
        }
    }
}

/// Rank every lure/line combination for one location, best first.
///
/// The rows are the same [`Evaluation`] an explicit selection returns, so the
/// ranking can never disagree with the outcome card. Ties sort by EXP, then
/// fish, then lure and line order, as the upstream comparator does.
pub fn rank(
    save: &FishingSave,
    account: &AccountInputs,
    character: usize,
    location: &str,
    objective: Objective,
    hours: f64,
) -> Vec<Evaluation> {
    let data = data();
    let mut rows = Vec::with_capacity(data.lures.len() * data.lines.len());
    for lure in 0..data.lures.len() {
        for line in 0..data.lines.len() {
            rows.push(evaluate(
                save, account, character, lure, line, location, hours,
            ));
        }
    }
    rows.sort_by(|a, b| {
        objective
            .metric(&b.outcome)
            .total_cmp(&objective.metric(&a.outcome))
            .then_with(|| b.outcome.exp.total_cmp(&a.outcome.exp))
            .then_with(|| b.outcome.fish_total.total_cmp(&a.outcome.fish_total))
    });
    rows
}

/// How many rows share the best metric: 1 when the top row wins outright.
///
/// The split and the fish counts are floored, so combinations with the same
/// depth and speed totals often round to the same score; the ranking then orders
/// them by EXP, then fish, then the upstream lure and line order.
pub fn ties(rows: &[Evaluation], objective: Objective) -> usize {
    let Some(best) = rows.first().map(|row| objective.metric(&row.outcome)) else {
        return 0;
    };
    rows.iter()
        .filter(|row| objective.metric(&row.outcome) == best)
        .count()
}
