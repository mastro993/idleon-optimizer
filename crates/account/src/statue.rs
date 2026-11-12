//! The statue calculator's account inputs.
//!
//! Ports `statueLevelingInputs` from
//! `js/stats/systems/common/statue-leveling.js`: the STARTUE EXP bubble
//! (`AlchemyDescription[3][2]`), the Skinny 0 Cal vial's double chance, the
//! Onyx/Zenith tier multiplier and whether the save carries Royal Guardian
//! Flair. A field the save does not carry reads as zero and is named in a
//! warning, as the upstream UI warns instead of guessing.

use super::Decoder;
use idleon_save::fields;
use idleon_statue::Leveling;
use serde_json::Value;

/// The Royal Guardian row that holds one Flair level per statue.
const FLAIR_ROW: usize = 22;
/// The account option holding the statue tier (2 = Onyx, 3 = Zenith).
const TIER_OPTION: usize = 69;

impl Decoder<'_> {
    /// The account inputs behind every statue target estimate.
    pub(crate) fn statue_leveling(&mut self) -> Leveling {
        let bubble_pct = self.bubble_value(&self.data.bubbles.statue_start).max(0.0);
        let double_chance = self.statue_double_chance();
        let (onyx, zenith) = self.statue_tiers();
        let (flair, flair_available) = self.statue_flair();
        Leveling::new(
            bubble_pct,
            double_chance,
            onyx,
            zenith,
            flair,
            flair_available,
        )
    }

    /// `computeVialByKey('StatueDouble')`: the Skinny 0 Cal vial's chance to
    /// double one deposit, with the lab, Rift, vault and meritocracy bonuses.
    fn statue_double_chance(&mut self) -> f64 {
        let (kind, x1, x2, index) = {
            let vial = &self.data.vials.statue_double;
            (vial.kind.clone(), vial.x1, vial.x2, vial.index)
        };
        let level = self
            .cauldron_row(4)
            .map_or(0.0, |row| fields::number_at(&row, index));
        let chance = if level <= 0.0 {
            0.0
        } else {
            let raw = idleon_common::curve::eval(&kind, x1, x2, level);
            let lab = if self.mainframe(10) == Some(2.0) {
                2.0
            } else {
                1.0
            };
            let rift_active = self.row_at("Rift", 0) > 34.0;
            let maxed_vials = if rift_active {
                self.cauldron_row(4)
                    .map(|row| row.iter().flatten().filter(|level| **level >= 13.0).count())
                    .unwrap_or(0) as f64
            } else {
                0.0
            };
            let vault = self.vault(42);
            let meritoc = self.meritoc(20);
            let bonus = (if rift_active { 2.0 * maxed_vials } else { 0.0 }) + vault;
            lab * (1.0 + bonus / 100.0) * (1.0 + meritoc / 100.0) * raw
        } / 100.0;
        chance.clamp(0.0, 1.0)
    }

    /// `statueGData` counted at or above each tier: Onyx from tier 2, Zenith
    /// from tier 3. `StuG` is the decoded name the upstream loader uses.
    fn statue_tiers(&self) -> (u32, u32) {
        let tier = self.ola(TIER_OPTION);
        let rows = fields::field(self.save, "StuG")
            .map(|value| match value {
                Value::Array(_) => fields::values(&value).unwrap_or_default(),
                _ => Vec::new(),
            })
            .unwrap_or_default();
        let count = |minimum: f64| {
            rows.iter()
                .flatten()
                .filter(|value| **value >= minimum)
                .count() as u32
        };
        let onyx = if tier >= 2.0 { count(2.0) } else { 0 };
        let zenith = if tier >= 3.0 { count(3.0) } else { 0 };
        (onyx, zenith)
    }

    /// `RoyalG[22]`: one Flair level per statue, and whether the save carries
    /// the row at all (`hasRoyalGData`).
    fn statue_flair(&self) -> (Vec<u32>, bool) {
        let available = fields::field(self.save, "RoyalG").is_some();
        let levels = fields::row(self.save, "RoyalG", &[FLAIR_ROW])
            .unwrap_or_default()
            .into_iter()
            .map(|level| level.unwrap_or(0.0).max(0.0).floor() as u32)
            .collect();
        (levels, available)
    }

    /// A cauldron's level row, `CauldronInfo[cauldron]`.
    fn cauldron_row(&self, cauldron: usize) -> Option<Vec<Option<f64>>> {
        let value = fields::field(self.save, "CauldronInfo")?;
        fields::object_row(&value, cauldron)
    }
}
