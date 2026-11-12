//! SA-04, the game-input sandbox and the change ledger it feeds.
//!
//! Ported from `createFormulaInputSandbox` in
//! `js/stats/formula-inputs.js`: the page keeps the saved inputs and a set of
//! pending edits, resolves every descriptor against the edited save, and
//! `_renderInputChanges` in `stats-all.html` lists what changed next to the
//! formulas whose value moved. Upstream finds an editable control in every
//! formula node whose `source` names a writable level-table row, talent or
//! equipment cell, and writes the edit back through `applyCombatSetupDetailed`.
//! This build has no save writer, so the sandbox offers the saved cells its
//! ported formulas actually read — the vault, gem-shop, talent, guild and stamp
//! levels the eight carry-capacity rows scale by — and splices each edit into a
//! clone of the save. The imported save is never touched.
//!
//! `formulaDifference` in `js/stats/formula-comparison.js` is the other half:
//! [`Sheet::affected`] reports the descriptors whose value a pending edit moved,
//! with the delta and percentage upstream prints in the affected-formula list.

use std::collections::BTreeMap;

use idleon_common::formula::ValueFormat;
use idleon_save::{Save, at, number};
use idleon_stats_data as data;
use serde_json::{Map, Value};

use super::breakdowns::Sheet;

/// The save cell one game input writes: a key and the indexes that reach it.
#[derive(Clone, Debug)]
struct Cell {
    key: String,
    path: Vec<usize>,
}

/// One editable saved game input: a level the ported formulas scale by.
#[derive(Clone, Debug)]
pub struct GameInput {
    /// The stable id the ledger and the window's automation bridge use.
    pub id: String,
    /// What the input belongs to, such as `Talent 78 (Character 1)`.
    pub name: String,
    /// What the number is, such as `Level`.
    pub label: &'static str,
    /// The value the imported save carries.
    pub saved: f64,
    /// The greatest value the save allows, such as a talent's own cap.
    pub max: Option<f64>,
    cell: Cell,
}

impl GameInput {
    /// `name · label`, as the ledger row names the input.
    pub fn title(&self) -> String {
        format!("{} · {}", self.name, self.label)
    }

    /// The value as the ledger prints it.
    pub fn show(value: f64) -> String {
        ValueFormat::Raw.format(value)
    }
}

/// The hypothetical sandbox: the imported save, the inputs its ported formulas
/// read, and the pending edits.
#[derive(Debug)]
pub struct Sandbox {
    save: Save,
    inputs: Vec<GameInput>,
    edits: BTreeMap<String, f64>,
}

impl Sandbox {
    /// The sandbox a freshly imported save opens with: the input list of
    /// `character`, and no edits.
    pub fn new(save: &Save, character: usize) -> Self {
        Self {
            save: save.clone(),
            inputs: catalog(save, character),
            edits: BTreeMap::new(),
        }
    }

    /// Follow a new save or character without losing the edits it still reads.
    ///
    /// An edit whose cell the new input list no longer carries is dropped, so
    /// the ledger and the save the breakdowns resolve against always agree.
    pub fn adopt(&mut self, save: &Save, character: usize) {
        self.save = save.clone();
        self.inputs = catalog(save, character);
        self.edits
            .retain(|id, _| self.inputs.iter().any(|input| &input.id == id));
    }

    /// The inputs the ported formulas read, in ledger order.
    pub fn inputs(&self) -> &[GameInput] {
        &self.inputs
    }

    /// How many inputs carry a pending edit.
    pub fn count(&self) -> usize {
        self.edits.len()
    }

    /// Whether `id` carries a pending edit.
    pub fn edited(&self, id: &str) -> bool {
        self.edits.contains_key(id)
    }

    /// One input's current value: the edit when there is one, else the save's.
    pub fn value(&self, id: &str) -> Option<f64> {
        let input = self.input(id)?;
        Some(self.edits.get(id).copied().unwrap_or(input.saved))
    }

