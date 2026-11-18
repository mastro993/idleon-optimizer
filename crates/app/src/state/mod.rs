//! Application state: where the window is, the simulation being edited and the run in flight.

mod activity;
mod documents;
mod history;
mod runs;
mod subscriptions;

use crate::assets::Manifest;
use crate::sections::cglunko::Tab as CglunkoTab;
use crate::sections::clamworks::{State as ClamworksState, Tab as ClamworksTab};
use crate::sections::cog::{
    PerfectRunInputs as CogPerfectInputs, RunInputs as CogRunInputs, State as CogState,
};
use crate::sections::coin_multi;
use crate::sections::combat_sim::RunResult as CombatEncounterOutcome;
use crate::sections::combat_sim::State as CombatState;
use crate::sections::cooking_mastery::State as MasteryPlanState;
use crate::sections::coral_god_rank::State as CoralState;
use crate::sections::damage::State as DamageState;
use crate::sections::drop_rate::State as DropRateState;
use crate::sections::farming::{
    CompareInputs as FarmingCompareInputs, RunInputs as FarmingRunInputs, State as FarmingState,
};
use crate::sections::fishing::State as FishingState;
use crate::sections::fountain::{MarblePlanInputs, SimInputs, State as FountainState};
use crate::sections::jar::State as JarState;
use crate::sections::jar::Tab as JarTab;
use crate::sections::jelly_operator::{
    OptimizerRunInputs as JellyOptimizerRunInputs, PlannerRunInputs as JellyPlanRunInputs,
    RunInputs as JellyRunInputs, SimulatorRunInputs as JellySimRunInputs, State as JellyState,
};
use crate::sections::killroy::{
    Controls as KillroyControls, KillroyOutcome, State as KillroyState,
};
use crate::sections::ladle::State as LadleSectionState;
use crate::sections::ladle::planner::{LadleDonation, LadlePlanRequest};
use crate::sections::monument::State as MonumentState;
use crate::sections::opal::State as OpalState;
use crate::sections::pickle::State as PickleState;
use crate::sections::prisma::{RunInputs as PrismaRunInputs, State as PrismaState};
use crate::sections::research::State as ResearchState;
use crate::sections::research::minehead::{
    OptimizeInputs as MineheadOptimizeInputs, RunInputs as MineheadRunInputs,
};
use crate::sections::research::tree::{Outcome as TreeOutcome, RunInputs as TreeRunInputs};
use crate::sections::royal_guardian::State as RoyalState;
use crate::sections::royal_guardian::forecast::Inputs as RoyalForecastInputs;
use crate::sections::sailing::State as SailingState;
use crate::sections::sailing::Trees as SailingTrees;
use crate::sections::sailing::simulation::RunInputs as SailingSimInputs;
use crate::sections::spelunking::OptimizerInputs as SpelunkOptimizerInputs;
use crate::sections::spelunking::RunInputs as SpelunkRunInputs;
use crate::sections::spelunking::State as SpelunkingState;
use crate::sections::stats_all::State as StatsAllState;
use crate::sections::statue::State as StatueState;
use crate::sections::worship_td::{OptimizeRunInputs, PlanRunInputs, State as WorshipState};
use crate::sections::{self, Section};
use crate::session::{self, Session, SidebarFold};
use crate::ui;
use crate::ui::export::Export;
use crate::ui::import::Import;
use crate::ui::views::Views;
pub(crate) use activity::{Activity, Mark, Shown};
use gpui::{AppContext as _, Context, Entity, FocusHandle, ScrollHandle, Subscription, Window};
use gpui_kit::component::input::{InputEvent, InputState};
#[cfg(feature = "automation")]
use gpui_mcp::BridgeHandle;
pub(crate) use history::History;
use idleon_cglunko::Cglunko;
use idleon_cglunko::{Page as CglunkoPage, Plan as CglunkoPlan};
use idleon_clamworks::Clamworks;
use idleon_clamworks::scurge::Inputs as ScurgeInputs;
use idleon_clamworks::{Plan as ClamworksPlan, PlanInput as ClamworksPlanInput};
use idleon_cog::{CogResult, PerfectBoard};
use idleon_combat::compare::{Comparison as CombatComparison, Plan as CombatComparePlan};
use idleon_combat::encounter::Options as CombatEncounterOptions;
use idleon_cooking_mastery::Mastery;
use idleon_coral::{Inputs as CoralInputs, Plan as CoralPlan};
use idleon_farming::{Allocation as FarmingOutcome, Comparison as FarmingComparison, Farm};
use idleon_fishing::FishingSave;
use idleon_fountain::{
    Fountain, MarblePlan, OptimizerPlan, PlanRequest, SimResult as FountainSimResult,
};
use idleon_jar::{Jar, SimInputs as JarSimInputs, SimResult as JarSimResult};
use idleon_jelly::optimizer::Optimization as JellyOptimization;
use idleon_jelly::planner::Plan as JellySectionPlan;
use idleon_jelly::{Estimate as JellyEstimate, Jelly, Trial as JellyTrial};
use idleon_killroy::{Forecast as KillroyForecast, KillroyError, Plan as KillroyPlan};
use idleon_ladle::Ladle;
use idleon_monument as monument;
use idleon_prisma::Outcome as PrismaOutcome;
use idleon_research::game;
use idleon_research::model::{
    Effort, Mag, MagKind, OptimizeResult, SimContext, SimOptions, Simulation, Target,
};
use idleon_research::optimize::shapes::ShapesOutcome;
use idleon_research::sim;
use idleon_royal_guardian::RoyalGuardian;
use idleon_sailing::{Fleet, RouteContext, SimResult};
use idleon_save::Save;
use idleon_spelunking::RunResult as SpelunkRunResult;
use idleon_spelunking::SearchResult as SpelunkSearchResult;
use idleon_ui::formula::FormulaTree;
use idleon_ui::runner::TaskRunner;
use idleon_worship::Inputs as WorshipInputs;
use idleon_worship::{self as worship, RunResult as WorshipRunResult};
pub(crate) use runs::discard_run;
use std::path::PathBuf;
pub(crate) use subscriptions::redraw_on_edit;
pub(crate) use subscriptions::{ProgressLine, RunReported, RunStatus, report_run};

/// What a strategy search stored: its recommendation, or why it could not run.
pub(crate) type SearchOutcome = Result<worship::Search, String>;

/// Where the window is: the landing dashboard, a registry section, or an id the
/// registry does not declare.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Route {
    Dashboard,
    Section(&'static Section),
    /// An id no registry entry declares: the shell renders a clear empty state.
    Missing(String),
}

impl Route {
    /// Resolve a section id against the registry.
    pub(crate) fn open(id: &str) -> Self {
        match sections::find(id) {
            Some(section) => Route::Section(section),
            None => Route::Missing(id.to_string()),
        }
    }

    /// The registry entry this route shows, if any.
    pub(crate) fn section(&self) -> Option<&'static Section> {
        match self {
            Route::Section(section) => Some(section),
            _ => None,
        }
    }

    /// Whether the dashboard is showing.
    pub(crate) fn is_dashboard(&self) -> bool {
        matches!(self, Route::Dashboard)
    }
}

/// What the shared page header says about the loaded save.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SaveStatus {
    /// No save has been imported yet.
    Empty,
    /// A save is loaded; later failed imports leave it intact.
    Loaded,
    /// A save is loaded, but the import left fields out and reported warnings.
    Partial,
    /// The last import failed and there is still no save.
    Error,
    /// The session file is still being read; the window opened ahead of it.
    Restoring,
}

impl SaveStatus {
    /// The status for the current loaded/partial/failed flags.
    pub(crate) fn of(loaded: bool, last_ok: bool, partial: bool) -> Self {
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
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Empty => "No save",
            Self::Loaded => "Save imported",
            Self::Partial => "Save imported with warnings",
            Self::Error => "Import failed",
            Self::Restoring => "Loading your save…",
        }
    }
}

/// The inputs a stored shape-placement result was produced from.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ShapeRunInputs {
    pub(crate) simulation: Simulation,
    pub(crate) context: SimContext,
    pub(crate) effort: Effort,
}

/// The inputs a stored optimizer result was produced from.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RunInputs {
    pub(crate) simulation: Simulation,
    pub(crate) context: SimContext,
    pub(crate) target: Target,
    pub(crate) effort: Effort,
    pub(crate) options: SimOptions,
}

