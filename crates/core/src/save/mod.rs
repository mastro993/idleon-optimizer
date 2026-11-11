//! Import a game save into a simulation, its multipliers, the farming field,
//! the account multiplier tree and the Clamworks state.
//!
//! The upstream calculators read the game's save through Google sign-in and a
//! decode step; this app takes the decoded object as text. The layout follows
//! the upstream decoder in `js/save/loader.js`:
//!
//! - `it.json` wraps the save in `data`; a plain `save.json` is flat. The
//!   server constants (`serverVars`) and the companion list sit beside `data`.
//! - The research state is row-encoded: `Research` is a JSON string holding
//!   rows `[gridLevels, shapeOverlay, occFound, insightProgress, insightLvs,
//!   magData, shapePositions, …]`, where `magData` and `shapePositions` are flat
//!   groups of four numbers.
//! - The research level is the highest `Lv0_<char>[20]` across characters, and
//!   the accumulated EXP is `Exp0_<char>[20]` for a character at that level.
//! - The farming side reads `FarmPlot`, `FarmRank`, `FarmUpg`, `FarmCrop`,
//!   `Spelunk[50]` and `GemItemsPurchased`; the active character's Farming level
//!   is `Lv0_<char>[16]`. A research-only save stays silent about farming.
//!
//! The app's own state exports use the decoded names (`gridLevels`, `magData`,
//! …) at the top level, so the importer accepts either shape. [`import`] reports
//! which shape it read in `Imported::format`, so a caller can preview a save and
//! apply it in a second step without parsing twice.
//!
//! Every account multiplier the research simulator reads is decoded from the
//! save by [`account`], which also reports the tree it built. A term whose
//! save field is missing is left out and warned about by that field's name;
//! nothing is guessed.

mod account;
mod farming;
mod formula;
pub(crate) mod json;
mod research;
mod shadow;
mod value;

pub use shadow::{Shadow, ShadowVerdict};
pub use value::{Save, at, number, row_values};

use crate::clamworks::Clamworks;
use crate::farming::Farm;
use crate::model::{SimContext, Simulation};
pub use account::{BonusTerm, BonusTree};
pub use formula::log as game_log;
use serde_json::{Map, Value};

/// The shape the imported text had, as the upstream loader detects it
/// (`raw.data ? 'it.json' : 'save.json'`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    /// `it.json`: the decoded save under `data`, with `serverVars` and the
    /// companion roster beside it.
    ItJson,
    /// `save.json`: the decoded save at the top level.
    SaveJson,
    /// The decoded research fields of an app state export, at the top level.
    ResearchState,
    /// The decoded farming fields of an app state export, at the top level.
    FarmingState,
}

impl Format {
    /// The name a preview shows.
    pub fn label(self) -> &'static str {
        match self {
            Format::ItJson => "it.json",
            Format::SaveJson => "save.json",
            Format::ResearchState => "research state",
            Format::FarmingState => "farming state",
        }
    }
}

/// What an import produced.
#[derive(Clone, Debug)]
pub struct Imported {
    pub simulation: Simulation,
    pub context: SimContext,
    /// The farming field, ranks, crops and market levels the save carries.
    pub farming: Farm,
    /// The save-derived multiplier tree behind [`Self::context`].
    pub tree: BonusTree,
    /// The Clamworks state the save carries, or the zero state when it has none.
    pub clamworks: Clamworks,
    /// The decoded save with its `it.json` wrapper, kept whole so read-only
    /// sections can read raw fields and audit it.
    pub save: Save,
    /// The `it.json` wrapper's companion roster, beside the save.
    pub companion: Option<Value>,
    /// Which shape the text had.
    pub format: Format,
    /// How many characters the save carries, from its `Lv0_<n>` rows.
    pub characters: usize,
    /// How many names the `charNames` roster beside the save carries.
    pub named_characters: usize,
    /// Save fields the import could not read or had to drop.
    pub warnings: Vec<String>,
    /// The shadow-ban flag (`OptLacc[26]`) and the answer the upstream page gives.
    pub shadow: Shadow,
}

/// A save field that may be a JSON value or a string holding one.
pub(crate) fn parsed(value: &Value, key: &str) -> Option<Value> {
    json::parsed_field(value.as_object()?, key)
}

/// Why an import could not be read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImportError {
    /// The text is not JSON.
    NotJson(String),
    /// JSON, but not an object that looks like a save.
    NoSaveFields(String),
}

impl std::fmt::Display for ImportError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ImportError::NotJson(detail) => write!(formatter, "not valid JSON: {detail}"),
            ImportError::NoSaveFields(detail) => {
                write!(formatter, "no save fields found: {detail}")
            }
        }
    }
}

impl std::error::Error for ImportError {}

/// Top-level names that mark an object as research state, whichever shape the
/// object uses (`Research` rows, or the decoded names of a state export).
const RESEARCH_FIELDS: [&str; 11] = [
    "Research",
    "researchLevel",
    "researchExp",
    "cachedResearchExp",
    "gridLevels",
    "shapeOverlay",
    "shapePositions",
    "occFound",
    "insightLvs",
    "insightProgress",
    "magData",
];

/// One character's progress, from `Lv0_<n>` and `Exp0_<n>`.
#[derive(Clone, Debug, Default)]
struct Character {
    /// The `Lv0_<n>` row: level 0, skills 1..21, divinity at 14, research at 20.
    levels: Vec<Option<f64>>,
    research_exp: Option<f64>,
}

impl Character {
    fn research_level(&self) -> Option<u32> {
        self.levels
            .get(20)
            .copied()
            .flatten()
            .map(|level| level.max(0.0).round() as u32)
    }

