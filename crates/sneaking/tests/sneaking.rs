//! The Sneaking loadout against the upstream calculator.
//!
//! `fixtures/sneaking.json` is written by `script/sneaking-oracle.mjs` from
//! `js/stats/systems/w6/sneaking-math.js`, `sneaking-stealth.js` and
//! `sneaking-optimizer.js`, so the port is checked against the source of truth
//! rather than hand-picked numbers. The committed synthetic save
//! (`fixtures/data.json`) is the input both sides read.
//!
//! The one account term the `Ninja` rows do not carry is the all-talent
//! multiplier inside the gemstone bonus; the oracle records upstream's value and
//! the test hands it to [`idleon_sneaking::Account`].

use idleon_common::FormulaNode;
use idleon_save::Save;
use idleon_sneaking::{
    Account, EquipError, Issue, Job, Loadout, SlotKind, action_speed, breakdown, can_equip_pair,
    charm_stat, detection_base, detection_chance, door_hp, ko_duration, ninja_bonus,
    stealth_for_detection, stealth_value, weapon_stat,
};
use serde_json::{Value, json};

/// The loadout with the account terms the importer resolves.
fn read_loadout(save: &Save) -> Option<Loadout> {
    Loadout::from_save(save, |loadout| {
        idleon_account::sneaking_account(save, loadout)
    })
}

