//! The account side of a save: the multipliers and flat grid points that live
//! outside the research grid.

use super::Character;
use super::json;
use crate::model::{GridState, SimContext, Simulation};
use serde_json::{Map, Value};

/// The companion roster: ids owned, and the ids at level two.
#[derive(Clone, Debug, Default)]
struct Companions {
    owned: Vec<u32>,
    enhanced: Vec<u32>,
}

impl Companions {
    fn owns(&self, id: u32) -> bool {
        self.owned.contains(&id)
    }

    fn is_enhanced(&self, id: u32) -> bool {
        self.enhanced.contains(&id)
    }
}

/// Account multipliers and flat grid points.
pub(crate) fn account(
    save: &Map<String, Value>,
    root: &Map<String, Value>,
    characters: &[Character],
    simulation: &Simulation,
    warnings: &mut Vec<String>,
) -> SimContext {
    let mut context = SimContext::default();
    let companions = companions(root);
    let weekly_boss = json::parsed_field(save, "WeeklyBoss").filter(Value::is_object);
    let unique_sushi = unique_sushi(save);

    let server_var = save
        .get("serverVarResXP")
        .and_then(json::number)
        .or_else(|| {
            root.get("serverVars")
                .and_then(|vars| vars.get("A_ResXP"))
                .and_then(json::number)
        });
    match server_var {
        Some(value) => context.server_var_res_xp = value.max(1.0),
        None => warnings.push("serverVarResXP: not in the save; kept at 1.0".into()),
    }

    match all_bonus_multi(
        companions.as_ref(),
        weekly_boss.as_ref(),
        unique_sushi,
        characters,
        &simulation.grid,
    ) {
        Ok(multi) => context.all_bonus_multi = multi,
        Err(missing) => warnings.push(format!(
            "allBonusMulti: {missing} missing from the save; kept at 1.0"
        )),
    }

    match save.get("cachedExtPctExSticker").and_then(json::number) {
        Some(pct) => context.extra_pct = pct,
        None => warnings.push(
            "extra_pct: the save's account additive research bonus (cachedExtPctExSticker) is not derivable from it; kept at 0.0"
                .into(),
        ),
    }

    context.extra_multiplier =
        extra_multiplier(save, companions.as_ref(), unique_sushi, characters);
    context.bonus_grid_points =
        grid_points_bonus(save, companions.as_ref(), unique_sushi, characters);
    context
}

/// `calcAllBonusMultiWith` from `research-math.js`: companion 55, companion 0's
/// divinity unlock, the weekly-boss cloud bonuses and the sushi RoG 53.
fn all_bonus_multi(
    companions: Option<&Companions>,
    weekly_boss: Option<&Value>,
    unique_sushi: Option<u32>,
    characters: &[Character],
    grid: &GridState,
) -> Result<f64, String> {
    let (Some(companions), Some(weekly_boss), Some(unique_sushi)) =
        (companions, weekly_boss, unique_sushi)
    else {
        let mut missing = Vec::new();
        if companions.is_none() {
            missing.push("companion list");
        }
        if weekly_boss.is_none() {
            missing.push("WeeklyBoss");
        }
        if unique_sushi.is_none() {
            missing.push("Sushi");
        }
        return Err(missing.join(", "));
    };
    // CompanionDB entries 55 (`w7b11`) and 0 (`babaMummy`) from the upstream
    // game data; 55 gives +15%, +25% when the companion is level two.
    let companion_55 = match (companions.owns(55), companions.is_enhanced(55)) {
        (true, true) => 25.0,
        (true, false) => 15.0,
        (false, _) => 0.0,
    };
    let companion_0 = if companions.owns(0)
        && characters
            .first()
            .and_then(|character| character.divinity)
            .is_some_and(|divinity| divinity >= 2)
        && grid.level(173) > 0
    {
        5.0
    } else {
        0.0
    };
    let clouds =
        cloud_bonus(weekly_boss, 71) + cloud_bonus(weekly_boss, 72) + cloud_bonus(weekly_boss, 76);
    // RoG 53 (`ROG_BONUS_QTY[53]` in `js/stats/data/w7/sushi.js`).
    let rog_53 = if unique_sushi > 53 { 1.0 } else { 0.0 };
    Ok(1.0 + (companion_55 + companion_0 + clouds + rog_53) / 100.0)
}

