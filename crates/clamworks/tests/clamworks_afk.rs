//! Clamworks AFK claims and the upgrade comparison on `fixtures/data.json`.
//!
//! The claim's combat stand-ins (kills per hour, drop rate) are fixed inputs,
//! because the combat simulator that derives them is not built here.

use idleon_clamworks::Clamworks;
use idleon_clamworks::afk::{ClaimError, Inputs, Phase};

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

/// The multikill total every claim here is evaluated at.
const MULTIKILL: f64 = 1234.5;

#[test]
fn the_claim_totals_its_two_drop_rows() {
    let claim = saved()
        .afk_claim(
            Inputs {
                hours: 8.0,
                kills_per_hour: 2000.0,
                drop_rate: 2.0,
                phase: Phase::Entry,
            },
            MULTIKILL,
        )
        .expect("the claim");
    close(
        claim.expected_pearls,
        claim.regular.pearls + claim.pure.pearls,
    );
    close(
        claim.expected_pearls,
        (claim.regular.expected + claim.pure.expected * claim.pure_multiplier)
            * claim.value
            * (1.0 + 9.0 * claim.lucky_chance),
    );
}

#[test]
fn an_unbought_pure_pearls_upgrade_drops_no_pure_pearls() {
    let mut clamworks = saved();
    clamworks.upgrades[5] = 0;
    for phase in Phase::ALL {
        let claim = clamworks
            .afk_claim(
                Inputs {
                    hours: 0.5,
                    kills_per_hour: 1400.0,
                    drop_rate: 2.0,
                    phase,
                },
                MULTIKILL,
            )
            .expect("the claim");
        assert_eq!(claim.pure.expected, 0.0);
        assert_eq!(claim.pure.chance, 0.0);
        assert!(!claim.pure.guaranteed);
    }
}

#[test]
fn a_claim_shorter_than_two_minutes_credits_nothing() {
    let claim = saved()
        .afk_claim(
            Inputs {
                hours: 0.03,
                kills_per_hour: 2000.0,
                drop_rate: 2.0,
                phase: Phase::Settled,
            },
            MULTIKILL,
        )
        .expect("the claim");
    assert!(!claim.eligible);
    assert_eq!(claim.rolls, 0);
    assert_eq!(claim.regular.expected, 0.0);
    assert_eq!(claim.pure.expected, 0.0);
    assert_eq!(claim.expected_pearls, 0.0);
    assert!(claim.warnings.iter().any(|w| w.contains("two minutes")));
}

#[test]
fn hours_outside_the_credited_range_are_rejected() {
    let clamworks = saved();
    for hours in [-1.0, f64::NAN, f64::INFINITY, 55_555.56, 1e9] {
        assert_eq!(
            clamworks.afk_claim(
                Inputs {
                    hours,
                    ..Inputs::default()
                },
                MULTIKILL
            ),
            Err(ClaimError::Hours),
            "{hours} hours must be rejected"
        );
    }
    for (kills_per_hour, drop_rate) in [(-1.0, 1.0), (f64::NAN, 1.0), (1.0, -2.0), (1.0, f64::NAN)]
    {
        assert_eq!(
            clamworks.afk_claim(
                Inputs {
                    kills_per_hour,
                    drop_rate,
                    ..Inputs::default()
                },
                MULTIKILL
            ),
            Err(ClaimError::Rates)
        );
    }
    assert!(clamworks.afk_claim(Inputs::default(), MULTIKILL).is_ok());
}

#[test]
fn the_comparison_is_one_row_per_unlocked_upgrade() {
    let mut clamworks = saved();
    assert_eq!(clamworks.compare_candidates(), [0, 1, 2, 3, 5, 6]);
    clamworks.unlocked = 8;
    assert_eq!(clamworks.compare_candidates(), [0, 1, 2, 3, 5, 6, 7]);
    // CLAM COMRADES stops at the 25-clam cap the map can hold.
    clamworks.upgrades[1] = 23;
    assert_eq!(clamworks.compare_candidates(), [0, 2, 3, 5, 6, 7]);
    clamworks.unlocked = 0;
    assert_eq!(clamworks.compare_candidates(), [0]);
}

#[test]
fn a_phase_change_moves_the_drop_upgrade_but_not_a_value_upgrade() {
    let clamworks = saved();
    let settled = clamworks
        .afk_compare(
            Inputs {
                hours: 8.0,
                kills_per_hour: 2000.0,
                drop_rate: 2.0,
                phase: Phase::Settled,
            },
            MULTIKILL,
        )
        .expect("the comparison");
    let encystation = settled
        .rows
        .iter()
        .find(|row| row.index == Some(6))
        .expect("ENCYSTATION UP is a candidate");
    let pearl_value = settled
        .rows
        .iter()
        .find(|row| row.index == Some(0))
        .expect("PEARL VALUE is a candidate");
    assert!(encystation.gain.is_some_and(|gain| gain > 0.0));
    assert!(pearl_value.gain.is_some_and(|gain| gain > 0.0));
}
