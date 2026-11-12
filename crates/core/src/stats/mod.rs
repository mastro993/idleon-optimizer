//! The All Stats Dashboard: the calculation context and the category
//! breakdowns it resolves against.
//!
//! The context is ported from `js/stats/systems/common/map-access.js`, which
//! decides which maps a character may pick, and from the
//! `populateMapSel`/`switchChar` wiring in `stats-all.html`. The breakdowns
//! follow `js/stats/registry.js`, `js/stats/stat-context.js` and the
//! descriptor definitions under `js/stats/defs/`.

mod access;
pub mod breakdowns;
pub mod context;
mod data;
pub mod maps;
mod snapshot;

pub use breakdowns::{
    Breakdown, CapacityKind, Category as BreakdownCategory, Inputs as BreakdownInputs, Resolution,
    Scope, Sheet,
};
pub use context::{ConstructionMode, Context, MapVisibility, Skill};
pub use data::{
    EquipmentSet, Monster, afk_target, class_name, equipment_set, kill_requirement, map_count,
    map_name, monster, monster_count, set_bonus, target_name,
};
pub use maps::{
    Category, ClassVisibility, MapClass, MapOption, arcane_multi, build_options, classify,
    display_name,
};
pub use snapshot::Snapshot;
