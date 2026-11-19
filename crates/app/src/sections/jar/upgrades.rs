//! The Upgrades tab: the jar schematics, the jar-type costs and the plan for the
//! next type the rupie balances can pay for. The schematics and the plan are
//! read once per applied save ([`super::Controls::sync`]).

use super::table::{Col, table, text};
use crate::sections::jar::{metric, row, tag};
use crate::state::AppState;
use gpui::prelude::*;
use gpui::{AnyElement, App, Context, FontWeight, IntoElement, ParentElement, Styled, div};
use idleon_jar::Jar;
use idleon_jar::{GILDED_TIERS, Schematic, TYPE_NAMES, UpgradePlan, UpgradeStatus};
use idleon_ui::format;
use idleon_ui::widgets::{Caption, Card, CardHeader, TEXT_BODY, Tone};

pub(crate) fn upgrades(state: &AppState, cx: &mut Context<AppState>) -> AnyElement {
    let jar = state
        .jar
        .as_ref()
        .expect("the section renders its import prompt without a save");
    let figures = state.jar_controls.figures();
    div()
        .id("jar-upgrades")
        .flex()
        .flex_col()
        .gap_3()
        .child(summary(jar, &figures.upgrade_plan))
        .child(schematics(&figures.schematics, cx))
        .child(type_costs(&figures.upgrade_plan, cx))
        .child(gilded(jar, cx))
        .into_any_element()
}

/// The next purchase and the budget it must fit in.
fn summary(jar: &Jar, plan: &UpgradePlan) -> AnyElement {
    let mut row = row("jar-upgrade-summary").child(metric(
        "jar-upgrade-types",
        "Jar types",
        format!("{} / {}", jar.types_available(), TYPE_NAMES.len()),
        "The base jar is free; the other nine are bought with the rupie below them".to_owned(),
        Tone::Accent,
    ));
    match plan.next() {
        Some(next) => {
            let affordable = next.affordable();
            row = row
                .child(metric(
                    "jar-upgrade-next",
                    "Next type",
                    format!("{} (#{})", next.name, next.jar_type),
                    "The only type that can be bought now".to_owned(),
                    Tone::Neutral,
                ))
                .child(metric(
                    "jar-upgrade-cost",
                    "Cost",
                    format!("{} {}", format::number(next.cost), next.currency_name),
                    format!("Paid with {} rupies", next.currency_name),
                    Tone::Info,
                ))
                .child(metric(
                    "jar-upgrade-budget",
                    "Balance",
                    format!("{} {}", format::number(next.balance), next.currency_name),
                    "The balance the plan spends from".to_owned(),
                    Tone::Neutral,
                ))
                .child(metric(
                    "jar-upgrade-status",
                    "Can afford",
                    match affordable {
                        true => "Yes".to_owned(),
                        false => format!("Short by {}", format::number(next.shortfall())),
                    },
                    match affordable {
                        true => "The next type fits the budget".to_owned(),
                        false => "Produce more of this rupie before buying the type".to_owned(),
                    },
                    match affordable {
                        true => Tone::Success,
                        false => Tone::Danger,
                    },
                ));
        }
        None => {
            row = row.child(metric(
                "jar-upgrade-max",
                "Upgrade plan",
                "Max tier".to_owned(),
                format!(
                    "Every jar type is owned; {} rupies are banked",
                    format::number(jar.rupies(TYPE_NAMES.len() - 1))
                ),
                Tone::Success,
            ));
        }
    }
    row.into_any_element()
}

