//! The damage layers, ported from `js/stats/defs/damage.js`.
//!
//! The descriptor multiplies three layers: `DDL0` (base damage after its two
//! softcaps), `DDL1` (the percentage multiplier after its three) and `DDL2` (the
//! multiplicative bonus groups after its six-stage chain). The minimum damage is
//! the maximum times the mastery ratio.
//!
//! Every group whose provider this build does not carry is recorded by name, so
//! a caller can show the layers and the reason they are a lower bound instead of
//! presenting a number nobody can justify.

use crate::systems::Ctx;
use crate::{Stat, with_ctx};
use idleon_common::curve;
use idleon_research::model::Simulation;
use idleon_save::Save;

/// The three layers and the damage they combine into.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Layers {
    pub base: f64,
    pub base_pre_softcap: f64,
    pub percent: f64,
    pub percent_pre_softcap: f64,
    pub multiplier: f64,
    pub multiplier_pre_softcap: f64,
    pub mastery: f64,
    /// Talent 113's penalty, as the mastery breakdown reads it.
    pub mastery_penalty: f64,
    pub max: f64,
    pub min: f64,
    /// The terms this build could not justify.
    pub missing: Vec<String>,
}

impl Layers {
    pub fn is_complete(&self) -> bool {
        self.missing.is_empty()
    }
}

/// `isDungeonMap` hides the summary: a dungeon's run damage is not in the save.
pub fn is_dungeon_map(map: usize) -> bool {
    idleon_stats_data::data()
        .maps
        .get(map)
        .is_some_and(|row| row.dungeon)
}

/// `DamageDealed('Max')`, `('Min')` and the mastery ratio for one character.
pub fn evaluate(
    save: &Save,
    simulation: &Simulation,
    character: usize,
    map: usize,
) -> Option<Layers> {
    if is_dungeon_map(map) {
        return None;
    }
    with_ctx(save, simulation, character, layers)
}

