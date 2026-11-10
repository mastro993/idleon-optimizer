//! The save-table metrics of `js/endgame-stats.js`.
//!
//! Every arm below is the upstream expression with its reads made explicit.
//! Metrics that need the account stats engine (`createStatContext`), the Tome
//! scorer, breeding totals or game-data tables this build does not ship return
//! `None`, so [`super::collect`] reports them as unavailable with a reason.

use super::{Access, Computed, max_zero, number};
use serde_json::Value;

/// `STATUE_COUNT`, from `js/stats/data/common/statues.js`.
const STATUE_COUNT: usize = 32;
/// `DreamUpg.length`, from `js/stats/data/game/customlists.js`.
const DREAM_UPGRADES: usize = 14;
/// `MapDetails[2][0][0]`: Blunder Hills' Green Mushroom spawn count.
const GREEN_MUSHROOM_KILLS: f64 = 25.0;

/// The skill levels `Lv0_<n>[1..=20]` carry, in the upstream `CATEGORIES.Skills` order.
const SKILLS: [&str; 20] = [
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

/// One metric, or `None` when it needs the stats engine.
pub(super) fn compute(access: &Access, id: &str) -> Result<Option<Computed>, String> {
    // `CATEGORIES.Skills` reads each skill's highest saved level at its index.
    if let Some(index) = SKILLS.iter().position(|skill| *skill == id) {
        let value = maxima(access, |ci| {
            access.number_at(&character(ci), &[index + 1], false)
        })?;
        return Ok(Some(Computed::new(value)));
    }

    let value = match id {
        // Whole-table sums.
        "totalStamps" | "totalCards" | "totalGrimoireUpgrades" | "totalArcanistUpgrades" => {
            access.sum_at(id_table(id), &[])?
        }
        "totalWaves" => access.sum_at("TotemInfo", &[0])?,
        "totalMeals" => access.sum_at("Meals", &[0])?,
        "totalVials" => access.sum_at("CauldronInfo", &[4])?,
        "totalPlotRanks" => access.sum_at("FarmRank", &[0])?,
        "totalPaletteLevels" => access.sum_at("Spelunk", &[9])?,
        "totalSpelunkingUpgrades" => access.sum_at("Spelunk", &[5])?,
        "totalGrandDiscoveries" => access.sum_at("Spelunk", &[44])?,
        "totalZenithMarketUpgrades" => access.sum_at("Spelunk", &[45])?,
        "totalAdviceFishUpgrades" => access.sum_at("Spelunk", &[11])?,
        "totalSummoningUpgrades" => access.sum_at("Summon", &[0])?,
        "totalSushiStationUpgrades" => access.sum_at("Sushi", &[2])?,
        "totalSushiKnowledgeLevels" => access.sum_at("Sushi", &[7])?,
        "totalJellyCellLevels" => access.sum_at("Research", &[16])?,
        "totalJellyUpgrades" => access.sum_at("Research", &[17])?,
        "totalCompassUpgrades" => access.sum_at("Compass", &[0])?,
        "totalRoyalArmoryUpgrades" => access.sum_at("RoyalG", &[2])?,
        "totalRoyalResourceGrades" => access.sum_at("RoyalG", &[5])?,
        "totalRoyalStatueLevels" => access.sum_at("RoyalG", &[0])?,
        "totalMenagerieBoostLevels" => access.sum_at("Menagerie", &[3])?,
        "totalMeasurementLevels" => access.sum_at("Holes", &[22])?,
        "totalOpals" => access.sum_at("Holes", &[7])?,
        "totalBarExpansions" => access.sum_at("Holes", &[8])?,
        "totalCollectibleLevels" => access.sum_at("Holes", &[24])?,
        "totalFountainUpgrades" => access.sum_at("Holes", &[31])?,
        "totalSpelunkingDepths" => access.sum_at("Spelunk", &[1])?,
        "totalManicSpelunkingDepths" => access.sum_at("Spelunk", &[47])?,
        "glimboTotalTrades" => access.sum_at("Research", &[12])?,

        // Half-open index ranges of a table.
        "totalBuildings" => access.slice_sum("Tower", &[], 0, 27)?,
        "totalDayMarketLevels" => access.slice_sum("FarmUpg", &[], 2, 10)?,
        "totalNightMarketLevels" => access.slice_sum("FarmUpg", &[], 10, 18)?,
        "totalDivinityBlessingLevels" => access.slice_sum("Divinity", &[], 28, 38)?,
        "totalEquinoxUpgrades" => access.slice_sum("Dream", &[], 2, 2 + DREAM_UPGRADES)?,
        "totalBubbles" => access.slice_sum("CauldronInfo", &[], 0, 4)?,
        "totalBraveryLevels" => access.slice_sum("Holes", &[15], 0, 10)?,
        "totalJusticeLevels" => access.slice_sum("Holes", &[15], 10, 20)?,
        "brokenJars" => access.slice_sum("Holes", &[11], 40, 50)?,
        "totalGambitTime" => access.slice_sum("Holes", &[11], 65, 71)?,

        // Single saved values.
        "bits" => access.number_at("Gaming", &[0], false)?,
        "jadeCoins" => access.number_at("Ninja", &[102, 0], false)?,
        "whiteEssence" => access.number_at("Summon", &[2, 0], false)?,
        "highestNugget" => access.number_at("Gaming", &[8], false)?,
        "cookingMasteryLevel" => access.number_at("CookMaster", &[1, 0], false)?,
        "bestJellyDps" => access.number_at("Research", &[7, 12], false)?,
        "bestSushiCombo" => access.number_at("Sushi", &[4, 8], false)?,
        "menagerieRank" => access.number_at("Menagerie", &[0, 2], false)?,
        "mineheadOpponentsDefeated" => access.number_at("Research", &[7, 4], false)?,
        "highestMegaFlesh" => access.number_at("Bubba", &[1, 8], false)?,
        "bestDenScore" => access.number_at("Holes", &[11, 8], false)?,
        "highestJusticeRound" => access.number_at("Holes", &[11, 74], false)?,
        "bellRings" => access.number_at("Holes", &[18, 1], false)?,
        "bellPings" => access.number_at("Holes", &[18, 3], false)?,
        "highestCogExp" => highest_cog_exp(access)?,
        "highestCallMeBob" => access.number_at("CauldronInfo", &[0, 11], false)?,
        "highestCallMeAsh" => access.number_at("CauldronInfo", &[1, 11], false)?,
        "highestHammerHammer" => access.number_at("CauldronInfo", &[1, 2], false)?,
        "highestBigP" => access.number_at("CauldronInfo", &[3, 21], false)?,

        // Saved account counters, read from `OptionsListAccount`/`OptLacc`.
        "totalGreenStacks" => access.option(224)?,
        "logSample" => access.option(211)?,
        "copperSample" => access.option(212)?,
        "fliesSample" => access.option(215)?,
        "sporeSample" => access.option(213)?,
        "goldFishSample" => access.option(214)?,
        "totalButtonPresses" => access.option(594)?,
        "arenaWaves" => access.option(89)?,
        "dkOrbKills" => access.option(138)?,
        "sbPlunderousKills" => access.option(139)?,
        "esWormholeKills" => access.option(152)?,
        "vwPortals" => access.option(158)?,
        "fractalHours" => access.option(184)?,
        "dungeonCredits" => access.option(72)?,
        "dungeonFlurbos" => access.option(73)?,
        "endlessSummoningWins" => access.option(319)?,
        "bestCrucibleRound" => access.option(628)?,
        "highestShowdownLevel" => access.option(369)?,
        "mostGiantsKilled" => access.option(217)?,
        "highestMegafeather" => access.option(262)?,
        "highestMegaFish" => access.option(279)?,
        "totalBonesCollected" => access.option(329)?,
        "totalDustCollected" => access.option(362)?,
        "totalTachyonsCollected" => access.option(394)?,
        "penpalMinigame" => access.option(99)?,
        "hoopsMinigame" => access.option(242)?,
        "dartsMinigame" => access.option(442)?,
        "totalCoralKidUpgrades" => options_sum(access, 616, 6)?,

        // Minigames, colosseums and salt ranks.
        "w1Colo" | "w2Colo" | "w3Colo" | "w4Colo" | "w5Colo" | "w6Colo" => {
            access.number_at("FamValColosseumHighscores", &[world_index(id)], false)?
        }
        "choppingMinigame" | "fishingMinigame" | "catchingMinigame" | "miningMinigame" => {
            access.number_at("FamValMinigameHiscores", &[minigame_index(id)], false)?
        }
        "redoxSaltRank"
        | "explosiveSaltRank"
        | "spontaneitySaltRank"
        | "dioxideSaltRank"
        | "purpleSaltRank"
        | "nulloSaltRank"
        | "crosslinkSaltRank"
        | "propagatedSaltRank"
        | "anionicSaltRank" => access.number_at("Refinery", &[salt_index(id), 1], false)?,

        // The world task boards, which moved from `Tasks` into `TaskZZ0`.
        "afkTime"
        | "monstersKilled"
        | "refinedSalts"
        | "totalMaterialPrinted"
        | "trashedCogs"
        | "plantsPicked" => task(access, id)?,

        // Counting the entries of a saved table.
        "slab" => access.entries("Cards1")?.len() as f64,
        "totalGamingCrowns" => access.entries_at("Research", &[11])?.len() as f64,
        "totalMedallions" => access.entries_at("Compass", &[3])?.len() as f64,
        "totalOutposts" => access
            .entries("RoyalMaps")?
            .into_iter()
            .filter(|(_, row)| row.as_array().is_some_and(|cells| cells.len() >= 3))
            .count() as f64,
        "totalSushiPerfectos" => {
            let mut perfectos = 0;
            for (_, value) in access.entries_at("Sushi", &[5])? {
                if number(&value, true)? >= 1.0 {
                    perfectos += 1;
                }
            }
            perfectos as f64
        }

        // Sums over saved rows.
        "totalShrines" => rows_sum(access, "Shrine", &[3])?,
        "totalPrimeKills" => prime_kills(access)?,
        "totalBoats" => boats(access)?,
        "totalMoney" => {
            let bank = access.number("MoneyBANK")?;
            let money = characters(access, |ci| access.number(&format!("Money_{ci}")))?;
            bank + money.iter().sum::<f64>()
        }
        "totalLevels" => {
            characters_sum(access, |ci| access.number_at(&character(ci), &[0], false))?
        }
        "totalSkillsLevels" => characters_sum(access, |ci| {
            let mut total = 0.0;
            for (index, value) in access.entries_at(&character(ci), &[])? {
                let Ok(index) = index.parse::<usize>() else {
                    continue;
                };
                if (1..22).contains(&index) {
                    total += number(&value, true)?.max(0.0);
                }
            }
            Ok(total)
        })?,
        "level" => maxima(access, |ci| access.number_at(&character(ci), &[0], false))?,
        "highestPowerPet" => highest_power_pet(access)?,
        "biggestHaulSpelunking" => max_zero(&access.at("Spelunk", &[2])?)?,
        "greenMushroomKills" => green_mushroom_kills(access)?,
        "godRank" => (access.number_at("Divinity", &[25], true)? - 10.0).max(0.0),
        "highestCropOg" => 2.0_f64.powf(access.option(219)?),
        "highestColonyLevel" => access.number_at("Holes", &[11, 26], false)? + 1.0,
        "highestSanctumLevel" => access.number_at("Holes", &[11, 55], false)? + 1.0,
        "totalLayersDestroyed" => {
            let mut total = 0.0;
            for index in [1, 3, 5, 7] {
                total += access.number_at("Holes", &[11, index], false)?;
            }
            total
        }
        "totalStringLevels" => {
            let mut total = 0.0;
            for (index, value) in access.entries_at("Holes", &[19])? {
                let Ok(index) = index.parse::<usize>() else {
                    continue;
                };
                if index < 12 && index % 2 == 0 {
                    total += number(&value, false)?;
                }
            }
            total
        }
        "logBook" => log_book(access)?,
        "totalStatues" => return statues(access).map(Some),

        // The metrics that need the account stats engine, the Tome scorer or
        // breeding totals this build does not port yet.
        "strength"
        | "agility"
        | "wisdom"
        | "luck"
        | "dropRate"
        | "cashMulti"
        | "defence"
        | "accuracy"
        | "hp"
        | "mp"
        | "highestDamage"
        | "postOfficeOrders"
        | "highestConstructExp/hr"
        | "highestSpelunkingPower"
        | "highestArcaneMapMulti"
        | "highestOutpostResourceRate"
        | "highestVillagerExp/hr"
        | "totalVillagerExp/hr"
        | "totalShinyLevels"
        | "totalBreedabilityLevels"
        | "totalTomePoints" => {
            return Ok(None);
        }

        other => return Err(format!("Leaderboard metric has no computation: {other}")),
    };
    Ok(Some(Computed::new(value)))
}

/// The saved table a whole-table sum reads.
fn id_table(id: &str) -> &'static str {
    match id {
        "totalStamps" => "StampLv",
        "totalCards" => "Cards0",
        "totalGrimoireUpgrades" => "Grimoire",
        _ => "Arcane",
    }
}

