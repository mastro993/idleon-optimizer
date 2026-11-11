//! The jar port against the upstream JS.
//!
//! Every figure below comes from `crates/core/tests/jar-oracle.json`, which
//! `script/jar-oracle.mjs` regenerates by running the upstream
//! `js/stats/systems/w5/jar.js` on `fixtures/data.json` with the `ext` object
//! `jar-calc.html` builds, so a drift in either the port or the fixture fails
//! here.
//!
//! Two external bonuses belong to systems this app has not ported (the Cavern
//! Resource stamp and the farming Exotic 52). The oracle computes them with the
//! upstream helpers and the test feeds them in, so the formulas are compared end
//! to end while `Ext::read` stays honest about what it does not decode.

use idleon_core::Jar;
use idleon_core::jar;
use idleon_core::save::import;
use serde_json::Value;

fn fixture() -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/data.json");
    std::fs::read_to_string(path).expect("fixtures/data.json is committed")
}

fn oracle() -> Value {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/jar-oracle.json");
    serde_json::from_str(
        &std::fs::read_to_string(path).expect("script/jar-oracle.mjs writes the fixture"),
    )
    .expect("the oracle fixture is valid JSON")
}

fn fixture_jar() -> Jar {
    import(&fixture())
        .expect("the fixture imports")
        .jar
        .expect("the fixture carries the cavern")
}

/// The fixture's cavern with the two unported external bonuses the oracle
/// computed from the same save.
fn jar() -> (Jar, Value) {
    let oracle = oracle();
    let mut jar = fixture_jar();
    let ext = &oracle["ext"];
    jar.ext.stamp_cavern_res = number(&ext["stampCavernRes"]);
    jar.ext.exotic = number(&ext["exoticBonus52"]);
    (jar, oracle)
}

fn number(value: &Value) -> f64 {
    value.as_f64().expect("the oracle writes numbers")
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
    let jar = fixture_jar();
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
    let jar = fixture_jar();
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
    assert_eq!(jar.dark_rupies(), 106_236_172_118_739.0);
}

#[test]
fn the_save_derived_bonuses_match_upstream() {
    let jar = fixture_jar();
    let oracle = oracle();
    let ext = &oracle["ext"];
    close(jar.ext.legend_pts, number(&ext["legendPts29"]));
    close(jar.ext.rupie_slugs, number(&ext["ola355"]));
    close(jar.ext.monument, number(&ext["monument2_1"]));
    close(jar.ext.bell, number(&ext["bell4"]));
    close(jar.ext.cosmo, number(&ext["cosmo0_3"]));
    close(jar.ext.cglunko, number(&ext["cglunko14"]));
    close(jar.ext.bolaia, number(&ext["bolaia10"]));
    close(jar.ext.palette, number(&ext["paletteBonus2"]));
    close(jar.ext.lore_episodes, number(&ext["spelunk6Len"]));
    close(jar.ext.super_bit, number(&ext["superBit32"]));
    close(jar.ext.fountain, number(&ext["fountBonTOT2_15"]));
    // The two systems this app has not ported stay at the upstream default.
    assert_eq!(jar.ext.stamp_cavern_res, 0.0);
    assert_eq!(jar.ext.exotic, 0.0);
}

#[test]
fn production_matches_upstream() {
    let (jar, oracle) = jar();
    assert_eq!(jar.production_slots(), 3);
    close(
        jar.production_per_hour(),
        number(&oracle["productionPerHour"]),
    );

    let terms = jar.production_terms();
    let expected = oracle["productionBreakdown"]
        .as_array()
        .expect("the oracle lists the production terms");
    // The oracle's first row is the base rate, which is not a percentage term.
    assert_eq!(terms.len(), expected.len() - 1);
    for (term, row) in terms.iter().zip(expected.iter().skip(1)) {
        let (label, percent) = (row[0].as_str().expect("a label"), number(&row[1]));
        // Light Speed's upstream label carries its own value.
        assert!(
            label.starts_with(term.label)
                || (term.label == "Light Speed" && label.starts_with("Light Speed")),
            "expected a term named {label}, got {}",
            term.label
        );
        close(term.percent, percent);
    }
}

#[test]
fn the_rupies_value_matches_upstream() {
    let (jar, oracle) = jar();
    close(jar.rupie_value(), number(&oracle["rupieValue"]));
    for (tier, row) in oracle["rupieValueByTier"]
        .as_array()
        .expect("the oracle lists the tiers")
        .iter()
        .enumerate()
    {
        close(jar.rupie_value_at_tier(tier as u32), number(row));
    }
}

#[test]
fn the_rupies_tree_multipliers_match_the_upstream_breakdown() {
    let (jar, oracle) = jar();
    let tree = jar.rupie_value_tree();
    close(tree.value, number(&oracle["rupieValue"]));
    // A multiplicative breakdown: the children multiply back to the root.
    let product: f64 = tree.children.iter().map(|child| child.value).product();
    assert!((product - tree.value).abs() <= tree.value * 1e-12);

    let expected = oracle["rupieValueBreakdown"]
        .as_array()
        .expect("the oracle lists the breakdown");
    assert_eq!(tree.children.len(), expected.len());
    for (child, row) in tree.children.iter().zip(expected) {
        let (label, multiplier) = (row[0].as_str().expect("a label"), number(&row[1]));
        assert_eq!(child.label, label);
        close(child.value, multiplier);
    }
}

#[test]
fn the_extra_rupies_match_upstream() {
    let (jar, oracle) = jar();
    close(
        jar.extra_rupie_chance(),
        number(&oracle["extraRupieChance"]),
    );
    close(
        jar.avg_rupies_per_break(),
        number(&oracle["avgRupiesPerBreak"]),
    );
    let terms: Vec<f64> = jar
        .extra_rupie_terms()
        .iter()
        .map(|term| term.percent)
        .collect();
    // The fixture owns Mach IV, so it is the only row the percentages need.
    assert_eq!(terms, vec![jar.ext.bell, jar.ext.cosmo, 50.0]);
}

