//! The drain staffing: which built outpost drains which node, the banked hours
//! the account has saved, and the projection through the daily reset.
//!
//! Ported from `royalCollectStreams`, `royalBanksTime`, `royalBankedHours`,
//! `royalResetTiming`, `resourceAllocationMetrics`,
//! `resourceAllocationToReset` and `currencyIncomeBreakdown`.
//!
//! A node fills from every stream linked to it, so one node's rate is the sum of
//! its outposts. A Savage outpost drains at its collection rate times the Savage
//! multiplier and pays no currency, which is why the drain rate and the currency
//! rate travel separately.

use super::*;
use std::collections::BTreeMap;

/// One outpost's drain on one resource node: upstream `royalCollectStreams`
/// entry.
#[derive(Clone, Debug, PartialEq)]
pub struct ResourceStream {
    pub map: usize,
    /// The connection slot the link sits in.
    pub slot: usize,
    pub resource: usize,
    /// 0 Normal, 2 Savage.
    pub kind: f64,
    /// What the outpost harvests per hour before the Savage multiplier.
    pub rate: f64,
    /// What the node loses per hour.
    pub drain_rate: f64,
    /// What the currency balance gains per hour.
    pub currency_rate: f64,
    pub savage: bool,
    pub capacity: f64,
    pub currency_slot: i64,
}

/// One resource node inside an allocation.
#[derive(Clone, Debug, PartialEq)]
pub struct NodeAllocation {
    pub resource: usize,
    /// Every stream's drain rate, summed.
    pub rate: f64,
    pub drain_rate: f64,
    pub currency_rate: f64,
    pub remaining: f64,
    pub drain_hours: f64,
    /// What the window collects from this node.
    pub collected: f64,
    /// Whether the node empties inside the window.
    pub emptied: bool,
    pub already_empty: bool,
    pub drained: bool,
    pub currency_slot: i64,
    pub streams: Vec<ResourceStream>,
    /// What the reset projection collects before the reset, when it exists.
    pub collectable_before_reset: Option<f64>,
    pub empties_by_reset: Option<bool>,
    pub initially_drained: bool,
    pub initially_empty: bool,
    pub refills_at_reset: bool,
    /// `saved-drained`, `collect-before-reset` or `none`.
    pub refill_boundary: &'static str,
    /// Whether the refill only happens once the player collects before reset.
    pub refill_requires_collection: bool,
    pub grade_gain_at_reset: Option<f64>,
    pub projected_grade_at_reset: Option<f64>,
    /// What the refilled node holds at the reset.
    pub projected_remaining_at_reset: Option<f64>,
}

/// The daily reset countdown: upstream `royalResetTiming`.
#[derive(Clone, Debug, PartialEq)]
pub struct ResetTiming {
    /// Whether the save carries `TimeAway.ShopRestock`.
    pub available: bool,
    pub seconds_remaining: Option<f64>,
    pub hours_remaining: Option<f64>,
    pub due: bool,
    pub missing: Vec<String>,
}

/// Every node's allocation over one fixed window, with the reset projection
/// upstream folds into the same result.
#[derive(Clone, Debug, PartialEq)]
pub struct Allocation {
    pub hours: f64,
    pub details: Vec<NodeAllocation>,
    /// Saved connections that are malformed or incompatible with their outpost.
    pub invalid: Vec<String>,
    /// The currency every node pays per hour, summed.
    pub resources_per_hour: f64,
    pub collected_within_window: f64,
    pub nodes_emptied: usize,
    pub already_empty: usize,
    /// The longest drain among the nodes that have one, `Infinity` when a node
    /// with a remaining amount has no rate at all.
    pub slowest_drain_hours: f64,
    pub drain_sum_hours: f64,
    pub income_by_currency: BTreeMap<i64, f64>,
    pub reset: ResetTiming,
    /// Whether the reset projection below has numbers.
    pub reset_projection_available: bool,
    pub collectable_before_reset: Option<f64>,
    pub projected_remaining_at_reset: Option<f64>,
    pub nodes_emptying_by_reset: Option<usize>,
    pub projected_refills_at_reset: Option<usize>,
    pub projected_grade_gains_at_reset: Option<f64>,
    pub missing: Vec<String>,
    pub partial: bool,
}

