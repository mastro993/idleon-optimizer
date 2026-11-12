//! Application state: where the window is, the simulation being edited and the run in flight.

use crate::assets::Manifest;
use crate::sections::cglunko::Tab as CglunkoTab;
use crate::sections::clamworks::{State as ClamworksState, Tab as ClamworksTab};
use crate::sections::cog::{
    PerfectRunInputs as CogPerfectInputs, RunInputs as CogRunInputs, State as CogState,
};
use crate::sections::combat_sim::{State as CombatState, Tab as CombatTab};
use crate::sections::cooking_mastery::State as MasteryPlanState;
use crate::sections::coral_god_rank::State as CoralState;
use crate::sections::drop_rate::State as DropRateState;
use crate::sections::endgame;
use crate::sections::farming::{
    CompareInputs as FarmingCompareInputs, RunInputs as FarmingRunInputs, State as FarmingState,
};
use crate::sections::fishing::State as FishingState;
use crate::sections::fountain::State as FountainState;
use crate::sections::jar::Tab as JarTab;
use crate::sections::jelly_operator::{
    RunInputs as JellyRunInputs, Simulator as JellySimulator,
    SimulatorRunInputs as JellySimRunInputs, State as JellyState, Tab as JellyTab,
};
use crate::sections::killroy::{KillroyOutcome, State as KillroyState};
use crate::sections::ladle::State as LadleSectionState;
use crate::sections::monument::justice::{
    Action as JusticeAction, Effect as JusticeEffect, Field as JusticeField,
};
use crate::sections::monument::{State as MonumentState, Tab as MonumentTab};
use crate::sections::opal::Tab as OpalTab;
use crate::sections::pickle::State as PickleState;
use crate::sections::prisma::{RunInputs as PrismaRunInputs, State as PrismaState};
use crate::sections::research::{ShapeDrag, State as ResearchState, Tab};
use crate::sections::royal_guardian::State as RoyalState;
use crate::sections::sailing::State as SailingState;
use crate::sections::sailing::Tab as SailingTab;
use crate::sections::sailing::Trees as SailingTrees;
use crate::sections::sailing::simulation::RunInputs as SailingSimInputs;
use crate::sections::spelunking::RunInputs as SpelunkRunInputs;
use crate::sections::spelunking::State as SpelunkingState;
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
use gpui::{
    App, AppContext as _, ClipboardItem, Context, Entity, FocusHandle, Subscription, Window,
};
use gpui_mcp::BridgeHandle;
use idleon_core::cglunko::{Page as CglunkoPage, Plan as CglunkoPlan};
use idleon_core::clamworks::{Plan as ClamworksPlan, PlanInput as ClamworksPlanInput};
use idleon_core::cog::{
    CogResult, Goal as CogGoal, OptimizeOptions, PerfectBoard, optimize as optimize_cog_board,
    perfect_board as perfect_cog_board,
};
use idleon_core::cooking_mastery::Mastery;
use idleon_core::coral::{Inputs as CoralInputs, Plan as CoralPlan};
use idleon_core::farming::{
    Allocation as FarmingOutcome, Comparison as FarmingComparison, Farm, Mode, Objective,
};
use idleon_core::fishing::{AccountInputs, FishingSave};
use idleon_core::import as pipeline;
use idleon_core::jelly::operation::{RevivePolicy, simulate as simulate_jelly_operation};
use idleon_core::jelly::{Estimate as JellyEstimate, Jelly, Trial as JellyTrial};
use idleon_core::killroy::Plan as KillroyPlan;
use idleon_core::ladle::Ladle;
use idleon_core::model::{
    Control, Effort, Mag, MagKind, OptimizeRequest, OptimizeResult, Placement, Progress,
    SimContext, SimOptions, Simulation, Target,
};
use idleon_core::opal::{Plan as OpalPlan, PlanInput as OpalPlanInput, PlanTarget as OpalTarget};
use idleon_core::pickle::{DEFAULT_TTK, MAX_DPS, MAX_TTK, MIN_TTK};
use idleon_core::prisma::Outcome as PrismaOutcome;
use idleon_core::sailing::{
    AssignmentMode, DEFAULT_HOURS, DEFAULT_SEED, Fleet, FleetComparison, FleetPlan, Goal,
    MAX_HOURS, MAX_TRIALS, MIN_HOURS, MIN_TRIALS, RouteContext, SimConfig,
    SimOptions as SailingSimOptions, SimResult, compare, optimize, saved, simulate,
};
use idleon_core::save::Shadow;
use idleon_core::spelunking::RunResult as SpelunkRunResult;
use idleon_core::stats::{ConstructionMode, MapVisibility, Skill};
use idleon_core::statue::Estimate as StatueEstimate;
use idleon_core::worship::Inputs as WorshipInputs;
use idleon_core::worship::{self, Run, RunResult as WorshipRunResult};
use idleon_core::{
    Cglunko, Clamworks, Fountain, Jar, Opal, RoyalGuardian, game, monument, optimize, save, shape,
    sim,
};

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

/// The inputs a stored shape-placement result was produced from.
#[derive(Clone, Debug, PartialEq)]
struct ShapeRunInputs {
    simulation: Simulation,
    context: SimContext,
    effort: Effort,
}

/// The inputs a stored optimizer result was produced from.
#[derive(Clone, Debug, PartialEq)]
struct RunInputs {
    simulation: Simulation,
    context: SimContext,
    target: Target,
    effort: Effort,
    options: SimOptions,
}

