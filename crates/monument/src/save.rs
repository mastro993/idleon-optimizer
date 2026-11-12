//! The raw save fields the Monument formulas read.
//!
//! The upstream calculator receives one decoded save object with every system's
//! rows already parsed. This type mirrors that object for the monument helpers:
//! arrays of `serde_json::Value` where a row mixes numbers and strings, and
//! small numeric accessors so the formulas stay readable.

use serde_json::Value;
use std::collections::BTreeSet;

/// The decoded save, narrowed to the rows the Monument section needs.
#[derive(Clone, Debug, Default)]
pub struct Save {
    /// `Holes`: cavern levels, monument levels, milestones and fountain upgrades.
    pub holes: Vec<Value>,
    /// `Research`: row 7 carries the Jelly obstruction count.
    pub research: Vec<Value>,
    /// `Lv0_<n>`: the account-skill total the measurement bonus uses.
    pub lv0: Vec<Value>,
    /// `OptLacc` (or `OptionsListAccount`), the account option list.
    pub options: Vec<Value>,
    /// `Spelunk`: row 18 carries the Legend talent levels.
    pub spelunk: Vec<Value>,
    /// `Summon`: row 1 lists normal victories.
    pub summon: Vec<Value>,
    /// `ArcadeUpg`: purchased arcade levels.
    pub arcade: Vec<Value>,
    /// `Ninja`: row 107 slot 8 is the pristine charm.
    pub ninja: Vec<Value>,
    /// `Sailing`: row 3 carries artifact rarity.
    pub sailing: Vec<Value>,
    /// `TaskZZ0`-`TaskZZ5`, indexed by task group.
    pub tasks: Vec<Value>,
    /// `GemItemsPurchased`: row 11 feeds the winner bonus.
    pub gem_items: Vec<Value>,
    /// `Arcane`: the tesseract levels behind Emperor bonuses.
    pub arcane: Vec<Value>,
    /// `Compass`: row 0 lists compass levels.
    pub compass: Vec<Value>,
    /// `Gaming`: row 12 is the superbit list the reward charge reads.
    pub gaming: Vec<Value>,
    /// `Sushi`: row 5 lists the discovered tiers behind the Meritoc RoG bonus.
    pub sushi: Vec<Value>,
    /// `KLA_<character>`: the cleared-map rows the Meritoc vote checks.
    pub kla: Vec<Value>,
    /// `EquipOrder_<n>` per character, for the permanent set fallback.
    pub equipment: Vec<Value>,
    /// `AchieveReg`: `-1` marks a completed achievement.
    pub achievements: Vec<f64>,
    /// `BundlesReceived`.
    pub bundles: Option<Value>,
    /// Companion ids owned, from the `it.json` wrapper's `companion.l`.
    pub companions: BTreeSet<u32>,
    /// Companion ids at level two.
    pub enhanced_companions: BTreeSet<u32>,
    /// Whether the wrapper carried a companion roster at all.
    pub companion_available: bool,
    /// `Research` row 7, the Minehead state the Clutch Overtime Block reads.
    pub state_r7: Vec<Value>,
    /// Whether row 7 exists.
    pub state_r7_available: bool,
}

impl Save {
    /// Build the monument view from the imported save object and the
    /// `it.json` companion roster beside it.
    pub fn from_parts(save: &Value, companion: Option<&Value>) -> Self {
        let research = array(save, "Research");
        let state_r7 = research.get(7).and_then(Value::as_array).cloned();
        let options = array(save, "OptionsListAccount");
        let options = if options.is_empty() {
            array(save, "OptLacc")
        } else {
            options
        };
        let roster = idleon_save::Roster::read(companion);
        Self {
            holes: array(save, "Holes"),
            state_r7_available: state_r7.is_some(),
            state_r7: state_r7.unwrap_or_default(),
            research,
            lv0: rows(save, "Lv0_"),
            options,
            spelunk: array(save, "Spelunk"),
            summon: array(save, "Summon"),
            arcade: array(save, "ArcadeUpg"),
            ninja: array(save, "Ninja"),
            sailing: array(save, "Sailing"),
            tasks: task_groups(save),
            gem_items: array(save, "GemItemsPurchased"),
            arcane: array(save, "Arcane"),
            compass: array(save, "Compass"),
            gaming: array(save, "Gaming"),
            sushi: array(save, "Sushi"),
            kla: rows(save, "KLA_"),
            equipment: rows(save, "EquipOrder_"),
            achievements: array(save, "AchieveReg").iter().map(number).collect(),
            bundles: idleon_save::json::parsed(save, "BundlesReceived"),
            companions: roster.owned,
            enhanced_companions: roster.enhanced,
            companion_available: roster.available,
        }
    }
}

