//! The sailing side of a save: the active boats and captains, artifact tiers
//! and the account fields the route model reads.

use idleon_research::model::GridState;
use idleon_sailing::{
    ArtifactUnlocks, Boat, Captain, Fleet, MinimumTravel, RouteContext, ServerVars,
};
use idleon_save::Character;
use idleon_save::json;
use serde_json::{Map, Value};

/// What the sailing part of an import produced.
pub(crate) struct SailingImport {
    pub fleet: Fleet,
    pub context: RouteContext,
}

/// Read the fleet, its artifact tiers and the route context.
pub(crate) fn import(
    save: &Map<String, Value>,
    root: &Map<String, Value>,
    characters: &[Character],
    grid: &GridState,
    warnings: &mut Vec<String>,
) -> SailingImport {
    let boats = rows(save, "Boats");
    let captains = rows(save, "Captains");
    let sailing = rows(save, "Sailing");
    // A research-only save may still carry `Sailing[3]` for artifact 37; that is
    // not a fleet, so it must not produce fleet warnings.
    let carries_fleet = save.contains_key("Boats")
        || save.contains_key("Captains")
        || sailing.get(2).is_some_and(|row| !row.is_empty());
    let sailing_boats = count(&sailing, 2, 1);
    let sailing_captains = count(&sailing, 2, 0);

    let fleet = Fleet {
        boats: boats
            .iter()
            .take(sailing_boats)
            .map(|row| boat(row))
            .collect(),
        captains: captains
            .iter()
            .take(sailing_captains)
            .map(|row| captain(row))
            .collect(),
        sailing_level: characters
            .first()
            .and_then(|character| character.sailing_level())
            .unwrap_or(0),
        artifact_tiers: sailing
            .get(3)
            .map(|row| {
                row.iter()
                    .map(|value| {
                        json::number(value).unwrap_or(0.0).max(0.0).round().min(7.0) as u32
                    })
                    .collect()
            })
            .unwrap_or_default(),
        island_progress: sailing
            .first()
            .map(|row| {
                row.iter()
                    .map(|value| json::number(value).unwrap_or(0.0))
                    .collect()
            })
            .unwrap_or_default(),
        currency_balances: sailing
            .get(1)
            .map(|row| {
                row.iter()
                    .map(|value| json::number(value).unwrap_or(0.0))
                    .collect()
            })
            .unwrap_or_default(),
    };
    if carries_fleet {
        if boats.is_empty() {
            warnings.push("Boats: not in the save; the fleet is empty".into());
        } else if fleet.boats.is_empty() {
            warnings.push(format!(
                "Boats: {sailing_boats} active slots but no boat rows; the fleet is empty"
            ));
        }
        if captains.is_empty() {
            warnings.push("Captains: not in the save; no captain is assigned".into());
        } else if fleet.captains.is_empty() {
            warnings.push(format!(
                "Captains: {sailing_captains} active slots but no captain rows; the fleet sails empty"
            ));
        }
        if sailing.is_empty() {
            warnings.push("Sailing: not in the save; one boat and one captain are assumed".into());
        } else if fleet.artifact_tiers.len() < idleon_sailing::artifact_count() {
            warnings.push(
                "Sailing[3]: artifact tiers are short; missing artifacts stay at tier 0".into(),
            );
        }
    }

    let context = route_context(save, root, grid, carries_fleet, warnings);
    SailingImport { fleet, context }
}

