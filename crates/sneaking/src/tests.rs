//! The loadout's own formula tests: the rules that do not need the oracle.

use super::*;

#[test]
fn an_empty_item_is_blank() {
    assert!(Item::blank().is_blank());
    assert!(!Item::new("NjTr4", 0).is_blank());
    assert_eq!(Item::new("NjItem0", 0).name(), "Straw Hat");
    assert_eq!(Item::new("Nope", 1).name(), "Nope");
}

#[test]
fn slots_map_to_save_rows() {
    assert_eq!(SlotKind::of(0), Some(SlotKind::Hat));
    assert_eq!(SlotKind::of(2), Some(SlotKind::Charm(0)));
    assert_eq!(SlotKind::of(3), Some(SlotKind::Charm(1)));
    assert_eq!(SlotKind::of(4), None);
    for slot in SlotKind::ALL {
        assert_eq!(SlotKind::of(slot.index()), Some(slot));
    }
    let hat = item("NjItem0").expect("the catalog carries NjItem0");
    let charm = item("NjTr4").expect("the catalog carries NjTr4");
    assert!(SlotKind::Hat.accepts(hat));
    assert!(!SlotKind::Weapon.accepts(hat));
    assert!(SlotKind::Charm(1).accepts(charm));
    assert!(!SlotKind::Hat.accepts(charm));
}

#[test]
fn the_emporium_letter_marks_the_charmed_bonus() {
    assert!(charmed_perk("dc"));
    assert!(charmed_perk("_lehvAraqxsympzgiwnBtbcojkfu"));
    assert!(!charmed_perk(""));
    assert!(!charmed_perk("x"));
}

#[test]
fn a_charm_stat_grows_towards_its_maximum() {
    let low = charm_stat("NjTr4", 100.0);
    let high = charm_stat("NjTr4", 9000.0);
    assert!(high > low, "{low} -> {high}");
    assert_eq!(charm_stat("NjItem0", 100.0), 0.0, "a hat is not a charm");
    assert_eq!(charm_stat("Nope", 100.0), 0.0);
}

#[test]
fn a_hat_has_no_weapon_stat() {
    assert_eq!(weapon_stat("NjItem0", 500.0), 0.0);
    assert_eq!(weapon_stat("Nope", 500.0), 0.0);
}

#[test]
fn the_tooltip_names_the_effect() {
    let info = equip_info(&Item::new("NjTr4", 1000)).expect("a charm tooltip");
    assert_eq!(
        info.label,
        charm_bonus_label(item("NjTr4").unwrap().bonus_type)
    );
    assert!(info.stat.is_some());
    let hat = equip_info(&Item::new("NjItem0", 0)).expect("a hat tooltip");
    assert_eq!(hat.label, "Hat");
    assert!(hat.stat.is_none());
    assert!(equip_info(&Item::new("Nope", 0)).is_none());
}

// ----- SN-03: the charm placement search -----

use idleon_common::Control;

/// A loadout with a roster and an inventory, and every table at zero.
fn empty_loadout(twins: Vec<Twin>, inventory: Vec<Item>) -> Loadout {
    let players = twins.len();
    Loadout::new(
        twins,
        inventory,
        vec![0.0; 40],
        vec![0.0; 40],
        vec![0.0; 64],
        vec![0.0; FLOOR_COUNT],
        vec![0.0; 20],
        vec![0.0; 12],
        vec![0.0; FLOOR_COUNT],
        vec![0.0; 12],
        3,
        0,
        0,
        false,
        players,
        Vec::new(),
    )
}

/// A free twin: a straw hat, a weapon and two charm slots.
fn equipped(index: usize, floor: usize, weapon: &str, charms: [Item; 2]) -> Twin {
    Twin::new(
        index,
        format!("Twin {}", index + 1),
        floor,
        0.0,
        0.0,
        100,
        [
            Item::new("NjItem0", 0),
            Item::new(weapon, 1000),
            charms[0].clone(),
            charms[1].clone(),
        ],
    )
}

/// A twin still trapped by its captive: `NinjaInfo[8]` needs ten points and the
/// save has none.
fn captive(index: usize) -> Twin {
    Twin::new(
        index,
        format!("Twin {}", index + 1),
        0,
        0.0,
        10.0,
        100,
        [
            Item::new("NjItem0", 0),
            Item::new("NjGl2", 1000),
            Item::blank(),
            Item::blank(),
        ],
    )
}

