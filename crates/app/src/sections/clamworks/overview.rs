//! The Clamworks overview: summary cards and the pearl value breakdown.

use super::{kpi, pane};
use crate::state::AppState;
use crate::ui;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, Window, div, px};
use gpui_kit::component::{ActiveTheme as _, h_flex, v_flex};
use idleon_ui::format;
use idleon_ui::widgets::{Caption, Card, CardHeader, KpiStrip, PANEL_GAP, show_number};

pub(crate) fn overview_pane(
    state: &mut AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    state.sync_clamworks_tree(cx);
    let clamworks = state.clamworks_shown();
    let multikill = state.clamworks_multikill;
    let muted = cx.theme().muted_foreground;
    let breakdown = clamworks.pearl_value_breakdown(multikill);
    let (balance, balance_suffix) = format::scale(clamworks.pearl_balance);
    let promo_cost = clamworks.promotion_cost();
    let save_note = if state.save_loaded {
        "Values read from the imported save."
    } else {
        "No save imported: every Clamworks value reads as zero. Import a save from the header."
    };

    let field = &state.clamworks_controls.multikill;
    show_number(field, multikill, 1, window, cx);
    let multikill_control = h_flex()
        .id("clamworks-multikill-row")
        .flex_none()
        .gap_3()
        .child(Caption::new("Multikill total").text_color(muted))
        .child(ui::number_field(
            "clamworks-multikill",
            field,
            "Multikill total",
        ))
        .child(Caption::new("%").text_color(muted))
        .child(Caption::new("Feeds Multi-Scalping. The save does not carry it.").text_color(muted));

    let first_row = div().id("clamworks-kpi-row-1").child(
        KpiStrip::new()
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
            )),
    );
    let second_row = div().id("clamworks-kpi-row-2").child(
        KpiStrip::new()
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
            )),
    );
    let third_row = div().id("clamworks-kpi-row-3").child(
        KpiStrip::new()
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
            )),
    );

    let body = v_flex()
        .id("clamworks-overview")
        .gap(PANEL_GAP)
        .child(multikill_control)
        .child(Caption::new(save_note).text_color(muted))
        .child(first_row)
        .child(second_row)
        .child(third_row)
        .child(formula_card(state.clamworks_breakdown.clone()));
    pane("clamworks-overview-scroll", body)
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
