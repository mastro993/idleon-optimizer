//! The Upgrades tab: every upgrade of a water, its costs, and the cost curve.
//!
//! The water strip picks one of the three waters and one log-scale curve plots
//! the picked upgrade, with every row carrying the next cost, the time to
//! afford it and any cap its effect nears.

use super::controls::{Upgrades, cached};
use super::overview::{num, time};
use super::planner::eta;
use super::table::{number, table, tag, text};
use crate::state::AppState;
use crate::ui::layout;
use gpui::prelude::*;
use gpui::{
    AnyElement, Context, IntoElement, ParentElement, SharedString, Styled, Window, div, px,
};
use gpui_kit::component::chart::LineChart;
use gpui_kit::component::select::Select;
use gpui_kit::component::table::TableRow;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use idleon_fountain as fountain;
use idleon_fountain::{Fountain, UpgradeRow};
use idleon_ui::widgets::{Caption, Card, CardHeader, Choice, PANEL_GAP, TEXT_CAPTION, Tone};

/// How many levels past the current one the cost curve plots.
const CURVE_STEPS: usize = 50;

/// The Upgrades pane: the water strip, the upgrade table and the cost curve.
pub(super) fn upgrades_pane(
    state: &mut AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let water = state.fountain_state.water.min(fountain::WATERS - 1);
    let selected = state.fountain_state.upgrade.unwrap_or(0);
    let Some(fountain) = state.fountain.as_ref() else {
        return div().into_any_element();
    };
    let upgrades = cached(
        &mut state.fountain_controls.upgrades,
        (state.save_revision, water, selected),
        || Upgrades {
            rows: fountain.upgrade_rows(water),
            curve: fountain
                .cost_curve(water, selected, CURVE_STEPS)
                .iter()
                .map(|cost| cost.max(1.0).log10())
                .collect(),
        },
    );
    let choices = upgrades
        .rows
        .iter()
        .map(|row| Choice::new(row.upgrade, row.name))
        .collect();
    let picker = &state.fountain_controls.curve_upgrade;
    picker.show(choices, Some(&selected), window, cx);
    let picker = h_flex()
        .gap_2()
        .child(Caption::new("Upgrade"))
        .child(
            div().w(px(200.0)).child(
                Select::new(picker.state())
                    .id("fountain-curve-upgrade")
                    .accessibility_label("Upgrade")
                    .small(),
            ),
        )
        .into_any_element();
    v_flex()
        .id("fountain-upgrades")
        .gap(PANEL_GAP)
        .child(water_strip(water, cx))
        .child(upgrade_table(fountain, &upgrades.rows, water, selected, cx))
        .child(cost_curve(&upgrades, selected, picker, cx))
        .into_any_element()
}

/// The Blue, Yellow and Green water tabs.
fn water_strip(water: usize, cx: &mut Context<AppState>) -> AnyElement {
    let entity = cx.entity();
    layout::tab_bar(
        "fountain-water",
        fountain::water_names()
            .iter()
            .enumerate()
            .map(|(index, name)| {
                (
                    SharedString::from(format!("fountain-water-{index}")),
                    SharedString::from(format!("{name} Water")),
                )
            }),
        water,
        move |water, _window, cx| {
            entity.update(cx, |state, cx| state.set_fountain_water(water, cx));
        },
    )
    .into_any_element()
}

/// Every upgrade of the selected water.
fn upgrade_table(
    fountain: &Fountain,
    rows: &[UpgradeRow],
    water: usize,
    selected: usize,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let theme = cx.theme();
    let (muted, accent, hover, radius) = (
        theme.muted_foreground,
        theme.primary,
        theme.list_hover,
        theme.radius,
    );
    let rows = rows.iter().map(|row| {
        let entity = cx.entity();
        let index = row.upgrade;
        let label = format!(
            "{}: level {} · next {} {} · {}",
            row.name,
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
            .rounded(radius)
            .px_1()
            .cursor_pointer()
            .when(index == selected, |cell| cell.text_color(accent))
            .hover(move |style| style.bg(hover))
            .on_click(move |_event, _window, cx| {
                entity.update(cx, |state, cx| {
                    state.set_fountain_upgrade(Some(index), cx);
                });
            })
            .child(row.name);
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
        let status = h_flex()
            .flex_wrap()
            .gap_1()
            .when(!row.accessible, |cell| {
                cell.child(tag("Locked", Tone::Danger))
            })
            .when(row.accessible && row.level > 0.0, |cell| {
                cell.child(tag(format!("Lv {}", num(row.level)), Tone::Success))
            })
            .when(row.accessible && row.level <= 0.0, |cell| {
                cell.child(tag("Available", Tone::Info))
            })
            .when(row.accessible, |cell| match &row.cap {
                Some(cap) if cap.capped => cell.child(tag(cap.label, Tone::Danger)),
                Some(cap) => cell.child(tag(cap.label, Tone::Warning)),
                None => cell,
            });
        TableRow::new()
            .id(("fountain-upgrade-row", index))
            .child(number(index.to_string()))
            .child(text(pick))
            .child(number(num(row.level)))
            .child(number(marble))
            .child(number(format!(
                "{} {}",
                num(row.next_cost),
                fountain::currency_label(row.currency)
            )))
            .child(number(eta(row.seconds_to_afford)))
            .child(number(marble_cost))
            .child(text(
                div()
                    .text_size(TEXT_CAPTION)
                    .line_clamp(3)
                    .child(description(fountain, water, index, &row.description)),
            ))
            .child(text(status))
    });
    let table = table(
        &[
            ("#", true),
            ("Upgrade", false),
            ("Level", true),
            ("Marble", true),
            ("Next Cost", true),
            ("Time to Afford", true),
            ("Marble Cost", true),
            ("Description", false),
            ("Status", false),
        ],
        rows,
    );
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
            v_flex()
                .id("fountain-upgrade-table")
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

/// One point of the cost curve: the level offset and `log10` of its cost.
#[derive(Clone, Copy)]
struct CurvePoint {
    step: usize,
    log_cost: f64,
}

/// The selected upgrade's cost curve over the next 50 levels, on a log scale.
fn cost_curve(
    upgrades: &Upgrades,
    selected: usize,
    picker: AnyElement,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let muted = cx.theme().muted_foreground;
    let Some(row) = upgrades.rows.iter().find(|row| row.upgrade == selected) else {
        return div().into_any_element();
    };
    let points: Vec<CurvePoint> = upgrades
        .curve
        .iter()
        .enumerate()
        .map(|(step, log_cost)| CurvePoint {
            step,
            log_cost: *log_cost,
        })
        .collect();
    // The chart plots log10(cost); the tick format turns an axis value back
    // into the cost the row shows.
    let chart = div().h(px(240.0)).child(
        LineChart::new(points)
            .id("fountain-cost-curve")
            .name(format!("{} (Lv {})", row.name, num(row.level)))
            .x(|point: &CurvePoint| SharedString::from(format!("+{}", point.step)))
            .y(|point: &CurvePoint| point.log_cost)
            .y_tick_format(|value| fountain::short_number(10_f64.powf(value)))
            .stroke(cx.theme().chart_1),
    );
    let cap = row.cap.as_ref().map(|cap| format!(" · {}", cap.detail));
    Card::new()
        .header(
            CardHeader::new("Upgrade Cost Curve")
                .description("The next fifty levels of the picked upgrade, plotted on a log scale")
                .action(picker),
        )
        .child(
            v_flex()
                .id("fountain-cost-curve-card")
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
