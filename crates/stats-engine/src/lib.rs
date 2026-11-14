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
