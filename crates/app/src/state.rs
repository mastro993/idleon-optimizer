//! Application state: where the window is, the simulation being edited and the run in flight.

use crate::assets::Manifest;
use crate::sections::clamworks::Tab as ClamworksTab;
use crate::sections::endgame;
use crate::sections::killroy::State as KillroyState;
use crate::sections::monument::{State as MonumentState, Tab as MonumentTab};
use crate::sections::pickle::State as PickleState;
use crate::sections::research::{ShapeDrag, State as ResearchState, Tab};
use crate::sections::{self, Section};
use crate::task::{TaskEvent, TaskRunner, TaskState};
use crate::ui;
use crate::ui::import::{self, Import};
use ely_gpui_component::forms::{InputEvent, TextInput};
use gpui::{App, AppContext as _, Context, Entity, FocusHandle, Subscription, Window};
use gpui_mcp::BridgeHandle;
use idleon_core::model::{
    Effort, Mag, MagKind, OptimizeRequest, OptimizeResult, Placement, SimContext, Simulation,
    Target,
};
use idleon_core::pickle::{DEFAULT_TTK, MAX_DPS, MAX_TTK, MIN_TTK};
use idleon_core::save::Shadow;
use idleon_core::{Clamworks, game, monument, optimize, save, shape, sim};

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

