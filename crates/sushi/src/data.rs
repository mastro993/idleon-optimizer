//! Static Sushi Station data, extracted from the upstream calculator.

use serde::Deserialize;
use std::sync::OnceLock;

const SUSHI_JSON: &str = include_str!("../data/sushi.json");

/// Everything the Sushi Station tab reads from the game.
#[derive(Debug, Deserialize)]
pub struct GameData {
    pub upgrades: Vec<Upgrade>,
    /// `Research[32]`: the upgrade index each slot sells.
    #[serde(rename = "slotToUpg")]
    pub slot_to_upg: Vec<usize>,
    /// `Research[33]`: the knowledge category each sushi tier feeds.
    #[serde(rename = "tierToKnowledgeCat")]
    pub tier_to_knowledge_cat: Vec<usize>,
    /// `Research[34]`: the knowledge category descriptions.
    #[serde(rename = "knowledgeCatDesc")]
    pub knowledge_cat_desc: Vec<String>,
    /// `Research[35]`: the knowledge bonus per level, per category.
    #[serde(rename = "knowledgeCatValue")]
    pub knowledge_cat_value: Vec<f64>,
    /// `Research[36]`: the Ring of Glory reward descriptions.
    #[serde(rename = "rogDesc")]
    pub rog_desc: Vec<String>,
    /// `Research[37]`: the Ring of Glory bonus quantities.
    #[serde(rename = "rogQty")]
    pub rog_qty: Vec<f64>,
    /// The bucks each sushi tier earns before the currency multiplier.
    #[serde(rename = "currencyPerTier")]
    pub currency_per_tier: Vec<f64>,
    /// The station's slot count (`MAX_SLOTS`).
    #[serde(rename = "maxSlots")]
    pub max_slots: usize,
}

/// One of the forty-six Sushi Station upgrades.
#[derive(Clone, Debug, Deserialize)]
pub struct Upgrade {
    pub name: String,
    #[serde(rename = "maxLv")]
    pub max_lv: f64,
    #[serde(rename = "costBase")]
    pub cost_base: f64,
    #[serde(rename = "bonusPerLv")]
    pub bonus_per_lv: f64,
    #[serde(rename = "costDN")]
    pub cost_dn: f64,
    pub desc: String,
}

impl GameData {
    /// The upgrade at `index`, if the table has one.
    pub fn upgrade(&self, index: usize) -> Option<&Upgrade> {
        self.upgrades.get(index)
    }

    /// The upgrade index a slot sells.
    pub fn slot_upg(&self, slot: usize) -> Option<usize> {
        self.slot_to_upg.get(slot).copied()
    }

    /// The highest sushi tier the knowledge table covers (`MAX_TIER`).
    pub fn max_tier(&self) -> usize {
        self.tier_to_knowledge_cat.len().saturating_sub(1)
    }

    /// The knowledge category a sushi tier feeds.
    pub fn knowledge_cat(&self, tier: usize) -> Option<usize> {
        self.tier_to_knowledge_cat.get(tier).copied()
    }

    /// The knowledge bonus one level in a category pays.
    pub fn knowledge_value(&self, cat: usize) -> f64 {
        self.knowledge_cat_value.get(cat).copied().unwrap_or(0.0)
    }

    /// The Ring of Glory bonus quantity at `index`, zero past the table.
    pub fn rog_qty_at(&self, index: usize) -> f64 {
        self.rog_qty.get(index).copied().unwrap_or(0.0)
    }

    /// The Ring of Glory reward text at `index`.
    pub fn rog_desc_at(&self, index: usize) -> Option<&str> {
        self.rog_desc.get(index).map(String::as_str)
    }

    /// The knowledge category description at `index`.
    pub fn knowledge_desc_at(&self, index: usize) -> Option<&str> {
        self.knowledge_cat_desc.get(index).map(String::as_str)
    }
}

/// The shared data table.
pub fn game() -> &'static GameData {
    static DATA: OnceLock<GameData> = OnceLock::new();
    DATA.get_or_init(|| serde_json::from_str(SUSHI_JSON).expect("crates/sushi/data/sushi.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tables_load() {
        let data = game();
        assert_eq!(data.upgrades.len(), 46, "Sushi upgrades");
        assert_eq!(
            data.slot_to_upg.len(),
            data.upgrades.len(),
            "one upgrade per slot"
        );
        assert_eq!(data.knowledge_cat_desc.len(), 11, "knowledge categories");
        assert_eq!(data.rog_qty.len(), data.rog_desc.len(), "RoG rows");
        assert!(data.max_tier() > 50, "sushi tiers");
        assert_eq!(data.max_slots, 120);
    }
}