fn character(index: usize) -> String {
    format!("Lv0_{index}")
}

/// `characters(compute).reduce(sum)`: the sum over every character.
fn characters_sum(
    access: &Access,
    mut compute: impl FnMut(usize) -> Result<f64, String>,
) -> Result<f64, String> {
    Ok(characters(access, &mut compute)?.iter().sum())
}

fn characters<T>(
    access: &Access,
    mut compute: impl FnMut(usize) -> Result<T, String>,
) -> Result<Vec<T>, String> {
    let count = access.characters()?;
    (0..count).map(&mut compute).collect()
}

/// `Math.max(...characters(compute))`.
fn maxima(
    access: &Access,
    mut compute: impl FnMut(usize) -> Result<f64, String>,
) -> Result<f64, String> {
    let mut max = f64::NEG_INFINITY;
    for ci in 0..access.characters()? {
        max = max.max(compute(ci)?);
    }
    Ok(max)
}

/// `sum + _number(read(key, ...path)[index])` over every row.
fn rows_sum(access: &Access, key: &str, path: &[usize]) -> Result<f64, String> {
    let mut total = 0.0;
    for (_, row) in access.entries(key)? {
        let mut value = row;
        for step in path {
            value = super::element(&value, *step)?;
        }
        total += number(&value, false)?;
    }
    Ok(total)
}

