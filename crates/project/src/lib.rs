//! The app's project file: the whole window state as one versioned JSON
//! document.
//!
//! The project file keeps the research state in plain JSON so it stays
//! diffable, beside the decoded save and one config object per section.
//!
//! ```json
//! {
//!   "format": "idleon-optimizer-project",
//!   "version": 1,
//!   "section": "research",
//!   "save": { "...": "the decoded save or its it.json wrapper" },
//!   "research": { "...": "the decoded research state" },
//!   "supplements": { "...": "the save-derived multipliers and user supplements" },
//!   "sections": { "killroy": { "weeks": 12 } }
//! }
//! ```
//!
//! The `research` object is the shape `idleon_import::import` already reads as
//! `Format::ResearchState`, so an export's research half pastes into the import
//! dialog on its own. `supplements` is `SimContext` field for field, so a round
//! trip keeps the visible rate even when a later build decodes the save
//! differently.
//!
//! A section config document is the same header around one section:
//!
//! ```json
//! { "format": "idleon-optimizer-section", "version": 1, "section": "killroy",
//!   "config": { "weeks": 12 } }
//! ```
//!
//! `read` never guesses: a version it does not know is an error naming the
//! version and the one this build reads, a `format` marker it does not know is
//! an error naming the marker and the ones it reads, and text with no marker at
//! all is [`Error::NotADocument`] so `idleon_import::pipeline` can try the raw save
//! importer. Fields this build does not model are named in a warning instead of
//! failing the document.

use idleon_research::data::game;
use idleon_research::model::{MagKind, SimContext, Simulation};
use idleon_research::save as research;
use idleon_research::tiers::Tiers;
use idleon_save::json;
use serde_json::{Map, Value, json};

/// The project file's format marker.
pub const FORMAT: &str = "idleon-optimizer-project";
/// The project file's schema version.
pub const VERSION: u32 = 1;
/// A per-section config document's format marker.
pub const SECTION_FORMAT: &str = "idleon-optimizer-section";

/// A project document.
#[derive(Clone, Debug, PartialEq)]
pub struct Project {
    pub version: u32,
    /// The section the window was on, when the export came from a section.
    pub section: Option<String>,
    /// The decoded save, wrapper included, so every account section round-trips.
    pub save: Option<Value>,
    pub simulation: Simulation,
    /// The save-derived multipliers and user supplements behind the rate.
    pub context: SimContext,
    /// Whether the document carried a `research` block; `false` means
    /// `simulation` is the default, not a state the document chose.
    pub has_research: bool,
    /// Whether the document carried a `supplements` block; `false` means
    /// `context` is the default.
    pub has_supplements: bool,
    /// One config object per section id.
    pub sections: Map<String, Value>,
    /// Fields the document carried that this build could not read.
    pub warnings: Vec<String>,
}

/// A single section's config document.
#[derive(Clone, Debug, PartialEq)]
pub struct SectionConfig {
    pub version: u32,
    pub section: String,
    pub config: Value,
    pub warnings: Vec<String>,
}

/// What an import text turned out to be.
#[derive(Clone, Debug, PartialEq)]
pub enum Document {
    /// Boxed: a project carries the simulation, the context and the save.
    Project(Box<Project>),
    Section(SectionConfig),
}

/// Why a document could not be read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// Neither a project nor a section config.
    NotADocument,
    /// A document with a format marker this build does not know.
    Format(String),
    /// A version this build cannot migrate.
    Version(String),
    /// A section config with no section id to apply it to.
    Section(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::NotADocument => write!(formatter, "neither a project file nor a section config"),
            Error::Format(found) => write!(
                formatter,
                "unknown format \"{found}\"; this build reads \"{FORMAT}\", \"{SECTION_FORMAT}\" or a decoded save"
            ),
            Error::Version(detail) => write!(formatter, "unsupported version: {detail}"),
            Error::Section(detail) => write!(formatter, "no section to apply: {detail}"),
        }
    }
}

impl std::error::Error for Error {}

