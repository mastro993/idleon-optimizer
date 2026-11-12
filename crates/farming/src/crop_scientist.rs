//! The Crop Scientist bonuses, from `computeCropSC` in
//! `js/stats/systems/w6/farming.js`.
//!
//! The upstream figure is a crop-count formula times an account multiplier:
//! Grimoire 22 (scaled by Grimoire 36), exotic 40 and Vault 79. Its lab
//! mainframe term needs the mainframe's connection walk, which this port does
//! not model, so it stays at zero and every bonus here is a lower bound. Each
//! row is also gated behind one emporium item (`Ninja[102][9]`).

use super::{Farm, data};

/// The ten permanent bonuses, in the upstream table's order.
const NAMES: [&str; 10] = [
    "Total Damage",
    "Gaming Evolution Chance",
    "Jade Coin Gain",
    "Meal Cooking Speed",
    "Coins from Monsters",
    "Breedability and Shiny Speed",
    "Base Critter Quantity",
    "Drop Rate",
    "Spelunking POW and Amber",
    "Research EXP",
];

/// What each bonus applies to, as the upstream table says.
const APPLIES_TO: [&str; 10] = [
    "All characters",
    "Gaming sprouts",
    "Sneaking",
    "All kitchens",
    "Monster cash",
    "Breeding",
    "Trapping, capped at +20",
    "All characters",
    "Spelunking",
    "Research",
];

/// The game caps the critter quantity bonus at +20; the upstream table shows
/// the capped figure while `computeCropSC` itself returns the raw one.
const CRITTER_CAP: f64 = 20.0;

impl Farm {
    /// The Crop Scientist table: every bonus, its value and whether the
    /// emporium item behind it is owned.
    pub fn crop_scientist(&self) -> CropScientist {
        let multiplier = self.crop_scientist_multiplier();
        let crops = f64::from(self.crop_count);
        let rows = NAMES
            .iter()
            .enumerate()
            .map(|(id, name)| {
                let unlocked = data()
                    .crop_scientist_gates
                    .get(id)
                    .is_some_and(|item| data().emporium_owned(&self.emporium_owned, *item));
                let raw = match id {
                    0 => 20.0 * crops,
                    1 => 1.02_f64.powf(crops),
                    2 => 8.0 * crops,
                    3 => 1.1_f64.powf(crops),
                    4 => 15.0 * crops,
                    5 => 7.0 * crops,
                    6 => 0.1 * crops,
                    7 => (crops - 100.0).max(0.0),
                    8 => 5.0 * (crops - 200.0).max(0.0),
                    _ => ((crops - 200.0) / 10.0).floor().max(0.0),
                };
                CropScientistBonus {
                    id,
                    name,
                    unit: UNIT[id],
                    applies_to: APPLIES_TO[id],
                    value: if unlocked { raw * multiplier } else { 0.0 },
                    unlocked,
                }
            })
            .collect();
        CropScientist { multiplier, rows }
    }

    /// `(1 + 0/100) * (1 + (Grimoire 22 + exotic 40 + Vault 79) / 100)`: the
    /// multiplier every Crop Scientist formula is scaled by. The lab mainframe
    /// term is not modelled and contributes nothing.
    pub(crate) fn crop_scientist_multiplier(&self) -> f64 {
        1.0 + (self.grimoire_crop_scientist
            + self.exotic_bonus(40)
            + self.vault_crop_scientist * data().vault_crop_scientist_per_level)
            / 100.0
    }
}

/// `computeCropSC`'s unit per row.
const UNIT: [CropScientistUnit; 10] = [
    CropScientistUnit::Percent,
    CropScientistUnit::Multiplier,
    CropScientistUnit::Percent,
    CropScientistUnit::Multiplier,
    CropScientistUnit::Percent,
    CropScientistUnit::Percent,
    CropScientistUnit::Flat,
    CropScientistUnit::Percent,
    CropScientistUnit::Percent,
    CropScientistUnit::Percent,
];

/// The whole Crop Scientist table.
#[derive(Clone, Debug, PartialEq)]
pub struct CropScientist {
    /// The account multiplier the crop-count formulas are scaled by.
    pub multiplier: f64,
    pub rows: Vec<CropScientistBonus>,
}

impl CropScientist {
    /// How many bonuses the account has unlocked.
    pub fn unlocked(&self) -> usize {
        self.rows.iter().filter(|row| row.unlocked).count()
    }
}

/// One permanent Crop Scientist bonus.
#[derive(Clone, Debug, PartialEq)]
pub struct CropScientistBonus {
    pub id: usize,
    pub name: &'static str,
    pub unit: CropScientistUnit,
    pub applies_to: &'static str,
    /// The crop-count formula times the account multiplier, as
    /// `computeCropSC` returns it.
    pub value: f64,
    pub unlocked: bool,
}

