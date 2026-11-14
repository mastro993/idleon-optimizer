//! The combat simulator's core: the loadout model its editor edits.
//!
//! Ported from `js/stats/systems/common/combat-sim-setup.js` and the equipment
//! filters in `js/ui/combat-sim-setup-ui.js`. CS-01 covers the loadout only: the
//! character, its equipment and talents, the pickers' item catalog and the
//! presets the project JSON keeps. The mechanics overrides (CS-02) are the
//! second half: the curated [`overrides`] catalog and the rules that turn it into
//! simulator input overrides. The encounter run (CS-03) replays the game's
//! spawn, respawn and kill loop over one map's roster in [`encounter`], and the
//! comparison (CS-04) runs that loop for character, loadout and map setups in
//! [`compare`]. The replay codec (CS-07) records that loop's frames in
//! [`replay`], and a Killroy handoff re-opens a practice run with the same map,
//! duration and seed.

pub mod compare;
pub mod data;
pub mod den;
pub mod drops;
pub mod encounter;
pub mod loadout;
pub mod overrides;
pub mod preset;
pub mod progress;
pub mod replay;

pub use data::{CombatData, ItemDef, SLOT_COUNT, item, item_name, options, slot_name};
pub use loadout::{Character, Issue, IssueKind, IssueTarget, Loadout, Slot, Talent, Totals};
pub use overrides::{
    Entries, Kind, Mode, Overrides, Rule, SavedValue, State, Toggle, Value, groups, toggle,
};
pub use preset::{Preset, PresetError};
