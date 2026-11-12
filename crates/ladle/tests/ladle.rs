//! Ladle rates, claim plans and Cooking EXP balloons against the upstream formulas.
//!
//! The expected numbers are what `js/stats/systems/w4/cooking-ladle.js` answers for
//! the same inputs, evaluated with Node against the upstream module itself: the
//! pure functions directly, and `cookingLadleStats` with a stat context whose
//! `skill-efficiency`, `skill-afk` and `skill-exp` resolve to the panel below. The
//! save is the small purpose-built one below, not `fixtures/data.json`.

use idleon_ladle::{
    self as ladle, Character, Claimer, EXP_BALLOONS, Ladle, Panel, balloons_to_level,
    claim_expectation, claim_minimum, claim_plan, cooking_exp_req, ladle_breakpoints,
    ladle_tier_outlook, ladles_per_day, levels_from_balloons, split_balloons,
};
use idleon_save::Save;
use serde_json::{Value, json};

mod common;
use common::close;

/// The panel values the oracle fed the upstream stat context.
const PANEL: Panel = Panel {
    efficiency: 12345.6,
    afk: 1.35,
    exp_multi: 2.75,
    prowess: 0.0,
    talent_148: 25.0,
};

/// The purpose-built save: three characters, two AFK-ing Cooking, one Prayer 2
/// cap, three Post Office 19c rolls, and a chest plus two character inventories.
fn save() -> Value {
    let empty = || vec![0.0; 21];
    let mut levels = [empty(), empty(), empty()];
    let mut exp = [empty(), empty(), empty()];
    for (index, cooking) in [42.0, 30.0, 60.0].into_iter().enumerate() {
        levels[index][ladle::COOKING_LEVEL_INDEX] = cooking;
    }
    exp[0][ladle::COOKING_LEVEL_INDEX] = 123_456_789.0;
    exp[2][ladle::COOKING_LEVEL_INDEX] = 5_000_000_000.0;
    let post_office = |points: f64| {
        let mut row = vec![0.0; 36];
        row[19] = points;
        row
    };
    json!({
        "charNames": ["Ana", "Bo", "Cy"],
        "Lv0_0": levels[0], "Lv0_1": levels[1], "Lv0_2": levels[2],
        "Exp0_0": exp[0], "Exp0_1": exp[1], "Exp0_2": exp[2],
        "AFKtarget_0": "Cooking", "AFKtarget_1": "Fighting", "AFKtarget_2": "Cooking",
        "PTimeAway_0": 986.4, "PTimeAway_1": 0, "PTimeAway_2": 0,
        "TimeAway": { "GlobalTime": 1_000_000.0, "ShopRestock": 27_000.0, "Player": 986_400.0 },
        "POu_0": post_office(150.0), "POu_1": post_office(0.0), "POu_2": post_office(400.0),
        "PrayOwned": [0.0, 0.0, 25.0],
        "Prayers_0": [-1.0], "Prayers_1": [-1.0], "Prayers_2": [2.0],
        "Gaming": [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, ""],
        "ChestOrder": ["Ladle", "ExpBalloon2"], "ChestQuantity": [120.0, 3.0],
        "InventoryOrder_0": ["ExpBalloon1", "Ladle"], "ItemQTY_0": [7.0, 4.0],
        "InventoryOrder_1": ["ExpBalloon3", "ExpBalloon1"], "ItemQTY_1": [2.0, 6.0],
        "InventoryOrder_2": ["Blank"], "ItemQTY_2": [0.0],
    })
}

fn imported() -> Ladle {
    Ladle::from_save(&Save::new(save())).expect("the save carries a roster")
}

#[test]
fn ladle_rate_matches_upstream() {
    close(ladles_per_day(1000.0, 0.0), 15.0);
    close(ladles_per_day(12345.6, 0.0234), 15.0);
    close(ladles_per_day(1e7, 0.05), 225.0);
    close(ladles_per_day(1e12, 0.05), 7515.0);
    // Prowess is clamped to the upstream 0.1 cap in both directions.
    close(ladles_per_day(1e12, 5.0), ladles_per_day(1e12, 0.1));
    close(ladles_per_day(1e12, -5.0), ladles_per_day(1e12, 0.0));
}

