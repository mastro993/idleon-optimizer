//! Save import: the decoded game save maps onto a simulation and its multipliers.

use idleon_core::model::{GridState, Mag, MagKind, Observations, Placement, Simulation};
use idleon_core::save::{ImportError, import};
use idleon_core::sim;
use serde_json::{Value, json};

/// An array of numbers as JSON.
fn array(values: impl IntoIterator<Item = f64>) -> Value {
    Value::Array(values.into_iter().map(|value| json!(value)).collect())
}

/// A field the save writes as a JSON string, e.g. `Lv0_0`.
fn stringified(value: &Value) -> Value {
    Value::String(value.to_string())
}

/// The research rows the decoder reads: grid, overlay, observations, insight,
/// magnifiers and shape placements, with the unused rows left empty.
fn research_rows() -> Vec<Value> {
    let mut grid = vec![0.0; 240];
    grid[50] = 3.0;
    grid[52] = 4.0;
    grid[90] = 2.0;
    grid[173] = 1.0;

    let mut occ_found = vec![0.0; 80];
    for obs in [0, 1, 2, 5] {
        occ_found[obs] = 1.0;
    }
    let mut insight_progress = vec![0.0; 80];
    insight_progress[0] = 5.5;
    let mut insight_levels = vec![0.0; 80];
    insight_levels[0] = 2.0;
    insight_levels[1] = 1.0;

    // x, y, slot, type groups; type 0 glass, 1 monocle, 2 kaleidoscope.
    let mags = vec![
        0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 3.0, 1.0, 0.0, 0.0,
        4.0, 2.0, 0.0, 0.0, 5.0, 0.0,
    ];
    // x, y, rotation, unknown groups.
    let shapes = vec![0.0, 0.0, 0.0, 0.0, 60.0, 0.0, 45.0, 0.0];

    let mut rows = vec![
        array(grid),
        array(vec![-1.0; 240]),
        array(occ_found),
        array(insight_progress),
        array(insight_levels),
        array(mags),
        array(shapes),
    ];
    while rows.len() < 48 {
        rows.push(json!([]));
    }
    rows
}

/// One character's `Lv0_<n>` row: level 20 is research, 14 is divinity.
fn character_row(research_level: f64, divinity: f64) -> Value {
    let mut cells = vec![0.0; 30];
    cells[14] = divinity;
    cells[20] = research_level;
    array(cells)
}

/// An `Exp0_<n>` row with the research EXP at index 20.
fn exp_row(research_exp: f64) -> Value {
    let mut cells = vec![0.0; 30];
    cells[20] = research_exp;
    array(cells)
}

/// A nested array of zeroes with one straight index filled, for `Holes`.
fn nested_row(length: usize, index: usize, value: f64) -> Value {
    let mut cells = vec![0.0; length];
    cells[index] = value;
    array(cells)
}

/// A flat array of zeroes with a few slots filled, for `OptLacc`-style fields.
fn sparse(length: usize, filled: &[(usize, f64)]) -> Value {
    let mut cells = vec![0.0; length];
    for (index, value) in filled {
        cells[*index] = *value;
    }
    array(cells)
}

/// A save carrying only the research rows and one character.
fn minimal_save() -> Value {
    json!({
        "data": {
            "Research": stringified(&Value::Array(research_rows())),
            "Lv0_0": stringified(&character_row(42.0, 0.0)),
            "Exp0_0": stringified(&exp_row(1000.0)),
        },
        "serverVars": { "A_ResXP": 1.01 },
    })
}

