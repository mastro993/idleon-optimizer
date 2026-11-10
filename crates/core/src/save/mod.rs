//! Import a game save into a simulation and its multipliers.
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
//!
//! The app's own state exports use the decoded names (`gridLevels`, `magData`,
//! …) at the top level, so the importer accepts either shape. [`import`] reports
//! which shape it read in `Imported::format`, so a caller can preview a save and
//! apply it in a second step without parsing twice.
//!
//! Deliberately unsupported, because the save does not carry the inputs the
//! upstream needs: the additive account research bonus tree (`extra_pct`, read
//! from `cachedExtPctExSticker` when the object has it) and the true
//! multipliers from The Button, the arcade, Clam Works, equipment, Menagerie,
//! Jelly and the Royal Guardian outpost. Warned about when the field is absent.

mod account;
pub(crate) mod json;
mod research;
mod shadow;

pub use shadow::{Shadow, ShadowVerdict};

use crate::model::{SimContext, Simulation};
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
}

impl Format {
    /// The name a preview shows.
    pub fn label(self) -> &'static str {
        match self {
            Format::ItJson => "it.json",
            Format::SaveJson => "save.json",
            Format::ResearchState => "research state",
        }
    }
}

/// What an import produced.
#[derive(Clone, Debug)]
pub struct Imported {
    pub simulation: Simulation,
    pub context: SimContext,
    /// The decoded save object (`data` in an `it.json`, the object itself in a
    /// flat save), kept whole so read-only sections can audit it.
    pub save: Value,
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

/// One character's research progress, from `Lv0_<n>` and `Exp0_<n>`.
#[derive(Clone, Copy, Debug, Default)]
struct Character {
    research_level: Option<u32>,
    research_exp: Option<f64>,
    divinity: Option<u32>,
}

/// Read a decoded save object (the `SaveData` JSON the game writes) into a run.
pub fn import(save_json: &str) -> Result<Imported, ImportError> {
    let root: Value =
        serde_json::from_str(save_json).map_err(|error| ImportError::NotJson(error.to_string()))?;
    let root = root
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
    let Some(format) = format.filter(|_| !rows.is_empty() || carries_research) else {
        return Err(ImportError::NoSaveFields(format!(
            "expected one of {} at the top level or in the Research rows",
            RESEARCH_FIELDS.join(", ")
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

    let context = account::account(save, root, &characters, &simulation, &mut warnings);

    Ok(Imported {
        simulation,
        context,
        save: Value::Object(save.clone()),
        format,
        characters: roster,
        named_characters,
        warnings,
        shadow: Shadow::read(save),
    })
}

/// Which shape the text has: a save wrapped in `data`, a flat save, or an app
/// state export under its decoded field names.
fn detect(root: &Map<String, Value>, save: &Map<String, Value>) -> Option<Format> {
    if matches!(root.get("data"), Some(Value::Object(_))) {
        return Some(Format::ItJson);
    }
    let raw = save.contains_key("Research")
        || save
            .keys()
            .any(|key| key.starts_with("Lv0_") || key.starts_with("Exp0_"));
    if raw {
        return Some(Format::SaveJson);
    }
    RESEARCH_FIELDS
        .iter()
        .any(|field| save.contains_key(*field))
        .then_some(Format::ResearchState)
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
        let cells = row.as_ref().and_then(Value::as_array);
        characters[index].research_level = cells
            .and_then(|cells| cells.get(20))
            .and_then(json::number)
            .map(|level| level.max(0.0).round() as u32);
        characters[index].divinity = cells
            .and_then(|cells| cells.get(14))
            .and_then(json::number)
            .map(|level| level.max(0.0).round() as u32);
        characters[index].research_exp = json::parsed_field(save, &format!("Exp0_{index}"))
            .as_ref()
            .and_then(Value::as_array)
            .and_then(|cells| cells.get(20))
            .and_then(json::number);
    }
    characters
}
