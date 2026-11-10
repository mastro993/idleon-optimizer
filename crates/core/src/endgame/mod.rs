//! The joke endgame check: the verdict, and the leaderboard audit behind it.
//!
//! Ported from the upstream `endgame.html`, `js/endgamers.js` and
//! `js/endgame-stats.js`. The verdict is `NO` for every save: upstream only
//! answers `YES` for a handful of character names whose SHA-256 hashes are
//! sealed in `endgamers.js`, and three of those unlock an AES-GCM message. The
//! names behind those hashes are the upstream authors' own accounts, so the
//! gate cannot be reproduced meaningfully and this build always answers `NO`.
//!
//! The stats are the save-table half of `collectEndgameStats`: every metric it
//! reads straight out of the decoded save, with the upstream ids, labels and
//! group order. Metrics that need the account stats engine (damage, drop rate,
//! coin multiplier, tome score, villager rates, …) come back without a value and
//! say why, exactly like a metric whose computation threw upstream.

mod stats;

use crate::save::json;
use serde_json::{Map, Value};

/// The endgame verdict. Upstream's sealed-name gate can only ever reject a
/// player's save, so the port answers this unconditionally.
pub const VERDICT: &str = "NO";

/// Why a metric has no value in this build.
pub const UNPORTED: &str = "Needs the account stats engine, which is not built yet.";

/// The leaderboard categories, in the upstream `CATEGORIES` order.
pub const GROUPS: [(&str, &[&str]); 7] = [
    ("General", GENERAL),
    ("Tasks", TASKS),
    ("Skills", SKILLS),
    ("Character", CHARACTER),
    ("Miscellaneous", MISCELLANEOUS),
    ("Masterclass", MASTERCLASS),
    ("Caverns", CAVERNS),
];

/// One leaderboard metric: its id, full label, group and saved value.
///
/// `value` is `None` when the save does not carry the field, the field is
/// invalid, or this build cannot compute the metric; `detail` says which.
#[derive(Clone, Debug, PartialEq)]
pub struct Stat {
    pub id: &'static str,
    pub label: String,
    pub group: &'static str,
    pub value: Option<f64>,
    /// Some character data the maximum cannot cover, so the value is a floor.
    pub partial: bool,
    pub detail: Option<String>,
}

/// Collect every upstream leaderboard metric from a decoded save object.
///
/// `save` is the `data` object of an `it.json`, or a flat save; `char_names` is
/// the length of the `charNames` roster that sits beside `data`. A save without
/// complete research rows is rejected, as upstream does before it collects.
pub fn collect(save: &Value, char_names: usize) -> Result<Vec<Stat>, String> {
    let Some(data) = save.as_object() else {
        return Err("No save data provided.".into());
    };
    let research = json::parsed_field(data, "Research")
        .ok_or_else(|| "Not included in this save.".to_string())?;
    let complete = research.as_array().is_some_and(|rows| {
        [0, 1, 2, 3, 4, 7, 8]
            .iter()
            .all(|index| rows.get(*index).is_some_and(Value::is_array))
    });
    if !complete {
        return Err("Research data is missing or incomplete.".into());
    }

    let access = Access::new(data, char_names);
    let mut collected = Vec::with_capacity(177);
    for (group, ids) in GROUPS {
        for id in ids {
            let mut stat = Stat {
                id,
                label: label(id),
                group,
                value: None,
                partial: false,
                detail: None,
            };
            match stats::compute(&access, id) {
                Ok(Some(computed)) => {
                    stat.value = Some(computed.value);
                    stat.partial = computed.partial;
                }
                Ok(None) => stat.detail = Some(UNPORTED.into()),
                Err(detail) => stat.detail = Some(detail),
            }
            collected.push(stat);
        }
    }
    Ok(collected)
}

