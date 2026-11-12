//! The save side of the Minehead tab: the upgrade levels, the current floor
//! and every account term the currency and damage formulas read.
//!
//! Upstream keeps these on `saveData` (`js/save/loader.js`): `stateR7` is
//! `Research[7]`, `mineheadUpgLevels` is `Research[8]`, and the rest come from
//! the account systems the shared decoder already reads. A field the save does
//! not carry is reported, never guessed.

use crate::data::{GameData, game};
use crate::math::{
    self, CurrencyInputs, DamageInputs, ShapeContext, legend40_mine_reduction, qty_levels,
};
use idleon_account::Decoder;
use idleon_research::model::Simulation;
use idleon_save::Save;
use idleon_save::json;

/// Everything the tab and the simulation read, decoded from one save.
#[derive(Clone, Debug, PartialEq)]
pub struct Inputs {
    pub floor: usize,
    /// The mine currency the save holds.
    pub currency: f64,
    /// The highest damage the save has dealt, the farm upgrade's exponent.
    pub highest_dmg: f64,
    pub server_var_hp: f64,
    pub server_var_cost: f64,
    pub research_level: u32,
    /// The thirty upgrade levels, as the save stores them.
    pub levels: Vec<f64>,
    /// Grid node 147's raw level, which Daily Tries reads.
    pub grid_level_147: f64,
    /// The Ring of Glory cost discount.
    pub rog_cost_pct: f64,
    /// The Menagerie-doubled upgrade indices.
    pub doublers: Vec<usize>,
    /// The same levels with the doubled upgrades counted twice.
    pub qty_levels: Vec<f64>,
    /// The seven research-grid nodes the tab reads: 129, 148, 147, 166, 125,
    /// 167 and 146.
    pub grid: [f64; 7],
    /// Button slot 1's bonus with and without grid 125.
    pub button1: f64,
    pub button_no_grid: f64,
    pub comp_143: f64,
    pub comp_143_level2: f64,
    pub atom13: f64,
    pub event_shop_44: f64,
    pub dancing_coral5: f64,
    pub task_currency_level: f64,
    pub arcade62: f64,
    pub legend40: f64,
    pub legend40_pts: f64,
    pub meal_mine_curr: f64,
    pub rog12: f64,
    pub royal_currency_multi: f64,
    pub jelly_currency8: f64,
    pub jelly_damage9: f64,
    pub jelly_gold_tiles45: f64,
    pub bundle_j: bool,
    /// The Mines_Opp reduction: superbit 66, cloud 41 and emporium 45 each
    /// count at most one, plus Legend 40's own term.
    pub reduction: f64,
    pub sailing38: f64,
}

impl Inputs {
    /// The boss health on the current floor.
    pub fn boss_hp(&self) -> f64 {
        math::floor_hp(self.floor, self.server_var_hp)
    }

    /// The depth charges the current floor's grid carries.
    pub fn mines(&self) -> f64 {
        math::mines_on_floor(self.floor, self.reduction, self.legend40_pts)
    }

    /// The lives one game starts with.
    pub fn lives(&self) -> f64 {
        math::max_hp_you(game(), &self.qty_levels)
    }

    /// The grid size at the saved Grid Expansion level.
    pub fn grid_dims(&self) -> (usize, usize) {
        math::grid_dims(game(), self.levels.get(2).copied().unwrap_or(0.0) as usize)
    }

    /// The daily tries, from grid 147's raw level.
    pub fn daily_tries(&self) -> f64 {
        math::daily_tries(self.grid_level_147)
    }

    /// The per-tile damage percentage.
    pub fn per_tile_pct(&self) -> f64 {
        math::bonus_dmg_per_tile_pct(game(), &self.qty_levels, self.grid[6])
    }

    /// The base damage one tile multiplies.
    pub fn base_dmg(&self) -> f64 {
        math::base_dmg(
            game(),
            &self.qty_levels,
            self.grid[5],
            self.sailing38,
            self.dancing_coral5,
            self.jelly_damage9,
            self.legend40,
        )
    }

    /// Currency per hour.
    pub fn currency_per_hour(&self) -> f64 {
        let data = game();
        math::currency_per_hour(
            data,
            &CurrencyInputs {
                grid_129: self.grid[0],
                grid_148: self.grid[1],
                grid_147: self.grid[2],
                grid_166: self.grid[3],
                comp_143: self.comp_143,
                comp_143_level2: self.comp_143_level2,
                bonus_qty_6: data.floor_bonus(6, self.floor),
                atom13: self.atom13,
                meal_mine_curr: self.meal_mine_curr,
                arcade62: self.arcade62,
                rog_bonus_12: self.rog12,
                button_bonus_1: self.button1,
                event_shop_44: self.event_shop_44,
                royal_currency_multi: self.royal_currency_multi,
                jelly_currency_8: self.jelly_currency8,
                bundle_j: self.bundle_j,
                task_currency_level: self.task_currency_level,
                dancing_coral_5: self.dancing_coral5,
                legend40: self.legend40,
                levels: self.qty_levels.clone(),
                highest_dmg: self.highest_dmg,
            },
        )
    }

