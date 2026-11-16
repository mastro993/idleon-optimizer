//! The bonuses other systems grant the jar formulas.
//!
//! [`Ext`] is built from the save and the W5 cavern rows; the formulas take it
//! as an argument. Everything the save itself carries is read here. Two bonuses
//! belong to systems this app does not model yet — the Cavern Resource stamp
//! and the farming Exotic 52 — so they stay at a default of zero and the rupie
//! value tree says the total leaves them out.

use super::data::data;
use super::{Jar, row, spelunk};

/// The external bonuses the rupie value, the production rate and the chances read.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Ext {
    /// The Legend talent that raises every collectible level,
    /// which the rupie value, the production and the chances all inherit.
    pub legend_pts: f64,
    /// `ola355`: Rupie Slugs, each multiplying the rupie value by 1.5.
    pub rupie_slugs: f64,
    /// `monument2_1`: the Wisdom: Jar Rupie Value monument bonus.
    pub monument: f64,
    /// `bell4`: the bell that adds extra rupie chance.
    pub bell: f64,
    /// `cosmo0_3`: the Cosmo upgrade that adds extra rupie chance.
    pub cosmo: f64,
    /// The Cavern Resource stamp's percentage, not decoded yet.
    pub stamp_cavern_res: f64,
    /// `cglunko14`: All Cavern Resources, from the Cglunko shop.
    pub cglunko: f64,
    /// `bolaia10`: the Bolaia study that raises the enchant chance.
    pub bolaia: f64,
    /// The palette that raises the collectible chance.
    pub palette: f64,
    /// The farming exotic that raises the collectible chance,
    /// not decoded yet.
    pub exotic: f64,
    /// How many lore episodes the account has found.
    pub lore_episodes: f64,
    /// Whether the SuperBit that scales the lore episodes is owned.
    pub super_bit: f64,
    /// The Fountain's cavern-resource boost.
    pub fountain: f64,
}

impl Ext {
    /// Build the external bonuses from the save the cavern was read out of.
    pub(super) fn read(jar: &Jar) -> Self {
        Self {
            legend_pts: jar.legend_points(29),
            rupie_slugs: jar.opt_lacc(355),
            monument: jar.monument_bonus(2, 1),
            bell: jar.cell(row::BELLS, 4) * 0.5,
            cosmo: jar.cosmo_bonus(row::COSMO, 3, data().cosmo.extra_rupie),
            // The W1 stamp table is not modelled: keep the default of zero, so
            // the total reads as a save with no Cavern Resource stamp.
            stamp_cavern_res: 0.0,
            cglunko: jar.opt_lacc(644) * 10.0,
            bolaia: jar.cell(row::BOLAIA, 10) * 25.0,
            palette: jar.palette_bonus(),
            // The W6 farming exotics are not modelled: Exotic 52 stays neutral.
            exotic: 0.0,
            lore_episodes: jar.spelunk_row(spelunk::LORE_ROW).len() as f64,
            super_bit: super_bit(jar),
            fountain: jar.fountain_total(2),
        }
    }
}

/// The SuperBit 32 key the account's `Gaming[12]` row contains.
fn super_bit(jar: &Jar) -> f64 {
    let key = &data().super_bit32;
    if !key.is_empty() && jar.gaming.contains(key.as_str()) {
        1.0
    } else {
        0.0
    }
}
