//! The Rupie Value tab: the multiplier every rupie is worth, its sources, the
//! extra rupie chance and what a break pays at each tier.
//!
//! The value tree's root is set once per applied save
//! ([`super::Controls::sync`]); render only places it.

use super::table::{Col, table, text};
use crate::sections::jar::{kpi, metric, row, stat};
use crate::state::AppState;
use crate::ui::segmented::segmented;
use gpui::prelude::*;
use gpui::{AnyElement, App, Context, Entity, IntoElement, ParentElement, Styled, div, px};
use idleon_jar::Jar;
use idleon_jar::{INCOME_TIERS, RUPIE_COLORS, RUPIE_NAMES, VALUE_TIERS};
use idleon_ui::format;
use idleon_ui::formula::FormulaTree;
use idleon_ui::widgets::{Caption, Card, CardHeader, Tone};

/// The tier column heads the tables take their first columns from.
pub(super) const TIER_HEADS: [&str; 6] =
    ["Tier 1", "Tier 2", "Tier 3", "Tier 4", "Tier 5", "Tier 6"];

pub(crate) fn rupies(state: &AppState, cx: &mut Context<AppState>) -> AnyElement {
    let jar = state
        .jar
        .as_ref()
        .expect("the section renders its import prompt without a save");
    let tier = state.jar_tier.min(INCOME_TIERS - 1);
    div()
        .id("jar-rupies")
        .flex()
        .flex_col()
        .gap_3()
        .child(headline(jar, tier))
        .child(tier_control(tier, cx))
        .child(breakdown(&state.jar_breakdown))
        .child(extra_chances(jar, cx))
        .child(income(jar, tier, cx))
        .child(value_by_tier(jar, cx))
        .into_any_element()
}

/// The four figures the tab leads with.
fn headline(jar: &Jar, tier: u32) -> AnyElement {
    let gilded = jar.doubler_bonus();
    let (gilded_value, gilded_caption, gilded_tone) = match gilded > 0.0 {
        true => (
            format!("+{}%", format::number(gilded)),
            "Applied to the next rupie break".to_owned(),
            Tone::Success,
        ),
        false => (
            "None".to_owned(),
            "Break a Gilded jar to bank a bonus".to_owned(),
            Tone::Neutral,
        ),
    };
    let value = jar.rupie_value();
    let payout = jar.rupies_per_break(tier);
    let extra = jar.extra_rupie_chance();
    row("jar-rupies-summary")
        .child(kpi(
            "jar-kpi-rupies",
            "Rupie value",
            format!("×{}", format::value(value)),
            stat("jar-kpi-rupies-value", "Rupie value", value).prefix("×"),
            "Applied to every rupie dropped",
        ))
        .child(kpi(
            "jar-kpi-rupies-break",
            "Rupies / break",
            format::value(payout),
            stat("jar-kpi-rupies-break-value", "Rupies / break", payout),
            format!(
                "Tier {} · {} rupies a break on average",
                tier + 1,
                format::value(jar.avg_rupies_per_break())
            ),
        ))
        .child(kpi(
            "jar-kpi-rupies-extra",
            "Extra rupie chance",
            format!("{}%", format::number(extra * 100.0)),
            stat(
                "jar-kpi-rupies-extra-value",
                "Extra rupie chance",
                extra * 100.0,
            )
            .suffix("%"),
            "One rupie a break, plus the chance of another",
        ))
        .child(metric(
            "jar-kpi-rupies-gilded",
            "Gilded",
            gilded_value,
            gilded_caption,
            gilded_tone,
        ))
        .into_any_element()
}

/// The tier the income figures are projected for. Segment `n` keeps the
/// `jar-tier-toggle-<n>` id, which the harness clicks.
fn tier_control(tier: u32, cx: &mut Context<AppState>) -> AnyElement {
    let entity = cx.entity();
    let control = segmented(
        "jar-tier",
        (0..INCOME_TIERS).map(|index| {
            (
                index,
                gpui::SharedString::from(format!("jar-tier-toggle-{index}")),
                format!("T{}", index + 1),
            )
        }),
        tier,
        move |index, _window, cx| entity.update(cx, |state, cx| state.set_jar_tier(index, cx)),
    );
    div()
        .id("jar-tier-row")
        .flex()
        .items_center()
        .gap_3()
        .child(Caption::new(format!(
            "Rupies per break at tier {}",
            tier + 1
        )))
        .child(control)
        .into_any_element()
}

