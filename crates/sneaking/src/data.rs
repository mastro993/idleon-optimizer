//! The Sneaking tables, extracted from the upstream JS calculators.
//!
//! `script/extract-sneaking-data.mjs` writes `crates/sneaking/data/sneaking.json`
//! from `js/stats/data/game/custommaps.js` (`NjEQ`), `customlists.js`
//! (`NinjaUpg`, `NinjaInfo`) and `js/stats/systems/w6/sneaking-math.js`
//! (`CHARM_BONUS_TYPES`). Nothing here is guessed: an item key the catalog does
//! not carry has no definition, and every formula reports it instead of
//! substituting a zero.

use serde::Deserialize;
use std::collections::HashMap;
use std::sync::OnceLock;

const SNEAKING_JSON: &str = include_str!("../data/sneaking.json");

/// The tables the loadout, inventory and breakdown workspaces read.
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
    /// The account-wide tables the breakdown trees read.
    pub account: AccountTables,
}

/// The account-wide systems behind the breakdown trees' factors.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountTables {
    /// `HolesInfo[37]`: the monument bonus per slot.
    pub monument_info: Vec<f64>,
    /// `TaskShopDesc[5][3][11]`: the Jade task-shop bonus per level.
    pub task_shop_jade_base: f64,
    pub guild14: Curve,
    /// `MSA_Bonus(idx)`'s static config.
    pub msa: Vec<MsaConfig>,
    /// `sigilTiers(idx)` for every Summoning sigil.
    pub sigils: Vec<Option<Vec<f64>>>,
    /// `AlchemyDescription[4]`: each vial's effect and curve.
    pub vials: Vec<VialRow>,
    /// `MealINFO`: each meal's per-level bonus and effect.
    pub meals: Vec<MealRow>,
    pub bubbles: Bubbles,
    /// `TalentDescriptions` curves the account terms read.
    pub talents: TalentTables,
    /// `NinjaInfo[38]`: the voting bonus values.
    pub voting_bonuses: Vec<f64>,
    /// `StarSigns[i][0]`: the name each index unlocks under.
    pub star_sign_names: Vec<String>,
    pub star_sign_bonuses: StarSignBonuses,
    /// `buildStampTypeMap` for the stamp effects the trees read.
    pub stamp_types: HashMap<String, Vec<StampRow>>,
    pub rift_ranks: Vec<f64>,
    pub zenith_per_level: f64,
}

/// One formula curve from a game table.
#[derive(Clone, Debug, Deserialize)]
pub struct Curve {
    pub x1: f64,
    pub x2: f64,
    pub formula: String,
}

/// `MSA_Bonus`'s per-index gate, multiplier and star offset.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MsaConfig {
    /// 0 = superbit, 1 = emporium.
    pub gate: u8,
    pub gate_idx: usize,
    pub mult: f64,
    pub offset: f64,
}

/// One alchemy vial's effect and curve.
#[derive(Debug, Deserialize)]
pub struct VialRow {
    pub index: usize,
    pub effect: String,
    #[serde(rename = "kind")]
    pub formula: String,
    pub x1: f64,
    pub x2: f64,
}

/// One meal's per-level bonus and effect key.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MealRow {
    pub index: usize,
    pub per_level: f64,
    pub effect: String,
}

/// The bubbles the trees read.
#[derive(Debug, Deserialize)]
pub struct Bubbles {
    pub a10: BubbleParam,
    pub y8: BubbleParam,
    pub y9: BubbleParam,
}

/// One bubble's curve, from `bubbleParams`.
#[derive(Clone, Debug, Deserialize)]
pub struct BubbleParam {
    pub cauldron: usize,
    pub index: usize,
    pub x1: f64,
    pub x2: f64,
    pub formula: String,
}

/// The talent curves the account terms read.
#[derive(Debug, Deserialize)]
pub struct TalentTables {
    #[serde(rename = "56")]
    pub statue: Curve,
    #[serde(rename = "207")]
    pub farm_rank: Curve,
    #[serde(rename = "431")]
    pub sneaking_exp_add: Curve,
    #[serde(rename = "432")]
    pub talent_432: Curve,
    pub talent431tab2: Curve,
}

/// The static star-sign bonuses for the three effects the trees read.
#[derive(Debug, Deserialize)]
pub struct StarSignBonuses {
    #[serde(rename = "Stealth")]
    pub stealth: HashMap<String, f64>,
    #[serde(rename = "Jade")]
    pub jade: HashMap<String, f64>,
    #[serde(rename = "SneakExp")]
    pub sneak_exp: HashMap<String, f64>,
}

/// One stamp row of an effect type.
#[derive(Debug, Deserialize)]
pub struct StampRow {
    pub cat: usize,
    pub idx: usize,
    pub x1: f64,
    pub x2: f64,
    pub formula: String,
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
    pub(crate) fn is_gold(&self) -> bool {
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
    /// The Jade base per floor, before the mastery scale.
    pub jade_base: Vec<f64>,
    /// The EXP base per floor, before the mastery scale.
    pub exp_base: Vec<f64>,
    /// `NinjaInfo[36]`: the Land Rank bonus each upgrade level pays.
    pub farm_rank_per_level: Vec<f64>,
}

/// The mastery scales the game applies to the `NinjaInfo` rows.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Scales {
    pub detection: f64,
    pub door_hp: f64,
    pub jade: f64,
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
pub(crate) fn weapon_class(bonus_type: i32) -> &'static str {
    match bonus_type {
        0 => "Gloves",
        1 => "Nunchaku",
        2 => "Kunai",
        3 => "Katana",
        _ => "Weapon",
    }
}

/// The emporium letter at `index`, which the saved string has to contain.
pub(crate) fn emporium_letter(index: usize) -> Option<char> {
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
