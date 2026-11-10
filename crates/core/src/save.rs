//! Import a game save into a simulation and its multipliers.
//!
//! The upstream calculators read the game's save through Google sign-in and a
//! decode step; this app takes the decoded object as text. The layout follows
//! the upstream decoder in `js/save/loader.js`:
//!
//! - `it.json` wraps the save in `data`; a plain `save.json` is flat. The
//!   server constants (`serverVars`) and the companion list sit beside `data`.
//! - The research state is row-encoded: `Research` is a JSON string holding
//!   rows `[gridLevels, shapeOverlay, occFound, insightProgress, insightLvs,
//!   magData, shapePositions, …]`, where `magData` and `shapePositions` are flat
//!   groups of four numbers.
//! - The research level is the highest `Lv0_<char>[20]` across characters, and
//!   the accumulated EXP is `Exp0_<char>[20]` for a character at that level.
//!
//! The app's own state exports use the decoded names (`gridLevels`, `magData`,
//! …) at the top level, so the importer accepts either shape.
//!
//! Deliberately unsupported, because the save does not carry the inputs the
//! upstream needs: the additive account research bonus tree (`extra_pct`, read
//! from `cachedExtPctExSticker` when the object has it) and the true
//! multipliers from The Button, the arcade, Clam Works, equipment, Menagerie,
//! Jelly and the Royal Guardian outpost. Warned about when the field is absent.

use crate::data::game;
use crate::model::{GridState, Mag, MagKind, Observations, Placement, SimContext, Simulation};
use serde_json::{Map, Value};

/// What an import produced.
#[derive(Clone, Debug)]
pub struct Imported {
    pub simulation: Simulation,
    pub context: SimContext,
    /// Save fields the import could not read or had to drop.
    pub warnings: Vec<String>,
}

/// Why an import could not be read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImportError {
    /// The text is not JSON.
    NotJson(String),
    /// JSON, but not an object that looks like a save.
    NoSaveFields(String),
}

impl std::fmt::Display for ImportError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ImportError::NotJson(detail) => write!(formatter, "not valid JSON: {detail}"),
            ImportError::NoSaveFields(detail) => {
                write!(formatter, "no save fields found: {detail}")
            }
        }
    }
}

impl std::error::Error for ImportError {}

/// Top-level names that mark an object as research state, whichever shape the
/// object uses (`Research` rows, or the decoded names of a state export).
const RESEARCH_FIELDS: [&str; 11] = [
    "Research",
    "researchLevel",
    "researchExp",
    "cachedResearchExp",
    "gridLevels",
    "shapeOverlay",
    "shapePositions",
    "occFound",
    "insightLvs",
    "insightProgress",
    "magData",
];

/// Read a decoded save object (the `SaveData` JSON the game writes) into a run.
pub fn import(save_json: &str) -> Result<Imported, ImportError> {
    let root: Value =
        serde_json::from_str(save_json).map_err(|error| ImportError::NotJson(error.to_string()))?;
    let root = root
        .as_object()
        .ok_or_else(|| ImportError::NoSaveFields("the top level is not an object".into()))?;
    // `it.json` nests the save under `data`; a plain save is the object itself.
    let save = match root.get("data") {
        Some(Value::Object(save)) => save,
        _ => root,
    };

    let mut warnings = Vec::new();
    let rows = research_rows(save, &mut warnings);
    if rows.is_empty()
        && !RESEARCH_FIELDS
            .iter()
            .any(|field| save.contains_key(*field))
    {
        return Err(ImportError::NoSaveFields(format!(
            "expected one of {} at the top level or in the Research rows",
            RESEARCH_FIELDS.join(", ")
        )));
    }

    let characters = characters(save);
    let mut simulation = Simulation::default();

    match research_level(save, &characters) {
        Some(level) => simulation.research_level = level,
        None => warnings.push("researchLevel: not in the save; kept at 1".into()),
    }
    simulation.research_exp = research_exp(save, &characters, simulation.research_level);

    match grid_levels(save, &rows) {
        Some(levels) => simulation.grid = grid(&levels, &mut warnings),
        None => warnings.push("gridLevels: not in the save; the grid stays empty".into()),
    }
    simulation.observations = observations(save, &rows, &mut warnings);
    simulation.mags = mags(save, &rows, &mut warnings);
    simulation.placements = placements(save, &rows, &mut warnings);

    let context = account(save, root, &characters, &simulation, &mut warnings);

    Ok(Imported {
        simulation,
        context,
        warnings,
    })
}

