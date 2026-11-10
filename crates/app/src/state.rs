//! Application state: the simulation being edited and the run in flight.

use crate::ui;
use ely_gpui_component::forms::TextInput;
use gpui::{AppContext as _, Context, Entity, FocusHandle, Task, Window};
use gpui_mcp::BridgeHandle;
use idleon_core::model::{
    Effort, Mag, MagKind, OptimizeRequest, OptimizeResult, Placement, SimContext, Simulation,
    Target,
};
use idleon_core::{game, optimize, save, shape, sim};

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

/// A shape drag in flight: where the pointer grabbed it, and where it was.
#[derive(Clone, Debug)]
pub struct ShapeDrag {
    pub shape: usize,
    pub start_pointer: (f32, f32),
    pub start_position: (f64, f64),
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
    /// The shape the pointer is dragging, and where the drag started.
    pub shape_drag: Option<ShapeDrag>,
    /// The shape whose row or polygon was last picked.
    pub selected_shape: Option<usize>,
    pub status: String,
    /// Whether the save-derived supplements card is expanded.
    pub supplements_open: bool,
    /// Whether the save import card is expanded.
    pub save_open: bool,
    /// The pasted save object.
    pub save_input: Entity<TextInput>,
    /// What the last import attempt said.
    pub save_message: String,
    /// Whether the last import succeeded, for the status tone.
    pub save_ok: bool,
    pub focus: FocusHandle,
    #[allow(dead_code)]
    bridge: BridgeHandle,
    run_task: Option<Task<()>>,
}