/// The saved banked time and what it collects: upstream `bankedResourceProjection`
/// in its current-state mode.
#[derive(Clone, Debug, PartialEq)]
pub struct Banked {
    /// Whether Armory 30 makes the game bank time while the menu is closed.
    pub banks_time: bool,
    pub hours: f64,
    /// What the banked hours collect, summed over every node.
    pub gain: f64,
    pub currency_gain: f64,
    pub reset: ResetTiming,
    pub allocation: Allocation,
}

/// One node inside a currency's income.
#[derive(Clone, Debug, PartialEq)]
pub struct CurrencyNode {
    pub resource: usize,
    /// What the node's normal streams pay per hour.
    pub currency_rate: f64,
    /// What the window collects in currency from this node.
    pub collectable_within_window: f64,
    pub allocation: NodeAllocation,
}

/// One currency's income: upstream `currencyIncomeBreakdown`.
#[derive(Clone, Debug, PartialEq)]
pub struct CurrencyIncome {
    pub slot: i64,
    pub balance: f64,
    pub nominal_per_hour: f64,
    /// What the window collects from the nodes that pay this currency.
    pub collectable_within_window: f64,
    pub active_streams: usize,
    pub active_nodes: usize,
    /// One row per node that pays this currency.
    pub nodes: Vec<CurrencyNode>,
    pub reset: ResetTiming,
    pub reset_projection_available: bool,
    /// What is left across the nodes right now.
    pub remaining_now: Option<f64>,
    pub projected_available_at_reset: Option<f64>,
    pub collectable_before_reset: Option<f64>,
    pub missing: Vec<String>,
    pub partial: bool,
}

impl RoyalGuardian {
    /// Whether Armory 30 makes the account bank Royal Guardian time.
    pub fn banks_time(&self) -> bool {
        self.armory_level(30) > 0.0
    }

    /// The seconds of Royal Guardian time the save has banked.
    pub fn banked_hours(&self) -> f64 {
        (self.g_cell(row::BANKED, 0) / 3600.0).max(0.0)
    }

    /// The daily reset countdown the save carries.
    pub fn reset_timing(&self) -> ResetTiming {
        match self.reset_seconds {
            Some(seconds) => {
                let seconds = seconds.max(0.0);
                ResetTiming {
                    available: true,
                    seconds_remaining: Some(seconds),
                    hours_remaining: Some(seconds / 3600.0),
                    due: seconds == 0.0,
                    missing: Vec::new(),
                }
            }
            None => ResetTiming {
                available: false,
                seconds_remaining: None,
                hours_remaining: None,
                due: false,
                missing: vec!["TimeAway.ShopRestock".to_owned()],
            },
        }
    }

    /// Every drain link the save holds, in map then slot order: upstream
    /// `royalCollectStreams`, with the links whose target or outpost kind makes
    /// them unusable reported separately.
    pub fn resource_streams(&self) -> (Vec<ResourceStream>, Vec<String>) {
        self.resource_streams_with(&self.collection_inputs())
    }

    /// Every drain link, with the collection inputs supplied.
    pub fn resource_streams_with(
        &self,
        inputs: &CollectionInputs,
    ) -> (Vec<ResourceStream>, Vec<String>) {
        let mut streams = Vec::new();
        let mut invalid = Vec::new();
        let savage_multiplier = self.savage_collection();
        for map in 0..self.maps.len() {
            if !self.outpost(map).built {
                continue;
            }
            let kind = self.outpost_type(map);
            for slot in 0..2 {
                let Endpoint::Resource(resource) =
                    Self::parse_endpoint(self.map_saved(map, cell::LINKS + slot))
                else {
                    continue;
                };
                if data::resource(resource).is_none() {
                    invalid.push(format!(
                        "map {map} slot {slot}: resource {resource} is invalid"
                    ));
                    continue;
                }
                if kind == 1.0 {
                    invalid.push(format!(
                        "map {map} slot {slot}: a Support outpost cannot drain a node"
                    ));
                    continue;
                }
                let rate = self.resource_production_with(map, resource, inputs).value;
                let savage = kind == 2.0;
                streams.push(ResourceStream {
                    map,
                    slot,
                    resource,
                    kind,
                    rate,
                    drain_rate: if savage {
                        savage_multiplier * rate
                    } else {
                        rate
                    },
                    currency_rate: if savage { 0.0 } else { rate },
                    savage,
                    capacity: self.resource_capacity(resource),
                    currency_slot: resource_currency(resource),
                });
            }
        }
        (streams, invalid)
    }

