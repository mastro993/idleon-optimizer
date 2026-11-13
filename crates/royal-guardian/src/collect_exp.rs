//! The native collect pass: what one Royal menu collection commits, and the EXP
//! projection the forecast steps between rank thresholds.
//!
//! Ported from `royalCollectExpModels`, `previewRoyalCollectExp`,
//! `royalCollectExpRates`, `applyRoyalCollectStreams` and `royalCollectAll`.
//!
//! The game runs `_customEvent_CollectAll` about every 41 frames while the menu
//! is open; closed, elapsed time banks in `RoyalG[3][0]` once Armory 30 is
//! owned. One pass drains every node, feeds every rank bar from the units
//! assigned to it and commits any clear the militia finished. The forecast calls
//! the same pass at each event boundary, so the two cannot drift.

use super::*;
use std::collections::BTreeMap;

/// The worlds whose assignment rows the pass reads.
const ASSIGNMENT_WORLDS: usize = 8;

/// One outpost's EXP state, and the rates its units feed it with: upstream
/// `royalCollectExpModels` model.
#[derive(Clone, Debug, PartialEq)]
pub struct ExpModel {
    pub map: usize,
    /// Armory 72's share of the Intel rank, already divided by the Intel
    /// multiplier.
    pub a72: f64,
    /// Whether the militia feeds its outpost's highest rank bar.
    pub feed: bool,
    pub traders: f64,
    pub surveyors: f64,
    /// The assignment kinds aimed at this outpost, in saved order.
    pub order: Vec<i64>,
    /// EXP per contributor for each bar, before the Intel rank multiplier.
    pub k: [f64; 5],
    pub exp: [f64; 5],
    pub ranks: [f64; 5],
}

/// A built outpost's ranks and EXP after one pass of `hours`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ExpPreview {
    pub exp: [f64; 5],
    pub ranks: [f64; 5],
}

/// An outpost militia is still clearing: upstream `royalCollectExpModels` clear.
#[derive(Clone, Debug, PartialEq)]
pub struct ClearModel {
    pub map: usize,
    pub rate: f64,
    pub progress: f64,
    pub requirement: f64,
}

/// Every outpost's EXP model: upstream `royalCollectExpModels`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ExpModels {
    pub models: Vec<ExpModel>,
    pub clears: Vec<ClearModel>,
}

/// A rank bar that crossed a threshold in one pass.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RankUp {
    pub map: usize,
    pub bar: usize,
    pub from: f64,
    pub to: f64,
    pub points_before: f64,
    pub points_after: f64,
}

/// An outpost clear the pass finished.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClearGain {
    pub map: usize,
    pub gain: f64,
}

/// What one native pass committed.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Pass {
    /// What each currency slot gained.
    pub currency: BTreeMap<i64, f64>,
    /// The nodes the pass drained.
    pub drained: Vec<usize>,
    pub rank_ups: Vec<RankUp>,
    /// Points each outpost gained, by map.
    pub points: BTreeMap<usize, f64>,
    /// The kills each clearing outpost gained.
    pub clears: Vec<ClearGain>,
}

