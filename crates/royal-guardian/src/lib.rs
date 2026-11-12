//! The Royal Guardian (W7): the outpost overview, the outpost map, the resource
//! nodes, the currencies and the range targets.
//!
//! Ported from `royal-guardian-calc.html`,
//! `js/stats/systems/w7/royal-guardian.js`,
//! `js/stats/systems/w7/royal-guardian-optimizer.js` and
//! `js/stats/systems/w7/royal-collect.js`. The upstream reads a decoded save
//! through `royalGData` and `royalMapsData`; here the same rows come from the
//! save's `RoyalG` and `RoyalMaps` fields, so every accessor below is a direct
//! transcription of one upstream reader.
//!
//! RG-01 covers the overview: the counters, the world progress cards, the raw
//! clearing values every outpost reports and the range target behind an
//! outpost's Advanced Logistics plan. RG-02 adds the outpost map, the resource
//! nodes, the currencies, the drain staffing and the banked hours; the reset
//! projection travels with the drain because the upstream computes both in one
//! pass. The upgrade list, the breakdown tree and the forecast tab are RG-03;
//! the optimizer, unit EXP and shelf steps are RG-04.
//!
//! The clearing rate multiplies the Royal Guardian talent 231 (Xtra Clear Killz)
//! by the Orblet, Spelunk big-fish and Jelly Operator factors; the Jelly one is
//! the shared [`idleon_jelly::Jelly`] port's completion bonus. The collection
//! rate reads the same talent machinery for 225, 226 and 230 plus the Orblet,
//! sushi, arcade and Armory factors; the W14 alchemy bubble and the companion
//! roster it also multiplies in are not decoded, so
//! [`rates::CollectionInputs`] names them instead of guessing.
//!
//! A talent reads its purchased level; the all-talent bonus levels that raise it
//! are not decoded yet, so a save whose Royal Guardian carries them reads a
//! lower rate than the game shows. [`RoyalGuardian::missing_sources`] names the
//! sources the save itself does not carry.

mod clearing;
mod collect;
mod connections;
mod counters;
mod data;
mod outposts;
mod ranges;
mod rates;
mod resources;

#[cfg(test)]
mod tests;

use idleon_save::json;
use serde_json::{Map, Value};
use std::collections::BTreeMap;

pub use collect::{
    Allocation, Banked, CurrencyIncome, NodeAllocation, ResetTiming, ResourceStream,
};
pub use connections::{
    Connection, Endpoint, ReachableOutpost, ReachableResource, SupportLink, SupportTarget,
};
pub use rates::{CollectionInputs, Factor, Rate, TotalStats};
pub use resources::{NodeCycle, ResourceNode, WindowMetrics, resource_position};

/// How many worlds the game ships outposts for.
pub const WORLDS: usize = 7;

/// How many maps each world page lists.
const MAPS_PER_WORLD: usize = 50;

/// How many resource nodes each world page lists.
const RESOURCES_PER_WORLD: usize = 20;

/// The class id the game gives a Royal Guardian character.
const ROYAL_GUARDIAN_CLASS: f64 = 16.0;

/// The talent whose level scales the militia clearing rate.
const XTRA_CLEAR_KILLZ: usize = 231;

/// How much reach a resource link needs beyond the outpost's own range.
const RESOURCE_ALLOWANCE: f64 = 15.0;

/// A rank no real save reaches; the rank walk stops here instead of looping.
const MAX_RANK: f64 = 1e6;

/// A logistics level no plan needs; the search stops here instead of looping.
const MAX_LOGISTICS: f64 = 1e9;

/// Cells of a `RoyalMaps` row, by their upstream index.
mod cell {
    /// The Advanced Logistics level, which is the range's one scalable term.
    pub const LOGISTICS: usize = 1;
    /// The first of the five rank EXP bars.
    pub const RANK_EXP: usize = 3;
    /// The first of the two connection endpoints.
    pub const LINKS: usize = 8;
    /// The outpost type: 0 Normal, 1 Support, 2 Savage.
    pub const KIND: usize = 10;
    /// The packed unit slots, one digit per slot.
    pub const UNITS: usize = 11;
    /// Whether the outpost is glorified.
    pub const GLORIFIED: usize = 12;
}

