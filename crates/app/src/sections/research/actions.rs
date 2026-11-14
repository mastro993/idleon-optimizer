//! What the window does when the research section is edited or run.

use crate::sections::research::{ShapeDrag, ShapeRotate, Tab};
use crate::state::{AppState, RunInputs, ShapeRunInputs};
use gpui::{App, Context, Window};
use idleon_common::Control;
use idleon_research::edit::Snapshot;
use idleon_research::model::{
    Effort, MagKind, OptimizeRequest, OptimizeResult, Placement, SimContext, SimOptions,
    Simulation, Target,
};
use idleon_research::{game, optimize, shape, sim};
use idleon_ui::runner::TaskState;

impl AppState {
    /// The optimizer run: running, progress, result and staleness.
    pub(crate) fn optimize_state(&self, cx: &App) -> TaskState<OptimizeResult> {
        self.optimizer.read(cx).state(&self.run_inputs())
    }

    /// Whether the research optimizer has a run in flight.
    pub(crate) fn optimize_running(&self, cx: &App) -> bool {
        self.optimizer.read(cx).running()
    }

    /// The inputs a run started now would use.
    pub(crate) fn run_inputs(&self) -> RunInputs {
        RunInputs {
            simulation: self.simulation.clone(),
            context: self.context.clone(),
            target: self.target,
            effort: self.effort,
            options: self.options,
        }
    }

    /// Current research EXP per hour.
    pub(crate) fn rate(&self) -> sim::ExpRate {
        sim::exp_rate(&self.simulation, &self.context)
    }

    /// Grid points earned, spent and free.
    pub(crate) fn grid_points(&self) -> sim::GridPoints {
        sim::grid_points(&self.simulation, &self.context)
    }

    /// Shapes the current level grants.
    pub(crate) fn shapes_owned(&self) -> usize {
        sim::shapes_owned(self.simulation.research_level, &self.context)
    }

    /// Magnifiers the current level and grid grant.
    pub(crate) fn magnifiers_owned(&self) -> u32 {
        sim::magnifiers_owned(&self.simulation, &self.context)
    }

