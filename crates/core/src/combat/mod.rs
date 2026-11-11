//! The combat simulator's core: the loadout model its editor edits.
//!
//! Ported from `js/stats/systems/common/combat-sim-setup.js` and the equipment
//! filters in `js/ui/combat-sim-setup-ui.js`. CS-01 covers the loadout only: the
//! character, its equipment and talents, the pickers' item catalog and the
//! presets the project JSON keeps. Encounter simulation and mechanics overrides
//! arrive with CS-02 and CS-03.

pub mod data;
pub mod loadout;
pub mod preset;

pub use data::{CombatData, ItemDef, SLOT_COUNT, item, item_name, options, slot_name};
pub use loadout::{Character, Issue, IssueKind, IssueTarget, Loadout, Slot, Talent, Totals};
pub use preset::{Preset, PresetError};
