//! The one import pipeline: detect the format, parse it, migrate it to the
//! current schema and hand back a document nothing has to read twice.
//!
//! Ported from the upstream `js/ui/save-loader.js`, which loads a file or a
//! paste into `js/state.js`, and `js/ui/state-io.js`, whose state box reads the
//! gzip+base64 `ROv2:` string. The upstream loader takes whatever JSON it is
//! given; this pipeline decides first, in one place:
//!
//! 1. [`idleon_project::read`] recognizes a project, a section config and an
//!    upstream state string, and migrates a state string to the project schema.
//! 2. Anything else goes through [`crate::import`], which reads a raw
//!    `it.json`/`save.json` or a decoded research/farming state export.
//! 3. A project's embedded save is decoded here, not when it is applied, so a
//!    document either previews completely or fails with one message. Applying a
//!    previewed document can then never fail halfway.
//!
//! Every failure is its own variant with its own message: malformed JSON names
//! the parser detail, a format marker this build does not know names the marker
//! and the markers it does read, and an unsupported version names the version
//! and the one this build migrates.

use crate::{ImportError, Imported};
use idleon_project::{self as project, Project, SectionConfig};
use serde_json::Value;

/// A project document with its embedded save already decoded.
#[derive(Clone, Debug)]
pub struct ProjectImport {
    pub document: Project,
    /// The save the document carries, decoded during [`read`]. `None` when the
    /// document is research-only, e.g. a migrated `ROv2:` state string.
    pub save: Option<Imported>,
}

impl ProjectImport {
    /// The format the preview names: a plain project file or a migrated state.
    pub fn format(&self) -> &'static str {
        match self.document.origin {
            project::Origin::Project => "project",
            project::Origin::UpstreamState => "upstream state",
        }
    }

    /// How many characters the embedded save carries.
    pub fn characters(&self) -> Option<usize> {
        self.save.as_ref().map(|save| save.characters)
    }
}

/// What an import text turned out to be, ready to preview and apply.
#[derive(Clone, Debug)]
pub enum Document {
    Project(Box<ProjectImport>),
    Section(SectionConfig),
    Save(Box<Imported>),
}

impl Document {
    /// The detected format, as the preview names it.
    pub fn format(&self) -> &'static str {
        match self {
            Document::Project(project) => project.format(),
            Document::Section(_) => "section config",
            Document::Save(imported) => imported.format.label(),
        }
    }

    /// The warnings the preview counts and lists.
    pub fn warnings(&self) -> &[String] {
        match self {
            Document::Project(project) => &project.document.warnings,
            Document::Section(section) => &section.warnings,
            Document::Save(imported) => &imported.warnings,
        }
    }

    /// The warning list, for a caller that knows a warning the reader cannot.
    pub fn warnings_mut(&mut self) -> &mut Vec<String> {
        match self {
            Document::Project(project) => &mut project.document.warnings,
            Document::Section(section) => &mut section.warnings,
            Document::Save(imported) => &mut imported.warnings,
        }
    }

    /// The research level the document carries, when it carries one.
    pub fn research_level(&self) -> Option<u32> {
        match self {
            Document::Project(project) => Some(project.document.simulation.research_level),
            Document::Section(_) => None,
            Document::Save(imported) => Some(imported.simulation.research_level),
        }
    }

    /// How many characters the document's save carries.
    pub fn characters(&self) -> Option<usize> {
        match self {
            Document::Project(project) => project.characters(),
            Document::Section(_) => None,
            Document::Save(imported) => Some(imported.characters),
        }
    }

    /// The section a config document targets.
    pub fn section_id(&self) -> Option<&str> {
        match self {
            Document::Section(section) => Some(&section.section),
            _ => None,
        }
    }

    /// How many section configs the document carries.
    pub fn section_configs(&self) -> usize {
        match self {
            Document::Project(project) => project.document.sections.len(),
            Document::Section(_) => 1,
            Document::Save(_) => 0,
        }
    }
}

/// Why an import text could not be read.
#[derive(Clone, Debug, PartialEq)]
pub enum Error {
    /// The text is not JSON, even after decompressing a state string.
    NotJson(String),
    /// JSON, but an object carrying none of the fields a save or a document has.
    NoSaveFields(String),
    /// A project, a section config or an upstream state string this build
    /// rejects: an unknown format marker or a version it cannot migrate.
    Document(project::Error),
    /// Save-shaped text the save reader rejected.
    Save(ImportError),
    /// A project document whose embedded save cannot be read. Nothing applies.
    EmbeddedSave(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::NotJson(detail) => write!(formatter, "not valid JSON: {detail}"),
            Error::NoSaveFields(detail) => write!(formatter, "no save fields found: {detail}"),
            Error::Document(error) => write!(formatter, "{error}"),
            Error::Save(error) => write!(formatter, "{error}"),
            Error::EmbeddedSave(detail) => write!(formatter, "save: {detail}"),
        }
    }
}

impl std::error::Error for Error {}

/// Read any accepted import text into the document it carries.
pub fn read(text: &str) -> Result<Document, Error> {
    let text = text.trim();
    if text.is_empty() {
        return Err(Error::NotJson("the text is empty".into()));
    }
    match project::read(text) {
        Ok(project::Document::Project(document)) => {
            let document = *document;
            let save = match document.save.as_ref() {
                Some(save) => Some(embedded_save(save)?),
                None => None,
            };
            Ok(Document::Project(Box::new(ProjectImport {
                document,
                save,
            })))
        }
        Ok(project::Document::Section(section)) => Ok(Document::Section(section)),
        // Not one of this build's documents, so the raw save reader names the
        // fields it expected.
        Err(project::Error::NotADocument) => match crate::import(text) {
            Ok(imported) => Ok(Document::Save(Box::new(imported))),
            Err(ImportError::NotJson(detail)) => Err(Error::NotJson(detail)),
            Err(error) => Err(Error::Save(error)),
        },
        Err(error) => Err(Error::Document(error)),
    }
}

/// The save a project document embeds, decoded now so applying cannot fail.
fn embedded_save(value: &Value) -> Result<Imported, Error> {
    crate::import(&value.to_string()).map_err(|error| {
        Error::EmbeddedSave(format!(
            "{error}; the project's research state was not applied either"
        ))
    })
}
