//! The account side of a save: every multiplier the research simulator reads.
//!
//! This is a port of the upstream save context (`js/save/context.js`) and the
//! research EXP descriptor (`js/stats/defs/research-exp.js`). Each term is
//! derived from the save field that carries it; when a save does not carry the
//! field, the term is omitted and warned about with that field's name. No term
//! is guessed.
//!
//! The tree is reported alongside the context so the UI can show where a
//! multiplier came from: [`BonusTree::additive`] and [`BonusTree::multipliers`]
//! hold the upstream descriptor's items, and `SimContext::extra_pct` is the
//! additive total with the grid, sticker and mastery terms removed, exactly as
//! `simTotalExpWith` expects.

mod bonuses;
mod cooking;
mod data;
mod equipment;
mod fields;
mod lab;
mod systems;
mod tome;
mod tome_high;
mod tree;

use super::Character;
use crate::data::game;
use crate::model::{GridState, SimContext, Simulation};
use data::{AccountData, account_data};
use serde_json::{Map, Value};

/// One line of the save-derived multiplier tree.
#[derive(Clone, Debug, PartialEq)]
pub struct BonusTerm {
    /// What the term is called in the upstream breakdown.
    pub name: &'static str,
    /// Its value: a percentage in the additive list, a factor in the list of
    /// true multipliers.
    pub value: f64,
    /// The save field the term needed and the save did not carry, when the
    /// term had to be omitted.
    pub omitted: Option<&'static str>,
}

impl BonusTerm {
    pub(super) fn derived(name: &'static str, value: f64) -> Self {
        Self {
            name,
            value,
            omitted: None,
        }
    }

    fn omitted(name: &'static str, field: &'static str) -> Self {
        Self {
            name,
            value: 0.0,
            omitted: Some(field),
        }
    }

    /// Whether the term was left out because its save field is missing.
    pub fn is_omitted(&self) -> bool {
        self.omitted.is_some()
    }
}

/// The research EXP multiplier tree the save decodes into.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BonusTree {
    /// Account bonuses that add together, as percentages.
    pub additive: Vec<BonusTerm>,
    /// Bonuses that multiply the additive result.
    pub multipliers: Vec<BonusTerm>,
    /// The additive total before the grid, sticker and mastery terms are
    /// removed for the simulator.
    pub additive_pct: f64,
    /// The product of [`Self::multipliers`].
    pub multiplier: f64,
}

impl BonusTree {
    /// A term by name in either list.
    pub fn term(&self, name: &str) -> Option<&BonusTerm> {
        self.additive
            .iter()
            .chain(&self.multipliers)
            .find(|term| term.name == name)
    }
}

/// The save fields one run of the decoder reads.
pub(super) struct Decoder<'a> {
    pub(super) save: &'a Map<String, Value>,
    pub(super) root: &'a Map<String, Value>,
    pub(super) data: &'static AccountData,
    pub(super) characters: &'a [Character],
    pub(super) grid: &'a GridState,
    pub(super) overlay: &'a [i32],
    /// The level the run starts at, which the mastery forecast advances.
    pub(super) research_level: u32,
    /// The save's own research level, which the mastery forecast starts from.
    pub(super) save_research_level: u32,
    /// Whether the lab graph is being derived; the mainframe lookup is 0 while
    /// it is, as it is before `computeLabConnectivity` finishes upstream.
    pub(super) deriving_lab: bool,
    pub(super) warnings: &'a mut Vec<String>,
}

impl Decoder<'_> {
    /// The save field a system did not find, warned about once.
    pub(super) fn warn(&mut self, field: &str, detail: &str) {
        if self
            .warnings
            .iter()
            .any(|warning| warning.starts_with(field))
        {
            return;
        }
        self.warnings.push(format!("{field}: {detail}"));
    }

    /// Report a term the decoder could not derive.
    pub(super) fn omission(
        &mut self,
        name: &'static str,
        field: &'static str,
        detail: &str,
    ) -> BonusTerm {
        self.warn(field, detail);
        BonusTerm::omitted(name, field)
    }

    /// The grid bonus of one cell, as `gbWith` computes it.
    pub(super) fn grid_bonus(&self, cell: usize) -> f64 {
        let Some(node) = game().node_at(cell) else {
            return 0.0;
        };
        let level = self.grid.level(cell);
        if level == 0 {
            return 0.0;
        }
        let mut value = node.bonus_per_lv * level as f64;
        let shape = self.overlay.get(cell).copied().unwrap_or(-1);
        if let Some(shape) = usize::try_from(shape)
            .ok()
            .and_then(|s| game().shapes.get(s))
        {
            value *= 1.0 + shape.bonus_pct / 100.0;
        }
        value * self.all_bonus_multi()
    }
}