/// Read a project or a section config.
pub fn read(text: &str) -> Result<Document, Error> {
    let text = text.trim();
    // Text that is not JSON is simply not one of this module's documents: the
    // caller's raw save importer reports the parse error with its own wording.
    let Ok(parsed) = serde_json::from_str::<Value>(text) else {
        return Err(Error::NotADocument);
    };
    // Same for an array or a scalar: not a document, so the save importer names
    // the shape it expected.
    let Some(object) = parsed.as_object() else {
        return Err(Error::NotADocument);
    };
    match object.get("format") {
        Some(Value::String(format)) if format == FORMAT => project(object),
        Some(Value::String(format)) if format == SECTION_FORMAT => section(object),
        // A marker this build does not know is a wrong format, not a save: say
        // which marker it found instead of falling through to `no save fields`.
        Some(Value::String(format)) => Err(Error::Format(format.clone())),
        _ => Err(Error::NotADocument),
    }
}

/// The project document as a JSON value: what [`write`] prints.
pub fn value(project: &Project) -> Value {
    let mut object = Map::new();
    object.insert("format".into(), Value::from(FORMAT));
    object.insert("version".into(), Value::from(project.version));
    if let Some(section) = &project.section {
        object.insert("section".into(), Value::from(section.clone()));
    }
    if let Some(save) = &project.save {
        object.insert("save".into(), save.clone());
    }
    object.insert("research".into(), research_value(&project.simulation));
    object.insert(
        "supplements".into(),
        serde_json::to_value(&project.context).unwrap_or(Value::Null),
    );
    object.insert("sections".into(), Value::Object(project.sections.clone()));
    Value::Object(object)
}

/// Write a project document, pretty-printed so a human can read and edit it.
pub fn write(project: &Project) -> String {
    serde_json::to_string_pretty(&value(project)).unwrap_or_default()
}

/// The decoded research state behind a simulation, in the shape both the
/// project file and the `Format::ResearchState` importer read.
fn research_value(simulation: &Simulation) -> Value {
    let mags: Vec<Value> = simulation
        .mags
        .iter()
        .map(|mag| {
            json!({
                "type": match mag.kind {
                    MagKind::Regular => 0,
                    MagKind::Monocle => 1,
                    MagKind::Kaleidoscope => 2,
                },
                "slot": mag.slot,
            })
        })
        .collect();
    let placements: Vec<Value> = simulation
        .placements
        .iter()
        .map(|placement| {
            json!({
                "shape": placement.shape,
                "x": placement.x,
                "y": placement.y,
                "rot": placement.rot,
            })
        })
        .collect();
    json!({
        "researchLevel": simulation.research_level,
        "researchExp": simulation.research_exp,
        "gridLevels": simulation.grid.levels(),
        "shapeOverlay": simulation.saved_overlay,
        "occFound": simulation.observations.found.iter().map(|found| u8::from(*found)).collect::<Vec<u8>>(),
        "insightLvs": simulation.observations.insight_level,
        "insightProgress": simulation.observations.insight_exp,
        "magData": mags,
        "shapePositions": placements,
        "preset": simulation.preset,
        "shapeTiers": simulation.shape_tiers,
    })
}

