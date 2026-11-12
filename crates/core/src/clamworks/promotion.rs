//! The promotion tracker and the worker-class rewards.
//!
//! Ports `promotionLevel`, `promotionCost`, `promotionChance`,
//! `expectedPearlsPerPromotion`, `clamHP` and `clamWorkBonus` from
//! `js/stats/systems/w7/clamworks.js`, and the rewards table from the promotion
//! tab of `clamworks-calc.html` (`Spelunky[30]`).
//!
//! A reward unlocks at the worker-class level it names: `clamWorkBonus(i)` is 1
//! only once the saved promotion level is past `i`.

use super::Clamworks;
use crate::data;

/// One worker-class reward and whether the saved level has unlocked it.
#[derive(Clone, Debug, PartialEq)]
pub struct Reward {
    /// The worker-class level that unlocks it, 1-based.
    pub level: u32,
    /// The upstream description with its underscores read as spaces.
    pub description: String,
    pub unlocked: bool,
}

/// The promotion tracker at the saved worker-class level.
#[derive(Clone, Debug, PartialEq)]
pub struct Promotion {
    /// The saved promotion level (`OLA[464]`).
    pub level: u32,
    /// The saved pearl balance, for the affordability line.
    pub balance: f64,
    /// `0.5 / (2 + level)`.
    pub chance: f64,
    /// `1e5 × 10^level` pearls per attempt.
    pub cost: f64,
    /// `ceil(1 / chance)`: the attempts one promotion takes on average.
    pub expected_attempts: u32,
    /// `cost / chance`: the pearls one promotion costs on average.
    pub expected_cost: f64,
    /// `1e16 × 30^(level + 1)`: clam HP after a successful promotion.
    pub clam_hp_after: f64,
    /// Every worker-class reward, in level order.
    pub rewards: Vec<Reward>,
}

impl Promotion {
    /// Whether the saved balance covers one attempt.
    pub fn affordable(&self) -> bool {
        self.balance >= self.cost
    }

    /// The pearls a balance short of one attempt still needs.
    pub fn shortfall(&self) -> f64 {
        (self.cost - self.balance).max(0.0)
    }

    /// How many rewards the saved level has unlocked.
    pub fn unlocked(&self) -> usize {
        self.rewards.iter().filter(|reward| reward.unlocked).count()
    }
}

impl Clamworks {
    /// The promotion tracker at this state, with every worker-class reward.
    pub fn promotion(&self) -> Promotion {
        let chance = self.promotion_chance();
        let rewards = data::clamworks()
            .promotion
            .iter()
            .enumerate()
            .map(|(index, description)| {
                let level = index as u32 + 1;
                Reward {
                    level,
                    description: description.replace('_', " "),
                    unlocked: self.promotion_level >= level,
                }
            })
            .collect();
        Promotion {
            level: self.promotion_level,
            balance: self.pearl_balance,
            chance,
            cost: self.promotion_cost(),
            expected_attempts: (1.0 / chance).ceil() as u32,
            expected_cost: self.promotion_cost() / chance,
            clam_hp_after: self.clam_hp_at(self.promotion_level + 1),
            rewards,
        }
    }
}
