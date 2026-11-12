//! The resource nodes: their grade, capacity, collected progress and the drain
//! projection that turns a collection rate into hours.
//!
//! Ported from `resourceGrade`, `resourceRawProgress`, `resourceCapacityAtGrade`,
//! `resourceCapacity`, `resourceCollected`, `resourceRemaining`,
//! `resourceDrained`, `resourceDrainHours`, `resourceCollectedWithin`,
//! `resourceEmptiedWithin`, `resourceWindowMetrics` and
//! `nextResourceCycleMetrics`.
//!
//! `RoyalG[4]` is the node's collected progress, not its inventory: the game
//! writes `-1` once the node is drained, so the saved cell reads the full
//! capacity instead and the collection rate fills the node towards it.

use super::*;

/// How many worlds the currency slots of a world page cover.
const TIER_WORLD: usize = 20;

/// One resource node's state, as the nodes table prints it.
#[derive(Clone, Debug, PartialEq)]
pub struct ResourceNode {
    pub resource: usize,
    pub world: usize,
    /// The saved grade, which scales the capacity by 1.5 and the rate by 25%.
    pub grade: f64,
    /// `RoyalResources[idx][2]`, before the grade and the world tier.
    pub base_capacity: f64,
    pub capacity: f64,
    /// The saved progress cell, including the drained `-1` sentinel.
    pub raw_progress: f64,
    pub collected: f64,
    pub remaining: f64,
    pub drained: bool,
    /// The currency slot the node pays, or `-1` when it pays none.
    pub currency_slot: i64,
}

/// One node's fixed-window projection: upstream `resourceWindowMetrics`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WindowMetrics {
    pub remaining: f64,
    /// What a rate collects inside the window, capped by what is left.
    pub collected: f64,
    /// Hours until the node is empty, `Infinity` when nothing drains it.
    pub drain_hours: f64,
    /// Whether the node empties inside the window.
    pub emptied: bool,
    pub drained: bool,
    /// Already drained or already empty: nothing is left to collect.
    pub already_empty: bool,
}

/// One node's next refill: upstream `nextResourceCycleMetrics`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NodeCycle {
    pub grade: f64,
    pub capacity: f64,
    pub remaining: f64,
    pub rate: f64,
    /// Hours for the current rate to drain what the node still holds.
    pub drain_hours: f64,
    /// The same projection as if the node were full at the current grade.
    pub full_capacity: f64,
    pub full_rate: f64,
    pub full_drain_hours: f64,
    /// Whether Armory 70 refills drained nodes at the daily reset.
    pub replenish_unlocked: bool,
    /// Whether Armory 0 adds a grade to each refill.
    pub grade_increase_unlocked: bool,
    /// Whether the reset can refill this node at all.
    pub next_cycle_available: bool,
    pub next_grade: f64,
    /// The refilled node's capacity, rate and drain, once a refill exists.
    pub next_capacity: Option<f64>,
    pub next_rate: Option<f64>,
    pub next_drain_hours: Option<f64>,
}

impl RoyalGuardian {
    /// A node's saved grade: upstream `resourceGrade`.
    pub fn resource_grade(&self, resource: usize) -> f64 {
        self.g_cell(row::GRADES, resource)
    }

    /// A node's saved progress, drained sentinel included: upstream
    /// `resourceRawProgress`.
    pub fn resource_raw_progress(&self, resource: usize) -> f64 {
        self.g_cell(row::RESOURCES, resource)
    }

    /// A node's capacity at one grade: upstream `resourceCapacityAtGrade`.
    pub fn resource_capacity_at_grade(resource: usize, grade: f64) -> f64 {
        let grade = if grade.is_finite() {
            grade.max(0.0).floor()
        } else {
            0.0
        };
        let base = data::resource(resource)
            .map(|row| row.base_capacity)
            .unwrap_or(0.0);
        5.0 * base * 1.5f64.powf(grade) * 5f64.powf((resource / TIER_WORLD) as f64)
    }

    /// A node's capacity at its saved grade: upstream `resourceCapacity`.
    pub fn resource_capacity(&self, resource: usize) -> f64 {
        let base = data::resource(resource)
            .map(|row| row.base_capacity)
            .unwrap_or(0.0);
        5.0 * base
            * 1.5f64.powf(self.resource_grade(resource))
            * 5f64.powf((resource / TIER_WORLD) as f64)
    }

    /// What the node holds, clamped to its capacity: upstream `resourceCollected`.
    pub fn resource_collected(&self, resource: usize) -> f64 {
        let capacity = self.resource_capacity(resource);
        let raw = self.resource_raw_progress(resource);
        if raw < 0.0 {
            return capacity;
        }
        raw.clamp(0.0, capacity)
    }

    /// What is still missing from a full node: upstream `resourceRemaining`.
    pub fn resource_remaining(&self, resource: usize) -> f64 {
        (self.resource_capacity(resource) - self.resource_collected(resource)).max(0.0)
    }

    /// Whether the node carries the drained sentinel: upstream `resourceDrained`.
    pub fn resource_drained(&self, resource: usize) -> bool {
        self.resource_raw_progress(resource) == -1.0
    }

