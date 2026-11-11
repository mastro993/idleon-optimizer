//! Static sailing tables, extracted from the upstream JS calculators.

use serde::Deserialize;
use std::sync::OnceLock;

const SAILING_JSON: &str = include_str!("../../data/sailing.json");

/// Everything the fleet model reads from the game.
#[derive(Debug, Deserialize)]
pub struct SailingData {
    pub artifacts: Vec<Artifact>,
    pub islands: Vec<Island>,
    #[serde(rename = "captainBonuses")]
    pub captain_bonuses: Vec<CaptainBonus>,
}

/// One sailing artifact: its base bonus, base odds and tier bonuses.
#[derive(Debug, Deserialize)]
pub struct Artifact {
    pub id: usize,
    pub name: String,
    /// The tier-zero denominator; `None` means the artifact never drops.
    #[serde(rename = "baseOdds")]
    pub base_odds: Option<f64>,
    #[serde(rename = "baseBonus")]
    pub base_bonus: f64,
    /// The bonus for tiers 2 through 7, in order.
    #[serde(rename = "tierBonuses")]
    pub tier_bonuses: Vec<f64>,
}

/// One island: travel distance, unlock cost, captain EXP and artifact count.
#[derive(Debug, Deserialize)]
pub struct Island {
    pub id: usize,
    pub distance: f64,
    #[serde(rename = "cloudRequired")]
    pub cloud_required: f64,
    #[serde(rename = "captainExp")]
    pub captain_exp: f64,
    #[serde(rename = "artifactCount")]
    pub artifact_count: usize,
}

/// A captain bonus type's display name.
#[derive(Debug, Deserialize)]
pub struct CaptainBonus {
    pub id: usize,
    pub name: String,
}

/// The sailing tables, parsed once.
pub fn sailing() -> &'static SailingData {
    static DATA: OnceLock<SailingData> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(SAILING_JSON).expect("the shipped sailing data is valid JSON")
    })
}

/// Every island upstream declares.
pub fn island_count() -> usize {
    sailing().islands.len()
}

/// Every artifact upstream declares.
pub fn artifact_count() -> usize {
    sailing().artifacts.len()
}

/// An island by index, if the data declares it.
pub fn island(idx: usize) -> Option<&'static Island> {
    sailing().islands.get(idx)
}

/// An artifact by index, if the data declares it.
pub fn artifact(idx: usize) -> Option<&'static Artifact> {
    sailing().artifacts.get(idx)
}

/// An island's travel distance, `0` for an island the data lacks.
pub fn island_distance(idx: usize) -> f64 {
    island(idx).map(|island| island.distance).unwrap_or(0.0)
}

/// An island's artifact count, `0` for an island the data lacks.
pub fn island_artifact_count(idx: usize) -> usize {
    island(idx).map(|island| island.artifact_count).unwrap_or(0)
}

/// The cloud progress an island needs before it is selectable.
pub fn island_cloud_required(idx: usize) -> f64 {
    island(idx)
        .map(|island| island.cloud_required)
        .unwrap_or(0.0)
}

/// The first artifact index an island can drop.
pub fn island_artifact_offset(idx: usize) -> usize {
    (0..idx).map(island_artifact_count).sum()
}

/// An artifact's tier-zero denominator, `Infinity` when it never drops.
pub fn artifact_base_odds(idx: usize) -> f64 {
    artifact(idx)
        .and_then(|artifact| artifact.base_odds)
        .unwrap_or(f64::INFINITY)
}

/// An artifact's base bonus at tier one.
pub fn artifact_base(idx: usize) -> f64 {
    artifact(idx)
        .map(|artifact| artifact.base_bonus)
        .unwrap_or(0.0)
}

/// An artifact's bonus at `tier`, matching upstream `artifactTierBonus`.
pub fn artifact_tier_bonus(idx: usize, tier: u32) -> f64 {
    if !(2..=7).contains(&tier) {
        return 0.0;
    }
    artifact(idx)
        .and_then(|artifact| artifact.tier_bonuses.get((tier - 2) as usize))
        .copied()
        .unwrap_or(0.0)
}

/// A captain bonus type's display name.
pub fn captain_bonus_name(idx: i32) -> String {
    if idx < 0 {
        return "No bonus".into();
    }
    sailing()
        .captain_bonuses
        .get(idx as usize)
        .map(|bonus| bonus.name.clone())
        .unwrap_or_else(|| format!("Bonus {idx}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sailing_tables_match_the_upstream_shape() {
        assert_eq!(island_count(), 17);
        assert_eq!(artifact_count(), 41);
        assert_eq!(island_distance(0), 25.0);
        assert_eq!(island_artifact_offset(1), 4);
        assert_eq!(artifact_base(5), 2.0);
        assert_eq!(artifact_tier_bonus(9, 2), 25.0);
        assert_eq!(captain_bonus_name(0), "Boat Speed");
    }
}
