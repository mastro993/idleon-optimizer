//! The editable loadout: the twins, their four equipment slots, the inventory
//! and the slot rules.
//!
//! Ported from `parseTwinData`, `parseTwinEquip` and `isTwinUntied` in
//! `js/stats/systems/w6/sneaking-math.js`, the slot model in `sneaking-calc.html`
//! and `canEquipCharmPair` in `js/stats/systems/w6/sneaking-optimizer.js`. The
//! formulas the editor's numbers come from live in [`super::math`], and where
//! each twin stands in [`super::placement`].
//!
//! An edit moves two items, so the editor never clones a charm the account does
//! not own, and every value is recomputed from the slots, so a view that reads a
//! twin after an edit never shows a stale figure.

use super::Account;
use super::data::{self, ItemDef};
use super::{CHARMED_PERK, INVENTORY_SLOTS, Item, SLOT_COUNT, SlotKind};
use super::{charm_stat, weapon_stat};

/// The charm the game limits to one copy across the account: Scroll of Power.
pub const SCROLL_OF_POWER: &str = "NjTr7";

/// One twin: its saved placement, its four slots and the copy they started from.
#[derive(Clone, Debug, PartialEq)]
pub struct Twin {
    pub index: usize,
    pub name: String,
    /// `Ninja[twin][0]`: the floor the twin is assigned to.
    pub floor: usize,
    /// `Ninja[twin][3]`: the untie progress.
    pub untie_progress: f64,
    /// `NinjaInfo[8][twin]`: the progress untieing needs.
    pub untie_required: f64,
    /// Whether the twin has been freed.
    pub untied: bool,
    /// `Lv0_<twin>[17]`: the twin's Sneaking level.
    pub level: u32,
    pub slots: [Item; SLOT_COUNT],
    saved: [Item; SLOT_COUNT],
}

impl Twin {
    /// Build one twin from the save and the slots it carries.
    pub(super) fn new(
        index: usize,
        name: String,
        floor: usize,
        untie_progress: f64,
        untie_required: f64,
        level: u32,
        slots: [Item; SLOT_COUNT],
    ) -> Self {
        Self {
            index,
            name,
            floor,
            untie_progress,
            untie_required,
            untied: untie_progress >= untie_required,
            level,
            saved: slots.clone(),
            slots,
        }
    }

    /// The item in one of the four slots.
    pub fn slot(&self, slot: SlotKind) -> &Item {
        &self.slots[slot.index()]
    }

    /// Whether a slot differs from the save.
    pub fn changed(&self, slot: SlotKind) -> bool {
        self.slots[slot.index()] != self.saved[slot.index()]
    }

    /// Whether any slot differs from the save.
    pub(crate) fn any_changed(&self) -> bool {
        (0..SLOT_COUNT).any(|index| self.slots[index] != self.saved[index])
    }

    /// The equipped weapon's subtype: 0 Gloves, 1 Nunchaku, 2 Kunai, 3 Katana.
    pub(crate) fn weapon_subtype(&self) -> Option<i32> {
        let def = self.slots[1].def()?;
        (def.kind == 1).then_some(def.bonus_type)
    }
}

/// The charm bonus map: one total per bonus type, indexed by `NjEQ` bonus type.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Bonuses {
    pub(super) values: [f64; 25],
}

impl Bonuses {
    /// The total for one charm bonus type, zero when no charm carries it.
    pub fn get(&self, bonus_type: i32) -> f64 {
        usize::try_from(bonus_type)
            .ok()
            .and_then(|index| self.values.get(index))
            .copied()
            .unwrap_or(0.0)
    }

    /// Every bonus type that carries a value, in type order.
    pub fn nonzero(&self) -> impl Iterator<Item = (i32, f64)> + '_ {
        self.values
            .iter()
            .copied()
            .enumerate()
            .filter(|(_, value)| *value != 0.0)
            .map(|(bonus_type, value)| (bonus_type as i32, value))
    }
}

/// One Gold charm the inventory scan picked for its bonus type.
#[derive(Clone, Debug, PartialEq)]
pub struct GoldPick {
    /// The inventory row the scan read, `0..40`.
    pub row: usize,
    pub key: String,
    pub raw: f64,
}

