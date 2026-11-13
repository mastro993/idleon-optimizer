//! Scurge combat parity: the per-kill EXP, the Self Affliction comparison and the
//! optimized allocation.
//!
//! `fixtures/clamworks-scurge.json` is written by
//! `script/clamworks-scurge-oracle.mjs`, which runs the upstream
//! `buildClamworksScurgePlan` on the fixture save with each case's `Menagerie`
//! row, and reads `CW.pearlsPerKill` beside it. The EXP figures are per kill, the
//! unit upstream's own comparison reports; the app multiplies them by a
//! kills-per-hour stand-in because the shared simulator does not run Clam Work.

use idleon_clamworks::Clamworks;
use idleon_clamworks::scurge::{self, Error, Inputs, Kind, Mode, Plan};
use serde_json::Value;

mod common;
use common::close;

fn fixture() -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/data.json");
    std::fs::read_to_string(path).expect("fixtures/data.json is committed")
}

fn oracle() -> Value {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/clamworks-scurge.json"
    );
    serde_json::from_str(&std::fs::read_to_string(path).expect("fixtures/clamworks-scurge.json"))
        .expect("the oracle is JSON")
}

fn saved() -> Clamworks {
    idleon_import::import(&fixture())
        .expect("the fixture imports")
        .clamworks
}

/// The fixture save's Clamworks state at one case's `Menagerie` row.
fn state(case: &Value, saved: &Clamworks) -> Clamworks {
    let levels = case["levels"].as_array().expect("levels");
    Clamworks {
        scurge_unlocked: case["unlocked"].as_bool().expect("unlocked"),
        scurge_level: case["rank"].as_u64().expect("rank") as u32,
        scurge_upgrades: std::array::from_fn(|index| {
            levels[index].as_u64().expect("a level") as u32
        }),
        ..saved.clone()
    }
}

fn inputs(case: &Value) -> Inputs {
    Inputs {
        mode: Mode::from_id(case["mode"].as_str().expect("mode")).expect("a known mode"),
        max_affliction: case["maxAffliction"].as_u64().expect("maxAffliction") as u32,
        other_players: case["otherPlayers"].as_u64().expect("otherPlayers") as u32,
    }
}

#[test]
fn every_scurge_case_matches_the_upstream_plan() {
    let oracle = oracle();
    let cases = oracle["cases"].as_array().expect("cases");
    assert!(
        cases.len() >= 9,
        "the oracle covers both modes and refusals"
    );
    let saved = saved();
    for case in cases {
        let name = case["name"].as_str().expect("name");
        let plan = Plan::build(&state(case, &saved), inputs(case));
        if let Some(message) = case["error"].as_str() {
            let error = plan.unwrap_err();
            let expected = match name {
                "not-unlocked" => Error::NotUnlocked,
                "over-budget" => Error::OverBudget,
                other => panic!("{other}: the upstream refused with {message}"),
            };
            assert_eq!(error, expected, "{name}: upstream refused with {message}");
            continue;
        }
        let plan = plan.unwrap_or_else(|error| panic!("{name}: {}", error.message()));
        assert_eq!(
            plan.rank,
            case["rank"].as_u64().expect("rank") as u32,
            "{name}"
        );
        assert_eq!(
            plan.available,
            case["available"].as_u64().expect("available") as u32,
            "{name}"
        );
        assert_eq!(
            plan.self_budget,
            case["selfBudget"].as_u64().expect("selfBudget") as u32,
            "{name}"
        );
        assert_eq!(
            plan.maximum,
            case["maximum"].as_u64().expect("maximum") as u32,
            "{name}"
        );
        let rows = case["rows"].as_array().expect("rows");
        assert_eq!(plan.rows.len(), rows.len(), "{name}: row count");
        for (row, expected) in plan.rows.iter().zip(rows) {
            assert_eq!(row.kind.label(), expected["label"].as_str().expect("label"));
            let levels = expected["levels"].as_array().expect("levels");
            for (level, expected) in row.levels.iter().zip(levels) {
                assert_eq!(
                    u64::from(*level),
                    expected.as_u64().expect("a level"),
                    "{name}: levels"
                );
            }
            assert_eq!(
                u64::from(row.points_left),
                expected["pointsLeft"].as_u64().expect("pointsLeft"),
                "{name}: points left"
            );
            close(
                row.exp_per_kill,
                expected["expPerKill"].as_f64().expect("expPerKill"),
            );
            let changes = expected["changes"].as_array().expect("changes");
            assert_eq!(row.changes.len(), changes.len(), "{name}: change count");
            for (change, expected) in row.changes.iter().zip(changes) {
                assert_eq!(
                    change.index as u64,
                    expected["index"].as_u64().expect("index")
                );
                assert_eq!(change.name(), expected["name"].as_str().expect("name"));
                assert_eq!(
                    u64::from(change.from),
                    expected["from"].as_u64().expect("from")
                );
                assert_eq!(u64::from(change.to), expected["to"].as_u64().expect("to"));
            }
        }
    }
}

