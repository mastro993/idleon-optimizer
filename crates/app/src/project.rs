//! The app's project file: what an export writes and an import applies.
//!
//! [`idleon_project`] owns the document — the format markers, the
//! version, the research state, the supplements and the ROv2 migration. This
//! module owns the part that is the window's: which section it was on and the
//! config object each section keeps as its visible state.
//!
//! Every section config is a plain serde struct, so an export is a `to_value`
//! and an import a `from_value`; a config whose field this build does not know
//! is named in a warning and the section keeps what it had, never a panic.

use crate::sections;
use crate::sections::clamworks::Tab as ClamworksTab;
use crate::sections::clamworks::state::StandIn;
use crate::sections::combat_sim::Tab as CombatTab;
use crate::sections::cooking_mastery::State as CookingMasteryState;
use crate::sections::farming::State as FarmingState;
use crate::sections::fishing::State as FishingState;
use crate::sections::fountain::Tab as FountainTab;
use crate::sections::jar::State as JarState;
use crate::sections::jar::Tab as JarTab;
use crate::sections::jelly_operator::Planner as JellyPlanner;
use crate::sections::jelly_operator::Tab as JellyTab;
use crate::sections::killroy::State as KillroyState;
use crate::sections::ladle::Tab as LadleTab;
use crate::sections::ladle::outcomes::OutcomeSort as LadleOutcomeSort;
use crate::sections::monument::State as MonumentState;
use crate::sections::opal::Tab as OpalTab;
use crate::sections::pickle::State as PickleState;
use crate::sections::research::CustomPreset;
use crate::sections::research::Tab as ResearchTab;
use crate::sections::royal_guardian::Tab as RoyalTab;
use crate::sections::sailing::State as SailingState;
use crate::sections::sailing::Tab as SailingTab;
use crate::sections::sneaking::State as SneakingState;
use crate::sections::spelunking::State as SpelunkingState;
use crate::sections::stats_all;
use crate::sections::statue::Sort as StatueSort;
use crate::sections::worship_td::{PlanSource, Tab as WorshipTab};
use crate::state::{AppState, Route};
use gpui::Context;
use idleon_clamworks::afk::Phase as ClaimPhase;
use idleon_cog::{Goal as CogGoal, WeightInputs as CogWeights};
use idleon_combat::Preset as CombatPreset;
use idleon_coral as coral;
use idleon_drop_rate as drop_rate;
use idleon_import::pipeline;
use idleon_ladle::Panel as LadlePanel;
use idleon_ladle::kitchen::Inputs as LadleKitchen;
use idleon_ladle::kitchen::Scenario as LadleScenario;
use idleon_project as project;
use idleon_research::game;
use idleon_research::model::{Effort, SimContext, SimOptions, Simulation};
use idleon_sailing::{AssignmentMode, Goal};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};
use std::collections::BTreeMap;

/// The research section's config: its tab, selection, target and supplements.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct ResearchConfig {
    tab: ResearchTab,
    selected_cell: Option<usize>,
    selected_shape: Option<usize>,
    /// `"hours"` or `"level"`, matching the target radio upstream.
    target_kind: String,
    target_value: f64,
    effort: Effort,
    /// The three upstream run toggles.
    options: SimOptions,
    supplements_open: bool,
    /// The named shape tier lists the window keeps.
    shape_presets: Vec<CustomPreset>,
    /// The shape preset the tiers were applied from; `_` ids are built-ins.
    active_shape_preset: Option<String>,
}

impl Default for ResearchConfig {
    fn default() -> Self {
        Self {
            tab: ResearchTab::Grid,
            selected_cell: None,
            selected_shape: None,
            target_kind: "hours".into(),
            target_value: 24.0,
            effort: Effort::Balanced,
            options: SimOptions::default(),
            supplements_open: true,
            shape_presets: Vec::new(),
            active_shape_preset: None,
        }
    }
}

/// The Prisma section's config: its character, cauldron, AFK claim and metric.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct PrismaConfig {
    character: usize,
    /// The cauldron tab id: `power`, `quicc`, `high-iq` or `kazam`.
    cauldron: String,
    afk_hours: f64,
    metric: Option<String>,
}

impl Default for PrismaConfig {
    fn default() -> Self {
        Self {
            character: 0,
            cauldron: idleon_prisma::Cauldron::default().id().to_string(),
            afk_hours: 1.0,
            metric: None,
        }
    }
}

/// The sailing section's config: its tab, pairings mode, island, goal and the
/// chest tier floor that lives on the route context.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct SailingConfig {
    tab: SailingTab,
    mode: AssignmentMode,
    island: usize,
    goal: Goal,
    minimum_tier: u32,
    sort_by_eta: bool,
    breakdown_boat: usize,
    sim_island: usize,
    sim_hours: f64,
    sim_trials: usize,
}

impl Default for SailingConfig {
    fn default() -> Self {
        Self {
            tab: SailingTab::default(),
            mode: AssignmentMode::default(),
            island: 0,
            goal: Goal::default(),
            minimum_tier: 0,
            sort_by_eta: false,
            breakdown_boat: 0,
            sim_island: 0,
            sim_hours: idleon_sailing::DEFAULT_HOURS,
            sim_trials: idleon_sailing::DEFAULT_TRIALS,
        }
    }
}

/// The Worship TD section's config: its trial, palette and playback.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct WorshipConfig {
    trial: usize,
    tower: usize,
    speed: f64,
    ranges: bool,
    tab: WorshipTab,
    plan_source: PlanSource,
    optimize: crate::sections::worship_td::OptimizeConfig,
}

impl Default for WorshipConfig {
    fn default() -> Self {
        Self {
            trial: 0,
            tower: 0,
            speed: 1.0,
            ranges: true,
            tab: WorshipTab::Simulator,
            plan_source: PlanSource::Guide,
            optimize: crate::sections::worship_td::OptimizeConfig::default(),
        }
    }
}

/// The Clamworks section's config: its tab, the entered multikill total, the
/// plan's budget and the AFK claim's character, hours, pearl table and
/// per-character stand-ins. A document written before the budget existed leaves
/// the current budget alone rather than zeroing it.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct ClamworksConfig {
    tab: ClamworksTab,
    multikill: f64,
    budget: Option<f64>,
    character: usize,
    hours: f64,
    phase: ClaimPhase,
    stand_ins: BTreeMap<usize, StandIn>,
}

/// The Ladle section's config: its tab, selectors, panel inputs and the checked
/// characters.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct LadleConfig {
    tab: LadleTab,
    ladle_character: usize,
    breakdown_character: usize,
    claim_hours: f64,
    panel: LadlePanel,
    assigned: Vec<usize>,
    balloon: usize,
    target_level: u32,
    split: bool,
    /// The Kitchens & Jellyfish inputs: the horizon, the donation policy, the
    /// meal cost terms and the layout pins.
    kitchen: LadleKitchen,
    /// Whether the reset plan spends the ladles the save carries.
    use_owned: bool,
    /// The forecast scenario the comparison and the reset list draw.
    #[serde(default)]
    scenario: LadleScenario,
    /// The meal outcomes table's order.
    #[serde(default)]
    outcome_sort: LadleOutcomeSort,
}

/// The cog board section's config: the objective and the custom weights. The
/// board itself comes from the save, so it is never part of a project document.
#[derive(Clone, Copy, Debug, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct CogConfig {
    goal: CogGoal,
    weighted: bool,
    weights: CogWeights,
}

/// The Jelly Operator section's config: its tab, the board on screen (which
/// starts as the save's own), the palette pick and the estimate knobs. The save
/// supplies the board and the metrics; the edited layout is a sandbox.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct JellyConfig {
    tab: JellyTab,
    /// Anchor/type pairs; a type the save has not unlocked is dropped.
    layout: Vec<(usize, usize)>,
    palette: usize,
    trials: u32,
    seed: u32,
    /// The Sections planner's controls; its plan itself is a run result.
    planner: JellyPlanner,
}

/// The Royal Guardian section's config: the pane, the world the tables list, the
/// outpost, node and currency the panels explain, the upgrade list and breakdown
/// it shows, and the forecast's horizon, collects and menu-open windows.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct RoyalConfig {
    tab: RoyalTab,
    world: usize,
    outpost: Option<usize>,
    resource: Option<usize>,
    currency: Option<i64>,
    upgrade_view: crate::sections::royal_guardian::UpgradeView,
    armory_plan: Option<usize>,
    breakdown: idleon_royal_guardian::Category,
    forecast_hours: f64,
    collects: Vec<f64>,
    windows: Vec<(f64, f64)>,
}

/// The Jar section's config: its tab, the tier the rupie value projects and
/// the enchant simulator's controls and history.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct JarConfig {
    tab: JarTab,
    tier: u32,
    enchant: JarState,
}

/// The Fountain section's config: its tab, the water the upgrade table lists,
/// the desired currency the currency tab simulates and the cost curve's pick.
/// Every number comes from the save.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct FountainConfig {
    tab: FountainTab,
    water: usize,
    desired: Option<usize>,
    upgrade: Option<usize>,
}

