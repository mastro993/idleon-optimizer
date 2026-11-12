//! The Clamworks overview: summary cards and the pearl value breakdown.

use super::kpi;
use crate::state::AppState;
use ely_gpui_component::forms::Stepper;
use ely_gpui_component::layout::{Card, CardHeader, ScrollArea};
use ely_gpui_component::theme::ActiveTheme as _;
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, Window, div, px};
use idleon_ui::format;

pub(crate) fn overview_pane(
    state: &mut AppState,
    _window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let clamworks = state.clamworks_view();
    let multikill = state.clamworks_multikill;
    let breakdown = clamworks.pearl_value_breakdown(multikill);
    let tree = state.clamworks_breakdown.clone();
    let root = clamworks.pearl_value_tree(multikill);
    tree.update(cx, |tree, cx| tree.set_root(root, cx));
    let (balance, balance_suffix) = format::scale(clamworks.pearl_balance);
    let promo_cost = clamworks.promotion_cost();
    let save_note = if state.save_loaded {
        "Values read from the imported save."
    } else {
        "No save imported: every Clamworks value reads as zero. Import a save from the header."
    };

    let entity = cx.entity();
    let multikill_control = div()
        .id("clamworks-multikill-row")
        .flex()
        .flex_none()
        .items_center()
        .gap_3()
        .child(Caption::new("Multikill total").text_color(cx.theme().colors.fg_muted))
        .child(
            Stepper::new("clamworks-multikill", multikill)
                .range(0.0, 1_000_000_000.0)
                .step(10.0)
                .precision(1)
                .on_change(move |value, _window, cx| {
                    entity.update(cx, |state, cx| state.set_clamworks_multikill(value, cx));
                }),
        )
        .child(Caption::new("%").text_color(cx.theme().colors.fg_muted))
        .child(
            Caption::new("Feeds Multi-Scalping. The save does not carry it.")
                .text_color(cx.theme().colors.fg_muted),
        );

    let first_row = div()
        .id("clamworks-kpi-row-1")
        .flex()
        .flex_wrap()
        .gap_3()
        .child(kpi(
            "clamworks-balance",
            "Pearl Balance",
            balance,
            0,
            Some(balance_suffix),
            "Saved pearl currency",
        ))
        .child(kpi(
            "clamworks-pearl-value",
            "Pearl Value",
            breakdown.product(),
            2,
            Some("×"),
            "Per pearl drop multiplier",
        ))
        .child(kpi(
            "clamworks-10x-chance",
            "10x Chance",
            clamworks.pearl_10x_chance() * 100.0,
            1,
            Some("%"),
            &format!("Expected ×{:.2}", clamworks.pearl_10x_expected()),
        ))
        .child(kpi(
            "clamworks-effective",
            "Effective / Drop",
            clamworks.effective_pearls_per_drop(multikill),
            2,
            None,
            "Value × 10x expected",
        ));
    let second_row = div()
        .id("clamworks-kpi-row-2")
        .flex()
        .flex_wrap()
        .gap_3()
        .child(kpi(
            "clamworks-drop-rate",
            "Pearl Drop Rate",
            clamworks.pearl_drop_rate() * 100.0,
            3,
            Some("%"),
            &format!("Promo penalty ×{:.3}", clamworks.promo_penalty()),
        ))
        .child(kpi(
            "clamworks-pure-rate",
            "Pure Pearl Rate",
            clamworks.black_pearl_drop_rate() * 100.0,
            6,
            Some("%"),
            &format!(
                "Value {} per drop",
                format::rate(clamworks.black_pearl_value())
            ),
        ))
        .child(kpi(
            "clamworks-pearls-kill",
            "Pearls / Kill",
            clamworks.pearls_per_kill(multikill),
            4,
            None,
            "Regular + pure pearls",
        ))
        .child(kpi(
            "clamworks-clams",
            "Clam Count",
            f64::from(clamworks.clam_count()),
            0,
            None,
            "Max 25",
        ));
    let third_row = div()
        .id("clamworks-kpi-row-3")
        .flex()
        .flex_wrap()
        .gap_3()
        .child(kpi(
            "clamworks-promotion",
            "Promotion Level",
            f64::from(clamworks.promotion_level),
            0,
            None,
            "Worker class level",
        ))
        .child(kpi(
            "clamworks-promo-chance",
            "Promo Chance",
            clamworks.promotion_chance() * 100.0,
            1,
            Some("%"),
            &format!("Cost {}", format::rate(promo_cost)),
        ))
        .child(kpi(
            "clamworks-cost-reduction",
            "Cost Reduction",
            (1.0 - clamworks.cost_reduction()) * 100.0,
            1,
            Some("%"),
            "FRUGALITY + ANTI INFLATION",
        ));

    let body = div()
        .id("clamworks-overview")
        .flex()
        .flex_col()
        .gap_3()
        .p_3()
        .child(multikill_control)
        .child(Caption::new(save_note).text_color(cx.theme().colors.fg_muted))
        .child(first_row)
        .child(second_row)
        .child(third_row)
        .child(formula_card(tree));

    ScrollArea::new("clamworks-overview-scroll")
        .vertical()
        .flex_1()
        .min_h_0()
        .child(body)
        .into_any_element()
}

/// The shared formula tree, collapsed to a fixed height.
fn formula_card(tree: gpui::Entity<idleon_ui::formula::FormulaTree>) -> impl IntoElement {
    Card::new()
        .header(
            CardHeader::new("Pearl value breakdown")
                .description("Search every factor and copy the full tree"),
        )
        .child(
            div()
                .id("clamworks-formula-card-body")
                .h(px(220.0))
                .min_h_0()
                .child(tree),
        )
}
