//! The upgrade list, the DR priority and the budgeted plan.
//!
//! Ports the Upgrades and Optimizer tabs of `cglunko-calc.html` and the
//! `upgNextCost`, `drMarginalGain` and `rankUpgrades` functions of
//! `js/stats/systems/w5/cglunko.js`. Each of the two pages is paid with its own
//! six stones: an upgrade's cost comes out of the stone its index maps to
//! (`floor(index / 2)`), which is what the upstream progression planner spends.
//!
//! The plan itself is this app's addition (the upstream page only ranks single
//! upgrades): repeat the ranking at the levels and balances each purchase
//! reaches, so every step pays the price `upgNextCost` would charge there and
//! never overdraws a stone. A purchase that spends a stone past a digit would
//! lower the drop rate that reads it; the plan skips it, the same guard the
//! upstream progression planner applies with `dr > drFarm`.

use super::{Cglunko, UPGRADE_COUNT, data};
use crate::model::{Control, Progress};

/// The most purchases one plan may make, so a huge balance stays a bounded run.
pub const MAX_STEPS: usize = 500;

/// One of the two upgrade pages, each paid with six stones of its colour.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Page {
    /// `OptLacc[630..642]`, paid with the blue stones at `OptLacc[654..660]`.
    #[default]
    Blue,
    /// `OptLacc[642..654]`, paid with the purple stones at `OptLacc[660..666]`.
    Purple,
}

impl Page {
    /// The two pages, in the order the upstream page lists them.
    pub const ALL: [Page; 2] = [Page::Blue, Page::Purple];
    /// The upgrades on one page.
    pub const SIZE: usize = 12;

    /// The page's position in the upstream page list.
    pub fn index(self) -> usize {
        match self {
            Page::Blue => 0,
            Page::Purple => 1,
        }
    }

    /// The page as a stable id, for the page selector and the oracle.
    pub fn id(self) -> &'static str {
        match self {
            Page::Blue => "blue",
            Page::Purple => "purple",
        }
    }

    /// The page's name, as the upstream tables title it.
    pub fn label(self) -> &'static str {
        match self {
            Page::Blue => "Blue",
            Page::Purple => "Purple",
        }
    }

    /// The page with that id, or `None` for an unknown one.
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|page| page.id() == id)
    }

    /// The page that carries an upgrade index.
    pub fn of(index: usize) -> Option<Self> {
        if index < Self::SIZE {
            Some(Page::Blue)
        } else if index < 2 * Self::SIZE {
            Some(Page::Purple)
        } else {
            None
        }
    }

    /// The upgrade indexes on the page.
    pub fn upgrades(self) -> std::ops::Range<usize> {
        let start = self.index() * Self::SIZE;
        start..start + Self::SIZE
    }
}

/// The stone an upgrade is bought with: `floor(index / 2)` across both pages.
pub fn stone(index: usize) -> usize {
    index / 2
}

/// One upgrade's row in the upstream priority table.
#[derive(Clone, Debug, PartialEq)]
pub struct Rank {
    /// The upgrade index in the game's order.
    pub index: usize,
    /// The page the upgrade sits on.
    pub page: Page,
    /// The saved level.
    pub level: u32,
    /// `upgNextCost`: what one more level costs at the saved levels.
    pub next_cost: f64,
    /// `drMarginalGain`: the fractional drop-rate gain one more level adds.
    pub dr_gain: f64,
    /// `drGain / nextCost`, the upstream rank's sort key.
    pub dr_efficiency: f64,
    /// Whether the upgrade feeds the drop rate (`dr-*` category).
    pub is_dr: bool,
    /// The stone balance this row's purchase would spend.
    pub balance: f64,
}

impl Rank {
    /// Whether the saved stone balance covers the next level.
    pub fn affordable(&self) -> bool {
        self.next_cost.is_finite() && self.next_cost > 0.0 && self.next_cost <= self.balance
    }
}

