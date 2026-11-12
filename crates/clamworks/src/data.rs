//! The Clamworks game tables, extracted from the upstream `Spelunky` list by
//! `script/extract-clamworks-data.mjs`.

use serde::Deserialize;
use std::sync::OnceLock;

const CLAMWORKS_JSON: &str = include_str!("../data/clamworks.json");

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
