//! Where each twin stands and what its next action does.
//!
//! Ported from `sneakingActionMode`, `captiveTargetForFloor`, `doorHP` and
//! `doorState` in `js/stats/systems/w6/sneaking-math.js`: an untied-less twin
//! feeds its captive, floor zero trains, a Nunchaku hits an unfinished door, a
//! Kunai frees the captive waiting on the floor, and any other weapon does the
//! floor's Jade action.

use super::data;
use super::{FLOOR_COUNT, Job, Loadout};

impl Loadout {
    // ----- Placement and actions -----

    /// Whether a twin is alone on its floor.
    pub(super) fn is_solo(&self, twin: usize) -> bool {
        let Some(entry) = self.twins.get(twin) else {
            return false;
        };
        !self
            .twins
            .iter()
            .take(self.players.min(self.twins.len()))
            .any(|other| other.index != twin && other.floor == entry.floor)
    }

    /// `captiveTargetForFloor`: the first untied-less twin whose captive waits
    /// on `floor`, or `None`.
    fn captive_target(&self, floor: usize) -> Option<usize> {
        self.twins
            .iter()
            .take(self.players.min(self.twins.len()))
            .find(|twin| {
                !twin.untied
                    && data::data()
                        .info
                        .captive_floor
                        .get(twin.index)
                        .copied()
                        .unwrap_or(0.0) as usize
                        == floor
            })
            .map(|twin| twin.index)
    }

    /// The floor whose door is unfinished, if any: upstream `doorState`'s
    /// `activeFloor` for the saved mastery.
    fn active_door_floor(&self) -> Option<usize> {
        (1..FLOOR_COUNT).find(|floor| {
            let max = door_hp(*floor, self.mastery);
            let progress = self.doors.get(*floor).copied().unwrap_or(0.0);
            max > progress
        })
    }

    /// `doorHP(floor, mastery)`: the door's maximum HP at a mastery level.
    /// Whether this floor's door still needs hits.
    fn door_active(&self, floor: usize) -> bool {
        floor > 0 && self.mastery >= self.mastery_max && self.active_door_floor() == Some(floor)
    }

    /// `sneakingActionMode`: what the twin's next action does.
    pub fn job(&self, twin: usize) -> Job {
        let Some(entry) = self.twins.get(twin) else {
            return Job::Jade;
        };
        if !entry.untied {
            return Job::Captive;
        }
        if entry.floor == 0 {
            return Job::Training;
        }
        match entry.weapon_subtype() {
            Some(1) if self.door_active(entry.floor) => Job::Door,
            Some(2) if self.captive_target(entry.floor).is_some() => Job::Untie,
            _ => Job::Jade,
        }
    }
}

/// `doorHP`: the door's maximum HP at a mastery level.
pub fn door_hp(floor: usize, mastery: u32) -> f64 {
    let raw = data::data().info.door_hp.get(floor).copied().unwrap_or(0.0);
    if mastery == 0 {
        return raw;
    }
    if floor == 0 {
        return 1.0;
    }
    // The game rescales NinjaInfo[3] in index order, so by floor eleven the
    // scale row has already become its mastery-scaled value.
    let base = data::data().scales.door_hp;
    let mastery = f64::from(mastery);
    let scale = if floor > 10 {
        0.01 * base * base.powf(mastery)
    } else {
        base
    };
    0.01 * raw * scale.powf(mastery)
}