impl Save {
    /// `holesData[row][column]`.
    pub fn hole(&self, row: usize, column: usize) -> f64 {
        cell(&self.holes, row, column)
    }

    /// `holesData[row][outer][inner]`, for the nested fountain rows.
    pub fn hole_nested(&self, row: usize, outer: usize, inner: usize) -> f64 {
        self.holes
            .get(row)
            .and_then(|row| at(row, outer))
            .and_then(|row| at(row, inner))
            .map(number)
            .unwrap_or(0.0)
    }

    /// `olaData[index]`.
    pub fn option(&self, index: usize) -> f64 {
        flat(&self.options, index)
    }

    /// `olaData[index]` as text, for the option-list set names.
    pub fn option_text(&self, index: usize) -> &str {
        self.options
            .get(index)
            .and_then(Value::as_str)
            .unwrap_or("")
    }

    /// `spelunkData[row][column]`.
    pub fn spelunk(&self, row: usize, column: usize) -> f64 {
        cell(&self.spelunk, row, column)
    }

    /// The normal summoning victories, `summonData[1]`.
    pub fn summon_wins(&self) -> &[Value] {
        self.summon
            .get(1)
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// `arcadeUpgData[index]`.
    pub fn arcade(&self, index: usize) -> f64 {
        flat(&self.arcade, index)
    }

    /// `ninjaData[row][column]`.
    pub fn ninja(&self, row: usize, column: usize) -> f64 {
        cell(&self.ninja, row, column)
    }

    /// `sailingData[row][column]`.
    pub fn sailing(&self, row: usize, column: usize) -> f64 {
        cell(&self.sailing, row, column)
    }

    /// `tasksGlobalData[group][row][column]`.
    pub fn task(&self, group: usize, row: usize, column: usize) -> f64 {
        self.tasks
            .get(group)
            .and_then(|group| at(group, row))
            .and_then(|row| at(row, column))
            .map(number)
            .unwrap_or(0.0)
    }

    /// `gemItemsData[index]`.
    pub fn gem_item(&self, index: usize) -> f64 {
        flat(&self.gem_items, index)
    }

    /// `arcaneData[index]`.
    pub fn arcane(&self, index: usize) -> f64 {
        flat(&self.arcane, index)
    }

    /// `compassData[row][column]`.
    pub fn compass(&self, row: usize, column: usize) -> f64 {
        cell(&self.compass, row, column)
    }

    /// `gamingData[row]` as text, for the encoded superbit list.
    pub fn gaming_text(&self, row: usize) -> String {
        self.gaming
            .get(row)
            .map(|cell| match cell {
                Value::String(text) => text.clone(),
                other => other.to_string(),
            })
            .unwrap_or_default()
    }

    /// `computeUniqueSushi`: the discovered tiers, counted until the first miss.
    pub fn unique_sushi(&self) -> usize {
        let tiers = self.sushi.get(5).and_then(Value::as_array);
        let Some(tiers) = tiers else {
            return 0;
        };
        let mut count = 0;
        for tier in tiers {
            if number(tier) < 0.0 {
                break;
            }
            count += 1;
        }
        count
    }

    /// `klaData[character][map]`: a number, or a one-element row's first cell.
    /// A missing or null cell is `None`, as the upstream `map250 == null` check
    /// treats both as unknown; an empty row reads as zero kills left.
    pub fn kla_map(&self, character: usize, map: usize) -> Option<f64> {
        match self.kla.get(character).and_then(|row| at(row, map))? {
            Value::Null => None,
            Value::Array(cells) => Some(cells.first().map(number).unwrap_or(0.0)),
            cell => Some(number(cell)),
        }
    }

    /// `research[row][column]`.
    pub fn research(&self, row: usize, column: usize) -> f64 {
        cell(&self.research, row, column)
    }

    /// The sum of a character's skills 1-21, the Account Skills LV tome slot.
    pub fn account_skills(&self) -> f64 {
        self.lv0
            .iter()
            .map(|row| {
                (1..=21)
                    .filter_map(|column| at(row, column))
                    .map(|value| number(value).max(0.0))
                    .sum::<f64>()
            })
            .sum()
    }

    /// `achieveRegData[index]`.
    pub fn achievement(&self, index: usize) -> f64 {
        self.achievements.get(index).copied().unwrap_or(0.0)
    }

    /// Whether a bundle was received.
    pub fn bundle(&self, key: &str) -> bool {
        self.bundles
            .as_ref()
            .and_then(|bundles| at_key(bundles, key))
            .map(number)
            .unwrap_or(0.0)
            == 1.0
    }

    /// Whether the save owns a companion.
    pub fn owns_companion(&self, id: u32) -> bool {
        self.companions.contains(&id)
    }

    /// Whether a companion is at level two.
    pub fn companion_enhanced(&self, id: u32) -> bool {
        self.enhanced_companions.contains(&id)
    }

    /// `EquipOrder_<character>`.
    pub fn equipment(&self, character: usize) -> Option<&Value> {
        self.equipment.get(character)
    }
}

/// A save field that must be an array.
fn array(save: &Value, key: &str) -> Vec<Value> {
    match idleon_save::json::parsed(save, key) {
        Some(Value::Array(items)) => items,
        _ => Vec::new(),
    }
}

/// Every `<prefix><n>` field, ordered by its numeric suffix.
fn rows(save: &Value, prefix: &str) -> Vec<Value> {
    let Some(fields) = save.as_object() else {
        return Vec::new();
    };
    let mut indexed: Vec<(usize, String)> = fields
        .keys()
        .filter_map(|key| {
            key.strip_prefix(prefix)
                .and_then(|tail| tail.parse::<usize>().ok())
                .map(|index| (index, key.clone()))
        })
        .collect();
    indexed.sort_by_key(|(index, _)| *index);
    indexed
        .into_iter()
        .map(|(_, key)| idleon_save::json::parsed(save, &key).unwrap_or(Value::Null))
        .collect()
}

/// `TaskZZ0`-`TaskZZ5`, the six task groups `tasksGlobalData` holds.
fn task_groups(save: &Value) -> Vec<Value> {
    (0..=5)
        .map(|group| {
            idleon_save::json::parsed(save, &format!("TaskZZ{group}")).unwrap_or(Value::Null)
        })
        .collect()
}

/// `rows[row][column]` as a number; a missing row or cell reads as zero.
fn cell(rows: &[Value], row: usize, column: usize) -> f64 {
    rows.get(row)
        .and_then(|row| at(row, column))
        .map(number)
        .unwrap_or(0.0)
}

/// `cells[index]` as a number; a missing cell reads as zero.
fn flat(cells: &[Value], index: usize) -> f64 {
    cells.get(index).map(number).unwrap_or(0.0)
}

/// Index into an array, or a numeric string key in an object.
pub(crate) fn at(value: &Value, index: usize) -> Option<&Value> {
    match value {
        Value::Array(items) => items.get(index),
        Value::Object(fields) => fields.get(&index.to_string()),
        _ => None,
    }
}

/// Index into an array, or a named field in an object.
pub(crate) fn at_key<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    match value {
        Value::Object(fields) => fields.get(key),
        Value::Array(items) => key.parse::<usize>().ok().and_then(|i| items.get(i)),
        _ => None,
    }
}

