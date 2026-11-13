//! Building blocks every calculator shares: renderer-agnostic formula trees,
//! the game's curve evaluator and the progress/stop protocol of long runs.

pub mod curve;
pub mod formula;
pub mod rng;
pub mod run;

pub use formula::{FormulaNode, ValueFormat};
pub use rng::Rng;
pub use run::{Control, Progress};
