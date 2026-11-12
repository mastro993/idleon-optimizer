//! The endgame audit: the verdict is always `NO`, and every save-table metric
//! matches `js/endgame-stats.js` on the committed fixture and on purpose-built
//! saves ported from the upstream test suite.

use idleon_endgame::{self as endgame, Stat, VERDICT};
use idleon_import::import;
use serde_json::{Map, Value, json};

/// The metrics that need the account stats engine, the Tome scorer or breeding
/// totals, so this build reports them without a value.
const UNPORTED: [&str; 21] = [
    "strength",
    "agility",
    "wisdom",
    "luck",
    "dropRate",
    "cashMulti",
    "defence",
    "accuracy",
    "hp",
    "mp",
    "highestDamage",
    "postOfficeOrders",
    "highestConstructExp/hr",
    "highestSpelunkingPower",
    "highestArcaneMapMulti",
    "highestOutpostResourceRate",
    "highestVillagerExp/hr",
    "totalVillagerExp/hr",
    "totalShinyLevels",
    "totalBreedabilityLevels",
    "totalTomePoints",
];

fn fixture() -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/data.json");
    std::fs::read_to_string(path).expect("fixtures/data.json is committed")
}

fn upstream_stats() -> Value {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/endgame-stats.json"
    );
    serde_json::from_str(&std::fs::read_to_string(path).expect("the upstream audit is committed"))
        .expect("the upstream audit is JSON")
}

fn value(stats: &[Stat], id: &str) -> Option<f64> {
    stats
        .iter()
        .find(|stat| stat.id == id)
        .unwrap_or_else(|| panic!("the audit lists {id}"))
        .value
}

fn detail<'a>(stats: &'a [Stat], id: &str) -> &'a str {
    stats
        .iter()
        .find(|stat| stat.id == id)
        .and_then(|stat| stat.detail.as_deref())
        .unwrap_or("")
}

fn zeros(length: usize) -> Value {
    Value::Array(vec![json!(0); length])
}

fn rows(count: usize) -> Vec<Value> {
    vec![json!([]); count]
}

fn set(rows: &mut [Value], index: usize, value: Value) {
    rows[index] = value;
}

#[test]
fn the_verdict_is_no_with_and_without_a_save() {
    assert_eq!(VERDICT, "NO");
}

#[test]
fn a_save_without_research_rows_fails_before_collecting() {
    assert_eq!(
        endgame::collect(&Value::Null, 0).unwrap_err(),
        "No save data provided."
    );
    assert_eq!(
        endgame::collect(&json!({ "Lv0_0": [1] }), 0).unwrap_err(),
        "Not included in this save."
    );
    assert_eq!(
        endgame::collect(&json!({ "Research": [] }), 0).unwrap_err(),
        "Research data is missing or incomplete."
    );
}

#[test]
fn the_fixture_audit_matches_the_upstream_js() {
    let imported = import(&fixture()).expect("the fixture imports");
    let stats = endgame::collect(&imported.save, imported.named_characters)
        .expect("the fixture has complete research rows");
    let expected = upstream_stats();
    let expected = expected["stats"].as_array().expect("the audit list");
    assert_eq!(stats.len(), expected.len());
    let mut compared = 0;
    for (stat, expected) in stats.iter().zip(expected) {
        assert_eq!(stat.id, expected["id"]);
        assert_eq!(stat.group, expected["group"]);
        assert_eq!(stat.label, expected["label"], "{} label", stat.id);
        if UNPORTED.contains(&stat.id) {
            assert!(stat.value.is_none(), "{} needs the engine", stat.id);
            assert_eq!(stat.detail.as_deref(), Some(endgame::UNPORTED));
            continue;
        }
        let wanted = expected["value"]
            .as_f64()
            .unwrap_or_else(|| panic!("{} has a value upstream", stat.id));
        let got = stat
            .value
            .unwrap_or_else(|| panic!("{} is computed: {:?}", stat.id, stat.detail));
        let scale = wanted.abs().max(1.0);
        assert!(
            (got - wanted).abs() / scale < 1e-9,
            "{}: got {got}, upstream {wanted}",
            stat.id
        );
        assert_eq!(
            stat.partial,
            expected["partial"].as_bool().unwrap_or(false),
            "{} partial",
            stat.id
        );
        compared += 1;
    }
    assert_eq!(compared, 177 - UNPORTED.len());
}

