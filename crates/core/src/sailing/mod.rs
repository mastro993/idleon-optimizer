//! Sailing fleet assignment: boats, captains, routes and the one-to-one
//! optimizer.
//!
//! The formulas follow the upstream `sailing-chests.js` and `sailing.js`:
//! boat base loot and speed, captain bonuses, chest tiers, artifact odds and
//! route cadence. Account-wide multipliers live in [`RouteContext`]; the save
//! import fills the ones it can read and leaves the rest neutral, so the
//! assignment ranking stays exact even when a bonus is not modelled.
//!
//! A route's captain is the only assignment variable. Everything else — boat
//! levels, island, account bonuses — is fixed per boat, which is what lets the
//! one-captain-per-boat maximum-weight matching be exact.

mod assign;
mod captain;
pub mod data;
mod metrics;
mod outcome;
mod replacement;
mod tree;
mod upgrade;

pub use assign::{FleetComparison, FleetPlan, compare, optimize, saved};
pub use captain::{
    CaptainExpMultiplier, CaptainLevelTime, RollBounds, RollQuality, exp_multiplier, exp_per_trip,
    exp_requirement, level_time, roll_bounds, roll_quality,
};
pub use data::{
    artifact, artifact_base, artifact_base_odds, artifact_count, artifact_tier_bonus,
    captain_bonus_name, island, island_artifact_count, island_artifact_offset, island_captain_exp,
    island_cloud_required, island_count, island_distance, sailing,
};
pub use metrics::{
    ArtifactMultiplier, ArtifactOdds, ArtifactOutcome, ChestDistribution, ChestTier, LootValue,
    RouteMetrics, RouteTiming, SpeedValue, TierRow, TreasureShares, artifact_odds_denominator,
    artifact_tier_unlocked, boat_artifact_multiplier, boat_base_loot, boat_base_speed,
    boat_loot_value, boat_speed, captain_bonus, chest_artifact_multiplier, chest_loot_multiplier,
    chest_loot_rolls, chest_rarity_scale, chest_tier_distribution, davey_jones_bonus,
    island_artifact_odds, minimum_travel_details, route_metrics, route_timing,
};
pub use outcome::{
    ChestPayoutBreakdown, CurrencyPayout, CurrencyPayouts, FleetChestOutcomes, FleetChestTier,
    MAX_ARTIFACT_TIER, NearFullTier, aggregate_fleet_tiers, fleet_chest_payout_breakdown,
    incomplete_artifacts, max_artifact_tier, near_full_pile,
};
pub use replacement::{
    ReplacementOutcome, ReplacementThreshold, compare_strength, overflow_upgrade_chance, ranked,
    replacement_outcome, replacement_threshold, strongest,
};
pub use tree::{artifact_tree, chest_tree, loot_tree, speed_tree};
pub use upgrade::{
    Currency, CurrencyShare, UpgradeBreakpoint, UpgradeCost, UpgradeEta, UpgradeStat, currency,
    currency_rate, next_breakpoint, upgrade_breakpoint, upgrade_cost, upgrade_cost_at_level,
    upgrade_cost_type, upgrade_eta,
};

/// A captain as the save stores one.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Captain {
    /// Rarity tier, `0` through `6`; `6` is legendary.
    pub rarity: u32,
    /// The two bonus types the captain rolled; negative means none.
    pub bonus_types: [i32; 2],
    pub level: u32,
    /// Experience banked toward the next level.
    pub exp: f64,
    /// The rolled value for each bonus type, as a percentage.
    pub rolls: [f64; 2],
}

impl Captain {
    pub fn is_legendary(&self) -> bool {
        self.rarity == 6
    }
}

/// A boat as the save stores one.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Boat {
    /// The captain the save has on this boat, if any.
    pub captain: Option<usize>,
    /// The destination the save has for this boat, if any.
    pub target_island: Option<usize>,
    pub loot_level: u32,
    pub speed_level: u32,
}

