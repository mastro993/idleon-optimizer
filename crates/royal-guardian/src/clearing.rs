//! The raw clearing values: the militia rate, the talent behind it and the
//! cross-system factors it multiplies.
//!
//! Ported from `outpostClearMetrics`, `XtraClearKillz`, `unitSpecEffect`,
//! `computeBigFishBonus` and `jellyCompletionBonus`.

use super::*;

impl RoyalGuardian {
    /// Whether the save carries the Jelly Operator rows the reward table reads:
    /// upstream `hasJellyData`.
    pub(super) fn jelly_available(&self) -> bool {
        self.jelly
            .as_ref()
            .is_some_and(idleon_jelly::Jelly::has_data)
    }

    /// The Jelly Operator reward at an obstruction index: upstream
    /// `jellyCompletionBonus`, which the shared Jelly port owns.
    pub fn jelly_bonus(&self, index: usize) -> f64 {
        self.jelly
            .as_ref()
            .filter(|jelly| jelly.has_data())
            .map_or(0.0, |jelly| jelly.completion_bonus(index))
    }

    /// A Spelunk big-fish level's bonus: upstream `computeBigFishBonus`.
    pub fn big_fish_bonus(&self, index: usize) -> f64 {
        let level = self.spelunk_cell(11, index);
        if level <= 0.0 {
            return 0.0;
        }
        let base = data::data()
            .big_fish_base
            .get(index)
            .copied()
            .unwrap_or(0.0);
        level / (100.0 + level) * base
    }

    /// The highest purchased Xtra Clear Killz level across the characters.
    pub fn xtra_clear_level(&self) -> f64 {
        self.talent_level(XTRA_CLEAR_KILLZ)
    }

    /// The talent's effect: upstream's `decayMulti` curve on the purchased level.
    pub fn talent_multi(&self) -> f64 {
        self.talent_value(XTRA_CLEAR_KILLZ).max(1.0)
    }

    /// The militia kill multiplier: upstream `XtraClearKillz`.
    pub fn xtra_clear_killz(&self) -> f64 {
        self.talent_multi()
            * (1.0 + (self.orblet_bonus(3) + self.big_fish_bonus(6)) / 100.0)
            * (1.0 + self.jelly_bonus(2) / 100.0)
    }

    /// An outpost stat's effect, by the upstream `unitSpecEffect` index.
    pub fn unit_spec(&self, index: usize) -> f64 {
        match index {
            0 => 50.0 + self.armory_bonus(19),
            1 => self.armory_bonus(20),
            2 => 25.0 + self.armory_bonus(21),
            3 => self.armory_bonus(22),
            4 => 4000.0 * (1.0 + self.armory_bonus(23) / 100.0) * self.xtra_clear_killz(),
            5 => self.armory_bonus(24),
            6 => self.armory_bonus(25),
            7 => self.armory_bonus(26),
            _ => 0.0,
        }
    }

    /// How many kills an outpost's militia clears per hour.
    pub fn militia_clear_rate(&self, map: usize) -> f64 {
        self.militia_assignment_count(map) * self.unit_spec(4)
    }

    /// One outpost's raw clearing values: upstream `outpostClearMetrics`.
    pub fn clear(&self, map: usize) -> Clear {
        let requirement = self.kill_requirement(map);
        let assignments = self.militia_assignment_count(map);
        let rate_per_hour = self.militia_clear_rate(map);
        let saved = self.map_row(map);
        let progress = saved
            .first()
            .and_then(json::number)
            .map(|value| value.max(0.0))
            .unwrap_or(0.0);
        let remaining = (requirement - progress).max(0.0);
        let mut clear = Clear {
            map,
            state: ClearState::Unavailable,
            active: false,
            progress,
            requirement,
            remaining,
            rate_per_hour,
            assignments,
            eta_hours: None,
            reason: None,
        };
        if saved.len() >= 3 {
            clear.state = ClearState::Built;
            return clear;
        }
        // Only a one-cell row accrues kills: a missing row and a row that has
        // already grown its second cell both stop the militia's progress.
        if saved.len() != 1 {
            clear.state = ClearState::NonAccruing;
            clear.reason = Some(if saved.is_empty() {
                "RoyalMaps row is missing".to_owned()
            } else {
                format!("RoyalMaps row length {} does not accrue", saved.len())
            });
            return clear;
        }
        if remaining <= 0.0 {
            clear.state = ClearState::Secured;
            return clear;
        }
        if assignments <= 0.0 || rate_per_hour <= 0.0 {
            clear.state = ClearState::WaitingMilitia;
            clear.reason = Some("no militia assignment".to_owned());
            return clear;
        }
        clear.state = ClearState::Clearing;
        clear.active = true;
        clear.eta_hours = Some(remaining / rate_per_hour);
        clear
    }
}
