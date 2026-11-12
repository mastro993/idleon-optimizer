//! The Jelly Operator fixture helpers both `jelly` test binaries share.
//!
//! `fixtures/jelly.json` is `script/jelly-oracle.mjs` running
//! `js/stats/systems/w7/jelly-operator.js` on `fixtures/data.json` and on three
//! documented save overrides, so a drift in the board rules, the metrics, the
//! seeded runtime, the estimate or the outlook fails there. Every expected
//! number is the upstream calculator's own output.
//!
//! Only some helpers apply to each binary, so the module allows dead code.
#![allow(dead_code)]

use idleon_import::import;
use idleon_jelly::Jelly;
use idleon_jelly::board::Layout;
use serde_json::Value;

pub fn fixture() -> String {
    std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/data.json"
    ))
    .expect("fixtures/data.json is committed")
}

pub fn oracle() -> Value {
    serde_json::from_str(
        &std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/jelly.json"
        ))
        .expect("fixtures/jelly.json is committed"),
    )
    .expect("the oracle is JSON")
}

pub fn case(name: &str) -> Value {
    oracle()["cases"]
        .as_array()
        .expect("the oracle carries cases")
        .iter()
        .find(|case| case["name"] == name)
        .unwrap_or_else(|| panic!("the oracle carries the {name} case"))
        .clone()
}

/// The committed save with one case's documented overrides applied.
pub fn jelly(name: &str) -> (Jelly, Value) {
    let case = case(name);
    let imported = import(&fixture()).expect("the fixture imports");
    let mut jelly = imported.jelly.expect("the fixture carries a jelly board");
    apply(&mut jelly, &case["overrides"]);
    (jelly, case)
}

pub fn apply(jelly: &mut Jelly, overrides: &Value) {
    if let Some(fever) = overrides["fever"].as_u64() {
        jelly.progress.fever = fever as u32;
    }
    if let Some(obstruction) = overrides["obstruction"].as_u64() {
        jelly.progress.obstruction = obstruction as usize;
    }
    if let Some(upgrades) = overrides["upgrades"].as_object() {
        for (id, level) in upgrades {
            let id: usize = id.parse().expect("upgrade ids are numbers");
            if jelly.upgrades.len() <= id {
                jelly.upgrades.resize(id + 1, 0);
            }
            jelly.upgrades[id] = level.as_u64().expect("levels are integers") as u32;
        }
    }
    if let Some(levels) = overrides["cellLevels"].as_array() {
        for (cell_type, level) in levels.iter().enumerate() {
            jelly.cell_level[cell_type] = level.as_u64().expect("levels are integers") as u32;
        }
    }
    if let Some(exp) = overrides["cellExp"].as_array() {
        for (cell_type, value) in exp.iter().enumerate() {
            jelly.cell_exp[cell_type] = value.as_f64().expect("EXP is a number");
        }
    }
    if let Some(layout) = overrides["layout"].as_object() {
        jelly.layout = Layout::new();
        for (anchor, cell_type) in layout {
            jelly.layout.insert(
                anchor.parse().expect("anchors are numbers"),
                cell_type.as_u64().expect("types are integers") as usize,
            );
        }
    }
}

pub fn close(actual: f64, expected: f64, what: &str) {
    let tolerance = 1e-9_f64.max(expected.abs() * 1e-9);
    assert!(
        (actual - expected).abs() <= tolerance,
        "{what}: {actual} != {expected}"
    );
}

pub fn close_opt(actual: Option<f64>, expected: &Value, what: &str) {
    match expected.as_f64() {
        Some(value) => close(
            actual.unwrap_or_else(|| panic!("{what}: expected {value}")),
            value,
            what,
        ),
        None => assert!(
            actual.is_none(),
            "{what}: expected no value, got {actual:?}"
        ),
    }
}

pub fn numbers(value: &Value) -> Vec<f64> {
    value
        .as_array()
        .expect("a number array")
        .iter()
        .map(|item| item.as_f64().expect("a number"))
        .collect()
}