/// The opal section's config: the villager the breakdown is switched to, the
/// optimizer's tab, target and total, and the manual simulator's counts. The
/// villagers themselves come from the save; a document from before the
/// optimizer has no `total`, and one from before the simulator no `manual`, so
/// both fall back to the save's own.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct OpalConfig {
    selected: Option<usize>,
    tab: OpalTab,
    target: Option<usize>,
    total: Option<f64>,
    manual: Option<[f64; idleon_opal::VILLAGERS]>,
}

/// The combat simulator's config: the selected character, its tab, the named
/// loadouts and the mechanics overrides. The working loadout itself comes from
/// the save; presets and overrides are what the project keeps.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct CombatConfig {
    character: usize,
    tab: CombatTab,
    presets: Vec<CombatPreset>,
    toggles: idleon_combat::State,
}

/// The statue section's config: selected character, sort, deposit estimate, the
/// plan inputs and the statue the farming breakdown describes. The search text
/// is a filter, not state, and the statue rows come from the save.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct StatueConfig {
    character: usize,
    sort: StatueSort,
    estimate: idleon_statue::Estimate,
    targets: Vec<Option<u32>>,
    rates: Vec<String>,
    breakdown_statue: usize,
}

/// The whole window state as a project document.
pub(crate) fn export(state: &AppState) -> String {
    project::write(&project::Project {
        version: project::VERSION,
        section: state
            .current_section()
            .map(|section| section.id.to_string()),
        save: state.save.as_ref().map(|save| (**save).clone()),
        simulation: state.simulation.clone(),
        context: state.context.clone(),
        sections: configs(state),
        warnings: Vec::new(),
        origin: project::Origin::Project,
    })
}

/// Every section's config object, keyed by registry id.
pub(crate) fn configs(state: &AppState) -> Map<String, Value> {
    let mut configs = Map::new();
    let mut insert = |id: &str, config: Value| {
        configs.insert(id.to_string(), config);
    };
    insert(
        "research",
        value(&ResearchConfig {
            tab: state.research.tab,
            selected_cell: state.research.selected_cell,
            selected_shape: state.research.selected_shape,
            target_kind: match state.target {
                idleon_research::model::Target::Hours(_) => "hours".into(),
                idleon_research::model::Target::Level(_) => "level".into(),
            },
            target_value: match state.target {
                idleon_research::model::Target::Hours(hours) => hours,
                idleon_research::model::Target::Level(level) => f64::from(level),
            },
            effort: state.effort,
            options: state.options,
            supplements_open: state.supplements_open,
            shape_presets: state.research.shape_presets.clone(),
            active_shape_preset: state.research.active_shape_preset.clone(),
        }),
    );
    insert("killroy", value(&state.killroy));
    insert(
        "sailing",
        value(&SailingConfig {
            tab: state.sailing_state.tab,
            mode: state.sailing_state.mode,
            island: state.sailing_state.island,
            goal: state.sailing_state.goal,
            minimum_tier: state.sailing.minimum_tier,
            sort_by_eta: state.sailing_state.sort_by_eta,
            breakdown_boat: state.sailing_state.breakdown_boat,
            sim_island: state.sailing_state.sim_island,
            sim_hours: state.sailing_state.sim_hours,
            sim_trials: state.sailing_state.sim_trials,
        }),
    );
    insert("farming", value(&state.farming));
    insert("spelunking", value(&state.spelunking));
    insert("fishing", value(&state.fishing));
    insert(
        "statue",
        value(&StatueConfig {
            character: state.statue.character,
            sort: state.statue.sort,
            estimate: state.statue.estimate,
            targets: state.statue.plan.targets().to_vec(),
            rates: state.statue.plan.rates().to_vec(),
            breakdown_statue: state.statue.breakdown_statue(),
        }),
    );
    insert(
        "combat-sim",
        value(&CombatConfig {
            character: state.combat.character,
            tab: state.combat.tab,
            presets: state.combat.presets.items.clone(),
            toggles: state.combat.overrides.clone(),
        }),
    );
    insert("monument", value(&state.monument));
    insert("pickle", value(&state.pickle));
    insert(
        "prisma",
        value(&PrismaConfig {
            character: state.prisma.character,
            cauldron: state.prisma.cauldron.id().to_string(),
            afk_hours: state.prisma.afk_hours,
            metric: state.prisma.metric.clone(),
        }),
    );
    insert(
        "clamworks",
        value(&ClamworksConfig {
            tab: state.clamworks_tab,
            multikill: state.clamworks_multikill,
            budget: Some(state.clamworks_budget),
            character: state.clamworks_claim.character,
            hours: state.clamworks_claim.hours,
            phase: state.clamworks_claim.phase,
            stand_ins: state.clamworks_claim.stand_ins.clone(),
        }),
    );
    insert(
        "cog",
        value(&CogConfig {
            goal: state.cog.goal,
            weighted: state.cog.weighted,
            weights: state.cog.weights,
        }),
    );
    insert(
        "jelly-operator",
        value(&JellyConfig {
            tab: state.jelly_state.tab,
            layout: state
                .jelly_state
                .layout
                .iter()
                .map(|(anchor, cell_type)| (*anchor, *cell_type))
                .collect(),
            palette: state.jelly_state.selected,
            trials: state.jelly_state.trials,
            seed: state.jelly_state.seed,
            planner: state.jelly_state.planner.clone(),
        }),
    );
    insert(
        "jar",
        value(&JarConfig {
            tab: state.jar_tab,
            tier: state.jar_tier,
            enchant: state.jar_state.clone(),
        }),
    );
    insert(
        "fountain",
        value(&FountainConfig {
            tab: state.fountain_state.tab,
            water: state.fountain_state.water,
            desired: state.fountain_state.desired,
            upgrade: state.fountain_state.upgrade,
        }),
    );
    insert(
        "royal-guardian",
        value(&RoyalConfig {
            tab: state.royal_state.tab,
            world: state.royal_state.world,
            outpost: state.royal_state.outpost,
            resource: state.royal_state.resource,
            currency: state.royal_state.currency,
            upgrade_view: state.royal_state.upgrade_view,
            armory_plan: state.royal_state.armory_plan,
            breakdown: state.royal_state.breakdown,
            forecast_hours: state.royal_state.forecast_hours,
            collects: state.royal_state.collects.clone(),
            windows: state.royal_state.windows.clone(),
        }),
    );
    insert(
        "opal",
        value(&OpalConfig {
            selected: state.opal_selected,
            tab: state.opal_tab,
            target: state.opal_target,
            total: Some(state.opal_total),
            manual: Some(state.opal_manual),
        }),
    );
    insert("sneaking", value(&state.sneaking_state));
    insert("stats-all", value(&state.stats_all.context));
    insert("drop-rate", value(&state.drop_rate.selection));
    insert("damage", value(&state.damage.context));
    insert(
        "ladle",
        value(&LadleConfig {
            tab: state.ladle_state.tab,
            ladle_character: state.ladle_state.ladle_character,
            breakdown_character: state.ladle_state.breakdown_character,
            claim_hours: state.ladle_state.claim_hours,
            panel: state.ladle_state.panel,
            assigned: state.ladle_state.assigned.clone(),
            balloon: state.ladle_state.balloon,
            target_level: state.ladle_state.target_level,
            split: state.ladle_state.split,
            kitchen: state.ladle_state.kitchen.clone(),
            use_owned: state.ladle_state.use_owned,
            scenario: state.ladle_state.scenario,
            outcome_sort: state.ladle_state.outcome_sort,
        }),
    );
    insert("coral-god-rank", value(&state.coral.inputs));
    insert("cooking-mastery", value(&state.mastery_plan));
    insert(
        "worship-td",
        value(&WorshipConfig {
            trial: state.worship.trial,
            tower: state.worship.tower,
            speed: state.worship.speed,
            ranges: state.worship.ranges,
            tab: state.worship.tab,
            plan_source: state.worship.plan_source,
            optimize: state.worship.optimize,
        }),
    );
    configs
}

