//! The long-run overgrowth curve, from `optimalOvergrowthTarget`: every target
//! OG level's reward, the full cycle it costs and the rate it settles at.

use super::account::Snapshot;
use super::{Farm, PLOTS, overgrowth_multiplier};

impl Farm {
    /// Every plot's curve under one snapshot.
    pub fn overgrowth_curves(&self, snapshot: &Snapshot) -> Vec<OvergrowthCurve> {
        (0..PLOTS)
            .map(|index| self.overgrowth_curve(index, snapshot))
            .collect()
    }

    /// `optimalOvergrowthTarget(plotIdx)`: the 0-30 curve, with the best
    /// long-run rate and what it costs from where the plot stands.
    pub fn overgrowth_curve(&self, index: usize, snapshot: &Snapshot) -> OvergrowthCurve {
        let plot = self.plots.get(index).copied().unwrap_or_default();
        let current_og = plot.overgrowth;
        let cycle_seconds = self.growth_cycle_seconds(&plot, snapshot);
        let mut rows = Vec::with_capacity(31);
        let mut repeat_seconds = cycle_seconds;
        let mut eta = 0.0;
        for target in 0..=30u32 {
            if target > 0 {
                let chance = overgrowth_chance(target, snapshot);
                repeat_seconds += cycle_seconds / chance.max(f64::MIN_POSITIVE);
            }
            if target > current_og {
                let chance = overgrowth_chance(target, snapshot);
                eta += cycle_seconds / chance.max(f64::MIN_POSITIVE);
            }
            let reward = overgrowth_multiplier(target);
            rows.push(OvergrowthRow {
                target,
                reward,
                repeat_seconds,
                long_run_rate: reward / repeat_seconds,
                eta_from_current: if target <= current_og { 0.0 } else { eta },
            });
        }
        // The first row with the best rate wins, as upstream's strict `>` does.
        let best = rows.iter().skip(1).fold(rows[0], |best, row| {
            if row.long_run_rate > best.long_run_rate {
                *row
            } else {
                best
            }
        });
        OvergrowthCurve {
            plot: index,
            current_og,
            cycle_seconds,
            best,
            rows,
        }
    }
}

/// `Math.min(1, 0.4^og * overgrowthCommon)`: the chance one growth tick takes
/// a plot from OG 0 to `og`.
fn overgrowth_chance(og: u32, snapshot: &Snapshot) -> f64 {
    (0.4_f64.powi(og as i32) * snapshot.overgrowth_common).min(1.0)
}

/// One plot's long-run overgrowth curve.
#[derive(Clone, Debug, PartialEq)]
pub struct OvergrowthCurve {
    pub plot: usize,
    pub current_og: u32,
    /// The plot's full growth cycle at its current overgrowth.
    pub cycle_seconds: f64,
    pub best: OvergrowthRow,
    pub rows: Vec<OvergrowthRow>,
}

impl OvergrowthCurve {
    /// The rows the upstream plot curve shows: the current level, the best
    /// target, the 30-step end and every target within four of the best.
    pub fn display_rows(&self) -> impl Iterator<Item = &OvergrowthRow> {
        let best = self.best.target;
        self.rows.iter().filter(move |row| {
            row.target.abs_diff(best) <= 4 || row.target == self.current_og || row.target == 30
        })
    }

    /// `Harvest` when the plot already sits at or past its best target, else
    /// how long the climb to it takes.
    pub fn advice(&self) -> OvergrowthAdvice {
        if self.current_og >= self.best.target {
            OvergrowthAdvice::Harvest
        } else {
            OvergrowthAdvice::Wait(self.best.eta_from_current)
        }
    }
}

/// What a plot's curve says to do next.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum OvergrowthAdvice {
    Harvest,
    Wait(f64),
}

