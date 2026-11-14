//! Insight ROI: the port against the upstream maths, and the row invariants.
//!
//! `fixtures/research-roi.json` is written by `script/research-roi-oracle.mjs`
//! from the upstream `js/optimizers/monos.js` and `research-math.js` on a
//! synthetic state, so the port is compared with the calculator's own output
//! rather than with numbers picked here.

use idleon_common::Control;
use idleon_research::insight::{self, Sort};
use idleon_research::model::{
    Effort, GridState, Mag, MagKind, Observations, SimContext, Simulation,
};
use idleon_research::{game, sim};
use serde_json::Value;

fn oracle() -> Value {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/research-roi.json"
    );
    let text = std::fs::read_to_string(path).expect("fixtures/research-roi.json is committed");
    serde_json::from_str(&text).expect("the oracle fixture is valid JSON")
}

/// The synthetic state the oracle ran, rebuilt on this side.
fn state(oracle: &Value) -> (Simulation, SimContext) {
    let state = &oracle["state"];
    let context: SimContext =
        serde_json::from_value(oracle["context"].clone()).expect("the context decodes");
    let mut grid = GridState::empty();
    for (cell, level) in state["gridLevels"]
        .as_array()
        .expect("grid levels")
        .iter()
        .enumerate()
    {
        grid.set(cell, level.as_u64().unwrap_or(0) as u32);
    }
    let found = state["occFound"].as_u64().expect("found count") as usize;
    let level = |key: &str| -> Vec<u32> {
        state[key]
            .as_array()
            .expect("an array")
            .iter()
            .map(|value| value.as_u64().unwrap_or(0) as u32)
            .collect()
    };
    let progress = |key: &str| -> Vec<f64> {
        state[key]
            .as_array()
            .expect("an array")
            .iter()
            .map(|value| value.as_f64().unwrap_or(0.0))
            .collect()
    };
    let mut observations = Observations::default();
    for (obs, flag) in observations.found.iter_mut().enumerate() {
        *flag = obs < found;
    }
    observations.insight_level = level("insightLevels");
    observations.insight_exp = progress("insightProgress");
    let mags = state["mags"]
        .as_array()
        .expect("mags")
        .iter()
        .map(|mag| {
            let kind = match mag["type"].as_u64() {
                Some(2) => MagKind::Kaleidoscope,
                Some(1) => MagKind::Monocle,
                _ => MagKind::Regular,
            };
            Mag::new(kind, Some(mag["slot"].as_u64().expect("slot") as usize))
        })
        .collect();
    let overlay: Vec<i32> = state["shapeOverlay"]
        .as_array()
        .expect("the shape overlay")
        .iter()
        .map(|value| value.as_i64().unwrap_or(-1) as i32)
        .collect();
    let simulation = Simulation {
        research_level: state["researchLevel"].as_u64().expect("level") as u32,
        grid,
        observations,
        mags,
        saved_overlay: Some(overlay),
        ..Simulation::default()
    };
    (simulation, context)
}

fn close(left: f64, right: f64, what: &str) {
    let scale = left.abs().max(right.abs()).max(1.0);
    assert!(
        (left - right).abs() <= 1e-9 * scale,
        "{what}: port {left} vs upstream {right}"
    );
}

#[test]
fn the_usable_observations_match_the_upstream_slots() {
    let oracle = oracle();
    let (simulation, _) = state(&oracle);
    let expected: Vec<usize> = oracle["availableSlots"]
        .as_array()
        .expect("slots")
        .iter()
        .map(|slot| slot.as_u64().expect("a slot") as usize)
        .collect();
    assert_eq!(
        sim::available_slots(simulation.research_level, &simulation.observations),
        expected
    );
    assert_eq!(
        sim::mag_cap(simulation.research_level) as u64,
        oracle["magCap"].as_u64().expect("mag cap")
    );
}

#[test]
fn the_insight_formulas_match_the_upstream_maths() {
    let oracle = oracle();
    let (simulation, context) = state(&oracle);
    let overlay = simulation.overlay();
    let kal_map = sim::kaleidoscope_map(&simulation.mags, game().observations.len());
    let kal_base = sim::kalei_base(&simulation.grid, &overlay, &context);

    for (obs, expected) in oracle["insightRates"]
        .as_array()
        .expect("rates")
        .iter()
        .map(|entry| {
            (
                entry[0].as_u64().expect("obs") as usize,
                entry[1].as_f64().expect("rate"),
            )
        })
    {
        let rate = sim::insight_rate(obs, &simulation, &overlay, &context, &kal_map, kal_base);
        close(
            rate,
            expected,
            &format!("insight rate of observation {obs}"),
        );
    }

    for entry in oracle["insightReq"].as_array().expect("requirements") {
        let obs = entry[0].as_u64().expect("obs") as usize;
        let level = entry[1].as_u64().expect("level") as u32;
        let expected = entry[2].as_f64().expect("requirement");
        close(
            sim::insight_exp_required(obs, level),
            expected,
            &format!("insight EXP for observation {obs} at level {level}"),
        );
    }
}

