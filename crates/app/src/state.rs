//! Application state: the simulation being edited and the run in flight.

use crate::ui;
use gpui::{Context, FocusHandle, Task, Window};
use gpui_mcp::BridgeHandle;
use idleon_core::model::{
    Effort, Mag, MagKind, OptimizeRequest, OptimizeResult, Placement, SimContext, Simulation,
    Target,
};
use idleon_core::{game, optimize, shape, sim};

/// The view the window shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    /// Grid canvas, shape list and node inspector.
    Grid,
    /// Rate breakdown and the observation board.
    Rates,
    /// Target, effort and the last result.
    Run,
}

impl Tab {
    pub const ALL: [Tab; 3] = [Tab::Grid, Tab::Rates, Tab::Run];

    pub fn id(self) -> &'static str {
        match self {
            Tab::Grid => "grid",
            Tab::Rates => "rates",
            Tab::Run => "run",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Tab::Grid => "Grid",
            Tab::Rates => "Rates",
            Tab::Run => "Optimize",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|tab| tab.id() == id)
    }
}

/// Everything the window renders and mutates.
pub struct AppState {
    pub simulation: Simulation,
    pub context: SimContext,
    pub result: Option<OptimizeResult>,
    pub running: bool,
    pub effort: Effort,
    pub target: Target,
    pub tab: Tab,
    pub selected_cell: Option<usize>,
    pub status: String,
    pub focus: FocusHandle,
    #[allow(dead_code)]
    bridge: BridgeHandle,
    run_task: Option<Task<()>>,
}

