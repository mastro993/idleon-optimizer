//! The Sneaking (W6) loadout and inventory: the save-backed charm and ninja kit
//! model behind `sneaking-calc.html`'s loadout workspace.
//!
//! Ported from `js/stats/systems/w6/sneaking-math.js` (the twin, equipment,
//! charm and Ninja Knowledge reads) and the loadout half of
//! `js/stats/systems/w6/sneaking-stealth.js` (`twinCharmBonuses`,
//! `goldInventoryBonusDetails`, `computeActionSpeed`, `computeTwinStealth`),
//! with the slot rules in `sneaking-calc.html` and
//! `js/stats/systems/w6/sneaking-optimizer.js` (`canEquipCharmPair`).
//!
//! The model edits a copy of the save's `Ninja` rows: a slot swap moves the two
//! items, never clones them, so the editor cannot invent a charm the account
//! does not own. The save itself stays read-only.
//!
//! Scope: this is the loadout's own maths — the item stats, the charm bonus map,
//! the symbol and Gold inventory terms, action speed, KO recovery, the level and
//! charm stealth factors, the ally and flower terms, detection and the action
//! each twin is on. The account-wide stealth multiplier stack (compass, gambit,
//! cards, statues, voting, lamp, hole and the rest) belongs to the breakdowns
//! (SN-02); [`Account`] carries the one all-talent term those terms need and the
//! neutral default is exact when the multiplier is absent.

mod account;
pub mod breakdown;
pub mod data;
mod loadout;
mod math;
mod placement;
mod rates;
mod save;
#[cfg(test)]
mod tests;

pub use account::{Account, DoorAccount, ExpAccount, JadeAccount};
pub use data::{ItemDef, charm_bonus_label, item};
pub use loadout::{
    Bonuses, EquipError, EquipInfo, GoldBonuses, GoldPick, Issue, Job, Loadout, SCROLL_OF_POWER,
    Twin, can_equip_pair, charmed_perk, equip_info,
};
pub use math::{
    AllyContribution, action_speed, charm_stat, detection_base, detection_chance, ko_duration,
    ninja_bonus, stealth_for_detection, stealth_value, weapon_stat,
};
pub use placement::door_hp;
pub use rates::{
    ActionMode, DoorDamage, DoorState, ExpAction, JadeCoin, JadeHour, Runtime, UntieProgress,
};

use serde::{Deserialize, Serialize};

/// The floors NinjaInfo describes.
pub const FLOOR_COUNT: usize = 12;
/// The twins the save carries.
pub const TWIN_COUNT: usize = 12;
/// The visible inventory slots, the game's native `InvSpace`.
pub const INVENTORY_SLOTS: usize = 39;
/// The inventory rows the Gold scan walks: upstream reads one row further than
/// the visible grid, because `NJbonusPerms` does.
pub const INVENTORY_SCAN: usize = 40;
/// The four equipment slots one twin carries.
pub const SLOT_COUNT: usize = 4;
/// The first twin row in `Ninja`; each twin's four equipment rows follow.
pub const EQUIP_START: usize = 12;
/// The first inventory row in `Ninja`.
pub const INVENTORY_START: usize = 60;
/// The Ninja Knowledge levels row.
pub const KNOWLEDGE_ROW: usize = 103;
/// The door progress row.
pub const DOOR_ROW: usize = 100;
/// The emporium row, whose cell 9 carries the unlocked bonuses.
pub const EMPORIUM_ROW: usize = 102;
/// The Charmed emporium bonus: `emporiumBonus(3, …)`.
pub const CHARMED_PERK: usize = 3;

/// One ninja item: its catalog key and its level. `Blank` is an empty slot.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Item {
    pub key: String,
    pub level: u32,
}

impl Item {
    /// The empty slot every save writes.
    pub fn blank() -> Self {
        Self {
            key: "Blank".into(),
            level: 0,
        }
    }

    /// An item at a level.
    pub fn new(key: impl Into<String>, level: u32) -> Self {
        Self {
            key: key.into(),
            level,
        }
    }

    /// Whether the slot holds nothing.
    pub fn is_blank(&self) -> bool {
        self.key.is_empty() || self.key == "Blank"
    }

    /// The catalog entry, when the game has one.
    pub fn def(&self) -> Option<&'static ItemDef> {
        item(&self.key)
    }

    /// The name the calculator prints, or the raw key for an unknown item.
    pub fn name(&self) -> String {
        self.def()
            .map_or_else(|| self.key.clone(), ItemDef::display_name)
    }
}

/// Which of a twin's four slots an index names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlotKind {
    Hat,
    Weapon,
    Charm(usize),
}

impl SlotKind {
    /// Every slot in save order: hat, weapon, charm one, charm two.
    pub const ALL: [SlotKind; SLOT_COUNT] = [
        SlotKind::Hat,
        SlotKind::Weapon,
        SlotKind::Charm(0),
        SlotKind::Charm(1),
    ];

    /// The slot at a save index, `0..4`.
    pub fn of(index: usize) -> Option<Self> {
        Self::ALL.get(index).copied()
    }

    /// The save index of this slot within a twin's four rows.
    pub fn index(self) -> usize {
        match self {
            SlotKind::Hat => 0,
            SlotKind::Weapon => 1,
            SlotKind::Charm(charm) => 2 + charm,
        }
    }

    /// The label upstream prints above the slot.
    pub fn label(self) -> &'static str {
        match self {
            SlotKind::Hat => "Hat",
            SlotKind::Weapon => "Weapon",
            SlotKind::Charm(0) => "Charm 1",
            SlotKind::Charm(_) => "Charm 2",
        }
    }

    /// Whether the catalog entry fits this slot: hats are type 0, weapons type
    /// 1 and charms type 2, the game's own slot model.
    pub fn accepts(self, def: &ItemDef) -> bool {
        match self {
            SlotKind::Hat => def.kind == 0,
            SlotKind::Weapon => def.kind == 1,
            SlotKind::Charm(_) => def.kind == 2,
        }
    }
}
