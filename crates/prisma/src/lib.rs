//! The Prisma calculator: which bubbles can take Prisma, what each one does to
//! its bubble bonus, the streamed impact search and its coverage.
//!
//! A run measures the outcomes this build models — every purchased bubble's
//! bonus (an intermediate) plus the Statue EXP retention the STARTUE EXP bubble
//! buys — and ranks the candidates by any of them. [`Coverage`] names every
//! effect, the native systems traced reading it and whether a model reads it,
//! so a bonus is never presented as a stat, skill or damage gain this build
//! does not compute.

mod data;
mod plan;
mod run;

pub use data::{Consumer, Mode};
pub use plan::{Cauldron, Character, Plan, PlanError, Row, Status, plan};
pub use run::{Change, Comparison, Coverage, CoverageEntry, Metric, Outcome, RowResult, run};
