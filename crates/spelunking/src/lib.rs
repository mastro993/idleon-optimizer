//! Spelunking (W7): expedition controls, party, elixir belt, depth estimates and
//! the run simulator.
//!
//! Ported from `spelunking-calc.html`, `js/ui/spelunking.js`,
//! `js/stats/systems/w7/spelunking.js`, `spelunk-inputs.js`,
//! `spelunk-expedition.js` (the config normaliser, the elixir effects and the
//! expedition run) and `spelunk-depth.js`. The static tables come from
//! `script/extract-spelunk-data.mjs`.
//!
//! The upstream page derives four account multipliers from its whole stat
//! context: Spelunking Power, the amber multiplier, stamina regeneration and the
//! Grand Discovery chance. This port does not decode that context yet, so
//! [`Account`] carries the values the user reads in game. Everything the save
//! itself holds — characters and saved stamina, caves, tools, elixirs, shop
//! upgrades and depth records — comes from [`Save`]. Nothing is guessed.
//!
//! Depth estimates are pure: given the same [`Inputs`] and [`Requested`]
//! configuration they produce the same [`Profile`] the upstream calculator
//! prints, which `fixtures/spelunking.json` pins against the JS. A run is
//! deterministic by seed: [`sim::Run`] replays the upstream 10 ms tick with the
//! same seeded generator, so the same inputs and seed give the same result.

pub mod config;
pub mod data;
pub mod depth;
pub mod save;
pub mod sim;

pub use config::{Config, ConfigError, Requested, normalize};
pub use data::{Cave, Elixir, Preset, PresetKind, Rock, Tool, data};
pub use depth::{DepthRange, Object, Profile, ReferenceTool, Row, profile};
pub use save::Save;
pub use sim::{
    AmberActivation, AmberPhase, AmberResult, AmberTransition, Contribution, DEFAULT_SEED,
    Diagnostics, Event, EventKind, Exit, ExitItem, ExitKind, MemberStamina, Run, RunOptions,
    RunResult, Status, simulate,
};

use std::fmt;

/// The account values the upstream stat context would supply.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Account {
    /// `Spelunking Power`, as the game's Skills panel shows it.
    pub power: f64,
    /// `SpelunkyBonus(1)` as a percentage: extra exits per depth.
    pub holes_bonus: f64,
    /// `Amber` multiplier: how much Amber each Amber drop is worth.
    pub amber: f64,
    /// Stamina regeneration per hour, for the recovery a run costs.
    pub regen: f64,
    /// The Grand Discovery chance per Grand rock, as a probability.
    pub grand: f64,
    /// Maximum stamina per character, when the save's current stamina is not it.
    /// A missing entry uses the saved stamina.
    pub max_stamina: Vec<f64>,
}

impl Account {
    /// The defaults a save without user overrides starts from. Power 1 is the
    /// neutral multiplier and the other three are the game's base values; the
    /// user reads the real ones in game.
    pub fn neutral() -> Self {
        Self {
            power: 1.0,
            holes_bonus: 0.0,
            amber: 1.0,
            regen: 5.0,
            grand: 4e-5,
            max_stamina: Vec::new(),
        }
    }
}

impl Default for Account {
    fn default() -> Self {
        Self::neutral()
    }
}

/// One character the expedition can take.
#[derive(Clone, Debug, PartialEq)]
pub struct Character {
    pub id: usize,
    pub name: String,
    /// The class index the portrait uses.
    pub class_id: u32,
    pub level: f64,
    /// Saved stamina, clamped to `max_stamina`.
    pub stamina: f64,
    pub max_stamina: f64,
}

impl Character {
    /// Whether the character may join a saved-stamina party.
    pub fn has_half_stamina(&self) -> bool {
        self.stamina >= self.max_stamina / 2.0
    }
}

