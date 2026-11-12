//! The Upgrades tab: the jar schematics, the jar-type costs and the plan for the
//! next type the rupie balances can pay for.

use crate::sections::jar::metric;
use crate::state::AppState;
use crate::ui::format;
use ely_gpui_component::data_display::{Badge, Tone};
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::ActiveTheme as _;
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, FontWeight, IntoElement, ParentElement, Styled, div, px};
use idleon_core::Jar;
use idleon_core::jar::{GILDED_TIERS, Schematic, TYPE_NAMES, UpgradePlan, UpgradeStatus};

pub fn upgrades(state: &AppState, cx: &mut Context<AppState>) -> AnyElement {
    let jar = state
        .jar
        .as_ref()
        .expect("the section renders its import prompt without a save");
    let plan = jar.upgrade_plan();
    div()
        .id("jar-upgrades")
        .flex()
        .flex_col()
        .gap_3()
        .child(summary(jar, &plan, cx))
        .child(schematics(jar, cx))
        .child(type_costs(&plan, cx))
        .child(gilded(jar, cx))
        .into_any_element()
}

/// The next purchase and the budget it must fit in.
fn summary(jar: &Jar, plan: &UpgradePlan, cx: &mut Context<AppState>) -> AnyElement {
    let mut row = div()
        .id("jar-upgrade-summary")
        .flex()
        .flex_wrap()
        .gap_3()
        .child(metric(
            "jar-upgrade-types",
            "Jar types",
            format!("{} / {}", jar.types_available(), TYPE_NAMES.len()),
            "The base jar is free; the other nine are bought with the rupie below them".to_owned(),
            Tone::Accent,
            cx,
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
                    cx,
                ))
                .child(metric(
                    "jar-upgrade-cost",
                    "Cost",
                    format!("{} {}", format::number(next.cost), next.currency_name),
                    format!("Paid with {} rupies", next.currency_name),
                    Tone::Info,
                    cx,
                ))
                .child(metric(
                    "jar-upgrade-budget",
                    "Balance",
                    format!("{} {}", format::number(next.balance), next.currency_name),
                    "The balance the plan spends from".to_owned(),
                    Tone::Neutral,
                    cx,
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
                    cx,
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
                cx,
            ));
        }
    }
    row.into_any_element()
}

/// `_renderUpgrades`' schematic tables, one per upstream group.
fn schematics(jar: &Jar, cx: &mut Context<AppState>) -> AnyElement {
    let rows = jar.schematics();
    let muted = cx.theme().colors.fg_muted;
    let mut groups: Vec<(&str, Vec<&Schematic>)> = Vec::new();
    for row in &rows {
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
        .children(groups.into_iter().map(|(group, entries)| {
            let mut table = Table::new(["Schematic", "Effect", "Status"]);
            for row in entries {
                table = table.row(schematic_row(row));
            }
            Card::new()
                .header(CardHeader::new(group))
                .child(table)
                .into_any_element()
        }))
        .child(
            Caption::new(
                "Schematics unlock by playing the cavern; the save records which ones are owned.",
            )
            .text_color(muted),
        );
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .text_size(px(13.0))
                .font_weight(FontWeight::SEMIBOLD)
                .child("Jar schematics"),
        )
        .child(body)
        .into_any_element()
}

fn schematic_row(row: &Schematic) -> [AnyElement; 3] {
    [
        div().child(row.name).into_any_element(),
        div().child(row.description).into_any_element(),
        div()
            .child(
                Badge::new(match row.owned {
                    true => "Owned",
                    false => "Locked",
                })
                .tone(match row.owned {
                    true => Tone::Success,
                    false => Tone::Warning,
                }),
            )
            .into_any_element(),
    ]
}

/// The nine jar-type purchases, with the balance each one is checked against.
fn type_costs(plan: &UpgradePlan, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let mut table = Table::new(["#", "Type", "Cost", "Currency", "Balance", "Status"]);
    for step in &plan.steps {
        table = table.row([
            div().child(step.jar_type.to_string()).into_any_element(),
            div().child(step.name).into_any_element(),
            div().child(format::number(step.cost)).into_any_element(),
            div().child(step.currency_name).into_any_element(),
            div().child(format::number(step.balance)).into_any_element(),
            div()
                .child(
                    Badge::new(match step.status {
                        UpgradeStatus::Owned => "Owned".to_owned(),
                        UpgradeStatus::Affordable => "Affordable".to_owned(),
                        UpgradeStatus::Shortfall => {
                            format!("Short by {}", format::number(step.shortfall()))
                        }
                        UpgradeStatus::Locked => "Locked".to_owned(),
                    })
                    .tone(status_tone(step.status)),
                )
                .into_any_element(),
        ]);
    }
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
                .child(table)
                .child(
                    Caption::new(format!(
                        "The plan spends {} of the {} rupie balance now.",
                        format::number(plan.spent()),
                        format::number(plan.budget())
                    ))
                    .text_color(muted),
                ),
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

/// `doublerTierValue`: the six gilded tiers and their multipliers.
fn gilded(jar: &Jar, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let mut table = Table::new(["Tier", "Gilded %", "Multiplier"]);
    for tier in 0..GILDED_TIERS {
        table = table.row([
            div().child(tier.to_string()).into_any_element(),
            div()
                .child(format!(
                    "+{}%",
                    format::number(Jar::doubler_tier_value(tier))
                ))
                .into_any_element(),
            div()
                .child(format!("{:.2}×", Jar::doubler_multiplier(tier)))
                .into_any_element(),
        ]);
    }
    let doubler = jar.doubler_bonus();
    div()
        .id("jar-gilded-tiers")
        .child(
            Card::new()
                .header(
                    CardHeader::new("Gilded tier values")
                        .description("What a Gilded jar's doubler is worth at each tier"),
                )
                .child(table)
                .child(
                    Caption::new(match doubler > 0.0 {
                        true => format!(
                            "The save's gilded bonus is +{}% and applies to the next rupie break.",
                            format::number(doubler)
                        ),
                        false => {
                            "No gilded bonus is banked; break a Gilded jar to start one.".to_owned()
                        }
                    })
                    .text_color(muted),
                ),
        )
        .into_any_element()
}
