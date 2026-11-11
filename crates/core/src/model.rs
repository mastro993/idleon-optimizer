//! Simulation state, its save-derived knobs, and the messages the optimizers take.

use crate::data::game;
use serde::{Deserialize, Serialize};

/// Levels of the grid squares, indexed by cell.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GridState {
    levels: Vec<u32>,
}

impl GridState {
    /// An empty grid, every square at level zero.
    pub fn empty() -> Self {
        Self {
            levels: vec![0; game().cells()],
        }
    }

    /// A grid from raw levels; panics when the length is not the grid's.
    pub fn from_levels(levels: Vec<u32>) -> Self {
        assert_eq!(levels.len(), game().cells(), "grid levels length");
        Self { levels }
    }

    /// A grid with every node at one level, the layout a new save starts near.
    pub fn seeded(level: u32) -> Self {
        let mut grid = Self::empty();
        for node in &game().nodes {
            grid.levels[node.id] = level.min(node.max_lv);
        }
        grid
    }

    pub fn level(&self, cell: usize) -> u32 {
        self.levels.get(cell).copied().unwrap_or(0)
    }

    pub fn set(&mut self, cell: usize, level: u32) {
        self.levels[cell] = level;
    }

    pub fn levels(&self) -> &[u32] {
        &self.levels
    }

    pub fn spent(&self) -> u32 {
        self.levels.iter().sum()
    }

    /// Every cell with a level on it.
    pub fn live_cells(&self) -> impl Iterator<Item = usize> + '_ {
        self.levels
            .iter()
            .enumerate()
            .filter(|(_, level)| **level > 0)
            .map(|(cell, _)| cell)
    }
}

/// A shape's polygon anchored on the grid, in grid pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    pub shape: usize,
    pub x: f64,
    pub y: f64,
    pub rot: f64,
}

/// Which kind of magnifier a pool entry is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MagKind {
    /// Magnifying Glass: flat research EXP on its observation.
    Regular,
    /// Optical Monocle: insight for its observation.
    Monocle,
    /// Kaleidoscope: multiplies its neighbours.
    Kaleidoscope,
}

/// One magnifier and the observation it sits on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mag {
    pub kind: MagKind,
    pub slot: Option<usize>,
}

impl Mag {
    pub fn new(kind: MagKind, slot: Option<usize>) -> Self {
        Self { kind, slot }
    }
}

/// Per-observation progress.
#[derive(Clone, Debug, PartialEq)]
pub struct Observations {
    pub found: Vec<bool>,
    pub insight_level: Vec<u32>,
    pub insight_exp: Vec<f64>,
}

impl Default for Observations {
    fn default() -> Self {
        let count = game().observations.len();
        Self {
            found: vec![false; count],
            insight_level: vec![0; count],
            insight_exp: vec![0.0; count],
        }
    }
}

impl Observations {
    /// Every observation found, with its insight at `level`.
    pub fn all_at(level: u32) -> Self {
        let mut observations = Self::default();
        observations.found.fill(true);
        observations.insight_level.fill(level);
        observations
    }
}