/// The shared formula tree under a fixed height, so its own scroll area works.
fn breakdown(tree: &Entity<FormulaTree>) -> AnyElement {
    Card::new()
        .header(
            CardHeader::new("Rupie value breakdown")
                .description("Every multiplier that scales a dropped rupie, multiplied together"),
        )
        .child(
            div()
                .id("jar-rupies-breakdown")
                .h(px(320.0))
                .min_h_0()
                .child(tree.clone()),
        )
        .into_any_element()
}

/// The sources that add a rupie to a break.
fn extra_chances(jar: &Jar, cx: &App) -> AnyElement {
    let rows = std::iter::once(vec![
        text("One rupie"),
        text("+100%"),
        text("Every break pays at least one"),
    ])
    .chain(jar.extra_rupie_terms().into_iter().map(|term| {
        vec![
            text(term.label),
            text(format!("+{}%", format::number(term.percent))),
            text(term.source),
        ]
    }))
    .chain(std::iter::once(vec![
        text("Total"),
        text(format!(
            "+{}%",
            format::number(jar.extra_rupie_chance() * 100.0)
        )),
        text(format!(
            "{} rupies a break, on average",
            format::value(jar.avg_rupies_per_break())
        )),
    ]));
    Card::new()
        .header(
            CardHeader::new("Extra rupie chance")
                .description("Each break pays one rupie plus the chance of another"),
        )
        .child(table(
            "jar-extra-chances",
            &[Col::Text("Source"), Col::Number("Value"), Col::Text("From")],
            rows,
            cx,
        ))
        .into_any_element()
}

/// What one break of each rupie jar pays at the chosen tier,
/// split across the colors the account has reached.
fn income(jar: &Jar, tier: u32, cx: &App) -> AnyElement {
    let mut rows = Vec::new();
    for (_, name, active) in jar.rupie_jars() {
        let total = jar.rupies_per_break(tier);
        rows.push(vec![
            text(name),
            text(
                active
                    .iter()
                    .map(|color| RUPIE_NAMES[*color])
                    .collect::<Vec<_>>()
                    .join(", "),
            ),
            text(format::value(total)),
        ]);
        if active.len() < 2 {
            continue;
        }
        let share = total / active.len() as f64;
        for color in active {
            rows.push(vec![
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .pl(px(12.0))
                    .child(dot(color))
                    .child(RUPIE_NAMES[color])
                    .into_any_element(),
                text("Splits evenly"),
                text(format::value(share)),
            ]);
        }
    }
    Card::new()
        .header(
            CardHeader::new("Rupie income per break").description(format!(
                "Tier {} jars multiply the payout by ten; a multi-color jar splits it evenly",
                tier + 1
            )),
        )
        .child(table(
            "jar-income",
            &[
                Col::Text("Jar type"),
                Col::Text("Colors"),
                Col::Number("Rupies / break"),
            ],
            rows,
            cx,
        ))
        .into_any_element()
}

/// A rupie's own dot, as the pills draw it.
fn dot(color: usize) -> AnyElement {
    div()
        .flex_none()
        .size(px(8.0))
        .rounded_full()
        .bg(format::hex(RUPIE_COLORS[color]))
        .into_any_element()
}

/// The multiplier at each of the five value tiers.
fn value_by_tier(jar: &Jar, cx: &App) -> AnyElement {
    let mut cols = vec![Col::Text("Rupie value")];
    cols.extend(
        TIER_HEADS[..VALUE_TIERS as usize]
            .iter()
            .copied()
            .map(Col::Number),
    );
    let mut cells = vec![text("Multiplier")];
    cells.extend(
        (0..VALUE_TIERS)
            .map(|tier| text(format!("×{}", format::value(jar.rupie_value_at_tier(tier))))),
    );
    Card::new()
        .header(
            CardHeader::new("Rupie value by tier").description(
                "A tier multiplies the rupie value by ten, on top of the income table",
            ),
        )
        .child(table("jar-value-tiers", &cols, [cells], cx))
        .into_any_element()
}