/// The Gold inventory bonuses and the charm that pays each of them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GoldBonuses {
    pub(super) values: [f64; 25],
    pub(super) selected: [Option<GoldPick>; 25],
}

impl GoldBonuses {
    /// The multiplied bonus for one type, zero when the inventory has none.
    pub fn get(&self, bonus_type: i32) -> f64 {
        usize::try_from(bonus_type)
            .ok()
            .and_then(|index| self.values.get(index))
            .copied()
            .unwrap_or(0.0)
    }

    /// The charm the scan selected for one type.
    pub fn selected(&self, bonus_type: i32) -> Option<&GoldPick> {
        usize::try_from(bonus_type)
            .ok()
            .and_then(|index| self.selected.get(index))
            .and_then(Option::as_ref)
    }
}

/// What a twin's next action is, which mirrors the game's Ninja action switch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Job {
    /// Not untied yet: the action goes to the captive.
    Captive,
    /// Floor zero trains rather than sneaks.
    Training,
    /// A Nunchaku on an active door hits the door.
    Door,
    /// A Kunai frees the captive waiting on the floor.
    Untie,
    /// Any other weapon does the floor's Jade action.
    Jade,
}

impl Job {
    /// The word the saved floors line prints.
    pub fn label(self) -> &'static str {
        match self {
            Job::Captive => "captive",
            Job::Training => "training",
            Job::Door => "door",
            Job::Untie => "untie",
            Job::Jade => "Jade",
        }
    }
}

/// A slot that does not fit the rules the game enforces.
#[derive(Clone, Debug, PartialEq)]
pub enum Issue {
    /// The catalog has no such item, so no formula can read it.
    UnknownItem {
        twin: usize,
        slot: SlotKind,
        key: String,
    },
    /// The item does not belong in that slot: a charm in the hat row, and so on.
    WrongSlot {
        twin: usize,
        slot: SlotKind,
        key: String,
    },
    /// Two copies of the same charm without the Charmed emporium bonus.
    SameCharmPair { twin: usize, key: String },
    /// Scroll of Power equipped more than once; the game allows one.
    ScrollEquippedTwice { twins: Vec<usize> },
}

impl Issue {
    /// The twin the issue is about, when it is about one.
    pub fn twin(&self) -> Option<usize> {
        match self {
            Issue::UnknownItem { twin, .. }
            | Issue::WrongSlot { twin, .. }
            | Issue::SameCharmPair { twin, .. } => Some(*twin),
            Issue::ScrollEquippedTwice { twins } => twins.first().copied(),
        }
    }

    /// The sentence the loadout card prints.
    pub fn message(&self) -> String {
        match self {
            Issue::UnknownItem { slot, key, .. } => {
                format!(
                    "{}: unknown item “{key}”; no formula can read it",
                    slot.label()
                )
            }
            Issue::WrongSlot { slot, key, .. } => {
                format!("{}: “{key}” does not fit this slot", slot.label())
            }
            Issue::SameCharmPair { key, .. } => format!(
                "Both charm slots hold “{key}”; the game only allows that with the Charmed emporium bonus"
            ),
            Issue::ScrollEquippedTwice { twins } => {
                let names: Vec<String> = twins
                    .iter()
                    .map(|twin| format!("twin {}", twin + 1))
                    .collect();
                format!(
                    "Scroll of Power is equipped {} times ({}); the game allows one",
                    twins.len(),
                    names.join(", ")
                )
            }
        }
    }
}

/// Why a slot edit was refused.
#[derive(Clone, Debug, PartialEq)]
pub enum EquipError {
    /// The twin index is outside the save's roster.
    UnknownTwin,
    /// The slot index is outside `0..4`.
    UnknownSlot,
    /// The inventory index is outside the visible grid.
    UnknownInventory,
    /// The inventory row holds nothing to equip.
    EmptySlot,
    /// The catalog has no such item.
    UnknownItem(String),
    /// The item does not fit the slot.
    WrongSlot(String),
    /// The pair would repeat a charm without the Charmed emporium bonus.
    SameCharmPair(String),
    /// Scroll of Power is already on another twin.
    ScrollEquipped,
    /// No inventory row is free to take the item being removed.
    NoInventorySpace,
}

