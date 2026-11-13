//! The Sushi Station formulas against the upstream calculator's own output.
//!
//! `script/sushi-oracle.mjs` runs `js/stats/systems/w7/sushi.js` on the
//! committed fixture save and writes `tests/fixtures/upstream.json`; every
//! assertion here compares one exported function with that run.

use idleon_save::Save;
use idleon_sushi::data::game;
use idleon_sushi::math::{self, External, Station};
use idleon_sushi::{Inputs, inputs};
use serde_json::Value;

fn fixture_text() -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/data.json");
    std::fs::read_to_string(path).expect("fixtures/data.json is committed")
}

fn oracle() -> Value {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/upstream.json");
    let text = std::fs::read_to_string(path).expect("the upstream fixture is committed");
    serde_json::from_str(&text).expect("the upstream fixture is JSON")
}

fn decoded() -> Inputs {
    let imported = idleon_import::import(&fixture_text()).expect("the fixture imports");
    let parsed: Value = serde_json::from_str(&fixture_text()).expect("JSON");
    let save = Save::new(parsed);
    inputs(&save, &imported.simulation).0
}

fn number(value: &Value) -> f64 {
    value.as_f64().expect("a number")
}

fn close(actual: f64, expected: f64, what: &str) {
    let tolerance = expected.abs().max(1.0) * 1e-9;
    assert!(
        (actual - expected).abs() <= tolerance,
        "{what}: {actual} != {expected}"
    );
}

#[test]
fn the_fixture_decodes_its_station() {
    let inputs = decoded();
    let oracle = oracle();
    assert_eq!(
        inputs.research_level,
        number(&oracle["researchLevel"]) as u32
    );
    assert_eq!(inputs.station.unique, number(&oracle["uniqueSushi"]) as u32);
    let ext = &oracle["ext"];
    let external = &inputs.external;
    close(external.arcade67, number(&ext["arcade67"]), "arcade 67");
    close(external.grid188, number(&ext["gridBonus188"]), "grid 188");
    close(external.grid189, number(&ext["gridBonus189"]), "grid 189");
    close(
        external.minehead11,
        number(&ext["mineheadBonus11"]),
        "minehead 11",
    );
    close(external.atom14, number(&ext["atom14"]), "atom 14");
    close(external.sailing39, number(&ext["sailing39"]), "sailing 39");
    assert_eq!(
        external.bundle_v,
        ext["hasBundleV"].as_bool().unwrap_or(false)
    );
    close(
        external.super_bit67,
        number(&ext["gamingSuperBit67"]),
        "super bit 67",
    );
    close(external.button2, number(&ext["buttonBonus2"]), "button 2");
    close(
        external.event_shop66,
        number(&ext["eventShop66"]),
        "event shop 66",
    );
    close(external.jelly18, number(&ext["jelly18"]), "jelly 18");
    assert_eq!(inputs.station.doublers, Vec::<usize>::new());
}

#[test]
fn the_level_and_cost_curves_match() {
    let inputs = decoded();
    let oracle = oracle();
    let station = &inputs.station;
    let reqs = oracle["upgLvReq"].as_array().expect("levels");
    for (slot, expected) in reqs.iter().enumerate() {
        close(
            math::upg_lv_req(slot),
            number(expected),
            &format!("slot {slot} requirement"),
        );
    }
    let qty = oracle["upgradeQTY"].as_array().expect("quantities");
    for (upg, expected) in qty.iter().enumerate() {
        close(
            station.qty(upg),
            number(expected),
            &format!("upgrade {upg} quantity"),
        );
    }
    let costs = oracle["upgCost"].as_array().expect("costs");
    for (slot, expected) in costs.iter().enumerate() {
        close(
            station.upgrade_cost(slot, &station.levels, &inputs.external),
            number(expected),
            &format!("slot {slot} cost"),
        );
    }
}

#[test]
fn the_tier_curves_match() {
    let data = game();
    let oracle = oracle();
    for (tier, expected) in oracle["currencyPerTier"]
        .as_array()
        .expect("tiers")
        .iter()
        .enumerate()
    {
        close(
            math::currency_per_tier(data, tier),
            number(expected),
            &format!("tier {tier} bucks"),
        );
    }
    for (tier, expected) in oracle["fuelCostPerTier"]
        .as_array()
        .expect("tiers")
        .iter()
        .enumerate()
    {
        close(
            math::fuel_cost_per_tier(tier),
            number(expected),
            &format!("tier {tier} fuel"),
        );
    }
}

