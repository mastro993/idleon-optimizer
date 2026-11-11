//! Application state: where the window is, the simulation being edited and the run in flight.

use crate::assets::Manifest;
use crate::sections::clamworks::Tab as ClamworksTab;
use crate::sections::cog::{RunInputs as CogRunInputs, State as CogState};
use crate::sections::combat_sim::{State as CombatState, Tab as CombatTab};
use crate::sections::coral_god_rank::State as CoralState;
use crate::sections::endgame;
use crate::sections::farming::State as FarmingState;
use crate::sections::fishing::State as FishingState;
use crate::sections::jar::Tab as JarTab;
use crate::sections::killroy::{KillroyOutcome, State as KillroyState};
use crate::sections::ladle::State as LadleSectionState;
use crate::sections::monument::{State as MonumentState, Tab as MonumentTab};
use crate::sections::pickle::State as PickleState;
use crate::sections::prisma::{RunInputs as PrismaRunInputs, State as PrismaState};
use crate::sections::research::{ShapeDrag, State as ResearchState, Tab};
use crate::sections::sailing::State as SailingState;
use crate::sections::sailing::Tab as SailingTab;
use crate::sections::stats_all::State as StatsAllState;
use crate::sections::statue::{Sort as StatueSort, State as StatueState};
use crate::sections::worship_td::{OptimizeRunInputs, PlanRunInputs, State as WorshipState};
use crate::sections::{self, Section};
use crate::session::{self, RecentImport, Session};
use crate::task::{TaskEvent, TaskRunner, TaskState};
use crate::ui;
use crate::ui::export::Export;
use crate::ui::import::{self, Import, Pending};
use ely_gpui_component::forms::{InputEvent, TextInput};
use gpui::{App, AppContext as _, Context, Entity, FocusHandle, Subscription, Window};
use gpui_mcp::BridgeHandle;
use idleon_core::cog::{
    CogResult, Goal as CogGoal, OptimizeOptions, optimize as optimize_cog_board,
};
use idleon_core::cooking_mastery::Mastery;
use idleon_core::farming::Farm;
use idleon_core::fishing::{AccountInputs, FishingSave};
use idleon_core::import as pipeline;
use idleon_core::killroy::Plan as KillroyPlan;
use idleon_core::ladle::Ladle;
use idleon_core::model::{
    Control, Effort, Mag, MagKind, OptimizeRequest, OptimizeResult, Placement, Progress,
    SimContext, Simulation, Target,
};
use idleon_core::pickle::{DEFAULT_TTK, MAX_DPS, MAX_TTK, MIN_TTK};
use idleon_core::prisma::Outcome as PrismaOutcome;
use idleon_core::sailing::{
    AssignmentMode, Fleet, FleetComparison, FleetPlan, Goal, RouteContext, compare, optimize, saved,
};
use idleon_core::save::Shadow;
use idleon_core::stats::{ConstructionMode, MapVisibility, Skill};
use idleon_core::worship::Inputs as WorshipInputs;
use idleon_core::worship::{self, Run, RunResult as WorshipRunResult};
use idleon_core::{Cglunko, Clamworks, Jar, game, monument, optimize, save, shape, sim};

/// What a strategy search stored: its recommendation, or why it could not run.
pub type SearchOutcome = Result<worship::Search, String>;

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
    /// The sailing fleet imported from the save, and the route context.
    pub fleet: Fleet,
    pub sailing: RouteContext,
    /// The sailing section's own state, kept alive across dashboard visits.
    pub sailing_state: SailingState,
    /// The farming field the last import carried.
    pub farm: Farm,
    /// The farming section's selection and crop target, kept alive too.
    pub farming: FarmingState,
    /// The fishing characters, rod and saved toolkit the last import carried.
    pub fishing_save: FishingSave,
    /// The fishing section's character, toolkit, pool and account values.
    pub fishing: FishingState,
    /// The monument section's own state, kept alive across dashboard visits.
    pub monument: MonumentState,
    /// The imported save, narrowed to the Monument rows.
    pub monument_save: Option<monument::Save>,
    /// The Cglunko state the last imported save carried.
    pub cglunko: Cglunko,
    /// The Clamworks state the last imported save carried.
    pub clamworks: Clamworks,
    /// The cog board section: the decoded save and the board on screen.
    pub cog: CogState,
    /// The Clamworks tab the section was left on.
    pub clamworks_tab: ClamworksTab,
    /// The multikill total Multi-Scalping reads, until the combat sim lands.
    pub clamworks_multikill: f64,
    /// The endgame section's audit of the last applied save.
    pub endgame: endgame::State,
    /// The jar section's active tab, kept across dashboard visits.
    pub jar_tab: JarTab,
    /// The Jar Cavern the loaded save carries, if it has one.
    pub jar: Option<Jar>,
    /// The ladle optimizer's save-derived roster and counts.
    pub ladle: Option<Ladle>,
    /// The ladle section's selectors, panel inputs and formula trees.
    pub ladle_state: LadleSectionState,
    /// The Cooking Mastery state the last imported save carried.
    pub mastery: Option<Mastery>,
    /// The BoneJoePickle section's inputs, kept alive the same way.
    pub pickle: PickleState,
    /// The Prisma calculator's controls and the plan for the save.
    pub prisma: PrismaState,
    /// The statue section's saved rows, character and table controls.
    pub statue: StatueState,
    /// The combat simulator's working loadout and named presets.
    pub combat: CombatState,
    /// The All Stats Dashboard's calculation context and its save slice.
    pub stats_all: StatsAllState,
    /// The coral god rank planner's inputs, kept alive across dashboard visits.
    pub coral: CoralState,
    /// The Worship TD simulator's config and live run.
    pub worship: WorshipState,
    /// The tower trial levels and account values the live run reads.
    pub worship_inputs: WorshipInputs,
    /// The dashboard search field.
    pub search: Entity<TextInput>,
    pub status: String,
    /// Whether the save-derived supplements card is expanded.
    pub supplements_open: bool,
    /// The shared import dialog every section's header opens.
    pub import: Import,
    /// The window's own local memory: the last project and the recent imports.
    pub session: Session,
    /// The shared export dialog: the whole window state as one JSON document.
    pub export: Export,
    /// Whether the loaded save left fields out and reported warnings.
    pub save_partial: bool,
    /// Whether a save is loaded, for the shared page header.
    pub save_loaded: bool,
    /// How many saves this window has applied; a Prisma run captures it.
    save_revision: u64,
    /// The shared background runner for the research optimizer.
    optimizer: Entity<TaskRunner<OptimizeResult, RunInputs>>,
    /// The shared runner for the Killroy comparison's trials.
    pub(crate) killroy_run: Entity<TaskRunner<KillroyOutcome, KillroyPlan>>,
    /// The shared runner for the cog board optimizer.
    pub(crate) cog_run: Entity<TaskRunner<CogResult, CogRunInputs>>,
    /// The shared runner for a Worship TD plan simulation.
    pub(crate) worship_plan: Entity<TaskRunner<WorshipRunResult, PlanRunInputs>>,
    /// The shared runner for the Worship TD strategy search.
    pub(crate) worship_optimize: Entity<TaskRunner<SearchOutcome, OptimizeRunInputs>>,
    /// The shared runner for the Prisma bubble search.
    prisma_run: Entity<TaskRunner<PrismaOutcome, PrismaRunInputs>>,
    /// The shared EXP formula tree on the research Rates pane.
    pub breakdown: Entity<ui::formula::FormulaTree>,
    /// The research dashboard's four breakdown trees.
    pub research_trees: crate::sections::research::Trees,
    /// The shared formula tree on the Cglunko overview.
    pub cglunko_breakdown: Entity<ui::formula::FormulaTree>,
    /// The shared formula tree on the Clamworks overview.
    pub clamworks_breakdown: Entity<ui::formula::FormulaTree>,
    /// The shared formula tree on the Fishing calculator.
    pub fishing_breakdown: Entity<ui::formula::FormulaTree>,
    /// The shared EXP rate tree on the Mastery EXP tab.
    pub mastery_breakdown: Entity<ui::formula::FormulaTree>,
    /// The shadow-ban flag the loaded save carries (`OptLacc[26]`).
    pub shadow: Shadow,
    pub focus: FocusHandle,
    #[allow(dead_code)]
    bridge: BridgeHandle,
    _search_subscription: Subscription,
    _statue_search_subscription: Subscription,
    _prisma_search_subscription: Subscription,
    _combat_name_subscription: Subscription,
    _combat_talent_subscription: Subscription,
    _import_subscription: Subscription,
    _optimizer_subscription: Subscription,
    _killroy_run_subscription: Subscription,
    _cog_subscription: Subscription,
    _worship_plan_subscription: Subscription,
    _worship_optimize_subscription: Subscription,
    _prisma_run_subscription: Subscription,
    _session_subscription: Option<Subscription>,
    _session_release_subscription: Option<Subscription>,
    _session_save_pending: bool,
}