/// A save with every field the importer reads, plus keys it must ignore.
fn full_save() -> Value {
    let mut sushi = vec![json!([]); 10];
    sushi[5] = json!([1.0, 2.0, 3.0, 4.0, 5.0, 6.0, -1.0]);
    let mut holes = vec![json!([]); 40];
    holes[31] = json!([[], [], nested_row(20, 16, 4.0)]);
    holes[32] = json!([[], [], nested_row(20, 16, 3.0)]);
    let mut sailing = vec![json!([]); 5];
    sailing[3] = nested_row(40, 37, 6.0);
    let mut tasks = vec![json!([]); 8];
    let mut task_row = vec![0.0; 12];
    task_row[3] = 2.0;
    tasks[6] = array(task_row);

    json!({
        "data": {
            "Research": stringified(&Value::Array(research_rows())),
            "Lv0_0": stringified(&character_row(100.0, 2.0)),
            "Exp0_0": stringified(&exp_row(123456.0)),
            "Lv0_1": stringified(&character_row(70.0, 0.0)),
            "Exp0_1": stringified(&exp_row(1.0)),
            "OptLacc": stringified(&sparse(700, &[(469, 50.0), (641, 3.0)])),
            "Dream": stringified(&sparse(20, &[(14, 2.0)])),
            "Holes": stringified(&Value::Array(holes)),
            "Sailing": stringified(&Value::Array(sailing)),
            "TaskZZ2": stringified(&Value::Array(tasks)),
            "BundlesReceived": stringified(&json!({ "ban_k": 1 })),
            "WeeklyBoss": stringified(&json!({ "d_71": -1, "d_72": -1, "d_76": 0 })),
            "Sushi": stringified(&Value::Array(sushi)),
            "Rift": stringified(&json!([15.0])),
            "cachedExtPctExSticker": 12.5,
            "ArcadeUpg": stringified(&sparse(70, &[])),
            "FamValColosseumHighscores": [1, 2, 3],
            "SomeFutureField": { "nested": [4, 5, 6] },
        },
        "companion": {
            "l": ["55,0,0,0,0", "52,0,0,0,1", "153,0,0,0,0", "54,0,0,0,1"],
        },
        "serverVars": { "A_ResXP": 1.2 },
        "charNames": ["main", "alt"],
    })
}

#[test]
fn minimal_save_imports_levels_magnifiers_and_insight() {
    let imported = import(&minimal_save().to_string()).expect("the minimal save imports");
    let simulation = &imported.simulation;

    assert_eq!(simulation.research_level, 42);
    assert!((simulation.research_exp - 1000.0).abs() < 1e-9);
    assert_eq!(simulation.grid.level(50), 3);
    assert_eq!(simulation.grid.level(90), 2);
    assert_eq!(simulation.grid.level(0), 0);

    assert_eq!(simulation.mags.len(), 6);
    assert_eq!(simulation.mags[0], Mag::new(MagKind::Regular, Some(0)));
    assert_eq!(simulation.mags[3], Mag::new(MagKind::Monocle, Some(3)));
    assert_eq!(simulation.mags[4], Mag::new(MagKind::Kaleidoscope, Some(4)));

    assert!(simulation.observations.found[0]);
    assert!(!simulation.observations.found[4]);
    assert_eq!(simulation.observations.insight_level[0], 2);
    assert!((simulation.observations.insight_exp[0] - 5.5).abs() < 1e-9);

    assert_eq!(simulation.placements.len(), 2);
    assert_eq!(simulation.placements[0].shape, 0);
    assert_eq!(simulation.placements[1].shape, 1);
    assert!((simulation.placements[1].rot - 45.0).abs() < 1e-9);

    assert!((imported.context.server_var_res_xp - 1.01).abs() < 1e-9);
    // The account multipliers need fields a minimal save does not carry.
    assert!((imported.context.all_bonus_multi - 1.0).abs() < 1e-9);
    assert!((imported.context.extra_pct - 0.0).abs() < 1e-9);
    assert!(
        imported
            .warnings
            .iter()
            .any(|warning| warning.contains("allBonusMulti")),
        "warnings name the missing account fields: {:?}",
        imported.warnings
    );
}

