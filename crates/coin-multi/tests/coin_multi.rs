//! The coin multiplier against imported saves.

use idleon_coin_multi::{self as coin_multi, Inputs};
use idleon_import::import;

fn fixture() -> String {
    std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/data.json"
    ))
    .expect("fixtures/data.json is committed")
}

fn close(left: f64, right: f64) -> bool {
    (left - right).abs() <= 1e-9 * left.abs().max(right.abs()).max(1.0)
}

#[test]
fn a_save_without_its_character_rows_has_no_coin_cards() {
    let imported = import(r#"{ "Research": "[]" }"#).expect("a research-only save imports");
    assert!(imported.coin.characters.is_empty());
    assert!(coin_multi::evaluate(&imported.coin, &Inputs::default()).is_err());
}

#[test]
fn the_wrapper_supplies_the_names_and_the_winning_vote() {
    // An `it.json` save: `data` holds the save, the names and the vote sit beside it.
    let wrapped = serde_json::json!({
        "data": {
            "Lv0_0": "[1]",
            "Research": "[[],[],[],[],[],[],[],[],[],[],[],[],[],[],[],[],[],[]]",
        },
        "charNames": ["Ana"],
        "serverVars": { "voteCategories": [34] },
    });
    let imported = import(&wrapped.to_string()).expect("the wrapped save imports");
    let character = imported.coin.character(0).expect("one character");
    assert_eq!(character.name, "Ana");
    // Vote 34 pays its 52 % base into the meal/pet group; the rest stays neutral.
    let with_vote = character.groups[16].value.expect("the group is modelled");
    assert!(close(with_vote, 1.52), "got {with_vote}");

    let unwrapped = serde_json::json!({
        "data": {
            "Lv0_0": "[1]",
            "Research": "[[],[],[],[],[],[],[],[],[],[],[],[],[],[],[],[],[],[]]",
        },
        "charNames": ["Ana"],
    });
    let imported = import(&unwrapped.to_string()).expect("the save imports");
    let without_vote = imported.coin.character(0).expect("one character").groups[16]
        .value
        .expect("the group is modelled");
    assert!(close(without_vote, 1.0), "got {without_vote}");
}

#[test]
fn the_other_sources_talent_scales_with_the_overkill_tier() {
    let imported = import(&fixture()).expect("the fixture imports");
    let mut first = imported.coin.inputs(0).expect("the inputs exist");
    first.overkill = Some(1);
    let mut second = imported.coin.inputs(0).expect("the inputs exist");
    second.overkill = Some(2);
    let one = coin_multi::evaluate(&imported.coin, &first).expect("the evaluation runs");
    let two = coin_multi::evaluate(&imported.coin, &second).expect("the evaluation runs");
    let term = |evaluation: &coin_multi::Evaluation| {
        evaluation.groups[22]
            .terms
            .iter()
            .find(|term| term.name == "Talent: Coins For Charon")
            .map(|term| term.value)
            .expect("the talent is active on the fixture")
    };
    assert!(close(term(&two), term(&one) * 2.0));
    assert!(close(
        two.groups[22].value.unwrap() - one.groups[22].value.unwrap(),
        term(&one) / 100.0
    ));
}
