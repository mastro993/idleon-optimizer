//! The tab's Best Upgrade pane: the efficiency ranking and the affordable
//! upgrade path, ported from `_renderBestUpgrade` in `js/ui/sushi-ui.js`.

use crate::data::game;
use crate::inputs::Inputs;
use crate::math::{Station, upg_lv_req};

/// The metric a ranking or a path maximises.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Metric {
    /// Bucks per hour.
    Bucks,
    /// Fuel generated per hour.
    FuelGen,
    /// Fuel capacity.
    FuelCap,
}

impl Metric {
    /// The station's value for this metric at a level row.
    fn value(self, inputs: &Inputs, levels: &[f64]) -> f64 {
        let station = &inputs.station;
        match self {
            Metric::Bucks => station.bucks_per_hour(levels, &inputs.external),
            Metric::FuelGen => station.fuel_gen(levels, &inputs.external),
            Metric::FuelCap => station.fuel_capacity(levels, &inputs.external),
        }
    }

    /// The upgrade indices a fuel path may buy; bucks may buy any.
    fn filter(self) -> Option<Vec<usize>> {
        match self {
            Metric::Bucks => None,
            Metric::FuelGen => Some((8..=12).collect()),
            Metric::FuelCap => Some((1..=5).collect()),
        }
    }
}

/// One upgrade's next level, ranked by the metric it gains per buck.
#[derive(Clone, Debug, PartialEq)]
pub struct Candidate {
    /// The slot the upgrade is sold from.
    pub slot: usize,
    /// The `SushiUPG` index.
    pub upg: usize,
    pub name: String,
    pub level: f64,
    /// The metric's change when the level goes up by one.
    pub gain: f64,
    /// The cost of that level at the saved levels.
    pub cost: f64,
    /// `gain / cost`.
    pub efficiency: f64,
}

/// One upgrade's levels in an affordable path.
#[derive(Clone, Debug, PartialEq)]
pub struct Purchase {
    pub upg: usize,
    pub name: String,
    pub from: f64,
    pub to: f64,
    /// The summed cost of the grouped levels.
    pub cost: f64,
}

/// The purchases one budget buys, greedily by efficiency.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Path {
    /// The grouped purchases, in buy order.
    pub purchases: Vec<Purchase>,
    /// How many levels were bought in total.
    pub steps: usize,
    pub spent: f64,
    pub remaining: f64,
    /// The metric's change over the whole path.
    pub gain: f64,
}

/// Whether an upgrade is locked for this save.
///
/// An upgrade is unlocked when the research level reaches its slot's
/// requirement and the previous slot has a level, or when the save already
/// bought a level (`upgradeSlots` in `_renderUpgrades`).
fn unlocked(inputs: &Inputs, slot: usize, levels: &[f64]) -> bool {
    let data = game();
    let Some(upg) = data.slot_upg(slot) else {
        return false;
    };
    let previous = if slot == 0 {
        1.0
    } else {
        data.slot_upg(slot - 1)
            .and_then(|index| levels.get(index).copied())
            .unwrap_or(0.0)
    };
    let level = levels.get(upg).copied().unwrap_or(0.0);
    (inputs.research_level as f64 >= upg_lv_req(slot) && previous >= 1.0) || level > 0.0
}

/// Whether an upgrade may be bought next: unlocked, and below its maximum.
fn buyable(inputs: &Inputs, slot: usize, levels: &[f64]) -> Option<usize> {
    let data = game();
    let upg = data.slot_upg(slot)?;
    let upgrade = data.upgrade(upg)?;
    let level = levels.get(upg).copied().unwrap_or(0.0);
    if level >= upgrade.max_lv && upgrade.max_lv < 9999.0 {
        return None;
    }
    if !unlocked(inputs, slot, levels) {
        return None;
    }
    Some(upg)
}

/// `currentLevels` padded to the upgrade table, as the path search copies it.
fn padded(inputs: &Inputs) -> Vec<f64> {
    let data = game();
    let mut levels = inputs.station.levels.clone();
    if levels.len() < data.upgrades.len() {
        levels.resize(data.upgrades.len(), 0.0);
    }
    levels
}