    /// The edit `id` now carries, after upstream's `set` checks.
    ///
    /// An edit back to the saved value clears itself; a non-finite, fractional
    /// or out-of-range value is refused with the message upstream shows instead
    /// of applying it.
    pub fn set(&mut self, id: &str, value: f64) -> Result<(), String> {
        let (saved, max) = match self.input(id) {
            Some(input) => (input.saved, input.max),
            None => return Err("Unknown editable game input.".into()),
        };
        if !value.is_finite() {
            return Err("Enter a finite number.".into());
        }
        if value.fract() != 0.0 {
            return Err("Enter a whole level.".into());
        }
        if value < 0.0 {
            return Err("Minimum: 0.".into());
        }
        if let Some(max) = max
            && value > max
        {
            return Err(format!("Maximum: {max}."));
        }
        if value == saved {
            self.edits.remove(id);
        } else {
            self.edits.insert(id.to_string(), value);
        }
        Ok(())
    }

    /// Put one input back to the saved value.
    pub fn reset(&mut self, id: &str) {
        self.edits.remove(id);
    }

    /// Put every input back to the saved value, which is all upstream's
    /// `Reset all edits` does.
    pub fn reset_all(&mut self) {
        self.edits.clear();
    }

    /// The save the breakdowns resolve against: the imported one with the
    /// pending edits spliced in.
    pub fn current(&self) -> Save {
        if self.edits.is_empty() {
            return self.save.clone();
        }
        let mut cells: Map<String, Value> = Map::new();
        for (id, value) in &self.edits {
            let Some(input) = self.input(id) else {
                continue;
            };
            let cell = cells
                .entry(input.cell.key.clone())
                .or_insert_with(|| self.save.get(&input.cell.key).unwrap_or(Value::Null));
            write(cell, &input.cell.path, *value);
        }
        let mut root = self.save.root().clone();
        if !root.is_object() {
            return self.save.clone();
        }
        let fields = match root.get_mut("data").and_then(Value::as_object_mut) {
            Some(fields) => fields,
            None => root.as_object_mut().expect("the root is an object"),
        };
        for (key, cell) in cells {
            fields.insert(key, cell);
        }
        Save::new(root)
    }

    /// Upstream's sandbox status line.
    pub fn status(&self) -> String {
        match self.count() {
            0 => "Game-input sandbox: no edits.".into(),
            1 => "Hypothetical: 1 edited game input.".into(),
            count => format!("Hypothetical: {count} edited game inputs."),
        }
    }

    /// One input by its id.
    fn input(&self, id: &str) -> Option<&GameInput> {
        self.inputs.iter().find(|input| input.id == id)
    }
}

/// One descriptor whose value the pending edits moved, from upstream's
/// `formulaDifference`.
#[derive(Debug, PartialEq)]
pub struct Affected {
    /// The descriptor's upstream id.
    pub id: String,
    /// The descriptor's upstream name.
    pub name: String,
    /// The value as the imported save resolves it.
    pub saved: String,
    /// The value as the pending edits resolve it.
    pub current: String,
    /// The raw difference, when both sides are numbers.
    pub delta: Option<f64>,
    /// The difference as a percentage of the saved value, when it is not zero.
    pub percent: Option<f64>,
}

impl Affected {
    /// The affected-formula line: `saved → current (Δ +x, +y%)`.
    pub fn line(&self) -> String {
        match (self.delta, self.percent) {
            (Some(delta), percent) => {
                let delta = if delta > 0.0 {
                    format!("+{}", ValueFormat::Raw.format(delta))
                } else {
                    ValueFormat::Raw.format(delta)
                };
                let percent = percent
                    .map(|percent| format!(", {}", ValueFormat::Percent.format(percent)))
                    .unwrap_or_else(|| "; % unavailable: saved zero".into());
                format!("{} → {} (Δ {delta}{percent})", self.saved, self.current)
            }
            _ => format!("{} → {}", self.saved, self.current),
        }
    }

