//! The simulator tab: a sandbox board, its run knobs and one seeded attempt.
//!
//! Ports the Simulator tab of `jelly-operator-calc.html`: the stage and Fever
//! selects, Stronkroid's activation second, the automatic revive policy, the
//! palette, remove mode, clear and reset, the hypothetical board sections and
//! the seeded `simulateJellyOperation` run. [`Simulator::sandbox`] is a copy of
//! the save's Jelly Operator slice, so the tab never edits the save.

mod result;
mod state;
mod view;

pub use state::{MAX_STAGE, RunInputs, Simulator};
pub use view::pane;

#[cfg(test)]
mod tests;
