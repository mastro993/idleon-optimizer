//! Farming assignment and Land Rank allocation on `fixtures/data.json`.

use idleon_common::Control;
use idleon_common::Progress;
use idleon_farming::{Farm, Mode, Objective, PLOTS, data};
use idleon_import::import;

/// The fixture save's farm, imported the way a user's save is.
fn farm() -> Farm {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/data.json");
    let save = std::fs::read_to_string(path).expect("fixtures/data.json is committed");
    import(&save).expect("the fixture save imports").farming
}

/// Run one allocation to completion, ignoring progress.
fn allocate(
    farm: &Farm,
    objective: Objective,
    mode: Mode,
    target: Option<u32>,
) -> idleon_farming::Allocation {
    farm.allocation(objective, mode, target, &mut |_progress| Control::Continue)
        .expect("an uncancelled allocation returns a result")
}

/// The target crop the evolution scores aim at.
const TARGET_CROP: u32 = 240;

#[test]
fn an_assignment_only_targets_crops_the_save_can_plant() {
    let farm = farm();
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
    let farm = farm();
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
                if index % 5 == 4 && after > before {
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
    let farm = farm();
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
            // not; the assignment does not check discovery.
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
fn the_comparison_plans_each_goal_once() {
    let farm = farm();
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
