//! The Rupie Value tab: the multiplier every rupie is worth, its sources, the
//! extra rupie chance and what a break pays at each tier.

use crate::sections::jar::{kpi, metric, stat};
use crate::state::AppState;
use ely_gpui_component::buttons::{ToggleGroup, ToggleItem};
use ely_gpui_component::data_display::Tone;
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize};
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, Entity, IntoElement, ParentElement, Styled, div, px};
use idleon_jar::Jar;
use idleon_jar::{INCOME_TIERS, RUPIE_COLORS, RUPIE_NAMES, VALUE_TIERS};
use idleon_ui::format;

pub(crate) fn rupies(state: &mut AppState, cx: &mut Context<AppState>) -> AnyElement {
    let jar = state
        .jar
        .as_ref()
        .expect("the section renders its import prompt without a save");
    let tier = state.jar_tier.min(INCOME_TIERS - 1);
    let tree = state.jar_breakdown.clone();
    tree.update(cx, |tree, cx| tree.set_root(jar.rupie_value_tree(), cx));
    div()
        .id("jar-rupies")
        .flex()
        .flex_col()
        .gap_3()
        .child(headline(jar, tier, cx))
        .child(tier_control(tier, cx))
        .child(breakdown(tree))
        .child(extra_chances(jar))
        .child(income(jar, tier))
        .child(value_by_tier(jar))
        .into_any_element()
}

/// The four figures the upstream page leads with.
fn headline(jar: &Jar, tier: u32, cx: &mut Context<AppState>) -> AnyElement {
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
    div()
        .id("jar-rupies-summary")
        .flex()
        .flex_wrap()
        .gap_3()
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
            cx,
        ))
        .into_any_element()
}

/// The tier the income figures are projected for.
fn tier_control(tier: u32, cx: &mut Context<AppState>) -> AnyElement {
    let entity = cx.entity();
    let mut control = ToggleGroup::new("jar-tier").size(ControlSize::Sm);
    for index in 0..INCOME_TIERS {
        control = control.item(ToggleItem::new(index.to_string()).label(format!("T{}", index + 1)));
    }
    let muted = cx.theme().colors.fg_muted;
    div()
        .id("jar-tier-row")
        .flex()
        .items_center()
        .gap_3()
        .child(Caption::new(format!("Rupies per break at tier {}", tier + 1)).text_color(muted))
        .child(
            control
                .selected([tier.to_string()])
                .on_change(move |selected, _window, cx| {
                    let Some(value) = selected.first() else {
                        return;
                    };
                    if let Ok(tier) = value.parse::<u32>() {
                        entity.update(cx, |state, cx| state.set_jar_tier(tier, cx));
                    }
                }),
        )
        .into_any_element()
}

/// The shared formula tree under a fixed height, so its own scroll area works.
fn breakdown(tree: Entity<idleon_ui::formula::FormulaTree>) -> AnyElement {
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
                .child(tree),
        )
        .into_any_element()
}

/// `extraRupieChance`: the sources that add a rupie to a break.
fn extra_chances(jar: &Jar) -> AnyElement {
    let mut table = Table::new(["Source", "Value", "From"]).row([
        div().child("One rupie").into_any_element(),
        div().child("+100%").into_any_element(),
        div()
            .child("Every break pays at least one")
            .into_any_element(),
    ]);
    for term in jar.extra_rupie_terms() {
        table = table.row([
            div().child(term.label).into_any_element(),
            div()
                .child(format!("+{}%", format::number(term.percent)))
                .into_any_element(),
            div().child(term.source).into_any_element(),
        ]);
    }
    table = table.row([
        div().child("Total").into_any_element(),
        div()
            .child(format!(
                "+{}%",
                format::number(jar.extra_rupie_chance() * 100.0)
            ))
            .into_any_element(),
        div()
            .child(format!(
                "{} rupies a break, on average",
                format::value(jar.avg_rupies_per_break())
            ))
            .into_any_element(),
    ]);
    Card::new()
        .header(
            CardHeader::new("Extra rupie chance")
                .description("Each break pays one rupie plus the chance of another"),
        )
        .child(table)
        .into_any_element()
}

/// `rupieTypeRange`: what one break of each rupie jar pays at the chosen tier,
/// split across the colors the account has reached.
fn income(jar: &Jar, tier: u32) -> AnyElement {
    let mut table = Table::new(["Jar type", "Colors", "Rupies / break"]);
    for (_, name, active) in jar.rupie_jars() {
        let total = jar.rupies_per_break(tier);
        table = table.row([
            div().child(name).into_any_element(),
            div()
                .child(
                    active
                        .iter()
                        .map(|color| RUPIE_NAMES[*color])
                        .collect::<Vec<_>>()
                        .join(", "),
                )
                .into_any_element(),
            div().child(format::value(total)).into_any_element(),
        ]);
        if active.len() < 2 {
            continue;
        }
        let share = total / active.len() as f64;
        for color in active {
            table = table.row([
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .pl(px(12.0))
                    .child(dot(color))
                    .child(RUPIE_NAMES[color])
                    .into_any_element(),
                div().child("Splits evenly").into_any_element(),
                div().child(format::value(share)).into_any_element(),
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
        .child(table)
        .into_any_element()
}

/// A rupie's own dot, as the upstream pills draw it.
fn dot(color: usize) -> AnyElement {
    div()
        .flex_none()
        .size(px(8.0))
        .rounded_full()
        .bg(format::hex(RUPIE_COLORS[color]))
        .into_any_element()
}

/// `rupieValueByTier`: the multiplier at each of the five value tiers.
fn value_by_tier(jar: &Jar) -> AnyElement {
    let mut headers: Vec<String> = vec!["Rupie value".into()];
    for tier in 0..VALUE_TIERS {
        headers.push(format!("Tier {}", tier + 1));
    }
    let mut cells: Vec<AnyElement> = vec![div().child("Multiplier").into_any_element()];
    for tier in 0..VALUE_TIERS {
        cells.push(
            div()
                .child(format!("×{}", format::value(jar.rupie_value_at_tier(tier))))
                .into_any_element(),
        );
    }
    Card::new()
        .header(
            CardHeader::new("Rupie value by tier").description(
                "A tier multiplies the rupie value by ten, on top of the income table",
            ),
        )
        .child(Table::new(headers).row(cells))
        .into_any_element()
}