/// Apply a project document to the window, collecting what it could not read
/// and every change it made.
///
/// The document arrives from [`pipeline::read`] with its embedded save already
/// decoded, and its section configs from [`prepare`], so this only assigns: the
/// save and the research state land together, then each decoded config is
/// assigned. Nothing here can fail, so the window never changes halfway.
pub(crate) fn apply(
    state: &mut AppState,
    import: pipeline::ProjectImport,
    configs: Vec<Config>,
    warnings: &mut Vec<String>,
    changes: &mut Vec<String>,
    cx: &mut Context<AppState>,
) {
    let pipeline::ProjectImport { document, save } = import;
    let project::Project {
        section,
        simulation,
        context,
        ..
    } = document;
    match save {
        Some(mut imported) => {
            warnings.extend(imported.warnings.iter().cloned());
            changes.push(format!(
                "save: {} · {} character(s)",
                imported.format.label(),
                imported.characters
            ));
            imported.simulation = simulation;
            imported.context = context;
            changes.extend(research_changes(&state.simulation, &imported.simulation));
            changes.extend(supplement_changes(&state.context, &imported.context));
            state.load_imported(imported, cx);
        }
        None => {
            changes.extend(research_changes(&state.simulation, &simulation));
            changes.extend(supplement_changes(&state.context, &context));
            state.simulation = simulation;
            state.research.history.clear();
            state
                .research
                .minehead
                .load(state.save.as_ref(), &state.simulation);
            state
                .research
                .sushi
                .load(state.save.as_ref(), &state.simulation);
            state.replace_context(context);
        }
    }
    for config in configs {
        assign(state, config, warnings, cx);
    }
    // A section id the registry does not declare was already named in the
    // preview's warnings, so the route simply stays where it is.
    if let Some(section) = section.as_deref().and_then(sections::find) {
        if state.route != Route::Section(section) {
            changes.push(format!(
                "section: {} → {}",
                route_name(&state.route),
                section.id
            ));
        }
        state.route = Route::Section(section);
        state.last_section = Some(section.id);
    }
}

/// One section's config, decoded while the dialog previews the document.
#[derive(Clone, Debug)]
pub(crate) enum Config {
    Research(ResearchConfig),
    Killroy(KillroyState),
    Sailing(SailingConfig),
    Farming(FarmingState),
    Fishing(FishingState),
    Statue(StatueConfig),
    Combat(CombatConfig),
    Monument(MonumentState),
    Pickle(PickleState),
    Prisma(PrismaConfig),
    Clamworks(ClamworksConfig),
    Cog(CogConfig),
    Jelly(JellyConfig),
    Jar(JarConfig),
    Fountain(FountainConfig),
    Royal(RoyalConfig),
    Opal(OpalConfig),
    /// The login context and the selected slot; the loadout itself comes from
    /// the save, so it is never part of a project.
    Sneaking(SneakingState),
    Ladle(LadleConfig),
    Spelunking(SpelunkingState),
    Coral(coral::Inputs),
    CookingMastery(CookingMasteryState),
    Worship(WorshipConfig),
    StatsAll(idleon_stats::Context),
    DropRate(idleon_drop_rate::Selection),
    Damage(idleon_damage::Context),
}

/// What decoding one section config found.
enum Decoded {
    Config(Box<Config>),
    /// The registry declares no section with this id.
    Unknown,
    /// The id is declared, but the config does not fit this build's shape.
    Malformed(String),
}

/// Decode every section config a document carries, without touching the window.
///
/// The preview runs this, so applying the document only assigns what was
/// already read: a malformed config fails the import instead of leaving the
/// window half-changed, and each unknown-field warning is counted once.
pub(crate) fn prepare(document: &mut pipeline::Document) -> Result<Vec<Config>, String> {
    let configs: Vec<(String, Value)> = match document {
        pipeline::Document::Project(project) => project
            .document
            .sections
            .iter()
            .map(|(id, config)| (id.clone(), config.clone()))
            .collect(),
        pipeline::Document::Section(section) => {
            vec![(section.section.clone(), section.config.clone())]
        }
        pipeline::Document::Save(_) => return Ok(Vec::new()),
    };
    let warnings = document.warnings_mut();
    let mut decoded = Vec::new();
    for (id, config) in &configs {
        match decode(id, config, warnings) {
            Decoded::Config(config) => decoded.push(*config),
            Decoded::Unknown => {}
            Decoded::Malformed(detail) => return Err(detail),
        }
    }
    Ok(decoded)
}

/// The tiers a built-in id or a saved preset name names.
fn shape_preset_tiers(state: &AppState, id: &str) -> Option<idleon_research::Tiers> {
    if id.starts_with('_') {
        game()
            .presets
            .iter()
            .find(|preset| preset.id == id)
            .and_then(|preset| crate::sections::research::preset_tiers(state, preset))
    } else {
        CustomPreset::find(&state.research.shape_presets, id).map(|preset| preset.tiers.clone())
    }
}