/// Read the research state back, warning about every field it had to leave out.
fn simulation(value: &Value, warnings: &mut Vec<String>) -> Simulation {
    let mut simulation = Simulation::default();
    let Some(object) = value.as_object() else {
        warnings.push("research: not an object; the simulation starts empty".into());
        return simulation;
    };
    match object.get("researchLevel").and_then(json::number) {
        Some(level) => simulation.research_level = level.max(0.0).round() as u32,
        None => warnings.push("researchLevel: not in the project; kept at 1".into()),
    }
    simulation.research_exp = object
        .get("researchExp")
        .or_else(|| object.get("cachedResearchExp"))
        .and_then(json::number)
        .unwrap_or(0.0);
    match research::grid_levels(object, &[]) {
        Some(levels) => simulation.grid = research::grid(&levels, warnings),
        None => warnings.push("gridLevels: not in the project; the grid stays empty".into()),
    }
    simulation.observations = research::observations(object, &[], warnings);
    simulation.mags = research::mags(object, &[], warnings);
    simulation.placements = research::placements(object, &[], warnings);
    simulation.saved_overlay = research::shape_overlay(object, &[]);
    if let Some(preset) = object.get("preset").and_then(Value::as_str) {
        if game().presets.iter().any(|entry| entry.id == preset) {
            simulation.preset = preset.to_string();
        } else {
            warnings.push(format!(
                "preset: {preset} is not a known preset; the default is used"
            ));
        }
    }
    match object.get("shapeTiers") {
        Some(value) => match serde_json::from_value::<Tiers>(value.clone()) {
            Ok(tiers) => simulation.shape_tiers = tiers.reconciled(),
            Err(error) => warnings.push(format!("shapeTiers: {error}; the default tiers are used")),
        },
        None => warnings.push("shapeTiers: not in the project; the default tiers are used".into()),
    }
    unmodelled(
        "research",
        object,
        &[
            "researchLevel",
            "researchExp",
            "cachedResearchExp",
            "gridLevels",
            "shapeOverlay",
            "occFound",
            "insightLvs",
            "insightProgress",
            "magData",
            "shapePositions",
            "preset",
            "shapeTiers",
        ],
        warnings,
    );
    simulation
}

/// The `SimContext` a project carries, warning about fields the shape does not
/// reach; a missing or malformed object keeps the defaults.
fn supplements(value: Option<&Value>, warnings: &mut Vec<String>) -> SimContext {
    let Some(value) = value else {
        return SimContext::default();
    };
    let known = serde_json::to_value(SimContext::default()).ok();
    if let (Some(object), Some(known)) =
        (value.as_object(), known.as_ref().and_then(Value::as_object))
    {
        let known: Vec<&str> = known.keys().map(String::as_str).collect();
        unmodelled("supplements", object, &known, warnings);
    }
    match serde_json::from_value(value.clone()) {
        Ok(context) => context,
        Err(error) => {
            warnings.push(format!("supplements: {error}; the defaults are used"));
            SimContext::default()
        }
    }
}

/// One project document from its parsed object.
fn project(object: &Map<String, Value>) -> Result<Document, Error> {
    let version = version(object, "project")?;
    let mut warnings = Vec::new();
    let section = object
        .get("section")
        .and_then(Value::as_str)
        .map(str::to_string);
    let save = object.get("save").filter(|save| !save.is_null()).cloned();
    let has_research = object.get("research").is_some();
    let has_supplements = object.get("supplements").is_some();
    let simulation = match object.get("research") {
        Some(research) => simulation(research, &mut warnings),
        None => {
            warnings.push("research: not in the project; the simulation starts empty".into());
            Simulation::default()
        }
    };
    let context = supplements(object.get("supplements"), &mut warnings);
    let sections = match object.get("sections") {
        Some(Value::Object(sections)) => sections.clone(),
        Some(_) => {
            warnings.push("sections: not an object; no section config is imported".into());
            Map::new()
        }
        None => Map::new(),
    };
    unmodelled(
        "project",
        object,
        &[
            "format",
            "version",
            "section",
            "save",
            "research",
            "supplements",
            "sections",
        ],
        &mut warnings,
    );
    Ok(Document::Project(Box::new(Project {
        version,
        section,
        save,
        simulation,
        context,
        sections,
        warnings,
        has_research,
        has_supplements,
    })))
}

/// One section config document from its parsed object.
fn section(object: &Map<String, Value>) -> Result<Document, Error> {
    let version = version(object, "section")?;
    let id = object
        .get("section")
        .and_then(Value::as_str)
        .filter(|id| !id.trim().is_empty())
        .ok_or_else(|| Error::Section("the document names no section".into()))?;
    let config = object
        .get("config")
        .cloned()
        .ok_or_else(|| Error::Section(format!("{id} has no config object")))?;
    let mut warnings = Vec::new();
    unmodelled(
        "section config",
        object,
        &["format", "version", "section", "config"],
        &mut warnings,
    );
    Ok(Document::Section(SectionConfig {
        version,
        section: id.to_string(),
        config,
        warnings,
    }))
}