/// One target overgrowth level on the curve.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OvergrowthRow {
    pub target: u32,
    /// The value and EXP multiplier at this overgrowth.
    pub reward: f64,
    /// The full cycle from a reset, including the overgrowth rolls.
    pub repeat_seconds: f64,
    pub long_run_rate: f64,
    pub eta_from_current: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Plot;

    fn planted(family: i64, overgrowth: u32) -> Farm {
        let mut farm = Farm::default();
        farm.plots[0] = Plot {
            seed_family: family,
            overgrowth,
            ..Plot::default()
        };
        farm
    }

    #[test]
    fn every_plot_has_a_full_curve() {
        let mut farm = Farm::default();
        for index in 0..PLOTS {
            farm.plots[index] = Plot {
                seed_family: (index % 7) as i64,
                ..Plot::default()
            };
        }
        let snapshot = farm.snapshot();
        let curves = farm.overgrowth_curves(&snapshot);
        assert_eq!(curves.len(), PLOTS);
        for curve in curves {
            assert_eq!(curve.rows.len(), 31);
            assert_eq!(curve.rows[0].target, 0);
            assert_eq!(curve.rows[30].target, 30);
            assert!(curve.rows.iter().all(|row| row.reward.is_finite()));
            assert!(curve.best.long_run_rate > 0.0, "plot {}", curve.plot);
            // The rate is the reward over the full cycle.
            for row in &curve.rows {
                assert_eq!(row.long_run_rate, row.reward / row.repeat_seconds);
            }
        }
    }

    #[test]
    fn the_curve_climbs_past_the_plots_current_level() {
        let farm = planted(0, 2);
        let curve = farm.overgrowth_curve(0, &farm.snapshot());
        assert_eq!(curve.current_og, 2);
        assert_eq!(curve.rows[2].eta_from_current, 0.0);
        assert_eq!(
            curve.rows[3].eta_from_current,
            curve.cycle_seconds / 0.4_f64.powi(3)
        );
        assert_eq!(curve.rows[1].eta_from_current, 0.0);
        // The reward doubles per level and needs the rolls, so the rate falls.
        assert_eq!(curve.rows[1].reward, 2.0);
        assert_eq!(curve.rows[3].reward, 8.0);
        assert!(curve.rows[3].long_run_rate < curve.rows[2].long_run_rate);
    }

    #[test]
    fn a_flat_multiplier_harvests_at_the_current_level() {
        // With the default overgrowth multiplier the rolls never pay: the best
        // long-run rate is the unmoved plot.
        let farm = planted(0, 0);
        let curve = farm.overgrowth_curve(0, &farm.snapshot());
        assert_eq!(curve.best.target, 0);
        assert_eq!(curve.advice(), OvergrowthAdvice::Harvest);
    }

    #[test]
    fn a_high_overgrowth_chance_pays_for_a_target_level() {
        let farm = planted(0, 0);
        let mut snapshot = farm.snapshot();
        snapshot.overgrowth_common = 100.0;
        let curve = farm.overgrowth_curve(0, &snapshot);
        assert!(curve.best.target > 0, "best is {}", curve.best.target);
        assert!(curve.best.target < 30);
        assert!(curve.best.long_run_rate >= curve.rows[30].long_run_rate);
        assert_eq!(
            curve.advice(),
            OvergrowthAdvice::Wait(curve.best.eta_from_current)
        );
        let settled = planted(0, curve.best.target);
        let mut settled_snapshot = settled.snapshot();
        settled_snapshot.overgrowth_common = 100.0;
        assert_eq!(
            settled.overgrowth_curve(0, &settled_snapshot).advice(),
            OvergrowthAdvice::Harvest
        );
    }

    #[test]
    fn the_display_rows_keep_the_current_best_and_last_target() {
        let farm = planted(0, 0);
        let mut snapshot = farm.snapshot();
        snapshot.overgrowth_common = 100.0;
        let curve = farm.overgrowth_curve(0, &snapshot);
        let shown: Vec<u32> = curve.display_rows().map(|row| row.target).collect();
        assert!(shown.contains(&curve.current_og));
        assert!(shown.contains(&curve.best.target));
        assert!(shown.contains(&30));
        assert!(shown.len() <= 11);
    }

    #[test]
    fn an_empty_plot_earns_nothing() {
        let farm = Farm::default();
        let curve = farm.overgrowth_curve(0, &farm.snapshot());
        assert!(curve.cycle_seconds.is_infinite());
        assert_eq!(curve.best.long_run_rate, 0.0);
        assert_eq!(curve.best.target, 0);
        assert_eq!(curve.advice(), OvergrowthAdvice::Harvest);
    }
}
