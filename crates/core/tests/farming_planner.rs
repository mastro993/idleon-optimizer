//! Farming assignment and Land Rank allocation: every figure is pinned against
//! `fixtures/farming.json`, which `script/farming-oracle.mjs` produces from the
//! upstream `farming-planner.js`.
//!
//! The same save is read through the importer, so the test also proves the port
//! reads the fields the upstream decoder does.

use idleon_core::farming::{Farm, Mode, Objective, PLOTS, data};
use idleon_core::model::{Control, Progress};
use idleon_core::save::import;
use serde::Deserialize;
use serde_json::Value;

fn fixture() -> Fixture {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/farming.json");
    serde_json::from_str(&std::fs::read_to_string(path).expect("fixtures/farming.json"))
        .expect("the farming oracle fixture parses")
}

fn farm(fixture: &Fixture) -> Farm {
    import(&fixture.save.to_string())
        .expect("the oracle save imports")
        .farming
}

/// A figure from the upstream planner, within rounding of the port.
#[track_caller]
fn close(actual: f64, expected: f64) {
    if actual == expected {
        return;
    }
    let tolerance = 1e-9 * actual.abs().max(expected.abs());
    assert!(
        (actual - expected).abs() <= tolerance,
        "expected {expected} from upstream, got {actual}"
    );
}

/// Run one allocation to completion, ignoring progress.
fn allocate(
    farm: &Farm,
    objective: Objective,
    mode: Mode,
    target: Option<u32>,
) -> idleon_core::farming::Allocation {
    farm.allocation(objective, mode, target, &mut |_progress| Control::Continue)
        .expect("an uncancelled allocation returns a result")
}

#[derive(Deserialize)]
struct Fixture {
    save: Value,
    snapshot: SnapshotRow,
    plans: Vec<PlanRow>,
    presets: PresetsRow,
}

/// The upstream page's own preset list and comparison table.
#[derive(Deserialize)]
struct PresetsRow {
    objectives: Vec<GoalRow>,
    comparisons: Vec<ComparisonRow>,
}