impl RoyalGuardian {
    /// Every outpost's EXP model: upstream `royalCollectExpModels`.
    pub fn exp_models(&self) -> ExpModels {
        let a72 = self.armory_bonus(72) / 100.0;
        let feed = self.armory_bonus(17) >= 1.0;
        let mut order: BTreeMap<usize, Vec<i64>> = BTreeMap::new();
        for world in 0..ASSIGNMENT_WORLDS {
            let Some(kinds) = self
                .g
                .get(row::MILITIA + 2 * world)
                .and_then(Value::as_array)
            else {
                continue;
            };
            let targets = self
                .g
                .get(row::MILITIA + 2 * world + 1)
                .and_then(Value::as_array);
            for (index, kind) in kinds.iter().enumerate() {
                let Some(kind) = json::number(kind) else {
                    continue;
                };
                let Some(map) = targets
                    .and_then(|row| row.get(index))
                    .and_then(json::number)
                else {
                    continue;
                };
                if (4.0..=7.0).contains(&kind) {
                    order.entry(map as usize).or_default().push(kind as i64);
                }
            }
        }
        let mut models = Vec::new();
        let mut clears = Vec::new();
        for map in 0..self.maps.len() {
            let row = self.map_row(map);
            if row.len() == 1 {
                let militia = order
                    .get(&map)
                    .map(|kinds| kinds.iter().filter(|kind| **kind == 4).count() as f64)
                    .unwrap_or(0.0);
                if militia > 0.0 {
                    clears.push(ClearModel {
                        map,
                        rate: militia * self.unit_spec(4),
                        progress: self.map_cell(map, 0),
                        requirement: self.kill_requirement(map),
                    });
                }
                continue;
            }
            if row.len() <= 2 || !self.outpost(map).built {
                continue;
            }
            let mult = 1.0 + self.rank(map, 1) * a72;
            let units = self.units_by_type(map);
            models.push(ExpModel {
                map,
                a72,
                feed,
                traders: units[1],
                surveyors: units[3],
                order: order.get(&map).cloned().unwrap_or_default(),
                k: [
                    self.bar_exp_rate(0, map) / mult,
                    self.bar_exp_rate(1, map) / mult,
                    self.bar_exp_rate(2, map) / mult,
                    self.bar_exp_rate(3, map) / mult,
                    self.bar_exp_rate(4, map) / mult,
                ],
                exp: [0, 1, 2, 3, 4].map(|bar| self.map_cell(map, cell::RANK_EXP + bar)),
                ranks: [0, 1, 2, 3, 4].map(|bar| self.rank(map, bar)),
            });
        }
        ExpModels { models, clears }
    }

    /// Applies one drain pass to the node quantities and currency balances:
    /// upstream `applyRoyalCollectStreams`. Returns the nodes it drained.
    pub fn apply_streams(
        &self,
        quantities: &mut [f64],
        currencies: &mut [f64],
        hours: f64,
        currency_gain: &mut BTreeMap<i64, f64>,
    ) -> Vec<usize> {
        let mut drained = Vec::new();
        let (streams, _) = self.resource_streams();
        for stream in streams {
            let Some(quantity) = quantities.get_mut(stream.resource) else {
                continue;
            };
            if *quantity <= -1.0 {
                continue;
            }
            if stream.kind == 0.0 {
                let mut amount = stream.rate * hours;
                if amount > stream.capacity - *quantity {
                    amount = stream.capacity - *quantity;
                }
                *quantity += amount;
                if stream.currency_slot >= 0
                    && let Some(balance) = currencies.get_mut(stream.currency_slot as usize)
                {
                    *balance += amount;
                    *currency_gain.entry(stream.currency_slot).or_insert(0.0) += amount;
                }
            } else if stream.kind == 2.0 {
                // A Savage outpost drains unclamped, at its Savage multiplier.
                *quantity += stream.drain_rate * hours;
            }
            if *quantity >= stream.capacity {
                *quantity = -1.0;
                if !drained.contains(&stream.resource) {
                    drained.push(stream.resource);
                }
            }
        }
        drained
    }

