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
mod friend;
mod jelly;
mod lab;
mod mastery;
mod opal;
mod prayer;
mod sneaking;
mod statue;
mod sushi;
mod systems;
mod tome;
mod tome_high;
mod tree;

use data::{AccountData, account_data};
use idleon_research::data::game;
use idleon_research::model::{GridState, SimContext, Simulation};
use idleon_save::Character;
use idleon_save::Save;
use idleon_save::fields;
use serde_json::{Map, Value};

pub use prayer::prayer;

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
    pub(crate) fn derived(name: &'static str, value: f64) -> Self {
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

/// A read-only view of the account systems one save carries.
///
/// Every account-wide lookup (vault, companions, event shop, lab, …) is a
/// method on this view, so the calculators that need one read it from here
/// instead of re-decoding the save.
pub struct Decoder<'a> {
    pub(crate) save: &'a Map<String, Value>,
    pub(crate) root: &'a Map<String, Value>,
    pub(crate) data: &'static AccountData,
    pub(crate) characters: &'a [Character],
    pub(crate) grid: &'a GridState,
    pub(crate) overlay: Vec<i32>,
    /// The level the run starts at, which the mastery forecast advances.
    pub(crate) research_level: u32,
    /// The save's own research level, which the mastery forecast starts from.
    pub(crate) save_research_level: u32,
    /// Whether the lab graph is being derived; the mainframe lookup is 0 while
    /// it is, as it is before `computeLabConnectivity` finishes upstream.
    pub(crate) deriving_lab: bool,
    pub(crate) warnings: &'a mut Vec<String>,
}

impl<'a> Decoder<'a> {
    /// A view of `save` (the decoded field map) and `root` (its `it.json`
    /// wrapper, or the save itself) for the grid and level `simulation` holds.
    pub fn new(
        save: &'a Map<String, Value>,
        root: &'a Map<String, Value>,
        characters: &'a [Character],
        simulation: &'a Simulation,
        warnings: &'a mut Vec<String>,
    ) -> Self {
        Self {
            save,
            root,
            data: account_data(),
            characters,
            grid: &simulation.grid,
            overlay: simulation.overlay(),
            research_level: simulation.research_level,
            save_research_level: simulation.research_level,
            deriving_lab: false,
            warnings,
        }
    }

