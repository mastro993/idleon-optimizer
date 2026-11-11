//! The account multipliers the plot formulas read, from the save terms this
//! port carries.

use super::{Farm, RANK_UPGRADES, data};

impl Farm {
    /// Crop ids at or above each GMO threshold.
    pub fn crop_threshold_counts(&self) -> [f64; 5] {
        let mut counts = [0.0; 5];
        for quantity in self.crop_quantities.values() {
            for (index, threshold) in data().gmo_thresholds.iter().enumerate() {
                if *quantity >= *threshold {
                    counts[index] += 1.0;
                }
            }
        }
        counts
    }

    /// `basketUpgrade(page, idx)` from the planner: a market effect in its
    /// percent or multiplier form, with `page = 99` reading the Night market.
    pub fn basket_upgrade(&self, page: usize, idx: usize) -> f64 {
        let Some(upgrade) = data().market(page, idx) else {
            return 0.0;
        };
        let level = |slot: usize| self.market_levels.get(slot).copied().unwrap_or(0.0);
        match page {
            0 => level(2 + idx) * upgrade.effect_per_level,
            1 => {
                let level = level(10 + idx);
                if idx == 1 || idx == 3 {
                    1.0 + level * upgrade.effect_per_level / 100.0
                } else {
                    level * upgrade.effect_per_level
                }
            }
            99 => {
                let night = level(10 + idx);
                let counts = self.crop_threshold_counts();
                match idx {
                    7 => 1.0 + night * upgrade.effect_per_level * counts[4] / 100.0,
                    1 => {
                        self.basket_upgrade(99, 7).max(1.0)
                            * (1.0 + night * upgrade.effect_per_level / 100.0).powf(counts[0])
                    }
                    5 => {
                        if night >= 1.0 {
                            1.0
                        } else {
                            0.0
                        }
                    }
                    _ => {
                        let count_index = idx / 2 + idx / 7;
                        let count = counts.get(count_index).copied().unwrap_or(0.0);
                        self.basket_upgrade(99, 7).max(1.0)
                            * (1.0 + night * upgrade.effect_per_level * count / 100.0)
                    }
                }
            }
            _ => 0.0,
        }
    }

    /// `ExoticBonusQTY(idx)`: the market level at `farmSlot`, scaled by the
    /// row's linear or decaying formula, doubled by an Organic label and capped
    /// for exotic 39.
    pub fn exotic_bonus(&self, idx: usize) -> f64 {
        let Some(exotic) = data().exotic(idx) else {
            return 0.0;
        };
        let level = self
            .market_levels
            .get(exotic.farm_slot)
            .copied()
            .unwrap_or(0.0);
        if level <= 0.0 {
            return 0.0;
        }
        let mut value = if exotic.decay {
            exotic.base * level / (1000.0 + level)
        } else {
            exotic.base * level
        };
        if self.organic_exotics.contains(&(idx as u32)) {
            value *= 2.0;
        }
        if idx == 39 {
            value = value.min(50.0);
        }
        value
    }

    /// `FarmRankUpgBonusAtLevel(idx, level)`. Talent 207 is not imported, so
    /// its multiplier stays at 1.
    pub fn rank_bonus(&self, idx: usize) -> f64 {
        let level = self.rank_levels.get(idx).copied().unwrap_or(0) as f64;
        if level <= 0.0 {
            return 0.0;
        }
        let Some(base) = data().ninja_rank_bases.get(idx).copied() else {
            return 0.0;
        };
        let exotic = 1.0 + self.exotic_bonus(14) / 100.0;
        if idx % 5 == 4 {
            exotic * base * level
        } else {
            exotic * 1.7 * base * level / (level + 80.0)
        }
    }

    /// The Land Rank totals the planner groups upgrades into.
    pub fn rank_totals(&self) -> RankTotals {
        let bonuses: [f64; RANK_UPGRADES] = std::array::from_fn(|idx| self.rank_bonus(idx));
        RankTotals {
            bonuses,
            evolution: (1.0 + bonuses[3] / 100.0)
                * (1.0 + bonuses[10] / 100.0)
                * (1.0 + bonuses[15] / 100.0),
            crop_value: bonuses[8] + bonuses[17],
            rank_xp: bonuses[6] + bonuses[13],
            overgrowth: bonuses[7] + bonuses[11] + bonuses[18],
            farming_exp: bonuses[5] + bonuses[12] + bonuses[16],
        }
    }

    /// Total Land Rank across the plots (`farmRankData[0]`).
    pub fn total_land_rank(&self) -> u32 {
        self.rank_ranks.iter().sum()
    }

    /// Rank points not spent on the twenty upgrades.
    pub fn land_rank_points_left(&self) -> i64 {
        let spent: u32 = self.rank_levels.iter().sum();
        i64::from(self.total_land_rank()) - i64::from(spent)
    }