/// A magnifier's `(type, slot)` from the save.
type MagEntry = (Option<f64>, Option<f64>);

/// A shape's `(x, y, rotation)` from the save.
type Position = (Option<f64>, Option<f64>, Option<f64>);

/// One character's research progress, from `Lv0_<n>` and `Exp0_<n>`.
#[derive(Clone, Copy, Debug, Default)]
struct Character {
    research_level: Option<u32>,
    research_exp: Option<f64>,
    divinity: Option<u32>,
}

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

/// The research rows: `Research` is a JSON string in the game's save, but the
/// app's own state exports use a plain array.
fn research_rows(save: &Map<String, Value>, warnings: &mut Vec<String>) -> Vec<Vec<Value>> {
    let Some(field) = save.get("Research") else {
        return Vec::new();
    };
    let parsed = match field {
        Value::String(text) => match serde_json::from_str::<Value>(text) {
            Ok(parsed) => parsed,
            Err(error) => {
                warnings.push(format!(
                    "Research: the research string is not JSON ({error})"
                ));
                return Vec::new();
            }
        },
        other => other.clone(),
    };
    let Value::Array(rows) = parsed else {
        warnings.push("Research: not an array of rows".into());
        return Vec::new();
    };
    rows.into_iter()
        .map(|row| match row {
            Value::Array(cells) => cells,
            _ => Vec::new(),
        })
        .collect()
}

fn characters(save: &Map<String, Value>) -> Vec<Character> {
    let mut characters: Vec<Character> = Vec::new();
    for (key, _) in save {
        let Some(index) = key
            .strip_prefix("Lv0_")
            .and_then(|tail| tail.parse::<usize>().ok())
        else {
            continue;
        };
        if characters.len() <= index {
            characters.resize(index + 1, Character::default());
        }
        let row = parsed_field(save, key);
        let cells = row.as_ref().and_then(Value::as_array);
        characters[index].research_level = cells
            .and_then(|cells| cells.get(20))
            .and_then(number)
            .map(|level| level.max(0.0).round() as u32);
        characters[index].divinity = cells
            .and_then(|cells| cells.get(14))
            .and_then(number)
            .map(|level| level.max(0.0).round() as u32);
        characters[index].research_exp = parsed_field(save, &format!("Exp0_{index}"))
            .as_ref()
            .and_then(Value::as_array)
            .and_then(|cells| cells.get(20))
            .and_then(number);
    }
    characters
}

/// Research level: the decoded field when present, else the highest character
/// level, as the upstream decoder reads it.
fn research_level(save: &Map<String, Value>, characters: &[Character]) -> Option<u32> {
    save.get("researchLevel")
        .and_then(number)
        .map(|level| level.max(0.0).round() as u32)
        .or_else(|| {
            characters
                .iter()
                .filter_map(|character| character.research_level)
                .max()
        })
}

/// Accumulated research EXP: the decoded field, else the EXP of a character at
/// the imported research level.
fn research_exp(save: &Map<String, Value>, characters: &[Character], research_level: u32) -> f64 {
    save.get("researchExp")
        .and_then(number)
        .or_else(|| save.get("cachedResearchExp").and_then(number))
        .or_else(|| {
            characters
                .iter()
                .filter(|character| character.research_level == Some(research_level))
                .filter_map(|character| character.research_exp)
                .max_by(f64::total_cmp)
        })
        .unwrap_or(0.0)
}

/// Per-square levels: the decoded `gridLevels`, else row 0 of `Research`.
fn grid_levels(save: &Map<String, Value>, rows: &[Vec<Value>]) -> Option<Vec<Option<f64>>> {
    field_array(save, "gridLevels", rows, 0)
}

