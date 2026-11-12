//! The strategy search and the greedy upgrade path, ported from
//! `js/minehead/sim.js`.
//!
//! Both searches run the same [`simulate_game`] the Play tab drives. The
//! optimizer scores win rate first and damage as a fraction of the boss's
//! health second, exactly as upstream's `_optimizerScore`.

use crate::math::floor_hp;
use crate::sim::{MonteCarlo, Options, Params, monte_carlo};
use idleon_common::{Control, Progress};

/// One knob's search values, in the upstream `expandGrid` order.
#[derive(Clone, Debug, PartialEq)]
pub struct Grid {
    pub knobs: Vec<(&'static str, Vec<f64>)>,
}

impl Grid {
    /// The combos this grid expands to.
    pub fn combos(&self) -> usize {
        self.knobs.iter().map(|(_, values)| values.len()).product()
    }

    /// Every combination, the last knob varying fastest, as upstream.
    pub fn expand(&self) -> Vec<Params> {
        let mut combos = vec![Params::default()];
        for (key, values) in &self.knobs {
            let mut next = Vec::with_capacity(combos.len() * values.len());
            for params in &combos {
                for value in values {
                    let mut params = *params;
                    set(&mut params, key, *value);
                    next.push(params);
                }
            }
            combos = next;
        }
        combos
    }
}

/// The knobs the optimizer searches when the caller does not narrow them.
pub fn default_grid() -> Grid {
    Grid {
        knobs: vec![
            ("ev_multiplier", vec![0.8, 1.0, 1.3, 2.0]),
            ("min_reveal", vec![0.0, 1.0, 3.0]),
            ("mine_cap_pct", vec![0.5, 1.0]),
            ("golden_mine_pct", vec![0.3, 1.0]),
            ("hp_threshold", vec![0.8, 1.0]),
            ("block_aggro", vec![1.0, 0.0]),
            ("commit_min", vec![0.0, 0.02]),
            ("life_aggro", vec![0.6, 1.0]),
            ("turn1_ev_mul", vec![0.0, 0.7]),
            ("crown_chase", vec![0.0, 0.4]),
            ("insta_mode", vec![0.0, 0.3]),
            ("commit_turns", vec![0.0]),
            ("post_mine_panic", vec![0.0]),
            ("last_turn_aggro", vec![0.0]),
            ("target_reveals", vec![0.0]),
            ("golden_first", vec![0.0]),
            ("crown_or_die", vec![0.0]),
            ("hot_streak", vec![0.0]),
            ("block_hoard", vec![0.0]),
            ("corner_bias", vec![1.0]),
            ("edge_bias", vec![1.0]),
            ("cluster_bias", vec![1.0]),
        ],
    }
}

/// The human-behaviour knobs the inferrer refines after the base sweep.
pub fn inference_grid() -> Grid {
    Grid {
        knobs: vec![
            ("post_mine_panic", vec![0.0, 0.2, 0.5, 0.8]),
            ("last_turn_aggro", vec![0.0, 0.2, 0.5]),
            ("target_reveals", vec![0.0, 3.0, 5.0, 8.0]),
            ("golden_first", vec![0.0, 1.0]),
            ("crown_or_die", vec![0.0, 0.3, 0.6]),
            ("hot_streak", vec![0.0, 0.2, 0.5]),
            ("block_hoard", vec![0.0, 0.3, 0.6]),
            ("corner_bias", vec![1.0, 1.5, 2.5]),
            ("edge_bias", vec![1.0, 1.3, 2.0]),
            ("cluster_bias", vec![1.0, 1.5, 2.5, 4.0]),
        ],
    }
}

/// Set one knob by its upstream key. An unknown key leaves the params alone.
pub fn set(params: &mut Params, key: &str, value: f64) -> bool {
    let flag = value != 0.0;
    match key {
        "ev_multiplier" => params.ev_multiplier = value,
        "min_reveal" => params.min_reveal = value,
        "mine_cap_pct" => params.mine_cap_pct = value,
        "golden_mine_pct" => params.golden_mine_pct = value,
        "hp_threshold" => params.hp_threshold = value,
        "block_aggro" => params.block_aggro = flag,
        "commit_min" => params.commit_min = value,
        "life_aggro" => params.life_aggro = value,
        "turn1_ev_mul" => params.turn1_ev_mul = value,
        "crown_chase" => params.crown_chase = value,
        "commit_turns" => params.commit_turns = value.max(0.0) as u32,
        "insta_mode" => params.insta_mode = value,
        "post_mine_panic" => params.post_mine_panic = value,
        "last_turn_aggro" => params.last_turn_aggro = value,
        "target_reveals" => params.target_reveals = value,
        "golden_first" => params.golden_first = flag,
        "crown_or_die" => params.crown_or_die = value,
        "hot_streak" => params.hot_streak = value,
        "block_hoard" => params.block_hoard = value,
        "corner_bias" => params.corner_bias = value,
        "edge_bias" => params.edge_bias = value,
        "cluster_bias" => params.cluster_bias = value,
        _ => return false,
    }
    true
}

fn flag(value: bool) -> f64 {
    if value { 1.0 } else { 0.0 }
}

/// Read one knob by its upstream key.
pub fn get(params: &Params, key: &str) -> f64 {
    match key {
        "ev_multiplier" => params.ev_multiplier,
        "min_reveal" => params.min_reveal,
        "mine_cap_pct" => params.mine_cap_pct,
        "golden_mine_pct" => params.golden_mine_pct,
        "hp_threshold" => params.hp_threshold,
        "block_aggro" => flag(params.block_aggro),
        "commit_min" => params.commit_min,
        "life_aggro" => params.life_aggro,
        "turn1_ev_mul" => params.turn1_ev_mul,
        "crown_chase" => params.crown_chase,
        "commit_turns" => f64::from(params.commit_turns),
        "insta_mode" => params.insta_mode,
        "post_mine_panic" => params.post_mine_panic,
        "last_turn_aggro" => params.last_turn_aggro,
        "target_reveals" => params.target_reveals,
        "golden_first" => flag(params.golden_first),
        "crown_or_die" => params.crown_or_die,
        "hot_streak" => params.hot_streak,
        "block_hoard" => params.block_hoard,
        "corner_bias" => params.corner_bias,
        "edge_bias" => params.edge_bias,
        "cluster_bias" => params.cluster_bias,
        _ => 0.0,
    }
}

/// One scored strategy.
#[derive(Clone, Debug, PartialEq)]
pub struct Candidate {
    pub params: Params,
    pub result: MonteCarlo,
    pub score: f64,
}

/// What one optimizer run found.
#[derive(Clone, Debug, PartialEq)]
pub struct Optimized {
    pub best: Candidate,
    pub top: Vec<Candidate>,
    pub total_combos: usize,
}

/// `_optimizerScore`: win rate first, damage fraction second.
pub fn score(result: &MonteCarlo, hp: f64) -> f64 {
    result.win_rate + result.avg_dmg / if hp == 0.0 { 1.0 } else { hp } * 0.001
}

/// Search the grid, then re-evaluate the best few with more trials.
pub fn optimize(
    options: &Options,
    grid: &Grid,
    screen_trials: u32,
    final_trials: u32,
    top_n: usize,
    seed: u32,
    progress: &mut dyn FnMut(Progress) -> Control,
) -> Option<Optimized> {
    let combos = grid.expand();
    let total = combos.len();
    let hp = floor_hp(options.floor, options.svar_hp);
    let mut scored: Vec<Candidate> = Vec::with_capacity(total);
    for (index, params) in combos.into_iter().enumerate() {
        let result = monte_carlo(options, &params, screen_trials, seed);
        scored.push(Candidate {
            params,
            score: score(&result, hp),
            result,
        });
        let step = Progress {
            fraction: (index + 1) as f32 / (total + top_n) as f32,
            message: format!("Screening {}/{}", index + 1, total),
        };
        if progress(step) == Control::Stop {
            return None;
        }
    }
    scored.sort_by(|left, right| right.score.total_cmp(&left.score));
    scored.truncate(top_n.max(1));
    for (index, candidate) in scored.iter_mut().enumerate() {
        candidate.result = monte_carlo(options, &candidate.params, final_trials, seed);
        candidate.score = score(&candidate.result, hp);
        let step = Progress {
            fraction: (total + index + 1) as f32 / (total + top_n.max(1)) as f32,
            message: format!("Refining {}/{}", index + 1, top_n.max(1)),
        };
        if progress(step) == Control::Stop {
            return None;
        }
    }
    scored.sort_by(|left, right| right.score.total_cmp(&left.score));
    let best = scored.first()?.clone();
    Some(Optimized {
        best,
        top: scored,
        total_combos: total,
    })
}

/// The inputs the cost path prices upgrades with.
#[derive(Clone, Debug, PartialEq)]
pub struct PathInputs {
    pub currency: f64,
    pub research_level: u32,
    pub qty26: f64,
    pub server_var_cost: f64,
    pub rog_cost_pct: f64,
    pub max_steps: u32,
}

/// One purchase in the cost path.
#[derive(Clone, Debug, PartialEq)]
pub struct Step {
    pub step: u32,
    pub upg_idx: usize,
    pub result: MonteCarlo,
    pub cost: f64,
}

/// What the affordable-upgrade search found.
#[derive(Clone, Debug, PartialEq)]
pub struct Path {
    pub steps: Vec<Step>,
    pub final_levels: Vec<f64>,
    pub baseline: MonteCarlo,
    pub final_result: MonteCarlo,
    pub spent: f64,
    pub remaining_currency: f64,
}

/// The Path tab's search: each step buys the affordable +1 with the best
/// damage gained per currency spent, stopping when nothing is affordable.
///
/// The scoring is the upstream `_runPathStep` rule: `dmg increase / cost` plus
/// `0.01 × win-rate increase`.
pub fn cost_path(
    options: &Options,
    params: &Params,
    inputs: &PathInputs,
    trials: u32,
    seed: u32,
    progress: &mut dyn FnMut(Progress) -> Control,
) -> Option<Path> {
    let data = crate::data::game();
    let mut levels = options.levels.clone();
    let mut currency = inputs.currency;
    let baseline = monte_carlo(options, params, trials, seed);
    let mut steps: Vec<Step> = Vec::new();
    let total = inputs.max_steps.max(1) as f32;
    for step in 0..inputs.max_steps {
        let candidates: Vec<usize> = (0..data.upgrades.len())
            .filter(|index| {
                let level = levels.get(*index).copied().unwrap_or(0.0);
                let max = data
                    .upgrade(*index)
                    .map(|upgrade| upgrade.max_lv)
                    .unwrap_or(0.0);
                let unlocked = inputs.research_level >= crate::math::upg_lv_req(*index);
                let not_maxed = level < max || max > 998.0;
                unlocked
                    && not_maxed
                    && crate::math::can_buy(
                        data,
                        *index,
                        level,
                        currency,
                        inputs.qty26,
                        inputs.server_var_cost,
                        inputs.rog_cost_pct,
                    )
            })
            .collect();
        if candidates.is_empty() {
            break;
        }
        let previous_dmg = steps
            .last()
            .map_or(baseline.avg_dmg, |step| step.result.avg_dmg);
        let previous_win = steps
            .last()
            .map_or(baseline.win_rate, |step| step.result.win_rate);
        let mut best: Option<(usize, f64, MonteCarlo, f64)> = None;
        for (position, index) in candidates.iter().enumerate() {
            let mut trial = options.clone();
            let mut modified = levels.clone();
            if modified.len() <= *index {
                modified.resize(*index + 1, 0.0);
            }
            modified[*index] += 1.0;
            trial.levels = modified;
            let result = monte_carlo(&trial, params, trials, seed);
            let level = levels.get(*index).copied().unwrap_or(0.0);
            let cost = crate::math::upg_cost(
                data,
                *index,
                level,
                inputs.qty26,
                inputs.server_var_cost,
                inputs.rog_cost_pct,
            );
            let win_increase = result.win_rate - previous_win;
            let score = if cost > 0.0 {
                (result.avg_dmg - previous_dmg) / cost + win_increase * 0.01
            } else {
                f64::NEG_INFINITY
            };
            if best.as_ref().is_none_or(|(_, best, _, _)| score > *best) {
                best = Some((*index, score, result, cost));
            }
            let line = Progress {
                fraction: (step as f32 + (position + 1) as f32 / candidates.len() as f32) / total,
                message: format!("Step {}: upgrade {}", step + 1, position + 1),
            };
            if progress(line) == Control::Stop {
                return None;
            }
        }
        let Some((index, _, result, cost)) = best else {
            break;
        };
        currency -= cost;
        if levels.len() <= index {
            levels.resize(index + 1, 0.0);
        }
        levels[index] += 1.0;
        steps.push(Step {
            step: step + 1,
            upg_idx: index,
            result,
            cost,
        });
    }
    let spent: f64 = steps.iter().map(|step| step.cost).sum();
    let mut final_options = options.clone();
    final_options.levels = levels.clone();
    let final_result = monte_carlo(&final_options, params, trials * 2, seed);
    Some(Path {
        steps,
        final_levels: levels,
        baseline,
        final_result,
        spent,
        remaining_currency: currency,
    })
}
