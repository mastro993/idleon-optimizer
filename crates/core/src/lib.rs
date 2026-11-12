//! Domain model, save decoding and section maths for Legends of Idleon.
//!
//! The app is a desktop port of the calculators in
//! <https://github.com/Corgan/idleon-research-optimizer>. Game constants are
//! extracted from that repo by `script/extract-research-data.mjs`,
//! `script/extract-statue-data.mjs`, `script/extract-farming-data.mjs`,
//! `script/extract-clamworks-data.mjs`, `script/extract-worship-td-data.mjs`,
//! `script/extract-stats-data.mjs`, `script/extract-cglunko-data.mjs` and
//! `script/extract-opal-data.mjs`; the maths follow
//! `js/stats/systems/w7/research-math.js`,
//! `js/stats/systems/common/statue-leveling.js`,
//! `js/stats/systems/w6/farming-planner.js`,
//! `js/stats/systems/w7/clamworks.js` (with the AFK claim from
//! `js/stats/systems/w7/clamworks-afk.js`),
//! `js/stats/systems/w3/worship-td.js`,
//! `js/stats/systems/w5/cglunko.js` and the inline script of `opal-calc.html`.

pub mod breakdown;
pub mod cglunko;
pub mod clamworks;
pub mod cog;
pub mod combat;
pub mod cooking_mastery;
pub mod coral;
pub mod dashboard;
pub mod data;
pub mod drop_rate;
pub mod endgame;
pub mod farming;
pub mod fishing;
pub mod formula;
pub mod fountain;
pub mod import;
pub mod jar;
pub mod jelly;
pub mod killroy;
pub mod ladle;
pub mod model;
pub mod monument;
pub mod opal;
pub mod optimize;
pub mod pickle;
pub mod prisma;
pub mod project;
pub mod royal_guardian;
pub mod sailing;
pub mod save;
pub mod shape;
pub mod sim;
pub mod sneaking;
pub mod spelunking;
pub mod stats;
pub mod statue;
pub mod tiers;
pub mod worship;

pub use cglunko::Cglunko;
pub use clamworks::Clamworks;
pub use cooking_mastery::{External, Mastery, Upgrade};
pub use data::{GameData, GridNode, Observation, Preset, ShapeDef, game};
pub use farming::{CropValue, Farm, Plot, PlotMetrics, Snapshot};
pub use fishing::{
    AccountInputs, Character, Evaluation, FishRow, FishingSave, Location, Pool, PoolOutcome,
};
pub use formula::{FormulaNode, ValueFormat};
pub use fountain::Fountain;
pub use jar::Jar;
pub use model::{
    Effort, GridState, Mag, MagKind, OptimizeRequest, OptimizeResult, Placement, Progress,
    SimContext, Simulation, Target,
};
pub use opal::Opal;
pub use royal_guardian::RoyalGuardian;
pub use sailing::{AssignmentMode, Boat, Captain, Fleet, Goal, RouteContext, RouteMetrics};
pub use tiers::{Tier, Tiers};
