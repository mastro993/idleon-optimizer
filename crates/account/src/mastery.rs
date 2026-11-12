//! The Cooking Mastery inputs the Mastery EXP tab and the yellow point
//! allocator read.
//!
//! Ports the save access in `cooking-mastery-calc.html` (`getCM`,
//! `getTotalCookingLv`, `getMeal73Level`, `getTotalRibbonLv`) and its
//! `computeExternalMulti`. The mastery upgrades themselves are
//! `CookMaster[2]`, the rank row is `CookMaster[1]`, the Divorce Cake level is
//! `Meals[0][73]` and the ribbon ranks start at `Ribbon[28]`.
//!
//! The allocator reads one more row of the same section: `CookMaster[0]`, the
//! yellow points each meal holds, beside the meal levels in `Meals[0]`.
//!
//! Every external term is the upstream system's own function: the grid bonus of
//! node 190, superbit 68, companion 87, menagerie boost 21, `Fountain_BonTOT(2,
//! 17)`, the Canteen Read vial, arcade 69 and salt lick 10. A save that lacks a
//! field leaves that term at zero; none of them is guessed.

use super::Decoder;
use idleon_common::curve as formula;
use idleon_cooking_mastery::{External, Kitchen, Mastery, MealCharacter, Yellow};
use idleon_save::Character;
use idleon_save::fields;
use serde_json::Value;

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

/// The event shop perk that doubles the meal cook goal (`eventShopOwned(53)`).
const EVENT_COOK_MULTI: usize = 53;

/// The event shop perk that grants yellow points (`eventShopOwned(54)`).
const EVENT_YELLOW_POINTS: usize = 54;

/// The Jelly Operator reward that grants yellow points (`jellyRewardBonus(5)`).
const JELLY_YELLOW_POINTS: usize = 5;

/// The grid node whose level grants yellow points (`gridLevels[190]`).
const GRID_YELLOW_POINTS: usize = 190;

/// The Rift Spooker companion, which grants five yellow points.
const COMPANION_YELLOW_POINTS: usize = 87;

/// The Rift depth that turns the vial threshold on (`riftData[0] > 34`).
const RIFT_VIALS: f64 = 34.0;

/// The vial level the Rift bonus counts (`(Number(vials[rvi]) || 0) >= 13`).
const VIAL_THRESHOLD: f64 = 13.0;

/// A `CookMaster[0]` or `Meals[0]` row, one number per `MealINFO` row.
fn meal_row(row: &[Option<f64>]) -> Vec<f64> {
    (0..idleon_cooking_mastery::meals().len())
        .map(|index| fields::number_at(row, index))
        .collect()
}

impl Decoder<'_> {
    /// The save's meal mastery state, or `None` when it carries no
    /// `CookMaster` rows for the allocator to read.
    pub(crate) fn yellow(&mut self) -> Option<Yellow> {
        fields::field(self.save, "CookMaster")?;
        let rank = self.nested_row("CookMaster", &[1]);
        let mastery = self.nested_row("CookMaster", &[0]);
        let levels = self.nested_row("Meals", &[0]);
        let budget = self.yellow_point_budget(fields::number_at(&rank, 0));
        let characters = self.meal_characters();
        let kitchens = self.kitchens();
        Some(Yellow {
            mastery: meal_row(&mastery),
            levels: meal_row(&levels),
            ribbons: (0..idleon_cooking_mastery::meals().len())
                .map(|meal| self.ribbon(28 + meal))
                .collect(),
            cook_multi: self
                .cooking_meal_multi()
                .map(|cooking| cooking.value)
                .unwrap_or(1.0),
            budget,
            characters,
            kitchens,
            event_53: self.event_shop(EVENT_COOK_MULTI) > 0.0,
        })
    }

    /// `yellowPointBudget`: the rank, the companion, the grid node, the event
    /// shop and the Jelly reward, rounded the way the page rounds it.
    fn yellow_point_budget(&self, rank: f64) -> f64 {
        let companion = 5.0 * self.companion(COMPANION_YELLOW_POINTS);
        let grid = f64::from(self.grid.level(GRID_YELLOW_POINTS));
        let event = 5.0 * self.event_shop(EVENT_YELLOW_POINTS);
        let jelly = self.jelly(JELLY_YELLOW_POINTS);
        (rank + 1.0 + companion + grid + event + jelly)
            .max(0.0)
            .round()
    }

    /// The characters the stat goals scale with: the `charNames` roster beside
    /// the save, or the `Lv0_<n>` rows when it has none.
    fn meal_characters(&self) -> Vec<MealCharacter> {
        let names: Vec<String> = self
            .root
            .get("charNames")
            .and_then(Value::as_array)
            .map(|names| {
                names
                    .iter()
                    .filter_map(|name| name.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        let count = names.len().max(self.characters.len());
        (0..count)
            .map(|index| MealCharacter {
                name: names
                    .get(index)
                    .cloned()
                    .unwrap_or_else(|| format!("Character {}", index + 1)),
                farming: self
                    .characters
                    .get(index)
                    .and_then(Character::farming_level)
                    .map(f64::from)
                    .unwrap_or(0.0),
                summoning: self
                    .characters
                    .get(index)
                    .and_then(Character::summoning_level)
                    .map(f64::from)
                    .unwrap_or(0.0),
            })
            .collect()
    }

    /// `kitchenCookingAggregate`'s input: every unlocked `Cooking` row, with the
    /// gem purchase the kitchen's rate doubles or triples on.
    fn kitchens(&self) -> Vec<Kitchen> {
        let gem_kitchens = self.row_at("GemItemsPurchased", 120).floor();
        let Some(rows) = fields::field(self.save, "Cooking") else {
            return Vec::new();
        };
        let Some(rows) = rows.as_array() else {
            return Vec::new();
        };
        rows.iter()
            .enumerate()
            .filter_map(|(index, row)| {
                let cells = fields::values(row)?;
                if cells.len() <= 8 || fields::number_at(&cells, 0) == 0.0 {
                    return None;
                }
                Some(Kitchen {
                    speed: fields::number_at(&cells, 6),
                    fire: fields::number_at(&cells, 7),
                    luck: fields::number_at(&cells, 8),
                    gem: (index as f64) < gem_kitchens,
                })
            })
            .collect()
    }
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
