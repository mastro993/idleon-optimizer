//! Fishing: AFK catches, the fish split and real-versus-panel EXP per pool.
//!
//! Ported from the upstream `js/stats/systems/w2/fishing.js`,
//! `js/stats/data/w2/fishing.js` and `fishing-calc.html`. The pure formulas in
//! [`math`] reproduce the upstream chain exactly; [`evaluate`] resolves one
//! character, toolkit and location into the same outcome the upstream evaluator
//! returns.
//!
//! The upstream evaluator derives its account inputs from every skill system
//! (efficiency, EXP multiplier, AFK rate, Multi-Fish, food speed, depths). This
//! port has not decoded those systems yet, so [`AccountInputs`] carries them as
//! explicit values: a value the user reads from the game gives the exact
//! upstream outcome, and leaving a value at its neutral default gives a lower
//! bound. Only the character slice — level, rod, saved toolkit, saved spot and
//! AFK target — comes from the save.

pub mod data;
pub mod math;
pub mod rank;
pub mod save;

pub use data::{FishingData, Location, Pool, Rod, ToolkitOption, ToolkitStats, data};
pub use math::{
    Depths, MultiFish, MultiFishInputs, SpeedInputs, catch_shares, catches_per_hour, depths,
    multi_fish, ore_value, speed,
};
pub use rank::{Objective, rank, ties};
pub use save::{Character, FishingSave};

use crate::formula::{FormulaNode, ValueFormat};

/// The account-side values the upstream evaluator derives from the skill
/// systems. They describe the selected lure + line; changing the toolkit does
/// not recompute them.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AccountInputs {
    /// `SkillStats("FishingEfficiency")`, as the game's Skills panel shows it.
    pub efficiency: f64,
    /// `SkillStats("FishingExpMulti")`; 1 is neutral.
    pub exp_multi: f64,
    /// `AFKgainrates("Fishing")`, a fraction; the upstream minimum is 0.01.
    pub afk_rate: f64,
    /// Multi-Fish percentage.
    pub multi_fish: f64,
    /// The additive speed bonus from food, cards, talents, the lab and etc.
    pub speed_bonus: f64,
    /// The attack loadout factor the game applies to hourly kills.
    pub attack_factor: f64,
    /// The exponent bonus `FishingOreValue` adds (`0.25 + prowess`).
    pub prowess: f64,
    /// The equipment depth bonus `EtcBonuses(12..14)`.
    pub equipment_depth: [f64; 3],
}

impl Default for AccountInputs {
    fn default() -> Self {
        Self {
            efficiency: 0.0,
            exp_multi: 1.0,
            afk_rate: 0.01,
            multi_fish: 0.0,
            speed_bonus: 0.0,
            attack_factor: 1.0,
            prowess: 0.0,
            equipment_depth: [0.0; 3],
        }
    }
}

impl AccountInputs {
    /// The catches per hour and catch time the character reaches with this
    /// toolkit.
    pub fn speed(&self, character: &Character, toolkit: &ToolkitStats) -> (f64, f64) {
        let speed_bonus = self.speed_bonus + toolkit.speed;
        let time = speed(&SpeedInputs {
            efficiency: self.efficiency,
            level: f64::from(character.level),
            rod_speed: character.rod_speed,
            speed_bonus_pct: speed_bonus,
        });
        (time, catches_per_hour(time, self.attack_factor))
    }
}

/// One fish type's row in an outcome.
#[derive(Clone, Debug, PartialEq)]
pub struct FishRow {
    pub slot: usize,
    pub item_key: String,
    pub name: String,
    pub exp_per_catch: f64,
    pub share: f64,
    pub active: bool,
    pub unlocked: bool,
    pub eff_req: f64,
    pub count: f64,
    pub exp: f64,
    pub credited: bool,
    pub overwritten: bool,
    pub credited_count: f64,
}

/// The outcome of one AFK claim at one pool.
#[derive(Clone, Debug)]
pub struct PoolOutcome {
    pub pool: &'static Pool,
    pub hours: f64,
    pub shares: [f64; 4],
    pub share_total: f64,
    pub ore_value: f64,
    pub catches: f64,
    pub fish_per_catch: f64,
    pub fish_total: f64,
    pub rolled_fish_total: f64,
    pub overwritten_fish: f64,
    pub exp: f64,
    pub exp_per_catch: f64,
    pub fish: Vec<FishRow>,
    pub panel_catches_per_hour: f64,
    pub panel_exp_per_hour: f64,
    pub panel_ore_value: f64,
}

