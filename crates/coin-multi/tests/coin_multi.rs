//! The coin multiplier port is compared against the upstream calculator's own
//! output: `script/coin-multi-oracle.mjs` runs `js/stats/defs/coin-multi.js`
//! over `fixtures/data.json` and a small purpose-built save, then writes
//! `fixtures/coin-multi.json`. Every expected number here is upstream's output,
//! not a value read back from this port.

use idleon_coin_multi::{self as coin_multi, Inputs};
use idleon_import::import;
use serde_json::Value;

fn fixture() -> String {
    std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/data.json"
    ))
    .expect("fixtures/data.json is committed")
}

fn oracle() -> Value {
    serde_json::from_str(
        &std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/coin-multi.json"
        ))
        .expect("fixtures/coin-multi.json is committed"),
    )
    .expect("the oracle fixture is JSON")
}

/// The group indices `crates/coin-multi/src/save.rs` resolves from the save: the
/// companions, event shop, equipment, RoG, grid, set, gambit, meal, pet, bundle,
/// vault, pristine and prayer groups.
const MODELLED: [usize; 19] = [
    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 16, 17, 18, 19, 20,
];

fn close(left: f64, right: f64) -> bool {
    (left - right).abs() <= 1e-9 * left.abs().max(right.abs()).max(1.0)
}

/// The small save that activates only the groups this build ports.
fn simple_save() -> Value {
    serde_json::json!({ "data": {
        "charNames": ["Ana"],
        "Lv0_0": "[1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0]",
        "CharacterClass_0": 1,
        "CurrentMap_0": 2,
        "AFKtarget_0": "frogG",
        "Research": "[[],[],[],[],[],[],[],[],[],[],[],[],[],[],[],[],[],[]]",
        "Holes": serde_json::to_string(&vec![vec![vec![0; 0]; 3]; 34]).unwrap(),
        "OptLacc": serde_json::to_string(&{
            let mut options = vec![serde_json::Value::from(0); 701];
            options[606] = serde_json::json!("24,45,159,38");
            options[311] = serde_json::json!("a,b,c,d,e,f,g,h,i,j");
            options
        }).unwrap(),
        "BundlesReceived": serde_json::json!({"bun_y": 1}).to_string(),
        "UpgVault": serde_json::to_string(&{
            let mut vault = vec![0.0; 90];
            vault[34] = 10.0;
            vault[37] = 10.0;
            vault
        }).unwrap(),
        "TaskZZ3": "[[[1,1]],[],[],[]]",
        "CauldronInfo": "[[5],[50],[120],[0]]",
    }})
}

/// One save's trees: every group named and valued like upstream's own tree, the
/// root equal to the oracle total, and each expanded group recomposing its terms.
fn check_trees(label: &str, snapshot: &coin_multi::Snapshot, expected: &Value) {
    let characters = expected["characters"]
        .as_array()
        .expect("the oracle carries characters");
    for character in characters {
        let index = character["character"].as_u64().expect("an index") as usize;
        let mut inputs = snapshot.inputs(index).expect("the inputs exist");
        inputs.damage = character["damage"].as_f64().unwrap();
        let evaluation = coin_multi::evaluate(snapshot, &inputs).expect("the evaluation runs");
        let tree = coin_multi::tree(&evaluation);
        let upstream = character["total"].as_f64().unwrap();

        assert_eq!(tree.label, "Coin Multiplier", "{label}");
        assert!(tree.source.is_some(), "{label}: the root names its source");
        assert!(
            close(tree.value, evaluation.total),
            "{label}: the tree root {} is the total card {}",
            tree.value,
            evaluation.total
        );
        let product: f64 = tree.children.iter().map(|child| child.value).product();
        assert!(
            close(product, evaluation.total),
            "{label}: the 23 factors multiply to {product}, the card reads {}",
            evaluation.total
        );
        // The partial total only equals upstream's when every group resolved.
        if evaluation.complete {
            assert!(close(evaluation.total, upstream), "{label}: the total");
        }

        let groups = character["groups"].as_array().expect("the oracle groups");
        assert_eq!(tree.children.len(), groups.len());
        for (group_index, group) in evaluation.groups.iter().enumerate() {
            let node = &tree.children[group_index];
            assert_eq!(
                node.label,
                groups[group_index]["name"].as_str().unwrap(),
                "{label}: group {group_index} is named like upstream's tree"
            );
            assert!(
                node.source.is_some(),
                "{label}: group {group_index} names its source"
            );
            match group.value {
                Some(value) => {
                    assert!(
                        close(node.value, value),
                        "{label}: group {group_index} value"
                    );
                    if group.terms.is_empty() {
                        continue;
                    }
                    let sum: f64 = group.terms.iter().map(|term| term.value).sum();
                    assert!(
                        close(1.0 + sum / group.divisor, value),
                        "{label}: group {group_index} terms recompose its factor"
                    );
                    assert_eq!(node.children.len(), group.terms.len());
                }
                None => assert_eq!(
                    node.value, 1.0,
                    "{label}: an unmodelled group is neutral in the product"
                ),
            }
        }

        // The shared tree's search reads labels, inputs, sources and notes, so
        // every term and every named source is reachable.
        assert!(
            tree.match_count("js/stats/") >= 24,
            "{label}: sources are named"
        );
        assert!(
            tree.match_count("vault") > 0,
            "{label}: a group is searchable"
        );
        for group in &evaluation.groups {
            for term in &group.terms {
                assert!(
                    tree.match_count(term.name) >= 1,
                    "{label}: term {} is searchable",
                    term.name
                );
            }
        }
    }
}