/// One purchase: `index` goes from `from_level` to `from_level + 1`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Step {
    /// The upgrade bought, in the game's order.
    pub index: usize,
    /// The level it was at before this purchase.
    pub from_level: u32,
    /// What the level costs at the plan's levels when it is bought.
    pub cost: f64,
    /// The upstream marginal DR gain at the levels before the purchase.
    pub dr_gain: f64,
    /// The stone balance before the purchase.
    pub balance_before: f64,
    /// The stone balance after the purchase.
    pub balance_after: f64,
}

impl Step {
    /// The level the purchase reaches.
    pub fn to_level(&self) -> u32 {
        self.from_level.saturating_add(1)
    }
}

/// The purchases one page's saved stones buy.
#[derive(Clone, Debug, PartialEq)]
pub struct Plan {
    /// The state the plan starts from, save balances included.
    pub start: Cglunko,
    /// The state after the last purchase.
    pub end: Cglunko,
    /// The page the plan spends.
    pub page: Page,
    pub steps: Vec<Step>,
    /// The sum of every step's cost.
    pub spent: f64,
    /// Whether the step cap, not the stone balances, ended the plan.
    pub capped: bool,
}

impl Plan {
    /// The drop rate at the levels the plan starts from.
    pub fn dr_before(&self) -> f64 {
        self.start.dr()
    }

    /// The drop rate at the levels and stone balances the plan reaches.
    pub fn dr_after(&self) -> f64 {
        self.end.dr()
    }

    /// The fractional drop-rate gain across the whole plan.
    pub fn gain(&self) -> f64 {
        let before = self.dr_before();
        if before <= 0.0 {
            return 0.0;
        }
        self.dr_after() / before - 1.0
    }

    /// Why the plan stopped, as the pane explains it.
    pub fn stop_reason(&self) -> &'static str {
        if self.capped {
            "Stopped at the step cap; buy again to spend the rest."
        } else if self.steps.is_empty() {
            "No drop-rate upgrade is affordable with the saved stones."
        } else {
            "Every remaining drop-rate upgrade costs more than its stone holds or would lower DR."
        }
    }
}

impl Cglunko {
    /// The six stones of one page.
    pub fn page_stones(&self, page: Page) -> &[f64; super::STONE_COUNT] {
        match page {
            Page::Blue => &self.blue_stones,
            Page::Purple => &self.purple_stones,
        }
    }

    /// The sum of one page's six stone balances.
    pub fn page_balance(&self, page: Page) -> f64 {
        self.page_stones(page).iter().sum()
    }

    /// The stone balance that funds `index`, empty outside the two pages.
    pub fn stone_balance(&self, index: usize) -> f64 {
        let Some(page) = Page::of(index) else {
            return 0.0;
        };
        self.page_stones(page)
            .get(stone(index) % super::STONE_COUNT)
            .copied()
            .unwrap_or(0.0)
    }

    /// Spend one purchase's cost on its stone, keeping the upstream's native
    /// 0.1 balance when the cost empties it.
    fn spend(&mut self, index: usize, cost: f64) {
        let Some(page) = Page::of(index) else {
            return;
        };
        let slot = stone(index) % super::STONE_COUNT;
        let stones = match page {
            Page::Blue => &mut self.blue_stones,
            Page::Purple => &mut self.purple_stones,
        };
        let left = stones[slot] - cost;
        stones[slot] = if left <= 0.0 { 0.1 } else { left };
    }

    /// `upgNextCost`: what one more level of `index` costs at the saved levels.
    ///
    /// The Active Drops discount divides the price and every odd index costs
    /// five times as much, both exactly as the upstream `upgNextCost` applies
    /// them. The upstream page's `bolaia17discount` extension is left off while
    /// Bolaia study 17 is not decoded, the way the page itself does.
    pub fn next_cost(&self, index: usize) -> f64 {
        cost_at(self.level(index).saturating_add(1), index, self.bonus(7))
    }