impl std::fmt::Display for EquipError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EquipError::UnknownTwin => write!(formatter, "no such twin"),
            EquipError::UnknownSlot => write!(formatter, "no such slot"),
            EquipError::UnknownInventory => write!(formatter, "no such inventory slot"),
            EquipError::EmptySlot => write!(formatter, "that inventory slot is empty"),
            EquipError::UnknownItem(key) => write!(formatter, "unknown item “{key}”"),
            EquipError::WrongSlot(key) => write!(formatter, "“{key}” does not fit this slot"),
            EquipError::SameCharmPair(key) => {
                write!(formatter, "“{key}” is already in the other charm slot")
            }
            EquipError::ScrollEquipped => {
                write!(formatter, "Scroll of Power is already equipped")
            }
            EquipError::NoInventorySpace => write!(formatter, "no free inventory slot"),
        }
    }
}

impl std::error::Error for EquipError {}

/// The save-backed loadout: the twins, the inventory and the account terms.
#[derive(Clone, Debug, PartialEq)]
pub struct Loadout {
    pub(super) twins: Vec<Twin>,
    /// Every inventory row the game stores: 39 visible plus the one row beyond
    /// the grid that the Gold scan reads.
    pub(super) inventory: Vec<Item>,
    pub(super) saved_inventory: Vec<Item>,
    pub(super) knowledge: Vec<f64>,
    pub(super) omni: Vec<f64>,
    pub(super) symbols: Vec<f64>,
    pub(super) flowers: Vec<f64>,
    pub(super) legend_points: Vec<f64>,
    pub(super) gem_levels: Vec<f64>,
    pub(super) doors: Vec<f64>,
    /// `Exp0_<n>[17]`, each twin's saved Sneaking EXP for the charm transfer.
    pub(super) exp: Vec<f64>,
    /// `Ninja[102][10]`, the item-find phase.
    pub(super) item_find_phase: u32,
    pub(super) mastery: u32,
    pub(super) mastery_max: u32,
    pub(super) charmed: bool,
    pub(super) players: usize,
    pub(super) account: Account,
    pub(super) missing: Vec<String>,
}