impl AppState {
    /// Build the window state. `section` opens that registry id at launch, so a
    /// lane or a harness can start on any section, including hidden tools.
    pub fn new(
        window: &mut Window,
        endpoint_dir: Option<std::path::PathBuf>,
        section: Option<String>,
        state_dir: Option<std::path::PathBuf>,
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
        let statue = StatueState::new(window, cx);
        let statue_search = statue.search.clone();
        let statue_search_subscription =
            cx.subscribe(&statue_search, |_state, _search, event: &InputEvent, cx| {
                if *event == InputEvent::Changed {
                    cx.notify();
                }
            });
        let prisma = PrismaState::new(window, cx);
        let prisma_search = prisma.search.clone();
        let prisma_search_subscription =
            cx.subscribe(&prisma_search, |_state, _search, event: &InputEvent, cx| {
                if *event == InputEvent::Changed {
                    cx.notify();
                }
            });
        // The loadout editor's name field and talent filter redraw the page on
        // every keystroke, like the statue search does.
        let combat = CombatState::new(window, cx);
        let combat_name = combat.name.clone();
        let combat_name_subscription =
            cx.subscribe(&combat_name, |_state, _input, event: &InputEvent, cx| {
                if *event == InputEvent::Changed {
                    cx.notify();
                }
            });
        let combat_talent = combat.talent_search.clone();
        let combat_talent_subscription =
            cx.subscribe(&combat_talent, |_state, _input, event: &InputEvent, cx| {
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
        // The cog board optimizer reports through the same runner; the section
        // reads its stored board as the placement on screen.
        let cog_run = cx.new(|_| TaskRunner::<CogResult, CogRunInputs>::new());
        let cog_subscription = cx.subscribe(&cog_run, |state, runner, event: &TaskEvent, cx| {
            match event {
                TaskEvent::Progress(progress) => {
                    state.status = format!(
                        "Optimizing the cog board… {:.0}%",
                        progress.fraction * 100.0
                    );
                }
                TaskEvent::Finished { cancelled: true } => {
                    state.status = if runner.read(cx).result().is_some() {
                        "Cog run cancelled; the previous result is unchanged.".into()
                    } else {
                        "Cog run cancelled; the board is unchanged.".into()
                    };
                }
                TaskEvent::Finished { cancelled: false } => {
                    if let Some(result) = runner.read(cx).result() {
                        state.status = format!(
                            "Cog board optimized: {:+.2}% with {} swaps.",
                            result.improvement_pct,
                            result.swaps.len()
                        );
                    }
                }
            }
            cx.notify();
        });
        // A plan run is a whole game run off the UI thread: the status line
        // carries its progress and its result stays in the runner for the pane.
        let worship_plan = cx.new(|_| TaskRunner::<WorshipRunResult, PlanRunInputs>::new());
        let worship_plan_subscription =
            cx.subscribe(&worship_plan, |state, runner, event: &TaskEvent, cx| {
                match event {
                    TaskEvent::Progress(progress) => {
                        state.status = format!(
                            "Simulating the Worship TD plan… {:.0}%",
                            progress.fraction * 100.0
                        );
                    }
                    TaskEvent::Finished { cancelled: true } => {
                        state.status = if runner.read(cx).result().is_some() {
                            "Plan run cancelled; the previous result is unchanged.".into()
                        } else {
                            "Plan run cancelled; nothing was applied.".into()
                        };
                    }
                    TaskEvent::Finished { cancelled: false } => {
                        if let Some(result) = runner.read(cx).result() {
                            state.status = format!(
                                "Plan simulated: best wave {}, {} towers.",
                                result.best_wave,
                                result.towers.len()
                            );
                        }
                    }
                }
                cx.notify();
            });
        // The Killroy comparison averages a few short trials per configuration.
        let killroy_run = cx.new(|_| TaskRunner::<KillroyOutcome, KillroyPlan>::new());
        let killroy_run_subscription =
            cx.subscribe(&killroy_run, |state, runner, event: &TaskEvent, cx| {
                match event {
                    TaskEvent::Progress(progress) => {
                        state.status = format!("Running Killroy trials… {}", progress.message);
                    }
                    TaskEvent::Finished { cancelled: true } => {
                        state.status = if runner.read(cx).result().is_some() {
                            "Killroy run cancelled; the previous comparison is unchanged.".into()
                        } else {
                            "Killroy run cancelled; nothing was applied.".into()
                        };
                    }
                    TaskEvent::Finished { cancelled: false } => {
                        if let Some(rows) = runner.read(cx).result() {
                            let best = rows.iter().filter(|row| row.best).count();
                            state.status = format!(
                                "Killroy trials finished: {} configurations, {best} best.",
                                rows.len()
                            );
                        }
                    }
                }
                cx.notify();
            });
        // The strategy search screens dozens of whole runs, so it reports
        // progress and can be cancelled like the plan run.
        let worship_optimize = cx.new(|_| TaskRunner::<SearchOutcome, OptimizeRunInputs>::new());
        let worship_optimize_subscription =
            cx.subscribe(&worship_optimize, |state, runner, event: &TaskEvent, cx| {
                match event {
                    TaskEvent::Progress(progress) => {
                        state.status = format!(
                            "Searching Worship TD strategies… {:.0}%",
                            progress.fraction * 100.0
                        );
                    }
                    TaskEvent::Finished { cancelled: true } => {
                        state.status = if runner.read(cx).result().is_some() {
                            "Search cancelled; the previous recommendation is unchanged.".into()
                        } else {
                            "Search cancelled; nothing was recommended.".into()
                        };
                    }
                    TaskEvent::Finished { cancelled: false } => match runner.read(cx).result() {
                        Some(Ok(result)) => {
                            state.status = format!(
                                "Search finished: {} clears {:.1} waves.",
                                result.label, result.score.primary
                            );
                        }
                        Some(Err(reason)) => state.status = reason.clone(),
                        None => {}
                    },
                }
                cx.notify();
            });
        // The Prisma search compares one bubble per report, so it uses the same
        // runner as every other streamed job.
        let prisma_run = cx.new(|_| TaskRunner::<PrismaOutcome, PrismaRunInputs>::new());
        let prisma_run_subscription =
            cx.subscribe(&prisma_run, |state, runner, event: &TaskEvent, cx| {
                match event {
                    TaskEvent::Progress(progress) => {
                        state.status = format!(
                            "Comparing Prisma bubbles… {:.0}%",
                            progress.fraction * 100.0
                        );
                    }
                    TaskEvent::Finished { cancelled: true } => {
                        state.status = if runner.read(cx).result().is_some() {
                            "Prisma run cancelled; the previous result is unchanged.".into()
                        } else {
                            "Prisma run cancelled; nothing was compared.".into()
                        };
                    }
                    TaskEvent::Finished { cancelled: false } => {
                        if let Some(result) = runner.read(cx).result() {
                            let changed = result
                                .results
                                .iter()
                                .filter(|row| !row.changes.is_empty())
                                .count();
                            state.status = format!(
                                "Compared {} Prisma bubbles; {changed} changed.",
                                result.results.len()
                            );
                        }
                    }
                }
                cx.notify();
            });
        let breakdown = cx.new(|cx| ui::formula::FormulaTree::new("formula", window, cx));
        let research_trees = crate::sections::research::Trees::new(window, cx);
        let ladle_state = LadleSectionState::new(window, cx);
        let cglunko_breakdown =
            cx.new(|cx| ui::formula::FormulaTree::new("cglunko-formula", window, cx));
        let clamworks_breakdown =
            cx.new(|cx| ui::formula::FormulaTree::new("clamworks-formula", window, cx));
        let fishing_breakdown =
            cx.new(|cx| ui::formula::FormulaTree::new("fishing-formula", window, cx));
        let mastery_breakdown =
            cx.new(|cx| ui::formula::FormulaTree::new("cooking-mastery-formula", window, cx));
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        let session = Session::load(Session::path(state_dir));
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
            fleet: Fleet::default(),
            sailing: RouteContext::default(),
            sailing_state: SailingState::default(),
            farm: Farm::default(),
            farming: FarmingState::default(),
            fishing_save: FishingSave::default(),
            fishing: FishingState::default(),
            monument: MonumentState::default(),
            monument_save: None,
            cog: CogState::default(),
            cglunko: Cglunko::default(),
            clamworks: Clamworks::default(),
            clamworks_tab: ClamworksTab::default(),
            clamworks_multikill: 0.0,
            endgame: endgame::State::default(),
            jar_tab: JarTab::default(),
            jar: None,
            ladle: None,
            ladle_state,
            mastery: None,
            pickle: PickleState::default(),
            prisma,
            statue,
            combat,
            stats_all: StatsAllState::default(),
            coral: CoralState::default(),
            worship: WorshipState::default(),
            worship_inputs: WorshipInputs::default(),
            search,
            status: "Ready. Pick a calculator, or open the research grid.".into(),
            supplements_open: true,
            import,
            session,
            export: Export::default(),
            save_partial: false,
            save_loaded: false,
            save_revision: 0,
            optimizer,
            killroy_run,
            cog_run,
            worship_plan,
            worship_optimize,
            prisma_run,
            breakdown,
            research_trees,
            cglunko_breakdown,
            clamworks_breakdown,
            fishing_breakdown,
            mastery_breakdown,
            shadow: Shadow::default(),
            focus,
            bridge,
            _search_subscription: search_subscription,
            _statue_search_subscription: statue_search_subscription,
            _prisma_search_subscription: prisma_search_subscription,
            _combat_name_subscription: combat_name_subscription,
            _combat_talent_subscription: combat_talent_subscription,
            _import_subscription: import_subscription,
            _optimizer_subscription: optimizer_subscription,
            _killroy_run_subscription: killroy_run_subscription,
            _cog_subscription: cog_subscription,
            _worship_plan_subscription: worship_plan_subscription,
            _worship_optimize_subscription: worship_optimize_subscription,
            _prisma_run_subscription: prisma_run_subscription,
            _session_subscription: None,
            _session_release_subscription: None,
            _session_save_pending: false,
        };
        if let Some(restore) = state.session.restore.take() {
            let mut warnings = Vec::new();
            let mut changes = Vec::new();
            crate::project::apply(
                &mut state,
                restore.project,
                restore.configs,
                &mut warnings,
                &mut changes,
            );
            state.save_partial |= !warnings.is_empty();
            state.status = "Restored the last session.".into();
        }
        if let Some(notice) = state.session.notice.clone() {
            state.status = notice;
        }
        if let Some(id) = section {
            state.open_section(&id, window, cx);
        }
        // The observer saves after the burst of edits a drag or a typing run
        // produces; the release hook catches the last one when the window goes.
        cx.defer_in(window, |state, _window, cx| {
            state._session_subscription =
                Some(cx.observe_self(|state, cx| state.schedule_session_save(cx)));
            state._session_release_subscription =
                Some(cx.on_release(|state, _cx| state.flush_session()));
        });
        state
    }

    /// The open section's registry entry, or `None` on the dashboard.
    pub fn current_section(&self) -> Option<&'static Section> {
        self.route.section()
    }

