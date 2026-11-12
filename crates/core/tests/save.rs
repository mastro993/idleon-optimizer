//! Save import: the decoded game save maps onto a simulation and its multipliers.

use idleon_core::model::{GridState, Mag, MagKind, Observations, Placement, Simulation};
use idleon_core::save::{Format, ImportError, ShadowVerdict, import};
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

/// A complete `StatueLevels_<n>` row: one `[level, exp]` pair per statue.
fn statue_rows() -> Value {
    let rows = (0..32)
        .map(|statue| json!([f64::from(statue) + 1.0, f64::from(statue) * 10.0]))
        .collect();
    stringified(&Value::Array(rows))
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
            "OptLacc": stringified(&sparse(700, &[(469, 50.0), (496, 4.0), (511, 5.0), (641, 3.0)])),
            "TimeAway": stringified(&json!({ "GlobalTime": 1_794_067_271.0 })),
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
            "StatueLevels_0": statue_rows(),
            "StatueLevels_1": statue_rows(),
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
    for field in ["Lab", "Tome", "EMm0_0"] {
        assert!(
            imported
                .warnings
                .iter()
                .any(|warning| warning.contains(field)),
            "warnings name {field}: {:?}",
            imported.warnings
        );
    }
    assert!(
        imported
            .tree
            .additive
            .iter()
            .any(|term| term.name == "Sticker Bonus" && !term.is_omitted()),
        "the tree is still built"
    );
}

#[test]
fn full_save_decodes_the_account_terms_it_carries() {
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
    assert!((context.save_global_time - 1_794_067_271.0).abs() < 1e-9);
    assert_eq!(context.tournament_last_day, 5.0);
    assert_eq!(context.tournament_cached_day, 4.0);
    // Companion 55 (+15) and the two weekly-boss clouds (+2) at one percent each.
    assert!((context.all_bonus_multi - 1.17).abs() < 1e-9);
    // Killroy 1.2, nonstop studies 1.06, Cglunko 1.03, fountain 1.12,
    // companions 52/153 3.6, companion 54 at level two 1.5, RoG 0 2.0.
    assert!((context.killroy5 - 1.2).abs() < 1e-9);
    assert!((context.dream14 - 2.0).abs() < 1e-9);
    assert!((context.cglunko11 - 3.0).abs() < 1e-9);
    assert!(
        (context.fountain_2_16 - 12.0).abs() < 1e-9,
        "fountain {}",
        context.fountain_2_16
    );
    assert!((context.companion_52 - 3.6).abs() < 1e-9);
    assert!(context.companion_153, "companion 153 is owned");
    assert!((context.companion_54_level2 - 1.0).abs() < 1e-9);
    assert!((context.rog(0) - 100.0).abs() < 1e-9);
    assert!((context.rog(3) - 2.0).abs() < 1e-9);
    // Companion 153 (10), RoG 3 (0 here), sailing art 37 (6), W7 task (2), ban_k (5).
    assert_eq!(context.sailing_art37, 6.0);
    assert_eq!(context.task_grid_points, 2.0);
    assert_eq!(context.bonus_grid_points, 25);
    // The tree is built and the terms the save cannot derive are named.
    assert!(imported.tree.additive_pct > 0.0);
    assert!(
        imported
            .warnings
            .iter()
            .all(|warning| warning.contains(':')),
        "every warning names its field: {:?}",
        imported.warnings
    );
    assert!(
        imported
            .warnings
            .iter()
            .any(|warning| warning.contains("Lab")),
        "the missing lab is warned about"
    );
}