/// The active fleet and the artifact tiers behind its odds.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Fleet {
    pub boats: Vec<Boat>,
    pub captains: Vec<Captain>,
    /// The active character's Sailing level, which sets the treasure split.
    pub sailing_level: u32,
    /// One tier per artifact, `0` for an artifact not found yet.
    pub artifact_tiers: Vec<u32>,
    /// `Sailing[0]`: each island's cloud progress; negative means discovered.
    pub island_progress: Vec<f64>,
    /// `Sailing[1]`: the banked amount of each upgrade currency.
    pub currency_balances: Vec<f64>,
}

impl Fleet {
    /// How many captains the fleet has at legendary rarity, capped at 30 as
    /// the upstream loot and artifact bonuses cap it.
    pub fn legendary_captains(&self) -> usize {
        self.captains
            .iter()
            .filter(|captain| captain.is_legendary())
            .count()
            .min(30)
    }

    /// The artifact tier for `idx`, or zero when the save lacks it.
    pub fn artifact_tier(&self, idx: usize) -> u32 {
        self.artifact_tiers.get(idx).copied().unwrap_or(0)
    }

    /// The banked amount of upgrade currency `kind`, or zero when the save lacks it.
    pub fn currency_balance(&self, kind: usize) -> f64 {
        self.currency_balances.get(kind).copied().unwrap_or(0.0)
    }

    /// How many islands the save has unlocked, matching upstream
    /// `availableIslandCount`: the first island, any negative progress, or progress
    /// at least the island's cloud requirement.
    pub fn available_islands(&self) -> usize {
        (0..island_count())
            .filter(|island| {
                if *island == 0 {
                    return true;
                }
                let progress = self.island_progress.get(*island).copied().unwrap_or(0.0);
                progress < 0.0 || progress >= island_cloud_required(*island)
            })
            .count()
    }
}

/// The travel-time reductions the save carries.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MinimumTravel {
    /// Siege Breaker family bonus, in percent.
    pub family: f64,
    /// Shiny breeding pets, in percent.
    pub shiny: f64,
    /// Davey Jones Returns legend points, in percent.
    pub legend: f64,
    /// Davey Jones Training gem purchases.
    pub gem_purchases: f64,
}

impl MinimumTravel {
    /// The combined percentage reduction.
    pub fn total_percent(&self) -> f64 {
        self.family + self.shiny + self.legend
    }

    /// The minutes the shortest trip can take, floored at 15.
    pub fn minutes(&self) -> f64 {
        let before_gem = 120.0 / (1.0 + self.total_percent() / 100.0);
        let after_gem = before_gem - 4.0 * self.gem_purchases;
        after_gem.max(15.0).round()
    }
}

/// Account-wide loot bonuses. `additive` is the percentage pool; `multiplier`
/// is the product of every multiplicative source.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LootBonuses {
    pub additive: f64,
    pub multiplier: f64,
}

impl Default for LootBonuses {
    fn default() -> Self {
        Self {
            additive: 0.0,
            multiplier: 1.0,
        }
    }
}

/// Account-wide speed bonuses, as the upstream `boatSpeed` factors.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpeedBonuses {
    /// `1 + (divinity + cards + bubble) / 125`.
    pub first: f64,
    pub bless4: f64,
    pub bless6: f64,
    pub vote: f64,
    /// `1 + (bless9 + artifact + stamp + statue + meal + vial + rift + msa +
    /// star + vault) / 125`.
    pub second: f64,
}

impl Default for SpeedBonuses {
    fn default() -> Self {
        Self {
            first: 1.0,
            bless4: 0.0,
            bless6: 0.0,
            vote: 0.0,
            second: 1.0,
        }
    }
}

/// Account-wide captain-EXP bonuses, split the way upstream sums and
/// multiplies them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CaptainExpBonuses {
    /// Talents, achievements, shiny pets and the vault, in percent.
    pub additive: f64,
    /// The product of the vote, companion, sushi and Nightmariner factors.
    pub multiplier: f64,
}

impl Default for CaptainExpBonuses {
    fn default() -> Self {
        Self {
            additive: 0.0,
            multiplier: 1.0,
        }
    }
}

/// Account-wide artifact-find bonuses, split the way upstream sums and
/// multiplies them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ArtifactBonuses {
    /// Every additive source except the captain's own bonus.
    pub additive: f64,
    /// The product of the multiplicative sources.
    pub multiplier: f64,
}