/// A metric's label: the upstream `LABELS` overrides, then the camel-case split
/// and capitalisation `collectEndgameStats` falls back to.
pub fn label(id: &str) -> String {
    let label = match id {
        "totalMoney" => "Total money",
        "totalLevels" => "Total character levels",
        "totalSkillsLevels" => "Total skill levels",
        "choppin" => "Chopping",
        "cashMulti" => "Highest coin multiplier",
        "totalTomePoints" => "Tome score",
        "bestJellyDps" => "Best Jelly DPS",
        "highestCropOg" => "Highest Crop OG",
        "dkOrbKills" => "DK Orb kills",
        "sbPlunderousKills" => "SB Plunderous kills",
        "esWormholeKills" => "ES Wormhole kills",
        "vwPortals" => "VW portals",
        "hp" => "HP",
        "mp" => "MP",
        _ => return derived_label(id),
    };
    label.into()
}

/// `id.replace(/([a-z0-9])([A-Z])/g, '$1 $2')`, the `w<N>Colo` rename, then a
/// capitalised first letter.
fn derived_label(id: &str) -> String {
    let chars: Vec<char> = id.chars().collect();
    let mut split = String::with_capacity(id.len() + 4);
    for (index, ch) in chars.iter().enumerate() {
        if let Some(previous) = index.checked_sub(1).and_then(|i| chars.get(i))
            && (previous.is_ascii_lowercase() || previous.is_ascii_digit())
            && ch.is_ascii_uppercase()
        {
            split.push(' ');
        }
        split.push(*ch);
    }
    let renamed = match split.strip_prefix('w') {
        Some(rest) => match rest.split_once(' ') {
            Some((world, "Colo")) if world.chars().all(|c| c.is_ascii_digit()) => {
                format!("W{world} Colosseum")
            }
            _ => split,
        },
        None => split,
    };
    let mut characters = renamed.chars();
    match characters.next() {
        Some(first) => first.to_uppercase().chain(characters).collect(),
        None => renamed,
    }
}

/// A metric this build can compute.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Computed {
    pub value: f64,
    pub partial: bool,
}

impl Computed {
    pub(crate) fn new(value: f64) -> Self {
        Self {
            value,
            partial: false,
        }
    }
}

const MISSING: &str = "Not included in this save.";
const INVALID: &str = "Saved value is missing or invalid.";
const BAD_TABLE: &str = "Saved table is invalid.";

/// The decoded save, read the way `endgame-stats.js` reads it: `parseSaveKey`
/// for every field, unsigned numbers unless a metric is explicitly signed.
pub(crate) struct Access<'a> {
    data: &'a Map<String, Value>,
    options_key: &'static str,
    characters: usize,
}

impl<'a> Access<'a> {
    fn new(data: &'a Map<String, Value>, char_names: usize) -> Self {
        let options_key = if data.contains_key("OptionsListAccount") {
            "OptionsListAccount"
        } else {
            "OptLacc"
        };
        Self {
            data,
            options_key,
            characters: character_count(data, char_names),
        }
    }

    /// How many characters the save carries, as `_characterCount` counts them.
    pub(crate) fn character_count(&self) -> usize {
        self.characters
    }

    /// The character count, or the upstream error when the save has none.
    pub(crate) fn characters(&self) -> Result<usize, String> {
        match self.characters {
            0 => Err("No character data available.".into()),
            count => Ok(count),
        }
    }

    /// Whether the save carries `key` at all, as `save.Tasks != null` checks.
    pub(crate) fn has(&self, key: &str) -> bool {
        self.data.contains_key(key)
    }

    /// A `parseSaveKey` field.
    pub(crate) fn field(&self, key: &str) -> Result<Value, String> {
        json::parsed_field(self.data, key).ok_or_else(|| MISSING.to_string())
    }

    /// A field drilled through array indexes, like `_read(save, key, ...path)`.
    pub(crate) fn at(&self, key: &str, path: &[usize]) -> Result<Value, String> {
        let mut value = self.field(key)?;
        for step in path {
            value = step_value(&value, *step).ok_or_else(|| MISSING.to_string())?;
            if value.is_null() {
                return Err(MISSING.into());
            }
        }
        Ok(value)
    }

    /// An unsigned number at `key`.
    pub(crate) fn number(&self, key: &str) -> Result<f64, String> {
        number(&self.field(key)?, false)
    }

    /// A number drilled through array indexes; unsigned unless `signed`.
    pub(crate) fn number_at(&self, key: &str, path: &[usize], signed: bool) -> Result<f64, String> {
        number(&self.at(key, path)?, signed)
    }