    /// The saved value as a descriptor row annotates it: `Saved: 35,000 (+2.5%)`.
    pub fn saved_note(&self) -> String {
        match self.percent {
            Some(percent) => format!(
                "Saved: {} ({})",
                self.saved,
                ValueFormat::Percent.format(percent)
            ),
            None => format!("Saved: {}", self.saved),
        }
    }
}

impl Sheet {
    /// The descriptors whose value `baseline` or this sheet moved, in registry
    /// order. Upstream calls this for every edit and lists the result under
    /// `Affected formulas`.
    pub fn affected(&self, baseline: &Sheet) -> Vec<Affected> {
        self.descriptors()
            .iter()
            .filter_map(|row| {
                let saved = baseline.get(row.id)?;
                let moved = row.resolution.value() != saved.resolution.value()
                    || row.display_value() != saved.display_value();
                moved.then(|| Affected {
                    id: row.id.to_string(),
                    name: row.name.to_string(),
                    saved: saved
                        .display_value()
                        .unwrap_or_else(|| "not modelled".into()),
                    current: row.display_value().unwrap_or_else(|| "not modelled".into()),
                    delta: match (saved.resolution.value(), row.resolution.value()) {
                        (Some(before), Some(after)) => Some(after - before),
                        _ => None,
                    },
                    percent: match (saved.resolution.value(), row.resolution.value()) {
                        (Some(before), Some(after)) if before != 0.0 => {
                            Some((after - before) / before.abs() * 100.0)
                        }
                        _ => None,
                    },
                })
            })
            .collect()
    }
}

/// Every input the ported rows read, in the order the ledger lists them.
fn catalog(save: &Save, character: usize) -> Vec<GameInput> {
    let mut inputs = Vec::new();
    inputs.extend(vault(save));
    inputs.extend(gem_shop(save));
    inputs.extend(talents(save, character));
    inputs.extend(guild(save));
    inputs.extend(stamps(save));
    inputs
}

/// The vault upgrade `Carry Capacity` reads at index 11.
fn vault(save: &Save) -> Option<GameInput> {
    input(
        save,
        "upg-vault-11".into(),
        "Vault upgrade 11".into(),
        "Level",
        Cell {
            key: "UpgVault".into(),
            path: vec![11],
        },
        None,
    )
}

/// The gem-shop capacity purchases the 25% per purchase reads at index 58.
fn gem_shop(save: &Save) -> Option<GameInput> {
    input(
        save,
        "gem-items-58".into(),
        "Gem shop capacity".into(),
        "Purchases",
        Cell {
            key: "GemItemsPurchased".into(),
            path: vec![58],
        },
        None,
    )
}

/// The two talents the capacity rows read, capped by the character's `SM_` row.
fn talents(save: &Save, character: usize) -> Vec<GameInput> {
    [
        (78, "Talent 78 · Mat Carry Cap"),
        (634, "Talent 634 · Carry Cap"),
    ]
    .into_iter()
    .filter_map(|(index, name)| {
        let max = read(
            save,
            &Cell {
                key: format!("SM_{character}"),
                path: vec![index],
            },
        );
        input(
            save,
            format!("sl-{character}-{index}"),
            format!("{name} (Character {})", character + 1),
            "Level",
            Cell {
                key: format!("SL_{character}"),
                path: vec![index],
            },
            max,
        )
    })
    .collect()
}

/// The guild's `Rucksack` points, the row's own bonus.
fn guild(save: &Save) -> Option<GameInput> {
    input(
        save,
        "guild-0-2".into(),
        "Guild · Rucksack".into(),
        "Points",
        Cell {
            key: "Guild".into(),
            path: vec![0, 2],
        },
        None,
    )
}

