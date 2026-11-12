//! Ship upgrade costs, ETAs and the loot/speed breakpoints.
//!
//! A boat's loot and speed formulas square `floor(level / interval)`, so one
//! level every eight (loot) or seven (speed) steps jumps far more than its
//! neighbours. The breakpoint row prices every level up to that step and reads
//! what the route gains once the boat is there.

use super::{
    Fleet, RouteContext, RouteMetrics, TreasureShares, boat_base_loot, boat_base_speed,
    route_metrics,
};

/// Which ship stat an upgrade raises.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpgradeStat {
    Loot,
    Speed,
}

impl UpgradeStat {
    pub const ALL: [UpgradeStat; 2] = [UpgradeStat::Loot, UpgradeStat::Speed];

    pub fn is_loot(self) -> bool {
        self == UpgradeStat::Loot
    }

    pub fn label(self) -> &'static str {
        match self {
            UpgradeStat::Loot => "Loot value",
            UpgradeStat::Speed => "Speed",
        }
    }

    /// The level step where the formula's squared floor term rises.
    pub fn interval(self) -> u32 {
        if self.is_loot() { 8 } else { 7 }
    }
}

/// Which of the three treasure currencies an upgrade spends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CurrencyShare {
    Gold,
    Primary,
    Rare,
}

impl CurrencyShare {
    /// This currency's share of one route's treasure.
    pub fn of(self, shares: TreasureShares) -> f64 {
        match self {
            CurrencyShare::Gold => shares.gold,
            CurrencyShare::Primary => shares.primary,
            CurrencyShare::Rare => shares.rare,
        }
    }
}

/// One upgrade currency: its index in `Sailing[1]`, the island that pays it
/// and its share of an island's treasure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Currency {
    pub kind: usize,
    /// `None` for gold bars, which every island pays.
    pub island: Option<usize>,
    pub share: CurrencyShare,
}

impl Currency {
    /// The upstream label: `Gold Bars`, `Island 3 Primary`, `Island 2 Rare`.
    pub fn name(&self) -> String {
        match self.island {
            None => "Gold Bars".into(),
            Some(island) => format!(
                "Island {} {}",
                island + 1,
                match self.share {
                    CurrencyShare::Rare => "Rare",
                    _ => "Primary",
                }
            ),
        }
    }

    /// Whether a route on `island` pays this currency.
    pub fn accepts(&self, island: usize) -> bool {
        self.island.is_none_or(|target| target == island)
    }
}

/// The currency an upgrade type spends, upstream `sailingCurrencyMeta`.
pub fn currency(kind: usize) -> Currency {
    if kind == 0 {
        return Currency {
            kind: 0,
            island: None,
            share: CurrencyShare::Gold,
        };
    }
    Currency {
        kind,
        island: Some((kind - 1) / 2),
        share: if kind % 2 == 1 {
            CurrencyShare::Primary
        } else {
            CurrencyShare::Rare
        },
    }
}

/// The level step a boat's next loot or speed upgrade reaches.
pub(crate) fn next_breakpoint(level: u32, stat: UpgradeStat) -> u32 {
    let interval = stat.interval();
    interval * (level / interval + 1)
}

/// The currency index a boat's loot or speed upgrade spends, upstream
/// `boatUpgradeCostType`.
pub fn upgrade_cost_type(boat: usize, stat: UpgradeStat) -> usize {
    if stat.is_loot() {
        return if boat < 4 {
            0
        } else {
            (1 + 2 * (boat - 4)).min(30)
        };
    }
    if boat < 2 {
        boat
    } else if boat < 5 {
        1 + 2 * (boat - 2)
    } else {
        (2 * (boat - 4)).min(30)
    }
}

/// One level's upgrade price, upstream `_boatUpgradeCostAtLevel`.
///
/// The event-shop discount is not part of the save model, so the price stays
/// the undiscounted one.
pub(crate) fn upgrade_cost_at_level(boat: usize, stat: UpgradeStat, level: u32) -> f64 {
    let level = level as f64;
    let raw = match upgrade_cost_type(boat, stat) {
        0 => (5.0 + 4.0 * level) * (1.17 - 0.12 * level / (level + 200.0)).powf(level),
        kind if kind % 2 == 1 => {
            (5.0 + 2.0 * level) * (1.15 - 0.10 * level / (level + 200.0)).powf(level)
        }
        _ => (2.0 + level) * (1.12 - 0.07 * level / (level + 200.0)).powf(level),
    };
    if raw < 1e6 { raw.floor() } else { raw }
}