    /// `option(index)` from `collectEndgameStats`.
    pub(crate) fn option(&self, index: usize) -> Result<f64, String> {
        self.number_at(self.options_key, &[index], false)
    }

    /// A table's entries, nested values included.
    pub(crate) fn entries(&self, key: &str) -> Result<Vec<(String, Value)>, String> {
        entries(&self.field(key)?)
    }

    pub(crate) fn entries_at(
        &self,
        key: &str,
        path: &[usize],
    ) -> Result<Vec<(String, Value)>, String> {
        entries(&self.at(key, path)?)
    }

    /// `table(key, ...path)`: the recursive sum of a saved table.
    pub(crate) fn sum_at(&self, key: &str, path: &[usize]) -> Result<f64, String> {
        sum(&self.at(key, path)?)
    }

    /// `sumSlice(key, path, start, end)`: the sum of the numbered entries in a
    /// half-open index range, as `totalDayMarketLevels` reads `FarmUpg`.
    pub(crate) fn slice_sum(
        &self,
        key: &str,
        path: &[usize],
        start: usize,
        end: usize,
    ) -> Result<f64, String> {
        let value = self.at(key, path)?;
        let mut total = 0.0;
        for (index, entry) in entries(&value)? {
            let Ok(index) = index.parse::<usize>() else {
                continue;
            };
            if (start..end).contains(&index) {
                total += match entry {
                    Value::Object(_) | Value::Array(_) => sum(&entry)?,
                    scalar => number(&scalar, true)?,
                };
            }
        }
        Ok(total)
    }
}

/// How many characters a save carries: the `charNames` roster or the highest
/// `CharacterClass_<n>`-style row index, whichever is larger.
fn character_count(data: &Map<String, Value>, char_names: usize) -> usize {
    const ROWS: [&str; 14] = [
        "Lv0_",
        "Exp0_",
        "CharacterClass_",
        "SL_",
        "SM_",
        "PlayerStuff_",
        "PVtStarSign_",
        "PVFishingToolkit_",
        "FoodSlO_",
        "AttackLoadout_",
        "MaxCarryCap_",
        "EquipOrder_",
        "InventoryOrder_",
        "ItemQTY_",
    ];
    let rows = data
        .keys()
        .filter_map(|key| {
            ROWS.iter().find_map(|prefix| {
                key.strip_prefix(prefix)
                    .and_then(|index| index.parse::<usize>().ok())
            })
        })
        .map(|index| index + 1)
        .max()
        .unwrap_or(0);
    rows.max(char_names)
}

/// `Object.entries` over an array or object, without `length` and nulls.
fn entries(value: &Value) -> Result<Vec<(String, Value)>, String> {
    let pairs: Vec<(String, Value)> = match value {
        Value::Object(fields) => fields
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect(),
        Value::Array(items) => items
            .iter()
            .enumerate()
            .map(|(index, value)| (index.to_string(), value.clone()))
            .collect(),
        _ => return Err(BAD_TABLE.into()),
    };
    Ok(pairs
        .into_iter()
        .filter(|(key, value)| key != "length" && !value.is_null())
        .collect())
}

/// `_sum`: recurse into nested tables, add every scalar as a signed number.
fn sum(value: &Value) -> Result<f64, String> {
    let mut total = 0.0;
    for (_, entry) in entries(value)? {
        total += match entry {
            Value::Object(_) | Value::Array(_) => sum(&entry)?,
            scalar => number(&scalar, true)?,
        };
    }
    Ok(total)
}

/// `_max`: the largest unsigned entry, never below zero.
pub(crate) fn max_zero(value: &Value) -> Result<f64, String> {
    let mut max = 0.0_f64;
    for (_, entry) in entries(value)? {
        max = max.max(number(&entry, false)?);
    }
    Ok(max)
}

/// `_number`: a finite number or numeric string; negatives only when `signed`.
pub(crate) fn number(value: &Value, signed: bool) -> Result<f64, String> {
    let parsed = match value {
        Value::Number(number) => number.as_f64(),
        Value::String(text) => match text.trim() {
            "" => None,
            text => text.parse::<f64>().ok(),
        },
        _ => None,
    };
    match parsed.filter(|number| number.is_finite()) {
        Some(number) if signed || number >= 0.0 => Ok(number),
        _ => Err(INVALID.into()),
    }
}