/// The inputs one pool outcome reads, matching the upstream `fishingPoolOutcome`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PoolInputs {
    pub efficiency: f64,
    pub depths: Depths,
    pub prowess: f64,
    pub catches_per_hour: f64,
    pub afk_rate: f64,
    pub multi_fish: f64,
    pub exp_multi: f64,
}

/// One pool's AFK claim: the upstream `fishingPoolOutcome`.
pub fn pool_outcome(pool: &'static Pool, inputs: &PoolInputs, hours: f64) -> PoolOutcome {
    let hours = if hours > 0.0 {
        (hours * 3600.0).round() / 3600.0
    } else {
        1.0
    };
    let shares = catch_shares(inputs.efficiency, &inputs.depths, pool.eff_reqs);
    let ore = ore_value(inputs.efficiency, pool.eff_reqs[2], inputs.prowess);
    let catches = hours * inputs.catches_per_hour * inputs.afk_rate;
    let fish_per_catch = (1.0 + inputs.multi_fish / 100.0) * ore;
    let mut exp = 0.0;
    let mut rolled_fish_total = 0.0;
    // The claim writes AFKrewMapz[itemKey] = count, not +=, for each positive
    // type: fish types sharing an item keep only the last positive count.
    let mut credited_slot: Vec<(&str, usize)> = Vec::new();
    let mut fish = Vec::with_capacity(pool.fish.len());
    for (tier, fish_type) in pool.fish.iter().enumerate() {
        let share = shares[tier];
        let count = (hours
            * inputs.catches_per_hour
            * inputs.afk_rate
            * (1.0 + inputs.multi_fish / 100.0)
            * ore
            * (share / 100.0))
            .floor()
            .max(0.0);
        let type_exp = if share > 0.0 {
            catches * fish_type.exp_per_catch * (share / 100.0) * inputs.exp_multi
        } else {
            0.0
        };
        exp += type_exp;
        rolled_fish_total += count;
        if count > 0.0 {
            match credited_slot
                .iter_mut()
                .find(|(key, _)| *key == fish_type.item_key)
            {
                Some(entry) => entry.1 = tier,
                None => credited_slot.push((&fish_type.item_key, tier)),
            }
        }
        fish.push(FishRow {
            slot: fish_type.slot,
            item_key: fish_type.item_key.clone(),
            name: fish_type.name.clone(),
            exp_per_catch: fish_type.exp_per_catch,
            share,
            active: share > 0.0,
            unlocked: tier == 0 || inputs.efficiency >= pool.eff_reqs[tier - 1],
            eff_req: if tier == 0 {
                0.0
            } else {
                pool.eff_reqs[tier - 1]
            },
            count,
            exp: type_exp,
            credited: false,
            overwritten: false,
            credited_count: 0.0,
        });
    }
    let mut fish_total = 0.0;
    for row in &mut fish {
        row.credited = row.count > 0.0
            && credited_slot
                .iter()
                .any(|(key, slot)| *key == row.item_key && *slot == row.slot);
        row.overwritten = row.count > 0.0 && !row.credited;
        row.credited_count = if row.credited { row.count } else { 0.0 };
        fish_total += row.credited_count;
    }
    if exp < 2e9 {
        exp = exp.floor();
    }
    let share_total: f64 = shares.iter().filter(|share| **share > 0.0).sum();
    let panel_catches_per_hour =
        (inputs.catches_per_hour * inputs.afk_rate * (1.0 + inputs.multi_fish / 100.0)).floor();
    let panel_exp_per_hour =
        inputs.catches_per_hour * inputs.afk_rate * pool.panel_exp_per_catch * inputs.exp_multi;
    PoolOutcome {
        pool,
        hours,
        shares,
        share_total,
        ore_value: ore,
        catches,
        fish_per_catch,
        fish_total,
        rolled_fish_total,
        overwritten_fish: rolled_fish_total - fish_total,
        exp,
        exp_per_catch: if catches > 0.0 {
            exp / (catches * inputs.exp_multi)
        } else {
            0.0
        },
        fish,
        panel_catches_per_hour,
        panel_exp_per_hour,
        panel_ore_value: (100.0 * ore).floor() / 100.0,
    }
}