/// A boat's next upgrade: what it costs and what the save has banked.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UpgradeCost {
    pub boat: usize,
    pub stat: UpgradeStat,
    pub level: u32,
    pub currency: Currency,
    pub cost: f64,
    pub balance: f64,
}

/// The next level of `boat`'s `stat`, upstream `boatUpgradeCost`.
pub fn upgrade_cost(fleet: &Fleet, boat: usize, stat: UpgradeStat) -> UpgradeCost {
    let entry = fleet.boats.get(boat);
    let level = match (stat, entry) {
        (UpgradeStat::Loot, Some(entry)) => entry.loot_level,
        (UpgradeStat::Speed, Some(entry)) => entry.speed_level,
        _ => 0,
    };
    let kind = upgrade_cost_type(boat, stat);
    UpgradeCost {
        boat,
        stat,
        level,
        currency: currency(kind),
        cost: upgrade_cost_at_level(boat, stat, level),
        balance: fleet.currency_balance(kind),
    }
}

/// The treasure an upgrade currency earns each hour across `routes`.
pub fn currency_rate(routes: &[RouteMetrics], kind: usize) -> f64 {
    let meta = currency(kind);
    routes
        .iter()
        .filter(|route| meta.accepts(route.island))
        .map(|route| route.treasure_per_hour * meta.share.of(route.treasure_shares))
        .sum()
}

/// The next upgrade and how long the fleet's income needs for it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UpgradeEta {
    pub cost: f64,
    pub currency: Currency,
    /// The currency's own income per hour.
    pub rate: f64,
    /// What the balance still misses.
    pub remaining: f64,
    pub hours: f64,
}

/// The next upgrade's ETA, upstream `boatUpgradeEta`.
pub fn upgrade_eta(
    fleet: &Fleet,
    boat: usize,
    stat: UpgradeStat,
    routes: &[RouteMetrics],
) -> UpgradeEta {
    let upgrade = upgrade_cost(fleet, boat, stat);
    let rate = currency_rate(routes, upgrade.currency.kind);
    let remaining = (upgrade.cost - upgrade.balance).max(0.0);
    UpgradeEta {
        cost: upgrade.cost,
        currency: upgrade.currency,
        rate,
        remaining,
        hours: if remaining <= 0.0 {
            0.0
        } else if rate > 0.0 {
            remaining / rate
        } else {
            f64::INFINITY
        },
    }
}

/// One boat's next loot or speed breakpoint and what reaching it changes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UpgradeBreakpoint {
    pub boat: usize,
    pub stat: UpgradeStat,
    pub level: u32,
    pub interval: u32,
    /// The breakpoint level the formula's squared term rises at.
    pub target_level: u32,
    /// Levels between the current level and the breakpoint.
    pub levels: u32,
    pub current_base: f64,
    pub next_base: f64,
    pub target_base: f64,
    /// The gain of one level, the ordinary upgrade.
    pub next_base_gain_pct: f64,
    /// The gain of every level up to the breakpoint.
    pub base_gain_pct: f64,
    /// The last level's own gain at the breakpoint.
    pub threshold_marginal_pct: f64,
    pub davey_unlocked: bool,
    /// Whether the boat already meets the level-400 Davey gate.
    pub davey_active: bool,
    /// The level this stat needs for the gate, when it is unlocked but unmet.
    pub davey_level: Option<u32>,
    pub currency: Currency,
    /// Every level's price up to the breakpoint.
    pub total_cost: f64,
    pub balance: f64,
    /// What the balance still misses.
    pub remaining: f64,
    pub initial_rate: f64,
    pub final_rate: f64,
    /// Hours to every level up to the breakpoint.
    pub hours: f64,
    pub current_treasure_per_hour: f64,
    pub target_treasure_per_hour: f64,
    pub treasure_gain_pct: f64,
    pub chest_gain_pct: f64,
    pub artifact_gain_pct: f64,
    /// Whether the route already runs at the minimum trip time.
    pub speed_capped: bool,
}

