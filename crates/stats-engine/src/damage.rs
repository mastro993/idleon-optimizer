//! The damage layers, ported from `js/stats/defs/damage.js`.
//!
//! The descriptor multiplies three layers: `DDL0` (base damage after its two
//! softcaps), `DDL1` (the percentage multiplier after its three) and `DDL2` (the
//! multiplicative bonus groups after its six-stage chain). The minimum damage is
//! the maximum times the mastery ratio.
//!
//! Every group whose provider this build does not carry is recorded by name, so
//! a caller can show the layers and the reason they are a lower bound instead of
//! presenting a number nobody can justify. `debug` mirrors the descriptor's own
//! `_debug` keys so the fixture can compare the port term by term.

use crate::systems::Ctx;
use crate::{Stat, with_ctx};
use idleon_common::curve;
use idleon_research::model::Simulation;
use idleon_save::Save;
use std::collections::BTreeMap;

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
    /// The descriptor's own `_debug` values, one entry per term it records.
    pub debug: BTreeMap<String, f64>,
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
    with_ctx(save, simulation, character, |ctx| layers(ctx, map))
}

fn layers(ctx: &mut Ctx<'_, '_>, map: usize) -> Layers {
    let mut debug = BTreeMap::new();
    let mut base_debug = BTreeMap::new();

    // === MASTERY (min damage ratio) ===
    let talent113 = ctx.talent_tab(113, 2);
    let mastery_bubble = ctx.bubble("Mastery");
    let card21 = ctx.card_bonus(21);
    let talent123 = ctx.talent(123);
    let etc21 = ctx.decoder.etc(21, ctx.character);
    let mastery =
        (0.35 - talent113 / 100.0 + (mastery_bubble + card21 + talent123 + etc21) / 100.0).min(0.8);

    // === DDL0: base damage ===
    let (base_pre, base) = base_layer(ctx, map, &mut base_debug);
    let primary = base_debug.get("PrimaryStat").copied().unwrap_or(0.0);
    let hp_max = base_debug.get("hpMax").copied().unwrap_or(0.0);
    let mp_max = base_debug.get("mpMax").copied().unwrap_or(0.0);
    let speed = base_debug.get("playerSpeed").copied().unwrap_or(0.0);
    let mut percent_debug = BTreeMap::new();
    let percent_pre = percent_layer(ctx, primary, hp_max, mp_max, speed, &mut percent_debug);
    let mut multiplier_debug = BTreeMap::new();
    let (multiplier_pre, multiplier_post) = multiplier_layer(ctx, map, &mut multiplier_debug);
    for (key, value) in base_debug {
        debug.insert(format!("base.{key}"), value);
    }
    for (key, value) in percent_debug {
        debug.insert(format!("percent.{key}"), value);
    }
    for (key, value) in multiplier_debug {
        debug.insert(format!("multiplier.{key}"), value);
    }

    let percent = softcap_multiplicative(percent_pre, &[(100.0, 0.86), (2e6, 0.5), (1e8, 0.3)]);
    let multiplier = multiplier_post;
    let max = base * percent * multiplier;
    debug.insert("percent.postSoftcap".into(), percent);

    debug.insert("mastery.value".into(), mastery);
    debug.insert("mastery.penalty".into(), talent113 / 100.0);
    debug.insert("mastery.bubble".into(), mastery_bubble);
    debug.insert("mastery.card".into(), card21);
    debug.insert("mastery.talent".into(), talent123);
    debug.insert("mastery.etc".into(), etc21);

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
        debug,
    }
}