/// The schematic tables, one per group.
fn schematics(rows: &[Schematic], cx: &App) -> AnyElement {
    let mut groups: Vec<(&str, Vec<&Schematic>)> = Vec::new();
    for row in rows {
        match groups.last_mut() {
            Some((group, items)) if *group == row.group => items.push(row),
            _ => groups.push((row.group, vec![row])),
        }
    }
    let body = div()
        .id("jar-schematic-groups")
        .flex()
        .flex_col()
        .gap_3()
        .children(
            groups
                .into_iter()
                .enumerate()
                .map(|(index, (group, entries))| {
                    Card::new()
                        .header(CardHeader::new(group))
                        .child(table(
                            format!("jar-schematics-{index}"),
                            &[
                                Col::Text("Schematic"),
                                Col::Text("Effect"),
                                Col::Text("Status"),
                            ],
                            entries.into_iter().map(schematic_row),
                            cx,
                        ))
                        .into_any_element()
                }),
        )
        .child(Caption::new(
            "Schematics unlock by playing the cavern; the save records which ones are owned.",
        ));
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .text_size(TEXT_BODY)
                .font_weight(FontWeight::SEMIBOLD)
                .child("Jar schematics"),
        )
        .child(body)
        .into_any_element()
}

fn schematic_row(row: &Schematic) -> Vec<AnyElement> {
    let (label, tone) = match row.owned {
        true => ("Owned", Tone::Success),
        false => ("Locked", Tone::Warning),
    };
    vec![
        text(row.name),
        text(row.description),
        tag(label, tone).into_any_element(),
    ]
}

/// The nine jar-type purchases, with the balance each one is checked against.
fn type_costs(plan: &UpgradePlan, cx: &App) -> AnyElement {
    let cols = [
        Col::Number("#"),
        Col::Text("Type"),
        Col::Number("Cost"),
        Col::Text("Currency"),
        Col::Number("Balance"),
        Col::Text("Status"),
    ];
    let rows = plan.steps.iter().map(|step| {
        let status = match step.status {
            UpgradeStatus::Owned => "Owned".to_owned(),
            UpgradeStatus::Affordable => "Affordable".to_owned(),
            UpgradeStatus::Shortfall => format!("Short by {}", format::number(step.shortfall())),
            UpgradeStatus::Locked => "Locked".to_owned(),
        };
        vec![
            text(step.jar_type.to_string()),
            text(step.name),
            text(format::number(step.cost)),
            text(step.currency_name),
            text(format::number(step.balance)),
            tag(status, status_tone(step.status)).into_any_element(),
        ]
    });
    div()
        .id("jar-type-plan")
        .child(
            Card::new()
                .header(
                    CardHeader::new("Jar type costs and upgrade plan").description(
                        "Only the next type can be bought, and it is marked affordable only when \
                         its rupie balance covers the cost",
                    ),
                )
                .child(table("jar-type-costs", &cols, rows, cx))
                .child(Caption::new(format!(
                    "The plan spends {} of the {} rupie balance now.",
                    format::number(plan.spent()),
                    format::number(plan.budget())
                ))),
        )
        .into_any_element()
}

fn status_tone(status: UpgradeStatus) -> Tone {
    match status {
        UpgradeStatus::Owned => Tone::Success,
        UpgradeStatus::Affordable => Tone::Info,
        UpgradeStatus::Shortfall => Tone::Danger,
        UpgradeStatus::Locked => Tone::Neutral,
    }
}

/// The six gilded tiers and their multipliers.
fn gilded(jar: &Jar, cx: &App) -> AnyElement {
    let rows = (0..GILDED_TIERS).map(|tier| {
        vec![
            text(tier.to_string()),
            text(format!(
                "+{}%",
                format::number(Jar::doubler_tier_value(tier))
            )),
            text(format!("{:.2}×", Jar::doubler_multiplier(tier))),
        ]
    });
    let doubler = jar.doubler_bonus();
    div()
        .id("jar-gilded-tiers")
        .child(
            Card::new()
                .header(
                    CardHeader::new("Gilded tier values")
                        .description("What a Gilded jar's doubler is worth at each tier"),
                )
                .child(table(
                    "jar-gilded-table",
                    &[
                        Col::Number("Tier"),
                        Col::Number("Gilded %"),
                        Col::Number("Multiplier"),
                    ],
                    rows,
                    cx,
                ))
                .child(Caption::new(match doubler > 0.0 {
                    true => format!(
                        "The save's gilded bonus is +{}% and applies to the next rupie break.",
                        format::number(doubler)
                    ),
                    false => {
                        "No gilded bonus is banked; break a Gilded jar to start one.".to_owned()
                    }
                })),
        )
        .into_any_element()
}