#[test]
fn the_layout_rates_match_the_upstream_totals() {
    let oracle = oracle();
    let (simulation, context) = state(&oracle);
    let expected = &oracle["expRates"];
    close(
        sim::exp_rate(&simulation, &context).total,
        expected["base"].as_f64().expect("the base rate"),
        "baseline EXP/hr",
    );
    let mut levelled = simulation.clone();
    levelled.observations.insight_level[6] += 1;
    close(
        sim::exp_rate(&levelled, &context).total,
        expected["afterInsight"]
            .as_f64()
            .expect("the levelled rate"),
        "EXP/hr with one more insight level",
    );
    // The all-bonus multiplier scales every grid square; the oracle's second
    // context has it at one, so this covers the wiring.
    let mut no_bonus = context.clone();
    no_bonus.all_bonus_multi = 1.0;
    close(
        sim::exp_rate(&simulation, &no_bonus).total,
        expected["bare"].as_f64().expect("the bare rate"),
        "EXP/hr without the all-bonus multiplier",
    );
}

#[test]
fn the_concentrated_layouts_match_the_upstream_builder() {
    let oracle = oracle();
    let (simulation, context) = state(&oracle);
    let overlay = simulation.overlay();
    let entries = oracle["concentrated"].as_array().expect("layouts");
    assert!(!entries.is_empty(), "the oracle has a layout to compare");
    for entry in entries {
        let target = entry["target"].as_u64().expect("target") as usize;
        let mags =
            insight::concentrated_layout(&simulation, &context, &simulation.mags, target, &overlay);
        let built: Vec<(u64, usize)> = mags
            .iter()
            .map(|mag| {
                let kind = match mag.kind {
                    MagKind::Regular => 0,
                    MagKind::Monocle => 1,
                    MagKind::Kaleidoscope => 2,
                };
                (kind, mag.slot.expect("a placed magnifier"))
            })
            .collect();
        let expected: Vec<(u64, usize)> = entry["layout"]
            .as_array()
            .expect("layout")
            .iter()
            .map(|mag| {
                (
                    mag["type"].as_u64().expect("type"),
                    mag["slot"].as_u64().expect("slot") as usize,
                )
            })
            .collect();
        assert_eq!(built, expected, "the concentrate layout for {target}");
        let mut trial = simulation.clone();
        trial.mags = mags;
        let kal_map = sim::kaleidoscope_map(&trial.mags, game().observations.len());
        let kal_base = sim::kalei_base(&simulation.grid, &overlay, &context);
        close(
            sim::insight_rate(target, &trial, &overlay, &context, &kal_map, kal_base),
            entry["insightRate"].as_f64().expect("the insight rate"),
            &format!("the grind insight rate on {target}"),
        );
        close(
            sim::exp_rate(&trial, &context).total,
            entry["expRate"].as_f64().expect("the grind rate"),
            &format!("the grind EXP/hr on {target}"),
        );
    }
}

/// A state with a handful of observations, so the whole analysis runs fast.
fn small() -> (Simulation, SimContext) {
    let mut grid = GridState::seeded(1);
    grid.set(31, 2);
    grid.set(91, 3);
    grid.set(92, 1);
    grid.set(93, 2);
    grid.set(72, 2);
    let mut observations = Observations::all_at(1);
    observations
        .found
        .iter_mut()
        .skip(6)
        .for_each(|found| *found = false);
    observations.insight_level[0] = 3;
    observations.insight_exp[0] = 400.0;
    let simulation = Simulation {
        research_level: 70,
        research_exp: 0.0,
        grid,
        observations,
        mags: vec![
            Mag::new(MagKind::Regular, Some(0)),
            Mag::new(MagKind::Regular, Some(1)),
            Mag::new(MagKind::Regular, Some(2)),
            Mag::new(MagKind::Monocle, Some(0)),
            Mag::new(MagKind::Monocle, Some(1)),
            Mag::new(MagKind::Kaleidoscope, Some(2)),
        ],
        ..Simulation::default()
    };
    (simulation, SimContext::default())
}

