//! The bag a Killroy run drops into.
//!
//! A Killroy encounter drops `Quest69` skulls plus the talent-point,
//! dungeon-credit and pearl items (`js/stats/systems/common/combat-sim-killroy.js`).
//! Every one of those is a big-stack type: `maxCapacityFor` in
//! `combat-sim-inputs.js` caps `dQuest` and `dCurrency` at 9,999,999, so a run
//! needs one bag slot per item type and a stack never overflows inside a run.
//!
//! `ItemPickupInTheFirstPlace` in `combat-sim-rewards.js` fills an existing
//! stack first, then a blank slot, and loses the drop when neither is free.
//! `unlimitedInventory` sends what the bag cannot hold to a virtual overflow
//! instead. A save without `InventoryOrder_<ci>` starts from the upstream
//! default bag: an empty inventory padded with sixteen Blank slots.

use super::row_values;
use crate::save::Save;

/// Every item a Killroy encounter drops, in the game's own keys.
const PICKUPS: [&str; 12] = [
    "Quest69",
    "TalentPoint1",
    "TalentPoint2",
    "TalentPoint3",
    "TalentPoint4",
    "TalentPoint5",
    "TalentPoint6",
    "DungCredits1",
    "DungCredits2",
    "Pearl4",
    "Pearl5",
    "Pearl6",
];

/// The skull item, the one drop the comparison ranks.
pub const SKULLS: &str = "Quest69";

/// The upstream default bag: an empty inventory padded with Blank slots.
const DEFAULT_BLANKS: usize = 16;

/// The bag space a run has: its blank slots and the pickup stacks it already holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bag {
    blanks: usize,
    stacks: u16,
    unlimited: bool,
}

impl Default for Bag {
    fn default() -> Self {
        Self {
            blanks: DEFAULT_BLANKS,
            stacks: 0,
            unlimited: false,
        }
    }
}

impl Bag {
    /// Unlimited inventory: every pickup fits, as the calculator's overflow store does.
    pub fn unlimited() -> Self {
        Self {
            unlimited: true,
            ..Self::default()
        }
    }

    /// Whether one more stack of `item` fits: its own stack, a blank slot, or
    /// unlimited mode.
    pub fn room_for(&self, item: &str) -> bool {
        if self.unlimited || self.blanks > 0 {
            return true;
        }
        index(item).is_some_and(|index| self.stacks & (1 << index) != 0)
    }

    /// Take one dropped stack: an existing stack is reused, a new item spends a
    /// blank slot, and a full bag loses the drop.
    pub(super) fn take(&mut self, item: &str) -> bool {
        if self.unlimited {
            return true;
        }
        let Some(index) = index(item) else {
            return false;
        };
        let bit = 1u16 << index;
        if self.stacks & bit != 0 {
            return true;
        }
        if self.blanks == 0 {
            return false;
        }
        self.blanks -= 1;
        self.stacks |= bit;
        true
    }
}

fn index(item: &str) -> Option<u16> {
    PICKUPS
        .iter()
        .position(|pickup| *pickup == item)
        .map(|index| index as u16)
}

/// The bag a save starts a character's run with, from `InventoryOrder_<ci>`.
///
/// The game's inventory order spans every owned slot, so the array itself is the
/// capacity; a save without it runs the upstream default bag.
pub(super) fn bag(save: &Save, ci: usize) -> Bag {
    let Some(order) = save.get(&format!("InventoryOrder_{ci}")) else {
        return Bag::default();
    };
    let slots = row_values(&order);
    if slots.is_empty() {
        return Bag::default();
    }
    let mut bag = Bag {
        blanks: 0,
        stacks: 0,
        unlimited: false,
    };
    for slot in slots {
        match slot.as_str() {
            None => {}
            Some("Blank") => bag.blanks += 1,
            Some("LockedInvSpace") => {}
            Some(name) => {
                if let Some(index) = index(name) {
                    bag.stacks |= 1 << index;
                }
            }
        }
    }
    bag
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn save(order: serde_json::Value) -> Save {
        Save::new(json!({ "InventoryOrder_0": order }))
    }

    #[test]
    fn a_save_without_an_order_starts_from_the_default_bag() {
        let bag = bag(&Save::new(json!({})), 0);
        assert_eq!(bag, Bag::default());
        assert!(bag.room_for(SKULLS));
        assert!(bag.room_for("Pearl4"));
    }

    #[test]
    fn blanks_and_pickup_stacks_come_from_the_save() {
        let bag = bag(
            &save(json!([
                "Quest69",
                "Blank",
                "LockedInvSpace",
                "Pearl4",
                "Quest25"
            ])),
            0,
        );
        assert_eq!(bag.blanks, 1);
        assert!(bag.room_for(SKULLS), "the existing skull stack is reused");
        assert!(bag.room_for("Pearl4"), "the existing pearl stack is reused");
        assert!(bag.room_for("TalentPoint1"), "a blank slot is free");
    }

    #[test]
    fn a_new_stack_spends_one_blank_slot() {
        let mut bag = bag(&save(json!(["Blank", "Blank"])), 0);
        assert!(bag.room_for("TalentPoint1"));
        assert!(bag.take("TalentPoint1"));
        assert_eq!(bag.blanks, 1);
        assert!(bag.take("TalentPoint1"), "the new stack is reused");
        assert_eq!(bag.blanks, 1);
        assert!(bag.take("DungCredits1"));
        assert_eq!(bag.blanks, 0);
        assert!(!bag.room_for("Pearl4"));
        assert!(!bag.take("Pearl4"));
    }

    #[test]
    fn a_full_bag_keeps_the_stacks_it_already_holds() {
        let mut bag = bag(&save(json!(["Quest69", "Pearl4"])), 0);
        assert!(bag.take(SKULLS));
        assert!(bag.take("Pearl4"));
        assert!(!bag.take("TalentPoint1"), "no blank slot for a new item");
    }

    #[test]
    fn unlimited_mode_takes_everything() {
        let mut bag = Bag::unlimited();
        for item in PICKUPS {
            assert!(bag.room_for(item));
            assert!(bag.take(item));
        }
        assert!(bag.room_for("Pearl6"));
    }
}
