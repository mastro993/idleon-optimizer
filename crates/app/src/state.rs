//! Application state: where the window is, the simulation being edited and the run in flight.

use crate::assets::Manifest;
use crate::sections::pickle::State as PickleState;
use crate::sections::research::{ShapeDrag, State as ResearchState, Tab};
use crate::sections::{self, Section};
use crate::ui;
use crate::ui::import::{self, Import};
use ely_gpui_component::forms::{InputEvent, TextInput};
use gpui::{AppContext as _, Context, Entity, FocusHandle, Subscription, Task, Window};
use gpui_mcp::BridgeHandle;
use idleon_core::model::{
    Control, Effort, Mag, MagKind, OptimizeRequest, OptimizeResult, Placement, Progress,
    SimContext, Simulation, Target,
};
use idleon_core::pickle::{DEFAULT_TTK, MAX_DPS, MAX_TTK, MIN_TTK};
use idleon_core::save::Shadow;
use idleon_core::{game, optimize, save, shape, sim};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Where the window is: the landing dashboard, a registry section, or an id the
/// registry does not declare.
#[derive(Clone, Debug, PartialEq)]
pub enum Route {
    Dashboard,
    Section(&'static Section),
    /// An id no registry entry declares: the shell renders a clear empty state.
    Missing(String),
}

impl Route {
    /// Resolve a section id against the registry.
    pub fn open(id: &str) -> Self {
        match sections::find(id) {
            Some(section) => Route::Section(section),
            None => Route::Missing(id.to_string()),
        }
    }

    /// The registry entry this route shows, if any.
    pub fn section(&self) -> Option<&'static Section> {
        match self {
            Route::Section(section) => Some(section),
            _ => None,
        }
    }

    /// Whether the dashboard is showing.
    pub fn is_dashboard(&self) -> bool {
        matches!(self, Route::Dashboard)
    }
}

/// What the shared page header says about the loaded save.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveStatus {
    /// No save has been imported yet.
    Empty,
    /// A save is loaded; later failed imports leave it intact.
    Loaded,
    /// A save is loaded, but the import left fields out and reported warnings.
    Partial,
    /// The last import failed and there is still no save.
    Error,
}

impl SaveStatus {
    /// The status for the current loaded/partial/failed flags.
    pub fn of(loaded: bool, last_ok: bool, partial: bool) -> Self {
        if loaded && partial {
            Self::Partial
        } else if loaded {
            Self::Loaded
        } else if last_ok {
            Self::Empty
        } else {
            Self::Error
        }
    }

    /// The line the header shows.
    pub fn label(self) -> &'static str {
        match self {
            Self::Empty => "No save",
            Self::Loaded => "Save imported",
            Self::Partial => "Save imported with warnings",
            Self::Error => "Import failed",
        }
    }
}

/// The inputs a stored optimizer result was produced from.
#[derive(Clone, Debug, PartialEq)]
struct RunInputs {
    simulation: Simulation,
    context: SimContext,
    target: Target,
    effort: Effort,
}

impl RunInputs {
    /// Whether these inputs still match the simulation the window shows.
    fn matches(
        &self,
        simulation: &Simulation,
        context: &SimContext,
        target: Target,
        effort: Effort,
    ) -> bool {
        self.simulation == *simulation
            && self.context == *context
            && self.target == target
            && self.effort == effort
    }
}

/// What a background run reports: its latest line and whether it has finished.
struct RunProgress {
    progress: Progress,
    done: bool,
}

