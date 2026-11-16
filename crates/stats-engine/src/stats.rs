//! The four primary stats share one shape: a top-level additive pool, a percent
//! multiplier, the equipment/obol pools it scales, a flat base and a flat add.
//! Each stat's sources are its config row; the tree keeps one
//! node per term so a caller can show where the total came from.

use crate::systems::Ctx;
use crate::{Stat, with_ctx};
use idleon_common::formula::FormulaNode;
use idleon_research::model::Simulation;
use idleon_save::Save;

/// One stat's config row.
pub(crate) struct Config {
    total_bubble: &'static str,
    dn_pct_talent: usize,
    dn_pct_stamp: &'static str,
    dn2_pct_talent: usize,
    pct_talent: Option<usize>,
    pct_bubble: Option<&'static str>,
    etc_pct: usize,
    pristine: usize,
    flat_talent: usize,
    guild_talent: usize,
    stamp_base: &'static str,
    box_base: Option<&'static str>,
    etc_flat: usize,
    ola_shimmer: usize,
    extra_talents: &'static [usize],
    extra_tab2: Option<usize>,
    buff: Option<(usize, usize)>,
    box_stat: &'static str,
    family: usize,
    card: usize,
    flat_talent2: Option<usize>,
    sigil: usize,
    shiny: usize,
    arcade: usize,
    a4_bubble: &'static str,
    quests_618: bool,
}

const STR: Config = Config {
    total_bubble: "TotalSTR",
    dn_pct_talent: 96,
    dn_pct_stamp: "PctSTR",
    dn2_pct_talent: 111,
    pct_talent: Some(143),
    pct_bubble: Some("W8"),
    etc_pct: 57,
    pristine: 4,
    flat_talent: 10,
    guild_talent: 51,
    stamp_base: "BaseSTR",
    box_base: None,
    etc_flat: 51,
    ola_shimmer: 174,
    extra_talents: &[98, 203],
    extra_tab2: Some(142),
    buff: Some((94, 1)),
    box_stat: "23b",
    family: 14,
    card: 9,
    flat_talent2: None,
    sigil: 0,
    shiny: 6,
    arcade: 19,
    a4_bubble: "W4",
    quests_618: false,
};

const AGI: Config = Config {
    total_bubble: "TotalAGI",
    dn_pct_talent: 276,
    dn_pct_stamp: "PctAGI",
    dn2_pct_talent: 291,
    pct_talent: Some(368),
    pct_bubble: Some("A9"),
    etc_pct: 25,
    pristine: 1,
    flat_talent: 11,
    guild_talent: 52,
    stamp_base: "BaseAGI",
    box_base: None,
    etc_flat: 52,
    ola_shimmer: 175,
    extra_talents: &[278, 428],
    extra_tab2: Some(367),
    buff: None,
    box_stat: "21b",
    family: 38,
    card: 7,
    flat_talent2: None,
    sigil: 1,
    shiny: 7,
    arcade: 20,
    a4_bubble: "A4",
    quests_618: false,
};

const WIS: Config = Config {
    total_bubble: "TotalWIS",
    dn_pct_talent: 456,
    dn_pct_stamp: "PctWIS",
    dn2_pct_talent: 486,
    pct_talent: Some(533),
    pct_bubble: Some("M9"),
    etc_pct: 58,
    pristine: 10,
    flat_talent: 12,
    guild_talent: 53,
    stamp_base: "BaseWIS",
    box_base: None,
    etc_flat: 53,
    ola_shimmer: 176,
    extra_talents: &[459, 593],
    extra_tab2: Some(532),
    buff: None,
    box_stat: "22b",
    family: 62,
    card: 5,
    flat_talent2: None,
    sigil: 2,
    shiny: 8,
    arcade: 21,
    a4_bubble: "M4",
    quests_618: false,
};

const LUK: Config = Config {
    total_bubble: "TotalLUK",
    dn_pct_talent: 21,
    dn_pct_stamp: "PctLUK",
    dn2_pct_talent: 36,
    pct_talent: None,
    pct_bubble: None,
    etc_pct: 17,
    pristine: 5,
    flat_talent: 13,
    guild_talent: 54,
    stamp_base: "BaseLUK",
    box_base: Some("15c"),
    etc_flat: 54,
    ola_shimmer: 177,
    extra_talents: &[],
    extra_tab2: None,
    buff: None,
    box_stat: "LUK",
    family: 4,
    card: 2,
    flat_talent2: Some(23),
    sigil: 3,
    shiny: 9,
    arcade: 22,
    a4_bubble: "A4",
    quests_618: true,
};

