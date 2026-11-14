//! The shared stat pipeline: the total STR/AGI/WIS/LUK pools the All Stats
//! dashboard and the damage layers both read, and the damage arithmetic on top
//! of them.
//!
//! Ported from `js/stats/systems/common/stats.js` (`computeTotalStat` and the
//! providers it sums) and `js/stats/defs/damage.js`. A provider whose upstream
//! formula this build does not carry is recorded by name and source instead of
//! being guessed, so a caller can show a partial result and its reason.

mod damage;
mod data;
pub mod stats;
mod systems;

use idleon_save::Save;

pub use damage::{Layers, evaluate as damage_layers, is_dungeon_map};
pub use data::data;
pub use stats::{Total, total_stat};

/// Whether a map has a monster the Death Bringer's kill count reads.
pub(crate) fn is_fighting_map(map: usize) -> bool {
    matches!(
        idleon_stats_data::afk_target(map),
        Some(target) if target != "Nothing" && target != "Z"
    )
}

/// One stat the engine totals.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stat {
    Str,
    Agi,
    Wis,
    Luk,
}

impl Stat {
    pub const ALL: [Stat; 4] = [Stat::Str, Stat::Agi, Stat::Wis, Stat::Luk];

    pub fn id(self) -> &'static str {
        match self {
            Stat::Str => "STR",
            Stat::Agi => "AGI",
            Stat::Wis => "WIS",
            Stat::Luk => "LUK",
        }
    }
}

/// The additive terms behind the Coin Multiplier's "Other Additive Sources"
/// group. The group itself lives in `js/stats/defs/coin-multi.js`; these are the
/// systems it sums.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct OtherSources {
    pub talent657: f64,
    pub vial: f64,
    pub etc3: f64,
    pub cards: f64,
    pub card_w5b1: f64,
    pub talent22: f64,
    pub flurbo: f64,
    pub arcade: f64,
    pub box_reward: f64,
    pub guild: f64,
    /// Talent 643's value per overkill tier.
    pub talent643: f64,
    /// Talent 644's value times the character's `Lv0[10] / 10`.
    pub talent644: f64,
    pub gold_food: f64,
    pub vault17: f64,
    pub achievements: f64,
    pub vault2: f64,
    pub vault14: f64,
    pub vault31: f64,
    pub ola420: f64,
    pub vault70: f64,
}

/// Resolve the systems the Coin Multiplier's "Other Additive Sources" group
/// sums. The map factor and the vault kill-count tiers are read here too, so a
/// caller only has to apply the overkill tier to [`OtherSources::talent643`].
pub fn other_sources(
    save: &Save,
    simulation: &idleon_research::model::Simulation,
    character: usize,
) -> Option<OtherSources> {
    with_ctx(save, simulation, character, |ctx| {
        let killz = idleon_account::vault_killz::vault_killz(ctx.decoder);
        let map = idleon_save::fields::char_field(ctx.save(), "CurrentMap", character)
            .and_then(|value| idleon_save::json::number(&value))
            .unwrap_or(0.0)
            .max(0.0) as usize;
        let guild_map = 1.0 + (map / 50) as f64;
        let guild_base = ctx.guild(8);
        let level10 = idleon_save::fields::char_field(ctx.save(), "Lv0", character)
            .and_then(|value| idleon_save::fields::values(&value))
            .map(|row| idleon_save::fields::number_at(&row, 10))
            .unwrap_or(0.0);
        let cards = idleon_save::fields::field(ctx.save(), "Cards0")
            .and_then(|value| value.as_object().cloned())
            .map(|cards| {
                cards
                    .values()
                    .filter(|level| idleon_save::json::number(level).unwrap_or(0.0) >= 1.0)
                    .count() as f64
            })
            .unwrap_or(0.0);
        OtherSources {
            talent657: ctx.talent(657),
            vial: ctx.vial("MonsterCash"),
            etc3: ctx.decoder.etc(3, character),
            cards: ctx.card_bonus(11),
            card_w5b1: 7.0 * ctx.decoder.card_level("w5b1"),
            talent22: ctx.talent(22),
            flurbo: ctx.flurbo_shop(4),
            arcade: ctx.decoder.arcade(10) + ctx.decoder.arcade(11),
            box_reward: ctx.box_reward("13c"),
            guild: guild_base * guild_map,
            talent643: ctx.talent(643),
            talent644: ctx.talent(644) * level10 / 10.0,
            gold_food: ctx.gold_food_bonus("MonsterCash"),
            vault17: ctx.decoder.vault(17) * idleon_common::curve::log(ctx.option(340)),
            achievements: 5.0 * ctx.achievement(235)
                + 10.0 * ctx.achievement(350)
                + 20.0 * ctx.achievement(376),
            vault2: ctx.decoder.vault(2),
            vault14: ctx.decoder.vault(14) * killz[4],
            vault31: ctx.decoder.vault(31) * killz[7],
            ola420: ctx.option(420),
            vault70: ctx.decoder.vault(70) * cards,
        }
    })
}

/// A decoded save plus the simulation state the providers read.
pub(crate) fn with_ctx<T>(
    save: &Save,
    simulation: &idleon_research::model::Simulation,
    character: usize,
    read: impl FnOnce(&mut systems::Ctx<'_, '_>) -> T,
) -> Option<T> {
    let (fields, root) = save.parts()?;
    let characters = idleon_save::characters(fields);
    let mut warnings = Vec::new();
    let mut decoder =
        idleon_account::Decoder::new(fields, root, &characters, simulation, &mut warnings);
    let mut ctx = systems::Ctx::new(&mut decoder, character);
    Some(read(&mut ctx))
}

/// The display name of an alchemy bubble by its effect key, title-cased the way
/// upstream's `label('Bubble', …)` renders it.
pub fn bubble_label(key: &str) -> Option<String> {
    let name = &data::data().bubble(key)?.name;
    Some(
        name.split('_')
            .map(|word| {
                let mut chars = word.chars();
                match chars.next() {
                    Some(first) => {
                        first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase()
                    }
                    None => String::new(),
                }
            })
            .collect::<Vec<_>>()
            .join(" "),
    )
}

/// The Cash Bubbles coin group: each bubble's value times `floor(stat / 250)`.
///
/// Ported from `js/stats/defs/coin-multi.js` group 1. The terms are returned
/// with the stat and bubble that produced them so the card can name its inputs.
pub fn cash_bubbles(
    save: &Save,
    simulation: &idleon_research::model::Simulation,
    character: usize,
) -> Option<Vec<(Stat, f64, f64)>> {
    with_ctx(save, simulation, character, |ctx| {
        let mut terms = Vec::new();
        for (stat, key) in [
            (Stat::Str, "CashSTR"),
            (Stat::Agi, "CashAGI"),
            (Stat::Wis, "CashWIS"),
        ] {
            let total = stats::compute_here(ctx, stat);
            // A group is only priced when every term it multiplies is
            // justified; a lower bound stays not modelled.
            if !total.is_complete() {
                return None;
            }
            let value = total.computed;
            let bubble = ctx.bubble(key);
            let part = bubble * (value / 250.0).floor();
            if part > 0.0 {
                terms.push((stat, value, part));
            }
        }
        Some(terms)
    })
    .flatten()
}
