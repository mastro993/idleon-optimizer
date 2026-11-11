//! The pool KPI strip, the every-location table and the clickable pool switch.

use super::spot_text;
use crate::state::AppState;
use crate::ui::format;
use ely_gpui_component::data_display::Badge;
use ely_gpui_component::layout::{Card, CardHeader, ScrollArea};
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::ActiveTheme as _;
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, Hsla, IntoElement, ParentElement, Pixels, Role, Styled, div, px};
use idleon_core::fishing::Evaluation;

/// The character's fishing context as one wrap of cards.
pub fn kpis(evaluation: &Evaluation, hours: f64, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let outcome = &evaluation.outcome;
    let account = &evaluation.account;
    let ratio = if outcome.panel_exp_per_hour > 0.0 {
        (outcome.exp / outcome.hours) / outcome.panel_exp_per_hour
    } else {
        0.0
    };
    div()
        .id("fishing-kpis")
        .role(Role::Label)
        .aria_label(format!(
            "{} catches/hr · {} AFK catches · {} fish · {} EXP · real over panel {:.3}",
            format::number(evaluation.catches_per_hour),
            format::number(outcome.catches),
            format::number(outcome.fish_total),
            format::number(outcome.exp),
            ratio
        ))
        .flex()
        .flex_wrap()
        .gap_3()
        .child(card(
            "fishing-kpi-depths",
            "Depths",
            format!(
                "D1 {} · D2 {} · D3 {}",
                format::number(evaluation.depths.d1),
                format::number(evaluation.depths.d2),
                format::number(evaluation.depths.d3)
            ),
            format!(
                "D0 {} · equipment {} · spot {}",
                format::number(evaluation.depths.d0),
                account
                    .equipment_depth
                    .iter()
                    .map(|value| format::number(*value))
                    .collect::<Vec<_>>()
                    .join("/"),
                spot_text(&evaluation.spot_bonus)
            ),
            muted,
        ))
        .child(card(
            "fishing-kpi-efficiency",
            "Fishing efficiency",
            format::number(account.efficiency),
            "Entered from the Skills panel",
            muted,
        ))
        .child(card(
            "fishing-kpi-catches",
            "Catches / hr",
            format::number(evaluation.catches_per_hour),
            format!(
                "Catch time {:.3}s + 0.4s · attack ×{:.2}",
                evaluation.speed, evaluation.attack_factor_used
            ),
            muted,
        ))
        .child(card(
            "fishing-kpi-afk",
            &format!("AFK catches · {hours}h"),
            format::number(outcome.catches),
            format!("Catches/hr × AFK {:.1}%", account.afk_rate * 100.0),
            muted,
        ))
        .child(card(
            "fishing-kpi-per-catch",
            "Fish / catch",
            format::number(outcome.fish_per_catch),
            format!(
                "Ore value {} × Multi-Fish",
                format::number(outcome.ore_value)
            ),
            muted,
        ))
        .child(card(
            "fishing-kpi-fish",
            "Fish",
            format::number(outcome.fish_total),
            format!(
                "{} rolled · {} overwritten",
                format::number(outcome.rolled_fish_total),
                format::number(outcome.overwritten_fish)
            ),
            muted,
        ))
        .child(card(
            "fishing-kpi-exp",
            "Fishing EXP",
            format::number(outcome.exp),
            format!("×{} EXP multiplier", format::number(account.exp_multi)),
            muted,
        ))
        .child(card(
            "fishing-kpi-panel",
            "Panel EXP / hr",
            format::number(outcome.panel_exp_per_hour),
            format!(
                "{} EXP/catch",
                format::number(outcome.pool.panel_exp_per_catch)
            ),
            muted,
        ))
        .child(card(
            "fishing-kpi-ratio",
            "Real ÷ panel",
            format!("×{ratio:.3}"),
            "The panel omits ore value and per-fish EXP",
            muted,
        ))
        .into_any_element()
}