/// The pair a layout gives one twin.
fn worn(layout: &Layout, twin: usize) -> [Item; 2] {
    layout
        .charms
        .iter()
        .find(|(index, _)| *index == twin)
        .map(|(_, pair)| pair.clone())
        .unwrap_or_else(|| [Item::blank(), Item::blank()])
}

/// The keys one pair holds, so an assertion does not depend on which slot the
/// search filled first.
fn keys(pair: &[Item; 2]) -> Vec<&str> {
    pair.iter().map(|item| item.key.as_str()).collect()
}

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
fn the_objective_ids_round_trip() {
    for objective in Objective::ALL {
        assert_eq!(Objective::from_id(objective.id()), Some(objective));
        assert_eq!(objective.to_string(), objective.label());
        assert!(objective.unit().ends_with("/ hour"));
    }
    assert_eq!(Objective::from_id("nope"), None);
    assert_eq!(Objective::default(), Objective::Jade);
}

#[test]
fn the_jade_objective_wears_the_jade_charm_and_the_exp_objective_the_exp_charm() {
    let mut inventory = vec![Item::blank(); INVENTORY_SLOTS];
    inventory[0] = Item::new("NjTr14", 5000);
    inventory[1] = Item::new("NjTr13", 5000);
    let loadout = empty_loadout(
        vec![equipped(0, 4, "NjGl2", [Item::blank(), Item::blank()])],
        inventory,
    );

    let jade = loadout.optimize(Request {
        objective: Objective::Jade,
    });
    assert!(jade.improved(), "the jade charm beats an empty pair");
    let jade_pair = worn(&jade.winner, 0);
    let pair = keys(&jade_pair);
    assert!(pair.contains(&"NjTr14"), "wears the jade charm: {pair:?}");
    assert!(
        !pair.contains(&"NjTr13"),
        "the EXP charm adds nothing to Jade: {pair:?}"
    );

    let exp = loadout.optimize(Request {
        objective: Objective::Exp,
    });
    assert!(exp.improved(), "the EXP charm beats an empty pair");
    let exp_pair = worn(&exp.winner, 0);
    let pair = keys(&exp_pair);
    assert!(pair.contains(&"NjTr13"), "wears the EXP charm: {pair:?}");
    assert!(
        !pair.contains(&"NjTr14"),
        "the Jade charm adds nothing to EXP: {pair:?}"
    );
    assert_ne!(jade.winner.charms, exp.winner.charms);
}

#[test]
fn the_door_objective_scores_the_twin_holding_the_nunchaku() {
    let mut inventory = vec![Item::blank(); INVENTORY_SLOTS];
    inventory[0] = Item::new("NjTr4", 5000);
    let loadout = empty_loadout(
        vec![equipped(0, 1, "NjWep5", [Item::blank(), Item::blank()])],
        inventory,
    );
    let outcome = loadout.optimize(Request {
        objective: Objective::Door,
    });
    assert!(outcome.baseline.score > 0.0, "the door is unfinished");
    assert!(
        outcome.improved(),
        "the stealth charm lowers detection and lifts the door rate"
    );
    let door_pair = worn(&outcome.winner, 0);
    assert!(keys(&door_pair).contains(&"NjTr4"));
}

#[test]
fn the_untie_objective_scores_the_twin_holding_the_kunai() {
    let mut inventory = vec![Item::blank(); INVENTORY_SLOTS];
    inventory[0] = Item::new("NjTr4", 5000);
    let loadout = empty_loadout(
        vec![
            equipped(0, 2, "NjWep1", [Item::blank(), Item::blank()]),
            captive(1),
        ],
        inventory,
    );
    let outcome = loadout.optimize(Request {
        objective: Objective::Untie,
    });
    assert!(outcome.baseline.score > 0.0, "a captive waits on floor two");
    assert!(outcome.improved());
    let untie_pair = worn(&outcome.winner, 0);
    assert!(keys(&untie_pair).contains(&"NjTr4"));
}

