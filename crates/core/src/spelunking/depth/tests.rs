//! Depth-estimate edges: exit odds, rock scaling and rejected configurations.
//!
//! The save-derived inputs and every profile are compared against the upstream
//! calculator in `crates/core/tests/spelunking.rs`.

use super::*;
use crate::spelunking::{Account, Save};

#[test]
fn extra_exits_split_the_hole_mix() {
    let mut inputs = fixture_inputs();
    inputs.holes_bonus = 0.0;
    inputs.shop[18] = 0.0;
    let plain = exit_distribution(&inputs);
    assert_eq!(plain.probabilities, vec![1.0, 0.0, 0.0]);
    assert_eq!(plain.expected, 1.0);

    inputs.shop[18] = 100.0;
    let doubled = exit_distribution(&inputs);
    assert_eq!(doubled.probabilities, vec![0.0, 1.0, 0.0]);
    assert_eq!(doubled.expected, 2.0);

    inputs.shop[18] = 200.0;
    let tripled = exit_distribution(&inputs);
    assert!((tripled.probabilities[2] - 1.0).abs() < 1e-12);
    assert_eq!(tripled.expected, 3.0);
}

#[test]
fn rocks_appear_only_from_their_first_depth_and_scale_with_growth() {
    let cave = &data().caves[0];
    let shallow = rock_probabilities(cave, 0);
    assert_eq!(shallow[0], 1.0);
    assert!(shallow[1..].iter().all(|chance| *chance == 0.0));
    let deep = rock_probabilities(cave, 40);
    assert!((deep.iter().sum::<f64>() - 1.0).abs() < 1e-9);
    assert!(deep[deep.len() - 1] > 0.0, "the last normal rock appears");

    let (hp, requirement, _) = rock_stats(cave, 0, 0);
    assert_eq!(hp, 1.0);
    assert!((requirement - 1.0).abs() < 1e-12);
    let spire = cave
        .rocks
        .iter()
        .position(|rock| rock.scaled)
        .expect("a scaled rock");
    let (hp, requirement, _) = rock_stats(cave, spire, 40);
    assert!(hp > cave.rocks[spire].hp);
    assert!(requirement > 100.0);
}

#[test]
fn a_profile_reads_the_whole_party_budget() {
    let inputs = fixture_inputs();
    let requested = Requested {
        cave: Some(0),
        party: Some(vec![0, 1]),
        stamina: Some(StaminaMode::Full),
        ..Requested::default()
    };
    let profile = profile(
        &inputs,
        &requested,
        &DepthRange {
            from: 1,
            to: 10,
            tool: ReferenceTool::Best,
        },
    )
    .expect("the party is valid");
    let budget: f64 = [0, 1]
        .iter()
        .map(|id| inputs.characters[*id].max_stamina)
        .sum();
    assert_eq!(profile.budget, budget);
    assert_eq!(profile.rows.len(), 10);
    assert_eq!(profile.rows[0].depth, 1);
    assert_eq!(profile.rows[9].depth, 10);
}

#[test]
fn a_duplicate_party_is_rejected_before_any_estimate() {
    let inputs = fixture_inputs();
    let requested = Requested {
        party: Some(vec![0, 0]),
        ..Requested::default()
    };
    let error =
        profile(&inputs, &requested, &DepthRange::default()).expect_err("duplicates cannot run");
    assert_eq!(
        error.to_string(),
        "Choose a unique, nonempty party within this cave's capacity."
    );
}

fn fixture_inputs() -> Inputs {
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/data.json"
    ))
    .expect("the fixture is committed");
    let imported = crate::save::import(&text).expect("the fixture imports");
    let save = Save::from_parts(&imported.save).expect("the fixture carries Spelunk rows");
    save.inputs(&Account::neutral())
}
