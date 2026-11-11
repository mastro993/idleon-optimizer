//! The God Rank offering model: its odds, its Divinity point cost and the
//! attempts a plan budgets for a target confidence.
//!
//! Ports `offeringOdds`, `offeringCost` and the attempt rows of `offeringStats`
//! from `js/stats/systems/w5/coral-god-rank.js`.

use super::Inputs;

/// `offeringOdds`: the success chance of one offering per offer tier.
pub const OFFERING_ODDS: [f64; 6] = [0.01, 0.05, 0.1, 0.25, 0.5, 1.0];

/// The highest free-offering chance `Y4` grants, as `offeringStats` caps it.
const MAX_FREE_CHANCE: f64 = 0.35;

/// The planning confidence bounds the beam search clamps to.
const MIN_CONFIDENCE: f64 = 0.5;
const MAX_CONFIDENCE: f64 = 0.999_999;

/// `offeringOdds`: the tier caps at the six-row odds table.
pub fn offering_odds(tier: u32) -> f64 {
    OFFERING_ODDS[tier.min(5) as usize]
}

/// What one offering attempt costs in Divinity points.
///
/// `offeringCost` returns the cost an offering at `god_rank` carries; from rank
/// 3 the server's `DivCostAfter3` percent compounds, capped at 80%.
pub fn offering_cost(god_rank: u32, div_cost_after3: f64, tier: u32) -> f64 {
    let rank = f64::from(god_rank);
    let odds = offering_odds(tier);
    let mut cost = (20.0 * (rank + 1.3).powf(2.3) * 2.2f64.powf(rank) + 60.0) * odds;
    if rank >= 3.0 {
        let growth = (1.0 + div_cost_after3 / 100.0).clamp(1.0, 1.8);
        cost *= growth.powf(rank - 2.0);
    }
    if cost < 1.0 {
        1.0
    } else if cost < 1e6 {
        cost.ceil()
    } else {
        cost
    }
}

/// One offering row: what it costs and what the plan reserves for it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Offer {
    pub odds: f64,
    /// The planning confidence the attempts below reach.
    pub confidence: f64,
    pub cost: f64,
    /// The chance an offering costs nothing, from the Y4 bubble.
    pub free_chance: f64,
    /// Attempts to reach the planning confidence.
    pub attempts: u32,
    /// What those attempts are expected to spend, counting free ones.
    pub expected_spend: f64,
    /// The worst case the plan budgets for: every attempt paid.
    pub max_paid_spend: f64,
}

/// The offering at `god_rank` under the section's inputs.
pub fn offer(inputs: &Inputs, god_rank: u32) -> Offer {
    let odds = offering_odds(inputs.tier);
    let cost = offering_cost(god_rank, inputs.div_cost_after3, inputs.tier);
    let free_chance = (inputs.y4 / 100.0).clamp(0.0, MAX_FREE_CHANCE);
    let confidence = inputs
        .planning_confidence
        .clamp(MIN_CONFIDENCE, MAX_CONFIDENCE);
    let attempts = attempts(odds, confidence);
    Offer {
        odds,
        confidence,
        cost,
        free_chance,
        attempts,
        expected_spend: cost * (1.0 - free_chance) * (1.0 - (1.0 - odds).powi(attempts as i32))
            / odds,
        max_paid_spend: f64::from(attempts) * cost,
    }
}

/// `offeringStats`: the attempts a confidence needs at these odds.
fn attempts(odds: f64, confidence: f64) -> u32 {
    if odds >= 1.0 {
        1
    } else if odds > 0.0 {
        ((1.0 - confidence).ln() / (1.0 - odds).ln()).ceil() as u32
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_odds_table_stops_at_the_highest_tier() {
        assert_eq!(offering_odds(0), 0.01);
        assert_eq!(offering_odds(1), 0.05);
        assert_eq!(offering_odds(5), 1.0);
        assert_eq!(offering_odds(9), 1.0);
    }

    #[test]
    fn a_cost_is_floored_at_one_and_ceiled_below_a_million() {
        assert_eq!(offering_cost(0, 0.0, 0), 1.0);
        // Values upstream's `offeringStats` reports at tier 1.
        assert_eq!(offering_cost(0, 0.0, 1), 5.0);
        assert_eq!(offering_cost(1, 0.0, 1), 18.0);
    }

    #[test]
    fn div_cost_after3_compounds_only_from_rank_three() {
        assert_eq!(offering_cost(2, 40.0, 1), offering_cost(2, 0.0, 1));
        assert!(offering_cost(5, 40.0, 1) > offering_cost(5, 0.0, 1));
        assert_eq!(
            offering_cost(6, 1_000.0, 1),
            offering_cost(6, 80.0, 1),
            "the growth caps at 80%"
        );
    }

    #[test]
    fn more_confidence_needs_more_attempts() {
        let low = Inputs {
            planning_confidence: 0.5,
            ..Inputs::default()
        };
        let high = Inputs {
            planning_confidence: 0.99,
            ..Inputs::default()
        };
        assert!(offer(&high, 10).attempts > offer(&low, 10).attempts);
    }

    #[test]
    fn the_free_chance_follows_y4_and_caps_at_35_percent() {
        let inputs = Inputs {
            y4: 100.0,
            ..Inputs::default()
        };
        assert_eq!(offer(&inputs, 0).free_chance, 0.35);
        assert!(offer(&inputs, 0).expected_spend > 0.0);
        assert!(offer(&inputs, 0).max_paid_spend >= offer(&inputs, 0).expected_spend);
    }
}