/// DDL0: `DamageDealtSTATtype`, the weapon-power chain and every flat source.
fn base_layer(ctx: &mut Ctx<'_, '_>, map: usize, debug: &mut BTreeMap<String, f64>) -> (f64, f64) {
    let stat_kind = crate::primary_stat_for_class(ctx.class());
    let stat_raw = crate::stats::total_stat_here(ctx, stat_kind);
    let talent95 = ctx.talent(95);
    let talent455 = ctx.talent(455);
    let talent20 = ctx.talent(20);
    let class = ctx.class();
    let (w6, a6, m6) = match class {
        0..=5 => (ctx.bubble("W6"), ctx.bubble("A6"), ctx.bubble("M6")),
        6..=17 => (ctx.bubble("W6"), 0.0, 0.0),
        18..=29 => (0.0, ctx.bubble("A6"), 0.0),
        _ => (0.0, 0.0, ctx.bubble("M6")),
    };
    let stat = stat_raw * (1.0 + (talent95 + talent455 + talent20 + w6 + a6 + m6) / 100.0);
    debug.insert("PrimaryStat".into(), stat);
    debug.insert("PrimaryStatRaw".into(), stat_raw);

    // Weapon power: equipment (row 0, gallery slots skipped), gallery, obols.
    let equip_wp = weapon_power(ctx);
    let gallery_wp = crate::stats::gallery_base_here(ctx, "Weapon_Power");
    let obol_wp = obol_weapon_power(ctx);
    let obol_misc = 1.0 + fields_number(ctx, "Menagerie", &[0, 9]) / 100.0;
    let obol_wp = obol_wp * obol_misc;
    let (w1, a1, m1) = match class {
        0..=6 => (0.0, 0.0, ctx.bubble("M1")),
        7..=17 => (ctx.bubble("W1"), 0.0, 0.0),
        18..=29 => (0.0, ctx.bubble("A1"), 0.0),
        _ => (0.0, 0.0, ctx.bubble("M1")),
    };
    let chip_wp = ctx.chip_bonus("weppow");
    let equip_scaled = (equip_wp + gallery_wp + obol_wp) * (1.0 + (chip_wp + w1 + a1 + m1) / 100.0);

    let box12a = ctx.box_reward("12a");
    let flurbo = ctx.flurbo_shop(0);
    let food_wp = ctx.weapon_food();
    let card_w5b2 = ctx.decoder.card_level("w5b2");
    let card18 = ctx.card_bonus(18);
    let sigil17 = ctx.sigil(17);
    let guild3 = ctx.guild(3);
    let beginner = ctx.beginner_weapon_cap();
    let vial_wp = ctx.vial("WeaponPOW");
    let fam16 = ctx.family(16);
    let star_wp = ctx.star_sign_bonus("WepPow");
    let arc17 = ctx.decoder.arcade(17);
    let t530 = ctx.talent(530) * (ctx.raw_level(12) / 10.0).floor();
    let t140 = ctx.talent(140) * (ctx.raw_level(10) / 10.0).floor();
    let t170 = ctx.talent(170) * (ctx.raw_level(15) / 10.0).floor();
    let t320 = ctx.talent(320) * (ctx.raw_level(13) / 10.0).floor();
    let t500 = ctx.talent(500) * (ctx.raw_level(14) / 10.0).floor();
    let pet_power = fields_number(ctx, "PetsStored", &[0, 2]).max(1.0);
    let t365 = ctx.talent(365) * curve::log(pet_power);
    let mut wp_raw = 5.0
        + box12a
        + flurbo
        + food_wp
        + card_w5b2
        + card18
        + sigil17
        + guild3
        + equip_scaled
        + beginner
        + vial_wp
        + fam16
        + star_wp
        + arc17
        + t530
        + t140
        + t170
        + t320
        + t500
        + t365;

    let talent97 = ctx.talent(97);
    let talent277 = ctx.talent(277);
    let talent457 = ctx.talent(457);
    let cosmo_wp = ctx.decoder.cosmo_bonus(2, 4);
    let talent5 = ctx.talent(5);
    let wp = wp_raw * (1.0 + (talent97 + talent277 + talent457 + cosmo_wp) / 100.0) + talent5;
    wp_raw = wp_raw.max(0.0);

    for (key, value) in [
        ("wpRaw", wp_raw),
        ("wp", wp),
        ("equipWP", equip_wp),
        ("galleryWP", gallery_wp),
        ("obolWP", obol_wp),
        ("_box12a", box12a),
        ("_flurbo", flurbo),
        ("_foodWP", food_wp),
        ("_card18", card18),
        ("_cardW5b2", card_w5b2),
        ("_sigil17", sigil17),
        ("_guild3", guild3),
        ("_chipWP", chip_wp),
        ("_bubW1", w1),
        ("_bubA1", a1),
        ("_bubM1", m1),
        ("_tc616", beginner),
        ("_vialWP", vial_wp),
        ("_fam16", fam16),
        ("_starWP", star_wp),
        ("_arc17", arc17),
        ("_t530", t530),
        ("_t140", t140),
        ("_t170", t170),
        ("_t320", t320),
        ("_t500", t500),
        ("_t365", t365),
    ] {
        debug.insert(key.into(), value);
    }

    let gf_base = ctx.gold_food_bonus("BaseDamage");
    let box_base = ctx.box_reward("basedmg");
    let arcade0 = ctx.decoder.arcade(0);
    let vault0 = ctx.decoder.vault(0);
    let vault20 = ctx.decoder.vault(20) * vault_killz(ctx, 5);
    let statue0 = ctx.statue_bonus(0);
    let etc16 = ctx.decoder.etc(16, ctx.character);
    let card4 = ctx.card_bonus(4);
    let stamp_base = ctx.stamp("BaseDmg");
    let owl1 = ctx.owl(1);
    let sigil4 = ctx.sigil(4);
    let halloween = ctx.decoder.card_level("fallEvent2");

    let hp_max = ctx.max_hp(map);
    let mp_max = ctx.max_mp();
    let speed = ctx.movement_speed();
    let bubble_hp = ctx.bubble("bdmgHP") * curve::log((hp_max - 250.0).max(1.0));
    let bubble_mp = ctx.bubble("bdmgMP") * curve::log((mp_max - 150.0).max(1.0));
    let bubble_spd = ctx.bubble("bdmgSPD") * (((speed - 0.1).max(0.0)).log2() / 0.25);

    let mut base = (wp / 3.0).powi(2) + stat + gf_base + 150.0f64.min(2.0 * wp_raw + stat);
    base += arcade0 + vault0 + vault20;
    base += statue0 + box_base + etc16 + card4;
    base += stamp_base + owl1 + sigil4 + halloween + bubble_hp + bubble_mp + bubble_spd;
    let base_pre = base;

    for (key, value) in [
        ("gfBaseDmg", gf_base),
        ("boxBaseDmg", box_base),
        ("arcade0", arcade0),
        ("vault0", vault0),
        ("statue0", statue0),
        ("card4", card4),
        ("sigil4", sigil4),
        ("owlBonus1", owl1),
        ("halloweenCard", halloween),
        ("hpMax", hp_max),
        ("mpMax", mp_max),
        ("playerSpeed", speed),
        ("bdmgHP", ctx.bubble("bdmgHP")),
        ("bdmgMP", ctx.bubble("bdmgMP")),
        ("bdmgSPD", ctx.bubble("bdmgSPD")),
        ("bubbleHP", bubble_hp),
        ("bubbleMP", bubble_mp),
        ("bubbleSPD", bubble_spd),
        ("stampBaseDmg", stamp_base),
        ("baseDmgFood", 0.0),
        ("preSoftcapRaw", base_pre),
    ] {
        debug.insert(key.into(), value);
    }

    let base_post = softcap_additive(base_pre);
    // Upstream records the post-softcap base under `preSoftcap` and the raw one
    // under `preSoftcapRaw`.
    debug.insert("preSoftcap".into(), base_post);
    (base_pre, base_post)
}

