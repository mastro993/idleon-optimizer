//! Ladle rates, the save roster and the formula trees, on the small purpose-built
//! save below rather than `fixtures/data.json`.

use idleon_ladle::{
    self as ladle, Character, Claimer, EXP_BALLOONS, Ladle, Panel, balloons_to_level,
    claim_expectation, claim_minimum, claim_plan, cooking_exp_req, ladle_breakpoints,
    ladles_per_day, levels_from_balloons, split_balloons,
};
use idleon_save::Save;
use serde_json::{Value, json};

mod common;
use common::close;

/// The stat panel the character rates read.
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

#[test]
fn balloons_carry_their_overflow() {
    let item = EXP_BALLOONS[0].item_id;
    // A target at or below the current level spends nothing.
    let same = balloons_to_level(10.0, 5.0, 10.0, 2.0, item);
    close(same.balloons, 0.0);
    close(same.level, 10.0);
    close(same.exp, 5.0);

    // Banked EXP alone pays for the levels it covers.
    let banked = cooking_exp_req(20.0) + cooking_exp_req(21.0);
    let free = balloons_to_level(20.0, banked, 22.0, 2.0, item);
    close(free.balloons, 0.0);
    close(free.level, 22.0);

    // The count reaches the target, and one balloon fewer does not.
    let reach = balloons_to_level(5.0, 0.0, 30.0, 1.5, item);
    assert!(reach.balloons > 1.0);
    assert!(reach.level >= 30.0);
    let spent = levels_from_balloons(5.0, 0.0, reach.balloons, 1.5, item);
    close(spent.used, reach.balloons);
    close(spent.level, reach.level);
    close(spent.exp, reach.exp);
    assert!(levels_from_balloons(5.0, 0.0, reach.balloons - 1.0, 1.5, item).level < 30.0);

    // No EXP multi means no balloon gain, so no count reaches the target.
    assert!(
        balloons_to_level(10.0, 0.0, 12.0, 0.0, item)
            .balloons
            .is_infinite()
    );
}

#[test]
fn split_share_keeps_levels_even() {
    let item = EXP_BALLOONS[0].item_id;
    let characters = [Some((30.0, 0.0)), None, Some((60.0, 0.0))];
    let split = split_balloons(&characters, 130.0, 2.75, item);
    close(split.used + split.left, 130.0);
    assert!(split.characters[1].is_none());
    let low = split.characters[0].expect("a share");
    let high = split.characters[2].expect("a share");
    // The lowest character takes every balloon until it catches up.
    close(low.used, 130.0);
    close(high.used, 0.0);
    assert!(low.level > low.start_level);
    close(high.level, 60.0);

    // With nothing to share the levels still absorb the banked EXP.
    let banked = [Some((30.0, cooking_exp_req(30.0)))];
    let idle = split_balloons(&banked, 0.0, 2.75, item);
    close(idle.used, 0.0);
    close(idle.left, 0.0);
    close(idle.characters[0].expect("a share").level, 31.0);
}

#[test]
fn each_breakpoint_is_the_efficiency_its_step_needs() {
    let efficiency = 12_345.6;
    close(ladles_per_day(efficiency, 0.0), 15.0);
    let steps = ladle_breakpoints(efficiency, 0.0, 3);
    for (step, point) in steps.iter().enumerate() {
        close(point.ladles_per_day, 15.0 * (step as f64 + 2.0));
        close(point.multiplier, point.efficiency / efficiency);
        close(
            ladles_per_day(point.efficiency * (1.0 + 1e-12), 0.0),
            point.ladles_per_day,
        );
        assert!(ladles_per_day(point.efficiency * (1.0 - 1e-9), 0.0) < point.ladles_per_day);
    }
}

#[test]
fn a_claim_rolls_the_post_office_double() {
    // Whole ladles: the floor, or twice the value floored on a double.
    close(claim_minimum(1.7), 1.0);
    close(claim_expectation(1.7, 40.0), 0.6 * 1.0 + 0.4 * 3.0);
    // Under one ladle nothing is guaranteed, and the roll still pays.
    close(claim_minimum(0.7), 0.0);
    close(claim_expectation(0.7, 40.0), 0.7 * 1.4 + 0.3 * 0.4);
    close(claim_expectation(0.3, 40.0), 0.3 * 1.4);
    close(claim_expectation(-1.0, 40.0), 0.0);
}

#[test]
fn the_claim_plan_splits_the_window_and_respects_the_cap() {
    let claimer = Claimer {
        index: 0,
        name: "Ana".into(),
        raw_per_hour: 1.0,
        double_chance: 0.0,
        cap_hours: 10.0,
        claim_hours: 8.0,
        pending_hours: 7.0,
    };
    let plan = claim_plan(std::slice::from_ref(&claimer), 20.0);
    // The pending seven hours plus the first four would pass the cap, so they
    // are claimed now; then claims land at 4, 12 and 20 hours.
    let at: Vec<f64> = plan.claims.iter().map(|claim| claim.at_hours).collect();
    assert_eq!(at, vec![0.0, 4.0, 12.0, 20.0]);
    assert!(plan.claims[0].now);
    let credited: Vec<f64> = plan
        .claims
        .iter()
        .map(|claim| claim.credited_hours)
        .collect();
    assert_eq!(credited, vec![7.0, 4.0, 8.0, 8.0]);
    close(plan.min_ladles, 27.0);
    close(plan.expected_ladles, 27.0);

    // Two minutes away or less grants nothing.
    let short = Claimer {
        pending_hours: 0.0,
        claim_hours: 0.01,
        ..claimer
    };
    let plan = claim_plan(&[short], 0.01);
    close(plan.min_ladles, 0.0);
}