#[test]
fn the_rupies_per_break_match_upstream() {
    let (jar, oracle) = jar();
    let jars = oracle["rupieJars"]
        .as_array()
        .expect("the oracle lists the rupie jars");
    assert_eq!(jar.rupie_jars().len(), jars.len());
    for ((jar_type, name, colors), row) in jar.rupie_jars().into_iter().zip(jars) {
        assert_eq!(Some(name), row["name"].as_str());
        let expected_colors: Vec<usize> = row["colors"]
            .as_array()
            .expect("the oracle lists the colors")
            .iter()
            .map(|color| color.as_u64().expect("a color index") as usize)
            .collect();
        assert_eq!(colors, expected_colors);
        for (tier, expected) in row["perBreak"]
            .as_array()
            .expect("the oracle lists the tiers")
            .iter()
            .enumerate()
        {
            close(jar.rupies_per_break(tier as u32), number(expected));
        }
        for color in &colors {
            for (tier, expected) in row["perColor"][colors.iter().position(|c| c == color).unwrap()]
                ["perBreak"]
                .as_array()
                .expect("the oracle lists the color tiers")
                .iter()
                .enumerate()
            {
                let share = jar.rupies_per_break(tier as u32) / colors.len() as f64;
                close(share, number(expected));
            }
        }
        // Every rupie jar is a rupie type, never a chance or gilded one.
        assert_eq!(jar::TYPE_CATEGORIES[jar_type], jar::TypeCategory::Rupie);
    }
}

#[test]
fn the_break_chances_match_upstream() {
    let (jar, oracle) = jar();
    close(
        jar.opal_chance(),
        chance(&oracle, "opal")["base"].as_f64().unwrap(),
    );
    close(
        jar.collectible_chance(),
        chance(&oracle, "collectible")["base"].as_f64().unwrap(),
    );
    close(
        jar.enchant_chance(),
        chance(&oracle, "enchant")["base"].as_f64().unwrap(),
    );
    close(jar.enchant_multi(), number(&oracle["enchantMulti"]));
    close(
        f64::from(jar.enchant_levels()),
        number(&oracle["totalEnchantLevels"]),
    );
    close(
        jar.targeted_enchant_chance(),
        number(&oracle["targetedEnchantProb"]),
    );
}

#[test]
fn the_chance_tiers_match_upstream() {
    let (jar, oracle) = jar();
    for name in ["opal", "collectible", "enchant"] {
        let event = chance(&oracle, name);
        let base = event["base"].as_f64().expect("a base chance");
        let Some(jar_type) = chance_jar_type(name) else {
            panic!("{name} names a chance jar");
        };
        for (tier, expected) in event["tiers"]
            .as_array()
            .expect("the oracle lists the tiers")
            .iter()
            .enumerate()
        {
            close(
                jar::Jar::chance_at_tier(base, tier as u32),
                number(expected),
            );
        }
        assert_eq!(
            jar::Jar::tier_for_certainty(base),
            event["tierFor100"].as_u64().map(|tier| tier as u32)
        );
        close(
            jar::Jar::expected_breaks(base),
            number(&event["expectedBreaks"]),
        );
        close(
            jar.expected_hours(jar_type, base),
            number(&event["expectedHours"]),
        );
    }
    let events = jar.chance_events();
    for (event, name) in events.iter().zip(["opal", "collectible", "enchant"]) {
        assert_eq!(Some(event.jar_type), chance_jar_type(name));
        close(
            event.chance,
            chance(&oracle, name)["base"].as_f64().unwrap(),
        );
    }
}

#[test]
fn the_production_requirements_match_upstream() {
    let (jar, oracle) = jar();
    for (jar_type, expected) in oracle["productionRequirement"]
        .as_array()
        .expect("the oracle lists the requirements")
        .iter()
        .enumerate()
    {
        close(jar.production_requirement(jar_type), number(expected));
    }
    for (jar_type, tiers) in oracle["timeToTier"]
        .as_array()
        .expect("the oracle lists the tiers")
        .iter()
        .enumerate()
    {
        for (tier, expected) in tiers
            .as_array()
            .expect("the oracle lists each jar's tiers")
            .iter()
            .enumerate()
        {
            close(jar.time_to_tier(jar_type, tier as u32), number(expected));
        }
    }
}

#[test]
fn the_slot_details_match_upstream() {
    let jar = fixture_jar();
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
fn the_jar_tables_line_up_with_the_save_rows() {
    assert_eq!(jar::TYPE_NAMES.len(), jar::TYPE_CATEGORIES.len());
    assert_eq!(jar::RUPIE_NAMES.len(), jar::RUPIE_COLORS.len());
    assert_eq!(jar::TYPE_CATEGORIES[7], jar::TypeCategory::Gilded);
    assert_eq!(jar::TYPE_CATEGORIES[1].label(), "chance");
}

/// The oracle's row for one chance jar.
#[track_caller]
fn chance<'a>(oracle: &'a Value, name: &str) -> &'a Value {
    oracle["chances"]
        .as_array()
        .expect("the oracle lists the chances")
        .iter()
        .find(|chance| chance["name"] == name)
        .unwrap_or_else(|| panic!("the oracle lists {name}"))
}

/// The jar type a chance event rolls on.
fn chance_jar_type(name: &str) -> Option<usize> {
    match name {
        "opal" => Some(1),
        "collectible" => Some(2),
        "enchant" => Some(4),
        _ => None,
    }
}
