//! `optimizeBlueJellyfish`: the Blue Jellyfish donation search behind the NMLB
//! planner.
//!
//! Donating ladles raises the grant every kitchen cooks at each reset, forever;
//! spending them buys meal levels now. The search plans the same horizon under an
//! up-front share of the owned ladles and a share of the guaranteed income over a
//! day window, refines the strongest split, and keeps the baseline when nothing
//! beats it.
//!
//! Upstream ranks the search by the kitchen speed the horizon reaches
//! (`_planObjective`'s default). Kitchen speed is one entered rate in this build
//! and does not follow the meal levels, so the search ranks by the section's own
//! objective instead: the meal levels gained, or the progress toward the meal
//! targets. Every other rule is the upstream search's.

use super::plan::{Inputs, Plan, Policy, Schedule, plan};
use super::{Objective, Reset};
use idleon_common::{Control, Progress};
use std::collections::HashSet;

/// The splits the search refines with, each a fraction of the horizon.
const REFINEMENT_STEPS: [f64; 3] = [0.25, 0.125, 0.0625];

/// One donation split: an up-front share of the owned ladles and a share of the
/// guaranteed income between two resets.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Split {
    pub owned_fraction: f64,
    pub income_fraction: f64,
    /// The first reset the daily share applies to; 0 means the whole horizon,
    /// as upstream leaves `fromDay` undefined.
    pub from_day: u32,
    /// The last reset the daily share applies to.
    pub until_day: u32,
}

/// The search's answer: the best plan it found and the schedule that reached it.
#[derive(Clone, Debug, PartialEq)]
pub struct Donation {
    /// The best plan, or the baseline when no split improved on it.
    pub plan: Plan,
    /// The same horizon with no new donations.
    pub baseline: Plan,
    /// The whole ladles the best split donates.
    pub schedule: Schedule,
    pub split: Split,
    /// Whether the best plan beats the baseline on the objective.
    pub improved: bool,
    /// The objective each plan reached.
    pub score: f64,
    pub baseline_score: f64,
    /// The schedules the search planned.
    pub evaluated: usize,
}

/// `_planObjective`: the value a plan reaches under the section's objective.
pub fn score(plan: &Plan, inputs: &Inputs) -> f64 {
    match inputs.objective {
        Objective::Ranks => plan.gained.iter().sum(),
        Objective::Targets => plan
            .levels
            .iter()
            .enumerate()
            .map(|(meal, level)| {
                let target = inputs.targets.get(meal).copied().unwrap_or(0.0).max(0.0);
                (level.min(target) - plan.start_levels[meal]).max(0.0)
            })
            .sum(),
    }
}

/// `optimizeBlueJellyfish`: compare donation splits against direct spending.
///
/// Returns `None` when the reporter stopped the run or the baseline plan cannot
/// be built, so a cancelled run stores nothing.
pub fn optimize(
    reset: &Reset,
    inputs: &Inputs,
    report: &mut dyn FnMut(Progress) -> Control,
) -> Option<Donation> {
    let base = Inputs {
        policy: Policy::None,
        owned_fraction: 0.0,
        donations: None,
        ..inputs.clone()
    };
    let baseline = plan(reset, &base).ok()?;
    let baseline_score = score(&baseline, inputs);
    let days = baseline.days;
    let mut search = Search {
        reset,
        inputs: &base,
        scoring: inputs,
        baseline_daily: baseline
            .timeline
            .iter()
            .map(|day| day.ladle_income)
            .collect(),
        best: baseline.clone(),
        best_score: baseline_score,
        best_split: Split::default(),
        seen: HashSet::new(),
        evaluated: 1,
    };
    if report(Progress {
        fraction: 0.0,
        message: "Starting donation comparison".into(),
    }) == Control::Stop
    {
        return None;
    }
    let cutoffs = cutoffs(days, inputs.ladle_multi.max(0.0));
    let mut splits = Vec::new();
    if reset.jelly.unlocked {
        for until in &cutoffs {
            for owned in [0.0, 0.5, 1.0] {
                for income in [0.0, 0.5, 1.0] {
                    splits.push(Split {
                        owned_fraction: owned,
                        income_fraction: income,
                        from_day: 0,
                        until_day: *until,
                    });
                }
            }
        }
        for from in &cutoffs {
            for owned in [0.0, 1.0] {
                for income in [0.5, 1.0] {
                    splits.push(Split {
                        owned_fraction: owned,
                        income_fraction: income,
                        from_day: (*from).max(1),
                        until_day: days,
                    });
                }
            }
        }
    }
    let count = splits.len().max(1) as f32;
    for (index, split) in splits.into_iter().enumerate() {
        search.evaluate(split)?;
        if report(Progress {
            fraction: 0.75 * (index + 1) as f32 / count,
            message: format!(
                "Comparing donation schedules · {} evaluated",
                search.evaluated
            ),
        }) == Control::Stop
        {
            return None;
        }
    }
    // Refine the strongest split around itself, one dimension at a time.
    if reset.jelly.unlocked {
        for step in REFINEMENT_STEPS {
            let center = search.best_split;
            let dimensions = match center.from_day > 0 {
                true => [Dimension::Owned, Dimension::Income, Dimension::From],
                false => [Dimension::Owned, Dimension::Income, Dimension::Until],
            };
            for dimension in dimensions {
                for direction in [-1.0, 1.0] {
                    let day = matches!(dimension, Dimension::From | Dimension::Until);
                    let delta = match day {
                        true => direction * (f64::from(days) * step).round().max(1.0),
                        false => direction * step,
                    };
                    search.evaluate(center.shifted(dimension, delta, days))?;
                }
            }
            if report(Progress {
                fraction: (0.75 + (1.0 - step * 4.0) * 0.2) as f32,
                message: format!("Refining the best split · {} evaluated", search.evaluated),
            }) == Control::Stop
            {
                return None;
            }
        }
    }
    report(Progress {
        fraction: 1.0,
        message: "Complete".into(),
    });
    let improved = search.best_score > baseline_score * (1.0 + 1e-12)
        || ((search.best_score - baseline_score).abs() <= baseline_score.abs().max(1.0) * 1e-12
            && search.best.jelly.donated < baseline.jelly.donated);
    let split = match improved {
        true => search.best_split,
        false => Split::default(),
    };
    let schedule = search.schedule(&split);
    let plan = match improved {
        true => search.best,
        false => baseline.clone(),
    };
    Some(Donation {
        schedule,
        score: search.best_score,
        baseline_score,
        improved,
        plan,
        baseline,
        split,
        evaluated: search.evaluated,
    })
}

