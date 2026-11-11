//! The pure fishing formulas, ported 1:1 from
//! `js/stats/systems/w2/fishing.js`.
//!
//! Every function keeps the upstream evaluation order and its floors, because
//! the AFK claim floors each fish count and the split rounds each share down to
//! hundredths: reordering the multiplications changes the result.

use serde::Deserialize;

/// The four toolkit depths, with `D0` already added to the spot bonus.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Depths {
    pub d0: f64,
    pub d1: f64,
    pub d2: f64,
    pub d3: f64,
}

impl Depths {
    /// The depth at `0..=3`.
    pub fn at(self, tier: usize) -> f64 {
        match tier {
            0 => self.d0,
            1 => self.d1,
            2 => self.d2,
            _ => self.d3,
        }
    }
}

/// `FishingToolkit("D0".."D3")`: toolkit + equipment depth + spot bonus.
pub fn depths(
    toolkit: &crate::fishing::data::ToolkitStats,
    equipment_depth: [f64; 3],
    spot_bonus: [f64; 4],
) -> Depths {
    let d0 = toolkit.d0 + spot_bonus[0];
    let depth = |tier: usize| {
        (toolkit.depth(tier) + equipment_depth[tier - 1] + spot_bonus[tier] - d0 / 3.0).max(0.0)
    };
    Depths {
        d0,
        d1: depth(1),
        d2: depth(2),
        d3: depth(3),
    }
}

/// `FishingToolkit("Pct1".."Pct4")`: the catch split, floored to 0.01%.
pub fn catch_shares(efficiency: f64, depths: &Depths, eff_reqs: [f64; 3]) -> [f64; 4] {
    let e = efficiency;
    let (d1, d2, d3) = (depths.d1, depths.d2, depths.d3);
    let pct4 = if e >= eff_reqs[2] {
        (3.0 * e / (e + 800.0) * (4.0 * d3 / (d3 + 100.0) + 1.0) * 100.0).floor() / 100.0
    } else {
        0.0
    };
    let pct3 = if e >= eff_reqs[1] {
        (100.0 * (2.0 * e / (e + 200.0) * 6.0 * (2.0 * d2 / (d2 + 100.0) + 1.0) - pct4 / 5.0))
            .floor()
            / 100.0
    } else {
        0.0
    };
    let pct2 = if e >= eff_reqs[0] {
        (100.0
            * (1.5 * e / (e + 10.0) * 20.0 * (2.0 * d1 / (d1 + 100.0) + 1.0)
                - (pct3 / 3.0 + pct4 / 5.0)))
            .floor()
            / 100.0
    } else {
        0.0
    };
    let pct1 = (100.0 * (100.0 - (pct2 + (pct3 + pct4)))).floor() / 100.0;
    [pct1, pct2, pct3, pct4]
}

/// `SkillStats("FishingOreValue")`: fish per catch before Multi-Fish.
pub fn ore_value(efficiency: f64, eff_req: f64, prowess: f64) -> f64 {
    let ratio = efficiency / eff_req.max(1e-9);
    ratio.powf(0.25 + prowess).max(1.0).floor()
}

/// The inputs the upstream `fishingMultiFish` reads.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct MultiFishInputs {
    /// Fishing skill power.
    pub power: f64,
    /// Fishing level.
    pub level: f64,
    /// Stamps: Multi-Fish, as a percentage.
    pub stamp: f64,
    /// The Fishing Active bubble's Multi-Fish, as a percentage.
    pub bubble: f64,
    /// Star signs: Multi-Fish, as a percentage.
    pub star_sign: f64,
    /// Cosmo Upgrade 2-2's cap bonus, as a percentage.
    pub cosmo: f64,
}

/// `SkillStats("FishingMultiOre")`: the Multi-Fish percentage and its cap.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct MultiFish {
    pub value: f64,
    pub raw: f64,
    pub cap: f64,
}

/// `SkillStats("FishingMultiOre")`, floored to the upstream cap.
pub fn multi_fish(inputs: &MultiFishInputs) -> MultiFish {
    let power = inputs.power.max(0.0);
    let level = inputs.level;
    let bubble = inputs.bubble;
    let raw = inputs.stamp
        + 100.0 * (power.sqrt() / (4.0 * power.sqrt() + 50.0) + 0.15 * level / (level + 40.0))
        + (bubble + inputs.star_sign);
    let cap = (if bubble < 20.0 { 100.0 } else { 300.0 }) + inputs.cosmo;
    MultiFish {
        value: cap.min(raw),
        raw,
        cap,
    }
}

/// The inputs the upstream `fishingSpeed` reads.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct SpeedInputs {
    pub efficiency: f64,
    pub level: f64,
    pub rod_speed: f64,
    /// The additive speed bonus, as a percentage.
    pub speed_bonus_pct: f64,
}

/// `SkillStats("FishingSpeed")` outside dungeons: the catch time in seconds.
pub fn speed(inputs: &SpeedInputs) -> f64 {
    let e = inputs.efficiency;
    let level = inputs.level;
    let rod = inputs.rod_speed;
    let base = 5.7 - ((e / (e + 300.0) * 1.7).min(0.8) + (e / (e + 1e3) * 2.0).min(1.7));
    let level_term = 0.9 * level.sqrt() / (level.sqrt() + 250.0) + 0.6 * level / (level + 40.0);
    let time = if rod < 3.0 {
        (base + (4.0 - rod).powf(2.2) - level_term).max(0.57)
    } else {
        (base - (0.2 * rod.powf(1.3) + level_term)).max(0.57)
    };
    time * (15.0 / (1.0 + inputs.speed_bonus_pct / 100.0))
}

/// `HourlyKillRate("Fishing")`: active catches per hour.
pub fn catches_per_hour(speed: f64, attack_factor: f64) -> f64 {
    let factor = if attack_factor.is_finite() {
        attack_factor.clamp(1.0, 3.0)
    } else {
        1.0
    };
    (3600.0 / (speed + 0.4) * factor).round()
}
