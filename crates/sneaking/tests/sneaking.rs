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
    Account, EquipError, Issue, Job, Loadout, Objective, Request, SlotKind, action_speed,
    breakdown, can_equip_pair, charm_stat, detection_base, detection_chance, door_eta, door_hp,
    ko_duration, ninja_bonus, stealth_for_detection, stealth_value, weapon_stat,
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

// ----- SN-03: the charm placement search -----

/// Every charm the account owns, wherever it sits.
fn charms_owned(loadout: &Loadout) -> Vec<(String, u32)> {
    let mut owned: Vec<(String, u32)> = loadout
        .twins()
        .iter()
        .flat_map(|twin| twin.charms())
        .chain(loadout.inventory().iter().cloned())
        .filter(|item| !item.is_blank())
        .map(|item| (item.key.clone(), item.level))
        .collect();
    owned.sort();
    owned
}

#[test]
fn the_optimizer_beats_the_saved_placement() {
    let loadout = loadout();
    for objective in [Objective::Jade, Objective::Exp] {
        let outcome = loadout.optimize(Request { objective });
        assert!(
            outcome.improved(),
            "{objective}: {} → {}",
            outcome.baseline.score,
            outcome.winner.score
        );
        assert!(outcome.gain_percent() > 0.0);
        assert_eq!(outcome.request.objective, objective);
        assert!(outcome.evaluations > 0);
        // The recommendation never wears a copy the account does not own.
        let owned = charms_owned(&loadout);
        let worn: Vec<(String, u32)> = outcome
            .winner
            .charms
            .iter()
            .flat_map(|(_, pair)| pair.clone())
            .filter(|item| !item.is_blank())
            .map(|item| (item.key, item.level))
            .collect();
        for copy in &worn {
            let owned = owned.iter().filter(|other| *other == copy).count();
            let used = worn.iter().filter(|other| *other == copy).count();
            assert!(used <= owned, "{copy:?} worn {used} times, owns {owned}");
        }
        // The floor, the hat and the weapon are the save's, not the search's.
        for twin in loadout.twins() {
            let row = outcome
                .winner
                .twins
                .iter()
                .find(|row| row.twin == twin.index)
                .expect("every twin scores");
            assert_eq!(row.floor, twin.floor);
        }
    }
}

#[test]
fn applying_a_recommendation_moves_only_charms() {
    let loadout = loadout();
    let outcome = loadout.optimize(Request {
        objective: Objective::Jade,
    });
    let mut applied = loadout.clone();
    assert!(applied.apply(&outcome.winner));
    assert_eq!(
        charms_owned(&applied),
        charms_owned(&loadout),
        "every owned charm is still somewhere"
    );
    for (before, after) in loadout.twins().iter().zip(applied.twins()) {
        assert_eq!(before.floor, after.floor);
        assert_eq!(before.slot(SlotKind::Hat), after.slot(SlotKind::Hat));
        assert_eq!(before.slot(SlotKind::Weapon), after.slot(SlotKind::Weapon));
        // A captive twin keeps its charms: the search never touches it.
        let expected = outcome
            .winner
            .charms
            .iter()
            .find(|(index, _)| *index == before.index)
            .map_or_else(|| before.charms(), |(_, pair)| pair.clone());
        assert_eq!(applied.twin(before.index).unwrap().charms(), expected);
    }
    // The loadout then scores exactly what the recommendation promised.
    let expected = outcome.winner.score;
    let actual = applied.score(Objective::Jade);
    assert!(
        (actual - expected).abs() <= 1e-9 * expected.abs().max(1.0),
        "applied {actual}, promised {expected}"
    );
}

#[test]
fn the_objective_switch_changes_what_is_measured() {
    let loadout = loadout();
    let jade = loadout.score(Objective::Jade);
    let exp = loadout.score(Objective::Exp);
    let items = loadout.score(Objective::Items);
    assert!(jade.is_finite() && exp.is_finite() && items.is_finite());
    assert!(jade > 0.0 && exp > 0.0 && items > 0.0);
    assert_ne!(jade, exp);
    // The fixture's twins wear Gloves, so no twin hits a door or frees a captive.
    assert_eq!(loadout.score(Objective::Door), 0.0);
    assert_eq!(loadout.score(Objective::Untie), 0.0);
}