impl AppState {
    pub fn new(
        window: &mut Window,
        endpoint_dir: Option<std::path::PathBuf>,
        cx: &mut Context<Self>,
    ) -> Self {
        let bridge = ui::bridge::install(window, cx, endpoint_dir);
        let mut simulation = Simulation {
            research_level: 100,
            grid: idleon_core::GridState::seeded(1),
            ..Simulation::default()
        };
        simulation.observations.found.fill(true);
        // Two glasses on the richest observations this level can already use.
        let mut usable: Vec<usize> = (0..game().observations.len())
            .filter(|obs| {
                sim::observation_usable(*obs, simulation.research_level, &simulation.observations)
            })
            .collect();
        usable.sort_by(|a, b| sim::obs_base_exp(*b).total_cmp(&sim::obs_base_exp(*a)));
        simulation.mags = usable
            .into_iter()
            .take(2)
            .map(|slot| Mag::new(MagKind::Regular, Some(slot)))
            .collect();
        let save_input = cx.new(|cx| {
            TextInput::new(window, cx)
                .multi_line(3, 8)
                .placeholder("Paste the decoded game save (JSON)")
                .label("Game save JSON")
        });
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        Self {
            simulation,
            context: SimContext::default(),
            result: None,
            running: false,
            effort: Effort::Balanced,
            target: Target::Hours(24.0),
            tab: Tab::Grid,
            selected_cell: None,
            shape_drag: None,
            selected_shape: None,
            status: "Ready. Level squares, place shapes, then optimize.".into(),
            supplements_open: true,
            save_open: false,
            save_input,
            save_message: "Nothing imported yet.".into(),
            save_ok: true,
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

    /// The topmost shape covering a square, if any.
    pub fn shape_at(&self, cell: usize) -> Option<usize> {
        self.simulation
            .placements
            .iter()
            .enumerate()
            .rev()
            .find(|(_, placement)| shape::cells_at(placement).contains(&cell))
            .map(|(index, _)| index)
    }

    /// The shape being dragged, if a drag is in flight.
    pub fn dragging_shape(&self) -> Option<usize> {
        self.shape_drag.as_ref().map(|drag| drag.shape)
    }

    /// Start dragging a shape from a pointer position in window pixels.
    pub fn begin_shape_drag(
        &mut self,
        shape: usize,
        pointer: (f32, f32),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(placement) = self.simulation.placements.get(shape) else {
            return;
        };
        self.selected_shape = Some(shape);
        self.status = "Dragging a shape; release to drop it.".into();
        self.shape_drag = Some(ShapeDrag {
            shape,
            start_pointer: pointer,
            start_position: (placement.x, placement.y),
        });
        window.focus(&self.focus, cx);
        cx.notify();
    }

    /// Move the dragged shape to follow the pointer, on the placement lattice.
    pub fn drag_shape_to(&mut self, pointer: (f32, f32), cx: &mut Context<Self>) {
        let Some(drag) = self.shape_drag.clone() else {
            return;
        };
        // The canvas draws one data pixel at CELL_PX / 30 logical pixels.
        let scale = 30.0 / crate::ui::grid_view::CELL_PX as f64;
        let dx = f64::from(pointer.0 - drag.start_pointer.0) * scale;
        let dy = f64::from(pointer.1 - drag.start_pointer.1) * scale;
        if let Some(placement) = self.simulation.placements.get_mut(drag.shape) {
            placement.x = shape::snap(drag.start_position.0 + dx);
            placement.y = shape::snap(drag.start_position.1 + dy);
        }
        cx.notify();
    }

    /// Finish a drag, keeping the shape where it was dropped.
    pub fn end_shape_drag(&mut self, cx: &mut Context<Self>) {
        if self.shape_drag.take().is_some() {
            if let Some(shape) = self.selected_shape {
                let name = game()
                    .shapes
                    .get(
                        self.simulation
                            .placements
                            .get(shape)
                            .map(|p| p.shape)
                            .unwrap_or(0),
                    )
                    .map(|def| def.name.clone())
                    .unwrap_or_default();
                self.status = format!("Placed {name} by hand.");
            }
            cx.notify();
        }
    }

    /// Highlight a shape without moving it.
    pub fn select_shape(&mut self, shape: Option<usize>, cx: &mut Context<Self>) {
        self.selected_shape = shape;
        cx.notify();
    }

    /// Select a square and hand it the keyboard.
    pub fn select(&mut self, cell: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.selected_cell = Some(cell);
        window.focus(&self.focus, cx);
        cx.notify();
    }

    /// Move the selection one square, clamped to the grid; selects the first square when none is.
    pub fn move_selection(&mut self, dx: i32, dy: i32, cx: &mut Context<Self>) {
        self.selected_cell = Some(step_cell(self.selected_cell, dx, dy));
        cx.notify();
    }

    /// Raise or lower the selected square's level.
    pub fn bump_selected(&mut self, delta: i64, cx: &mut Context<Self>) {
        let Some(cell) = self.selected_cell else {
            return;
        };
        self.bump_level(cell, delta, cx);
    }

    pub fn deselect(&mut self, cx: &mut Context<Self>) {
        self.selected_cell = None;
        cx.notify();
    }

    pub fn set_level(&mut self, cell: usize, level: u32, cx: &mut Context<Self>) {
        let cap = game().node_at(cell).map(|node| node.max_lv).unwrap_or(0);
        self.simulation.grid.set(cell, level.min(cap));
        cx.notify();
    }

    pub fn bump_level(&mut self, cell: usize, delta: i64, cx: &mut Context<Self>) {
        let cap = game().node_at(cell).map(|node| node.max_lv).unwrap_or(1);
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
        self.status = format!(
            "Added {} on {}.",
            shape.name,
            idleon_core::model::coord(cell)
        );
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

    pub fn set_supplements_open(&mut self, open: bool, cx: &mut Context<Self>) {
        self.supplements_open = open;
        cx.notify();
    }

    pub fn set_save_open(&mut self, open: bool, cx: &mut Context<Self>) {
        self.save_open = open;
        cx.notify();
    }

    /// The save-derived supplements the account carries outside the grid.
    pub fn set_server_var_res_xp(&mut self, value: f64, cx: &mut Context<Self>) {
        self.context.server_var_res_xp = value.clamp(1.0, 10.0);
        self.context_changed(cx);
    }

    pub fn set_all_bonus_multi(&mut self, value: f64, cx: &mut Context<Self>) {
        self.context.all_bonus_multi = value.clamp(1.0, 100.0);
        self.context_changed(cx);
    }

    pub fn set_extra_pct(&mut self, value: f64, cx: &mut Context<Self>) {
        self.context.extra_pct = value.clamp(0.0, 10_000.0);
        self.context_changed(cx);
    }

    pub fn set_extra_multiplier(&mut self, value: f64, cx: &mut Context<Self>) {
        self.context.extra_multiplier = value.clamp(0.0, 100.0);
        self.context_changed(cx);
    }

    pub fn set_bonus_grid_points(&mut self, value: f64, cx: &mut Context<Self>) {
        self.context.bonus_grid_points = value.clamp(0.0, 100_000.0).round() as u32;
        self.context_changed(cx);
    }

    pub fn set_extra_shapes(&mut self, value: f64, cx: &mut Context<Self>) {
        self.context.extra_shapes = value.clamp(0.0, 100.0).round() as u32;
        self.context_changed(cx);
    }

    pub fn set_extra_magnifiers(&mut self, value: f64, cx: &mut Context<Self>) {
        self.context.extra_magnifiers = value.clamp(0.0, 200.0).round() as u32;
        self.context_changed(cx);
    }

    /// Put every supplement back to its default and drop the stale result.
    pub fn reset_supplements(&mut self, cx: &mut Context<Self>) {
        self.context = SimContext::default();
        self.status = "Supplements reset to their defaults.".into();
        self.context_changed(cx);
    }

    fn context_changed(&mut self, cx: &mut Context<Self>) {
        self.result = None;
        cx.notify();
    }

    /// Read the pasted save and, if it parses, replace the run with it.
    pub fn import_save(&mut self, cx: &mut Context<Self>) {
        let text = self.save_input.read(cx).text().to_string();
        match save::import(&text) {
            Ok(imported) => {
                self.simulation = imported.simulation;
                self.context = imported.context;
                self.result = None;
                self.selected_cell = None;
                self.save_ok = true;
                self.save_message = imported_summary(&imported.warnings);
                self.status = "Imported a save.".into();
            }
            Err(error) => {
                self.save_ok = false;
                self.save_message = error.to_string();
            }
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
                .spawn(async move { optimize::run(&simulation, &context, &request, &mut |_| {}) })
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

/// One arrow press from `current`, clamped to the grid's edges.
fn step_cell(current: Option<usize>, dx: i32, dy: i32) -> usize {
    let game = game();
    let cols = game.grid.cols as i32;
    let rows = game.grid.rows as i32;
    let Some(cell) = current else {
        return 0;
    };
    let col = (cell as i32 % cols + dx).clamp(0, cols - 1);
    let row = (cell as i32 / cols + dy).clamp(0, rows - 1);
    (row * cols + col) as usize
}

/// What an import left out, in one line for the panel.
fn imported_summary(warnings: &[String]) -> String {
    if warnings.is_empty() {
        return "Imported: the save had every field this app reads.".into();
    }
    format!(
        "Imported with {} warning(s): {}",
        warnings.len(),
        warnings.join("; ")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arrows_move_one_square_and_stop_at_the_edges() {
        assert_eq!(step_cell(Some(129), 1, 0), 130);
        assert_eq!(step_cell(Some(129), 0, -1), 109);
        assert_eq!(step_cell(Some(19), 1, 0), 19);
        assert_eq!(step_cell(Some(0), -1, -1), 0);
        assert_eq!(step_cell(None, 1, 0), 0);
    }

    #[test]
    fn import_summary_names_the_warnings() {
        assert!(imported_summary(&[]).contains("Imported"));
        let summary = imported_summary(&["crowns".into(), "sailing".into()]);
        assert!(summary.contains("2 warning"));
        assert!(summary.contains("crowns"));
    }
}