impl Loadout {
    /// The model's constructor for the save reader and the tests.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        twins: Vec<Twin>,
        inventory: Vec<Item>,
        knowledge: Vec<f64>,
        omni: Vec<f64>,
        symbols: Vec<f64>,
        flowers: Vec<f64>,
        legend_points: Vec<f64>,
        gem_levels: Vec<f64>,
        doors: Vec<f64>,
        exp: Vec<f64>,
        item_find_phase: u32,
        mastery: u32,
        mastery_max: u32,
        charmed: bool,
        players: usize,
        missing: Vec<String>,
    ) -> Self {
        Self {
            saved_inventory: inventory.clone(),
            twins,
            inventory,
            knowledge,
            omni,
            symbols,
            flowers,
            legend_points,
            gem_levels,
            doors,
            exp,
            item_find_phase,
            mastery,
            mastery_max,
            charmed,
            players,
            account: Account::default(),
            missing,
        }
    }

    /// The account terms, for a caller that resolved the all-talent multiplier.
    pub fn with_account(mut self, account: Account) -> Self {
        self.account = account;
        self
    }

    /// The account terms the formulas resolved.
    pub fn account(&self) -> Account {
        self.account
    }

    pub fn twins(&self) -> &[Twin] {
        &self.twins
    }

    pub fn twin(&self, index: usize) -> Option<&Twin> {
        self.twins.get(index)
    }

    /// The visible inventory grid, the game's 39 slots.
    pub fn inventory(&self) -> &[Item] {
        &self.inventory[..INVENTORY_SLOTS.min(self.inventory.len())]
    }

    /// The Ninja Knowledge levels (`Ninja[103]`).
    pub fn knowledge(&self) -> &[f64] {
        &self.knowledge
    }

    /// The saved mastery and its maximum (`OptLacc[231]`, `[232]`).
    pub fn mastery(&self) -> (u32, u32) {
        (self.mastery, self.mastery_max)
    }

    /// Whether the account owns the Charmed emporium bonus.
    pub fn charmed(&self) -> bool {
        self.charmed
    }

    /// How many twins the save carries.
    pub fn players(&self) -> usize {
        self.players
    }

    /// Save fields the loader could not read, named for the UI.
    pub fn missing(&self) -> &[String] {
        &self.missing
    }

    /// Whether any slot differs from the save.
    pub fn changed(&self) -> bool {
        self.twins.iter().any(Twin::any_changed) || self.inventory != self.saved_inventory
    }

    /// Put every slot and the inventory back to the save.
    pub fn reset(&mut self) {
        for twin in &mut self.twins {
            twin.slots = twin.saved.clone();
        }
        self.inventory = self.saved_inventory.clone();
    }

    /// Move an inventory item into a slot, swapping whatever the slot held.
    pub fn equip(&mut self, twin: usize, slot: SlotKind, row: usize) -> Result<(), EquipError> {
        if twin >= self.twins.len() {
            return Err(EquipError::UnknownTwin);
        }
        if row >= INVENTORY_SLOTS.min(self.inventory.len()) {
            return Err(EquipError::UnknownInventory);
        }
        let item = self.inventory[row].clone();
        if item.is_blank() {
            return Err(EquipError::EmptySlot);
        }
        self.check_slot(twin, slot, &item)?;
        std::mem::swap(
            &mut self.twins[twin].slots[slot.index()],
            &mut self.inventory[row],
        );
        Ok(())
    }

    /// Move a slot's item into the first free inventory row.
    pub fn unequip(&mut self, twin: usize, slot: SlotKind) -> Result<(), EquipError> {
        if twin >= self.twins.len() {
            return Err(EquipError::UnknownTwin);
        }
        if self.twins[twin].slots[slot.index()].is_blank() {
            return Err(EquipError::EmptySlot);
        }
        let free = (0..INVENTORY_SLOTS)
            .find(|row| self.inventory[*row].is_blank())
            .ok_or(EquipError::NoInventorySpace)?;
        std::mem::swap(
            &mut self.twins[twin].slots[slot.index()],
            &mut self.inventory[free],
        );
        Ok(())
    }

    /// Whether an item may go into a slot, the rules `equip` enforces.
    pub fn check_slot(&self, twin: usize, slot: SlotKind, item: &Item) -> Result<(), EquipError> {
        let Some(def) = item.def() else {
            return Err(EquipError::UnknownItem(item.key.clone()));
        };
        if !slot.accepts(def) {
            return Err(EquipError::WrongSlot(item.key.clone()));
        }
        if let SlotKind::Charm(charm) = slot {
            let other = &self.twins[twin].slots[SlotKind::Charm(1 - charm).index()];
            if !can_equip_pair(&item.key, &other.key, self.charmed) {
                return Err(EquipError::SameCharmPair(item.key.clone()));
            }
            if item.key == SCROLL_OF_POWER && self.scroll_twins(twin).next().is_some() {
                return Err(EquipError::ScrollEquipped);
            }
        }
        Ok(())
    }

    /// The twins other than `twin` that hold Scroll of Power.
    fn scroll_twins(&self, twin: usize) -> impl Iterator<Item = usize> + '_ {
        self.twins
            .iter()
            .filter(move |candidate| candidate.index != twin)
            .filter(|candidate| {
                candidate
                    .slots
                    .iter()
                    .any(|item| item.key == SCROLL_OF_POWER)
            })
            .map(|candidate| candidate.index)
    }

    /// Every slot the game's rules refuse, across the twins.
    pub fn issues(&self) -> Vec<Issue> {
        let mut issues = Vec::new();
        for twin in &self.twins {
            for slot in SlotKind::ALL {
                let item = twin.slot(slot);
                if item.is_blank() {
                    continue;
                }
                match item.def() {
                    None => issues.push(Issue::UnknownItem {
                        twin: twin.index,
                        slot,
                        key: item.key.clone(),
                    }),
                    Some(def) if !slot.accepts(def) => issues.push(Issue::WrongSlot {
                        twin: twin.index,
                        slot,
                        key: item.key.clone(),
                    }),
                    Some(_) => {}
                }
            }
            let first = twin.slot(SlotKind::Charm(0));
            let second = twin.slot(SlotKind::Charm(1));
            if !first.is_blank()
                && !second.is_blank()
                && !can_equip_pair(&first.key, &second.key, self.charmed)
            {
                issues.push(Issue::SameCharmPair {
                    twin: twin.index,
                    key: first.key.clone(),
                });
            }
        }
        let scrolls: Vec<usize> = self
            .twins
            .iter()
            .filter(|twin| twin.slots.iter().any(|item| item.key == SCROLL_OF_POWER))
            .map(|twin| twin.index)
            .collect();
        if scrolls.len() > 1 {
            issues.push(Issue::ScrollEquippedTwice { twins: scrolls });
        }
        issues
    }

    /// The issues about one twin.
    pub fn twin_issues(&self, twin: usize) -> Vec<Issue> {
        self.issues()
            .into_iter()
            .filter(|issue| issue.twin() == Some(twin))
            .collect()
    }
}