    /// Every node's fixed-window projection: upstream `resourceAllocationMetrics`.
    pub fn resource_allocation(&self, hours: f64) -> Allocation {
        self.resource_allocation_with(&self.collection_inputs(), hours)
    }

    /// Every node's fixed-window projection with the inputs supplied.
    pub fn resource_allocation_with(&self, inputs: &CollectionInputs, hours: f64) -> Allocation {
        let hours = hours.max(0.0);
        let (streams, invalid) = self.resource_streams_with(inputs);
        let mut grouped: Vec<Vec<ResourceStream>> = vec![Vec::new(); data::resource_count()];
        for stream in streams {
            if let Some(slot) = grouped.get_mut(stream.resource) {
                slot.push(stream);
            }
        }
        let details: Vec<NodeAllocation> = grouped
            .into_iter()
            .enumerate()
            .map(|(resource, streams)| {
                let rate: f64 = streams.iter().map(|stream| stream.rate).sum();
                let currency_rate: f64 = streams.iter().map(|stream| stream.currency_rate).sum();
                let window = self.resource_window(resource, rate, hours);
                NodeAllocation {
                    resource,
                    rate,
                    drain_rate: rate,
                    currency_rate,
                    remaining: window.remaining,
                    drain_hours: window.drain_hours,
                    collected: window.collected,
                    emptied: !window.already_empty && window.emptied,
                    already_empty: window.already_empty,
                    drained: window.drained,
                    currency_slot: resource_currency(resource),
                    streams,
                    collectable_before_reset: None,
                    empties_by_reset: None,
                    initially_drained: window.drained,
                    initially_empty: window.already_empty,
                    refills_at_reset: false,
                    refill_boundary: "none",
                    refill_requires_collection: false,
                    grade_gain_at_reset: None,
                    projected_grade_at_reset: None,
                    projected_remaining_at_reset: None,
                }
            })
            .collect();
        let scoped: Vec<&NodeAllocation> = details
            .iter()
            .filter(|detail| detail.remaining > 0.0 && detail.rate > 0.0)
            .collect();
        let slowest_drain_hours = scoped
            .iter()
            .map(|detail| detail.drain_hours)
            .fold(0.0f64, f64::max);
        let drain_sum_hours: f64 = scoped
            .iter()
            .map(|detail| {
                if detail.drain_hours.is_finite() {
                    detail.drain_hours
                } else {
                    0.0
                }
            })
            .sum();
        let mut income_by_currency = BTreeMap::new();
        for detail in &details {
            if detail.currency_slot >= 0 && detail.currency_rate > 0.0 {
                *income_by_currency
                    .entry(detail.currency_slot)
                    .or_insert(0.0) += detail.currency_rate;
            }
        }
        Allocation {
            hours,
            resources_per_hour: details.iter().map(|detail| detail.currency_rate).sum(),
            collected_within_window: details.iter().map(|detail| detail.collected).sum(),
            nodes_emptied: details.iter().filter(|detail| detail.emptied).count(),
            already_empty: details.iter().filter(|detail| detail.already_empty).count(),
            slowest_drain_hours,
            drain_sum_hours,
            income_by_currency,
            reset: self.reset_timing(),
            reset_projection_available: false,
            collectable_before_reset: None,
            projected_remaining_at_reset: None,
            nodes_emptying_by_reset: None,
            projected_refills_at_reset: None,
            projected_grade_gains_at_reset: None,
            partial: !invalid.is_empty() || !inputs.missing.is_empty(),
            invalid,
            missing: inputs.missing.clone(),
            details,
        }
    }

    /// The fixed-window allocation with the daily reset's projection folded in:
    /// upstream `resourceAllocationToReset`.
    pub fn allocation_to_reset(&self) -> Allocation {
        self.allocation_to_reset_with(&self.collection_inputs())
    }