/// Build the grid, padding short arrays and trimming long ones.
fn grid(levels: &[Option<f64>], warnings: &mut Vec<String>) -> GridState {
    let cells = game().cells();
    let mut grid = GridState::empty();
    for (cell, level) in levels.iter().take(cells).enumerate() {
        if let Some(level) = level {
            grid.set(cell, level.max(0.0).round() as u32);
        }
    }
    if levels.len() < cells {
        warnings.push(format!(
            "gridLevels: {} of {cells} squares; the rest stay at level 0",
            levels.len()
        ));
    } else if levels.len() > cells {
        warnings.push(format!(
            "gridLevels: {} entries, only the first {cells} squares are used",
            levels.len()
        ));
    }
    grid
}

/// Observation found flags, insight levels and insight progress.
fn observations(
    save: &Map<String, Value>,
    rows: &[Vec<Value>],
    warnings: &mut Vec<String>,
) -> Observations {
    let mut observations = Observations::default();
    let count = observations.found.len();

    match field_array(save, "occFound", rows, 2) {
        Some(values) => {
            for (obs, value) in values.iter().take(count).enumerate() {
                observations.found[obs] = value.is_some_and(|value| value >= 1.0);
            }
        }
        None => warnings.push("occFound: not in the save; no observation is found".into()),
    }
    match field_array(save, "insightLvs", rows, 4) {
        Some(values) => {
            for (obs, value) in values.iter().take(count).enumerate() {
                if let Some(value) = value {
                    observations.insight_level[obs] = value.max(0.0).round() as u32;
                }
            }
        }
        None => warnings.push("insightLvs: not in the save; insight stays at level 0".into()),
    }
    match field_array(save, "insightProgress", rows, 3) {
        Some(values) => {
            for (obs, value) in values.iter().take(count).enumerate() {
                if let Some(value) = value {
                    observations.insight_exp[obs] = value.max(0.0);
                }
            }
        }
        None => warnings.push("insightProgress: not in the save; insight EXP stays at 0".into()),
    }
    observations
}

/// Magnifiers with the observation they sit on. The save's type codes are
/// `0` magnifying glass, `1` optical monocle, `2` kaleidoscope.
fn mags(save: &Map<String, Value>, rows: &[Vec<Value>], warnings: &mut Vec<String>) -> Vec<Mag> {
    let entries = save.get("magData").and_then(mag_entries).or_else(|| {
        rows.get(5)
            .filter(|flat| !flat.is_empty())
            .and_then(|flat| mag_entries(&Value::Array(flat.clone())))
    });
    let entries = match entries {
        Some(entries) => entries,
        None => {
            warnings.push("magData: not in the save; no magnifier imported".into());
            return Vec::new();
        }
    };

    let slots = game().observations.len();
    let mut mags = Vec::with_capacity(entries.len());
    let mut reported: Vec<usize> = Vec::new();
    for (kind, slot) in entries {
        let kind = match kind.map(|kind| kind.round() as u32) {
            Some(0) => MagKind::Regular,
            Some(1) => MagKind::Monocle,
            Some(2) => MagKind::Kaleidoscope,
            Some(other) => {
                warnings.push(format!("magData: unknown magnifier type {other}; skipped"));
                continue;
            }
            None => {
                warnings.push("magData: an entry carries no type; skipped".into());
                continue;
            }
        };
        let slot = slot.and_then(|slot| (slot >= 0.0).then(|| slot.round() as usize));
        match slot {
            Some(slot) if slot >= slots => {
                if !reported.contains(&slot) {
                    reported.push(slot);
                    warnings.push(format!(
                        "magData: a magnifier sits on observation slot {slot}, outside the {slots} observations modelled; imported unplaced"
                    ));
                }
                mags.push(Mag::new(kind, None));
            }
            _ => mags.push(Mag::new(kind, slot)),
        }
    }
    mags
}

