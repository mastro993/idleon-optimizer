//! Static Monument data, extracted from the upstream JS calculators.

use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::OnceLock;

const MONUMENT_JSON: &str = include_str!("../data/monument.json");

/// The tables the Bravery calculator reads.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Data {
    pub holes: Holes,
    pub cosmo: Cosmo,
    pub fountain_per_level: Vec<f64>,
    pub collectible19: f64,
    pub legend_per_point: HashMap<String, f64>,
    pub summon: Summon,
    pub arcade: HashMap<String, Vec<Value>>,
    pub compass: HashMap<String, Vec<Value>>,
    pub minehead_bonus_qty: Vec<f64>,
    pub companions: HashMap<String, [f64; 2]>,
    /// `MERITOC_BASE`, the vote bonuses the reward charge reads.
    pub meritoc_base: Vec<f64>,
    /// `ROG_BONUS_QTY`, the sushi-tier bonuses the Meritoc vote counts.
    pub rog_bonus_qty: Vec<f64>,
    /// `N2L`: item `t` is owned when a saved list carries `encoding[t]`.
    pub encoding: String,
    pub artifact32: f64,
    pub emperor: Emperor,
    pub arcane48_per_level: f64,
    pub godshard_set: Vec<Value>,
    pub jelly_rewards: Vec<f64>,
}

/// The `HolesInfo` rows the monument formulas use.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Holes {
    pub milestone_hours: Vec<f64>,
    pub milestone_text: Vec<String>,
    pub bonus_text: Vec<String>,
    pub bonus_per_level: Vec<f64>,
    pub measurement_type: Vec<f64>,
    pub measurement_base: Vec<String>,
    pub bolaia_per_level: Vec<f64>,
    /// `HolesInfo[62]`: the Justice case rows, raw and pipe-separated.
    pub justice_cases: Vec<String>,
    /// `HolesInfo[63]`: the Justice visitor names.
    pub justice_visitors: Vec<String>,
}

/// `CosmoBonus` bases for the monument and measurement bonuses.
#[derive(Debug, Deserialize)]
pub struct Cosmo {
    pub monument: f64,
    pub measurement: f64,
}

/// Summoning victory tables.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Summon {
    pub normal: HashMap<String, [f64; 2]>,
    pub endless_type: Vec<f64>,
    pub endless_val: Vec<f64>,
}

/// Emperor bonus values and the slot type each kill rolls.
#[derive(Debug, Deserialize)]
pub struct Emperor {
    pub val: Vec<f64>,
    pub types: Vec<f64>,
}

impl Data {
    /// `HolesInfo[37]` at `slot`.
    pub fn bonus_per_level(&self, slot: usize) -> f64 {
        self.holes.bonus_per_level.get(slot).copied().unwrap_or(0.0)
    }

    /// `HolesInfo[32]` at `slot`.
    pub fn bonus_text(&self, slot: usize) -> &str {
        self.holes
            .bonus_text
            .get(slot)
            .map(String::as_str)
            .unwrap_or("")
    }

    /// `HolesInfo[70]` at `index`.
    pub fn bolaia_per_level(&self, index: usize) -> f64 {
        self.holes
            .bolaia_per_level
            .get(index)
            .copied()
            .unwrap_or(0.0)
    }

    /// The raw `HolesInfo[62]` Justice case rows.
    pub fn justice_cases(&self) -> &[String] {
        &self.holes.justice_cases
    }

    /// `HolesInfo[63][visitor]`, the `_`-written visitor name.
    pub fn justice_visitor(&self, visitor: usize) -> &str {
        self.holes
            .justice_visitors
            .get(visitor)
            .map(String::as_str)
            .unwrap_or("")
    }

    /// `HolesInfo[52]` at `index`.
    pub fn measurement_type(&self, index: usize) -> f64 {
        self.holes
            .measurement_type
            .get(index)
            .copied()
            .unwrap_or(0.0)
    }

    /// `HolesInfo[55]` at `index`.
    pub fn measurement_base(&self, index: usize) -> &str {
        self.holes
            .measurement_base
            .get(index)
            .map(String::as_str)
            .unwrap_or("0")
    }

    /// `LegendTalents[index][2]`.
    pub fn legend_per_point(&self, index: usize) -> f64 {
        self.legend_per_point
            .get(&index.to_string())
            .copied()
            .unwrap_or(0.0)
    }

    /// `ArcadeShopInfo[index]`.
    pub fn arcade(&self, index: usize) -> Option<&[Value]> {
        self.arcade.get(&index.to_string()).map(Vec::as_slice)
    }

