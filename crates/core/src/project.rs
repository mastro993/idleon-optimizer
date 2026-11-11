//! The app's project file: the whole window state as one versioned JSON
//! document, plus the upstream `ROv2:` state string it can also read.
//!
//! Ported from the upstream `js/ui/state-io.js`, which writes the research
//! state as gzip+base64 behind an `ROv2:` prefix and a `_version` field. The
//! project file keeps the same research shape in plain JSON so it stays
//! diffable, and adds what the upstream state never carried: the decoded save
//! and one config object per section.
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
//! The `research` object is the shape [`crate::save::import`] already reads as
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
//! `read` never guesses: a version it does not know is an error, a format it
//! does not recognize is [`Error::NotADocument`] so the caller can try the raw
//! save importer, and fields this build does not model are named in a warning
//! instead of failing the document.

use crate::data::game;
use crate::model::{MagKind, SimContext, Simulation};
use crate::save::{json, research};
use base64::Engine as _;
use flate2::read::GzDecoder;
use serde_json::{Map, Value, json};
use std::io::Read;

/// The project file's format marker.
pub const FORMAT: &str = "idleon-optimizer-project";
/// The project file's schema version.
pub const VERSION: u32 = 1;
/// A per-section config document's format marker.
pub const SECTION_FORMAT: &str = "idleon-optimizer-section";
/// The prefix the upstream state string carries.
const STATE_PREFIX: &str = "ROv2:";

/// The upstream state's format marker.
const UPSTREAM_FORMAT: &str = "research-optimizer-state";
/// The upstream schema versions this build migrates.
const UPSTREAM_VERSIONS: [u32; 2] = [1, 2];

/// Where the document was read from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    /// A plain project file.
    Project,
    /// An upstream `ROv2:` state string, decompressed and migrated.
    UpstreamState,
}

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
    /// One config object per section id.
    pub sections: Map<String, Value>,
    /// Fields the document carried that this build could not read.
    pub warnings: Vec<String>,
    pub origin: Origin,
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
    /// The text is not JSON, even after decompressing a state string.
    NotJson(String),
    /// A state string's payload is not base64.
    Base64(String),
    /// A state string's payload is not gzip.
    Gzip(String),
    /// JSON, but neither a project, a section config nor an upstream state.
    NotADocument,
    /// A document with a format marker this build does not know.
    Format(String),
    /// A version this build cannot migrate.
    Version(String),
    /// A document whose top level is not the object it must be.
    Shape(String),
    /// A section config with no section id to apply it to.
    Section(String),
}

impl Error {
    /// Whether the text is simply not one of this module's documents, so the
    /// caller should try the raw save importer instead.
    pub fn is_not_document(&self) -> bool {
        matches!(self, Error::NotADocument)
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::NotJson(detail) => write!(formatter, "not valid JSON: {detail}"),
            Error::Base64(detail) => write!(formatter, "the state string is not base64: {detail}"),
            Error::Gzip(detail) => write!(formatter, "the state string is not gzip: {detail}"),
            Error::NotADocument => write!(
                formatter,
                "neither a project file, a section config nor an ROv2 state string"
            ),
            Error::Format(detail) => write!(formatter, "unknown document format: {detail}"),
            Error::Version(detail) => write!(formatter, "unsupported version: {detail}"),
            Error::Shape(detail) => write!(formatter, "not a document object: {detail}"),
            Error::Section(detail) => write!(formatter, "no section to apply: {detail}"),
        }
    }
}

impl std::error::Error for Error {}

/// Read a project, a section config or an upstream state string.
pub fn read(text: &str) -> Result<Document, Error> {
    let text = text.trim();
    if let Some(payload) = text.strip_prefix(STATE_PREFIX) {
        return Ok(Document::Project(Box::new(upstream_state(payload)?)));
    }
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
    match object.get("format").and_then(Value::as_str) {
        Some(FORMAT) => project(object),
        Some(SECTION_FORMAT) => section(object),
        _ => Err(Error::NotADocument),
    }
}

/// Write a project document, pretty-printed so a human can read and edit it.
pub fn write(project: &Project) -> String {
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
    serde_json::to_string_pretty(&Value::Object(object)).unwrap_or_default()
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
    let known = [
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
    ];
    let unknown: Vec<&str> = object
        .keys()
        .map(String::as_str)
        .filter(|key| !known.contains(key))
        .collect();
    unmodelled("research", &unknown, warnings);
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
        let unknown: Vec<&str> = object
            .keys()
            .map(String::as_str)
            .filter(|key| !known.contains_key(*key))
            .collect();
        unmodelled("supplements", &unknown, warnings);
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
    let version = version(object)?;
    if version != VERSION {
        return Err(Error::Version(format!(
            "project version {version}; this build reads {VERSION}"
        )));
    }
    let mut warnings = Vec::new();
    let section = object
        .get("section")
        .and_then(Value::as_str)
        .map(str::to_string);
    let save = object.get("save").filter(|save| !save.is_null()).cloned();
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
    let known = [
        "format",
        "version",
        "section",
        "save",
        "research",
        "supplements",
        "sections",
    ];
    let unknown: Vec<&str> = object
        .keys()
        .map(String::as_str)
        .filter(|key| !known.contains(key))
        .collect();
    unmodelled("project", &unknown, &mut warnings);
    Ok(Document::Project(Box::new(Project {
        version,
        section,
        save,
        simulation,
        context,
        sections,
        warnings,
        origin: Origin::Project,
    })))
}