#[test]
fn breakpoints_match_upstream() {
    let breakpoints = ladle_breakpoints(12345.6, 0.0234, 3);
    close(breakpoints[0].ladles_per_day, 30.0);
    close(breakpoints[0].efficiency, 12620.039116763031);
    close(breakpoints[0].multiplier, 1.022229710727954);
    close(breakpoints[1].efficiency, 55608.33888539554);
    close(breakpoints[2].efficiency, 159265.38730862903);

    let large = ladle_breakpoints(1e7, 0.05, 3);
    close(large[0].efficiency, 10321273.240738805);
    close(large[0].multiplier, 1.0321273240738806);
    close(large[2].efficiency, 15284163.811026292);
}

#[test]
fn tier_outlook_matches_upstream() {
    let small = ladle_tier_outlook(12345.6, 0.0234);
    close(small.tier, 1.0);
    close(small.next_tier_gain_pct, 100.0);
    close(small.next_tier_efficiency_pct, 2.22297107279541);
    close(small.plus_10_efficiency_gain_pct, 100.0);
    assert!(small.small_tier);

    let large = ladle_tier_outlook(1e12, 0.05);
    close(large.tier, 501.0);
    close(large.next.ladles_per_day, 7530.0);
    close(large.next.efficiency, 1005415841625.8009);
    close(large.next_tier_gain_pct, 0.1996007984031936);
    close(large.plus_10_efficiency_gain_pct, 2.794411177644718);
    close(large.efficiency_pct_for_plus_1_pct, 3.918578590223598);
    assert!(!large.small_tier);

    let empty = ladle_tier_outlook(0.0, 0.0);
    close(empty.base_per_day, 15.0);
    assert!(empty.efficiency_pct_for_plus_1_pct.is_infinite());
    close(empty.next.multiplier, 1.6e304);
}

#[test]
fn cooking_exp_requirement_matches_upstream() {
    let expected = [0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 30.0, 42.0, 50.0, 60.0, 100.0];
    let upstream = [
        -15.0,
        7.892941176470593,
        42.90860950443788,
        94.48142695750184,
        168.31086280687504,
        271.65710753750926,
        169535.1407505769,
        1386662.3491217974,
        4897004.572641804,
        21252385.238685455,
        3637894427.872396,
    ];
    for (level, expected) in expected.iter().zip(upstream) {
        close(cooking_exp_req(*level), expected);
    }
    close(ladle::balloon_gain(50.0, 10.0, 2.5), 1375.0);
}

#[test]
fn balloons_carry_their_overflow() {
    let first = EXP_BALLOONS[0].item_id;
    let use_first = balloons_to_level(0.0, 0.0, 3.0, 1.0, first);
    close(use_first.balloons, 1.0);
    close(use_first.level, 4.0);
    close(use_first.exp, 19.717022361589684);

    // Two levels from one balloon, because the overflow pays most of the second.
    let two = balloons_to_level(0.0, 0.0, 2.0, 1.0, first);
    close(two.balloons, 0.0);
    close(two.level, 2.0);
    close(two.exp, 7.107058823529407);

    // Banked EXP alone can finish the target, which is the overflow rule.
    let banked = balloons_to_level(42.0, 123_456_789.0, 50.0, 2.75, EXP_BALLOONS[1].item_id);
    close(banked.balloons, 0.0);
    close(banked.level, 59.0);
    close(banked.exp, 16805456.861110434);

    // A target at or below the current level spends nothing.
    let same = balloons_to_level(10.0, 5.0, 10.0, 2.0, first);
    close(same.balloons, 0.0);
    close(same.level, 10.0);
    close(same.exp, 5.0);

    // No EXP multi means no balloon gain, which upstream reports as infinity.
    assert!(
        balloons_to_level(10.0, 0.0, 12.0, 0.0, first)
            .balloons
            .is_infinite()
    );
}

