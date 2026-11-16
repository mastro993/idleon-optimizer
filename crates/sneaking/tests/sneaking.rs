//! The Sneaking loadout read from a save: edits, rules, the charm search and
//! the floor table.

use idleon_save::Save;
use idleon_sneaking::{EquipError, Issue, Job, Loadout, Objective, Request, SlotKind};
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

fn save() -> Save {
    Save::new(serde_json::from_str(&read("../../fixtures/data.json")).expect("the fixture save"))
}

fn loadout() -> Loadout {
    read_loadout(&save()).expect("the fixture carries Ninja rows")
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
    // The inventory's fit check asks about twins by position too.
    let charm = loadout.twin(0).unwrap().slot(SlotKind::Charm(0)).clone();
    assert_eq!(
        loadout.check_slot(9, SlotKind::Charm(1), &charm),
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