fn read(path: &str) -> String {
    let path = format!("{}/{path}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|_| panic!("{path} is committed"))
}

fn oracle() -> Value {
    serde_json::from_str(&read("../../fixtures/sneaking.json")).expect("the oracle JSON reads")
}

fn save() -> Save {
    Save::new(serde_json::from_str(&read("../../fixtures/data.json")).expect("the fixture save"))
}

fn loadout() -> Loadout {
    read_loadout(&save()).expect("the fixture carries Ninja rows")
}

fn numbers(value: &Value) -> Vec<f64> {
    value
        .as_array()
        .expect("a number array")
        .iter()
        .map(|cell| cell.as_f64().expect("a number"))
        .collect()
}

/// A formula value, including the `"inf"` marker the oracle writes when a
/// mastery-scaled row overflows.
fn value(cell: &Value) -> f64 {
    match cell {
        Value::String(text) if text == "inf" => f64::INFINITY,
        _ => cell.as_f64().expect("a number or the inf marker"),
    }
}

fn close(actual: f64, expected: f64, name: &str) {
    if expected.is_infinite() {
        assert!(
            actual.is_infinite(),
            "{name}: got {actual}, upstream {expected}"
        );
        return;
    }
    let tolerance = 1e-9 * expected.abs().max(1.0);
    assert!(
        (actual - expected).abs() <= tolerance,
        "{name}: got {actual}, upstream {expected}"
    );
}

#[test]
fn the_fixture_roster_matches_upstream() {
    let loadout = loadout();
    let oracle = oracle();
    let twins = oracle["twins"].as_array().expect("twins");
    assert_eq!(loadout.twins().len(), twins.len());
    assert_eq!(loadout.players(), twins.len());
    for (twin, expected) in loadout.twins().iter().zip(twins) {
        let name = format!("twin {}", twin.index);
        assert_eq!(
            twin.floor,
            expected["floor"].as_u64().unwrap() as usize,
            "{name} floor"
        );
        assert_eq!(
            twin.untied,
            expected["untied"].as_bool().unwrap(),
            "{name} untied"
        );
        assert_eq!(
            twin.level,
            expected["level"].as_u64().unwrap() as u32,
            "{name} level"
        );
        let equip = expected["equip"].as_array().expect("equipment");
        for (slot, item) in SlotKind::ALL.into_iter().zip(equip) {
            assert_eq!(
                twin.slot(slot).key,
                item["key"].as_str().unwrap(),
                "{name} {}",
                slot.label()
            );
            assert_eq!(
                u64::from(twin.slot(slot).level),
                item["level"].as_u64().unwrap(),
                "{name} {} level",
                slot.label()
            );
        }
    }
    assert!(
        loadout.charmed(),
        "the fixture's emporium string carries Charmed"
    );
    assert_eq!(loadout.mastery(), (7, 7));
}

#[test]
fn the_inventory_matches_upstream() {
    let loadout = loadout();
    let oracle = oracle();
    let expected = oracle["inventory"].as_array().expect("inventory");
    let carried: Vec<(usize, String, u32)> = loadout
        .inventory()
        .iter()
        .enumerate()
        .filter(|(_, item)| !item.is_blank())
        .map(|(row, item)| (row, item.key.clone(), item.level))
        .collect();
    assert_eq!(carried.len(), expected.len());
    for (actual, expected) in carried.iter().zip(expected) {
        let name = format!("inventory row {}", actual.0);
        assert_eq!(
            actual.0,
            expected["row"].as_u64().unwrap() as usize,
            "{name}"
        );
        assert_eq!(actual.1, expected["key"].as_str().unwrap(), "{name}");
        assert_eq!(
            u64::from(actual.2),
            expected["level"].as_u64().unwrap(),
            "{name}"
        );
    }
}

#[test]
fn the_gold_inventory_bonuses_match_upstream() {
    let oracle = oracle();
    let inputs = &oracle["inputs"];
    let loadout = loadout().with_account(Account {
        talent_432: inputs["talent432"].as_f64().expect("talent 432"),
        ..Account::default()
    });
    assert_eq!(
        loadout.legend_points(6),
        inputs["legend6"].as_f64().unwrap()
    );
    let gemstones = numbers(&inputs["gemstones"]);
    for (index, expected) in gemstones.iter().enumerate() {
        close(
            loadout.gemstone(index),
            *expected,
            &format!("gemstone {index}"),
        );
    }
    let gold = loadout.gold_bonuses();
    let expected = &oracle["gold"]["bonuses"];
    for (bonus_type, value) in expected.as_object().expect("the gold map") {
        let index: i32 = bonus_type.parse().expect("a bonus type");
        close(
            gold.get(index),
            value.as_f64().unwrap(),
            &format!("gold {index}"),
        );
    }
    for (bonus_type, pick) in oracle["gold"]["selected"]
        .as_object()
        .expect("the selected map")
    {
        let index: i32 = bonus_type.parse().expect("a bonus type");
        let chosen = gold
            .selected(index)
            .unwrap_or_else(|| panic!("a Gold charm is selected for {index}"));
        assert_eq!(
            chosen.row,
            pick["row"].as_u64().unwrap() as usize,
            "{index} row"
        );
        assert_eq!(chosen.key, pick["key"].as_str().unwrap(), "{index} key");
        close(
            chosen.raw,
            pick["raw"].as_f64().unwrap(),
            &format!("{index} raw"),
        );
    }
}

#[test]
fn the_charm_maps_and_action_speeds_match_upstream() {
    let oracle = oracle();
    let loadout = loadout().with_account(Account {
        talent_432: oracle["inputs"]["talent432"].as_f64().unwrap(),
        ..Account::default()
    });
    let maps = oracle["charmMaps"].as_array().expect("charm maps");
    for (twin, map) in loadout.twins().iter().zip(maps) {
        let bonuses = loadout.bonuses(twin.index);
        for (bonus_type, value) in map.as_object().expect("a bonus map") {
            let index: i32 = bonus_type.parse().expect("a bonus type");
            close(
                bonuses.get(index),
                value.as_f64().unwrap(),
                &format!("twin {} bonus {index}", twin.index),
            );
        }
        assert_eq!(
            bonuses.nonzero().count(),
            map.as_object().unwrap().len(),
            "twin {} carries every upstream bonus type",
            twin.index
        );
    }
    let speeds = numbers(&oracle["actionSpeeds"]);
    for (twin, expected) in loadout.twins().iter().zip(speeds) {
        close(
            loadout.action_speed(twin.index),
            expected,
            &format!("twin {} action speed", twin.index),
        );
    }
}

#[test]
fn the_pure_formulas_match_upstream() {
    let oracle = oracle();
    let functions = &oracle["functions"];
    let knowledge = numbers(&oracle["inputs"]["knowledge"]);
    let omni = numbers(&oracle["inputs"]["omni"]);
    let mastery = oracle["inputs"]["mastery"].as_u64().unwrap() as u32;

    for row in functions["ninjaBonus"].as_array().expect("ninja bonuses") {
        let index = row["index"].as_u64().unwrap() as usize;
        close(
            ninja_bonus(&knowledge, &omni, index),
            value(&row["value"]),
            &format!("NLbonuses({index})"),
        );
    }
    for row in functions["actionSpeed"].as_array().expect("action speeds") {
        close(
            action_speed(&knowledge, &omni, value(&row["equipSpeed"])),
            value(&row["value"]),
            "actionSpeed",
        );
    }
    for row in functions["koDuration"].as_array().expect("ko durations") {
        close(
            ko_duration(&knowledge, &omni, value(&row["equipKo"])),
            value(&row["value"]),
            "koDuration",
        );
    }
    for row in functions["detectionBase"].as_array().expect("bases") {
        let floor = row["floor"].as_u64().unwrap() as usize;
        let level = row["mastery"].as_u64().unwrap() as u32;
        close(
            detection_base(floor, level),
            value(&row["value"]),
            &format!("detection base floor {floor} mastery {level}"),
        );
    }
    for row in functions["detection"].as_array().expect("chances") {
        close(
            detection_chance(
                value(&row["stealth"]),
                detection_base(
                    row["floor"].as_u64().unwrap() as usize,
                    row["mastery"].as_u64().unwrap() as u32,
                ),
            ),
            value(&row["chance"]),
            "detectionChance",
        );
    }
    for row in functions["stealthValue"].as_array().expect("stealth") {
        close(
            stealth_value(
                value(&row["sneakLevel"]),
                &knowledge,
                &omni,
                row["multi"].as_f64().unwrap(),
                value(&row["funeralPct"]),
            ),
            value(&row["value"]),
            "stealthValue",
        );
    }
    for row in functions["doorHp"].as_array().expect("door hp") {
        close(
            door_hp(
                row["floor"].as_u64().unwrap() as usize,
                row["mastery"].as_u64().unwrap() as u32,
            ),
            value(&row["value"]),
            "doorHP",
        );
    }
    for row in functions["stealthForDetection"]
        .as_array()
        .expect("targets")
    {
        close(
            stealth_for_detection(
                row["floor"].as_u64().unwrap() as usize,
                row["mastery"].as_u64().unwrap() as u32,
                value(&row["target"]),
            ),
            value(&row["value"]),
            "stealthForDetection",
        );
    }
    assert_eq!(mastery, 7);
}

#[test]
fn every_catalog_item_matches_upstream() {
    let oracle = oracle();
    let rows = oracle["functions"]["items"].as_array().expect("item stats");
    assert!(rows.len() > 400, "the oracle covers the catalog");
    for row in rows {
        let key = row["key"].as_str().unwrap();
        let level = row["level"].as_f64().unwrap();
        close(
            charm_stat(key, level),
            value(&row["charm"]),
            &format!("{key} charm at {level}"),
        );
        close(
            weapon_stat(key, level),
            value(&row["weapon"]),
            &format!("{key} weapon at {level}"),
        );
    }
}

#[test]
fn the_charm_pair_rule_matches_upstream() {
    let oracle = oracle();
    for row in oracle["functions"]["pairs"].as_array().expect("pairs") {
        let first = row["first"].as_str().unwrap();
        let second = row["second"].as_str().unwrap();
        let charmed = row["charmed"].as_bool().unwrap();
        assert_eq!(
            can_equip_pair(first, second, charmed),
            row["allowed"].as_bool().unwrap(),
            "{first} + {second} charmed={charmed}"
        );
    }
}

/// A minimal save: three characters, the first twin wearing two copies of one
/// charm, the second carrying Scroll of Power, and four inventory rows.
fn small_save() -> Value {
    let mut ninja = vec![json!([4, 0, 0, 10])];
    ninja.resize(12, json!([4, 0, 0, 10]));
    // Twin one: hat, gloves, and a same-key charm pair.
    ninja.push(json!(["NjItem0", 0]));
    ninja.push(json!(["NjGl2", 10]));
    ninja.push(json!(["NjTr4", 100]));
    ninja.push(json!(["NjTr4", 200]));
    // Twin two: hat, nunchaku, Scroll of Power.
    ninja.push(json!(["NjItem0", 0]));
    ninja.push(json!(["NjWep5", 10]));
    ninja.push(json!(["NjTr7", 5]));
    ninja.push(json!(["Blank", 0]));
    // The rest of the equipment rows start blank.
    ninja.resize(60, json!(["Blank", 0]));
    // Inventory: Rosaries, a second Scroll, a hat and a stronger Silk Veil.
    ninja.push(json!(["NjTr17", 300]));
    ninja.push(json!(["NjTr7", 5]));
    ninja.push(json!(["NjItem0", 0]));
    ninja.push(json!(["NjTr4", 200]));
    ninja.resize(100, json!(["Blank", 0]));
    ninja.push(json!([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]));
    ninja.push(json!([0]));
    ninja.push(json!([0, 0, 0, 0, 0, 0, 0, 0, 0, "dc"]));
    let mut knowledge = vec![json!(0); 32];
    knowledge[4] = json!(10);
    knowledge[13] = json!(5);
    knowledge[15] = json!(10);
    knowledge[21] = json!(4);
    knowledge[23] = json!(2);
    ninja.push(Value::Array(knowledge));
    let mut spelunk = vec![Value::Null; 19];
    spelunk[14] = json!([1, 0, 0, 0]);
    spelunk[15] = json!([0, 100, 0, 0]);
    spelunk[18] = json!([0, 0, 0, 0, 0, 0, 24]);
    json!({
        "Ninja": Value::Array(ninja),
        "Spelunk": Value::Array(spelunk),
        "Menagerie": json!([null, null, null, null, null, null, null, [0, 0, 0, 0, 0, 0, 0, 0]]),
        "OptLacc": json!([]),
        "Lv0_0": json!([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 500]),
        "Lv0_1": json!([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 400]),
        "Lv0_2": json!([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 300]),
        "charNames": json!(["Ana", "Bo", "Cy"]),
    })
}

fn small() -> Save {
    Save::new(small_save())
}

/// The same save with the emporium string cleared, so Charmed is not owned.
fn uncharmed_save() -> Save {
    let mut save = small_save();
    save["Ninja"][102] = json!([0, 0, 0, 0, 0, 0, 0, 0, 0, "x"]);
    Save::new(save)
}

#[test]
fn an_edit_swaps_the_slot_and_updates_the_formulas() {
    let mut loadout = read_loadout(&small()).expect("the small save reads");
    assert!(loadout.charmed());
    assert_eq!(loadout.twins().len(), 3);
    let before = loadout.bonuses(0).get(4);
    let speed_before = loadout.action_speed(0);
    let carried = loadout.twin(0).unwrap().slot(SlotKind::Charm(0)).clone();
    // Row three holds the stronger copy of the same charm.
    loadout
        .equip(0, SlotKind::Charm(0), 3)
        .expect("the row holds a charm");
    let equipped = loadout.twin(0).unwrap().slot(SlotKind::Charm(0));
    assert_eq!((equipped.key.as_str(), equipped.level), ("NjTr4", 200));
    assert_eq!(loadout.inventory()[3], carried);
    assert!(
        loadout.bonuses(0).get(4) > before,
        "the higher charm level raises the Total Stealth bonus"
    );
    assert!(
        (loadout.action_speed(0) - speed_before).abs() < 1e-12,
        "a stealth charm does not change action speed"
    );
    assert!(loadout.changed());
    loadout.reset();
    assert!(!loadout.changed());
    let rested = loadout.twin(0).unwrap().slot(SlotKind::Charm(0));
    assert_eq!((rested.key.as_str(), rested.level), ("NjTr4", 100));
    assert_eq!(loadout.inventory()[3].level, 200);
}

#[test]
fn an_edit_moves_the_stealth_and_detection() {
    let mut loadout = read_loadout(&small()).expect("the small save reads");
    let stealth_before = loadout.stealth(0);
    let detection_before = loadout.detection(0);
    // Rosaries carry the Total Stealth multiplier.
    loadout
        .equip(0, SlotKind::Charm(1), 0)
        .expect("Rosaries go into the second charm slot");
    assert!(loadout.stealth(0) > stealth_before);
    // More stealth never raises the detection chance; a saturated floor stays
    // at zero either way.
    assert!(loadout.detection(0) <= detection_before);
    assert!((0.0..=1.0).contains(&loadout.detection(0)));
}

#[test]
fn a_slot_refuses_an_item_that_does_not_fit() {
    let mut loadout = read_loadout(&small()).expect("the small save reads");
    assert_eq!(
        loadout.equip(0, SlotKind::Charm(0), 2),
        Err(EquipError::WrongSlot("NjItem0".into()))
    );
    assert_eq!(
        loadout.equip(0, SlotKind::Hat, 0),
        Err(EquipError::WrongSlot("NjTr17".into()))
    );
    assert_eq!(
        loadout.equip(0, SlotKind::Charm(0), 4),
        Err(EquipError::EmptySlot)
    );
    assert_eq!(
        loadout.equip(9, SlotKind::Hat, 0),
        Err(EquipError::UnknownTwin)
    );
    assert_eq!(
        loadout.equip(0, SlotKind::Charm(0), 99),
        Err(EquipError::UnknownInventory)
    );
    // Unequipping needs a free inventory row.
    let mut full = read_loadout(&small()).expect("the small save reads");
    full.reset();
    assert_eq!(full.unequip(0, SlotKind::Charm(0)), Ok(()));
    assert_eq!(full.twin(0).unwrap().slot(SlotKind::Charm(0)).key, "Blank");
}

#[test]
fn a_repeated_charm_needs_the_charmed_bonus() {
    let loadout = read_loadout(&small()).expect("the small save reads");
    assert!(loadout.charmed());
    assert!(
        loadout.issues().is_empty(),
        "the save's same-key pair is legal with Charmed: {:?}",
        loadout.issues()
    );
    let mut plain = read_loadout(&uncharmed_save()).expect("the save reads");
    assert!(!plain.charmed());
    let issues = plain.issues();
    assert!(
        issues
            .iter()
            .any(|issue| matches!(issue, Issue::SameCharmPair { twin: 0, key } if key == "NjTr4")),
        "{issues:?}"
    );
    assert_eq!(
        plain.equip(0, SlotKind::Charm(1), 3),
        Err(EquipError::SameCharmPair("NjTr4".into()))
    );
}

#[test]
fn scroll_of_power_is_account_unique() {
    let mut loadout = read_loadout(&small()).expect("the small save reads");
    // Twin two already wears the scroll; the inventory copy cannot join twin one.
    assert_eq!(
        loadout.equip(0, SlotKind::Charm(0), 1),
        Err(EquipError::ScrollEquipped)
    );
    assert!(loadout.issues().is_empty());
    // Freeing the slot makes the inventory copy usable.
    loadout
        .unequip(1, SlotKind::Charm(0))
        .expect("the scroll goes back to the inventory");
    loadout
        .equip(0, SlotKind::Charm(0), 1)
        .expect("the scroll moves to twin one");
}

#[test]
fn the_saved_placement_and_jobs_read_from_the_save() {
    let loadout = read_loadout(&small()).expect("the small save reads");
    assert_eq!(loadout.twin(0).unwrap().floor, 4);
    assert!(
        loadout.twin(0).unwrap().untied,
        "ten progress frees twin one"
    );
    // Gloves on a floor with no active door still do the Jade action.
    assert_eq!(loadout.job(0), Job::Jade);
    assert!(!loadout.twin(2).unwrap().untied);
    assert_eq!(loadout.job(2), Job::Captive);
    assert_eq!(loadout.first_batch().unwrap().0, "Ana");
}

#[test]
fn a_save_without_ninja_rows_has_no_loadout() {
    assert!(read_loadout(&Save::new(json!({ "Lv0_0": json!([]) }))).is_none());
}

fn find<'a>(node: &'a FormulaNode, label: &str) -> Option<&'a FormulaNode> {
    if node.label == label {
        return Some(node);
    }
    node.children.iter().find_map(|child| find(child, label))
}