#[test]
fn spending_a_stack_matches_upstream() {
    let first = EXP_BALLOONS[0].item_id;
    let none = levels_from_balloons(0.0, 0.0, 0.0, 1.0, first);
    close(none.level, 2.0);
    close(none.exp, 7.107058823529407);
    close(none.used, 0.0);

    let one = levels_from_balloons(0.0, 0.0, 1.0, 1.0, first);
    close(one.level, 4.0);
    close(one.exp, 19.717022361589684);
    close(one.used, 1.0);
    close(one.left, 0.0);

    let ten = levels_from_balloons(42.0, 123_456_789.0, 10.0, 2.75, first);
    close(ten.level, 59.0);
    close(ten.exp, 16887956.861110434);
    close(ten.used, 10.0);
}

#[test]
fn split_share_keeps_levels_even() {
    let first = EXP_BALLOONS[0].item_id;
    let characters = [
        Some((42.0, 123_456_789.0)),
        Some((30.0, 0.0)),
        Some((60.0, 5_000_000_000.0)),
    ];
    let split = split_balloons(&characters, 130.0, 2.75, first);
    close(split.used, 130.0);
    close(split.left, 0.0);
    let shares: Vec<(f64, f64, f64)> = split
        .characters
        .iter()
        .map(|share| {
            let share = share.expect("every input character has a share");
            (share.level, share.exp, share.used)
        })
        .collect();
    close(shares[0].0, 59.0);
    close(shares[0].1, 16805456.861110434);
    close(shares[0].2, 0.0);
    close(shares[1].0, 32.0);
    close(shares[1].1, 198391.18864515974);
    close(shares[1].2, 130.0);
    close(shares[2].0, 86.0);
    close(shares[2].1, 317013526.9260253);

    // With nothing to share the levels still absorb the banked EXP.
    let idle = split_balloons(&characters, 0.0, 2.75, first);
    close(idle.used, 0.0);
    close(idle.left, 0.0);
    close(idle.characters[1].expect("a share").level, 30.0);
    assert!(split_balloons(&[None], 10.0, 2.75, first).characters[0].is_none());
}

#[test]
fn claim_roll_matches_upstream() {
    close(claim_expectation(0.7, 40.0), 1.0999999999999999);
    close(claim_minimum(0.7), 0.0);
    close(claim_expectation(1.7, 40.0), 1.8000000000000003);
    close(claim_minimum(1.7), 1.0);
    close(claim_expectation(3.2, 40.0), 4.2);
    close(claim_minimum(3.2), 3.0);
    close(claim_expectation(3.2, 0.0), 3.0);
    close(claim_expectation(3.2, 100.0), 6.0);
    close(claim_expectation(-5.0, 40.0), 0.0);
}

#[test]
fn claim_plan_matches_upstream() {
    let plan = claim_plan(
        &[Claimer {
            index: 0,
            name: "A".into(),
            raw_per_hour: 4.0,
            double_chance: 0.0,
            cap_hours: 10.0,
            claim_hours: 8.0,
            pending_hours: 27.0,
        }],
        24.0,
    );
    // Pending time would lose to the cap, so an immediate claim comes first.
    let rows: Vec<(f64, f64, f64, bool)> = plan
        .claims
        .iter()
        .map(|claim| (claim.at_hours, claim.hours, claim.raw, claim.now))
        .collect();
    assert_eq!(
        rows,
        vec![
            (0.0, 27.0, 40.0, true),
            (8.0, 8.0, 32.0, false),
            (16.0, 8.0, 32.0, false),
            (24.0, 8.0, 32.0, false),
        ]
    );
    close(plan.min_ladles, 136.0);
    close(plan.expected_ladles, 136.0);
    close(plan.per_character[0].min_ladles, 136.0);

    // A zero-length window only claims the time already accrued.
    let accrued = claim_plan(
        &[Claimer {
            index: 1,
            name: "B".into(),
            raw_per_hour: 4.0,
            double_chance: 0.0,
            cap_hours: f64::INFINITY,
            claim_hours: 8.0,
            pending_hours: 5.0,
        }],
        0.0,
    );
    assert_eq!(accrued.claims.len(), 1);
    close(accrued.claims[0].raw, 20.0);
    assert!(accrued.claims[0].now);

    // No accrued time and no window: nothing to claim.
    let empty = claim_plan(&[], 24.0);
    assert!(empty.claims.is_empty());
    close(empty.min_ladles, 0.0);
}

