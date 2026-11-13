//! What one estimate means for the next obstructions, the daily cycle and the
//! cell levels.
//!
//! Ports `jellyOperationInsights` and `clearProbabilityProxy` from
//! `js/stats/systems/w7/jelly-operator.js`. The page also accepts a board to
//! project the outlook with the deterministic surrogate; this port always reads
//! the surrogate, so the outlook rows match the page's default.

use super::estimate::Estimate;
use super::surrogate::{Objective, Surrogate, SurrogateOptions};
use super::{Jelly, boss_hp, boss_time};

/// One cell type's EXP progress per attempt.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ExpOutlook {
    pub cell_type: usize,
    pub name: String,
    pub current_level: u32,
    pub current_exp: f64,
    pub requirement: f64,
    pub remaining_exp: f64,
    pub mean_exp_per_attempt: f64,
    pub attempts_to_next_level: Option<f64>,
    pub days_to_next_level: Option<f64>,
}

/// One obstruction row of the outlook.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ObstructionOutlook {
    pub obstruction: usize,
    pub hp: f64,
    pub time: f64,
    pub estimated_clear_probability: Option<f64>,
    pub damage_ratio: Option<f64>,
    pub expected_attempts: Option<f64>,
    pub clear_within_daily_attempts: Option<f64>,
}

/// `jellyOperationInsights`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Insights {
    pub valid: bool,
    pub trials: u32,
    pub clear_probability: f64,
    pub daily_attempts: f64,
    pub expected_attempts_to_clear: Option<f64>,
    pub probability_within_daily_attempts: f64,
    pub expected_days_to_clear: Option<f64>,
    pub expected_daily_cycles_to_clear: Option<f64>,
    pub expected_bloodcells_per_attempt: f64,
    pub saved_daily_passive_bloodcells: f64,
    pub expected_bloodcells_per_daily_cycle: f64,
    pub expected_net_attempt_delta: f64,
    pub expected_net_attempt_delta_per_daily_cycle: f64,
    pub normal_damage: f64,
    pub critical_damage: f64,
    pub normal_damage_share: Option<f64>,
    pub critical_damage_share: Option<f64>,
    pub exp_rows: Vec<ExpOutlook>,
    pub obstruction_outlook: Vec<ObstructionOutlook>,
    /// How the outlook rows were projected; the page names the method too.
    pub method: &'static str,
}

/// `clearProbabilityProxy`: the bounded probability a damage ratio buys.
pub fn clear_probability_proxy(damage_ratio: f64) -> f64 {
    let ratio = damage_ratio.max(0.0);
    if ratio < 1.0 {
        (0.5 * ratio * ratio).min(0.5)
    } else {
        (0.5 + 0.5 * (1.0 - (-(ratio - 1.0)).exp())).min(0.999)
    }
}