    /// How many levels the fifth Land Rank column allows. Grimoire 9 and the
    /// Spelunk legend points are not imported and stay at zero.
    pub fn fifth_column_max(&self) -> u32 {
        1 + self.exotic_bonus(15).ceil() as u32
    }

    /// The account multipliers the plot formulas read, from the save terms
    /// this port carries.
    pub fn snapshot(&self) -> Snapshot {
        let crop_counts = self.crop_threshold_counts();
        let rank = self.rank_totals();
        let mut evolution_exotics = 0.0;
        for idx in 4..=8 {
            let level = f64::from(self.farm_level) - 50.0 * (idx as f64 - 3.0);
            evolution_exotics += level.max(0.0) * self.exotic_bonus(idx);
        }
        let growth_rate = self.basket_upgrade(99, 2).max(1.0)
            * (1.0 + (self.basket_upgrade(0, 2) + self.exotic_bonus(30)) / 100.0);
        let next_crop_common = [
            1.0 + self.basket_upgrade(0, 4) / 100.0,
            self.basket_upgrade(99, 1).max(1.0),
            rank.evolution.max(1.0),
            1.0 + self.exotic_bonus(0) / 100.0,
            1.0 + self.exotic_bonus(1) / 100.0,
            1.0 + self.exotic_bonus(2) / 100.0,
            1.0 + self.exotic_bonus(3) / 100.0,
            1.0 + evolution_exotics / 100.0,
        ]
        .iter()
        .product();
        let overgrowth_common = self.basket_upgrade(1, 3).max(1.0)
            * (1.0 + rank.overgrowth / 100.0)
            * (1.0 + self.exotic_bonus(26) / 100.0)
            * (1.0 + self.exotic_bonus(27) / 100.0);
        let farming_exp = self.basket_upgrade(99, 4).max(1.0)
            * (1.0 + (self.basket_upgrade(0, 3) + self.exotic_bonus(21)) / 100.0)
            * (1.0 + self.exotic_bonus(22) / 100.0)
            * (1.0 + rank.farming_exp / 100.0);
        let bean_multiplier = (1.0 + self.basket_upgrade(0, 6) / 100.0)
            * (1.0
                + (self.exotic_bonus(16) + self.exotic_bonus(17) + self.exotic_bonus(18)) / 100.0)
            * (1.0 + self.exotic_bonus(19) / 100.0)
            * (1.0 + self.exotic_bonus(20) / 100.0);
        let crop_value_cap = 10_000.0
            * (1.0
                + (self.exotic_bonus(23) + self.exotic_bonus(24) + self.exotic_bonus(25)) / 100.0);
        let product_bonus =
            self.basket_upgrade(0, 5) + self.exotic_bonus(28) + self.exotic_bonus(29);
        let gem_139 = self.gem_items.get(139).copied().unwrap_or(0.0);
        let vine_bonus = self.basket_upgrade(0, 1)
            + 20.0 * gem_139
            + self.exotic_bonus(31)
            + self.exotic_bonus(32)
            + self.exotic_bonus(33);
        let plot_rank_display = (1.0
            + (self.basket_upgrade(0, 7)
                + self.exotic_bonus(9)
                + self.exotic_bonus(10)
                + self.exotic_bonus(11))
                / 100.0)
            * (1.0 + self.exotic_bonus(12) / 100.0)
            * (1.0 + self.exotic_bonus(13) / 100.0);
        Snapshot {
            farm_level: self.farm_level,
            crop_counts,
            rank,
            growth_rate,
            next_crop_common,
            overgrowth_common,
            farming_exp,
            bean_multiplier,
            crop_value_cap,
            product_bonus,
            expected_vines: 1.0 + vine_bonus / 100.0,
            plot_rank_display,
            voting29: 0.0,
        }
    }
}

/// The account multipliers the plot formulas use.
#[derive(Clone, Debug)]
pub struct Snapshot {
    pub farm_level: u32,
    pub crop_counts: [f64; 5],
    pub rank: RankTotals,
    pub growth_rate: f64,
    /// Global evolution multiplier before the per-plot rank.
    pub next_crop_common: f64,
    pub overgrowth_common: f64,
    pub farming_exp: f64,
    pub bean_multiplier: f64,
    pub crop_value_cap: f64,
    pub product_bonus: f64,
    pub expected_vines: f64,
    pub plot_rank_display: f64,
    /// Voting bonus 29; the voting system is not imported.
    pub voting29: f64,
}

/// The Land Rank bonuses grouped by what they modify.
#[derive(Clone, Debug)]
pub struct RankTotals {
    pub bonuses: [f64; RANK_UPGRADES],
    pub evolution: f64,
    pub crop_value: f64,
    pub rank_xp: f64,
    pub overgrowth: f64,
    pub farming_exp: f64,
}