/// Everything the window renders and mutates.
pub struct AppState {
    pub simulation: Simulation,
    /// The live context the simulation reads: the save-derived values plus the
    /// user's supplements on top.
    pub context: SimContext,
    /// The context the last applied save or document established.
    /// [`AppState::reset_supplements`] returns here, so a reset never throws
    /// away a multiplier the save decoded.
    context_base: SimContext,
    pub effort: Effort,
    pub target: Target,
    /// The three upstream run toggles the next projection reads.
    pub options: SimOptions,
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
    /// The sailing breakdowns' formula trees.
    pub sailing_trees: SailingTrees,
    /// The shared runner for the fleet pile simulation.
    sailing_sim: Entity<TaskRunner<SimResult, SailingSimInputs>>,
    /// The farming field the last import carried.
    pub farm: Farm,
    /// The farming section's selection and crop target, kept alive too.
    pub farming: FarmingState,
    /// The farming preset bar's name field.
    pub farming_name: Entity<TextInput>,
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
    /// The Cglunko tab the section was left on.
    pub cglunko_tab: CglunkoTab,
    /// The page the Cglunko upgrade list and plan work on.
    pub cglunko_page: CglunkoPage,
    /// The Clamworks state the last imported save carried.
    pub clamworks: Clamworks,
    /// The cog board section: the decoded save and the board on screen.
    pub cog: CogState,
    /// The Clamworks tab the section was left on.
    pub clamworks_tab: ClamworksTab,
    /// The multikill total Multi-Scalping reads, until the combat sim lands.
    pub clamworks_multikill: f64,
    /// The pearls the Clamworks upgrade plan may spend.
    pub clamworks_budget: f64,
    /// The levels the applied plan previews; `None` shows the saved levels.
    pub clamworks_projection: Option<[u32; 9]>,
    /// The AFK claim's character, stand-ins, hours and pearl table.
    pub clamworks_claim: ClamworksState,
    /// The endgame section's audit of the last applied save.
    pub endgame: endgame::State,
    /// The Jelly Operator board the last imported save carried.
    pub jelly: Option<Jelly>,
    /// The Jelly Operator section's board on screen and its palette pick.
    pub jelly_state: JellyState,
    /// The jar section's active tab, kept across dashboard visits.
    pub jar_tab: JarTab,
    /// The tier the rupie value tab projects its income for.
    pub jar_tier: u32,
    /// The rupie value breakdown tree on the jar section.
    pub jar_breakdown: Entity<ui::formula::FormulaTree>,
    /// The Jar Cavern the loaded save carries, if it has one.
    pub jar: Option<Jar>,
    /// The Spelunking section's controls and account inputs.
    pub spelunking: SpelunkingState,
    /// The Spelunking rows the loaded save carries, if it has them.
    pub spelunking_save: Option<idleon_core::spelunking::Save>,
    /// The shared runner for the Spelunking expedition run.
    pub(crate) spelunk_sim: Entity<TaskRunner<SpelunkRunResult, SpelunkRunInputs>>,
    /// The ladle optimizer's save-derived roster and counts.
    pub ladle: Option<Ladle>,
    /// The ladle section's selectors, panel inputs and formula trees.
    pub ladle_state: LadleSectionState,
    /// The Cooking Mastery state the last imported save carried.
    pub mastery: Option<Mastery>,
    /// The Mastery EXP tab's distribution goal and budget, kept across visits.
    pub mastery_plan: MasteryPlanState,
    /// The Fountain Cavern the loaded save carries, if it has one.
    pub fountain: Option<Fountain>,
    /// The Fountain section's tab, water, desired currency and curve pick.
    pub fountain_state: FountainState,
    /// The Opal Optimizer's villagers and account bonuses, when the save has them.
    pub opal: Option<Opal>,
    /// The villager the opal breakdown is switched to, or the total.
    pub opal_selected: Option<usize>,
    /// The Opal Optimizer's tab, kept across dashboard visits.
    pub opal_tab: OpalTab,
    /// The villager the opal optimizer maximizes, or `None` for the total.
    pub opal_target: Option<usize>,
    /// The opal total the optimizer distributes, seeded from the save.
    pub opal_total: f64,
    /// The Royal Guardian rows the loaded save carries, if it has them.
    pub royal: Option<RoyalGuardian>,
    /// The Royal Guardian section's world and selected outpost.
    pub royal_state: RoyalState,
    /// The ninja loadout and inventory the loaded save carries, if it has any.
    pub sneaking: Option<idleon_core::sneaking::Loadout>,
    /// The sneaking workspace's login context and selected slot.
    pub sneaking_state: crate::sections::sneaking::State,
    /// The shared formula tree on the sneaking breakdowns workspace.
    pub sneaking_breakdown: Entity<ui::formula::FormulaTree>,
    /// The BoneJoePickle section's inputs, kept alive the same way.
    pub pickle: PickleState,
    /// The Prisma calculator's controls and the plan for the save.
    pub prisma: PrismaState,
    /// The statue section's saved rows, character and table controls.
    pub statue: StatueState,
    /// The combat simulator's working loadout and named presets.
    pub combat: CombatState,
    /// The All Stats Dashboard's calculation context, save slice and breakdowns.
    pub stats_all: StatsAllState,
    /// The shared formula tree on the All Stats breakdown card.
    pub stats_breakdown: Entity<ui::formula::FormulaTree>,
    /// The Drop Rate Calculator's context, save slice and live dungeon fields.
    pub drop_rate: DropRateState,
    /// The coral god rank planner's inputs, kept alive across dashboard visits.
    pub coral: CoralState,
    /// The Worship TD simulator's config and live run.
    pub worship: WorshipState,
    /// The tower trial levels and account values the live run reads.
    pub worship_inputs: WorshipInputs,
    /// The dashboard search field.
    pub search: Entity<TextInput>,
    /// The Justice case search field.
    pub monument_search: Entity<TextInput>,
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
    pub(crate) save_revision: u64,
    /// The shared background runner for the research optimizer.
    optimizer: Entity<TaskRunner<OptimizeResult, RunInputs>>,
    /// The shared runner for the Shape Optimization tab's placement.
    shape_run: Entity<TaskRunner<idleon_core::optimize::shapes::ShapesOutcome, ShapeRunInputs>>,
    /// The shared runner for the Killroy comparison's trials.
    pub(crate) killroy_run: Entity<TaskRunner<KillroyOutcome, KillroyPlan>>,
    /// The shared runner for the coral god rank beam search.
    pub(crate) coral_run: Entity<TaskRunner<CoralPlan, CoralInputs>>,
    /// The shared runner for the Jelly Operator operation estimate.
    pub(crate) jelly_run: Entity<TaskRunner<JellyEstimate, JellyRunInputs>>,
    /// The shared runner for one simulator operation.
    pub(crate) jelly_sim_run: Entity<TaskRunner<JellyTrial, JellySimRunInputs>>,
    /// The shared runner for the cog board optimizer.
    pub(crate) cog_run: Entity<TaskRunner<CogResult, CogRunInputs>>,
    /// The shared runner for the perfect cog board.
    pub(crate) cog_perfect_run: Entity<TaskRunner<PerfectBoard, CogPerfectInputs>>,
    /// The shared runner for the Cglunko upgrade plan.
    cglunko_run: Entity<TaskRunner<CglunkoPlan, CglunkoPage>>,
    /// The shared runner for the Clamworks upgrade plan.
    clamworks_run: Entity<TaskRunner<ClamworksPlan, ClamworksPlanInput>>,
    /// The shared runner for the opal distribution plan.
    opal_run: Entity<TaskRunner<OpalPlan, OpalPlanInput>>,
    /// The shared runner for a Worship TD plan simulation.
    pub(crate) worship_plan: Entity<TaskRunner<WorshipRunResult, PlanRunInputs>>,
    /// The shared runner for the Worship TD strategy search.
    pub(crate) worship_optimize: Entity<TaskRunner<SearchOutcome, OptimizeRunInputs>>,
    /// The shared runner for the Prisma bubble search.
    prisma_run: Entity<TaskRunner<PrismaOutcome, PrismaRunInputs>>,
    /// The shared runner for the Land Rank allocation.
    farming_run: Entity<TaskRunner<FarmingOutcome, FarmingRunInputs>>,
    /// The shared runner for the preset comparison.
    farming_compare_run: Entity<TaskRunner<FarmingComparison, FarmingCompareInputs>>,
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
    /// The shared formula tree on the Fountain overview.
    pub fountain_breakdown: Entity<ui::formula::FormulaTree>,
    /// The shared formula tree on the Opal overview.
    pub opal_breakdown: Entity<ui::formula::FormulaTree>,
    /// The shadow-ban flag the loaded save carries (`OptLacc[26]`).
    pub shadow: Shadow,
    pub focus: FocusHandle,
    #[allow(dead_code)]
    bridge: BridgeHandle,
    _search_subscription: Subscription,
    _monument_search_subscription: Subscription,
    _statue_search_subscription: Subscription,
    _prisma_search_subscription: Subscription,
    _combat_name_subscription: Subscription,
    _combat_talent_subscription: Subscription,
    _combat_overrides_subscription: Subscription,
    _research_preset_subscription: Subscription,
    _import_subscription: Subscription,
    _optimizer_subscription: Subscription,
    _shape_run_subscription: Subscription,
    _killroy_run_subscription: Subscription,
    _coral_run_subscription: Subscription,
    _jelly_run_subscription: Subscription,
    _jelly_sim_run_subscription: Subscription,
    _cog_subscription: Subscription,
    _cog_perfect_subscription: Subscription,
    _cglunko_run_subscription: Subscription,
    _clamworks_run_subscription: Subscription,
    _opal_run_subscription: Subscription,
    _worship_plan_subscription: Subscription,
    _worship_optimize_subscription: Subscription,
    _prisma_run_subscription: Subscription,
    _farming_run_subscription: Subscription,
    _farming_compare_subscription: Subscription,
    _sailing_sim_subscription: Subscription,
    _spelunk_sim_subscription: Subscription,
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
        let monument_search = cx.new(|cx| {
            TextInput::new(window, cx)
                .placeholder("Visitor or words from the request…")
                .label("Find a Justice case")
        });
        let monument_search_subscription = cx.subscribe(
            &monument_search,
            |_state, _search, event: &InputEvent, cx| {
                if *event == InputEvent::Changed {
                    cx.notify();
                }
            },
        );
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
        let combat_overrides = combat.override_search.clone();
        let combat_overrides_subscription = cx.subscribe(
            &combat_overrides,
            |_state, _input, event: &InputEvent, cx| {
                if *event == InputEvent::Changed {
                    cx.notify();
                }
            },
        );
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
        // The Shape Optimization tab runs the real placement optimizer through
        // the same runner, so its preview is exactly what a run produced.
        let shape_run = cx.new(|_| {
            TaskRunner::<idleon_core::optimize::shapes::ShapesOutcome, ShapeRunInputs>::new()
        });
        let shape_run_subscription =
            cx.subscribe(&shape_run, |state, runner, event: &TaskEvent, cx| {
                match event {
                    TaskEvent::Progress(progress) => {
                        state.status = format!("Placing shapes… {:.0}%", progress.fraction * 100.0);
                    }
                    TaskEvent::Finished { cancelled: true } => {
                        state.status = if runner.read(cx).result().is_some() {
                            "Shape placement cancelled; the previous layout is unchanged.".into()
                        } else {
                            "Shape placement cancelled; nothing was placed.".into()
                        };
                    }
                    TaskEvent::Finished { cancelled: false } => {
                        if let Some(result) = runner.read(cx).result() {
                            state.status = result
                                .steps
                                .last()
                                .cloned()
                                .unwrap_or_else(|| "Shape placement finished.".into());
                        }
                    }
                }
                cx.notify();
            });
        // The cog board optimizer reports through the same runner; a finished
        // run opens its walkthrough, which reads the stored board one step at a
        // time until Done keeps it.
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
                    let result = runner.read(cx).result().cloned();
                    if let Some(result) = result {
                        state.cog.begin_walkthrough(&result);
                        state.status = if result.swaps.is_empty() {
                            "Cog board optimized: no swaps needed.".into()
                        } else {
                            format!(
                                "Cog board optimized: {:+.2}% with {} swaps. Walk through them, then press Done to keep the board.",
                                result.improvement_pct,
                                result.swaps.len()
                            )
                        };
                    }
                }
            }
            cx.notify();
        });
        // The perfect board is a separate, heavier run: it rebuilds a candidate
        // for every player slot. It keeps its own result so the optimizer's
        // stored board is not overwritten.
        let cog_perfect_run = cx.new(|_| TaskRunner::<PerfectBoard, CogPerfectInputs>::new());
        let cog_perfect_subscription =
            cx.subscribe(&cog_perfect_run, |state, runner, event: &TaskEvent, cx| {
                match event {
                    TaskEvent::Progress(progress) => {
                        state.status = format!(
                            "Building the perfect cog board… {:.0}%",
                            progress.fraction * 100.0
                        );
                    }
                    TaskEvent::Finished { cancelled: true } => {
                        state.status = if runner.read(cx).result().is_some() {
                            "Perfect board cancelled; the previous result is unchanged.".into()
                        } else {
                            "Perfect board cancelled; the board is unchanged.".into()
                        };
                    }
                    TaskEvent::Finished { cancelled: false } => {
                        if let Some(perfect) = runner.read(cx).result() {
                            state.status = format!(
                                "Perfect cog board ready: {} layer(s), {} Excogia block(s), {} Yang.",
                                perfect.layers.len(),
                                perfect.excogia_blocks,
                                perfect.yang_placed
                            );
                        }
                    }
                }
                cx.notify();
            });
        // The Cglunko plan is a short bounded greedy run that reports every
        // purchase and stops where the saved stones run out.
        let cglunko_run = cx.new(|_| TaskRunner::<CglunkoPlan, CglunkoPage>::new());
        let cglunko_run_subscription =
            cx.subscribe(&cglunko_run, |state, runner, event: &TaskEvent, cx| {
                match event {
                    TaskEvent::Progress(progress) => {
                        state.status = format!("Planning Cglunko upgrades… {}", progress.message);
                    }
                    TaskEvent::Finished { cancelled: true } => {
                        state.status = if runner.read(cx).result().is_some() {
                            "Cglunko plan cancelled; the previous plan is unchanged.".into()
                        } else {
                            "Cglunko plan cancelled; nothing was planned.".into()
                        };
                    }
                    TaskEvent::Finished { cancelled: false } => {
                        if let Some(plan) = runner.read(cx).result() {
                            state.status = format!(
                                "Cglunko plan ready: {} purchases, {:.2}× DR.",
                                plan.steps.len(),
                                plan.dr_after()
                            );
                        }
                    }
                }
                cx.notify();
            });
        // The Clamworks plan is a short bounded greedy run, but it reports every
        // purchase and stops where the budget runs out.
        let clamworks_run = cx.new(|_| TaskRunner::<ClamworksPlan, ClamworksPlanInput>::new());
        let clamworks_run_subscription =
            cx.subscribe(&clamworks_run, |state, runner, event: &TaskEvent, cx| {
                match event {
                    TaskEvent::Progress(progress) => {
                        state.status = format!("Planning Clamworks upgrades… {}", progress.message);
                    }
                    TaskEvent::Finished { cancelled: true } => {
                        state.status = if runner.read(cx).result().is_some() {
                            "Clamworks plan cancelled; the previous plan is unchanged.".into()
                        } else {
                            "Clamworks plan cancelled; nothing was applied.".into()
                        };
                    }
                    TaskEvent::Finished { cancelled: false } => {
                        if let Some(plan) = runner.read(cx).result() {
                            state.status = format!(
                                "Clamworks plan ready: {} purchases, {:.0} of {:.0} pearls.",
                                plan.steps.len(),
                                plan.spent,
                                plan.budget
                            );
                        }
                    }
                }
                cx.notify();
            });
        // The opal optimizer reports each lowest count it tries, so the status
        // line names the search and a cancel stops it between candidates.
        let opal_run = cx.new(|_| TaskRunner::<OpalPlan, OpalPlanInput>::new());
        let opal_run_subscription =
            cx.subscribe(&opal_run, |state, runner, event: &TaskEvent, cx| {
                match event {
                    TaskEvent::Progress(progress) => {
                        state.status =
                            format!("Optimizing the opal distribution… {}", progress.message);
                    }
                    TaskEvent::Finished { cancelled: true } => {
                        state.status = if runner.read(cx).result().is_some() {
                            "Opal plan cancelled; the previous plan is unchanged.".into()
                        } else {
                            "Opal plan cancelled; nothing was planned.".into()
                        };
                    }
                    TaskEvent::Finished { cancelled: false } => {
                        if let Some(plan) = runner.read(cx).result() {
                            state.status = format!(
                                "Opal plan ready: {} opals across the villagers.",
                                ui::format::count(plan.spent() as u64)
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
                        if let Some(outcome) = runner.read(cx).result() {
                            let best = outcome.rows.iter().filter(|row| row.best).count();
                            state.status = format!(
                                "Killroy trials finished: {} configurations, {best} best.",
                                outcome.rows.len()
                            );
                        }
                    }
                }
                cx.notify();
            });
        // The estimate runs one seeded trial at a time, so it reports each one
        // and can be cancelled at a trial boundary.
        let jelly_run = cx.new(|_| TaskRunner::<JellyEstimate, JellyRunInputs>::new());
        let jelly_run_subscription =
            cx.subscribe(&jelly_run, |state, runner, event: &TaskEvent, cx| {
                match event {
                    TaskEvent::Progress(progress) => {
                        state.status =
                            format!("Estimating the Jelly operation… {}", progress.message);
                    }
                    TaskEvent::Finished { cancelled: true } => {
                        state.status = if runner.read(cx).result().is_some() {
                            "Jelly estimate cancelled; the previous estimate is unchanged.".into()
                        } else {
                            "Jelly estimate cancelled; nothing was stored.".into()
                        };
                    }
                    TaskEvent::Finished { cancelled: false } => {
                        if let Some(estimate) = runner.read(cx).result() {
                            state.status = format!(
                                "Jelly estimate ready: {} of {} trials cleared.",
                                estimate.success_count, estimate.trials
                            );
                        }
                    }
                }
                cx.notify();
            });
        // The simulator runs one seeded operation off the UI thread, so its
        // result is stored the same way as the estimate's aggregate.
        let jelly_sim_run = cx.new(|_| TaskRunner::<JellyTrial, JellySimRunInputs>::new());
        let jelly_sim_run_subscription =
            cx.subscribe(&jelly_sim_run, |state, runner, event: &TaskEvent, cx| {
                match event {
                    TaskEvent::Progress(progress) => {
                        state.status =
                            format!("Simulating the Jelly operation… {}", progress.message);
                    }
                    TaskEvent::Finished { cancelled: true } => {
                        state.status = if runner.read(cx).result().is_some() {
                            "Jelly simulation cancelled; the previous run is unchanged.".into()
                        } else {
                            "Jelly simulation cancelled; nothing was stored.".into()
                        };
                    }
                    TaskEvent::Finished { cancelled: false } => {
                        if let Some(trial) = runner.read(cx).result() {
                            state.status = format!(
                                "Jelly operation finished: {} at {:.2}s with {} damage.",
                                trial.ended_by.replace('-', " "),
                                trial.time,
                                crate::ui::format::value(trial.damage)
                            );
                        }
                    }
                }
                cx.notify();
            });
        // The coral beam search is bounded but budget-sized, so it reports each
        // step and can be cancelled like the other optimizers.
        let coral_run = cx.new(|_| TaskRunner::<CoralPlan, CoralInputs>::new());
        let coral_run_subscription =
            cx.subscribe(&coral_run, |state, runner, event: &TaskEvent, cx| {
                match event {
                    TaskEvent::Progress(progress) => {
                        state.status = format!("Planning Coral Kid upgrades… {}", progress.message);
                    }
                    TaskEvent::Finished { cancelled: true } => {
                        state.status = if runner.read(cx).result().is_some() {
                            "Coral plan cancelled; the previous plan is unchanged.".into()
                        } else {
                            "Coral plan cancelled; nothing was planned.".into()
                        };
                    }
                    TaskEvent::Finished { cancelled: false } => {
                        if let Some(plan) = runner.read(cx).result() {
                            state.status = format!(
                                "Coral plan ready: {} actions, {:.0}% budget committed.",
                                plan.actions.len(),
                                if plan.budget > 0.0 {
                                    plan.spent / plan.budget * 100.0
                                } else {
                                    0.0
                                }
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
        // The Land Rank allocator streams its points through the same runner,
        // so progress, cancel and the stale marker read like every other run.
        let farming_run = cx.new(|_| TaskRunner::<FarmingOutcome, FarmingRunInputs>::new());
        // The preset comparison plans every goal, so it streams through its
        // own runner and keeps the table the previous run produced.
        let farming_compare_run =
            cx.new(|_| TaskRunner::<FarmingComparison, FarmingCompareInputs>::new());
        let farming_compare_subscription = cx.subscribe(
            &farming_compare_run,
            |state, runner, event: &TaskEvent, cx| {
                match event {
                    TaskEvent::Progress(progress) => {
                        state.status = format!(
                            "Comparing farming presets… {:.0}%",
                            progress.fraction * 100.0
                        );
                    }
                    TaskEvent::Finished { cancelled: true } => {
                        state.status = if runner.read(cx).result().is_some() {
                            "Preset comparison cancelled; the previous table is unchanged.".into()
                        } else {
                            "Preset comparison cancelled; no table was stored.".into()
                        };
                    }
                    TaskEvent::Finished { cancelled: false } => {
                        if let Some(result) = runner.read(cx).result() {
                            state.status = format!("Compared {} farming goals.", result.rows.len());
                        }
                    }
                }
                cx.notify();
            },
        );
        let farming_name = cx.new(|cx| {
            TextInput::new(window, cx)
                .placeholder("Preset name")
                .label("Preset name")
        });
        let farming_run_subscription =
            cx.subscribe(&farming_run, |state, runner, event: &TaskEvent, cx| {
                match event {
                    TaskEvent::Progress(progress) => {
                        state.status =
                            format!("Allocating Land Rank… {:.0}%", progress.fraction * 100.0);
                    }
                    TaskEvent::Finished { cancelled: true } => {
                        state.status = if runner.read(cx).result().is_some() {
                            "Land Rank allocation cancelled; the previous result is unchanged."
                                .into()
                        } else {
                            "Land Rank allocation cancelled; nothing was assigned.".into()
                        };
                    }
                    TaskEvent::Finished { cancelled: false } => {
                        if let Some(result) = runner.read(cx).result() {
                            state.status = format!(
                                "Allocated {} Land Rank point(s) for {}.",
                                result.points_spent,
                                result.objective.label()
                            );
                        }
                    }
                }
                cx.notify();
            });
        let research = ResearchState::new(window, cx);
        let research_preset = research.preset_name.clone();
        let research_preset_subscription = cx.subscribe(
            &research_preset,
            |_state, _input, event: &InputEvent, cx| {
                if *event == InputEvent::Changed {
                    cx.notify();
                }
            },
        );
        let research_trees = crate::sections::research::Trees::new(window, cx);
        let sailing_trees = SailingTrees::new(window, cx);
        // The pile simulation reports each trial batch and keeps the previous
        // result when a run is cancelled, like every other long run.
        let sailing_sim = cx.new(|_| TaskRunner::<SimResult, SailingSimInputs>::new());
        let sailing_sim_subscription =
            cx.subscribe(&sailing_sim, |state, runner, event: &TaskEvent, cx| {
                match event {
                    TaskEvent::Progress(progress) => {
                        state.status = format!("Simulating the fleet pile… {}", progress.message);
                    }
                    TaskEvent::Finished { cancelled: true } => {
                        state.status = if runner.read(cx).result().is_some() {
                            "Fleet pile simulation cancelled; the previous result is unchanged."
                                .into()
                        } else {
                            "Fleet pile simulation cancelled; nothing was stored.".into()
                        };
                    }
                    TaskEvent::Finished { cancelled: false } => {
                        if let Some(result) = runner.read(cx).result() {
                            state.status = format!(
                                "Fleet pile simulation ready: {:.2} chests of {} trials.",
                                result.pile.count.mean, result.setup.trials
                            );
                        }
                    }
                }
                cx.notify();
            });
        // The expedition run reports each progress line and keeps the previous
        // result when a run is cancelled, like every other long run.
        let spelunk_sim = cx.new(|_| TaskRunner::<SpelunkRunResult, SpelunkRunInputs>::new());
        let spelunk_sim_subscription =
            cx.subscribe(&spelunk_sim, |state, runner, event: &TaskEvent, cx| {
                match event {
                    TaskEvent::Progress(progress) => {
                        state.status = format!("Running the expedition… {}", progress.message);
                    }
                    TaskEvent::Finished { cancelled: true } => {
                        state.status = if runner.read(cx).result().is_some() {
                            "Expedition run cancelled; the previous result is unchanged.".into()
                        } else {
                            "Expedition run cancelled; nothing was stored.".into()
                        };
                    }
                    TaskEvent::Finished { cancelled: false } => {
                        if let Some(result) = runner.read(cx).result() {
                            state.status = format!(
                                "Expedition run ready: depth {} with seed {}.",
                                result.depth, result.seed
                            );
                        }
                    }
                }
                cx.notify();
            });
        let ladle_state = LadleSectionState::new(window, cx);
        let cglunko_breakdown =
            cx.new(|cx| ui::formula::FormulaTree::new("cglunko-formula", window, cx));
        let clamworks_breakdown =
            cx.new(|cx| ui::formula::FormulaTree::new("clamworks-formula", window, cx));
        let fishing_breakdown =
            cx.new(|cx| ui::formula::FormulaTree::new("fishing-formula", window, cx));
        let mastery_breakdown =
            cx.new(|cx| ui::formula::FormulaTree::new("cooking-mastery-formula", window, cx));
        let fountain_breakdown =
            cx.new(|cx| ui::formula::FormulaTree::new("fountain-formula", window, cx));
        let jar_breakdown = cx.new(|cx| ui::formula::FormulaTree::new("jar-formula", window, cx));
        let opal_breakdown = cx.new(|cx| ui::formula::FormulaTree::new("opal-formula", window, cx));
        let stats_breakdown =
            cx.new(|cx| ui::formula::FormulaTree::new("stats-formula", window, cx));
        let sneaking_breakdown =
            cx.new(|cx| ui::formula::FormulaTree::new("sneaking-formula", window, cx));
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        let session = Session::load(Session::path(state_dir));
        let mut state = Self {
            simulation,
            context: SimContext::default(),
            context_base: SimContext::default(),
            effort: Effort::Balanced,
            target: Target::Hours(24.0),
            options: SimOptions::default(),
            assets: Manifest::load_default(),
            route: Route::Dashboard,
            last_section: None,
            research,
            killroy: KillroyState::default(),
            save: None,
            fleet: Fleet::default(),
            sailing: RouteContext::default(),
            sailing_state: SailingState::default(),
            royal: None,
            royal_state: RoyalState::default(),
            sailing_trees,
            sailing_sim,
            farm: Farm::default(),
            farming: FarmingState::default(),
            farming_name,
            fishing_save: FishingSave::default(),
            fishing: FishingState::default(),
            monument: MonumentState::default(),
            monument_save: None,
            cog: CogState::default(),
            cglunko: Cglunko::default(),
            cglunko_tab: CglunkoTab::default(),
            cglunko_page: CglunkoPage::default(),
            clamworks: Clamworks::default(),
            clamworks_tab: ClamworksTab::default(),
            clamworks_multikill: 0.0,
            clamworks_budget: 0.0,
            clamworks_projection: None,
            clamworks_claim: sections::clamworks::State::new(window, cx),
            endgame: endgame::State::default(),
            jar_tab: JarTab::default(),
            jar_tier: 0,
            jelly: None,
            jelly_state: JellyState::default(),
            jar_breakdown,
            jar: None,
            spelunking: SpelunkingState::default(),
            spelunking_save: None,
            spelunk_sim,
            ladle: None,
            ladle_state,
            mastery: None,
            mastery_plan: MasteryPlanState::default(),
            fountain: None,
            fountain_state: FountainState::default(),
            opal: None,
            opal_selected: None,
            opal_tab: OpalTab::default(),
            opal_target: None,
            opal_total: 0.0,
            sneaking: None,
            sneaking_state: Default::default(),
            sneaking_breakdown,
            pickle: PickleState::default(),
            prisma,
            statue,
            combat,
            stats_all: StatsAllState::default(),
            stats_breakdown,
            drop_rate: DropRateState::new(window, cx),
            coral: CoralState::default(),
            worship: WorshipState::default(),
            worship_inputs: WorshipInputs::default(),
            search,
            monument_search,
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
            coral_run,
            jelly_run,
            jelly_sim_run,
            cog_run,
            cog_perfect_run,
            cglunko_run,
            clamworks_run,
            opal_run,
            worship_plan,
            worship_optimize,
            prisma_run,
            farming_run,
            farming_compare_run,
            breakdown,
            research_trees,
            cglunko_breakdown,
            clamworks_breakdown,
            fishing_breakdown,
            mastery_breakdown,
            fountain_breakdown,
            opal_breakdown,
            shadow: Shadow::default(),
            focus,
            bridge,
            _search_subscription: search_subscription,
            _monument_search_subscription: monument_search_subscription,
            _statue_search_subscription: statue_search_subscription,
            _prisma_search_subscription: prisma_search_subscription,
            _combat_name_subscription: combat_name_subscription,
            _combat_talent_subscription: combat_talent_subscription,
            _combat_overrides_subscription: combat_overrides_subscription,
            _research_preset_subscription: research_preset_subscription,
            _import_subscription: import_subscription,
            _optimizer_subscription: optimizer_subscription,
            shape_run,
            _shape_run_subscription: shape_run_subscription,
            _killroy_run_subscription: killroy_run_subscription,
            _coral_run_subscription: coral_run_subscription,
            _jelly_run_subscription: jelly_run_subscription,
            _jelly_sim_run_subscription: jelly_sim_run_subscription,
            _cog_subscription: cog_subscription,
            _cog_perfect_subscription: cog_perfect_subscription,
            _cglunko_run_subscription: cglunko_run_subscription,
            _clamworks_run_subscription: clamworks_run_subscription,
            _opal_run_subscription: opal_run_subscription,
            _worship_plan_subscription: worship_plan_subscription,
            _worship_optimize_subscription: worship_optimize_subscription,
            _prisma_run_subscription: prisma_run_subscription,
            _farming_run_subscription: farming_run_subscription,
            _farming_compare_subscription: farming_compare_subscription,
            _sailing_sim_subscription: sailing_sim_subscription,
            _spelunk_sim_subscription: spelunk_sim_subscription,
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
                cx,
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
            options: self.options,
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
                self.load_imported(*imported, cx);
            }
            pipeline::Document::Project(project) => {
                crate::project::apply(self, *project, configs, &mut warnings, &mut changes, cx);
                self.save_partial |= !warnings.is_empty();
            }
            pipeline::Document::Section(_) => {
                for config in configs {
                    crate::project::assign(self, config, &mut warnings, cx);
                }
            }
        }
        if self.cog.save != before_cog {
            self.cog_run.update(cx, |runner, _| {
                runner.cancel();
                runner.clear();
            });
            self.cog_perfect_run.update(cx, |runner, _| {
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
    pub(crate) fn load_imported(&mut self, imported: save::Imported, cx: &mut Context<Self>) {
        self.monument_save = Some(monument::Save::from_parts(
            &imported.save,
            imported.companion.as_ref(),
        ));
        let monument_characters = imported.save.character_count();
        if monument_characters == 0 || self.monument.character >= monument_characters {
            self.monument.character = 0;
        }
        // The Justice worksheet transcribes this account's story, so a new save
        // starts it from the new fresh-story stats.
        if let Some(save) = self.monument_save.as_ref() {
            let initial = monument::justice::initial_state(save);
            self.monument.worksheet.reset(initial);
            self.monument.notice.clear();
        }
        self.endgame = endgame::State::audit(&imported);
        self.fishing_save = FishingSave::from_save(&imported.save);
        self.fishing.adopt(&self.fishing_save);
        self.simulation = imported.simulation;
        self.context = imported.context.clone();
        self.context_base = imported.context;
        self.normalize_target();
        let combat_character = if self.combat.character < imported.save.character_count() {
            self.combat.character
        } else {
            0
        };
        let character_count = imported.save.character_count();
        self.combat.load(&imported.save, combat_character);
        self.stats_all
            .load(&imported.save, &self.simulation, &self.context);
        self.drop_rate.load(&imported.save);
        self.save = Some(imported.save);
        self.save_revision = self.save_revision.wrapping_add(1);
        self.prisma.adopt(self.save.as_ref());
        self.statue.load(imported.statues, cx);
        self.coral.adopt(&imported.coral);
        self.worship_adopt(imported.worship);
        self.fleet = imported.fleet;
        self.sailing = imported.sailing;
        self.farm = imported.farming;
        self.farming.adopt(&self.farm);
        self.cog.load(imported.cog);
        self.cglunko = imported.cglunko;
        // A plan from the previous save describes inputs this one no longer has.
        self.cglunko_run.update(cx, |runner, _| {
            runner.cancel();
            runner.clear();
        });
        self.clamworks = imported.clamworks;
        // The plan's default budget follows the new balance; a projection from
        // the previous save no longer describes this one.
        self.clamworks_budget = self.clamworks.pearl_balance;
        self.clamworks_projection = None;
        self.clamworks_claim.load(character_count);
        self.jar = imported.jar;
        self.spelunking_save = imported.spelunking;
        if let Some(save) = self.spelunking_save.as_ref() {
            // Seed from the save alone: a previous save's maximum-stamina
            // overrides must not become the new save's saved stamina.
            let inputs = save.inputs(&idleon_core::spelunking::Account::neutral());
            self.spelunking.adopt(&inputs);
        }
        self.ladle_state
            .adopt(&imported.ladle.clone().unwrap_or_default());
        self.ladle = imported.ladle;
        self.mastery = imported.mastery;
        // The distribution's default budget is the new save's point total.
        self.mastery_plan.adopt();
        self.jelly = imported.jelly;
        if let Some(jelly) = self.jelly.clone() {
            self.jelly_state.adopt(&jelly);
        }
        self.fountain = imported.fountain;
        self.fountain_state.adopt(self.fountain.as_ref());
        self.opal = imported.opal;
        self.opal_selected = None;
        self.opal_target = None;
        // The plan's default total is the new save's invested opals.
        self.opal_total = self.opal.as_ref().map_or(0.0, Opal::total_opals);
        self.royal = imported.royal;
        self.sneaking = imported.sneaking;
        self.research.selected_cell = None;
        self.sailing_state.island = self
            .fleet
            .boats
            .first()
            .and_then(|boat| boat.target_island)
            .filter(|island| *island < self.fleet.available_islands())
            .unwrap_or(0);
        // The simulation has its own destination; it follows the save's island
        // count like the fleet selector does.
        self.sailing_state.sim_island = self
            .sailing_state
            .sim_island
            .min(self.fleet.available_islands().saturating_sub(1));
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
            "Exporting the project ({} bytes) and its state string ({} bytes).",
            self.export.bytes(),
            self.export.state_bytes()
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

    /// Put the `ROv2:` state string on the clipboard, for the upstream tool or
    /// the import dialog's paste box.
    pub fn copy_export_state(&mut self, cx: &mut Context<Self>) {
        crate::ui::export::copy_state(self, cx);
        self.status = format!(
            "Copied the ROv2 state string to the clipboard ({} bytes).",
            self.export.state_bytes()
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

    /// The boat the artifact ranking and the breakdowns read.
    pub fn set_sailing_breakdown_boat(&mut self, boat: usize, cx: &mut Context<Self>) {
        self.sailing_state.breakdown_boat = boat.min(self.fleet.boats.len().saturating_sub(1));
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

    /// The stored expedition run and whether its inputs changed since the run.
    pub fn spelunk_simulation(&self, cx: &App) -> TaskState<SpelunkRunResult> {
        self.spelunk_sim.read(cx).state(&self.spelunk_sim_inputs())
    }

    /// The inputs a run started now would read.
    fn spelunk_sim_inputs(&self) -> SpelunkRunInputs {
        SpelunkRunInputs {
            requested: self.spelunking.run_requested(),
            account: self.spelunking.account.clone(),
            seed: self.spelunking.sim_seed,
            revision: self.save_revision,
        }
    }

    /// The stored pile simulation and whether its inputs changed since the run.
    pub fn sailing_simulation(&self, cx: &App) -> TaskState<SimResult> {
        self.sailing_sim.read(cx).state(&self.sailing_sim_inputs())
    }

    /// The inputs a simulation started now would read.
    fn sailing_sim_inputs(&self) -> SailingSimInputs {
        SailingSimInputs {
            island: self.sailing_state.sim_island,
            goal: self.sailing_state.goal,
            mode: self.sailing_state.mode,
            minimum_tier: self.sailing.minimum_tier,
            hours: self.sailing_state.sim_hours,
            trials: self.sailing_state.sim_trials,
            unending_loot_search: self.sailing.unending_loot_search,
            capacity: self.sailing.chest_capacity,
            revision: self.save_revision,
        }
    }

    /// Select the destination the pile simulation runs on.
    pub fn set_sailing_sim_island(&mut self, island: usize, cx: &mut Context<Self>) {
        let total = self.fleet.available_islands().max(1);
        self.sailing_state.sim_island = island.min(total - 1);
        cx.notify();
    }

    /// Set the interval one simulation trial covers.
    pub fn set_sailing_sim_hours(&mut self, hours: f64, cx: &mut Context<Self>) {
        self.sailing_state.sim_hours = if hours.is_finite() {
            hours.clamp(MIN_HOURS, MAX_HOURS)
        } else {
            DEFAULT_HOURS
        };
        cx.notify();
    }

    /// Set how many independent trials a run makes.
    pub fn set_sailing_sim_trials(&mut self, trials: usize, cx: &mut Context<Self>) {
        self.sailing_state.sim_trials = trials.clamp(MIN_TRIALS, MAX_TRIALS);
        cx.notify();
    }

    /// Run the pile simulation off the UI thread through the shared runner.
    pub fn run_sailing_simulation(&mut self, cx: &mut Context<Self>) {
        let inputs = self.sailing_sim_inputs();
        let plan = self.fleet_plan(inputs.island, inputs.goal);
        if plan.routes.is_empty() {
            self.status = "No active boats are available for the pile simulation.".into();
            cx.notify();
            return;
        }
        let config = SimConfig::from_plan(
            &plan,
            &self.sailing,
            SailingSimOptions {
                hours: inputs.hours,
                trials: inputs.trials,
                seed: DEFAULT_SEED,
            },
        );
        let started = self.sailing_sim.update(cx, |runner, cx| {
            runner.start(cx, inputs, move |_, report| simulate(&config, report))
        });
        if started {
            self.status = format!(
                "Simulating the fleet pile for {}h over {} trials…",
                self.sailing_state.sim_hours, self.sailing_state.sim_trials
            );
            cx.notify();
        }
    }

    /// Ask a running pile simulation to stop; the previous result stays.
    pub fn cancel_sailing_simulation(&mut self, cx: &mut Context<Self>) {
        if self.sailing_sim.update(cx, |runner, _| runner.cancel()) {
            self.status = "Cancelling the fleet pile simulation…".into();
            cx.notify();
        }
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

    /// Switch the opal breakdown between the total and one villager.
    ///
    /// A locked villager (or no save) falls back to the total, so a stale
    /// selection never points at a villager this save does not have.
    pub fn set_opal_villager(&mut self, villager: Option<usize>, cx: &mut Context<Self>) {
        let selected = villager.filter(|villager| {
            self.opal
                .as_ref()
                .is_some_and(|opal| opal.unlocked(*villager))
        });
        if self.opal_selected != selected {
            self.opal_selected = selected;
            cx.notify();
        }
    }

    /// Switch the Opal Optimizer between the total and one villager.
    pub fn set_opal_target(&mut self, villager: Option<usize>, cx: &mut Context<Self>) {
        let target = villager.filter(|villager| {
            self.opal
                .as_ref()
                .is_some_and(|opal| opal.unlocked(*villager))
        });
        if self.opal_target != target {
            self.opal_target = target;
            cx.notify();
        }
    }

    /// Switch the Opal Optimizer's tab.
    pub fn set_opal_tab(&mut self, tab: OpalTab, cx: &mut Context<Self>) {
        if self.opal_tab != tab {
            self.opal_tab = tab;
            cx.notify();
        }
    }

    /// Set the opal total the optimizer splits; whole opals only.
    pub fn set_opal_total(&mut self, total: f64, cx: &mut Context<Self>) {
        let total = if total.is_finite() {
            total.max(0.0).floor()
        } else {
            0.0
        };
        if self.opal_total != total {
            self.opal_total = total;
            cx.notify();
        }
    }

    /// Put the optimizer back on the save's own opal total.
    pub fn use_saved_opal_total(&mut self, cx: &mut Context<Self>) {
        let total = self.opal.as_ref().map_or(0.0, Opal::total_opals);
        self.set_opal_total(total, cx);
    }

    /// The plan's inputs: the selected target, the total and the save's split.
    pub fn opal_plan_input(&self) -> OpalPlanInput {
        let current = self
            .opal
            .as_ref()
            .map_or([0.0; idleon_core::opal::VILLAGERS], Opal::opals);
        let target = match self.opal_target {
            Some(villager)
                if self
                    .opal
                    .as_ref()
                    .is_some_and(|opal| opal.unlocked(villager)) =>
            {
                OpalTarget::Villager(villager)
            }
            _ => OpalTarget::Total,
        };
        OpalPlanInput {
            target,
            total: self.opal_total,
            current,
        }
    }

    /// The plan run's progress, result and staleness.
    pub fn opal_task(&self, cx: &App) -> TaskState<OpalPlan> {
        self.opal_run.read(cx).state(&self.opal_plan_input())
    }

    /// Search the distribution off the UI thread through the shared runner.
    pub fn run_opal_plan(&mut self, cx: &mut Context<Self>) {
        let Some(opal) = self.opal.clone() else {
            self.status = "Opal plan: import a save with villagers first.".into();
            cx.notify();
            return;
        };
        let input = self.opal_plan_input();
        if opal.unlocked_count() == 0 {
            self.status = "Opal plan: no villager is unlocked in this save.".into();
            cx.notify();
            return;
        }
        if input.total <= 0.0 {
            self.status = "Opal plan: set a total above zero.".into();
            cx.notify();
            return;
        }
        let started = self.opal_run.update(cx, |runner, cx| {
            runner.start(cx, input, move |input, report| {
                opal.plan_with(input, report)
            })
        });
        if started {
            self.status = "Optimizing the opal distribution…".into();
            cx.notify();
        }
    }

    /// Ask a running opal plan to stop at its next report.
    pub fn cancel_opal_plan(&mut self, cx: &mut Context<Self>) {
        if self.opal_run.update(cx, |runner, _| runner.cancel()) {
            self.status = "Cancelling the opal plan…".into();
            cx.notify();
        }
    }

    /// Copy the last finished plan as tab-separated text.
    pub fn copy_opal_plan(&mut self, cx: &mut Context<Self>) {
        let Some(plan) = self.opal_run.read(cx).result().cloned() else {
            self.status = "Run the optimizer before copying the plan.".into();
            cx.notify();
            return;
        };
        let Some(opal) = self.opal.as_ref() else {
            self.status = "Opal plan: import a save first.".into();
            cx.notify();
            return;
        };
        let text = plan.text(opal);
        cx.write_to_clipboard(ClipboardItem::new_string(text.clone()));
        self.status = format!(
            "Copied the opal plan to the clipboard ({} bytes).",
            text.len()
        );
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

    /// Apply a ranked lure/line pair from the all-combination table.
    pub fn use_fishing_toolkit(&mut self, lure: usize, line: usize, cx: &mut Context<Self>) {
        self.fishing.use_toolkit(lure, line);
        cx.notify();
    }

    /// Switch the lure/line ranking between Fishing EXP and fish quantity.
    pub fn set_fishing_objective(
        &mut self,
        objective: idleon_core::fishing::Objective,
        cx: &mut Context<Self>,
    ) {
        self.fishing.objective = objective;
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

    /// Set how many Trench depths the projection covers.
    pub fn set_fishing_trench_depths(&mut self, depths: usize, cx: &mut Context<Self>) {
        self.fishing.trench_depths = depths.clamp(1, 25);
        cx.notify();
    }

    /// Switch how the other Trench characters choose their lure and line.
    pub fn set_fishing_helper_mode(
        &mut self,
        mode: idleon_core::fishing::HelperMode,
        cx: &mut Context<Self>,
    ) {
        self.fishing.trench_helper_mode = mode;
        cx.notify();
    }

    /// Tick or untick another character on the Trench helper table.
    pub fn set_fishing_helper(&mut self, index: usize, on: bool, cx: &mut Context<Self>) {
        let characters = self.fishing_save.characters.len();
        self.fishing.set_helper(index, on, characters);
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

    /// Switch the farming tab; the section keeps it across dashboard visits.
    pub fn set_farming_tab(&mut self, tab: crate::sections::farming::Tab, cx: &mut Context<Self>) {
        self.farming.set_tab(tab);
        cx.notify();
    }

    /// Aim the later evolution features at a crop id, clamped to the families.
    pub fn set_target_crop(&mut self, value: f64, cx: &mut Context<Self>) {
        self.farming.set_target_crop(value);
        self.farming.adopt(&self.farm);
        cx.notify();
    }

    /// The inputs a Land Rank allocation started now would use.
    pub fn farming_inputs(&self) -> FarmingRunInputs {
        FarmingRunInputs {
            save_revision: self.save_revision,
            objective: self.farming.objective,
            mode: self.farming.mode,
            target_crop: self.farming.target_crop,
        }
    }

    /// The allocation run: running, progress, result and staleness.
    pub fn farming_task(&self, cx: &App) -> TaskState<FarmingOutcome> {
        self.farming_run.read(cx).state(&self.farming_inputs())
    }

    /// Plan the assignment and the allocation for another objective.
    pub fn set_farming_objective(&mut self, objective: Objective, cx: &mut Context<Self>) {
        self.farming.set_objective(objective);
        self.farming.adopt(&self.farm);
        self.status = format!("Farming objective: {}.", objective.label());
        cx.notify();
    }

    /// Keep the saved Land Rank levels, or reallocate every point.
    pub fn set_farming_mode(&mut self, mode: Mode, cx: &mut Context<Self>) {
        self.farming.set_mode(mode);
        self.status = format!("Land Rank allocation: {}.", mode.label());
        cx.notify();
    }

    /// Spend the account's Land Rank points off the UI thread.
    pub fn run_farming(&mut self, cx: &mut Context<Self>) {
        if !self.save_loaded {
            self.status = "Import a save to allocate Land Rank.".into();
            cx.notify();
            return;
        }
        let inputs = self.farming_inputs();
        let farm = self.farm.clone();
        let (objective, mode, target) = (
            self.farming.objective,
            self.farming.mode,
            Some(self.farming.target_crop),
        );
        let started = self.farming_run.update(cx, |runner, cx| {
            runner.start(cx, inputs, move |_inputs, report| {
                farm.allocation(objective, mode, target, report)
            })
        });
        if started {
            self.status = "Allocating Land Rank…".into();
            cx.notify();
        }
    }

    /// Ask a running allocation to stop; the previous result stays.
    pub fn cancel_farming(&mut self, cx: &mut Context<Self>) {
        if self.farming_run.update(cx, |runner, _| runner.cancel()) {
            self.status = "Cancelling the Land Rank allocation…".into();
            cx.notify();
        }
    }

    /// The inputs a preset comparison started now would use.
    pub fn farming_compare_inputs(&self) -> FarmingCompareInputs {
        FarmingCompareInputs {
            save_revision: self.save_revision,
            mode: self.farming.mode,
            target_crop: self.farming.target_crop,
        }
    }

    /// The comparison run: running, progress, result and staleness.
    pub fn farming_compare_task(&self, cx: &App) -> TaskState<FarmingComparison> {
        self.farming_compare_run
            .read(cx)
            .state(&self.farming_compare_inputs())
    }

    /// Plan every upstream goal from the current baseline off the UI thread.
    pub fn compare_farming(&mut self, cx: &mut Context<Self>) {
        if !self.save_loaded {
            self.status = "Import a save to compare the farming preset goals.".into();
            cx.notify();
            return;
        }
        let inputs = self.farming_compare_inputs();
        let farm = self.farm.clone();
        let (mode, target) = (self.farming.mode, Some(self.farming.target_crop));
        let started = self.farming_compare_run.update(cx, |runner, cx| {
            runner.start(cx, inputs, move |_inputs, report| {
                farm.comparison(mode, target, report)
            })
        });
        if started {
            self.status = "Comparing farming preset goals…".into();
            cx.notify();
        }
    }

    /// Ask a running comparison to stop; the previous table stays.
    pub fn cancel_farming_compare(&mut self, cx: &mut Context<Self>) {
        if self
            .farming_compare_run
            .update(cx, |runner, _| runner.cancel())
        {
            self.status = "Cancelling the preset comparison…".into();
            cx.notify();
        }
    }

    /// Save the preset bar's name against the section's goal, baseline and crop.
    pub fn save_farming_preset(&mut self, cx: &mut Context<Self>) {
        let name = self.farming_name.read(cx).text().trim().to_string();
        match self.farming.save_preset(&name) {
            Ok(message) => {
                self.farming_name
                    .update(cx, |input, cx| input.set_text("", cx));
                self.status = message;
            }
            Err(message) => self.status = message,
        }
        cx.notify();
    }

    /// Apply a saved farming preset to the section's inputs.
    pub fn apply_farming_preset(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(name) = self.farming.apply_preset(index) {
            self.farming.adopt(&self.farm);
            self.status = format!("Applied the farming preset “{name}”.");
        }
        cx.notify();
    }

    /// Forget one saved farming preset.
    pub fn delete_farming_preset(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(preset) = self.farming.delete_preset(index) {
            self.status = format!("Deleted the farming preset “{}”.", preset.name);
        }
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

    /// The world the Royal Guardian tables list.
    pub fn set_royal_world(&mut self, world: usize, cx: &mut Context<Self>) {
        self.royal_state.world = world.min(idleon_core::royal_guardian::WORLDS - 1);
        self.royal_state.outpost = None;
        cx.notify();
    }

    /// The outpost whose next range target the panel explains.
    pub fn set_royal_outpost(&mut self, map: usize, cx: &mut Context<Self>) {
        self.royal_state.outpost = Some(map);
        cx.notify();
    }

    pub fn set_jar_tab(&mut self, tab: JarTab, cx: &mut Context<Self>) {
        self.jar_tab = tab;
        cx.notify();
    }

    /// Switch the tier the rupie value tab projects, clamped to the six tiers.
    pub fn set_jar_tier(&mut self, tier: u32, cx: &mut Context<Self>) {
        self.jar_tier = tier.min(idleon_core::jar::INCOME_TIERS - 1);
        cx.notify();
    }

    /// Switch the character whose Monument charge and warnings are read.
    pub fn set_monument_character(&mut self, character: usize, cx: &mut Context<Self>) {
        let count = self
            .save
            .as_ref()
            .map_or(0, idleon_core::save::Save::character_count);
        self.monument.character = if count == 0 {
            0
        } else {
            character.min(count - 1)
        };
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
            .map(|save| monument::snapshot(save, self.monument.character))
    }

    /// The Wisdom snapshot for the imported save, if any.
    pub fn monument_wisdom(&self) -> Option<monument::WisdomSnapshot> {
        self.monument_save
            .as_ref()
            .map(|save| monument::wisdom_snapshot(save, self.monument.character))
    }

    /// The Justice snapshot for the imported save, if any.
    pub fn monument_justice(&self) -> Option<monument::JusticeSnapshot> {
        self.monument_save
            .as_ref()
            .map(|save| monument::justice_snapshot(save, self.monument.character))
    }

    /// Write one Justice stat, clamped like its input, as one undoable step.
    pub fn set_monument_justice_field(
        &mut self,
        field: JusticeField,
        value: f64,
        cx: &mut Context<Self>,
    ) {
        if !value.is_finite() {
            return;
        }
        let mut next = self.monument.worksheet.state.clone();
        field.set(&mut next, value);
        self.monument.worksheet.edit(next);
        self.monument.notice.clear();
        cx.notify();
    }

    /// Add or remove one deal, promise or investment as one undoable step.
    pub fn toggle_monument_justice_effect(
        &mut self,
        effect: JusticeEffect,
        on: bool,
        cx: &mut Context<Self>,
    ) {
        let mut next = self.monument.worksheet.state.clone();
        if on {
            next.effects.entry(effect).or_insert(0);
        } else {
            next.effects.remove(&effect);
        }
        self.monument.worksheet.edit(next);
        self.monument.notice.clear();
        cx.notify();
    }

    /// Change an active effect's value as one undoable step.
    pub fn set_monument_justice_effect(
        &mut self,
        effect: JusticeEffect,
        value: f64,
        cx: &mut Context<Self>,
    ) {
        if !value.is_finite() {
            return;
        }
        let mut next = self.monument.worksheet.state.clone();
        if let Some(slot) = next.effects.get_mut(&effect) {
            *slot = value.round().max(0.0) as i64;
            self.monument.worksheet.edit(next);
            self.monument.notice.clear();
            cx.notify();
        }
    }

    /// Choose the case the player actually received; not an undoable step.
    pub fn select_monument_justice_case(&mut self, id: usize, cx: &mut Context<Self>) {
        self.monument.worksheet.select(id);
        self.monument.notice.clear();
        cx.notify();
    }

    /// Record the response and outcome that happened, and wait for the next case.
    pub fn record_monument_justice(
        &mut self,
        action: JusticeAction,
        outcome: usize,
        cx: &mut Context<Self>,
    ) {
        let worksheet = &self.monument.worksheet;
        let Ok(report) = monument::justice::responses(&worksheet.state, worksheet.case_id) else {
            return;
        };
        let Some(chosen) = report
            .responses
            .iter()
            .find(|response| response.action == action)
            .and_then(|response| response.outcomes.get(outcome))
        else {
            return;
        };
        let next = chosen.state.clone();
        let ended = next.health <= 0;
        self.monument.worksheet.record(next);
        self.monument.notice = if ended {
            "Story ended. Fresh story starts another worksheet."
        } else {
            "Response recorded. Select the next case, then open it to settle delayed effects."
        }
        .into();
        cx.notify();
    }

    /// Open the selected case once, settling the delayed effects it triggers.
    pub fn open_monument_justice(&mut self, cx: &mut Context<Self>) {
        self.monument.notice = match self.monument.worksheet.open_next() {
            Ok(notes) if notes.is_empty() => {
                "Next case opened; no delayed payments this turn.".to_string()
            }
            Ok(notes) => notes.join(" "),
            Err(error) => error.to_string(),
        };
        cx.notify();
    }

    /// Walk one worksheet step back.
    pub fn undo_monument_justice(&mut self, cx: &mut Context<Self>) {
        if self.monument.worksheet.undo() {
            self.monument.notice = "Worksheet step undone.".into();
            cx.notify();
        }
    }

    /// Start the worksheet over from the loaded save's fresh-story stats.
    pub fn reset_monument_justice(&mut self, cx: &mut Context<Self>) {
        let Some(save) = self.monument_save.as_ref() else {
            return;
        };
        let initial = monument::justice::initial_state(save);
        self.monument.worksheet.reset(initial);
        self.monument.notice =
            "Fresh-story stats loaded. Select the first visitor you actually received.".into();
        cx.notify();
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

    pub fn set_cglunko_tab(&mut self, tab: CglunkoTab, cx: &mut Context<Self>) {
        self.cglunko_tab = tab;
        cx.notify();
    }

    /// Pick the page the upgrade list and the plan work on. A stored plan is
    /// marked stale by the runner, not dropped, so the previous numbers stay
    /// readable until the next run.
    pub fn set_cglunko_page(&mut self, page: CglunkoPage, cx: &mut Context<Self>) {
        self.cglunko_page = page;
        cx.notify();
    }

    /// The plan run's progress, result and staleness for the selected page.
    pub fn cglunko_task(&self, cx: &App) -> TaskState<CglunkoPlan> {
        self.cglunko_run.read(cx).state(&self.cglunko_page)
    }

    /// Plan the selected page's purchases off the UI thread through the shared
    /// runner.
    pub fn run_cglunko_plan(&mut self, cx: &mut Context<Self>) {
        if !self.save_loaded {
            self.status = "Cglunko plan: import a save first.".into();
            cx.notify();
            return;
        }
        let page = self.cglunko_page;
        let cglunko = self.cglunko.clone();
        let started = self.cglunko_run.update(cx, |runner, cx| {
            runner.start(cx, page, move |page, report| {
                cglunko.plan_with(page, report)
            })
        });
        if started {
            self.status = format!("Planning Cglunko {} upgrades…", page.label());
            cx.notify();
        }
    }

    /// Ask a running plan to stop at its next purchase.
    pub fn cancel_cglunko_plan(&mut self, cx: &mut Context<Self>) {
        if self.cglunko_run.update(cx, |runner, _| runner.cancel()) {
            self.status = "Cancelling the Cglunko plan…".into();
            cx.notify();
        }
    }

    pub fn set_clamworks_tab(&mut self, tab: ClamworksTab, cx: &mut Context<Self>) {
        self.clamworks_tab = tab;
        cx.notify();
    }

    /// The desired currency the fountain currency tab simulates.
    pub fn fountain_desired(&self) -> usize {
        self.fountain_state
            .desired
            .or_else(|| self.fountain.as_ref().map(Fountain::desired_currency))
            .unwrap_or(0)
    }

    pub fn set_fountain_tab(&mut self, tab: sections::fountain::Tab, cx: &mut Context<Self>) {
        self.fountain_state.tab = tab;
        cx.notify();
    }

    pub fn set_fountain_water(&mut self, water: usize, cx: &mut Context<Self>) {
        self.fountain_state.water = water.min(idleon_core::fountain::WATERS - 1);
        cx.notify();
    }

    /// Simulate a desired currency; `None` returns to the save's choice.
    pub fn set_fountain_desired(&mut self, desired: Option<usize>, cx: &mut Context<Self>) {
        self.fountain_state.desired =
            desired.filter(|currency| *currency < idleon_core::fountain::CURRENCIES);
        cx.notify();
    }

    /// Pick the upgrade the upgrade tab's cost curve plots.
    pub fn set_fountain_upgrade(&mut self, upgrade: Option<usize>, cx: &mut Context<Self>) {
        self.fountain_state.upgrade = upgrade;
        cx.notify();
    }

    pub fn set_clamworks_multikill(&mut self, multikill: f64, cx: &mut Context<Self>) {
        self.clamworks_multikill = multikill.max(0.0);
        cx.notify();
    }

    /// The Clamworks levels the section shows: the applied plan's, or the save's.
    pub fn clamworks_view(&self) -> Clamworks {
        let Some(levels) = self.clamworks_projection else {
            return self.clamworks.clone();
        };
        Clamworks {
            upgrades: levels,
            ..self.clamworks.clone()
        }
    }

    /// The inputs a Clamworks plan run started now would use.
    pub fn clamworks_plan_input(&self) -> ClamworksPlanInput {
        ClamworksPlanInput {
            budget: self.clamworks_budget,
            multikill_total: self.clamworks_multikill,
        }
    }

    /// The plan run's progress, result and staleness.
    pub fn clamworks_task(&self, cx: &App) -> TaskState<ClamworksPlan> {
        self.clamworks_run
            .read(cx)
            .state(&self.clamworks_plan_input())
    }

    /// Set the pearls the plan may spend; the saved balance until it is changed.
    pub fn set_clamworks_budget(&mut self, budget: f64, cx: &mut Context<Self>) {
        self.clamworks_budget = if budget.is_finite() {
            budget.max(0.0)
        } else {
            0.0
        };
        cx.notify();
    }

    /// Search the upgrade plans off the UI thread through the shared runner.
    pub fn run_clamworks_plan(&mut self, cx: &mut Context<Self>) {
        let input = self.clamworks_plan_input();
        let clamworks = self.clamworks_view();
        if input.budget <= 0.0 {
            self.status = "Clamworks plan: set a pearl budget above zero.".into();
            cx.notify();
            return;
        }
        let started = self.clamworks_run.update(cx, |runner, cx| {
            runner.start(cx, input, move |input, report| {
                clamworks.plan_with(input, report)
            })
        });
        if started {
            self.status = "Planning Clamworks upgrades…".into();
            cx.notify();
        }
    }

    /// Ask a running plan to stop at its next purchase.
    pub fn cancel_clamworks_plan(&mut self, cx: &mut Context<Self>) {
        if self.clamworks_run.update(cx, |runner, _| runner.cancel()) {
            self.status = "Cancelling the Clamworks plan…".into();
            cx.notify();
        }
    }

    /// Preview the plan's levels across the section, as applying a layout does.
    pub fn apply_clamworks_plan(&mut self, cx: &mut Context<Self>) {
        let Some(plan) = self
            .clamworks_run
            .update(cx, |runner, _| runner.take_result())
        else {
            return;
        };
        self.clamworks_projection = Some(plan.end.upgrades);
        self.status = format!(
            "Clamworks plan applied: {} purchases, {:.2}× pearls per pearl.",
            plan.steps.len(),
            plan.pearl_value_after()
        );
        cx.notify();
    }

    /// Drop the stored plan without previewing it.
    pub fn dismiss_clamworks_plan(&mut self, cx: &mut Context<Self>) {
        self.clamworks_run.update(cx, |runner, _| runner.clear());
        self.status = "Clamworks plan dismissed.".into();
        cx.notify();
    }

    /// Go back to the saved upgrade levels after previewing a plan.
    pub fn show_saved_clamworks_levels(&mut self, cx: &mut Context<Self>) {
        if self.clamworks_projection.take().is_some() {
            self.status = "Clamworks: showing the saved upgrade levels.".into();
            cx.notify();
        }
    }

    /// Pick the character the AFK claim and its comparison run for.
    pub fn set_clamworks_character(&mut self, character: usize, cx: &mut Context<Self>) {
        let count = self.save.as_ref().map_or(0, |save| save.character_count());
        self.clamworks_claim.set_character(character, count);
        cx.notify();
    }

    pub fn set_clamworks_phase(
        &mut self,
        phase: idleon_core::clamworks::afk::Phase,
        cx: &mut Context<Self>,
    ) {
        self.clamworks_claim.set_phase(phase);
        cx.notify();
    }

    /// The selected character's kills per hour, the combat simulator's stand-in.
    pub fn set_clamworks_kills_per_hour(&mut self, kills: f64, cx: &mut Context<Self>) {
        let character = self.clamworks_claim.character;
        self.clamworks_claim
            .set_stand_in(character, |stand_in| stand_in.kills_per_hour = kills);
        cx.notify();
    }

    /// The selected character's drop-rate multiplier, the same stand-in.
    pub fn set_clamworks_drop_rate(&mut self, drop_rate: f64, cx: &mut Context<Self>) {
        let character = self.clamworks_claim.character;
        self.clamworks_claim
            .set_stand_in(character, |stand_in| stand_in.drop_rate = drop_rate);
        cx.notify();
    }

    /// Read the typed hours and let the claim accept or refuse them.
    pub fn calculate_clamworks_claim(&mut self, cx: &mut Context<Self>) {
        let text = self.clamworks_claim.hours_text(cx);
        self.clamworks_claim.commit_hours(&text);
        if let Some(error) = self.clamworks_claim.error {
            self.status = format!("Clamworks claim: {}", error.message());
        } else {
            let multikill = self.clamworks_multikill;
            self.status = match self
                .clamworks_view()
                .afk_claim(self.clamworks_claim.inputs(), multikill)
            {
                Ok(claim) => format!(
                    "Clamworks claim: {} pearls, {} drop rolls.",
                    crate::ui::format::number(claim.expected_pearls),
                    claim.rolls
                ),
                Err(error) => format!("Clamworks claim: {}", error.message()),
            };
        }
        cx.notify();
    }
    /// The cog optimizer's run: running, progress, result and staleness.
    pub fn cog_task(&self, cx: &App) -> TaskState<CogResult> {
        self.cog_run.read(cx).state(&self.cog.inputs())
    }

    /// The perfect board's run: running, progress, result and staleness.
    pub fn cog_perfect_task(&self, cx: &App) -> TaskState<PerfectBoard> {
        self.cog_perfect_run
            .read(cx)
            .state(&self.cog.perfect_inputs())
    }

    /// What the shared Jelly estimate runner shows against the current inputs.
    pub fn jelly_task(&self, cx: &App) -> TaskState<JellyEstimate> {
        let inputs = self.jelly_state.inputs(self.jelly.as_ref());
        self.jelly_run.read(cx).state(&inputs)
    }

    /// Switch the Jelly Operator tab.
    pub fn set_jelly_tab(&mut self, tab: JellyTab, cx: &mut Context<Self>) {
        if self.jelly_state.tab != tab {
            self.jelly_state.tab = tab;
            cx.notify();
        }
    }

    /// Pick the palette cell type the next board click places.
    pub fn jelly_select(&mut self, cell_type: usize, cx: &mut Context<Self>) {
        self.jelly_state.select(cell_type);
        let message = self.jelly_state.message.clone();
        self.status = if message.is_empty() {
            format!(
                "Jelly palette: {}.",
                idleon_core::jelly::data().cell_name(cell_type)
            )
        } else {
            message
        };
        cx.notify();
    }

    /// Place the picked cell on the slot, or remove the cell that covers it.
    pub fn jelly_click_slot(&mut self, anchor: usize, cx: &mut Context<Self>) {
        let Some(jelly) = self.jelly.clone() else {
            return;
        };
        let layout = self.jelly_state.layout.clone();
        if idleon_core::jelly::board::occupancy(&layout).contains_key(&anchor) {
            self.jelly_state.remove_at(&layout, anchor);
        } else {
            self.jelly_state.place(&jelly, anchor);
        }
        self.status = self.jelly_state.message.clone();
        cx.notify();
    }

    /// Put the board back to the save's own layout.
    pub fn jelly_reset_layout(&mut self, cx: &mut Context<Self>) {
        let Some(jelly) = self.jelly.clone() else {
            return;
        };
        self.jelly_state.reset(&jelly);
        self.status = self.jelly_state.message.clone();
        cx.notify();
    }

    /// How many seeded trials an estimate runs.
    pub fn set_jelly_trials(&mut self, value: f64, cx: &mut Context<Self>) {
        let trials = value.clamp(1.0, 1024.0).round() as u32;
        if self.jelly_state.trials != trials {
            self.jelly_state.trials = trials;
            cx.notify();
        }
    }

    /// The seed the trials are derived from.
    pub fn set_jelly_seed(&mut self, value: f64, cx: &mut Context<Self>) {
        let seed = if value.is_finite() {
            value.clamp(0.0, 4_294_967_295.0) as u32
        } else {
            1
        };
        if self.jelly_state.seed != seed {
            self.jelly_state.seed = seed;
            cx.notify();
        }
    }

    /// Run the operation estimate off the UI thread through the shared runner.
    pub fn run_jelly_estimate(&mut self, cx: &mut Context<Self>) {
        let inputs = self.jelly_state.inputs(self.jelly.as_ref());
        let Some(jelly) = inputs.jelly.clone() else {
            self.status = "Jelly estimate: import a save with a board first.".into();
            cx.notify();
            return;
        };
        let trials = inputs.trials;
        let layout = inputs.layout.clone();
        let seed = inputs.seed;
        let started = self.jelly_run.update(cx, |runner, cx| {
            runner.start(cx, inputs, move |_inputs, report| {
                JellyEstimate::run_with(&jelly, &layout, trials, seed, |done| {
                    report(Progress {
                        fraction: done as f32 / trials as f32,
                        message: format!("Trial {done} of {trials}"),
                    }) == Control::Continue
                })
            })
        });
        if started {
            self.status = format!("Estimating the Jelly operation over {trials} trials…");
            cx.notify();
        }
    }

    /// Ask a running estimate to stop at its next trial boundary.
    pub fn cancel_jelly_estimate(&mut self, cx: &mut Context<Self>) {
        if self.jelly_run.update(cx, |runner, _| runner.cancel()) {
            self.status = "Cancelling the Jelly estimate…".into();
            cx.notify();
        }
    }

    /// What the shared Jelly simulator runner shows against the sandbox.
    pub fn jelly_sim_task(&self, cx: &App) -> TaskState<JellyTrial> {
        let inputs = self.jelly_state.simulator.inputs(self.jelly.as_ref());
        self.jelly_sim_run.read(cx).state(&inputs)
    }

    /// Apply one sandbox edit, publish its message, and let the runner notice
    /// the new inputs.
    fn jelly_sim_edit(
        &mut self,
        cx: &mut Context<Self>,
        edit: impl FnOnce(&mut JellySimulator, &Jelly),
    ) {
        let Some(jelly) = self.jelly.clone() else {
            return;
        };
        edit(&mut self.jelly_state.simulator, &jelly);
        self.status = self.jelly_state.simulator.message.clone();
        cx.notify();
    }

    /// The simulator's stage select.
    pub fn set_jelly_sim_stage(&mut self, stage: usize, cx: &mut Context<Self>) {
        self.jelly_sim_edit(cx, |simulator, jelly| simulator.set_stage(jelly, stage));
    }

    /// The simulator's Fever select.
    pub fn set_jelly_sim_fever(&mut self, fever: u32, cx: &mut Context<Self>) {
        self.jelly_sim_edit(cx, |simulator, jelly| simulator.set_fever(jelly, fever));
    }

    /// Stronkroid's activation second; a negative value disables it.
    pub fn set_jelly_sim_roid(&mut self, seconds: f64, cx: &mut Context<Self>) {
        self.jelly_sim_edit(cx, |simulator, _| simulator.set_roid(seconds));
    }

    /// The simulator's automatic revive policy.
    pub fn set_jelly_sim_revive(&mut self, revive: RevivePolicy, cx: &mut Context<Self>) {
        self.jelly_sim_edit(cx, |simulator, _| simulator.set_revive(revive));
    }

    /// The seed one simulator attempt is drawn from.
    pub fn set_jelly_sim_seed(&mut self, seed: f64, cx: &mut Context<Self>) {
        self.jelly_sim_edit(cx, |simulator, _| simulator.set_seed(seed));
    }

    /// Pick the cell the next sandbox board click places.
    pub fn jelly_sim_select(&mut self, cell_type: usize, cx: &mut Context<Self>) {
        self.jelly_sim_edit(cx, |simulator, _| simulator.select(cell_type));
    }

    /// Remove mode: the next sandbox board click removes what it covers.
    pub fn jelly_sim_remove_mode(&mut self, cx: &mut Context<Self>) {
        self.jelly_sim_edit(cx, |simulator, _| simulator.remove_mode());
    }

    /// Place the picked cell on the sandbox slot, or remove the cell there.
    pub fn jelly_sim_click_slot(&mut self, slot: usize, cx: &mut Context<Self>) {
        self.jelly_sim_edit(cx, |simulator, jelly| simulator.click(jelly, slot));
    }

    /// Empty the sandbox board.
    pub fn clear_jelly_simulator(&mut self, cx: &mut Context<Self>) {
        self.jelly_sim_edit(cx, |simulator, _| simulator.clear_board());
    }

    /// Put the sandbox back to the save's own board and knobs.
    pub fn reset_jelly_simulator(&mut self, cx: &mut Context<Self>) {
        self.jelly_sim_edit(cx, |simulator, jelly| simulator.reset(jelly));
    }

    /// Restore the save's purchased board sections in the sandbox.
    pub fn jelly_sim_use_save_sections(&mut self, cx: &mut Context<Self>) {
        self.jelly_sim_edit(cx, |simulator, jelly| simulator.use_save_sections(jelly));
    }

    /// Lock every purchased board section in the sandbox.
    pub fn jelly_sim_lock_sections(&mut self, cx: &mut Context<Self>) {
        self.jelly_sim_edit(cx, |simulator, jelly| simulator.lock_sections(jelly));
    }

    /// Unlock every board section in the sandbox.
    pub fn jelly_sim_unlock_all_sections(&mut self, cx: &mut Context<Self>) {
        self.jelly_sim_edit(cx, |simulator, jelly| simulator.unlock_all_sections(jelly));
    }

    /// Apply the whole board-section selection the sandbox toggle group reports.
    pub fn jelly_sim_set_plots(&mut self, plots: &[usize], cx: &mut Context<Self>) {
        self.jelly_sim_edit(cx, |simulator, jelly| simulator.set_plots(jelly, plots));
    }

    /// Run one seeded operation of the sandbox board off the UI thread.
    pub fn run_jelly_simulation(&mut self, cx: &mut Context<Self>) {
        let Some(jelly) = self.jelly.clone() else {
            self.status = "Jelly simulation: import a save with a board first.".into();
            cx.notify();
            return;
        };
        let simulator = &self.jelly_state.simulator;
        let sandbox = simulator.sandbox(&jelly);
        let layout = simulator.layout.clone();
        if let Err(reason) = idleon_core::jelly::board::validate(&sandbox, &layout) {
            let simulator = &mut self.jelly_state.simulator;
            simulator.message = format!("The sandbox board is not valid: {reason}");
            simulator.message_bad = true;
            self.status = simulator.message.clone();
            cx.notify();
            return;
        }
        let inputs = simulator.inputs(Some(&jelly));
        let options = simulator.options();
        let started = self.jelly_sim_run.update(cx, |runner, cx| {
            runner.start(cx, inputs, move |_inputs, report| {
                let _ = report(Progress {
                    fraction: 0.0,
                    message: "One seeded 60 FPS operation…".into(),
                });
                Some(simulate_jelly_operation(&sandbox, &layout, options))
            })
        });
        if started {
            self.status = "Simulating one Jelly operation…".into();
            cx.notify();
        }
    }

    /// Ask a running simulation to stop at its next report.
    pub fn cancel_jelly_simulation(&mut self, cx: &mut Context<Self>) {
        if self.jelly_sim_run.update(cx, |runner, _| runner.cancel()) {
            self.status = "Cancelling the Jelly simulation…".into();
            cx.notify();
        }
    }

    /// Change the objective; a stored result becomes stale until it reruns.
    pub fn set_cog_goal(&mut self, goal: CogGoal, cx: &mut Context<Self>) {
        if self.cog.goal != goal {
            self.cog.goal = goal;
            self.status = format!("Cog objective: {}.", goal.label());
            cx.notify();
        }
    }

    /// Turn the custom weights on or off; a stored result becomes stale.
    pub fn set_cog_weighted(&mut self, weighted: bool, cx: &mut Context<Self>) {
        if self.cog.weighted != weighted {
            self.cog.weighted = weighted;
            self.status = if weighted {
                "Cog objective: custom weights.".into()
            } else {
                format!("Cog objective: {}.", self.cog.goal.label())
            };
            cx.notify();
        }
    }

    /// Replace one custom weight; the controls clamp it to 0..=100.
    pub fn set_cog_weight(&mut self, goal: CogGoal, value: f64, cx: &mut Context<Self>) {
        let mut weights = self.cog.weights;
        weights.set(goal, value);
        if self.cog.weights != weights {
            self.cog.weights = weights;
            self.status = match self.cog.weight_problem() {
                Some(problem) => format!("Cog weights: {problem}"),
                None => format!("Cog {} weight: {:.0}.", goal.label(), value),
            };
            cx.notify();
        }
    }

    /// Optimize the board off the UI thread through the shared task runner.
    pub fn optimize_cog(&mut self, cx: &mut Context<Self>) {
        if let Some(problem) = self.cog.weight_problem() {
            self.status = format!("Cog weights: {problem}");
            cx.notify();
            return;
        }
        self.cog.dismiss_walkthrough();
        self.cog.show_perfect = false;
        let inputs = self.cog.inputs();
        if inputs.board.iter().all(Option::is_none) {
            self.status = "Cog board: no cogs to place.".into();
            cx.notify();
            return;
        }
        self.cog_perfect_run.update(cx, |runner, _| {
            runner.cancel();
        });
        let started = self.cog_run.update(cx, |runner, cx| {
            runner.start(cx, inputs, move |inputs, report| {
                optimize_cog_board(
                    &inputs.board,
                    &inputs.shelf,
                    &inputs.objective,
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

    /// Apply the next swap in the open walkthrough.
    pub fn next_cog_swap(&mut self, cx: &mut Context<Self>) {
        if !self.cog.next_swap() {
            return;
        }
        let (applied, total, complete) =
            self.cog
                .walkthrough
                .as_ref()
                .map_or((0, 0, false), |walkthrough| {
                    (
                        walkthrough.applied(),
                        walkthrough.total(),
                        walkthrough.is_complete(),
                    )
                });
        self.status = if complete {
            format!("Cog walkthrough: all steps shown ({total}); press Done to keep the board.")
        } else {
            format!("Cog walkthrough: step {} of {total}.", applied + 1)
        };
        cx.notify();
    }

    /// Restore the board before the last applied swap.
    pub fn previous_cog_swap(&mut self, cx: &mut Context<Self>) {
        if !self.cog.previous_swap() {
            return;
        }
        let (applied, total) = self.cog.walkthrough.as_ref().map_or((0, 0), |walkthrough| {
            (walkthrough.applied(), walkthrough.total())
        });
        self.status = if applied == 0 {
            format!("Cog walkthrough: back at the run's start board; step 1 of {total}.")
        } else {
            format!("Cog walkthrough: step {} of {total}.", applied + 1)
        };
        cx.notify();
    }

    /// Keep the optimized board the walkthrough ends on.
    pub fn finish_cog_swaps(&mut self, cx: &mut Context<Self>) {
        // Done changes the board itself, so the stored result's inputs must move
        // with it; otherwise the run would read as stale straight after Done. A
        // genuinely stale result (a goal edit, say) stays stale.
        let stale = self.cog_task(cx).stale;
        if !self.cog.finish_walkthrough() {
            return;
        }
        if !stale {
            let inputs = self.cog.inputs();
            self.cog_run
                .update(cx, |runner, _| runner.commit_inputs(inputs));
        }
        self.status = "Cog walkthrough: final board kept; Reset restores the save board.".into();
        cx.notify();
    }

    /// Ask a running cog optimization to stop at its next report.
    pub fn cancel_cog(&mut self, cx: &mut Context<Self>) {
        if self.cog_run.update(cx, |runner, _| runner.cancel()) {
            self.status = "Cancelling the cog optimization…".into();
            cx.notify();
        }
    }

    /// Build the perfect board off the UI thread. It needs a player cog on the
    /// save board, which the section's button checks before calling this.
    pub fn perfect_cog(&mut self, cx: &mut Context<Self>) {
        if idleon_core::cog::player_slot(&self.cog.save.board).is_none() {
            self.status = "Perfect board needs a player cog on the save board.".into();
            cx.notify();
            return;
        }
        self.cog.dismiss_walkthrough();
        self.cog.show_perfect = true;
        self.cog_run.update(cx, |runner, _| {
            runner.cancel();
        });
        let inputs = self.cog.perfect_inputs();
        let started = self.cog_perfect_run.update(cx, |runner, cx| {
            runner.start(cx, inputs, move |inputs, report| {
                perfect_cog_board(
                    &inputs.board,
                    &inputs.shelf,
                    inputs.construction_level,
                    inputs.goal,
                    report,
                )
            })
        });
        if started {
            self.status = "Building the perfect cog board…".into();
            cx.notify();
        }
    }

    /// Ask a running perfect-board build to stop at its next report.
    pub fn cancel_cog_perfect(&mut self, cx: &mut Context<Self>) {
        if self.cog_perfect_run.update(cx, |runner, _| runner.cancel()) {
            self.status = "Cancelling the perfect board…".into();
            cx.notify();
        }
    }

    /// Restore the save board and drop both stored results.
    pub fn reset_cog(&mut self, cx: &mut Context<Self>) {
        self.cog.reset();
        self.cog_run.update(cx, |runner, _| {
            runner.cancel();
            runner.clear();
        });
        self.cog_perfect_run.update(cx, |runner, _| {
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

    /// Switch the breakdown tab and select the first row of the new category.
    pub fn set_stats_category(
        &mut self,
        category: idleon_core::stats::BreakdownCategory,
        cx: &mut Context<Self>,
    ) {
        self.stats_all.category = category;
        self.stats_all.select_first();
        cx.notify();
    }

    /// Switch the breakdown row the formula tree shows.
    pub fn set_stats_descriptor(&mut self, id: String, cx: &mut Context<Self>) {
        if self
            .stats_all
            .sheet
            .as_ref()
            .is_some_and(|sheet| sheet.get(&id).is_some())
        {
            self.stats_all.selected = Some(id);
            cx.notify();
        }
    }

    /// Re-resolve the breakdowns from the save and the context.
    pub(crate) fn rebuild_stats_breakdowns(&mut self) {
        let Some(save) = self.save.as_ref() else {
            return;
        };
        self.stats_all
            .rebuild(save, &self.simulation, &self.context);
    }

    /// A context change refreshes every value the page derives from it.
    fn stats_context_changed(&mut self, cx: &mut Context<Self>) {
        self.rebuild_stats_breakdowns();
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

    /// Pick a character and map for the drop-rate page, exactly like upstream
    /// `switchChar`/`switchMap`.
    pub fn set_drop_rate_character(&mut self, character: usize, cx: &mut Context<Self>) {
        let Some(snapshot) = self.drop_rate.snapshot.as_ref() else {
            return;
        };
        let mut selection = self.drop_rate.selection;
        selection.select_character(character, snapshot);
        self.drop_rate.selection = selection;
        self.drop_rate_changed(cx);
    }

    /// Pick the drop-rate page's map from the selector's rows.
    pub fn set_drop_rate_map(&mut self, map: usize, cx: &mut Context<Self>) {
        let Some(snapshot) = self.drop_rate.snapshot.as_ref() else {
            return;
        };
        let mut selection = self.drop_rate.selection;
        selection.select_map(map, snapshot);
        self.drop_rate.selection = selection;
        self.drop_rate_changed(cx);
    }

    /// A context change refreshes the total the page derives from it.
    fn drop_rate_changed(&mut self, cx: &mut Context<Self>) {
        let selection = self.drop_rate.selection;
        let character = self
            .drop_rate
            .snapshot
            .as_ref()
            .map(|snapshot| snapshot.name(selection.character))
            .unwrap_or_default();
        let map = idleon_core::stats::classify(selection.map);
        let run = if idleon_core::drop_rate::is_dungeon_map(selection.map) {
            " · live dungeon inputs"
        } else {
            ""
        };
        self.status = if character.is_empty() {
            "Load a save to resolve the drop rate.".into()
        } else {
            format!("Drop rate: {character} · {}{run}", map.name)
        };
        cx.notify();
    }

    /// Pick a character for the statue table; characters without saved rows
    /// cannot be selected, so the table always has a valid subject.
    pub fn set_statue_character(&mut self, character: usize, cx: &mut Context<Self>) {
        if self.statue.save.has_character(character) {
            self.statue.select_character(character, cx);
            cx.notify();
        }
    }

    pub fn set_statue_sort(&mut self, sort: StatueSort, cx: &mut Context<Self>) {
        self.statue.sort = sort;
        cx.notify();
    }

    /// Switch the statue table between guaranteed and average deposit estimates.
    pub fn set_statue_estimate(&mut self, estimate: StatueEstimate, cx: &mut Context<Self>) {
        self.statue.estimate = estimate;
        cx.notify();
    }

    /// Set one statue's target level. A hand edit ends the batch undo step and
    /// the status that described it.
    pub fn set_statue_target(&mut self, statue: usize, target: u32, cx: &mut Context<Self>) {
        self.statue.set_target(statue, target);
        self.statue.batch_status = None;
        cx.notify();
    }

    /// Set every statue's target to the level in the batch entry, or show why
    /// the plan left it alone.
    pub fn set_statue_targets_all(&mut self, cx: &mut Context<Self>) {
        let text = self.statue.batch_input.read(cx).text().to_string();
        match self
            .statue
            .plan
            .set_all(&self.statue.save, self.statue.character, &text)
        {
            Ok(level) => {
                self.statue.batch_error = None;
                self.statue.batch_status = Some(format!(
                    "All {} targets set to Lv {level}.",
                    idleon_core::statue::count()
                ));
            }
            Err(error) => self.statue.batch_error = Some(error.message()),
        }
        cx.notify();
    }

    /// Next level for all: clear the entered targets, so every row follows its
    /// saved level plus one.
    pub fn set_statue_next_level(&mut self, cx: &mut Context<Self>) {
        self.statue.plan.next_level();
        self.statue.batch_error = None;
        self.statue.batch_status = Some("Targets follow each saved level plus one.".to_string());
        cx.notify();
    }

    /// Restore the targets the last statue batch action replaced.
    pub fn undo_statue_batch(&mut self, cx: &mut Context<Self>) {
        if let Some(label) = self.statue.plan.undo() {
            self.statue.batch_error = None;
            self.statue.batch_status = Some(format!("Undid {label}."));
            cx.notify();
        }
    }

    /// Pick the statue the farming breakdown describes.
    pub fn set_statue_breakdown(&mut self, statue: usize, cx: &mut Context<Self>) {
        if statue < idleon_core::statue::count() {
            self.statue.breakdown_statue = statue;
            cx.notify();
        }
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

    /// Pick a mechanic's mode; `None` is Saved, which clears its override.
    pub fn set_combat_override_mode(
        &mut self,
        id: &str,
        mode: Option<idleon_core::combat::Mode>,
        cx: &mut Context<Self>,
    ) {
        match mode {
            Some(mode) => {
                let current = self.combat.overrides.rule(id);
                let rule = idleon_core::combat::Rule::seeded(mode, current.as_ref(), None);
                self.combat.overrides.set(id, rule);
                let name =
                    idleon_core::combat::toggle(id).map_or(id, |toggle| toggle.label.as_str());
                self.status = format!("{name}: {} override.", mode.label());
            }
            None => {
                self.combat.overrides.remove(id);
                self.status = format!("Override cleared for {id}.");
            }
        }
        cx.notify();
    }

    /// Set the value a Set or Scale rule uses.
    pub fn set_combat_override_value(&mut self, id: &str, value: f64, cx: &mut Context<Self>) {
        let Some(rule) = self.combat.overrides.rule(id) else {
            return;
        };
        if !matches!(
            rule.mode,
            idleon_core::combat::Mode::Set | idleon_core::combat::Mode::Scale
        ) {
            return;
        }
        self.combat
            .overrides
            .set(id, idleon_core::combat::Rule::with_value(rule.mode, value));
        cx.notify();
    }

    /// Forget one mechanic's override, putting its input back to the save.
    pub fn remove_combat_override(&mut self, id: &str, cx: &mut Context<Self>) {
        if self.combat.overrides.rule(id).is_some() {
            self.combat.overrides.remove(id);
            cx.notify();
        }
    }

    /// Put every mechanic back to its saved value.
    pub fn reset_combat_overrides(&mut self, cx: &mut Context<Self>) {
        self.combat.overrides.reset();
        self.status = "Overrides reset: every mechanic uses the save.".into();
        cx.notify();
    }

    /// Filter the mechanics pane by group; empty means every group.
    pub fn set_combat_override_group(&mut self, group: &str, cx: &mut Context<Self>) {
        self.combat.override_group = group.to_string();
        cx.notify();
    }

    /// Whether the mechanics pane lists only overridden mechanics.
    pub fn set_combat_overrides_only_active(&mut self, only: bool, cx: &mut Context<Self>) {
        self.combat.override_only_active = only;
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

    /// Replace the run toggles; a changed option marks the last result stale.
    pub fn set_options(&mut self, options: SimOptions, cx: &mut Context<Self>) {
        self.options = options;
        cx.notify();
    }

    /// Why the target on screen cannot run, for the Optimize button and its note.
    pub fn target_problem(&self) -> Option<String> {
        self.target.validate(self.simulation.research_level).err()
    }

    /// Bring a level target above the current level after a save or a config
    /// changed the research level.
    pub fn normalize_target(&mut self) {
        self.target = self.target.normalized(self.simulation.research_level);
    }

    pub fn set_preset(&mut self, preset: String, cx: &mut Context<Self>) {
        let Some(entry) = game().presets.iter().find(|entry| entry.id == preset) else {
            return;
        };
        let Some(tiers) = entry.tiers_for(&self.simulation) else {
            self.status = format!("{} has no readable tier list.", entry.name);
            return cx.notify();
        };
        self.simulation.shape_tiers = tiers;
        self.simulation.preset = preset.clone();
        self.research.active_shape_preset = Some(preset);
        cx.notify();
    }

    /// Apply a built-in shape preset: a fresh copy of its tiers, never a link
    /// back to the preset data, so a later edit detaches this run.
    pub fn apply_shape_preset(&mut self, id: &str, cx: &mut Context<Self>) {
        let Some(preset) = game().presets.iter().find(|preset| preset.id == id) else {
            return;
        };
        let Some(tiers) = preset.tiers_for(&self.simulation) else {
            self.status = format!("{} has no readable tier list.", preset.name);
            return cx.notify();
        };
        self.simulation.shape_tiers = tiers;
        self.simulation.preset = preset.id.clone();
        self.research.active_shape_preset = Some(preset.id.clone());
        self.status = format!("Shape priorities: {}.", preset.name);
        cx.notify();
    }

    /// Apply a saved custom preset by name.
    pub fn apply_custom_shape_preset(&mut self, name: &str, cx: &mut Context<Self>) {
        let Some(preset) =
            crate::sections::research::CustomPreset::find(&self.research.shape_presets, name)
        else {
            return;
        };
        self.simulation.shape_tiers = preset.tiers.clone();
        self.simulation.preset = "_none".into();
        self.research.active_shape_preset = Some(preset.name.clone());
        self.status = format!("Shape priorities: {}.", preset.name);
        cx.notify();
    }

    /// Save the current tiers under the name in the preset field.
    pub fn save_shape_preset(&mut self, cx: &mut Context<Self>) {
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
    pub fn delete_shape_preset(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(preset) = crate::sections::research::CustomPreset::delete(
            &mut self.research.shape_presets,
            index,
        ) else {
            return;
        };
        if self.research.active_shape_preset.as_deref() == Some(preset.name.as_str()) {
            self.research.active_shape_preset = None;
        }
        self.status = format!("Deleted the shape preset “{}”.", preset.name);
        cx.notify();
    }

    /// Move a node between the tier zones. Editing detaches the active preset:
    /// the built-in or saved list it came from is never written.
    pub fn move_tier_node(
        &mut self,
        cell: usize,
        tier: idleon_core::Tier,
        before: Option<usize>,
        cx: &mut Context<Self>,
    ) {
        if !self.simulation.shape_tiers.move_node(cell, tier, before) {
            return;
        }
        self.research.active_shape_preset = None;
        self.simulation.preset = "_none".into();
        let zone = match tier {
            idleon_core::Tier::Above => "above",
            idleon_core::Tier::Exp => "Res EXP",
            idleon_core::Tier::Below => "below",
        };
        self.status = format!(
            "Moved {} to the {zone} tier; the preset is detached.",
            idleon_core::model::coord(cell)
        );
        cx.notify();
    }

    pub fn set_target_level(&mut self, level: f64, cx: &mut Context<Self>) {
        let floor = f64::from(self.simulation.research_level.saturating_add(1));
        self.target = Target::Level(level.max(floor).round() as u32);
        cx.notify();
    }

    pub fn set_target_hours(&mut self, hours: f64, cx: &mut Context<Self>) {
        if hours.is_finite() {
            self.target = Target::Hours(hours.max(1.0));
            cx.notify();
        }
    }

    pub fn set_target_kind(&mut self, kind: &str, cx: &mut Context<Self>) {
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

    /// Put every supplement back to what the last save or document set; the
    /// stored result turns stale.
    pub fn reset_supplements(&mut self, cx: &mut Context<Self>) {
        self.context = self.context_base.clone();
        self.status = "Supplements reset to their save-derived values.".into();
        self.context_changed(cx);
    }

    /// Adopt a document's context as both the live and the save-derived one.
    pub(crate) fn replace_context(&mut self, context: SimContext) {
        self.context = context.clone();
        self.context_base = context;
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
            options: inputs.options,
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

    /// The Shape Optimization run: running, progress, result and staleness.
    pub fn shape_optimize_state(
        &self,
        cx: &App,
    ) -> TaskState<idleon_core::optimize::shapes::ShapesOutcome> {
        self.shape_run.read(cx).state(&self.shape_inputs())
    }

    pub fn worship_plan_task(&self, cx: &App) -> TaskState<WorshipRunResult> {
        self.worship_plan
            .read(cx)
            .state(&self.worship_plan_inputs())
    }

    /// The inputs a shape placement started now would use.
    fn shape_inputs(&self) -> ShapeRunInputs {
        ShapeRunInputs {
            simulation: self.simulation.clone(),
            context: self.context.clone(),
            effort: self.effort,
        }
    }

    /// Run the real shape optimizer for the current tiers off the UI thread.
    pub fn optimize_shapes(&mut self, cx: &mut Context<Self>) {
        let inputs = self.shape_inputs();
        let started = self.shape_run.update(cx, |runner, cx| {
            runner.start(cx, inputs, move |inputs, report| {
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
            })
        });
        if started {
            self.status = "Placing shapes…".into();
            cx.notify();
        }
    }

    /// Stop the Shape Optimization run at its next report.
    pub fn cancel_shape_optimize(&mut self, cx: &mut Context<Self>) {
        if self.shape_run.update(cx, |runner, _cx| runner.cancel()) {
            self.status = "Cancelling the shape placement…".into();
            cx.notify();
        }
    }

    /// Write the stored placement to the grid editor, so the shapes the tab
    /// previewed are the ones the rest of the section reads.
    pub fn apply_shape_layout(&mut self, cx: &mut Context<Self>) {
        let run = self.shape_optimize_state(cx);
        let Some(result) = run.result else {
            self.status = "Run the placement optimizer first.".into();
            return cx.notify();
        };
        if run.stale {
            self.status = "The tiers changed after this run; run it again before applying.".into();
            return cx.notify();
        }
        self.simulation.placements = result.placements;
        self.simulation.saved_overlay = None;
        self.status = format!(
            "Applied {} shapes at {} EXP/hr.",
            self.simulation.placements.len(),
            sim::format_rate(result.rate)
        );
        cx.notify();
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