/// The version every document carries, which must be the one this build reads;
/// a document without one cannot be migrated.
fn version(object: &Map<String, Value>, kind: &str) -> Result<u32, Error> {
    let version = match object.get("version").and_then(json::number) {
        Some(version) if version >= 0.0 => version.round() as u32,
        Some(version) => return Err(Error::Version(version.to_string())),
        None => return Err(Error::Version("the document carries no version".into())),
    };
    if version != VERSION {
        return Err(Error::Version(format!(
            "{kind} version {version}; this build reads {VERSION}"
        )));
    }
    Ok(version)
}

/// Name the fields of `object` outside `known`, which this build does not
/// model, in one warning: a document can carry a dozen unknown fields, and one
/// line each would bury the fields that matter.
fn unmodelled(
    scope: &str,
    object: &Map<String, Value>,
    known: &[&str],
    warnings: &mut Vec<String>,
) {
    let fields: Vec<&str> = object
        .keys()
        .map(String::as_str)
        .filter(|key| !known.contains(key))
        .collect();
    if fields.is_empty() {
        return;
    }
    let shown: Vec<&str> = fields.iter().copied().take(5).collect();
    let rest = fields.len() - shown.len();
    warnings.push(format!(
        "{scope}: {} field(s) not modelled were skipped ({}{})",
        fields.len(),
        shown.join(", "),
        if rest > 0 { ", …" } else { "" }
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use idleon_research::model::{Mag, Placement};

    fn simulation_fixture() -> Simulation {
        let mut simulation = Simulation {
            grid: idleon_research::GridState::seeded(2),
            ..Simulation::default()
        };
        simulation.research_level = 122;
        simulation.research_exp = 4_200.0;
        simulation.observations.found[0] = true;
        simulation.observations.insight_level[0] = 12;
        simulation.observations.insight_exp[3] = 55.5;
        simulation.mags = vec![
            Mag::new(MagKind::Regular, Some(0)),
            Mag::new(MagKind::Monocle, None),
        ];
        simulation.placements = vec![Placement {
            shape: 1,
            x: 30.0,
            y: 60.0,
            rot: 90.0,
        }];
        simulation.saved_overlay = Some(vec![0; game().cells()]);
        simulation.preset = "_none".into();
        simulation.shape_tiers = Tiers::parse("J6,I5|").expect("the fixture tiers");
        simulation
    }

    fn project_fixture() -> Project {
        Project {
            version: VERSION,
            section: Some("killroy".into()),
            save: Some(json!({ "Research": "[]", "Lv0_0": "[100]" })),
            simulation: simulation_fixture(),
            context: SimContext {
                server_var_res_xp: 1.5,
                all_bonus_multi: 2.0,
                extra_pct: 30.0,
                bonus_grid_points: 8,
                extra_shapes: 2,
                extra_magnifiers: 4,
                ..SimContext::default()
            },
            sections: [("killroy".to_string(), json!({ "weeks": 20 }))]
                .into_iter()
                .collect(),
            warnings: Vec::new(),
            has_research: true,
            has_supplements: true,
        }
    }

    #[test]
    fn a_project_round_trips_through_write_and_read() {
        let project = project_fixture();
        let text = write(&project);
        let Document::Project(back) = read(&text).expect("the export reads back") else {
            panic!("an export is a project document");
        };
        assert_eq!(back.version, VERSION);
        assert_eq!(back.section.as_deref(), Some("killroy"));
        assert_eq!(back.save, project.save);
        assert_eq!(back.simulation, project.simulation);
        assert_eq!(back.context, project.context);
        assert_eq!(back.sections, project.sections);
        assert!(back.warnings.is_empty(), "{:?}", back.warnings);
    }

    #[test]
    fn the_value_is_what_write_prints() {
        let project = project_fixture();
        let printed: Value = serde_json::from_str(&write(&project)).expect("the export is JSON");
        assert_eq!(value(&project), printed);
    }

    #[test]
    fn a_project_without_research_or_supplements_says_so() {
        let text = r#"{ "format": "idleon-optimizer-project", "version": 1, "sections": {} }"#;
        let Document::Project(project) = read(text).expect("the document reads") else {
            panic!("a project document");
        };
        assert!(!project.has_research);
        assert!(!project.has_supplements);
        let Document::Project(back) = read(&write(&project_fixture())).expect("reads") else {
            panic!("a project document");
        };
        assert!(back.has_research && back.has_supplements);
    }

    #[test]
    fn every_research_field_survives_the_round_trip() {
        let start = simulation_fixture();
        let mut warnings = Vec::new();
        let back = simulation(&research_value(&start), &mut warnings);
        assert_eq!(back, start);
        assert!(warnings.is_empty(), "{warnings:?}");
    }

    #[test]
    fn a_section_config_round_trips() {
        let text = json!({
            "format": SECTION_FORMAT,
            "version": VERSION,
            "section": "sailing",
            "config": { "mode": "saved", "island": 3 },
        })
        .to_string();
        let Document::Section(config) = read(&text).expect("the config reads back") else {
            panic!("a section export is a section document");
        };
        assert_eq!(config.section, "sailing");
        assert_eq!(config.config, json!({ "mode": "saved", "island": 3 }));
        assert!(config.warnings.is_empty());
    }

    #[test]
    fn a_version_this_build_does_not_know_is_an_error() {
        let text = r#"{ "format": "idleon-optimizer-project", "version": 9 }"#;
        let error = read(text).expect_err("version 9 is not readable");
        assert!(error.to_string().contains("version 9"), "{error}");
        let missing = r#"{ "format": "idleon-optimizer-section", "section": "jar" }"#;
        assert!(read(missing).is_err());
    }

    #[test]
    fn a_section_config_without_an_id_or_config_is_an_error() {
        let no_id = r#"{ "format": "idleon-optimizer-section", "version": 1, "config": {} }"#;
        assert!(matches!(read(no_id), Err(Error::Section(_))));
        let no_config =
            r#"{ "format": "idleon-optimizer-section", "version": 1, "section": "jar" }"#;
        assert!(matches!(read(no_config), Err(Error::Section(_))));
    }

    #[test]
    fn text_that_is_not_a_document_falls_through_to_the_save_importer() {
        assert!(matches!(
            read("{\"Research\":\"[]\"}"),
            Err(Error::NotADocument)
        ));
        assert!(matches!(read("not json"), Err(Error::NotADocument)));
    }

    #[test]
    fn a_format_marker_this_build_does_not_know_names_the_marker() {
        let error = read(r#"{ "format": "some-other-tool", "version": 1 }"#)
            .expect_err("an unknown marker is not a save");
        assert!(matches!(error, Error::Format(_)), "{error:?}");
        let message = error.to_string();
        assert!(message.contains("some-other-tool"), "{message}");
        assert!(message.contains(FORMAT), "{message}");
        assert!(message.contains(SECTION_FORMAT), "{message}");
    }

    #[test]
    fn unknown_fields_warn_and_do_not_fail() {
        let text = r#"{
            "format": "idleon-optimizer-project",
            "version": 1,
            "frogs": 3,
            "sections": { "killroy": { "weeks": 12 } }
        }"#;
        let Document::Project(project) = read(text).expect("unknown fields are tolerated") else {
            panic!("still a project");
        };
        assert!(
            project
                .warnings
                .iter()
                .any(|warning| warning.contains("frogs")),
            "{:?}",
            project.warnings
        );
        assert_eq!(project.sections.len(), 1);
    }

    #[test]
    fn a_missing_research_object_starts_empty_and_says_so() {
        let text = r#"{ "format": "idleon-optimizer-project", "version": 1 }"#;
        let Document::Project(project) = read(text).expect("a bare project reads") else {
            panic!("still a project");
        };
        assert_eq!(project.simulation, Simulation::default());
        assert!(
            project
                .warnings
                .iter()
                .any(|warning| warning.contains("research"))
        );
    }
}