impl AppState {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let bridge = ui::bridge::install(window, cx);
        let mut simulation = Simulation {
            research_level: 100,
            grid: idleon_core::GridState::seeded(1),
            ..Simulation::default()
        };
        simulation.observations.found.fill(true);
        // Two glasses on the richest observations this level can already use.
        let mut usable: Vec<usize> = (0..game().observations.len())
            .filter(|obs| sim::observation_usable(*obs, simulation.research_level, &simulation.observations))
            .collect();
        usable.sort_by(|a, b| sim::obs_base_exp(*b).total_cmp(&sim::obs_base_exp(*a)));
        simulation.mags = usable
            .into_iter()
            .take(2)
            .map(|slot| Mag::new(MagKind::Regular, Some(slot)))
            .collect();
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        let clear = focus.clone();
        let _ = clear;
        Self {
            simulation,
            context: SimContext::default(),
            result: None,
            running: false,
            effort: Effort::Balanced,
            target: Target::Level(100),
            tab: Tab::Grid,
            selected_cell: None,
            status: "Ready. Level squares, place shapes, then optimize.".into(),
            focus,
            bridge,
            run_task: None,
        }
    }

    /// Current research EXP per hour.
    pub fn rate(&self) -> sim::ExpRate {
        sim::exp_rate(&self.simulation, &self.context)
    }

    /// Grid points earned, spent and free.
    pub fn grid_points(&self) -> sim::GridPoints {
        sim::grid_points(&self.simulation, &self.context)
    }

    /// Shapes the current level grants.
    pub fn shapes_owned(&self) -> usize {
        sim::shapes_owned(self.simulation.research_level, &self.context)
    }

    /// Magnifiers the current level and grid grant.
    pub fn magnifiers_owned(&self) -> u32 {
        sim::magnifiers_owned(&self.simulation, &self.context)
    }

    /// The selected square's game data, if a square is selected.
    pub fn selected_node(&self) -> Option<&'static idleon_core::GridNode> {
        self.selected_cell.and_then(|cell| game().node_at(cell))
    }

    pub fn select(&mut self, cell: usize, cx: &mut Context<Self>) {
        self.selected_cell = Some(cell);
        cx.notify();
    }

    pub fn set_level(&mut self, cell: usize, level: u32, cx: &mut Context<Self>) {
        let cap = game()
            .node_at(cell)
            .map(|node| node.max_lv)
            .unwrap_or(0);
        self.simulation.grid.set(cell, level.min(cap));
        cx.notify();
    }

    pub fn bump_level(&mut self, cell: usize, delta: i64, cx: &mut Context<Self>) {
        let cap = game()
            .node_at(cell)
            .map(|node| node.max_lv)
            .unwrap_or(1);
        let next = (self.simulation.grid.level(cell) as i64 + delta).clamp(0, cap as i64);
        self.set_level(cell, next as u32, cx);
    }

    pub fn set_tab(&mut self, tab: Tab, cx: &mut Context<Self>) {
        self.tab = tab;
        cx.notify();
    }

    pub fn set_effort(&mut self, effort: Effort, cx: &mut Context<Self>) {
        self.effort = effort;
        cx.notify();
    }

    pub fn set_preset(&mut self, preset: String, cx: &mut Context<Self>) {
        if game().presets.iter().any(|entry| entry.id == preset) {
            self.simulation.preset = preset;
            cx.notify();
        }
    }

    pub fn set_target_level(&mut self, level: f64, cx: &mut Context<Self>) {
        self.target = Target::Level(level.max(1.0).round() as u32);
        cx.notify();
    }

    pub fn set_target_hours(&mut self, hours: f64, cx: &mut Context<Self>) {
        self.target = Target::Hours(hours.max(0.5));
        cx.notify();
    }

    pub fn set_target_kind(&mut self, kind: &str, cx: &mut Context<Self>) {
        self.target = match kind {
            "hours" => Target::Hours(match self.target {
                Target::Hours(hours) => hours,
                Target::Level(_) => 24.0,
            }),
            _ => Target::Level(match self.target {
                Target::Level(level) => level,
                Target::Hours(_) => 100,
            }),
        };
        cx.notify();
    }

    /// Add the next unlocked shape, picked in bonus order.
    pub fn add_shape(&mut self, cx: &mut Context<Self>) {
        let owned = self.shapes_owned();
        let placed = self.simulation.placements.len();
        if placed >= owned {
            self.status = format!("{owned} shapes unlocked; none free.");
            return cx.notify();
        }
        let taken: Vec<usize> = self.simulation.placements.iter().map(|p| p.shape).collect();
        let Some(shape) = game()
            .shapes
            .iter()
            .filter(|candidate| !taken.contains(&candidate.id))
            .max_by(|a, b| a.bonus_pct.total_cmp(&b.bonus_pct))
        else {
            return cx.notify();
        };
        // Centre the new shape on the most valuable square it can still touch.
        let overlay = self.simulation.overlay();
        let values = sim::cell_values(&self.simulation, &self.context);
        let cell = (0..overlay.len())
            .filter(|cell| overlay[*cell] < 0 && self.simulation.grid.level(*cell) > 0)
            .max_by(|a, b| values[*a].total_cmp(&values[*b]))
            .or(self.selected_cell)
            .or_else(|| self.simulation.grid.live_cells().next())
            .unwrap_or(0);
        let [x, y] = shape::cell_origin(cell);
        self.simulation.placements.push(Placement {
            shape: shape.id,
            x: shape::snap(x - shape.dims[0] / 2.0),
            y: shape::snap(y - shape.dims[1] / 2.0),
            rot: 0.0,
        });
        self.status = format!("Added {} on {}.", shape.name, idleon_core::model::coord(cell));
        cx.notify();
    }

    /// Turn a shape a step, keeping it on the lattice.
    pub fn rotate_shape(&mut self, index: usize, delta: f64, cx: &mut Context<Self>) {
        if let Some(placement) = self.simulation.placements.get_mut(index) {
            placement.rot = (placement.rot + delta).rem_euclid(360.0);
        }
        cx.notify();
    }

    /// Move a shape by whole steps of the placement lattice.
    pub fn nudge_shape(&mut self, index: usize, dx: f64, dy: f64, cx: &mut Context<Self>) {
        if let Some(placement) = self.simulation.placements.get_mut(index) {
            placement.x = shape::snap(placement.x + dx);
            placement.y = shape::snap(placement.y + dy);
        }
        cx.notify();
    }

    pub fn remove_shape(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.simulation.placements.len() {
            self.simulation.placements.remove(index);
        }
        cx.notify();
    }

    /// Add a magnifier of `kind` to the best free observation.
    pub fn add_mag(&mut self, kind: &str, cx: &mut Context<Self>) {
        let kind = match kind {
            "monocle" => MagKind::Monocle,
            "kaleidoscope" => MagKind::Kaleidoscope,
            _ => MagKind::Regular,
        };
        let cap = sim::mag_cap(self.simulation.research_level) as usize;
        let to_find = sim::observations_to_find(
            self.simulation.research_level,
            &self.simulation.observations.found,
        );
        let slot = (0..to_find).find(|obs| {
            self.simulation
                .mags
                .iter()
                .filter(|mag| mag.slot == Some(*obs))
                .count()
                < cap
        });
        match slot {
            Some(slot) => {
                self.simulation.mags.push(Mag::new(kind, Some(slot)));
                self.status = format!("Added a {} on #{}.", kind_label(kind), slot + 1);
            }
            None => self.status = "No observation has room for another magnifier.".into(),
        }
        cx.notify();
    }

    pub fn clear_mags(&mut self, cx: &mut Context<Self>) {
        self.simulation.mags.clear();
        self.status = "Cleared the magnifier pool.".into();
        cx.notify();
    }

    pub fn toggle_found(&mut self, obs: usize, cx: &mut Context<Self>) {
        if let Some(found) = self.simulation.observations.found.get_mut(obs) {
            *found = !*found;
        }
        cx.notify();
    }

    pub fn bump_insight(&mut self, obs: usize, delta: i64, cx: &mut Context<Self>) {
        if let Some(level) = self.simulation.observations.insight_level.get_mut(obs) {
            *level = (*level as i64 + delta).clamp(0, 400) as u32;
        }
        cx.notify();
    }

    pub fn reset_layout(&mut self, cx: &mut Context<Self>) {
        self.simulation.placements.clear();
        self.simulation.mags.clear();
        self.result = None;
        self.status = "Cleared shapes, magnifiers and the last result.".into();
        cx.notify();
    }

    pub fn apply_result(&mut self, cx: &mut Context<Self>) {
        if let Some(result) = self.result.take() {
            let gain = result.gain_pct();
            self.simulation = result.after;
            self.status = format!("Applied the optimized layout ({gain:+.1}%).");
        }
        cx.notify();
    }

    pub fn dismiss_result(&mut self, cx: &mut Context<Self>) {
        self.result = None;
        cx.notify();
    }

    /// Run the optimizers off the UI thread and store the result.
    pub fn optimize(&mut self, cx: &mut Context<Self>) {
        if self.running {
            return;
        }
        self.running = true;
        self.status = "Optimizing…".into();
        let request = OptimizeRequest {
            target: self.target,
            effort: self.effort,
            respec_grid: matches!(self.effort, Effort::Thorough),
            grid_points: None,
        };
        let simulation = self.simulation.clone();
        let context = self.context.clone();
        cx.notify();

        self.run_task = Some(cx.spawn(async move |this, cx| {
            let outcome = cx
                .background_executor()
                .spawn(async move {
                    optimize::run(&simulation, &context, &request, &mut |_| {})
                })
                .await;
            this.update(cx, |state, cx| {
                state.running = false;
                state.status = format!(
                    "{} steps in {} ms.",
                    outcome.steps.len(),
                    outcome.elapsed_ms
                );
                state.result = Some(outcome);
                cx.notify();
            })
            .ok();
        }));
    }
}

fn kind_label(kind: MagKind) -> &'static str {
    match kind {
        MagKind::Regular => "magnifying glass",
        MagKind::Monocle => "monocle",
        MagKind::Kaleidoscope => "kaleidoscope",
    }
}