/// One section config document from its parsed object.
fn section(object: &Map<String, Value>) -> Result<Document, Error> {
    let version = version(object)?;
    if version != VERSION {
        return Err(Error::Version(format!(
            "section version {version}; this build reads {VERSION}"
        )));
    }
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
    let known = ["format", "version", "section", "config"];
    let unknown: Vec<&str> = object
        .keys()
        .map(String::as_str)
        .filter(|key| !known.contains(key))
        .collect();
    unmodelled("section config", &unknown, &mut warnings);
    Ok(Document::Section(SectionConfig {
        version,
        section: id.to_string(),
        config,
        warnings,
    }))
}

/// The version every document carries; a document without one cannot be migrated.
fn version(object: &Map<String, Value>) -> Result<u32, Error> {
    match object.get("version").and_then(json::number) {
        Some(version) if version >= 0.0 => Ok(version.round() as u32),
        Some(version) => Err(Error::Version(version.to_string())),
        None => Err(Error::Version("the document carries no version".into())),
    }
}

/// An upstream `ROv2:` payload: base64, then gzip, then the state object.
fn upstream_state(payload: &str) -> Result<Project, Error> {
    let compact: String = payload.chars().filter(|ch| !ch.is_whitespace()).collect();
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(&compact)
        .or_else(|_| base64::engine::general_purpose::STANDARD_NO_PAD.decode(&compact))
        .map_err(|error| Error::Base64(error.to_string()))?;
    let mut text = String::new();
    GzDecoder::new(&bytes[..])
        .read_to_string(&mut text)
        .map_err(|error| Error::Gzip(error.to_string()))?;
    let parsed: Value =
        serde_json::from_str(&text).map_err(|error| Error::NotJson(error.to_string()))?;
    let object = parsed
        .as_object()
        .ok_or_else(|| Error::Shape("the state string is not an object".into()))?;
    let format = object.get("_format").and_then(Value::as_str);
    if format != Some(UPSTREAM_FORMAT) {
        return Err(Error::Format(format.unwrap_or("none").to_string()));
    }
    let version = object
        .get("_version")
        .and_then(json::number)
        .map_or(0.0, |v| v.round());
    if !UPSTREAM_VERSIONS
        .iter()
        .any(|known| f64::from(*known) == version)
    {
        return Err(Error::Version(format!(
            "upstream state {version}; this build reads {}",
            UPSTREAM_VERSIONS
                .iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join(" and ")
        )));
    }
    Ok(migrate(object, version as u32))
}

/// The upstream state's fields, mapped onto the project document.
///
/// The upstream export only carries research state, so the save has to come
/// from the import dialog; every field the app models only from the save is
/// named in a warning instead of guessed.
fn migrate(object: &Map<String, Value>, _version: u32) -> Project {
    let mut warnings = vec![
        "save: an ROv2 state carries no account save; sections that read it keep the loaded one"
            .into(),
    ];
    let mut state = Map::new();
    for key in [
        "researchLevel",
        "gridLevels",
        "shapeOverlay",
        "occFound",
        "insightLvs",
        "insightProgress",
        "magData",
        "shapePositions",
    ] {
        if let Some(value) = object.get(key) {
            state.insert(key.to_string(), value.clone());
        }
    }
    if let Some(value) = object.get("cachedResearchExp") {
        state.insert("researchExp".into(), value.clone());
    }
    let simulation = simulation(&Value::Object(state), &mut warnings);
    let mut context = SimContext::default();
    if let Some(value) = object.get("serverVarResXP").and_then(json::number) {
        context.server_var_res_xp = value;
    }
    if let Some(value) = object.get("allBonusMulti").and_then(json::number) {
        context.all_bonus_multi = value;
    }
    // The state's additive account percentage; newer exports split the sticker
    // half into `cachedExtPctExSticker`, which is the value this app models.
    if let Some(value) = object
        .get("cachedExtPctExSticker")
        .or_else(|| object.get("externalResearchPct"))
        .and_then(json::number)
    {
        context.extra_pct = value;
    }
    if let Some(value) = object.get("comp52TrueMulti").and_then(json::number) {
        context.companion_52 = value;
    }
    let mapped = [
        "_format",
        "_version",
        "researchLevel",
        "cachedResearchExp",
        "gridLevels",
        "shapeOverlay",
        "occFound",
        "insightLvs",
        "insightProgress",
        "magData",
        "shapePositions",
        "serverVarResXP",
        "allBonusMulti",
        "externalResearchPct",
        "cachedExtPctExSticker",
        "comp52TrueMulti",
    ];
    let skipped: Vec<&str> = object
        .keys()
        .map(String::as_str)
        .filter(|key| !mapped.contains(key))
        .collect();
    unmodelled("state", &skipped, &mut warnings);
    Project {
        version: VERSION,
        section: None,
        save: None,
        simulation,
        context,
        sections: Map::new(),
        warnings,
        origin: Origin::UpstreamState,
    }
}

