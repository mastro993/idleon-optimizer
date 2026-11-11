//! Static Jar Cavern data, extracted from the upstream JS calculators.
//!
//! `script/extract-jar-data.mjs` writes `data/jar.json` from the upstream
//! checkout; the rows are the ones the jar formulas read, not a copy of the
//! game's whole data set.

use serde::Deserialize;
use std::collections::HashMap;
use std::sync::OnceLock;

const JAR_JSON: &str = include_str!("../../data/jar.json");

/// The tables the jar formulas read.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Data {
    pub cosmo: Cosmo,
    /// `LegendTalents[index][2]`: the points each legend talent level grants.
    pub legend_per_point: HashMap<String, f64>,
    /// `paletteParams(2)`, the palette that grants collectible chance.
    pub palette2: Palette,
    /// `UPG_DATA[2][upgrade][6]`, the Fountain bonus per level.
    pub fountain_per_level: Fountain,
    /// `HolesInfo[37]`: the per-level value of each monument bonus.
    pub monument_bonus_per_level: Vec<f64>,
    /// `HolesInfo[52]`: the input a measurement scales from.
    pub measurement_type: Vec<f64>,
    /// `HolesInfo[55]`: a measurement's base text; `TOT` rows decay with level.
    pub measurement_base: Vec<String>,
    /// `N2L[32]`, the SuperBit 32 key matched against `Gaming[12]`.
    pub super_bit32: String,
}

/// `CosmoBonus` bases for the upgrades the jar reads.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Cosmo {
    /// `cosmoUpgBase(0, 0)`, the monument chain.
    pub monument: f64,
    /// `cosmoUpgBase(0, 3)`, the extra rupie chance.
    pub extra_rupie: f64,
    /// `cosmoUpgBase(1, 3)`, the measurement scale.
    pub measurement: f64,
}

/// `paletteParams(idx)`: `denom` is fixed at 25 for every palette.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Palette {
    pub coeff: f64,
    pub is_decay: bool,
}

/// The two Fountain upgrades the jar bonuses lean on.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Fountain {
    /// `UPG_DATA[2][13][6]`, the monument boost.
    pub monument: f64,
    /// `UPG_DATA[2][15][6]`, the cavern-resource boost.
    pub total: f64,
}

impl Data {
    /// `holesMeasBase(idx)`: the base text of a measurement.
    pub fn measurement_base(&self, index: usize) -> &str {
        self.measurement_base
            .get(index)
            .map(String::as_str)
            .unwrap_or("0")
    }

    /// `holesMeasType(idx)`: the input type a measurement scales from.
    pub fn measurement_type(&self, index: usize) -> u32 {
        self.measurement_type
            .get(index)
            .map(|kind| *kind as u32)
            .unwrap_or(0)
    }

    /// `legendTalentPerPt(idx)`: points each level of a legend talent grants.
    pub fn legend_per_point(&self, index: usize) -> f64 {
        self.legend_per_point
            .get(&index.to_string())
            .copied()
            .unwrap_or(0.0)
    }

    /// `HolesInfo[37][slot]`: a monument bonus's value per level.
    pub fn monument_bonus_per_level(&self, slot: usize) -> f64 {
        self.monument_bonus_per_level
            .get(slot)
            .copied()
            .unwrap_or(0.0)
    }
}

/// The shared [`Data`].
pub fn data() -> &'static Data {
    static DATA: OnceLock<Data> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(JAR_JSON).expect("crates/core/data/jar.json is a valid data file")
    })
}
