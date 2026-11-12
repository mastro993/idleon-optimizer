//! Static game data, extracted from the upstream Depth Charge calculator.

use serde::Deserialize;
use std::sync::OnceLock;

const MINEHEAD_JSON: &str = include_str!("../data/minehead.json");

/// Everything the Minehead tab reads from the game.
#[derive(Debug, Deserialize)]
pub struct GameData {
    pub upgrades: Vec<Upgrade>,
    #[serde(rename = "floorRewards")]
    pub floor_rewards: FloorRewards,
    #[serde(rename = "gridDims")]
    pub grid_dims: Vec<String>,
    #[serde(rename = "bossNames")]
    pub boss_names: Vec<String>,
    #[serde(rename = "tileMultipliers")]
    pub tile_multipliers: Vec<f64>,
    #[serde(rename = "buttonRates")]
    pub button_rates: Vec<f64>,
}

/// One of the thirty Depth Charge upgrades.
#[derive(Clone, Debug, Deserialize)]
pub struct Upgrade {
    pub name: String,
    #[serde(rename = "maxLv")]
    pub max_lv: f64,
    #[serde(rename = "costExp")]
    pub cost_exp: f64,
    pub bonus: f64,
    pub desc: String,
}

/// The floor rewards: the bonus each beaten floor pays, and its text.
#[derive(Clone, Debug, Deserialize)]
pub struct FloorRewards {
    pub qty: Vec<f64>,
    pub desc: Vec<String>,
}

impl GameData {
    /// The upgrade at `index`, if the table has one.
    pub fn upgrade(&self, index: usize) -> Option<&Upgrade> {
        self.upgrades.get(index)
    }

    /// The boss name for a floor, as the dashboard shows it.
    pub fn boss_name(&self, floor: usize) -> String {
        self.boss_names
            .get(floor)
            .cloned()
            .unwrap_or_else(|| format!("Boss {floor}"))
    }

    /// The floor reward quantity for a beaten floor.
    pub fn floor_reward(&self, floor: usize) -> f64 {
        self.floor_rewards.qty.get(floor).copied().unwrap_or(0.0)
    }

    /// The floor reward description with its `{`/`}` markers filled in, as the
    /// dashboard renders it.
    pub fn floor_reward_desc(&self, floor: usize) -> String {
        let Some(text) = self.floor_rewards.desc.get(floor) else {
            return String::new();
        };
        let qty = self.floor_reward(floor);
        text.replace('{', &qty.to_string())
            .replace('}', &format!("{:.2}", 1.0 + qty / 100.0))
    }

    /// The bonus for beating `floor`, as the AFK context reads it: a floor
    /// pays its reward only once it is behind you.
    pub fn floor_bonus(&self, floor: usize, cleared: usize) -> f64 {
        if cleared > floor {
            self.floor_reward(floor)
        } else {
            0.0
        }
    }
}

/// The game data, parsed once.
pub fn game() -> &'static GameData {
    static GAME: OnceLock<GameData> = OnceLock::new();
    GAME.get_or_init(|| serde_json::from_str(MINEHEAD_JSON).expect("the Minehead data parses"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tables_carry_every_upstream_row() {
        let game = game();
        assert_eq!(game.upgrades.len(), 30);
        assert_eq!(game.grid_dims.len(), 17);
        assert_eq!(game.floor_rewards.qty.len(), game.floor_rewards.desc.len());
        assert_eq!(game.tile_multipliers.len(), 14);
        assert_eq!(game.button_rates.len(), 9);
        assert_eq!(game.upgrades[26].name, "El' Cheapo Upgrado");
        assert_eq!(game.grid_dims[0], "3,3");
        assert_eq!(game.grid_dims[16], "12,6");
    }

    #[test]
    fn a_floor_reward_fills_its_markers() {
        let game = game();
        assert_eq!(game.floor_reward(6), 50.0);
        assert!(game.floor_reward_desc(6).contains("1.50x"));
        assert_eq!(game.floor_bonus(6, 6), 0.0);
        assert_eq!(game.floor_bonus(6, 7), 50.0);
    }
}