impl Stat {
    pub(crate) fn config(self) -> &'static Config {
        match self {
            Stat::Str => &STR,
            Stat::Agi => &AGI,
            Stat::Wis => &WIS,
            Stat::Luk => &LUK,
        }
    }
}

/// One stat's total, with the tree it was built from and the terms this build
/// could not justify.
#[derive(Clone, Debug, PartialEq)]
pub struct Total {
    pub stat: Stat,
    pub computed: f64,
    pub tree: FormulaNode,
    pub missing: Vec<String>,
}

impl Total {
    /// Whether every term the total sums was modelled.
    pub fn is_complete(&self) -> bool {
        self.missing.is_empty()
    }
}

/// A character's total of one primary stat.
pub fn total_stat(
    save: &Save,
    simulation: &Simulation,
    character: usize,
    stat: Stat,
) -> Option<Total> {
    with_ctx(save, simulation, character, |ctx| compute(ctx, stat))
}

/// The total from a context that already exists, for the damage layers.
pub(crate) fn total_stat_here(ctx: &mut Ctx<'_, '_>, stat: Stat) -> f64 {
    compute(ctx, stat).computed
}

/// The whole [`Total`] from an existing context, gaps included.
pub(crate) fn compute_here(ctx: &mut Ctx<'_, '_>, stat: Stat) -> Total {
    compute(ctx, stat)
}