/// DDL1: the additive percentage pool, then its three multipliers.
fn percent_layer(
    ctx: &mut Ctx<'_, '_>,
    stat_type: f64,
    hp_max: f64,
    mp_max: f64,
    speed: f64,
    debug: &mut BTreeMap<String, f64>,
) -> f64 {
    let stat_pow = stat_type.powf(0.7);
    let vault27 = ctx.decoder.vault(27) * vault_killz(ctx, 6);
    let vault15 = ctx.decoder.vault(15);
    let ola338 = ctx.option(338);
    let vault10 = ctx.decoder.vault(10);
    let talent113 = ctx.talent_tab(113, 2);
    let talent86 = ctx.talent(86);
    let talent446 = ctx.talent(446);
    let hp_log = curve::log(hp_max.max(1.0));
    let mp_log = curve::log(mp_max.max(1.0));

    let farm14 = ctx.farm_rank_upgrade(14);
    let stamp_pct = ctx.stamp("PctDmg");
    let statue22 = ctx.statue_bonus(22);
    let owl2 = ctx.owl(2);
    let roo4 = ctx.roo(4);
    let bubba4 = ctx.bubba_rog(4);
    let vault80 = ctx.decoder.vault(80);

    let add_base = stat_pow
        + vault27
        + vault15 * ola338
        + 0.4 * vault10
        + ctx.bribe(30)
        + ctx.bribe(20)
        + stamp_pct
        + farm14
        + statue22
        + talent113
        + hp_log * talent86
        + mp_log * talent446
        + owl2
        + roo4
        + bubba4
        + vault80;
    let pct_base = 1.0 + add_base / 100.0;

    let smith = 1.0 + ctx.talent(284) * (ctx.raw_level(2).min(100.0) / 10.0) / 100.0;
    let win_bonus = ctx.decoder.win_bonus(0);
    let win = 1.0 + win_bonus / 100.0;

    // The talent chain: class bubbles, ten Slabbo/TalentCalc rows, the Cavern
    // schematic, the speed talent, dreams, quests, Divinity and the minigame.
    let chain = talent_chain(ctx, speed, debug);
    let gf_damage = 1.0 + ctx.gold_food_bonus("Damage") / 100.0;

    let percent = pct_base * smith * win * (1.0 + chain / 100.0) * gf_damage;

    for (key, value) in [
        ("statPow", stat_pow),
        ("vault27", vault27),
        ("vault15", vault15 * ola338),
        ("ola338", ola338),
        ("vault10", 0.4 * vault10),
        ("vault80", vault80),
        ("stampPctDmg", stamp_pct),
        ("farmRank14", farm14),
        ("statue22", statue22),
        ("hpLog", hp_log),
        ("talent86", talent86),
        ("mpLog", mp_log),
        ("talent446", talent446),
        ("owlBonus2", owl2),
        ("rooBonus4", roo4),
        ("bubbaRoG4", bubba4),
        ("addBase", add_base),
        ("pctMultBase", pct_base),
        ("smithMult", smith),
        ("winBonus0", win_bonus),
        ("winMult", win),
        ("talentChainSum", chain),
        ("talentChainMult", 1.0 + chain / 100.0),
        ("gfDamage", gf_damage),
        ("preSoftcap", percent),
    ] {
        debug.insert(key.into(), value);
    }
    percent
}

