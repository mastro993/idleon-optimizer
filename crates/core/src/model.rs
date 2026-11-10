//! Simulation state, its save-derived knobs, and the messages the optimizers take.

use crate::data::game;

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
#[derive(Clone, Debug, PartialEq)]
pub struct SimContext {
    /// Save constant scaling the research EXP curve.
    pub server_var_res_xp: f64,
    /// The all-bonus multiplier every grid square gets (`1.0` is none).
    pub all_bonus_multi: f64,
    /// Account multipliers outside the grid, as a percentage.
    pub extra_pct: f64,
    /// Account multipliers outside the grid, multiplicative.
    pub extra_multiplier: f64,
    /// Flat grid points from companions, artifacts and the like.
    pub bonus_grid_points: u32,
    /// Shapes owned beyond the research level's own grants.
    pub extra_shapes: u32,
    /// Magnifiers owned beyond the grid's own grants.
    pub extra_magnifiers: u32,
}

impl Default for SimContext {
    fn default() -> Self {
        Self {
            server_var_res_xp: 1.0,
            all_bonus_multi: 1.0,
            extra_pct: 0.0,
            extra_multiplier: 1.0,
            bonus_grid_points: 0,
            extra_shapes: 0,
            extra_magnifiers: 0,
        }
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
            preset: "_none".into(),
        }
    }
}

impl Simulation {
    /// Shape index covering each cell, `-1` where none does.
    pub fn overlay(&self) -> Vec<i32> {
        crate::shape::overlay(&self.placements)
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
