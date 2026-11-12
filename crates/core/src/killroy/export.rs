//! The results document the section copies as JSON.
//!
//! The upstream results screen has no export, so this document is the section's
//! own, versioned shape: the run's inputs and assumptions, the saved buffs each
//! selected character starts with, and one entry per configuration with its
//! average gains, its delta against the baseline and the best marker.
//!
//! Nothing here reads the clock or the window state, so exporting the same plan
//! twice writes the same bytes.

use super::planner::{Plan, Row};
use super::summary::RowSummary;
use serde_json::Value;

/// The document's `format` marker.
pub const FORMAT: &str = "idleon-killroy-results";
/// The document's schema version.
pub const VERSION: u32 = 1;

/// The results document: the plan's inputs and every summary row.
pub fn document(plan: &Plan, rows: &[RowSummary]) -> Value {
    serde_json::json!({
        "format": FORMAT,
        "version": VERSION,
        "trials": plan.seeds.len(),
        "seeds": plan.seeds,
        "keepBuffs": plan.keep_buffs,
        "unlimitedInventory": plan.unlimited_inventory,
        "weeks": plan.weeks,
        "characters": plan.characters,
        "rows": rows.iter().map(row_document).collect::<Vec<_>>(),
        "warnings": plan.warnings,
    })
}

/// The results document as pretty JSON, ready for the clipboard.
pub fn to_json(plan: &Plan, rows: &[RowSummary]) -> String {
    serde_json::to_string_pretty(&document(plan, rows)).unwrap_or_default()
}

/// One configuration and what its trials averaged.
fn row_document(row: &RowSummary) -> Value {
    let job: &Row = &row.row;
    serde_json::json!({
        "key": job.key,
        "baseline": job.baseline,
        "character": job.character,
        "ci": job.ci,
        "slot": job.slot,
        "weekOffset": job.week_offset,
        "week": job.encounter.week,
        "map": job.encounter.map_name,
        "monster": job.encounter.monster,
        "family": job.encounter.family_name,
        "upgrade": job.upgrade,
        "runs": row.runs,
        "complete": row.complete,
        "best": row.best,
        "mean": row.mean,
        "delta": row.delta,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::killroy::{Comparisons, PlanInput, Selection, plan as build, run, trial_seeds};
    use crate::save::{Save, import};
    use serde_json::json;

    fn save() -> Save {
        let path = format!("{}/../../fixtures/data.json", env!("CARGO_MANIFEST_DIR"));
        let save: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(path).expect("the fixture is committed"))
                .expect("fixture JSON");
        import(&json!({ "data": save, "serverVars": { "KillroySwap": 7 } }).to_string())
            .expect("the wrapped fixture imports")
            .save
    }

    fn planned() -> Plan {
        build(
            &save(),
            &PlanInput {
                selections: vec![Selection {
                    week_offset: 0,
                    ci: 0,
                    slot: 1,
                }],
                seeds: trial_seeds(1, 2),
                comparisons: Comparisons::Only(vec![0, 2]),
                kills_per_second: 3.0,
                keep_buffs: true,
                unlimited_inventory: true,
            },
        )
        .expect("the plan builds")
    }

    #[test]
    fn the_document_carries_the_inputs_the_buffs_and_every_row() {
        let plan = planned();
        let rows = run(&plan, |_, _| false).expect("the run finishes");
        let document = document(&plan, &rows);
        assert_eq!(document["format"], FORMAT);
        assert_eq!(document["version"], VERSION);
        assert_eq!(document["trials"], 2);
        assert_eq!(document["seeds"], json!([1, 2]));
        assert_eq!(document["keepBuffs"], true);
        assert_eq!(document["unlimitedInventory"], true);
        assert_eq!(document["warnings"], json!(plan.warnings));
        assert_eq!(document["weeks"].as_array().map(Vec::len), Some(1));
        assert_eq!(document["rows"].as_array().map(Vec::len), Some(rows.len()));

        let first = &document["rows"][0];
        assert_eq!(first["key"], rows[0].row.key);
        assert_eq!(first["baseline"], rows[0].row.baseline);
        assert_eq!(first["character"], rows[0].row.character);
        assert_eq!(first["slot"], rows[0].row.slot);
        assert_eq!(first["weekOffset"], rows[0].row.week_offset);
        assert_eq!(first["map"], rows[0].row.encounter.map_name);
        assert_eq!(first["family"], rows[0].row.encounter.family_name);
        assert_eq!(first["runs"], 2);
        assert_eq!(first["complete"], true);
        assert_eq!(first["best"], rows[0].best);
        assert!(first["mean"]["skullsPicked"].is_number());
        assert!(first["delta"].is_null(), "a baseline has no delta");
    }

    #[test]
    fn a_saved_buff_document_matches_the_save() {
        let plan = planned();
        let rows = run(&plan, |_, _| false).expect("the run finishes");
        let document = document(&plan, &rows);
        let characters = document["characters"].as_array().expect("characters");
        assert_eq!(characters.len(), 1);
        let buffs = characters[0]["buffs"].as_array().expect("buffs");
        let expected = crate::killroy::saved_buffs(&save(), 0, true);
        assert_eq!(buffs.len(), expected.len());
        assert_eq!(buffs[0]["id"], expected[0].id);
        assert_eq!(buffs[0]["kept"], true);
    }

    #[test]
    fn the_same_plan_exports_the_same_bytes() {
        let plan = planned();
        let rows = run(&plan, |_, _| false).expect("the run finishes");
        let text = to_json(&plan, &rows);
        assert_eq!(text, to_json(&plan, &rows));
        let parsed: Value = serde_json::from_str(&text).expect("the export is JSON");
        assert_eq!(parsed["format"], FORMAT);
    }

    #[test]
    fn an_upgrade_row_exports_its_upgrade_and_delta() {
        let plan = planned();
        let rows = run(&plan, |_, _| false).expect("the run finishes");
        let document = document(&plan, &rows);
        let upgrade = document["rows"]
            .as_array()
            .expect("rows")
            .iter()
            .find(|row| row["upgrade"].is_object())
            .expect("a comparison row");
        assert!(upgrade["upgrade"]["name"].is_string());
        assert!(upgrade["delta"]["skullsPicked"].is_number());
    }
}