/// `_sum` over the `count` options starting at `start`.
fn options_sum(access: &Access, start: usize, count: usize) -> Result<f64, String> {
    let mut total = 0.0;
    for index in start..start + count {
        total += access.option(index)?;
    }
    Ok(total)
}

/// `save.Tasks != null ? read('Tasks', 0, world, index) : read('TaskZZ0', world, index)`.
fn task(access: &Access, id: &str) -> Result<f64, String> {
    let (world, index) = match id {
        "highestDamage" => (1, 0),
        "afkTime" => (0, 2),
        "monstersKilled" => (0, 0),
        "refinedSalts" => (2, 0),
        "trashedCogs" => (2, 1),
        "totalMaterialPrinted" => (2, 3),
        _ => (4, 3),
    };
    if access.has("Tasks") {
        access.number_at("Tasks", &[0, world, index], false)
    } else {
        access.number_at("TaskZZ0", &[world, index], false)
    }
}

fn world_index(id: &str) -> usize {
    id.as_bytes()[1].wrapping_sub(b'1') as usize
}

fn minigame_index(id: &str) -> usize {
    match id {
        "choppingMinigame" => 0,
        "fishingMinigame" => 1,
        "catchingMinigame" => 2,
        _ => 3,
    }
}

fn salt_index(id: &str) -> usize {
    const RANKS: [&str; 9] = [
        "redoxSaltRank",
        "explosiveSaltRank",
        "spontaneitySaltRank",
        "dioxideSaltRank",
        "purpleSaltRank",
        "nulloSaltRank",
        "crosslinkSaltRank",
        "propagatedSaltRank",
        "anionicSaltRank",
    ];
    3 + RANKS
        .iter()
        .position(|rank| *rank == id)
        .expect("a salt rank id")
}