/// The talent chain inside DDL1.
fn talent_chain(ctx: &mut Ctx<'_, '_>, speed: f64, debug: &mut BTreeMap<String, f64>) -> f64 {
    // The descriptor adds all three class bubbles here, without the class
    // gating the W6/A6/M6 step applies.
    let w12 = ctx.bubble("W12");
    let a12 = ctx.bubble("A12");
    let m12 = ctx.bubble("M12");
    let mut sum = w12 + a12 + m12;

    let lowest_skill = (1..=9)
        .map(|skill| ctx.level(skill))
        .fold(f64::INFINITY, f64::min);
    let lowest_skill = if lowest_skill > 1e8 {
        0.0
    } else {
        lowest_skill
    };
    let tc31 = ctx.talent(31) * (lowest_skill / 5.0).floor();

    let gtn1_110 = ctx.talent(110);
    let gtn2_110 = ctx.talent_tab(110, 2);
    let mut monsters_over = 0.0f64;
    if gtn1_110 > 0.0 {
        let holder = death_bringer(ctx);
        let kla = idleon_save::fields::char_field(ctx.save(), "KLA", holder);
        for map in 0..idleon_stats_data::map_count() {
            // `TalentCalc(110)` skips the filler rows as well, so it counts a
            // few fewer maps than `isFightingMap` does.
            if !crate::is_fighting_map(map) || idleon_stats_data::afk_target(map) == Some("Filler")
            {
                continue;
            }
            let req = idleon_stats_data::kill_requirement(map);
            let left = kla
                .as_ref()
                .and_then(|value| idleon_save::json::index(value, &[map]).cloned())
                .and_then(|value| value.as_array().and_then(|row| row.first()).cloned())
                .and_then(|value| idleon_save::json::number(&value))
                .unwrap_or(0.0);
            if req - left >= 100_000.0 {
                monsters_over += 1.0;
            }
        }
    }
    let tc110 = gtn1_110 * monsters_over.min(gtn2_110);
    debug.insert("monsters100k".into(), monsters_over);
    debug.insert("gtn110".into(), gtn1_110);
    debug.insert("gtn110tab2".into(), gtn2_110);

    let refinery: f64 = (3..=8)
        .map(|row| fields_number(ctx, "Refinery", &[row, 1]))
        .sum();
    let tc125 = ctx.talent(125) * refinery;
    let vials_above = fields::field(ctx.save(), "CauldronInfo")
        .and_then(|value| idleon_save::fields::object_row(&value, 4))
        .map(|row| row.iter().filter(|cell| cell.unwrap_or(0.0) > 3.0).count() as f64)
        .unwrap_or(0.0);
    let tc485 = ctx.talent(485) * vials_above;
    let card_count = fields::len_of(ctx.save(), "Cards1") as f64;
    let tc305 = ctx.talent(305) * card_count / 50.0;
    let stamp_count = fields::field(ctx.save(), "StampLv")
        .map(|value| {
            let mut count = 0.0;
            for category in 0..3 {
                if let Some(row) = idleon_save::fields::object_row(&value, category) {
                    count += row.iter().filter(|cell| cell.unwrap_or(0.0) > 0.5).count() as f64;
                }
            }
            count
        })
        .unwrap_or(0.0);
    let tc470 = ctx.talent(470) * stamp_count / 10.0;
    let caver57 = if fields_number(ctx, "Holes", &[13, 57]) > 0.0 {
        20.0 * curve::log(fields_number(ctx, "Holes", &[9, 1]).max(1.0)).floor()
    } else {
        0.0
    };
    let t290 = ctx.talent(290) * ((speed - 1.0).min(10.0) / 0.15).floor();
    let dreams = fields::field(ctx.save(), "WeeklyBoss")
        .and_then(|value| value.as_object().cloned())
        .map(|rows| {
            (0..100)
                .filter(|index| {
                    rows.get(&format!("d_{index}"))
                        .and_then(idleon_save::json::number)
                        == Some(-1.0)
                })
                .count() as f64
        })
        .unwrap_or(0.0);
    let tc656 = ctx.talent(656) * dreams;
    let t649 = curve::log(ctx.option(161)) * ctx.talent(649);
    let t638 = curve::log(ctx.option(71)) * ctx.talent(638);
    let talent658 = ctx.talent(658);
    let quests658 = ctx.quests().min(talent658);
    let div7 = ctx.divinity_minor(7);
    let minigame = fields_number(ctx, "FamValMinigameHiscores", &[0]);
    let tc463 = ctx.talent_tab(463, 2) * (minigame / 25.0).floor();

    sum += tc31
        + tc110
        + tc125
        + tc485
        + tc305
        + tc470
        + caver57
        + t290
        + tc656
        + t649
        + t638
        + quests658
        + div7
        + tc463;
    for (key, value) in [
        ("bubbleW12", w12),
        ("bubbleA12", a12),
        ("bubbleM12", m12),
        ("tc31", tc31),
        ("tc110", tc110),
        ("tc125", tc125),
        ("tc485", tc485),
        ("tc305d50", tc305),
        ("tc470d10", tc470),
        ("bUpg57", caver57),
        ("t290speed", t290),
        ("tc656", tc656),
        ("talent658", talent658),
        ("quests658", quests658),
        ("divMinor7", div7),
        ("talent463", ctx.talent_tab(463, 2)),
        ("minigameHS", minigame),
    ] {
        debug.insert(key.into(), value);
    }
    sum
}