/// The ledger name of the stamp group one capacity row reads, if it reads one.
fn stamp_name(kind: &str) -> Option<&'static str> {
    Some(match kind {
        "MinCap" => "Mining capacity stamp",
        "ChopCap" => "Chopping capacity stamp",
        "FishCap" => "Fishing capacity stamp",
        "CatchCap" => "Catching capacity stamp",
        "MatCap" => "Material capacity stamp",
        "AllCarryCap" => "All carry capacity stamp",
        _ => return None,
    })
}

/// One row per capacity stamp group, read from `StampLv[category][index]`, in
/// the stamp table's own order.
fn stamps(save: &Save) -> Vec<GameInput> {
    data::capacity()
        .stamps
        .iter()
        .filter_map(|stamp| {
            input(
                save,
                format!("stamp-lv-{}-{}", stamp.cat, stamp.idx),
                stamp_name(&stamp.kind)?.into(),
                "Level",
                Cell {
                    key: "StampLv".into(),
                    path: vec![stamp.cat, stamp.idx],
                },
                None,
            )
        })
        .collect()
}

/// One input, when the save carries a finite value at its cell.
fn input(
    save: &Save,
    id: String,
    name: String,
    label: &'static str,
    cell: Cell,
    max: Option<f64>,
) -> Option<GameInput> {
    let saved = read(save, &cell)?;
    Some(GameInput {
        id,
        name,
        label,
        saved,
        max,
        cell,
    })
}

/// The number a cell holds, when the save has one.
fn read(save: &Save, cell: &Cell) -> Option<f64> {
    let mut value = save.get(&cell.key)?;
    for index in &cell.path {
        value = at(&value, *index)?.clone();
    }
    number(&value)
}