/// The upstream `statSave`: two characters, a few tables, and the loaded options.
fn base_save() -> Value {
    let mut research = rows(60);
    set(&mut research, 0, json!([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 7]));
    set(
        &mut research,
        7,
        json!([0, 0, 0, 0, 0, 0, 0, 0, 0, 12, 0, 0, 345]),
    );
    let mut options = vec![json!(0); 700];
    options[211] = json!(1e9);
    options[212] = json!(2e9);

    let mut data = Map::new();
    data.insert("Research".into(), Value::Array(research));
    data.insert("OptLacc".into(), Value::Array(options));
    data.insert("Lv0_0".into(), json!([100]));
    data.insert("Lv0_1".into(), json!([200]));
    data.insert(
        "Boats".into(),
        json!([[-1, 0, 0, "10", 0, "20"], [-1, 0, 0, 5, 0, 6]]),
    );
    data.insert("Meals".into(), json!([[5, 6]]));
    data.insert("StampLv".into(), json!([[1, 2]]));
    data.insert("CauldronInfo".into(), json!([[], [], [], [], [2, 3]]));
    data.insert("Tower".into(), json!([1, 2]));
    data.insert("Summon".into(), json!([[5, 6]]));
    data.insert("Gaming".into(), json!([0, 0, 0, 0, 0, 0, 0, 0, 1234]));
    Value::Object(data)
}

#[test]
fn saved_tables_and_counters_are_summed_like_upstream() {
    let stats = endgame::collect(&base_save(), 0).expect("complete research rows");
    let expected = [
        ("totalBoats", 41.0),
        ("totalLevels", 300.0),
        ("totalStamps", 3.0),
        ("totalVials", 5.0),
        ("totalMeals", 11.0),
        ("highestNugget", 1234.0),
        ("totalSummoningUpgrades", 11.0),
        ("bestJellyDps", 345.0),
        ("logSample", 1e9),
        ("copperSample", 2e9),
    ];
    for (id, wanted) in expected {
        assert_eq!(value(&stats, id), Some(wanted), "{id}");
    }
    assert_eq!(
        value(&stats, "totalRoyalResourceGrades"),
        None,
        "absent world data is unavailable, not zero"
    );
    assert!(detail(&stats, "totalRoyalResourceGrades").contains("Not included"));
}

/// The upstream `expanded` save: every saved field a metric can read.
fn expanded_save() -> Value {
    let Value::Object(mut data) = base_save() else {
        unreachable!()
    };
    data.insert("Lv0_0".into(), json!([100, 8, 3]));
    data.insert("Lv0_1".into(), json!([200, 5, 6]));
    data.insert("MoneyBANK".into(), json!(100));
    data.insert("Money_0".into(), json!(5));
    data.insert("Money_1".into(), json!(7));
    data.insert(
        "TaskZZ0".into(),
        json!([[11, 0, 22], [33], [44, 55, 0, 66], [], [0, 0, 0, 77]]),
    );
    data.insert(
        "FarmUpg".into(),
        Value::Array((0..20).map(|index| json!(index)).collect()),
    );
    data.insert("CookMaster".into(), json!([[], [9, 123]]));
    data.insert(
        "KRbest".into(),
        json!({ "bean": 4, "frog": "5", "SummzTrz0": 999 }),
    );
    data.insert(
        "Sushi".into(),
        json!([
            [],
            [],
            [1, 2],
            [],
            [0, 0, 0, 0, 0, 0, 0, 0, 123],
            [-1, 0, 1, 2],
            [],
            [3, 4]
        ]),
    );
    let mut gaming = vec![json!(0); 12];
    gaming[9] = json!(999);
    gaming[11] = json!("_abcdefghj");
    data.insert("Gaming".into(), Value::Array(gaming));
    data.insert("Compass".into(), json!([[1, 2], [], [], ["a", "a", "b"]]));
    let mut spelunk = rows(54);
    set(&mut spelunk, 9, json!([3, 4]));
    set(&mut spelunk, 1, json!([3, 5]));
    set(&mut spelunk, 47, json!([7, 11]));
    data.insert("Spelunk".into(), Value::Array(spelunk));
    data.insert(
        "CogM".into(),
        json!([{ "d": 10, "b": 999 }, { "d": 20, "b": 1 }]),
    );
    let mut cauldron = vec![json!([]); 5];
    cauldron[1] = json!([0, 0, 345]);
    cauldron[4] = json!([2, 3]);
    // The five columns the upstream test fills directly.
    cauldron[0] = json!([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 123]);
    cauldron[1] = json!([0, 0, 345, 0, 0, 0, 0, 0, 0, 0, 0, 234]);
    cauldron[3] = json!([
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 456
    ]);
    data.insert("CauldronInfo".into(), Value::Array(cauldron));
    let mut dream = vec![json!(0); 17];
    dream[0] = json!(999);
    dream[1] = json!(888);
    dream[2] = json!(2);
    dream[3] = json!(3);
    dream[16] = json!(999);
    data.insert("Dream".into(), Value::Array(dream));
    let mut holes = vec![zeros(100); 35];
    set(&mut holes, 1, json!([13, 1, 1, 1, 1]));
    set(&mut holes, 3, json!([1, 1, 1, 1, 1]));
    set(&mut holes, 23, json!([0, 0, 0, 0, 0]));
    set(
        &mut holes,
        31,
        Value::Array((0..3).map(|_| zeros(20)).collect()),
    );
    set(
        &mut holes,
        32,
        Value::Array((0..3).map(|_| zeros(20)).collect()),
    );
    data.insert("Holes".into(), Value::Array(holes));
    let mut options: Vec<Value> = (0..700).map(|_| json!(0)).collect();
    for option in options.iter_mut().take(395).skip(388) {
        *option = json!(1);
    }
    for option in options.iter_mut().take(622).skip(616) {
        *option = json!(2);
    }
    options[242] = json!(69);
    options[442] = json!(1317);
    options[419] = json!(999);
    options[435] = json!(888);
    data.insert("OptLacc".into(), Value::Array(options));
    data.insert("FamValMinigameHiscores".into(), json!([11, 22, 33, 44]));
    let mut breeding = rows(31);
    set(&mut breeding, 22, json!([0, 1]));
    data.insert("Breeding".into(), Value::Array(breeding));
    Value::Object(data)
}