/// DDL2: the multiplicative groups and their softcap chain.
fn multiplier_layer(
    ctx: &mut Ctx<'_, '_>,
    map: usize,
    debug: &mut BTreeMap<String, f64>,
) -> (f64, f64) {
    let t508 = ctx.talent_best(508);
    let t208 = ctx.talent_best(208);
    let mut multi = (1.0 + t508 * curve::log(ctx.option(152)) / 100.0)
        * (1.0 + t208 * curve::log(ctx.option(329)) / 100.0);
    if multi < 1.0 {
        multi = 1.0;
    }
    debug.insert("wb".to_string(), multi);

    multi *= 1.0 + ctx.vial("7dmg") / 100.0;
    multi *= 1.0 + ctx.eclipse_skulls() / 100.0;
    multi *= 1.0 + ctx.decoder.palette(34) / 100.0;
    multi *= 1.0 + fields_number(ctx, "Dream", &[6]) / 10.0;
    multi *= 1.0 + ctx.decoder.pristine(0) / 100.0;
    multi *= 1.0 + ctx.summ_upg(79) / 100.0;
    multi *= 1.0 + ctx.cglunko_dmg() / 100.0;

    // Friends, star signs and Divinity.
    let div25 = fields_number(ctx, "Divinity", &[25]);
    let class_level = ctx.class_level();
    let group1 = ctx.buff(108)
        + ctx.decoder.friend(0)
        + ctx.star_sign_bonus("PctDmg")
        + (div25 - 10.0).max(0.0) * ctx.talent_best(507)
        + ((class_level - 200.0).max(0.0) / 50.0).floor() * ctx.talent_best(50);
    multi *= 1.0 + group1 / 100.0;

    // Grimoire, sets, shrines, monuments and account pools.
    let group2 = ctx.decoder.grimoire(35)
        + idleon_account::sets::set_bonus(ctx.decoder, "LUSTRE_SET", ctx.character)
        + ctx.shrine(0, 0)
        + ctx.monument_rog(0, 6)
        + ctx.box_reward("12c")
        + ctx.box_reward("21c")
        + ctx.box_reward("23c")
        + ctx.family(20)
        + ctx.artifact(25)
        + fields_number(ctx, "Atoms", &[9]) * ctx.data.atom(9)
        + ctx.shiny(5)
        + ctx.msa(0)
        + ctx.option(178) * ctx.shimmer()
        + ctx.decoder.crop_scientist(0)
        + ctx.decoder.vault(41) * curve::log(ctx.option(346));
    multi *= 1.0 + group2 / 100.0;

    // Rift, damage bubbles and the four stat bubbles.
    let (w5, a5, m5) = match ctx.class() {
        0..=17 => (ctx.bubble("W5"), 0.0, 0.0),
        18..=29 => (0.0, ctx.bubble("A5"), 0.0),
        _ => (0.0, 0.0, ctx.bubble("M5")),
    };
    let str = crate::stats::total_stat_here(ctx, Stat::Str);
    let agi = crate::stats::total_stat_here(ctx, Stat::Agi);
    let wis = crate::stats::total_stat_here(ctx, Stat::Wis);
    let luk = crate::stats::total_stat_here(ctx, Stat::Luk);
    let stat_bubbles = w5 * (str.max(luk) / 250.0).floor()
        + a5 * (agi / 250.0).floor()
        + m5 * (wis / 250.0).floor();
    let group3 = ctx.rift_skill_etc(0)
        + ctx.bubble("pctDmg1")
        + ctx.bubble("pctDmg2")
        + ctx.bubble("pctDmg3")
        + ctx.artifact(2)
        + ctx.artifact(8)
        + stat_bubbles
        + ctx.const_mastery()
        + ctx.tome_bonus(0)
        + ctx.compass(48)
        + ctx.exotic(41)
        + ctx.option(419);
    multi *= 1.0 + group3 / 100.0;

    // Talents, Salt Lick, prayers and the Lab.
    let b_upg84 = if fields_number(ctx, "Holes", &[11, 55]) > 0.0
        && fields_number(ctx, "Holes", &[13, 84]) > 0.0
    {
        100.0 * fields_number(ctx, "Holes", &[11, 55])
    } else {
        0.0
    };
    let group4 = ctx.talent(6)
        + ctx.salt_lick(9)
        + ctx.decoder.etc(45, ctx.character)
        + idleon_account::prayer::prayer(ctx.save(), ctx.character, 15, false)
        + ctx.mainframe(0)
        + ctx.mainframe(11)
        + ctx.mainframe(110)
        + ctx.artifact(27)
        + ctx.artifact(29)
        + b_upg84
        + ctx.decoder.arcade(46)
        + ctx.option(435);
    multi *= 1.0 + group4 / 100.0;

    // Companions and cards.
    let group5 = ctx.decoder.companion(10)
        + ctx.decoder.companion(156)
        + ctx.card_bonus(42)
        + ctx.card_set_bonus("5");
    multi *= 1.0 + group5 / 100.0;

    // Pets, meals, achievements and Divinity blessings.
    let achievements = 2.0 * ctx.decoder.achievement(58)
        + 3.0 * ctx.decoder.achievement(59)
        + 5.0 * ctx.decoder.achievement(60)
        + 5.0 * ctx.decoder.achievement(62)
        + 2.0 * ctx.decoder.achievement(119)
        + 3.0 * ctx.decoder.achievement(120)
        + 5.0 * ctx.decoder.achievement(121)
        + 4.0 * ctx.decoder.achievement(189)
        + 2.0 * ctx.decoder.achievement(185)
        + 3.0 * ctx.decoder.achievement(186)
        + 5.0 * ctx.decoder.achievement(187)
        + ctx.decoder.achievement(240)
        + ctx.decoder.achievement(280)
        + 3.0 * ctx.decoder.achievement(297)
        + 2.0 * ctx.decoder.achievement(303)
        + 2.0 * ctx.decoder.achievement(364)
        + 4.0 * ctx.decoder.achievement(354)
        + 3.0 * ctx.decoder.achievement(375);
    let group6 = 20.0 * ctx.pet_arena(2)
        + 40.0 * ctx.pet_arena(15)
        + ctx.chip_bonus("dmg")
        + ctx.meal_bonus("TotDmg")
        + achievements
        + ctx.divinity_bless(7)
        + ctx.divinity_bless(8);
    multi *= 1.0 + group6 / 100.0;

    // Penalties.
    let penalty = ((1.0 - ctx.talent(24) / 100.0)
        * (1.0 - ctx.buff(124) / 100.0)
        * (1.0
            - (idleon_account::prayer::prayer(ctx.save(), ctx.character, 6, true)
                + idleon_account::prayer::prayer(ctx.save(), ctx.character, 13, true))
                / 100.0)
            .max(0.01))
    .max(0.05);
    multi *= penalty;

    // Minehead, jelly, the Royal Statue and Coral Kid.
    let weapon = fields::string_row(ctx.save(), &format!("EquipOrder_{}", ctx.character), &[0])
        .unwrap_or_default()
        .get(1)
        .cloned()
        .unwrap_or_default();
    let weapon_wp = ctx
        .data
        .items
        .get(&weapon)
        .map(|item| item.wp)
        .unwrap_or(0.0);
    let weapon_affix = fields::char_field(ctx.save(), "EMm0", ctx.character)
        .as_ref()
        .and_then(|value| row_of(value, 1))
        .and_then(|value| value.get("Weapon_Power"))
        .and_then(idleon_save::json::number)
        .unwrap_or(0.0);
    let minehead_wp = ctx.minehead_bonus(4) * (weapon_wp + weapon_affix);
    let royal_statue = ctx.royal_statue(0);
    let coral_level = ctx.option(621);
    let coral = ctx.coral_kid(11, coral_level);
    let coral_multi = (1.0 + coral / 100.0).powf((div25 - 35.0).max(0.0));
    multi *= 1.0 + ctx.minehead_bonus(0) / 100.0;
    multi *= 1.0 + minehead_wp / 100.0;
    multi *= 1.0 + ctx.jelly_reward(37) / 100.0;
    multi *= 1.0 + royal_statue / 100.0;
    multi *= coral_multi;

    let weekly = ctx.weekly_boss();
    if weekly != 0.0 {
        multi *= 1.0 + weekly.min(150.0) / 100.0;
    }

    let pre_softcap = multi;

    // The six-stage DDL2 softcap chain.
    let mut softcapped = pre_softcap;
    for (index, (cap, exponent)) in [
        (100.0, 0.86),
        (2e7, 0.8),
        (5e8, 0.6),
        (2e9, 0.45),
        (15e9, 0.36),
        (6e10, 0.28),
    ]
    .iter()
    .enumerate()
    {
        if softcapped <= *cap {
            continue;
        }
        softcapped = if index == 0 {
            cap + (softcapped - cap).max(0.0).powf(*exponent)
        } else {
            cap * (softcapped / cap).powf(*exponent)
        };
    }
    multi = softcapped;

    // Post-softcap multipliers.
    let sticker = ctx.sticker_dmg().max(1.0);
    let etc72 = ctx.decoder.etc(72, ctx.character);
    let etc75 = ctx.decoder.etc(75, ctx.character);
    let etc104 = ctx.decoder.etc(104, ctx.character);
    let voting = ctx.decoder.voting(1);
    let fountain = ctx.fountain_bon(1, 16);
    let rog49 = ctx.decoder.rog_qty(49);
    let superbit = if ctx.decoder.superbit(64) == 1.0 {
        1.0
    } else {
        0.0
    };
    let ola232 = ctx.option(232);
    let ola232_bonus = 10.0 * ((96.0 + ola232) / 100.0).floor();
    let card96 = ctx.card_bonus(96);
    let tome6 = ctx.tome_bonus(6);
    let ach371 = ctx.decoder.achievement(371);
    let ach384 = ctx.decoder.achievement(384);
    let killroy = (ctx.killroy_dmg() / 100.0).clamp(0.0, 2.0);

    multi *= 1.0 + etc72 / 100.0;
    multi *= 1.0 + etc75 / 100.0;
    multi *= 1.0 + etc104 / 100.0;
    multi *= 1.0 + voting / 100.0;
    multi *= 1.0 + fountain / 100.0;
    multi *= 1.0 + rog49 / 100.0;
    multi *= 1.0 + 0.1 * superbit;
    multi *= 1.0 + ola232_bonus / 100.0;
    multi *= 1.0 + card96 / 100.0;
    multi *= sticker;
    multi *= 1.0 + (tome6 + ach371 + ach384) / 100.0;
    multi *= 1.0 + killroy;

    // Post-softcap companions and the account multipliers.
    let comp12 = ctx.decoder.companion(12);
    let comp33 = ctx.decoder.companion(33);
    let comp160 = ctx.decoder.companion(160);
    let comp168 = ctx.decoder.companion(168);
    let comp170 = ctx.decoder.companion(170);
    let comp_multi = ((1.0 + comp12)
        * (1.0 + comp33)
        * (1.0 + 2.0 * comp160)
        * (1.0 + 0.5 * comp168)
        * (1.0 + comp170))
        .max(1.0);
    let crystal = (1.5 * ctx.decoder.card_level("Crystal6")).min(15.0);
    let meritoc5 = ctx.decoder.meritoc(5);
    let green = (ctx.green_sigils() * ctx.decoder.grid_bonus(127)).min(100.0);
    multi *= comp_multi;
    multi *= 1.0 + crystal / 100.0;
    multi *= 1.0 + meritoc5 / 100.0;
    multi *= 1.0 + green / 100.0;

    let bundle = ctx.decoder.bundle("bon_a");
    if bundle {
        multi *= 1.5;
    }
    multi *= 1.0 + ctx.family(80) / 100.0;

    let reliquarium = ctx.reliquarium();
    if reliquarium {
        multi = multi.powf(4.0 / (5.0 + ctx.option(473)));
    }
    let scurge = ctx.scurge_level(map);
    multi /= 100f64.powf(scurge);

    for (key, value) in [
        ("preSoftcap", pre_softcap),
        ("postSoftcap", softcapped),
        ("final", multi),
        ("penaltyMult", penalty),
        ("compMult", comp_multi),
        ("etc72", etc72),
        ("etc75", etc75),
        ("etc104", etc104),
        ("votingDmg", voting),
        ("comp12", comp12),
        ("comp33", comp33),
        ("comp160", comp160),
        ("comp168", comp168),
        ("comp170", comp170),
        ("cglunkoDmg", ctx.cglunko_dmg()),
        ("buff108", ctx.buff(108)),
        ("buff124", ctx.buff(124)),
        ("fountainDmg", fountain),
        ("stickerDmg", sticker),
        ("ola232", ola232),
        ("ola232Bonus", ola232_bonus),
        ("card96", card96),
        ("ach371", ach371),
        ("ach384", ach384),
        ("rog49", rog49),
        ("superBit64", superbit),
        ("tomeBonus6", tome6),
        ("crystal6Bonus", ctx.decoder.card_level("Crystal6")),
        ("meritoc5", meritoc5),
        ("famBonus80", ctx.family(80)),
        ("reliquariumOwned", if reliquarium { 1.0 } else { 0.0 }),
        ("scurgeLevel", scurge),
        ("scurgeDivisor", 100f64.powf(scurge)),
        ("bundleApplied", if bundle { 1.0 } else { 0.0 }),
        ("group1", group1),
        ("group2", group2),
        ("group3", group3),
        ("group4", group4),
        ("group5", group5),
        ("group6", group6),
        ("minehead0", ctx.minehead_bonus(0)),
        ("mineheadWp", minehead_wp),
        ("jelly37", ctx.jelly_reward(37)),
        ("royalStatue0", royal_statue),
        ("coralKid11", coral),
        ("coralMulti", coral_multi),
        ("weekly", weekly),
        ("divLvBonus", (div25 - 10.0).max(0.0) * ctx.talent_best(507)),
        (
            "lvlBonus",
            ((class_level - 200.0).max(0.0) / 50.0).floor() * ctx.talent_best(50),
        ),
        ("div25", div25),
        ("statBubbles", stat_bubbles),
        ("artifact25", ctx.artifact(25)),
        ("crop0", ctx.decoder.crop_scientist(0)),
        ("monument06", ctx.monument_rog(0, 6)),
        ("grimoire35", ctx.decoder.grimoire(35)),
        ("msa0", ctx.msa(0)),
        ("tome0", ctx.tome_bonus(0)),
        ("compass48", ctx.compass(48)),
        ("exotic41", ctx.exotic(41)),
        ("shimmer", ctx.shimmer()),
        ("shiny5", ctx.shiny(5)),
        ("artifact27", ctx.artifact(27)),
        ("artifact29", ctx.artifact(29)),
        ("artifact2", ctx.artifact(2)),
        ("artifact8", ctx.artifact(8)),
        ("riftETC0", ctx.rift_skill_etc(0)),
        ("saltLick9", ctx.salt_lick(9)),
        (
            "prayer15",
            idleon_account::prayer::prayer(ctx.save(), ctx.character, 15, false),
        ),
        ("mf0", ctx.mainframe(0)),
        ("mf11", ctx.mainframe(11)),
        ("mf110", ctx.mainframe(110)),
        ("bUpg84", b_upg84),
        ("arcade46", ctx.decoder.arcade(46)),
        ("bUpg57", fields_number(ctx, "Holes", &[13, 57])),
    ] {
        debug.insert(key.into(), value);
    }
    (pre_softcap, multi)
}