#[test]
fn save_reads_the_roster_and_counts() {
    let ladle = imported();
    let names: Vec<&str> = ladle
        .characters
        .iter()
        .map(|character| character.name.as_str())
        .collect();
    assert_eq!(names, vec!["Ana", "Bo", "Cy"]);
    assert_eq!(ladle.cooking_characters(), vec![0, 2]);
    close(ladle.characters[0].level, 42.0);
    close(ladle.characters[0].exp, 123456789.0);
    close(
        ladle.characters[0]
            .pending_hours
            .expect("AFK time is known"),
        (1_000_000.0 - 986_400.0) / 3600.0,
    );
    assert!(ladle.characters[1].pending_hours.is_none());
    // Post Office 19c: decay(88, 200, points - 99.9) at box 19.
    close(ladle.characters[0].double_chance, 17.6);
    close(ladle.characters[1].double_chance, 0.0);
    close(ladle.characters[2].double_chance, 52.8);
    // Prayer 2 at level 25 with round(10 × 3.4) = 34 is over the two-hour gate.
    assert!(!ladle.characters[0].claim_capped);
    assert!(ladle.characters[2].claim_capped);
    close(ladle.ladles.value, 124.0);
    assert!(!ladle.ladles.partial);
    close(ladle.balloons[0].value, 13.0);
    close(ladle.balloons[1].value, 3.0);
    close(ladle.balloons[2].value, 2.0);
    close(
        ladle.reset_hours.expect("the save has a reset countdown"),
        7.5,
    );
}

#[test]
fn save_without_an_inventory_says_what_is_missing() {
    let mut save = save();
    let object = save.as_object_mut().expect("the save is an object");
    object.remove("ChestOrder");
    object.remove("ChestQuantity");
    object.remove("InventoryOrder_0");
    object.remove("ItemQTY_0");
    let ladle = Ladle::from_save(&Save::new(save)).expect("the roster is still there");
    assert!(ladle.ladles.partial);
    assert!(
        ladle
            .ladles
            .missing
            .iter()
            .any(|name| name == "storage inventory")
    );
    assert!(
        ladle
            .ladles
            .missing
            .iter()
            .any(|name| name == "character inventory 1")
    );
}

#[test]
fn save_without_a_reset_countdown_reports_none() {
    let mut save = save();
    save.as_object_mut()
        .expect("the save is an object")
        .remove("TimeAway");
    let ladle = Ladle::from_save(&Save::new(save)).expect("the roster is still there");
    assert!(ladle.reset_hours.is_none());
}

#[test]
fn stats_match_upstream() {
    let ladle = imported();
    let character = ladle.character(0).expect("a first character");
    let stats = ladle::stats(character, PANEL, 24.0);
    close(stats.efficiency, 12345.6);
    close(stats.base_per_day, 15.0);
    close(stats.ladle_multi, 1.0);
    close(stats.panel_per_day, 20.25);
    close(stats.double_chance, 17.6);
    close(stats.effective_claim_seconds, 86_400.0);
    close(stats.raw_per_claim, 20.25);
    close(stats.per_claim, 23.52);
    close(stats.per_day, 23.52);
    close(stats.per_hour, 0.98);
    close(stats.min_per_claim, 20.0);
    close(stats.min_per_day, 20.0);
    close(stats.raw_per_hour, 0.84375);
    assert!(stats.claim_cap_hours.is_infinite());
    close(stats.pending_hours, (1_000_000.0 - 986_400.0) / 3600.0);
    assert!(stats.pending_known);
    close(stats.level, 42.0);
    close(stats.exp, 123456789.0);
    close(stats.exp_req, 1386662.3491217974);
    close(stats.exp_per_hour, 371.25);
    close(stats.next_level_balloons, 0.0);

    let capped = ladle::stats(ladle.character(2).expect("a third character"), PANEL, 24.0);
    assert!(capped.claim_capped);
    close(capped.effective_claim_seconds, 36_000.0);
    close(capped.claim_cap_hours, 10.0);
    close(capped.raw_per_claim, 8.4375);
    close(capped.per_claim, 12.224);
    close(capped.per_day, 12.223999999999998);
    close(capped.min_per_day, 8.0);

    let twelve = ladle::stats(character, PANEL, 12.0);
    close(twelve.raw_per_claim, 10.125);
    close(twelve.per_claim, 11.76);
    close(twelve.per_day, 23.52);
    close(twelve.min_per_day, 20.0);

    let plainly = ladle::stats(ladle.character(1).expect("a second character"), PANEL, 24.0);
    close(plainly.double_chance, 0.0);
    close(plainly.per_day, 20.0);
    close(plainly.next_level_balloons, 40.0);
}

