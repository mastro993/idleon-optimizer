//! The Sneaking tables, extracted from the upstream JS calculators.
//!
//! `script/extract-sneaking-data.mjs` writes `crates/core/data/sneaking.json`
//! from `js/stats/data/game/custommaps.js` (`NjEQ`), `customlists.js`
//! (`NinjaUpg`, `NinjaInfo`) and `js/stats/systems/w6/sneaking-math.js`
//! (`CHARM_BONUS_TYPES`). Nothing here is guessed: an item key the catalog does
//! not carry has no definition, and every formula reports it instead of
//! substituting a zero.

use serde::Deserialize;
use std::collections::HashMap;
use std::sync::OnceLock;

const SNEAKING_JSON: &str = include_str!("../../data/sneaking.json");

/// The tables the loadout and inventory workspaces read.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Data {
    pub items: HashMap<String, ItemDef>,
    /// `NinjaUpg`: the Ninja Knowledge upgrades in save order.
    pub upgrades: Vec<Upgrade>,
    pub info: Info,
    pub scales: Scales,
    /// `CHARM_BONUS_TYPES`: the readable name of every charm bonus type.
    pub charm_bonus_types: HashMap<String, String>,
    /// The emporium letters `emporiumBonus` tests the `Ninja[102][9]` string for.
    pub emporium_letters: String,
    /// `LegendTalents[idx][2]`: the points each legend talent level pays.
    pub legend_talent_per_point: Vec<f64>,
}

/// One `NjEQ` row: the ninja equipment catalog.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemDef {
    /// 0 = hat, 1 = weapon, 2 = charm, anything else is not equippable.
    #[serde(rename = "type")]
    pub kind: u8,
    /// The weapon class or charm bonus type. Hats carry their hat index here and
    /// the odd gem rows a negative one, so it is not a bonus type for every item.
    pub bonus_type: i32,
    /// The raw key name, with the game's underscores.
    pub name: String,
    /// The hat flavour text, the weapon damage or the charm base value.
    pub stat: String,
    /// The charm description, with `{` and `}` placeholders.
    pub desc: String,
    /// The charm's maximum value at level infinity.
    pub max: f64,
}

impl ItemDef {
    /// The name as the calculator prints it: the game stores underscores.
    pub fn display_name(&self) -> String {
        self.name.replace('_', " ")
    }

    /// The weapon damage, for the `Damage: N` line upstream prints.
    pub fn damage(&self) -> f64 {
        self.stat.trim().parse().unwrap_or(0.0)
    }

    /// Whether the name marks a Gold inventory charm, which never equips.
    pub fn is_gold(&self) -> bool {
        self.name.contains("Gold ") || self.name.contains("Gold_")
    }
}

/// One `NinjaUpg` row.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Upgrade {
    /// The bonus each level pays: `NinjaUpg[t][5]`.
    pub per_level: f64,
}

/// The `NinjaInfo` rows the loadout formulas read.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Info {
    /// Door HP per floor, before the mastery scale.
    pub door_hp: Vec<f64>,
    /// The floor each twin's captive waits on.
    pub captive_floor: Vec<f64>,
    /// The untie progress each twin needs to be freed.
    pub untie_required: Vec<f64>,
    /// The detection base per floor, before the mastery scale.
    pub detection_base: Vec<f64>,
}

/// The mastery scales the game applies to the `NinjaInfo` rows.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Scales {
    pub detection: f64,
    pub door_hp: f64,
}

/// The extracted tables, parsed once.
pub fn data() -> &'static Data {
    static DATA: OnceLock<Data> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(SNEAKING_JSON).expect("the extracted sneaking data is valid JSON")
    })
}

/// The catalog entry for an item key, when the game has one.
pub fn item(key: &str) -> Option<&'static ItemDef> {
    data().items.get(key)
}

/// The readable label of a charm bonus type, or `Unknown charm bonus`.
pub fn charm_bonus_label(bonus_type: i32) -> &'static str {
    data()
        .charm_bonus_types
        .get(&bonus_type.to_string())
        .map(String::as_str)
        .unwrap_or("Unknown charm bonus")
}

/// The weapon class label upstream prints beside a weapon: `equipInfo`'s
/// `WEAPON_CLASS` map.
pub fn weapon_class(bonus_type: i32) -> &'static str {
    match bonus_type {
        0 => "Gloves",
        1 => "Nunchaku",
        2 => "Kunai",
        3 => "Katana",
        _ => "Weapon",
    }
}

/// The emporium letter at `index`, which the saved string has to contain.
pub fn emporium_letter(index: usize) -> Option<char> {
    data().emporium_letters.chars().nth(index)
}

/// `LegendTalents[idx][2]`: the points one legend talent level pays.
pub fn legend_per_point(index: usize) -> f64 {
    data()
        .legend_talent_per_point
        .get(index)
        .copied()
        .unwrap_or(0.0)
}

/// `NinjaUpg[idx]`, when the table carries it.
pub fn upgrade(index: usize) -> Option<&'static Upgrade> {
    data().upgrades.get(index)
}
