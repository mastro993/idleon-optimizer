//! The board section planner against the upstream JS.
//!
//! `fixtures/jelly.json` carries `planJellySections` for the committed save:
//! one token, a three-token plan at two beam widths, a low-risk strategy and a
//! zero-token edge case. Every expected number is the upstream calculator's own
//! output; the tests also hold the plan's own rules (every row's board is legal
//! for the sections it buys) and its reproducibility.

mod common;

use common::{close, close_opt, jelly};
use idleon_jelly::board::{self, Layout};
use idleon_jelly::planner::{Objective, Plan, PlanOptions, Quality, SectionRow, Strategy};
use serde_json::Value;
use std::collections::BTreeMap;

fn plan_key(layout: &Layout) -> String {
    layout
        .iter()
        .map(|(anchor, cell_type)| format!("{anchor}:{cell_type}"))
        .collect::<Vec<_>>()
        .join("|")
}

fn layout_of(value: &Value) -> Layout {
    value
        .as_object()
        .expect("a layout object")
        .iter()
        .map(|(anchor, cell_type)| {
            (
                anchor.parse().expect("a numeric anchor"),
                cell_type.as_u64().expect("a cell type") as usize,
            )
        })
        .collect::<BTreeMap<usize, usize>>()
}

fn options(case: &Value) -> PlanOptions {
    let options = &case["sectionsOptions"];
    PlanOptions {
        strategy: options["strategy"]
            .as_str()
            .and_then(Strategy::from_id)
            .unwrap_or_default(),
        objective: options["objective"]
            .as_str()
            .and_then(Objective::from_id)
            .unwrap_or_default(),
        exp_cell_type: options["expCellType"]
            .as_i64()
            .filter(|cell_type| *cell_type >= 0)
            .map(|cell_type| cell_type as usize),
        tokens: options["tokens"].as_u64().map(|tokens| tokens as usize),
        allow_hypothetical_tokens: options["allowHypotheticalTokens"]
            .as_bool()
            .unwrap_or(false),
        quality: options["quality"]
            .as_str()
            .and_then(Quality::from_id)
            .unwrap_or_default(),
        beam_width: options["beamWidth"].as_u64().unwrap_or(0) as usize,
        seed: options["seed"].as_u64().unwrap_or(1) as u32,
        layout: None,
    }
}

/// The estimate fields a plan row keeps.
fn assert_estimate(expected: &Value, actual: &idleon_jelly::Estimate, label: &str) {
    assert_eq!(
        actual.valid,
        expected["valid"].as_bool().unwrap(),
        "{label}: valid"
    );
    assert_eq!(
        actual.trials,
        expected["trials"].as_u64().unwrap() as u32,
        "{label}: trials"
    );
    assert_eq!(
        actual.success_count,
        expected["successCount"].as_u64().unwrap() as u32,
        "{label}: successes"
    );
    close(
        actual.clear_probability,
        expected["clearProbability"].as_f64().unwrap(),
        &format!("{label}: clear probability"),
    );
    close(
        actual.clear_probability_low,
        expected["clearProbabilityLow"].as_f64().unwrap(),
        &format!("{label}: clear probability low"),
    );
    close(
        actual.clear_probability_high,
        expected["clearProbabilityHigh"].as_f64().unwrap(),
        &format!("{label}: clear probability high"),
    );
    close(
        actual.mean_time,
        expected["meanTime"].as_f64().unwrap(),
        &format!("{label}: mean time"),
    );
    close(
        actual.mean_damage,
        expected["meanDamage"].as_f64().unwrap(),
        &format!("{label}: mean damage"),
    );
    close(
        actual.mean_damage_standard_error,
        expected["meanDamageStandardError"].as_f64().unwrap(),
        &format!("{label}: mean damage standard error"),
    );
    close(
        actual.mean_bloodcells,
        expected["meanBloodcells"].as_f64().unwrap(),
        &format!("{label}: mean Bloodcells"),
    );
    close(
        actual.mean_cell_exp,
        expected["meanCellExp"].as_f64().unwrap(),
        &format!("{label}: mean Cell EXP"),
    );
    for cell_type in 0..9 {
        close(
            actual.mean_cell_exp_by_type[cell_type],
            expected["meanCellExpByType"][cell_type].as_f64().unwrap(),
            &format!("{label}: mean Cell EXP for cell {cell_type}"),
        );
    }
    close(
        actual.mean_critical_time,
        expected["meanCriticalTime"].as_f64().unwrap(),
        &format!("{label}: mean Critical time"),
    );
    close(
        actual.mean_boss_attacks,
        expected["meanBossAttacks"].as_f64().unwrap(),
        &format!("{label}: mean boss attacks"),
    );
}

