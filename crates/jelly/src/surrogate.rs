//! The deterministic operation surrogate.
//!
//! Ports `jellyOperationSurrogate` from
//! `js/stats/systems/w7/jelly-operator.js`: a closed-form projection of one
//! operation from the board metrics, the projectile travel time and the boss
//! attack cadence. The overview's obstruction outlook reads it for every row
//! instead of running a Monte Carlo per obstruction.

use super::Jelly;
use super::board::Layout;
use super::data::data;
use super::metrics::{Cadence, MetricOptions, attack_cadence};
use super::operation::{RoidTiming, boss_interval_seconds, projectile_hit_delay};
use super::{boss_hp, boss_time, roid_duration_seconds};

/// Which metric the surrogate scores a board by.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Objective {
    Clear,
    Bloodcells,
    Exp,
    #[default]
    Dps,
    Survival,
}

/// The knobs one surrogate projection reads.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SurrogateOptions {
    pub obstruction: Option<usize>,
    pub fever: Option<u32>,
    pub objective: Objective,
    pub exp_cell_type: Option<usize>,
    pub roid_timing: RoidTiming,
    pub max_critical_seconds: Option<f64>,
    pub travel_scale: Option<f64>,
    pub opening_fraction: Option<f64>,
}

/// `jellyOperationSurrogate`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Surrogate {
    pub valid: bool,
    pub reason: String,
    pub obstruction: usize,
    pub fever: u32,
    pub hp: f64,
    pub normal_seconds: f64,
    pub critical_seconds: f64,
    pub projected_damage: f64,
    pub projected_bloodcells: f64,
    pub projected_exp: f64,
    pub projected_exp_by_type: [f64; 9],
    pub projected_attacks: f64,
    pub damage_ratio: f64,
    pub clear_time: f64,
    pub critical_events: f64,
    pub score: f64,
}

/// `proxyObjectiveScore`: the deterministic projection a layout ranks by.
///
/// Steady DPS reads the board metrics; every other objective reads the
/// surrogate's score. `obstruction` defaults to the save's own stage.
pub fn proxy_score(
    jelly: &Jelly,
    layout: &Layout,
    objective: Objective,
    exp_cell_type: Option<usize>,
    obstruction: Option<usize>,
) -> f64 {
    if objective == Objective::Dps {
        let metrics = jelly.layout_metrics(layout, MetricOptions::default());
        return if metrics.valid {
            metrics.dps
        } else {
            f64::NEG_INFINITY
        };
    }
    Surrogate::project(
        jelly,
        layout,
        SurrogateOptions {
            objective,
            exp_cell_type,
            obstruction,
            ..SurrogateOptions::default()
        },
    )
    .score
}