    /// The selected square's game data, if a square is selected.
    pub(crate) fn selected_node(&self) -> Option<&'static idleon_research::GridNode> {
        self.research
            .selected_cell
            .and_then(|cell| game().node_at(cell))
    }

    /// Record the editable state an edit replaced, so it can be undone.
    fn record(&mut self, before: Snapshot) {
        if Snapshot::of(&self.simulation) != before {
            self.research.history.record(before);
        }
    }

    /// Run one manual edit, keeping the state it replaced for undo.
    fn edit(&mut self, change: impl FnOnce(&mut Simulation), cx: &mut Context<Self>) {
        let before = Snapshot::of(&self.simulation);
        change(&mut self.simulation);
        self.record(before);
        cx.notify();
    }

    /// Whether an edit can be taken back.
    pub(crate) fn can_undo(&self) -> bool {
        self.research.history.can_undo()
    }

    /// Whether an undone edit can be redone.
    pub(crate) fn can_redo(&self) -> bool {
        self.research.history.can_redo()
    }

    /// Put the layout back the way it was before the last edit.
    pub(crate) fn undo(&mut self, cx: &mut Context<Self>) {
        let current = Snapshot::of(&self.simulation);
        let Some(previous) = self.research.history.undo(current) else {
            return self.set_status("Nothing left to undo.", cx);
        };
        previous.apply(&mut self.simulation);
        self.set_status("Undid the last edit.", cx);
    }

    /// Re-apply the edit the last undo took back.
    pub(crate) fn redo(&mut self, cx: &mut Context<Self>) {
        let current = Snapshot::of(&self.simulation);
        let Some(next) = self.research.history.redo(current) else {
            return self.set_status("Nothing left to redo.", cx);
        };
        next.apply(&mut self.simulation);
        self.set_status("Redid the last edit.", cx);
    }

    /// The topmost shape covering a square, if any.
    pub(crate) fn shape_at(&self, cell: usize) -> Option<usize> {
        self.simulation
            .placements
            .iter()
            .enumerate()
            .rev()
            .find(|(_, placement)| shape::cells_at(placement).contains(&cell))
            .map(|(index, _)| index)
    }

    /// The shape being dragged, if a drag is in flight.
    pub(crate) fn dragging_shape(&self) -> Option<usize> {
        self.research.shape_drag.as_ref().map(|drag| drag.shape)
    }

    /// The shape the rotation handle is turning, if a drag is in flight.
    pub(crate) fn rotating_shape(&self) -> Option<usize> {
        self.research
            .shape_rotate
            .as_ref()
            .map(|rotate| rotate.shape)
    }

    /// Turn the picked shape by one step, the keyboard copy of the handle.
    pub(crate) fn turn_selected(&mut self, delta: f64, cx: &mut Context<Self>) {
        let Some(index) = self.research.selected_shape else {
            return self.set_status("Pick a shape on the canvas to turn it.", cx);
        };
        self.rotate_shape(index, delta, cx);
    }

    /// Start dragging a shape from a pointer position in window pixels.
    pub(crate) fn begin_shape_drag(
        &mut self,
        shape: usize,
        pointer: (f32, f32),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(placement) = self.simulation.placements.get(shape) else {
            return;
        };
        self.research.selected_shape = Some(shape);
        self.status = "Dragging a shape; release to drop it.".into();
        self.research.shape_drag = Some(ShapeDrag {
            shape,
            start_pointer: pointer,
            start_position: (placement.x, placement.y),
            before: Snapshot::of(&self.simulation),
        });
        window.focus(&self.focus, cx);
        cx.notify();
    }

    /// Move the dragged shape to follow the pointer, on the placement lattice.
    pub(crate) fn drag_shape_to(&mut self, pointer: (f32, f32), cx: &mut Context<Self>) {
        let Some(drag) = self.research.shape_drag.clone() else {
            return;
        };
        self.simulation.saved_overlay = None;
        // The canvas draws one data pixel at CELL_PX / 30 logical pixels.
        let scale = 30.0 / crate::sections::research::grid::CELL_PX as f64;
        let dx = f64::from(pointer.0 - drag.start_pointer.0) * scale;
        let dy = f64::from(pointer.1 - drag.start_pointer.1) * scale;
        if let Some(placement) = self.simulation.placements.get_mut(drag.shape) {
            placement.x = shape::snap(drag.start_position.0 + dx);
            placement.y = shape::snap(drag.start_position.1 + dy);
        }
        cx.notify();
    }

    /// Finish a drag, keeping the shape where it was dropped.
    pub(crate) fn end_shape_drag(&mut self, cx: &mut Context<Self>) {
        if let Some(drag) = self.research.shape_drag.take() {
            self.record(drag.before);
            let name = game()
                .shapes
                .get(
                    self.simulation
                        .placements
                        .get(drag.shape)
                        .map(|p| p.shape)
                        .unwrap_or(0),
                )
                .map(|def| def.name.clone())
                .unwrap_or_default();
            self.status = format!("Placed {name} by hand.");
            cx.notify();
        }
    }

    /// Grab the selected shape's rotation handle at `pointer`, around a centre.
    pub(crate) fn begin_shape_rotate(
        &mut self,
        shape: usize,
        pointer: (f32, f32),
        handle_offset: (f32, f32),
        cx: &mut Context<Self>,
    ) {
        let Some(placement) = self.simulation.placements.get(shape) else {
            return;
        };
        // The handle sits at a known offset from the rotation centre, and the
        // pointer grabbed it, so the centre follows from the grab position.
        let centre = (pointer.0 - handle_offset.0, pointer.1 - handle_offset.1);
        self.research.selected_shape = Some(shape);
        self.research.shape_rotate = Some(ShapeRotate {
            shape,
            centre,
            start_angle: pointer_angle(centre, pointer),
            start_rot: placement.rot,
            before: Snapshot::of(&self.simulation),
        });
        cx.notify();
    }

    /// Turn the grabbed shape to follow the pointer.
    pub(crate) fn rotate_shape_to(&mut self, pointer: (f32, f32), cx: &mut Context<Self>) {
        let Some(rotate) = self.research.shape_rotate.clone() else {
            return;
        };
        let turn = pointer_angle(rotate.centre, pointer) - rotate.start_angle;
        self.simulation.saved_overlay = None;
        if let Some(placement) = self.simulation.placements.get_mut(rotate.shape) {
            shape::set_rotation(placement, rotate.start_rot + f64::from(turn.to_degrees()));
        }
        cx.notify();
    }

    /// Release the rotation handle; a grab that turned nothing is a click, and
    /// clicks turn the shape one step to the right.
    pub(crate) fn end_shape_rotate(&mut self, cx: &mut Context<Self>) {
        let Some(rotate) = self.research.shape_rotate.take() else {
            return;
        };
        if Snapshot::of(&self.simulation) == rotate.before {
            self.simulation.saved_overlay = None;
            if let Some(placement) = self.simulation.placements.get_mut(rotate.shape) {
                shape::set_rotation(placement, placement.rot + shape::ROT_STEP);
            }
        }
        self.record(rotate.before);
        let turned = self
            .simulation
            .placements
            .get(rotate.shape)
            .map(|placement| placement.rot)
            .unwrap_or(0.0);
        self.status = format!("Turned the shape to {turned:.0}°.");
        cx.notify();
    }

    /// Highlight a shape without moving it.
    pub(crate) fn select_shape(&mut self, shape: Option<usize>, cx: &mut Context<Self>) {
        self.research.selected_shape = shape;
        cx.notify();
    }

    /// Select a square and hand it the keyboard.
    pub(crate) fn select(&mut self, cell: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.research.selected_cell = Some(cell);
        window.focus(&self.focus, cx);
        cx.notify();
    }

    /// Move the selection one square, clamped to the grid; selects the first square when none is.
    pub(crate) fn move_selection(&mut self, dx: i32, dy: i32, cx: &mut Context<Self>) {
        self.research.selected_cell = Some(step_cell(self.research.selected_cell, dx, dy));
        cx.notify();
    }

    /// Raise or lower the selected square's level.
    pub(crate) fn bump_selected(&mut self, delta: i64, cx: &mut Context<Self>) {
        let Some(cell) = self.research.selected_cell else {
            return;
        };
        self.bump_level(cell, delta, cx);
    }

    pub(crate) fn deselect(&mut self, cx: &mut Context<Self>) {
        self.research.selected_cell = None;
        cx.notify();
    }

    pub(crate) fn set_level(&mut self, cell: usize, level: u32, cx: &mut Context<Self>) {
        let cap = game().node_at(cell).map(|node| node.max_lv).unwrap_or(0);
        self.edit(|sim| sim.grid.set(cell, level.min(cap)), cx);
    }

    pub(crate) fn bump_level(&mut self, cell: usize, delta: i64, cx: &mut Context<Self>) {
        let cap = game().node_at(cell).map(|node| node.max_lv).unwrap_or(1);
        let next = (self.simulation.grid.level(cell) as i64 + delta).clamp(0, cap as i64);
        self.set_level(cell, next as u32, cx);
    }

    pub(crate) fn set_tab(&mut self, tab: Tab, cx: &mut Context<Self>) {
        self.research.tab = tab;
        cx.notify();
    }

    /// Open the Grid editor on a square the dashboard preview picked.
    pub(crate) fn open_grid_cell(
        &mut self,
        cell: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.research.tab = Tab::Grid;
        self.research.selected_cell = Some(cell);
        window.focus(&self.focus, cx);
        cx.notify();
    }

    /// Open the Rates editor on an observation the dashboard preview picked.
    pub(crate) fn open_observation(&mut self, obs: usize, cx: &mut Context<Self>) {
        self.research.tab = Tab::Rates;
        self.select_observation(obs, cx);
    }

    /// Pick an observation for the magnifier assignment.
    pub(crate) fn select_observation(&mut self, obs: usize, cx: &mut Context<Self>) {
        self.research.selected_observation = Some(obs);
        cx.notify();
    }

    pub(crate) fn set_effort(&mut self, effort: Effort, cx: &mut Context<Self>) {
        self.effort = effort;
        cx.notify();
    }

    /// Replace the run toggles; a changed option marks the last result stale.
    pub(crate) fn set_options(&mut self, options: SimOptions, cx: &mut Context<Self>) {
        self.options = options;
        cx.notify();
    }

    /// Why the target on screen cannot run, for the Optimize button and its note.
    pub(crate) fn target_problem(&self) -> Option<String> {
        self.target.validate(self.simulation.research_level).err()
    }

    /// Bring a level target above the current level after a save or a config
    /// changed the research level.
    pub(crate) fn normalize_target(&mut self) {
        self.target = self.target.normalized(self.simulation.research_level);
    }

    /// Pick the built-in shape preset the optimizer's preset strip selected.
    pub(crate) fn set_preset(&mut self, id: &str, cx: &mut Context<Self>) {
        self.copy_shape_preset(id, cx);
    }

    /// Apply a built-in shape preset and name it in the status line.
    pub(crate) fn apply_shape_preset(&mut self, id: &str, cx: &mut Context<Self>) {
        if let Some(name) = self.copy_shape_preset(id, cx) {
            self.set_status(format!("Shape priorities: {name}."), cx);
        }
    }

    /// Copy a built-in preset's tiers into the run: a fresh copy, never a link
    /// back to the preset data, so a later edit detaches this run. Returns the
    /// preset's name, or `None` when it is unknown or unreadable.
    fn copy_shape_preset(&mut self, id: &str, cx: &mut Context<Self>) -> Option<&'static str> {
        let preset = game().presets.iter().find(|preset| preset.id == id)?;
        let Some(tiers) = super::preset_tiers(self, preset) else {
            self.set_status(format!("{} has no readable tier list.", preset.name), cx);
            return None;
        };
        self.simulation.shape_tiers = tiers;
        self.simulation.preset = preset.id.clone();
        self.research.active_shape_preset = Some(preset.id.clone());
        cx.notify();
        Some(&preset.name)
    }

    /// Apply a saved custom preset by name.
    pub(crate) fn apply_custom_shape_preset(&mut self, name: &str, cx: &mut Context<Self>) {
        let Some(preset) =
            crate::sections::research::CustomPreset::find(&self.research.shape_presets, name)
        else {
            return;
        };
        self.simulation.shape_tiers = preset.tiers.clone();
        self.simulation.preset = "_none".into();
        self.research.active_shape_preset = Some(preset.name.clone());
        self.set_status(format!("Shape priorities: {}.", preset.name), cx);
    }

    /// Save the current tiers under the name in the preset field.
    pub(crate) fn save_shape_preset(&mut self, cx: &mut Context<Self>) {
        let name = self.research.preset_name.read(cx).text().trim().to_string();
        match crate::sections::research::CustomPreset::store(
            &mut self.research.shape_presets,
            &name,
            &self.simulation.shape_tiers,
        ) {
            Ok(message) => {
                self.research
                    .preset_name
                    .update(cx, |input, cx| input.set_text("", cx));
                self.research.active_shape_preset = Some(name);
                self.status = message;
            }
            Err(message) => self.status = message,
        }
        cx.notify();
    }

    /// Forget the custom preset at `index`.
    pub(crate) fn delete_shape_preset(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(preset) = crate::sections::research::CustomPreset::delete(
            &mut self.research.shape_presets,
            index,
        ) else {
            return;
        };
        if self.research.active_shape_preset.as_deref() == Some(preset.name.as_str()) {
            self.research.active_shape_preset = None;
        }
        self.set_status(format!("Deleted the shape preset “{}”.", preset.name), cx);
    }

    /// Move a node between the tier zones. Editing detaches the active preset:
    /// the built-in or saved list it came from is never written.
    pub(crate) fn move_tier_node(
        &mut self,
        cell: usize,
        tier: idleon_research::Tier,
        before: Option<usize>,
        cx: &mut Context<Self>,
    ) {
        if !self.simulation.shape_tiers.move_node(cell, tier, before) {
            return;
        }
        self.research.active_shape_preset = None;
        self.simulation.preset = "_none".into();
        let zone = match tier {
            idleon_research::Tier::Above => "above",
            idleon_research::Tier::Exp => "Res EXP",
            idleon_research::Tier::Below => "below",
        };
        self.set_status(
            format!(
                "Moved {} to the {zone} tier; the preset is detached.",
                idleon_research::model::coord(cell)
            ),
            cx,
        );
    }

    pub(crate) fn set_target_level(&mut self, level: f64, cx: &mut Context<Self>) {
        let floor = f64::from(self.simulation.research_level.saturating_add(1));
        self.target = Target::Level(level.max(floor).round() as u32);
        cx.notify();
    }

    pub(crate) fn set_target_hours(&mut self, hours: f64, cx: &mut Context<Self>) {
        if hours.is_finite() {
            self.target = Target::Hours(hours.max(1.0));
            cx.notify();
        }
    }

    pub(crate) fn set_target_kind(&mut self, kind: &str, cx: &mut Context<Self>) {
        self.target = match kind {
            "hours" => Target::Hours(match self.target {
                Target::Hours(hours) => hours,
                Target::Level(_) => 24.0,
            }),
            _ => Target::Level(match self.target {
                Target::Level(level) => level,
                Target::Hours(_) => self.simulation.research_level.saturating_add(1),
            }),
        };
        self.normalize_target();
        cx.notify();
    }

    /// Add the next unlocked shape, picked in bonus order.
    pub(crate) fn add_shape(&mut self, cx: &mut Context<Self>) {
        let owned = self.shapes_owned();
        let placed = self.simulation.placements.len();
        if placed >= owned {
            return self.set_status(format!("{owned} shapes unlocked; none free."), cx);
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
            .or(self.research.selected_cell)
            .or_else(|| self.simulation.grid.live_cells().next())
            .unwrap_or(0);
        let [x, y] = shape::cell_origin(cell);
        self.edit(
            |sim| {
                sim.saved_overlay = None;
                sim.placements.push(Placement {
                    shape: shape.id,
                    x: shape::snap(x - shape.dims[0] / 2.0),
                    y: shape::snap(y - shape.dims[1] / 2.0),
                    rot: 0.0,
                });
            },
            cx,
        );
        self.research.selected_shape = Some(self.simulation.placements.len() - 1);
        self.set_status(
            format!(
                "Added {} on {}.",
                shape.name,
                idleon_research::model::coord(cell)
            ),
            cx,
        );
    }

    /// Turn a shape a step, keeping it on the lattice.
    pub(crate) fn rotate_shape(&mut self, index: usize, delta: f64, cx: &mut Context<Self>) {
        self.edit(
            |sim| {
                sim.saved_overlay = None;
                if let Some(placement) = sim.placements.get_mut(index) {
                    let turned = placement.rot + delta;
                    shape::set_rotation(placement, turned);
                }
            },
            cx,
        );
    }

    /// Move a shape by whole steps of the placement lattice.
    pub(crate) fn nudge_shape(&mut self, index: usize, dx: f64, dy: f64, cx: &mut Context<Self>) {
        self.edit(
            |sim| {
                sim.saved_overlay = None;
                if let Some(placement) = sim.placements.get_mut(index) {
                    placement.x = shape::snap(placement.x + dx);
                    placement.y = shape::snap(placement.y + dy);
                }
            },
            cx,
        );
    }

    pub(crate) fn remove_shape(&mut self, index: usize, cx: &mut Context<Self>) {
        self.edit(
            |sim| {
                sim.saved_overlay = None;
                if index < sim.placements.len() {
                    sim.placements.remove(index);
                }
            },
            cx,
        );
    }

    /// Put a free magnifier of `kind` on `obs`, if the pool and the slot allow it.
    pub(crate) fn assign_mag(&mut self, kind: MagKind, obs: usize, cx: &mut Context<Self>) {
        let before = Snapshot::of(&self.simulation);
        if !sim::place_mag(&mut self.simulation, &self.context, kind, obs) {
            return self.set_status(
                format!(
                    "No free {} for {}.",
                    kind_label(kind),
                    observation_name(obs)
                ),
                cx,
            );
        }
        self.record(before);
        self.set_status(
            format!("Put a {} on {}.", kind_label(kind), observation_name(obs)),
            cx,
        );
    }

    /// Take the newest magnifier off `obs` and back into the pool.
    pub(crate) fn lift_mag(&mut self, obs: usize, cx: &mut Context<Self>) {
        let before = Snapshot::of(&self.simulation);
        if !sim::lift_mag(&mut self.simulation, obs) {
            return self.set_status(format!("{} holds no magnifier.", observation_name(obs)), cx);
        }
        self.record(before);
        self.set_status(
            format!("Took a magnifier off {}.", observation_name(obs)),
            cx,
        );
    }

    pub(crate) fn clear_mags(&mut self, cx: &mut Context<Self>) {
        self.edit(|sim| sim.mags.clear(), cx);
    }

    pub(crate) fn toggle_found(&mut self, obs: usize, cx: &mut Context<Self>) {
        self.edit(
            |sim| {
                if let Some(found) = sim.observations.found.get_mut(obs) {
                    *found = !*found;
                }
            },
            cx,
        );
    }

    pub(crate) fn bump_insight(&mut self, obs: usize, delta: i64, cx: &mut Context<Self>) {
        self.edit(
            |sim| {
                if let Some(level) = sim.observations.insight_level.get_mut(obs) {
                    *level = (*level as i64 + delta).clamp(0, 400) as u32;
                }
            },
            cx,
        );
    }

    pub(crate) fn set_supplements_open(&mut self, open: bool, cx: &mut Context<Self>) {
        self.supplements_open = open;
        cx.notify();
    }

    /// The save-derived supplements the account carries outside the grid.
    pub(crate) fn set_server_var_res_xp(&mut self, value: f64, cx: &mut Context<Self>) {
        self.context.server_var_res_xp = value.clamp(1.0, 10.0);
        self.context_changed(cx);
    }

    pub(crate) fn set_all_bonus_multi(&mut self, value: f64, cx: &mut Context<Self>) {
        self.context.all_bonus_multi = value.clamp(1.0, 100.0);
        self.context_changed(cx);
    }

    pub(crate) fn set_extra_pct(&mut self, value: f64, cx: &mut Context<Self>) {
        self.context.extra_pct = value.clamp(0.0, 10_000.0);
        self.context_changed(cx);
    }

    pub(crate) fn set_extra_multiplier(&mut self, value: f64, cx: &mut Context<Self>) {
        self.context.extra_multiplier = value.clamp(0.0, 100.0);
        self.context_changed(cx);
    }

    pub(crate) fn set_bonus_grid_points(&mut self, value: f64, cx: &mut Context<Self>) {
        self.context.bonus_grid_points = value.clamp(0.0, 100_000.0).round() as u32;
        self.context_changed(cx);
    }

    pub(crate) fn set_extra_shapes(&mut self, value: f64, cx: &mut Context<Self>) {
        self.context.extra_shapes = value.clamp(0.0, 100.0).round() as u32;
        self.context_changed(cx);
    }

    pub(crate) fn set_extra_magnifiers(&mut self, value: f64, cx: &mut Context<Self>) {
        self.context.extra_magnifiers = value.clamp(0.0, 200.0).round() as u32;
        self.context_changed(cx);
    }

    /// Put every supplement back to what the last save or document set; the
    /// stored result turns stale.
    pub(crate) fn reset_supplements(&mut self, cx: &mut Context<Self>) {
        self.context = self.context_base.clone();
        self.status = "Supplements reset to their save-derived values.".into();
        self.context_changed(cx);
    }

    /// Adopt a document's context as both the live and the save-derived one.
    pub(crate) fn replace_context(&mut self, context: SimContext) {
        self.context = context.clone();
        self.context_base = context;
    }

    pub(crate) fn context_changed(&mut self, cx: &mut Context<Self>) {
        // The stored result stays visible and the runner marks it stale: a change
        // to the inputs must not silently throw away what the last run found.
        cx.notify();
    }

    pub(crate) fn reset_layout(&mut self, cx: &mut Context<Self>) {
        self.edit(
            |sim| {
                sim.placements.clear();
                sim.mags.clear();
            },
            cx,
        );
        self.set_status("Cleared shapes and magnifiers.", cx);
    }

    pub(crate) fn apply_result(&mut self, cx: &mut Context<Self>) {
        if let Some(result) = self.optimizer.update(cx, |runner, _| runner.take_result()) {
            let gain = result.gain_pct();
            self.edit(move |sim| *sim = result.after, cx);
            self.status = format!("Applied the optimized layout ({gain:+.1}%).");
        }
        self.research.run_phase = None;
        cx.notify();
    }

    pub(crate) fn dismiss_result(&mut self, cx: &mut Context<Self>) {
        self.optimizer.update(cx, |runner, _| runner.clear());
        self.research.run_phase = None;
        cx.notify();
    }

    /// The action the result's timeline marks; opening it again closes it.
    pub(crate) fn toggle_run_phase(&mut self, index: usize, cx: &mut Context<Self>) {
        let phase = &mut self.research.run_phase;
        *phase = (*phase != Some(index)).then_some(index);
        cx.notify();
    }

    /// Stop the run in flight at its next progress report; the previous result stays.
    pub(crate) fn cancel_optimize(&mut self, cx: &mut Context<Self>) {
        self.cancel_run(self.optimizer.clone(), "Cancelling the run…", cx);
    }

    /// Run the optimizers off the UI thread through the shared task runner.
    pub(crate) fn optimize(&mut self, cx: &mut Context<Self>) {
        let inputs = self.run_inputs();
        self.research.run_phase = None;
        let request = OptimizeRequest {
            target: inputs.target,
            effort: inputs.effort,
            options: inputs.options,
            respec_grid: matches!(inputs.effort, Effort::Thorough),
            grid_points: None,
        };
        self.start_run(
            self.optimizer.clone(),
            "Optimizing…",
            inputs,
            move |inputs, report| {
                optimize::run(&inputs.simulation, &inputs.context, &request, report)
            },
            cx,
        );
    }

    /// The Shape Optimization run: running, progress, result and staleness.
    pub(crate) fn shape_optimize_state(
        &self,
        cx: &App,
    ) -> TaskState<optimize::shapes::ShapesOutcome> {
        self.shape_run.read(cx).state(&self.shape_inputs())
    }

    /// The inputs a shape placement started now would use.
    pub(crate) fn shape_inputs(&self) -> ShapeRunInputs {
        ShapeRunInputs {
            simulation: self.simulation.clone(),
            context: self.context.clone(),
            effort: self.effort,
        }
    }

    /// Run the real shape optimizer for the current tiers off the UI thread.
    pub(crate) fn optimize_shapes(&mut self, cx: &mut Context<Self>) {
        let inputs = self.shape_inputs();
        self.start_run(
            self.shape_run.clone(),
            "Placing shapes…",
            inputs,
            move |inputs, report| {
                let mut stopped = false;
                let outcome = optimize::shapes::optimize(
                    &inputs.simulation,
                    &inputs.context,
                    inputs.effort,
                    &mut |progress| {
                        let control = report(progress);
                        stopped |= control == Control::Stop;
                        control
                    },
                );
                (!stopped).then_some(outcome)
            },
            cx,
        );
    }

    /// Stop the Shape Optimization run at its next report.
    pub(crate) fn cancel_shape_optimize(&mut self, cx: &mut Context<Self>) {
        self.cancel_run(
            self.shape_run.clone(),
            "Cancelling the shape placement…",
            cx,
        );
    }

    /// The Insight ROI analysis: running, progress, result and staleness.
    pub(crate) fn insight_state(&self, cx: &App) -> TaskState<idleon_research::insight::Outcome> {
        self.insight_run.read(cx).state(&self.insight_inputs())
    }

    /// The inputs an insight analysis started now would use.
    pub(crate) fn insight_inputs(&self) -> ShapeRunInputs {
        self.shape_inputs()
    }

    /// Run the insight ROI analysis off the UI thread through the shared runner.
    pub(crate) fn run_insight_roi(&mut self, cx: &mut Context<Self>) {
        let inputs = self.insight_inputs();
        self.start_run(
            self.insight_run.clone(),
            "Computing insight ROI…",
            inputs,
            move |inputs, report| {
                idleon_research::insight::analyze(
                    &inputs.simulation,
                    &inputs.context,
                    inputs.effort,
                    report,
                )
            },
            cx,
        );
    }

    /// Stop the insight analysis at its next report; the previous table stays.
    pub(crate) fn cancel_insight_roi(&mut self, cx: &mut Context<Self>) {
        self.cancel_run(
            self.insight_run.clone(),
            "Cancelling the insight analysis…",
            cx,
        );
    }

    /// The Observation Unlocks analysis: running, progress, result and staleness.
    pub(crate) fn unlock_state(&self, cx: &App) -> TaskState<idleon_research::unlock::Outcome> {
        self.unlock_run.read(cx).state(&self.shape_inputs())
    }

    /// Run the unlock ranking off the UI thread through the shared runner.
    pub(crate) fn run_unlocks(&mut self, cx: &mut Context<Self>) {
        let inputs = self.shape_inputs();
        self.start_run(
            self.unlock_run.clone(),
            "Ranking observation unlocks…",
            inputs,
            move |inputs, report| {
                idleon_research::unlock::analyze(
                    &inputs.simulation,
                    &inputs.context,
                    inputs.effort,
                    report,
                )
            },
            cx,
        );
    }

    /// Stop the unlock ranking at its next report; the previous table stays.
    pub(crate) fn cancel_unlocks(&mut self, cx: &mut Context<Self>) {
        self.cancel_run(
            self.unlock_run.clone(),
            "Cancelling the unlock ranking…",
            cx,
        );
    }

    /// Which undiscovered observations the Unlocks table lists.
    pub(crate) fn set_unlock_filter(
        &mut self,
        filter: idleon_research::unlock::Filter,
        cx: &mut Context<Self>,
    ) {
        self.research.unlock_filter = filter;
        cx.notify();
    }

    /// The Insight ROI sort mode, as the table's segmented control names it.
    pub(crate) fn set_insight_sort(
        &mut self,
        sort: idleon_research::insight::Sort,
        cx: &mut Context<Self>,
    ) {
        self.research.insight_sort = sort;
        cx.notify();
    }

    /// Whether the Insight ROI table hides observations that lose no EXP.
    pub(crate) fn set_insight_hide_free(&mut self, hide: bool, cx: &mut Context<Self>) {
        self.research.insight_hide_free = hide;
        cx.notify();
    }

    /// Write the stored placement to the grid editor, so the shapes the tab
    /// previewed are the ones the rest of the section reads.
    pub(crate) fn apply_shape_layout(&mut self, cx: &mut Context<Self>) {
        let run = self.shape_optimize_state(cx);
        let Some(result) = run.result else {
            return self.set_status("Run the placement optimizer first.", cx);
        };
        if run.stale {
            return self.set_status(
                "The tiers changed after this run; run it again before applying.",
                cx,
            );
        }
        self.edit(
            |sim| {
                sim.placements = result.placements;
                sim.saved_overlay = None;
            },
            cx,
        );
        self.set_status(
            format!(
                "Applied {} shapes at {} EXP/hr.",
                self.simulation.placements.len(),
                sim::format_rate(result.rate)
            ),
            cx,
        );
    }
}