    /// The reset projection with every collection input supplied.
    pub fn allocation_to_reset_with(&self, inputs: &CollectionInputs) -> Allocation {
        let reset = self.reset_timing();
        let mut allocation =
            self.resource_allocation_with(inputs, reset.hours_remaining.unwrap_or(0.0));
        allocation.reset = reset.clone();
        if !reset.available {
            for detail in &mut allocation.details {
                detail.collectable_before_reset = None;
                detail.empties_by_reset = None;
                detail.refills_at_reset = false;
                detail.refill_boundary = "none";
                detail.grade_gain_at_reset = None;
                detail.projected_grade_at_reset = None;
                detail.projected_remaining_at_reset = None;
            }
            for missing in &reset.missing {
                if !allocation.missing.contains(missing) {
                    allocation.missing.push(missing.clone());
                }
            }
            allocation.partial = true;
            return allocation;
        }
        let replenish = self.armory_bonus(70) >= 1.0;
        let grade_increase = self.armory_bonus(0) >= 1.0;
        for detail in &mut allocation.details {
            let initially_drained = detail.drained;
            let initially_empty = detail.already_empty;
            let empties_by_reset = !initially_empty && detail.emptied;
            let refills_at_reset = (initially_drained || empties_by_reset) && replenish;
            let grade_gain_at_reset = if refills_at_reset && grade_increase {
                1.0
            } else {
                0.0
            };
            let projected_grade_at_reset =
                self.resource_grade(detail.resource) + grade_gain_at_reset;
            let projected_remaining_at_reset = if refills_at_reset {
                Self::resource_capacity_at_grade(detail.resource, projected_grade_at_reset)
            } else {
                (detail.remaining - detail.collected).max(0.0)
            };
            detail.collectable_before_reset = Some(detail.collected);
            detail.empties_by_reset = Some(empties_by_reset);
            detail.initially_drained = initially_drained;
            detail.initially_empty = initially_empty;
            detail.refills_at_reset = refills_at_reset;
            detail.refill_boundary = if initially_drained {
                "saved-drained"
            } else if empties_by_reset {
                "collect-before-reset"
            } else {
                "none"
            };
            detail.refill_requires_collection = !initially_drained && empties_by_reset && replenish;
            detail.grade_gain_at_reset = Some(grade_gain_at_reset);
            detail.projected_grade_at_reset = Some(projected_grade_at_reset);
            detail.projected_remaining_at_reset = Some(projected_remaining_at_reset);
        }
        allocation.reset_projection_available = true;
        allocation.collectable_before_reset = Some(allocation.collected_within_window);
        allocation.projected_remaining_at_reset = Some(
            allocation
                .details
                .iter()
                .map(|detail| detail.projected_remaining_at_reset.unwrap_or(0.0))
                .sum(),
        );
        allocation.nodes_emptying_by_reset = Some(allocation.nodes_emptied);
        allocation.projected_refills_at_reset = Some(
            allocation
                .details
                .iter()
                .filter(|detail| detail.refills_at_reset)
                .count(),
        );
        allocation.projected_grade_gains_at_reset = Some(
            allocation
                .details
                .iter()
                .map(|detail| detail.grade_gain_at_reset.unwrap_or(0.0))
                .sum(),
        );
        allocation
    }

    /// What the saved banked time would collect right now: upstream
    /// `bankedResourceProjection` in its current-state mode.
    pub fn banked(&self) -> Banked {
        self.banked_with(&self.collection_inputs())
    }

    /// The banked projection with every collection input supplied.
    pub fn banked_with(&self, inputs: &CollectionInputs) -> Banked {
        let hours = self.banked_hours();
        let allocation = self.resource_allocation_with(inputs, hours);
        let mut gain = 0.0;
        let mut currency_gain = 0.0;
        for detail in &allocation.details {
            // A drained node refills only at the reset, so the banked hours
            // collect nothing from it.
            let collected = if detail.drained {
                0.0
            } else {
                detail.collected
            };
            if collected > 0.0 && detail.remaining > 0.0 {
                let share = if detail.rate > 0.0 {
                    detail.currency_rate / detail.rate
                } else {
                    0.0
                };
                gain += collected;
                currency_gain += collected * share;
            }
        }
        Banked {
            banks_time: self.banks_time(),
            hours,
            gain,
            currency_gain,
            reset: self.reset_timing(),
            allocation,
        }
    }

