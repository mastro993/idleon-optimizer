//! The jar port against the upstream JS.
//!
//! Every figure below was produced by running the upstream
//! `js/stats/systems/w5/jar.js` on `fixtures/data.json` (its `Holes` rows plus
//! the `Lv0_<n>` character levels it reads through `lv0AllData`), so a drift in
//! either the port or the fixture fails here.

use idleon_core::Jar;
use idleon_core::jar;
use idleon_core::save::import;

fn fixture() -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/data.json");
    std::fs::read_to_string(path).expect("fixtures/data.json is committed")
}

fn jar() -> Jar {
    import(&fixture())
        .expect("the fixture imports")
        .jar
        .expect("the fixture carries the cavern")
}

/// Upstream figures are doubles; compare to the last few bits they share.
#[track_caller]
fn close(actual: f64, expected: f64) {
    let tolerance = expected.abs().max(1.0) * 1e-9;
    assert!(
        (actual - expected).abs() <= tolerance,
        "expected {expected}, got {actual}"
    );
}

#[test]
fn the_jar_summary_matches_upstream() {
    let jar = jar();
    assert_eq!(jar.types_purchased(), 9);
    assert_eq!(jar.types_available(), 10);
    assert_eq!(jar.missing_types().count(), 0);
    close(jar.next_type_cost(), 2_138_657_015.530_170_7);
    // Every type is bought, so there is no next jar to pay for.
    assert!(!jar.can_afford_next_type());
    close(jar.opals(), 21.0);
    assert_eq!(jar.collectibles_found(), 40);
    assert_eq!(jar.enchant_levels(), 320);
    assert!(jar.page_two_unlocked());
    assert_eq!(jar.collectible_slots(), 40);
    close(jar.doubler_bonus(), 0.0);
    assert!(jar.break_all_unlocked());
    assert_eq!(jar.targeted_enchant(), Some(30));
    close(jar.targeted_enchant_chance(), 5.0 / 43.0);
}

#[test]
fn the_rupie_balances_match_upstream() {
    let jar = jar();
    let expected = [
        544_159_529_207.887,
        767_333_979_180.378,
        8_715_286_155_646.38,
        9.280_918_748_761_12e18,
        1.472_384_684_014_49e19,
        2.572_064_456_133_22e20,
        881_567_293_442.686,
        8_686_299_164_677.48,
        8_729_448_059_584.1,
        3_678_985_229_966.1,
        123_378_032_339_347.0,
        106_236_172_118_739.0,
    ];
    for (index, value) in expected.into_iter().enumerate() {
        close(jar.rupies(index), value);
    }
    // White and dark live in their own storage, not in the standard row.
    assert_eq!(jar.white_rupies(), 123_378_032_339_347.0);
    assert_eq!(jar.rupies(11), 106_236_172_118_739.0);
}

#[test]
fn production_matches_upstream() {
    let jar = jar();
    assert_eq!(jar.production_slots(), 3);
    close(jar.production_per_hour(), 705_553.486_375_814_5);

    let terms: Vec<(&str, f64)> = jar
        .production_terms()
        .into_iter()
        .map(|term| (term.label, term.percent))
        .collect();
    let expected = [
        ("Sapphire Droplet", 90.0),
        ("Frost Spirestone", 108.0),
        ("Earthbound Geode", 135.0),
        ("Jar Production Speed Measurement", 380.521_186_206_896_6),
        ("Light Speed", 140.912_078_101_667_87),
    ];
    assert_eq!(terms.len(), expected.len());
    for ((label, percent), (expected_label, expected_percent)) in terms.into_iter().zip(expected) {
        assert_eq!(label, expected_label);
        close(percent, expected_percent);
    }
}

#[test]
fn the_slot_details_match_upstream() {
    let jar = jar();
    let expected = [
        (3, 2_839.307_730_234_88),
        (3, 4_349.701_644_610_34),
        (3, 58.602_453_498_965_6),
    ];
    for (slot, (jar_type, progress)) in expected.into_iter().enumerate() {
        assert_eq!(jar.slot_type(slot), jar_type);
        close(jar.slot_progress(slot), progress);
        // The fixture's slots all hold a Decent Rupie jar.
        assert_eq!(jar::TYPE_NAMES[jar.slot_type(slot)], "Decent Rupie");
    }
}

#[test]
fn the_production_requirements_match_upstream() {
    let jar = jar();
    let expected = [
        598.577_362_230_794,
        1_795.732_086_692_382_2,
        3_053.296_299_065_859,
        4_435.042_426_851_435,
        5_159.067_004_455_555,
        6_620.933_256_314_794,
        8_138.633_848_974_016,
        8_994.587_183_411_293,
        10_998.978_174_575_204,
        11_867.330_973_852_62,
    ];
    for (jar_type, value) in expected.into_iter().enumerate() {
        close(jar.production_requirement(jar_type), value);
    }
    let tiers = [
        0.006_285_905_338_846_417,
        0.062_859_053_388_464_18,
        0.628_590_533_884_641_7,
        6.285_905_338_846_417,
        62.859_053_388_464_17,
        628.590_533_884_641_7,
    ];
    for (tier, value) in tiers.into_iter().enumerate() {
        close(jar.time_to_tier(3, tier as u32), value);
    }
}

#[test]
fn the_jar_tables_line_up_with_the_save_rows() {
    assert_eq!(jar::TYPE_NAMES.len(), jar::TYPE_CATEGORIES.len());
    assert_eq!(jar::RUPIE_NAMES.len(), jar::RUPIE_COLORS.len());
    assert_eq!(jar::TYPE_CATEGORIES[7], jar::TypeCategory::Gilded);
    assert_eq!(jar::TYPE_CATEGORIES[1].label(), "chance");
}