fn kind_label(kind: MagKind) -> &'static str {
    match kind {
        MagKind::Regular => "magnifying glass",
        MagKind::Monocle => "monocle",
        MagKind::Kaleidoscope => "kaleidoscope",
    }
}

/// An observation's display name, for the status line.
fn observation_name(obs: usize) -> String {
    game()
        .observations
        .get(obs)
        .map(|observation| observation.name.replace('_', " "))
        .unwrap_or_else(|| format!("observation {}", obs + 1))
}

/// The pointer's angle around `centre`, in radians.
fn pointer_angle(centre: (f32, f32), pointer: (f32, f32)) -> f32 {
    (pointer.1 - centre.1).atan2(pointer.0 - centre.0)
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

    /// The canvas handle turns by the angle the pointer swings around the shape;
    /// the `[` and `]` keys step a fixed 5°. A quarter turn is 18 steps either way.
    #[test]
    fn a_pointer_swing_and_keyboard_steps_agree_on_the_rotation() {
        let centre = (200.0, 200.0);
        let grabbed = pointer_angle(centre, (200.0, 120.0));
        let swung = pointer_angle(centre, (280.0, 200.0));
        let mut dragged = idleon_research::model::Placement {
            shape: 0,
            x: 0.0,
            y: 0.0,
            rot: 0.0,
        };
        let swing = f64::from((swung - grabbed).to_degrees());
        let start = dragged.rot;
        shape::set_rotation(&mut dragged, start + swing);

        let mut stepped = dragged;
        stepped.rot = 0.0;
        for _ in 0..(90.0 / shape::ROT_STEP) as usize {
            let next = stepped.rot + shape::ROT_STEP;
            shape::set_rotation(&mut stepped, next);
        }
        assert_eq!(dragged.rot, 90.0);
        assert_eq!(
            dragged, stepped,
            "a dragged quarter turn equals 18 key steps"
        );
    }
}
