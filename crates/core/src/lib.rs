//! Research-grid domain model and local optimizers for Legends of Idleon.
//!
//! The app is a desktop port of the Research Shape Optimizer in
//! <https://github.com/Corgan/idleon-research-optimizer>. Game constants are
//! extracted from that repo by `script/extract-research-data.mjs`; the maths
//! follow `js/stats/systems/w7/research-math.js`.

pub mod data;
pub mod model;
pub mod monument;
pub mod optimize;
pub mod save;
pub mod shape;
pub mod sim;

pub use data::{GameData, GridNode, Observation, Preset, ShapeDef, game};
pub use model::{
    Effort, GridState, Mag, MagKind, OptimizeRequest, OptimizeResult, Placement, Progress,
    SimContext, Simulation, Target,
};