impl Insights {
    /// The estimate's meaning for the save and the board on screen.
    pub fn of(jelly: &Jelly, layout: &super::board::Layout, estimate: &Estimate) -> Insights {
        let valid = estimate.valid;
        let clear_probability = if valid {
            estimate.clear_probability.clamp(0.0, 1.0)
        } else {
            0.0
        };
        let daily_attempts = jelly.daily_tries().max(0.0).floor();
        let expected_attempts_to_clear = (clear_probability > 0.0).then(|| 1.0 / clear_probability);
        let probability_within_daily_attempts = if daily_attempts > 0.0 {
            1.0 - (1.0 - clear_probability).powf(daily_attempts)
        } else {
            0.0
        };
        let expected_days_to_clear = expected_attempts_to_clear
            .filter(|_| daily_attempts > 0.0)
            .map(|attempts| attempts / daily_attempts);
        let expected_daily_cycles_to_clear = (probability_within_daily_attempts > 0.0)
            .then(|| 1.0 / probability_within_daily_attempts);
        let saved_daily_passive_bloodcells = jelly.daily_bloodcells();
        let expected_bloodcells_per_attempt = if valid { estimate.mean_bloodcells } else { 0.0 };
        let expected_bloodcells_per_daily_cycle =
            expected_bloodcells_per_attempt * daily_attempts + saved_daily_passive_bloodcells;
        let expected_net_attempt_delta = if valid {
            estimate.mean_attempt_delta
        } else {
            0.0
        };
        let normal_damage = if valid {
            estimate.mean_normal_damage
        } else {
            0.0
        };
        let critical_damage = if valid {
            estimate.mean_critical_damage
        } else {
            0.0
        };
        let total = normal_damage + critical_damage;
        let mut exp_rows = Vec::new();
        for (cell_type, &mean_exp) in estimate
            .mean_cell_exp_by_type
            .iter()
            .enumerate()
            .take(jelly.units_owned())
        {
            let current_level = jelly.cell_level(cell_type);
            let current_exp = jelly.cell_exp(cell_type);
            let requirement = jelly.cell_exp_requirement(cell_type);
            let remaining_exp = (requirement - current_exp).max(0.0);
            let mean_exp_per_attempt = if valid { mean_exp } else { 0.0 };
            let attempts_to_next_level = if remaining_exp <= 0.0 {
                Some(0.0)
            } else if mean_exp_per_attempt > 0.0 {
                Some(remaining_exp / mean_exp_per_attempt)
            } else {
                None
            };
            let days_to_next_level = attempts_to_next_level
                .filter(|_| daily_attempts > 0.0)
                .map(|attempts| attempts / daily_attempts);
            exp_rows.push(ExpOutlook {
                cell_type,
                name: super::data().cell_name(cell_type).to_string(),
                current_level,
                current_exp,
                requirement,
                remaining_exp,
                mean_exp_per_attempt,
                attempts_to_next_level,
                days_to_next_level,
            });
        }

        let current_obstruction = estimate.representative.obstruction;
        let source_time = estimate.representative.standard_time.max(1e-12);
        let source_damage = if valid { estimate.mean_damage } else { 0.0 };
        let count = (72usize).saturating_sub(current_obstruction).min(6);
        let mut obstruction_outlook = Vec::with_capacity(count);
        for offset in 0..count {
            let obstruction = current_obstruction + offset;
            let hp = boss_hp(obstruction);
            let time = boss_time(obstruction);
            let surrogate = Surrogate::project(
                jelly,
                layout,
                SurrogateOptions {
                    obstruction: Some(obstruction),
                    fever: Some(estimate.representative.fever),
                    objective: Objective::Clear,
                    ..SurrogateOptions::default()
                },
            );
            let damage_ratio = if surrogate.valid {
                surrogate.damage_ratio
            } else {
                let estimated_damage = source_damage * time / source_time;
                if hp > 0.0 { estimated_damage / hp } else { 0.0 }
            };
            let estimated_clear_probability = if offset == 0 && valid {
                clear_probability
            } else {
                clear_probability_proxy(damage_ratio)
            };
            let expected_attempts =
                (estimated_clear_probability > 0.0).then(|| 1.0 / estimated_clear_probability);
            let clear_within_daily_attempts = if daily_attempts > 0.0 {
                1.0 - (1.0 - estimated_clear_probability).powf(daily_attempts)
            } else {
                0.0
            };
            obstruction_outlook.push(ObstructionOutlook {
                obstruction,
                hp,
                time,
                estimated_clear_probability: Some(estimated_clear_probability),
                damage_ratio: Some(damage_ratio),
                expected_attempts,
                clear_within_daily_attempts: Some(clear_within_daily_attempts),
            });
        }
        Insights {
            valid,
            trials: estimate.trials,
            clear_probability,
            daily_attempts,
            expected_attempts_to_clear,
            probability_within_daily_attempts,
            expected_days_to_clear,
            expected_daily_cycles_to_clear,
            expected_bloodcells_per_attempt,
            saved_daily_passive_bloodcells,
            expected_bloodcells_per_daily_cycle,
            expected_net_attempt_delta,
            expected_net_attempt_delta_per_daily_cycle: expected_net_attempt_delta * daily_attempts,
            normal_damage,
            critical_damage,
            normal_damage_share: (total > 0.0).then(|| normal_damage / total),
            critical_damage_share: (total > 0.0).then(|| critical_damage / total),
            exp_rows,
            obstruction_outlook,
            method: "layout-surrogate",
        }
    }
}