fn compute(ctx: &mut Ctx<'_, '_>, stat: Stat) -> Total {
    let config = stat.config();
    let name = stat.id();

    // Equipment and obols: the raw item stat plus the saved affixes.
    let equip_base = equip_base(ctx, name);
    let gallery_base = gallery_base(ctx, name);
    let equip_pct_talent = ctx.talent(config.dn_pct_talent);
    let equip_pct_stamp = ctx.stamp(config.dn_pct_stamp);
    let equip_pct = 1.0 + (equip_pct_talent + equip_pct_stamp) / 100.0;
    let dn = (equip_base + gallery_base) * equip_pct;
    let obol_base = obol_base(ctx, name);
    let obol_pct_talent = ctx.talent(config.dn2_pct_talent);
    let obol_pct =
        1.0 + (obol_pct_talent + 40.0 * (ctx.decoder.superbit(2) == 1.0) as i32 as f64) / 100.0;
    let dn2 = obol_base * obol_pct;

    // Percent pool.
    let pct_talent = config.pct_talent.map_or(0.0, |id| ctx.talent(id));
    let etc_pct = ctx.decoder.etc(config.etc_pct, ctx.character);
    let etc_46 = ctx.decoder.etc(46, ctx.character);
    let all_stat = ctx.all_stat_pct();
    let pristine = ctx.decoder.pristine(config.pristine);
    let (star_flat, star_pct) = ctx.star_sign_stat(name);
    let pct_bubble = config.pct_bubble.map_or(0.0, |key| ctx.bubble_total(key));
    let pct_sum = pct_talent + etc_pct + etc_46 + all_stat + pristine + star_pct + pct_bubble;
    let pct_mult = 1.0 + pct_sum / 100.0;

    // Flat base.
    let flat_talent = ctx.talent(config.flat_talent);
    let guild_talent = ctx.talent_best(config.guild_talent);
    let stamp_base = ctx.stamp(config.stamp_base);
    let box_base = config.box_base.map_or(0.0, |key| ctx.box_reward(key));
    let etc_flat = ctx.decoder.etc(config.etc_flat, ctx.character);
    let shimmer = ctx.option(config.ola_shimmer) * dream_shimmer(ctx);
    let mut flat_base = flat_talent + guild_talent + stamp_base + etc_flat;
    if stat == Stat::Luk {
        flat_base += box_base + shimmer;
    }

    // Flat add.
    let mut extras = 0.0;
    for talent in config.extra_talents {
        extras += ctx.talent(*talent);
    }
    if let Some(talent) = config.extra_tab2 {
        extras += ctx.talent_tab(talent, 2);
    }
    let buff = config.buff.map_or(0.0, |(id, _)| {
        let active = fields_active(ctx, id);
        if active { ctx.talent(id) } else { 0.0 }
    });
    let box_stat = ctx.box_reward(config.box_stat);
    let family = ctx
        .family_bonuses()
        .get(&config.family)
        .copied()
        .unwrap_or(0.0);
    let quests_618 = if config.quests_618 {
        ctx.decoder.quest_total().min(ctx.talent(618))
    } else {
        0.0
    };
    let tal620_raw = ctx.talent(620);
    let tal620 = if tal620_raw > 0.0 {
        let mut max: f64 = 0.0;
        for character in 0..ctx.decoder.characters().len() {
            max = max.max(
                ctx.decoder
                    .characters()
                    .get(character)
                    .map_or(0.0, |row| row.class_level()),
            );
        }
        tal620_raw.min((max / 10.0).floor())
    } else {
        0.0
    };
    let card = ctx.card_bonus(config.card);
    let flat_talent2 = config.flat_talent2.map_or(0.0, |id| ctx.talent(id));
    let stamp_all = ctx.stamp("BaseAllStat");
    let shared = (ctx.box_reward("20a") + ctx.guild(1)).floor();
    let sigil = ctx.sigil(config.sigil);
    let a4 = ctx.bubble_total(config.a4_bubble);
    let shiny = ctx.decoder.shiny_bonus(config.shiny as u32);
    let arcade = ctx.decoder.arcade(config.arcade);
    let owl = ctx.owl(5);

    let mut flat_add = extras
        + buff
        + shimmer
        + box_stat
        + family
        + star_flat
        + quests_618
        + tal620
        + card
        + flat_talent2
        + stamp_all
        + shared
        + sigil
        + a4
        + shiny
        + arcade
        + owl;
    if stat == Stat::Luk {
        flat_add -= shimmer;
    }

    // Top level.
    let total_bubble = ctx.bubble_total(config.total_bubble);
    let tal652 = ctx.talent(652);
    let comp8 = ctx.decoder.companion(8);
    let top = total_bubble + tal652 + comp8;

    let inner = pct_mult * (dn + dn2 + flat_base + flat_add);
    let computed = (top + inner).floor();

    let tree = FormulaNode::new(format!("Total {name}"), computed).with_children([
        FormulaNode::new("Top Level", top).with_children([
            FormulaNode::new(
                format!("Bubble: {}", bubble_name(ctx, config.total_bubble)),
                total_bubble,
            ),
            FormulaNode::new("Talent: Stat Overload (Stat Overload)", tal652),
            FormulaNode::new("Companion: Sandy Pot", comp8),
        ]),
        FormulaNode::new("Total Stat Multiplier", pct_mult).with_children([
            FormulaNode::new("Talent: stat percentage", pct_talent),
            FormulaNode::new(format!("Combined Item Sources: {name}"), etc_pct),
            FormulaNode::new("Combined Item Sources: ALL STATS", etc_46),
            FormulaNode::new("All Stats Percentage", all_stat),
            FormulaNode::new("Pristine", pristine),
            FormulaNode::new(format!("Star Signs: Total {name}"), star_pct),
            FormulaNode::new("Bubble: stat percentage", pct_bubble),
        ]),
        FormulaNode::new("Equipment and Obol Stats", dn + dn2).with_children([
            FormulaNode::new(format!("Equipment Base {name}"), equip_base),
            FormulaNode::new(format!("Gallery Base {name}"), gallery_base),
            FormulaNode::new("Equipment Stat Multiplier", equip_pct),
            FormulaNode::new(format!("Obol Base {name}"), obol_base),
            FormulaNode::new("Obol Stat Multiplier", obol_pct),
        ]),
        FormulaNode::new("Flat Base", flat_base).with_children([
            FormulaNode::new("Talent: flat base", flat_talent),
            FormulaNode::new("Talent: guild (Account Best)", guild_talent),
            FormulaNode::new(format!("Stamps: Base {name}"), stamp_base),
            FormulaNode::new("Combined Item Sources: flat", etc_flat),
        ]),
        FormulaNode::new("Flat Add", flat_add).with_children([
            FormulaNode::new("Extra Talents", extras),
            FormulaNode::new("Buff Bonus", buff),
            FormulaNode::new("Equinox Shimmer Bonus", shimmer),
            FormulaNode::new(format!("Box Rewards: {name}"), box_stat),
            FormulaNode::new(format!("Family Bonus: {name}"), family),
            FormulaNode::new(format!("Star Signs: Base {name}"), star_flat),
            FormulaNode::new("Talent: 618 Quest Scaling", quests_618),
            FormulaNode::new("Talent: 620", tal620),
            FormulaNode::new("Cards", card),
            FormulaNode::new("Talent: flat add", flat_talent2),
            FormulaNode::new("Stamps: Base All Stats", stamp_all),
            FormulaNode::new("Shared Base Stats", shared),
            FormulaNode::new("Sigil", sigil),
            FormulaNode::new("Bubble: slab", a4),
            FormulaNode::new("Breeding: shiny", shiny),
            FormulaNode::new("Arcade", arcade),
            FormulaNode::new("Owl: All Stats", owl),
        ]),
        FormulaNode::new("Computed Total", computed),
    ]);

    Total {
        stat,
        computed,
        tree,
        missing: ctx.missing.clone(),
    }
}