/// Name the fields a document carried that this build does not model, in one
/// warning: a state string carries a dozen save-derived caches, and one line
/// each would bury the fields that matter.
fn unmodelled(scope: &str, fields: &[&str], warnings: &mut Vec<String>) {
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
    use crate::model::{Mag, Placement};
    use crate::save;

    fn simulation_fixture() -> Simulation {
        let mut simulation = Simulation {
            grid: crate::GridState::seeded(2),
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
            origin: Origin::Project,
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
        assert_eq!(back.origin, Origin::Project);
        assert_eq!(back.section.as_deref(), Some("killroy"));
        assert_eq!(back.save, project.save);
        assert_eq!(back.simulation, project.simulation);
        assert_eq!(back.context, project.context);
        assert_eq!(back.sections, project.sections);
        assert!(back.warnings.is_empty(), "{:?}", back.warnings);
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
    fn an_exported_project_reads_as_a_research_state_in_the_save_importer() {
        let research = research_value(&simulation_fixture());
        let imported = save::import(&research.to_string()).expect("the research half imports");
        assert_eq!(imported.simulation, simulation_fixture());
        assert_eq!(imported.format, save::Format::ResearchState);
    }

    #[test]
    fn a_section_config_round_trips() {
        use crate::project::SECTION_FORMAT;

        let text = serde_json::json!({
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
        assert!(
            read("{\"Research\":\"[]\"}")
                .expect_err("a save is not a document")
                .is_not_document()
        );
        assert!(
            read("not json")
                .expect_err("garbage is not a document")
                .is_not_document()
        );
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
    fn an_upstream_state_string_is_decompressed_and_migrated() {
        use flate2::Compression;
        use flate2::write::GzEncoder;
        use std::io::Write as _;

        let state = json!({
            "_format": "research-optimizer-state",
            "_version": 2,
            "researchLevel": 140,
            "cachedResearchExp": 900.0,
            "gridLevels": vec![1; game().cells()],
            "occFound": vec![1; game().observations.len()],
            "insightLvs": vec![7; game().observations.len()],
            "insightProgress": vec![0; game().observations.len()],
            "magData": [0, 0, 0, 0],
            "shapePositions": [],
            "serverVarResXP": 1.25,
            "allBonusMulti": 3.0,
            "cachedExtPctExSticker": 12.5,
            "comp52TrueMulti": 1.1,
            "magMaxPerSlot": 2,
            "stateR7": vec![0; 20],
        });
        let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
        encoder
            .write_all(state.to_string().as_bytes())
            .expect("gzip");
        let bytes = encoder.finish().expect("gzip");
        let text = format!(
            "{STATE_PREFIX}{}",
            base64::engine::general_purpose::STANDARD.encode(bytes)
        );

        let Document::Project(project) = read(&text).expect("the state string reads") else {
            panic!("a state string is a project document");
        };
        assert_eq!(project.origin, Origin::UpstreamState);
        assert_eq!(project.version, VERSION);
        assert_eq!(project.simulation.research_level, 140);
        assert_eq!(project.simulation.research_exp, 900.0);
        assert_eq!(project.simulation.grid.level(0), 1);
        assert_eq!(
            project.simulation.mags,
            vec![Mag::new(MagKind::Regular, Some(0))]
        );
        assert_eq!(project.context.server_var_res_xp, 1.25);
        assert_eq!(project.context.all_bonus_multi, 3.0);
        assert_eq!(project.context.extra_pct, 12.5);
        assert_eq!(project.context.companion_52, 1.1);
        assert!(project.save.is_none());
        assert!(
            project
                .warnings
                .iter()
                .any(|warning| warning.contains("magMaxPerSlot")),
            "{:?}",
            project.warnings
        );
    }

    #[test]
    fn an_upstream_state_with_an_unknown_version_or_format_is_an_error() {
        use flate2::Compression;
        use flate2::write::GzEncoder;
        use std::io::Write as _;

        let encode = |state: Value| {
            let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
            encoder
                .write_all(state.to_string().as_bytes())
                .expect("gzip");
            format!(
                "{STATE_PREFIX}{}",
                base64::engine::general_purpose::STANDARD.encode(encoder.finish().expect("gzip"))
            )
        };
        let wrong_version = encode(json!({ "_format": "research-optimizer-state", "_version": 7 }));
        assert!(matches!(read(&wrong_version), Err(Error::Version(_))));
        let wrong_format = encode(json!({ "_format": "something-else", "_version": 2 }));
        assert!(matches!(read(&wrong_format), Err(Error::Format(_))));
        assert!(matches!(read("ROv2:not-base64!"), Err(Error::Base64(_))));
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