#[test]
fn the_affliction_rows_track_the_saved_level_outwards() {
    let saved = saved();
    let plan = Plan::build(&saved, Inputs::default()).expect("the fixture save is unlocked");
    // The fixture spends every point, so the three refunded levels are the limit.
    assert_eq!((plan.available, plan.maximum), (0, 3));
    let levels: Vec<Option<u32>> = plan
        .rows
        .iter()
        .map(|row| match row.kind {
            Kind::Affliction(level) => Some(level),
            _ => None,
        })
        .collect();
    assert_eq!(levels, [None, Some(0), Some(1), Some(2)]);
    // Zero affliction is the cheapest EXP and the most refunded points, so
    // Scurge EXP per kill falls as the affliction level drops.
    assert_eq!(plan.baseline().self_affliction(), 3);
    assert!(plan.rows[1].exp_per_kill < plan.baseline().exp_per_kill);
    close(
        plan.rows[3].exp_per_kill,
        plan.baseline().exp_per_kill * 5.0 / 7.0,
    );
    assert_eq!(
        plan.best().kind,
        Kind::Current,
        "nothing beats the saved +3"
    );
    close(plan.gain(&plan.rows[1], 1000.0), -4_731_000.0);
}

#[test]
fn the_upgrade_allocation_spends_every_point_once() {
    let mut clamworks = saved();
    clamworks.scurge_level = 700;
    let plan = Plan::build(
        &clamworks,
        Inputs {
            mode: Mode::Upgrades,
            other_players: 3,
            ..Inputs::default()
        },
    )
    .expect("the fixture save is unlocked");
    assert_eq!(plan.rows.len(), 2);
    let optimized = &plan.rows[1];
    assert_eq!(optimized.kind, Kind::Optimized);
    assert_eq!(optimized.points_left, 0);
    assert_eq!(
        optimized.self_affliction(),
        3,
        "the affliction level is held"
    );
    assert_eq!(
        optimized.levels.iter().sum::<u32>(),
        700,
        "the allocation spends the budget"
    );
    // Three unlocked EXP upgrades with equal marginal cost water-fill level.
    let allocated = [0, 4, 7].map(|index| optimized.levels[index]);
    let highest = allocated.iter().max().expect("a factor");
    let lowest = allocated.iter().min().expect("a factor");
    assert!(highest - lowest <= 3, "{allocated:?}");
    assert_eq!(plan.best(), optimized);
    assert!(plan.gain(optimized, 1.0) > 0.0);
}

#[test]
fn a_locked_or_unrepresentable_state_is_refused() {
    let mut clamworks = saved();
    assert_eq!(
        Plan::build(
            &Clamworks {
                scurge_unlocked: false,
                ..clamworks.clone()
            },
            Inputs::default()
        ),
        Err(Error::NotUnlocked)
    );
    clamworks.scurge_level = 5;
    assert_eq!(
        Plan::build(&clamworks, Inputs::default()),
        Err(Error::OverBudget)
    );
    let mut clamworks = saved();
    clamworks.scurge_level = 5;
    clamworks.scurge_upgrades = [0; 9];
    let locked = Plan::build(&clamworks, Inputs::default()).expect("a low rank is valid");
    assert_eq!(locked.maximum, 0, "SELF AFFLICTION unlocks at 10");
    assert_eq!(locked.rows.len(), 1, "only the saved allocation is tested");
    assert_eq!(
        Plan::build(
            &clamworks,
            Inputs {
                max_affliction: scurge::MAX_AFFLICTION + 1,
                ..Inputs::default()
            }
        ),
        Err(Error::MaxAffliction)
    );
    assert_eq!(
        Plan::build(
            &clamworks,
            Inputs {
                other_players: scurge::MAX_OTHER_PLAYERS + 1,
                ..Inputs::default()
            }
        ),
        Err(Error::OtherPlayers)
    );
}