/// The saved row element at `step`, or the upstream missing-field error.
pub(crate) fn element(value: &Value, step: usize) -> Result<Value, String> {
    step_value(value, step).ok_or_else(missing)
}

/// The upstream `_read` error for a field the save does not carry.
pub(crate) fn missing() -> String {
    MISSING.into()
}

/// A numeric index into an array or an object keyed by that index.
fn step_value(value: &Value, step: usize) -> Option<Value> {
    match value {
        Value::Array(items) => items.get(step).cloned(),
        Value::Object(fields) => fields.get(&step.to_string()).cloned(),
        _ => None,
    }
}

/// `General` — the upstream leaderboard category.
const GENERAL: &[&str] = &[
    "totalMoney",
    "totalLevels",
    "totalStatues",
    "totalBuildings",
    "totalGreenStacks",
    "logSample",
    "copperSample",
    "fliesSample",
    "sporeSample",
    "goldFishSample",
    "totalWaves",
    "totalMeals",
    "totalShrines",
    "totalVials",
    "totalCards",
    "bits",
    "godRank",
    "greenMushroomKills",
    "highestCropOg",
    "jadeCoins",
    "whiteEssence",
    "totalBoats",
    "logBook",
    "totalShinyLevels",
    "totalBreedabilityLevels",
    "totalPlotRanks",
    "totalDayMarketLevels",
    "totalNightMarketLevels",
    "totalPrimeKills",
    "highestPowerPet",
    "slab",
    "totalTomePoints",
    "totalPaletteLevels",
    "totalDivinityBlessingLevels",
    "totalSummoningUpgrades",
    "totalSpelunkingUpgrades",
    "totalZenithMarketUpgrades",
    "totalCoralKidUpgrades",
    "totalSushiStationUpgrades",
    "totalSushiKnowledgeLevels",
    "totalSushiPerfectos",
    "totalButtonPresses",
    "cookingMasteryLevel",
    "totalAdviceFishUpgrades",
    "totalEquinoxUpgrades",
    "bestJellyDps",
    "totalJellyCellLevels",
    "totalJellyUpgrades",
    "bestSushiCombo",
    "arenaWaves",
    "dkOrbKills",
    "sbPlunderousKills",
    "esWormholeKills",
    "vwPortals",
    "fractalHours",
    "dungeonCredits",
    "dungeonFlurbos",
    "endlessSummoningWins",
    "bestCrucibleRound",
    "menagerieRank",
    "totalMenagerieBoostLevels",
];

/// `Tasks` — the upstream leaderboard category.
const TASKS: &[&str] = &[
    "highestDamage",
    "afkTime",
    "monstersKilled",
    "postOfficeOrders",
    "refinedSalts",
    "totalMaterialPrinted",
    "trashedCogs",
    "plantsPicked",
    "totalBubbles",
    "totalStamps",
    "highestNugget",
];

/// `Skills` — the upstream leaderboard category.
const SKILLS: &[&str] = &[
    "totalSkillsLevels",
    "mining",
    "smithing",
    "choppin",
    "fishing",
    "alchemy",
    "catching",
    "trapping",
    "construction",
    "worship",
    "cooking",
    "breeding",
    "laboratory",
    "sailing",
    "divinity",
    "gaming",
    "farming",
    "sneaking",
    "summoning",
    "spelunking",
    "research",
];

/// `Character` — the upstream leaderboard category.
const CHARACTER: &[&str] = &[
    "strength",
    "agility",
    "wisdom",
    "luck",
    "level",
    "dropRate",
    "cashMulti",
    "defence",
    "accuracy",
    "hp",
    "mp",
];

