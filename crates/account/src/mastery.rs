//! The Cooking Mastery inputs the Mastery EXP tab reads.
//!
//! Ports the save access in `cooking-mastery-calc.html` (`getCM`,
//! `getTotalCookingLv`, `getMeal73Level`, `getTotalRibbonLv`) and its
//! `computeExternalMulti`. The mastery upgrades themselves are
//! `CookMaster[2]`, the rank row is `CookMaster[1]`, the Divorce Cake level is
//! `Meals[0][73]` and the ribbon ranks start at `Ribbon[28]`.
//!
//! Every external term is the upstream system's own function: the grid bonus of
//! node 190, superbit 68, companion 87, menagerie boost 21, `Fountain_BonTOT(2,
//! 17)`, the Canteen Read vial, arcade 69 and salt lick 10. A save that lacks a
//! field leaves that term at zero; none of them is guessed.

use super::Decoder;
use idleon_common::curve as formula;
use idleon_cooking_mastery::{External, Mastery};
use idleon_save::fields;

/// The first ribbon slot a mastery rank reads; the earlier slots are cards.
const FIRST_RIBBON: usize = 28;

/// The salt lick that boosts Cooking Mastery EXP (`SaltLicks[10]`, Refinery6).
const SALT_LICK: usize = 10;

/// The fountain upgrade that boosts it (`UPG_DATA[2][17]`, Cook Maxxing), in
/// yellow water.
const FOUNTAIN_UPGRADE: usize = 17;
const FOUNTAIN_WATER: usize = 2;

/// The Divorce Cake meal's index in `Meals[0]`.
const DIVORCE_CAKE: usize = 73;

/// The Rift depth that turns the vial threshold on (`riftData[0] > 34`).
const RIFT_VIALS: f64 = 34.0;

/// The vial level the Rift bonus counts (`(Number(vials[rvi]) || 0) >= 13`).
const VIAL_THRESHOLD: f64 = 13.0;

impl Decoder<'_> {
    /// The save's Cooking Mastery state, or `None` when it carries none.
    ///
    /// The page shows "Cooking Mastery data not found in save" in that case: it
    /// needs Rift 59 or the Rift Spooker, and the section has nothing to read.
    pub(crate) fn cooking_mastery(&mut self) -> Option<Mastery> {
        fields::field(self.save, "CookMaster")?;
        let rank = self.nested_row("CookMaster", &[1]);
        let upgrades = self.nested_row("CookMaster", &[2]);
        let per_lv: [f64; 6] = std::array::from_fn(|index| {
            self.data
                .cooking_mastery_per_lv
                .get(index)
                .copied()
                .unwrap_or(0.0)
        });
        let mut mastery = Mastery {
            per_lv,
            level: fields::number_at(&rank, 0),
            ladles: fields::number_at(&rank, 3),
            cooking_levels: self
                .characters
                .iter()
                .map(|character| character.skill(9))
                .sum(),
            divorce_cake: self.nested("Meals", &[0, DIVORCE_CAKE]),
            ribbons: self.ribbon_total(),
            upgrades: std::array::from_fn(|index| fields::number_at(&upgrades, index)),
            companion_87_owned: self.companion_owned(87),
            external: External::default(),
        };
        mastery.external = self.external_multi();
        Some(mastery)
    }

    /// `getTotalRibbonLv`: every ribbon rank from slot 28 on.
    fn ribbon_total(&self) -> f64 {
        self.row("Ribbon")
            .iter()
            .skip(FIRST_RIBBON)
            .map(|rank| rank.unwrap_or(0.0))
            .sum()
    }

    /// `computeExternalMulti`: the factors outside the mastery upgrades.
    fn external_multi(&mut self) -> External {
        External {
            menagerie: self.menagerie_boost_qty(21),
            grid_190: self.grid_bonus(190),
            super_bit_68: self.superbit(68),
            companion_87: self.companion(87),
            fountain: self.fountain(FOUNTAIN_WATER, FOUNTAIN_UPGRADE),
            vial: self.cook_mastery_vial(),
            arcade: self.arcade(69),
            salt_lick: self.row_at("SaltLick", SALT_LICK)
                * self.data.salt_licks.get(SALT_LICK).copied().unwrap_or(0.0),
            vial_level: self.vial_level(),
        }
    }

    /// The Canteen Read vial, with the lab, vault and meritocracy factors the
    /// page applies to it. Zero until the vial is bought.
    fn cook_mastery_vial(&mut self) -> f64 {
        let level = self.vial_level();
        if level <= 0.0 {
            return 0.0;
        }
        let Some(vial) = self.data.cooking_mastery_vial.as_ref() else {
            return 0.0;
        };
        let base = formula::eval(&vial.kind, vial.x1, vial.x2, level);
        let maxed = if self.row_at("Rift", 0) > RIFT_VIALS {
            2.0 * self.vials_at_threshold()
        } else {
            0.0
        };
        let multi = 1.0 + (maxed + self.vault(42)) / 100.0;
        let lab = if self.mainframe(10) == Some(2.0) {
            2.0
        } else {
            1.0
        };
        lab * multi * (1.0 + self.meritoc(20) / 100.0) * base
    }

    /// `CauldronInfo[4][vial]`: the Canteen Read vial's level.
    fn vial_level(&self) -> f64 {
        let Some(vial) = self.data.cooking_mastery_vial.as_ref() else {
            return 0.0;
        };
        let row = fields::row(self.save, "CauldronInfo", &[4]).unwrap_or_default();
        fields::number_at(&row, vial.index)
    }

    /// How many vials are at or past the level the Rift bonus counts.
    fn vials_at_threshold(&self) -> f64 {
        fields::row(self.save, "CauldronInfo", &[4])
            .unwrap_or_default()
            .iter()
            .filter(|level| level.unwrap_or(0.0) >= VIAL_THRESHOLD)
            .count() as f64
    }
}
