//! Static game data, extracted from the upstream JS calculators.

use serde::Deserialize;
use std::sync::OnceLock;

const RESEARCH_JSON: &str = include_str!("../data/research.json");

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
    /// The research level that unlocks the observation (`isObsUsable`).
    pub roll: f64,
    /// The roll to beat when hunting for it (`computeObsUnlockPriority`).
    #[serde(rename = "rollReq")]
    pub roll_req: f64,
}

/// A built-in shape-placement preset: its upstream `above|below` tier string.
#[derive(Debug, Deserialize)]
pub struct Preset {
    pub id: String,
    pub name: String,
    pub data: String,
}

impl Preset {
    /// The preset's priority tiers, parsed from its upstream `data` string.
    pub fn tiers(&self) -> Option<crate::tiers::Tiers> {
        crate::tiers::Tiers::parse(&self.data)
    }

    /// The preset's tiers against the loaded grid: the two upstream presets
    /// whose `build` only re-ranks a fixed node pool do so here, and every
    /// other preset keeps its static string. `Minehead`'s ranking reads the
    /// save, so the app builds it from `idleon-minehead` before calling this;
    /// `Lumi/hr` needs tables this build does not model and keeps the static
    /// order.
    pub fn tiers_for(&self, simulation: &crate::model::Simulation) -> Option<crate::tiers::Tiers> {
        let pool: &[usize] = match self.id.as_str() {
            "_classexp" => &[130, 131, 132, 152],
            "_insight" => &[92, 91],
            _ => return self.tiers(),
        };
        if !simulation.grid.levels().iter().any(|level| *level > 0) {
            return self.tiers();
        }
        let mut above: Vec<usize> = pool
            .iter()
            .copied()
            .filter(|cell| simulation.grid.level(*cell) > 0)
            .collect();
        above.sort_by(|left, right| {
            let value = |cell: usize| {
                game()
                    .node_at(cell)
                    .map(|node| node.bonus_per_lv * f64::from(simulation.grid.level(cell)))
                    .unwrap_or(0.0)
            };
            value(*right).total_cmp(&value(*left))
        });
        Some(crate::tiers::Tiers::from_above(above))
    }
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
    fn the_pool_presets_re_rank_against_the_grid() {
        use crate::model::Simulation;
        let mut simulation = Simulation::default();
        let preset = game()
            .presets
            .iter()
            .find(|preset| preset.id == "_insight")
            .expect("the Insight preset");
        // With no levels the static string is the fallback.
        assert_eq!(preset.tiers_for(&simulation), preset.tiers());
        simulation.grid.set(91, 4);
        let tiers = preset.tiers_for(&simulation).expect("the tiers");
        assert_eq!(tiers.above, vec![91], "only the live pool node is ranked");
        simulation.grid.set(92, 1);
        let tiers = preset.tiers_for(&simulation).expect("the tiers");
        assert_eq!(
            tiers.above,
            vec![92, 91],
            "the pool's bonus per level times its level orders the list"
        );
    }

    #[test]
    fn presets_resolve_to_tier_lists() {
        let game = game();
        let minehead = game
            .presets
            .iter()
            .find(|preset| preset.id == "_minehead")
            .expect("minehead preset");
        let tiers = minehead.tiers().expect("minehead tiers");
        assert!(tiers.above.len() >= 5);
        for target in &tiers.above {
            assert!(
                game.node_at(*target).is_some(),
                "target {target} is not a node"
            );
        }
    }
}
