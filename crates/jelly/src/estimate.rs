//! The operation estimate: the seeded Monte Carlo the overview shows.
//!
//! Ports `jellyOperationEstimate`, `simulateJellyTrials` and
//! `summarizeJellyTrials` from `js/stats/systems/w7/jelly-operator.js`. One
//! trial is [`super::operation::simulate`]; the aggregate keeps the upstream's
//! statistics, including its Wilson interval for the clear probability.
//!
//! The upstream batched, cached and resumable trial runners are search
//! optimizations for its optimizer; this port runs the trials directly, so the
//! numbers are the same and a run stays cancellable at a trial boundary.

use super::Jelly;
use super::board::Layout;
use super::operation::{Contribution, OperationOptions, Trial, simulate};

/// The Monte Carlo aggregate one estimate shows.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Estimate {
    pub valid: bool,
    pub reason: String,
    pub trials: u32,
    pub seed: u32,
    pub success_count: u32,
    pub clear_probability: f64,
    pub clear_probability_low: f64,
    pub clear_probability_high: f64,
    pub mean_time: f64,
    pub median_time: Option<f64>,
    pub p90_time: Option<f64>,
    pub mean_damage: f64,
    pub mean_damage_standard_error: f64,
    pub median_damage: Option<f64>,
    pub mean_bloodcells: f64,
    pub mean_bloodcells_standard_error: f64,
    pub median_bloodcells: Option<f64>,
    pub p90_bloodcells: Option<f64>,
    pub max_bloodcells: f64,
    pub mean_cell_exp: f64,
    pub mean_cell_exp_by_type: [f64; 9],
    pub mean_critical_time: f64,
    pub mean_deaths: f64,
    pub mean_revives: f64,
    pub mean_best_dps: f64,
    pub mean_best_dps_multiplier: f64,
    pub mean_peak_dps: f64,
    pub mean_normal_damage: f64,
    pub mean_critical_damage: f64,
    pub mean_shots: f64,
    pub mean_hits: f64,
    pub mean_boss_attacks: f64,
    pub mean_immunoid_focus_time: f64,
    pub mean_live_slots: f64,
    pub mean_attempt_delta: f64,
    pub mean_amoeba_stacks_at_critical_start: f64,
    pub mean_amoeba_stacks_at_critical_end: f64,
    pub mean_contribution_by_type: Vec<Contribution>,
    pub min_damage: f64,
    pub max_damage: f64,
    /// The first trial, which the page's contribution table and timeline show.
    pub representative: Trial,
}

impl Estimate {
    /// `jellyOperationEstimate`: `trials` seeded attempts and their aggregate.
    pub fn run(jelly: &Jelly, layout: &Layout, trials: u32, seed: u32) -> Estimate {
        Self::run_with(jelly, layout, trials, seed, |_done| true)
            .expect("a run that never cancels finishes")
    }

    /// The same run, reporting after each finished trial. The callback returns
    /// whether to keep going; a stopped run reports `None`.
    pub fn run_with(
        jelly: &Jelly,
        layout: &Layout,
        trials: u32,
        seed: u32,
        mut report: impl FnMut(u32) -> bool,
    ) -> Option<Estimate> {
        let trials = trials.clamp(1, 8192);
        let mut results = Vec::with_capacity(trials as usize);
        for index in 0..trials {
            let trial_seed = seed.wrapping_add(index.wrapping_mul(2_654_435_761));
            results.push(simulate(
                jelly,
                layout,
                OperationOptions {
                    seed: trial_seed,
                    ..OperationOptions::default()
                },
            ));
            if !report(index + 1) {
                return None;
            }
        }
        Some(summarize(&results, seed))
    }
}

