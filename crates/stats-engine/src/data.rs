//! The static tables the engine reads, from the game's data.
//!
//! Every row keeps the shape its lookup uses: bubbles and vials by their effect
//! key, sigil tiers by index, card bonuses by card key, and the post-office,
//! guild, achievement and statue tables by their game index.

use serde::Deserialize;
use std::collections::BTreeMap;
use std::sync::OnceLock;

const ENGINE_JSON: &str = include_str!("../data/engine.json");

/// The tables, as `script/extract-engine-data.mjs` writes them.
#[derive(Debug, Deserialize)]
pub struct Data {
    pub items: BTreeMap<String, Item>,
    #[serde(rename = "starSigns")]
    pub star_signs: Vec<Vec<String>>,
    #[serde(rename = "starSignBonuses")]
    pub star_sign_bonuses: BTreeMap<String, BTreeMap<String, f64>>,
    pub bubbles: BTreeMap<String, Bubble>,
    #[serde(rename = "bigBubbles")]
    pub big_bubbles: Vec<BigBubble>,
    pub vials: BTreeMap<String, Vec<Vial>>,
    pub sigils: Vec<Vec<f64>>,
    pub cards: Cards,
    #[serde(rename = "boxRewards")]
    pub box_rewards: Vec<Vec<serde_json::Value>>,
    pub guilds: Vec<Curve>,
    pub achievements: BTreeMap<String, f64>,
    pub atoms: Vec<Atom>,
    pub artifacts: Vec<f64>,
    /// Per artifact, the tiers 2-7 whose description doubles..seventuples it.
    #[serde(rename = "artifactTierBonus", default)]
    pub artifact_tier_bonus: Vec<Vec<bool>>,
    #[serde(rename = "owlBase")]
    pub owl_base: Vec<f64>,
    #[serde(rename = "companionBonus")]
    pub companion_bonus: BTreeMap<String, f64>,
    #[serde(rename = "goldFood")]
    pub gold_food: GoldFood,
    pub meals: Vec<Meal>,
    pub cosmo: Vec<Vec<f64>>,
    #[serde(rename = "saltLicks")]
    pub salt_licks: Vec<f64>,
    pub statues: Vec<f64>,
    pub stamps: Vec<Stamp>,
    #[serde(rename = "farmRankPerLevel")]
    pub farm_rank_per_level: Vec<f64>,
    #[serde(rename = "dungPassive")]
    pub dung_passive: Vec<Option<Curve>>,
    #[serde(rename = "classParent")]
    pub class_parent: BTreeMap<String, usize>,
    /// `Spelunky[33]`: the Bubba RoG bonus per index.
    #[serde(rename = "bubbaBase", default)]
    pub bubba_base: Vec<f64>,
    /// `ShrineInfo[t][2]`.
    #[serde(rename = "shrineBase", default)]
    pub shrine_base: Vec<f64>,
    /// `ShrineInfo[t][3]`.
    #[serde(rename = "shrinePerLevel", default)]
    pub shrine_per_level: Vec<f64>,
    /// `SummonUPG[t]`, reduced to the three columns the damage chain reads.
    #[serde(rename = "summonUpg", default)]
    pub summon_upg: Vec<SummonUpgrade>,
    /// `GodsInfo[t][14]`: the Divinity blessing's bonus per level.
    #[serde(rename = "godBlessBase", default)]
    pub god_bless_base: Vec<f64>,
    /// `ChipDesc[t][10]` and `[11]`: a lab chip's effect key and value.
    #[serde(default)]
    pub chips: Vec<Chip>,
    /// `BOSS3B_CARD_PCT`.
    #[serde(rename = "boss3bCardPct", default)]
    pub boss3b_card_pct: f64,
    /// `TOME_DATA`, reduced to the fields `tome.resolve` reads.
    #[serde(rename = "tomeBonus", default)]
    pub tome_bonus: BTreeMap<String, TomeBonus>,
    /// `RandoListo2[13]`: each Cove upgrade's per-level value.
    #[serde(rename = "cglunkoPerLevel", default)]
    pub cglunko_per_level: Vec<f64>,
    /// The Royal Guardian outpost rows the statue bonus reads.
    #[serde(rename = "royalOutpost", default)]
    pub royal_outpost: RoyalOutpost,
}