    /// One native collect pass over `hours` of banked time: upstream
    /// `royalCollectAll`. The rows this reads are the same ones it writes, so
    /// callers clone the state before passing it in.
    pub fn collect_all(&mut self, hours: f64) -> Pass {
        let hours = hours.max(0.0);
        let mut pass = Pass::default();
        let mut before: BTreeMap<usize, ([f64; 5], f64)> = BTreeMap::new();
        for map in 0..self.maps.len() {
            if !self.collecting(map) {
                continue;
            }
            before.insert(
                map,
                (
                    [0, 1, 2, 3, 4].map(|bar| self.rank(map, bar)),
                    self.points_left(map),
                ),
            );
        }
        if hours > 0.0 {
            let quantities: Vec<f64> = self
                .g
                .get(row::RESOURCES)
                .and_then(Value::as_array)
                .map(|row| {
                    row.iter()
                        .map(|cell| json::number(cell).unwrap_or(0.0))
                        .collect()
                })
                .unwrap_or_else(|| vec![0.0; data::resource_count()]);
            let mut quantities = quantities;
            let mut currencies = self.g_row(row::CURRENCIES, 0);
            pass.drained =
                self.apply_streams(&mut quantities, &mut currencies, hours, &mut pass.currency);
            self.write_row(row::RESOURCES, &quantities);
            self.write_row(row::CURRENCIES, &currencies);
            for map in 0..self.maps.len() {
                if !self.collecting(map) {
                    continue;
                }
                let units = self.units_by_type(map);
                let trade = self.bar_exp_rate(0, map) * units[1] * hours;
                let intel = self.bar_exp_rate(1, map) * units[3] * hours;
                self.add_map_cell(map, cell::RANK_EXP, trade);
                self.add_map_cell(map, cell::RANK_EXP + 1, intel);
            }
            let feed = self.armory_bonus(17) >= 1.0;
            for world in 0..ASSIGNMENT_WORLDS {
                let Some(kinds) = self
                    .g
                    .get(row::MILITIA + 2 * world)
                    .and_then(Value::as_array)
                    .cloned()
                else {
                    continue;
                };
                let targets = self
                    .g
                    .get(row::MILITIA + 2 * world + 1)
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                for (index, kind) in kinds.iter().enumerate() {
                    let Some(kind) = json::number(kind) else {
                        continue;
                    };
                    let Some(map) = targets.get(index).and_then(json::number) else {
                        continue;
                    };
                    let map = map as usize;
                    if self.map_row(map).is_empty() {
                        continue;
                    }
                    let kind = kind as i64;
                    if (5..=7).contains(&kind) {
                        let gain = self.bar_exp_rate((kind - 3) as usize, map) * hours;
                        self.add_map_cell(map, cell::RANK_EXP + (kind - 3) as usize, gain);
                    } else if kind == 4 {
                        if self.map_row(map).len() == 1 {
                            let gain = self.unit_spec(4) * hours;
                            self.add_map_cell(map, 0, gain);
                            pass.clears.push(ClearGain { map, gain });
                        } else if feed && self.map_row(map).len() > 2 {
                            let bar = self.highest_rank_bar(map);
                            let gain = self.bar_exp_rate(3, map) / 2.0 * hours;
                            self.add_map_cell(map, cell::RANK_EXP + bar, gain);
                        }
                    }
                }
            }
        }
        for (map, (prior_ranks, prior_points)) in before {
            let ranks = [0, 1, 2, 3, 4].map(|bar| self.rank(map, bar));
            let points = self.points_left(map);
            let mut ranked_up = false;
            for bar in 0..5 {
                if ranks[bar] > prior_ranks[bar] {
                    ranked_up = true;
                    pass.rank_ups.push(RankUp {
                        map,
                        bar,
                        from: prior_ranks[bar],
                        to: ranks[bar],
                        points_before: prior_points,
                        points_after: points,
                    });
                }
            }
            // Upstream records the point delta for every map that ranked up,
            // including the zero a non-Trading bar leaves.
            if ranked_up {
                pass.points.insert(map, points - prior_points);
            }
        }
        self.set_g_cell(row::BANKED, 0, 0.0);
        pass
    }

    /// Whether the outpost takes part in a collect pass: upstream `_collecting`.
    pub(super) fn collecting(&self, map: usize) -> bool {
        self.map_row(map).len() > 2 && self.outpost(map).built
    }

    /// The highest rank bar the outpost has reached, lowest bar on a tie:
    /// upstream `_highestBar`.
    pub(super) fn highest_rank_bar(&self, map: usize) -> usize {
        let mut best = 0;
        let mut rank = -1.0;
        for bar in 0..5 {
            let value = self.rank(map, bar);
            if value > rank {
                rank = value;
                best = bar;
            }
        }
        best
    }

    /// One `RoyalG` row as numbers, padded to at least `min` entries.
    fn g_row(&self, index: usize, min: usize) -> Vec<f64> {
        let mut row: Vec<f64> = self
            .g
            .get(index)
            .and_then(Value::as_array)
            .map(|cells| {
                cells
                    .iter()
                    .map(|cell| json::number(cell).unwrap_or(0.0))
                    .collect()
            })
            .unwrap_or_default();
        row.resize(row.len().max(min), 0.0);
        row
    }

