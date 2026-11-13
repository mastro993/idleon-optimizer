//! The obstruction reward table: every `Research[45..47]` row, the account
//! system it feeds and whether the save has cleared it.
//!
//! Ports `jellyRewards`, `jellyRewardTarget`, `jellyRewardBonus`,
//! `gameDescription` and `rewardEffectText` from `jelly-operator-calc.html` and
//! `js/stats/data/w7/jelly-operator.js`.
//!
//! The game's reward text is written against the game's own number format and
//! two placeholders: `{`/`$` are the reward's value and `}` the multiplier it
//! buys. The effect column is the page's normalisation of that text, because the
//! 72 descriptions are not written in one consistent unit.

use super::{Jelly, fmt_val};

/// One obstruction reward as the Bonuses tab prints it.
#[derive(Clone, Debug, PartialEq)]
pub struct Bonus {
    pub index: usize,
    pub name: String,
    /// The account system the reward feeds.
    pub target: String,
    /// The game's own description with its placeholders filled.
    pub description: String,
    pub value: f64,
    /// Whether the obstruction that grants it has been cleared.
    pub active: bool,
    /// The reward in one line, as the page normalises it.
    pub effect: String,
}

impl Jelly {
    /// `jellyRewards`: every obstructions' reward, cleared or not.
    pub fn bonuses(&self) -> Vec<Bonus> {
        super::data()
            .rewards
            .iter()
            .enumerate()
            .map(|(index, reward)| Bonus {
                index,
                name: reward.name.clone(),
                target: reward.target.clone(),
                description: game_description(&reward.description, reward.value),
                value: reward.value,
                active: self.progress.obstruction > index,
                effect: effect_text(index, &reward.description, reward.value),
            })
            .collect()
    }

    /// Whether Operational Bonuses (upgrade 39) opens the reward interface.
    pub fn bonuses_unlocked(&self) -> bool {
        self.upgrade_level(39) >= 1.0
    }
}

/// `gameDescription`: the game text with `{`/`$` as the value, `}` as the
/// multiplier and `@` as a line break.
fn game_description(description: &str, value: f64) -> String {
    let amount = fmt_val(value);
    description
        .replace('}', &format!("{:.2}", 1.0 + value / 100.0))
        .replace(['{', '$'], &amount)
        .replace('@', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// `rewardEffectText`: the reward in one line, in the unit its text means.
fn effect_text(index: usize, description: &str, value: f64) -> String {
    let text = description.to_lowercase();
    let multiplier = || format!("{:.2}x", 1.0 + value / 100.0);
    if text.contains("nothing yet") {
        return "No effect".to_string();
    }
    match index {
        0 => return format!("{} Research EXP; +2 Jelly slots", multiplier()),
        1 => return format!("{} Rank EXP; +2 Jelly slots", multiplier()),
        8 => {
            return format!(
                "+{}% Crystal chance; {} Minehead currency",
                fmt_val(value),
                multiplier()
            );
        }
        _ => {}
    }
    if text.starts_with("unlocks") {
        return "Unlock".to_string();
    }
    const COUNTS: [&str; 9] = [
        "slot unlock",
        "golden tile",
        "weekly exotic",
        "regal mob",
        "research point",
        "talent pts",
        "cooking mastery",
        "max lv",
        "maximum possible roll",
    ];
    if COUNTS.iter().any(|needle| text.contains(needle)) {
        return format!("+{}", fmt_val(value));
    }
    if text.contains("0.01x higher") {
        return format!("+{:.2}x", value / 100.0);
    }
    if text.contains("0.1% chance") {
        return format!("+{:.1}%", value / 10.0);
    }
    if text.contains("}x") {
        return multiplier();
    }
    if text.contains('%') {
        return format!("+{}%", fmt_val(value));
    }
    format!("+{}", fmt_val(value))
}