/// Assign one decoded section config. Warnings are for values the loaded save
/// or the grid no longer has room for; they never fail the import.
pub(crate) fn assign(
    state: &mut AppState,
    config: Config,
    warnings: &mut Vec<String>,
    cx: &mut Context<AppState>,
) {
    match config {
        Config::Research(config) => {
            state.research.tab = config.tab;
            state.effort = config.effort;
            state.options = config.options;
            state.supplements_open = config.supplements_open;
            state.research.selected_cell = within(
                config.selected_cell,
                game().cells(),
                "selectedCell",
                warnings,
            );
            state.research.selected_shape = within(
                config.selected_shape,
                state.simulation.placements.len(),
                "selectedShape",
                warnings,
            );
            match config.target_kind.as_str() {
                "level" => {
                    state.target = idleon_research::model::Target::Level(
                        config.target_value.max(1.0).round() as u32,
                    )
                }
                "hours" => {
                    state.target =
                        idleon_research::model::Target::Hours(config.target_value.max(0.5))
                }
                other => warnings.push(format!(
                    "research: targetKind {other:?} is not \"hours\" or \"level\"; the target is kept"
                )),
            }
            state.normalize_target();
            let wanted = config.active_shape_preset;
            state.research.shape_presets = CustomPreset::sanitise(config.shape_presets);
            state.research.active_shape_preset = wanted
                .as_deref()
                .filter(|id| shape_preset_tiers(state, id).is_some())
                .map(str::to_string);
            if let Some(tiers) = state
                .research
                .active_shape_preset
                .as_deref()
                .and_then(|id| shape_preset_tiers(state, id))
            {
                state.simulation.shape_tiers = tiers;
            } else if let Some(id) = wanted {
                warnings.push(format!(
                    "research: shape preset {id:?} is not in this build; the tiers are kept"
                ));
            }
        }
        Config::Killroy(config) => {
            state.killroy = config.normalized();
        }
        Config::Sailing(config) => {
            state.sailing_state = SailingState {
                tab: config.tab,
                mode: config.mode,
                island: config
                    .island
                    .min(state.fleet.available_islands().saturating_sub(1)),
                goal: config.goal,
                sort_by_eta: config.sort_by_eta,
                breakdown_boat: config
                    .breakdown_boat
                    .min(state.fleet.boats.len().saturating_sub(1)),
                sim_island: config
                    .sim_island
                    .min(state.fleet.available_islands().saturating_sub(1)),
                sim_hours: if config.sim_hours.is_finite() {
                    config
                        .sim_hours
                        .clamp(idleon_sailing::MIN_HOURS, idleon_sailing::MAX_HOURS)
                } else {
                    idleon_sailing::DEFAULT_HOURS
                },
                sim_trials: config
                    .sim_trials
                    .clamp(idleon_sailing::MIN_TRIALS, idleon_sailing::MAX_TRIALS),
            };
            state.sailing.minimum_tier = config.minimum_tier.min(5);
        }
        Config::Farming(config) => {
            state.farming = FarmingState {
                tab: config.tab,
                selected_plot: config.selected_plot.min(idleon_farming::PLOTS - 1),
                target_crop: config.target_crop,
                objective: config.objective,
                mode: config.mode,
                presets: crate::sections::farming::CustomPreset::sanitise(config.presets),
                ..FarmingState::default()
            };
            // The assignment follows the farm this document already carries.
            state.farming.adopt(&state.farm);
        }
        Config::Combat(config) => {
            state.combat.tab = config.tab;
            state.combat.overrides = config.toggles;
            state.combat.presets.items = config
                .presets
                .into_iter()
                .filter(|preset| !preset.name.trim().is_empty())
                .take(crate::sections::presets::MAX)
                .collect();
            let character = if let Some(save) = state.save.as_ref() {
                if config.character < save.character_count() {
                    config.character
                } else {
                    warnings.push(format!(
                        "combat-sim: character {} is not in the save; the editor keeps character {}",
                        config.character + 1,
                        state.combat.character + 1
                    ));
                    state.combat.character
                }
            } else {
                config.character
            };
            state.combat.character = character;
            if let Some(save) = state.save.as_ref() {
                state.combat.loadout = idleon_combat::Loadout::from_save(save, character);
            }
        }
        Config::Fishing(config) => {
            state.fishing = config;
            state.fishing.clamp(&state.fishing_save);
        }
        Config::Statue(config) => {
            state.statue.sort = config.sort;
            state.statue.estimate = config.estimate;
            if state.statue.save.has_character(config.character) {
                state.statue.character = config.character;
            } else {
                warnings.push(format!(
                    "statue: character {} has no saved rows; the table keeps its character",
                    config.character + 1
                ));
            }
            state.statue.set_plan(config.targets, config.rates, cx);
            if config.breakdown_statue < idleon_statue::count() {
                state.statue.breakdown_statue = config.breakdown_statue;
            }
        }
        Config::Monument(config) => {
            let characters = state
                .save
                .as_ref()
                .map_or(0, idleon_save::Save::character_count);
            state.monument = MonumentState {
                tab: config.tab,
                character: if characters == 0 {
                    0
                } else {
                    config.character.min(characters - 1)
                },
                wave: config.wave.clamp(1, 1001),
                boosts: config.boosts,
                rerolls: config.rerolls,
                remaining_hp: config.remaining_hp.filter(|hp| hp.is_finite() && *hp > 0.0),
                worksheet: config.worksheet.restored(),
                notice: String::new(),
            };
        }
        Config::Pickle(config) => {
            state.pickle = config.clamped();
        }
        Config::Prisma(config) => {
            state.prisma.character = config.character;
            state.prisma.cauldron = idleon_prisma::Cauldron::from_id(&config.cauldron)
                .unwrap_or_else(|| {
                    warnings.push(format!(
                        "prisma: cauldron {:?} is not a tab id; the section keeps {}",
                        config.cauldron,
                        state.prisma.cauldron.label()
                    ));
                    state.prisma.cauldron
                });
            state.prisma.afk_hours = if config.afk_hours.is_finite() {
                config.afk_hours.clamp(
                    crate::sections::prisma::MIN_HOURS,
                    crate::sections::prisma::MAX_HOURS,
                )
            } else {
                state.prisma.afk_hours
            };
            state.prisma.metric = config.metric.filter(|metric| !metric.trim().is_empty());
            state.prisma.adopt(state.save.as_ref());
        }
        Config::Clamworks(config) => {
            state.clamworks_tab = config.tab;
            state.clamworks_multikill = if config.multikill.is_finite() {
                config.multikill.max(0.0)
            } else {
                0.0
            };
            if let Some(budget) = config.budget {
                state.clamworks_budget = if budget.is_finite() {
                    budget.max(0.0)
                } else {
                    0.0
                };
            }
            let count = state.save.as_ref().map_or(0, |save| save.character_count());
            state.clamworks_claim.adopt(
                config.character,
                config.stand_ins,
                config.hours,
                config.phase,
                count,
                cx,
            );
        }
        Config::Cog(config) => {
            state.cog.goal = config.goal;
            state.cog.weights = config.weights;
            match config.weights.weights() {
                Ok(_) => state.cog.weighted = config.weighted,
                Err(error) => {
                    state.cog.weighted = false;
                    warnings.push(format!("cog: weights ignored ({error})"));
                }
            }
        }
        Config::Jelly(config) => {
            state.jelly_state.tab = config.tab;
            state.jelly_state.selected = config.palette.min(7);
            state.jelly_state.trials = config.trials.clamp(1, 1024);
            state.jelly_state.seed = config.seed;
            let Some(jelly) = state.jelly.clone() else {
                warnings.push("jelly-operator: no imported save; the board config is kept for the next import".into());
                return;
            };
            let mut layout = idleon_jelly::board::Layout::new();
            for (anchor, cell_type) in config.layout {
                if cell_type < jelly.units_owned()
                    && idleon_jelly::board::footprint(anchor, cell_type).is_some()
                {
                    layout.insert(anchor, cell_type);
                }
            }
            if idleon_jelly::board::validate(&jelly, &layout).is_ok() {
                state.jelly_state.layout = layout;
            } else {
                warnings.push(
                    "jelly-operator: the saved board no longer fits this save; the save's layout is shown".into(),
                );
                state.jelly_state.layout = jelly.layout.clone();
            }
            state.jelly_state.planner = config.planner;
            state.jelly_state.planner.adopt(&jelly);
        }
        Config::Jar(config) => {
            state.jar_tab = config.tab;
            state.jar_tier = config.tier.min(idleon_jar::INCOME_TIERS - 1);
            let levels = state.jar.as_ref().map(idleon_jar::Jar::collectible_levels);
            state.jar_state = config.enchant.normalized(levels);
        }
        Config::Spelunking(config) => {
            state.spelunking = config;
            if let Some(inputs) = state.spelunk_inputs() {
                state.spelunking.clamp(&inputs);
            }
        }
        Config::Fountain(config) => {
            state.fountain_state.tab = config.tab;
            state.fountain_state.water = config.water;
            state.fountain_state.desired = config.desired;
            state.fountain_state.upgrade = config.upgrade;
            state.fountain_state.adopt(state.fountain.as_ref());
        }
        Config::Royal(config) => {
            state.royal_state = crate::sections::royal_guardian::State {
                tab: config.tab,
                world: config.world.min(idleon_royal_guardian::WORLDS - 1),
                outpost: config.outpost,
                resource: config.resource,
                currency: config.currency,
                upgrade_view: config.upgrade_view,
                armory_plan: config.armory_plan,
                breakdown: config.breakdown,
                forecast_hours: config.forecast_hours,
                collects: config.collects,
                windows: config.windows,
            };
            state.royal_forecast = None;
            state.royal_forecast_key = None;
        }
        Config::Opal(config) => {
            state.set_opal_villager(config.selected, cx);
            state.set_opal_tab(config.tab, cx);
            state.set_opal_target(config.target, cx);
            match config.total {
                Some(total) => state.set_opal_total(total, cx),
                None => state.use_saved_opal_total(cx),
            }
            match config.manual {
                Some(manual) => {
                    state.opal_manual = manual.map(idleon_opal::whole);
                    cx.notify();
                }
                None => state.reset_opal_manual(cx),
            }
        }
        Config::Sneaking(config) => {
            let players = state
                .sneaking
                .as_ref()
                .map_or(0, idleon_sneaking::Loadout::players);
            state.sneaking_state = SneakingState {
                login: config.login.filter(|index| *index < players.max(1)),
                twin: config.twin.filter(|index| *index < players.max(1)),
                slot: config.slot.filter(|slot| *slot < 4),
                workspace: config.workspace,
                formula_tab: config.formula_tab,
                breakdown_twin: config
                    .breakdown_twin
                    .filter(|index| *index < players.max(1)),
                objective: config.objective,
            };
        }
        Config::Ladle(config) => {
            let characters = state
                .ladle
                .as_ref()
                .map(|ladle| ladle.characters.len())
                .unwrap_or(0);
            state.ladle_state.tab = config.tab;
            state.ladle_state.ladle_character =
                config.ladle_character.min(characters.saturating_sub(1));
            state.ladle_state.breakdown_character =
                config.breakdown_character.min(characters.saturating_sub(1));
            state.ladle_state.claim_hours = if config.claim_hours.is_finite() {
                config.claim_hours.clamp(0.1, 24.0)
            } else {
                24.0
            };
            state.ladle_state.panel = config.panel;
            state.ladle_state.assigned = config
                .assigned
                .into_iter()
                .filter(|index| *index < characters)
                .collect();
            state.ladle_state.balloon = config.balloon.min(idleon_ladle::EXP_BALLOONS.len() - 1);
            state.ladle_state.target_level = config.target_level.min(1000);
            state.ladle_state.split = config.split;
            state.ladle_state.kitchen = config.kitchen;
            state.ladle_state.use_owned = config.use_owned;
            state.ladle_state.scenario = config.scenario;
            state.ladle_state.outcome_sort = config.outcome_sort;
        }
        Config::Coral(config) => {
            state.coral.inputs = config;
        }
        Config::CookingMastery(config) => {
            state.mastery_plan = config;
        }
        Config::StatsAll(context) => stats_all::restore(state, context, warnings),
        Config::DropRate(selection) => {
            state.drop_rate.selection = selection;
            if let Some(snapshot) = state.drop_rate.snapshot.as_ref() {
                let characters = snapshot.characters();
                let mut selection = state.drop_rate.selection;
                if selection.character >= characters {
                    warnings.push(format!(
                        "drop-rate: character {} is outside the {characters} the save carries; character 1 is used",
                        selection.character + 1
                    ));
                    selection.character = 0;
                }
                if !drop_rate::maps(Some(snapshot), selection.character)
                    .iter()
                    .any(|row| row.map == selection.map)
                {
                    warnings.push(format!(
                        "drop-rate: map {} is not in the selector; the character's current map is used",
                        selection.map
                    ));
                    selection.map = snapshot.current_map(selection.character).unwrap_or(0);
                }
                state.drop_rate.selection = selection;
            }
        }
        Config::Damage(context) => crate::sections::damage::restore(state, context, warnings),
        Config::Worship(config) => {
            let last_trial = idleon_worship::data().trials.len().saturating_sub(1);
            state.worship.trial = config.trial.min(last_trial);
            state.worship.tower = config.tower.min(idleon_worship::TOWER_COUNT - 1);
            state.worship.speed = config.speed.clamp(0.5, 5.0);
            state.worship.ranges = config.ranges;
            state.worship.tab = config.tab;
            state.worship.plan_source = config.plan_source;
            state.worship.optimize = crate::sections::worship_td::OptimizeConfig {
                mode: config.optimize.mode,
                seeds: config.optimize.seeds.clamp(1, 6),
                base_seed: config.optimize.base_seed.min(999_999),
                max_wave: config.optimize.max_wave.clamp(10, 1000),
            };
            state.worship.plan = None;
            state.worship.plan_from_optimizer = false;
            state.worship.run = None;
            state.worship.view = None;
            state.worship.selected = None;
            state.worship.playing = false;
            state.worship.pumping = false;
            state.worship.generation += 1;
        }
    }
}