/// One item's fields the weapon-power chain reads.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Item {
    pub wp: f64,
    #[serde(rename = "STR")]
    pub str: f64,
    #[serde(rename = "AGI")]
    pub agi: f64,
    #[serde(rename = "WIS")]
    pub wis: f64,
    #[serde(rename = "LUK")]
    pub luk: f64,
    #[serde(rename = "Defence")]
    pub defence: f64,
    #[serde(rename = "type")]
    pub kind: Option<String>,
    pub effect: Option<String>,
    pub amount: f64,
    pub desc: Option<String>,
}

/// One alchemy bubble by its effect key.
#[derive(Clone, Debug, Deserialize)]
pub struct Bubble {
    pub name: String,
    pub cauldron: usize,
    pub index: usize,
    pub x1: f64,
    pub x2: f64,
    pub formula: String,
    /// `BUBBLE_KEYS[key].slab`: the bubble scales by discovered items / 100.
    #[serde(default)]
    pub slab: bool,
    /// `BUBBLE_KEYS[key].tome`: the bubble scales by Tome points.
    #[serde(default)]
    pub tome: bool,
}

/// The bubble a cauldron's slab and tome bubbles multiply by.
#[derive(Clone, Debug, Deserialize)]
pub struct BigBubble {
    pub indices: Vec<usize>,
    pub key: String,
}

/// One alchemy vial by its effect key.
#[derive(Clone, Debug, Deserialize)]
pub struct Vial {
    pub name: String,
    pub index: usize,
    pub x1: f64,
    pub x2: f64,
    pub formula: String,
}

/// The card tables: each card's bonus description and value, and the type ids
/// the descriptors look a bonus up by.
#[derive(Debug, Deserialize)]
pub struct Cards {
    pub bonus: BTreeMap<String, CardBonus>,
    #[serde(rename = "idForBonus")]
    pub id_for_bonus: BTreeMap<String, String>,
    #[serde(rename = "idForSet")]
    pub id_for_set: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CardBonus {
    pub desc: String,
    pub val: f64,
}

/// One guild bonus's curve.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Curve {
    #[serde(default)]
    pub name: String,
    pub x1: f64,
    pub x2: f64,
    pub formula: String,
}

/// One `SummonUPG[t]` row.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct SummonUpgrade {
    #[serde(rename = "stoneType")]
    pub stone_type: f64,
    #[serde(rename = "perLevel")]
    pub per_level: f64,
    #[serde(rename = "stoneEligible")]
    pub stone_eligible: f64,
}

/// One lab chip's effect key and value.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Chip {
    pub key: String,
    pub value: f64,
}

/// One `TOME_DATA` row.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct TomeBonus {
    pub base: f64,
    pub threshold: f64,
    pub divisor: f64,
    #[serde(rename = "exp")]
    pub exponent: f64,
    #[serde(rename = "unlockType")]
    pub unlock_type: String,
    #[serde(rename = "unlockIdx")]
    pub unlock_index: usize,
}

/// The Royal Guardian tables the statue bonus reads.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct RoyalOutpost {
    pub requirements: Vec<f64>,
    pub costs: Vec<f64>,
    #[serde(rename = "armoryPerLevel")]
    pub armory_per_level: Vec<f64>,
}

/// One atom upgrade's per-level bonus (`AtomInfo[index][4]`).
#[derive(Clone, Debug, Deserialize)]
pub struct Atom {
    pub index: usize,
    #[serde(rename = "perLevel")]
    pub per_level: f64,
}

