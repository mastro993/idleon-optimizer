//! The Cglunko overview: the summary cards and the drop-rate formula tree.
//!
//! Every card and the tree read the same [`idleon_core::Cglunko`] state, so the
//! KPI figures and the breakdown's total cannot disagree. The section header
//! prints the one-line summary; the page below is the cards and the tree.

use crate::state::AppState;
use ely_gpui_component::data_display::{KpiCard, Statistic};
use ely_gpui_component::layout::{Card, CardHeader, ScrollArea};
use ely_gpui_component::theme::TextSize;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, Window, div, px};
use idleon_core::cglunko;

/// The overview page: the upstream cards and the DR tree.
pub fn overview_pane(
    state: &mut AppState,
    _window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let cglunko = state.cglunko.clone();
    let tree = state.cglunko_breakdown.clone();
    tree.update(cx, |tree, cx| tree.set_root(cglunko.breakdown(), cx));

    let first_row = div()
        .id("cglunko-kpi-row-1")
        .flex()
        .flex_wrap()
        .gap_3()
        .child(kpi(
            Statistic::new("cglunko-dr", "Total DR Bonus", cglunko.dr())
                .decimals(2)
                .suffix("×")
                .size(TextSize::Xl),
            "Combined Cglunko DR multiplier",
        ))
        .child(kpi(
            Statistic::new("cglunko-afk", "AFK Gains", cglunko.afk_gains() * 100.0)
                .decimals(1)
                .prefix("+")
                .suffix("%")
                .size(TextSize::Xl),
            "Cglunko AFK rate boost",
        ))
        .child(kpi(
            Statistic::new(
                "cglunko-mk-tier",
                "Multikill per Damage Tier",
                cglunko.mk_tier(),
            )
            .prefix("+")
            .suffix("%")
            .size(TextSize::Xl),
            cglunko::upgrade_name(6),
        ))
        .child(kpi(
            Statistic::new("cglunko-mk-base", "Base Multikill", cglunko.mk_base())
                .prefix("+")
                .suffix("%")
                .size(TextSize::Xl),
            cglunko::upgrade_name(15),
        ));

    let second_row = div()
        .id("cglunko-kpi-row-2")
        .flex()
        .flex_wrap()
        .gap_3()
        .child(kpi(
            Statistic::new("cglunko-respawn", "Respawn", cglunko.respawn())
                .prefix("+")
                .suffix("%")
                .size(TextSize::Xl),
            "Crystal respawn rate",
        ))
        .child(kpi(
            Statistic::new("cglunko-cavern", "Cavern Multi", cglunko.cavern_currency())
                .prefix("+")
                .suffix("%")
                .size(TextSize::Xl),
            "All cavern resources",
        ))
        .child(kpi(
            Statistic::new(
                "cglunko-research-exp",
                "Research EXP",
                1.0 + cglunko.research_exp() / 100.0,
            )
            .decimals(2)
            .prefix("×")
            .size(TextSize::Xl),
            "Research EXP multiplier",
        ))
        .child(kpi(
            Statistic::new(
                "cglunko-total-dmg",
                "Total DMG",
                1.0 + cglunko.total_dmg() / 100.0,
            )
            .decimals(2)
            .prefix("×")
            .size(TextSize::Xl),
            "All character damage",
        ));

    let third_row = div()
        .id("cglunko-kpi-row-3")
        .flex()
        .flex_wrap()
        .gap_3()
        .child(kpi(
            Statistic::new(
                "cglunko-blue-digits",
                "Blue-shape Digits",
                f64::from(cglunko.blue_digits()),
            )
            .size(TextSize::Xl),
            "Sum of ceil(log10) of blue stones",
        ))
        .child(kpi(
            Statistic::new(
                "cglunko-purple-digits",
                "Purple-shape Digits",
                f64::from(cglunko.purple_digits()),
            )
            .size(TextSize::Xl),
            "Sum of ceil(log10) of purple stones",
        ))
        .child(kpi(
            Statistic::new(
                "cglunko-grand-discovery",
                "Grand Discovery",
                cglunko.grand_discovery(),
            )
            .prefix("+")
            .suffix("%")
            .size(TextSize::Xl),
            "Spelunking find chance",
        ));

    let body = div()
        .id("cglunko-overview")
        .flex()
        .flex_col()
        .gap_3()
        .p_3()
        .child(first_row)
        .child(second_row)
        .child(third_row)
        .child(
            Card::new()
                .header(
                    CardHeader::new("DR formula breakdown")
                        .description("Search every factor and copy the full tree"),
                )
                .child(
                    div()
                        .id("cglunko-formula-body")
                        .h(px(280.0))
                        .min_h_0()
                        .child(tree),
                ),
        );

    ScrollArea::new("cglunko-overview-scroll")
        .vertical()
        .flex_1()
        .min_h_0()
        .child(body)
        .into_any_element()
}

/// One summary card: the figure and a line of context.
fn kpi(statistic: Statistic, caption: impl Into<gpui::SharedString>) -> AnyElement {
    KpiCard::new(statistic).caption(caption).into_any_element()
}