impl Surrogate {
    /// Project one operation for `layout` and the options.
    pub fn project(
        jelly: &Jelly,
        layout: &super::board::Layout,
        options: SurrogateOptions,
    ) -> Surrogate {
        let obstruction = options.obstruction.unwrap_or(jelly.progress.obstruction);
        let duration = boss_time(obstruction);
        let fever = options.fever.unwrap_or(jelly.progress.fever);
        let metrics = jelly.layout_metrics(
            layout,
            MetricOptions {
                fever: Some(fever),
                fever_ramp_pct: if fever == 0 { duration / 2.0 } else { 0.0 },
                ..MetricOptions::default()
            },
        );
        if !metrics.valid {
            return Surrogate {
                reason: metrics.reason,
                score: f64::NEG_INFINITY,
                ..Surrogate::default()
            };
        }
        let hp = boss_hp(obstruction);
        let roid_level = jelly.upgrade_quantity(29);
        let roid_start = if roid_level >= 1.0 {
            match options.roid_timing {
                RoidTiming::Never => f64::INFINITY,
                RoidTiming::Immediate => 0.0,
                RoidTiming::FiveSeconds => 5.0,
                RoidTiming::Critical => duration,
                RoidTiming::At(seconds) => seconds.max(0.0),
            }
        } else {
            f64::INFINITY
        };
        let roid_end = roid_start + roid_duration_seconds();
        let roid_factor = 1.0 + (50.0 + roid_level) / 100.0;
        let cold_average = if fever == 0 {
            1.0 + duration / 200.0
        } else {
            1.0
        };
        let amoeba_unlocked = jelly.upgrade_quantity(28) >= 1.0;
        let boss_interval = boss_interval_seconds(obstruction);
        let global_speed = jelly.cell_speed_multiplier(Some(fever)) * metrics.speed_passive;
        let travel_scale = options.travel_scale.unwrap_or(1.0).clamp(0.8, 1.2);
        let opening_fraction = options.opening_fraction.unwrap_or(0.5).clamp(0.0, 1.0);
        let data = data();

        let mut expected_amoeba_hits = 0.0;
        let mut projected_damage = 0.0;
        let mut alive_slots = 0.0;
        let mut live_immunoid_slots = 0.0;
        let mut attacks_by_type = [0.0_f64; 9];
        let mut survival: Vec<f64> = Vec::with_capacity(metrics.cells.len());
        for cell in &metrics.cells {
            let travel = projectile_hit_delay(cell.anchor, cell.cell_type) * travel_scale;
            let first_shot = opening_fraction / cell.attacks_per_second.max(1e-12);
            let active_seconds = (duration - travel - first_shot).max(0.0);
            let roid_overlap = (duration.min(roid_end)
                - (0.0_f64).max(roid_start).max(travel + first_shot))
            .max(0.0);
            let roid_cadence: Cadence = attack_cadence(
                data.cells.base_cooldowns[cell.cell_type],
                Some(data.cells.base_cooldowns[cell.cell_type] / global_speed.max(1e-12)),
                Some(0.65 * cell.organelle * cell.proximity * roid_factor),
            );
            let attacks = active_seconds * cell.attacks_per_second
                + roid_overlap * (roid_cadence.attacks_per_second - cell.attacks_per_second);
            survival.push(1.0);
            if cell.cell_type == 0 {
                expected_amoeba_hits += attacks;
            }
            projected_damage += attacks * cell.damage * cold_average;
            attacks_by_type[cell.cell_type] += attacks;
            alive_slots += cell.slots.len() as f64;
            if cell.cell_type == 4 {
                live_immunoid_slots += cell.slots.len() as f64;
            }
        }
        let normal_ramp = if amoeba_unlocked {
            1.0 + expected_amoeba_hits / 200.0
        } else {
            1.0
        };
        projected_damage *= normal_ramp;
        let mut projected_attacks: f64 = attacks_by_type.iter().sum();

        let critical_unlocked = jelly.upgrade_quantity(36) == 1.0;
        let mut revives = jelly.upgrade_quantity(35).floor().max(0.0);
        let max_critical_seconds = options
            .max_critical_seconds
            .map(|seconds| seconds.max(0.0))
            .unwrap_or(600.0)
            .max(10.0);
        let attrition_bias = 1.0_f64;
        let mut critical_seconds = 0.0;
        let mut previous_event = 0.0;
        let mut next_event = boss_interval;
        let mut critical_events = 0.0;

        let integrate = |seconds: f64,
                         cells: &[super::metrics::CellMetrics],
                         survival: &mut [f64],
                         projected_damage: &mut f64,
                         projected_attacks: &mut f64,
                         attacks_by_type: &mut [f64; 9],
                         expected_amoeba_hits: &mut f64| {
            if seconds <= 0.0 {
                return;
            }
            let mut attacks = 0.0;
            let mut damage = 0.0;
            let mut amoeba_hits = 0.0;
            for (index, cell) in cells.iter().enumerate() {
                let cell_attacks = cell.attacks_per_second * survival[index] * seconds;
                attacks += cell_attacks;
                attacks_by_type[cell.cell_type] += cell_attacks;
                damage += cell.dps * survival[index];
                if cell.cell_type == 0 {
                    amoeba_hits += cell.attacks_per_second * survival[index];
                }
            }
            damage *= seconds;
            amoeba_hits *= seconds;
            let ramp = if amoeba_unlocked {
                1.0 + (*expected_amoeba_hits + amoeba_hits / 2.0) / 100.0
            } else {
                1.0
            };
            *projected_damage += damage * ramp;
            *projected_attacks += attacks;
            *expected_amoeba_hits += amoeba_hits;
        };

        while critical_unlocked
            && alive_slots > 0.0
            && next_event <= max_critical_seconds
            && projected_damage < hp
        {
            integrate(
                next_event - previous_event,
                &metrics.cells,
                &mut survival,
                &mut projected_damage,
                &mut projected_attacks,
                &mut attacks_by_type,
                &mut expected_amoeba_hits,
            );
            critical_seconds = next_event;
            previous_event = next_event;
            critical_events += 1.0;
            let focused = live_immunoid_slots > 0.0;
            let target_slots = if focused {
                live_immunoid_slots
            } else {
                alive_slots
            };
            for (index, cell) in metrics.cells.iter().enumerate() {
                if (focused && cell.cell_type != 4) || survival[index] <= 0.0 {
                    continue;
                }
                survival[index] *= (1.0 - attrition_bias / target_slots.max(1.0)).max(0.0);
            }
            if focused {
                live_immunoid_slots -= 1.0;
            }
            if revives > 0.0 {
                revives -= 1.0;
            } else {
                alive_slots -= 1.0;
            }
            next_event += boss_interval * if focused { 3.0 } else { 1.0 };
        }
        if critical_unlocked
            && projected_damage < hp
            && alive_slots > 0.0
            && previous_event < max_critical_seconds
        {
            integrate(
                max_critical_seconds.min(next_event) - previous_event,
                &metrics.cells,
                &mut survival,
                &mut projected_damage,
                &mut projected_attacks,
                &mut attacks_by_type,
                &mut expected_amoeba_hits,
            );
            critical_seconds = max_critical_seconds.min(next_event);
        }

        let total_cell_level = jelly.total_cell_level();
        let currency_multiplier =
            jelly.currency_multiplier(Some(fever), total_cell_level, jelly.progress.best_dps);
        let exp_multiplier = jelly.cell_exp_multiplier(Some(fever));
        let projected_bloodcells = projected_damage * currency_multiplier;
        let projected_exp_by_type: [f64; 9] =
            std::array::from_fn(|cell_type| attacks_by_type[cell_type] * exp_multiplier);
        let projected_exp: f64 = projected_exp_by_type.iter().sum();
        let damage_ratio = if hp > 0.0 { projected_damage / hp } else { 0.0 };
        let cap = duration + critical_seconds;
        let clear_time = if projected_damage > 0.0 {
            cap.min(cap / damage_ratio.max(1e-12))
        } else {
            f64::INFINITY
        };
        let score = match options.objective {
            Objective::Dps => projected_damage,
            Objective::Bloodcells => projected_bloodcells,
            Objective::Exp => options.exp_cell_type.map_or(projected_exp, |cell_type| {
                projected_exp_by_type[cell_type.min(8)]
            }),
            Objective::Survival => critical_seconds * 1e9 + projected_damage,
            Objective::Clear => {
                damage_ratio.min(1.0) * 1e12
                    + (damage_ratio - 1.0).max(0.0) * 1e10
                    + if damage_ratio >= 1.0 {
                        (cap - clear_time).max(0.0) * 1e7
                    } else {
                        0.0
                    }
            }
        };
        Surrogate {
            valid: true,
            reason: String::new(),
            obstruction,
            fever,
            hp,
            normal_seconds: duration,
            critical_seconds,
            projected_damage,
            projected_bloodcells,
            projected_exp,
            projected_exp_by_type,
            projected_attacks,
            damage_ratio,
            clear_time,
            critical_events,
            score,
        }
    }
}