#[derive(Debug, Deserialize)]
pub struct GoldFood {
    pub info: BTreeMap<String, GoldFoodInfo>,
    pub slots: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct GoldFoodInfo {
    pub effect: String,
    pub amount: f64,
}

/// One stamp's bonus type and curve, from its item description.
#[derive(Clone, Debug, Deserialize)]
pub struct Stamp {
    pub item: String,
    pub kind: String,
    pub cat: usize,
    pub idx: usize,
    pub formula: String,
    pub x1: f64,
    pub x2: f64,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Meal {
    pub index: usize,
    pub name: String,
    #[serde(rename = "perLevel")]
    pub per_level: f64,
    pub effect: String,
}

impl Data {
    /// A bubble by its effect key.
    pub fn bubble(&self, key: &str) -> Option<&Bubble> {
        self.bubbles.get(key)
    }

    /// The card bonus a type id names.
    pub fn card_desc(&self, type_id: usize) -> Option<&str> {
        self.cards
            .id_for_bonus
            .get(&type_id.to_string())
            .map(String::as_str)
    }

    /// The guild bonus curve at `index`.
    pub fn guild(&self, index: usize) -> Option<&Curve> {
        self.guilds.get(index)
    }

    /// The tiered value for a completed achievement.
    pub fn achievement(&self, index: usize) -> f64 {
        self.achievements
            .get(&index.to_string())
            .copied()
            .unwrap_or(0.0)
    }

    /// `AtomInfo[index][4]`.
    pub fn atom(&self, index: usize) -> f64 {
        self.atoms
            .iter()
            .find(|atom| atom.index == index)
            .map_or(0.0, |atom| atom.per_level)
    }

    /// Whether one artifact's tier multiplies its main bonus.
    pub fn artifact_tier_bonus(&self, index: usize, tier: usize) -> bool {
        (2..=7).contains(&tier)
            && self
                .artifact_tier_bonus
                .get(index)
                .and_then(|tiers| tiers.get(tier - 2))
                .copied()
                .unwrap_or(false)
    }

    /// The Sailing artifact's base bonus at `index`.
    pub fn artifact(&self, index: usize) -> f64 {
        self.artifacts.get(index).copied().unwrap_or(0.0)
    }

    /// `OWL_BASE[index]`.
    pub fn owl(&self, index: usize) -> f64 {
        self.owl_base.get(index).copied().unwrap_or(1.0)
    }

    /// `COMPANION_BONUS[index]`.
    pub fn companion_base(&self, index: usize) -> f64 {
        self.companion_bonus
            .get(&index.to_string())
            .copied()
            .unwrap_or(0.0)
    }

    /// The Cosmos upgrade's base bonus at `tier` and `index`.
    pub fn cosmo(&self, tier: usize, index: usize) -> f64 {
        self.cosmo
            .get(tier)
            .and_then(|row| row.get(index))
            .copied()
            .unwrap_or(0.0)
    }

    /// `StatueInfo[index][3]`: the bonus per statue level.
    pub fn statue(&self, index: usize) -> f64 {
        self.statues.get(index).copied().unwrap_or(1.0)
    }

    /// The stamps with one bonus type.
    pub fn stamps(&self, kind: &str) -> impl Iterator<Item = &Stamp> {
        self.stamps.iter().filter(move |stamp| stamp.kind == kind)
    }

    /// The sigil's tier values at `index`.
    pub fn sigil(&self, index: usize) -> Option<&[f64]> {
        self.sigils.get(index).map(Vec::as_slice)
    }

    /// A vial's rows by effect key.
    pub fn vials(&self, key: &str) -> &[Vial] {
        self.vials.get(key).map(Vec::as_slice).unwrap_or(&[])
    }

    /// `MealINFO` rows with one effect.
    pub fn meals(&self, effect: &str) -> impl Iterator<Item = &Meal> {
        self.meals.iter().filter(move |meal| meal.effect == effect)
    }

    /// The gold food row for an item.
    pub fn gold_food(&self, item: &str) -> Option<&GoldFoodInfo> {
        self.gold_food.info.get(item)
    }

    /// The Land Rank bonus per level.
    pub fn farm_rank_per_level(&self, rank: usize) -> f64 {
        self.farm_rank_per_level.get(rank).copied().unwrap_or(0.0)
    }

