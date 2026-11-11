//! The app's project file: what an export writes and an import applies.
//!
//! [`idleon_core::project`] owns the document — the format markers, the
//! version, the research state, the supplements and the ROv2 migration. This
//! module owns the part that is the window's: which section it was on and the
//! config object each section keeps as its visible state.
//!
//! Every section config is a plain serde struct, so an export is a `to_value`
//! and an import a `from_value`; a config whose field this build does not know
//! is named in a warning and the section keeps what it had, never a panic.

use crate::sections;
use crate::sections::clamworks::Tab as ClamworksTab;
use crate::sections::farming::State as FarmingState;
use crate::sections::jar::Tab as JarTab;
use crate::sections::killroy::State as KillroyState;
use crate::sections::monument::State as MonumentState;
use crate::sections::pickle::State as PickleState;
use crate::sections::research::Tab as ResearchTab;
use crate::sections::sailing::State as SailingState;
use crate::sections::statue::Sort as StatueSort;
use crate::state::{AppState, Route};
use idleon_core::coral;
use idleon_core::model::Effort;
use idleon_core::pickle::{DEFAULT_TTK, MAX_DPS, MAX_TTK, MIN_TTK};
use idleon_core::project;
use idleon_core::sailing::{AssignmentMode, Goal};
use idleon_core::{game, save};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};

/// The research section's config: its tab, selection, target and supplements.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct ResearchConfig {
    tab: ResearchTab,
    selected_cell: Option<usize>,
    selected_shape: Option<usize>,
    /// `"hours"` or `"level"`, matching the target radio upstream.
    target_kind: String,
    target_value: f64,
    effort: Effort,
    supplements_open: bool,
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
            supplements_open: true,
        }
    }
}

/// The sailing section's config: its pairings mode, island, goal and the chest
/// tier floor that lives on the route context.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct SailingConfig {
    mode: AssignmentMode,
    island: usize,
    goal: Goal,
    minimum_tier: u32,
}

/// The Clamworks section's config: its tab and the entered multikill total.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct ClamworksConfig {
    tab: ClamworksTab,
    multikill: f64,
}

/// The Jar section's config: its tab.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct JarConfig {
    tab: JarTab,
}

/// The statue section's config: selected character and sort. The search text is
/// a filter, not state, and the statue rows come from the save.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct StatueConfig {
    character: usize,
    sort: StatueSort,
}

/// The whole window state as a project document.
pub fn export(state: &AppState) -> String {
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
pub fn configs(state: &AppState) -> Map<String, Value> {
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
                idleon_core::model::Target::Hours(_) => "hours".into(),
                idleon_core::model::Target::Level(_) => "level".into(),
            },
            target_value: match state.target {
                idleon_core::model::Target::Hours(hours) => hours,
                idleon_core::model::Target::Level(level) => f64::from(level),
            },
            effort: state.effort,
            supplements_open: state.supplements_open,
        }),
    );
    insert("killroy", value(&state.killroy));
    insert(
        "sailing",
        value(&SailingConfig {
            mode: state.sailing_state.mode,
            island: state.sailing_state.island,
            goal: state.sailing_state.goal,
            minimum_tier: state.sailing.minimum_tier,
        }),
    );
    insert("farming", value(&state.farming));
    insert(
        "statue",
        value(&StatueConfig {
            character: state.statue.character,
            sort: state.statue.sort,
        }),
    );
    insert("monument", value(&state.monument));
    insert("pickle", value(&state.pickle));
    insert(
        "clamworks",
        value(&ClamworksConfig {
            tab: state.clamworks_tab,
            multikill: state.clamworks_multikill,
        }),
    );
    insert("jar", value(&JarConfig { tab: state.jar_tab }));
    insert("coral-god-rank", value(&state.coral.inputs));
    configs
}