/// Decode one section's config by registry id, naming the fields this build
/// does not know in `warnings`.
fn decode(id: &str, config: &Value, warnings: &mut Vec<String>) -> Decoded {
    macro_rules! decode {
        ($kind:ident, $config:ty) => {
            match read::<$config>(config, id, warnings) {
                Ok(config) => Decoded::Config(Box::new(Config::$kind(config))),
                Err(detail) => Decoded::Malformed(detail),
            }
        };
    }
    match id {
        "research" => decode!(Research, ResearchConfig),
        "killroy" => decode!(Killroy, KillroyState),
        "sailing" => decode!(Sailing, SailingConfig),
        "farming" => decode!(Farming, FarmingState),
        "fishing" => decode!(Fishing, FishingState),
        "statue" => decode!(Statue, StatueConfig),
        "combat-sim" => decode!(Combat, CombatConfig),
        "monument" => decode!(Monument, MonumentState),
        "pickle" => decode!(Pickle, PickleState),
        "prisma" => decode!(Prisma, PrismaConfig),
        "clamworks" => decode!(Clamworks, ClamworksConfig),
        "cog" => decode!(Cog, CogConfig),
        "jelly-operator" => decode!(Jelly, JellyConfig),
        "jar" => decode!(Jar, JarConfig),
        "fountain" => decode!(Fountain, FountainConfig),
        "royal-guardian" => decode!(Royal, RoyalConfig),
        "opal" => decode!(Opal, OpalConfig),
        "sneaking" => decode!(Sneaking, SneakingState),
        "ladle" => decode!(Ladle, LadleConfig),
        "spelunking" => decode!(Spelunking, SpelunkingState),
        "coral-god-rank" => decode!(Coral, coral::Inputs),
        "cooking-mastery" => decode!(CookingMastery, CookingMasteryState),
        "stats-all" => decode!(StatsAll, idleon_stats::Context),
        "drop-rate" => decode!(DropRate, idleon_drop_rate::Selection),
        "damage" => decode!(Damage, idleon_damage::Context),
        "worship-td" => decode!(Worship, WorshipConfig),
        _ => Decoded::Unknown,
    }
}

/// A config struct as the document stores it.
fn value<T: Serialize>(config: &T) -> Value {
    serde_json::to_value(config).unwrap_or(Value::Null)
}

/// The research fields that changed, named as the document names them.
pub(crate) fn research_changes(before: &Simulation, after: &Simulation) -> Vec<String> {
    let mut changes = Vec::new();
    if before.research_level != after.research_level {
        changes.push(format!(
            "researchLevel: {} → {}",
            before.research_level, after.research_level
        ));
    }
    if before.research_exp != after.research_exp {
        changes.push(format!(
            "researchExp: {} → {}",
            plain_number(before.research_exp),
            plain_number(after.research_exp)
        ));
    }
    let squares = (0..game().cells())
        .filter(|cell| before.grid.level(*cell) != after.grid.level(*cell))
        .count();
    if squares > 0 {
        changes.push(format!("gridLevels: {squares} square(s) changed"));
    }
    if before.mags != after.mags {
        changes.push(format!(
            "magData: {} → {} magnifier(s)",
            before.mags.len(),
            after.mags.len()
        ));
    }
    if before.placements != after.placements {
        changes.push(format!(
            "shapePositions: {} → {} shape(s)",
            before.placements.len(),
            after.placements.len()
        ));
    }
    if before.observations != after.observations {
        changes.push("insightLvs/occFound: observation progress changed".into());
    }
    changes
}

/// The save-derived multiplier fields that changed, by their document name.
pub(crate) fn supplement_changes(before: &SimContext, after: &SimContext) -> Vec<String> {
    let (Ok(before), Ok(after)) = (serde_json::to_value(before), serde_json::to_value(after))
    else {
        return Vec::new();
    };
    let (Some(before), Some(after)) = (before.as_object(), after.as_object()) else {
        return Vec::new();
    };
    let changed: Vec<&str> = after
        .keys()
        .map(String::as_str)
        .filter(|field| before.get(*field) != after.get(*field))
        .collect();
    if changed.is_empty() {
        Vec::new()
    } else {
        vec![format!("supplements: {}", changed.join(", "))]
    }
}

/// The section configs that differ between two snapshots of the window.
pub(crate) fn config_changes(
    before: &Map<String, Value>,
    after: &Map<String, Value>,
) -> Vec<String> {
    after
        .iter()
        .filter(|(id, config)| before.get(*id) != Some(*config))
        .map(|(id, _)| format!("{id}: config changed"))
        .collect()
}

/// A change log as the one line the status bar shows.
pub(crate) fn change_line(changes: &[String]) -> String {
    match changes.first() {
        None => "Nothing changed.".into(),
        Some(first) if changes.len() == 1 => format!("Changed: {first}."),
        Some(first) => format!(
            "{} changes: {first} (and {} more).",
            changes.len(),
            changes.len() - 1
        ),
    }
}

/// The route as the change log names it.
fn route_name(route: &Route) -> &str {
    match route {
        Route::Dashboard => "dashboard",
        Route::Section(section) => section.id,
        Route::Missing(id) => id,
    }
}

/// A number without its trailing `.0`, for a change-log line.
fn plain_number(value: f64) -> String {
    if !value.is_finite() {
        return value.to_string();
    }
    if value.fract() == 0.0 {
        format!("{value:.0}")
    } else {
        format!("{value:.2}")
    }
}

/// Read one section's config, naming the fields this build does not know.
///
/// A config the shape does not fit is an error naming the section: the caller
/// refuses the whole document rather than applying the rest of it.
fn read<T: Serialize + DeserializeOwned + Default>(
    config: &Value,
    section: &str,
    warnings: &mut Vec<String>,
) -> Result<T, String> {
    let known = serde_json::to_value(T::default()).ok();
    if let (Some(object), Some(known)) = (
        config.as_object(),
        known.as_ref().and_then(Value::as_object),
    ) {
        let unknown: Vec<&str> = object
            .keys()
            .map(String::as_str)
            .filter(|key| !known.contains_key(*key))
            .collect();
        if !unknown.is_empty() {
            warnings.push(format!(
                "{section}: unknown field(s) ignored ({})",
                unknown.join(", ")
            ));
        }
    }
    serde_json::from_value(config.clone()).map_err(|error| format!("{section}: {error}"))
}