    /// `drMarginalGain`: the fractional drop-rate gain one more level of
    /// `index` adds, or zero for an upgrade outside the drop rate.
    pub fn dr_gain(&self, index: usize) -> f64 {
        if index >= UPGRADE_COUNT || !data::is_dr(index) {
            return 0.0;
        }
        let before = self.dr();
        if before <= 0.0 {
            return 0.0;
        }
        let mut next = self.clone();
        next.upgrades[index] = next.upgrades[index].saturating_add(1);
        next.dr() / before - 1.0
    }

    /// The upstream `rankUpgrades` table for one page: marginal drop-rate gain
    /// per unit cost, best first, then the page's non-DR upgrades. The locked
    /// upgrade is left out, as the upstream `rankUpgrades` skips it.
    pub fn priority(&self, page: Page) -> Vec<Rank> {
        let mut rows: Vec<Rank> = page
            .upgrades()
            .filter(|index| !data::locked(*index))
            .map(|index| {
                let next_cost = self.next_cost(index);
                let dr_gain = self.dr_gain(index);
                Rank {
                    index,
                    page,
                    level: self.level(index),
                    next_cost,
                    dr_gain,
                    dr_efficiency: if next_cost > 0.0 && next_cost.is_finite() {
                        dr_gain / next_cost
                    } else {
                        0.0
                    },
                    is_dr: data::is_dr(index),
                    balance: self.stone_balance(index),
                }
            })
            .collect();
        rows.sort_by(|a, b| {
            b.dr_efficiency
                .total_cmp(&a.dr_efficiency)
                .then_with(|| a.index.cmp(&b.index))
        });
        rows
    }

    /// The plan one page's saved stones buy, without progress reports.
    pub fn plan(&self, page: Page) -> Plan {
        self.plan_with(page, |_| Control::Continue)
            .expect("a plan without reports is never asked to stop")
    }

    /// The plan one page's saved stones buy, reporting each purchase.
    ///
    /// The best affordable DR gain per unit cost is bought, then the ranking is
    /// recomputed at the levels and balances the purchase reached. A purchase
    /// the next level's cost cannot cover is skipped in favour of the next one,
    /// never overbought; a purchase that would make DR worse (spending a stone
    /// past a digit costs more than the level adds) is skipped too, so the
    /// result is never worse than the state the plan started from.
    pub fn plan_with(
        &self,
        page: Page,
        mut report: impl FnMut(Progress) -> Control,
    ) -> Option<Plan> {
        let mut current = self.clone();
        let total = current.page_balance(page);
        let mut steps: Vec<Step> = Vec::new();
        let mut spent = 0.0;
        let mut refused = [false; UPGRADE_COUNT];
        let mut capped = false;
        loop {
            if steps.len() >= MAX_STEPS {
                capped = true;
                break;
            }
            let Some((index, cost, dr_gain)) = best_purchase(&current, page, &refused) else {
                break;
            };
            let balance_before = current.stone_balance(index);
            let mut next = current.clone();
            next.upgrades[index] = next.upgrades[index].saturating_add(1);
            next.spend(index, cost);
            // The drop rate must not fall: a digit lost to the purchase can
            // outweigh the level it bought.
            if next.dr() <= current.dr() {
                refused[index] = true;
                continue;
            }
            steps.push(Step {
                index,
                from_level: current.level(index),
                cost,
                dr_gain,
                balance_before,
                balance_after: next.stone_balance(index),
            });
            spent += cost;
            current = next;
            let fraction = if total > 0.0 {
                (spent / total).clamp(0.0, 1.0) as f32
            } else {
                1.0
            };
            let progress = Progress {
                fraction,
                message: format!(
                    "{} → Lv {} · {:.0} {} #{} stones · +{:.2}% DR",
                    data::name(index),
                    current.level(index),
                    cost,
                    page.label(),
                    stone(index) % super::STONE_COUNT + 1,
                    dr_gain * 100.0
                ),
            };
            if report(progress) == Control::Stop {
                return None;
            }
        }
        Some(Plan {
            start: self.clone(),
            end: current,
            page,
            steps,
            spent,
            capped,
        })
    }
}