/// Everything the window renders and mutates.
pub struct AppState {
    pub simulation: Simulation,
    pub context: SimContext,
    pub result: Option<OptimizeResult>,
    pub running: bool,
    pub effort: Effort,
    pub target: Target,
    /// Upstream artwork resolved through `assets/upstream/manifest.json`.
    pub assets: Manifest,
    /// Where the window is: the dashboard or one section.
    pub route: Route,
    /// The last section opened; the dashboard offers to resume it.
    pub last_section: Option<&'static str>,
    /// The research section's own state, kept alive across dashboard visits.
    pub research: ResearchState,
    /// The BoneJoePickle section's inputs, kept alive the same way.
    pub pickle: PickleState,
    /// The dashboard search field.
    pub search: Entity<TextInput>,
    pub status: String,
    /// Whether the save-derived supplements card is expanded.
    pub supplements_open: bool,
    /// The shared import dialog every section's header opens.
    pub import: Import,
    /// Whether the loaded save left fields out and reported warnings.
    pub save_partial: bool,
    /// Whether a save is loaded, for the shared page header.
    pub save_loaded: bool,
    /// The shadow-ban flag the loaded save carries (`OptLacc[26]`).
    pub shadow: Shadow,
    /// The latest progress line while a run is in flight.
    pub run_progress: Option<Progress>,
    /// Whether the last run was cancelled; cleared by the next run or result action.
    pub run_cancelled: bool,
    /// The inputs the stored result came from; `None` when there is no result.
    result_inputs: Option<RunInputs>,
    /// Set to stop the background run at its next progress report.
    cancel: Option<Arc<AtomicBool>>,
    pub focus: FocusHandle,
    #[allow(dead_code)]
    bridge: BridgeHandle,
    run_task: Option<Task<()>>,
    progress_task: Option<Task<()>>,
    _search_subscription: Subscription,
    _import_subscription: Subscription,
}

impl AppState {
    /// Build the window state. `section` opens that registry id at launch, so a
    /// lane or a harness can start on any section, including hidden tools.
    pub fn new(
        window: &mut Window,
        endpoint_dir: Option<std::path::PathBuf>,
        section: Option<String>,
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
        let import = Import::new(window, cx);
        let search = cx.new(|cx| {
            TextInput::new(window, cx)
                .placeholder("Search calculators…")
                .label("Search calculators")
        });
        let search_subscription =
            cx.subscribe(&search, |_state, _search, event: &InputEvent, cx| {
                if *event == InputEvent::Changed {
                    cx.notify();
                }
            });
        // Every edit of the paste box re-checks the text, so the preview and the
        // Apply button always describe what is on screen.
        let import_input = import.input.clone();
        let import_subscription =
            cx.subscribe(&import_input, |state, _input, event: &InputEvent, cx| {
                if *event == InputEvent::Changed {
                    state.check_paste(cx);
                }
            });
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        let mut state = Self {
            simulation,
            context: SimContext::default(),
            result: None,
            running: false,
            effort: Effort::Balanced,
            target: Target::Hours(24.0),
            assets: Manifest::load_default(),
            route: Route::Dashboard,
            last_section: None,
            research: ResearchState::default(),
            pickle: PickleState::default(),
            search,
            status: "Ready. Pick a calculator, or open the research grid.".into(),
            supplements_open: true,
            import,
            save_partial: false,
            save_loaded: false,
            shadow: Shadow::default(),
            run_progress: None,
            run_cancelled: false,
            result_inputs: None,
            cancel: None,
            focus,
            bridge,
            run_task: None,
            progress_task: None,
            _search_subscription: search_subscription,
            _import_subscription: import_subscription,
        };
        if let Some(id) = section {
            state.open_section(&id, window, cx);
        }
        state
    }