#[test]
fn full_save_imports_without_unjustified_warnings() {
    let imported = import(&full_save().to_string()).expect("the full save imports");
    let simulation = &imported.simulation;
    let context = &imported.context;

    assert_eq!(simulation.research_level, 100);
    assert!((simulation.research_exp - 123456.0).abs() < 1e-9);
    assert_eq!(simulation.grid.level(52), 4);
    assert_eq!(simulation.placements.len(), 2);
    assert!((simulation.placements[1].x - 60.0).abs() < 1e-9);
    assert!((simulation.placements[1].rot - 45.0).abs() < 1e-9);

    assert!((context.server_var_res_xp - 1.2).abs() < 1e-9);
    // Companion 55 (+15) and the two weekly-boss clouds (+2) at one percent each.
    assert!((context.all_bonus_multi - 1.17).abs() < 1e-9);
    assert!((context.extra_pct - 12.5).abs() < 1e-9);
    // Killroy 1.2, nonstop studies 1.06, Cglunko 1.03, fountain 1.12,
    // companions 52/153 3.6, companion 54 at level two 1.5, RoG 0 2.0.
    assert!(
        (context.extra_multiplier - 15.847_695_36).abs() < 1e-9,
        "extra_multiplier is {}",
        context.extra_multiplier
    );
    // Companion 153 (10), RoG 3 (2), sailing art 37 (6), W7 task (2), ban_k (5).
    assert_eq!(context.bonus_grid_points, 25);

    assert!(
        imported.warnings.is_empty(),
        "unknown keys must not warn: {:?}",
        imported.warnings
    );
}

#[test]
fn garbage_and_a_json_array_are_rejected() {
    assert!(matches!(
        import("this is not json"),
        Err(ImportError::NotJson(_))
    ));
    assert!(matches!(import("{ trailing"), Err(ImportError::NotJson(_))));
    assert!(matches!(import("[]"), Err(ImportError::NoSaveFields(_))));
    assert!(matches!(import("{}"), Err(ImportError::NoSaveFields(_))));
    assert!(matches!(
        import(r#"{"hello": "world"}"#),
        Err(ImportError::NoSaveFields(_))
    ));
}

#[test]
fn imported_rate_matches_a_hand_built_simulation() {
    let imported = import(&full_save().to_string()).expect("the full save imports");

    let mut grid = GridState::empty();
    for (cell, level) in [(50, 3), (52, 4), (90, 2), (173, 1)] {
        grid.set(cell, level);
    }
    let mut observations = Observations::default();
    for obs in [0, 1, 2, 5] {
        observations.found[obs] = true;
    }
    observations.insight_level[0] = 2;
    observations.insight_level[1] = 1;
    observations.insight_exp[0] = 5.5;
    let expected = Simulation {
        research_level: 100,
        research_exp: 123456.0,
        grid,
        observations,
        mags: vec![
            Mag::new(MagKind::Regular, Some(0)),
            Mag::new(MagKind::Regular, Some(1)),
            Mag::new(MagKind::Regular, Some(2)),
            Mag::new(MagKind::Monocle, Some(3)),
            Mag::new(MagKind::Kaleidoscope, Some(4)),
            Mag::new(MagKind::Regular, Some(5)),
        ],
        placements: vec![
            Placement {
                shape: 0,
                x: 0.0,
                y: 0.0,
                rot: 0.0,
            },
            Placement {
                shape: 1,
                x: 60.0,
                y: 0.0,
                rot: 45.0,
            },
        ],
        preset: "_none".into(),
    };

    let actual = sim::exp_rate(&imported.simulation, &imported.context);
    let wanted = sim::exp_rate(&expected, &imported.context);
    assert!(actual.total > 0.0);
    assert!(
        (actual.total - wanted.total).abs() < 1e-9,
        "imported {} vs hand built {}",
        actual.total,
        wanted.total
    );
    assert!((actual.multi - wanted.multi).abs() < 1e-9);
}
