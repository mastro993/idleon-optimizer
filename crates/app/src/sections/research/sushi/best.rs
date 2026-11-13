//! The Best Upgrade pane: the efficiency ranking and the affordable path for
//! bucks, fuel generation and fuel capacity, as `_renderBestUpgrade` in
//! `js/ui/sushi-ui.js`.

use crate::state::AppState;
use ely_gpui_component::layout::{Card, CardHeader, ScrollArea};
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::ActiveTheme as _;
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, div, px};
use idleon_sushi::{Metric, Path, candidates, path};

/// One column's title and the metric it ranks by.
struct Column {
    key: &'static str,
    title: &'static str,
    metric: Metric,
}

const COLUMNS: [Column; 3] = [
    Column {
        key: "bucks",
        title: "Bucks/hr",
        metric: Metric::Bucks,
    },
    Column {
        key: "fuel",
        title: "Fuel/hr",
        metric: Metric::FuelGen,
    },
    Column {
        key: "cap",
        title: "Fuel Cap",
        metric: Metric::FuelCap,
    },
];

/// The path summary and its grouped purchases.
fn path_card(column: &Column, outcome: &Path, cx: &Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let accent = cx.theme().colors.accent;
    let success = cx.theme().colors.success;
    let mut table = Table::new(["Upgrade", "Level", "Cost"]);
    for purchase in &outcome.purchases {
        table = table.row([
            div()
                .id(format!("su-path-{}-row-{}", column.key, purchase.upg))
                .child(purchase.name.clone())
                .into_any_element(),
            div()
                .text_color(accent)
                .child(format!("{:.0} -> {:.0}", purchase.from, purchase.to))
                .into_any_element(),
            div()
                .text_color(muted)
                .child(idleon_sushi::fmt_num(purchase.cost))
                .into_any_element(),
        ]);
    }
    let headline = if outcome.steps == 0 {
        "No affordable upgrades.".to_string()
    } else {
        format!(
            "{} buys · {} spent · {} left",
            outcome.steps,
            idleon_sushi::fmt_num(outcome.spent),
            idleon_sushi::fmt_num(outcome.remaining)
        )
    };
    div()
        .id(format!("su-path-{}", column.key))
        .flex()
        .flex_col()
        .gap_1()
        .rounded_md()
        .bg(cx.theme().colors.sunken)
        .p_2()
        .child(Caption::new(headline).text_color(muted))
        .child(
            Caption::new(format!(
                "Total gain +{}",
                idleon_sushi::fmt_num(outcome.gain)
            ))
            .text_color(success),
        )
        .children((outcome.steps > 0).then_some(table))
        .into_any_element()
}

pub(super) fn pane(state: &mut AppState, cx: &mut Context<AppState>) -> AnyElement {
    let Some(inputs) = state.research.sushi.inputs.clone() else {
        return crate::ui::section::no_save(cx);
    };
    let station = &inputs.station;
    let ext = &inputs.external;
    let levels = &station.levels;
    let current_bucks = station.bucks_per_hour(levels, ext);
    let current_fuel = station.fuel_gen(levels, ext);
    let current_cap = station.fuel_capacity(levels, ext);
    let muted = cx.theme().colors.fg_muted;
    let accent = cx.theme().colors.accent;
    let success = cx.theme().colors.success;

    let mut body = div().flex().flex_col().gap_3();
    for column in &COLUMNS {
        let rows = candidates(&inputs, column.metric);
        let outcome = path(&inputs, column.metric, station.bucks());
        let mut table = Table::new(["Upgrade", "Lvl", "Cost", "Gain", "Eff."]);
        for candidate in rows.iter().take(25) {
            table = table.row([
                div()
                    .id(format!("su-best-{}-{}", column.key, candidate.slot))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child(candidate.name.clone())
                    .into_any_element(),
                div()
                    .text_color(accent)
                    .child(format!("{:.0}", candidate.level))
                    .into_any_element(),
                div()
                    .text_color(muted)
                    .child(idleon_sushi::fmt_num(candidate.cost))
                    .into_any_element(),
                div()
                    .text_color(success)
                    .child(format!("+{}", idleon_sushi::fmt_num(candidate.gain)))
                    .into_any_element(),
                div()
                    .text_color(accent)
                    .child(if candidate.efficiency > 0.0 {
                        format!("{:.2e}", candidate.efficiency)
                    } else {
                        "--".into()
                    })
                    .into_any_element(),
            ]);
        }
        let table = ScrollArea::new(format!("su-best-{}-scroll", column.key))
            .vertical()
            .h(px(330.0))
            .child(div().min_w(px(520.0)).child(table));
        body = body.child(
            Card::new()
                .header(
                    CardHeader::new(column.title).description("Gain per buck, at the saved levels"),
                )
                .child(
                    div()
                        .flex()
                        .items_start()
                        .gap_3()
                        .p_3()
                        .child(
                            div()
                                .w(px(380.0))
                                .flex_none()
                                .child(path_card(column, &outcome, cx)),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .min_h_0()
                                .overflow_hidden()
                                .child(table),
                        ),
                ),
        );
    }

    div()
        .id("sushi-best")
        .flex()
        .flex_col()
        .gap_3()
        .flex_1()
        .min_h_0()
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap_3()
                .child(Caption::new(format!(
                    "{} bucks/hr · {} fuel/hr · {} fuel cap · {} bucks in bank",
                    idleon_sushi::fmt_num(current_bucks),
                    idleon_sushi::fmt_num(current_fuel),
                    idleon_sushi::fmt_num(current_cap),
                    idleon_sushi::fmt_num(station.bucks()),
                )))
                .child(
                    Caption::new("Ranked by gain per buck, at the saved levels.").text_color(muted),
                ),
        )
        .child(
            ScrollArea::new("su-best-scroll")
                .vertical()
                .flex_1()
                .min_h_0()
                .child(body),
        )
        .into_any_element()
}