    /// One currency's income: upstream `currencyIncomeBreakdown`.
    pub fn currency_income(&self, slot: i64) -> CurrencyIncome {
        self.currency_income_with(slot, &self.collection_inputs())
    }

    /// One currency's income with every collection input supplied.
    pub fn currency_income_with(&self, slot: i64, inputs: &CollectionInputs) -> CurrencyIncome {
        let balance = self.currency_balance(slot);
        let reset = self.reset_timing();
        let allocation =
            self.resource_allocation_with(inputs, reset.hours_remaining.unwrap_or(0.0));
        // The reset projection is what tells a node that is about to refill from
        // one that only empties, so the currency rows read the same details.
        let projected = self.allocation_to_reset_with(inputs);
        let nodes: Vec<CurrencyNode> = allocation
            .details
            .iter()
            .filter(|detail| detail.currency_slot == slot)
            .map(|detail| {
                let reset_detail = projected
                    .details
                    .iter()
                    .find(|node| node.resource == detail.resource);
                let mut node_allocation = detail.clone();
                if let Some(reset_detail) = reset_detail {
                    node_allocation.collectable_before_reset =
                        reset_detail.collectable_before_reset;
                    node_allocation.empties_by_reset = reset_detail.empties_by_reset;
                    node_allocation.initially_drained = reset_detail.initially_drained;
                    node_allocation.initially_empty = reset_detail.initially_empty;
                    node_allocation.refills_at_reset = reset_detail.refills_at_reset;
                    node_allocation.refill_boundary = reset_detail.refill_boundary;
                    node_allocation.refill_requires_collection =
                        reset_detail.refill_requires_collection;
                    node_allocation.grade_gain_at_reset = reset_detail.grade_gain_at_reset;
                    node_allocation.projected_grade_at_reset =
                        reset_detail.projected_grade_at_reset;
                    node_allocation.projected_remaining_at_reset =
                        reset_detail.projected_remaining_at_reset;
                }
                // Only a Normal stream pays currency, so the window's collectable
                // currency is the node's collected amount scaled by that share.
                let collectable_within_window = if detail.remaining > 0.0 && detail.rate > 0.0 {
                    let normal_rate: f64 = detail
                        .streams
                        .iter()
                        .filter(|stream| !stream.savage)
                        .map(|stream| stream.drain_rate)
                        .sum();
                    (detail.currency_rate * allocation.hours)
                        .min(detail.remaining * normal_rate / detail.rate)
                } else {
                    0.0
                };
                CurrencyNode {
                    resource: detail.resource,
                    currency_rate: detail.currency_rate,
                    collectable_within_window,
                    allocation: node_allocation,
                }
            })
            .collect();
        let nominal_per_hour: f64 = nodes.iter().map(|node| node.currency_rate).sum();
        let collectable_within_window: f64 = nodes
            .iter()
            .map(|node| node.collectable_within_window)
            .sum();
        let remaining_now = if projected.reset_projection_available {
            Some(nodes.iter().map(|node| node.allocation.remaining).sum())
        } else {
            None
        };
        let mut income = CurrencyIncome {
            slot,
            balance,
            nominal_per_hour,
            collectable_within_window,
            active_streams: nodes
                .iter()
                .map(|node| {
                    node.allocation
                        .streams
                        .iter()
                        .filter(|stream| stream.currency_rate > 0.0)
                        .count()
                })
                .sum(),
            active_nodes: nodes.iter().filter(|node| node.currency_rate > 0.0).count(),
            nodes,
            reset,
            reset_projection_available: projected.reset_projection_available,
            remaining_now,
            projected_available_at_reset: None,
            collectable_before_reset: None,
            missing: allocation.missing.clone(),
            partial: allocation.partial,
        };
        if income.reset_projection_available {
            income.collectable_before_reset = Some(income.collectable_within_window);
            income.projected_available_at_reset = Some(
                income
                    .nodes
                    .iter()
                    .map(|node| node.allocation.projected_remaining_at_reset.unwrap_or(0.0))
                    .sum(),
            );
        }
        income
    }

    /// One currency slot's saved balance: upstream `royalG(S, 1, slot)`.
    pub fn currency_balance(&self, slot: i64) -> f64 {
        if slot < 0 {
            return 0.0;
        }
        self.g_cell(row::CURRENCIES, slot as usize)
    }
}
