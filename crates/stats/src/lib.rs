//! The All Stats Dashboard: the calculation context, the category breakdowns
//! it resolves against, the formula explorer over the same descriptors and the
//! game-input sandbox whose edits those rows resolve hypothetically.
//!
//! The context is ported from `js/stats/systems/common/map-access.js`, which
//! decides which maps a character may pick, and from the
//! `populateMapSel`/`switchChar` wiring in `stats-all.html`. The breakdowns
//! follow `js/stats/registry.js`, `js/stats/stat-context.js` and the
//! descriptor definitions under `js/stats/defs/`.

mod access;
pub mod breakdowns;
pub mod context;
pub mod edits;
pub mod explorer;
use idleon_stats_data as data;
pub mod maps;
mod snapshot;

pub use breakdowns::{
    Breakdown, CapacityKind, Category as BreakdownCategory, Inputs as BreakdownInputs, Resolution,
    Scope, Sheet, UPSTREAM,
};
pub use context::{ConstructionMode, Context, MapVisibility, Skill};
pub use data::{
    EquipmentSet, Monster, afk_target, class_name, equipment_set, kill_requirement, map_count,
    map_name, monster, monster_count, set_bonus, target_name,
};
pub use edits::{Affected, GameInput, Sandbox};
pub use explorer::{
    CategoryCoverage, Coverage, Filter as ExplorerFilter, Status as ExplorerStatus,
};
pub use maps::{
    Category, ClassVisibility, MapClass, MapOption, arcane_multi, build_options, classify,
    display_name,
};
pub use snapshot::Snapshot;