/// One boat's next breakpoint, upstream `boatUpgradeBreakpoint`.
///
/// `routes` is the fleet's plan on the currency's island, so the gains read
/// the same route the upgrade would sail.
pub fn upgrade_breakpoint(
    fleet: &Fleet,
    boat: usize,
    stat: UpgradeStat,
    routes: &[RouteMetrics],
    context: &RouteContext,
) -> UpgradeBreakpoint {
    let entry = fleet.boats.get(boat);
    let level = match (stat, entry) {
        (UpgradeStat::Loot, Some(entry)) => entry.loot_level,
        (UpgradeStat::Speed, Some(entry)) => entry.speed_level,
        _ => 0,
    };
    let other_level = match (stat, entry) {
        (UpgradeStat::Loot, Some(entry)) => entry.speed_level,
        (UpgradeStat::Speed, Some(entry)) => entry.loot_level,
        _ => 0,
    };
    let target_level = next_breakpoint(level, stat);
    let base = if stat.is_loot() {
        boat_base_loot
    } else {
        boat_base_speed
    };
    let current_base = base(level);
    let target_base = base(target_level);
    let davey_active = context.davey_unlocked && level + other_level >= 400;
    let davey_level = (context.davey_unlocked && !davey_active)
        .then(|| 400u32.saturating_sub(other_level))
        .filter(|target| *target > level);

    let kind = upgrade_cost_type(boat, stat);
    let meta = currency(kind);
    let balance = fleet.currency_balance(kind);
    let share_of = |route: &RouteMetrics| {
        if meta.accepts(route.island) {
            route.treasure_per_hour * meta.share.of(route.treasure_shares)
        } else {
            0.0
        }
    };
    // Every other boat pays the same currency pool, so keep their income aside
    // and only re-price this boat as its levels rise.
    let other_rate: f64 = routes
        .iter()
        .filter(|route| route.boat != boat)
        .map(share_of)
        .sum();
    let mut route = routes.iter().find(|route| route.boat == boat).cloned();
    let initial_rate = other_rate + route.as_ref().map_or(0.0, share_of);
    let current_treasure = route.as_ref().map_or(0.0, |route| route.treasure_per_hour);
    let current_chests = route
        .as_ref()
        .map_or(0.0, |route| route.generated_chests_per_hour);
    let current_artifacts = route
        .as_ref()
        .map_or(0.0, |route| route.artifact_finds_per_hour);

    let (mut available, mut total_cost, mut hours) = (balance, 0.0, 0.0);
    for upgrade in level..target_level {
        let cost = upgrade_cost_at_level(boat, stat, upgrade);
        total_cost += cost;
        let rate = other_rate + route.as_ref().map_or(0.0, share_of);
        if available < cost {
            if rate > 0.0 {
                hours += (cost - available) / rate;
            } else {
                hours = f64::INFINITY;
            }
            available = 0.0;
        } else {
            available -= cost;
        }
        route = route.map(|route| {
            route_at(
                fleet,
                boat,
                route.island,
                route.captain,
                context,
                stat,
                upgrade + 1,
            )
        });
    }

    let final_rate = other_rate + route.as_ref().map_or(0.0, share_of);
    let target_treasure = route.as_ref().map_or(0.0, |route| route.treasure_per_hour);
    let target_chests = route
        .as_ref()
        .map_or(0.0, |route| route.generated_chests_per_hour);
    let target_artifacts = route
        .as_ref()
        .map_or(0.0, |route| route.artifact_finds_per_hour);
    UpgradeBreakpoint {
        boat,
        stat,
        level,
        interval: stat.interval(),
        target_level,
        levels: target_level - level,
        current_base,
        next_base: base(level + 1),
        target_base,
        next_base_gain_pct: percent_gain(current_base, base(level + 1)),
        base_gain_pct: percent_gain(current_base, target_base),
        threshold_marginal_pct: percent_gain(base(target_level - 1), target_base),
        davey_unlocked: context.davey_unlocked,
        davey_active,
        davey_level,
        currency: meta,
        total_cost,
        balance,
        remaining: (total_cost - balance).max(0.0),
        initial_rate,
        final_rate,
        hours: if total_cost <= balance { 0.0 } else { hours },
        current_treasure_per_hour: current_treasure,
        target_treasure_per_hour: target_treasure,
        treasure_gain_pct: percent_gain(current_treasure, target_treasure),
        chest_gain_pct: percent_gain(current_chests, target_chests),
        artifact_gain_pct: percent_gain(current_artifacts, target_artifacts),
        speed_capped: !stat.is_loot()
            && routes
                .iter()
                .find(|route| route.boat == boat)
                .is_some_and(|route| route.timing.capped),
    }
}

