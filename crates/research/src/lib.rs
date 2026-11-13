//! The research grid: its game data, the simulation state a save decodes into,
//! the EXP and insight maths (`js/stats/systems/w7/research-math.js`) and the
//! optimizers that spend grid points, place shapes and assign magnifiers.

pub mod breakdown;
pub mod dashboard;
pub mod data;
pub mod edit;
pub mod engine;
pub mod model;
pub mod optimize;
pub mod save;
pub mod shape;
pub mod sim;
pub mod tiers;
pub mod timeline;

pub use data::{GameData, GridNode, Observation, Preset, ShapeDef, game};
pub use model::{
    Effort, GridState, Lead, Mag, MagKind, OptimizeRequest, OptimizeResult, Placement, SimContext,
    Simulation, Strategy, Target,
};
pub use tiers::{Tier, Tiers};