/// A number, or a string holding one; the game writes some values as strings.
pub(crate) fn number(value: &Value) -> f64 {
    match value {
        Value::Number(number) => number.as_f64().unwrap_or(0.0),
        Value::String(text) => text.trim().parse().unwrap_or(0.0),
        _ => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use crate::tests::fixture;

    #[test]
    fn the_fixture_exposes_the_monument_rows() {
        let save = fixture();
        assert_eq!(save.hole(14, 0), 121945.0);
        assert_eq!(save.hole(15, 8), 12122.0);
        assert_eq!(save.hole_nested(31, 0, 13), 8.0);
        assert_eq!(save.option(453), 1.0);
        assert_eq!(save.spelunk(18, 29), 5.0);
        assert_eq!(save.research(7, 9), 32.0);
        assert_eq!(save.arcade(42), 100.0);
        assert_eq!(save.ninja(107, 8), 1.0);
        assert_eq!(save.sailing(3, 32), 5.0);
        assert_eq!(save.task(2, 5, 4), 10.0);
        assert_eq!(save.gem_item(11), 5.0);
        assert_eq!(save.arcane(48), 50.0);
        assert_eq!(save.compass(0, 55), 200.0);
        assert!(save.gaming_text(12).contains('E'));
        assert!(save.unique_sushi() > 0);
        assert!(save.kla_map(0, 250).is_some());
        assert!(save.bundle("ban_i"));
        assert!(!save.companion_available);
        assert!(save.state_r7_available);
        assert!(save.account_skills() > 0.0);
    }
}