#[test]
fn equipment_uq_stats_feed_the_research_multiplier() {
    let mut save = minimal_save();
    let data = save["data"]
        .as_object_mut()
        .expect("the save sits under data");
    // A gear roll with no built-in UQ: the saved `UQ1txt` names the stat and
    // `UQ1val` is the bonus, as `equipment.resolve` scans it.
    let mut slots = vec![json!({}); 16];
    slots[15] = json!({ "UQ1txt": "%_RESEARCH_EXP", "UQ1val": 40, "UQ2txt": "0", "UQ2val": 0 });
    let mut order = vec![json!(""); 16];
    order[15] = json!("Plain_Hat");
    // `EquipOrder_<char>` holds the gear row then the tool row.
    data.insert("EMm0_0".into(), stringified(&Value::Array(slots)));
    data.insert(
        "EquipOrder_0".into(),
        stringified(&json!([Value::Array(order), json!([])])),
    );

    let imported = import(&save.to_string()).expect("the equipment save imports");
    // `EtcBonuses(108)` is the UQ value; the multiplier applies it once.
    assert!(
        (imported.context.research_equipment_multi - 1.4).abs() < 1e-9,
        "equipment multiplier is {}",
        imported.context.research_equipment_multi
    );
    assert!(
        imported
            .tree
            .term("Equipment Research EXP")
            .is_some_and(|term| (term.value - 1.4).abs() < 1e-9),
        "the tree reports the equipment term"
    );
}