    /// The decoded save field map.
    pub fn save(&self) -> &'a Map<String, Value> {
        self.save
    }

    /// The save's characters, by `Lv0_<n>` index.
    pub fn characters(&self) -> &'a [Character] {
        self.characters
    }

    /// The save field a system did not find, warned about once.
    pub(crate) fn warn(&mut self, field: &str, detail: &str) {
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
    pub(crate) fn omission(
        &mut self,
        name: &'static str,
        field: &'static str,
        detail: &str,
    ) -> BonusTerm {
        self.warn(field, detail);
        BonusTerm::omitted(name, field)
    }

    /// The grid bonus of one cell, as `gbWith` computes it.
    pub fn grid_bonus(&self, cell: usize) -> f64 {
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

/// Decode the Cooking Mastery state the section reads.
///
/// The page's `computeExternalMulti` reads systems the same decoder owns, so
/// this runs one on the same save instead of a second parser.
pub fn cooking_mastery(
    save: &Map<String, Value>,
    root: &Map<String, Value>,
    characters: &[Character],
    simulation: &Simulation,
    warnings: &mut Vec<String>,
) -> Option<idleon_cooking_mastery::Mastery> {
    let mut decoder = Decoder::new(save, root, characters, simulation, warnings);
    let mastery = decoder.cooking_mastery();
    if mastery.is_none() {
        decoder.warn(
            "CookMaster",
            "not in the save; the Cooking Mastery section has no upgrades to read",
        );
    }
    mastery
}

/// Decode the yellow point allocator's meal state.
///
/// The allocator reads one more row of the same section (the per-meal yellow
/// points) plus the systems the decoder already owns: the per-meal ribbon
/// bonus, the meal multiplier, the character levels the stat goals scale with
/// and the kitchen rows.
pub fn yellow(
    save: &Map<String, Value>,
    root: &Map<String, Value>,
    characters: &[Character],
    simulation: &Simulation,
    warnings: &mut Vec<String>,
) -> Option<idleon_cooking_mastery::Yellow> {
    let mut decoder = Decoder::new(save, root, characters, simulation, warnings);
    let yellow = decoder.yellow();
    if yellow.is_none() {
        decoder.warn(
            "CookMaster",
            "not in the save; the yellow point allocator has no meals to spend on",
        );
    }
    yellow
}

/// Decode the Jelly Operator slice the section reads.
///
/// The page also reads systems this decoder already owns (the research grid
/// nodes, Spelunking palettes, arcade, event shop, Sushi and Menagerie), so it
/// runs one on the same save instead of a second parser.
pub fn jelly(
    save: &Map<String, Value>,
    characters: &[Character],
    simulation: &Simulation,
    warnings: &mut Vec<String>,
) -> Option<idleon_jelly::Jelly> {
    let mut decoder = Decoder::new(save, save, characters, simulation, warnings);
    let jelly = decoder.jelly_state();
    if jelly.is_none() {
        decoder.warn(
            "Research[14]/[17]",
            "not in the save; the Jelly Operator section has no board to read",
        );
    }
    jelly
}

/// Decode the account terms into the simulator context and its tree.
pub fn account(
    save: &Map<String, Value>,
    root: &Map<String, Value>,
    characters: &[Character],
    simulation: &Simulation,
    warnings: &mut Vec<String>,
) -> (SimContext, BonusTree) {
    let mut decoder = Decoder::new(save, root, characters, simulation, warnings);
    let mut context = decoder.context();
    let tree = decoder.tree(&mut context);
    (context, tree)
}

/// The Opal Optimizer's villagers and every account bonus their EXP rates
/// apply, or `None` when the save carries no `Holes` rows.
pub fn opal(
    save: &Map<String, Value>,
    root: &Map<String, Value>,
    characters: &[Character],
    simulation: &Simulation,
) -> Option<idleon_opal::Opal> {
    let mut warnings = Vec::new();
    let mut decoder = Decoder::new(save, root, characters, simulation, &mut warnings);
    decoder.opal()
}

/// The account-wide terms the sneaking breakdown trees read.
pub fn sneaking_account(
    save: &Save,
    loadout: &idleon_sneaking::Loadout,
) -> idleon_sneaking::Account {
    let Some((fields, root)) = save.parts() else {
        return idleon_sneaking::Account::default();
    };
    let characters = idleon_save::characters(fields);
    let simulation = Simulation::default();
    let mut warnings = Vec::new();
    let mut decoder = Decoder::new(fields, root, &characters, &simulation, &mut warnings);
    decoder.sneaking_account(loadout)
}

/// Decode the statue calculator's account inputs: the STARTUE EXP bubble, the
/// Skinny 0 Cal vial, the Onyx/Zenith tier multiplier and the Royal Guardian
/// Flair levels.
pub fn statue_leveling(
    save: &Map<String, Value>,
    root: &Map<String, Value>,
    characters: &[Character],
    simulation: &Simulation,
    warnings: &mut Vec<String>,
) -> idleon_statue::Leveling {
    for (field, detail) in [
        (
            "CauldronInfo",
            "not in the save; the STARTUE EXP bubble and Skinny 0 Cal vial read as zero",
        ),
        (
            "RoyalG",
            "not in the save; statue Flair levels are unknown and estimates assume no Flair bonus",
        ),
        (
            "StuG",
            "not in the save; the Onyx/Zenith statue multiplier stays at 1×",
        ),
    ] {
        if fields::field(save, field).is_none()
            && !warnings.iter().any(|warning| warning.starts_with(field))
        {
            warnings.push(format!("{field}: {detail}"));
        }
    }
    let mut decoder = Decoder::new(save, root, characters, simulation, warnings);
    decoder.statue_leveling()
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
        context.companion_28 = self.companion(28);
        context.companion_153_level2 = if self.companion_level2(153) { 1.0 } else { 0.0 };
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
        context.save_global_time = self.save_global_time();
        context.tournament_last_day = self.ola(511);
        context.tournament_cached_day = self.ola(496);
        context.bundle_ban_m = self.bundle("ban_m");
        context.bundle_ban_k = self.bundle("ban_k");
        context.sailing_art37 = self.sailing_art37();
        context.sailing_art36 = self.sailing_art36();
        context.card_w7b11 = self.card_level("w7b11").min(10.0);
        context.gem_shop_45 = self.row_at("GemItemsPurchased", 45);
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

/// `getPrismaBonusMult` for the imported save, with no grid or character state.
///
/// The App data needs the companion roster's bonus without building a full
/// simulation: `companionBonusForSave`.
pub fn companion_bonus(id: usize, enhanced: bool) -> f64 {
    data::account_data().companion(id, enhanced)
}

/// `vaultUpgBonus(index)`: the App data uses upgrade 11 as flat carry capacity.
pub fn vault_bonus(save: &Save) -> f64 {
    let Some((fields, _)) = save.parts() else {
        return 0.0;
    };
    bonuses::vault_upgrade_bonus(fields, 11)
}

/// `mainframeBonus(index)`: the lab's connected-mainframe bonus, when the save
/// carries a lab layout. The stat breakdowns read node 7 (Certified Stamp Book).
pub fn mainframe_bonus(save: &Save, index: usize) -> Option<f64> {
    let (fields, root) = save.parts()?;
    let characters = idleon_save::characters(fields);
    let simulation = Simulation::default();
    let mut warnings = Vec::new();
    let mut decoder = Decoder::new(fields, root, &characters, &simulation, &mut warnings);
    decoder.mainframe(index)
}

/// The Prisma calculator needs the multiplier the game applies to every
/// Prisma-ed bubble; every term it sums comes from account fields, so this
/// reads them without building a full simulation.
pub fn prisma_multiplier(save: &Save) -> f64 {
    let Some((fields, root)) = save.parts() else {
        return 1.0;
    };
    let simulation = Simulation::default();
    let mut warnings = Vec::new();
    let decoder = Decoder::new(fields, root, &[], &simulation, &mut warnings);
    decoder.prisma_bonus_multi().max(1.0)
}