fn layers(ctx: &mut Ctx<'_, '_>) -> Layers {
    // Mastery: min(0.8, 0.35 - talent113 / 100 + (bubble + card21 + talent123 + etc21) / 100).
    let talent113 = ctx.talent(113);
    let mastery = (0.35 - talent113 / 100.0
        + (ctx.bubble("Mastery")
            + ctx.card_bonus(21)
            + ctx.talent(123)
            + ctx.decoder.etc(21, ctx.character))
            / 100.0)
        .min(0.8);

    // === DDL0 ===
    let stat = crate::stats::total_stat_here(ctx, Stat::Str);
    let stat = stat
        * (1.0 + (ctx.talent(95) + ctx.talent(455) + ctx.talent(20) + class_bubbles(ctx)) / 100.0);

    let (equipment_wp, tool_wp) = weapon_power(ctx);
    let gallery_wp = ctx.decoder.gallery_base_stat("Weapon_Power", ctx.character);
    let obol_wp = obol_weapon_power(ctx);
    let equipment_scaled =
        (equipment_wp + tool_wp + gallery_wp + obol_wp) * (1.0 + class_weapon_bubbles(ctx) / 100.0);
    let mut wp_raw = 5.0
        + ctx.box_reward("12a")
        + equipment_scaled
        + ctx.vial("WeaponPOW")
        + family(ctx, 16)
        + ctx.star_sign_bonus("WepPow")
        + ctx.decoder.arcade(17);
    for (talent, skill) in [(530, 12), (140, 10), (170, 15), (320, 13), (500, 14)] {
        wp_raw += ctx.talent(talent) * (ctx.level(skill) / 10.0).floor();
    }
    wp_raw += ctx.talent(365) * curve::log(pet_power(ctx).max(1.0));
    let wp = wp_raw
        * (1.0
            + (ctx.talent(97) + ctx.talent(277) + ctx.talent(457) + ctx.decoder.cosmo_bonus(2, 4))
                / 100.0)
        + ctx.talent(5);

    let golden = ctx.gold_food_bonus("BaseDamage");
    let mut base = (wp / 3.0).powi(2) + stat + golden + (150.0f64).min(2.0 * wp_raw + stat);
    base += ctx.decoder.arcade(0)
        + ctx.decoder.vault(0)
        + ctx.decoder.vault(20) * vault_killz(ctx, 5)
        + ctx.statue_bonus(0)
        + ctx.box_reward("basedmg")
        + ctx.decoder.etc(16, ctx.character)
        + ctx.card_bonus(4);
    base += ctx.stamp("BaseDmg") + ctx.owl(1) + ctx.sigil(4) + ctx.decoder.card_level("fallEvent2");
    // The HP/MP/speed-scaled bubbles read the max-hp, max-mp and movement-speed
    // descriptors, which this build does not port.
    for (name, source) in [
        ("HP-scaled base damage", "js/stats/defs/max-hp.js"),
        ("MP-scaled base damage", "js/stats/defs/max-mp.js"),
        (
            "Speed-scaled base damage",
            "js/stats/defs/movement-speed.js",
        ),
    ] {
        ctx.miss(name, source);
    }
    let base_pre = base;

    // === DDL1 ===
    let add = stat.powf(0.7)
        + ctx.decoder.vault(27) * vault_killz(ctx, 6)
        + ctx.decoder.vault(15) * ctx.option(338)
        + 0.4 * ctx.decoder.vault(10)
        + ctx.bribe(30)
        + ctx.bribe(20)
        + ctx.stamp("PctDmg")
        + ctx.statue_bonus(22)
        + talent113
        + ctx.owl(2)
        + ctx.decoder.roo(4)
        + ctx.decoder.vault(80);
    // `log(HP) * talent 86`, `log(MP) * talent 446`, Roo, Bubba, Divinity minor,
    // combat talents 31/110/125/305/463/470/485/649/656/658 and the Golden Food
    // damage multiplier need descriptors this build does not port yet.
    for (name, source) in [
        ("HP-scaled damage talent", "js/stats/defs/max-hp.js"),
        ("MP-scaled damage talent", "js/stats/defs/max-mp.js"),
        ("Roo damage", "js/stats/systems/w7/sushi.js"),
        ("Bubba damage", "js/stats/systems/w7/bubba.js"),
        ("Talent chain", "js/stats/systems/common/talent.js"),
    ] {
        ctx.miss(name, source);
    }
    let percent_pre = (1.0 + add / 100.0)
        * (1.0 + ctx.talent(284) * (ctx.level(2).min(100.0) / 10.0) / 100.0)
        * (1.0 + ctx.decoder.win_bonus(0) / 100.0);

    // === DDL2 ===
    let mut multiplier = (1.0 + ctx.talent_best(508) * curve::log(ctx.option(152)) / 100.0)
        * (1.0 + ctx.talent_best(208) * curve::log(ctx.option(329)) / 100.0);
    multiplier = multiplier.max(1.0);
    multiplier *= 1.0 + ctx.vial("7dmg") / 100.0;
    multiplier *= 1.0 + ctx.decoder.palette(34) / 100.0;
    multiplier *= 1.0 + dream(ctx, 6) / 10.0;
    multiplier *= 1.0 + ctx.decoder.pristine(0) / 100.0;
    multiplier *= 1.0 + (ctx.decoder.friend(0) + ctx.star_sign_bonus("PctDmg")) / 100.0;
    multiplier *= 1.0
        + (ctx.decoder.grimoire_22()
            + idleon_account::sets::set_bonus(ctx.decoder, "LUSTRE_SET", ctx.character)
            + ctx.box_reward("12c"))
            / 100.0;
    multiplier *= 1.0
        + (ctx.bubble("pctDmg1")
            + ctx.bubble("pctDmg2")
            + ctx.bubble("pctDmg3")
            + ctx.decoder.arcade(46)
            + ctx.option(419))
            / 100.0;
    multiplier *=
        1.0 + (ctx.talent(6) + ctx.decoder.etc(45, ctx.character) + ctx.option(435)) / 100.0;
    multiplier *=
        1.0 + (ctx.decoder.companion(10) + ctx.decoder.companion(156) + ctx.card_bonus(42)) / 100.0;
    multiplier *= 1.0
        + ((20.0 * ctx.decoder.pet_arena(2))
            + (40.0 * ctx.decoder.pet_arena(15))
            + ctx.decoder.meal("TotDmg"))
            / 100.0;
    for (name, source) in [
        ("Divinity minor damage", "js/stats/systems/w5/divinity.js"),
        ("Buffs", "js/stats/systems/common/buff.js"),
        ("Mainframe damage", "js/stats/systems/w4/lab.js"),
        ("Sailing artifact damage", "js/stats/systems/w5/sailing.js"),
        ("Shimmer damage", "js/stats/systems/w3/equinox.js"),
        ("Minehead damage", "js/stats/systems/w7/minehead.js"),
        ("Scurge divisor", "js/stats/systems/w2/dungeon.js"),
    ] {
        ctx.miss(name, source);
    }
    let multiplier_pre = multiplier;

    let base = softcap_additive(base_pre);
    let percent = softcap_multiplicative(percent_pre, &[(100.0, 0.86), (2e6, 0.5), (1e8, 0.3)]);
    let multiplier = softcap_multiplicative(
        multiplier_pre,
        &[
            (100.0, 0.86),
            (2e7, 0.8),
            (5e8, 0.6),
            (2e9, 0.45),
            (15e9, 0.36),
            (6e10, 0.28),
        ],
    );
    let max = base * percent * multiplier;
    Layers {
        base,
        base_pre_softcap: base_pre,
        percent,
        percent_pre_softcap: percent_pre,
        multiplier,
        multiplier_pre_softcap: multiplier_pre,
        mastery,
        mastery_penalty: talent113 / 100.0,
        max,
        min: max * mastery,
        missing: ctx.missing.clone(),
    }
}

/// The `W6`/`A6`/`M6` bubbles the active class keeps.
fn class_bubbles(ctx: &mut Ctx<'_, '_>) -> f64 {
    class_keys(ctx, ["W6", "A6", "M6"])
}

/// The `W1`/`A1`/`M1` weapon-power bubbles the active class keeps.
fn class_weapon_bubbles(ctx: &mut Ctx<'_, '_>) -> f64 {
    class_keys(ctx, ["W1", "A1", "M1"])
}

