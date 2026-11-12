//! The strategy optimizer: a bounded, seeded search over build plans.
//!
//! Ported from `optimizeWorshipTd` and `_searchTdLayouts` (free mode) in
//! `js/stats/systems/w3/worship-td.js`. Every candidate is simulated by the same
//! engine the Plan pane runs, candidates are deduplicated by their exact plan,
//! and a diverse beam refines several competing families instead of one winner.
//! The finalists, the coverage heuristic and every guide preset are then
//! re-simulated on shared fresh seeds, so a baseline cannot be dropped because
//! it screened badly. Plans are edited as single towers, same-type groups,
//! whole types and sell-and-rebuild phases.
//!
//! The hold-template layout search (`optimizeWorshipTdLayout`) and the
//! souls-per-hour objective are neutral this build: [`crate::ASSUMPTIONS`]
//! names them.

mod edits;
mod plans;
mod search;

pub use search::optimize;

use super::{Inputs, Plan, Run, RunResult};

/// The optimizer's search budget.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// A short screen: enough to beat the baseline on small trials.
    Quick,
    /// The default: several families refined, then a shared final comparison.
    #[default]
    Standard,
    /// More refinement per family, for a slower, better plan.
    Deep,
}

impl Mode {
    /// Every mode, in pane order.
    pub const ALL: [Mode; 3] = [Mode::Quick, Mode::Standard, Mode::Deep];

    /// The stable id the pane stores.
    pub fn id(self) -> &'static str {
        match self {
            Mode::Quick => "quick",
            Mode::Standard => "standard",
            Mode::Deep => "deep",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Mode::Quick => "Quick",
            Mode::Standard => "Standard",
            Mode::Deep => "Deep",
        }
    }

    /// The mode for a stored id.
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|mode| mode.id() == id)
    }

    /// How many beam families are refined.
    pub fn restarts(self) -> usize {
        match self {
            Mode::Quick => 2,
            Mode::Standard | Mode::Deep => 4,
        }
    }

    /// How many refinements each family gets.
    pub fn iterations(self) -> usize {
        match self {
            Mode::Quick => 6,
            Mode::Standard => 12,
            Mode::Deep => 32,
        }
    }
}

/// What one optimizer run searches with.
#[derive(Clone, Debug, PartialEq)]
pub struct Options {
    pub trial: usize,
    pub mode: Mode,
    /// Screening runs per candidate.
    pub seeds: usize,
    /// The seed the whole search derives from; two runs with it agree.
    pub base_seed: u64,
    /// Stop a simulated run once it starts this wave.
    pub max_wave: u32,
    /// Simulated seconds per run; the game's save limit is four hours.
    pub max_time_sec: f64,
    /// Towers a plan may build across every stage.
    pub max_towers: usize,
    /// An extra plan to compare the search against, usually the one on screen.
    pub seed_plan: Option<Plan>,
    /// Screening evaluations; `None` takes the mode's budget.
    pub budget: Option<usize>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            trial: 0,
            mode: Mode::Standard,
            seeds: 2,
            base_seed: 12_345,
            max_wave: 240,
            max_time_sec: 4.0 * 3600.0,
            max_towers: 30,
            seed_plan: None,
            budget: None,
        }
    }
}

impl Options {
    /// The screening budget this run spends.
    pub fn budget(&self) -> usize {
        self.budget
            .unwrap_or_else(|| self.mode.restarts() * (self.mode.iterations() + 1))
    }

    /// The seeds every candidate is screened on.
    pub(crate) fn screening_seeds(&self) -> Vec<u64> {
        (0..self.seeds.max(1))
            .map(|index| self.base_seed.wrapping_add(index as u64 * 7919))
            .collect()
    }

    /// The fresh seeds the finalists and baselines are compared on: twice the
    /// screening count, so a plan cannot win on screening luck.
    pub(crate) fn final_seeds(&self) -> Vec<u64> {
        (0..(self.seeds.max(1) * 2).max(2))
            .map(|index| self.base_seed.wrapping_add(1_000_003 + index as u64 * 7919))
            .collect()
    }
}

/// Every run of one plan on one seed list.
#[derive(Clone, Debug, PartialEq)]
pub struct Trials {
    pub runs: Vec<RunResult>,
    pub mean_best_wave: f64,
    pub min_best_wave: u32,
    pub max_best_wave: u32,
    pub mean_highest_wave: f64,
}

/// Simulate one plan on every seed through the plan engine.
pub fn simulate_trials(inputs: &Inputs, plan: &Plan, options: &Options, seeds: &[u64]) -> Trials {
    let mut runs = Vec::with_capacity(seeds.len());
    for seed in seeds {
        runs.push(simulate(inputs, plan, options, *seed));
    }
    let count = runs.len().max(1) as f64;
    let mean = |value: fn(&RunResult) -> f64| runs.iter().map(value).sum::<f64>() / count;
    Trials {
        mean_best_wave: mean(|run| f64::from(run.best_wave)),
        min_best_wave: runs.iter().map(|run| run.best_wave).min().unwrap_or(0),
        max_best_wave: runs.iter().map(|run| run.best_wave).max().unwrap_or(0),
        mean_highest_wave: mean(|run| f64::from(run.highest_wave)),
        runs,
    }
}