    /// The open section's registry entry, or `None` on the dashboard.
    pub fn current_section(&self) -> Option<&'static Section> {
        self.route.section()
    }

    /// Open a section, leaving the dashboard. An id the registry does not declare
    /// lands on the missing-section state instead of panicking.
    pub fn open_section(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.route = Route::open(id);
        match self.route.section() {
            Some(section) => {
                self.last_section = Some(section.id);
                self.status = match section.status {
                    sections::Status::Ready => {
                        format!("{} · {}", section.title, section.description)
                    }
                    sections::Status::Planned => {
                        format!(
                            "{} is planned; its spec is {}.",
                            section.title, section.spec
                        )
                    }
                };
            }
            None => self.status = format!("Unknown section: {id}."),
        }
        window.focus(&self.focus, cx);
        cx.notify();
    }

    /// Return to the landing dashboard, keeping every section's state alive.
    pub fn back_to_dashboard(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.route = Route::Dashboard;
        self.status = "Dashboard. Pick a calculator.".into();
        window.focus(&self.focus, cx);
        cx.notify();
    }

    /// Reopen the last section, restored to the state it was left in.
    pub fn resume_last_section(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.last_section else {
            self.status = "No section opened yet.".into();
            return cx.notify();
        };
        self.open_section(id, window, cx);
    }

    /// The shared header's save status. While the dialog is open it owns the
    /// verdict: a half-typed paste must not relabel the loaded run.
    pub fn save_status(&self) -> SaveStatus {
        SaveStatus::of(
            self.save_loaded,
            self.import.ok || self.import.open,
            self.save_partial,
        )
    }

    /// Whether the stored result no longer matches the visible simulation.
    pub fn result_stale(&self) -> bool {
        self.result_inputs.as_ref().is_some_and(|inputs| {
            !inputs.matches(&self.simulation, &self.context, self.target, self.effort)
        })
    }

    /// Open the shared import dialog, where every section's header import goes.
    pub fn open_import(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.import.open = true;
        let focus = self.import.input.read(cx).focus().clone();
        window.focus(&focus, cx);
        self.status = "Importing a save: drop a file, browse, or paste the JSON.".into();
        cx.notify();
    }

    /// Close the dialog without applying; the loaded run stays as it was.
    pub fn close_import(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.import.open = false;
        window.focus(&self.focus, cx);
        cx.notify();
    }

    /// Re-check the paste box. Runs on every edit, so the preview always describes
    /// what is on screen.
    pub fn check_paste(&mut self, cx: &mut Context<Self>) {
        let text = self.import.input.read(cx).text().to_string();
        self.check_import(&text, "pasted text", cx);
    }

    /// Parse `text` into a pending import and describe it. Nothing is applied
    /// here: a failed parse only replaces the message and the preview.
    pub fn check_import(&mut self, text: &str, source: &str, cx: &mut Context<Self>) {
        self.import.source = source.to_string();
        if text.trim().is_empty() {
            self.import.pending = None;
            self.import.ok = true;
            self.import.message = "Paste a decoded save, or drop a JSON file.".into();
            return cx.notify();
        }
        match save::import(text) {
            Ok(imported) => {
                self.import.message = import::summary(&imported);
                self.import.pending = Some(imported);
                self.import.ok = true;
            }
            Err(error) => self.fail_import(source, &error.to_string(), cx),
        }
        cx.notify();
    }

    /// Record a dropped file that could not even be read.
    pub fn fail_import(&mut self, source: &str, reason: &str, cx: &mut Context<Self>) {
        self.import.source = source.to_string();
        self.import.pending = None;
        self.import.ok = false;
        self.import.message = format!("Could not read {source}: {reason}");
        cx.notify();
    }

    /// Apply the previewed save; a dialog with nothing pending changes nothing.
    pub fn apply_import(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(imported) = self.import.pending.take() else {
            return;
        };
        let message = import::applied(&imported);
        self.simulation = imported.simulation;
        self.context = imported.context;
        self.research.selected_cell = None;
        self.save_loaded = true;
        self.save_partial = !imported.warnings.is_empty();
        self.shadow = imported.shadow;
        self.import.ok = true;
        self.import.message = message.clone();
        self.status = message;
        self.close_import(window, cx);
    }

    /// Empty the dashboard search; Escape calls this.
    pub fn clear_search(&mut self, cx: &mut Context<Self>) {
        self.search.update(cx, |search, cx| search.set_text("", cx));
        cx.notify();
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
        self.research
            .selected_cell
            .and_then(|cell| game().node_at(cell))
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
        self.research.shape_drag.as_ref().map(|drag| drag.shape)
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
        self.research.selected_shape = Some(shape);
        self.status = "Dragging a shape; release to drop it.".into();
        self.research.shape_drag = Some(ShapeDrag {
            shape,
            start_pointer: pointer,
            start_position: (placement.x, placement.y),
        });
        window.focus(&self.focus, cx);
        cx.notify();
    }

    /// Move the dragged shape to follow the pointer, on the placement lattice.
    pub fn drag_shape_to(&mut self, pointer: (f32, f32), cx: &mut Context<Self>) {
        let Some(drag) = self.research.shape_drag.clone() else {
            return;
        };
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
    pub fn end_shape_drag(&mut self, cx: &mut Context<Self>) {
        if self.research.shape_drag.take().is_some() {
            if let Some(shape) = self.research.selected_shape {
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
        self.research.selected_shape = shape;
        cx.notify();
    }

    /// Select a square and hand it the keyboard.
    pub fn select(&mut self, cell: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.research.selected_cell = Some(cell);
        window.focus(&self.focus, cx);
        cx.notify();
    }

    /// Move the selection one square, clamped to the grid; selects the first square when none is.
    pub fn move_selection(&mut self, dx: i32, dy: i32, cx: &mut Context<Self>) {
        self.research.selected_cell = Some(step_cell(self.research.selected_cell, dx, dy));
        cx.notify();
    }

    /// Raise or lower the selected square's level.
    pub fn bump_selected(&mut self, delta: i64, cx: &mut Context<Self>) {
        let Some(cell) = self.research.selected_cell else {
            return;
        };
        self.bump_level(cell, delta, cx);
    }

    pub fn deselect(&mut self, cx: &mut Context<Self>) {
        self.research.selected_cell = None;
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
        self.research.tab = tab;
        cx.notify();
    }

    /// The damage per second the pickle table solves for.
    pub fn set_pickle_dps(&mut self, dps: f64, cx: &mut Context<Self>) {
        self.pickle.dps = if dps.is_finite() {
            dps.clamp(0.0, MAX_DPS)
        } else {
            0.0
        };
        cx.notify();
    }

    /// The kill-time budget the pickle table solves inside.
    pub fn set_pickle_ttk(&mut self, seconds: f64, cx: &mut Context<Self>) {
        self.pickle.max_ttk = if seconds.is_finite() {
            seconds.clamp(MIN_TTK, MAX_TTK)
        } else {
            DEFAULT_TTK
        };
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
            .or(self.research.selected_cell)
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
        // The stored result stays visible and `result_stale` marks it: a change
        // to the inputs must not silently throw away what the last run found.
        cx.notify();
    }

    pub fn reset_layout(&mut self, cx: &mut Context<Self>) {
        self.simulation.placements.clear();
        self.simulation.mags.clear();
        self.status = "Cleared shapes and magnifiers.".into();
        cx.notify();
    }

    pub fn apply_result(&mut self, cx: &mut Context<Self>) {
        if let Some(result) = self.result.take() {
            let gain = result.gain_pct();
            self.simulation = result.after;
            self.status = format!("Applied the optimized layout ({gain:+.1}%).");
        }
        self.result_inputs = None;
        self.run_cancelled = false;
        cx.notify();
    }

    pub fn dismiss_result(&mut self, cx: &mut Context<Self>) {
        self.result = None;
        self.result_inputs = None;
        self.run_cancelled = false;
        cx.notify();
    }

    /// Stop the run in flight at its next progress report; the previous result stays.
    pub fn cancel_optimize(&mut self, cx: &mut Context<Self>) {
        if !self.running {
            return;
        }
        if let Some(cancel) = &self.cancel {
            cancel.store(true, Ordering::Relaxed);
        }
        if let Some(progress) = &mut self.run_progress {
            progress.message = "Cancelling…".into();
        }
        self.status = "Cancelling the run…".into();
        cx.notify();
    }

    /// Run the optimizers off the UI thread, reporting progress and honouring cancel.
    pub fn optimize(&mut self, cx: &mut Context<Self>) {
        if self.running {
            return;
        }
        self.running = true;
        self.run_cancelled = false;
        let starting = Progress {
            fraction: 0.0,
            message: "Starting…".into(),
        };
        self.run_progress = Some(starting.clone());
        self.status = "Optimizing…".into();
        let inputs = RunInputs {
            simulation: self.simulation.clone(),
            context: self.context.clone(),
            target: self.target,
            effort: self.effort,
        };
        let request = OptimizeRequest {
            target: inputs.target,
            effort: inputs.effort,
            respec_grid: matches!(inputs.effort, Effort::Thorough),
            grid_points: None,
        };
        let simulation = inputs.simulation.clone();
        let context = inputs.context.clone();
        let cancel = Arc::new(AtomicBool::new(false));
        self.cancel = Some(cancel.clone());
        let shared = Arc::new(Mutex::new(RunProgress {
            progress: starting,
            done: false,
        }));
        cx.notify();

        // The run reports from a background thread, so a foreground pump copies
        // its latest line into the entity while the work is in flight.
        let pump_shared = shared.clone();
        self.progress_task = Some(cx.spawn(async move |this, cx| {
            loop {
                let (progress, done) = {
                    let shared = pump_shared.lock().expect("run progress");
                    (shared.progress.clone(), shared.done)
                };
                let percent = progress.fraction * 100.0;
                this.update(cx, |state, cx| {
                    if state.running {
                        state.run_progress = Some(progress);
                        state.status = format!("Optimizing… {percent:.0}%");
                        cx.notify();
                    }
                })
                .ok();
                if done {
                    break;
                }
                cx.background_executor()
                    .timer(Duration::from_millis(40))
                    .await;
            }
        }));

        self.run_task = Some(cx.spawn(async move |this, cx| {
            let report_shared = shared.clone();
            let outcome = cx
                .background_executor()
                .spawn(async move {
                    let result = optimize::run(&simulation, &context, &request, &mut |progress| {
                        if cancel.load(Ordering::Relaxed) {
                            return Control::Stop;
                        }
                        report_shared.lock().expect("run progress").progress = progress;
                        Control::Continue
                    });
                    report_shared.lock().expect("run progress").done = true;
                    result
                })
                .await;
            this.update(cx, |state, cx| state.finish_run(outcome, inputs, cx))
                .ok();
        }));
    }

    /// Store a finished run, or keep the previous result when it was cancelled.
    fn finish_run(
        &mut self,
        outcome: Option<OptimizeResult>,
        inputs: RunInputs,
        cx: &mut Context<Self>,
    ) {
        self.running = false;
        self.run_progress = None;
        self.cancel = None;
        if settle_run(&mut self.result, &mut self.result_inputs, outcome, inputs) {
            self.run_cancelled = true;
            self.status = if self.result.is_some() {
                "Run cancelled; the previous result is unchanged.".into()
            } else {
                "Run cancelled; nothing was applied.".into()
            };
        } else if let Some(result) = &self.result {
            self.status = format!("{} steps in {} ms.", result.steps.len(), result.elapsed_ms);
        }
        cx.notify();
    }
}

/// Apply a finished run to the stored result; `true` when it was cancelled.
///
/// A cancelled run stores nothing, so the result the window already shows stays
/// exactly as it was.
fn settle_run(
    result: &mut Option<OptimizeResult>,
    result_inputs: &mut Option<RunInputs>,
    outcome: Option<OptimizeResult>,
    inputs: RunInputs,
) -> bool {
    match outcome {
        Some(outcome) => {
            *result = Some(outcome);
            *result_inputs = Some(inputs);
            false
        }
        None => true,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_known_id_routes_to_its_registry_entry() {
        let route = Route::open("research");
        assert_eq!(route.section().map(|section| section.id), Some("research"));
        assert!(!route.is_dashboard());
    }

    #[test]
    fn an_unknown_id_routes_to_the_missing_state_instead_of_panicking() {
        let route = Route::open("no-such-section");
        assert_eq!(route, Route::Missing("no-such-section".into()));
        assert_eq!(route.section(), None);
        assert!(!route.is_dashboard());
        assert!(Route::open("").section().is_none());
    }

    #[test]
    fn hidden_tools_still_route_for_the_harness_and_deep_links() {
        let route = Route::open("damage");
        let section = route.section().expect("the damage page is registered");
        assert!(!section.dashboard);
        assert_eq!(section.status, sections::Status::Planned);
    }

    #[test]
    fn arrows_move_one_square_and_stop_at_the_edges() {
        assert_eq!(step_cell(Some(129), 1, 0), 130);
        assert_eq!(step_cell(Some(129), 0, -1), 109);
        assert_eq!(step_cell(Some(19), 1, 0), 19);
        assert_eq!(step_cell(Some(0), -1, -1), 0);
        assert_eq!(step_cell(None, 1, 0), 0);
    }

    #[test]
    fn save_status_prefers_a_loaded_save_over_a_later_failure() {
        assert_eq!(SaveStatus::of(false, true, false), SaveStatus::Empty);
        assert_eq!(SaveStatus::of(false, false, false), SaveStatus::Error);
        assert_eq!(SaveStatus::of(true, true, false), SaveStatus::Loaded);
        assert_eq!(SaveStatus::of(true, false, false), SaveStatus::Loaded);
        assert_eq!(SaveStatus::of(true, true, true), SaveStatus::Partial);
        assert_eq!(SaveStatus::Loaded.label(), "Save imported");
        assert_eq!(SaveStatus::Partial.label(), "Save imported with warnings");
    }

    #[test]
    fn stored_result_inputs_match_only_the_simulation_they_ran_on() {
        let inputs = RunInputs {
            simulation: Simulation::default(),
            context: SimContext::default(),
            target: Target::Hours(24.0),
            effort: Effort::Balanced,
        };
        assert!(inputs.matches(
            &Simulation::default(),
            &SimContext::default(),
            Target::Hours(24.0),
            Effort::Balanced,
        ));
        let changed = Simulation {
            research_level: 2,
            ..Simulation::default()
        };
        assert!(!inputs.matches(
            &changed,
            &SimContext::default(),
            Target::Hours(24.0),
            Effort::Balanced,
        ));
        let context = SimContext {
            extra_shapes: 1,
            ..SimContext::default()
        };
        assert!(!inputs.matches(
            &Simulation::default(),
            &context,
            Target::Hours(24.0),
            Effort::Balanced,
        ));
        assert!(!inputs.matches(
            &Simulation::default(),
            &SimContext::default(),
            Target::Level(120),
            Effort::Balanced,
        ));
        assert!(!inputs.matches(
            &Simulation::default(),
            &SimContext::default(),
            Target::Hours(24.0),
            Effort::Thorough,
        ));
    }

    fn dummy_result() -> OptimizeResult {
        OptimizeResult {
            before: Simulation::default(),
            after: Simulation::default(),
            before_rate: 10.0,
            after_rate: 12.0,
            projection: Default::default(),
            steps: vec!["Ran.".into()],
            elapsed_ms: 3,
        }
    }

    #[test]
    fn a_cancelled_run_keeps_the_previous_result() {
        let inputs = RunInputs {
            simulation: Simulation::default(),
            context: SimContext::default(),
            target: Target::Hours(24.0),
            effort: Effort::Balanced,
        };
        let previous = dummy_result();
        let mut result = Some(previous.clone());
        let mut result_inputs = Some(inputs.clone());
        assert!(settle_run(
            &mut result,
            &mut result_inputs,
            None,
            inputs.clone()
        ));
        assert_eq!(
            result.as_ref().map(|result| result.after_rate),
            Some(previous.after_rate),
            "cancelling replaced the result the window was showing"
        );
        assert_eq!(result_inputs, Some(inputs));
    }

    #[test]
    fn a_finished_run_replaces_the_result_and_its_inputs() {
        let inputs = RunInputs {
            simulation: Simulation::default(),
            context: SimContext::default(),
            target: Target::Hours(24.0),
            effort: Effort::Balanced,
        };
        let mut result = Some(dummy_result());
        let mut result_inputs = None;
        assert!(!settle_run(
            &mut result,
            &mut result_inputs,
            Some(dummy_result()),
            inputs.clone()
        ));
        assert_eq!(result.as_ref().map(|result| result.after_rate), Some(12.0));
        assert_eq!(result_inputs, Some(inputs));
    }
}
