//! Averages the trials into per-row means, deltas against the baseline and the
//! best option each baseline carries.
//!
//! Ports `summarizeKillroy` from `js/stats/systems/common/killroy-planner.js`.
//! Every row averages the same seeds, so an average is a mean per attempt, and a
//! `+1` row's delta is the difference against its own baseline.

use super::planner::{Plan, Row};
use super::trial::{Record, trial};

/// The trial seeds `trials` attempts derive from a base seed, matching the
/// upstream `((first + i - 1) % 0xffffffff) + 1`.
pub fn trial_seeds(base: u64, trials: usize) -> Vec<u64> {
    let first = base.max(1);
    (0..trials.max(1) as u64)
        .map(|index| (first.saturating_add(index) - 1) % 0xffff_ffff + 1)
        .collect()
}

/// One row's average gains per attempt.
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Means {
    pub kills: f64,
    pub skulls_picked: f64,
    pub talent_points: f64,
    pub dungeon_credits: f64,
    pub pearls: f64,
}

/// A row's difference against its baseline.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Delta {
    pub kills: f64,
    pub skulls_picked: f64,
}

/// One row after its trials ran.
#[derive(Clone, Debug, PartialEq)]
pub struct RowSummary {
    pub row: Row,
    pub runs: usize,
    pub complete: bool,
    pub totals: Means,
    pub mean: Option<Means>,
    pub delta: Option<Delta>,
    /// The best comparable option for this row's baseline, by collected skulls.
    pub best: bool,
}

impl RowSummary {
    /// Whether the row compares an upgrade.
    pub fn comparing(&self) -> bool {
        self.row.upgrade.is_some()
    }
}

/// Run every attempt of `plan`, reporting how far the run has come.
///
/// `report` takes the finished attempts and the total; returning `true` stops
/// the run and leaves no summary.
pub fn run(plan: &Plan, mut report: impl FnMut(usize, usize) -> bool) -> Option<Vec<RowSummary>> {
    let mut records = Vec::with_capacity(plan.attempts());
    for (index, job) in plan.jobs.iter().enumerate() {
        let row = plan
            .rows
            .iter()
            .find(|row| row.key == job.key)
            .expect("every job belongs to a row");
        records.push(trial(&row.trial, job.seed));
        if report(index + 1, plan.attempts()) {
            return None;
        }
    }
    Some(summarize(plan, &records))
}

