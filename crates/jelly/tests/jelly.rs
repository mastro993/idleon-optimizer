//! The Jelly Operator state, layout rules, metrics and operation runtime on the
//! committed save; see `tests/common/mod.rs` for the save setups.

mod common;

use common::{clear, save, small_board};
use idleon_jelly::board::{self, Layout};
use idleon_jelly::metrics::MetricOptions;
use idleon_jelly::operation::simulate;
use idleon_jelly::{OperationOptions, Progress};

#[test]
fn the_layout_rules_refuse_a_locked_cell_and_an_overlap() {
    // Two cell types owned on the first stage: type 7 is not unlocked yet,
    // and a two-slot cell at 77 already covers 78.
    let mut jelly = save();
    jelly.progress.obstruction = 0;
    jelly.upgrades = vec![0; 40];
    jelly.upgrades[0] = 1;
    jelly.upgrades[1] = 1;
    let locked = Layout::from([(77, 7)]);
    assert_eq!(
        board::validate(&jelly, &locked).unwrap_err(),
        "Cell 7 is locked"
    );
    let overlap = Layout::from([(77, 1), (78, 0)]);
    assert_eq!(
        board::validate(&jelly, &overlap).unwrap_err(),
        "Cells overlap at slot 78"
    );
}

#[test]
fn every_saved_board_has_valid_metrics() {
    for jelly in [save(), small_board(), clear()] {
        let metrics = jelly.layout_metrics(
            &jelly.layout,
            MetricOptions {
                fever: Some(jelly.progress.fever),
                ..MetricOptions::default()
            },
        );
        assert!(metrics.valid, "{}", metrics.reason);
        assert_eq!(metrics.cells.len(), jelly.layout.len());
    }
}

#[test]
fn every_cell_type_is_placeable_and_loads_back() {
    let jelly = small_board();
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
    let anchor = placed[0].0;
    layout = board::remove(&layout, anchor);
    assert!(!layout.contains_key(&anchor), "remove drops the anchor");
    board::validate(&jelly, &layout).expect("removing keeps the layout valid");
}

#[test]
fn trial_seeds_are_separate_attempts() {
    let jelly = clear();
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
    let jelly = save();
    assert_ne!(jelly.progress, Progress::default());
    assert!(jelly.has_data());
    assert!(!jelly.layout.is_empty());
}

#[test]
fn a_cancelled_estimate_reports_nothing() {
    let jelly = save();
    let stopped = idleon_jelly::Estimate::run_with(&jelly, &jelly.layout, 4, 9, |done| done < 2);
    assert!(stopped.is_none(), "a stopped run stores nothing");
}

#[test]
fn estimate_is_deterministic_and_layout_sensitive() {
    let jelly = save();
    let first = idleon_jelly::Estimate::run(&jelly, &jelly.layout, 2, 5);
    let second = idleon_jelly::Estimate::run(&jelly, &jelly.layout, 2, 5);
    assert_eq!(first, second, "a seed makes the estimate reproducible");
    let empty = idleon_jelly::Estimate::run(&jelly, &Layout::new(), 2, 5);
    assert!(empty.valid);
    assert!(
        empty.mean_damage < first.mean_damage,
        "an empty board deals less"
    );
}

#[test]
fn normalizing_a_sandbox_drops_the_cells_that_no_longer_fit() {
    let mut jelly = save();
    jelly.progress.obstruction = 2;
    let mut layout = Layout::new();
    layout.insert(75, 0);
    layout.insert(77, 0);
    let (normalized, removed) = board::normalize(&jelly, &layout);
    assert_eq!(removed, 0, "the second clear opens slot 75");
    assert_eq!(normalized.len(), 2);
    // Stepping the sandbox back to its first stage relocks the second clear.
    jelly.progress.obstruction = 0;
    let (normalized, removed) = board::normalize(&jelly, &layout);
    assert_eq!(removed, 1);
    assert_eq!(normalized.keys().copied().collect::<Vec<_>>(), vec![77]);
}

#[test]
fn an_empty_sandbox_board_runs_a_trial_with_no_projectiles() {
    let mut jelly = save();
    jelly.upgrades = vec![0; 40];
    let options = OperationOptions {
        obstruction: Some(0),
        fever: Some(0),
        seed: 4,
        ..OperationOptions::default()
    };
    let trial = simulate(&jelly, &Layout::new(), options);
    assert!(trial.valid, "{}", trial.reason);
    assert!(!trial.success);
    assert_eq!(trial.ended_by, "timer");
    assert_eq!(trial.shots, 0.0);
    assert_eq!(trial.hits, 0.0);
    assert_eq!(trial.damage, 0.0);
    assert_eq!(trial.normal_time, idleon_jelly::boss_time(0));
    assert!(trial.time >= idleon_jelly::boss_time(0));
    assert_eq!(
        trial,
        simulate(&jelly, &Layout::new(), options),
        "the same seed is the same attempt"
    );
}

#[test]
fn the_count_bonus_needs_its_upgrade() {
    let mut jelly = save();
    let with_bonus = jelly.layout_metrics(&jelly.layout, MetricOptions::default());
    assert!(with_bonus.valid);
    jelly.upgrades[14] = 0;
    let without = jelly.layout_metrics(&jelly.layout, MetricOptions::default());
    assert_eq!(without.counts, without.raw_counts);
    assert_eq!(
        with_bonus.counts[0],
        with_bonus.raw_counts[0] + (with_bonus.raw_counts[0] / 3.0).floor()
    );
    assert_eq!(
        with_bonus.counts[5], with_bonus.raw_counts[5],
        "viruses keep their count"
    );
}