#[test]
fn a_cancelled_run_stores_nothing_and_leaves_the_loadout_alone() {
    let mut inventory = vec![Item::blank(); INVENTORY_SLOTS];
    inventory[0] = Item::new("NjTr4", 5000);
    inventory[1] = Item::new("NjTr7", 5000);
    let loadout = empty_loadout(
        vec![equipped(0, 4, "NjGl2", [Item::blank(), Item::blank()])],
        inventory,
    );
    let mut reports = 0;
    let cancelled = loadout.optimize_with(
        Request {
            objective: Objective::Jade,
        },
        |_| {
            reports += 1;
            Control::Stop
        },
    );
    assert!(cancelled.is_none());
    assert_eq!(reports, 1, "the run stops at its first report");
    assert!(
        !loadout.changed(),
        "a cancelled run never edits the loadout"
    );

    let outcome = loadout.optimize(Request {
        objective: Objective::Jade,
    });
    assert!(outcome.improved());
    assert!(!loadout.changed(), "a finished run never edits the loadout");
}

#[test]
fn applying_the_winner_keeps_every_charm_the_account_owns() {
    let mut inventory = vec![Item::blank(); INVENTORY_SLOTS];
    inventory[0] = Item::new("NjTr4", 5000);
    inventory[1] = Item::new("NjTr19", 4000);
    let mut loadout = empty_loadout(
        vec![
            equipped(0, 4, "NjGl2", [Item::new("NjTr13", 3000), Item::blank()]),
            equipped(1, 4, "NjGl2", [Item::blank(), Item::blank()]),
        ],
        inventory,
    );
    let outcome = loadout.optimize(Request {
        objective: Objective::Jade,
    });
    let before = charms_owned(&loadout);
    assert!(loadout.apply(&outcome.winner));
    assert_eq!(
        charms_owned(&loadout),
        before,
        "no charm appears or vanishes"
    );
    for (twin, pair) in &outcome.winner.charms {
        assert_eq!(loadout.twin(*twin).unwrap().charms(), *pair);
    }
}

#[test]
fn the_search_never_wears_a_charm_the_account_owns_once() {
    let mut inventory = vec![Item::blank(); INVENTORY_SLOTS];
    inventory[0] = Item::new("NjTr4", 5000);
    inventory[1] = Item::new("NjTr7", 5000);
    let mut loadout = empty_loadout(
        vec![
            equipped(0, 4, "NjGl2", [Item::blank(), Item::blank()]),
            equipped(1, 4, "NjGl2", [Item::blank(), Item::blank()]),
        ],
        inventory,
    );
    // The Charmed perk makes a same-key pair legal, so only the copy count and
    // the Scroll of Power rule can keep the pair apart.
    loadout.charmed = true;
    let outcome = loadout.optimize(Request {
        objective: Objective::Jade,
    });
    let first = worn(&outcome.winner, 0);
    let second = worn(&outcome.winner, 1);
    for item in [&first[0], &first[1], &second[0], &second[1]] {
        if item.key == SCROLL_OF_POWER {
            assert!(
                [&first[0], &first[1], &second[0], &second[1]]
                    .iter()
                    .filter(|other| other.key == SCROLL_OF_POWER)
                    .count()
                    == 1,
                "Scroll of Power is account-unique"
            );
        }
        if item.key == "NjTr4" {
            let copies = [&first[0], &first[1], &second[0], &second[1]]
                .iter()
                .filter(|other| other.key == "NjTr4")
                .count();
            assert!(copies <= 1, "one owned copy cannot be worn twice");
        }
    }
}

#[test]
fn the_mastery_levels_clamp_at_zero() {
    let loadout = empty_loadout(Vec::new(), Vec::new());
    assert_eq!(loadout.mastery_levels(), vec![0, 1]);
    let rows = loadout.floor_mastery();
    assert_eq!(rows.len(), 2 * FLOOR_COUNT);
    // No twins, no doors and no captives: every row is a state, never a blank.
    for row in &rows {
        assert!(row.twins.is_empty());
        assert!(row.best_detection.is_none());
        assert!(!row.door_state.is_empty());
        // No attackers: a door that needs hits has no ETA, a finished one is 0.
        assert!(row.door_eta.is_none() || row.door_eta == Some(0.0));
        assert!(row.untie_eta.is_none());
    }
    // No doors saved means the first door is the unfinished one, and no twin
    // can hit it yet.
    assert!(loadout.active_door().unwrap().eta.is_none());
    assert!(loadout.next_captive().is_none());
}