fn card(
    id: &'static str,
    label: &str,
    value: String,
    caption: impl Into<String>,
    muted: Hsla,
) -> AnyElement {
    Card::new()
        .child(
            div()
                .id(id)
                .role(Role::Label)
                .aria_label(format!("{label}: {value}"))
                .flex()
                .flex_col()
                .gap_1()
                .p_3()
                .min_w(px(150.0))
                .child(Caption::new(label.to_string()).text_color(muted))
                .child(div().text_size(px(18.0)).child(value))
                .child(Caption::new(caption.into()).text_color(muted)),
        )
        .into_any_element()
}

/// Every location, each row switching the selected pool. The selected row is
/// marked, and its first cell is the harness/click target
/// `fishing-location-<id with : as ->`.
pub fn location_table(
    rows: &[Evaluation],
    selected: &str,
    hours: f64,
    height: Pixels,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let mut table = Table::new([
        "Location",
        "Pool",
        "Spot depth",
        "Fish 1",
        "Fish 2",
        "Fish 3",
        "Fish 4",
        "Fish / catch",
        "Fish",
        "Fishing EXP",
        "Panel EXP/hr",
        "Real ÷ panel",
    ]);
    for evaluation in rows {
        let outcome = &evaluation.outcome;
        let entity = cx.entity();
        let id = eval_id(&evaluation.location.id);
        let is_current = evaluation.location.id == selected;
        let label = if is_current {
            format!("{} · selected", evaluation.location.label)
        } else {
            evaluation.location.label.clone()
        };
        let ratio = if outcome.panel_exp_per_hour > 0.0 {
            (outcome.exp / outcome.hours) / outcome.panel_exp_per_hour
        } else {
            0.0
        };
        let location_id = evaluation.location.id.clone();
        let mut first = div()
            .id(id)
            .role(Role::Button)
            .aria_label(format!("{}: select this pool", evaluation.location.label))
            .cursor_pointer()
            .on_click(move |_event, _window, cx| {
                entity.update(cx, |state, cx| {
                    state.set_fishing_location(location_id.clone(), cx)
                });
            })
            .flex()
            .items_center()
            .gap_2()
            .child(label);
        if is_current {
            first = first.child(Badge::new("Selected"));
        }
        table = table.row([
            first.into_any_element(),
            div()
                .child(evaluation.location.pool_name.clone())
                .into_any_element(),
            div()
                .text_color(cx.theme().colors.fg_muted)
                .child(spot_text(&evaluation.spot_bonus))
                .into_any_element(),
            share_cell(outcome, 0, muted),
            share_cell(outcome, 1, muted),
            share_cell(outcome, 2, muted),
            share_cell(outcome, 3, muted),
            div()
                .child(format::number(outcome.fish_per_catch))
                .into_any_element(),
            div()
                .child(format::number(outcome.fish_total))
                .into_any_element(),
            div().child(format::number(outcome.exp)).into_any_element(),
            div()
                .child(format::number(outcome.panel_exp_per_hour))
                .into_any_element(),
            div()
                .text_color(if ratio >= 1.0 {
                    cx.theme().colors.success
                } else {
                    cx.theme().colors.warning
                })
                .child(format!("×{ratio:.3}"))
                .into_any_element(),
        ]);
    }
    Card::new()
        .header(
            CardHeader::new("Every fishing location").description(format!(
                "Each spot has its own pool and depth bonus · totals {} · click a row to switch the pool",
                if hours == 1.0 {
                    "/hr".to_string()
                } else {
                    format!("/{hours}h")
                }
            )),
        )
        .child(
            div()
                .id("fishing-table")
                .min_h_0()
                .p_3()
                .child(
                    ScrollArea::new("fishing-table-scroll")
                        .vertical()
                        .h(height)
                        .child(table),
                ),
        )
        .into_any_element()
}

fn share_cell(outcome: &idleon_core::fishing::PoolOutcome, tier: usize, muted: Hsla) -> AnyElement {
    let fish = &outcome.fish[tier];
    let share = outcome.shares[tier];
    let text = if !fish.unlocked {
        "locked".to_string()
    } else if !fish.active {
        "skipped".to_string()
    } else {
        format!("{share:.2}%")
    };
    div().text_color(muted).child(text).into_any_element()
}

/// `54:0` becomes `fishing-location-54-0` for the automation bridge.
pub fn eval_id(id: &str) -> String {
    format!("fishing-location-{}", id.replace(':', "-"))
}