    /// `CompassUpg[index]`.
    pub fn compass(&self, index: usize) -> Option<&[Value]> {
        self.compass.get(&index.to_string()).map(Vec::as_slice)
    }

    /// `MINEHEAD_BONUS_QTY[index]`.
    pub fn minehead_bonus(&self, index: usize) -> f64 {
        self.minehead_bonus_qty.get(index).copied().unwrap_or(0.0)
    }

    /// A companion's base and enhanced bonuses.
    pub fn companion(&self, index: usize) -> Option<[f64; 2]> {
        self.companions.get(&index.to_string()).copied()
    }

    /// `MERITOC_BASE[index]`.
    pub fn meritoc_base(&self, index: usize) -> f64 {
        self.meritoc_base.get(index).copied().unwrap_or(0.0)
    }

    /// `ROG_BONUS_QTY[index]`.
    pub fn rog_bonus_qty(&self, index: usize) -> f64 {
        self.rog_bonus_qty.get(index).copied().unwrap_or(0.0)
    }

    /// The encoding letter that marks item `index` as owned.
    pub fn encoding_letter(&self, index: usize) -> Option<char> {
        self.encoding.chars().nth(index)
    }

    /// `EmperorBon[1][index]`.
    pub fn emperor_value(&self, index: usize) -> f64 {
        self.emperor.val.get(index).copied().unwrap_or(0.0)
    }

    /// `EmperorBon[2][slot]`.
    pub fn emperor_type(&self, slot: usize) -> f64 {
        self.emperor.types.get(slot).copied().unwrap_or(0.0)
    }

    /// A `GODSHARD_SET` row: armor pieces, tool pieces, special pieces and caps.
    pub fn godshard_set(&self) -> Option<GodshardSet<'_>> {
        let armor = self.godshard_set.first()?.as_array()?;
        let tools = self.godshard_set.get(1)?.as_array()?;
        let special = self.godshard_set.get(2)?.as_array()?;
        let caps = self.godshard_set.get(3)?.as_array()?;
        Some(GodshardSet {
            armor,
            tools,
            special,
            tools_cap: caps.first().map(super::save::number).unwrap_or(0.0) as usize,
            special_cap: caps.get(1).map(super::save::number).unwrap_or(0.0) as usize,
            bonus: caps.get(2).map(super::save::number).unwrap_or(0.0),
        })
    }

    /// `Research[47][index]`, the jelly reward value.
    pub fn jelly_reward(&self, index: usize) -> f64 {
        self.jelly_rewards.get(index).copied().unwrap_or(0.0)
    }
}

/// The summoning set that raises winner bonuses.
pub struct GodshardSet<'a> {
    pub armor: &'a [Value],
    pub tools: &'a [Value],
    pub special: &'a [Value],
    pub tools_cap: usize,
    pub special_cap: usize,
    pub bonus: f64,
}

/// The Monument data, parsed once.
pub fn data() -> &'static Data {
    static DATA: OnceLock<Data> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(MONUMENT_JSON).expect("bundled monument data is valid JSON")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_covers_the_bravery_formulas() {
        let data = data();
        assert_eq!(data.holes.milestone_hours.len(), 8);
        assert_eq!(data.holes.milestone_text.len(), 24);
        assert_eq!(data.justice_cases().len(), 90);
        assert_eq!(data.justice_visitor(4), "The_Harbinger");
        assert_eq!(data.holes.bonus_text.len(), 30);
        assert_eq!(data.holes.bonus_per_level.len(), 30);
        assert_eq!(data.fountain_per_level.len(), 3);
        assert_eq!(data.companion(27), Some([1.0, 1.5]));
        assert_eq!(data.companion(39), Some([50.0, 75.0]));
        assert_eq!(data.companion(135), Some([100.0, 200.0]));
        assert_eq!(data.companion(161), Some([100.0, 150.0]));
        assert_eq!(data.meritoc_base(7), 200.0);
        assert_eq!(data.rog_bonus_qty(51), 10.0);
        assert_eq!(data.encoding_letter(23), Some('w'));
        assert_eq!(data.encoding_letter(31), Some('E'));
        assert_eq!(data.emperor_value(8), 1.0);
        assert_eq!(data.legend_per_point(29), 20.0);
        assert_eq!(data.godshard_set().map(|set| set.bonus), Some(15.0));
        assert!(data.summon.normal.contains_key("mushG"));
        assert!(data.arcade(42).is_some());
        assert!(data.compass(55).is_some());
    }
}