// ----- SN-04: the floor × mastery table, the active door and the captive -----

/// The word upstream prints for an action.
fn job_name(job: Job) -> &'static str {
    match job {
        Job::Jade => "normal",
        other => other.label(),
    }
}

fn option(cell: &Value) -> Option<f64> {
    match cell {
        Value::Null => None,
        cell => Some(value(cell)),
    }
}

#[test]
fn the_floor_mastery_table_matches_upstream() {
    let loadout = loadout();
    let oracle = oracle();
    let expected = &oracle["floorMastery"];
    let rows = loadout.floor_mastery();
    let upstream_rows = expected["rows"].as_array().expect("floor rows");
    assert_eq!(rows.len(), upstream_rows.len());
    let levels: Vec<u32> = expected["levels"]
        .as_array()
        .expect("mastery levels")
        .iter()
        .map(|level| level.as_u64().unwrap() as u32)
        .collect();
    assert_eq!(loadout.mastery_levels(), levels);
    for row in &rows {
        let name = format!("floor {} at M{}", row.floor, row.mastery);
        let upstream = upstream_rows
            .iter()
            .find(|candidate| {
                candidate["floor"].as_u64().unwrap() as usize == row.floor
                    && candidate["mastery"].as_u64().unwrap() as u32 == row.mastery
            })
            .unwrap_or_else(|| panic!("{name}: upstream records the row"));
        assert_eq!(
            row.current,
            upstream["current"].as_bool().unwrap(),
            "{name}"
        );
        // The mastery values themselves: the flowers, the floor bases and the
        // mastery-scaled door HP.
        close(
            row.flowers,
            value(&upstream["flowers"]),
            &format!("{name} flowers"),
        );
        close(
            row.funeral_percent,
            value(&upstream["funeralPercent"]),
            &format!("{name} funeral"),
        );
        close(
            row.detection_base,
            value(&upstream["detectionBase"]),
            &format!("{name} detection base"),
        );
        close(
            row.jade_base,
            value(&upstream["jadeBase"]),
            &format!("{name} jade base"),
        );
        close(
            row.exp_base,
            value(&upstream["expBase"]),
            &format!("{name} exp base"),
        );
        close(
            row.door_hp,
            value(&upstream["doorHp"]),
            &format!("{name} door HP"),
        );
        assert_eq!(
            row.door_state,
            upstream["doorState"].as_str().unwrap(),
            "{name} door state"
        );
        // Who stands where and what the floor's action is at this mastery.
        let twins: Vec<usize> = upstream["twins"]
            .as_array()
            .expect("twins")
            .iter()
            .map(|twin| twin.as_u64().unwrap() as usize)
            .collect();
        assert_eq!(row.twins, twins, "{name} twins");
        let jobs: Vec<&str> = upstream["jobs"]
            .as_array()
            .expect("jobs")
            .iter()
            .map(|job| job.as_str().unwrap())
            .collect();
        let actual_jobs: Vec<&str> = row.jobs.iter().copied().map(job_name).collect();
        assert_eq!(actual_jobs, jobs, "{name} jobs");
        assert_eq!(
            row.captive,
            upstream["captive"].as_u64().map(|index| index as usize),
            "{name} captive"
        );
        close(
            row.untie_remaining,
            value(&upstream["untieRemaining"]),
            &format!("{name} untie remaining"),
        );
        // The loadout's own rates stay finite, and an empty floor has no
        // detection to report.
        assert!(row.jade_per_hour.is_finite() && row.jade_per_hour >= 0.0);
        assert!(row.exp_per_hour.is_finite() && row.exp_per_hour >= 0.0);
        assert_eq!(row.best_detection.is_some(), !row.twins.is_empty());
    }
}