/// One resolved claim: the character, toolkit, location and its outcome.
#[derive(Clone, Debug)]
pub struct Evaluation {
    pub character: Character,
    pub account: AccountInputs,
    pub location: &'static Location,
    pub lure: ToolkitOption,
    pub line: ToolkitOption,
    pub toolkit: ToolkitStats,
    pub depths: Depths,
    pub spot_bonus: [f64; 4],
    pub speed_bonus: f64,
    pub speed: f64,
    pub catches_per_hour: f64,
    pub attack_factor_used: f64,
    pub outcome: PoolOutcome,
}

/// Resolve one character, lure/line and location into the upstream outcome.
///
/// `lure` and `line` index the upstream toolkit lists; an out-of-range index
/// falls back to the first option, as the upstream does.
pub fn evaluate(
    save: &FishingSave,
    account: &AccountInputs,
    character: usize,
    lure: usize,
    line: usize,
    location: &str,
    hours: f64,
) -> Evaluation {
    let character = save
        .character(character)
        .cloned()
        .unwrap_or_else(|| Character {
            index: character,
            name: format!("Character {}", character + 1),
            ..Character::default()
        });
    let data = data();
    let lure_option = data.toolkit(0, lure).cloned().expect("fishing lures");
    let line_option = data.toolkit(1, line).cloned().expect("fishing lines");
    let toolkit = lure_option.stats.plus(line_option.stats);
    let spot = data
        .location(location)
        .or_else(|| character.saved_location())
        .unwrap_or_else(|| data.default_location());
    let depth = depths(&toolkit, account.equipment_depth, spot.bonus);
    let (catch_time, catches) = account.speed(&character, &toolkit);
    let pool = data
        .pool(&spot.pool_key)
        .expect("every fishing location names a bundled pool");
    let inputs = PoolInputs {
        efficiency: account.efficiency,
        depths: depth,
        prowess: account.prowess,
        catches_per_hour: catches,
        afk_rate: account.afk_rate,
        multi_fish: account.multi_fish,
        exp_multi: account.exp_multi,
    };
    Evaluation {
        character,
        account: *account,
        location: spot,
        lure: lure_option,
        line: line_option,
        toolkit,
        depths: depth,
        spot_bonus: spot.bonus,
        speed_bonus: account.speed_bonus + toolkit.speed,
        speed: catch_time,
        catches_per_hour: catches,
        attack_factor_used: if account.attack_factor.is_finite() {
            account.attack_factor.clamp(1.0, 3.0)
        } else {
            1.0
        },
        outcome: pool_outcome(pool, &inputs, hours),
    }
}

