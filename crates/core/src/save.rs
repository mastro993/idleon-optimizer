//! Import a game save into a simulation and its multipliers.
//!
//! The upstream calculators read the game's save through Google sign-in and a
//! decode step; this app takes the decoded object as text. The shapes of the
//! fields are documented in the upstream repo's `js/save/` modules.

use crate::model::{SimContext, Simulation};

/// What an import produced.
#[derive(Clone, Debug)]
pub struct Imported {
    pub simulation: Simulation,
    pub context: SimContext,
    /// Fields the save carried that this app does not model yet.
    pub warnings: Vec<String>,
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
            ImportError::NoSaveFields(detail) => write!(formatter, "no save fields found: {detail}"),
        }
    }
}

impl std::error::Error for ImportError {}

/// Read a decoded save object (the `SaveData` JSON the game writes) into a run.
pub fn import(_save_json: &str) -> Result<Imported, ImportError> {
    Err(ImportError::NoSaveFields(
        "save import is not implemented yet".into(),
    ))
}