#[test]
fn the_active_door_and_next_captive_match_upstream() {
    let loadout = loadout();
    let upstream = &oracle()["floorMastery"];
    let door = loadout
        .active_door()
        .expect("the fixture has an unfinished door");
    let expected = &upstream["activeDoor"];
    assert_eq!(door.floor, expected["floor"].as_u64().unwrap() as usize);
    close(door.max_hp, value(&expected["maxHp"]), "door max HP");
    close(door.progress, value(&expected["progress"]), "door progress");
    close(
        door.remaining_hp,
        value(&expected["remainingHp"]),
        "door remaining HP",
    );
    // The fixture's twins wear Gloves, so the saved placement cannot hit it.
    assert!(door.eta.is_none());

    let captive = loadout.next_captive().expect("the fixture holds a captive");
    let expected = &upstream["nextCaptive"];
    assert_eq!(captive.twin, expected["twin"].as_u64().unwrap() as usize);
    assert_eq!(captive.floor, expected["floor"].as_u64().unwrap() as usize);
    close(
        captive.remaining,
        value(&expected["remaining"]),
        "untie remaining",
    );
    assert!(captive.eta.is_none());
}

#[test]
fn the_door_clear_estimate_matches_upstream() {
    let functions = &oracle()["functions"];
    for case in functions["doorClearEstimate"]
        .as_array()
        .expect("estimates")
    {
        let remaining = value(&case["remaining"]);
        let damage = value(&case["damage"]);
        let hits = value(&case["hits"]);
        // An endless estimate is the port's `None`, which the table prints as
        // the door's state instead of an infinity.
        let expected = option(&case["hours"]).filter(|hours| hours.is_finite());
        let actual = door_eta(remaining, damage, hits);
        match (actual, expected) {
            (None, None) => {}
            (Some(actual), Some(expected)) => close(
                actual,
                expected,
                &format!("estimate {remaining}/{damage}/{hits}"),
            ),
            (actual, expected) => {
                panic!("estimate {remaining}/{damage}/{hits}: {actual:?} vs {expected:?}")
            }
        }
    }
}

#[test]
fn the_door_eta_and_empty_floors_follow_the_loadout() {
    let mut loadout = loadout();
    let table = loadout.floor_mastery();
    // An empty floor is a state, not a blank row: no twins, no detection, and
    // the door says why it cannot be hit.
    let empty = table
        .iter()
        .find(|row| row.floor == 5 && row.current)
        .expect("floor five is in the table");
    assert!(empty.twins.is_empty());
    assert!(empty.best_detection.is_none());
    assert_eq!(empty.door_state, "Locked by F4");
    // A locked door has no remaining HP, so the estimate is zero; the state is
    // what the table prints.
    assert_eq!(empty.door_eta, Some(0.0));
    // The captured twin's floor shows the captive and no Kunai wielder.
    let captive = table
        .iter()
        .find(|row| row.floor == 10 && row.current)
        .expect("floor ten is in the table");
    assert_eq!(captive.captive, Some(10));
    assert!(captive.untie_eta.is_none());
    assert!(loadout.active_door().unwrap().eta.is_none());

    // A Nunchaku on the active door's floor gives the ETA a value; the loadout
    // edit is what moves it, so the table reads the editor's own state.
    loadout
        .equip(0, SlotKind::Weapon, 22)
        .expect("inventory row 22 holds a Nunchaku");
    let attack = loadout.active_door().expect("the door is still unfinished");
    assert!(attack.damage_per_hour > 0.0);
    assert!(
        attack
            .eta
            .is_some_and(|hours| hours.is_finite() && hours > 0.0)
    );
    let floor = loadout
        .floor_mastery()
        .into_iter()
        .find(|row| row.floor == attack.floor && row.current)
        .expect("the active floor is in the table");
    assert_eq!(floor.door_eta, attack.eta);
    assert!(loadout.next_captive().unwrap().eta.is_none());
}

#[test]
fn a_projected_mastery_relocks_the_floors() {
    let loadout = loadout();
    let (mastery, _) = loadout.mastery();
    let table = loadout.floor_mastery();
    let current = table
        .iter()
        .find(|row| row.floor == 4 && row.current)
        .expect("floor four at the saved mastery");
    let projected = table
        .iter()
        .find(|row| row.floor == 4 && row.mastery == mastery + 1)
        .expect("the next mastery projects floor four");
    assert!(!projected.current);
    assert_eq!(projected.flowers, 0.0);
    assert_eq!(projected.door_state, "Fresh");
    assert!(projected.door_hp > current.door_hp);
}