/// `RoyalG` rows the section reads, by their upstream index.
mod row {
    /// The armory levels.
    pub const ARMORY: usize = 2;
    /// The currency balances, indexed by currency slot.
    pub const CURRENCIES: usize = 1;
    /// The resource node progress: one cell per node, `-1` once drained.
    pub const RESOURCES: usize = 4;
    /// The resource node grades: one cell per node.
    pub const GRADES: usize = 5;
    /// The seconds of Royal Guardian time the account banks.
    pub const BANKED: usize = 3;
    /// The Orblet market levels.
    pub const ORBLET: usize = 23;
    /// The first world's militia assignment types; the map row follows.
    pub const MILITIA: usize = 6;
}

/// One outpost's map-page row: upstream `outpostEligibility`.
#[derive(Clone, Debug, PartialEq)]
pub struct Outpost {
    pub map: usize,
    pub world: usize,
    /// The map's upstream display name.
    pub name: &'static str,
    /// Whether the map carries an outpost at all.
    pub eligible: bool,
    pub built: bool,
    /// Why the outpost is out of reach, when it is.
    pub reason: Option<&'static str>,
}

/// The state the clearing metrics report: upstream `outpostClearMetrics.state`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClearState {
    /// Built, so no militia is clearing it.
    Built,
    /// The row's shape does not accrue kills.
    NonAccruing,
    /// Enough progress to build; waiting on the build.
    Secured,
    /// Open to attack but no militia is assigned.
    WaitingMilitia,
    /// Militia is accruing kills.
    Clearing,
    /// The save carries no Royal Guardian data.
    Unavailable,
}

impl ClearState {
    /// The upstream state name, for the oracle comparison and the table.
    pub fn id(self) -> &'static str {
        match self {
            ClearState::Built => "built",
            ClearState::NonAccruing => "non-accruing",
            ClearState::Secured => "secured",
            ClearState::WaitingMilitia => "waiting-militia",
            ClearState::Clearing => "clearing",
            ClearState::Unavailable => "unavailable",
        }
    }

    /// The word the state badge prints.
    pub fn label(self) -> &'static str {
        match self {
            ClearState::Built => "Built",
            ClearState::NonAccruing => "Not accruing",
            ClearState::Secured => "Secured",
            ClearState::WaitingMilitia => "Waiting for militia",
            ClearState::Clearing => "Militia clearing",
            ClearState::Unavailable => "Unavailable",
        }
    }
}

/// One outpost's raw clearing values: upstream `outpostClearMetrics`.
#[derive(Clone, Debug, PartialEq)]
pub struct Clear {
    pub map: usize,
    pub state: ClearState,
    /// Whether militia is accruing kills right now.
    pub active: bool,
    pub progress: f64,
    pub requirement: f64,
    pub remaining: f64,
    /// Militia clearing per hour.
    pub rate_per_hour: f64,
    /// Militia assigned to this outpost.
    pub assignments: f64,
    /// Hours until the outpost is secured, when it is clearing.
    pub eta_hours: Option<f64>,
    /// Why the outpost is not accruing, when it is not.
    pub reason: Option<String>,
}

/// The Advanced Logistics plan for a range target: upstream
/// `logisticsUpgradesToReach`.
#[derive(Clone, Debug, PartialEq)]
pub struct Plan {
    pub possible: bool,
    pub current_level: f64,
    pub current_range: f64,
    /// The reach the node needs from the outpost's own range.
    pub required_range: f64,
    /// The level that reaches it, when one exists.
    pub target_level: Option<f64>,
    /// How many Advanced Logistics levels the plan buys.
    pub levels_needed: Option<f64>,
    /// Why the node is out of reach, when it is.
    pub reason: Option<&'static str>,
}

