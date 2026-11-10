//! Research-grid, statue and optimizer domain model for Legends of Idleon.
//!
//! The app is a desktop port of the calculators in
//! <https://github.com/Corgan/idleon-research-optimizer>. Game constants are
//! extracted from that repo by `script/extract-research-data.mjs` and
//! `script/extract-statue-data.mjs`; the maths follow the upstream systems,
//! starting with `js/stats/systems/w7/research-math.js` and
//! `js/stats/systems/common/statue-leveling.js`.

pub mod breakdown;
pub mod data;
pub mod endgame;
pub mod formula;
pub mod model;
pub mod optimize;
pub mod pickle;
pub mod save;
pub mod shape;
pub mod sim;
pub mod statue;

pub use data::{GameData, GridNode, Observation, Preset, ShapeDef, game};
pub use formula::{FormulaNode, ValueFormat};
pub use model::{
    Effort, GridState, Mag, MagKind, OptimizeRequest, OptimizeResult, Placement, Progress,
    SimContext, Simulation, Target,
};
