//! Static game data, extracted from the upstream JS calculators.

use serde::Deserialize;
use std::sync::OnceLock;

const RESEARCH_JSON: &str = include_str!("../data/research.json");
const CLAMWORKS_JSON: &str = include_str!("../data/clamworks.json");

/// Everything the optimizers read from the game.
#[derive(Debug, Deserialize)]
pub struct GameData {
    pub grid: GridSize,
    pub nodes: Vec<GridNode>,
    pub shapes: Vec<ShapeDef>,
    pub observations: Vec<Observation>,
    pub presets: Vec<Preset>,
}

/// Grid dimensions in cells.
#[derive(Debug, Deserialize)]
pub struct GridSize {
    pub cols: usize,
    pub rows: usize,
}

/// One upgradeable square on the research grid.
#[derive(Debug, Deserialize)]
pub struct GridNode {
    pub id: usize,
    pub name: String,
    #[serde(rename = "maxLv")]
    pub max_lv: u32,
    #[serde(rename = "bonusPerLv")]
    pub bonus_per_lv: f64,
    pub desc: String,
    pub goal: Option<String>,
}

/// A research shape: a polygon that multiplies the bonuses it touches.
#[derive(Debug, Deserialize)]
pub struct ShapeDef {
    pub id: usize,
    pub name: String,
    #[serde(rename = "bonusPct")]
    pub bonus_pct: f64,
    pub vertices: Vec<[f64; 2]>,
    pub dims: [f64; 2],
    pub color: String,
}

/// An observation slot a magnifier can be placed on.
#[derive(Debug, Deserialize)]
pub struct Observation {
    pub id: usize,
    pub name: String,
    pub roll: f64,
    #[serde(rename = "rollReq")]
    pub roll_req: f64,
}

/// A built-in shape-placement preset and the grid nodes it prioritises.
#[derive(Debug, Deserialize)]
pub struct Preset {
    pub id: String,
    pub name: String,
    pub data: String,
    pub targets: Vec<usize>,
}

impl GameData {
    /// Grid nodes indexed by cell, with empty cells left as `None`.
    pub fn node_at(&self, cell: usize) -> Option<&GridNode> {
        self.nodes.iter().find(|node| node.id == cell)
    }

    /// Total cells on the grid.
    pub fn cells(&self) -> usize {
        self.grid.cols * self.grid.rows
    }
}

/// The game data, parsed once.
pub fn game() -> &'static GameData {
    static GAME: OnceLock<GameData> = OnceLock::new();
    GAME.get_or_init(|| {
        serde_json::from_str(RESEARCH_JSON).expect("bundled research data is valid JSON")
    })
}

/// The Clamworks game tables, extracted from the upstream `Spelunky` list.
#[derive(Debug, Deserialize)]
pub struct ClamworksData {
    /// The nine pearl upgrades, in the game's order.
    pub upgrades: Vec<ClamworksUpgrade>,
    /// The nine Scurge upgrades, in the game's order.
    pub scurge: Vec<ClamworksScurge>,
    /// The worker-class reward descriptions, one per promotion level.
    pub promotion: Vec<String>,
}

/// One pearl upgrade's name, description and cost curve.
#[derive(Debug, Deserialize)]
pub struct ClamworksUpgrade {
    pub name: String,
    pub desc: String,
    #[serde(rename = "perLv")]
    pub per_lv: f64,
    #[serde(rename = "costBase")]
    pub cost_base: f64,
}

/// One Scurge upgrade's name, description, per-level value and unlock level.
#[derive(Debug, Deserialize)]
pub struct ClamworksScurge {
    pub name: String,
    pub desc: String,
    #[serde(rename = "perLv")]
    pub per_lv: f64,
    #[serde(rename = "unlockLevel")]
    pub unlock_level: f64,
}

/// The Clamworks tables, parsed once.
pub fn clamworks() -> &'static ClamworksData {
    static CLAMWORKS: OnceLock<ClamworksData> = OnceLock::new();
    CLAMWORKS.get_or_init(|| {
        serde_json::from_str(CLAMWORKS_JSON).expect("bundled Clamworks data is valid JSON")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_parses_and_covers_the_grid() {
        let game = game();
        assert_eq!(game.cells(), 240);
        assert!(!game.nodes.is_empty());
        assert_eq!(game.shapes.len(), 10);
        assert_eq!(game.observations.len(), 43);
        assert!(game.presets.iter().any(|preset| preset.id == "_minehead"));
        for node in &game.nodes {
            assert!(node.id < game.cells(), "node {} off grid", node.id);
            assert!(node.max_lv >= 1);
        }
        for shape in &game.shapes {
            assert!(shape.vertices.len() >= 3);
            assert!(shape.bonus_pct > 0.0);
        }
    }

    #[test]
    fn preset_targets_resolve_to_nodes() {
        let game = game();
        let minehead = game
            .presets
            .iter()
            .find(|preset| preset.id == "_minehead")
            .expect("minehead preset");
        assert!(minehead.targets.len() >= 5);
        for target in &minehead.targets {
            assert!(
                game.node_at(*target).is_some(),
                "target {target} is not a node"
            );
        }
    }

    #[test]
    fn clamworks_data_covers_every_upgrade_and_reward() {
        let clamworks = clamworks();
        assert_eq!(clamworks.upgrades.len(), 9);
        assert_eq!(clamworks.scurge.len(), 9);
        assert_eq!(clamworks.promotion.len(), 12);
        assert_eq!(clamworks.upgrades[0].name, "PEARL VALUE");
        assert_eq!(clamworks.upgrades[0].cost_base, 1.2);
        assert_eq!(clamworks.upgrades[3].per_lv, 1.0);
        assert_eq!(clamworks.scurge[0].name, "RECKONING");
        assert_eq!(clamworks.scurge[8].unlock_level, 1000.0);
        for upgrade in &clamworks.upgrades {
            assert!(!upgrade.name.is_empty());
            assert!(upgrade.per_lv > 0.0);
            assert!(upgrade.cost_base > 1.0);
        }
        assert!(clamworks.promotion.iter().all(|reward| !reward.is_empty()));
    }
}