    /// The damage pipeline's inputs.
    pub fn damage_inputs(&self) -> DamageInputs {
        DamageInputs {
            grid_167: self.grid[5],
            grid_146: self.grid[6],
            wep_pow_dmg_pct: 0.0,
            sailing38: self.sailing38,
            dancing_coral_5: self.dancing_coral5,
            jelly_damage_9: self.jelly_damage9,
            legend40: self.legend40,
        }
    }

    /// The shape ranking's inputs at a candidate shape percentage.
    pub fn shape_context(&self, shape_pct: f64, tiles: f64) -> ShapeContext {
        ShapeContext {
            grid: self.grid,
            meal_mine_curr: self.meal_mine_curr,
            button_no_grid: self.button_no_grid,
            button1: self.button1,
            per_tile: math::upgrade_qty(
                game(),
                9,
                self.levels.get(9).copied().unwrap_or(0.0),
                math::doubler_multi(&self.doublers, 9),
            ),
            shape_pct,
            tiles,
        }
    }

    /// The upgrade cost at the saved level.
    pub fn upgrade_cost(&self, t: usize) -> f64 {
        math::upg_cost(
            game(),
            t,
            self.levels.get(t).copied().unwrap_or(0.0),
            self.levels.get(26).copied().unwrap_or(0.0),
            self.server_var_cost,
            self.rog_cost_pct(),
        )
    }

    /// The Ring of Glory cost discount: the larger of bonuses 1 and 16.
    pub fn rog_cost_pct(&self) -> f64 {
        self.rog_cost_pct
    }
}

/// The base data table, for callers that format upgrade rows.
pub fn data() -> &'static GameData {
    game()
}

/// Decode the tab's inputs, with a warning per field the save does not carry.
pub fn inputs(save: &Save, simulation: &Simulation) -> (Inputs, Vec<String>) {
    let mut warnings = Vec::new();
    let rows = research_rows(save, &mut warnings);
    let state = rows.get(7).cloned().unwrap_or_default();
    let levels: Vec<f64> = rows
        .get(8)
        .map(|row| {
            row.iter()
                .map(|value| json::number(value).unwrap_or(0.0))
                .collect()
        })
        .unwrap_or_default();
    if levels.len() < 30 {
        warnings.push(format!(
            "Research[8]: {} of 30 Minehead upgrade levels; the rest stay at 0",
            levels.len()
        ));
    }
    let floor = state
        .get(4)
        .and_then(json::number)
        .map(|floor| floor.max(0.0) as usize)
        .unwrap_or(0);
    let currency = state.get(5).and_then(json::number).unwrap_or(0.0);
    let highest_dmg = state.get(6).and_then(json::number).unwrap_or(0.0);

    let (server_var_hp, server_var_cost) = server_vars(save);

    let Some((fields, root)) = save.parts() else {
        warnings.push("save: not a JSON object; the Minehead tab has no inputs".into());
        return (empty(levels), warnings);
    };
    let characters = idleon_save::characters(fields);
    let research_level = idleon_research::save::research_level(fields, &characters).unwrap_or(0);
    let mut decoder = Decoder::new(fields, root, &characters, simulation, &mut warnings);
    let grid = [
        decoder.grid_bonus(129),
        decoder.grid_bonus(148),
        decoder.grid_bonus(147),
        decoder.grid_bonus(166),
        decoder.grid_bonus(125),
        decoder.grid_bonus(167),
        decoder.grid_bonus(146),
    ];
    let doublers = decoder.minehead_doublers();
    let qty = qty_levels(&levels, &doublers);

    // Button_Bonuses(1) without grid 125, so a shape change can be applied live.
    let presses = decoder.option(594);
    let data = game();
    let no_grid = if presses > 0.0 {
        let hits = (presses / 45.0).floor() * 5.0 + (presses % 45.0 - 5.0).clamp(0.0, 5.0);
        let rate = data.button_rates.get(1).copied().unwrap_or(0.0);
        hits * rate * (1.0 + decoder.companion(147) / 100.0)
    } else {
        0.0
    };
    let button1 = no_grid * (1.0 + grid[4] / 100.0);
    let comp_143 = decoder.companion(143);
    let comp_143_level2 = if decoder.companion_level2(143) {
        1.0
    } else {
        0.0
    };
    let atom13 = decoder.atom(13);
    let event_shop_44 = decoder.event_shop(44);
    let dancing_coral5 = decoder.dancing_coral(5);
    let task_currency_level = decoder.minehead_task_level();
    let arcade62 = decoder.arcade(62);
    let legend40 = decoder.legend(40);
    let legend40_pts = decoder.spelunk(18, 40);
    let meal_mine_curr = decoder.meal("MineCurr");
    let rog12 = decoder.rog_qty(12);
    let rog_cost_pct = decoder.rog_qty(1).max(decoder.rog_qty(16));
    let grid_level_147 = simulation.grid.level(147) as f64;
    let royal_currency_multi = decoder.outpost_rog(3);
    let jelly_currency8 = decoder.jelly(8);
    let jelly_damage9 = decoder.jelly(9);
    let jelly_gold_tiles45 = decoder.jelly(45);
    let bundle_j = decoder.bundle("ban_j");
    let reduction = decoder.superbit(66).min(1.0)
        + decoder.cloud_bonus(41).min(1.0)
        + decoder.emporium(45).min(1.0)
        + legend40_mine_reduction(legend40_pts);
    let sailing38 = save
        .get("Sailing")
        .and_then(|sailing| json::index(&sailing, &[3, 38]).and_then(json::number))
        .unwrap_or(0.0);

    let inputs = Inputs {
        floor,
        currency,
        highest_dmg,
        server_var_hp,
        server_var_cost,
        research_level,
        levels,
        grid_level_147,
        rog_cost_pct,
        qty_levels: qty,
        doublers,
        grid,
        button1,
        button_no_grid: no_grid,
        comp_143,
        comp_143_level2,
        atom13,
        event_shop_44,
        dancing_coral5,
        task_currency_level,
        arcade62,
        legend40,
        legend40_pts,
        meal_mine_curr,
        rog12,
        royal_currency_multi,
        jelly_currency8,
        jelly_damage9,
        jelly_gold_tiles45,
        bundle_j,
        reduction,
        sailing38,
    };
    (inputs, warnings)
}