/// Decode the account terms into the simulator context and its tree.
pub(crate) fn account(
    save: &Map<String, Value>,
    root: &Map<String, Value>,
    characters: &[Character],
    simulation: &Simulation,
    warnings: &mut Vec<String>,
) -> (SimContext, BonusTree) {
    let overlay = simulation.overlay();
    let mut decoder = Decoder {
        save,
        root,
        data: account_data(),
        characters,
        grid: &simulation.grid,
        overlay: &overlay,
        research_level: simulation.research_level,
        save_research_level: simulation.research_level,
        deriving_lab: false,
        warnings,
    };
    let mut context = decoder.context();
    let tree = decoder.tree(&mut context);
    (context, tree)
}

impl Decoder<'_> {
    fn context(&mut self) -> SimContext {
        let mut context = SimContext::default();
        context.research_level = self.research_level;
        context.server_var_res_xp = self.server_var();
        context.event_shop = self.event_shop_flags();
        context.superbits = self.superbit_flags();
        context.emporium = self.emporium_flags();
        context.minehead = self.minehead();
        context.all_bonus_multi = self.all_bonus_multi();
        context.rog = self.rog();
        context.companion_52 = self.companion_52();
        context.sticker_fixed = self.sticker_fixed();
        context.boony_count = self.boony_count();
        context.event_shop_37 = context.event_shop(37);
        let (button_base, button) = self.button0();
        context.button_base_no_grid = button_base;
        context.button0 = button;
        context.killroy5 = self.killroy5();
        context.dream14 = self.dream14();
        context.cglunko11 = self.cglunko11();
        context.fountain_2_16 = self.fountain_2_16();
        context.companion_54_level2 = if self.companion_level2(54) { 1.0 } else { 0.0 };
        context.companion_153 = self.companion(153) > 0.0;
        context.royal_research = self.royal_research();
        context.sticker_multiplicative = self.menagerie_unlock(22) == 1.0;
        context.arcade63 = self.arcade(63);
        context.clam_work9 = if self.ola(464) > 9.0 { 1.0 } else { 0.0 };
        context.menagerie_b3 = self.menagerie_boost_qty(3);
        context.research_equipment_multi = self.research_equipment_multi().unwrap_or(1.0);
        context.jelly_research_multi = self.jelly_research_multi();
        context.jelly_insight_pct = self.jelly(7);
        context.jelly_kalei_pct = self.jelly(25);
        let mastery = self.mastery();
        context.mastery_additive_pct = mastery.additive;
        context.mastery_exp_multi = mastery.exp_multi;
        context.spellunky_upg7 = self.spelunk(0, 7);
        context.bundle_ban_m = self.bundle("ban_m");
        context.bundle_ban_k = self.bundle("ban_k");
        context.sailing_art37 = self.sailing_art37();
        context.task_grid_points = self.task_grid_points();
        context.jelly_grid_points = self.jelly(4) + self.jelly(57);
        context.bonus_grid_points = self.grid_point_sources(&context).iter().sum::<f64>() as u32;
        context
    }

    /// `gridPointSources` from `js/stats/systems/w7/research-math.js`.
    fn grid_point_sources(&self, context: &SimContext) -> Vec<f64> {
        vec![
            if self.companion(153) > 0.0 { 10.0 } else { 0.0 },
            if self.companion_level2(153) { 5.0 } else { 0.0 },
            context.rog(3),
            context.rog(13),
            context.sailing_art37,
            context.task_grid_points,
            self.jelly(4),
            self.jelly(57),
            if context.bundle_ban_k { 5.0 } else { 0.0 },
            context.event_shop(64),
            self.mastery().grid_points as f64,
        ]
    }
}