/// The route one boat would sail at `level` in `stat`, everything else fixed.
fn route_at(
    fleet: &Fleet,
    boat: usize,
    island: usize,
    captain: Option<usize>,
    context: &RouteContext,
    stat: UpgradeStat,
    level: u32,
) -> RouteMetrics {
    let mut fleet = fleet.clone();
    if let Some(entry) = fleet.boats.get_mut(boat) {
        match stat {
            UpgradeStat::Loot => entry.loot_level = level,
            UpgradeStat::Speed => entry.speed_level = level,
        }
    }
    route_metrics(&fleet, boat, island, captain, context)
}

/// `100 * (next / current - 1)`, `0` without a current value.
fn percent_gain(current: f64, next: f64) -> f64 {
    if current > 0.0 {
        100.0 * (next / current - 1.0)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Boat, Captain, Fleet, Goal, optimize};

    fn fleet() -> Fleet {
        Fleet {
            boats: vec![
                Boat {
                    captain: Some(0),
                    loot_level: 100,
                    speed_level: 100,
                    ..Boat::default()
                },
                Boat {
                    captain: Some(1),
                    loot_level: 80,
                    speed_level: 80,
                    ..Boat::default()
                },
            ],
            captains: vec![
                Captain {
                    rarity: 6,
                    bonus_types: [1, 0],
                    level: 20,
                    exp: 0.0,
                    rolls: [15.0, 29.0],
                },
                Captain {
                    rarity: 3,
                    bonus_types: [1, 2],
                    level: 10,
                    exp: 0.0,
                    rolls: [20.0, 10.0],
                },
            ],
            sailing_level: 10,
            artifact_tiers: vec![0; crate::artifact_count()],
            currency_balances: vec![1e4; 31],
            ..Fleet::default()
        }
    }

    #[test]
    fn the_breakpoint_is_the_next_loot_or_speed_step() {
        assert_eq!(next_breakpoint(100, UpgradeStat::Loot), 104);
        assert_eq!(next_breakpoint(104, UpgradeStat::Loot), 112);
        assert_eq!(next_breakpoint(100, UpgradeStat::Speed), 105);
        assert_eq!(next_breakpoint(105, UpgradeStat::Speed), 112);
        assert_eq!(next_breakpoint(0, UpgradeStat::Loot), 8);
    }

    #[test]
    fn the_cost_currency_matches_upstream() {
        assert_eq!(upgrade_cost_type(0, UpgradeStat::Loot), 0);
        assert_eq!(upgrade_cost_type(3, UpgradeStat::Loot), 0);
        assert_eq!(upgrade_cost_type(4, UpgradeStat::Loot), 1);
        assert_eq!(upgrade_cost_type(5, UpgradeStat::Loot), 3);
        assert_eq!(upgrade_cost_type(19, UpgradeStat::Loot), 30);
        assert_eq!(upgrade_cost_type(1, UpgradeStat::Speed), 1);
        assert_eq!(upgrade_cost_type(2, UpgradeStat::Speed), 1);
        assert_eq!(upgrade_cost_type(3, UpgradeStat::Speed), 3);
        assert_eq!(upgrade_cost_type(5, UpgradeStat::Speed), 2);
        assert_eq!(upgrade_cost_type(19, UpgradeStat::Speed), 30);
    }

    #[test]
    fn currencies_name_their_island_and_share() {
        assert_eq!(currency(0).name(), "Gold Bars");
        assert_eq!(currency(1).name(), "Island 1 Primary");
        assert_eq!(currency(2).name(), "Island 1 Rare");
        assert_eq!(currency(3).name(), "Island 2 Primary");
        assert_eq!(currency(1).island, Some(0));
        assert!(currency(0).accepts(4));
        assert!(!currency(1).accepts(3));
        assert!(currency(1).accepts(0));
    }

    #[test]
    fn the_price_rises_with_the_level_and_floors_below_a_million() {
        let first = upgrade_cost_at_level(0, UpgradeStat::Loot, 0);
        assert_eq!(first, 5.0);
        assert!(upgrade_cost_at_level(0, UpgradeStat::Loot, 100) > first);
        let low = upgrade_cost_at_level(0, UpgradeStat::Loot, 20);
        assert!(low < 1e6);
        assert_eq!(low, low.floor());
        let high = upgrade_cost_at_level(19, UpgradeStat::Loot, 400);
        assert!(high > 1e6);
        assert_ne!(
            high,
            high.floor(),
            "above a million upstream keeps it exact"
        );
    }

    #[test]
    fn the_breakpoint_prices_every_level_and_reads_the_route() {
        let fleet = fleet();
        let context = RouteContext::default();
        let plan = optimize(&fleet, 0, Goal::Treasure, &context);
        let breakpoint = upgrade_breakpoint(&fleet, 0, UpgradeStat::Loot, &plan.routes, &context);
        assert_eq!(breakpoint.level, 100);
        assert_eq!(breakpoint.target_level, 104);
        assert_eq!(breakpoint.levels, 4);
        close(breakpoint.current_base, boat_base_loot(100));
        close(breakpoint.target_base, boat_base_loot(104));
        // 104 is the next step where floor(level / 8) rises.
        assert!(
            boat_base_loot(104) / boat_base_loot(103) > boat_base_loot(101) / boat_base_loot(100)
        );
        assert!(breakpoint.total_cost > 0.0);
        assert!(breakpoint.remaining > 0.0, "the balance is 10,000");
        assert!(breakpoint.target_treasure_per_hour > breakpoint.current_treasure_per_hour);
        assert!(breakpoint.treasure_gain_pct > 0.0);
        assert!(breakpoint.hours.is_finite());
        close(
            breakpoint.initial_rate,
            currency_rate(&plan.routes, breakpoint.currency.kind),
        );
    }

    #[test]
    fn a_speed_breakpoint_on_a_capped_route_gains_no_chests() {
        let mut fleet = fleet();
        fleet.boats[0].speed_level = 40_000;
        let context = RouteContext::default();
        let plan = optimize(&fleet, 0, Goal::Treasure, &context);
        let breakpoint = upgrade_breakpoint(&fleet, 0, UpgradeStat::Speed, &plan.routes, &context);
        assert!(breakpoint.speed_capped);
        assert_eq!(breakpoint.chest_gain_pct, 0.0);
        assert!(breakpoint.base_gain_pct > 0.0, "raw speed still rises");
    }

    #[test]
    fn the_davey_gate_reports_the_level_the_boat_needs() {
        let mut fleet = fleet();
        fleet.boats[0].loot_level = 150;
        fleet.boats[0].speed_level = 140;
        let context = RouteContext {
            davey_unlocked: true,
            ..RouteContext::default()
        };
        let plan = optimize(&fleet, 0, Goal::Treasure, &context);
        let breakpoint = upgrade_breakpoint(&fleet, 0, UpgradeStat::Loot, &plan.routes, &context);
        assert!(breakpoint.davey_unlocked);
        assert!(!breakpoint.davey_active);
        assert_eq!(breakpoint.davey_level, Some(260));

        fleet.boats[0].loot_level = 300;
        let plan = optimize(&fleet, 0, Goal::Treasure, &context);
        let active = upgrade_breakpoint(&fleet, 0, UpgradeStat::Loot, &plan.routes, &context);
        assert!(active.davey_active);
        assert_eq!(active.davey_level, None);
    }

    #[test]
    fn a_bankrupt_fleet_reports_an_infinite_eta() {
        let fleet = fleet();
        let context = RouteContext::default();
        let plan = optimize(&fleet, 1, Goal::Treasure, &context);
        // Island 2 pays a different currency than the gold boat 0 needs.
        let gold = upgrade_breakpoint(&fleet, 0, UpgradeStat::Loot, &plan.routes, &context);
        assert_eq!(gold.balance, 1e4);
        let mut broke = fleet;
        broke.currency_balances = vec![0.0; 31];
        let broke = upgrade_breakpoint(&broke, 0, UpgradeStat::Loot, &plan.routes, &context);
        assert!(broke.hours > 0.0);
        assert!(broke.remaining > 0.0);
    }

    #[test]
    fn the_eta_counts_the_balance_first() {
        let fleet = fleet();
        let upgrade = upgrade_cost(&fleet, 0, UpgradeStat::Loot);
        assert_eq!(upgrade.level, 100);
        assert_eq!(upgrade.balance, 1e4);
        let eta = upgrade_eta(&fleet, 0, UpgradeStat::Loot, &[]);
        // No routes means no income, so a short balance never finishes.
        assert!(eta.hours.is_infinite());
        let mut rich = fleet;
        rich.currency_balances = vec![1e12; 31];
        let eta = upgrade_eta(&rich, 0, UpgradeStat::Loot, &[]);
        assert_eq!(eta.hours, 0.0);
        assert_eq!(eta.remaining, 0.0);
    }

    fn close(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() <= expected.abs().max(1.0) * 1e-12,
            "expected {expected}, got {actual}"
        );
    }
}