/// Apply a project document to the window, collecting what it could not read.
///
/// The embedded save goes through the one save importer every section already
/// reads, so the account sections land exactly where a raw save import leaves
/// them; a project without a readable save still applies its research half.
pub fn apply(state: &mut AppState, project: project::Project, warnings: &mut Vec<String>) {
    let imported = project
        .save
        .as_ref()
        .and_then(|save| match save::import(&save.to_string()) {
            Ok(imported) => Some(imported),
            Err(error) => {
                warnings.push(format!(
                    "save: {error}; the project's research state still applies"
                ));
                None
            }
        });
    match imported {
        Some(mut imported) => {
            warnings.extend(imported.warnings.iter().cloned());
            imported.simulation = project.simulation;
            imported.context = project.context;
            state.load_imported(imported);
        }
        None => {
            state.simulation = project.simulation;
            state.context = project.context;
        }
    }
    for (id, config) in &project.sections {
        if !apply_section(state, id, config, warnings) {
            warnings.push(format!(
                "sections: {id} is not a section this build declares"
            ));
        }
    }
    if let Some(id) = &project.section {
        match sections::find(id) {
            Some(section) => {
                state.route = Route::Section(section);
                state.last_section = Some(section.id);
            }
            None => warnings.push(format!(
                "section: {id} is not a section this build declares"
            )),
        }
    }
}

/// Apply one section's config. Returns whether the registry declares the id.
pub fn apply_section(
    state: &mut AppState,
    id: &str,
    config: &Value,
    warnings: &mut Vec<String>,
) -> bool {
    match id {
        "research" => {
            let Some(config) = read::<ResearchConfig>(config, id, warnings) else {
                return true;
            };
            state.research.tab = config.tab;
            state.effort = config.effort;
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
                "level" => state.target = idleon_core::model::Target::Level(config.target_value.max(1.0).round() as u32),
                "hours" => state.target = idleon_core::model::Target::Hours(config.target_value.max(0.5)),
                other => warnings.push(format!(
                    "research: targetKind {other:?} is not \"hours\" or \"level\"; the target is kept"
                )),
            }
        }
        "killroy" => {
            if let Some(config) = read::<KillroyState>(config, id, warnings) {
                state.killroy = KillroyState {
                    weeks: config.weeks.clamp(1, 52),
                };
            }
        }
        "sailing" => {
            let Some(config) = read::<SailingConfig>(config, id, warnings) else {
                return true;
            };
            state.sailing_state = SailingState {
                mode: config.mode,
                island: config
                    .island
                    .min(state.fleet.available_islands().saturating_sub(1)),
                goal: config.goal,
            };
            state.sailing.minimum_tier = config.minimum_tier.min(5);
        }
        "farming" => {
            if let Some(config) = read::<FarmingState>(config, id, warnings) {
                state.farming = FarmingState {
                    selected_plot: config.selected_plot.min(idleon_core::farming::PLOTS - 1),
                    target_crop: config.target_crop,
                };
            }
        }
        "statue" => {
            if let Some(config) = read::<StatueConfig>(config, id, warnings) {
                state.statue.sort = config.sort;
                if state.statue.save.has_character(config.character) {
                    state.statue.character = config.character;
                } else {
                    warnings.push(format!(
                        "statue: character {} has no saved rows; the table keeps its character",
                        config.character + 1
                    ));
                }
            }
        }
        "monument" => {
            if let Some(config) = read::<MonumentState>(config, id, warnings) {
                state.monument = MonumentState {
                    tab: config.tab,
                    wave: config.wave.clamp(1, 1001),
                    boosts: config.boosts,
                    rerolls: config.rerolls,
                    remaining_hp: config.remaining_hp.filter(|hp| hp.is_finite() && *hp > 0.0),
                };
            }
        }
        "pickle" => {
            if let Some(config) = read::<PickleState>(config, id, warnings) {
                state.pickle = PickleState {
                    dps: if config.dps.is_finite() {
                        config.dps.clamp(0.0, MAX_DPS)
                    } else {
                        0.0
                    },
                    max_ttk: if config.max_ttk.is_finite() {
                        config.max_ttk.clamp(MIN_TTK, MAX_TTK)
                    } else {
                        DEFAULT_TTK
                    },
                };
            }
        }
        "clamworks" => {
            if let Some(config) = read::<ClamworksConfig>(config, id, warnings) {
                state.clamworks_tab = config.tab;
                state.clamworks_multikill = if config.multikill.is_finite() {
                    config.multikill.max(0.0)
                } else {
                    0.0
                };
            }
        }
        "jar" => {
            if let Some(config) = read::<JarConfig>(config, id, warnings) {
                state.jar_tab = config.tab;
            }
        }
        "coral-god-rank" => {
            if let Some(config) = read::<coral::Inputs>(config, id, warnings) {
                state.coral.inputs = config;
            }
        }
        _ => return false,
    }
    true
}

/// A config struct as the document stores it.
fn value<T: Serialize>(config: &T) -> Value {
    serde_json::to_value(config).unwrap_or(Value::Null)
}

