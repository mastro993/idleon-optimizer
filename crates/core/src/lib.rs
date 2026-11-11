//! Domain model, save decoding and section maths for Legends of Idleon.
//!
//! The app is a desktop port of the calculators in
//! <https://github.com/Corgan/idleon-research-optimizer>. Game constants are
//! extracted from that repo by `script/extract-research-data.mjs`,
//! `script/extract-statue-data.mjs`, `script/extract-farming-data.mjs`,
//! `script/extract-clamworks-data.mjs`, `script/extract-worship-td-data.mjs` and
//! `script/extract-stats-data.mjs`; the maths follow
//! `js/stats/systems/w7/research-math.js`,
//! `js/stats/systems/common/statue-leveling.js`,
//! `js/stats/systems/w6/farming-planner.js`,
//! `js/stats/systems/w7/clamworks.js` and
//! `js/stats/systems/w3/worship-td.js`.

pub mod breakdown;
pub mod clamworks;
pub mod combat;
pub mod coral;
pub mod dashboard;
pub mod data;
pub mod endgame;
pub mod farming;
pub mod fishing;
pub mod formula;
pub mod import;
pub mod jar;
pub mod killroy;
pub mod ladle;
pub mod model;
pub mod monument;
pub mod optimize;
pub mod pickle;
pub mod project;
pub mod sailing;
pub mod save;
pub mod shape;
pub mod sim;
pub mod stats;
pub mod statue;
pub mod worship;

pub use clamworks::Clamworks;
pub use data::{GameData, GridNode, Observation, Preset, ShapeDef, game};
pub use farming::{CropValue, Farm, Plot, PlotMetrics, Snapshot};
pub use fishing::{
    AccountInputs, Character, Evaluation, FishRow, FishingSave, Location, Pool, PoolOutcome,
};
pub use formula::{FormulaNode, ValueFormat};
pub use jar::Jar;
pub use model::{
    Effort, GridState, Mag, MagKind, OptimizeRequest, OptimizeResult, Placement, Progress,
    SimContext, Simulation, Target,
};
pub use sailing::{AssignmentMode, Boat, Captain, Fleet, Goal, RouteContext, RouteMetrics};
