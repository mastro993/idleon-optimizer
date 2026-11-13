//! Application state: where the window is, the simulation being edited and the run in flight.

mod documents;
mod runs;
mod subscriptions;

use crate::assets::Manifest;
use crate::sections::cglunko::Tab as CglunkoTab;
use crate::sections::clamworks::{State as ClamworksState, Tab as ClamworksTab};
use crate::sections::cog::{
    PerfectRunInputs as CogPerfectInputs, RunInputs as CogRunInputs, State as CogState,
};
use crate::sections::coin_multi;
use crate::sections::combat_sim::State as CombatState;
use crate::sections::cooking_mastery::State as MasteryPlanState;
use crate::sections::coral_god_rank::State as CoralState;
use crate::sections::damage::State as DamageState;
use crate::sections::drop_rate::State as DropRateState;
use crate::sections::endgame;
use crate::sections::farming::{
    CompareInputs as FarmingCompareInputs, RunInputs as FarmingRunInputs, State as FarmingState,
};
use crate::sections::fishing::State as FishingState;
use crate::sections::fountain::State as FountainState;
use crate::sections::jar::State as JarState;
use crate::sections::jar::Tab as JarTab;
use crate::sections::jelly_operator::{
    PlannerRunInputs as JellyPlanRunInputs, RunInputs as JellyRunInputs,
    SimulatorRunInputs as JellySimRunInputs, State as JellyState,
};
use crate::sections::killroy::{KillroyOutcome, State as KillroyState};
use crate::sections::ladle::State as LadleSectionState;
use crate::sections::monument::State as MonumentState;
use crate::sections::opal::Tab as OpalTab;
use crate::sections::pickle::State as PickleState;
use crate::sections::prisma::{RunInputs as PrismaRunInputs, State as PrismaState};
use crate::sections::research::State as ResearchState;
use crate::sections::research::minehead::{
    OptimizeInputs as MineheadOptimizeInputs, RunInputs as MineheadRunInputs,
};
use crate::sections::royal_guardian::State as RoyalState;
use crate::sections::royal_guardian::forecast::Inputs as RoyalForecastInputs;
use crate::sections::sailing::State as SailingState;
use crate::sections::sailing::Trees as SailingTrees;
use crate::sections::sailing::simulation::RunInputs as SailingSimInputs;
use crate::sections::spelunking::RunInputs as SpelunkRunInputs;
use crate::sections::spelunking::State as SpelunkingState;
use crate::sections::stats_all::State as StatsAllState;
use crate::sections::statue::State as StatueState;
use crate::sections::worship_td::{OptimizeRunInputs, PlanRunInputs, State as WorshipState};
use crate::sections::{self, Section};
use crate::session::{self, Session};
use crate::ui;
use crate::ui::export::Export;
use crate::ui::import::Import;
use ely_gpui_component::forms::{InputEvent, TextInput};
use gpui::{Context, Entity, FocusHandle, Subscription, Window};
use gpui_mcp::BridgeHandle;
use idleon_cglunko::Cglunko;
use idleon_cglunko::{Page as CglunkoPage, Plan as CglunkoPlan};
use idleon_clamworks::Clamworks;
use idleon_clamworks::{Plan as ClamworksPlan, PlanInput as ClamworksPlanInput};
use idleon_cog::{CogResult, PerfectBoard};
use idleon_combat::encounter::Options as CombatEncounterOptions;
use idleon_combat::encounter::Outcome as CombatEncounterOutcome;
use idleon_cooking_mastery::Mastery;
use idleon_coral::{Inputs as CoralInputs, Plan as CoralPlan};
use idleon_farming::{Allocation as FarmingOutcome, Comparison as FarmingComparison, Farm};
use idleon_fishing::FishingSave;
use idleon_fountain::{Fountain, OptimizerPlan, PlanRequest};
use idleon_import::Shadow;
use idleon_jar::{Jar, SimInputs as JarSimInputs, SimResult as JarSimResult};
use idleon_jelly::planner::Plan as JellySectionPlan;
use idleon_jelly::{Estimate as JellyEstimate, Jelly, Trial as JellyTrial};
use idleon_killroy::Plan as KillroyPlan;
use idleon_ladle::Ladle;
use idleon_monument as monument;
use idleon_opal::Opal;
use idleon_opal::{Plan as OpalPlan, PlanInput as OpalPlanInput};
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
use idleon_ui::formula::FormulaTree;
use idleon_ui::runner::TaskRunner;
use idleon_worship::Inputs as WorshipInputs;
use idleon_worship::{self as worship, RunResult as WorshipRunResult};
pub(crate) use runs::discard_run;
pub(crate) use subscriptions::redraw_on_edit;
use subscriptions::{ProgressLine, RunStatus, report_run};

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
    /// The three upstream run toggles the next projection reads.
    pub(crate) options: SimOptions,
    /// Upstream artwork resolved through `assets/upstream/manifest.json`.
    pub(crate) assets: Manifest,
    /// Where the window is: the dashboard or one section.
    pub(crate) route: Route,
    /// The last section opened; the dashboard offers to resume it.
    pub(crate) last_section: Option<&'static str>,
    /// The research section's own state, kept alive across dashboard visits.
    pub(crate) research: ResearchState,
    /// The Killroy section's forecast settings.
    pub(crate) killroy: KillroyState,
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
    pub(crate) farming_name: Entity<TextInput>,
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
    /// The endgame section's audit of the last applied save.
    pub(crate) endgame: endgame::State,
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
    /// The ladle optimizer's save-derived roster and counts.
    pub(crate) ladle: Option<Ladle>,
    /// The ladle section's selectors, panel inputs and formula trees.
    pub(crate) ladle_state: LadleSectionState,
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
    /// The Opal Optimizer's villagers and account bonuses, when the save has them.
    pub(crate) opal: Option<Opal>,
    /// The villager the opal breakdown is switched to, or the total.
    pub(crate) opal_selected: Option<usize>,
    /// The Opal Optimizer's tab, kept across dashboard visits.
    pub(crate) opal_tab: OpalTab,
    /// The villager the opal optimizer maximizes, or `None` for the total.
    pub(crate) opal_target: Option<usize>,
    /// The opal total the optimizer distributes, seeded from the save.
    pub(crate) opal_total: f64,
    /// The manual simulator's opal counts, seeded from and reset to the save.
    pub(crate) opal_manual: [f64; idleon_opal::VILLAGERS],
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
    /// The All Stats Dashboard's calculation context, save slice and breakdowns.
    pub(crate) stats_all: StatsAllState,
    /// The shared formula tree on the All Stats breakdown card.
    pub(crate) stats_breakdown: Entity<FormulaTree>,
    /// The formula explorer's search field.
    pub(crate) stats_explorer_search: Entity<TextInput>,
    /// The shared formula tree on the All Stats formula card.
    pub(crate) stats_explorer_formula: Entity<FormulaTree>,
    /// The Drop Rate Calculator's context, save slice and live dungeon fields.
    pub(crate) drop_rate: DropRateState,
    /// The Coin Multiplier Calculator's cards and damage field.
    pub(crate) coin_multi: coin_multi::State,
    /// The Damage Breakdown's character/map context and summary.
    pub(crate) damage: DamageState,
    /// The coral god rank planner's inputs, kept alive across dashboard visits.
    pub(crate) coral: CoralState,
    /// The Worship TD simulator's config and live run.
    pub(crate) worship: WorshipState,
    /// The tower trial levels and account values the live run reads.
    pub(crate) worship_inputs: WorshipInputs,
    /// The dashboard search field.
    pub(crate) search: Entity<TextInput>,
    /// The Justice case search field.
    pub(crate) monument_search: Entity<TextInput>,
    pub(crate) status: String,
    /// Whether the save-derived supplements card is expanded.
    pub(crate) supplements_open: bool,
    /// The shared import dialog every section's header opens.
    pub(crate) import: Import,
    /// The window's own local memory: the last project and the recent imports.
    pub(crate) session: Session,
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
    /// The shared runner for the Killroy comparison's trials.
    pub(crate) killroy_run: Entity<TaskRunner<KillroyOutcome, KillroyPlan>>,
    /// The shared runner for the coral god rank beam search.
    pub(crate) coral_run: Entity<TaskRunner<CoralPlan, CoralInputs>>,
    /// The shared runner for the Jelly Operator operation estimate.
    pub(crate) jelly_run: Entity<TaskRunner<JellyEstimate, JellyRunInputs>>,
    pub(crate) jelly_plan_run: Entity<TaskRunner<JellySectionPlan, JellyPlanRunInputs>>,
    /// The shared runner for one simulator operation.
    pub(crate) jelly_sim_run: Entity<TaskRunner<JellyTrial, JellySimRunInputs>>,
    /// The shared runner for the cog board optimizer.
    pub(crate) cog_run: Entity<TaskRunner<CogResult, CogRunInputs>>,
    /// The shared runner for the perfect cog board.
    pub(crate) cog_perfect_run: Entity<TaskRunner<PerfectBoard, CogPerfectInputs>>,
    /// The shared runner for the Cglunko upgrade plan.
    pub(crate) cglunko_run: Entity<TaskRunner<CglunkoPlan, CglunkoPage>>,
    /// The shared runner for the Clamworks upgrade plan.
    pub(crate) clamworks_run: Entity<TaskRunner<ClamworksPlan, ClamworksPlanInput>>,
    /// The shared runner for the opal distribution plan.
    pub(crate) opal_run: Entity<TaskRunner<OpalPlan, OpalPlanInput>>,
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
    /// The shared formula tree on the Opal overview.
    pub(crate) opal_breakdown: Entity<FormulaTree>,
    /// The shadow-ban flag the loaded save carries (`OptLacc[26]`).
    pub(crate) shadow: Shadow,
    pub(crate) focus: FocusHandle,
    /// Held so the automation bridge stays installed while the window lives.
    _bridge: BridgeHandle,
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
        cx: &mut Context<Self>,
    ) -> Self {
        let bridge = ui::bridge::install(window, cx, endpoint_dir);
        let import = Import::new(window, cx);
        let search = ui::text_input("Search calculators…", "Search calculators", window, cx);
        let monument_search = ui::text_input(
            "Visitor or words from the request…",
            "Find a Justice case",
            window,
            cx,
        );
        let statue = StatueState::new(window, cx);
        let prisma = PrismaState::new(window, cx);
        let combat = CombatState::new(window, cx);
        let research = ResearchState::new(window, cx);
        let farming_name = ui::text_input("Preset name", "Preset name", window, cx);
        let stats_explorer_search =
            ui::text_input("Name, saved input or source", "Search formulas", window, cx);
        // Filter and name fields redraw the page on every keystroke.
        let mut subscriptions: Vec<Subscription> = [
            &search,
            &monument_search,
            &statue.search,
            &prisma.search,
            &combat.name,
            &combat.talent_search,
            &combat.override_search,
            &research.preset_name,
            &stats_explorer_search,
        ]
        .into_iter()
        .map(|input| redraw_on_edit(cx, input))
        .collect();
        // Every edit of the paste box re-checks the text, so the preview and the
        // Apply button always describe what is on screen.
        subscriptions.push(
            cx.subscribe(&import.input, |state, _input, event: &InputEvent, cx| {
                if *event == InputEvent::Changed {
                    state.check_paste(cx);
                }
            }),
        );

        // Each long run owns a runner; the window mirrors its status line, so
        // every section's run reports the same way in the status bar.
        let optimizer = report_run::<OptimizeResult, RunInputs>(
            cx,
            &mut subscriptions,
            RunStatus {
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
        // A finished cog run opens its walkthrough, which reads the stored
        // board one step at a time until Done keeps it.
        let cog_run = report_run::<CogResult, CogRunInputs>(
            cx,
            &mut subscriptions,
            RunStatus {
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
        let clamworks_run = report_run::<ClamworksPlan, ClamworksPlanInput>(
            cx,
            &mut subscriptions,
            RunStatus {
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
        let opal_run = report_run::<OpalPlan, OpalPlanInput>(
            cx,
            &mut subscriptions,
            RunStatus {
                progress: ProgressLine::Message("Optimizing the opal distribution…"),
                cancelled_kept: "Opal plan cancelled; the previous plan is unchanged.",
                cancelled_empty: "Opal plan cancelled; nothing was planned.",
            },
            |_, plan| {
                Some(format!(
                    "Opal plan ready: {} opals across the villagers.",
                    idleon_ui::format::count(plan.spent() as u64)
                ))
            },
        );
        let worship_plan = report_run::<WorshipRunResult, PlanRunInputs>(
            cx,
            &mut subscriptions,
            RunStatus {
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
        let jelly_sim_run = report_run::<JellyTrial, JellySimRunInputs>(
            cx,
            &mut subscriptions,
            RunStatus {
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
                progress: ProgressLine::Message("Running the encounter…"),
                cancelled_kept: "Encounter run cancelled; the previous result is unchanged.",
                cancelled_empty: "Encounter run cancelled; nothing was stored.",
            },
            |_, result| {
                Some(format!(
                    "Encounter run ready: {} kills over {:.0}s.",
                    result.kills,
                    result.simulated_ms as f64 / 1000.0
                ))
            },
        );
        let minehead_breakdown = ui::formula_tree("minehead-formula", window, cx);
        let sushi_bucks = ui::formula_tree("sushi-bucks-formula", window, cx);
        let sushi_fuel = ui::formula_tree("sushi-fuel-formula", window, cx);
        let sushi_capacity = ui::formula_tree("sushi-capacity-formula", window, cx);
        let minehead_optimize = report_run::<idleon_minehead::Optimized, MineheadOptimizeInputs>(
            cx,
            &mut subscriptions,
            RunStatus {
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
                progress: ProgressLine::Message("Finding the upgrade path…"),
                cancelled_kept: "Upgrade path cancelled; the previous path is unchanged.",
                cancelled_empty: "Upgrade path cancelled; nothing was stored.",
            },
            |_, result| Some(format!("Path ready: {} upgrades.", result.steps.len())),
        );
        let research_trees = crate::sections::research::Trees::new(window, cx);
        let sailing_trees = SailingTrees::new(window, cx);
        let sailing_sim = report_run::<SimResult, SailingSimInputs>(
            cx,
            &mut subscriptions,
            RunStatus {
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
        let ladle_state = LadleSectionState::new(window, cx);
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
        let opal_breakdown = ui::formula_tree("opal-formula", window, cx);
        let stats_breakdown = ui::formula_tree("stats-formula", window, cx);
        let stats_explorer_formula = ui::formula_tree("stats-explorer-formula", window, cx);
        let sneaking_breakdown = ui::formula_tree("sneaking-formula", window, cx);
        let sneaking_opt = report_run::<idleon_sneaking::Outcome, idleon_sneaking::Request>(
            cx,
            &mut subscriptions,
            RunStatus {
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
        let session = Session::load(Session::path(state_dir));
        let mut state = Self {
            simulation: starter_simulation(),
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
            royal_breakdown: crate::ui::formula_tree("royal-breakdown", window, cx),
            royal_forecast_inputs: RoyalForecastInputs::new(window, cx),
            royal_forecast: None,
            royal_forecast_key: None,
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
            jar_state: JarState::default(),
            jar_sim,
            spelunking: SpelunkingState::default(),
            spelunking_save: None,
            spelunk_sim,
            ladle: None,
            ladle_state,
            mastery: None,
            yellow: None,
            mastery_plan: MasteryPlanState::default(),
            fountain: None,
            fountain_state: FountainState::default(),
            fountain_plan,
            opal: None,
            opal_selected: None,
            opal_tab: OpalTab::default(),
            opal_target: None,
            opal_total: 0.0,
            opal_manual: [0.0; idleon_opal::VILLAGERS],
            sneaking: None,
            sneaking_state: Default::default(),
            sneaking_opt,
            sneaking_breakdown,
            pickle: PickleState::default(),
            prisma,
            statue,
            combat,
            combat_encounter,
            stats_all: StatsAllState::default(),
            stats_breakdown,
            stats_explorer_search,
            stats_explorer_formula,
            drop_rate: DropRateState::new(window, cx),
            coin_multi: sections::coin_multi::State::new(window, cx),
            damage: DamageState::default(),
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
            jelly_plan_run,
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
            opal_breakdown,
            shadow: Shadow::default(),
            focus,
            _bridge: bridge,
            shape_run,
            _subscriptions: subscriptions,
            session_save_pending: false,
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
            let save = cx.observe_self(|state, cx| state.schedule_session_save(cx));
            let flush = cx.on_release(|state, _cx| state.flush_session());
            state._subscriptions.extend([save, flush]);
        });
        state
    }

    /// The open section's registry entry, or `None` on the dashboard.
    pub(crate) fn current_section(&self) -> Option<&'static Section> {
        self.route.section()
    }

    /// Save the session now, for a route change, an applied import or a close.
    pub(crate) fn flush_session(&mut self) {
        let Some(project) = session::project_value(self) else {
            return;
        };
        if let Err(error) = self.session.save(&project) {
            eprintln!("idleon-optimizer: could not save the session: {error}");
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

    /// Open a section, leaving the dashboard. An id the registry does not declare
    /// lands on the missing-section state instead of panicking.
    pub(crate) fn open_section(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.route = Route::open(id);
        match self.route.section() {
            Some(section) => {
                self.last_section = Some(section.id);
                self.status = format!("{} · {}", section.title, section.description);
            }
            None => self.status = format!("Unknown section: {id}."),
        }
        self.flush_session();
        window.focus(&self.focus, cx);
        cx.notify();
    }

    /// Return to the landing dashboard, keeping every section's state alive.
    pub(crate) fn back_to_dashboard(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.route = Route::Dashboard;
        self.status = "Dashboard. Pick a calculator.".into();
        self.flush_session();
        window.focus(&self.focus, cx);
        cx.notify();
    }

    /// Reopen the last section, restored to the state it was left in.
    pub(crate) fn resume_last_section(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.last_section else {
            return self.set_status("No section opened yet.", cx);
        };
        self.open_section(id, window, cx);
    }

    /// The shared header's save status. While the dialog is open it owns the
    /// verdict: a half-typed paste must not relabel the loaded run.
    pub(crate) fn save_status(&self) -> SaveStatus {
        SaveStatus::of(
            self.save_loaded,
            self.import.ok || self.import.open,
            self.save_partial,
        )
    }

    /// Empty the dashboard search; Escape calls this.
    pub(crate) fn clear_search(&mut self, cx: &mut Context<Self>) {
        self.search.update(cx, |search, cx| search.set_text("", cx));
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