fn class_keys(ctx: &mut Ctx<'_, '_>, keys: [&str; 3]) -> f64 {
    let class = ctx.class();
    let mut total = 0.0;
    for (index, key) in keys.iter().enumerate() {
        let keep = match index {
            0 => class < 18,
            1 => (18..30).contains(&class),
            _ => class >= 30,
        };
        if keep {
            total += ctx.bubble(key);
        }
    }
    total
}

/// `(equipment, tools)` weapon power: each slot's base plus its affix.
fn weapon_power(ctx: &mut Ctx<'_, '_>) -> (f64, f64) {
    let mut values = [0.0f64; 2];
    let order = idleon_save::fields::char_field(ctx.save(), "EquipOrder", ctx.character);
    let affix = idleon_save::fields::char_field(ctx.save(), "EMm0", ctx.character);
    for (row, slots) in [(0usize, 16usize), (1, 8)] {
        let Some(names) = order.as_ref().and_then(|value| row_of(value, row)) else {
            continue;
        };
        for slot in 0..slots {
            let Some(name) = row_of(names, slot).and_then(|value| value.as_str()) else {
                continue;
            };
            if name.is_empty() || name == "Blank" {
                continue;
            }
            let base = ctx.data.items.get(name).map(|item| item.wp).unwrap_or(0.0);
            let stone = affix
                .as_ref()
                .and_then(|value| row_of(value, row))
                .and_then(|value| row_of(value, slot))
                .and_then(|value| value.get("Weapon_Power"))
                .and_then(serde_json::Value::as_f64)
                .unwrap_or(0.0);
            values[row] += base + stone;
        }
    }
    (values[0], values[1])
}

/// The obols' weapon power, excluding the skill obols the game reclassifies.
fn obol_weapon_power(ctx: &mut Ctx<'_, '_>) -> f64 {
    let mut total = 0.0;
    for (names, maps) in [
        (
            format!("ObolEqO0_{}", ctx.character),
            format!("ObolEqMAP_{}", ctx.character),
        ),
        ("ObolEqO1".to_string(), "ObolEqMAPz1".to_string()),
    ] {
        let (Some(names), Some(maps)) = (
            idleon_save::fields::field(ctx.save(), &names),
            idleon_save::fields::field(ctx.save(), &maps),
        ) else {
            continue;
        };
        let Some(rows) = names.as_array() else {
            continue;
        };
        for (slot, name) in rows.iter().enumerate() {
            let Some(name) = name.as_str() else {
                continue;
            };
            if name.is_empty() || name == "Blank" || name == "Null" || skill_obol(ctx, name) {
                continue;
            }
            let base = ctx.data.items.get(name).map(|item| item.wp).unwrap_or(0.0);
            let rolled = row_of(&maps, slot)
                .and_then(|value| value.get("Weapon_Power"))
                .and_then(serde_json::Value::as_f64)
                .unwrap_or(0.0);
            total += base + rolled;
        }
    }
    total
}

fn skill_obol(ctx: &Ctx<'_, '_>, name: &str) -> bool {
    [
        "Mining", "Choppin", "Fishing", "Catching", "Trapping", "Worship",
    ]
    .iter()
    .any(|skill| name.contains(skill))
        || ctx
            .data
            .items
            .get(name)
            .and_then(|item| item.kind.as_deref())
            .is_some_and(|kind| kind == "PICKAXE" || kind == "HATCHET")
}

fn family(ctx: &mut Ctx<'_, '_>, key: usize) -> f64 {
    ctx.family_bonuses().get(&key).copied().unwrap_or(0.0)
}

fn vault_killz(ctx: &mut Ctx<'_, '_>, index: usize) -> f64 {
    idleon_account::vault_killz::vault_killz(ctx.decoder)[index.min(11)]
}

fn dream(ctx: &Ctx<'_, '_>, index: usize) -> f64 {
    idleon_save::fields::at(ctx.save(), "Dream", &[index]).unwrap_or(0.0)
}

fn pet_power(ctx: &Ctx<'_, '_>) -> f64 {
    idleon_save::fields::at(ctx.save(), "PetsStored", &[0, 2]).unwrap_or(0.0)
}

fn softcap_additive(value: f64) -> f64 {
    let mut value = value;
    if value > 4000.0 {
        value = 4000.0 + (value - 4000.0).powf(0.91).max(0.0);
        if value > 15_000.0 {
            value = 15_000.0 + (value - 15_000.0).powf(0.84).max(0.0);
        }
    }
    value
}

fn softcap_multiplicative(mut value: f64, caps: &[(f64, f64)]) -> f64 {
    for (cap, exponent) in caps {
        if value > *cap {
            value = *cap * (value / cap).powf(*exponent);
        }
    }
    value
}

fn row_of(value: &serde_json::Value, index: usize) -> Option<&serde_json::Value> {
    match value {
        serde_json::Value::Array(rows) => rows.get(index),
        serde_json::Value::Object(rows) => rows.get(&index.to_string()),
        _ => None,
    }
}