impl Plan {
    /// What the plan spends: Advanced Logistics costs two points per level.
    pub fn points(&self) -> f64 {
        self.levels_needed.unwrap_or(0.0) * 2.0
    }
}

/// The nearest resource node an outpost cannot reach yet: upstream
/// `nextResourceRangeTarget`.
#[derive(Clone, Debug, PartialEq)]
pub struct RangeTarget {
    pub resource: usize,
    pub distance: f64,
    /// The outpost's own range, before the resource allowance.
    pub range: f64,
    /// The distance the outpost reaches for a resource link.
    pub reach: f64,
    pub plan: Plan,
}

/// One militia assignment: upstream `militiaAssignments` entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Assignment {
    pub world: usize,
    pub assignment: usize,
    /// 4 is militia, 5 to 7 are the command-rank units.
    pub kind: i64,
    pub map: i64,
}

/// The overview's counters, all read straight from the save.
#[derive(Clone, Debug, PartialEq)]
pub struct Counters {
    pub built_outposts: usize,
    /// How many maps carry an outpost across every world.
    pub outposts: usize,
    pub worlds_unlocked: usize,
    /// Every Armory level the save holds, summed.
    pub armory_levels: f64,
    /// Every unit stationed in a built outpost, permanent units included.
    pub units: f64,
    /// Built outposts the save has purified.
    pub purified: usize,
    pub support: usize,
    pub savage: usize,
    /// Outposts militia is clearing right now.
    pub militia_clearing: usize,
    /// Kills per hour across every clearing outpost.
    pub militia_clears_per_hour: f64,
}

/// The Royal Guardian rows one save carries.
#[derive(Clone, Debug)]
pub struct RoyalGuardian {
    /// `RoyalG`: the account rows the armory, orblets and militia live in.
    g: Vec<Value>,
    /// `RoyalMaps`: one row per map, empty until the outpost is built.
    maps: Vec<Value>,
    /// `Spelunk`: the big-fish levels the clearing rate reads.
    spelunk: Vec<Value>,
    /// `Sushi`: the tiers the unique-sushi count reads.
    sushi: Vec<Value>,
    /// `ArcadeUpg`: the arcade levels the collection rate reads.
    arcade: Vec<Value>,
    /// `Lv0_<n>`: one row per character, whose first cell is its level.
    levels: Vec<Value>,
    /// `TimeAway.ShopRestock`: the seconds left before the daily reset.
    reset_seconds: Option<f64>,
    /// The Jelly Operator state its completion bonus reads.
    jelly: Option<idleon_jelly::Jelly>,
    /// The purchased level of each talent the section reads, by talent id.
    talents: BTreeMap<usize, f64>,
    /// The index of the save's Royal Guardian character, when it carries one.
    character: Option<usize>,
    /// The bonus sources the save does not carry.
    missing: Vec<String>,
}

impl RoyalGuardian {
    /// Read the Royal Guardian rows out of a decoded save, or `None` when it
    /// carries neither `RoyalG` nor `RoyalMaps`.
    pub fn from_save(
        save: &Map<String, Value>,
        jelly: Option<&idleon_jelly::Jelly>,
    ) -> Option<Self> {
        let g = into_array(json::parsed_field(save, "RoyalG")?)?;
        let maps = into_array(json::parsed_field(save, "RoyalMaps")?)?;
        let levels = levels(save);
        let (character, talents) = talent_read(save, &levels);
        let mut royal = Self {
            g,
            maps,
            spelunk: cells(save, "Spelunk"),
            sushi: cells(save, "Sushi"),
            arcade: cells(save, "ArcadeUpg"),
            levels,
            reset_seconds: reset_seconds(save),
            jelly: jelly.cloned(),
            talents,
            character,
            missing: Vec::new(),
        };
        royal.missing = royal.find_missing();
        Some(royal)
    }

    /// The bonus sources the save does not carry.
    pub fn missing_sources(&self) -> &[String] {
        &self.missing
    }