/// `Math.max(0, ...CogM rows' `d`)`.
fn highest_cog_exp(access: &Access) -> Result<f64, String> {
    let mut max: f64 = 0.0;
    for (_, row) in access.entries("CogM")? {
        let depth = match row.get("d") {
            None | Some(Value::Null) => 0.0,
            Some(depth) => number(depth, false)?,
        };
        max = max.max(depth);
    }
    Ok(max)
}

/// `_entries(KRbest)` without the summoning rows, summed.
fn prime_kills(access: &Access) -> Result<f64, String> {
    let mut total = 0.0;
    for (key, value) in access.entries("KRbest")? {
        if !key.contains("SummzTrz") {
            total += number(&value, false)?;
        }
    }
    Ok(total)
}

/// `totalBoats`: every saved ship's loot and speed levels.
fn boats(access: &Access) -> Result<f64, String> {
    let mut total = 0.0;
    for (_, row) in access.entries("Boats")? {
        total += number(&super::element(&row, 3)?, false)?;
        total += number(&super::element(&row, 5)?, false)?;
    }
    Ok(total)
}

/// `highestPowerPet`: the strongest pet in the roster or storage.
fn highest_power_pet(access: &Access) -> Result<f64, String> {
    let mut max: f64 = 0.0;
    for key in ["Pets", "PetsStored"] {
        for (_, row) in access.entries(key)? {
            max = max.max(number(&super::element(&row, 2)?, false)?);
        }
    }
    Ok(max)
}

