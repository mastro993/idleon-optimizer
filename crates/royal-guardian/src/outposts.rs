//! The outpost rows, their ranks, the militia assigned to them and their units.
//!
//! Ported from `outpostState`, `worldOutpostRows`, `outpostRank`,
//! `militiaAssignments`, `totalUnitsByType` and the readers around them.

use super::*;

impl RoyalGuardian {
    /// One map's outpost row: upstream `outpostEligibility`.
    pub fn outpost(&self, map: usize) -> Outpost {
        let info = data::map(map);
        let world = Self::world_of(map);
        let eligible = info.map(|row| row.eligible).unwrap_or(false);
        let built = self.map_row(map).len() >= 3;
        let reason = (!built).then_some(if !eligible {
            "map is statically ineligible"
        } else if !self.world_unlocked(world) {
            "world is locked"
        } else {
            "map is unavailable"
        });
        Outpost {
            map,
            world,
            name: info.map(|row| row.name.as_str()).unwrap_or("Unknown"),
            eligible,
            built,
            reason,
        }
    }

    /// One world's outposts, in map order: upstream `worldOutpostRows`.
    pub fn world_outposts(&self, world: usize) -> Vec<Outpost> {
        (0..MAPS_PER_WORLD)
            .map(|offset| world * MAPS_PER_WORLD + offset)
            .filter(|map| data::map(*map).is_some())
            .map(|map| self.outpost(map))
            .filter(|outpost| outpost.eligible)
            .collect()
    }

    /// How many kills an outpost needs: upstream `outpostKillRequirement`.
    pub fn kill_requirement(&self, map: usize) -> f64 {
        data::map(map)
            .map(|row| row.kill_requirement)
            .unwrap_or(0.0)
    }

    /// An outpost's saved type: 0 Normal, 1 Support, 2 Savage.
    pub fn outpost_type(&self, map: usize) -> f64 {
        self.map_cell(map, cell::KIND)
    }

    /// Whether the outpost is glorified.
    pub fn glorified(&self, map: usize) -> bool {
        self.map_cell(map, cell::GLORIFIED) >= 1.0
    }

    /// Whether the outpost is purified: built and past Purity rank 0.
    pub fn purified(&self, map: usize) -> bool {
        self.outpost(map).built && self.rank(map, 4) >= 1.0
    }

    /// One rank bar's EXP requirement: upstream `outpostExpFormula`.
    pub fn exp_formula(rank: f64, bar: usize) -> f64 {
        match bar {
            4 => 1e5 * 10f64.powf(rank),
            2 => (50.0 + 50.0 * rank) * 1.8f64.powf(rank),
            _ => (10.0 + 5.0 * rank) * 1.3f64.powf(rank),
        }
    }

    /// One rank bar's rank for the outpost's saved EXP.
    pub fn rank(&self, map: usize, bar: usize) -> f64 {
        let exp = self.map_cell(map, cell::RANK_EXP + bar);
        let mut rank = 0.0;
        while rank < MAX_RANK && exp >= Self::exp_formula(rank, bar) {
            rank += 1.0;
        }
        rank
    }

    /// Every militia and command-rank assignment the save holds.
    pub fn militia_assignments(&self) -> Vec<Assignment> {
        let mut assignments = Vec::new();
        for world in 0..WORLDS {
            let kinds = self
                .g
                .get(row::MILITIA + 2 * world)
                .and_then(Value::as_array);
            let maps = self
                .g
                .get(row::MILITIA + 2 * world + 1)
                .and_then(Value::as_array);
            let (Some(kinds), Some(maps)) = (kinds, maps) else {
                continue;
            };
            for index in 0..kinds.len().min(maps.len()) {
                let (Some(kind), Some(map)) =
                    (json::number(&kinds[index]), json::number(&maps[index]))
                else {
                    continue;
                };
                assignments.push(Assignment {
                    world,
                    assignment: index,
                    kind: kind as i64,
                    map: map as i64,
                });
            }
        }
        assignments
    }

    /// How many militia the outpost has assigned to clearing it.
    pub fn militia_assignment_count(&self, map: usize) -> f64 {
        let target = map as i64;
        self.militia_assignments()
            .iter()
            .filter(|assignment| assignment.kind == 4 && assignment.map == target)
            .count() as f64
    }

    /// A packed unit row decoded into its slots: upstream `decodePackedUnits`.
    pub(super) fn packed_units(packed: f64) -> Vec<(usize, usize)> {
        let digits = format!("{:09}", packed.max(0.0) as u64);
        let digits = &digits[digits.len().saturating_sub(9)..];
        digits
            .chars()
            .enumerate()
            .filter_map(|(slot, digit)| {
                let digit = digit.to_digit(10)?;
                (2..=5)
                    .contains(&digit)
                    .then(|| (slot, (digit - 2) as usize))
            })
            .collect()
    }

    /// How many permanent units the command rank grants, by profession.
    fn command_rank_units(&self, map: usize, profession: usize) -> f64 {
        let rank = self.rank(map, 2).max(0.0);
        ((rank - profession as f64).max(0.0) / 4.0).ceil()
    }

    /// A profession's units in the outpost, permanents included: upstream
    /// `totalUnitsByType`.
    pub fn units_by_type(&self, map: usize) -> [f64; 4] {
        let mut totals = [0.0; 4];
        for (_, profession) in Self::packed_units(self.map_cell(map, cell::UNITS)) {
            if let Some(total) = totals.get_mut(profession) {
                *total += 1.0;
            }
        }
        for (profession, total) in totals.iter_mut().enumerate() {
            let glorified = if profession == 0 && self.glorified(map) {
                1.0
            } else {
                0.0
            };
            *total += self.command_rank_units(map, profession) + glorified;
        }
        totals
    }

    /// Every unit stationed in the outpost.
    pub fn total_units(&self, map: usize) -> f64 {
        self.units_by_type(map).iter().sum()
    }

    /// The points a map has left to spend: upstream `outpostPointsLeft`.
    pub fn points_left(&self, map: usize) -> f64 {
        if self.map_row(map).len() <= 3 {
            return 0.0;
        }
        let rank = self.rank(map, 0);
        let level_71 = self.armory_level(71);
        let world = (map / MAPS_PER_WORLD).min(6);
        let world_reward = [3.0, 15.0, 32.0, 43.0, 48.0, 52.0, 61.0][world];
        let mut points =
            2.0 + self.jelly_bonus(world_reward as usize) + self.armory_bonus(9 + world) + rank;
        if level_71 >= 1.0 {
            points += (rank / (11.0 - level_71)).floor();
        }
        if self.map_cell(map, cell::GLORIFIED) > 0.0 {
            points += 10.0;
        }
        points - self.map_cell(map, 0) * 12.0 - self.map_cell(map, 1) * 2.0 - self.map_cell(map, 2)
    }

    /// A rank bar's EXP per hour, for one outpost: upstream `barExpRate`.
    pub fn bar_exp_rate(&self, bar: usize, map: usize) -> f64 {
        let education = 1.0 + 10.0 * self.map_cell(map, 2) / 100.0;
        self.bar_exp_rate_base(bar)
            * education
            * if self.glorified(map) { 2.0 } else { 1.0 }
            * if self.purified(map) { 3.0 } else { 1.0 }
            * (1.0 + self.rank(map, 1) * self.armory_bonus(72) / 100.0)
            * (1.0 + self.support_collection() * self.support_count(map) as f64 / 100.0)
    }
}