/// `canEquipCharmPair`: a same-key pair needs the Charmed emporium bonus, and
/// Scroll of Power is account-unique.
pub fn can_equip_pair(first: &str, second: &str, charmed: bool) -> bool {
    if first.is_empty() || first == "Blank" || second.is_empty() || second == "Blank" {
        return true;
    }
    if first == SCROLL_OF_POWER && second == SCROLL_OF_POWER {
        return false;
    }
    first != second || charmed
}

/// `equipInfo`: the type, the readable name and the effect line upstream prints
/// in a slot's tooltip.
pub fn equip_info(item: &Item) -> Option<EquipInfo> {
    let def: &ItemDef = item.def()?;
    let name = def.display_name();
    match def.kind {
        0 => Some(EquipInfo {
            label: "Hat".into(),
            name,
            detail: def.stat.replace('_', " "),
            stat: None,
        }),
        1 => Some(EquipInfo {
            label: data::weapon_class(def.bonus_type).into(),
            name,
            detail: format!("Damage: {}", def.damage()),
            stat: Some(weapon_stat(&item.key, f64::from(item.level))),
        }),
        2 => {
            let stat = charm_stat(&item.key, f64::from(item.level));
            let detail = def
                .desc
                .replace('_', " ")
                .replace('{', &format!("{stat:.1}"))
                .replace('}', &format!("{:.2}", 1.0 + stat / 100.0));
            Some(EquipInfo {
                label: data::charm_bonus_label(def.bonus_type).into(),
                name,
                detail,
                stat: Some(stat),
            })
        }
        _ => Some(EquipInfo {
            label: "Unknown equipment type".into(),
            name,
            detail: String::new(),
            stat: None,
        }),
    }
}

/// One item's tooltip text, as `equipInfo` describes it.
#[derive(Clone, Debug, PartialEq)]
pub struct EquipInfo {
    pub label: String,
    pub name: String,
    pub detail: String,
    pub stat: Option<f64>,
}

/// The emporium string's Charmed flag, from `Ninja[102][9]`.
pub fn charmed_perk(emporium: &str) -> bool {
    data::emporium_letter(CHARMED_PERK).is_some_and(|letter| emporium.contains(letter))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_same_key_pair_needs_the_charmed_bonus() {
        assert!(!can_equip_pair("NjTr4", "NjTr4", false));
        assert!(can_equip_pair("NjTr4", "NjTr4", true));
        assert!(can_equip_pair("NjTr4", "NjTr5", false));
        assert!(can_equip_pair("Blank", "NjTr4", false));
    }

    #[test]
    fn scroll_of_power_never_pairs() {
        assert!(!can_equip_pair(SCROLL_OF_POWER, SCROLL_OF_POWER, true));
    }
}