/// Writes `next` at `path`, keeping the row shape the game wrote.
fn write(value: &mut Value, path: &[usize], next: f64) -> bool {
    let Some((index, rest)) = path.split_first() else {
        return false;
    };
    match &mut *value {
        Value::Array(cells) => match cells.get_mut(*index) {
            Some(slot) if rest.is_empty() => {
                *slot = Value::from(next);
                true
            }
            Some(slot) => write(slot, rest, next),
            None => false,
        },
        Value::Object(cells) => match cells.get_mut(&index.to_string()) {
            Some(slot) if rest.is_empty() => {
                *slot = Value::from(next);
                true
            }
            Some(slot) => write(slot, rest, next),
            None => false,
        },
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::breakdowns::Inputs;
    use idleon_research::model::{SimContext, Simulation};
    use serde_json::json;

    /// A row of `length` cells, with `cells` set and the rest zero.
    fn row(length: usize, cells: &[(usize, f64)]) -> Vec<Value> {
        let mut row = vec![json!(0); length];
        for (index, value) in cells {
            row[*index] = json!(value);
        }
        row
    }

    /// One character's bag rows, a vault, four gem purchases, a guild row, two
    /// talents and the carry-capacity stamps: enough for the capacity rows to
    /// resolve and for every input family to appear.
    fn save() -> Save {
        Save::new(json!({
            "charNames": ["Ana", "Bo"],
            "MaxCarryCap_0": "{\"Mining\":1000,\"Chopping\":1000}",
            "MaxCarryCap_1": "{\"Mining\":2000}",
            "UpgVault": row(12, &[(11, 25.0)]),
            "GemItemsPurchased": row(59, &[(58, 4.0)]),
            "Guild": [row(3, &[(2, 50.0)])],
            "SL_0": row(100, &[(78, 100.0)]),
            "SM_0": row(100, &[(78, 200.0)]),
            "StampLv": [row(2, &[]), row(23, &[(3, 5.0)]), row(2, &[(1, 7.0)])],
        }))
    }

    fn sheet(save: &Save) -> Sheet {
        Sheet::build(&Inputs {
            save,
            simulation: &Simulation::default(),
            sim_context: &SimContext::default(),
            character: 0,
            map: 0,
        })
    }

    fn capacity(sheet: &Sheet, id: &str) -> f64 {
        sheet
            .get(id)
            .and_then(|row| row.resolution.value())
            .unwrap_or_else(|| panic!("{id} resolves"))
    }

    #[test]
    fn the_catalog_lists_the_cells_the_capacity_rows_scale_by() {
        let sandbox = Sandbox::new(&save(), 0);
        let ids: Vec<&str> = sandbox
            .inputs()
            .iter()
            .map(|input| input.id.as_str())
            .collect();
        assert_eq!(
            ids,
            [
                "upg-vault-11",
                "gem-items-58",
                "sl-0-78",
                "guild-0-2",
                "stamp-lv-1-3",
                "stamp-lv-1-5",
                "stamp-lv-1-7",
                "stamp-lv-1-20",
                "stamp-lv-1-22",
                "stamp-lv-2-1",
            ]
        );
        let talent = sandbox.input("sl-0-78").expect("the talent is listed");
        assert_eq!(talent.saved, 100.0);
        assert_eq!(talent.max, Some(200.0));
        assert_eq!(talent.label, "Level");
        assert!(talent.name.contains("Character 1"), "{}", talent.name);
        let stamp = sandbox.input("stamp-lv-1-3").expect("the stamp is listed");
        assert_eq!(stamp.saved, 5.0);
        assert_eq!(stamp.name, "Mining capacity stamp");
    }

    #[test]
    fn a_save_without_the_cells_offers_no_inputs_for_them() {
        let bare = Save::new(json!({ "charNames": ["Ana"] }));
        assert!(Sandbox::new(&bare, 0).inputs().is_empty());
        // Character two has no SL_1 row of its own; the account inputs remain.
        let other = Sandbox::new(&save(), 1);
        assert!(other.input("sl-1-78").is_none());
        assert!(other.input("gem-items-58").is_some());
    }

    #[test]
    fn both_stamps_apply_to_the_one_row_they_share() {
        let mut sandbox = Sandbox::new(&save(), 0);
        sandbox.set("stamp-lv-1-3", 20.0).expect("a level");
        sandbox.set("stamp-lv-2-1", 9.0).expect("a level");
        let stamps = sandbox.current().get("StampLv").expect("the row");
        assert_eq!(
            stamps,
            json!([row(2, &[]), row(23, &[(3, 20.0)]), row(2, &[(1, 9.0)])]),
            "one key holds both edits"
        );
    }

    #[test]
    fn an_edit_moves_the_capacity_rows_and_the_ledger_names_them() {
        let save = save();
        let baseline = sheet(&save);
        let mut sandbox = Sandbox::new(&save, 0);
        // The gem factor is `1 + 25 * purchases / 100`, so four purchases
        // double every capacity row and none of them halves.
        sandbox.set("gem-items-58", 0.0).expect("a purchase count");
        let edited = sheet(&sandbox.current());
        let before = capacity(&baseline, "carry-capacity-mining");
        let after = capacity(&edited, "carry-capacity-mining");
        assert!(
            before == 2.0 * after || before == 2.0 * after + 1.0,
            "{before} is not twice {after}"
        );

        let affected = edited.affected(&baseline);
        assert_eq!(affected.len(), 8, "{affected:?}");
        for row in &affected {
            assert!(row.id.starts_with("carry-capacity-"), "{}", row.id);
            assert_eq!(
                row.delta,
                Some(capacity(&edited, &row.id) - capacity(&baseline, &row.id))
            );
            let percent = row.percent.expect("a percentage of a non-zero saved value");
            assert!((percent + 50.0).abs() < 0.5, "{}: {percent}", row.id);
        }
        assert!(edited.affected(&edited).is_empty(), "no edit, no change");
    }

    #[test]
    fn an_affected_row_prints_upstreams_change_line() {
        let affected = Affected {
            id: "carry-capacity-mining".into(),
            name: "Mining Carry Capacity".into(),
            saved: "2,000".into(),
            current: "1,000".into(),
            delta: Some(-1_000.0),
            percent: Some(-50.0),
        };
        assert_eq!(
            affected.line(),
            "2,000 → 1,000 (Δ -1000, -50%)",
            "the line the affected-formula list shows"
        );
        assert_eq!(affected.saved_note(), "Saved: 2,000 (-50%)");
        let zero = Affected {
            percent: None,
            delta: Some(3.0),
            ..affected
        };
        assert_eq!(
            zero.line(),
            "2,000 → 1,000 (Δ +3; % unavailable: saved zero)"
        );
        assert_eq!(zero.saved_note(), "Saved: 2,000");
    }

    #[test]
    fn reset_restores_every_saved_value() {
        let save = save();
        let baseline = sheet(&save);
        let mut sandbox = Sandbox::new(&save, 0);
        assert_eq!(sandbox.status(), "Game-input sandbox: no edits.");
        sandbox.set("gem-items-58", 0.0).expect("a purchase count");
        sandbox.set("upg-vault-11", 40.0).expect("a vault level");
        assert_eq!(sandbox.count(), 2);
        assert_eq!(sandbox.status(), "Hypothetical: 2 edited game inputs.");
        assert_eq!(sandbox.value("gem-items-58"), Some(0.0));

        sandbox.reset("gem-items-58");
        assert_eq!(sandbox.count(), 1);
        assert_eq!(sandbox.value("gem-items-58"), Some(4.0));
        assert!(sandbox.edited("upg-vault-11"));
        assert!(
            sandbox
                .current()
                .get("GemItemsPurchased")
                .is_some_and(|row| at(&row, 58).and_then(number) == Some(4.0))
        );

        sandbox.reset_all();
        assert_eq!(sandbox.count(), 0);
        assert_eq!(sandbox.current().get("UpgVault"), save.get("UpgVault"));
        assert_eq!(sheet(&sandbox.current()).affected(&baseline), Vec::new());
    }

    #[test]
    fn setting_an_input_back_to_its_saved_value_is_not_an_edit() {
        let mut sandbox = Sandbox::new(&save(), 0);
        sandbox.set("gem-items-58", 9.0).expect("a purchase count");
        assert!(sandbox.edited("gem-items-58"));
        sandbox.set("gem-items-58", 4.0).expect("the saved count");
        assert!(!sandbox.edited("gem-items-58"));
        assert_eq!(sandbox.count(), 0);
    }

    #[test]
    fn an_edit_outside_the_upstream_checks_is_refused() {
        let mut sandbox = Sandbox::new(&save(), 0);
        assert_eq!(
            sandbox.set("nope", 1.0),
            Err("Unknown editable game input.".into())
        );
        assert_eq!(
            sandbox.set("gem-items-58", f64::NAN),
            Err("Enter a finite number.".into())
        );
        assert_eq!(
            sandbox.set("gem-items-58", 1.5),
            Err("Enter a whole level.".into())
        );
        assert_eq!(sandbox.set("gem-items-58", -1.0), Err("Minimum: 0.".into()));
        assert_eq!(sandbox.set("sl-0-78", 201.0), Err("Maximum: 200.".into()));
        assert_eq!(sandbox.count(), 0);
    }

    #[test]
    fn a_character_switch_keeps_only_the_inputs_it_still_lists() {
        let save = save();
        let mut sandbox = Sandbox::new(&save, 0);
        sandbox.set("sl-0-78", 120.0).expect("a talent level");
        sandbox.set("gem-items-58", 6.0).expect("a purchase count");
        sandbox.adopt(&save, 1);
        // Character two has no SL_1 row, so that edit leaves with its input.
        assert!(sandbox.input("sl-0-78").is_none());
        assert_eq!(sandbox.count(), 1);
        assert!(sandbox.edited("gem-items-58"));
    }
}