    /// Save the session now, for a route change, an applied import or a close.
    pub fn flush_session(&mut self) {
        let Some(project) = session::project_value(self) else {
            return;
        };
        if let Err(error) = self.session.save(&project) {
            eprintln!("idleon-optimizer: could not save the session: {error}");
        }
    }

    /// Let a burst of edits settle before writing the session once.
    fn schedule_session_save(&mut self, cx: &mut Context<Self>) {
        if self._session_save_pending {
            return;
        }
        self._session_save_pending = true;
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(session::SAVE_DEBOUNCE).await;
            this.update(cx, |state, _cx| {
                state._session_save_pending = false;
                state.flush_session();
            })
            .ok();
        })
        .detach();
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
        self.flush_session();
        window.focus(&self.focus, cx);
        cx.notify();
    }

    /// Return to the landing dashboard, keeping every section's state alive.
    pub fn back_to_dashboard(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.route = Route::Dashboard;
        self.status = "Dashboard. Pick a calculator.".into();
        self.flush_session();
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
    /// here: the core pipeline decodes everything the document needs, so a
    /// failure only replaces the message and Apply cannot fail halfway.
    pub fn check_import(&mut self, text: &str, source: &str, cx: &mut Context<Self>) {
        self.import.source = source.to_string();
        if text.trim().is_empty() {
            self.import.pending = None;
            self.import.ok = true;
            self.import.message =
                "Paste a decoded save, a project file, an ROv2 state string, or drop one.".into();
            return cx.notify();
        }
        match pipeline::read(text) {
            Ok(mut document) => {
                let ids: Vec<String> = match &document {
                    pipeline::Document::Project(project) => project
                        .document
                        .sections
                        .keys()
                        .chain(project.document.section.iter())
                        .cloned()
                        .collect(),
                    pipeline::Document::Section(section) => vec![section.section.clone()],
                    pipeline::Document::Save(_) => Vec::new(),
                };
                for id in ids {
                    warn_unknown_sections(document.warnings_mut(), Some(&id));
                }
                // Decode every section config now: a document that cannot be
                // applied whole never becomes pending.
                match crate::project::prepare(&mut document) {
                    Ok(configs) => {
                        self.import.message = import::summary(&document);
                        self.import.pending = Some(Pending { document, configs });
                        self.import.ok = true;
                    }
                    Err(error) => self.fail_import(source, &error, cx),
                }
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

    /// Apply the previewed document; a dialog with nothing pending changes
    /// nothing. Everything was decoded while previewing, so this only assigns:
    /// a document either applies whole or never becomes pending. Every change
    /// is logged.
    pub fn apply_import(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(Pending { document, configs }) = self.import.pending.take() else {
            return;
        };
        let format = document.format().to_string();
        let characters = document.characters();
        let research_level = document.research_level();
        let mut warnings = document.warnings().to_vec();
        let mut message = import::applied(&document);
        let before_configs = crate::project::configs(self);
        let before_cog = self.cog.save.clone();
        let mut changes = Vec::new();
        match document {
            pipeline::Document::Save(imported) => {
                changes.push(format!(
                    "save: {} · {} character(s)",
                    imported.format.label(),
                    imported.characters
                ));
                changes.extend(crate::project::research_changes(
                    &self.simulation,
                    &imported.simulation,
                ));
                changes.extend(crate::project::supplement_changes(
                    &self.context,
                    &imported.context,
                ));
                self.load_imported(*imported);
            }
            pipeline::Document::Project(project) => {
                crate::project::apply(self, *project, configs, &mut warnings, &mut changes);
                self.save_partial |= !warnings.is_empty();
            }
            pipeline::Document::Section(_) => {
                for config in configs {
                    crate::project::assign(self, config, &mut warnings);
                }
            }
        }
        if self.cog.save != before_cog {
            self.cog_run.update(cx, |runner, _| {
                runner.cancel();
                runner.clear();
            });
        }
        changes.extend(crate::project::config_changes(
            &before_configs,
            &crate::project::configs(self),
        ));
        if !changes.is_empty() {
            message = format!("{message} {}", crate::project::change_line(&changes));
        }
        if !warnings.is_empty() {
            message = format!("{message} {}", warnings_line(&warnings));
        }
        self.import.ok = true;
        self.import.changes = changes;
        self.import.message = message.clone();
        self.status = message;
        self.session.record(RecentImport::new(
            format,
            self.import.source.clone(),
            characters,
            research_level,
            warnings.len(),
        ));
        self.flush_session();
        self.close_import(window, cx);
    }

    /// Take a save the importer already read into every section that reads it.
    pub(crate) fn load_imported(&mut self, imported: save::Imported) {
        self.monument_save = Some(monument::Save::from_parts(
            &imported.save,
            imported.companion.as_ref(),
        ));
        self.endgame = endgame::State::audit(&imported);
        self.fishing_save = FishingSave::from_save(&imported.save);
        self.fishing.adopt(&self.fishing_save);
        self.simulation = imported.simulation;
        self.context = imported.context;
        let combat_character = if self.combat.character < imported.save.character_count() {
            self.combat.character
        } else {
            0
        };
        self.combat.load(&imported.save, combat_character);
        self.stats_all.load(&imported.save);
        self.save = Some(imported.save);
        self.save_revision = self.save_revision.wrapping_add(1);
        self.prisma.adopt(self.save.as_ref());
        self.statue.load(imported.statues);
        self.coral.adopt(&imported.coral);
        self.worship_adopt(imported.worship);
        self.fleet = imported.fleet;
        self.sailing = imported.sailing;
        self.farm = imported.farming;
        self.cog.load(imported.cog);
        self.cglunko = imported.cglunko;
        self.clamworks = imported.clamworks;
        self.jar = imported.jar;
        self.ladle_state
            .adopt(&imported.ladle.clone().unwrap_or_default());
        self.ladle = imported.ladle;
        self.mastery = imported.mastery;
        self.research.selected_cell = None;
        self.sailing_state.island = self
            .fleet
            .boats
            .first()
            .and_then(|boat| boat.target_island)
            .filter(|island| *island < self.fleet.available_islands())
            .unwrap_or(0);
        self.sailing_state.mode = AssignmentMode::Optimized;
        self.sailing_state.goal = Goal::Treasure;
        self.save_loaded = true;
        self.save_partial = !imported.warnings.is_empty();
        self.shadow = imported.shadow;
        // The stored comparison stays; a save that changes the plan marks it
        // stale through the runner's own input check.
        self.killroy_default_selection();
    }

    /// Open the export dialog over a freshly written project document.
    pub fn open_export(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.export = Export::open(self);
        self.status = format!(
            "Exporting the project: {} bytes, ready to copy.",
            self.export.bytes()
        );
        window.focus(&self.focus, cx);
        cx.notify();
    }

    /// Close the export dialog without touching the project.
    pub fn close_export(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.export.open = false;
        window.focus(&self.focus, cx);
        cx.notify();
    }

    /// Put the whole project document on the clipboard.
    pub fn copy_export(&mut self, cx: &mut Context<Self>) {
        crate::ui::export::copy(self, cx);
        self.status = format!(
            "Copied the project JSON to the clipboard ({} bytes).",
            self.export.bytes()
        );
        cx.notify();
    }

    /// The inputs a Prisma run started now would use.
    pub fn prisma_inputs(&self) -> PrismaRunInputs {
        PrismaRunInputs {
            save_revision: self.save_revision,
            character: self.prisma.character,
            cauldron: self.prisma.cauldron.index(),
            afk_seconds: if crate::sections::prisma::valid_hours(self.prisma.afk_hours) {
                (self.prisma.afk_hours * 3600.0).round() as u64
            } else {
                0
            },
        }
    }

    /// The Prisma run: running, progress, result and staleness.
    pub fn prisma_task(&self, cx: &App) -> TaskState<PrismaOutcome> {
        self.prisma_run.read(cx).state(&self.prisma_inputs())
    }

    /// Select the character the plan reads its Prisma state for.
    pub fn set_prisma_character(&mut self, character: usize, cx: &mut Context<Self>) {
        self.prisma.character = character;
        self.prisma.adopt(self.save.as_ref());
        cx.notify();
    }

    /// Switch cauldron, which rebuilds the plan and its eligible rows.
    pub fn set_prisma_cauldron(
        &mut self,
        cauldron: idleon_core::prisma::Cauldron,
        cx: &mut Context<Self>,
    ) {
        self.prisma.cauldron = cauldron;
        // Metric ids carry the cauldron, so a new cauldron cannot keep the old one.
        self.prisma.metric = None;
        self.prisma.adopt(self.save.as_ref());
        cx.notify();
    }

    /// Set the AFK claim the run captures.
    pub fn set_prisma_hours(&mut self, hours: f64, cx: &mut Context<Self>) {
        if hours.is_finite() {
            self.prisma.afk_hours = hours.clamp(
                crate::sections::prisma::MIN_HOURS,
                crate::sections::prisma::MAX_HOURS,
            );
        }
        cx.notify();
    }

    /// Select the metric the rows rank by; empty is the bubble order.
    pub fn set_prisma_metric(&mut self, metric: &str, cx: &mut Context<Self>) {
        self.prisma.metric = (!metric.is_empty()).then(|| metric.to_string());
        cx.notify();
    }

    /// Run the bubble comparison off the UI thread through the shared task runner.
    pub fn run_prisma(&mut self, cx: &mut Context<Self>) {
        if !crate::sections::prisma::valid_hours(self.prisma.afk_hours) {
            self.status = "Enter AFK claim hours above 2 minutes and at most 55,555.".into();
            cx.notify();
            return;
        }
        let Some(plan) = self.prisma.plan.clone() else {
            self.status = "Import a save to compare Prisma bubbles.".into();
            cx.notify();
            return;
        };
        let inputs = self.prisma_inputs();
        let started = self.prisma_run.update(cx, |runner, cx| {
            runner.start(cx, inputs, move |_inputs, report| {
                idleon_core::prisma::run(&plan, report)
            })
        });
        if started {
            self.status = "Comparing Prisma bubbles…".into();
            cx.notify();
        }
    }

    /// Ask a running Prisma comparison to stop; the previous result stays.
    pub fn cancel_prisma(&mut self, cx: &mut Context<Self>) {
        if self.prisma_run.update(cx, |runner, _| runner.cancel()) {
            self.status = "Cancelling the Prisma run…".into();
            cx.notify();
        }
    }

    /// Empty the dashboard search; Escape calls this.
    pub fn clear_search(&mut self, cx: &mut Context<Self>) {
        self.search.update(cx, |search, cx| search.set_text("", cx));
        cx.notify();
    }

    /// The sailing fleet comparison for the section's current island and goal.
    pub fn fleet_comparison(&self) -> FleetComparison {
        compare(
            &self.fleet,
            self.sailing_state.island,
            self.sailing_state.goal,
            &self.sailing,
        )
    }

    /// One island's plan under the selected assignment mode.
    pub fn fleet_plan(&self, island: usize, goal: Goal) -> FleetPlan {
        match self.sailing_state.mode {
            AssignmentMode::Optimized => optimize(&self.fleet, island, goal, &self.sailing),
            AssignmentMode::Saved => saved(&self.fleet, island, goal, &self.sailing),
        }
    }

    pub fn set_sailing_tab(&mut self, tab: SailingTab, cx: &mut Context<Self>) {
        self.sailing_state.tab = tab;
        cx.notify();
    }

    pub fn set_sailing_sort_by_eta(&mut self, sorted: bool, cx: &mut Context<Self>) {
        self.sailing_state.sort_by_eta = sorted;
        cx.notify();
    }

    /// The Unending Loot Search talent percentage the replacement matrix reads.
    pub fn set_unending_loot_search(&mut self, percent: f64, cx: &mut Context<Self>) {
        self.sailing.unending_loot_search = percent.clamp(0.0, 100.0);
        cx.notify();
    }

    pub fn set_fleet_mode(&mut self, mode: AssignmentMode, cx: &mut Context<Self>) {
        self.sailing_state.mode = mode;
        cx.notify();
    }

    pub fn set_fleet_island(&mut self, island: usize, cx: &mut Context<Self>) {
        let total = self.fleet.available_islands().max(1);
        self.sailing_state.island = island.min(total - 1);
        cx.notify();
    }

    pub fn set_fleet_goal(&mut self, goal: Goal, cx: &mut Context<Self>) {
        self.sailing_state.goal = goal;
        cx.notify();
    }

    pub fn set_fleet_minimum_tier(&mut self, tier: u32, cx: &mut Context<Self>) {
        self.sailing.minimum_tier = tier.min(5);
        cx.notify();
    }

    /// Reset the fishing toolkit and location to the selected character's save.
    pub fn fishing_reset(&mut self, cx: &mut Context<Self>) {
        self.fishing.character_defaults(&self.fishing_save);
        cx.notify();
    }

    /// Select a fishing character and take that character's saved toolkit/spot.
    pub fn set_fishing_character(&mut self, index: usize, cx: &mut Context<Self>) {
        self.fishing.character = index.min(self.fishing_save.characters.len().saturating_sub(1));
        self.fishing.character_defaults(&self.fishing_save);
        cx.notify();
    }

    /// Select a lure index, clamped to the upstream list.
    pub fn set_fishing_lure(&mut self, index: usize, cx: &mut Context<Self>) {
        self.fishing.lure = index.min(idleon_core::fishing::data().lures.len().saturating_sub(1));
        cx.notify();
    }

    /// Select a line index, clamped to the upstream list.
    pub fn set_fishing_line(&mut self, index: usize, cx: &mut Context<Self>) {
        self.fishing.line = index.min(idleon_core::fishing::data().lines.len().saturating_sub(1));
        cx.notify();
    }

    /// Switch the selected pool by its location id.
    pub fn set_fishing_location(&mut self, id: String, cx: &mut Context<Self>) {
        if idleon_core::fishing::data().location(&id).is_some() {
            self.fishing.location = id;
            cx.notify();
        }
    }

    /// Set the AFK claim length in hours.
    pub fn set_fishing_hours(&mut self, hours: f64, cx: &mut Context<Self>) {
        self.fishing.hours = if hours.is_finite() {
            hours.clamp(0.04, 10_000.0)
        } else {
            1.0
        };
        cx.notify();
    }

    /// Edit one account value the fishing evaluator reads.
    pub fn set_fishing_account(
        &mut self,
        change: impl FnOnce(&mut AccountInputs),
        cx: &mut Context<Self>,
    ) {
        change(&mut self.fishing.account);
        self.fishing.clamp(&self.fishing_save);
        cx.notify();
    }

    /// Select a plot; the farming section keeps it across dashboard visits.
    pub fn select_plot(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.farming.select(index);
        self.status = format!("Plot {} selected.", self.farming.selected_plot + 1);
        window.focus(&self.focus, cx);
        cx.notify();
    }

    /// Aim the later evolution features at a crop id, clamped to the families.
    pub fn set_target_crop(&mut self, value: f64, cx: &mut Context<Self>) {
        self.farming.set_target_crop(value);
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

    /// Open the Grid editor on a square the dashboard preview picked.
    pub fn open_grid_cell(&mut self, cell: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.research.tab = Tab::Grid;
        self.research.selected_cell = Some(cell);
        window.focus(&self.focus, cx);
        cx.notify();
    }

    /// Open the Rates editor on an observation the dashboard preview picked.
    pub fn open_observation(&mut self, obs: usize, cx: &mut Context<Self>) {
        self.research.tab = Tab::Rates;
        self.research.selected_observation = Some(obs);
        cx.notify();
    }

    pub fn set_jar_tab(&mut self, tab: JarTab, cx: &mut Context<Self>) {
        self.jar_tab = tab;
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

    /// The cog optimizer's run: running, progress, result and staleness.
    pub fn cog_task(&self, cx: &App) -> TaskState<CogResult> {
        self.cog_run.read(cx).state(&self.cog.inputs())
    }

    /// Change the objective; a stored result becomes stale until it reruns.
    pub fn set_cog_goal(&mut self, goal: CogGoal, cx: &mut Context<Self>) {
        if self.cog.goal != goal {
            self.cog.goal = goal;
            self.status = format!("Cog objective: {}.", goal.label());
            cx.notify();
        }
    }

    /// Optimize the board off the UI thread through the shared task runner.
    pub fn optimize_cog(&mut self, cx: &mut Context<Self>) {
        let inputs = self.cog.inputs();
        if inputs.board.iter().all(Option::is_none) {
            self.status = "Cog board: no cogs to place.".into();
            cx.notify();
            return;
        }
        let started = self.cog_run.update(cx, |runner, cx| {
            runner.start(cx, inputs, move |inputs, report| {
                optimize_cog_board(
                    &inputs.board,
                    &inputs.shelf,
                    inputs.goal,
                    &OptimizeOptions::default(),
                    report,
                )
            })
        });
        if started {
            self.status = "Optimizing the cog board…".into();
            cx.notify();
        }
    }

    /// Ask a running cog optimization to stop at its next report.
    pub fn cancel_cog(&mut self, cx: &mut Context<Self>) {
        if self.cog_run.update(cx, |runner, _| runner.cancel()) {
            self.status = "Cancelling the cog optimization…".into();
            cx.notify();
        }
    }

    /// Restore the save board and drop the stored result.
    pub fn reset_cog(&mut self, cx: &mut Context<Self>) {
        self.cog.reset();
        self.cog_run.update(cx, |runner, _| {
            runner.cancel();
            runner.clear();
        });
        self.status = "Cog board reset to the save state.".into();
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

    /// Pick the stats context's character; the map follows its current map and
    /// then the map list, exactly like an upstream character switch.
    pub fn set_stats_character(&mut self, character: usize, cx: &mut Context<Self>) {
        let Some(snapshot) = self.stats_all.snapshot.clone() else {
            return;
        };
        if character >= snapshot.characters() {
            return;
        }
        self.stats_all
            .context
            .select_character(character, &snapshot);
        self.stats_context_changed(cx);
    }

    /// Pick the stats context's map, from the current map list.
    pub fn set_stats_map(&mut self, map: usize, cx: &mut Context<Self>) {
        if !self
            .stats_all
            .options()
            .iter()
            .any(|option| option.class.map == map)
        {
            return;
        }
        self.stats_all.context.map = map;
        self.stats_context_changed(cx);
    }

    /// Switch the map list level; the selected map follows it when it can.
    pub fn set_stats_map_visibility(&mut self, level: MapVisibility, cx: &mut Context<Self>) {
        self.stats_all.context.map_visibility = level;
        self.stats_all.clamp(&mut Vec::new());
        self.stats_context_changed(cx);
    }

    /// Switch the skill the skill-scoped descriptors read.
    pub fn set_stats_skill(&mut self, skill: Skill, cx: &mut Context<Self>) {
        self.stats_all.context.skill = skill;
        self.stats_context_changed(cx);
    }

    /// Switch construction mode on or off.
    pub fn set_stats_construction(&mut self, mode: ConstructionMode, cx: &mut Context<Self>) {
        self.stats_all.context.construction = mode;
        self.stats_context_changed(cx);
    }

    /// A context change refreshes every value the page derives from it.
    fn stats_context_changed(&mut self, cx: &mut Context<Self>) {
        let class = idleon_core::stats::classify(self.stats_all.context.map);
        let character = self
            .stats_all
            .snapshot
            .as_ref()
            .map(|snapshot| snapshot.name(self.stats_all.context.character))
            .unwrap_or_default();
        self.status = if character.is_empty() {
            "Load a save to resolve the stats context.".into()
        } else {
            format!(
                "Stats context: {character} · {} · {} · Construction {}",
                class.name,
                self.stats_all.context.skill.label(),
                self.stats_all.context.construction.label()
            )
        };
        cx.notify();
    }

    /// Pick a character for the statue table; characters without saved rows
    /// cannot be selected, so the table always has a valid subject.
    pub fn set_statue_character(&mut self, character: usize, cx: &mut Context<Self>) {
        if self.statue.save.has_character(character) {
            self.statue.character = character;
            cx.notify();
        }
    }

    pub fn set_statue_sort(&mut self, sort: StatueSort, cx: &mut Context<Self>) {
        self.statue.sort = sort;
        cx.notify();
    }

    /// Pick the loadout editor's tab.
    pub fn set_combat_tab(&mut self, tab: CombatTab, cx: &mut Context<Self>) {
        self.combat.tab = tab;
        cx.notify();
    }

    /// Switch the loadout editor to another character of the loaded save.
    pub fn set_combat_character(&mut self, character: usize, cx: &mut Context<Self>) {
        let Some(save) = self.save.as_ref() else {
            return;
        };
        if character >= save.character_count() {
            return;
        }
        self.combat.load(save, character);
        self.status = format!("Editing {}.", self.combat.loadout.character.name);
        cx.notify();
    }

    /// Select the equipment slot the detail panel edits.
    pub fn set_combat_slot(&mut self, slot: usize, cx: &mut Context<Self>) {
        if slot < self.combat.loadout.slots.len() {
            self.combat.selected = slot;
            cx.notify();
        }
    }

    /// Equip an item in one slot; an empty key unequips it.
    pub fn set_combat_item(&mut self, row: usize, slot: usize, key: &str, cx: &mut Context<Self>) {
        self.combat.loadout.set_item(row, slot, key);
        if let Some(entry) = self.combat.loadout.slot(row, slot) {
            self.status = format!("{}: {}.", entry.name(), entry.item_name());
        }
        cx.notify();
    }

    /// Set one talent's level, clamped to its saved maximum.
    pub fn set_combat_talent(&mut self, id: u32, level: u32, cx: &mut Context<Self>) {
        self.combat.loadout.set_talent(id, level);
        cx.notify();
    }

    /// Put every slot and talent back to the imported save.
    pub fn reset_combat_loadout(&mut self, cx: &mut Context<Self>) {
        self.combat.loadout.reset();
        self.status = "Loadout reset to the save.".into();
        cx.notify();
    }

    /// Save the working loadout under the typed name, replacing one with the
    /// same name for the same character, like the upstream storage key does.
    pub fn save_combat_preset(&mut self, cx: &mut Context<Self>) {
        let name = self.combat.preset_name(cx);
        match self.combat.presets.store(&self.combat.loadout, &name) {
            Ok(message) => {
                self.combat
                    .name
                    .update(cx, |input, cx| input.set_text("", cx));
                self.status = message;
            }
            Err(message) => self.status = message,
        }
        cx.notify();
    }

    /// Apply a named loadout to its character.
    pub fn load_combat_preset(&mut self, index: usize, cx: &mut Context<Self>) {
        self.status = self.combat.presets.apply(index, &mut self.combat.loadout);
        cx.notify();
    }

    /// Forget one named loadout.
    pub fn delete_combat_preset(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(preset) = self.combat.presets.delete(index) {
            self.status = format!("Deleted the loadout “{}”.", preset.name);
            cx.notify();
        }
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

    /// The Worship plan run: running, progress, result and staleness.
    pub fn worship_plan_task(&self, cx: &App) -> TaskState<WorshipRunResult> {
        self.worship_plan
            .read(cx)
            .state(&self.worship_plan_inputs())
    }

    /// The inputs a plan run started now would use.
    pub fn worship_plan_inputs(&self) -> PlanRunInputs {
        PlanRunInputs {
            trial: self.worship.trial,
            source: self.worship.plan_source,
            seed: self.worship.seed,
            inputs: self.worship_inputs.clone(),
            plan: self.worship.plan.clone().unwrap_or_default(),
        }
    }

    /// Simulate the plan to the end off the UI thread.
    pub fn simulate_worship_plan(&mut self, cx: &mut Context<Self>) {
        self.worship_ensure_plan();
        let inputs = self.worship_plan_inputs();
        let started = self.worship_plan.update(cx, |runner, cx| {
            runner.start(cx, inputs, move |inputs, report| {
                let mut run = Run::with_plan(inputs.inputs, inputs.trial, inputs.seed, inputs.plan);
                let max_ticks = run.max_ticks().max(1);
                while !run.done() {
                    run.step(500);
                    let progress = Progress {
                        fraction: (run.tick() as f64 / max_ticks as f64).min(0.99) as f32,
                        message: format!("Simulating plan · wave {}", run.wave()),
                    };
                    if report(progress) == Control::Stop {
                        return None;
                    }
                }
                Some(run.result())
            })
        });
        if started {
            self.status = "Simulating the Worship TD plan…".into();
            cx.notify();
        }
    }

    /// Ask a running plan simulation to stop at its next report.
    pub fn cancel_worship_plan(&mut self, cx: &mut Context<Self>) {
        if self.worship_plan.update(cx, |runner, _| runner.cancel()) {
            self.status = "Cancelling the plan run…".into();
            cx.notify();
        }
    }

    /// The strategy search: running, progress, result and staleness.
    pub fn worship_optimize_task(&self, cx: &App) -> TaskState<SearchOutcome> {
        self.worship_optimize
            .read(cx)
            .state(&self.worship_optimize_inputs())
    }

    /// The inputs a search started now would use.
    pub fn worship_optimize_inputs(&self) -> OptimizeRunInputs {
        OptimizeRunInputs {
            trial: self.worship.trial,
            config: self.worship.optimize,
            inputs: self.worship_inputs.clone(),
        }
    }

    /// Search strategies off the UI thread through the shared task runner.
    pub fn optimize_worship(&mut self, cx: &mut Context<Self>) {
        self.worship_ensure_plan();
        let inputs = self.worship_optimize_inputs();
        // The plan on screen is another baseline to beat, not part of the key:
        // adopting a recommendation must not mark it stale.
        let options = inputs
            .config
            .options(inputs.trial, self.worship.plan.clone());
        let started = self.worship_optimize.update(cx, |runner, cx| {
            runner.start(cx, inputs, move |inputs, report| {
                match worship::optimize(&inputs.inputs, &options, report) {
                    worship::Outcome::Found(search) => Some(Ok(*search)),
                    worship::Outcome::Cancelled => None,
                    worship::Outcome::Refused(reason) => Some(Err(reason.to_string())),
                }
            })
        });
        if started {
            self.status = "Searching the Worship TD strategies…".into();
            cx.notify();
        }
    }

    /// Ask a running search to stop at its next report.
    pub fn cancel_worship_optimize(&mut self, cx: &mut Context<Self>) {
        if self
            .worship_optimize
            .update(cx, |runner, _| runner.cancel())
        {
            self.status = "Cancelling the strategy search…".into();
            cx.notify();
        }
    }

    /// Adopt the recommendation into the Plan pane and open it.
    pub fn worship_use_optimized_plan(&mut self, cx: &mut Context<Self>) {
        let Some(Ok(result)) = self.worship_optimize.read(cx).result().cloned() else {
            return;
        };
        let trial = self
            .worship
            .trial
            .min(worship::data().trials.len().saturating_sub(1));
        if result.trial != trial {
            // A recommendation for another trial cannot be placed here.
            self.status = "That recommendation was searched on another trial.".into();
            cx.notify();
            return;
        }
        let Some(trial_data) = worship::trial(trial) else {
            return;
        };
        self.worship.plan_stages = worship::plan_stages(trial_data, &result.plan);
        self.worship.plan_issues =
            worship::plan_issues(&self.worship_inputs, trial_data, &result.plan);
        self.worship.plan = Some(result.plan);
        self.worship.plan_from_optimizer = true;
        self.worship_plan.update(cx, |runner, _| {
            runner.cancel();
            runner.clear();
        });
        self.status = format!(
            "Using the recommended plan: {} clears {:.0} waves.",
            result.label, result.score.primary
        );
        self.worship_set_tab(crate::sections::worship_td::Tab::Plan, cx);
    }
}

/// A warning count worth the status line, naming the first one.
fn warnings_line(warnings: &[String]) -> String {
    match warnings.first() {
        None => "No warnings.".into(),
        Some(first) if warnings.len() == 1 => first.clone(),
        Some(first) => format!("{first} (and {} more warnings)", warnings.len() - 1),
    }
}

/// Name a section id the registry does not declare, in the preview's warnings,
/// and leave a declared id alone.
fn warn_unknown_sections(warnings: &mut Vec<String>, id: Option<&str>) {
    let Some(id) = id else {
        return;
    };
    if crate::sections::find(id).is_none() {
        warnings.push(format!(
            "section: {id} is not a section this build declares"
        ));
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
    fn an_unknown_section_id_warns_and_a_declared_one_does_not() {
        let mut warnings = Vec::new();
        warn_unknown_sections(&mut warnings, Some("research"));
        assert!(warnings.is_empty(), "{warnings:?}");
        warn_unknown_sections(&mut warnings, Some("no-such-section"));
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("no-such-section"), "{warnings:?}");
        warn_unknown_sections(&mut warnings, None);
        assert_eq!(warnings.len(), 1);
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