/// Everything one expedition reads, decoded from the save plus [`Account`].
#[derive(Clone, Debug, PartialEq)]
pub struct Inputs {
    pub characters: Vec<Character>,
    pub caves: Vec<Cave>,
    pub tools: Vec<Tool>,
    pub elixirs: Vec<Elixir>,
    /// `shopUpgBonus(i, S)` for every shop row, so the formulas read the same array.
    pub shop: Vec<f64>,
    /// The account inputs the save cannot supply.
    pub power: f64,
    pub holes_bonus: f64,
    /// The Amber multiplier each Amber drop is worth.
    pub amber: f64,
    /// Stamina regeneration per hour, for the recovery a run costs.
    pub regen: f64,
    /// The Grand Discovery chance per Grand rock, as a probability.
    pub grand: f64,
    /// Unlocked elixir count, belt slots, duplicate limit and memory slots.
    pub elixir_owned: usize,
    pub slots: usize,
    pub dupes: usize,
    pub memory_slots: usize,
    /// The chance a used elixir charge is not consumed.
    pub nonconsume: f64,
    /// Super Bit 36: a tool may dig below its own stamina cost.
    pub overuse: bool,
    /// Shop row 38: a party switch sets off a nova.
    pub switch_nova: bool,
    /// Amber bag gem purchases, which raise the bag drop chance.
    pub bags: f64,
    /// The saved Prisma and Exalt counts, which scale their drop odds.
    pub prisma: f64,
    pub exalt: f64,
    /// Menagerie unlock 1: the saved shortcut entry is available.
    pub shortcut: bool,
    /// Saved belt and memory, and the saved selections the defaults use.
    pub saved_elixirs: Vec<i32>,
    pub memorized: Vec<usize>,
    pub saved_manic: bool,
    pub full_clear: bool,
    pub auto: bool,
    pub speed_max: u32,
    pub manic: bool,
    /// Normal and Manic depth records per cave, as the cave chips show them.
    pub depths: Vec<f64>,
    pub manic_depths: Vec<f64>,
    /// `Spelunk`: `[0]` boss unlocks, `[44]` Grand Discoveries, `[6]` discoveries.
    pub bosses: Vec<f64>,
    pub grands: Vec<f64>,
    pub discoveries: Vec<String>,
    /// Whether the account multipliers are partial.
    pub partial: bool,
}

/// The six expedition policies, in the upstream menu order.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Policy {
    /// The native Auto strategy, when it is unlocked.
    #[default]
    Auto,
    /// Manual depth diving.
    Dive,
    /// Manual stamina efficiency.
    Efficient,
    /// Manual clear every fifth depth.
    ClearFifth,
    /// Manual full clear.
    Clear,
    /// Phased manual amber farming.
    AmberFarm,
}

impl Policy {
    /// The ids upstream's `SPELUNK_POLICIES` lists, with the legacy `push-farm`
    /// alias it maps to `efficient`.
    pub const ALL: [Policy; 6] = [
        Policy::Auto,
        Policy::Dive,
        Policy::Efficient,
        Policy::ClearFifth,
        Policy::Clear,
        Policy::AmberFarm,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Policy::Auto => "auto",
            Policy::Dive => "dive",
            Policy::Efficient => "efficient",
            Policy::ClearFifth => "clear-fifth",
            Policy::Clear => "clear",
            Policy::AmberFarm => "amber-farm",
        }
    }

    /// The menu label, matching the upstream page.
    pub fn label(self) -> &'static str {
        match self {
            Policy::Auto => "Native Auto",
            Policy::Dive => "Manual depth diving",
            Policy::Efficient => "Manual stamina efficiency",
            Policy::ClearFifth => "Manual clear every 5 depths",
            Policy::Clear => "Manual full clear",
            Policy::AmberFarm => "Phased manual Amber farming",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "auto" => Some(Policy::Auto),
            "dive" => Some(Policy::Dive),
            // Upstream still accepts the retired `push-farm` id.
            "efficient" | "push-farm" => Some(Policy::Efficient),
            "clear-fifth" => Some(Policy::ClearFifth),
            "clear" => Some(Policy::Clear),
            "amber-farm" => Some(Policy::AmberFarm),
            _ => None,
        }
    }
}

