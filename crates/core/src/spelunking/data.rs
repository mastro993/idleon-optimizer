//! Static Spelunking data, extracted from the upstream JS calculators.
//!
//! `script/extract-spelunk-data.mjs` writes `data/spelunking.json` from
//! `js/stats/data/game/customlists.js`, `js/stats/data/w7/spelunking.js` and the
//! elixir names/presets `js/ui/spelunking.js` hardcodes.

use serde::Deserialize;
use std::sync::OnceLock;

const SPELUNK_JSON: &str = include_str!("../../data/spelunking.json");

/// Every table the section reads.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Data {
    pub caves: Vec<Cave>,
    pub tools: Vec<Tool>,
    pub elixirs: Vec<Elixir>,
    pub elixir_presets: Vec<Preset>,
    /// The upstream policy ids, in menu order.
    pub policies: Vec<String>,
    /// `SpelunkUpg[idx][4]`: the per-level value of each shop upgrade.
    pub upgrade_per_level: Vec<f64>,
    /// `SpelunkChapters[chapter][bonus]`: the chapter-bonus formula rows.
    pub chapters: Vec<Vec<Chapter>>,
    /// `N2L[36]`, the Super Bit token `superBitType(36, Gaming[12])` matches.
    pub overuse_super_bit: String,
    /// The twelve reward names the run reports, in reward-index order.
    pub rewards: Vec<String>,
    /// The native 1x `[width, height]` of each sprite the floor geometry reads.
    pub sprites: std::collections::BTreeMap<String, [f64; 2]>,
}

/// One chapter-bonus row: the formula `chapterBonus` evaluates.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Chapter {
    pub label: String,
    pub x1: f64,
    pub x2: f64,
    /// The game's formula kind, e.g. `decay`.
    pub kind: String,
    /// Whether the artifact bonus scales the row.
    pub artifact: f64,
}

/// One tunnel's rock table.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Cave {
    pub id: usize,
    /// The display name, with underscores already turned into spaces.
    pub name: String,
    /// How many characters the cave admits.
    pub capacity: usize,
    /// The zero-based depth the boss spawns on.
    pub boss_depth: f64,
    pub boss_offset: [f64; 2],
    /// The sprite letter the rock names use.
    pub letter: String,
    pub rocks: Vec<Rock>,
}

/// One rock type in a cave's table.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Rock {
    pub name: String,
    /// The power multiplier the requirement scales with.
    pub pow: f64,
    pub hp: f64,
    pub amber: f64,
    /// Whether HP, requirement and amber scale with depth.
    pub scaled: bool,
    /// The per-depth growth for a scaled rock.
    pub growth: f64,
    /// The depth the rock first appears on, zero-based.
    pub depth: f64,
    /// The base power the requirement multiplies; only the first rock carries it.
    pub base: Option<f64>,
}

/// One tool the player can dig with.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Tool {
    pub id: usize,
    pub name: String,
    pub radius: f64,
    pub damage: f64,
    pub cost: f64,
}

/// One elixir in the belt catalogue.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Elixir {
    pub id: usize,
    pub name: String,
    /// Charges one belt slot starts with.
    pub charges: f64,
    /// The percentage or count the description substitutes for `{`.
    pub quantity: f64,
    pub description: String,
}

/// Which half of the belt a built-in preset fills.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
pub enum PresetKind {
    /// The equipped belt; upstream calls it `elixir`.
    #[serde(rename = "elixir")]
    Belt,
    /// The memorized set.
    #[serde(rename = "memory")]
    Memory,
}

/// One built-in elixir loadout from the upstream page.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Preset {
    pub id: String,
    pub label: String,
    pub kind: PresetKind,
    pub elixirs: Vec<usize>,
}

/// The extracted tables.
pub fn data() -> &'static Data {
    static DATA: OnceLock<Data> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(SPELUNK_JSON).expect("data/spelunking.json is generated and valid")
    })
}

/// A sprite's native `[width, height]`, or `None` when the table lacks it.
pub fn sprite(name: &str) -> Option<[f64; 2]> {
    data().sprites.get(name).copied()
}