/// A weekly-boss cloud bonus is present when the save marks its map as cleared
/// (`d_<n>` equal to `-1`).
fn cloud_bonus(weekly_boss: &Value, index: usize) -> f64 {
    match weekly_boss.get(format!("d_{index}")).and_then(json::number) {
        Some(-1.0) => 1.0,
        _ => 0.0,
    }
}

/// The true account multipliers the save lets us recover, as the product
/// `simTotalExpWith` applies: Killroy, nonstop studies, Cglunko, the fountain,
/// companions 52/54/153, sushi RoG 0 and research mastery.
fn extra_multiplier(
    save: &Map<String, Value>,
    companions: Option<&Companions>,
    unique_sushi: Option<u32>,
    characters: &[Character],
) -> f64 {
    let mut multi = 1.0;
    if let Some(options) = json::parsed_field(save, "OptLacc") {
        // KillroyBonuses(5): `1 + OLA[469] / (150 + OLA[469]) * 0.8`.
        if let Some(killroy) = json::index(&options, &[469]).and_then(json::number)
            && killroy > 0.0
        {
            multi *= 1.0 + killroy / (150.0 + killroy) * 0.8;
        }
        // Cglunko_upgBon(11): `OLA[641]` at 1% per level.
        if let Some(cglunko) = json::index(&options, &[641]).and_then(json::number) {
            multi *= 1.0 + cglunko / 100.0;
        }
    }
    // Nonstop Studies: `Dream[14]` at 3% per level.
    if let Some(dream) = json::parsed_field(save, "Dream")
        .and_then(|dream| json::index(&dream, &[14]).and_then(json::number))
    {
        multi *= 1.0 + 3.0 * dream / 100.0;
    }
    // Fountain_BonTOT(2,16): `Holes[31][2][16]` levels at 1, and the master
    // level `Holes[32][2][16]` scales the base by 1.5 + 0.5 per level.
    if let Some(holes) = json::parsed_field(save, "Holes") {
        let level = json::index(&holes, &[31, 2, 16])
            .and_then(json::number)
            .unwrap_or(0.0);
        let master = json::index(&holes, &[32, 2, 16])
            .and_then(json::number)
            .unwrap_or(0.0);
        let bonus = if master <= 0.0 {
            level
        } else {
            (1.5 + 0.5 * master) * level
        };
        multi *= 1.0 + bonus.round() / 100.0;
    }
    if let Some(companions) = companions {
        // CompanionDB 52 (`w7b5`) +0.5x/+0.8x, 153 (`rift5`) +1x with a
        // second level adding another +1x, 54 (`w7b6b`) +0.5x at level two.
        let bonus = |id: u32, base: f64, enhanced: f64| match companions.owns(id) {
            true if companions.is_enhanced(id) => enhanced,
            true => base,
            false => 0.0,
        };
        let companion_52 = bonus(52, 0.5, 0.8);
        let companion_153 = bonus(153, 1.0, 1.0);
        let companion_153_level_two = if companions.is_enhanced(153) {
            1.0
        } else {
            0.0
        };
        multi *= (1.0 + companion_52) * (1.0 + companion_153 + companion_153_level_two);
        if companions.is_enhanced(54) {
            multi *= 1.5;
        }
    }
    // RoG 0 (`ROG_BONUS_QTY[0]`) is +100% once any sushi tier is unlocked.
    if unique_sushi.is_some_and(|count| count > 0) {
        multi *= 2.0;
    }
    // Research skill mastery: rift 15 unlocks it, 750 summed research levels
    // add 10%.
    if let Some(rift) = json::parsed_field(save, "Rift")
        .and_then(|rift| json::index(&rift, &[0]).and_then(json::number))
        && rift >= 15.0
    {
        let total: u32 = characters
            .iter()
            .filter_map(|character| character.research_level)
            .sum();
        if total >= 750 {
            multi *= 1.1;
        }
    }
    multi
}