/// Where the run starts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Entry {
    Surface,
    #[default]
    Shortcut,
}

impl Entry {
    pub fn id(self) -> &'static str {
        match self {
            Entry::Surface => "surface",
            Entry::Shortcut => "shortcut",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Entry::Surface => "Surface",
            Entry::Shortcut => "Saved shortcut if unlocked",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "surface" => Some(Entry::Surface),
            "shortcut" => Some(Entry::Shortcut),
            _ => None,
        }
    }
}

/// Whether the party starts at its saved stamina or fully recovered.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StaminaMode {
    #[default]
    Saved,
    Full,
}

impl StaminaMode {
    pub fn id(self) -> &'static str {
        match self {
            StaminaMode::Saved => "saved",
            StaminaMode::Full => "full",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "saved" => Some(StaminaMode::Saved),
            "full" => Some(StaminaMode::Full),
            _ => None,
        }
    }
}

/// The manual tool choice: adaptive, or one unlocked tool.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ToolChoice {
    #[default]
    Adaptive,
    Tool(usize),
}

impl ToolChoice {
    pub fn from_id(id: &str) -> Option<Self> {
        if id == "adaptive" {
            return Some(ToolChoice::Adaptive);
        }
        id.parse().ok().map(ToolChoice::Tool)
    }
}

impl serde::Serialize for ToolChoice {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            ToolChoice::Adaptive => serializer.serialize_str("adaptive"),
            ToolChoice::Tool(id) => serializer.serialize_u64(*id as u64),
        }
    }
}

impl<'de> serde::Deserialize<'de> for ToolChoice {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl serde::de::Visitor<'_> for Visitor {
            type Value = ToolChoice;
            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("`adaptive` or a tool index")
            }
            fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<ToolChoice, E> {
                if value == "adaptive" {
                    return Ok(ToolChoice::Adaptive);
                }
                value
                    .parse()
                    .map(ToolChoice::Tool)
                    .map_err(|_| E::custom("`adaptive` or a tool index"))
            }
            fn visit_u64<E: serde::de::Error>(self, value: u64) -> Result<ToolChoice, E> {
                Ok(ToolChoice::Tool(value as usize))
            }
        }
        deserializer.deserialize_any(Visitor)
    }
}

/// The amber-farming phase limits, ported from `normalizeSpelunkConfig`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AmberPolicy {
    pub dump_depth: u32,
    pub finishers: u32,
    pub reserve_pct: u32,
    pub harvest_depths: u32,
    pub rope_reserve_pct: u32,
}

impl Default for AmberPolicy {
    fn default() -> Self {
        Self {
            dump_depth: 20,
            finishers: 1,
            reserve_pct: 100,
            harvest_depths: 3,
            rope_reserve_pct: 25,
        }
    }
}

/// The divisor in the upstream `getLOG`, written exactly as the JS source has
/// it. It is not `LN_10`: the game's truncated constant is part of the formula.
#[allow(clippy::approx_constant)]
pub(crate) const LOG_DIVISOR: f64 = 2.30259;

/// The base-10 logarithm the upstream `getLOG` uses.
pub(crate) fn game_log(value: f64) -> f64 {
    value.max(1.0).ln() / LOG_DIVISOR
}

/// The game's `Math.round`: halves round up, so `-0.5` is `0`.
pub(crate) fn js_round(value: f64) -> f64 {
    (value + 0.5).floor()
}

/// The inputs `fixtures/data.json` carries under a neutral account.
#[cfg(test)]
pub(crate) fn fixture_inputs() -> Inputs {
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/data.json"
    ))
    .expect("the fixture is committed");
    let save = idleon_save::Save::new(serde_json::from_str(&text).expect("the fixture is JSON"));
    let save = Save::from_parts(&save).expect("the fixture carries Spelunk rows");
    save.inputs(&Account::neutral())
}