#[test]
fn expanded_save_metrics_match_the_upstream_test() {
    let save = expanded_save();
    let stats = endgame::collect(&save, 0).expect("complete research rows");
    let expected = [
        ("totalMoney", 112.0),
        ("mining", 8.0),
        ("smithing", 6.0),
        ("totalSkillsLevels", 22.0),
        ("afkTime", 22.0),
        ("monstersKilled", 11.0),
        ("refinedSalts", 44.0),
        ("trashedCogs", 55.0),
        ("totalMaterialPrinted", 66.0),
        ("plantsPicked", 77.0),
        ("totalDayMarketLevels", 44.0),
        ("totalNightMarketLevels", 108.0),
        ("totalPrimeKills", 9.0),
        ("cookingMasteryLevel", 9.0),
        ("totalSushiStationUpgrades", 3.0),
        ("totalSushiKnowledgeLevels", 7.0),
        ("totalSushiPerfectos", 2.0),
        ("bestSushiCombo", 123.0),
        ("totalPaletteLevels", 7.0),
        ("logBook", 46.0),
        ("totalCompassUpgrades", 3.0),
        ("totalMedallions", 3.0),
        ("totalTachyonsCollected", 1.0),
        ("totalCoralKidUpgrades", 12.0),
        ("totalSpelunkingDepths", 8.0),
        ("totalManicSpelunkingDepths", 18.0),
        ("highestCogExp", 20.0),
        ("highestCallMeBob", 123.0),
        ("highestCallMeAsh", 234.0),
        ("highestHammerHammer", 345.0),
        ("highestBigP", 456.0),
        ("totalEquinoxUpgrades", 5.0),
        ("hoopsMinigame", 69.0),
        ("dartsMinigame", 1317.0),
        ("choppingMinigame", 11.0),
        ("fishingMinigame", 22.0),
        ("catchingMinigame", 33.0),
        ("miningMinigame", 44.0),
    ];
    for (id, wanted) in expected {
        assert_eq!(
            value(&stats, id),
            Some(wanted),
            "{}: {}",
            id,
            detail(&stats, id)
        );
    }
    assert!(value(&stats, "totalShinyLevels").is_none());
    assert!(value(&stats, "totalBreedabilityLevels").is_none());
}