#[derive(Deserialize)]
struct GoalRow {
    id: String,
    name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ComparisonRow {
    mode: String,
    rows: Vec<PresetRow>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PresetRow {
    objective: String,
    relative_gain: f64,
    points_spent: u32,
    points_unused: u32,
    initial_score: f64,
    final_score: f64,
    moves: Vec<MoveRow>,
}

#[derive(Deserialize)]
struct MoveRow {
    id: usize,
    delta: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SnapshotRow {
    growth_rate: f64,
    next_crop_common: f64,
    overgrowth_common: f64,
    farming_exp: f64,
    crop_value_cap: f64,
    product_bonus: f64,
    expected_vines: f64,
    plot_rank_display: f64,
    crop_counts: Vec<f64>,
    rank_bonuses: Vec<f64>,
    rank_evolution: f64,
    rank_crop_value: f64,
    rank_xp: f64,
    rank_overgrowth: f64,
    rank_farming_exp: f64,
    total_land_rank: u32,
    points_left: i64,
    fifth_column_max: u32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlanRow {
    objective: String,
    mode: String,
    assignment: AssignmentRow,
    allocation: AllocationRow,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AssignmentRow {
    target_crop_id: i64,
    rows: Vec<CropRow>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CropRow {
    plot: usize,
    current_crop_id: i64,
    current_family_id: i64,
    target_crop_id: i64,
    target_family_id: i64,
    target_evolution: u32,
    change_needed: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AllocationRow {
    points_available: u32,
    points_spent: u32,
    points_unused: u32,
    initial_score: f64,
    final_score: f64,
    relative_gain: f64,
    initial_levels: Vec<u32>,
    levels: Vec<u32>,
    changes: Vec<ChangeRow>,
}

#[derive(Deserialize)]
struct ChangeRow {
    id: usize,
    before: u32,
    after: u32,
}

/// The target crop the calculator's target stepper held when the fixture was
/// generated; the allocation's evolution scores depend on it.
const TARGET_CROP: u32 = 240;

#[test]
fn the_account_snapshot_matches_upstream() {
    let fixture = fixture();
    let farm = farm(&fixture);
    let snapshot = farm.snapshot();
    let upstream = &fixture.snapshot;

    close(snapshot.growth_rate, upstream.growth_rate);
    close(snapshot.next_crop_common, upstream.next_crop_common);
    close(snapshot.overgrowth_common, upstream.overgrowth_common);
    close(snapshot.farming_exp, upstream.farming_exp);
    close(snapshot.crop_value_cap, upstream.crop_value_cap);
    close(snapshot.product_bonus, upstream.product_bonus);
    close(snapshot.expected_vines, upstream.expected_vines);
    close(snapshot.plot_rank_display, upstream.plot_rank_display);
    for (actual, expected) in snapshot.crop_counts.iter().zip(&upstream.crop_counts) {
        close(*actual, *expected);
    }
    for (actual, expected) in snapshot.rank.bonuses.iter().zip(&upstream.rank_bonuses) {
        close(*actual, *expected);
    }
    close(snapshot.rank.evolution, upstream.rank_evolution);
    close(snapshot.rank.crop_value, upstream.rank_crop_value);
    close(snapshot.rank.rank_xp, upstream.rank_xp);
    close(snapshot.rank.overgrowth, upstream.rank_overgrowth);
    close(snapshot.rank.farming_exp, upstream.rank_farming_exp);

    assert_eq!(farm.total_land_rank(), upstream.total_land_rank);
    assert_eq!(farm.land_rank_points_left(), upstream.points_left);
    assert_eq!(farm.fifth_column_max(), upstream.fifth_column_max);
}

#[test]
fn every_assignment_matches_upstream() {
    let fixture = fixture();
    let farm = farm(&fixture);
    for plan in &fixture.plans {
        let objective = Objective::from_id(&plan.objective).expect("a known objective");
        let assignment = farm.assignment(objective, Some(TARGET_CROP));
        assert_eq!(
            assignment.rows.len(),
            PLOTS,
            "{}: every plot has a row",
            plan.objective
        );
        match objective {
            Objective::Evolution => assert_eq!(
                assignment.target_crop.map(i64::from),
                Some(plan.assignment.target_crop_id),
                "{}: the assignment names the evolution target",
                plan.objective
            ),
            _ => assert_eq!(
                assignment.target_crop, None,
                "{}: only the evolution objective names a target crop",
                plan.objective
            ),
        }
        for (row, expected) in assignment.rows.iter().zip(&plan.assignment.rows) {
            let label = format!("{} plot {}", plan.objective, row.plot + 1);
            assert_eq!(row.plot, expected.plot, "{label}: plot index");
            assert_eq!(
                row.current_crop, expected.current_crop_id,
                "{label}: current crop"
            );
            assert_eq!(
                row.current_family, expected.current_family_id,
                "{label}: current family"
            );
            assert_eq!(
                row.target_crop, expected.target_crop_id,
                "{label}: target crop"
            );
            assert_eq!(
                row.target_family, expected.target_family_id,
                "{label}: target family"
            );
            assert_eq!(
                row.target_evolution, expected.target_evolution,
                "{label}: target evolution"
            );
            assert_eq!(
                row.change_needed(),
                expected.change_needed,
                "{label}: change needed"
            );
        }
    }
}

#[test]
fn every_allocation_matches_upstream() {
    let fixture = fixture();
    let farm = farm(&fixture);
    for plan in &fixture.plans {
        let objective = Objective::from_id(&plan.objective).expect("a known objective");
        let mode = Mode::from_id(&plan.mode).expect("a known mode");
        let allocation = allocate(&farm, objective, mode, Some(TARGET_CROP));
        let expected = &plan.allocation;
        let label = format!("{} / {}", plan.objective, plan.mode);
        assert_eq!(
            allocation.points_available, expected.points_available,
            "{label}: points available"
        );
        assert_eq!(
            allocation.points_spent, expected.points_spent,
            "{label}: points spent"
        );
        assert_eq!(
            allocation.points_unused(),
            expected.points_unused,
            "{label}: points unused"
        );
        close(allocation.initial_score, expected.initial_score);
        close(allocation.final_score, expected.final_score);
        close(allocation.relative_gain(), expected.relative_gain);
        assert_eq!(
            allocation.initial_levels, expected.initial_levels,
            "{label}: initial levels"
        );
        assert_eq!(allocation.levels, expected.levels, "{label}: levels");
        // The upstream change list is exactly the rows the levels moved.
        let moved: Vec<(usize, u32, u32)> = allocation
            .levels
            .iter()
            .zip(&allocation.initial_levels)
            .enumerate()
            .filter(|(_, (after, before))| after != before)
            .map(|(id, (after, before))| (id, *before, *after))
            .collect();
        assert_eq!(
            moved.len(),
            expected.changes.len(),
            "{label}: the rows the levels moved"
        );
        for (id, before, after) in moved {
            assert!(
                expected.changes.iter().any(|change| change.id == id
                    && change.before == before
                    && change.after == after),
                "{label}: row {id} moved {before} to {after} but upstream moved it differently"
            );
        }
    }
}

#[test]
fn an_assignment_only_targets_crops_the_save_can_plant() {
    let fixture = fixture();
    let farm = farm(&fixture);
    let discovered: Vec<i64> = farm
        .crop_quantities
        .keys()
        .map(|crop| i64::from(*crop))
        .collect();
    for objective in Objective::ALL {
        let assignment = farm.assignment(objective, Some(TARGET_CROP));
        for row in &assignment.rows {
            let label = format!("{} plot {}", objective.id(), row.plot + 1);
            if row.target_crop < 0 {
                assert_eq!(
                    row.target_family, -1,
                    "{label}: an unknown crop has no family"
                );
                continue;
            }
            let family = data()
                .family_of(row.target_crop)
                .unwrap_or_else(|| panic!("{label}: crop {} is in no family", row.target_crop));
            assert_eq!(row.target_family, family.id as i64, "{label}: family");
            assert_eq!(
                i64::from(row.target_evolution),
                row.target_crop - family.start,
                "{label}: the evolution depth is the crop's offset in its family"
            );
            let expected = if objective == Objective::Evolution {
                i64::from(TARGET_CROP) - 1
            } else {
                assert!(
                    discovered.contains(&row.target_crop),
                    "{label}: crop {} is not in the save",
                    row.target_crop
                );
                row.target_crop
            };
            assert_eq!(row.target_crop, expected, "{label}: target");
        }
    }
}

#[test]
fn an_allocation_respects_points_and_never_loses() {
    let fixture = fixture();
    let farm = farm(&fixture);
    let total = farm.total_land_rank();
    let fifth_max = farm.fifth_column_max();
    for objective in Objective::ALL {
        for mode in Mode::ALL {
            let label = format!("{} / {}", objective.id(), mode.id());
            let allocation = allocate(&farm, objective, mode, Some(TARGET_CROP));
            assert!(
                allocation.points_spent <= allocation.points_available,
                "{label}: spent more points than the account has"
            );
            assert!(
                allocation.final_score >= allocation.initial_score,
                "{label}: the allocation scored below its baseline"
            );
            assert!(
                allocation.relative_gain() >= 0.0,
                "{label}: a negative gain"
            );
            let spent: u32 = allocation
                .levels
                .iter()
                .zip(&allocation.initial_levels)
                .map(|(after, before)| after.saturating_sub(*before))
                .sum();
            assert_eq!(
                spent, allocation.points_spent,
                "{label}: the levels spend the points"
            );
            for (index, (after, before)) in allocation
                .levels
                .iter()
                .zip(&allocation.initial_levels)
                .enumerate()
            {
                assert!(after >= before, "{label}: row {index} lost a level");
                if after > before {
                    assert!(
                        total >= data().rank_unlocks[index],
                        "{label}: row {index} is still locked"
                    );
                }
                if index % 5 == 4 {
                    assert!(
                        *after <= fifth_max,
                        "{label}: row {index} passed the fifth-column cap"
                    );
                }
            }
            for (index, (after, before)) in allocation
                .levels
                .iter()
                .zip(&allocation.initial_levels)
                .enumerate()
            {
                if after != before {
                    assert!(
                        allocation.points_spent > 0,
                        "{label}: row {index} moved with no point spent"
                    );
                }
            }
        }
    }
}

#[test]
fn cancelling_an_allocation_stores_nothing() {
    let fixture = fixture();
    let farm = farm(&fixture);
    let mut reports: Vec<Progress> = Vec::new();
    let outcome = farm.allocation(Objective::RankXp, Mode::Unspent, None, &mut |progress| {
        reports.push(progress);
        Control::Stop
    });
    assert!(outcome.is_none(), "a cancelled allocation returns nothing");
    assert!(!reports.is_empty(), "the run reported before it stopped");
}

#[test]
fn an_empty_farm_targets_the_evolution_predecessor_only() {
    let farm = Farm::default();
    for objective in Objective::ALL {
        let assignment = farm.assignment(objective, Some(1));
        assert_eq!(assignment.rows.len(), PLOTS);
        if objective == Objective::Evolution {
            // Crop 0 is the predecessor of crop 1, whether the save found it or
            // not; the upstream assignment does not check discovery either.
            assert!(assignment.rows.iter().all(|row| row.target_crop == 0));
            assert_eq!(assignment.target_crop, Some(0));
            assert_eq!(assignment.changes(), PLOTS);
        } else {
            assert!(assignment.rows.iter().all(|row| row.target_crop == -1));
            assert_eq!(assignment.target_crop, None);
            assert_eq!(assignment.changes(), 0);
        }
        let allocation = allocate(&farm, objective, Mode::Unspent, Some(1));
        assert_eq!(allocation.points_available, 0);
        assert_eq!(allocation.points_spent, 0);
    }
}

#[test]
fn every_upstream_preset_exists() {
    let fixture = fixture();
    // The page's OBJECTIVES list is the built-in preset list: same goals, same
    // order and same labels.
    assert_eq!(
        Objective::ALL.len(),
        fixture.presets.objectives.len(),
        "the port offers every upstream goal"
    );
    for (objective, upstream) in Objective::ALL.iter().zip(&fixture.presets.objectives) {
        assert_eq!(objective.id(), upstream.id, "preset id");
        assert_eq!(
            objective.label(),
            upstream.name,
            "{}: the label matches upstream",
            upstream.id
        );
        assert_eq!(Objective::from_id(&upstream.id), Some(*objective));
    }
}

#[test]
fn the_comparison_matches_upstream_per_objective() {
    let fixture = fixture();
    let farm = farm(&fixture);
    for comparison in &fixture.presets.comparisons {
        let mode = Mode::from_id(&comparison.mode).expect("a known mode");
        let label = format!("comparison / {}", comparison.mode);
        let planned = farm
            .comparison(mode, Some(TARGET_CROP), &mut |_progress| Control::Continue)
            .expect("an uncancelled comparison returns every goal");
        assert_eq!(planned.mode, mode, "{label}: mode");
        assert_eq!(planned.target_crop, Some(TARGET_CROP), "{label}: target");
        assert_eq!(
            planned.rows.len(),
            comparison.rows.len(),
            "{label}: one row per goal"
        );
        for (row, expected) in planned.rows.iter().zip(&comparison.rows) {
            let row_label = format!("{label} / {}", expected.objective);
            assert_eq!(
                row.objective.id(),
                expected.objective,
                "{row_label}: the row keeps the upstream goal order"
            );
            close(row.relative_gain(), expected.relative_gain);
            assert_eq!(
                row.points_spent, expected.points_spent,
                "{row_label}: points"
            );
            assert_eq!(
                row.points_unused(),
                expected.points_unused,
                "{row_label}: unused points"
            );
            close(row.initial_score, expected.initial_score);
            close(row.final_score, expected.final_score);
            // The table's "Largest Moves" column reads the upstream change list
            // in order, so the ordering is part of the port.
            let moves: Vec<(usize, i64)> = row
                .changes
                .iter()
                .map(|change| (change.id, change.delta()))
                .collect();
            let expected_moves: Vec<(usize, i64)> = expected
                .moves
                .iter()
                .map(|change| (change.id, change.delta))
                .collect();
            assert_eq!(moves, expected_moves, "{row_label}: largest moves");
        }
    }
}

#[test]
fn the_comparison_plans_each_goal_once() {
    let fixture = fixture();
    let farm = farm(&fixture);
    let planned = farm
        .comparison(Mode::Unspent, Some(TARGET_CROP), &mut |_progress| {
            Control::Continue
        })
        .expect("the comparison completes");
    for row in &planned.rows {
        let single = allocate(&farm, row.objective, Mode::Unspent, Some(TARGET_CROP));
        assert_eq!(row, &single, "{}: the same plan twice", row.objective.id());
    }
    // Every row can only spend what the account has, and never scores below
    // the baseline it started from.
    for row in &planned.rows {
        assert!(row.points_spent <= row.points_available);
        assert!(row.final_score >= row.initial_score);
    }
}