/// Simulate one plan on one seed to the end.
pub(super) fn simulate(inputs: &Inputs, plan: &Plan, options: &Options, seed: u64) -> RunResult {
    let mut run = Run::with_plan(inputs.clone(), options.trial, seed, plan.clone())
        .with_limits(options.max_wave, options.max_time_sec);
    while !run.done() {
        run.step(1_000);
    }
    run.result()
}

/// What candidates are ranked on: cleared wave first, then consistency, how far
/// into the failing waves the runs got, and the time they took.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Score {
    /// Cleared waves, the ranking's first key.
    pub primary: f64,
    /// The mean cleared wave.
    pub wave: f64,
    /// The worst run's cleared wave.
    pub min_wave: f64,
    /// The mean highest wave reached, cleared or not.
    pub reached: f64,
    /// The mean share of the failing wave's HP still standing.
    pub end_frac: f64,
    /// The mean seconds to clear the best wave.
    pub time: f64,
}

impl Score {
    pub(super) fn of(trials: &Trials) -> Self {
        let count = trials.runs.len().max(1) as f64;
        let end_frac = trials
            .runs
            .iter()
            .map(|run| run.end_pending_frac)
            .sum::<f64>()
            / count;
        let time = trials
            .runs
            .iter()
            .map(|run| run.best_wave_time_sec.unwrap_or(0.0))
            .sum::<f64>()
            / count;
        Self {
            primary: trials.mean_best_wave,
            wave: trials.mean_best_wave,
            min_wave: f64::from(trials.min_best_wave),
            reached: trials.mean_highest_wave,
            end_frac,
            time,
        }
    }
}

/// Whether `a` ranks above `b`. The game records a best wave only on an empty
/// board, so the reached wave breaks ties between plans clearing the same wave.
pub(super) fn better(a: &Score, b: &Score) -> bool {
    if a.primary != b.primary {
        return a.primary > b.primary;
    }
    if a.min_wave != b.min_wave {
        return a.min_wave > b.min_wave;
    }
    if a.reached != b.reached {
        return a.reached > b.reached;
    }
    if a.end_frac != b.end_frac {
        return a.end_frac < b.end_frac;
    }
    a.time < b.time
}

/// One plan in a final or baseline comparison.
#[derive(Clone, Debug, PartialEq)]
pub struct Finalist {
    pub label: String,
    pub score: Score,
}

/// A finished search: the recommended plan and what it was compared against.
#[derive(Clone, Debug, PartialEq)]
pub struct Search {
    pub trial: usize,
    /// What the winner was: `seed`, `heuristic`, `preset`, `refined` or `random`.
    pub kind: &'static str,
    pub label: String,
    /// The recommended plan, legalized against the planning checks.
    pub plan: Plan,
    /// The winner's score on the shared final seeds.
    pub score: Score,
    /// The winner's first final run: its final towers and phase log.
    pub detail: RunResult,
    pub mean_best_wave: f64,
    pub min_best_wave: u32,
    pub max_best_wave: u32,
    /// The best wave the save already cleared on this trial.
    pub saved_best: u32,
    /// Screening plus final evaluations.
    pub evaluations: usize,
    /// The shared fresh seeds the winner and the baselines were compared on.
    pub seeds: Vec<u64>,
    pub finalists: Vec<Finalist>,
    /// The heuristic, the guide presets and the plan on screen, scored on the
    /// same final seeds the winner was.
    pub baselines: Vec<Finalist>,
    pub duplicates_skipped: usize,
    pub families: usize,
}

impl Search {
    /// The best baseline, if the search compared any.
    pub fn best_baseline(&self) -> Option<&Finalist> {
        let mut best: Option<&Finalist> = None;
        for baseline in &self.baselines {
            if best.is_none_or(|current| better(&baseline.score, &current.score)) {
                best = Some(baseline);
            }
        }
        best
    }

    /// Whether the recommended plan clears at least the best baseline's wave.
    pub fn beats_baseline(&self) -> bool {
        self.best_baseline()
            .is_none_or(|baseline| self.score.primary >= baseline.score.primary)
    }
}

/// What a run reported, or why it stopped.
#[derive(Clone, Debug, PartialEq)]
pub enum Outcome {
    /// The search finished with a recommended plan.
    Found(Box<Search>),
    /// The reporter asked the run to stop.
    Cancelled,
    /// The search had nothing to search: no unlocked type or legal slot.
    Refused(&'static str),
}

#[derive(Clone)]
pub(super) struct Candidate {
    pub(super) kind: &'static str,
    pub(super) label: String,
    pub(super) family: String,
    pub(super) plan: Plan,
    pub(super) agg: Trials,
    pub(super) score: Score,
    /// The score on the shared final seeds, once compared.
    pub(super) final_score: Option<Score>,
}

impl Candidate {
    pub(super) fn new(kind: &'static str, label: String, family: String, plan: Plan) -> Self {
        Self {
            kind,
            label,
            family,
            plan,
            agg: Trials {
                runs: Vec::new(),
                mean_best_wave: 0.0,
                min_best_wave: 0,
                max_best_wave: 0,
                mean_highest_wave: 0.0,
            },
            score: Score::default(),
            final_score: None,
        }
    }
}

/// One family's unplayed neighbors, consumed two at a time.
pub(super) struct Neighborhood {
    pub(super) list: Vec<Candidate>,
    pub(super) at: usize,
}