#[test]
fn the_leaderboard_audit_repairs_reject_bad_rows_per_metric() {
    let Value::Object(mut data) = expanded_save() else {
        unreachable!()
    };
    data.insert(
        "Cards0".into(),
        json!({ "frogG": 1000000, "beanG": "2000000", "length": 999 }),
    );
    let Value::Array(mut spelunk) = data["Spelunk"].clone() else {
        unreachable!()
    };
    set(&mut spelunk, 6, json!([1, 2, 3]));
    set(&mut spelunk, 44, json!([4, 7]));
    data.insert("Spelunk".into(), Value::Array(spelunk));
    data.insert("StatueLevels_0".into(), json!([[2, 999999], [10, 999999]]));
    let mut statues = vec![json!([0, 0]); 33];
    statues[0] = json!([7, 0]);
    statues[1] = json!([3, 0]);
    // Index 32 is past the statue list, so the bad row is filtered out.
    statues[32] = json!(["unused padding", 0]);
    data.insert("StatueLevels_1".into(), Value::Array(statues));
    let Value::Array(mut character) = data["Lv0_0"].clone() else {
        unreachable!()
    };
    character.push(json!(-1));
    data.insert("Lv0_0".into(), Value::Array(character));
    let mut options = data["OptLacc"].as_array().cloned().expect("options");
    options[131] = json!(7);
    options[347] = json!(5);
    options[394] = json!(12345);
    data.insert("OptLacc".into(), Value::Array(options));

    let stats = endgame::collect(&Value::Object(data), 0).expect("complete research rows");
    for (id, wanted) in [
        ("totalCards", 3_000_000.0),
        ("totalGrandDiscoveries", 11.0),
        ("totalStatues", 17.0),
        ("totalTachyonsCollected", 12_345.0),
        ("totalSkillsLevels", 22.0),
    ] {
        assert_eq!(value(&stats, id), Some(wanted), "{id}");
    }
}

#[test]
fn missing_or_invalid_fields_only_blank_their_own_metric() {
    let Value::Object(mut data) = expanded_save() else {
        unreachable!()
    };
    data.remove("MoneyBANK");
    let Value::Array(mut spelunk) = data["Spelunk"].clone() else {
        unreachable!()
    };
    // A deleted element comes back as `null` through the JSON round trip.
    set(&mut spelunk, 9, Value::Null);
    data.insert("Spelunk".into(), Value::Array(spelunk));
    let mut options = data["OptLacc"].as_array().cloned().expect("options");
    options[242] = Value::Null;
    data.insert("OptLacc".into(), Value::Array(options));
    let stats = endgame::collect(&Value::Object(data), 0).expect("complete research rows");
    for id in ["totalMoney", "totalPaletteLevels", "hoopsMinigame"] {
        assert!(value(&stats, id).is_none(), "{id} stays unavailable");
        assert!(!detail(&stats, id).is_empty(), "{id} explains itself");
    }

    let Value::Object(mut data) = expanded_save() else {
        unreachable!()
    };
    let mut gaming = data["Gaming"].as_array().cloned().expect("gaming");
    gaming[11] = json!("invalid");
    data.insert("Gaming".into(), Value::Array(gaming));
    let stats = endgame::collect(&Value::Object(data), 0).expect("complete research rows");
    assert!(value(&stats, "logBook").is_none());
    assert_eq!(
        detail(&stats, "logBook"),
        "Saved Gaming logbook is invalid."
    );
}

#[test]
fn truncated_options_and_empty_tables_are_reported_not_guessed() {
    let Value::Object(mut data) = expanded_save() else {
        unreachable!()
    };
    data.remove("Boats");
    let Value::Array(mut gaming) = data["Gaming"].clone() else {
        unreachable!()
    };
    gaming[8] = json!("invalid");
    data.insert("Gaming".into(), Value::Array(gaming));
    let Value::Array(mut summon) = data["Summon"].clone() else {
        unreachable!()
    };
    summon[0] = json!([]);
    data.insert("Summon".into(), Value::Array(summon));
    let stats = endgame::collect(&Value::Object(data), 0).expect("complete research rows");
    assert!(value(&stats, "totalBoats").is_none());
    assert!(value(&stats, "highestNugget").is_none());
    assert_eq!(value(&stats, "totalSummoningUpgrades"), Some(0.0));

    let Value::Object(mut data) = expanded_save() else {
        unreachable!()
    };
    let mut options = data["OptLacc"].as_array().cloned().expect("options");
    options.truncate(390);
    data.insert("OptLacc".into(), Value::Array(options));
    let stats = endgame::collect(&Value::Object(data), 0).expect("complete research rows");
    for id in [
        "totalTachyonsCollected",
        "totalCoralKidUpgrades",
        "dartsMinigame",
    ] {
        assert!(value(&stats, id).is_none(), "{id} requires every counter");
    }
}

#[test]
fn a_named_character_without_rows_cannot_certify_the_statue_maximum() {
    let save = json!({
        "Research": [json!([]), json!([]), json!([]), json!([]), json!([]),
                     json!([]), json!([]), json!([]), json!([])],
        "Lv0_0": [10],
        "StatueLevels_0": [[5, 0]],
    });
    // A `charNames` roster adds characters whose saved rows are absent.
    let stats = endgame::collect(&save, 3).expect("complete research rows");
    let statues = stats
        .iter()
        .find(|stat| stat.id == "totalStatues")
        .expect("the audit lists totalStatues");
    assert_eq!(statues.value, Some(5.0));
    assert!(statues.partial, "two characters have no statue rows");
}