/// The upgrade rows, decoded from `Research`'s JSON string when needed.
fn research_rows(save: &Save, warnings: &mut Vec<String>) -> Vec<Vec<serde_json::Value>> {
    let Some(research) = save.get("Research") else {
        warnings.push("Research: not in the save; the Minehead rows are skipped".into());
        return Vec::new();
    };
    let Some(rows) = research.as_array() else {
        warnings.push("Research: not an array of rows; the Minehead rows are skipped".into());
        return Vec::new();
    };
    rows.iter()
        .map(|row| row.as_array().cloned().unwrap_or_default())
        .collect()
}

/// `serverVars.A_MineHP` and `A_MineCost`, defaulting to one upstream.
fn server_vars(save: &Save) -> (f64, f64) {
    let decoded = |key: &str| save.get(key).and_then(|value| json::number(&value));
    let wrapper = |key: &str| {
        save.root()
            .get("serverVars")
            .and_then(|vars| vars.get(key))
            .and_then(json::number)
    };
    (
        decoded("serverVarMineHP")
            .or_else(|| wrapper("A_MineHP"))
            .map(|value| value.max(1.0))
            .unwrap_or(1.0),
        decoded("serverVarMineCost")
            .or_else(|| wrapper("A_MineCost"))
            .map(|value| value.max(1.0))
            .unwrap_or(1.0),
    )
}

fn empty(levels: Vec<f64>) -> Inputs {
    Inputs {
        floor: 0,
        currency: 0.0,
        highest_dmg: 0.0,
        server_var_hp: 1.0,
        server_var_cost: 1.0,
        research_level: 0,
        qty_levels: levels.clone(),
        levels,
        grid_level_147: 0.0,
        rog_cost_pct: 0.0,
        doublers: Vec::new(),
        grid: [0.0; 7],
        button1: 0.0,
        button_no_grid: 0.0,
        comp_143: 0.0,
        comp_143_level2: 0.0,
        atom13: 0.0,
        event_shop_44: 0.0,
        dancing_coral5: 0.0,
        task_currency_level: 0.0,
        arcade62: 0.0,
        legend40: 0.0,
        legend40_pts: 0.0,
        meal_mine_curr: 0.0,
        rog12: 0.0,
        royal_currency_multi: 1.0,
        jelly_currency8: 0.0,
        jelly_damage9: 0.0,
        jelly_gold_tiles45: 0.0,
        bundle_j: false,
        reduction: 0.0,
        sailing38: 0.0,
    }
}
