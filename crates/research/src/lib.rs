//! The research grid: its game data, the simulation state a save decodes into,
//! the EXP and insight maths (`js/stats/systems/w7/research-math.js`) and the
//! optimizers that spend grid points, place shapes and assign magnifiers.

pub mod breakdown;
pub mod dashboard;
pub mod data;
pub mod engine;
pub mod model;
pub mod optimize;
pub mod save;
pub mod shape;
pub mod sim;
pub mod tiers;

pub use data::{GameData, GridNode, Observation, Preset, ShapeDef, game};
pub use model::{
    Effort, GridState, Mag, MagKind, OptimizeRequest, OptimizeResult, Placement, SimContext,
    Simulation, Target,
};
pub use tiers::{Tier, Tiers};