    /// `DungPassiveStats2[index]`: a Flurbo shop passive's curve.
    pub fn dung_passive(&self, index: usize) -> Option<&Curve> {
        self.dung_passive.get(index).and_then(Option::as_ref)
    }

    /// `Spelunky[33][idx]`: the Bubba RoG bonus.
    pub fn bubba_base(&self, index: usize) -> f64 {
        self.bubba_base.get(index).copied().unwrap_or(0.0)
    }

    /// The shrine's base bonus at `index`.
    pub fn shrine_base(&self, index: usize) -> f64 {
        self.shrine_base.get(index).copied().unwrap_or(0.0)
    }

    /// The shrine's bonus per level at `index`.
    pub fn shrine_per_level(&self, index: usize) -> f64 {
        self.shrine_per_level.get(index).copied().unwrap_or(0.0)
    }

    /// `SummonUPG[t]`: the per-level bonus, stone gate and stone type.
    pub fn summon_upgrade(&self, index: usize) -> Option<&SummonUpgrade> {
        self.summon_upg.get(index)
    }

    /// `GodsInfo[t][14]`: the Divinity blessing's bonus per level.
    pub fn god_bless_base(&self, index: usize) -> f64 {
        self.god_bless_base.get(index).copied().unwrap_or(0.0)
    }

    /// A lab chip effect: the summed value of every equipped chip
    /// with this effect key.
    pub fn chip_value(&self, key: &str) -> f64 {
        self.chips
            .iter()
            .find(|chip| chip.key == key)
            .map_or(0.0, |chip| chip.value)
    }

    /// The per-level value of one Cove upgrade (`RandoListo2[13]`).
    pub fn cglunko_per_level(&self, index: usize) -> f64 {
        self.cglunko_per_level.get(index).copied().unwrap_or(0.0)
    }

    /// `TOME_DATA[id]`.
    pub fn tome_bonus(&self, index: usize) -> Option<&TomeBonus> {
        self.tome_bonus.get(&index.to_string())
    }

    /// `OUTPOST_RANK_REQUIREMENTS[idx]`.
    pub fn outpost_requirement(&self, index: usize) -> f64 {
        self.royal_outpost
            .requirements
            .get(index)
            .copied()
            .unwrap_or(0.0)
    }

    /// `OUTPOST_RANK_COSTS[idx]`.
    pub fn outpost_cost(&self, index: usize) -> f64 {
        self.royal_outpost.costs.get(index).copied().unwrap_or(0.0)
    }

    /// One armory upgrade's bonus per level.
    pub fn armory_per_level(&self, index: usize) -> f64 {
        self.royal_outpost
            .armory_per_level
            .get(index)
            .copied()
            .unwrap_or(0.0)
    }

    /// `ReturnClasses`' parent table.
    pub fn class_parent(&self, class: usize) -> Option<usize> {
        self.class_parent.get(&class.to_string()).copied()
    }

    /// An item's raw stat field, as `ITEMS[name][stat]`.
    pub fn item_stat(&self, item: &str, stat: &str) -> f64 {
        let Some(row) = self.items.get(item) else {
            return 0.0;
        };
        match stat {
            "STR" => row.str,
            "AGI" => row.agi,
            "WIS" => row.wis,
            "LUK" => row.luk,
            "Defence" => row.defence,
            "Weapon_Power" => row.wp,
            _ => 0.0,
        }
    }

    /// A star sign's three effect strings.
    pub fn star_sign(&self, index: usize) -> Option<&[String]> {
        self.star_signs.get(index).map(Vec::as_slice)
    }

    /// `SIGN_BONUSES[key]`: the hardcoded per-sign effect values.
    pub fn star_sign_bonus(&self, key: &str, sign: usize) -> Option<f64> {
        self.star_sign_bonuses
            .get(key)
            .and_then(|row| row.get(&sign.to_string()))
            .copied()
    }
}

/// The engine's tables, loaded once.
pub fn data() -> &'static Data {
    static DATA: OnceLock<Data> = OnceLock::new();
    DATA.get_or_init(|| serde_json::from_str(ENGINE_JSON).expect("engine.json is valid"))
}
