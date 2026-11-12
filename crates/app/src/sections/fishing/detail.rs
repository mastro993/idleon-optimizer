//! The selected pool's fish split: requirements, shares, counts and EXP, plus
//! the real-versus-panel comparison the upstream footer explains.

use super::spot_text;
use crate::state::AppState;
use ely_gpui_component::data_display::Badge;
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::ActiveTheme as _;
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Role, Styled, div, px};
use idleon_fishing::Evaluation;
use idleon_ui::format;

/// The selected pool's split card.
pub(crate) fn card(evaluation: &Evaluation, hours: f64, cx: &mut Context<AppState>) -> AnyElement {
    let colors = &cx.theme().colors;
    let (muted, success, warning) = (colors.fg_muted, colors.success, colors.warning);
    let outcome = &evaluation.outcome;
    let location = evaluation.location;
    let ratio = if outcome.panel_exp_per_hour > 0.0 {
        (outcome.exp / outcome.hours) / outcome.panel_exp_per_hour
    } else {
        0.0
    };
    let mut table = Table::new([
        "Fish",
        "Efficiency req",
        "Share",
        "EXP / catch",
        "Catches",
        "Fish",
        "EXP",
        "EXP %",
    ]);
    for fish in &outcome.fish {
        let catches = if fish.active {
            outcome.catches * fish.share / 100.0
        } else {
            0.0
        };
        let status = if !fish.unlocked {
            Some(Badge::new(format!(
                "needs {} eff",
                format::number(fish.eff_req)
            )))
        } else if !fish.active {
            Some(Badge::new("skipped"))
        } else if fish.overwritten {
            Some(Badge::new("overwritten"))
        } else {
            None
        };
        let name = div()
            .flex()
            .items_center()
            .gap_2()
            .child(format!("{}: {}", fish.slot + 1, fish.name))
            .when_some(status, |row, badge| row.child(badge))
            .into_any_element();
        table = table.row([
            name,
            div()
                .child(if fish.eff_req > 0.0 {
                    format::number(fish.eff_req)
                } else {
                    "—".into()
                })
                .into_any_element(),
            div()
                .child(format!("{:.2}%", fish.share))
                .into_any_element(),
            div()
                .child(format::number(fish.exp_per_catch))
                .into_any_element(),
            div().child(format::number(catches)).into_any_element(),
            div()
                .child(format::number(fish.credited_count))
                .into_any_element(),
            div().child(format::number(fish.exp)).into_any_element(),
            div()
                .child(if outcome.exp > 0.0 {
                    format!("{:.1}%", fish.exp / outcome.exp * 100.0)
                } else {
                    "—".into()
                })
                .into_any_element(),
        ]);
    }

    Card::new()
        .header(
            CardHeader::new(format!("{} · {}", location.label, outcome.pool.name))
                .description(format!(
                    "Depth bonus {} · fish per catch {} · ×{:.2} Multi-Fish · panel {} EXP/catch",
                    spot_text(&evaluation.spot_bonus),
                    format::number(outcome.ore_value),
                    1.0 + evaluation.account.multi_fish / 100.0,
                    format::number(outcome.pool.panel_exp_per_catch)
                ))
                .action(Badge::new(if hours == 1.0 {
                    "/hr".to_string()
                } else {
                    format!("{hours}h")
                })),
        )
        .child(
            div()
                .id("fishing-detail")
                .role(Role::Label)
                .aria_label(format!(
                    "{}: {} fish and {} EXP",
                    outcome.pool.name,
                    format::number(outcome.fish_total),
                    format::number(outcome.exp)
                ))
                .flex()
                .flex_col()
                .gap_3()
                .p_3()
                .child(table)
                .child(
                    div()
                        .id("fishing-detail-totals")
                        .flex()
                        .items_center()
                        .gap_4()
                        .text_size(px(11.0))
                        .text_color(muted)
                        .child(format!("Share total {}%", format::number(outcome.share_total)))
                        .child(format!(
                            "{} avg EXP/catch",
                            format::number(outcome.exp_per_catch)
                        ))
                        .child(format!("{} catches", format::number(outcome.catches))),
                )
                .child(
                    div()
                        .id("fishing-panel-ratio")
                        .role(Role::Label)
                        .aria_label(format!("Real EXP is {ratio:.3} times the panel"))
                        .flex()
                        .flex_col()
                        .gap_1()
                        .border_t_1()
                        .border_color(colors.border)
                        .pt_3()
                        .child(
                            Caption::new(format!(
                                "Real EXP/hr {} vs panel EXP/hr {} · ×{ratio:.3}",
                                format::number(outcome.exp / outcome.hours),
                                format::number(outcome.panel_exp_per_hour)
                            ))
                            .text_color(if ratio >= 1.0 { success } else { warning }),
                        )
                        .child(
                            Caption::new(
                                "The AFK panel uses MonsterDefinitions[pool].ExpGiven for every fish and omits ore value and Multi-Fish.",
                            )
                            .text_color(muted),
                        ),
                ),
        )
        .into_any_element()
}