#[test]
fn a_save_without_the_cove_rows_reads_zero_and_warns() {
    let imported = import(&minimal_save().to_string()).expect("the minimal save imports");
    assert_eq!(imported.cglunko, idleon_core::Cglunko::default());
    assert!(
        imported
            .warnings
            .iter()
            .any(|warning| warning
                .starts_with("OptLacc: not in the save; Cglunko values kept at zero")),
        "{:?}",
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
    assert!(matches!(
        import(r#"{"data": {}}"#),
        Err(ImportError::NoSaveFields(_))
    ));
}

/// A save carrying only the research rows, plus one `OptLacc` shape.
fn shadow_save(opt_lacc: Option<Value>) -> Value {
    let mut data = json!({
        "Research": stringified(&Value::Array(research_rows())),
    });
    if let Some(opt_lacc) = opt_lacc {
        data["OptLacc"] = opt_lacc;
    }
    json!({ "data": data, "serverVars": { "A_ResXP": 1.01 } })
}

/// `OptLacc` with `value` in slot 26, stringified like the game writes it.
fn opt_lacc(value: Value) -> Value {
    let mut cells = vec![json!(0.0); 30];
    cells[26] = value;
    stringified(&Value::Array(cells))
}

#[test]
fn the_shadow_verdict_reaches_all_three_states() {
    let shadow = |opt_lacc: Option<Value>| {
        import(&shadow_save(opt_lacc).to_string())
            .expect("the shadow save imports")
            .shadow
    };

    assert_eq!(
        shadow(Some(opt_lacc(json!(1.0)))).verdict(),
        ShadowVerdict::Yes
    );
    assert_eq!(
        shadow(Some(opt_lacc(json!(0.0)))).verdict(),
        ShadowVerdict::No
    );
    assert_eq!(shadow(None).verdict(), ShadowVerdict::Unknown);
}

#[test]
fn the_shadow_verdict_never_guesses() {
    let shadow = |opt_lacc: Option<Value>| {
        import(&shadow_save(opt_lacc).to_string())
            .expect("the shadow save imports")
            .shadow
    };

    // The flag the page reads is kept alongside the answer.
    assert_eq!(shadow(Some(opt_lacc(json!(2.0)))).flag, Some(2.0));
    assert_eq!(
        shadow(Some(opt_lacc(json!(-3.0)))).verdict(),
        ShadowVerdict::Yes,
        "every nonzero finite flag is a yes"
    );
    // The page accepts a numeric string the way `Number(value)` does.
    assert_eq!(
        shadow(Some(opt_lacc(json!("3")))).verdict(),
        ShadowVerdict::Yes
    );
    assert_eq!(
        shadow(Some(opt_lacc(json!("0")))).verdict(),
        ShadowVerdict::No
    );
    assert_eq!(
        shadow(Some(stringified(&json!({ "26": 1 })))).verdict(),
        ShadowVerdict::Yes,
        "the upstream lookup also reads an object key"
    );
    for unreadable in [
        json!(null),
        json!(""),
        json!("Infinity"),
        json!("NaN"),
        json!(true),
        json!([0.0]),
    ] {
        assert_eq!(
            shadow(Some(opt_lacc(unreadable.clone()))).verdict(),
            ShadowVerdict::Unknown,
            "{unreadable} is not a readable flag"
        );
    }
    // A short `OptLacc` has no slot 26 at all.
    assert_eq!(
        shadow(Some(stringified(&json!([0.0, 1.0])))).verdict(),
        ShadowVerdict::Unknown
    );
}

/// The same fields without the `data` wrapper, as a plain `save.json` writes them.
fn save_json(wrapped: &Value) -> Value {
    let mut flat = wrapped
        .get("data")
        .and_then(Value::as_object)
        .expect("the it.json save sits under data")
        .clone();
    for key in ["companion", "serverVars"] {
        if let Some(value) = wrapped.get(key) {
            flat.insert(key.to_string(), value.clone());
        }
    }
    Value::Object(flat)
}

#[test]
fn it_json_and_save_json_import_the_same_values() {
    let wrapped = full_save();
    let it_json = import(&wrapped.to_string()).expect("it.json imports");
    let save_json = import(&save_json(&wrapped).to_string()).expect("save.json imports");

    assert_eq!(it_json.format, Format::ItJson);
    assert_eq!(save_json.format, Format::SaveJson);
    assert_eq!(it_json.characters, 2);
    assert_eq!(save_json.characters, it_json.characters);
    assert_eq!(save_json.warnings, it_json.warnings);
    // The audit-ready save and its named roster survive the import; a flat
    // save.json carries the same decoded object without the names.
    assert_eq!(it_json.named_characters, 2);
    assert_eq!(save_json.named_characters, 0);
    for field in ["Research", "Lv0_0", "OptLacc", "Holes"] {
        assert_eq!(
            it_json.save.get(field),
            save_json.save.get(field),
            "{field} is the same decoded field"
        );
    }

    let it_rate = sim::exp_rate(&it_json.simulation, &it_json.context);
    let save_rate = sim::exp_rate(&save_json.simulation, &save_json.context);
    assert_eq!(save_rate.total, it_rate.total);
    assert_eq!(save_rate.multi, it_rate.multi);
    assert_eq!(save_json.simulation.grid, it_json.simulation.grid);
    assert_eq!(
        save_json.simulation.placements,
        it_json.simulation.placements
    );
    assert_eq!(
        (save_json.context.bonus_grid_points),
        it_json.context.bonus_grid_points
    );
    assert_eq!(save_json.context, it_json.context);
    assert_eq!(save_json.tree, it_json.tree);
}

#[test]
fn a_state_export_imports_under_its_decoded_names() {
    let mut grid = vec![0.0; 240];
    grid[50] = 4.0;
    let text = json!({
        "researchLevel": 7,
        "researchExp": 12.5,
        "gridLevels": array(grid),
        "occFound": array([1.0, 0.0]),
    });

    let imported = import(&text.to_string()).expect("a decoded state imports");
    assert_eq!(imported.format, Format::ResearchState);
    assert_eq!(imported.simulation.research_level, 7);
    assert_eq!(imported.simulation.grid.level(50), 4);
    assert!(imported.simulation.observations.found[0]);
    assert_eq!(imported.characters, 0);
    assert!(
        imported
            .warnings
            .iter()
            .any(|warning| warning.contains("characters")),
        "warnings say no character rows were found: {:?}",
        imported.warnings
    );
}

#[test]
fn the_character_count_counts_rows_not_their_indices() {
    let save = json!({
        "data": {
            "Research": stringified(&Value::Array(research_rows())),
            "Lv0_3": stringified(&character_row(40.0, 0.0)),
        },
        "serverVars": { "A_ResXP": 1.01 },
    });

    let imported = import(&save.to_string()).expect("a sparse roster imports");
    assert_eq!(imported.characters, 1);
    assert_eq!(imported.simulation.research_level, 40);
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
        saved_overlay: Some(vec![-1; 240]),
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