/// Magnifier `(type, slot)` pairs from either the decoded objects or the
/// save's flat groups of four numbers `[x, y, slot, type]`.
fn mag_entries(value: &Value) -> Option<Vec<MagEntry>> {
    let items = value.as_array()?;
    if items.iter().all(Value::is_object) {
        return Some(
            items
                .iter()
                .map(|item| {
                    (
                        item.get("type").and_then(number),
                        item.get("slot").and_then(number),
                    )
                })
                .collect(),
        );
    }
    Some(
        items
            .chunks(4)
            .map(|group| (group.get(3).and_then(number), group.get(2).and_then(number)))
            .collect(),
    )
}

/// Shape placements. The save's `shapePositions` are groups of four numbers
/// `[x, y, rotation, unknown]`, indexed by shape.
fn placements(
    save: &Map<String, Value>,
    rows: &[Vec<Value>],
    warnings: &mut Vec<String>,
) -> Vec<Placement> {
    let positions = save
        .get("shapePositions")
        .and_then(position_entries)
        .or_else(|| {
            rows.get(6)
                .filter(|flat| !flat.is_empty())
                .and_then(|flat| position_entries(&Value::Array(flat.clone())))
        });
    let positions = match positions {
        Some(positions) => positions,
        None => {
            if save.contains_key("shapeOverlay") {
                warnings.push(
                    "shapeOverlay: present, but the placements behind it cannot be rebuilt; no shape is placed"
                        .into(),
                );
            } else {
                warnings.push("shapePositions: not in the save; no shape is placed".into());
            }
            return Vec::new();
        }
    };

    let shapes = game().shapes.len();
    let mut placements = Vec::new();
    for (shape, (x, y, rot)) in positions.iter().enumerate() {
        if shape >= shapes {
            warnings.push(format!(
                "shapePositions: {} entries, only {shapes} shapes exist; the rest are ignored",
                positions.len()
            ));
            break;
        }
        let (Some(x), Some(y)) = (x, y) else {
            continue;
        };
        if !x.is_finite() || !y.is_finite() {
            continue;
        }
        placements.push(Placement {
            shape,
            x: *x,
            y: *y,
            rot: rot.unwrap_or(0.0),
        });
    }
    placements
}

/// Shape `(x, y, rotation)` triples from either the decoded objects or the
/// save's flat groups of four numbers `[x, y, rotation, unknown]`.
fn position_entries(value: &Value) -> Option<Vec<Position>> {
    let items = value.as_array()?;
    if items.iter().all(Value::is_object) {
        return Some(
            items
                .iter()
                .map(|item| {
                    (
                        item.get("x").and_then(number),
                        item.get("y").and_then(number),
                        item.get("rot").and_then(number),
                    )
                })
                .collect(),
        );
    }
    Some(
        items
            .chunks(4)
            .map(|group| {
                (
                    group.first().and_then(number),
                    group.get(1).and_then(number),
                    group.get(2).and_then(number),
                )
            })
            .collect(),
    )
}

