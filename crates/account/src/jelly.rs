//! The Jelly Operator save slice.
//!
//! Ports the save access in `jelly-operator-calc.html` and
//! `js/stats/systems/w7/jelly-operator.js`: the progress row `Research[7]`, the
//! layout `Research[14]`, the cell EXP and levels `Research[15..16]`, the
//! upgrade levels `Research[17]` and the plot tokens `Research[18]`, plus the
//! account systems the page multiplies: grid nodes 185-187, Spelunking palette
//! 1, Legend Talent 41, the Bloodclot artifact, arcade 72, Atom 15, the `ban_j`
//! bundle, event shop 56/57, Sushi's `rogBonusQTY(63)` and Menagerie's Upgrade
//! Doubler row `Spelunk[53]`.
//!
//! A save without `Research[14]` and `Research[17]` has no board to read; the
//! section names that instead of showing a guessed one.

use super::Decoder;
use idleon_jelly::{Account, Jelly, Progress};
use idleon_save::fields;

/// The progress row: obstruction, tries, Bloodcells, Best DPS, Fever, best run.
const PROGRESS_ROW: usize = 7;
/// The layout row: an anchor-to-cell-type row over the 180 slots.
const LAYOUT_ROW: usize = 14;
/// The cell EXP and level rows.
const CELL_EXP_ROW: usize = 15;
const CELL_LEVEL_ROW: usize = 16;
/// The upgrade level row.
const UPGRADE_ROW: usize = 17;
/// The plot token ids.
const PLOT_ROW: usize = 18;

/// Menagerie's Upgrade Doubler list for the Jelly systems (`Spelunk[53]`).
const JELLY_DOUBLER_ROW: usize = 53;
/// Legend Talent 41's row.
const LEGEND_ROW: usize = 18;
/// The Legend talent whose points grant plot tokens.
const LEGEND_PLOT_TALENT: usize = 41;
/// Sushi's `rogBonusQTY(63)`.
const SASHIMI_63: usize = 63;
/// The Bloodclot artifact: `Sailing[3][40]`.
const BLOODCLOT_ARTIFACT: usize = 40;
/// The Atom row and Sulfur's index.
const ATOM_ROW: usize = 15;

impl Decoder<'_> {
    /// The save's Jelly Operator state, or `None` when it carries no board.
    pub(crate) fn jelly_state(&mut self) -> Option<Jelly> {
        fields::row(self.save, "Research", &[LAYOUT_ROW])?;
        let upgrade_row = fields::row(self.save, "Research", &[UPGRADE_ROW])?;
        let progress = fields::row(self.save, "Research", &[PROGRESS_ROW]).unwrap_or_default();
        let layout = idleon_jelly::board::layout_from_save(
            &fields::row(self.save, "Research", &[LAYOUT_ROW]).unwrap_or_default(),
        );
        let cell_exp = fields::row(self.save, "Research", &[CELL_EXP_ROW]).unwrap_or_default();
        let cell_level = fields::row(self.save, "Research", &[CELL_LEVEL_ROW]).unwrap_or_default();
        let plots = fields::row(self.save, "Research", &[PLOT_ROW]).unwrap_or_default();
        let at = |index: usize| {
            progress
                .get(index)
                .copied()
                .flatten()
                .unwrap_or(0.0)
                .max(0.0)
        };
        let doubled = self
            .spelunk_row(JELLY_DOUBLER_ROW)
            .iter()
            .flatten()
            .map(|value| value.round() as usize)
            .collect();
        let account = Account {
            grid_185: self.grid_bonus(185),
            grid_187: self.grid_bonus(187),
            grid_186: f64::from(self.grid.level(186)),
            palette_1: self.palette(1),
            legend_points: self.spelunk(LEGEND_ROW, LEGEND_PLOT_TALENT),
            legend_41: self.legend(LEGEND_PLOT_TALENT),
            bloodclot_tier: fields::at(self.save, "Sailing", &[3, BLOODCLOT_ARTIFACT])
                .unwrap_or(0.0),
            arcade_72: self.arcade(72),
            atom_15: self.row_at("Atoms", ATOM_ROW).max(0.0).floor()
                * idleon_jelly::data().atom_jelly_per_level,
            bundle_ban_j: self.bundle("ban_j"),
            event_shop_56: self.event_shop(56),
            event_shop_57: self.event_shop(57),
            legend_slot_tokens: (self.spelunk(LEGEND_ROW, LEGEND_PLOT_TALENT) / 2.0)
                .ceil()
                .min(3.0),
            rog_63: self.rog_qty(SASHIMI_63),
            doubled,
            research_level: f64::from(self.save_research_level),
        };
        let mut level = [0_u32; 9];
        for (cell_type, raw) in cell_level.iter().take(9).enumerate() {
            level[cell_type] = raw.map_or(0, |value| value.max(0.0).floor() as u32);
        }
        let mut exp = [0.0_f64; 9];
        for (cell_type, raw) in cell_exp.iter().take(9).enumerate() {
            exp[cell_type] = raw.unwrap_or(0.0).max(0.0);
        }
        let mut plot_ids: Vec<usize> = plots
            .iter()
            .flatten()
            .map(|value| value.max(0.0).floor() as usize)
            .collect();
        plot_ids.sort_unstable();
        plot_ids.dedup();
        Some(Jelly {
            progress: Progress {
                obstruction: at(9).floor() as usize,
                tries: at(10),
                bloodcells: at(11),
                best_dps: at(12),
                fever: at(13).floor() as u32,
                best_bloodcells: at(14),
            },
            layout,
            cell_exp: exp,
            cell_level: level,
            upgrades: upgrade_row
                .iter()
                .map(|value| value.unwrap_or(0.0).max(0.0).floor() as u32)
                .collect(),
            plots: plot_ids,
            account,
        })
    }
}