/// Save-derived multipliers that are not on the grid.
///
/// Shapes the upstream save context and `simTotalExpWith` read: the fields
/// named after `js/save/context.js` keep that meaning here. Every value is
/// decoded from the save by `save::account`; none is guessed.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SimContext {
    /// The save's research level, kept so the mastery forecast can advance it.
    pub research_level: u32,
    /// Save constant scaling the research EXP curve.
    pub server_var_res_xp: f64,
    /// The all-bonus multiplier every grid square gets (`1.0` is none).
    pub all_bonus_multi: f64,
    /// Additive account bonuses the grid does not provide, as a percentage.
    /// Upstream calls this `extPctExSticker`: the additive pool with the
    /// sticker, grid and mastery terms removed.
    pub extra_pct: f64,
    /// Companion 52's bonus, already folded with companion 153's.
    pub companion_52: f64,
    /// Companion 28's bonus, the research AFK gains term.
    pub companion_28: f64,
    /// Companion 153's second level, worth another +10% AFK gains.
    pub companion_153_level2: f64,
    /// The sticker bonus before the crown multiplier.
    pub sticker_fixed: f64,
    /// Rat King crowns found, which scale the sticker.
    pub boony_count: f64,
    /// Event shop 37, a flat sticker bonus.
    pub event_shop_37: f64,
    /// Button slot 0's bonus.
    pub button0: f64,
    /// Button slot 0 without grid 125, so the simulator can apply it live.
    pub button_base_no_grid: f64,
    /// Killroy bonus 5.
    pub killroy5: f64,
    /// Nonstop Studies levels (`Dream[14]`).
    pub dream14: f64,
    /// Cglunko research upgrade 11.
    pub cglunko11: f64,
    /// Fountain Pen N Paper (`Holes[31][2][16]`), rounded.
    pub fountain_2_16: f64,
    /// Companion 54's second level, worth another +50%.
    pub companion_54_level2: f64,
    /// Whether companion 153 is owned (one extra magnifier).
    pub companion_153: bool,
    /// Royal Guardian research focus.
    pub royal_research: f64,
    /// Whether Menagerie unlock 22 moved the sticker to its own multiplier.
    pub sticker_multiplicative: bool,
    /// Arcade upgrade 63.
    pub arcade63: f64,
    /// Clam Works promotion 9.
    pub clam_work9: f64,
    /// Menagerie boost 3.
    pub menagerie_b3: f64,
    /// Equipment research EXP, capped at 15x.
    pub research_equipment_multi: f64,
    /// Jelly rewards 0, 23, 46 and 54 as one factor.
    pub jelly_research_multi: f64,
    /// Jelly reward 7.
    pub jelly_insight_pct: f64,
    /// Jelly reward 25.
    pub jelly_kalei_pct: f64,
    /// Ring of Glory bonuses by index, zero when locked.
    pub rog: Vec<f64>,
    /// Research skill mastery's additive bonus.
    pub mastery_additive_pct: f64,
    /// Research skill mastery's multiplier.
    pub mastery_exp_multi: f64,
    /// Event shop items owned, by index.
    pub event_shop: Vec<u8>,
    /// Superbits owned, by index.
    pub superbits: Vec<u8>,
    /// Ninja emporium bonuses owned, by index.
    pub emporium: Vec<u8>,
    /// Minehead floor bonuses, by index.
    pub minehead: Vec<f64>,
    /// Flat grid points from companions, artifacts, tasks and bundles.
    pub bonus_grid_points: u32,
    /// Whether the `ban_m` bundle is owned (one extra shape).
    pub bundle_ban_m: bool,
    /// Whether the `ban_k` bundle is owned (five grid points).
    pub bundle_ban_k: bool,
    /// Sailing artifact 37, capped at ten grid points.
    pub sailing_art37: f64,
    /// Sailing artifact 36, the research AFK gains term (capped at 6).
    pub sailing_art36: f64,
    /// Card `w7b11`'s level, the pirate deckhand AFK term (capped at 10).
    pub card_w7b11: f64,
    /// Gem shop slot 45 (`GemItemsPurchased[45]`), worth 2% AFK each.
    pub gem_shop_45: f64,
    /// W7 task board research grid points.
    pub task_grid_points: f64,
    /// Jelly rewards 4 and 57, flat grid points.
    pub jelly_grid_points: f64,
    /// Spelunking upgrade 7, which unlocks shape count with the level.
    pub spellunky_upg7: f64,
    /// A user supplement on the account multiplier; `1.0` is none.
    pub extra_multiplier: f64,
    /// A user supplement on the owned shape count.
    pub extra_shapes: u32,
    /// A user supplement on the owned magnifier count.
    pub extra_magnifiers: u32,
}

impl Default for SimContext {
    fn default() -> Self {
        Self {
            research_level: 1,
            server_var_res_xp: 1.01,
            all_bonus_multi: 1.0,
            extra_pct: 0.0,
            companion_52: 1.0,
            companion_28: 0.0,
            companion_153_level2: 0.0,
            sticker_fixed: 0.0,
            boony_count: 0.0,
            event_shop_37: 0.0,
            button0: 0.0,
            button_base_no_grid: 0.0,
            killroy5: 1.0,
            dream14: 0.0,
            cglunko11: 0.0,
            fountain_2_16: 0.0,
            companion_54_level2: 0.0,
            companion_153: false,
            royal_research: 1.0,
            sticker_multiplicative: false,
            arcade63: 0.0,
            clam_work9: 0.0,
            menagerie_b3: 0.0,
            research_equipment_multi: 1.0,
            jelly_research_multi: 1.0,
            jelly_insight_pct: 0.0,
            jelly_kalei_pct: 0.0,
            rog: Vec::new(),
            mastery_additive_pct: 0.0,
            mastery_exp_multi: 1.0,
            event_shop: Vec::new(),
            superbits: Vec::new(),
            emporium: Vec::new(),
            minehead: Vec::new(),
            bonus_grid_points: 0,
            bundle_ban_m: false,
            bundle_ban_k: false,
            sailing_art37: 0.0,
            sailing_art36: 0.0,
            card_w7b11: 0.0,
            gem_shop_45: 0.0,
            task_grid_points: 0.0,
            jelly_grid_points: 0.0,
            spellunky_upg7: 0.0,
            extra_multiplier: 1.0,
            extra_shapes: 0,
            extra_magnifiers: 0,
        }
    }
}

impl SimContext {
    /// The Ring of Glory bonus at `index`.
    pub fn rog(&self, index: usize) -> f64 {
        self.rog.get(index).copied().unwrap_or(0.0)
    }

    /// The event shop item at `index`.
    pub fn event_shop(&self, index: usize) -> f64 {
        self.event_shop.get(index).copied().unwrap_or(0) as f64
    }

    /// The minehead bonus at `index`.
    pub fn minehead(&self, index: usize) -> f64 {
        self.minehead.get(index).copied().unwrap_or(0.0)
    }