/// `Miscellaneous` — the upstream leaderboard category.
const MISCELLANEOUS: &[&str] = &[
    "w1Colo",
    "w2Colo",
    "w3Colo",
    "w4Colo",
    "w5Colo",
    "w6Colo",
    "choppingMinigame",
    "fishingMinigame",
    "catchingMinigame",
    "miningMinigame",
    "penpalMinigame",
    "hoopsMinigame",
    "dartsMinigame",
    "redoxSaltRank",
    "explosiveSaltRank",
    "spontaneitySaltRank",
    "dioxideSaltRank",
    "purpleSaltRank",
    "nulloSaltRank",
    "crosslinkSaltRank",
    "propagatedSaltRank",
    "anionicSaltRank",
    "highestCogExp",
    "highestConstructExp/hr",
    "highestCallMeBob",
    "highestCallMeAsh",
    "highestHammerHammer",
    "highestBigP",
    "highestShowdownLevel",
    "mostGiantsKilled",
    "biggestHaulSpelunking",
    "highestSpelunkingPower",
    "totalSpelunkingDepths",
    "totalManicSpelunkingDepths",
    "mineheadOpponentsDefeated",
    "totalGrandDiscoveries",
    "glimboTotalTrades",
    "totalGamingCrowns",
    "highestMegafeather",
    "highestMegaFish",
    "highestMegaFlesh",
];

/// `Masterclass` — the upstream leaderboard category.
const MASTERCLASS: &[&str] = &[
    "totalGrimoireUpgrades",
    "totalBonesCollected",
    "totalCompassUpgrades",
    "totalDustCollected",
    "totalMedallions",
    "totalArcanistUpgrades",
    "totalTachyonsCollected",
    "highestArcaneMapMulti",
    "totalRoyalArmoryUpgrades",
    "totalRoyalResourceGrades",
    "totalRoyalStatueLevels",
    "totalOutposts",
    "highestOutpostResourceRate",
];

/// `Caverns` — the upstream leaderboard category.
const CAVERNS: &[&str] = &[
    "totalMeasurementLevels",
    "totalOpals",
    "totalLayersDestroyed",
    "totalBarExpansions",
    "bestDenScore",
    "totalBraveryLevels",
    "totalJusticeLevels",
    "highestJusticeRound",
    "bellRings",
    "bellPings",
    "totalStringLevels",
    "brokenJars",
    "totalCollectibleLevels",
    "totalGambitTime",
    "highestVillagerExp/hr",
    "totalVillagerExp/hr",
    "highestColonyLevel",
    "highestSanctumLevel",
    "totalFountainUpgrades",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_upstream_metric_appears_once_in_its_group() {
        let ids: Vec<&str> = GROUPS
            .iter()
            .flat_map(|(_, ids)| ids.iter().copied())
            .collect();
        assert_eq!(ids.len(), 177);
        let mut unique = ids.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), 177, "metric ids are unique");
        let counts: Vec<usize> = GROUPS.iter().map(|(_, ids)| ids.len()).collect();
        assert_eq!(counts, [61, 11, 21, 11, 41, 13, 19]);
        assert_eq!(
            GROUPS.iter().map(|(group, _)| *group).collect::<Vec<_>>(),
            [
                "General",
                "Tasks",
                "Skills",
                "Character",
                "Miscellaneous",
                "Masterclass",
                "Caverns"
            ]
        );
    }

    #[test]
    fn labels_match_the_upstream_overrides_and_derivation() {
        assert_eq!(label("totalMoney"), "Total money");
        assert_eq!(label("totalLevels"), "Total character levels");
        assert_eq!(label("choppin"), "Chopping");
        assert_eq!(label("highestCoin"), "Highest Coin");
        assert_eq!(label("w1Colo"), "W1 Colosseum");
        assert_eq!(label("w12Colo"), "W12 Colosseum");
        assert_eq!(label("highestConstructExp/hr"), "Highest Construct Exp/hr");
        assert_eq!(label("choppingMinigame"), "Chopping Minigame");
        assert_eq!(label("dkOrbKills"), "DK Orb kills");
    }

    #[test]
    fn a_save_without_research_rows_is_rejected_before_collecting() {
        let error = |save: &Value| collect(save, 0).expect_err("the save has no research rows");
        assert_eq!(error(&Value::Null), "No save data provided.");
        assert_eq!(
            error(&serde_json::json!({ "Lv0_0": [1] })),
            "Not included in this save."
        );
        assert_eq!(
            error(&serde_json::json!({ "Research": [] })),
            "Research data is missing or incomplete."
        );
    }
}
