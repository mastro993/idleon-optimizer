//! The farming breakdown behind one statue's row.
//!
//! Ported from `statue-calc.html`'s "EXP multipliers & calculation details"
//! breakdown (the STARTUE EXP bubble and its retention, the shared Onyx/Zenith
//! tier multiplier, each statue's saved Royal Guardian Flair and the Skinny
//! 0 Cal vial) plus the table's own statues-needed, rate and time columns, so
//! every number the row shows can be traced to the terms that produce it.

use super::{Estimate, Leveling, StatueSave, hours, info, target_cost};
use idleon_common::formula::{FormulaNode, ValueFormat};

/// One statue's farmed target: what it costs, what a deposit gives, and the
/// time the entered raw statues/hour rate takes.
///
/// An unavailable row still returns a root, so the breakdown names the reason
/// instead of showing a total the table does not claim.
pub fn breakdown_tree(
    save: &StatueSave,
    character: usize,
    statue: usize,
    target: Option<u32>,
    estimate: Estimate,
    rate: f64,
) -> FormulaNode {
    let Some(info) = info(statue) else {
        return FormulaNode::new("Unknown statue", 0.0)
            .with_note("This index is not one of the game's statues.");
    };
    let leveling: &Leveling = save.leveling();
    let Some(target) = target else {
        return FormulaNode::new(format!("{} · unavailable", info.name), 0.0)
            .with_note("This save has no readable row for the statue.");
    };
    let cost = match target_cost(save, leveling, character, statue, target) {
        Ok(cost) => cost,
        Err(error) => {
            return FormulaNode::new(format!("{} · unavailable", info.name), 0.0)
                .with_note(error.label(statue));
        }
    };
    let statues = estimate.statues(&cost);
    let needed = FormulaNode::leaf("EXP needed", cost.exp_needed, ValueFormat::Raw)
        .with_input(format!(
            "{} bars from Lv {} to Lv {}",
            target.saturating_sub(cost.level),
            cost.level,
            target
        ))
        .with_source("Full EXP bars with the saved progress removed")
        .with_children([
            FormulaNode::leaf(
                "STARTUE EXP bubble",
                leveling.bubble_pct,
                ValueFormat::Percent,
            )
            .with_input("Prisma bonus included")
            .with_source("Alchemy bubble · StatueStartEXP"),
            FormulaNode::leaf(
                "EXP retention",
                leveling.retention_pct,
                ValueFormat::Percent,
            )
            .with_input("1 − bubble, capped at 50%")
            .with_note(
                "Retention lowers the EXP a level consumes; the final bar still triggers it.",
            ),
        ]);
    let deposit = FormulaNode::leaf(
        "EXP per deposited statue",
        estimate.deposit(&cost),
        ValueFormat::Raw,
    )
    .with_input("tier × Flair × vial outcome")
    .with_children([
        FormulaNode::leaf(
            "Onyx / Zenith tier",
            leveling.tier_multiplier,
            ValueFormat::Multiplier,
        )
        .with_input(format!(
            "1 + 10% × {} Onyx+ + 15% × {} Zenith",
            leveling.onyx, leveling.zenith
        ))
        .with_source("Saved StuG tiers while OptLacc 69 is 2 or higher"),
        FormulaNode::leaf(
            "Royal Guardian Flair",
            cost.flair_multiplier,
            ValueFormat::Multiplier,
        )
        .with_input(format!("Flair level {}", cost.flair_level))
        .with_source("RoyalG · this statue's saved Flair"),
        FormulaNode::leaf(
            "Skinny 0 Cal vial",
            estimate.vial(leveling.double_chance),
            ValueFormat::Multiplier,
        )
        .with_input(format!(
            "{:.1}% double chance",
            leveling.double_chance * 100.0
        ))
        .with_note(match estimate {
            Estimate::Guaranteed => "A chance below 100% adds no guaranteed double.",
            Estimate::Average => "The mean deposit over the vial's double chance.",
        }),
    ]);
    let hourly = FormulaNode::leaf("Raw statues per hour", rate, ValueFormat::Rate).with_source(
        if rate > 0.0 {
            "Manual entry"
        } else {
            "No rate entered"
        },
    );
    let time = FormulaNode::leaf(
        "Time to target (hours)",
        hours(statues, rate),
        ValueFormat::Raw,
    )
    .with_input(format!("{statues} ÷ raw statues/hr"))
    .with_note(if rate > 0.0 {
        "Assumes the entered rate is kept until the target is met."
    } else {
        "Enter a raw statues/hour rate to estimate the time."
    });

    FormulaNode::leaf(
        format!("{} · statues needed", info.name),
        statues as f64,
        ValueFormat::Count,
    )
    .with_input(format!(
        "Lv {} → Lv {} · {}",
        cost.level,
        target,
        estimate.label()
    ))
    .with_source(save.name(character))
    .with_note(info.effect.clone())
    .with_children([needed, deposit, hourly, time])
}