/// The ranking table for one metric, best efficiency first.
pub fn candidates(inputs: &Inputs, metric: Metric) -> Vec<Candidate> {
    let data = game();
    let station: &Station = &inputs.station;
    let levels = &station.levels;
    let current = metric.value(inputs, levels);
    let mut rows = Vec::new();
    for slot in 0..data.slot_to_upg.len() {
        let Some(upg) = buyable(inputs, slot, levels) else {
            continue;
        };
        if let Some(filter) = metric.filter()
            && !filter.contains(&upg)
        {
            continue;
        }
        let mut next = levels.clone();
        if next.len() <= upg {
            next.resize(upg + 1, 0.0);
        }
        next[upg] += 1.0;
        let gain = metric.value(inputs, &next) - current;
        let cost = station.upgrade_cost(slot, levels, &inputs.external);
        rows.push(Candidate {
            slot,
            upg,
            name: data
                .upgrade(upg)
                .map(|upgrade| upgrade.name.clone())
                .unwrap_or_default(),
            level: levels.get(upg).copied().unwrap_or(0.0),
            gain,
            cost,
            efficiency: if cost > 0.0 { gain / cost } else { 0.0 },
        });
    }
    rows.sort_by(|left, right| right.efficiency.total_cmp(&left.efficiency));
    rows
}

/// `_computePath`: buys the best efficiency per buck until the budget runs out.
pub fn path(inputs: &Inputs, metric: Metric, budget: f64) -> Path {
    let data = game();
    let station = &inputs.station;
    let mut levels = padded(inputs);
    let filter = metric.filter();
    let mut remaining = budget;
    let mut purchases: Vec<Candidate> = Vec::new();
    for _ in 0..50 {
        if remaining <= 0.0 {
            break;
        }
        let baseline = metric.value(inputs, &levels);
        let mut best: Option<Candidate> = None;
        for slot in 0..data.slot_to_upg.len() {
            let Some(upg) = buyable(inputs, slot, &levels) else {
                continue;
            };
            if let Some(filter) = filter.as_ref()
                && !filter.contains(&upg)
            {
                continue;
            }
            let cost = station.upgrade_cost(slot, &levels, &inputs.external);
            if !cost.is_finite() || cost <= 0.0 || cost > remaining {
                continue;
            }
            let mut next = levels.clone();
            next[upg] += 1.0;
            let gain = metric.value(inputs, &next) - baseline;
            if gain <= 0.0 {
                continue;
            }
            let efficiency = gain / cost;
            if best
                .as_ref()
                .is_none_or(|candidate| efficiency > candidate.efficiency)
            {
                best = Some(Candidate {
                    slot,
                    upg,
                    name: data
                        .upgrade(upg)
                        .map(|upgrade| upgrade.name.clone())
                        .unwrap_or_default(),
                    level: levels[upg],
                    gain,
                    cost,
                    efficiency,
                });
            }
        }
        let Some(best) = best else {
            break;
        };
        remaining -= best.cost;
        levels[best.upg] += 1.0;
        purchases.push(best);
    }

    let mut grouped: Vec<Purchase> = Vec::new();
    let mut spent = 0.0;
    for purchase in &purchases {
        spent += purchase.cost;
        match grouped.iter_mut().find(|row| row.upg == purchase.upg) {
            Some(row) => {
                row.to = purchase.level + 1.0;
                row.cost += purchase.cost;
            }
            None => grouped.push(Purchase {
                upg: purchase.upg,
                name: purchase.name.clone(),
                from: purchase.level,
                to: purchase.level + 1.0,
                cost: purchase.cost,
            }),
        }
    }
    Path {
        steps: purchases.len(),
        purchases: grouped,
        spent,
        remaining,
        gain: metric.value(inputs, &levels) - metric.value(inputs, &station.levels),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn station() -> Station {
        Station {
            tiers: vec![0.0, 3.0, -1.0],
            types: vec![0.0, 1.0, 0.0],
            levels: vec![5.0, 0.0, 2.0],
            fires: vec![0.0, 1.0],
            ..Station::default()
        }
    }

    fn inputs(level: u32) -> Inputs {
        Inputs {
            research_level: level,
            station: station(),
            external: Default::default(),
        }
    }

    #[test]
    fn a_locked_upgrade_is_neither_ranked_nor_bought() {
        let low = inputs(1);
        for row in candidates(&low, Metric::Bucks) {
            assert!(
                low.station.level(row.upg) > 0.0 || upg_lv_req(row.slot) <= 1.0,
                "slot {} is locked",
                row.slot
            );
        }
        // Slot 20 sells upgrade 35, which the save has not bought.
        assert!(
            path(&low, Metric::Bucks, 1e9)
                .purchases
                .iter()
                .all(|row| row.upg != 35)
        );
    }

    #[test]
    fn a_path_never_spends_more_than_its_budget() {
        let inputs = inputs(200);
        let outcome = path(&inputs, Metric::Bucks, 1e6);
        assert!(outcome.spent <= 1e6, "{}", outcome.spent);
        assert!(outcome.gain > 0.0, "{}", outcome.gain);
        assert_eq!(
            outcome
                .purchases
                .iter()
                .map(|row| (row.to - row.from) as usize)
                .sum::<usize>(),
            outcome.steps
        );
    }
}