#[test]
fn every_character_matches_the_upstream_groups_and_overkill() {
    let imported = import(&fixture()).expect("the fixture imports");
    let oracle = oracle();
    let characters = oracle["full"]["characters"]
        .as_array()
        .expect("the oracle carries characters");
    assert_eq!(imported.coin.characters.len(), characters.len());

    for expected in characters {
        let index = expected["character"].as_u64().expect("an index") as usize;
        let character = imported
            .coin
            .character(index)
            .expect("the imported snapshot carries the character");
        assert_eq!(character.map, expected["map"].as_u64().unwrap() as usize);
        assert_eq!(character.target, expected["target"].as_str().unwrap());
        assert!(!character.target_name.is_empty());
        assert!(close(
            character.monster_hp,
            expected["monsterHp"].as_f64().unwrap()
        ));
        assert!(close(
            character.highest_damage,
            expected["highestDamage"].as_f64().unwrap()
        ));

        let mut inputs = imported.coin.inputs(index).expect("the inputs exist");
        inputs.damage = expected["damage"].as_f64().unwrap();
        let evaluation =
            coin_multi::evaluate(&imported.coin, &inputs).expect("the evaluation runs");

        let overkill = &expected["overkill"];
        assert_eq!(
            evaluation.overkill.tier,
            overkill["tier"].as_u64().unwrap() as u32,
            "character {}: overkill tier",
            index + 1
        );
        assert_eq!(
            evaluation.overkill.partial,
            overkill["partial"].as_bool().unwrap()
        );
        assert!(close(
            evaluation.overkill.exponent,
            overkill["exponent"].as_f64().unwrap()
        ));

        let groups = expected["groups"].as_array().expect("the oracle groups");
        assert_eq!(groups.len(), 23, "upstream multiplies 23 groups");
        for (group_index, group) in evaluation.groups.iter().enumerate() {
            let upstream = groups[group_index]["value"].as_f64().unwrap();
            assert_eq!(
                group.name,
                groups[group_index]["name"].as_str().unwrap(),
                "character {} group {group_index} name",
                index + 1
            );
            if MODELLED.contains(&group_index) {
                let value = group
                    .value
                    .unwrap_or_else(|| panic!("group {group_index} should be modelled"));
                assert!(
                    close(value, upstream),
                    "character {} group {group_index} ({}): got {value}, upstream {upstream}",
                    index + 1,
                    group.name
                );
            } else {
                assert!(
                    !group.is_modelled(),
                    "group {group_index} is not ported yet, so it must not claim a value"
                );
                assert!(
                    !group.reason.is_empty(),
                    "an unmodelled group names its source"
                );
            }
        }
    }
}

#[test]
fn the_modelled_sources_produce_the_upstream_total_when_the_rest_are_neutral() {
    let save = simple_save();
    let imported = import(&save.to_string()).expect("the simple save imports");
    let expected = &oracle()["simple"]["characters"][0];
    let character = imported.coin.character(0).expect("one character");
    assert_eq!(character.target, "frogG");
    assert!(close(
        character.monster_hp,
        expected["monsterHp"].as_f64().unwrap()
    ));

    let mut inputs = imported.coin.inputs(0).expect("the inputs exist");
    inputs.damage = expected["damage"].as_f64().unwrap();
    let evaluation = coin_multi::evaluate(&imported.coin, &inputs).expect("the evaluation runs");

    // Every group this build does not model is neutral in the simple save, so
    // the resolved product is upstream's own total.
    let groups = expected["groups"].as_array().unwrap();
    for (index, group) in groups.iter().enumerate() {
        if !MODELLED.contains(&index) {
            assert!(
                close(group["value"].as_f64().unwrap(), 1.0),
                "simple group {index} should be neutral"
            );
        }
    }
    let total = expected["total"].as_f64().unwrap();
    assert!(
        close(evaluation.total, total),
        "got {}, upstream {total}",
        evaluation.total
    );
    assert!(
        !evaluation.complete,
        "four groups still need the shared stat pipeline"
    );
    assert_eq!(evaluation.excluded.len(), 4);
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
fn the_formula_tree_reproduces_the_upstream_tree_and_total() {
    let full = import(&fixture()).expect("the fixture imports");
    check_trees("fixtures/data.json", &full.coin, &oracle()["full"]);
    let simple = import(&simple_save().to_string()).expect("the simple save imports");
    check_trees("simple", &simple.coin, &oracle()["simple"]);
}
