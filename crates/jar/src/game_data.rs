//! The jar's fixed game tables.
//!
//! Names, colours and the per-level collectible bonus the formulas read. The
//! extracted tables live in [`mod@super::data`]; these are small enough to keep in
//! code, next to the order they share.

/// The ten jar types, in save order.
pub const TYPE_NAMES: [&str; 10] = [
    "Basic Rupie",
    "Opal",
    "Collectible",
    "Decent Rupie",
    "Enchant",
    "White Rupie",
    "Elegant Rupie",
    "Gilded",
    "Dark Rupie",
    "Master Rupie",
];

/// What breaking a jar type gives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TypeCategory {
    /// Rupies of one or more colors.
    Rupie,
    /// A chance at an opal, a collectible or an enchant.
    Chance,
    /// The gilded doubler bonus.
    Gilded,
}

impl TypeCategory {
    /// The word the slot table's badge shows.
    pub fn label(self) -> &'static str {
        match self {
            Self::Rupie => "rupie",
            Self::Chance => "chance",
            Self::Gilded => "gilded",
        }
    }
}

/// One category per jar type.
pub const TYPE_CATEGORIES: [TypeCategory; 10] = [
    TypeCategory::Rupie,
    TypeCategory::Chance,
    TypeCategory::Chance,
    TypeCategory::Rupie,
    TypeCategory::Chance,
    TypeCategory::Rupie,
    TypeCategory::Rupie,
    TypeCategory::Gilded,
    TypeCategory::Rupie,
    TypeCategory::Rupie,
];

/// The ten standard rupies, then white and dark.
pub const RUPIE_NAMES: [&str; 12] = [
    "Red", "Blue", "Green", "Purple", "Yellow", "Orange", "Cyan", "Pink", "Crimson", "Master",
    "White", "Dark",
];

/// The dot each rupie wears, in the same order as [`RUPIE_NAMES`].
pub const RUPIE_COLORS: [&str; 12] = [
    "#f44", "#4488ff", "#4caf50", "#bb86fc", "#ffe066", "#ff9800", "#4dd0e1", "#f48fb1", "#e53935",
    "#ffd700", "#eee", "#666",
];

/// The forty-collectible table lives in `crates/jar/data/jar.json`; these are
/// its page and slot counts.
/// The collectible page two unlocks with `Advanced Collection`.
pub(super) const PAGE_TWO_COLLECTIBLES: usize = 16;
/// How many collectibles the cavern carries, across both pages.
pub const COLLECTIBLES: usize = 40;

/// Cells of `Holes[9]` the ten standard rupies start at.
pub(super) const STANDARD_RUPIES: usize = 20;

/// Progress a slot makes per hour before any bonus.
pub const BASE_PRODUCTION_PER_HOUR: f64 = 36_000.0;
