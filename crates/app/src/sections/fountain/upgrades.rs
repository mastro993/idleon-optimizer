//! The Upgrades tab: every upgrade of a water, its costs, and the cost curve.
//!
//! Ported from the `#tab-upgrades` half of `fountain-calc.html`. The page tabs
//! the three waters and plots selected upgrades on a log scale; here the water
//! strip and one curve for the pick keep the same reading, with every row
//! carrying the next cost, the time to afford it and any cap its effect nears.

use super::overview::{num, time};
use crate::state::AppState;
use ely_gpui_component::charts::{LineChart, Series};
use ely_gpui_component::data_display::{Badge, Tone};
use ely_gpui_component::forms::{Choice, Select};
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::shell::{TabBar, WindowTab};
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize};
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, SharedString, Styled, div, px};
use idleon_core::Fountain;
use idleon_core::fountain;

/// How many levels past the current one the cost curve plots.
const CURVE_STEPS: usize = 50;

/// The Upgrades pane: the water strip, the upgrade table and the cost curve.
pub(super) fn upgrades_pane(state: &mut AppState, cx: &mut Context<AppState>) -> AnyElement {
    let water = state.fountain_state.water.min(fountain::WATERS - 1);
    let selected = state.fountain_state.upgrade.unwrap_or(0);
    let Some(fountain) = state.fountain.as_ref() else {
        return div().into_any_element();
    };
    div()
        .id("fountain-upgrades")
        .flex()
        .flex_col()
        .gap_3()
        .child(water_strip(water, cx))
        .child(upgrade_table(fountain, water, selected, cx))
        .child(cost_curve(fountain, water, selected, cx))
        .into_any_element()
}

/// The Blue, Yellow and Green water tabs.
fn water_strip(water: usize, cx: &mut Context<AppState>) -> AnyElement {
    let entity = cx.entity();
    let mut tabs = TabBar::new("fountain-water")
        .selected(SharedString::from(format!("fountain-water-{water}")));
    for (index, name) in fountain::water_names().iter().enumerate() {
        tabs = tabs.tab(WindowTab::new(
            format!("fountain-water-{index}"),
            format!("{name} Water"),
        ));
    }
    tabs.on_select(move |id, _window, cx| {
        if let Some(water) = id
            .as_ref()
            .strip_prefix("fountain-water-")
            .and_then(|water| water.parse::<usize>().ok())
        {
            entity.update(cx, |state, cx| state.set_fountain_water(water, cx));
        }
    })
    .into_any_element()
}

/// Every upgrade of the selected water.
fn upgrade_table(
    fountain: &Fountain,
    water: usize,
    selected: usize,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let accent = cx.theme().colors.accent;
    let hover = cx.theme().colors.hover;
    let mut table = Table::new([
        "#",
        "Upgrade",
        "Level",
        "Marble",
        "Next Cost",
        "Time to Afford",
        "Marble Cost",
        "Description",
        "Status",
    ]);
    for row in fountain.upgrade_rows(water) {
        let entity = cx.entity();
        let index = row.upgrade;
        let name = row.name.to_owned();
        let label = format!(
            "{}: level {} · next {} {} · {}",
            name,
            row.level,
            num(row.next_cost),
            fountain::currency_label(row.currency),
            row.description
        );
        let pick = div()
            .id(SharedString::from(format!(
                "fountain-upgrade-{water}-{index}"
            )))
            .role(gpui::Role::Button)
            .aria_label(label)
            .aria_selected(index == selected)
            .rounded(px(3.0))
            .px_1()
            .cursor_pointer()
            .when(index == selected, |cell| cell.text_color(accent))
            .hover(move |style| style.bg(hover))
            .on_click(move |_event, _window, cx| {
                entity.update(cx, |state, cx| {
                    state.set_fountain_upgrade(Some(index), cx);
                });
            })
            .child(name)
            .into_any_element();
        let marble = if row.marbable {
            format!("{} ({:.1}×)", num(row.marble), row.marble_bonus)
        } else {
            "—".to_owned()
        };
        let marble_cost = if row.marbable && row.level > 0.0 {
            num(row.marble_cost)
        } else {
            "—".to_owned()
        };
        let status = div()
            .flex()
            .items_center()
            .gap_1()
            .when(!row.accessible, |cell| {
                cell.child(Badge::new("Locked").tone(Tone::Danger))
            })
            .when(row.accessible && row.level > 0.0, |cell| {
                cell.child(Badge::new(format!("Lv {}", num(row.level))).tone(Tone::Success))
            })
            .when(row.accessible && row.level <= 0.0, |cell| {
                cell.child(Badge::new("Available").tone(Tone::Info))
            })
            .when(row.accessible, |cell| match &row.cap {
                Some(cap) if cap.capped => cell.child(Badge::new(cap.label).tone(Tone::Danger)),
                Some(cap) => cell.child(Badge::new(cap.label).tone(Tone::Warning)),
                None => cell,
            })
            .into_any_element();
        table = table.row([
            div().child(index.to_string()).into_any_element(),
            pick,
            div().child(num(row.level)).into_any_element(),
            div().child(marble).into_any_element(),
            div()
                .child(format!(
                    "{} {}",
                    num(row.next_cost),
                    fountain::currency_label(row.currency)
                ))
                .into_any_element(),
            afford_cell(row.seconds_to_afford),
            div().child(marble_cost).into_any_element(),
            div()
                .max_w(px(260.0))
                .text_size(px(11.0))
                .line_height(px(15.0))
                .line_clamp(3)
                .child(description(fountain, water, index, &row.description))
                .into_any_element(),
            status,
        ]);
    }
    let assumption = if fountain.ignore_unlocked() {
        "Estimated Optimal /hr: active play, full royal stacks, the desired bonus and full focus \
         on the required currency."
    } else {
        "Estimated Optimal /hr: active play, full royal stacks, the desired bonus and the current \
         active pool, because Turn and Push is locked."
    };
    Card::new()
        .header(CardHeader::new(format!(
            "{} Water Upgrades",
            fountain::water_names()[water]
        )))
        .child(
            div()
                .id("fountain-upgrade-table")
                .flex()
                .flex_col()
                .gap_2()
                .child(table)
                .child(Caption::new(assumption).text_color(muted)),
        )
        .into_any_element()
}