/// The equipment base stat: the equipped items' raw stat plus the saved affix.
fn equip_base(ctx: &mut Ctx<'_, '_>, stat: &str) -> f64 {
    let Some(order) = fields_json(ctx, &format!("EquipOrder_{}", ctx.character)) else {
        return 0.0;
    };
    let gear = fields_json(ctx, &format!("EMm0_{}", ctx.character));
    let tools = fields_json(ctx, &format!("EMm1_{}", ctx.character));
    let gallery = ctx.decoder.gallery_on(ctx.character);
    let premhat = ctx.decoder.premhat_on(ctx.character);
    let capevine = ctx.decoder.capevine_on(ctx.character);
    let mut total = 0.0;
    for (row, slots) in [(0usize, 16usize), (1, 8)] {
        let Some(order) = at_index(&order, row) else {
            continue;
        };
        // `EMm<row>_<char>` is the row's flat slot map, indexed by slot.
        let emm = match row {
            0 => gear.as_ref(),
            _ => tools.as_ref(),
        };
        for slot in 0..slots {
            if row == 0 && gallery && (slot == 10 || slot == 14) {
                continue;
            }
            if row == 0 && premhat && slot == 8 {
                continue;
            }
            if row == 0 && capevine && slot == 12 {
                continue;
            }
            let Some(name) = at_index(order, slot).and_then(|value| value.as_str()) else {
                continue;
            };
            if name.is_empty() || name == "Blank" {
                continue;
            }
            let base = ctx.data.item_stat(name, stat);
            // The affix rows are written as arrays or numeric-keyed objects.
            let stone = emm
                .and_then(|row| at_index(row, slot))
                .and_then(|value| value.get(stat))
                .and_then(idleon_save::json::number)
                .unwrap_or(0.0);
            total += base + stone;
        }
    }
    total
}

/// The gallery base stat: the trophy, nametag, hatrack and Cape Vine pools.
pub(crate) fn gallery_base_here(ctx: &mut Ctx<'_, '_>, stat: &str) -> f64 {
    gallery_base(ctx, stat)
}

fn gallery_base(ctx: &mut Ctx<'_, '_>, stat: &str) -> f64 {
    let character = ctx.character;
    let gallery = ctx.decoder.gallery_on(character);
    let premhat = ctx.decoder.premhat_on(character);
    let capevine = ctx.decoder.capevine_on(character);
    let mut total = 0.0;
    if gallery {
        let multiplier = ctx.decoder.gallery_bonus_multi(character);
        let spelunk = fields_json(ctx, "Spelunk");
        if let Some(slots) = spelunk
            .as_ref()
            .and_then(|v| at_index(v, 16))
            .and_then(|v| v.as_array())
        {
            for (index, slot) in slots.iter().enumerate() {
                let id = slot.as_f64().unwrap_or(0.0);
                if id < 1.0 {
                    continue;
                }
                let value = item_row(ctx, &format!("Trophy{}", id as usize), stat);
                if value != 0.0 {
                    total += ctx.decoder.trophy_tier(index) * multiplier * value;
                }
            }
        }
        if let Some(levels) = number_row(ctx, "Spelunk", 17) {
            for (index, level) in levels.iter().enumerate() {
                let level = level.unwrap_or(0.0);
                if level < 1.0 {
                    continue;
                }
                let key = if index == 6 {
                    "EquipmentNametag6b".to_string()
                } else {
                    format!("EquipmentNametag{index}")
                };
                let value = item_row(ctx, &key, stat);
                if value == 0.0 {
                    continue;
                }
                let tier = idleon_account_data_nametag_tier((level as usize - 1).min(4));
                total += tier * multiplier * value;
            }
        }
    }
    let mut hatrack_total = 0.0;
    if premhat || capevine {
        let multiplier = ctx.decoder.hatrack_bonus_multi();
        if let Some(hats) = string_row(ctx, "Spelunk", 46) {
            for hat in hats {
                hatrack_total += multiplier * item_row(ctx, &hat, stat);
            }
        }
    }
    if premhat {
        total += hatrack_total;
    }
    if capevine {
        let capes = string_row(ctx, "Spelunk", 54).unwrap_or_default();
        if let Some(cape) = capes.last()
            && matches!(
                stat,
                "Weapon_Power" | "STR" | "AGI" | "WIS" | "LUK" | "Defence"
            )
        {
            let base = item_row(ctx, cape, stat);
            let multi = ctx.decoder.capevine_bonus_multi(capes.len() - 1);
            total += hatrack_total + multi * base;
        }
    }
    total
}

