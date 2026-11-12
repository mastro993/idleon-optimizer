//! The Rank Upgrades tab: what one more level of each unlocked upgrade does to
//! a floor run, ported from `_runUpgradeRank` (`js/ui/minehead-ui.js`).

use crate::data::game;
use crate::math::{can_buy, upg_cost, upg_lv_req};
use crate::optimize::PathInputs;
use crate::sim::{MonteCarlo, Options, Params, monte_carlo};
use idleon_common::{Control, Progress};

/// One upgrade's +1 comparison against the baseline.
#[derive(Clone, Debug, PartialEq)]
pub struct RankRow {
    pub idx: usize,
    pub name: String,
    pub dmg_pct: f64,
    pub win_delta: f64,
    pub cost: f64,
    pub dmg_per_cost: f64,
    pub can_afford: bool,
    pub result: MonteCarlo,
}

/// What one ranking run found.
#[derive(Clone, Debug, PartialEq)]
pub struct RankOutcome {
    pub baseline: MonteCarlo,
    pub rows: Vec<RankRow>,
    pub trials: u32,
}

/// Rank each unlocked upgrade by the damage one more level buys.
pub fn rank_upgrades(
    options: &Options,
    params: &Params,
    inputs: &PathInputs,
    trials: u32,
    seed: u32,
    progress: &mut dyn FnMut(Progress) -> Control,
) -> Option<RankOutcome> {
    let data = game();
    let eligible: Vec<usize> = (0..data.upgrades.len())
        .filter(|index| {
            let level = options.levels.get(*index).copied().unwrap_or(0.0);
            let max = data
                .upgrade(*index)
                .map(|upgrade| upgrade.max_lv)
                .unwrap_or(0.0);
            (level < max || max > 998.0) && inputs.research_level >= upg_lv_req(*index)
        })
        .collect();
    let total = eligible.len() as f32 + 1.0;
    let baseline = monte_carlo(options, params, trials, seed);
    if progress(Progress {
        fraction: 1.0 / total,
        message: "Baseline".into(),
    }) == Control::Stop
    {
        return None;
    }
    let qty26 = inputs.qty26;
    let mut rows = Vec::with_capacity(eligible.len());
    for (position, index) in eligible.iter().enumerate() {
        let mut trial = options.clone();
        let mut levels = options.levels.clone();
        if levels.len() <= *index {
            levels.resize(*index + 1, 0.0);
        }
        levels[*index] += 1.0;
        trial.levels = levels;
        let result = monte_carlo(&trial, params, trials, seed);
        let level = options.levels.get(*index).copied().unwrap_or(0.0);
        let cost = upg_cost(
            data,
            *index,
            level,
            qty26,
            inputs.server_var_cost,
            inputs.rog_cost_pct,
        );
        let dmg_pct = if baseline.avg_dmg > 0.0 {
            (result.avg_dmg - baseline.avg_dmg) / baseline.avg_dmg * 100.0
        } else {
            0.0
        };
        let damage = result.avg_dmg - baseline.avg_dmg;
        rows.push(RankRow {
            idx: *index,
            name: data
                .upgrade(*index)
                .map(|upgrade| upgrade.name.clone())
                .unwrap_or_default(),
            dmg_pct,
            win_delta: (result.win_rate - baseline.win_rate) * 100.0,
            cost,
            dmg_per_cost: if cost > 0.0 { damage / cost } else { 0.0 },
            can_afford: can_buy(
                data,
                *index,
                level,
                inputs.currency,
                qty26,
                inputs.server_var_cost,
                inputs.rog_cost_pct,
            ),
            result,
        });
        let line = Progress {
            fraction: (position as f32 + 2.0) / total,
            message: format!("Upgrade {}/{}", position + 1, eligible.len()),
        };
        if progress(line) == Control::Stop {
            return None;
        }
    }
    rows.sort_by(|left, right| right.dmg_per_cost.total_cmp(&left.dmg_per_cost));
    Some(RankOutcome {
        baseline,
        rows,
        trials,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options() -> Options {
        let mut levels = vec![0.0; 30];
        levels[2] = 2.0;
        levels[6] = 3.0;
        Options {
            floor: 0,
            levels,
            doublers: Vec::new(),
            grid_bonus_166_1: 0.0,
            damage: crate::math::DamageInputs::default(),
            svar_hp: 1.0,
            max_turns: 200,
            mine_reduction: 0.0,
            legend40_pts: 0.0,
            jelly_gold_tiles45: 0.0,
        }
    }

    #[test]
    fn the_ranking_covers_the_unlocked_upgrades() {
        let outcome = rank_upgrades(
            &options(),
            &Params::default(),
            &PathInputs {
                currency: 0.0,
                research_level: 40,
                qty26: 0.0,
                server_var_cost: 1.0,
                rog_cost_pct: 0.0,
                max_steps: 50,
            },
            5,
            1,
            &mut |_| Control::Continue,
        )
        .expect("a ranking");
        assert!(outcome.rows.len() > 5);
        assert!(outcome.rows.iter().all(|row| !row.can_afford));
        assert!(
            outcome
                .rows
                .windows(2)
                .all(|pair| pair[0].dmg_per_cost >= pair[1].dmg_per_cost)
        );
    }
}