/// Everything the window renders and mutates.
pub(crate) struct AppState {
    pub(crate) simulation: Simulation,
    /// The live context the simulation reads: the save-derived values plus the
    /// user's supplements on top.
    pub(crate) context: SimContext,
    /// The context the last applied save or document established.
    /// [`AppState::reset_supplements`] returns here, so a reset never throws
    /// away a multiplier the save decoded.
    pub(crate) context_base: SimContext,
    pub(crate) effort: Effort,
    pub(crate) target: Target,
    /// The three run toggles the next projection reads.
    pub(crate) options: SimOptions,
    /// Game artwork resolved through `assets/game/manifest.json`.
    pub(crate) assets: Manifest,
    /// The places opened in this window; its current one is where the window is.
    pub(crate) history: History,
    /// The last section opened; the dashboard offers to resume it.
    pub(crate) last_section: Option<&'static str>,
    /// What each section's runs show on its sidebar item.
    pub(crate) activity: Activity,
    /// The research section's own state, kept alive across dashboard visits.
    pub(crate) research: ResearchState,
    /// The Killroy section's forecast settings.
    pub(crate) killroy: KillroyState,
    /// The Killroy forecast for the current save and week count, kept until
    /// either moves; `Err` is kept too, so a refused save is not re-read.
    pub(crate) killroy_forecast: Option<Result<KillroyForecast, KillroyError>>,
    /// The `(save_revision, weeks)` `killroy_forecast` was computed from.
    pub(crate) killroy_forecast_key: Option<(u64, usize)>,
    /// The Killroy section's kit fields, tables and the rows derived for them.
    pub(crate) killroy_controls: KillroyControls,
    /// The decoded save behind the simulation, for sections that read raw fields.
    pub(crate) save: Option<Save>,
    /// The sailing fleet imported from the save, and the route context.
    pub(crate) fleet: Fleet,
    pub(crate) sailing: RouteContext,
    /// The sailing section's own state, kept alive across dashboard visits.
    pub(crate) sailing_state: SailingState,
    /// The sailing breakdowns' formula trees.
    pub(crate) sailing_trees: SailingTrees,
    /// The shared runner for the fleet pile simulation.
    pub(crate) sailing_sim: Entity<TaskRunner<SimResult, SailingSimInputs>>,
    /// The farming field the last import carried.
    pub(crate) farm: Farm,
    /// The farming section's selection and crop target, kept alive too.
    pub(crate) farming: FarmingState,
    /// The farming preset bar's name field.
    pub(crate) farming_name: Entity<InputState>,
    /// The fishing characters, rod and saved toolkit the last import carried.
    pub(crate) fishing_save: FishingSave,
    /// The fishing section's character, toolkit, pool and account values.
    pub(crate) fishing: FishingState,
    /// The monument section's own state, kept alive across dashboard visits.
    pub(crate) monument: MonumentState,
    /// The imported save, narrowed to the Monument rows.
    pub(crate) monument_save: Option<monument::Save>,
    /// The Cglunko state the last imported save carried.
    pub(crate) cglunko: Cglunko,
    /// The Cglunko tab the section was left on.
    pub(crate) cglunko_tab: CglunkoTab,
    /// The page the Cglunko upgrade list and plan work on.
    pub(crate) cglunko_page: CglunkoPage,
    /// The Clamworks state the last imported save carried.
    pub(crate) clamworks: Clamworks,
    /// The cog board section: the decoded save and the board on screen.
    pub(crate) cog: CogState,
    /// The Clamworks tab the section was left on.
    pub(crate) clamworks_tab: ClamworksTab,
    /// The multikill total Multi-Scalping reads, until the combat sim lands.
    pub(crate) clamworks_multikill: f64,
    /// The pearls the Clamworks upgrade plan may spend.
    pub(crate) clamworks_budget: f64,
    /// The levels the applied plan previews; `None` shows the saved levels.
    pub(crate) clamworks_projection: Option<[u32; 9]>,
    /// The AFK claim's character, stand-ins, hours and pearl table.
    pub(crate) clamworks_claim: ClamworksState,
    /// The Scurge panel the section was left on.
    pub(crate) clamworks_scurge: ScurgeInputs,
    /// The Jelly Operator board the last imported save carried.
    pub(crate) jelly: Option<Jelly>,
    /// The Jelly Operator section's board on screen and its palette pick.
    pub(crate) jelly_state: JellyState,
    /// The jar section's active tab, kept across dashboard visits.
    pub(crate) jar_tab: JarTab,
    /// The tier the rupie value tab projects its income for.
    pub(crate) jar_tier: u32,
    /// The rupie value breakdown tree on the jar section.
    pub(crate) jar_breakdown: Entity<FormulaTree>,
    /// The Jar Cavern the loaded save carries, if it has one.
    pub(crate) jar: Option<Jar>,
    /// The enchant simulator's controls and the saved history.
    pub(crate) jar_state: JarState,
    /// The shared runner for the enchant simulation.
    pub(crate) jar_sim: Entity<TaskRunner<JarSimResult, JarSimInputs>>,
    /// The Spelunking section's controls and account inputs.
    pub(crate) spelunking: SpelunkingState,
    /// The Spelunking rows the loaded save carries, if it has them.
    pub(crate) spelunking_save: Option<idleon_spelunking::Save>,
    /// The shared runner for the Spelunking expedition run.
    pub(crate) spelunk_sim: Entity<TaskRunner<SpelunkRunResult, SpelunkRunInputs>>,
    /// The shared runner for the Spelunking optimizer.
    pub(crate) spelunk_opt: Entity<TaskRunner<SpelunkSearchResult, SpelunkOptimizerInputs>>,
    /// The ladle optimizer's save-derived roster and counts.
    pub(crate) ladle: Option<Ladle>,
    /// The ladle section's selectors, panel inputs and formula trees.
    pub(crate) ladle_state: LadleSectionState,
    /// The shared runner for the Blue Jellyfish donation search.
    pub(crate) ladle_plan: Entity<TaskRunner<LadleDonation, LadlePlanRequest>>,
    /// The Cooking Mastery state the last imported save carried.
    pub(crate) mastery: Option<Mastery>,
    /// The yellow point allocator's meal state, from the same save section.
    pub(crate) yellow: Option<idleon_cooking_mastery::Yellow>,
    /// The section's tab, distribution goal, budgets and character, kept across
    /// visits.
    pub(crate) mastery_plan: MasteryPlanState,
    /// The Fountain Cavern the loaded save carries, if it has one.
    pub(crate) fountain: Option<Fountain>,
    /// The Fountain section's tab, water, desired currency and curve pick.
    pub(crate) fountain_state: FountainState,
    /// The shared runner for the fountain upgrade optimizer.
    pub(crate) fountain_plan: Entity<TaskRunner<OptimizerPlan, PlanRequest>>,
    /// The shared runner for the fountain marble optimizer.
    pub(crate) fountain_marble_plan: Entity<TaskRunner<MarblePlan, MarblePlanInputs>>,
    /// The shared runner for the fountain simulation.
    pub(crate) fountain_sim: Entity<TaskRunner<FountainSimResult, SimInputs>>,
    /// The Opal Optimizer's villagers, selections, plan run and formula tree.
    pub(crate) opal: OpalState,
    /// The Royal Guardian rows the loaded save carries, if it has them.
    pub(crate) royal: Option<RoyalGuardian>,
    /// The Royal Guardian section's world and selected outpost.
    pub(crate) royal_state: RoyalState,
    /// The shared formula tree on the Royal Guardian breakdowns pane.
    pub(crate) royal_breakdown: Entity<FormulaTree>,
    /// The Royal Guardian forecast's live collect and window fields.
    pub(crate) royal_forecast_inputs: RoyalForecastInputs,
    /// The forecast for the current inputs, kept until they move.
    pub(crate) royal_forecast: Option<idleon_royal_guardian::Forecast>,
    /// The inputs `royal_forecast` was computed from.
    pub(crate) royal_forecast_key: Option<idleon_royal_guardian::ForecastOptions>,
    /// The shared runner for the Royal Guardian outpost optimizer.
    pub(crate) royal_optimizer:
        Entity<TaskRunner<idleon_royal_guardian::Outcome, idleon_royal_guardian::Request>>,
    /// The shared runner for the unit EXP plan.
    pub(crate) royal_unit_exp: Entity<
        TaskRunner<idleon_royal_guardian::UnitExpPlan, idleon_royal_guardian::UnitExpRequest>,
    >,
    /// The shared runner for the next-shelf plan.
    pub(crate) royal_shelf:
        Entity<TaskRunner<idleon_royal_guardian::ShelfPlan, idleon_royal_guardian::ShelfRequest>>,
    /// The ninja loadout and inventory the loaded save carries, if it has any.
    pub(crate) sneaking: Option<idleon_sneaking::Loadout>,
    /// The sneaking workspace's login context and selected slot.
    pub(crate) sneaking_state: crate::sections::sneaking::State,
    /// The shared runner for the sneaking charm placement search.
    pub(crate) sneaking_opt: Entity<TaskRunner<idleon_sneaking::Outcome, idleon_sneaking::Request>>,
    /// The shared formula tree on the sneaking breakdowns workspace.
    pub(crate) sneaking_breakdown: Entity<FormulaTree>,
    /// The BoneJoePickle section's inputs, kept alive the same way.
    pub(crate) pickle: PickleState,
    /// The Prisma calculator's controls and the plan for the save.
    pub(crate) prisma: PrismaState,
    /// The statue section's saved rows, character and table controls.
    pub(crate) statue: StatueState,
    /// The combat simulator's working loadout and named presets.
    pub(crate) combat: CombatState,
    /// The encounter run (CS-03) in flight and its stored outcome.
    pub(crate) combat_encounter: Entity<TaskRunner<CombatEncounterOutcome, CombatEncounterOptions>>,
    /// The character, loadout and map comparison (CS-04) and its stored result.
    pub(crate) combat_compare: Entity<TaskRunner<CombatComparison, CombatComparePlan>>,
    /// The All Stats Dashboard's calculation context, save slice and breakdowns.
    pub(crate) stats_all: StatsAllState,
    /// The shared formula tree on the All Stats breakdown card.
    pub(crate) stats_breakdown: Entity<FormulaTree>,
    /// The formula explorer's search field.
    pub(crate) stats_explorer_search: Entity<InputState>,
    /// The shared formula tree on the All Stats formula card.
    pub(crate) stats_explorer_formula: Entity<FormulaTree>,
    /// The Drop Rate Calculator's context, save slice and live dungeon fields.
    pub(crate) drop_rate: DropRateState,
    /// The Coin Multiplier Calculator's cards and damage field.
    pub(crate) coin_multi: coin_multi::State,
    /// The Damage Breakdown's character/map context, layer tab and summary.
    pub(crate) damage: DamageState,
    /// The shared formula tree on the damage section's mastery breakdown card.
    pub(crate) damage_mastery: Entity<FormulaTree>,
    /// The shared formula tree on the damage section's formula breakdown card.
    pub(crate) damage_formula: Entity<FormulaTree>,
    /// The coral god rank planner's inputs, kept alive across dashboard visits.
    pub(crate) coral: CoralState,
    /// The Worship TD simulator's config and live run.
    pub(crate) worship: WorshipState,
    /// The tower trial levels and account values the live run reads.
    pub(crate) worship_inputs: WorshipInputs,
    /// The dashboard search field.
    pub(crate) search: Entity<InputState>,
    /// The Justice case search field.
    pub(crate) monument_search: Entity<InputState>,
    pub(crate) status: String,
    /// Whether the save-derived supplements card is expanded.
    pub(crate) supplements_open: bool,
    /// The shared import dialog every section's header opens.
    pub(crate) import: Import,
    /// The window's own local memory: the last project and the recent imports.
    pub(crate) session: Session,
    /// Whether the session file is still being read. The window opens first and
    /// veils for the loader while the save decodes, and nothing is written back
    /// to the session until it lands: a half-restored window must never
    /// overwrite the file it is restoring from.
    pub(crate) restoring: bool,
    /// The `--import <file>` this launch named, applied once the restore lands
    /// so it lands on top of the session as it did when both ran in order.
    launch_import: Option<PathBuf>,
    /// The `--section <id>` this launch named, applied the same way.
    launch_section: Option<String>,
    /// The shared export dialog: the whole window state as one JSON document.
    pub(crate) export: Export,
    /// Whether the loaded save left fields out and reported warnings.
    pub(crate) save_partial: bool,
    /// Whether a save is loaded, for the shared page header.
    pub(crate) save_loaded: bool,
    /// How many saves this window has applied; a Prisma run captures it.
    pub(crate) save_revision: u64,
    /// The shared background runner for the research optimizer.
    pub(crate) optimizer: Entity<TaskRunner<OptimizeResult, RunInputs>>,
    /// The shared runner for the Shape Optimization tab's placement.
    pub(crate) shape_run: Entity<TaskRunner<ShapesOutcome, ShapeRunInputs>>,
    /// The shared runner for the Insight ROI analysis; it takes the same
    /// simulation, context and effort a shape placement does.
    pub(crate) insight_run: Entity<TaskRunner<idleon_research::insight::Outcome, ShapeRunInputs>>,
    /// The shared runner for the Observation Unlocks ranking; it takes the same
    /// simulation, context and effort.
    pub(crate) unlock_run: Entity<TaskRunner<idleon_research::unlock::Outcome, ShapeRunInputs>>,
    /// The shared runner for the Decision Tree's steps, branches and auto fits.
    pub(crate) tree_run: Entity<TaskRunner<TreeOutcome, TreeRunInputs>>,
    /// The shared runner for the Killroy comparison's trials.
    pub(crate) killroy_run: Entity<TaskRunner<KillroyOutcome, KillroyPlan>>,
    /// The shared runner for the coral god rank beam search.
    pub(crate) coral_run: Entity<TaskRunner<CoralPlan, CoralInputs>>,
    /// The shared runner for the Jelly Operator operation estimate.
    pub(crate) jelly_run: Entity<TaskRunner<JellyEstimate, JellyRunInputs>>,
    pub(crate) jelly_plan_run: Entity<TaskRunner<JellySectionPlan, JellyPlanRunInputs>>,
    /// The shared runner for the Jelly coordinator's layout search.
    pub(crate) jelly_optimize_run: Entity<TaskRunner<JellyOptimization, JellyOptimizerRunInputs>>,
    /// The shared runner for one simulator operation.
    pub(crate) jelly_sim_run: Entity<TaskRunner<JellyTrial, JellySimRunInputs>>,
    /// The shared formula tree on the Jelly Operator breakdowns pane.
    pub(crate) jelly_breakdown: Entity<FormulaTree>,
    /// The shared runner for the cog board optimizer.
    pub(crate) cog_run: Entity<TaskRunner<CogResult, CogRunInputs>>,
    /// The shared runner for the perfect cog board.
    pub(crate) cog_perfect_run: Entity<TaskRunner<PerfectBoard, CogPerfectInputs>>,
    /// The shared runner for the Cglunko upgrade plan.
    pub(crate) cglunko_run: Entity<TaskRunner<CglunkoPlan, CglunkoPage>>,
    /// The shared runner for the Clamworks upgrade plan.
    pub(crate) clamworks_run: Entity<TaskRunner<ClamworksPlan, ClamworksPlanInput>>,
    /// The shared runner for a Worship TD plan simulation.
    pub(crate) worship_plan: Entity<TaskRunner<WorshipRunResult, PlanRunInputs>>,
    /// The shared runner for the Worship TD strategy search.
    pub(crate) worship_optimize: Entity<TaskRunner<SearchOutcome, OptimizeRunInputs>>,
    /// The shared runner for the Prisma bubble search.
    pub(crate) prisma_run: Entity<TaskRunner<PrismaOutcome, PrismaRunInputs>>,
    /// The shared runner for the Land Rank allocation.
    pub(crate) farming_run: Entity<TaskRunner<FarmingOutcome, FarmingRunInputs>>,
    /// The shared runner for the preset comparison.
    pub(crate) farming_compare_run: Entity<TaskRunner<FarmingComparison, FarmingCompareInputs>>,
    /// The shared EXP formula tree on the research Rates pane.
    pub(crate) breakdown: Entity<FormulaTree>,
    /// The shared EXP formula tree on the Insight ROI baseline.
    pub(crate) insight_breakdown: Entity<FormulaTree>,
    /// The shared formula tree on the Minehead currency pane.
    pub(crate) minehead_breakdown: Entity<FormulaTree>,
    /// The three shared formula trees on the Sushi currency pane.
    pub(crate) sushi_bucks: Entity<FormulaTree>,
    pub(crate) sushi_fuel: Entity<FormulaTree>,
    pub(crate) sushi_capacity: Entity<FormulaTree>,
    /// The Minehead strategy search.
    pub(crate) minehead_optimize:
        Entity<TaskRunner<idleon_minehead::Optimized, MineheadOptimizeInputs>>,
    /// The Minehead +1 upgrade ranking.
    pub(crate) minehead_rank: Entity<TaskRunner<idleon_minehead::RankOutcome, MineheadRunInputs>>,
    /// The Minehead affordable upgrade path.
    pub(crate) minehead_path: Entity<TaskRunner<idleon_minehead::Path, MineheadRunInputs>>,
    /// The research dashboard's four breakdown trees.
    pub(crate) research_trees: crate::sections::research::Trees,
    /// The shared formula tree on the Cglunko overview.
    pub(crate) cglunko_breakdown: Entity<FormulaTree>,
    /// The shared formula tree on the Clamworks overview.
    pub(crate) clamworks_breakdown: Entity<FormulaTree>,
    /// The shared formula tree on the Fishing calculator.
    pub(crate) fishing_breakdown: Entity<FormulaTree>,
    /// The shared EXP rate tree on the Mastery EXP tab.
    pub(crate) mastery_breakdown: Entity<FormulaTree>,
    /// The shared formula tree on the Fountain overview.
    pub(crate) fountain_breakdown: Entity<FormulaTree>,
    pub(crate) focus: FocusHandle,
    /// The sidebar's scroll position, moved to reveal the active item.
    pub(crate) sidebar_scroll: ScrollHandle,
    /// The header, sidebar, Home, status and section views the shell embeds,
    /// made on the first frame.
    pub(crate) views: Option<Views>,
    /// Held so the automation bridge stays installed while the window lives;
    /// `None` when it could not start.
    #[cfg(feature = "automation")]
    _bridge: Option<BridgeHandle>,
    _subscriptions: Vec<Subscription>,
    /// Whether a debounced session save is already scheduled.
    session_save_pending: bool,
}