#[test]
fn the_knowledge_totals_match() {
    let inputs = decoded();
    let oracle = oracle();
    let station = &inputs.station;
    for (tier, expected) in oracle["knowledgeBonusSpecific"]
        .as_array()
        .expect("tiers")
        .iter()
        .enumerate()
    {
        close(
            station.knowledge_bonus(tier),
            number(expected),
            &format!("tier {tier} knowledge"),
        );
    }
    let totals = station.knowledge_totals();
    for (cat, expected) in oracle["knowledgeTotals"]
        .as_array()
        .expect("totals")
        .iter()
        .enumerate()
    {
        close(totals[cat], number(expected), &format!("category {cat}"));
    }
    close(
        math::fireplace_effect_base(&totals, station.sparks()),
        number(&oracle["fireplaceEffectBase"]),
        "fireplace base",
    );
    close(
        math::slot_effect_base(&totals),
        number(&oracle["slotEffectBase"]),
        "slot effect base",
    );
    for (tier, expected) in oracle["knowledgeBonusNext"]
        .as_array()
        .expect("tiers")
        .iter()
        .enumerate()
    {
        close(
            station.knowledge_bonus_next(tier),
            number(expected),
            &format!("tier {tier} next knowledge"),
        );
    }
    for (tier, expected) in oracle["perfectoOdds"]
        .as_array()
        .expect("tiers")
        .iter()
        .enumerate()
    {
        close(
            station.perfecto_odds(tier),
            number(expected),
            &format!("tier {tier} perfecto odds"),
        );
    }
    for (tier, expected) in oracle["knowledgeXPReq"]
        .as_array()
        .expect("levels")
        .iter()
        .enumerate()
    {
        close(
            math::knowledge_xp_req(tier as f64),
            number(expected),
            &format!("knowledge level {tier} EXP"),
        );
    }
}

#[test]
fn the_rate_formulas_match() {
    let inputs = decoded();
    let oracle = oracle();
    let station = &inputs.station;
    let ext = &inputs.external;
    let levels = &station.levels;
    close(
        station.orange_fire_sum(),
        number(&oracle["orangeFireSum"]),
        "orange fire sum",
    );
    close(
        station.fuel_gen(levels, ext),
        number(&oracle["fuelGen"]),
        "fuel gen",
    );
    close(
        station.fuel_capacity(levels, ext),
        number(&oracle["fuelCap"]),
        "fuel capacity",
    );
    close(
        station.currency_multi(levels, ext),
        number(&oracle["currencyMulti"]),
        "currency multiplier",
    );
    close(
        station.bucks_per_hour(levels, ext),
        number(&oracle["bucksPerHr"]),
        "bucks per hour",
    );
    let multi = station.currency_multi(levels, ext);
    for (slot, expected) in oracle["bucksPerSlot"]
        .as_array()
        .expect("slots")
        .iter()
        .enumerate()
    {
        close(
            station.currency_per_slot(slot, multi),
            number(expected),
            &format!("slot {slot} bucks"),
        );
    }
    close(
        math::compute_overtuned_multi(station.spa()),
        number(&oracle["overtunedMulti"]),
        "overtuned",
    );
    let combo = &oracle["combo"];
    close(
        math::combo_multiplier(station.combo_score()),
        number(&combo["multiplier"]),
        "combo multiplier",
    );
    assert_eq!(
        math::combo_meter_unlocked(station.qty(39)),
        combo["unlocked"].as_bool().unwrap_or(false)
    );
    close(
        station.slots_owned(),
        number(&oracle["slotsOwned"]),
        "slots owned",
    );
    assert_eq!(
        station.active_slots(),
        number(&oracle["activeSlots"]) as usize
    );
    close(
        station.max_cook_tier(),
        number(&oracle["maxCookTier"]),
        "max cook tier",
    );
    close(
        station.bonus_cook_tier_pct(),
        number(&oracle["bonusCookTierPct"]),
        "bonus cook tier",
    );
    close(
        station.free_shaker_chance(ext),
        number(&oracle["freeShakerChance"]),
        "free shaker chance",
    );
    close(
        station.saffron_hrs(),
        number(&oracle["saffronHrs"]),
        "saffron hours",
    );
    close(
        station.knowledge_xp_base(),
        number(&oracle["knowledgeXPBase"]),
        "knowledge EXP base",
    );
    for (slot, expected) in oracle["knowledgeXPMulti"]
        .as_array()
        .expect("slots")
        .iter()
        .enumerate()
    {
        close(
            station.knowledge_xp_multi(slot),
            number(expected),
            &format!("slot {slot} knowledge EXP"),
        );
    }
    for (fire_type, expected) in oracle["fireplaceEffects"]
        .as_array()
        .expect("types")
        .iter()
        .enumerate()
    {
        close(
            math::fireplace_effect_by_type(fire_type, station.fireplace_base()),
            number(expected),
            &format!("fireplace type {fire_type}"),
        );
    }
}