    /// Hours for `rate` to drain what is left: upstream `resourceDrainHours`.
    pub fn resource_drain_hours(&self, resource: usize, rate: f64) -> f64 {
        let remaining = self.resource_remaining(resource);
        if remaining == 0.0 {
            0.0
        } else if rate > 0.0 {
            remaining / rate
        } else {
            f64::INFINITY
        }
    }

    /// What `rate` collects inside `hours`: upstream `resourceCollectedWithin`.
    pub fn resource_collected_within(&self, resource: usize, rate: f64, hours: f64) -> f64 {
        if self.resource_drained(resource) {
            return 0.0;
        }
        self.resource_remaining(resource)
            .min(rate * hours.max(0.0))
            .max(0.0)
    }

    /// Whether `rate` empties the node inside `hours`: upstream
    /// `resourceEmptiedWithin`.
    pub fn resource_emptied_within(&self, resource: usize, rate: f64, hours: f64) -> bool {
        self.resource_drained(resource)
            || self.resource_remaining(resource) == 0.0
            || self.resource_collected_within(resource, rate, hours)
                >= self.resource_remaining(resource)
    }

    /// One node's fixed-window projection.
    pub fn resource_window(&self, resource: usize, rate: f64, hours: f64) -> WindowMetrics {
        let drained = self.resource_drained(resource);
        let remaining = self.resource_remaining(resource);
        WindowMetrics {
            remaining,
            collected: self.resource_collected_within(resource, rate, hours),
            drain_hours: self.resource_drain_hours(resource, rate),
            emptied: self.resource_emptied_within(resource, rate, hours),
            drained,
            already_empty: drained || remaining == 0.0,
        }
    }

    /// One node's next refill at its current rate.
    pub fn resource_cycle(&self, resource: usize, rate: f64) -> NodeCycle {
        let valid = data::resource(resource).is_some();
        let grade = if valid {
            self.resource_grade(resource)
        } else {
            0.0
        };
        let capacity = if valid {
            self.resource_capacity(resource)
        } else {
            0.0
        };
        // The rate carries the grade multiplier, so dividing it back out leaves
        // the rate a refilled node at another grade would collect.
        let grade_factor = 1.0 + grade * 25.0 / 100.0;
        let ungraded_rate = if grade_factor > 0.0 {
            rate / grade_factor
        } else {
            0.0
        };
        let full_rate = ungraded_rate * grade_factor;
        let full_drain_hours = if capacity == 0.0 {
            0.0
        } else if full_rate > 0.0 {
            capacity / full_rate
        } else {
            f64::INFINITY
        };
        let replenish_unlocked = valid && self.armory_bonus(70) >= 1.0;
        let grade_increase_unlocked = valid && self.armory_bonus(0) >= 1.0;
        let next_cycle_available = replenish_unlocked;
        let next_grade = if next_cycle_available && grade_increase_unlocked {
            grade + 1.0
        } else {
            grade
        };
        let next_rate =
            next_cycle_available.then(|| ungraded_rate * (1.0 + next_grade * 25.0 / 100.0));
        let next_capacity =
            next_cycle_available.then(|| Self::resource_capacity_at_grade(resource, next_grade));
        let next_drain_hours = next_capacity.map(|capacity| match next_rate {
            Some(_) if capacity == 0.0 => 0.0,
            Some(rate) if rate > 0.0 => capacity / rate,
            _ => f64::INFINITY,
        });
        NodeCycle {
            grade,
            capacity,
            remaining: if valid {
                self.resource_remaining(resource)
            } else {
                0.0
            },
            rate,
            drain_hours: if valid {
                self.resource_drain_hours(resource, rate)
            } else {
                0.0
            },
            full_capacity: capacity,
            full_rate,
            full_drain_hours,
            replenish_unlocked,
            grade_increase_unlocked,
            next_cycle_available,
            next_grade,
            next_capacity,
            next_rate,
            next_drain_hours,
        }
    }

    /// One node's state, as the nodes table prints it.
    pub fn resource_node(&self, resource: usize) -> ResourceNode {
        ResourceNode {
            resource,
            world: Self::resource_world(resource),
            grade: self.resource_grade(resource),
            base_capacity: data::resource(resource)
                .map(|row| row.base_capacity)
                .unwrap_or(0.0),
            capacity: self.resource_capacity(resource),
            raw_progress: self.resource_raw_progress(resource),
            collected: self.resource_collected(resource),
            remaining: self.resource_remaining(resource),
            drained: self.resource_drained(resource),
            currency_slot: resource_currency(resource),
        }
    }

    /// The nodes a world page lists.
    pub fn world_resources(&self, world: usize) -> Vec<ResourceNode> {
        (0..data::resource_count())
            .filter(|resource| Self::resource_world(*resource) == world)
            .map(|resource| self.resource_node(resource))
            .collect()
    }
}

/// A node's anchor on the royal map: upstream `_resourcePosition`.
pub fn resource_position(resource: usize) -> Option<(f64, f64)> {
    data::resource(resource).map(|row| (row.x + 28.0, row.y + 26.0))
}