impl AppState {
    /// Build the window state. `section` opens that registry id at launch, so a
    /// lane or a harness can start on any section, including hidden tools.
    pub(crate) fn new(
        window: &mut Window,
        endpoint_dir: Option<std::path::PathBuf>,
        section: Option<String>,
        state_dir: Option<std::path::PathBuf>,
        launch_import: Option<std::path::PathBuf>,
        cx: &mut Context<Self>,
    ) -> Self {
        #[cfg(feature = "automation")]
        let bridge = ui::bridge::install(window, cx, endpoint_dir);
        // Without the bridge the flag has nothing to scope.
        #[cfg(not(feature = "automation"))]
        let _ = endpoint_dir;
        let import = Import::new(window, cx);
        let search = ui::input_state("Search calculators…", window, cx);
        let monument_search = ui::input_state("Visitor or words from the request…", window, cx);
        let statue = StatueState::new(window, cx);
        let prisma = PrismaState::new(window, cx);
        let combat = CombatState::new(window, cx);
        let killroy_controls = KillroyControls::new(window, cx);
        let research = ResearchState::new(window, cx);
        let farming_name = ui::input_state("Preset name", window, cx);
        let stats_explorer_search = ui::input_state("Name, saved input or source", window, cx);
        // Filter and name fields redraw the page on every keystroke.
        let mut subscriptions: Vec<Subscription> = vec![
            redraw_on_edit(cx, &search),
            redraw_on_edit(cx, &monument_search),
            redraw_on_edit(cx, &statue.search),
            redraw_on_edit(cx, &prisma.search),
            redraw_on_edit(cx, &combat.name),
            redraw_on_edit(cx, &combat.talent_search),
            redraw_on_edit(cx, &combat.override_search),
            redraw_on_edit(cx, &research.preset_name),
            redraw_on_edit(cx, &stats_explorer_search),
        ];
        // Every edit of the paste box re-checks the text, so the preview and the
        // Apply button always describe what is on screen.
        subscriptions.push(
            cx.subscribe(&import.input, |state, _input, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    state.check_paste(cx);
                }
            }),
        );

        // Each long run owns a runner; the window mirrors it in the status line
        // and in its section's sidebar activity.
        let optimizer = report_run::<OptimizeResult, RunInputs>(
            cx,
            &mut subscriptions,
            RunStatus {
                section: "research",
                progress: ProgressLine::Percent("Optimizing…"),
                cancelled_kept: "Run cancelled; the previous result is unchanged.",
                cancelled_empty: "Run cancelled; nothing was applied.",
            },
            |_, result| {
                Some(format!(
                    "{} steps in {} ms.",
                    result.steps.len(),
                    result.elapsed_ms
                ))
            },
        );
        let shape_run = report_run::<ShapesOutcome, ShapeRunInputs>(
            cx,
            &mut subscriptions,
            RunStatus {
                section: "research",
                progress: ProgressLine::Percent("Placing shapes…"),
                cancelled_kept: "Shape placement cancelled; the previous layout is unchanged.",
                cancelled_empty: "Shape placement cancelled; nothing was placed.",
            },
            |_, result| {
                Some(
                    result
                        .steps
                        .last()
                        .cloned()
                        .unwrap_or_else(|| "Shape placement finished.".into()),
                )
            },
        );
        let insight_run = report_run::<idleon_research::insight::Outcome, ShapeRunInputs>(
            cx,
            &mut subscriptions,
            RunStatus {
                section: "research",
                progress: ProgressLine::Message("Computing insight ROI…"),
                cancelled_kept: "Insight analysis cancelled; the previous table is unchanged.",
                cancelled_empty: "Insight analysis cancelled; nothing was stored.",
            },
            |_, result| {
                Some(format!(
                    "Ranked {} observations by insight ROI.",
                    result.rows.len()
                ))
            },
        );
        let tree_run = report_run::<TreeOutcome, TreeRunInputs>(
            cx,
            &mut subscriptions,
            RunStatus {
                section: "research",
                progress: ProgressLine::Message("Stepping the tree…"),
                cancelled_kept: "Tree step cancelled; the tree is unchanged.",
                cancelled_empty: "Tree step cancelled; nothing changed.",
            },
            |state, outcome: &TreeOutcome| {
                state.research.tree.tree = Some(outcome.tree.clone());
                state.research.tree.selected = outcome.select.or(state.research.tree.selected);
                Some(outcome.message.clone())
            },
        );
        let unlock_run = report_run::<idleon_research::unlock::Outcome, ShapeRunInputs>(
            cx,
            &mut subscriptions,
            RunStatus {
                section: "research",
                progress: ProgressLine::Message("Ranking observation unlocks…"),
                cancelled_kept: "Unlock ranking cancelled; the previous table is unchanged.",
                cancelled_empty: "Unlock ranking cancelled; nothing was stored.",
            },
            |_, result| {
                Some(format!(
                    "Ranked {} undiscovered observations.",
                    result.rows.len()
                ))
            },
        );
        // A finished cog run opens its walkthrough, which reads the stored
        // board one step at a time until Done keeps it.
        let cog_run = report_run::<CogResult, CogRunInputs>(
            cx,
            &mut subscriptions,
            RunStatus {
                section: "cog",
                progress: ProgressLine::Percent("Optimizing the cog board…"),
                cancelled_kept: "Cog run cancelled; the previous result is unchanged.",
                cancelled_empty: "Cog run cancelled; the board is unchanged.",
            },
            |state, result| {
                state.cog.begin_walkthrough(result);
                Some(if result.swaps.is_empty() {
                    "Cog board optimized: no swaps needed.".into()
                } else {
                    format!(
                        "Cog board optimized: {:+.2}% with {} swaps. Walk through them, then press Done to keep the board.",
                        result.improvement_pct,
                        result.swaps.len()
                    )
                })
            },
        );
        // The perfect board keeps its own result so the optimizer's stored
        // board is not overwritten.
        let cog_perfect_run = report_run::<PerfectBoard, CogPerfectInputs>(
            cx,
            &mut subscriptions,
            RunStatus {
                section: "cog",
                progress: ProgressLine::Percent("Building the perfect cog board…"),
                cancelled_kept: "Perfect board cancelled; the previous result is unchanged.",
                cancelled_empty: "Perfect board cancelled; the board is unchanged.",
            },
            |_, perfect| {
                Some(format!(
                    "Perfect cog board ready: {} layer(s), {} Excogia block(s), {} Yang.",
                    perfect.layers.len(),
                    perfect.excogia_blocks,
                    perfect.yang_placed
                ))
            },
        );
        let cglunko_run = report_run::<CglunkoPlan, CglunkoPage>(
            cx,
            &mut subscriptions,
            RunStatus {
                section: "cglunko",
                progress: ProgressLine::Message("Planning Cglunko upgrades…"),
                cancelled_kept: "Cglunko plan cancelled; the previous plan is unchanged.",
                cancelled_empty: "Cglunko plan cancelled; nothing was planned.",
            },
            |_, plan| {
                Some(format!(
                    "Cglunko plan ready: {} purchases, {:.2}× DR.",
                    plan.steps.len(),
                    plan.dr_after()
                ))
            },
        );
        let fountain_plan = report_run::<OptimizerPlan, PlanRequest>(
            cx,
            &mut subscriptions,
            RunStatus {
                section: "fountain",
                progress: ProgressLine::Message("Planning fountain upgrades…"),
                cancelled_kept: "Fountain plan cancelled; the previous plan is unchanged.",
                cancelled_empty: "Fountain plan cancelled; nothing was planned.",
            },
            |_, plan| {
                Some(format!(
                    "Fountain plan ready: {} steps, {} → {} ({:+.1}%).",
                    plan.steps.len(),
                    idleon_fountain::short_number(plan.start_value),
                    idleon_fountain::short_number(plan.end_value),
                    plan.gain_pct
                ))
            },
        );
        let fountain_marble_plan = report_run::<MarblePlan, MarblePlanInputs>(
            cx,
            &mut subscriptions,
            RunStatus {
                section: "fountain",
                progress: ProgressLine::Message("Planning fountain marble upgrades…"),
                cancelled_kept: "Marble plan cancelled; the previous plan is unchanged.",
                cancelled_empty: "Marble plan cancelled; nothing was planned.",
            },
            |_, plan| {
                Some(format!(
                    "Marble plan ready: {} levels, {} → {} ({:+.1}%).",
                    plan.steps.len(),
                    idleon_fountain::short_number(plan.start_value),
                    idleon_fountain::short_number(plan.end_value),
                    plan.gain_pct
                ))
            },
        );
        let fountain_sim = report_run::<FountainSimResult, SimInputs>(
            cx,
            &mut subscriptions,
            RunStatus {
                section: "fountain",
                progress: ProgressLine::Message("Simulating fountain play…"),
                cancelled_kept: "Fountain simulation cancelled; the previous result is unchanged.",
                cancelled_empty: "Fountain simulation cancelled; nothing was stored.",
            },
            |_, result| {
                Some(format!(
                    "Fountain simulation ready: {} steps, marble {} → {}/hr.",
                    result.steps,
                    idleon_fountain::short_number(result.start_marble_rate),
                    idleon_fountain::short_number(result.end_marble_rate)
                ))
            },
        );
        let clamworks_run = report_run::<ClamworksPlan, ClamworksPlanInput>(
            cx,
            &mut subscriptions,
            RunStatus {
                section: "clamworks",
                progress: ProgressLine::Message("Planning Clamworks upgrades…"),
                cancelled_kept: "Clamworks plan cancelled; the previous plan is unchanged.",
                cancelled_empty: "Clamworks plan cancelled; nothing was applied.",
            },
            |_, plan| {
                Some(format!(
                    "Clamworks plan ready: {} purchases, {:.0} of {:.0} pearls.",
                    plan.steps.len(),
                    plan.spent,
                    plan.budget
                ))
            },
        );
        let worship_plan = report_run::<WorshipRunResult, PlanRunInputs>(
            cx,
            &mut subscriptions,
            RunStatus {
                section: "worship-td",
                progress: ProgressLine::Percent("Simulating the Worship TD plan…"),
                cancelled_kept: "Plan run cancelled; the previous result is unchanged.",
                cancelled_empty: "Plan run cancelled; nothing was applied.",
            },
            |_, result| {
                Some(format!(
                    "Plan simulated: best wave {}, {} towers.",
                    result.best_wave,
                    result.towers.len()
                ))
            },
        );
        let killroy_run = report_run::<KillroyOutcome, KillroyPlan>(
            cx,
            &mut subscriptions,
            RunStatus {
                section: "killroy",
                progress: ProgressLine::Message("Running Killroy trials…"),
                cancelled_kept: "Killroy run cancelled; the previous comparison is unchanged.",
                cancelled_empty: "Killroy run cancelled; nothing was applied.",
            },
            |_, outcome| {
                let best = outcome.rows.iter().filter(|row| row.best).count();
                Some(format!(
                    "Killroy trials finished: {} configurations, {best} best.",
                    outcome.rows.len()
                ))
            },
        );
        let jelly_run = report_run::<JellyEstimate, JellyRunInputs>(
            cx,
            &mut subscriptions,
            RunStatus {
                section: "jelly-operator",
                progress: ProgressLine::Message("Estimating the Jelly operation…"),
                cancelled_kept: "Jelly estimate cancelled; the previous estimate is unchanged.",
                cancelled_empty: "Jelly estimate cancelled; nothing was stored.",
            },
            |_, estimate| {
                Some(format!(
                    "Jelly estimate ready: {} of {} trials cleared.",
                    estimate.success_count, estimate.trials
                ))
            },
        );
        let jelly_plan_run = report_run::<JellySectionPlan, JellyPlanRunInputs>(
            cx,
            &mut subscriptions,
            RunStatus {
                section: "jelly-operator",
                progress: ProgressLine::Message("Ranking the Jelly board sections…"),
                cancelled_kept: "Jelly section plan cancelled; the previous plan is unchanged.",
                cancelled_empty: "Jelly section plan cancelled; nothing was stored.",
            },
            |_, plan| {
                Some(match plan.recommended.as_ref() {
                    Some(row) => format!(
                        "Jelly section plan ready: {}, {} sequence(s) ranked.",
                        if row.plot_ids.is_empty() {
                            "none recommended".to_string()
                        } else {
                            row.plot_ids
                                .iter()
                                .map(|plot| format!("section {}", plot + 1))
                                .collect::<Vec<_>>()
                                .join(" then ")
                        },
                        plan.sequences.len()
                    ),
                    None => "Jelly section plan ready: no recommendation.".to_string(),
                })
            },
        );
        let jelly_optimize_run = report_run::<JellyOptimization, JellyOptimizerRunInputs>(
            cx,
            &mut subscriptions,
            RunStatus {
                section: "jelly-operator",
                progress: ProgressLine::Message("Optimizing the Jelly board…"),
                cancelled_kept: "Jelly optimization cancelled; the previous board is unchanged.",
                cancelled_empty: "Jelly optimization cancelled; nothing was stored.",
            },
            |_, result| {
                Some(format!(
                    "Jelly board optimized: {} anchor change(s), {} finalist(s) simulated.",
                    result.moves.len(),
                    result.finalists.len()
                ))
            },
        );
        let jelly_sim_run = report_run::<JellyTrial, JellySimRunInputs>(
            cx,
            &mut subscriptions,
            RunStatus {
                section: "jelly-operator",
                progress: ProgressLine::Message("Simulating the Jelly operation…"),
                cancelled_kept: "Jelly simulation cancelled; the previous run is unchanged.",
                cancelled_empty: "Jelly simulation cancelled; nothing was stored.",
            },
            |_, trial| {
                Some(format!(
                    "Jelly operation finished: {} at {:.2}s with {} damage.",
                    trial.ended_by.replace('-', " "),
                    trial.time,
                    idleon_ui::format::value(trial.damage)
                ))
            },
        );
        let coral_run = report_run::<CoralPlan, CoralInputs>(
            cx,
            &mut subscriptions,
            RunStatus {
                section: "coral-god-rank",
                progress: ProgressLine::Message("Planning Coral Kid upgrades…"),
                cancelled_kept: "Coral plan cancelled; the previous plan is unchanged.",
                cancelled_empty: "Coral plan cancelled; nothing was planned.",
            },
            |_, plan| {
                let committed = if plan.budget > 0.0 {
                    plan.spent / plan.budget * 100.0
                } else {
                    0.0
                };
                Some(format!(
                    "Coral plan ready: {} actions, {committed:.0}% budget committed.",
                    plan.actions.len()
                ))
            },
        );
        let worship_optimize = report_run::<SearchOutcome, OptimizeRunInputs>(
            cx,
            &mut subscriptions,
            RunStatus {
                section: "worship-td",
                progress: ProgressLine::Percent("Searching Worship TD strategies…"),
                cancelled_kept: "Search cancelled; the previous recommendation is unchanged.",
                cancelled_empty: "Search cancelled; nothing was recommended.",
            },
            |_, outcome| {
                Some(match outcome {
                    Ok(result) => format!(
                        "Search finished: {} clears {:.1} waves.",
                        result.label, result.score.primary
                    ),
                    Err(reason) => reason.clone(),
                })
            },
        );
        let prisma_run = report_run::<PrismaOutcome, PrismaRunInputs>(
            cx,
            &mut subscriptions,
            RunStatus {
                section: "prisma",
                progress: ProgressLine::Percent("Comparing Prisma bubbles…"),
                cancelled_kept: "Prisma run cancelled; the previous result is unchanged.",
                cancelled_empty: "Prisma run cancelled; nothing was compared.",
            },
            |_, result| {
                let changed = result
                    .results
                    .iter()
                    .filter(|row| !row.changes.is_empty())
                    .count();
                Some(format!(
                    "Compared {} Prisma bubbles; {changed} changed.",
                    result.results.len()
                ))
            },
        );
        let breakdown = ui::formula_tree("formula", window, cx);
        let farming_run = report_run::<FarmingOutcome, FarmingRunInputs>(
            cx,
            &mut subscriptions,
            RunStatus {
                section: "farming",
                progress: ProgressLine::Percent("Allocating Land Rank…"),
                cancelled_kept: "Land Rank allocation cancelled; the previous result is unchanged.",
                cancelled_empty: "Land Rank allocation cancelled; nothing was assigned.",
            },
            |_, result| {
                Some(format!(
                    "Allocated {} Land Rank point(s) for {}.",
                    result.points_spent,
                    result.objective.label()
                ))
            },
        );
        let farming_compare_run = report_run::<FarmingComparison, FarmingCompareInputs>(
            cx,
            &mut subscriptions,
            RunStatus {
                section: "farming",
                progress: ProgressLine::Percent("Comparing farming presets…"),
                cancelled_kept: "Preset comparison cancelled; the previous table is unchanged.",
                cancelled_empty: "Preset comparison cancelled; no table was stored.",
            },
            |_, result| Some(format!("Compared {} farming goals.", result.rows.len())),
        );
        let combat_encounter = report_run::<CombatEncounterOutcome, CombatEncounterOptions>(
            cx,
            &mut subscriptions,
            RunStatus {
                section: "combat-sim",
                progress: ProgressLine::Message("Running the encounter…"),
                cancelled_kept: "Encounter run cancelled; the previous result is unchanged.",
                cancelled_empty: "Encounter run cancelled; nothing was stored.",
            },
            |_, result| {
                Some(format!(
                    "Encounter run ready: {} kills over {:.0}s.",
                    result.run.kills,
                    result.run.simulated_ms as f64 / 1000.0
                ))
            },
        );
        let combat_compare = report_run::<CombatComparison, CombatComparePlan>(
            cx,
            &mut subscriptions,
            RunStatus {
                section: "combat-sim",
                progress: ProgressLine::Message("Comparing the selected setups…"),
                cancelled_kept: "Comparison cancelled; the previous comparison is unchanged.",
                cancelled_empty: "Comparison cancelled; nothing was stored.",
            },
            |_, result| Some(format!("Compared {} setups.", result.rows.len())),
        );
        let minehead_breakdown = ui::formula_tree("minehead-formula", window, cx);
        let sushi_bucks = ui::formula_tree("sushi-bucks-formula", window, cx);
        let sushi_fuel = ui::formula_tree("sushi-fuel-formula", window, cx);
        let sushi_capacity = ui::formula_tree("sushi-capacity-formula", window, cx);
        let minehead_optimize = report_run::<idleon_minehead::Optimized, MineheadOptimizeInputs>(
            cx,
            &mut subscriptions,
            RunStatus {
                section: "research",
                progress: ProgressLine::Message("Optimizing the strategy…"),
                cancelled_kept: "Strategy search cancelled; the previous result is unchanged.",
                cancelled_empty: "Strategy search cancelled; nothing was stored.",
            },
            |_, result| {
                Some(format!(
                    "Searched {} strategies; best win rate {:.1}%.",
                    result.total_combos,
                    result.best.result.win_rate * 100.0
                ))
            },
        );
        let minehead_rank = report_run::<idleon_minehead::RankOutcome, MineheadRunInputs>(
            cx,
            &mut subscriptions,
            RunStatus {
                section: "research",
                progress: ProgressLine::Message("Ranking upgrades…"),
                cancelled_kept: "Upgrade ranking cancelled; the previous table is unchanged.",
                cancelled_empty: "Upgrade ranking cancelled; nothing was stored.",
            },
            |_, result| Some(format!("Ranked {} upgrades.", result.rows.len())),
        );
        let minehead_path = report_run::<idleon_minehead::Path, MineheadRunInputs>(
            cx,
            &mut subscriptions,
            RunStatus {
                section: "research",
                progress: ProgressLine::Message("Finding the upgrade path…"),
                cancelled_kept: "Upgrade path cancelled; the previous path is unchanged.",
                cancelled_empty: "Upgrade path cancelled; nothing was stored.",
            },
            |_, result| Some(format!("Path ready: {} upgrades.", result.steps.len())),
        );
        let research_trees = crate::sections::research::Trees::new(window, cx);
        let insight_breakdown = ui::formula_tree("insight-formula", window, cx);
        let sailing_trees = SailingTrees::new(window, cx);
        let sailing_sim = report_run::<SimResult, SailingSimInputs>(
            cx,
            &mut subscriptions,
            RunStatus {
                section: "sailing",
                progress: ProgressLine::Message("Simulating the fleet pile…"),
                cancelled_kept: "Fleet pile simulation cancelled; the previous result is unchanged.",
                cancelled_empty: "Fleet pile simulation cancelled; nothing was stored.",
            },
            |_, result| {
                Some(format!(
                    "Fleet pile simulation ready: {:.2} chests of {} trials.",
                    result.pile.count.mean, result.setup.trials
                ))
            },
        );
        let spelunk_sim = report_run::<SpelunkRunResult, SpelunkRunInputs>(
            cx,
            &mut subscriptions,
            RunStatus {
                section: "spelunking",
                progress: ProgressLine::Message("Running the expedition…"),
                cancelled_kept: "Expedition run cancelled; the previous result is unchanged.",
                cancelled_empty: "Expedition run cancelled; nothing was stored.",
            },
            |_, result| {
                Some(format!(
                    "Expedition run ready: depth {} with seed {}.",
                    result.depth, result.seed
                ))
            },
        );
        let spelunk_opt = report_run::<SpelunkSearchResult, SpelunkOptimizerInputs>(
            cx,
            &mut subscriptions,
            RunStatus {
                section: "spelunking",
                progress: ProgressLine::Message("Optimizing the expedition…"),
                cancelled_kept: "Expedition optimization cancelled; the previous recommendation is unchanged.",
                cancelled_empty: "Expedition optimization cancelled; no recommendation was stored.",
            },
            |_, result| {
                Some(match &result.winner {
                    Some(winner) => format!(
                        "Expedition optimization ready: {} {} against a baseline of {}.",
                        result.objective.label(),
                        winner.summary.score,
                        result.baseline.summary.score
                    ),
                    None => "Expedition optimization finished without a recommendation.".into(),
                })
            },
        );
        let ladle_state = LadleSectionState::new(window, cx);
        let ladle_plan = report_run::<LadleDonation, LadlePlanRequest>(
            cx,
            &mut subscriptions,
            RunStatus {
                section: "ladle",
                progress: ProgressLine::Message("Comparing donation schedules…"),
                cancelled_kept: "Donation search cancelled; the previous recommendation is unchanged.",
                cancelled_empty: "Donation search cancelled; nothing was stored.",
            },
            |_, donation| {
                Some(format!(
                    "Donation plan ready: {} ladles over the plan, {} schedules compared.",
                    idleon_ui::format::number(donation.plan.jelly.donated),
                    donation.evaluated
                ))
            },
        );
        let cglunko_breakdown = ui::formula_tree("cglunko-formula", window, cx);
        let clamworks_breakdown = ui::formula_tree("clamworks-formula", window, cx);
        let fishing_breakdown = ui::formula_tree("fishing-formula", window, cx);
        let mastery_breakdown = ui::formula_tree("cooking-mastery-formula", window, cx);
        let fountain_breakdown = ui::formula_tree("fountain-formula", window, cx);
        let jar_breakdown = ui::formula_tree("jar-formula", window, cx);
        let jar_sim = report_run::<JarSimResult, JarSimInputs>(
            cx,
            &mut subscriptions,
            RunStatus {
                section: "jar",
                progress: ProgressLine::Message("Simulating enchants…"),
                cancelled_kept: "Enchant simulation cancelled; the previous spread is unchanged.",
                cancelled_empty: "Enchant simulation cancelled; nothing was stored.",
            },
            |_, result| {
                Some(format!(
                    "Enchant simulation ready: {} enchants over {} sims with seed {}.",
                    result.enchants, result.sims, result.seed
                ))
            },
        );
        let stats_breakdown = ui::formula_tree("stats-formula", window, cx);
        let stats_explorer_formula = ui::formula_tree("stats-explorer-formula", window, cx);
        let sneaking_breakdown = ui::formula_tree("sneaking-formula", window, cx);
        let sneaking_opt = report_run::<idleon_sneaking::Outcome, idleon_sneaking::Request>(
            cx,
            &mut subscriptions,
            RunStatus {
                section: "sneaking",
                progress: ProgressLine::Message("Optimizing the sneaking charm placement…"),
                cancelled_kept: "Sneaking optimizer cancelled; the previous recommendation is unchanged.",
                cancelled_empty: "Sneaking optimizer cancelled; nothing was stored, and the loadout is untouched.",
            },
            |_, outcome| {
                Some(if outcome.improved() {
                    format!(
                        "Sneaking optimizer ready: {} {:.3} → {:.3} ({:+.1}%).",
                        outcome.request.objective.unit(),
                        outcome.baseline.score,
                        outcome.winner.score,
                        outcome.gain_percent()
                    )
                } else {
                    format!(
                        "Sneaking optimizer: the saved placement already scores best for {}.",
                        outcome.request.objective.label()
                    )
                })
            },
        );
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        // A session file means there is something to restore, so the window
        // veils for the loader and reads it off the main thread; a first launch
        // has nothing to wait for and reads the missing file where it stands.
        let session_path = Session::path(state_dir.clone());
        let restoring = session_path.is_file();
        let session = if restoring {
            Session::blank(session_path.clone())
        } else {
            Session::load(session_path.clone())
        };
        let royal_optimizer = report_run::<
            idleon_royal_guardian::Outcome,
            idleon_royal_guardian::Request,
        >(
            cx,
            &mut subscriptions,
            RunStatus {
                section: "royal-guardian",
                progress: ProgressLine::Message("Optimizing the Royal Guardian kingdom…"),
                cancelled_kept: "Royal Guardian optimizer cancelled; the previous layout is unchanged.",
                cancelled_empty: "Royal Guardian optimizer cancelled; nothing was stored.",
            },
            |_, _| Some("Royal Guardian optimizer ready.".into()),
        );
        let royal_unit_exp = report_run::<
            idleon_royal_guardian::UnitExpPlan,
            idleon_royal_guardian::UnitExpRequest,
        >(
            cx,
            &mut subscriptions,
            RunStatus {
                section: "royal-guardian",
                progress: ProgressLine::Message("Planning the Royal Guardian unit EXP…"),
                cancelled_kept: "Unit EXP plan cancelled; the previous plan is unchanged.",
                cancelled_empty: "Unit EXP plan cancelled; nothing was planned.",
            },
            |_, _| Some("Unit EXP plan ready.".into()),
        );
        let royal_shelf =
            report_run::<idleon_royal_guardian::ShelfPlan, idleon_royal_guardian::ShelfRequest>(
                cx,
                &mut subscriptions,
                RunStatus {
                    section: "royal-guardian",
                    progress: ProgressLine::Message("Planning the next Royal Guardian shelf…"),
                    cancelled_kept: "Shelf plan cancelled; the previous plan is unchanged.",
                    cancelled_empty: "Shelf plan cancelled; nothing was planned.",
                },
                |_, _| Some("Shelf plan ready.".into()),
            );
        let mut state = Self {
            simulation: starter_simulation(),
            context: SimContext::default(),
            context_base: SimContext::default(),
            effort: Effort::Balanced,
            target: Target::Hours(24.0),
            options: SimOptions::default(),
            assets: Manifest::load_default(),
            history: History::new(Route::Dashboard),
            last_section: None,
            activity: Activity::default(),
            research,
            killroy: KillroyState::default(),
            killroy_forecast: None,
            killroy_forecast_key: None,
            killroy_controls,
            save: None,
            fleet: Fleet::default(),
            sailing: RouteContext::default(),
            sailing_state: SailingState::default(),
            royal: None,
            royal_state: RoyalState::default(),
            royal_breakdown: crate::ui::formula_tree("royal-breakdown", window, cx),
            royal_forecast_inputs: RoyalForecastInputs::new(window, cx),
            royal_forecast: None,
            royal_forecast_key: None,
            royal_optimizer,
            royal_unit_exp,
            royal_shelf,
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
            clamworks_scurge: ScurgeInputs::default(),
            jar_tab: JarTab::default(),
            jar_tier: 0,
            jelly: None,
            jelly_state: JellyState::default(),
            jar_breakdown,
            jar: None,
            jar_state: JarState::default(),
            jar_sim,
            spelunking: SpelunkingState::default(),
            spelunking_save: None,
            spelunk_sim,
            spelunk_opt,
            ladle: None,
            ladle_state,
            ladle_plan,
            mastery: None,
            yellow: None,
            mastery_plan: MasteryPlanState::default(),
            fountain: None,
            fountain_state: FountainState::default(),
            fountain_plan,
            fountain_marble_plan,
            fountain_sim,
            opal: OpalState::new(window, cx),
            sneaking: None,
            sneaking_state: Default::default(),
            sneaking_opt,
            sneaking_breakdown,
            pickle: PickleState::default(),
            prisma,
            statue,
            combat,
            combat_encounter,
            combat_compare,
            stats_all: StatsAllState::default(),
            stats_breakdown,
            stats_explorer_search,
            stats_explorer_formula,
            drop_rate: DropRateState::new(window, cx),
            coin_multi: sections::coin_multi::State::new(window, cx),
            damage: DamageState::default(),
            damage_mastery: crate::ui::formula_tree("damage-mastery", window, cx),
            damage_formula: crate::ui::formula_tree("damage-formula", window, cx),
            coral: CoralState::default(),
            worship: WorshipState::default(),
            worship_inputs: WorshipInputs::default(),
            search,
            monument_search,
            status: "Ready. Pick a calculator, or open the research grid.".into(),
            supplements_open: true,
            import,
            session,
            restoring,
            launch_import,
            launch_section: section,
            export: Export::default(),
            save_partial: false,
            save_loaded: false,
            save_revision: 0,
            optimizer,
            killroy_run,
            coral_run,
            jelly_run,
            jelly_plan_run,
            jelly_optimize_run,
            jelly_sim_run,
            jelly_breakdown: crate::ui::formula_tree("jelly-breakdown", window, cx),
            cog_run,
            cog_perfect_run,
            cglunko_run,
            clamworks_run,
            worship_plan,
            worship_optimize,
            prisma_run,
            farming_run,
            farming_compare_run,
            breakdown,
            minehead_breakdown,
            sushi_bucks,
            sushi_fuel,
            sushi_capacity,
            minehead_optimize,
            minehead_rank,
            minehead_path,
            research_trees,
            cglunko_breakdown,
            clamworks_breakdown,
            fishing_breakdown,
            mastery_breakdown,
            fountain_breakdown,
            focus,
            sidebar_scroll: ScrollHandle::new(),
            views: None,
            #[cfg(feature = "automation")]
            _bridge: bridge,
            shape_run,
            insight_run,
            unlock_run,
            tree_run,
            insight_breakdown,
            _subscriptions: subscriptions,
            session_save_pending: false,
        };
        if state.restoring {
            state.restore_session(session_path, window, cx);
        } else {
            state.apply_launch(window, cx);
        }
        // The observer saves after the burst of edits a drag or a typing run
        // produces; the release hook catches the last one when the window goes.
        cx.defer_in(window, |state, _window, cx| {
            let save = cx.observe_self(|state, cx| state.schedule_session_save(cx));
            let flush = cx.on_release(|state, _cx| state.flush_session());
            state._subscriptions.extend([save, flush]);
        });
        state
    }

    /// Where the window is: the current place in the history.
    pub(crate) fn route(&self) -> &Route {
        self.history.current()
    }

    /// The open section's registry entry, or `None` on the dashboard.
    pub(crate) fn current_section(&self) -> Option<&'static Section> {
        self.route().section()
    }

    /// Read the session file off the main thread and hand it to
    /// [`Self::adopt_session`], so the window is on screen while the save it
    /// restores decodes.
    fn restore_session(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        cx.spawn_in(window, async move |this, cx| {
            let session = cx
                .background_spawn(async move { Session::load(path) })
                .await;
            this.update_in(cx, |state, window, cx| {
                state.adopt_session(session, window, cx)
            })
            .ok();
        })
        .detach();
    }

    /// Take the session that finished reading: restore its project, apply this
    /// launch's `--import` and `--section` on top, and save from here on.
    fn adopt_session(&mut self, session: Session, window: &mut Window, cx: &mut Context<Self>) {
        self.session = session;
        self.restoring = false;
        if let Some(restore) = self.session.restore.take() {
            let mut warnings = restore.project.document.warnings.clone();
            let mut changes = Vec::new();
            crate::project::apply(
                self,
                restore.project,
                restore.configs,
                &mut warnings,
                &mut changes,
                cx,
            );
            self.save_partial |= !warnings.is_empty();
            self.status = match warnings.first() {
                None => "Restored the last session.".into(),
                Some(first) if warnings.len() == 1 => {
                    format!("Restored the last session with 1 warning: {first}")
                }
                Some(first) => format!(
                    "Restored the last session with {} warnings: {first} (and {} more)",
                    warnings.len(),
                    warnings.len() - 1
                ),
            };
        }
        if let Some(notice) = self.session.notice.clone() {
            self.status = notice;
        }
        self.apply_launch(window, cx);
        cx.notify();
    }

    /// Apply the `--import` and `--section` this launch named, in that order:
    /// an import replaces the save and the restore's section, and `--section`
    /// then reopens the section the harness asked for.
    fn apply_launch(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(path) = self.launch_import.take() {
            self.import_file(&path, window, cx);
        }
        if let Some(id) = self.launch_section.take() {
            self.open_place(Route::open(&id), window, cx);
        }
    }

    /// Save the session now, for a route change, an applied import or a close.
    pub(crate) fn flush_session(&mut self) {
        // A window still restoring has no session of its own: writing here would
        // replace the file it is reading with this window's defaults.
        if self.restoring {
            return;
        }
        let project = session::project_value(self);
        if let Err(error) = self.session.save(&project) {
            eprintln!("idleon-optimizer: could not save the session: {error}");
            log::error!("could not save the session: {error}");
            self.status = format!(
                "Could not save the session ({error}); this window's state will not survive a restart."
            );
        }
    }

    /// Let a burst of edits settle before writing the session once.
    fn schedule_session_save(&mut self, cx: &mut Context<Self>) {
        if self.session_save_pending {
            return;
        }
        self.session_save_pending = true;
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(session::SAVE_DEBOUNCE).await;
            this.update(cx, |state, _cx| {
                state.session_save_pending = false;
                state.flush_session();
            })
            .ok();
        })
        .detach();
    }

    /// Open a place as a new history step: Home, a section, or an id the
    /// registry does not declare, which lands on the missing-section state.
    /// Opening the place already showing does nothing.
    pub(crate) fn open_place(&mut self, place: Route, window: &mut Window, cx: &mut Context<Self>) {
        if self.history.open(place) {
            self.arrive(window, cx);
        }
    }

    /// Step back to the previous place.
    pub(crate) fn go_back(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.history.go_back() {
            self.arrive(window, cx);
        }
    }

    /// Step forward to the next place.
    pub(crate) fn go_forward(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.history.go_forward() {
            self.arrive(window, cx);
        }
    }

    /// Jump to a place the history list picked, without adding a step.
    pub(crate) fn go_to(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.history.go_to(index) {
            self.arrive(window, cx);
        }
    }

    /// Show the current place, keeping every section's state alive.
    fn arrive(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.status = match self.route().clone() {
            Route::Dashboard => "Dashboard. Pick a calculator.".into(),
            Route::Section(section) => {
                self.last_section = Some(section.id);
                self.activity.open(section.id);
                format!("{} · {}", section.title, section.description)
            }
            Route::Missing(id) => format!("Unknown section: {id}."),
        };
        self.flush_session();
        window.focus(&self.focus, cx);
        cx.notify();
    }

    /// Fold the sidebar to the rail, or expand it again, and remember the choice.
    pub(crate) fn toggle_sidebar(&mut self, cx: &mut Context<Self>) {
        let shell = &mut self.session.shell;
        shell.sidebar = match shell.sidebar {
            SidebarFold::Expanded => SidebarFold::Rail,
            SidebarFold::Rail => SidebarFold::Expanded,
        };
        self.flush_session();
        cx.notify();
    }

    /// Reopen the last section, restored to the state it was left in.
    pub(crate) fn resume_last_section(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.last_section else {
            return self.set_status("No section opened yet.", cx);
        };
        self.open_place(Route::open(id), window, cx);
    }

    /// The shared header's save status. While the dialog is open it owns the
    /// verdict: a half-typed paste must not relabel the loaded run.
    pub(crate) fn save_status(&self) -> SaveStatus {
        if self.restoring {
            return SaveStatus::Restoring;
        }
        SaveStatus::of(
            self.save_loaded,
            self.import.ok || self.import.open,
            self.save_partial,
        )
    }

    /// Empty the dashboard search; Escape calls this.
    pub(crate) fn clear_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.search
            .update(cx, |search, cx| search.set_value("", window, cx));
        cx.notify();
    }
}

