//! The bonuses other systems grant the cavern, as `jar-calc.html` reads them.
//!
//! Each function is one upstream reader: the lamp's Infinite Resources, the
//! Evertree synergy, the Fountain totals, the monument retelling chain, the
//! Sailing palette and the measurement scale. They live here, away from the
//! save readers, so the formulas in [`super`] read as the upstream page does.

use super::data::data;
use super::{Jar, row, spelunk, state, upgrade};

impl Jar {
    /// `_lampBonus99`: Infinite Resources' rupie value, from the lamp level and
    /// the Spelunking upgrade that scales it.
    pub(super) fn lamp_bonus(&self) -> f64 {
        let lamp = self.cell(row::LAMP, 7);
        25.0 * lamp * (1.0 + self.spelunk(spelunk::LAMP, 2) / 100.0)
    }

    /// `evertreeTrunks`: the trunks Big Jar's rupie synergy counts.
    pub(super) fn evertree_trunks(&self) -> f64 {
        self.cell(row::STATE, state::EVERTREE_TRUNKS)
    }

    /// `evertree`: Big Jar's Evertree ~ Rupie Synergy, 1.10 per trunk.
    pub(super) fn evertree_multiplier(&self) -> f64 {
        (self.upgrade_value(upgrade::EVER_TREE_SYNERGY, 1.0) * 1.1_f64.powf(self.evertree_trunks()))
            .max(1.0)
    }

    /// `marbleBon`: what a Fountain marble level multiplies an upgrade by.
    pub(super) fn marble_bonus(level: f64) -> f64 {
        if level <= 0.0 { 1.0 } else { 1.5 + 0.5 * level }
    }

    /// `bonTOT`: a Fountain upgrade's total at its marble level.
    pub(super) fn fountain_bonus(&self, water: usize, per_level: f64) -> f64 {
        let level = self
            .fountain
            .upgrades
            .get(water)
            .and_then(|row| row.get(13))
            .copied()
            .unwrap_or(0.0);
        let marbles = self
            .fountain
            .marbles
            .get(water)
            .and_then(|row| row.get(13))
            .copied()
            .unwrap_or(0.0);
        (Self::marble_bonus(marbles) * level * per_level).round()
    }

    /// `bonTOT(water, 15)`: the Fountain's cavern-resource boost.
    pub(super) fn fountain_total(&self, water: usize) -> f64 {
        let level = self
            .fountain
            .upgrades
            .get(water)
            .and_then(|row| row.get(15))
            .copied()
            .unwrap_or(0.0);
        let marbles = self
            .fountain
            .marbles
            .get(water)
            .and_then(|row| row.get(15))
            .copied()
            .unwrap_or(0.0);
        (Self::marble_bonus(marbles) * level * data().fountain_per_level.total).round()
    }

    /// `computeMonumentROGbonus`: one monument bonus, through the retelling
    /// chain that multiplies every bonus below the last.
    pub(super) fn monument_bonus(&self, water: usize, index: usize) -> f64 {
        let slot = 10 * water + index;
        let level = self.cell(row::MONUMENTS, slot);
        if level <= 0.0 {
            return 0.0;
        }
        let per_level = data().monument_bonus_per_level(slot);
        if per_level <= 0.0 {
            return 0.0;
        }
        let mut chain = 1.0;
        if index != 9 {
            chain = 1.0
                + self.monument_bonus(water, 9) / 100.0
                + self.cosmo_bonus(row::COSMO, 0, data().cosmo.monument) / 100.0;
        }
        chain *= 1.0 + self.fountain_bonus(water, data().fountain_per_level.monument) / 100.0;
        let chain = chain.max(1.0);
        if per_level < 30.0 {
            level * per_level * chain
        } else {
            0.1 * (level / (250.0 + level) * 10.0 * per_level * chain).ceil()
        }
    }

    /// `computePaletteBonus(2)`: the palette that grants collectible chance.
    pub(super) fn palette_bonus(&self) -> f64 {
        let level = self.spelunk(spelunk::PALETTES, 2);
        if level <= 0.0 {
            return 0.0;
        }
        let palette = &data().palette2;
        // `paletteParams` fixes the decay denominator at 25.
        let raw = if palette.is_decay {
            level / (level + 25.0) * palette.coeff
        } else {
            level * palette.coeff
        };
        let legend = self.legend_points(10);
        let stars = self.spelunk(spelunk::BLACK_STARS, 2);
        let black_star = (1.0 + stars + (stars / 2.0).floor()).max(1.0);
        let lore = if self.spelunk(0, spelunk::LORE) >= 1.0 {
            1.5
        } else {
            1.0
        };
        raw * (1.0 + legend / 100.0) * black_star * lore
    }

    /// `legendPTSbonus`: a legend talent's points times its per-level value.
    pub(super) fn legend_points(&self, index: usize) -> f64 {
        (self.spelunk(spelunk::LEGEND, index) * data().legend_per_point(index)).round()
    }
}
