//! Growth breakpoints: the raw and active cycle each seed family takes, from
//! `growthBreakpointRows`.

use super::account::Snapshot;
use super::{Farm, data};

impl Farm {
    /// `growthBreakpointRows`: one row per seed family with its raw cycle, the
    /// active cycle the scheduler allows and the rate the one-second floor
    /// would need.
    pub fn growth_breakpoints(&self, snapshot: &Snapshot) -> Vec<GrowthBreakpoint> {
        data()
            .seeds
            .iter()
            .map(|seed| row(seed, snapshot))
            .collect()
    }
}

/// One seed family's growth cycle.
#[derive(Clone, Debug, PartialEq)]
pub struct GrowthBreakpoint {
    pub family: usize,
    pub name: String,
    /// The growth progress one harvest needs.
    pub requirement: f64,
    pub raw_seconds: f64,
    pub active_seconds: f64,
    /// The rate the one-second floor would need; infinite for the Medal cycle.
    pub one_second_rate: f64,
    pub at_active_cap: bool,
    /// Medal seeds run on a fixed seven-hour cycle that the growth rate scales,
    /// so they have no one-second threshold.
    pub medal_fixed: bool,
}

fn row(seed: &super::SeedFamily, snapshot: &Snapshot) -> GrowthBreakpoint {
    let medal_fixed = seed.id == 6;
    let requirement = if medal_fixed {
        25_200.0 * snapshot.growth_rate
    } else {
        14_400.0 * 1.5_f64.powf(seed.id as f64)
    };
    let raw_seconds = requirement / snapshot.growth_rate;
    GrowthBreakpoint {
        family: seed.id,
        name: seed.name.clone(),
        requirement,
        raw_seconds,
        active_seconds: raw_seconds.max(1.0),
        one_second_rate: if medal_fixed {
            f64::INFINITY
        } else {
            requirement
        },
        at_active_cap: !medal_fixed && raw_seconds <= 1.0,
        medal_fixed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_family_has_a_breakpoint_row() {
        let farm = Farm::default();
        let rows = farm.growth_breakpoints(&farm.snapshot());
        assert_eq!(rows.len(), 7);
        assert_eq!(rows[0].requirement, 14_400.0);
        assert!((rows[1].requirement - 21_600.0).abs() < 1e-9);
        assert!(rows[6].medal_fixed);
        assert!(rows[6].one_second_rate.is_infinite());
        assert!(!rows[0].medal_fixed);
        assert!(!rows[0].at_active_cap);
    }

    #[test]
    fn the_active_cycle_never_drops_below_one_second() {
        let farm = Farm::default();
        let mut snapshot = farm.snapshot();
        snapshot.growth_rate = 30_000.0;
        let rows = farm.growth_breakpoints(&snapshot);
        assert!(rows[0].raw_seconds < 1.0);
        assert_eq!(rows[0].active_seconds, 1.0);
        assert!(rows[0].at_active_cap);
        assert!(!rows[6].at_active_cap);
        // Family 5 needs 109 350 progress, so it is still above the floor.
        assert!(!rows[5].at_active_cap);
    }

    #[test]
    fn the_medal_cycle_scales_with_the_growth_rate() {
        let farm = Farm::default();
        let mut snapshot = farm.snapshot();
        snapshot.growth_rate = 4.0;
        let rows = farm.growth_breakpoints(&snapshot);
        assert_eq!(rows[6].requirement, 25_200.0 * 4.0);
        assert_eq!(rows[6].raw_seconds, 25_200.0);
        assert_eq!(rows[0].requirement, 14_400.0);
        assert_eq!(rows[0].raw_seconds, 3_600.0);
    }
}
