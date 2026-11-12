//! The Jelly Operator state, layout rules and metrics against the upstream JS.
//!
//! Every expected number comes from `fixtures/jelly.json`, which
//! `script/jelly-oracle.mjs` writes by running the upstream calculator on the
//! committed save and on three documented overrides; see `tests/common/mod.rs`
//! for the fixture helpers.

mod common;

use common::{close, jelly, numbers};
use idleon_core::jelly::board::{self, Layout};
use idleon_core::jelly::metrics::MetricOptions;
use idleon_core::jelly::operation::simulate;
use idleon_core::jelly::{OperationOptions, Progress};

#[test]
fn progress_and_unlocks_match_upstream() {
    for name in ["save", "small-board", "clear"] {
        let (jelly, case) = jelly(name);
        let state = &case["state"];
        let progress = &state["progress"];
        assert_eq!(
            jelly.progress.obstruction,
            progress["obstruction"].as_u64().unwrap() as usize,
            "{name}: obstruction"
        );
        assert_eq!(
            jelly.progress.fever,
            progress["fever"].as_u64().unwrap() as u32,
            "{name}: fever"
        );
        close(
            jelly.progress.tries,
            progress["tries"].as_f64().unwrap(),
            "tries",
        );
        close(
            jelly.progress.bloodcells,
            progress["bloodcells"].as_f64().unwrap(),
            "bloodcells",
        );
        close(
            jelly.progress.best_dps,
            progress["bestDps"].as_f64().unwrap(),
            "best DPS",
        );
        close(
            jelly.progress.best_bloodcells,
            progress["bestBloodcells"].as_f64().unwrap(),
            "best Bloodcells",
        );
        assert_eq!(
            jelly.units_owned(),
            state["unitsOwned"].as_u64().unwrap() as usize,
            "{name}: units owned"
        );
        let unlocked: Vec<usize> = board::unlocked_slots(&jelly)
            .iter()
            .enumerate()
            .filter(|(_, open)| **open)
            .map(|(slot, _)| slot)
            .collect();
        let expected: Vec<usize> = numbers(&state["unlocked"])
            .into_iter()
            .map(|slot| slot as usize)
            .collect();
        assert_eq!(unlocked, expected, "{name}: unlocked slots");
        close(
            board::slot_purchases_left(&jelly),
            state["slotPurchasesLeft"].as_f64().unwrap(),
            "slot purchases left",
        );
        close(
            jelly.daily_tries(),
            state["dailyTries"].as_f64().unwrap(),
            "daily tries",
        );
        for cell_type in 0..9 {
            assert_eq!(
                jelly.cell_level(cell_type),
                state["cellLevels"][cell_type].as_u64().unwrap() as u32,
                "{name}: cell {cell_type} level"
            );
            close(
                jelly.cell_exp(cell_type),
                state["cellExp"][cell_type].as_f64().unwrap(),
                "cell EXP",
            );
        }
    }
}

#[test]
fn multipliers_match_upstream() {
    for name in ["save", "small-board", "clear"] {
        let (jelly, case) = jelly(name);
        let state = &case["state"];
        let fever = Some(jelly.progress.fever);
        let total = jelly.total_cell_level();
        close(
            jelly.cell_damage_multiplier(MetricOptions {
                fever,
                total_cell_level: Some(total),
                ..MetricOptions::default()
            }),
            state["cellDamageMultiplier"].as_f64().unwrap(),
            "cell damage multiplier",
        );
        close(
            jelly.cell_speed_multiplier(fever),
            state["cellSpeedMultiplier"].as_f64().unwrap(),
            "cell speed multiplier",
        );
        close(
            jelly.cell_exp_multiplier(fever),
            state["cellExpMultiplier"].as_f64().unwrap(),
            "cell EXP multiplier",
        );
        close(
            jelly.currency_multiplier(fever, total, jelly.progress.best_dps),
            state["currencyMultiplier"].as_f64().unwrap(),
            "currency multiplier",
        );
    }
}