#[test]
fn trees_agree_with_the_rows() {
    let ladle = imported();
    let stats = ladle::stats(ladle.character(0).expect("a first character"), PANEL, 24.0);

    let ladles = stats.ladles_tree();
    close(ladles.value, stats.panel_per_day);
    let base = &ladles.children[0];
    close(base.value, stats.base_per_day);
    close(base.children[0].value, stats.ladle_multi);
    close(base.children[0].children[0].value, stats.efficiency);
    close(ladles.children[1].value, stats.afk);

    let claims = stats.claims_tree();
    close(claims.value, stats.per_day);
    close(claims.children[0].value, stats.panel_per_day);
    close(claims.children[3].value, stats.double_chance);
    close(claims.children[4].value, stats.per_claim);
    close(claims.children[6].value, stats.min_per_day);

    let balloon = stats.balloons_tree(EXP_BALLOONS[1], 3.0, Some(45.0), None);
    close(balloon.value, stats.next_level_balloons);
    close(balloon.children[0].value, stats.level);
    close(balloon.children[2].value, stats.exp_req);
    close(balloon.children[3].value, 120.0 * 43.0 * 2.75);
    let to_target = &balloon.children[5];
    assert_eq!(to_target.label, "Balloons to Lv 45");
    let target = balloons_to_level(42.0, 123456789.0, 45.0, 2.75, EXP_BALLOONS[1].item_id);
    close(to_target.value, target.balloons.max(0.0));
}

#[test]
fn ladle_hours_follow_talent_148() {
    let panel = Panel {
        talent_148: 25.0,
        ..Panel::default()
    };
    close(panel.ladle_hours(), 1.25);
    let without = Panel::default();
    close(without.ladle_hours(), 1.0);
    // The exponent the tier uses is the clamped prowess.
    close(panel.exponent(), 0.25);
    close(
        Panel {
            prowess: 0.4,
            ..Panel::default()
        }
        .exponent(),
        0.35,
    );
}

#[test]
fn a_character_with_no_rows_still_reads() {
    let character = Character {
        index: 0,
        name: "Nobody".into(),
        afk_target: String::new(),
        level: 10.0,
        exp: 0.0,
        pending_hours: None,
        double_chance: 0.0,
        claim_capped: false,
    };
    let stats = ladle::stats(&character, Panel::default(), 24.0);
    // The neutral panel still floors the ladle tier at one: 15 × 0.01 = 0.15.
    close(stats.base_per_day, 15.0);
    close(stats.panel_per_day, 0.15);
    close(stats.min_per_day, 0.0);
    close(stats.exp_per_hour, 1.0);
    assert!(stats.next_level_balloons.is_finite());
    assert!(!stats.pending_known);
    // Zero everywhere is the floor: no efficiency and no AFK time.
    let none = ladle::stats(
        &character,
        Panel {
            afk: 0.0,
            ..Panel::default()
        },
        24.0,
    );
    close(none.panel_per_day, 0.0);
    close(none.per_day, 0.0);
}