#[test]
fn the_doubled_upgrades_match() {
    let mut inputs = decoded();
    let oracle = oracle();
    let doubled = &oracle["doubled"];
    let rows: Vec<usize> = doubled["rows"]
        .as_array()
        .expect("rows")
        .iter()
        .map(|row| number(row) as usize)
        .collect();
    inputs.station.doublers = rows.clone();
    let station = &inputs.station;
    let ext = &inputs.external;
    let levels = &station.levels;
    for (upg, expected) in doubled["upgradeQTY"]
        .as_array()
        .expect("quantities")
        .iter()
        .enumerate()
    {
        close(
            station.qty(upg),
            number(expected),
            &format!("upgrade {upg} doubled quantity"),
        );
    }
    for (slot, expected) in doubled["upgCost"]
        .as_array()
        .expect("costs")
        .iter()
        .enumerate()
    {
        close(
            station.upgrade_cost(slot, levels, ext),
            number(expected),
            &format!("slot {slot} doubled cost"),
        );
    }
    close(
        station.fuel_gen(levels, ext),
        number(&doubled["fuelGen"]),
        "doubled fuel gen",
    );
    close(
        station.fuel_capacity(levels, ext),
        number(&doubled["fuelCap"]),
        "doubled fuel capacity",
    );
    close(
        station.free_shaker_chance(ext),
        number(&doubled["freeShakerChance"]),
        "doubled free shaker chance",
    );
    close(
        station.saffron_hrs(),
        number(&doubled["saffronHrs"]),
        "doubled saffron hours",
    );
    close(
        station.knowledge_xp_base(),
        number(&doubled["knowledgeXPBase"]),
        "doubled knowledge EXP base",
    );
    // The slot and cook tiers ignore the doublers, as upstream does.
    close(
        station.slots_owned(),
        number(&doubled["slotsOwned"]),
        "doubled slots owned",
    );
    close(
        station.max_cook_tier(),
        number(&doubled["maxCookTier"]),
        "doubled max cook tier",
    );
}

#[test]
fn the_ring_of_glory_table_follows_the_unique_tiers() {
    let oracle = oracle();
    let unique = number(&oracle["uniqueSushi"]) as u32;
    let station = Station {
        unique,
        ..Station::default()
    };
    let data = game();
    let rog = math::build_rog_array(data, unique);
    assert_eq!(rog.len(), data.rog_qty.len());
    assert!(rog[0] > 0.0, "the first tier is unlocked");
    assert_eq!(rog[unique as usize], 0.0, "the next tier is locked");
    assert_eq!(station.rog(53), rog[53]);
    assert_eq!(station.rog(unique as usize), 0.0);
    // A save without the tier unlocks nothing.
    assert_eq!(
        math::build_rog_array(data, 0),
        vec![0.0; data.rog_qty.len()]
    );
}

#[test]
fn a_station_without_rows_reads_as_empty() {
    let save = Save::new(serde_json::json!({}));
    let (inputs, warnings) = inputs(&save, &idleon_research::model::Simulation::default());
    assert_eq!(inputs.station, Station::default());
    assert_eq!(inputs.external, External::default());
    assert!(warnings.iter().any(|warning| warning.contains("Sushi")));
    assert_eq!(inputs.station.bucks_per_hour(&[], &inputs.external), 0.0);
}
