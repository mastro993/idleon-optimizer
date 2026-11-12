//! The range maths: where the outposts and resource nodes sit, how far an
//! outpost reaches and the Advanced Logistics plan that closes the gap.
//!
//! Ported from `outpostMapPosition`, `resourceDistance`, `outpostRange`,
//! `logisticsUpgradesToReach` and `nextResourceRangeTarget`.

use super::*;

impl RoyalGuardian {
    /// A map's anchor on the royal map: upstream `outpostMapPosition`.
    pub fn outpost_position(map: usize) -> Option<(f64, f64)> {
        data::map(map).map(|row| (row.x + 15.0, row.y + 13.0))
    }

    /// How far a resource node sits from an outpost on the same world map.
    pub fn resource_distance(map: usize, resource: usize) -> f64 {
        if Self::world_of(map) != Self::resource_world(resource) {
            return f64::INFINITY;
        }
        let Some((x2, y2)) = resource_position(resource) else {
            return f64::INFINITY;
        };
        let Some((x1, y1)) = Self::outpost_position(map) else {
            return f64::INFINITY;
        };
        ((x1 - x2).powi(2) + (y1 - y2).powi(2)).sqrt()
    }

    /// An outpost's range without its Advanced Logistics term.
    fn range_without_logistics(&self, map: usize) -> f64 {
        80.0 + self.orblet_bonus(8)
            + self.unit_spec(2) * self.units_by_type(map)[2]
            + self.rank(map, 3) * self.armory_bonus(74)
    }

    /// An outpost's range at one Advanced Logistics level: upstream
    /// `outpostRangeAtLogisticsLevel`.
    pub fn range_at_logistics(&self, map: usize, level: f64) -> f64 {
        let level = level.max(0.0).floor();
        let range = self.range_without_logistics(map) + 250.0 * level / (level + 100.0);
        range.min(999.0).floor()
    }

    /// An outpost's saved range: upstream `outpostRange`.
    pub fn range(&self, map: usize) -> f64 {
        self.range_at_logistics(map, self.map_cell(map, cell::LOGISTICS))
    }

    /// Whether an outpost links a resource node: built, not Support, and in
    /// reach: upstream `resourceReachable`.
    pub fn resource_reachable(&self, map: usize, resource: usize) -> bool {
        let kind = self.outpost_type(map);
        self.outpost(map).built
            && (kind == 0.0 || kind == 2.0)
            && Self::resource_distance(map, resource) <= self.range(map) + RESOURCE_ALLOWANCE
    }

    /// The Advanced Logistics plan that reaches a resource node: upstream
    /// `logisticsUpgradesToReach`.
    pub fn logistics_plan(&self, map: usize, distance: f64) -> Plan {
        let current_level = self.map_cell(map, cell::LOGISTICS).max(0.0).floor();
        let current_range = self.range(map);
        let required_range = (distance - RESOURCE_ALLOWANCE).ceil();
        let mut plan = Plan {
            possible: false,
            current_level,
            current_range,
            required_range,
            target_level: None,
            levels_needed: None,
            reason: None,
        };
        if !distance.is_finite() {
            plan.reason = Some("distance unavailable");
            return plan;
        }
        if required_range <= current_range {
            plan.possible = true;
            plan.target_level = Some(current_level);
            plan.levels_needed = Some(0.0);
            return plan;
        }
        let delta = (required_range - self.range_without_logistics(map)).max(0.0);
        if required_range > 999.0 || delta >= 250.0 {
            plan.reason = Some("beyond maximum Advanced Logistics range");
            return plan;
        }
        let mut target_level = current_level.max((100.0 * delta / (250.0 - delta)).ceil());
        while target_level < MAX_LOGISTICS
            && self.range_at_logistics(map, target_level) < required_range
        {
            target_level += 1.0;
        }
        if target_level >= MAX_LOGISTICS {
            plan.reason = Some("beyond maximum Advanced Logistics range");
            return plan;
        }
        plan.possible = true;
        plan.levels_needed = Some(target_level - current_level);
        plan.target_level = Some(target_level);
        plan
    }

    /// The nearest resource node the outpost cannot reach yet: upstream
    /// `nextResourceRangeTarget`.
    pub fn next_range_target(&self, map: usize) -> Option<RangeTarget> {
        let outpost = self.outpost(map);
        if !outpost.built {
            return None;
        }
        let range = self.range(map);
        let mut nodes: Vec<(usize, f64)> = (0..data::resource_count())
            .filter(|resource| Self::resource_world(*resource) == outpost.world)
            .map(|resource| (resource, Self::resource_distance(map, resource)))
            .collect();
        nodes.sort_by(|left, right| left.1.total_cmp(&right.1));
        let (resource, distance) = nodes.into_iter().find(|(resource, _)| {
            resource_currency(*resource) >= 0 && !self.resource_reachable(map, *resource)
        })?;
        let plan = if self.outpost_type(map) == 1.0 {
            Plan {
                possible: false,
                current_level: self.map_cell(map, cell::LOGISTICS).max(0.0).floor(),
                current_range: range,
                required_range: (distance - RESOURCE_ALLOWANCE).ceil(),
                target_level: None,
                levels_needed: None,
                reason: Some("Support outposts cannot connect to resources"),
            }
        } else {
            self.logistics_plan(map, distance)
        };
        Some(RangeTarget {
            resource,
            distance,
            range,
            reach: range + RESOURCE_ALLOWANCE,
            plan,
        })
    }
}
