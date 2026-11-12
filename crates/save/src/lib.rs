//! Raw access to a decoded Legends of Idleon save.
//!
//! The game writes some fields as JSON strings and others as one character per
//! key; [`Save`] and the [`json`] helpers read both the way the upstream loader
//! does, so the calculators never re-implement the codec.

mod character;
mod companion;
pub mod fields;
pub mod json;
mod value;

pub use character::{Character, character_count, characters};
pub use companion::{Roster, companion_entries};
pub use value::{Parts, Save, at, number, row_values};