/// Read one section's config, naming the fields this build does not know and
/// keeping the section's current config when the shape does not fit.
fn read<T: Serialize + DeserializeOwned + Default>(
    config: &Value,
    section: &str,
    warnings: &mut Vec<String>,
) -> Option<T> {
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
    match serde_json::from_value(config.clone()) {
        Ok(config) => Some(config),
        Err(error) => {
            warnings.push(format!(
                "{section}: {error}; the section keeps its current config"
            ));
            None
        }
    }
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
    use idleon_core::coral::Goal as CoralGoal;
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
        let killroy = round_trip(&KillroyState { weeks: 20 });
        assert_eq!(killroy.weeks, 20);
        let sailing = round_trip(&SailingState {
            mode: AssignmentMode::Saved,
            island: 3,
            goal: Goal::Chests,
        });
        assert_eq!(sailing.mode, AssignmentMode::Saved);
        assert_eq!(sailing.island, 3);
        assert_eq!(sailing.goal, Goal::Chests);
        let farming = round_trip(&FarmingState {
            selected_plot: 5,
            target_crop: 12,
        });
        assert_eq!(farming.selected_plot, 5);
        assert_eq!(farming.target_crop, 12);
        let monument = round_trip(&MonumentState {
            tab: crate::sections::monument::Tab::Sources,
            wave: 42,
            boosts: 3,
            rerolls: Some(7),
            remaining_hp: Some(1234.5),
        });
        assert_eq!(monument.tab, crate::sections::monument::Tab::Sources);
        assert_eq!(monument.wave, 42);
        assert_eq!(monument.boosts, 3);
        assert_eq!(monument.rerolls, Some(7));
        assert_eq!(monument.remaining_hp, Some(1234.5));
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
            supplements_open: false,
        });
        assert_eq!(research.tab, ResearchTab::Run);
        assert_eq!(research.selected_cell, Some(129));
        assert_eq!(research.selected_shape, Some(2));
        assert_eq!(research.target_kind, "level");
        assert_eq!(research.target_value, 140.0);
        assert_eq!(research.effort, Effort::Thorough);
        assert!(!research.supplements_open);
        let clamworks = round_trip(&ClamworksConfig {
            tab: ClamworksTab::Upgrades,
            multikill: 7.0,
        });
        assert_eq!(clamworks.tab, ClamworksTab::Upgrades);
        assert_eq!(clamworks.multikill, 7.0);
        let jar = round_trip(&JarConfig {
            tab: JarTab::Production,
        });
        assert_eq!(jar.tab, JarTab::Production);
        let statue = round_trip(&StatueConfig {
            character: 4,
            sort: StatueSort::Remaining,
        });
        assert_eq!(statue.character, 4);
        assert_eq!(statue.sort, StatueSort::Remaining);
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
    fn a_malformed_config_warns_instead_of_panicking() {
        let mut warnings = Vec::new();
        let config: Option<KillroyState> =
            read(&json!({ "weeks": "twelve" }), "killroy", &mut warnings);
        assert!(config.is_none());
        assert!(warnings[0].contains("killroy"), "{warnings:?}");

        let mut warnings = Vec::new();
        let sailing: Option<SailingState> =
            read(&json!({ "mode": "frogs" }), "sailing", &mut warnings);
        assert!(sailing.is_none());
        assert!(!warnings.is_empty());
    }

    #[test]
    fn a_partial_config_keeps_its_defaults() {
        let mut warnings = Vec::new();
        let config: SailingState =
            read(&json!({ "island": 2 }), "sailing", &mut warnings).expect("the config reads");
        assert_eq!(config.island, 2);
        assert_eq!(config.mode, AssignmentMode::Optimized);
        assert_eq!(config.goal, Goal::Treasure);
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
    fn every_built_section_is_a_config_id_or_a_page_the_save_alone_fills() {
        // The sections whose visible state is the save alone carry no config;
        // a planned section has no state at all yet.
        let derived = ["endgame", "shadow"];
        for section in sections::SECTIONS.iter() {
            let id = section.id;
            let configured = [
                "research",
                "killroy",
                "sailing",
                "farming",
                "statue",
                "monument",
                "pickle",
                "clamworks",
                "jar",
                "coral-god-rank",
            ]
            .contains(&id);
            let covered = configured || derived.contains(&id);
            assert!(
                covered || section.status == sections::Status::Planned,
                "{id} has no project config"
            );
        }
    }
}