fn route_context(
    save: &Map<String, Value>,
    root: &Map<String, Value>,
    grid: &GridState,
    carries_fleet: bool,
    warnings: &mut Vec<String>,
) -> RouteContext {
    let minimum_tier = json::parsed_field(save, "OptLacc")
        .and_then(|options| json::index(&options, &[127]).and_then(json::number))
        .unwrap_or(0.0)
        .clamp(0.0, 5.0)
        .round() as u32;
    let travel = MinimumTravel {
        family: 0.0,
        shiny: 0.0,
        // Davey Jones Returns: Spelunk[18][11] points at 15% per point.
        legend: json::parsed_field(save, "Spelunk")
            .and_then(|spelunk| json::index(&spelunk, &[18, 11]).and_then(json::number))
            .map(|points| (points * 15.0).round())
            .unwrap_or(0.0),
        gem_purchases: json::parsed_field(save, "GemItemsPurchased")
            .and_then(|items| json::index(&items, &[8]).and_then(json::number))
            .unwrap_or(0.0),
    };
    let unlocks = ArtifactUnlocks {
        rift: json::parsed_field(save, "Rift")
            .and_then(|rift| json::index(&rift, &[0]).and_then(json::number))
            .is_some_and(|level| level > 29.0),
        // Tier three's ninja code contains the alphabet's 36th character, `J`.
        ninja: json::parsed_field(save, "Ninja").is_some_and(|ninja| {
            json::index(&ninja, &[102, 9])
                .and_then(Value::as_str)
                .is_some_and(|code| code.contains('J'))
        }),
        spelunk: json::parsed_field(save, "Spelunk")
            .and_then(|spelunk| json::index(&spelunk, &[0, 0]).and_then(json::number))
            .is_some_and(|episode| episode >= 1.0),
        grid_109: grid.level(109) >= 1,
        verisimilitude: json::parsed_field(save, "Menagerie")
            .and_then(|menagerie| json::index(&menagerie, &[2, 0]).and_then(json::number))
            .is_some_and(|owned| owned >= 1.0),
    };
    let chest_rolls = if achievement_complete(save, 300) {
        1
    } else {
        0
    };
    // `SailChests` is the banked pile; the simulation always starts fresh, so
    // only the capacity from the account rows is read here.
    let gem_purchase = json::parsed_field(save, "GemItemsPurchased")
        .and_then(|items| json::index(&items, &[129]).and_then(json::number))
        .unwrap_or(0.0);
    let task_slots = json::parsed_field(save, "TaskZZ2")
        .and_then(|tasks| json::index(&tasks, &[4, 2]).and_then(json::number))
        .unwrap_or(0.0);
    let chest_capacity = idleon_sailing::chest_capacity(
        gem_purchase,
        artifact_tier(save, 19),
        task_slots,
        achievement_complete(save, 287),
        achievement_complete(save, 290),
    );
    let server_vars = ServerVars {
        ancient_odd_per_island: root
            .get("serverVars")
            .and_then(|vars| vars.get("AncientOddPerIsland"))
            .and_then(json::number)
            .unwrap_or(960.0),
        ancient_arti_pct: root
            .get("serverVars")
            .and_then(|vars| vars.get("AncientArtiPCT"))
            .and_then(json::number)
            .unwrap_or(0.0),
    };
    // The save does not expose these without the full account multiplier tree.
    if carries_fleet {
        warnings.push(
            "sailing travel: Siege Breaker family and shiny breeding bonuses are not modelled; trip times stay conservative"
                .into(),
        );
    }
    RouteContext {
        minimum_tier,
        travel,
        unlocks,
        davey_unlocked: grid.level(105) > 0,
        chest_rolls,
        server_vars,
        chest_capacity,
        ..RouteContext::default()
    }
}

/// `AchieveReg[index] == -1` marks a completed achievement.
fn achievement_complete(save: &Map<String, Value>, index: usize) -> bool {
    json::parsed_field(save, "AchieveReg")
        .and_then(|achievements| json::index(&achievements, &[index]).and_then(json::number))
        .is_some_and(|status| status == -1.0)
}

/// `Sailing[3][index]`, the saved artifact tier.
fn artifact_tier(save: &Map<String, Value>, index: usize) -> u32 {
    json::parsed_field(save, "Sailing")
        .and_then(|sailing| json::index(&sailing, &[3, index]).and_then(json::number))
        .unwrap_or(0.0)
        .max(0.0)
        .round()
        .min(7.0) as u32
}

/// A field that may be a JSON array or a string holding one.
fn rows(save: &Map<String, Value>, key: &str) -> Vec<Vec<Value>> {
    match json::parsed_field(save, key) {
        Some(Value::Array(rows)) => rows
            .into_iter()
            .map(|row| match row {
                Value::Array(cells) => cells,
                _ => Vec::new(),
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// `1 + Sailing[row][cell]`, the active slot count upstream uses.
fn count(sailing: &[Vec<Value>], row: usize, cell: usize) -> usize {
    let active = sailing
        .get(row)
        .and_then(|row| row.get(cell))
        .and_then(json::number)
        .unwrap_or(0.0);
    (1.0 + active).max(1.0) as usize
}

fn boat(row: &[Value]) -> Boat {
    let number = |index: usize| row.get(index).and_then(json::number);
    let captain = number(0)
        .filter(|idx| *idx >= 0.0)
        .map(|idx| idx.round() as usize);
    let island = number(1)
        .filter(|idx| *idx >= 0.0)
        .map(|idx| idx.round() as usize);
    let state = number(2).unwrap_or(0.0);
    Boat {
        captain,
        target_island: island.or_else(|| {
            (2.0..2.25)
                .contains(&state)
                .then_some((100.0 * (state - 2.0)).round() as usize)
        }),
        loot_level: number(3).unwrap_or(0.0).max(0.0).round() as u32,
        speed_level: number(5).unwrap_or(0.0).max(0.0).round() as u32,
    }
}

fn captain(row: &[Value]) -> Captain {
    let number = |index: usize| row.get(index).and_then(json::number).unwrap_or(0.0);
    Captain {
        rarity: number(0).max(0.0).round() as u32,
        bonus_types: [number(1).round() as i32, number(2).round() as i32],
        level: number(3).max(0.0).round() as u32,
        exp: number(4).max(0.0),
        rolls: [number(5), number(6)],
    }
}
