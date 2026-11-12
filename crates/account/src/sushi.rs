//! Sushi Station terms, ported from `js/stats/systems/w7/sushi.js`.
//!
//! `computeRooBonus` pays Poppy's per-index bonus once enough tiers of her
//! sushi upgrades are found: `OptLacc[271]` counts the tiers, `OptLacc[279]`
//! the Mega Feather progress. The per-index multipliers are the upstream table,
//! and the Mega Feather rows add their percentage to the whole bonus.

use super::Decoder;

/// `perIdx`: the per-index multiplier of one Roo bonus, `3` when unlisted.
const PER_INDEX: [f64; 7] = [3.0, 3.0, 5.0, 2.0, 2.0, 0.5, 3.0];
/// The `RooMegafeather` milestones, in `OptLacc[279]` order.
const MEGA_FEATHER: [f64; 5] = [1.0, 3.0, 6.0, 8.0, 11.0];

impl Decoder<'_> {
    /// `computeRooBonus(index)`: the bonus one of Poppy's rows pays.
    pub fn roo(&self, index: usize) -> f64 {
        let tiers = ((self.ola(271) - index as f64) / 7.0).ceil().max(0.0);
        if tiers <= 0.0 {
            return 0.0;
        }
        let mega = self.ola(279);
        let mut feather = 0.0;
        for (position, milestone) in MEGA_FEATHER.iter().enumerate() {
            let found = if mega > *milestone {
                if *milestone == 11.0 { mega - 11.0 } else { 1.0 }
            } else {
                0.0
            };
            feather += 50.0 * found.min(1.0);
            if position == 4 {
                feather += 25.0 * (found - 1.0).max(0.0);
            }
        }
        let multiplier = PER_INDEX.get(index).copied().unwrap_or(3.0);
        multiplier
            * (1.0 + self.legend(26) / 100.0)
            * (1.0 + self.companion(51))
            * (1.0 + feather / 100.0)
            * tiers
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use idleon_research::model::Simulation;
    use serde_json::{Map, Value};

    /// A save with the sushi tiers at `[271]` and the Mega Feather at `[279]`.
    fn save(tiers: f64, feather: f64) -> Map<String, Value> {
        let mut options = vec![Value::from(0); 701];
        options[271] = Value::from(tiers);
        options[279] = Value::from(feather);
        serde_json::json!({
            "OptLacc": serde_json::to_string(&options).expect("an array"),
        })
        .as_object()
        .expect("an object")
        .clone()
    }

    fn roo(save: &Map<String, Value>, index: usize) -> f64 {
        let simulation = Simulation::default();
        let mut warnings = Vec::new();
        let view = Decoder::new(save, save, &[], &simulation, &mut warnings);
        view.roo(index)
    }

    #[test]
    fn the_tiers_round_up_from_the_bonus_index() {
        // (66 - 6) / 7 = 8.57, so nine tiers; the Feather rows add 475 %.
        assert_eq!(roo(&save(66.0, 21.0), 6), 3.0 * 5.75 * 9.0);
    }

    #[test]
    fn an_index_past_the_saved_tiers_pays_nothing() {
        assert_eq!(roo(&save(6.0, 0.0), 6), 0.0);
    }
}