    /// The emporium bonus at `index`.
    pub fn emporium(&self, index: usize) -> f64 {
        self.emporium.get(index).copied().unwrap_or(0) as f64
    }

    /// The superbit at `index`.
    pub fn superbit(&self, index: usize) -> f64 {
        self.superbits.get(index).copied().unwrap_or(0) as f64
    }
}

/// Everything a simulation run reads.
#[derive(Clone, Debug, PartialEq)]
pub struct Simulation {
    pub research_level: u32,
    pub research_exp: f64,
    pub grid: GridState,
    pub observations: Observations,
    pub mags: Vec<Mag>,
    pub placements: Vec<Placement>,
    /// The save's own shape overlay. It can disagree with what `placements`
    /// reproduces, and the game's grid bonuses use it, so it wins until the
    /// shapes are edited.
    pub saved_overlay: Option<Vec<i32>>,
    /// Preset whose priority drives shape placement (`"_none"` maximises EXP).
    pub preset: String,
}

impl Default for Simulation {
    fn default() -> Self {
        Self {
            research_level: 1,
            research_exp: 0.0,
            grid: GridState::empty(),
            observations: Observations::default(),
            mags: Vec::new(),
            placements: Vec::new(),
            saved_overlay: None,
            preset: "_none".into(),
        }
    }
}

impl Simulation {
    /// Shape index covering each cell, `-1` where none does.
    pub fn overlay(&self) -> Vec<i32> {
        self.saved_overlay
            .clone()
            .unwrap_or_else(|| crate::shape::overlay(&self.placements))
    }

    /// The preset this run optimises for, falling back to Res EXP.
    pub fn preset(&self) -> &crate::data::Preset {
        let game = game();
        game.presets
            .iter()
            .find(|preset| preset.id == self.preset)
            .unwrap_or_else(|| {
                game.presets
                    .iter()
                    .find(|preset| preset.id == "_none")
                    .expect("the Res EXP preset ships with the game data")
            })
    }
}

/// What the run stops at.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Target {
    /// Reach this research level.
    Level(u32),
    /// Simulate this many hours.
    Hours(f64),
}

/// How hard the optimizers search.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Effort {
    /// Greedy placement only: instant, good enough while dragging.
    Quick,
    /// Greedy plus local search over placements and magnifiers.
    Balanced,
    /// Balanced plus a beam search over grid points.
    Thorough,
}

/// A full optimize request.
#[derive(Clone, Debug)]
pub struct OptimizeRequest {
    pub target: Target,
    pub effort: Effort,
    pub respec_grid: bool,
    /// Grid point budget for the respec; `None` uses what the level earns.
    pub grid_points: Option<u32>,
}

/// Progress reported back to the UI while a run is in flight.
#[derive(Clone, Debug)]
pub struct Progress {
    pub fraction: f32,
    pub message: String,
}

/// What a progress reporter asks the run to do next.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Control {
    /// Keep going.
    Continue,
    /// Stop the run and leave the previous result in place.
    Stop,
}

/// Outcome of one optimizer run.
#[derive(Clone, Debug)]
pub struct OptimizeResult {
    pub before: Simulation,
    pub after: Simulation,
    pub before_rate: f64,
    pub after_rate: f64,
    pub projection: crate::sim::Projection,
    pub steps: Vec<String>,
    pub elapsed_ms: u128,
}

impl OptimizeResult {
    /// Percentage gain in research EXP per hour.
    pub fn gain_pct(&self) -> f64 {
        if self.before_rate <= 0.0 {
            return 0.0;
        }
        (self.after_rate / self.before_rate - 1.0) * 100.0
    }
}

/// Grid coordinate label, e.g. `J6`, for a cell index.
pub fn coord(cell: usize) -> String {
    let game = game();
    let col = cell % game.grid.cols;
    let row = cell / game.grid.cols;
    format!("{}{}", (b'A' + col as u8) as char, game.grid.rows - row)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coordinates_match_the_grid_labels() {
        assert_eq!(coord(0), "A12");
        assert_eq!(coord(19), "T12");
        assert_eq!(coord(239), "T1");
        assert_eq!(coord(129), "J6");
    }

    #[test]
    fn seeded_grid_stays_within_node_caps() {
        let grid = GridState::seeded(2);
        for node in &game().nodes {
            assert!(grid.level(node.id) <= node.max_lv);
        }
        assert!(grid.spent() > 0);
    }

    #[test]
    fn overlay_marks_covered_cells() {
        let mut placements = Vec::new();
        'search: for x in (0..600).step_by(5) {
            for y in (0..400).step_by(5) {
                let placement = Placement {
                    shape: 0,
                    x: x as f64,
                    y: y as f64,
                    rot: 0.0,
                };
                if crate::shape::cells_at(&placement).contains(&129) {
                    placements.push(placement);
                    break 'search;
                }
            }
        }
        let simulation = Simulation {
            placements,
            ..Simulation::default()
        };
        let overlay = simulation.overlay();
        assert_eq!(overlay.len(), game().cells());
        assert!(overlay.contains(&0));
    }
}
