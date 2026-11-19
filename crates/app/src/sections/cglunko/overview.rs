//! The Cglunko overview: the summary cards and the drop-rate formula tree.
//!
//! Every card and the tree read the same [`idleon_cglunko::Cglunko`] state, so the
//! KPI figures and the breakdown's total cannot disagree. The section header
//! prints the one-line summary; the page below is the cards and the tree.

use super::{grouped, pane};
use crate::state::AppState;
use gpui::prelude::*;
use gpui::{AnyElement, ParentElement, Styled, div, px};
use idleon_cglunko as cglunko;
use idleon_ui::widgets::{Card, CardHeader, KpiStrip, Statistic};

/// The overview page: the summary figures and the DR tree. The tree's root is
/// set when a save is applied ([`super::Figures::sync`]).
pub(crate) fn overview_pane(state: &AppState) -> AnyElement {
    let cglunko = &state.cglunko;
    let first_row = KpiStrip::new()
        .child(
            Statistic::new("cglunko-dr", "Total DR Bonus", grouped(cglunko.dr(), 2))
                .suffix("×")
                .caption("Combined Cglunko DR multiplier"),
        )
        .child(
            Statistic::new(
                "cglunko-afk",
                "AFK Gains",
                grouped(cglunko.afk_gains() * 100.0, 1),
            )
            .prefix("+")
            .suffix("%")
            .caption("Cglunko AFK rate boost"),
        )
        .child(
            Statistic::new(
                "cglunko-mk-tier",
                "Multikill per Damage Tier",
                grouped(cglunko.mk_tier(), 0),
            )
            .prefix("+")
            .suffix("%")
            .caption(cglunko::upgrade_name(6)),
        )
        .child(
            Statistic::new(
                "cglunko-mk-base",
                "Base Multikill",
                grouped(cglunko.mk_base(), 0),
            )
            .prefix("+")
            .suffix("%")
            .caption(cglunko::upgrade_name(15)),
        );

    let second_row = KpiStrip::new()
        .child(
            Statistic::new("cglunko-respawn", "Respawn", grouped(cglunko.respawn(), 0))
                .prefix("+")
                .suffix("%")
                .caption("Crystal respawn rate"),
        )
        .child(
            Statistic::new(
                "cglunko-cavern",
                "Cavern Multi",
                grouped(cglunko.cavern_currency(), 0),
            )
            .prefix("+")
            .suffix("%")
            .caption("All cavern resources"),
        )
        .child(
            Statistic::new(
                "cglunko-research-exp",
                "Research EXP",
                grouped(1.0 + cglunko.research_exp() / 100.0, 2),
            )
            .prefix("×")
            .caption("Research EXP multiplier"),
        )
        .child(
            Statistic::new(
                "cglunko-total-dmg",
                "Total DMG",
                grouped(1.0 + cglunko.total_dmg() / 100.0, 2),
            )
            .prefix("×")
            .caption("All character damage"),
        );

    let third_row = KpiStrip::new()
        .child(
            Statistic::new(
                "cglunko-blue-digits",
                "Blue-shape Digits",
                cglunko.blue_digits().to_string(),
            )
            .caption("Sum of ceil(log10) of blue stones"),
        )
        .child(
            Statistic::new(
                "cglunko-purple-digits",
                "Purple-shape Digits",
                cglunko.purple_digits().to_string(),
            )
            .caption("Sum of ceil(log10) of purple stones"),
        )
        .child(
            Statistic::new(
                "cglunko-grand-discovery",
                "Grand Discovery",
                grouped(cglunko.grand_discovery(), 0),
            )
            .prefix("+")
            .suffix("%")
            .caption("Spelunking find chance"),
        );

    let body = div()
        .id("cglunko-overview")
        .flex()
        .flex_col()
        .gap_3()
        .p_3()
        .child(div().id("cglunko-kpi-row-1").child(first_row))
        .child(div().id("cglunko-kpi-row-2").child(second_row))
        .child(div().id("cglunko-kpi-row-3").child(third_row))
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
                        .child(state.cglunko_breakdown.clone()),
                ),
        );
    pane("cglunko-overview-scroll", body)
}
