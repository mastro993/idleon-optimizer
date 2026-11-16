//! The manual simulator: one hand-typed distribution read against the save.
//!
//! The section draws one input per unlocked villager; the simulation sums them,
//! flags a typed total that leaves the save's ("Total must stay the same for
//! fair comparison") and reads each villager's rate against what it earns
//! today. The arithmetic is here; the section owns the inputs.

use super::{Opal, VILLAGERS, relative, whole};

/// A typed distribution and the saved one it is compared with.
#[derive(Clone, Debug, PartialEq)]
pub struct Simulation {
    /// The typed opal counts, in save order, whole and never negative.
    pub opals: [f64; VILLAGERS],
    /// The save's counts, the comparison's other side.
    pub current: [f64; VILLAGERS],
}

impl Opal {
    /// `opals` against the save's own split.
    ///
    /// A typed count is whole and never negative; a decimal never reaches the
    /// rate.
    pub fn simulation(&self, opals: &[f64; VILLAGERS]) -> Simulation {
        Simulation {
            opals: opals.map(whole),
            current: self.opals(),
        }
    }
}

impl Simulation {
    /// The opals the typed distribution spends.
    pub fn spent(&self) -> f64 {
        self.opals.iter().sum()
    }

    /// The opals the save holds.
    pub fn saved_spent(&self) -> f64 {
        self.current.iter().sum()
    }

    /// Whether the typed total matches the save's: the fair comparison, and the
    /// one thing the simulator warns about.
    pub fn balanced(&self) -> bool {
        self.spent() == self.saved_spent()
    }

    /// The rate the villager earns with the typed counts.
    pub fn rate(&self, opal: &Opal, villager: usize) -> f64 {
        opal.exp_rate(villager, &self.opals)
    }

    /// The rate the villager earns in the save.
    pub fn saved_rate(&self, opal: &Opal, villager: usize) -> f64 {
        opal.exp_rate(villager, &self.current)
    }

    /// The rate every unlocked villager earns with the typed counts.
    pub fn total(&self, opal: &Opal) -> f64 {
        opal.total_exp_rate(&self.opals)
    }

    /// The lowest count among the typed counts: what Equal Spread scales with.
    pub fn min_opals(&self, opal: &Opal) -> f64 {
        opal.min_opals(&self.opals)
    }

    /// Equal Spread's percentage at the typed counts.
    pub fn equal_spread(&self, opal: &Opal) -> f64 {
        opal.equal_spread(&self.opals)
    }

    /// The villager's relative change, or `None` when it earned nothing before.
    pub fn change(&self, opal: &Opal, villager: usize) -> Option<f64> {
        relative(self.saved_rate(opal, villager), self.rate(opal, villager))
    }

    /// The total's relative change, or `None` when the save earns nothing.
    pub fn total_change(&self, opal: &Opal) -> Option<f64> {
        relative(opal.total_exp_rate(&self.current), self.total(opal))
    }
}
