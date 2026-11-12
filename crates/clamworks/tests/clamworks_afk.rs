//! Clamworks AFK claim and comparison parity.
//!
//! `fixtures/clamworks-afk.json` is written by `script/clamworks-afk-oracle.mjs`,
//! which runs the upstream `clamworksAfkSummary`, `afkMonsterDropOutcomes` and
//! the comparison in `clamworks-combat.js` against `fixtures/data.json`. The
//! claim's combat stand-ins (kills per hour, drop rate) are the same values on
//! both sides, because the combat simulator that derives them is not built here.

use idleon_clamworks::Clamworks;
use idleon_clamworks::afk::{ClaimError, Inputs, Phase};
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
        "/../../fixtures/clamworks-afk.json"
    );
    serde_json::from_str(&std::fs::read_to_string(path).expect("fixtures/clamworks-afk.json"))
        .expect("the oracle is JSON")
}

fn saved() -> Clamworks {
    idleon_import::import(&fixture())
        .expect("the fixture imports")
        .clamworks
}

fn inputs(case: &Value) -> Inputs {
    Inputs {
        hours: case["hours"].as_f64().expect("hours"),
        kills_per_hour: case["killsPerHour"].as_f64().expect("killsPerHour"),
        drop_rate: case["dropRate"].as_f64().expect("dropRate"),
        phase: Phase::from_id(case["phase"].as_str().expect("phase")).expect("a known phase"),
    }
}

fn multikill() -> f64 {
    oracle()["multikillTotal"].as_f64().expect("the total")
}

#[test]
fn every_claim_matches_the_upstream_payout() {
    let clamworks = saved();
    for case in oracle()["claims"].as_array().expect("claims") {
        let claim = clamworks
            .afk_claim(inputs(case), multikill())
            .unwrap_or_else(|error| panic!("{} claim: {error}", case["phase"]));
        assert_eq!(claim.rolls, case["rolls"].as_u64().expect("rolls"));
        assert_eq!(claim.kills, case["batch"].as_f64().expect("batch"));
        close(claim.value, case["value"].as_f64().expect("value"));
        close(
            claim.lucky_chance,
            case["luckyChance"].as_f64().expect("luckyChance"),
        );
        close(
            claim.pure_multiplier,
            case["pureMultiplier"].as_f64().expect("pureMultiplier"),
        );
        close(
            claim.expected_pearls,
            case["expectedPearls"].as_f64().expect("expectedPearls"),
        );
        for (drop, expected) in [
            (&claim.regular, &case["regular"]),
            (&claim.pure, &case["pure"]),
        ] {
            close(
                drop.expected,
                expected["expected"].as_f64().expect("expected"),
            );
            close(drop.chance, expected["chance"].as_f64().expect("chance"));
            close(drop.minimum, expected["minimum"].as_f64().expect("minimum"));
            close(drop.pearls, expected["pearls"].as_f64().expect("pearls"));
            assert_eq!(
                drop.guaranteed,
                expected["guaranteed"].as_bool().expect("guaranteed"),
                "{} guaranteed",
                case["phase"]
            );
        }
    }
}

#[test]
fn the_claim_totals_its_two_drop_rows() {
    let claim = saved()
        .afk_claim(inputs(&oracle()["claims"][0]), multikill())
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
    for case in oracle()["pureLocked"]
        .as_array()
        .expect("pure-locked cases")
    {
        let phase = Phase::from_id(case["phase"].as_str().expect("phase")).expect("a phase");
        let claim = clamworks
            .afk_claim(
                Inputs {
                    hours: 0.5,
                    kills_per_hour: 1400.0,
                    drop_rate: 2.0,
                    phase,
                },
                multikill(),
            )
            .expect("the claim");
        assert_eq!(claim.pure.expected, 0.0);
        assert_eq!(claim.pure.chance, 0.0);
        assert!(!claim.pure.guaranteed);
        close(
            claim.expected_pearls,
            case["expectedPearls"].as_f64().expect("expectedPearls"),
        );
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
            multikill(),
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
                multikill()
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
                multikill()
            ),
            Err(ClaimError::Rates)
        );
    }
    assert!(clamworks.afk_claim(Inputs::default(), multikill()).is_ok());
}

#[test]
fn the_comparison_matches_the_upstream_rows() {
    let clamworks = saved();
    let compare = clamworks
        .afk_compare(inputs(&oracle()["compare"]), multikill())
        .expect("the comparison");
    let oracle = oracle();
    let expected = oracle["compare"]["rows"].as_array().expect("rows");
    assert_eq!(compare.rows.len(), expected.len());
    assert!(compare.warnings.is_empty());
    for (row, expected) in compare.rows.iter().zip(expected) {
        assert_eq!(
            row.index,
            expected["index"].as_u64().map(|index| index as usize),
            "{} index",
            expected["key"]
        );
        assert_eq!(row.name, expected["label"].as_str().expect("label"));
        close(
            row.claim.expected_pearls,
            expected["expectedPearls"].as_f64().unwrap(),
        );
        close(
            row.claim.pure.chance,
            expected["pureChance"].as_f64().unwrap(),
        );
        match (row.cost, expected["cost"].as_f64()) {
            (Some(actual), Some(upstream)) => {
                close(actual, upstream);
                assert_eq!(row.affordable, expected["affordable"].as_bool().unwrap());
                assert_eq!(row.level, expected["from"].as_u64().unwrap() as u32);
                assert_eq!(row.next_level, expected["to"].as_u64().unwrap() as u32);
                close(
                    row.gain.expect("a gain"),
                    expected["gain"].as_f64().unwrap(),
                );
            }
            (None, None) => assert!(row.gain.is_none()),
            (actual, upstream) => panic!("cost {actual:?} != {upstream:?}"),
        }
        match (row.payback_claims, expected["paybackClaims"].as_f64()) {
            (Some(actual), Some(upstream)) => {
                close(actual, upstream);
                close(
                    row.payback_hours.expect("payback hours"),
                    expected["paybackHours"].as_f64().unwrap(),
                );
            }
            (None, None) => assert!(row.payback_hours.is_none()),
            (actual, upstream) => panic!("payback {actual:?} != {upstream:?}"),
        }
    }
    assert_eq!(compare.best().and_then(|row| row.index), Some(2));
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
            multikill(),
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