#[test]
fn layout_metrics_match_upstream() {
    for name in ["save", "small-board", "clear"] {
        let (jelly, case) = jelly(name);
        let expected = &case["metrics"];
        let metrics = jelly.layout_metrics(
            &jelly.layout,
            MetricOptions {
                fever: Some(jelly.progress.fever),
                ..MetricOptions::default()
            },
        );
        assert!(metrics.valid, "{name}: {}", metrics.reason);
        assert_eq!(
            metrics.valid,
            expected["valid"].as_bool().unwrap(),
            "{name}"
        );
        close(
            metrics.damage_passive,
            expected["damagePassive"].as_f64().unwrap(),
            "damage passive",
        );
        close(
            metrics.speed_passive,
            expected["speedPassive"].as_f64().unwrap(),
            "speed passive",
        );
        close(
            metrics.virus_multiplier,
            expected["virusMultiplier"].as_f64().unwrap(),
            "virus",
        );
        close(metrics.dps, expected["dps"].as_f64().unwrap(), "dps");
        close(
            metrics.attacks_per_second,
            expected["attacksPerSecond"].as_f64().unwrap(),
            "attacks per second",
        );
        assert_eq!(
            metrics.infected_slots,
            expected["infectedSlots"].as_u64().unwrap() as usize,
            "{name}: infected slots"
        );
        for index in 0..9 {
            close(
                metrics.counts[index],
                expected["counts"][index].as_f64().unwrap(),
                "counts",
            );
            close(
                metrics.raw_counts[index],
                expected["rawCounts"][index].as_f64().unwrap(),
                "raw counts",
            );
        }
        let expected_cells = expected["cells"].as_array().unwrap();
        assert_eq!(
            metrics.cells.len(),
            expected_cells.len(),
            "{name}: cell count"
        );
        for (cell, expected) in metrics.cells.iter().zip(expected_cells) {
            assert_eq!(cell.anchor, expected["anchor"].as_u64().unwrap() as usize);
            assert_eq!(cell.cell_type, expected["type"].as_u64().unwrap() as usize);
            close(
                cell.organelle,
                expected["organelle"].as_f64().unwrap(),
                "organelle",
            );
            close(
                cell.proximity,
                expected["proximity"].as_f64().unwrap(),
                "proximity",
            );
            close(
                cell.damage,
                expected["damage"].as_f64().unwrap(),
                "cell damage",
            );
            close(
                cell.cooldown_progress,
                expected["cooldownProgress"].as_f64().unwrap(),
                "cooldown",
            );
            close(
                cell.progress_per_frame,
                expected["progressPerFrame"].as_f64().unwrap(),
                "progress per frame",
            );
            close(
                cell.frames_per_attack,
                expected["framesPerAttack"].as_f64().unwrap(),
                "frames per attack",
            );
            close(cell.dps, expected["dps"].as_f64().unwrap(), "cell dps");
        }
    }
}

#[test]
fn layout_rules_match_upstream() {
    for name in ["locked-cell", "overlap"] {
        let (jelly, case) = jelly(name);
        let expected = &case["validation"];
        let result = board::validate(&jelly, &jelly.layout);
        if expected["valid"].as_bool().unwrap() {
            assert!(result.is_ok(), "{name}: {}", result.unwrap_err());
        } else {
            assert_eq!(
                result.unwrap_err(),
                expected["reason"].as_str().unwrap(),
                "{name}: reason"
            );
        }
    }
}

#[test]
fn every_cell_type_is_placeable_and_loads_back() {
    let (jelly, _case) = jelly("small-board");
    let mut layout = Layout::new();
    let mut placed = Vec::new();
    for cell_type in 0..jelly.units_owned() {
        let anchor = board::unlocked_slots(&jelly)
            .iter()
            .enumerate()
            .filter(|(_, open)| **open)
            .map(|(slot, _)| slot)
            .find(|slot| board::place(&jelly, &layout, *slot, cell_type).is_ok())
            .unwrap_or_else(|| panic!("no free slot for cell type {cell_type}"));
        layout = board::place(&jelly, &layout, anchor, cell_type)
            .unwrap_or_else(|reason| panic!("cell type {cell_type} refused: {reason}"));
        placed.push((anchor, cell_type));
    }
    assert_eq!(placed.len(), 8, "every cell type is placeable");
    let occupancy = board::occupancy(&layout);
    assert!(
        occupancy.len() >= 8,
        "each placed cell covers its footprint"
    );
    assert!(!board::layout_key(&layout).is_empty());
    let anchor = placed[0].0;
    layout = board::remove(&layout, anchor);
    assert!(!layout.contains_key(&anchor), "remove drops the anchor");
    board::validate(&jelly, &layout).expect("removing keeps the layout valid");
}

#[test]
fn trial_seeds_are_separate_attempts() {
    let (jelly, _case) = jelly("clear");
    let trial = |seed| {
        simulate(
            &jelly,
            &jelly.layout,
            OperationOptions {
                seed,
                ..OperationOptions::default()
            },
        )
    };
    assert_eq!(trial(3), trial(3), "the same seed is the same attempt");
    assert_ne!(trial(3), trial(4), "a new seed is a new attempt");
}

#[test]
fn progress_and_board_come_from_the_save() {
    let (jelly, _case) = jelly("save");
    assert_ne!(jelly.progress, Progress::default());
    assert!(jelly.has_data());
    assert!(!jelly.layout.is_empty());
}

#[test]
fn a_cancelled_estimate_reports_nothing() {
    let (jelly, _case) = jelly("save");
    let stopped =
        idleon_core::jelly::Estimate::run_with(&jelly, &jelly.layout, 4, 9, |done| done < 2);
    assert!(stopped.is_none(), "a stopped run stores nothing");
}

#[test]
fn estimate_is_deterministic_and_layout_sensitive() {
    let (jelly, _case) = jelly("save");
    let first = idleon_core::jelly::Estimate::run(&jelly, &jelly.layout, 2, 5);
    let second = idleon_core::jelly::Estimate::run(&jelly, &jelly.layout, 2, 5);
    assert_eq!(first, second, "a seed makes the estimate reproducible");
    let empty = idleon_core::jelly::Estimate::run(&jelly, &Layout::new(), 2, 5);
    assert!(empty.valid);
    assert!(
        empty.mean_damage < first.mean_damage,
        "an empty board deals less"
    );
}
