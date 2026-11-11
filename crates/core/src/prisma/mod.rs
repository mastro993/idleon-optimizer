//! The Prisma calculator: which bubbles can take Prisma, what each one does to
//! its bubble bonus, and the streamed, cancellable impact search.
//!
//! Ported from `prisma-calc.html` and `js/ui/prisma.js`, with the maths from
//! `js/stats/prisma.js`, `js/stats/systems/w2/alchemy.js` (`isBubblePrismad`,
//! `getPrismaBonusMult` and the bubble bonus) and `js/formulas.js`
//! (`formulaEval`).
//!
//! The plan and the run are complete; the downstream outcome families (damage,
//! stats, skills and account totals) are PR-02, so [`Coverage`] reports every
//! effect that still has no model rather than presenting a bubble bonus as a
//! downstream gain.

mod data;
mod plan;
mod run;

pub use plan::{Cauldron, Character, Plan, PlanError, Row, Status, plan};
pub use run::{Change, Coverage, CoverageEntry, Metric, Outcome, RowResult, run};
