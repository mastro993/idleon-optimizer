//! The All Stats Dashboard: the calculation context, the category breakdowns
//! it resolves against, the formula explorer over the same descriptors and the
//! game-input sandbox whose edits those rows resolve hypothetically.
//!
//! The context decides which maps a character may pick and follows the
//! character and map selectors.

mod access;
pub mod breakdowns;
pub mod combat;
pub mod context;
pub mod edits;
pub mod explorer;
use idleon_stats_data as data;
pub mod maps;
mod snapshot;

pub use breakdowns::{
    Breakdown, CapacityKind, Category as BreakdownCategory, Inputs as BreakdownInputs, Resolution,
    Scope, Sheet,
};
pub use context::{ConstructionMode, Context, MapVisibility, Skill};
pub use data::{
    EquipmentSet, Monster, afk_target, class_label, class_name, equipment_set, kill_requirement,
    map_count, map_name, monster, monster_count, set_bonus, target_name,
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