/// The research run a fresh window opens on: a level-100 seeded grid with two
/// glasses on the richest observations that level can already use.
fn starter_simulation() -> Simulation {
    let mut simulation = Simulation {
        research_level: 100,
        grid: idleon_research::GridState::seeded(1),
        ..Simulation::default()
    };
    simulation.observations.found.fill(true);
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
    simulation
}

/// Both UI libraries, set up as a window state needs them under GPUI's test
/// platform. Ely's `init_for_tests` stands in for `init`, which needs the
/// bundled fonts the test platform has no asset source for.
#[cfg(test)]
pub(crate) fn init_for_tests(cx: &mut gpui::App) {
    ely_gpui_component::init_for_tests(cx);
    gpui_kit::init(cx);
}

/// A window state built under GPUI's test platform, its session and
/// automation endpoint kept in a private directory; without an endpoint
/// directory the bridge would register in the user's real one.
#[cfg(test)]
pub(crate) fn headless(
    cx: &mut gpui::TestAppContext,
    dir: &std::path::Path,
) -> gpui::WindowHandle<AppState> {
    cx.update(init_for_tests);
    let (endpoint, state_dir) = (dir.join("endpoint"), dir.join("state"));
    let window = cx.add_window(|window, cx| {
        AppState::new(window, Some(endpoint), None, Some(state_dir), None, cx)
    });
    // The window opens before the session file is read, off the main thread.
    cx.run_until_parked();
    window
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
        assert_eq!(section.status, sections::Status::Ready);
    }

    /// A window launched with `--import <path>`.
    fn launched_with_import(
        cx: &mut gpui::TestAppContext,
        dir: &std::path::Path,
        import: std::path::PathBuf,
    ) -> gpui::WindowHandle<AppState> {
        cx.update(init_for_tests);
        let (endpoint, state_dir) = (dir.join("endpoint"), dir.join("state"));
        let window = cx.add_window(|window, cx| {
            AppState::new(
                window,
                Some(endpoint),
                None,
                Some(state_dir),
                Some(import),
                cx,
            )
        });
        // The launch import lands with the session the window opened ahead of.
        cx.run_until_parked();
        window
    }

    #[test]
    fn a_launch_import_applies_the_file_like_the_import_dialog() {
        let dir = std::env::temp_dir().join(format!("idleon-launch-import-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let fixture =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/data.json");
        let mut cx = gpui::TestAppContext::single();
        let window = launched_with_import(&mut cx, &dir, fixture);
        window
            .update(&mut cx, |state, _, _| {
                assert!(state.save_loaded, "{}", state.status);
                assert!(!state.import.open);
                assert_eq!(
                    state
                        .session
                        .recent
                        .first()
                        .map(|entry| entry.source.as_str()),
                    Some("data.json")
                );
            })
            .expect("the window is open");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_launch_import_that_cannot_be_read_says_so_and_loads_nothing() {
        let dir =
            std::env::temp_dir().join(format!("idleon-launch-missing-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut cx = gpui::TestAppContext::single();
        let window = launched_with_import(&mut cx, &dir, dir.join("missing.json"));
        window
            .update(&mut cx, |state, _, _| {
                assert!(!state.save_loaded);
                assert!(
                    state.status.starts_with("Could not read missing.json"),
                    "{}",
                    state.status
                );
            })
            .expect("the window is open");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_sidebar_toggle_folds_to_the_rail_and_the_next_launch_restores_it() {
        let dir = std::env::temp_dir().join(format!("idleon-sidebar-fold-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut cx = gpui::TestAppContext::single();
        let window = headless(&mut cx, &dir);
        let path = Session::path(Some(dir.join("state")));
        window
            .update(&mut cx, |state, _, cx| {
                assert_eq!(state.session.shell.sidebar, SidebarFold::Expanded);
                state.toggle_sidebar(cx);
                assert_eq!(state.session.shell.sidebar, SidebarFold::Rail);
            })
            .expect("the window is open");
        assert_eq!(Session::load(path.clone()).shell.sidebar, SidebarFold::Rail);

        window
            .update(&mut cx, |state, _, cx| state.toggle_sidebar(cx))
            .expect("the window is open");
        assert_eq!(Session::load(path).shell.sidebar, SidebarFold::Expanded);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn two_sections_run_at_once_and_both_store_their_results() {
        let dir = std::env::temp_dir().join(format!("idleon-two-runs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let fixture =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/data.json");
        let mut cx = gpui::TestAppContext::single();
        let window = launched_with_import(&mut cx, &dir, fixture.clone());
        let ring = Some(Shown::Running {
            runs: 1,
            fraction: 0.0,
        });
        window
            .update(&mut cx, |state, window, cx| {
                state.open_place(Route::open("farming"), window, cx);
                state.run_farming(cx);
                state.open_place(Route::open("sailing"), window, cx);
                state.set_sailing_sim_trials(1, cx);
                state.set_sailing_sim_hours(1.0, cx);
                state.run_sailing_simulation(cx);
            })
            .expect("the window is open");
        window
            .update(&mut cx, |state, _, cx| {
                assert!(state.farming_run.read(cx).running(), "{}", state.status);
                assert!(state.sailing_sim.read(cx).running(), "{}", state.status);
                assert_eq!(state.activity.shown("farming"), ring);
                assert_eq!(state.activity.shown("sailing"), ring);
            })
            .expect("the window is open");

        cx.run_until_parked();
        window
            .update(&mut cx, |state, window, cx| {
                assert!(state.farming_run.read(cx).result().is_some());
                assert!(state.sailing_sim.read(cx).result().is_some());
                // Farming finished while Sailing was open; Sailing's own run
                // finished in view.
                assert_eq!(
                    state.activity.shown("farming"),
                    Some(Shown::Mark(Mark::Finished))
                );
                assert_eq!(state.activity.shown("sailing"), None);

                state.open_place(Route::open("farming"), window, cx);
                assert_eq!(state.activity.shown("farming"), None);

                state.open_place(Route::open("sailing"), window, cx);
                state.run_farming(cx);
            })
            .expect("the window is open");
        cx.run_until_parked();
        window
            .update(&mut cx, |state, window, cx| {
                assert_eq!(
                    state.activity.shown("farming"),
                    Some(Shown::Mark(Mark::Finished))
                );
                state.import_file(&fixture, window, cx);
                assert_eq!(state.activity.shown("farming"), None, "{}", state.status);
            })
            .expect("the window is open");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A project document a session file can hold, with no save.
    fn session_project() -> serde_json::Value {
        idleon_project::value(&idleon_project::Project {
            version: idleon_project::VERSION,
            section: None,
            save: None,
            simulation: Default::default(),
            context: Default::default(),
            sections: Default::default(),
            warnings: Vec::new(),
            has_research: false,
            has_supplements: false,
        })
    }

    #[test]
    fn a_window_opens_before_its_session_is_read_and_writes_nothing_meanwhile() {
        let dir = std::env::temp_dir().join(format!("idleon-restoring-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let state_dir = dir.join("state");
        std::fs::create_dir_all(&state_dir).expect("the state directory");
        let path = Session::path(Some(state_dir.clone()));
        // A session the window will find: a folded sidebar is enough to tell
        // whether it landed.
        let mut session = Session::blank(path.clone());
        session.shell.sidebar = SidebarFold::Rail;
        session.save(&session_project()).expect("the session saves");
        let written = std::fs::read(&path).expect("the session file");

        let mut cx = gpui::TestAppContext::single();
        cx.update(init_for_tests);
        let endpoint = dir.join("endpoint");
        let window = cx.add_window(|window, cx| {
            AppState::new(window, Some(endpoint), None, Some(state_dir), None, cx)
        });
        window
            .update(&mut cx, |state, _, _| {
                assert!(state.restoring, "the window opens ahead of its session");
                assert_eq!(
                    state.save_status(),
                    SaveStatus::Restoring,
                    "the header announces the load instead of no save"
                );
                // Nothing writes the file the window is still reading from.
                state.flush_session();
            })
            .expect("the window is open");
        assert_eq!(
            std::fs::read(&path).expect("the session file"),
            written,
            "a restoring window must not overwrite the session it restores"
        );

        cx.run_until_parked();
        window
            .update(&mut cx, |state, _, _| {
                assert!(!state.restoring);
                assert_eq!(
                    state.session.shell.sidebar,
                    SidebarFold::Rail,
                    "the session landed"
                );
            })
            .expect("the window is open");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_next_launch_restores_the_imported_save_after_the_loader() {
        let dir = std::env::temp_dir().join(format!("idleon-restored-save-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let fixture =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/data.json");
        let mut cx = gpui::TestAppContext::single();
        let window = launched_with_import(&mut cx, &dir, fixture);
        window
            .update(&mut cx, |state, _, _| assert!(state.save_loaded))
            .expect("the window is open");

        // A second window on the same state directory opens ahead of the
        // session and lands on the save the first one wrote.
        let (endpoint, state_dir) = (dir.join("endpoint"), dir.join("state"));
        let restored = cx.add_window(|window, cx| {
            AppState::new(window, Some(endpoint), None, Some(state_dir), None, cx)
        });
        restored
            .update(&mut cx, |state, _, _| assert!(state.restoring))
            .expect("the window is open");
        cx.run_until_parked();
        restored
            .update(&mut cx, |state, _, _| {
                assert!(!state.restoring);
                assert!(state.save_loaded, "{}", state.status);
                assert!(state.status.starts_with("Restored the last session"));
            })
            .expect("the window is open");

        // A lane's `--section`: the restore lands first, then the launch
        // section opens on top of it.
        let (endpoint, state_dir) = (dir.join("endpoint"), dir.join("state"));
        let sectioned = cx.add_window(|window, cx| {
            AppState::new(
                window,
                Some(endpoint),
                Some("farming".into()),
                Some(state_dir),
                None,
                cx,
            )
        });
        cx.run_until_parked();
        sectioned
            .update(&mut cx, |state, _, _| {
                assert!(state.save_loaded, "{}", state.status);
                assert_eq!(
                    state.route().section().map(|section| section.id),
                    Some("farming")
                );
            })
            .expect("the window is open");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_stats_and_damage_pages_resolve_their_own_sheets_on_the_first_render() {
        let dir = std::env::temp_dir().join(format!("idleon-lazy-sheets-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let fixture =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/data.json");
        let mut cx = gpui::TestAppContext::single();
        let window = launched_with_import(&mut cx, &dir, fixture);
        // An import leaves the two expensive pages for their own first render.
        window
            .update(&mut cx, |state, _, _| {
                assert!(state.stats_all.sheet.is_none());
                assert!(state.damage.summary.is_none());
            })
            .expect("the window is open");
        for (id, resolved) in [
            (
                "stats-all",
                (|state: &AppState| state.stats_all.sheet.is_some()) as fn(&AppState) -> bool,
            ),
            ("damage", |state: &AppState| state.damage.summary.is_some()),
        ] {
            window
                .update(&mut cx, |state, window, cx| {
                    state.open_place(Route::open(id), window, cx);
                })
                .expect("the window is open");
            cx.run_until_parked();
            window
                .update(&mut cx, |state, _, _| {
                    assert!(resolved(state), "{id} resolved its sheet when it rendered");
                })
                .expect("the window is open");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn save_status_prefers_a_loaded_save_over_a_later_failure() {
        assert_eq!(SaveStatus::of(false, true, false), SaveStatus::Empty);
        assert_eq!(SaveStatus::of(false, false, false), SaveStatus::Error);
        assert_eq!(SaveStatus::of(true, true, false), SaveStatus::Loaded);
        assert_eq!(SaveStatus::of(true, false, false), SaveStatus::Loaded);
        assert_eq!(SaveStatus::of(true, true, true), SaveStatus::Partial);
        assert_eq!(SaveStatus::Restoring.label(), "Loading your save…");
        assert_eq!(SaveStatus::Loaded.label(), "Save imported");
        assert_eq!(SaveStatus::Partial.label(), "Save imported with warnings");
    }
}