    fn find_missing(&self) -> Vec<String> {
        let mut missing = Vec::new();
        if self.spelunk.is_empty() {
            missing.push("Spelunk big-fish levels (Spelunk[11])".to_owned());
        }
        if !self.jelly_available() {
            missing.push("Jelly Operator obstructions (Research[7][9])".to_owned());
        }
        if self.character.is_none() {
            missing.push("Royal Guardian character (CharacterClass_<n> 16)".to_owned());
        }
        missing
    }

    // ----- raw readers -----

    /// One cell of a `RoyalG` row.
    fn g_cell(&self, row: usize, cell: usize) -> f64 {
        nested_cell(&self.g, row, cell)
    }

    /// One `RoyalMaps` row; a row the save does not carry reads as empty.
    fn map_row(&self, map: usize) -> &[Value] {
        self.maps
            .get(map)
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    /// One cell of a `RoyalMaps` row.
    fn map_cell(&self, map: usize, cell: usize) -> f64 {
        self.map_saved(map, cell).unwrap_or(0.0)
    }

    /// One cell of a `RoyalMaps` row, or `None` when the row is too short.
    fn map_saved(&self, map: usize, cell: usize) -> Option<f64> {
        self.map_row(map).get(cell).and_then(json::number)
    }

    /// A cell of `Spelunk`.
    fn spelunk_cell(&self, row: usize, cell: usize) -> f64 {
        nested_cell(&self.spelunk, row, cell)
    }

    /// Whether the save carries a Royal Guardian character.
    pub fn has_character(&self) -> bool {
        self.character.is_some()
    }

    /// The index of the save's Royal Guardian character.
    pub fn character(&self) -> Option<usize> {
        self.character
    }

    /// The character's level, when the save carries the row.
    pub fn character_level(&self) -> f64 {
        self.character
            .map(|index| nested_cell(&self.levels, index, 0))
            .unwrap_or(0.0)
    }

    // ----- armory and orblets -----

    /// An Armory upgrade's saved level.
    pub fn armory_level(&self, index: usize) -> f64 {
        self.g_cell(row::ARMORY, index)
    }

    /// An Armory upgrade's effect: its level times the row's bonus.
    pub fn armory_bonus(&self, index: usize) -> f64 {
        self.armory_level(index) * bonus_per_level(&data::data().armory_bonus_per_level, index)
    }

    /// Every Armory level the save holds, summed.
    pub fn armory_levels(&self) -> f64 {
        self.g
            .get(row::ARMORY)
            .and_then(Value::as_array)
            .map(|row| row.iter().filter_map(json::number).sum())
            .unwrap_or(0.0)
    }

    /// An Orblet market upgrade's saved level.
    pub fn orblet_level(&self, index: usize) -> f64 {
        self.g_cell(row::ORBLET, index)
    }

    /// An Orblet market upgrade's effect, floored the way upstream floors it.
    pub fn orblet_bonus(&self, index: usize) -> f64 {
        (self.orblet_level(index) * bonus_per_level(&data::data().orblet_bonus_per_level, index))
            .floor()
    }

    // ----- world progress -----

    /// Which world a map belongs to.
    pub fn world_of(map: usize) -> usize {
        map / MAPS_PER_WORLD
    }

    /// Which world a resource node belongs to.
    pub fn resource_world(resource: usize) -> usize {
        resource / RESOURCES_PER_WORLD
    }

    /// Whether the armory has opened a world: upstream `royalWorldUnlocked`.
    pub fn world_unlocked(&self, world: usize) -> bool {
        if world >= WORLDS {
            return false;
        }
        if world == 0 {
            return true;
        }
        // The first world is always open; each unlock upgrade adds one.
        let unlocked = 1.0
            + (2..=WORLDS)
                .map(|index| self.armory_bonus(index).clamp(0.0, 1.0))
                .sum::<f64>();
        unlocked >= world as f64 + 1.0
    }

    /// How many worlds the armory has opened.
    pub fn worlds_unlocked(&self) -> usize {
        (0..WORLDS)
            .filter(|world| self.world_unlocked(*world))
            .count()
    }
}

/// An armory or orblet row's bonus per level.
fn bonus_per_level(row: &[f64], index: usize) -> f64 {
    row.get(index).copied().unwrap_or(0.0)
}

/// A resource node's currency slot: -1 when the node pays no currency.
pub fn resource_currency(resource: usize) -> i64 {
    data::resource(resource)
        .map(|row| row.currency_slot)
        .unwrap_or(-1)
}

/// How many resource nodes the game ships.
pub fn resource_count() -> usize {
    data::resource_count()
}

/// Every currency slot a resource node pays, in slot order: upstream
/// `validCurrencySlots`.
pub fn currency_slots() -> Vec<i64> {
    let mut slots: Vec<i64> = (0..data::resource_count())
        .map(resource_currency)
        .filter(|slot| *slot >= 0)
        .collect();
    slots.sort_unstable();
    slots.dedup();
    slots
}

/// The save's Royal Guardian character - the highest-level one when several
/// carry the class, the way upstream picks it - and each read talent's highest
/// purchased level.
fn talent_read(
    save: &Map<String, Value>,
    levels: &[Value],
) -> (Option<usize>, BTreeMap<usize, f64>) {
    let mut classes = Vec::new();
    let mut index = 0;
    while let Some(value) = json::parsed_field(save, &format!("CharacterClass_{index}")) {
        classes.push(value);
        index += 1;
    }
    if classes.is_empty()
        && let Some(array) = json::parsed_field(save, "charClassData")
    {
        classes.extend(array.as_array().cloned().unwrap_or_default());
    }
    let mut character = None;
    let mut best = 0.0;
    for (index, class) in classes.iter().enumerate() {
        if json::number(class) != Some(ROYAL_GUARDIAN_CLASS) {
            continue;
        }
        let level = nested_cell(levels, index, 0);
        if character.is_none() || level > best {
            character = Some(index);
            best = level;
        }
    }
    let mut levels = BTreeMap::new();
    for index in 0..classes.len() {
        let Some(rows) = json::parsed_field(save, &format!("SL_{index}")) else {
            continue;
        };
        for talent in data::talents() {
            let Some(level) = rows
                .as_object()
                .and_then(|map| map.get(&talent.to_string()))
                .and_then(json::number)
            else {
                continue;
            };
            let slot: &mut f64 = levels.entry(*talent).or_insert(0.0);
            *slot = (*slot).max(level);
        }
    }
    (character, levels)
}

/// Every `Lv0_<n>` row, in character order.
fn levels(save: &Map<String, Value>) -> Vec<Value> {
    let mut rows = Vec::new();
    let mut index = 0;
    while let Some(value) = json::parsed_field(save, &format!("Lv0_{index}")) {
        rows.push(value);
        index += 1;
    }
    rows
}

/// A saved account row, as a flat list of cells.
fn cells(save: &Map<String, Value>, key: &str) -> Vec<Value> {
    json::parsed_field(save, key)
        .and_then(into_array)
        .unwrap_or_default()
}

/// The array a value holds, or `None` when it is not one.
fn into_array(value: Value) -> Option<Vec<Value>> {
    match value {
        Value::Array(cells) => Some(cells),
        _ => None,
    }
}

/// One number of a row-of-rows, or 0 when the row or cell is missing.
fn nested_cell(rows: &[Value], row: usize, cell: usize) -> f64 {
    rows.get(row)
        .and_then(Value::as_array)
        .and_then(|cells| cells.get(cell))
        .and_then(json::number)
        .unwrap_or(0.0)
}

/// The seconds the save has left before the daily reset.
fn reset_seconds(save: &Map<String, Value>) -> Option<f64> {
    json::parsed_field(save, "TimeAway")?
        .get("ShopRestock")
        .and_then(json::number)
}
