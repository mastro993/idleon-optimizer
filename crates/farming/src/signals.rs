//! The priority signals the overview leads with, from `farmingDiagnostics`.

use super::account::Snapshot;
use super::{Farm, PLOTS};

impl Farm {
    /// `farmingDiagnostics`: what the loaded field is telling the player — the
    /// plots at the value cap, the growth floor, the Medal plots and the Land
    /// Rank points waiting to be spent.
    pub fn diagnostics(&self, snapshot: &Snapshot) -> Diagnostics {
        let plots = (0..PLOTS).map(|index| self.plot_metrics_with(index, snapshot));
        let mut capped = 0;
        let mut ordinary_at_cap = true;
        let mut guaranteed = 0;
        let mut medal = 0;
        for metric in plots {
            if metric.crop_value.capped {
                capped += 1;
            }
            let plot = self.plots.get(metric.plot).copied().unwrap_or_default();
            if (0..6).contains(&metric.seed_family)
                && self.growth_cycle_seconds(&plot, snapshot) > 1.0
            {
                ordinary_at_cap = false;
            }
            if metric.next_crop_chance >= 1.0 {
                guaranteed += 1;
            }
            if metric.seed_family == 6 {
                medal += 1;
            }
        }
        Diagnostics {
            crop_count: self.crop_count,
            crop_value_cap: snapshot.crop_value_cap,
            capped_plots: capped,
            all_plots_capped: capped == PLOTS,
            ordinary_growth_at_active_cap: ordinary_at_cap,
            medal_plot_count: medal,
            guaranteed_evolution_plots: guaranteed,
            unspent_rank_points: self.land_rank_points_left().max(0) as u32,
            fifth_column_max: self.fifth_column_max(),
            gmo_counts: snapshot.crop_counts,
        }
    }
}

/// `farmingDiagnostics`'s result.
#[derive(Clone, Debug, PartialEq)]
pub struct Diagnostics {
    pub crop_count: u32,
    pub crop_value_cap: f64,
    pub capped_plots: usize,
    pub all_plots_capped: bool,
    /// Whether every ordinary seed's cycle is at the one-second active floor.
    pub ordinary_growth_at_active_cap: bool,
    pub medal_plot_count: usize,
    /// Plots whose next evolution roll is already certain.
    pub guaranteed_evolution_plots: usize,
    pub unspent_rank_points: u32,
    pub fifth_column_max: u32,
    /// Crops discovered at or above each GMO threshold.
    pub gmo_counts: [f64; 5],
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Plot;

    fn planted(family: i64, overgrowth: u32) -> Plot {
        Plot {
            seed_family: family,
            overgrowth,
            ..Plot::default()
        }
    }

    #[test]
    fn an_empty_field_signals_nothing() {
        let farm = Farm::default();
        let diagnostics = farm.diagnostics(&farm.snapshot());
        assert!(!diagnostics.all_plots_capped);
        // Vacuously: no ordinary plot is above the floor.
        assert!(diagnostics.ordinary_growth_at_active_cap);
        assert_eq!(diagnostics.medal_plot_count, 0);
        assert_eq!(diagnostics.capped_plots, 0);
        assert_eq!(diagnostics.crop_count, 0);
        assert_eq!(diagnostics.gmo_counts, [0.0; 5]);
    }

    #[test]
    fn a_plot_above_the_floor_clears_the_growth_signal() {
        let mut farm = Farm::default();
        farm.plots[0] = planted(0, 0);
        farm.plots[1] = planted(6, 0);
        let mut snapshot = farm.snapshot();
        // The default field's growth rate leaves family 0 far above 1 second.
        assert!(farm.growth_cycle_seconds(&farm.plots[0], &snapshot) > 1.0);
        assert!(!farm.diagnostics(&snapshot).ordinary_growth_at_active_cap);
        assert_eq!(farm.diagnostics(&snapshot).medal_plot_count, 1);

        // A rate past the slowest ordinary family puts them all at the floor;
        // the Medal plot keeps its fixed cycle either way.
        snapshot.growth_rate = 120_000.0;
        let fast = farm.diagnostics(&snapshot);
        assert!(fast.ordinary_growth_at_active_cap);
        assert_eq!(fast.medal_plot_count, 1);
        assert!(farm.growth_cycle_seconds(&farm.plots[1], &snapshot) > 1.0);
    }

    #[test]
    fn the_value_cap_counts_every_capped_plot() {
        let mut farm = Farm::default();
        for index in 0..PLOTS {
            farm.plots[index] = planted(0, 0);
        }
        let mut snapshot = farm.snapshot();
        let uncapped = farm.diagnostics(&snapshot);
        assert_eq!(uncapped.capped_plots, 0);
        assert!(!uncapped.all_plots_capped);

        snapshot.crop_value_cap = 1.0;
        let capped = farm.diagnostics(&snapshot);
        assert_eq!(capped.capped_plots, PLOTS);
        assert!(capped.all_plots_capped);
        assert_eq!(capped.crop_value_cap, 1.0);
    }

    #[test]
    fn a_certain_roll_is_counted_as_a_guaranteed_evolution() {
        let mut farm = Farm::default();
        farm.plots[0] = planted(0, 0);
        farm.farm_level = 75;
        let mut snapshot = farm.snapshot();
        assert!(farm.plot_metrics_with(0, &snapshot).next_crop_chance < 1.0);
        assert_eq!(farm.diagnostics(&snapshot).guaranteed_evolution_plots, 0);
        snapshot.next_crop_common = 1e12;
        assert!(farm.plot_metrics_with(0, &snapshot).next_crop_chance >= 1.0);
        assert_eq!(farm.diagnostics(&snapshot).guaranteed_evolution_plots, 1);
    }

    #[test]
    fn unspent_points_never_go_negative() {
        let mut farm = Farm::default();
        farm.plots[0] = planted(0, 0);
        farm.rank_ranks[0] = 2;
        farm.rank_ranks[1] = 2;
        farm.rank_levels[0] = 6;
        assert!(farm.land_rank_points_left() < 0);
        assert_eq!(farm.diagnostics(&farm.snapshot()).unspent_rank_points, 0);
        farm.rank_ranks[0] = 10;
        assert_eq!(farm.diagnostics(&farm.snapshot()).unspent_rank_points, 6);
        assert_eq!(farm.diagnostics(&farm.snapshot()).fifth_column_max, 1);
    }

    #[test]
    fn the_gmo_counts_are_the_snapshot_counts() {
        let mut farm = Farm::default();
        farm.crop_quantities.insert(3, 250.0);
        farm.crop_quantities.insert(40, 1500.0);
        farm.crop_count = 2;
        let diagnostics = farm.diagnostics(&farm.snapshot());
        assert_eq!(diagnostics.gmo_counts, [2.0, 1.0, 0.0, 0.0, 0.0]);
        assert_eq!(diagnostics.crop_count, 2);
    }
}