#[test]
fn every_usable_observation_gets_a_row_with_the_upstream_columns() {
    let (simulation, context) = small();
    let outcome = insight::analyze(&simulation, &context, Effort::Quick, &mut |_| {
        Control::Continue
    })
    .expect("a quick run is not cancelled");
    let usable = sim::available_slots(simulation.research_level, &simulation.observations);
    assert!(!usable.is_empty());
    assert_eq!(outcome.rows.len(), usable.len());
    assert_eq!(outcome.monocles, 2);
    // The stored order is the default sort: the best break-even first.
    for pair in outcome.rows.windows(2) {
        assert!(
            pair[0].scenarios[0].break_even_hours <= pair[1].scenarios[0].break_even_hours,
            "rows are stored in break-even order"
        );
    }
    // Each row carries the one- and two-level scenarios in that order.
    for row in &outcome.rows {
        assert_eq!(
            row.after_overlay.len(),
            simulation.overlay().len(),
            "{} keeps a full post-grind overlay",
            row.name
        );
        assert_eq!(
            row.scenarios.map(|scenario| scenario.level_gain),
            [1, 2],
            "{} lists +1 then +2",
            row.name
        );
        assert!(!row.name.is_empty());
    }
    // Every sort mode reorders the same rows.
    for sort in Sort::ALL {
        let sorted: Vec<usize> = outcome.sorted(sort).iter().map(|row| row.obs).collect();
        assert_eq!(sorted.len(), outcome.rows.len());
    }
}

#[test]
fn every_scenario_is_built_from_the_insight_formulas() {
    let (simulation, context) = small();
    let quick = match insight::analyze(&simulation, &context, Effort::Quick, &mut |_| {
        Control::Continue
    }) {
        Some(outcome) => outcome,
        None => panic!("a quick run is not cancelled"),
    };
    assert!(quick.base_rate > 0.0, "the baseline earns EXP");
    for row in &quick.rows {
        let one = &row.scenarios[0];
        let two = &row.scenarios[1];
        assert_eq!(one.level_gain, 1);
        assert_eq!(two.level_gain, 2);
        if one.worth {
            assert!(one.rate_gain > 0.0 && one.break_even_hours.is_finite());
        }
        assert_eq!(
            one.total_exp_lost,
            if one.exp_lost_per_hour > 0.0 {
                one.exp_lost_per_hour * one.grind_hours
            } else {
                0.0
            }
        );
        assert_eq!(
            one.break_even_hours,
            one.grind_hours + one.recoup_hours,
            "break-even is the grind plus the recoup"
        );
        // The gain is the post-grind rate against the same baseline.
        assert!(
            one.post_rate >= 0.0 && one.grind_rate >= 0.0,
            "{} reports non-negative rates",
            row.name
        );
        if one.rate_gain > 0.0 {
            assert_eq!(one.recoup_hours, one.total_exp_lost / one.rate_gain);
        } else {
            assert!(one.recoup_hours.is_infinite());
        }
        // A two-level grind never finishes sooner than a one-level grind.
        assert!(
            two.grind_hours >= one.grind_hours || two.grind_hours.is_infinite(),
            "{}: +2 takes longer than +1",
            row.name
        );
        // The grind hours are the insight requirement over the insight rate.
        let level = simulation.observations.insight(row.obs);
        let banked = simulation.observations.insight_exp[row.obs];
        if one.insight_rate > 0.0 {
            close(
                one.grind_hours,
                (sim::insight_exp_required(row.obs, level) - banked).max(0.0) / one.insight_rate,
                &format!("{}: the one-level grind", row.name),
            );
        } else {
            assert!(one.grind_hours.is_infinite());
        }
    }
}

#[test]
fn no_monocle_means_no_rows() {
    let (mut simulation, context) = small();
    simulation.mags.retain(|mag| mag.kind != MagKind::Monocle);
    let outcome = insight::analyze(&simulation, &context, Effort::Quick, &mut |_| {
        Control::Continue
    })
    .expect("a quick run is not cancelled");
    assert_eq!(outcome.monocles, 0);
    assert!(outcome.rows.is_empty());
}

#[test]
fn a_cancelled_run_reports_nothing() {
    let (simulation, context) = small();
    assert!(
        insight::analyze(&simulation, &context, Effort::Quick, &mut |_| Control::Stop).is_none(),
        "a cancelled analysis stores no result"
    );
}

#[test]
#[ignore = "long simulation: full-account insight ROI"]
fn the_account_fixture_reports_every_usable_observation() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/data.json");
    let save = std::fs::read_to_string(path).expect("fixtures/data.json is committed");
    let imported = idleon_import::import(&save).expect("the fixture imports");
    let outcome = insight::analyze(
        &imported.simulation,
        &imported.context,
        Effort::Quick,
        &mut |_| Control::Continue,
    )
    .expect("the run is not cancelled");
    let usable = sim::available_slots(
        imported.simulation.research_level,
        &imported.simulation.observations,
    );
    assert_eq!(outcome.rows.len(), usable.len());
    assert!(outcome.base_rate > 0.0, "the fixture earns EXP");
    let rows: std::collections::BTreeSet<usize> = outcome.rows.iter().map(|row| row.obs).collect();
    assert_eq!(rows, usable.into_iter().collect());
}