/// Account multipliers and flat grid points.
fn account(
    save: &Map<String, Value>,
    root: &Map<String, Value>,
    characters: &[Character],
    simulation: &Simulation,
    warnings: &mut Vec<String>,
) -> SimContext {
    let mut context = SimContext::default();
    let companions = companions(root);
    let weekly_boss = parsed_field(save, "WeeklyBoss").filter(Value::is_object);
    let unique_sushi = unique_sushi(save);

    let server_var = save.get("serverVarResXP").and_then(number).or_else(|| {
        root.get("serverVars")
            .and_then(|vars| vars.get("A_ResXP"))
            .and_then(number)
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

    match save.get("cachedExtPctExSticker").and_then(number) {
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
    match weekly_boss.get(format!("d_{index}")).and_then(number) {
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
    if let Some(options) = parsed_field(save, "OptLacc") {
        // KillroyBonuses(5): `1 + OLA[469] / (150 + OLA[469]) * 0.8`.
        if let Some(killroy) = index(&options, &[469]).and_then(number)
            && killroy > 0.0
        {
            multi *= 1.0 + killroy / (150.0 + killroy) * 0.8;
        }
        // Cglunko_upgBon(11): `OLA[641]` at 1% per level.
        if let Some(cglunko) = index(&options, &[641]).and_then(number) {
            multi *= 1.0 + cglunko / 100.0;
        }
    }
    // Nonstop Studies: `Dream[14]` at 3% per level.
    if let Some(dream) =
        parsed_field(save, "Dream").and_then(|dream| index(&dream, &[14]).and_then(number))
    {
        multi *= 1.0 + 3.0 * dream / 100.0;
    }
    // Fountain_BonTOT(2,16): `Holes[31][2][16]` levels at 1, and the master
    // level `Holes[32][2][16]` scales the base by 1.5 + 0.5 per level.
    if let Some(holes) = parsed_field(save, "Holes") {
        let level = index(&holes, &[31, 2, 16]).and_then(number).unwrap_or(0.0);
        let master = index(&holes, &[32, 2, 16]).and_then(number).unwrap_or(0.0);
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
    if let Some(rift) =
        parsed_field(save, "Rift").and_then(|rift| index(&rift, &[0]).and_then(number))
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
    if let Some(sailing) =
        parsed_field(save, "Sailing").and_then(|sailing| index(&sailing, &[3, 37]).and_then(number))
    {
        points += sailing.round().clamp(0.0, 10.0) as u32;
    }
    // W7 task board: `researchGridPointsTaskLevel` is `TaskZZ2[6][3]`.
    if let Some(task) =
        parsed_field(save, "TaskZZ2").and_then(|tasks| index(&tasks, &[6, 3]).and_then(number))
    {
        points += task.max(0.0) as u32;
    }
    if bundle_ban_k(save) {
        points += 5;
    }
    if let Some(rift) =
        parsed_field(save, "Rift").and_then(|rift| index(&rift, &[0]).and_then(number))
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
    let Some(Value::Object(bundles)) = parsed_field(save, "BundlesReceived") else {
        return false;
    };
    let holder = match bundles.get("h") {
        Some(Value::Object(inner)) => inner,
        _ => &bundles,
    };
    holder
        .get("ban_k")
        .and_then(number)
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
    let sushi = parsed_field(save, "Sushi")?;
    let tiers = index(&sushi, &[5])?.as_array()?;
    let mut count = 0;
    for tier in tiers {
        if number(tier).unwrap_or(0.0) < 0.0 {
            break;
        }
        count += 1;
    }
    Some(count)
}

/// A save field that may be a JSON value or a string holding one, as the
/// upstream `parseSaveKey` helper reads it.
fn parsed_field(save: &Map<String, Value>, key: &str) -> Option<Value> {
    match save.get(key)? {
        Value::String(text) => serde_json::from_str(text).ok(),
        value => Some(value.clone()),
    }
}

/// Index into nested arrays, returning `None` for a missing or non-array step.
fn index<'a>(value: &'a Value, path: &[usize]) -> Option<&'a Value> {
    let mut current = value;
    for step in path {
        current = current.as_array()?.get(*step)?;
    }
    Some(current)
}

/// A number, or a string holding one; the game writes some values as strings.
fn number(value: &Value) -> Option<f64> {
    match value {
        Value::Number(number) => number.as_f64(),
        Value::String(text) => text.trim().parse().ok(),
        _ => None,
    }
}

/// A grid-shaped array of optional numbers.
fn levels(value: &Value) -> Option<Vec<Option<f64>>> {
    value
        .as_array()
        .map(|items| items.iter().map(number).collect())
}

/// A decoded field when present, else the matching `Research` row.
fn field_array(
    save: &Map<String, Value>,
    key: &str,
    rows: &[Vec<Value>],
    row: usize,
) -> Option<Vec<Option<f64>>> {
    let values = save.get(key).and_then(levels).or_else(|| {
        rows.get(row)
            .filter(|cells| !cells.is_empty())
            .map(|cells| cells.iter().map(number).collect())
    })?;
    (!values.is_empty()).then_some(values)
}
