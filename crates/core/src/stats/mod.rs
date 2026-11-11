//! The All Stats Dashboard's calculation context: the selectors upstream's
//! `stats-all.html` carries and the save defaults they start from.
//!
//! Ported from `js/stats/systems/common/map-access.js`, which decides which
//! maps a character may pick, and from the `populateMapSel`/`switchChar`
//! wiring in `stats-all.html`. The two-layer descriptor model of
//! `js/stats/stat-context.js` arrives with the breakdowns (SA-02); this module
//! is only the context those descriptors will be resolved against.

mod access;
pub mod context;
mod data;
pub mod maps;
mod snapshot;

pub use context::{ConstructionMode, Context, MapVisibility, Skill};
pub use data::{class_name, map_count};
pub use maps::{
    Category, ClassVisibility, MapClass, MapOption, arcane_multi, build_options, classify,
};
pub use snapshot::Snapshot;