impl Default for ArtifactBonuses {
    fn default() -> Self {
        Self {
            additive: 0.0,
            multiplier: 1.0,
        }
    }
}

/// The ancient-artifact server constants.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ServerVars {
    pub ancient_odd_per_island: f64,
    pub ancient_arti_pct: f64,
}

impl Default for ServerVars {
    fn default() -> Self {
        Self {
            ancient_odd_per_island: 960.0,
            ancient_arti_pct: 0.0,
        }
    }
}

/// The save unlocks that gate artifact tiers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ArtifactUnlocks {
    /// Rift 30, which unlocks tier two.
    pub rift: bool,
    /// The ninja code that unlocks tier three.
    pub ninja: bool,
    /// Spelunking episode one, which unlocks tier four.
    pub spelunk: bool,
    /// Research grid 109, which unlocks tier five.
    pub grid_109: bool,
    /// Menagerie unlock 0, which unlocks tier six.
    pub verisimilitude: bool,
}

/// Everything a route reads besides the fleet itself.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RouteContext {
    /// The character whose Sailing level and bonuses apply.
    pub active_character: usize,
    /// Chests below this tier are left in the pile.
    pub minimum_tier: u32,
    pub travel: MinimumTravel,
    pub loot: LootBonuses,
    pub speed: SpeedBonuses,
    pub artifact: ArtifactBonuses,
    pub server_vars: ServerVars,
    pub unlocks: ArtifactUnlocks,
    /// Research grid 105: Davey Jones is active once a boat reaches level 400.
    pub davey_unlocked: bool,
    /// Extra chest loot rolls from achievement 300.
    pub chest_rolls: u32,
    pub captain_exp: CaptainExpBonuses,
    /// Unending Loot Search (talent 325) as a percentage. It scales chest
    /// loot, and zero turns near-full replacement off as upstream does
    /// without the talent.
    pub unending_loot_search: f64,
}

/// What the optimizer maximises.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Goal {
    /// Direct treasure value per hour.
    #[default]
    Treasure,
    /// Expected artifact finds per hour.
    Artifact,
    /// Chests kept per hour at the minimum tier.
    Chests,
}

impl Goal {
    pub const ALL: [Goal; 3] = [Goal::Treasure, Goal::Artifact, Goal::Chests];

    pub fn id(self) -> &'static str {
        match self {
            Goal::Treasure => "treasure",
            Goal::Artifact => "artifact",
            Goal::Chests => "chests",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Goal::Treasure => "Loot value",
            Goal::Artifact => "Artifacts",
            Goal::Chests => "Chest rate",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|goal| goal.id() == id)
    }
}

/// Whether the table shows the save's pairings or the optimizer's.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AssignmentMode {
    #[default]
    Optimized,
    Saved,
}

impl AssignmentMode {
    pub const ALL: [AssignmentMode; 2] = [AssignmentMode::Optimized, AssignmentMode::Saved];

    pub fn id(self) -> &'static str {
        match self {
            AssignmentMode::Optimized => "optimized",
            AssignmentMode::Saved => "saved",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            AssignmentMode::Optimized => "Optimized",
            AssignmentMode::Saved => "Current save",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|mode| mode.id() == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_minimum_trip_floor_and_gem_discount_match_upstream() {
        let travel = MinimumTravel {
            family: 20.0,
            shiny: 10.0,
            legend: 15.0,
            gem_purchases: 8.0,
        };
        // 120 / 1.45 = 82.76, minus 32 = 50.76, rounded to 51.
        assert_eq!(travel.minutes(), 51.0);
        assert_eq!(
            MinimumTravel {
                gem_purchases: 100.0,
                ..MinimumTravel::default()
            }
            .minutes(),
            15.0
        );
    }

    #[test]
    fn goals_round_trip_through_their_ids() {
        for goal in Goal::ALL {
            assert_eq!(Goal::from_id(goal.id()), Some(goal));
        }
        for mode in AssignmentMode::ALL {
            assert_eq!(AssignmentMode::from_id(mode.id()), Some(mode));
        }
    }
}