/// The `W6`/`A6`/`M6` class bubbles were folded into `base_layer`; this keeps
/// the shared helper for the holder lookup the ten Slabbo rows need.
fn fields_number(ctx: &Ctx<'_, '_>, key: &str, path: &[usize]) -> f64 {
    fields::at(ctx.save(), key, path).unwrap_or(0.0)
}

use idleon_save::fields;

/// The character whose KLA row the ten Slabbo rows read: the Death Bringer.
fn death_bringer(ctx: &Ctx<'_, '_>) -> usize {
    let mut holder = 0;
    for character in 0..ctx.characters() {
        let class = fields::char_field(ctx.save(), "CharacterClass", character)
            .and_then(|value| idleon_save::json::number(&value))
            .unwrap_or(0.0) as usize;
        if idleon_account::class_tree(class).is_some_and(|tree| tree.get(3) == Some(&10u32)) {
            holder = character;
        }
    }
    holder
}

fn weapon_power(ctx: &mut Ctx<'_, '_>) -> f64 {
    let order = fields::string_row(ctx.save(), &format!("EquipOrder_{}", ctx.character), &[0])
        .unwrap_or_default();
    let affix = fields::char_field(ctx.save(), "EMm0", ctx.character);
    let gallery = fields::field(ctx.save(), "Spelunk")
        .and_then(|value| idleon_save::json::index(&value, &[16]).cloned())
        .and_then(|value| value.as_array().map(|row| !row.is_empty()))
        .unwrap_or(false);
    let mut total = 0.0;
    for slot in 0..16 {
        if gallery && matches!(slot, 8 | 10 | 14) {
            continue;
        }
        let Some(name) = order.get(slot).map(String::as_str) else {
            continue;
        };
        if name.is_empty() || name == "Blank" {
            continue;
        }
        let base = ctx.data.items.get(name).map(|item| item.wp).unwrap_or(0.0);
        let stone = affix
            .as_ref()
            .and_then(|value| row_of(value, slot))
            .and_then(|value| value.get("Weapon_Power"))
            .and_then(idleon_save::json::number)
            .unwrap_or(0.0);
        total += base + stone;
    }
    total
}

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

fn vault_killz(ctx: &mut Ctx<'_, '_>, index: usize) -> f64 {
    idleon_account::vault_killz::vault_killz(ctx.decoder)[index.min(11)]
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
    for (index, (cap, exponent)) in caps.iter().enumerate() {
        if value <= *cap {
            continue;
        }
        // The first stage of each chain is additive: `cap + excess^exponent`.
        value = if index == 0 {
            *cap + (value - cap).max(0.0).powf(*exponent)
        } else {
            *cap * (value / cap).powf(*exponent)
        };
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