/// Everything the window renders and mutates.
pub struct AppState {
    pub simulation: Simulation,
    pub context: SimContext,
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
    /// The Killroy section's forecast settings.
    pub killroy: KillroyState,
    /// The decoded save behind the simulation, for sections that read raw fields.
    pub save: Option<save::Save>,
    /// The monument section's own state, kept alive across dashboard visits.
    pub monument: MonumentState,
    /// The imported save, narrowed to the Monument rows.
    pub monument_save: Option<monument::Save>,
    /// The Clamworks state the last imported save carried.
    pub clamworks: Clamworks,
    /// The Clamworks tab the section was left on.
    pub clamworks_tab: ClamworksTab,
    /// The multikill total Multi-Scalping reads, until the combat sim lands.
    pub clamworks_multikill: f64,
    /// The endgame section's audit of the last applied save.
    pub endgame: endgame::State,
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
    /// The shared background runner for the research optimizer.
    optimizer: Entity<TaskRunner<OptimizeResult, RunInputs>>,
    /// The shared EXP formula tree on the research Rates pane.
    pub breakdown: Entity<ui::formula::FormulaTree>,
    /// The shared formula tree on the Clamworks overview.
    pub clamworks_breakdown: Entity<ui::formula::FormulaTree>,
    /// The shadow-ban flag the loaded save carries (`OptLacc[26]`).
    pub shadow: Shadow,
    pub focus: FocusHandle,
    #[allow(dead_code)]
    bridge: BridgeHandle,
    _search_subscription: Subscription,
    _import_subscription: Subscription,
    _optimizer_subscription: Subscription,
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
        // The runner owns the off-thread work; the window only mirrors its status
        // line, so every section's run reports the same way in the status bar.
        let optimizer = cx.new(|_| TaskRunner::<OptimizeResult, RunInputs>::new());
        let optimizer_subscription =
            cx.subscribe(&optimizer, |state, runner, event: &TaskEvent, cx| {
                match event {
                    TaskEvent::Progress(progress) => {
                        state.status = format!("Optimizing… {:.0}%", progress.fraction * 100.0);
                    }
                    TaskEvent::Finished { cancelled: true } => {
                        state.status = if runner.read(cx).result().is_some() {
                            "Run cancelled; the previous result is unchanged.".into()
                        } else {
                            "Run cancelled; nothing was applied.".into()
                        };
                    }
                    TaskEvent::Finished { cancelled: false } => {
                        if let Some(result) = runner.read(cx).result() {
                            state.status = format!(
                                "{} steps in {} ms.",
                                result.steps.len(),
                                result.elapsed_ms
                            );
                        }
                    }
                }
                cx.notify();
            });
        let breakdown = cx.new(|cx| ui::formula::FormulaTree::new("formula", window, cx));
        let clamworks_breakdown =
            cx.new(|cx| ui::formula::FormulaTree::new("clamworks-formula", window, cx));
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        let mut state = Self {
            simulation,
            context: SimContext::default(),
            effort: Effort::Balanced,
            target: Target::Hours(24.0),
            assets: Manifest::load_default(),
            route: Route::Dashboard,
            last_section: None,
            research: ResearchState::default(),
            killroy: KillroyState::default(),
            save: None,
            monument: MonumentState::default(),
            monument_save: None,
            clamworks: Clamworks::default(),
            clamworks_tab: ClamworksTab::default(),
            clamworks_multikill: 0.0,
            endgame: endgame::State::default(),
            pickle: PickleState::default(),
            search,
            status: "Ready. Pick a calculator, or open the research grid.".into(),
            supplements_open: true,
            import,
            save_partial: false,
            save_loaded: false,
            optimizer,
            breakdown,
            clamworks_breakdown,
            shadow: Shadow::default(),
            focus,
            bridge,
            _search_subscription: search_subscription,
            _import_subscription: import_subscription,
            _optimizer_subscription: optimizer_subscription,
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

    /// The optimizer run: running, progress, result and staleness.
    pub fn optimize_state(&self, cx: &App) -> TaskState<OptimizeResult> {
        self.optimizer.read(cx).state(&self.run_inputs())
    }

    /// Whether the research optimizer has a run in flight.
    pub fn optimize_running(&self, cx: &App) -> bool {
        self.optimizer.read(cx).running()
    }

    /// The inputs a run started now would use.
    fn run_inputs(&self) -> RunInputs {
        RunInputs {
            simulation: self.simulation.clone(),
            context: self.context.clone(),
            target: self.target,
            effort: self.effort,
        }
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
        self.monument_save = Some(monument::Save::from_parts(
            &imported.save,
            imported.companion.as_ref(),
        ));
        self.endgame = endgame::State::audit(&imported);
        self.simulation = imported.simulation;
        self.context = imported.context;
        self.save = Some(imported.save);
        self.clamworks = imported.clamworks;
        self.research.selected_cell = None;
        self.save_loaded = true;
        self.save_partial = !imported.warnings.is_empty();
        self.shadow = imported.shadow;
        self.import.ok = true;
        self.import.message = message.clone();
        self.status = message;
        self.close_import(window, cx);
    }

    /// Set how many upcoming weeks the Killroy forecast covers.
    pub fn set_killroy_weeks(&mut self, value: f64, cx: &mut Context<Self>) {
        self.killroy.weeks = value.round().clamp(1.0, 52.0) as usize;
        cx.notify();
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
        self.simulation.saved_overlay = None;
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

    /// Switch the monument tab.
    pub fn set_monument_tab(&mut self, tab: MonumentTab, cx: &mut Context<Self>) {
        self.monument.tab = tab;
        cx.notify();
    }

    /// The enemy number the odds are for, clamped to the upstream 1-1001 range.
    pub fn set_monument_wave(&mut self, value: f64, cx: &mut Context<Self>) {
        self.monument.wave = value.round().clamp(1.0, 1001.0) as u32;
        cx.notify();
    }

    /// The damage-boost stacks already earned this story.
    pub fn set_monument_boosts(&mut self, value: f64, cx: &mut Context<Self>) {
        self.monument.boosts = value.round().max(0.0) as u32;
        cx.notify();
    }

    /// Rethrows left; `None` means the saved maximum.
    pub fn set_monument_rerolls(&mut self, value: f64, cx: &mut Context<Self>) {
        self.monument.rerolls = Some(value.round().max(0.0) as u32);
        cx.notify();
    }

    /// Go back to the saved maximum rethrows.
    pub fn reset_monument_rerolls(&mut self, cx: &mut Context<Self>) {
        self.monument.rerolls = None;
        cx.notify();
    }

    /// The HP before the last sword, or `None` when unknown.
    pub fn set_monument_hp(&mut self, value: f64, cx: &mut Context<Self>) {
        self.monument.remaining_hp = Some(value.max(f64::MIN_POSITIVE));
        cx.notify();
    }

    /// Clear the optional last-sword HP.
    pub fn clear_monument_hp(&mut self, cx: &mut Context<Self>) {
        self.monument.remaining_hp = None;
        cx.notify();
    }

    /// The Bravery snapshot for the imported save, if any.
    pub fn monument_snapshot(&self) -> Option<monument::Snapshot> {
        self.monument_save
            .as_ref()
            .map(|save| monument::snapshot(save, 0))
    }

    /// The sword odds for the current inputs.
    pub fn monument_odds(&self) -> Option<Result<monument::Odds, monument::Error>> {
        let save = self.monument_save.as_ref()?;
        Some(monument::bravery_odds(
            save,
            self.monument.wave.saturating_sub(1),
            monument::Runtime {
                rerolls: self.monument.rerolls,
                boosts: self.monument.boosts,
            },
            self.monument.remaining_hp,
        ))
    }

    pub fn set_clamworks_tab(&mut self, tab: ClamworksTab, cx: &mut Context<Self>) {
        self.clamworks_tab = tab;
        cx.notify();
    }

    pub fn set_clamworks_multikill(&mut self, multikill: f64, cx: &mut Context<Self>) {
        self.clamworks_multikill = multikill.max(0.0);
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
        self.simulation.saved_overlay = None;
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
        self.simulation.saved_overlay = None;
        if let Some(placement) = self.simulation.placements.get_mut(index) {
            placement.rot = (placement.rot + delta).rem_euclid(360.0);
        }
        cx.notify();
    }

    /// Move a shape by whole steps of the placement lattice.
    pub fn nudge_shape(&mut self, index: usize, dx: f64, dy: f64, cx: &mut Context<Self>) {
        self.simulation.saved_overlay = None;
        if let Some(placement) = self.simulation.placements.get_mut(index) {
            placement.x = shape::snap(placement.x + dx);
            placement.y = shape::snap(placement.y + dy);
        }
        cx.notify();
    }

    pub fn remove_shape(&mut self, index: usize, cx: &mut Context<Self>) {
        self.simulation.saved_overlay = None;
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

    /// Put every supplement back to its default; the stored result turns stale.
    pub fn reset_supplements(&mut self, cx: &mut Context<Self>) {
        self.context = SimContext::default();
        self.status = "Supplements reset to their defaults.".into();
        self.context_changed(cx);
    }

    fn context_changed(&mut self, cx: &mut Context<Self>) {
        // The stored result stays visible and the runner marks it stale: a change
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
        if let Some(result) = self.optimizer.update(cx, |runner, _| runner.take_result()) {
            let gain = result.gain_pct();
            self.simulation = result.after;
            self.status = format!("Applied the optimized layout ({gain:+.1}%).");
        }
        cx.notify();
    }

    pub fn dismiss_result(&mut self, cx: &mut Context<Self>) {
        self.optimizer.update(cx, |runner, _| runner.clear());
        cx.notify();
    }

    /// Stop the run in flight at its next progress report; the previous result stays.
    pub fn cancel_optimize(&mut self, cx: &mut Context<Self>) {
        if self.optimizer.update(cx, |runner, _| runner.cancel()) {
            self.status = "Cancelling the run…".into();
            cx.notify();
        }
    }

    /// Run the optimizers off the UI thread through the shared task runner.
    pub fn optimize(&mut self, cx: &mut Context<Self>) {
        let inputs = self.run_inputs();
        let request = OptimizeRequest {
            target: inputs.target,
            effort: inputs.effort,
            respec_grid: matches!(inputs.effort, Effort::Thorough),
            grid_points: None,
        };
        let started = self.optimizer.update(cx, |runner, cx| {
            runner.start(cx, inputs, move |inputs, report| {
                optimize::run(&inputs.simulation, &inputs.context, &request, report)
            })
        });
        if started {
            self.status = "Optimizing…".into();
            cx.notify();
        }
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
}