    /// Writes a `RoyalG` row, padding it to `cells` entries.
    fn write_row(&mut self, index: usize, cells: &[f64]) {
        let row = row_mut(&mut self.g, index);
        row.clear();
        row.extend(cells.iter().map(|value| Value::from(*value)));
    }

    /// Sets a `RoyalG` cell.
    fn set_g_cell(&mut self, row: usize, cell: usize, value: f64) {
        set_cell(row_mut(&mut self.g, row), cell, value);
    }

    /// Adds to a `RoyalMaps` cell, padding the row.
    pub(super) fn add_map_cell(&mut self, map: usize, cell: usize, amount: f64) {
        let cells = row_mut(&mut self.maps, map);
        let current = cells.get(cell).and_then(json::number).unwrap_or(0.0);
        set_cell(cells, cell, current + amount);
    }
}

/// One row of rows, created as an empty array when the save does not carry it.
pub(crate) fn row_mut(rows: &mut Vec<Value>, index: usize) -> &mut Vec<Value> {
    while rows.len() <= index {
        rows.push(Value::Array(Vec::new()));
    }
    if !rows[index].is_array() {
        rows[index] = Value::Array(Vec::new());
    }
    rows[index]
        .as_array_mut()
        .expect("the row was just made an array")
}

/// Sets one cell, padding the row.
fn set_cell(cells: &mut Vec<Value>, cell: usize, value: f64) {
    while cells.len() <= cell {
        cells.push(Value::from(0.0));
    }
    cells[cell] = Value::from(value);
}

impl ExpModel {
    /// EXP and ranks after one pass of `hours`: upstream
    /// `previewRoyalCollectExp`.
    pub fn preview(&self, hours: f64) -> ExpPreview {
        let mut exp = self.exp;
        let mut ranks = self.ranks;
        let add = |bar: usize, amount: f64, exp: &mut [f64; 5], ranks: &mut [f64; 5]| {
            exp[bar] += amount;
            ranks[bar] = rank_from(exp[bar], bar, ranks[bar]);
        };
        let mult0 = 1.0 + ranks[1] * self.a72;
        add(
            0,
            self.k[0] * mult0 * self.traders * hours,
            &mut exp,
            &mut ranks,
        );
        add(
            1,
            self.k[1] * mult0 * self.surveyors * hours,
            &mut exp,
            &mut ranks,
        );
        for kind in &self.order {
            let mult = 1.0 + ranks[1] * self.a72;
            let purity = if self.ranks[4] < 1.0 && ranks[4] >= 1.0 {
                3.0
            } else {
                1.0
            };
            if *kind >= 5 {
                let bar = (*kind - 3) as usize;
                add(
                    bar,
                    self.k[bar] * purity * mult * hours,
                    &mut exp,
                    &mut ranks,
                );
            } else if self.feed {
                let mut bar = 0;
                for index in 1..5 {
                    if ranks[index] > ranks[bar] {
                        bar = index;
                    }
                }
                add(
                    bar,
                    self.k[3] * purity * mult / 2.0 * hours,
                    &mut exp,
                    &mut ranks,
                );
            }
        }
        ExpPreview { exp, ranks }
    }

    /// First-order continuous rates per bar: upstream `royalCollectExpRates`.
    pub fn rates(&self) -> [f64; 5] {
        let mult = 1.0 + self.ranks[1] * self.a72;
        let mut rates = [0.0; 5];
        rates[0] = self.k[0] * mult * self.traders;
        rates[1] = self.k[1] * mult * self.surveyors;
        let mut bar = 0;
        for index in 1..5 {
            if self.ranks[index] > self.ranks[bar] {
                bar = index;
            }
        }
        for kind in &self.order {
            if *kind >= 5 {
                let index = (*kind - 3) as usize;
                rates[index] += self.k[index] * mult;
            } else if self.feed {
                rates[bar] += self.k[3] * mult / 2.0;
            }
        }
        rates
    }
}

/// The rank an EXP bar reaches at or above `exp`: upstream `_rankFrom`.
fn rank_from(exp: f64, bar: usize, mut rank: f64) -> f64 {
    while exp >= RoyalGuardian::exp_formula(rank, bar) {
        rank += 1.0;
    }
    rank
}
