//! Scurge combat: the per-kill EXP, the Self Affliction comparison and the
//! optimized allocation.
//!
//! The EXP figures are per kill; the app multiplies them by a kills-per-hour
//! stand-in because the shared simulator does not run Clam Work.

use idleon_clamworks::Clamworks;
use idleon_clamworks::scurge::{self, Error, Inputs, Kind, Mode, Plan};

mod common;
use common::close;

fn fixture() -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/data.json");
    std::fs::read_to_string(path).expect("fixtures/data.json is committed")
}

fn saved() -> Clamworks {
    idleon_import::import(&fixture())
        .expect("the fixture imports")
        .clamworks
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
