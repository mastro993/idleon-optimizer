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