/// `greenMushroomKills`: the map's spawn count minus each character's remaining
/// kills, summed and clamped at zero.
fn green_mushroom_kills(access: &Access) -> Result<f64, String> {
    let mut total = 0.0;
    for ci in 0..access.characters()? {
        let row = access.at(&format!("KLA_{ci}"), &[2])?;
        let remaining = match &row {
            Value::Array(items) => number(items.first().ok_or_else(super::missing)?, true)?,
            other => number(other, true)?,
        };
        total += GREEN_MUSHROOM_KILLS - remaining;
    }
    Ok(total.max(0.0))
}

/// `totalStatues`: each statue's highest saved character level.
fn statues(access: &Access) -> Result<Computed, String> {
    let count = access.character_count();
    if count == 0 {
        return Err("No character statue data available.".into());
    }
    let mut levels = [0.0_f64; STATUE_COUNT];
    let mut missing = 0;
    for ci in 0..count {
        let rows = access
            .entries_at(&format!("StatueLevels_{ci}"), &[])
            .and_then(|rows| {
                rows.into_iter()
                    .filter_map(|(index, row)| {
                        let index = index.parse::<usize>().ok()?;
                        (index < STATUE_COUNT).then_some((index, row))
                    })
                    .map(|(index, row)| {
                        let level = match &row {
                            Value::Array(cells) => {
                                number(cells.first().ok_or_else(super::missing)?, false)?
                            }
                            other => number(other, false)?,
                        };
                        Ok((index, level))
                    })
                    .collect::<Result<Vec<_>, String>>()
            });
        match rows {
            Ok(rows) => {
                for (index, level) in rows {
                    levels[index] = levels[index].max(level);
                }
            }
            Err(_) => missing += 1,
        }
    }
    if missing == count {
        return Err("No character statue data available.".into());
    }
    Ok(Computed {
        value: levels.iter().sum(),
        partial: missing > 0,
    })
}

/// `logBook`: the saved Gaming logbook's ten collection levels.
fn log_book(access: &Access) -> Result<f64, String> {
    const ALPHABET: &str = "_abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
    let Value::String(encoded) = access.at("Gaming", &[11])? else {
        return Err("Saved Gaming logbook is invalid.".into());
    };
    if encoded.chars().count() != 10 || encoded.chars().any(|letter| !ALPHABET.contains(letter)) {
        return Err("Saved Gaming logbook is invalid.".into());
    }
    Ok(encoded
        .chars()
        .map(|letter| {
            ALPHABET
                .chars()
                .position(|candidate| candidate == letter)
                .unwrap_or(0)
                .min(10) as f64
        })
        .sum())
}