    fn divinity(&self) -> Option<u32> {
        self.levels
            .get(14)
            .copied()
            .flatten()
            .map(|level| level.max(0.0).round() as u32)
    }

    /// The Farming skill level the farming planner reads (`Lv0_<n>[16]`).
    fn farming_level(&self) -> Option<u32> {
        self.levels
            .get(16)
            .copied()
            .flatten()
            .map(|level| level.max(0.0).round() as u32)
    }

    /// The summed skill level at `skill` (0 = the first skill row).
    fn skill(&self, skill: usize) -> f64 {
        self.levels.get(skill + 1).copied().flatten().unwrap_or(0.0)
    }
}

/// Read a decoded save object (the `SaveData` JSON the game writes) into a run.
pub fn import(save_json: &str) -> Result<Imported, ImportError> {
    let parsed: Value =
        serde_json::from_str(save_json).map_err(|error| ImportError::NotJson(error.to_string()))?;
    let root = parsed
        .as_object()
        .ok_or_else(|| ImportError::NoSaveFields("the top level is not an object".into()))?;
    // `it.json` nests the save under `data`; a plain save is the object itself.
    let save = match root.get("data") {
        Some(Value::Object(save)) => save,
        _ => root,
    };

    let mut warnings = Vec::new();
    let rows = research::research_rows(save, &mut warnings);
    let format = detect(root, save);
    let carries_research = RESEARCH_FIELDS
        .iter()
        .any(|field| save.contains_key(*field));
    let carries_farming = farming::has_farming(save);
    let Some(format) = format.filter(|_| !rows.is_empty() || carries_research || carries_farming)
    else {
        return Err(ImportError::NoSaveFields(format!(
            "expected a research seed field ({}) or a farming field ({}) at the top level or in the Research rows",
            RESEARCH_FIELDS.join(", "),
            farming::FARMING_FIELDS.join(", ")
        )));
    };

    let characters = characters(save);
    let roster = character_count(save);
    let named_characters = root
        .get("charNames")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    if roster == 0 {
        warnings
            .push("characters: no Lv0_<n> rows in the save; only the account fields import".into());
    }
    let mut simulation = Simulation::default();

    match research::research_level(save, &characters) {
        Some(level) => simulation.research_level = level,
        None => warnings.push("researchLevel: not in the save; kept at 1".into()),
    }
    simulation.research_exp = research::research_exp(save, &characters, simulation.research_level);

    match research::grid_levels(save, &rows) {
        Some(levels) => simulation.grid = research::grid(&levels, &mut warnings),
        None => warnings.push("gridLevels: not in the save; the grid stays empty".into()),
    }
    simulation.observations = research::observations(save, &rows, &mut warnings);
    simulation.mags = research::mags(save, &rows, &mut warnings);
    simulation.placements = research::placements(save, &rows, &mut warnings);
    simulation.saved_overlay = research::shape_overlay(save, &rows);

    let (context, tree) = account::account(save, root, &characters, &simulation, &mut warnings);
    let farm_level = characters
        .first()
        .and_then(Character::farming_level)
        .unwrap_or(0);
    let farming = farming::farm(save, farm_level, &mut warnings).unwrap_or_default();
    let clamworks = Clamworks::from_save(save, &mut warnings);
    let shadow = Shadow::read(save);
    let companion = root.get("companion").cloned();

    Ok(Imported {
        simulation,
        context,
        farming,
        tree,
        clamworks,
        save: Save::new(parsed),
        companion,
        format,
        characters: roster,
        named_characters,
        warnings,
        shadow,
    })
}

/// Which shape the text has: a save wrapped in `data`, a flat save, or an app
/// state export under its decoded field names.
fn detect(root: &Map<String, Value>, save: &Map<String, Value>) -> Option<Format> {
    if matches!(root.get("data"), Some(Value::Object(_))) {
        return Some(Format::ItJson);
    }
    let raw = save.contains_key("Research")
        || farming::RAW_FARMING_FIELDS
            .iter()
            .any(|field| save.contains_key(*field))
        || save
            .keys()
            .any(|key| key.starts_with("Lv0_") || key.starts_with("Exp0_"));
    if raw {
        return Some(Format::SaveJson);
    }
    if RESEARCH_FIELDS
        .iter()
        .any(|field| save.contains_key(*field))
    {
        return Some(Format::ResearchState);
    }
    farming::FARMING_FIELDS
        .iter()
        .any(|field| save.contains_key(*field))
        .then_some(Format::FarmingState)
}

/// How many characters the save carries: one per `Lv0_<n>` row.
fn character_count(save: &Map<String, Value>) -> usize {
    save.keys()
        .filter(|key| {
            key.strip_prefix("Lv0_")
                .is_some_and(|index| index.parse::<usize>().is_ok())
        })
        .count()
}

fn characters(save: &Map<String, Value>) -> Vec<Character> {
    let mut characters: Vec<Character> = Vec::new();
    for (key, _) in save {
        let Some(index) = key
            .strip_prefix("Lv0_")
            .and_then(|tail| tail.parse::<usize>().ok())
        else {
            continue;
        };
        if characters.len() <= index {
            characters.resize(index + 1, Character::default());
        }
        let row = json::parsed_field(save, key);
        characters[index].levels = row
            .as_ref()
            .and_then(Value::as_array)
            .map(|cells| cells.iter().map(json::number).collect())
            .unwrap_or_default();
        characters[index].research_exp = json::parsed_field(save, &format!("Exp0_{index}"))
            .as_ref()
            .and_then(Value::as_array)
            .and_then(|cells| cells.get(20))
            .and_then(json::number);
    }
    characters
}