/// `NAMETAG_TIER_SCALE`.
fn idleon_account_data_nametag_tier(index: usize) -> f64 {
    [1.0, 1.6, 2.0, 2.3, 2.5].get(index).copied().unwrap_or(1.0)
}

fn item_row(ctx: &Ctx<'_, '_>, item: &str, stat: &str) -> f64 {
    ctx.data.item_stat(item, stat)
}

fn at_index(value: &serde_json::Value, index: usize) -> Option<&serde_json::Value> {
    match value {
        serde_json::Value::Array(rows) => rows.get(index),
        serde_json::Value::Object(rows) => rows.get(&index.to_string()),
        _ => None,
    }
}

fn number_row(ctx: &Ctx<'_, '_>, key: &str, index: usize) -> Option<Vec<Option<f64>>> {
    let value = fields_json(ctx, key)?;
    idleon_save::fields::values(at_index(&value, index)?)
}

fn string_row(ctx: &Ctx<'_, '_>, key: &str, index: usize) -> Option<Vec<String>> {
    let value = fields_json(ctx, key)?;
    at_index(&value, index)?.as_array().map(|cells| {
        cells
            .iter()
            .map(|cell| cell.as_str().map(str::to_owned).unwrap_or_default())
            .collect()
    })
}

/// The obol base stat: personal and family obols plus their saved rolls.
fn obol_base(ctx: &mut Ctx<'_, '_>, stat: &str) -> f64 {
    let personal = obols(
        ctx,
        Some(format!("ObolEqO0_{}", ctx.character)),
        Some(format!("ObolEqMAP_{}", ctx.character)),
        stat,
    );
    let family = obols(
        ctx,
        Some("ObolEqO1".to_string()),
        Some("ObolEqMAPz1".to_string()),
        stat,
    );
    personal + family
}

fn obols(ctx: &mut Ctx<'_, '_>, names: Option<String>, maps: Option<String>, stat: &str) -> f64 {
    let (Some(names), Some(maps)) = (names, maps) else {
        return 0.0;
    };
    let (Some(names), Some(maps)) = (fields_json(ctx, &names), fields_json(ctx, &maps)) else {
        return 0.0;
    };
    let Some(names) = names.as_array() else {
        return 0.0;
    };
    let mut total = 0.0;
    for (slot, name) in names.iter().enumerate() {
        let Some(name) = name.as_str() else {
            continue;
        };
        if name.is_empty() || name == "Blank" || name == "Null" {
            continue;
        }
        let base = ctx.data.item_stat(name, stat);
        let rolled = at_index(&maps, slot)
            .and_then(|value| value.get(stat))
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(0.0);
        let value = base + rolled;
        if value > 0.0 {
            total += value;
        }
    }
    total
}

/// The Dream shimmer: the Sailing artifact 31 tier's clamped multiplier.
fn dream_shimmer(ctx: &mut Ctx<'_, '_>) -> f64 {
    let tier = fields_json(ctx, "Sailing")
        .and_then(|value| value.as_array().and_then(|rows| rows.get(3)).cloned())
        .and_then(|row| row.get(31).and_then(serde_json::Value::as_f64))
        .unwrap_or(0.0);
    if tier <= 0.0 {
        1.0
    } else {
        (1.0 + tier).clamp(1.0, 4.0)
    }
}

/// The bubble's display name, as the tree labels it.
fn bubble_name(ctx: &Ctx<'_, '_>, key: &str) -> String {
    ctx.data
        .bubble(key)
        .map(|bubble| bubble.name.replace('_', " "))
        .unwrap_or_else(|| key.to_string())
}

fn fields_json(ctx: &Ctx<'_, '_>, key: &str) -> Option<serde_json::Value> {
    idleon_save::fields::field(ctx.save(), key)
}

/// Whether `BuffsActive_<char>` lists the buff id.
fn fields_active(ctx: &Ctx<'_, '_>, id: usize) -> bool {
    let Some(row) = idleon_save::fields::char_field(ctx.save(), "BuffsActive", ctx.character)
        .and_then(|value| value.as_array().cloned())
    else {
        return false;
    };
    row.iter().any(|buff| {
        let slot = buff.get(0).or_else(|| buff.get("0"));
        slot.and_then(idleon_save::json::number) == Some(id as f64)
    })
}