/// The row's description, with the live fill time the page appends to three rows.
fn description(fountain: &Fountain, water: usize, upgrade: usize, base: &str) -> String {
    let mut text = base.to_owned();
    if water == 0 && upgrade == 9 {
        text.push_str(&format!(
            " • Fill: {} (active: {})",
            time(fountain.bar_fill_time()),
            time(fountain.active_fill_time())
        ));
    }
    if water == 0 && upgrade == 12 {
        text.push_str(&format!(
            " • Active fill: {}",
            time(fountain.active_fill_time())
        ));
    }
    if water == 1 && upgrade == 10 {
        text.push_str(&format!(
            " • Marble fill: {} ({} /fill)",
            time(Fountain::marble_bar_fill_time()),
            num(fountain.marble_per_fill())
        ));
    }
    text
}

/// The next level's time to afford: Now, a duration or the unreachable infinity.
fn afford_cell(seconds: f64) -> AnyElement {
    if seconds <= 0.0 {
        return div()
            .flex()
            .items_center()
            .child(Badge::new("Now").tone(Tone::Success))
            .into_any_element();
    }
    if !seconds.is_finite() {
        return div().child("∞").into_any_element();
    }
    div().child(time(seconds)).into_any_element()
}

/// The selected upgrade's cost curve over the next 50 levels, on a log scale.
fn cost_curve(
    fountain: &Fountain,
    water: usize,
    selected: usize,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let Some(row) = fountain.upgrade_row(water, selected) else {
        return div().into_any_element();
    };
    let entity = cx.entity();
    let choices: Vec<Choice> = fountain
        .upgrade_rows(water)
        .into_iter()
        .map(|row| Choice::new(row.upgrade.to_string(), row.name))
        .collect();
    let picker = Select::new("fountain-curve-upgrade", choices)
        .label("Upgrade")
        .selected(selected.to_string())
        .size(ControlSize::Sm)
        .on_change(move |value, _window, cx| {
            if let Ok(upgrade) = value.parse::<usize>() {
                entity.update(cx, |state, cx| {
                    state.set_fountain_upgrade(Some(upgrade), cx);
                });
            }
        });
    let costs = fountain.cost_curve(water, selected, CURVE_STEPS);
    let labels: Vec<String> = (0..=CURVE_STEPS).map(|step| format!("+{step}")).collect();
    // The chart plots log10(cost); `format` turns an axis value back into the
    // cost the row shows, so a hover reads the real number.
    let series: Vec<f64> = costs.iter().map(|cost| (cost.max(1.0)).log10()).collect();
    let name = format!("{} (Lv {})", row.name, num(row.level));
    let chart = LineChart::new("fountain-cost-curve", labels)
        .series(Series::new(name, series))
        .format(|value| fountain::short_number(10_f64.powf(value)))
        .h(px(240.0));
    let cap = row.cap.as_ref().map(|cap| format!(" · {}", cap.detail));
    Card::new()
        .header(
            CardHeader::new("Upgrade Cost Curve")
                .description("The next fifty levels of the picked upgrade, plotted on a log scale")
                .action(picker),
        )
        .child(
            div()
                .id("fountain-cost-curve-card")
                .flex()
                .flex_col()
                .gap_2()
                .child(chart)
                .child(
                    Caption::new(format!(
                        "{} · {} {} for the next level{}",
                        row.name,
                        num(row.next_cost),
                        fountain::currency_label(row.currency),
                        cap.unwrap_or_default()
                    ))
                    .text_color(muted),
                )
                .child(
                    Caption::new(
                        "A capped or soft-capped effect keeps costing more per level; pick another \
                         upgrade in the selector to compare the curves.",
                    )
                    .text_color(muted),
                ),
        )
        .into_any_element()
}