/// An optional index, dropped with a warning when it is outside `limit`.
fn within(
    index: Option<usize>,
    limit: usize,
    field: &str,
    warnings: &mut Vec<String>,
) -> Option<usize> {
    match index {
        Some(index) if index < limit => Some(index),
        Some(index) => {
            warnings.push(format!(
                "{field}: {index} is outside the {limit} available; cleared"
            ));
            None
        }
        None => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use idleon_coral::Goal as CoralGoal;
    use serde_json::json;

    fn round_trip<T>(config: &T) -> T
    where
        T: Serialize + DeserializeOwned + Default,
    {
        let mut warnings = Vec::new();
        let back = read(&value(config), "test", &mut warnings).expect("the config reads back");
        assert!(warnings.is_empty(), "{warnings:?}");
        back
    }

    #[test]
    fn every_section_state_round_trips_through_its_config() {
        let killroy = round_trip(&KillroyState {
            weeks: 20,
            trials: 5,
            mode: crate::sections::killroy::Mode::Upgrades,
            upgrades: vec![0, 3],
            ..KillroyState::default()
        });
        assert_eq!(killroy.weeks, 20);
        assert_eq!(killroy.trials, 5);
        assert_eq!(killroy.mode, crate::sections::killroy::Mode::Upgrades);
        assert_eq!(killroy.upgrades, vec![0, 3]);
        let sailing = round_trip(&SailingState {
            tab: crate::sections::sailing::Tab::Ships,
            mode: AssignmentMode::Saved,
            island: 3,
            goal: Goal::Chests,
            sort_by_eta: true,
            breakdown_boat: 2,
            sim_island: 4,
            sim_hours: 12.0,
            sim_trials: 800,
        });
        assert_eq!(sailing.tab, crate::sections::sailing::Tab::Ships);
        assert_eq!(sailing.mode, AssignmentMode::Saved);
        assert_eq!(sailing.island, 3);
        assert_eq!(sailing.goal, Goal::Chests);
        assert!(sailing.sort_by_eta);
        assert_eq!(sailing.breakdown_boat, 2);
        assert_eq!(sailing.sim_island, 4);
        assert_eq!(sailing.sim_hours, 12.0);
        assert_eq!(sailing.sim_trials, 800);
        let farming = round_trip(&FarmingState {
            tab: crate::sections::farming::Tab::Plots,
            selected_plot: 5,
            target_crop: 12,
            objective: idleon_farming::Objective::Beans,
            mode: idleon_farming::Mode::Reset,
            presets: vec![crate::sections::farming::CustomPreset {
                name: "Beans run".into(),
                objective: idleon_farming::Objective::Beans,
                mode: idleon_farming::Mode::Reset,
                target_crop: 230,
            }],
            ..FarmingState::default()
        });
        assert_eq!(farming.tab, crate::sections::farming::Tab::Plots);
        assert_eq!(farming.selected_plot, 5);
        assert_eq!(farming.target_crop, 12);
        assert_eq!(farming.objective, idleon_farming::Objective::Beans);
        assert_eq!(farming.mode, idleon_farming::Mode::Reset);
        assert_eq!(farming.presets.len(), 1);
        assert_eq!(farming.presets[0].name, "Beans run");
        assert_eq!(farming.presets[0].target_crop, 230);
        // The Justice worksheet travels with the document, but its undo trail is
        // a session convenience and starts empty.
        let mut justice = idleon_monument::justice::Worksheet::default();
        justice.select(3);
        let mut justice_state = justice.state.clone();
        justice_state.health = 7;
        justice_state
            .effects
            .insert(idleon_monument::justice::Effect::CryptoInvest, 4);
        justice.edit(justice_state);
        assert!(justice.can_undo());
        let monument = round_trip(&MonumentState {
            tab: crate::sections::monument::Tab::Sources,
            character: 4,
            wave: 42,
            boosts: 3,
            rerolls: Some(7),
            remaining_hp: Some(1234.5),
            worksheet: justice,
            notice: String::new(),
        });
        assert_eq!(monument.tab, crate::sections::monument::Tab::Sources);
        assert_eq!(monument.character, 4);
        assert_eq!(monument.wave, 42);
        assert_eq!(monument.boosts, 3);
        assert_eq!(monument.rerolls, Some(7));
        assert_eq!(monument.remaining_hp, Some(1234.5));
        assert_eq!(monument.worksheet.case_id, 3);
        assert_eq!(monument.worksheet.state.health, 7);
        assert_eq!(
            monument
                .worksheet
                .state
                .effects
                .get(&idleon_monument::justice::Effect::CryptoInvest),
            Some(&4)
        );
        assert!(
            !monument.worksheet.can_undo(),
            "a project keeps the worksheet, not its trail"
        );
        let royal = round_trip(&RoyalConfig {
            tab: RoyalTab::Forecast,
            world: 2,
            outpost: Some(51),
            resource: Some(44),
            currency: Some(3),
            upgrade_view: crate::sections::royal_guardian::UpgradeView::Orblets,
            armory_plan: Some(4),
            breakdown: idleon_royal_guardian::Category::Savage,
            forecast_hours: 168.0,
            collects: vec![6.0, 30.0],
            windows: vec![(20.0, 22.0)],
        });
        assert_eq!(royal.tab, RoyalTab::Forecast);
        assert_eq!(royal.world, 2);
        assert_eq!(royal.outpost, Some(51));
        assert_eq!(royal.resource, Some(44));
        assert_eq!(royal.currency, Some(3));
        assert_eq!(
            royal.upgrade_view,
            crate::sections::royal_guardian::UpgradeView::Orblets
        );
        assert_eq!(royal.armory_plan, Some(4));
        assert_eq!(royal.breakdown, idleon_royal_guardian::Category::Savage);
        assert_eq!(royal.forecast_hours, 168.0);
        assert_eq!(royal.collects, vec![6.0, 30.0]);
        assert_eq!(royal.windows, vec![(20.0, 22.0)]);
        let pickle = round_trip(&PickleState {
            dps: 5.5e7,
            max_ttk: 120.0,
        });
        assert_eq!(pickle.dps, 5.5e7);
        assert_eq!(pickle.max_ttk, 120.0);
        let research = round_trip(&ResearchConfig {
            tab: ResearchTab::Run,
            selected_cell: Some(129),
            selected_shape: Some(2),
            target_kind: "level".into(),
            target_value: 140.0,
            effort: Effort::Thorough,
            options: SimOptions {
                assume_observations: true,
                extended_insight: false,
                include_tournament: true,
            },
            supplements_open: false,
            shape_presets: vec![CustomPreset {
                name: "Minehead run".into(),
                tiers: idleon_research::Tiers::parse("J6|").expect("the tiers"),
            }],
            active_shape_preset: Some("Minehead run".into()),
        });
        assert_eq!(research.tab, ResearchTab::Run);
        assert_eq!(research.selected_cell, Some(129));
        assert_eq!(research.selected_shape, Some(2));
        assert_eq!(research.target_kind, "level");
        assert_eq!(research.target_value, 140.0);
        assert_eq!(research.effort, Effort::Thorough);
        assert!(research.options.assume_observations);
        assert!(!research.options.extended_insight);
        assert!(research.options.include_tournament);
        assert!(!research.supplements_open);
        assert_eq!(research.shape_presets.len(), 1);
        assert_eq!(research.shape_presets[0].name, "Minehead run");
        assert_eq!(
            research.active_shape_preset.as_deref(),
            Some("Minehead run")
        );
        let clamworks = round_trip(&ClamworksConfig {
            tab: ClamworksTab::Claim,
            multikill: 7.0,
            budget: Some(2_500_000.0),
            character: 2,
            hours: 8.5,
            phase: ClaimPhase::Settled,
            stand_ins: BTreeMap::from([(
                2,
                StandIn {
                    kills_per_hour: 2_000.0,
                    drop_rate: 1.5,
                },
            )]),
        });
        assert_eq!(clamworks.tab, ClamworksTab::Claim);
        assert_eq!(clamworks.multikill, 7.0);
        assert_eq!(clamworks.budget, Some(2_500_000.0));
        assert_eq!(clamworks.character, 2);
        assert_eq!(clamworks.hours, 8.5);
        assert_eq!(clamworks.phase, ClaimPhase::Settled);
        assert_eq!(
            clamworks.stand_ins.get(&2),
            Some(&StandIn {
                kills_per_hour: 2_000.0,
                drop_rate: 1.5,
            })
        );
        let legacy: ClamworksConfig = serde_json::from_value(serde_json::json!({
            "tab": "promotion",
            "multikill": 7.0,
        }))
        .expect("a config without a budget still reads");
        assert_eq!(legacy.tab, ClamworksTab::Promotion);
        assert_eq!(legacy.budget, None);
        let cog = round_trip(&CogConfig {
            goal: CogGoal::Flaggy,
            weighted: true,
            weights: CogWeights {
                build: 50.0,
                conexp: 30.0,
                flaggy: 20.0,
            },
        });
        assert_eq!(cog.goal, CogGoal::Flaggy);
        assert!(cog.weighted);
        assert_eq!(cog.weights.build, 50.0);
        let legacy_cog: CogConfig = serde_json::from_value(serde_json::json!({
            "goal": "build",
        }))
        .expect("a config without weights still reads");
        assert!(!legacy_cog.weighted);
        assert_eq!(legacy_cog.weights.build, 33.0);
        let jar = round_trip(&JarConfig {
            tab: JarTab::Production,
            tier: 3,
            enchant: Default::default(),
        });
        assert_eq!(jar.tab, JarTab::Production);
        assert_eq!(jar.tier, 3);
        // The rupie tab keeps its upstream `rupie` name, not the enum's.
        let rupies = round_trip(&JarConfig {
            tab: JarTab::RupieValue,
            tier: 2,
            enchant: Default::default(),
        });
        assert_eq!(rupies.tab, JarTab::RupieValue);
        assert_eq!(rupies.tier, 2);
        // JR-04: the simulator's controls and the saved spreads live in the
        // same config, so an export carries the history.
        let enchant = JarState {
            enchants: 120,
            sims: 500,
            strategy: idleon_jar::Strategy::Smart,
            focus: Some(5),
            targets: [(5, 3)].into_iter().collect(),
            seed: 42,
            history: vec![idleon_jar::EnchantSnapshot {
                at: 1_700_000_000,
                total: 4,
                spread: vec![0, 0, 0, 0, 0, 4],
            }],
        };
        let sim = round_trip(&JarConfig {
            tab: JarTab::EnchantSim,
            tier: 0,
            enchant: enchant.clone(),
        });
        assert_eq!(sim.tab, JarTab::EnchantSim);
        assert_eq!(sim.enchant, enchant);
        let fountain = round_trip(&FountainConfig {
            tab: FountainTab::Upgrades,
            water: 2,
            desired: Some(5),
            upgrade: Some(11),
        });
        assert_eq!(fountain.tab, FountainTab::Upgrades);
        assert_eq!(fountain.water, 2);
        assert_eq!(fountain.desired, Some(5));
        assert_eq!(fountain.upgrade, Some(11));
        // The JR-03 and JR-04 tabs keep the upstream ids in a saved project.
        for tab in [
            JarTab::Collectibles,
            JarTab::Upgrades,
            JarTab::NextTier,
            JarTab::EnchantSim,
            JarTab::EnchantHistory,
        ] {
            let config = JarConfig {
                tab,
                tier: 0,
                enchant: Default::default(),
            };
            let saved = serde_json::to_value(&config).expect("the jar config saves");
            assert_eq!(saved["tab"], tab.id());
            let restored: JarConfig =
                serde_json::from_value(saved).expect("the jar config reads back");
            assert_eq!(restored.tab, tab);
        }
        let prism = round_trip(&PrismaConfig {
            character: 2,
            cauldron: "kazam".into(),
            afk_hours: 8.5,
            metric: Some("bubble-bonus:0:0".into()),
        });
        assert_eq!(prism.character, 2);
        assert_eq!(prism.cauldron, "kazam");
        assert_eq!(prism.afk_hours, 8.5);
        assert_eq!(prism.metric.as_deref(), Some("bubble-bonus:0:0"));
        let stats = round_trip(&idleon_stats::Context {
            character: 2,
            map: 120,
            map_visibility: idleon_stats::MapVisibility::Advanced,
            skill: idleon_stats::Skill::Divinity,
            construction: idleon_stats::ConstructionMode::Inactive,
        });
        assert_eq!(stats.character, 2);
        assert_eq!(stats.map, 120);
        assert_eq!(stats.map_visibility, idleon_stats::MapVisibility::Advanced);
        assert_eq!(stats.skill, idleon_stats::Skill::Divinity);
        assert_eq!(stats.construction, idleon_stats::ConstructionMode::Inactive);
        let drop_rate = round_trip(&idleon_drop_rate::Selection {
            character: 3,
            map: 118,
        });
        assert_eq!(drop_rate.character, 3);
        assert_eq!(drop_rate.map, 118);
        let ladle = round_trip(&LadleConfig {
            tab: LadleTab::Exp,
            ladle_character: 3,
            breakdown_character: 2,
            claim_hours: 12.0,
            panel: LadlePanel {
                efficiency: 12_345.6,
                afk: 1.35,
                exp_multi: 2.75,
                prowess: 0.02,
                talent_148: 25.0,
            },
            assigned: vec![0, 2],
            balloon: 1,
            target_level: 60,
            split: true,
            kitchen: LadleKitchen {
                days: 12,
                base_speed: 480.0,
                policy: idleon_ladle::kitchen::Policy::Share,
                income_fraction: 0.25,
                ..LadleKitchen::default()
            },
            use_owned: false,
            scenario: LadleScenario::Layout,
            outcome_sort: LadleOutcomeSort::Name,
        });
        assert_eq!(ladle.tab, LadleTab::Exp);
        assert_eq!(ladle.ladle_character, 3);
        assert_eq!(ladle.breakdown_character, 2);
        assert_eq!(ladle.claim_hours, 12.0);
        assert_eq!(ladle.panel.efficiency, 12_345.6);
        assert_eq!(ladle.panel.talent_148, 25.0);
        assert_eq!(ladle.assigned, vec![0, 2]);
        assert_eq!(ladle.balloon, 1);
        assert_eq!(ladle.target_level, 60);
        assert!(ladle.split);
        assert_eq!(ladle.kitchen.days, 12);
        assert_eq!(ladle.kitchen.base_speed, 480.0);
        assert_eq!(ladle.kitchen.policy, idleon_ladle::kitchen::Policy::Share);
        assert_eq!(ladle.kitchen.income_fraction, 0.25);
        assert!(!ladle.use_owned);
        assert_eq!(ladle.scenario, LadleScenario::Layout);
        assert_eq!(ladle.outcome_sort, LadleOutcomeSort::Name);
        let statue = round_trip(&StatueConfig {
            character: 4,
            sort: StatueSort::Remaining,
            estimate: idleon_statue::Estimate::Average,
            targets: vec![Some(120), None],
            rates: vec!["1.2M".into(), String::new()],
            breakdown_statue: 7,
        });
        assert_eq!(statue.character, 4);
        assert_eq!(statue.sort, StatueSort::Remaining);
        assert_eq!(statue.estimate, idleon_statue::Estimate::Average);
        assert_eq!(statue.targets, vec![Some(120), None]);
        assert_eq!(statue.rates, vec!["1.2M".to_string(), String::new()]);
        assert_eq!(statue.breakdown_statue, 7);
        let mut toggles = idleon_combat::State::new();
        toggles.set(
            "dropRate",
            idleon_combat::Rule::with_value(idleon_combat::Mode::Scale, 2.0),
        );
        toggles.set(
            "compass3",
            idleon_combat::Rule::new(idleon_combat::Mode::On),
        );
        let combat = round_trip(&CombatConfig {
            character: 2,
            tab: CombatTab::Talents,
            presets: vec![CombatPreset {
                name: "Bossing".into(),
                character: 2,
                items: vec!["Blank".into(); idleon_combat::SLOT_COUNT],
                talents: [("0".to_string(), 5u32)].into_iter().collect(),
            }],
            toggles,
        });
        assert_eq!(combat.character, 2);
        assert_eq!(combat.tab, CombatTab::Talents);
        assert_eq!(combat.presets.len(), 1);
        assert_eq!(combat.presets[0].name, "Bossing");
        assert_eq!(combat.presets[0].items.len(), idleon_combat::SLOT_COUNT);
        assert_eq!(combat.toggles.len(), 2);
        assert_eq!(
            combat.toggles.rule("dropRate"),
            Some(idleon_combat::Rule::with_value(
                idleon_combat::Mode::Scale,
                2.0
            ))
        );
        assert_eq!(
            combat.toggles.rule("compass3"),
            Some(idleon_combat::Rule::new(idleon_combat::Mode::On))
        );
        let mastery = round_trip(&CookingMasteryState {
            goal: idleon_cooking_mastery::Goal::NextUnlock,
            budget: Some(12.0),
            tab: crate::sections::cooking_mastery::Tab::Yellow,
            yellow_goal: Some("totdmg".into()),
            yellow_budget: Some(9.0),
            character: 2,
        });
        assert_eq!(mastery.goal, idleon_cooking_mastery::Goal::NextUnlock);
        assert_eq!(mastery.budget, Some(12.0));
        assert_eq!(mastery.tab, crate::sections::cooking_mastery::Tab::Yellow);
        assert_eq!(mastery.yellow_goal.as_deref(), Some("totdmg"));
        assert_eq!(mastery.yellow_budget, Some(9.0));
        assert_eq!(mastery.character, 2);
        // A document that never touched the budget leaves it at every point.
        let untouched = round_trip(&CookingMasteryState::default());
        assert_eq!(untouched.goal, idleon_cooking_mastery::Goal::Now);
        assert_eq!(untouched.budget, None);
        assert_eq!(untouched.tab, crate::sections::cooking_mastery::Tab::Exp);
        assert_eq!(untouched.yellow_budget, None);
        let coral = round_trip(&coral::Inputs {
            goal: CoralGoal::DivinityPoints,
            budget: 250.0,
            god_rank: 3,
            assume_unlocked: true,
            ..coral::Inputs::default()
        });
        assert_eq!(coral.goal, CoralGoal::DivinityPoints);
        assert_eq!(coral.budget, 250.0);
        assert_eq!(coral.god_rank, 3);
        assert!(coral.assume_unlocked);
    }

    #[test]
    fn an_unknown_config_field_warns_and_the_rest_applies() {
        let mut warnings = Vec::new();
        let config: KillroyState = read(
            &json!({ "weeks": 20, "frogs": 2 }),
            "killroy",
            &mut warnings,
        )
        .expect("the known fields still read");
        assert_eq!(config.weeks, 20);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("frogs"), "{warnings:?}");
    }

    #[test]
    fn a_malformed_config_names_the_section_and_applies_nothing() {
        let mut warnings = Vec::new();
        let error = read::<KillroyState>(&json!({ "weeks": "twelve" }), "killroy", &mut warnings)
            .expect_err("a string where a number belongs is refused");
        assert!(error.contains("killroy"), "{error}");
        assert!(warnings.is_empty(), "{warnings:?}");

        let mut warnings = Vec::new();
        assert!(
            read::<SailingState>(&json!({ "mode": "frogs" }), "sailing", &mut warnings).is_err()
        );
    }

    #[test]
    fn prepare_refuses_a_document_whose_config_does_not_fit() {
        use idleon_import::pipeline;

        let mut document =
            pipeline::read(&project_text(json!({ "weeks": "twelve" }))).expect("the project reads");
        let error = prepare(&mut document).expect_err("the config is malformed");
        assert!(error.contains("killroy"), "{error}");
        // A document whose configs fit prepares every one of them.
        let mut document =
            pipeline::read(&project_text(json!({ "weeks": 20 }))).expect("the project reads");
        let configs = prepare(&mut document).expect("the config fits");
        assert_eq!(configs.len(), 1);
        assert!(matches!(
            &configs[0],
            Config::Killroy(KillroyState { weeks: 20, .. })
        ));
    }

    #[test]
    fn prepare_decodes_the_stats_context_from_a_project() {
        let text = json!({
            "format": idleon_project::FORMAT,
            "version": idleon_project::VERSION,
            "research": { "researchLevel": 120, "gridLevels": vec![0; 240] },
            "sections": {
                "stats-all": {
                    "character": 2,
                    "map": 120,
                    "mapVisibility": "all",
                    "skill": "divinity",
                    "construction": "inactive",
                },
            },
        })
        .to_string();
        let mut document = pipeline::read(&text).expect("the project reads");
        let configs = prepare(&mut document).expect("the context config fits");
        let Config::StatsAll(context) = &configs[0] else {
            panic!("the config is a stats context");
        };
        assert_eq!(context.character, 2);
        assert_eq!(context.map, 120);
        assert_eq!(context.map_visibility, idleon_stats::MapVisibility::All);
        assert_eq!(context.skill, idleon_stats::Skill::Divinity);
        assert_eq!(
            context.construction,
            idleon_stats::ConstructionMode::Inactive
        );
    }

    #[test]
    fn prepare_decodes_the_drop_rate_selection_from_a_project() {
        let text = json!({
            "format": idleon_project::FORMAT,
            "version": idleon_project::VERSION,
            "research": { "researchLevel": 120, "gridLevels": vec![0; 240] },
            "sections": { "drop-rate": { "character": 3, "map": 118 } },
        })
        .to_string();
        let mut document = pipeline::read(&text).expect("the project reads");
        let configs = prepare(&mut document).expect("the drop-rate config fits");
        let Config::DropRate(selection) = &configs[0] else {
            panic!("the config is a drop-rate selection");
        };
        assert_eq!(selection.character, 3);
        assert_eq!(selection.map, 118);
    }

    #[test]
    fn prepare_decodes_the_damage_context_from_a_project() {
        let text = json!({
            "format": idleon_project::FORMAT,
            "version": idleon_project::VERSION,
            "research": { "researchLevel": 120, "gridLevels": vec![0; 240] },
            "sections": { "damage": { "character": 3, "map": 120 } },
        })
        .to_string();
        let mut document = pipeline::read(&text).expect("the project reads");
        let configs = prepare(&mut document).expect("the damage config fits");
        let Config::Damage(context) = &configs[0] else {
            panic!("the config is a damage context");
        };
        assert_eq!(context.character, 3);
        assert_eq!(context.map, 120);
    }

    /// A project document carrying one killroy config, for the prepare tests.
    fn project_text(config: serde_json::Value) -> String {
        json!({
            "format": idleon_project::FORMAT,
            "version": idleon_project::VERSION,
            "research": { "researchLevel": 120, "gridLevels": vec![0; 240] },
            "sections": { "killroy": config },
        })
        .to_string()
    }

    #[test]
    fn a_partial_config_keeps_its_defaults() {
        let mut warnings = Vec::new();
        let config: SailingState =
            read(&json!({ "island": 2 }), "sailing", &mut warnings).expect("the config reads");
        assert_eq!(config.island, 2);
        assert_eq!(config.tab, crate::sections::sailing::Tab::Fleet);
        assert_eq!(config.mode, AssignmentMode::Optimized);
        assert_eq!(config.goal, Goal::Treasure);
        assert!(!config.sort_by_eta);
        assert_eq!(config.breakdown_boat, 0);
        assert!(warnings.is_empty(), "{warnings:?}");
    }

    #[test]
    fn an_index_past_the_grid_is_dropped_with_a_warning() {
        let mut warnings = Vec::new();
        assert_eq!(within(Some(3), 4, "selectedCell", &mut warnings), Some(3));
        assert_eq!(within(Some(4), 4, "selectedCell", &mut warnings), None);
        assert!(warnings.len() == 1, "{warnings:?}");
    }

    #[test]
    fn a_change_log_names_the_field_its_old_and_its_new_value() {
        let before = Simulation {
            research_level: 100,
            research_exp: 10.0,
            ..Simulation::default()
        };
        let after = Simulation {
            research_level: 122,
            research_exp: 42.5,
            ..Simulation::default()
        };
        let changes = research_changes(&before, &after);
        assert!(
            changes
                .iter()
                .any(|line| line == "researchLevel: 100 → 122"),
            "{changes:?}"
        );
        assert!(
            changes.iter().any(|line| line == "researchExp: 10 → 42.50"),
            "{changes:?}"
        );
        assert!(research_changes(&before, &before).is_empty());
    }

    #[test]
    fn supplement_changes_name_every_field_that_moved() {
        let before = SimContext::default();
        let after = SimContext {
            server_var_res_xp: 1.5,
            extra_pct: 10.0,
            ..SimContext::default()
        };
        let changes = supplement_changes(&before, &after);
        assert_eq!(changes.len(), 1, "{changes:?}");
        assert!(changes[0].starts_with("supplements: "), "{changes:?}");
        assert!(changes[0].contains("serverVarResXp"), "{changes:?}");
        assert!(changes[0].contains("extraPct"), "{changes:?}");
        assert!(supplement_changes(&before, &before).is_empty());
    }

    #[test]
    fn config_changes_name_every_section_whose_config_moved() {
        let before: Map<String, Value> = [("killroy".to_string(), json!({ "weeks": 12 }))]
            .into_iter()
            .collect();
        let after: Map<String, Value> = [
            ("killroy".to_string(), json!({ "weeks": 20 })),
            ("jar".to_string(), json!({ "tab": "production" })),
        ]
        .into_iter()
        .collect();
        let changes = config_changes(&before, &after);
        assert!(
            changes.contains(&"killroy: config changed".to_string()),
            "{changes:?}"
        );
        assert!(
            changes.contains(&"jar: config changed".to_string()),
            "{changes:?}"
        );
        assert!(config_changes(&before, &before).is_empty());
    }

    #[test]
    fn the_status_line_counts_the_changes_and_names_the_first() {
        assert_eq!(change_line(&[]), "Nothing changed.");
        assert_eq!(change_line(&["a".to_string()]), "Changed: a.");
        assert_eq!(
            change_line(&["a".to_string(), "b".to_string(), "c".to_string()]),
            "3 changes: a (and 2 more)."
        );
    }

    #[test]
    fn a_jelly_config_decodes_and_names_its_tab() {
        let text = serde_json::json!({
            "format": idleon_project::FORMAT,
            "version": idleon_project::VERSION,
            "section": "jelly-operator",
            "save": null,
            "sections": {
                "jelly-operator": {
                    "tab": "layout",
                    "layout": [[0, 6], [4, 2]],
                    "palette": 3,
                    "trials": 4,
                    "seed": 9
                }
            }
        })
        .to_string();
        let mut document = pipeline::read(&text).expect("the project reads");
        let configs = prepare(&mut document).expect("the jelly config decodes");
        match &configs[0] {
            Config::Jelly(config) => {
                assert_eq!(config.tab, crate::sections::jelly_operator::Tab::Layout);
                assert_eq!(config.layout, vec![(0, 6), (4, 2)]);
                assert_eq!(config.palette, 3);
                assert_eq!(config.trials, 4);
                assert_eq!(config.seed, 9);
                assert_eq!(
                    config.planner,
                    crate::sections::jelly_operator::Planner::default(),
                    "a document from before the planner keeps its defaults"
                );
            }
            _ => panic!("the jelly config decodes as the jelly variant"),
        }
    }

    #[test]
    fn the_jelly_config_round_trips_the_planner_controls() {
        let planner = crate::sections::jelly_operator::Planner {
            strategy: idleon_jelly::planner::Strategy::LowRisk,
            objective: idleon_jelly::planner::Objective::Exp,
            exp_cell_type: Some(2),
            tokens: Some(3),
            quality: idleon_jelly::planner::Quality::Fast,
            beam_width: 12,
            seed: 7,
        };
        let text = serde_json::to_string(&planner).expect("the planner serializes");
        let decoded: crate::sections::jelly_operator::Planner =
            serde_json::from_str(&text).expect("the planner deserializes");
        assert_eq!(decoded, planner);
        assert!(text.contains("\"expCellType\":2"), "{text}");
        assert!(text.contains("\"beamWidth\":12"), "{text}");
    }

    #[test]
    fn every_built_section_is_a_config_id_or_a_page_the_save_alone_fills() {
        // The sections whose visible state is the save alone carry no config;
        // a planned section has no state at all yet.
        let derived = [
            "cglunko",
            "coin-multi",
            "cooking-mastery",
            "endgame",
            "fountain",
            "shadow",
        ];
        for section in sections::SECTIONS.iter() {
            let id = section.id;
            let configured = [
                "research",
                "killroy",
                "sailing",
                "farming",
                "fishing",
                "statue",
                "monument",
                "pickle",
                "prisma",
                "clamworks",
                "cog",
                "jelly-operator",
                "jar",
                "opal",
                "ladle",
                "spelunking",
                "coral-god-rank",
                "cooking-mastery",
                "worship-td",
                "combat-sim",
                "stats-all",
                "damage",
                "royal-guardian",
                "sneaking",
                "drop-rate",
            ]
            .contains(&id);
            let covered = configured || derived.contains(&id);
            assert!(covered, "{id} has no project config");
        }
    }
}