/// `summarizeJellyTrials`.
pub fn summarize(results: &[Trial], seed: u32) -> Estimate {
    let trials = results.len() as u32;
    if let Some(invalid) = results.iter().find(|trial| !trial.valid) {
        return Estimate {
            valid: false,
            reason: if invalid.reason.is_empty() {
                "Invalid layout".to_string()
            } else {
                invalid.reason.clone()
            },
            trials,
            seed,
            ..Estimate::default()
        };
    }
    let successes: Vec<&Trial> = results.iter().filter(|trial| trial.success).collect();
    let times: Vec<f64> = successes.iter().map(|trial| trial.time).collect();
    let numbers = |pick: &dyn Fn(&Trial) -> f64| -> Vec<f64> { results.iter().map(pick).collect() };
    let damages = numbers(&|trial| trial.damage);
    let bloodcells = numbers(&|trial| trial.bloodcells);
    let cell_exp = numbers(&|trial| trial.cell_exp);
    let critical_times = numbers(&|trial| trial.critical_time);
    let deaths = numbers(&|trial| trial.deaths);
    let revives = numbers(&|trial| trial.revives_used);
    let best_dps = numbers(&|trial| trial.best_dps_after);
    let peak_dps = numbers(&|trial| trial.peak_dps);
    let normal_damage = numbers(&|trial| trial.damage_at_critical_start);
    let critical_damage =
        numbers(&|trial| (trial.damage - trial.damage_at_critical_start).max(0.0));
    let shots = numbers(&|trial| trial.shots);
    let hits = numbers(&|trial| trial.hits);
    let boss_attacks = numbers(&|trial| trial.boss_attacks);
    let immunoid_focus = numbers(&|trial| trial.immunoid_focus_time);
    let live_slots = numbers(&|trial| trial.live_slots);
    let attempt_delta = numbers(&|trial| trial.attempt_delta);
    let amoeba_start = numbers(&|trial| trial.amoeba_stacks_at_critical_start);
    let amoeba_end = numbers(&|trial| trial.amoeba_stacks);
    let cell_exp_by_type: Vec<[f64; 9]> = results.iter().map(|trial| trial.exp_by_type).collect();
    let contribution = (0..9)
        .map(|cell_type| Contribution {
            cell_type,
            name: super::data().cell_name(cell_type).to_string(),
            shots: mean(&numbers(&|trial| trial.contribution[cell_type].shots)),
            hits: mean(&numbers(&|trial| trial.contribution[cell_type].hits)),
            damage: mean(&numbers(&|trial| trial.contribution[cell_type].damage)),
            bloodcells: mean(&numbers(&|trial| trial.contribution[cell_type].bloodcells)),
            exp: mean(&numbers(&|trial| trial.contribution[cell_type].exp)),
        })
        .collect();
    let interval = wilson(successes.len() as f64, f64::from(trials));
    Estimate {
        valid: true,
        reason: String::new(),
        trials,
        seed,
        success_count: successes.len() as u32,
        clear_probability: successes.len() as f64 / f64::from(trials),
        clear_probability_low: interval.0,
        clear_probability_high: interval.1,
        mean_time: mean(&times),
        median_time: quantile(&times, 0.5),
        p90_time: quantile(&times, 0.9),
        mean_damage: mean(&damages),
        mean_damage_standard_error: standard_error(&damages),
        median_damage: quantile(&damages, 0.5),
        mean_bloodcells: mean(&bloodcells),
        mean_bloodcells_standard_error: standard_error(&bloodcells),
        median_bloodcells: quantile(&bloodcells, 0.5),
        p90_bloodcells: quantile(&bloodcells, 0.9),
        max_bloodcells: bloodcells.iter().copied().fold(f64::MIN, f64::max),
        mean_cell_exp: mean(&cell_exp),
        mean_cell_exp_by_type: std::array::from_fn(|cell_type| {
            mean(
                &cell_exp_by_type
                    .iter()
                    .map(|row| row[cell_type])
                    .collect::<Vec<_>>(),
            )
        }),
        mean_critical_time: mean(&critical_times),
        mean_deaths: mean(&deaths),
        mean_revives: mean(&revives),
        mean_best_dps: mean(&best_dps),
        mean_best_dps_multiplier: mean(
            &best_dps
                .iter()
                .map(|dps| super::metrics::best_dps_multiplier_from_value(*dps))
                .collect::<Vec<_>>(),
        ),
        mean_peak_dps: mean(&peak_dps),
        mean_normal_damage: mean(&normal_damage),
        mean_critical_damage: mean(&critical_damage),
        mean_shots: mean(&shots),
        mean_hits: mean(&hits),
        mean_boss_attacks: mean(&boss_attacks),
        mean_immunoid_focus_time: mean(&immunoid_focus),
        mean_live_slots: mean(&live_slots),
        mean_attempt_delta: mean(&attempt_delta),
        mean_amoeba_stacks_at_critical_start: mean(&amoeba_start),
        mean_amoeba_stacks_at_critical_end: mean(&amoeba_end),
        mean_contribution_by_type: contribution,
        min_damage: damages.iter().copied().fold(f64::MAX, f64::min),
        max_damage: damages.iter().copied().fold(f64::MIN, f64::max),
        representative: results[0].clone(),
    }
}

/// The arithmetic mean, zero for an empty list.
pub fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        0.0
    } else {
        values.iter().sum::<f64>() / values.len() as f64
    }
}

/// `quantile`: the linearly interpolated position.
pub fn quantile(values: &[f64], fraction: f64) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let position = (sorted.len() - 1) as f64 * fraction;
    let lower = position.floor() as usize;
    let upper = position.ceil() as usize;
    if lower == upper {
        return Some(sorted[lower]);
    }
    Some(sorted[lower] + (sorted[upper] - sorted[lower]) * (position - lower as f64))
}

/// `standardError`.
pub fn standard_error(values: &[f64]) -> f64 {
    if values.len() < 2 {
        return 0.0;
    }
    let average = mean(values);
    let variance = values
        .iter()
        .map(|value| (value - average).powi(2))
        .sum::<f64>()
        / (values.len() - 1) as f64;
    (variance / values.len() as f64).sqrt()
}

/// `wilsonInterval`: the 95% Wilson score interval.
pub fn wilson(successes: f64, trials: f64) -> (f64, f64) {
    if trials <= 0.0 {
        return (0.0, 0.0);
    }
    let z = 1.959963984540054_f64;
    let probability = successes / trials;
    let z2 = z * z;
    let denominator = 1.0 + z2 / trials;
    let center = (probability + z2 / (2.0 * trials)) / denominator;
    let margin = z
        * (probability * (1.0 - probability) / trials + z2 / (4.0 * trials * trials)).sqrt()
        / denominator;
    ((center - margin).max(0.0), (center + margin).min(1.0))
}