/// The shared formula tree for one resolved claim.
pub fn breakdown(evaluation: &Evaluation) -> FormulaNode {
    let outcome = &evaluation.outcome;
    let location = evaluation.location;
    let root_label = format!("{} · {}", location.label, outcome.pool.name);
    let catches = FormulaNode::new("AFK catches", outcome.catches)
        .with_format(ValueFormat::Count)
        .with_input("Catches/hr × AFK rate × hours")
        .with_multiplier(evaluation.account.afk_rate)
        .with_source("fishing.js · fishingPoolOutcome")
        .with_children([
            FormulaNode::new("Catches/hr", evaluation.catches_per_hour)
                .with_format(ValueFormat::Count)
                .with_input(format!(
                    "Catch time {:.3}s + 0.4s · attack ×{:.2}",
                    evaluation.speed, evaluation.attack_factor_used
                ))
                .with_source("HourlyKillRate(\"Fishing\")")
                .with_children([FormulaNode::new("Catch time (seconds)", evaluation.speed)
                    .with_input(format!(
                        "Level {} · rod speed {} · speed bonus {}%",
                        evaluation.character.level,
                        evaluation.character.rod_speed,
                        ValueFormat::Raw.format(evaluation.speed_bonus)
                    ))
                    .with_source("SkillStats(\"FishingSpeed\")")
                    .with_note(
                        "max(base time ± rod − level, 0.57) × 15 / (1 + speed bonus / 100)",
                    )]),
            FormulaNode::leaf(
                "AFK gains rate",
                evaluation.account.afk_rate,
                ValueFormat::Multiplier,
            )
            .with_input("Manual account value")
            .with_note("The upstream resolves AFKgainrates(\"Fishing\"); this port enters it."),
        ]);

    let fish_per_catch = FormulaNode::new("Fish per catch", outcome.fish_per_catch)
        .with_input("(1 + Multi-Fish / 100) × ore value")
        .with_multiplier(1.0 + evaluation.account.multi_fish / 100.0)
        .with_children([
            FormulaNode::leaf(
                "Multi-Fish",
                evaluation.account.multi_fish,
                ValueFormat::Percent,
            )
            .with_source("Manual account value"),
            FormulaNode::new("Fish per catch (ore value)", outcome.ore_value)
                .with_input(format!(
                    "({} / {})^(0.25 + {})",
                    ValueFormat::Raw.format(evaluation.account.efficiency),
                    ValueFormat::Raw.format(outcome.pool.eff_reqs[2]),
                    ValueFormat::Raw.format(evaluation.account.prowess)
                ))
                .with_source("SkillStats(\"FishingOreValue\")")
                .with_note("floor(max((efficiency / Fish 4 requirement)^(0.25 + prowess), 1))"),
        ]);

    let split = FormulaNode::group(
        "Fish split",
        ValueFormat::Percent,
        outcome.fish.iter().map(|fish| {
            let mut node = FormulaNode::new(
                format!("Fish {}: {} ({:.2}%)", fish.slot + 1, fish.name, fish.share),
                fish.share,
            )
            .with_format(ValueFormat::Percent);
            if fish.slot > 0 {
                node = node.with_note(format!(
                    "Needs {} efficiency{} · D{} {}",
                    ValueFormat::Raw.format(fish.eff_req),
                    if fish.unlocked { "" } else { " (locked)" },
                    fish.slot,
                    ValueFormat::Raw.format(evaluation.depths.at(fish.slot))
                ));
            }
            node
        }),
    )
    .with_input("Each tier requires efficiency ≥ its requirement")
    .with_source("fishingCatchShares");

    let fish = FormulaNode::group(
        "Fish",
        ValueFormat::Count,
        outcome.fish.iter().map(|fish| {
            let node = FormulaNode::new(
                format!(
                    "Fish {}: {}{}",
                    fish.slot + 1,
                    fish.name,
                    if fish.overwritten {
                        " — overwritten"
                    } else {
                        ""
                    }
                ),
                fish.credited_count,
            )
            .with_format(ValueFormat::Count);
            if fish.overwritten {
                node.with_children([FormulaNode::leaf(
                    "Rolled before overwrite",
                    fish.count,
                    ValueFormat::Count,
                )])
            } else {
                node
            }
        }),
    )
    .with_input("floor(hours × catches/hr × AFK rate × fish per catch × share)")
    .with_source("AFKrewMapz claim");

    let exp = FormulaNode::new("Fishing EXP", outcome.exp)
        .with_format(ValueFormat::Raw)
        .with_input("Σ AFK catches × fish EXP × share × EXP multiplier")
        .with_multiplier(evaluation.account.exp_multi)
        .with_source("FishPools EXP")
        .with_children(outcome.fish.iter().filter(|fish| fish.active).map(|fish| {
            FormulaNode::new(
                format!(
                    "Fish {}: {} EXP × {:.2}%",
                    fish.slot + 1,
                    fish.exp_per_catch,
                    fish.share
                ),
                fish.exp,
            )
        }));

    let panel = FormulaNode::new("Panel EXP/hr", outcome.panel_exp_per_hour)
        .with_format(ValueFormat::Rate)
        .with_input(format!(
            "{} catches/hr × {} panel EXP/catch",
            ValueFormat::Raw.format(outcome.panel_catches_per_hour),
            ValueFormat::Raw.format(outcome.pool.panel_exp_per_catch)
        ))
        .with_source("MonsterDefinitions[pool].ExpGiven")
        .with_note(format!(
            "Real EXP/hr ×{:.3} the panel: the panel omits Multi-Fish, ore value and per-fish EXP.",
            if outcome.panel_exp_per_hour > 0.0 {
                (outcome.exp / outcome.hours) / outcome.panel_exp_per_hour
            } else {
                0.0
            }
        ));

    FormulaNode::new(root_label, outcome.fish_total)
        .with_format(ValueFormat::Count)
        .with_input(format!(
            "Fish per {}h AFK claim",
            ValueFormat::Raw.format(outcome.hours)
        ))
        .with_source("fishing.js · fishingPoolOutcome")
        .with_children([catches, fish_per_catch, split, fish, exp, panel])
}

#[cfg(test)]
mod tests;
