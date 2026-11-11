//! Domain model, save decoding and section maths for Legends of Idleon.
//!
//! The app is a desktop port of the calculators in
//! <https://github.com/Corgan/idleon-research-optimizer>. Game constants are
//! extracted from that repo by `script/extract-research-data.mjs`; the research
//! maths follow `js/stats/systems/w7/research-math.js` and the Clamworks maths
//! follow `js/stats/systems/w7/clamworks.js`.

pub mod breakdown;
pub mod clamworks;
pub mod coral;
pub mod data;
pub mod endgame;
pub mod formula;
pub mod model;
pub mod monument;
pub mod optimize;
pub mod pickle;
pub mod save;
pub mod shape;
pub mod sim;

pub use clamworks::Clamworks;
pub use data::{GameData, GridNode, Observation, Preset, ShapeDef, game};
pub use formula::{FormulaNode, ValueFormat};
pub use model::{
    Effort, GridState, Mag, MagKind, OptimizeRequest, OptimizeResult, Placement, Progress,
    SimContext, Simulation, Target,
};