impl CropScientistBonus {
    /// The figure the card shows: the critter quantity row is capped at +20.
    pub fn display_value(&self) -> f64 {
        if self.id == 6 {
            self.value.min(CRITTER_CAP)
        } else {
            self.value
        }
    }
}

/// The unit a bonus is quoted in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CropScientistUnit {
    /// `+%`.
    Percent,
    /// `x`.
    Multiplier,
    /// A flat `+` amount.
    Flat,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn farm(owned: &str, crop_count: u32) -> Farm {
        Farm {
            crop_count,
            emporium_owned: owned.into(),
            ..Farm::default()
        }
    }

    /// The gates express which emporium item unlocks each row.
    #[test]
    fn a_row_is_gated_behind_its_emporium_item() {
        // `A` is emporium item 27 (Total Damage); `x` is item 24.
        let locked = farm("", 120).crop_scientist();
        assert_eq!(locked.unlocked(), 0);
        assert!(locked.rows.iter().all(|row| row.value == 0.0));
        let one = farm("A", 120).crop_scientist();
        assert_eq!(one.rows[0].value, 20.0 * 120.0);
        assert_eq!(one.rows[1].value, 0.0);
        assert!(!one.rows[1].unlocked);
        let two = farm("x", 120).crop_scientist();
        assert_eq!(two.rows[1].value, 1.02_f64.powf(120.0));
        assert_eq!(two.rows[0].value, 0.0);
    }

    #[test]
    fn the_crop_count_formulas_match_the_upstream_table() {
        let full = farm("AxyzBCwLNR", 250).crop_scientist();
        assert_eq!(full.unlocked(), 10);
        assert_eq!(full.rows[0].value, 20.0 * 250.0);
        assert_eq!(full.rows[1].value, 1.02_f64.powf(250.0));
        assert_eq!(full.rows[2].value, 8.0 * 250.0);
        assert_eq!(full.rows[3].value, 1.1_f64.powf(250.0));
        assert_eq!(full.rows[4].value, 15.0 * 250.0);
        assert_eq!(full.rows[5].value, 7.0 * 250.0);
        // 0.1 per crop, capped at +20 by the game.
        assert_eq!(full.rows[6].value, 25.0);
        assert_eq!(full.rows[6].display_value(), 20.0);
        assert_eq!(full.rows[7].value, 150.0);
        assert_eq!(full.rows[8].value, 5.0 * 50.0);
        assert_eq!(full.rows[9].value, 5.0);
        assert_eq!(full.rows[0].unit, CropScientistUnit::Percent);
        assert_eq!(full.rows[1].unit, CropScientistUnit::Multiplier);
        assert_eq!(full.rows[6].unit, CropScientistUnit::Flat);
    }

    #[test]
    fn the_floor_terms_stop_at_zero() {
        let rows = farm("AxyzBCwLNR", 90).crop_scientist();
        assert_eq!(rows.rows[7].value, 0.0);
        assert_eq!(rows.rows[8].value, 0.0);
        assert_eq!(rows.rows[9].value, 0.0);
        assert_eq!(rows.rows[6].value, 9.0);
        assert_eq!(rows.rows[6].display_value(), 9.0);
    }

    #[test]
    fn the_account_multiplier_adds_grimoire_exotic_and_vault() {
        let base = Farm {
            crop_count: 100,
            emporium_owned: "A".into(),
            ..Farm::default()
        };
        // Grimoire 22 = 12, scaled by Grimoire 36 = 30 → 15.6.
        let mut grimoire = base.clone();
        grimoire.grimoire_crop_scientist = 12.0 * 1.3;
        let expected = 1.0 + (12.0 * 1.3 + grimoire.exotic_bonus(40)) / 100.0;
        assert!((grimoire.crop_scientist_multiplier() - expected).abs() < 1e-12);
        assert!(grimoire.crop_scientist().rows[0].value > base.crop_scientist().rows[0].value);
        let mut vault = base.clone();
        vault.vault_crop_scientist = 18.0;
        assert!(
            (vault.crop_scientist_multiplier() - (1.0 + (vault.exotic_bonus(40) + 18.0) / 100.0))
                .abs()
                < 1e-12
        );
        // Exotic 40 is a decaying market upgrade: 20 × lv / (1000 + lv).
        let mut exotic = base.clone();
        exotic.market_levels = vec![0.0; 100];
        exotic.market_levels[60] = 500.0;
        assert!((exotic.exotic_bonus(40) - 20.0 * 500.0 / 1500.0).abs() < 1e-12);
    }
}