fn assert_row(expected: &Value, actual: &SectionRow, label: &str) {
    assert_eq!(
        actual.plot_id,
        expected["plotId"].as_u64().map(|plot| plot as usize),
        "{label}: plot id"
    );
    assert_eq!(
        actual.start,
        expected["start"].as_u64().map(|start| start as usize),
        "{label}: start"
    );
    let plot_ids: Vec<usize> = expected["plotIds"]
        .as_array()
        .unwrap()
        .iter()
        .map(|plot| plot.as_u64().unwrap() as usize)
        .collect();
    assert_eq!(actual.plot_ids, plot_ids, "{label}: plot ids");
    assert_eq!(
        actual.new_usable_slots,
        expected["newUsableSlots"].as_u64().unwrap() as usize,
        "{label}: new usable slots"
    );
    let new_slots: Vec<usize> = expected["newSlots"]
        .as_array()
        .unwrap()
        .iter()
        .map(|slot| slot.as_u64().unwrap() as usize)
        .collect();
    assert_eq!(actual.new_slots, new_slots, "{label}: new slots");
    assert_eq!(
        actual.proximity_slots,
        expected["proximitySlots"].as_u64().unwrap() as usize,
        "{label}: proximity slots"
    );
    let placements = expected["newlyLegalFootprintPlacements"]
        .as_array()
        .unwrap();
    assert_eq!(
        actual.newly_legal_footprint_placements.len(),
        placements.len(),
        "{label}: footprint placement rows"
    );
    for (expected, actual) in placements
        .iter()
        .zip(&actual.newly_legal_footprint_placements)
    {
        assert_eq!(
            actual.cell_type,
            expected["type"].as_u64().unwrap() as usize,
            "{label}: placement cell type"
        );
        assert_eq!(
            actual.name,
            expected["name"].as_str().unwrap(),
            "{label}: placement cell name"
        );
        assert_eq!(
            actual.count,
            expected["count"].as_u64().unwrap() as usize,
            "{label}: placement count"
        );
    }
    assert_eq!(
        plan_key(&actual.layout),
        expected["layoutKey"].as_str().unwrap(),
        "{label}: completed layout"
    );
    if let Some(layout) = expected.get("layout") {
        assert_eq!(actual.layout, layout_of(layout), "{label}: layout");
    }
    close(
        actual.proxy_score,
        expected["proxyScore"].as_f64().unwrap(),
        &format!("{label}: proxy score"),
    );
    close(
        actual.proxy_gain,
        expected["proxyGain"].as_f64().unwrap(),
        &format!("{label}: proxy gain"),
    );
    match expected["operation"].as_object() {
        Some(operation) => assert_estimate(
            &Value::Object(operation.clone()),
            actual.operation.as_ref().expect("a simulated row"),
            label,
        ),
        None => {
            assert!(actual.operation.is_none(), "{label}: no operation");
            // A zero-token plan screens geometry only, so it ranks no row.
            assert_eq!(actual.objective_value, 0.0, "{label}: no objective value");
            assert_eq!(actual.objective_gain, 0.0, "{label}: no objective gain");
            assert_eq!(actual.gain_per_slot, 0.0, "{label}: no gain per slot");
            assert!(actual.confidence.is_none(), "{label}: no confidence");
            assert!(
                actual.simulation_trials.is_none(),
                "{label}: no simulation trials"
            );
            return;
        }
    }
    close(
        actual.objective_value,
        expected["objectiveValue"].as_f64().unwrap(),
        &format!("{label}: objective value"),
    );
    close(
        actual.objective_gain,
        expected["objectiveGain"].as_f64().unwrap(),
        &format!("{label}: objective gain"),
    );
    close(
        actual.gain_per_slot,
        expected["gainPerSlot"].as_f64().unwrap(),
        &format!("{label}: gain per slot"),
    );
    match expected["confidence"].as_object() {
        Some(confidence) => {
            let actual = actual.confidence.as_ref().expect("a paired confidence");
            assert_eq!(
                actual.trials,
                confidence["trials"].as_u64().unwrap() as usize,
                "{label}: paired trials"
            );
            close(
                actual.chance_beats_baseline,
                confidence["chanceBeatsBaseline"].as_f64().unwrap(),
                &format!("{label}: chance beats baseline"),
            );
            close(
                actual.mean_difference,
                confidence["meanDifference"].as_f64().unwrap(),
                &format!("{label}: mean difference"),
            );
            close(
                actual.low,
                confidence["low"].as_f64().unwrap(),
                &format!("{label}: difference low"),
            );
            close(
                actual.high,
                confidence["high"].as_f64().unwrap(),
                &format!("{label}: difference high"),
            );
        }
        None => assert!(actual.confidence.is_none(), "{label}: no confidence"),
    }
    close_opt(
        actual.chance_beats_baseline,
        &expected["chanceBeatsBaseline"],
        &format!("{label}: row chance"),
    );
    assert_eq!(
        actual.simulation_trials,
        expected["simulationTrials"]
            .as_u64()
            .map(|trials| trials as u32),
        "{label}: simulation trials"
    );
    assert_eq!(
        actual.finalist,
        expected["finalist"].as_bool().unwrap_or(false),
        "{label}: finalist"
    );
}

