//! Static Jelly Operator data, extracted from the upstream JS calculator.
//!
//! `script/extract-jelly-data.mjs` writes `data/jelly.json` from
//! `js/stats/data/w7/jelly-operator.js`, which reads its rows out of the game's
//! `customlists.js`: the cell table, the footprints (`Research[49]`), the board
//! plots (`Research[50]`), the projectile visuals (`Research[51]`), the upgrade
//! table (`JellyUPG`), the upgrade order (`Research[44]`) and the obstruction
//! rewards (`Research[45..47]`).

use serde::Deserialize;
use std::sync::OnceLock;

const JELLY_JSON: &str = include_str!("../data/jelly.json");

/// The tables the Jelly Operator board, metrics and estimate read.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Data {
    pub cols: usize,
    pub rows: usize,
    pub obstruction_slots: Vec<usize>,
    pub cells: Cells,
    pub plots: Vec<Plot>,
    pub upgrades: Vec<Upgrade>,
    pub upgrade_order: Vec<usize>,
    pub rewards: Vec<Reward>,
    /// `AtomInfo[15][4]`: the Bloodcell gain Sulfur adds a level.
    pub atom_jelly_per_level: f64,
}

/// The cell types, in the game's own order.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Cells {
    pub names: Vec<String>,
    pub base_cooldowns: Vec<f64>,
    pub base_damage: Vec<f64>,
    /// `cellFootprint(type)`: the slot offsets from the anchor.
    pub footprints: Vec<Vec<i32>>,
    pub projectiles: Vec<Projectile>,
}

/// `jellyProjectileData(type)`: the sprite offset and the launch spread.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Projectile {
    pub offset_x: f64,
    pub offset_y: f64,
    pub spread: f64,
}

/// `jellySlotPlot(plotId)`: the slot, width and height a plot token unlocks.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Plot {
    pub plot_id: usize,
    pub start: usize,
    pub width: usize,
    pub height: usize,
}

/// One `JellyUPG` row.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Upgrade {
    pub id: usize,
    pub name: String,
    pub max_level: f64,
    pub cost_scale: f64,
    pub per_level: f64,
    pub base_cost: f64,
    pub description: String,
}

/// One `Research[45..47]` obstruction reward.
#[derive(Debug, Deserialize)]
pub struct Reward {
    pub name: String,
    /// The account system the reward feeds (`jellyRewardTarget`).
    pub target: String,
    pub description: String,
    pub value: f64,
}

/// The parsed Jelly Operator data, extracted once.
pub fn data() -> &'static Data {
    static DATA: OnceLock<Data> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(JELLY_JSON).expect("crates/jelly/data/jelly.json is extracted")
    })
}

impl Data {
    /// The board slot count (`18 × 10`).
    pub fn size(&self) -> usize {
        self.cols * self.rows
    }

    /// The cell footprint for one type, in slot offsets.
    pub fn footprint(&self, cell_type: usize) -> &[i32] {
        self.cells
            .footprints
            .get(cell_type)
            .map(Vec::as_slice)
            .unwrap_or(&[0])
    }

    /// The projectile visual of one type.
    pub fn projectile(&self, cell_type: usize) -> Projectile {
        self.cells
            .projectiles
            .get(cell_type)
            .map(|projectile| Projectile {
                offset_x: projectile.offset_x,
                offset_y: projectile.offset_y,
                spread: projectile.spread,
            })
            .unwrap_or(Projectile {
                offset_x: 0.0,
                offset_y: 0.0,
                spread: 0.0,
            })
    }

    /// The cell type's display name.
    pub fn cell_name(&self, cell_type: usize) -> &str {
        self.cells
            .names
            .get(cell_type)
            .map(String::as_str)
            .unwrap_or("Cell")
    }

    /// The plot token `Research[18]` refers to by id.
    pub fn plot(&self, plot_id: usize) -> Option<&Plot> {
        self.plots.iter().find(|plot| plot.plot_id == plot_id)
    }

    /// One `JellyUPG` row. The extraction keeps the table's own short list,
    /// while the purchase order names 100 slots the game has not filled yet.
    pub fn upgrade(&self, id: usize) -> Option<&Upgrade> {
        self.upgrades.iter().find(|upgrade| upgrade.id == id)
    }

    /// One obstruction reward.
    pub fn reward(&self, obstruction: usize) -> Option<&Reward> {
        self.rewards.get(obstruction)
    }

    /// Whether a slot is one of the sixteen board obstructions.
    pub fn is_obstruction(&self, slot: usize) -> bool {
        self.obstruction_slots.contains(&slot)
    }
}