/// The account systems the port decodes exactly. The trees also carry terms
/// that need the All-Talent resolver, the vials/meals/stamps chains and the
/// Gambit point chain; those are explicit inputs in this build, so their nodes
/// are not compared to the upstream tree yet.
fn assert_factors(tree: &FormulaNode, factors: &serde_json::Map<String, Value>, labels: &[&str]) {
    for label in labels {
        let expected = factors
            .get(*label)
            .unwrap_or_else(|| panic!("the oracle records {label}"))
            .as_f64()
            .unwrap();
        let node = find(tree, label).unwrap_or_else(|| panic!("the tree carries {label}"));
        close(node.value, expected, label);
    }
}

#[test]
fn the_modelled_account_factors_match_upstream() {
    let oracle = oracle();
    let loadout = loadout();
    let breakdowns = &oracle["breakdowns"];

    assert_factors(
        &breakdown::stealth(&loadout, 0),
        breakdowns["stealthFactors"]
            .as_object()
            .expect("stealth factors"),
        &[
            "Compass: Sneaking Stealth",
            "Sneaking Statue",
            "Crystal 5 Card",
            "Achievement 368",
            "Lamp: Sneaking Stealth",
            "Holes Upgrade 54",
            "Fractal Island",
            "Emperor: Sneaking Stealth",
            "RoG: Sneaking Stealth",
            "Jelly Operator: Sneaking Stealth",
            "Shh! Ninja Knowledge",
            "Companion 163",
            "Equipped Charm: EXP, Jade & Stealth",
            "Voting: Sneaking Stealth",
        ],
    );
    assert_factors(
        &breakdown::jade(&loadout, 0),
        breakdowns["jadeFactors"].as_object().expect("jade factors"),
        &[
            "Sneaking Level Jade",
            "Equipped Jade Charms",
            "Summoning Winner: Jade",
            "Jade Achievements",
            "Monument: Jade",
            "Pristine Charm: Jade",
            "Ninja Knowledge 28",
            "Killroy Jade Multiplier",
            "MSA + Sigil + Arcade + Vault",
            "Jade Star Sign",
            "Task Shop Jade",
            "Rift + Compass + Companion",
            "Crop Scientist: Jade",
        ],
    );
    assert_factors(
        &breakdown::exp(&loadout, 0),
        breakdowns["expFactors"].as_object().expect("exp factors"),
        &[
            "Shared Skill EXP Multiplier",
            "Ninja Knowledge",
            "Arcade",
            "Compass: Sneaking EXP",
            "Equipped EXP Charms",
            "No-item-find Penalty",
            "Summoning Winner",
            "Companion: Smoke Devil",
        ],
    );
}