/// One dimension the refinement moves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Dimension {
    Owned,
    Income,
    From,
    Until,
}

impl Split {
    /// The split one refinement step away, clamped to the search's own bounds.
    fn shifted(self, dimension: Dimension, delta: f64, days: u32) -> Self {
        let mut split = self;
        let day = |value: u32, delta: f64, min: f64, max: f64| {
            (f64::from(value) + delta).clamp(min, max) as u32
        };
        match dimension {
            Dimension::Owned => {
                split.owned_fraction = (self.owned_fraction + delta).clamp(0.0, 1.0);
            }
            Dimension::Income => {
                split.income_fraction = (self.income_fraction + delta).clamp(0.0, 1.0);
            }
            Dimension::From => {
                split.from_day = day(self.from_day, delta, 1.0, f64::from(days));
            }
            Dimension::Until => {
                split.until_day = day(self.until_day, delta, 0.0, f64::from(days));
            }
        }
        split
    }
}

/// The day cutoffs the coarse search stops at, in day order.
fn cutoffs(days: u32, ladle_multi: f64) -> Vec<u32> {
    let last = days.saturating_sub(ladle_multi.ceil().max(0.0) as u32);
    let mut cutoffs = vec![0, 1, days / 4, days / 2, 3 * days / 4, last, days];
    cutoffs.sort_unstable();
    cutoffs.dedup();
    cutoffs
}

/// The search's mutable state.
struct Search<'a> {
    reset: &'a Reset,
    /// The inputs every candidate plans from: no policy, no schedule.
    inputs: &'a Inputs,
    /// The inputs the objective reads its targets from.
    scoring: &'a Inputs,
    /// Each reset's guaranteed income, from the baseline.
    baseline_daily: Vec<f64>,
    best: Plan,
    best_score: f64,
    best_split: Split,
    /// The schedules already planned.
    seen: HashSet<(i64, Vec<i64>)>,
    evaluated: usize,
}

impl Search<'_> {
    /// The whole-ladle schedule a split names.
    fn schedule(&self, split: &Split) -> Schedule {
        Schedule {
            owned: (self.inputs.start_ladles.max(0.0) * split.owned_fraction).floor(),
            daily: self
                .baseline_daily
                .iter()
                .enumerate()
                .map(|(index, income)| {
                    let day = index as u32 + 1;
                    match day >= split.from_day.max(1) && day <= split.until_day {
                        true => (income * split.income_fraction).floor(),
                        false => 0.0,
                    }
                })
                .collect(),
        }
    }

    /// Plan one split and keep it when it beats the best. `None` when the plan
    /// cannot be built, which only the baseline's own inputs can cause.
    fn evaluate(&mut self, split: Split) -> Option<()> {
        let schedule = self.schedule(&split);
        let key = (
            schedule.owned as i64,
            schedule
                .daily
                .iter()
                .map(|day| *day as i64)
                .collect::<Vec<i64>>(),
        );
        if !self.seen.insert(key) {
            return Some(());
        }
        let candidate = plan(
            self.reset,
            &Inputs {
                donations: Some(schedule),
                ..self.inputs.clone()
            },
        )
        .ok()?;
        self.evaluated += 1;
        let score = score(&candidate, self.scoring);
        let better = score > self.best_score * (1.0 + 1e-12)
            || ((score - self.best_score).abs() <= self.best_score.abs().max(1.0) * 1e-12
                && candidate.jelly.donated < self.best.jelly.donated);
        if better {
            self.best = candidate;
            self.best_score = score;
            self.best_split = split;
        }
        Some(())
    }
}