/// The best affordable DR purchase on `page`, as (index, cost, marginal gain).
///
/// Non-DR upgrades rank at zero gain, so the greedy pass leaves them alone; the
/// upstream priority table lists them separately for exactly that reason. A
/// refused purchase is skipped: its ratio is multiplicative, so a state change
/// that leaves its own factor and the stone digits alone cannot make it good.
fn best_purchase(
    state: &Cglunko,
    page: Page,
    refused: &[bool; UPGRADE_COUNT],
) -> Option<(usize, f64, f64)> {
    let mut best: Option<(usize, f64, f64, f64)> = None;
    for index in page.upgrades() {
        if refused[index] || !data::is_dr(index) {
            continue;
        }
        let cost = state.next_cost(index);
        if !cost.is_finite() || cost <= 0.0 || cost > state.stone_balance(index) {
            continue;
        }
        let gain = state.dr_gain(index);
        if !gain.is_finite() || gain <= 0.0 {
            continue;
        }
        let efficiency = gain / cost;
        if best.is_none_or(|(_, _, _, best)| efficiency > best) {
            best = Some((index, cost, gain, efficiency));
        }
    }
    best.map(|(index, cost, gain, _)| (index, cost, gain))
}

/// `upgNextCost`'s curve at one level: `(base^level + level)` over the Active
/// Drops discount, five times as much on the odd indexes, rounded below a
/// million the way the upstream cost is.
fn cost_at(level: u32, index: usize, cost_reduction: f64) -> f64 {
    let base = (data::cost_base(index).powf(f64::from(level)) + f64::from(level))
        / (1.0 + cost_reduction / 100.0);
    let base = if index % 2 == 1 { base * 5.0 } else { base };
    if base < 1e6 { base.round() } else { base }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The fixture-like state: blue levels and stones, purple levels at zero.
    fn sample() -> Cglunko {
        let mut upgrades = [0_u32; UPGRADE_COUNT];
        upgrades[1] = 2; // Slop Drop, 15 per level.
        upgrades[3] = 10; // Droplier, 8 per level.
        upgrades[4] = 1; // Killionaire, 3 per level.
        upgrades[7] = 5; // Active Drops, 3% cost reduction per level.
        upgrades[10] = 2; // Blue Collector, 1 per level.
        upgrades[17] = 1; // Slop Drop+, 25 per level.
        upgrades[21] = 4; // Slop Drop++, 150 per level.
        Cglunko {
            upgrades,
            blue_stones: [1000.0, 10.0, 100.0, 10.0, 100.0, 1000.0],
            purple_stones: [100.0; super::super::STONE_COUNT],
            kills: 1000.0,
            normal_dr: 100.0,
            saved: true,
        }
    }

    #[test]
    fn each_page_owns_twelve_upgrades_and_six_stones() {
        assert_eq!(Page::Blue.upgrades(), 0..12);
        assert_eq!(Page::Purple.upgrades(), 12..24);
        assert_eq!(Page::of(0), Some(Page::Blue));
        assert_eq!(Page::of(11), Some(Page::Blue));
        assert_eq!(Page::of(12), Some(Page::Purple));
        assert_eq!(Page::of(23), Some(Page::Purple));
        assert_eq!(Page::of(24), None);
        for page in Page::ALL {
            assert_eq!(Page::from_id(page.id()), Some(page));
        }
        assert_eq!(Page::from_id("blue "), None);
        // The stone map pairs upgrades: 0 and 1 pay blue #1, 22 and 23 pay
        // purple #6.
        assert_eq!(stone(0), 0);
        assert_eq!(stone(1), 0);
        assert_eq!(stone(11), 5);
        assert_eq!(stone(12), 6);
        assert_eq!(stone(23), 11);
        let cglunko = sample();
        assert_eq!(cglunko.stone_balance(1), 1000.0);
        assert_eq!(cglunko.stone_balance(3), 10.0);
        assert_eq!(cglunko.stone_balance(22), 100.0);
        assert_eq!(cglunko.stone_balance(24), 0.0);
        assert_eq!(cglunko.page_balance(Page::Blue), 2220.0);
        assert_eq!(cglunko.page_balance(Page::Purple), 600.0);
    }

    #[test]
    fn the_next_cost_follows_the_upstream_curve() {
        let cglunko = sample();
        // Droplier's next level is 11: (1.55^11 + 11) / 1.15, five times on an
        // odd index.
        let expected = ((1.55_f64.powi(11) + 11.0) / 1.15 * 5.0).round();
        assert_eq!(cglunko.next_cost(3), expected);
        // Killionaire's next level is 2: (1.6^2 + 2) / 1.15, no odd factor.
        assert_eq!(
            cglunko.next_cost(4),
            ((1.6_f64.powi(2) + 2.0) / 1.15).round()
        );
        // A zero level still costs its base plus one, discounted.
        let fresh = Cglunko::default();
        assert_eq!(fresh.next_cost(0), ((7.5_f64 + 1.0) / 1.0).round());
        assert_eq!(fresh.next_cost(1), (((1.35_f64 + 1.0) / 1.0) * 5.0).round());
        // A non-DR upgrade still prices; an unknown index reads zero base.
        assert_eq!(fresh.next_cost(24), 1.0);
    }

    #[test]
    fn the_marginal_gain_is_the_upstream_factor_ratio() {
        let cglunko = sample();
        let factors = cglunko.factors();
        // Upgrading Droplier from 10 to 11 levels adds 8 bonus points to a
        // factor of 1.8, so the gain is 1.88 / 1.8 - 1.
        let expected = 1.88 / factors.droplier - 1.0;
        assert!((cglunko.dr_gain(3) - expected).abs() < 1e-12);
        // Killionaire's digits come from the kills, not from its own level.
        let kills = 1.0 + (cglunko.bonus(4) + 3.0) * 3.0 / 100.0;
        assert!((cglunko.dr_gain(4) - (kills / factors.kills - 1.0)).abs() < 1e-12);
        // A non-DR upgrade does not move the drop rate, and neither does an
        // unknown index.
        assert_eq!(cglunko.dr_gain(7), 0.0);
        assert_eq!(cglunko.dr_gain(24), 0.0);
    }

    #[test]
    fn the_priority_ranks_each_page_by_gain_per_cost() {
        let cglunko = sample();
        for page in Page::ALL {
            let rows = cglunko.priority(page);
            // Only the purple page's locked upgrade is left out.
            let expected = Page::SIZE - usize::from(page == Page::Purple);
            assert_eq!(rows.len(), expected);
            assert!(rows.iter().all(|row| row.page == page));
            assert!(rows.iter().all(|row| row.index != 23));
            // Every row ranks by efficiency, best first, ties by index.
            for pair in rows.windows(2) {
                let (a, b) = (&pair[0], &pair[1]);
                assert!(
                    a.dr_efficiency > b.dr_efficiency
                        || (a.dr_efficiency == b.dr_efficiency && a.index < b.index)
                );
            }
            // The non-DR upgrades sit at zero gain.
            for row in rows.iter().filter(|row| !row.is_dr) {
                assert_eq!(row.dr_gain, 0.0);
            }
        }
        // The two pages rank different upgrades: blue's factor rows differ from
        // purple's.
        let blue = cglunko.priority(Page::Blue);
        let purple = cglunko.priority(Page::Purple);
        assert!(blue.iter().any(|row| row.index < 12 && row.is_dr));
        assert!(purple.iter().any(|row| row.index >= 12 && row.is_dr));
        assert_ne!(
            blue.iter().map(|row| row.index).collect::<Vec<_>>(),
            purple.iter().map(|row| row.index).collect::<Vec<_>>()
        );
    }

    #[test]
    fn the_plan_never_spends_more_than_its_stone_holds_and_never_loses_dr() {
        let cglunko = sample();
        for page in Page::ALL {
            let plan = cglunko.plan(page);
            assert!(plan.dr_after() >= plan.dr_before());
            let mut balances: [f64; super::super::STONE_COUNT] = *cglunko.page_stones(page);
            for step in &plan.steps {
                assert_eq!(Page::of(step.index), Some(page));
                assert_eq!(step.balance_before, balances[stone(step.index) % 6]);
                assert!(step.cost <= step.balance_before);
                assert!(step.dr_gain > 0.0);
                balances[stone(step.index) % 6] = step.balance_after;
            }
            // The reported spent total is the sum of the steps.
            let spent: f64 = plan.steps.iter().map(|step| step.cost).sum();
            assert!((plan.spent - spent).abs() < 1e-9);
            assert!((plan.dr_after() - plan.end.dr()).abs() < 1e-12);
        }
    }

    #[test]
    fn a_digit_losing_purchase_is_skipped_when_it_costs_more_dr_than_it_adds() {
        // One blue stone sits exactly on its first digit. Buying the cheapest
        // Droplier level would spend it down to 0.1, losing a digit that costs
        // more than the level adds, so the plan refuses it and buys nothing.
        let mut cglunko = Cglunko {
            blue_stones: [0.0, 0.0, 10.0, 0.0, 0.0, 0.0],
            purple_stones: [0.0; super::super::STONE_COUNT],
            kills: 1.0,
            normal_dr: 1.0,
            saved: true,
            ..Cglunko::default()
        };
        cglunko.upgrades[10] = 5;
        let plan = cglunko.plan(Page::Blue);
        // Only upgrade 3 lives on blue stone #2, and its next cost is 13
        // stones, so nothing is affordable here at all.
        assert!(plan.steps.is_empty());
        assert_eq!(plan.dr_after(), plan.dr_before());
        // Afford Droplier with a balance that keeps the digit.
        cglunko.blue_stones[1] = 20.0;
        let plan = cglunko.plan(Page::Blue);
        assert_eq!(plan.steps.len(), 1);
        assert_eq!(plan.steps[0].index, 3);
        assert!(plan.end.blue_stones[1] >= 1.0);
        assert!(plan.dr_after() > plan.dr_before());
        // Spend the same stone down to just under the next cost and the plan
        // still refuses the purchase that would lose its digit.
        let mut tight = cglunko.clone();
        tight.blue_stones[1] = 13.0;
        let refused = tight.plan(Page::Blue);
        assert!(refused.steps.iter().all(|step| step.index != 3));
        assert!(refused.dr_after() >= refused.dr_before());
    }

    #[test]
    fn an_unaffordable_page_makes_an_empty_plan() {
        let cglunko = Cglunko {
            saved: true,
            ..Cglunko::default()
        };
        let plan = cglunko.plan(Page::Purple);
        assert!(plan.steps.is_empty());
        assert_eq!(plan.spent, 0.0);
        assert_eq!(plan.dr_after(), plan.dr_before());
        assert_eq!(
            plan.stop_reason(),
            "No drop-rate upgrade is affordable with the saved stones."
        );
    }

    #[test]
    fn the_report_turns_a_plan_run_into_a_cancel() {
        let cglunko = sample();
        let mut messages = Vec::new();
        let plan = cglunko.plan_with(Page::Blue, |progress| {
            messages.push(progress.message);
            Control::Stop
        });
        assert!(plan.is_none());
        assert!(!messages.is_empty());
    }
}
