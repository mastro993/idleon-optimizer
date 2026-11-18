//! What the tabs show that costs a calculation, worked out when its inputs
//! change instead of on every frame.
//!
//! The save's own figures are read once per import ([`SaveFigures`]); the
//! board's are read again only when the board or the breakdown tree changes
//! ([`BoardFigures`]); the sandbox's unlocked slots only when its stage or
//! sections change ([`SandboxFigures`]); and a run's figures once, when the run
//! stores its result ([`Results`]).

use super::simulator::Simulator;
use idleon_jelly::board::{self, Layout};
use idleon_jelly::optimizer::Optimization;
use idleon_jelly::planner::Plan;
use idleon_jelly::{
    Bonus, Breakdown, BreakdownCategory, CellRow, Estimate, Insights, Jelly, LayoutBreakdown,
    MetricOptions, Metrics, Trial, UpgradeRow,
};
use std::collections::BTreeSet;
use std::rc::Rc;

/// The Bloodcell, Fever and slot figures the save alone decides.
#[derive(Clone, Debug, Default)]
pub(crate) struct SaveFigures {
    /// The saved board's metrics under the save's Fever.
    pub(crate) snapshot: Metrics,
    /// The grid slots the save has unlocked.
    pub(crate) unlocked: Vec<bool>,
    pub(crate) bonuses: Vec<Bonus>,
    pub(crate) upgrades: Vec<UpgradeRow>,
}

impl SaveFigures {
    pub(crate) fn of(jelly: &Jelly) -> Self {
        Self {
            snapshot: jelly.layout_metrics(&jelly.layout, options(jelly)),
            unlocked: board::unlocked_slots(jelly),
            bonuses: jelly.bonuses(),
            upgrades: jelly.upgrade_rows(),
        }
    }
}

/// The board on screen's metrics, breakdown tree and per-cell output, kept
/// with the board and tree they were read from.
#[derive(Clone, Debug, Default)]
pub(crate) struct BoardFigures {
    key: Option<(Layout, BreakdownCategory)>,
    pub(crate) metrics: Metrics,
    pub(crate) breakdown: Option<Breakdown>,
    pub(crate) layout: LayoutBreakdown,
    pub(crate) cells: Vec<CellRow>,
}

impl BoardFigures {
    /// Whether these figures describe `layout` with the `category` tree.
    pub(crate) fn describe(&self, layout: &Layout, category: BreakdownCategory) -> bool {
        self.key
            .as_ref()
            .is_some_and(|(known, tree)| known == layout && *tree == category)
    }

    pub(crate) fn of(jelly: &Jelly, layout: &Layout, category: BreakdownCategory) -> Self {
        let options = options(jelly);
        let breakdown = jelly.breakdown(category, layout, options);
        let figures = jelly.layout_breakdown(layout, options.fever);
        Self {
            key: Some((layout.clone(), category)),
            metrics: jelly.layout_metrics(layout, options),
            breakdown: Some(breakdown),
            cells: figures.cell_rows(),
            layout: figures,
        }
    }
}

/// The sandbox's unlocked slots, kept with the stage and sections they hold for.
#[derive(Clone, Debug, Default)]
pub(crate) struct SandboxFigures {
    key: Option<(usize, BTreeSet<usize>)>,
    pub(crate) unlocked: Vec<bool>,
    pub(crate) unlocked_count: usize,
}

impl SandboxFigures {
    pub(crate) fn describe(&self, simulator: &Simulator) -> bool {
        self.key.as_ref().is_some_and(|(stage, plots)| {
            *stage == simulator.obstruction && *plots == simulator.plots
        })
    }

    pub(crate) fn of(jelly: &Jelly, simulator: &Simulator) -> Self {
        let sandbox = simulator.sandbox(jelly);
        Self {
            key: Some((simulator.obstruction, simulator.plots.clone())),
            unlocked: board::unlocked_slots(&sandbox),
            unlocked_count: board::unlocked_count(&sandbox),
        }
    }
}

/// Each run's stored result, shared with render without a per-frame copy, and
/// what the tabs derive from it. `Controls` fills it when a run stores one.
#[derive(Clone, Debug, Default)]
pub(crate) struct Results {
    pub(crate) estimate: Option<Rc<Estimate>>,
    /// The estimate's outlook, read against the board it ran.
    pub(crate) insights: Option<Rc<Insights>>,
    pub(crate) trial: Option<Rc<Trial>>,
    pub(crate) plan: Option<Rc<Plan>>,
    /// The slots the plan's recommended sections unlock.
    pub(crate) plan_unlocked: Vec<bool>,
    pub(crate) optimization: Option<Rc<Optimization>>,
}

/// The metric options the tabs read: the save's own Fever.
fn options(jelly: &Jelly) -> MetricOptions {
    MetricOptions {
        fever: Some(jelly.progress.fever),
        ..MetricOptions::default()
    }
}