fn assert_plan(name: &str) {
    let (jelly, case) = jelly(name);
    let expected = &case["sections"];
    let plan = Plan::run(&jelly, options(&case));

    assert_eq!(
        plan.objective.id(),
        expected["objective"].as_str().unwrap(),
        "{name}: objective"
    );
    assert_eq!(
        plan.exp_cell_type,
        expected["expCellType"]
            .as_i64()
            .filter(|cell_type| *cell_type >= 0)
            .map(|cell_type| cell_type as usize),
        "{name}: EXP target"
    );
    assert_eq!(
        plan.quality.id(),
        expected["quality"].as_str().unwrap(),
        "{name}: quality"
    );
    assert_eq!(
        plan.strategy.id(),
        expected["strategy"].as_str().unwrap(),
        "{name}: strategy"
    );
    let profile = &expected["qualityProfile"];
    assert_eq!(
        plan.quality_profile.screening_trials,
        profile["screeningTrials"].as_u64().unwrap() as u32,
        "{name}: screening trials"
    );
    assert_eq!(
        plan.quality_profile.racing_trials,
        profile["racingTrials"].as_u64().unwrap() as u32,
        "{name}: racing trials"
    );
    assert_eq!(
        plan.quality_profile.final_trials,
        profile["finalTrials"].as_u64().unwrap() as u32,
        "{name}: final trials"
    );
    let stages: Vec<u32> = profile["stages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|stage| stage.as_u64().unwrap() as u32)
        .collect();
    assert_eq!(plan.quality_profile.stages, stages, "{name}: stages");
    assert_eq!(
        plan.tokens,
        expected["tokens"].as_u64().unwrap() as usize,
        "{name}: tokens"
    );
    assert_eq!(
        plan.requested_tokens,
        expected["requestedTokens"].as_u64().unwrap() as usize,
        "{name}: requested tokens"
    );
    assert_eq!(
        plan.available_tokens,
        expected["availableTokens"].as_u64().unwrap() as usize,
        "{name}: available tokens"
    );
    assert_eq!(
        plan.baseline_layout,
        layout_of(&expected["baselineLayout"]),
        "{name}: baseline layout"
    );
    let mut unlocked: Vec<usize> = expected["baselineUnlockedSlots"]
        .as_array()
        .unwrap()
        .iter()
        .map(|slot| slot.as_u64().unwrap() as usize)
        .collect();
    unlocked.sort_unstable();
    assert_eq!(
        plan.baseline_unlocked_slots, unlocked,
        "{name}: baseline unlocked slots"
    );
    assert_eq!(
        plan.baseline_obstruction,
        expected["baselineObstruction"].as_u64().unwrap() as usize,
        "{name}: baseline obstruction"
    );
    match expected["baselineOperation"].as_object() {
        Some(operation) => assert_estimate(
            &Value::Object(operation.clone()),
            plan.baseline_operation
                .as_ref()
                .expect("a baseline operation"),
            &format!("{name}: baseline"),
        ),
        None => assert!(
            plan.baseline_operation.is_none(),
            "{name}: no baseline operation"
        ),
    }
    assert_eq!(
        plan.plots.len(),
        expected["plots"].as_array().unwrap().len(),
        "{name}: plot rows"
    );
    for (expected, actual) in expected["plots"]
        .as_array()
        .unwrap()
        .iter()
        .zip(&plan.plots)
    {
        assert_row(
            expected,
            actual,
            &format!("{name}: plot {:?}", expected["plotId"]),
        );
    }
    assert_eq!(
        plan.sequences.len(),
        expected["sequences"].as_array().unwrap().len(),
        "{name}: sequence rows"
    );
    for (expected, actual) in expected["sequences"]
        .as_array()
        .unwrap()
        .iter()
        .zip(&plan.sequences)
    {
        assert_row(
            expected,
            actual,
            &format!("{name}: sequence {:?}", expected["plotIds"]),
        );
    }
    let recommended: Vec<usize> = expected["recommendedPlotIds"]
        .as_array()
        .unwrap()
        .iter()
        .map(|plot| plot.as_u64().unwrap() as usize)
        .collect();
    assert_eq!(
        plan.recommended
            .as_ref()
            .map(|row| row.plot_ids.clone())
            .unwrap_or_default(),
        recommended,
        "{name}: recommendation"
    );
    assert_eq!(
        plan.final_layout,
        layout_of(&expected["finalLayout"]),
        "{name}: final layout"
    );
    match expected["finalOperation"].as_object() {
        Some(operation) => assert_estimate(
            &Value::Object(operation.clone()),
            plan.final_operation.as_ref().expect("a final operation"),
            &format!("{name}: final"),
        ),
        None => assert!(plan.final_operation.is_none(), "{name}: no final operation"),
    }
    assert_eq!(
        plan.note,
        expected["note"].as_str().unwrap(),
        "{name}: note"
    );
    assert_plan_rules(&jelly, &plan, name);
}

/// The planner's own rules: every row buys unpurchased sections and fills a
/// legal board for the save those sections make.
fn assert_plan_rules(jelly: &idleon_jelly::Jelly, plan: &Plan, name: &str) {
    assert!(
        plan.tokens <= plan.requested_tokens,
        "{name}: more sections planned than the request asked for"
    );
    assert!(
        plan.tokens <= idleon_jelly::data().plots.len(),
        "{name}: more sections planned than the board has"
    );
    for rows in [&plan.plots, &plan.sequences] {
        for row in rows.iter() {
            assert!(
                row.plot_ids.iter().all(|plot| !jelly.plots.contains(plot)),
                "{name}: a row buys a section the save already owns"
            );
            for slot in &row.new_slots {
                assert!(
                    !board::unlocked_slots(jelly)[*slot],
                    "{name}: a new slot was already unlocked"
                );
            }
            assert_eq!(
                row.new_usable_slots,
                row.new_slots.len(),
                "{name}: the new slot count matches the slots"
            );
            let mut expanded = jelly.clone();
            expanded.plots.extend(row.plot_ids.iter().copied());
            board::validate(&expanded, &row.layout)
                .unwrap_or_else(|reason| panic!("{name}: the row board is illegal: {reason}"));
        }
    }
    if plan.tokens == 0 {
        assert!(
            plan.sequences.is_empty(),
            "{name}: a zero-token plan ranks no sequence"
        );
        assert_eq!(
            plan.final_layout, plan.baseline_layout,
            "{name}: no recommendation keeps the baseline"
        );
    }
}

#[test]
fn the_plan_matches_upstream() {
    for name in [
        "sections-single",
        "sections-beam-narrow",
        "sections-beam-wide",
        "sections-low-risk",
        "sections-zero",
        "sections-exp",
        "sections-dps",
    ] {
        assert_plan(name);
    }
}

#[test]
fn a_narrow_beam_searches_different_sequences() {
    let (save, narrow_case) = jelly("sections-beam-narrow");
    let (_, wide_case) = jelly("sections-beam-wide");
    let narrow = Plan::run(&save, options(&narrow_case));
    let wide = Plan::run(&save, options(&wide_case));
    assert_eq!(narrow.beam_width, 2);
    assert_eq!(wide.beam_width, 8);
    let ids = |plan: &Plan| {
        plan.sequences
            .iter()
            .map(|row| row.plot_ids.clone())
            .collect::<Vec<_>>()
    };
    assert_ne!(
        ids(&narrow),
        ids(&wide),
        "the beam width must change which section combinations the search keeps"
    );
}

#[test]
fn the_plan_is_reproducible() {
    let (jelly, case) = jelly("sections-beam-narrow");
    let first = Plan::run(&jelly, options(&case));
    let second = Plan::run(&jelly, options(&case));
    assert_eq!(first, second, "one input must give one plan");
    let mut reseeded = options(&case);
    reseeded.seed += 1;
    let reseeded = Plan::run(&jelly, reseeded);
    assert_ne!(
        first, reseeded,
        "the seed moves the shared-seed trials the plan reports"
    );
}

#[test]
fn a_stopped_plan_reports_nothing() {
    let (jelly, case) = jelly("sections-single");
    let stopped = Plan::run_with(&jelly, options(&case), |_| false);
    assert!(stopped.is_none(), "the first report stops the plan");
}