/// Average the records into per-row means, deltas and the best option.
///
/// `records` are in `plan.jobs` order; every job must have exactly one record.
pub fn summarize(plan: &Plan, records: &[Record]) -> Vec<RowSummary> {
    let mut rows: Vec<RowSummary> = plan
        .rows
        .iter()
        .map(|row| RowSummary {
            row: row.clone(),
            runs: 0,
            complete: false,
            totals: Means::default(),
            mean: None,
            delta: None,
            best: false,
        })
        .collect();
    for (job, record) in plan.jobs.iter().zip(records) {
        let summary = rows
            .iter_mut()
            .find(|row| row.row.key == job.key)
            .expect("every record belongs to a row");
        summary.runs += 1;
        summary.totals.kills += record.kills as f64;
        summary.totals.skulls_picked += record.skulls_picked as f64;
        summary.totals.talent_points += record.talent_points as f64;
        summary.totals.dungeon_credits += record.dungeon_credits as f64;
        summary.totals.pearls += record.pearls as f64;
    }
    for summary in &mut rows {
        summary.complete = summary.runs == plan.seeds.len() && summary.runs > 0;
        summary.mean = (summary.runs > 0).then(|| Means {
            kills: summary.totals.kills / summary.runs as f64,
            skulls_picked: summary.totals.skulls_picked / summary.runs as f64,
            talent_points: summary.totals.talent_points / summary.runs as f64,
            dungeon_credits: summary.totals.dungeon_credits / summary.runs as f64,
            pearls: summary.totals.pearls / summary.runs as f64,
        });
    }
    for index in 0..rows.len() {
        let Some(mean) = rows[index].mean else {
            continue;
        };
        let baseline_key = rows[index].row.baseline.clone();
        if rows[index].row.upgrade.is_none() {
            continue;
        }
        let Some(base) = rows
            .iter()
            .find(|row| row.row.key == baseline_key)
            .and_then(|row| row.mean)
        else {
            continue;
        };
        rows[index].delta = Some(Delta {
            kills: mean.kills - base.kills,
            skulls_picked: mean.skulls_picked - base.skulls_picked,
        });
    }
    // The best comparable option per baseline: the highest collected skulls,
    // then kills, then the lower upgrade id so a tie stays deterministic.
    let mut best: Vec<(usize, String)> = Vec::new();
    for (index, row) in rows.iter().enumerate() {
        if !row.comparing() || row.mean.is_none() || !row.complete {
            continue;
        }
        match best
            .iter()
            .find(|(_, baseline)| *baseline == row.row.baseline)
        {
            None => best.push((index, row.row.baseline.clone())),
            Some((current, _)) => {
                let other = &rows[*current];
                let (Some(a), Some(b)) = (row.mean, other.mean) else {
                    continue;
                };
                let better = (a.skulls_picked, a.kills) > (b.skulls_picked, b.kills);
                if better {
                    best.retain(|(_, baseline)| *baseline != row.row.baseline);
                    best.push((index, row.row.baseline.clone()));
                }
            }
        }
    }
    for (index, _) in best {
        rows[index].best = true;
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::killroy::{Comparisons, PlanInput, Selection};
    use crate::save::import;
    use serde_json::json;

    fn save() -> crate::save::Save {
        let path = format!("{}/../../fixtures/data.json", env!("CARGO_MANIFEST_DIR"));
        let save: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(path).expect("the fixture is committed"))
                .expect("fixture JSON");
        import(&json!({ "data": save, "serverVars": { "KillroySwap": 7 } }).to_string())
            .expect("the wrapped fixture imports")
            .save
    }

    fn planned(seeds: Vec<u64>) -> Plan {
        crate::killroy::plan(
            &save(),
            &PlanInput {
                selections: vec![Selection {
                    week_offset: 0,
                    ci: 0,
                    slot: 1,
                }],
                seeds,
                comparisons: Comparisons::All,
                kills_per_second: 3.0,
                keep_buffs: false,
                unlimited_inventory: false,
            },
        )
        .expect("the plan builds")
    }

    #[test]
    fn the_same_seeds_produce_the_same_summary() {
        let plan = planned(vec![11, 12, 13]);
        let first = run(&plan, |_, _| false).expect("the run finishes");
        let second = run(&plan, |_, _| false).expect("the run finishes");
        assert_eq!(first, second);
        assert!(first.iter().all(|row| row.complete));
        assert!(first.iter().all(|row| row.runs == 3));
    }

    #[test]
    fn a_cancelled_run_stores_nothing() {
        let plan = planned(vec![11, 12, 13]);
        assert!(run(&plan, |_, _| true).is_none());
    }

    #[test]
    fn every_baseline_highlights_one_best_option() {
        let plan = planned(vec![11, 12, 13]);
        let rows = run(&plan, |_, _| false).expect("the run finishes");
        let baselines: Vec<&str> = rows
            .iter()
            .filter(|row| !row.comparing())
            .map(|row| row.row.key.as_str())
            .collect();
        for baseline in baselines {
            let options: Vec<&RowSummary> = rows
                .iter()
                .filter(|row| row.comparing() && row.row.baseline == baseline)
                .collect();
            let best: Vec<&RowSummary> = options.iter().copied().filter(|row| row.best).collect();
            assert_eq!(best.len(), 1, "one best option per baseline");
            let top = best[0].mean.expect("a finished run has a mean");
            for option in options {
                let mean = option.mean.expect("a finished run has a mean");
                assert!(
                    (top.skulls_picked, top.kills) >= (mean.skulls_picked, mean.kills),
                    "{} beats the best option",
                    option.row.key
                );
            }
        }
    }

    #[test]
    fn a_delta_is_the_difference_against_its_baseline() {
        let plan = planned(vec![21, 22]);
        let rows = run(&plan, |_, _| false).expect("the run finishes");
        for row in rows.iter().filter(|row| row.comparing()) {
            let base = rows
                .iter()
                .find(|other| other.row.key == row.row.baseline)
                .expect("the baseline row exists");
            let (Some(mean), Some(base), Some(delta)) = (row.mean, base.mean, row.delta) else {
                panic!("a finished comparison has a delta");
            };
            assert!((mean.kills - base.kills - delta.kills).abs() < 1e-9);
            assert!((mean.skulls_picked - base.skulls_picked - delta.skulls_picked).abs() < 1e-9);
        }
    }

    #[test]
    fn trial_seeds_step_from_the_base_and_stay_in_range() {
        assert_eq!(trial_seeds(1, 3), vec![1, 2, 3]);
        assert_eq!(
            trial_seeds(0xffff_fffe, 3),
            vec![0xffff_fffe, 0xffff_ffff, 1]
        );
        assert_eq!(trial_seeds(7, 20).len(), 20);
    }
}