/// Flat research grid points from the sources a save carries: companion 153,
/// sushi RoG 3/13, the sailing artifact, the W7 task, the `ban_k` bundle and
/// research mastery. Jelly rewards, the event shop thesis and the minehead
/// upgrades need game data the save does not carry and stay out.
fn grid_points_bonus(
    save: &Map<String, Value>,
    companions: Option<&Companions>,
    unique_sushi: Option<u32>,
    characters: &[Character],
) -> u32 {
    let mut points = 0_u32;
    if let Some(companions) = companions
        && companions.owns(153)
    {
        points += 10;
        if companions.is_enhanced(153) {
            points += 5;
        }
    }
    if let Some(unique) = unique_sushi {
        // `ROG_BONUS_QTY[3]` and `ROG_BONUS_QTY[13]` in the upstream game data.
        if unique > 3 {
            points += 2;
        }
        if unique > 13 {
            points += 1;
        }
    }
    // Sailing artifact 37 counts up to ten.
    if let Some(sailing) = json::parsed_field(save, "Sailing")
        .and_then(|sailing| json::index(&sailing, &[3, 37]).and_then(json::number))
    {
        points += sailing.round().clamp(0.0, 10.0) as u32;
    }
    // W7 task board: `researchGridPointsTaskLevel` is `TaskZZ2[6][3]`.
    if let Some(task) = json::parsed_field(save, "TaskZZ2")
        .and_then(|tasks| json::index(&tasks, &[6, 3]).and_then(json::number))
    {
        points += task.max(0.0) as u32;
    }
    if bundle_ban_k(save) {
        points += 5;
    }
    if let Some(rift) = json::parsed_field(save, "Rift")
        .and_then(|rift| json::index(&rift, &[0]).and_then(json::number))
        && rift >= 15.0
    {
        let total: u32 = characters
            .iter()
            .filter_map(|character| character.research_level)
            .sum();
        if total >= 300 {
            points += 1;
        }
        if total >= 500 {
            points += 1;
        }
    }
    points
}

/// The `ban_k` bundle, which grants five grid points.
fn bundle_ban_k(save: &Map<String, Value>) -> bool {
    let Some(Value::Object(bundles)) = json::parsed_field(save, "BundlesReceived") else {
        return false;
    };
    let holder = match bundles.get("h") {
        Some(Value::Object(inner)) => inner,
        _ => &bundles,
    };
    holder
        .get("ban_k")
        .and_then(json::number)
        .is_some_and(|value| value == 1.0)
}

/// Companion roster from `companion.l`; entries are `id,…,level2,…` strings.
fn companions(root: &Map<String, Value>) -> Option<Companions> {
    let list = root.get("companion")?.get("l")?;
    let entries: Vec<String> = match list {
        Value::Array(items) => items
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect(),
        Value::Object(fields) => {
            let mut keys: Vec<&String> = fields.keys().collect();
            keys.sort_by_key(|key| key.parse::<u64>().unwrap_or(u64::MAX));
            keys.iter()
                .filter_map(|key| fields.get(*key).and_then(Value::as_str))
                .map(str::to_owned)
                .collect()
        }
        _ => return None,
    };
    let mut roster = Companions::default();
    for entry in entries {
        let fields: Vec<&str> = entry.split(',').collect();
        let Some(id) = fields.first().and_then(|id| id.trim().parse::<u32>().ok()) else {
            continue;
        };
        roster.owned.push(id);
        let level_two = fields
            .get(4)
            .and_then(|flag| flag.trim().parse::<f64>().ok())
            .unwrap_or(0.0);
        if level_two >= 1.0 {
            roster.enhanced.push(id);
        }
    }
    Some(roster)
}

/// Unique sushi tiers unlocked (`computeUniqueSushi` counts `Sushi[5]` from the
/// first negative tier).
fn unique_sushi(save: &Map<String, Value>) -> Option<u32> {
    let sushi = json::parsed_field(save, "Sushi")?;
    let tiers = json::index(&sushi, &[5])?.as_array()?;
    let mut count = 0;
    for tier in tiers {
        if json::number(tier).unwrap_or(0.0) < 0.0 {
            break;
        }
        count += 1;
    }
    Some(count)
}
