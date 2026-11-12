//! The Clamworks promotion tracker and the worker-class rewards.
//!
//! `clamworks.promotion` reads the saved worker-class level (`OLA[464]`) and
//! prices the next attempt, and the rewards table lists every `Spelunky[30]`
//! reward with the level that unlocks it, matching the upstream promotion tab.

use super::kpi;
use crate::state::AppState;
use crate::ui::format;
use ely_gpui_component::data_display::{Badge, Tone};
use ely_gpui_component::layout::{Card, CardHeader, ScrollArea};
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::ActiveTheme as _;
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, Window, div};

pub fn promotion_pane(
    state: &mut AppState,
    _window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let promotion = state.clamworks.promotion();
    let muted = cx.theme().colors.fg_muted;
    let balance = format::scale(promotion.balance);
    let cost = format::scale(promotion.cost);
    let expected = format::scale(promotion.expected_cost);
    let hp = format::scale(promotion.clam_hp_after);
    let affordable = if promotion.affordable() {
        "Can afford".to_string()
    } else {
        format!("Need {} more", format::rate(promotion.shortfall()))
    };
    let first_row = div()
        .id("clamworks-promotion-row-1")
        .flex()
        .flex_wrap()
        .gap_3()
        .child(kpi(
            "clamworks-promotion-level",
            "Worker Class Level",
            f64::from(promotion.level),
            0,
            None,
            "Saved promotion level",
        ))
        .child(kpi(
            "clamworks-promotion-chance",
            "Next Promo Chance",
            promotion.chance * 100.0,
            1,
            Some("%"),
            &format!("0.5 / (2 + {})", promotion.level),
        ))
        .child(kpi(
            "clamworks-promotion-cost",
            "Next Promo Cost",
            cost.0,
            2,
            Some(cost.1),
            &affordable,
        ));
    let second_row = div()
        .id("clamworks-promotion-row-2")
        .flex()
        .flex_wrap()
        .gap_3()
        .child(kpi(
            "clamworks-promotion-attempts",
            "Expected Attempts",
            f64::from(promotion.expected_attempts),
            0,
            None,
            "On average to promote",
        ))
        .child(kpi(
            "clamworks-promotion-expected",
            "Expected Total Cost",
            expected.0,
            2,
            Some(expected.1),
            "Cost ÷ chance",
        ))
        .child(kpi(
            "clamworks-promotion-hp",
            "Clam HP After",
            hp.0,
            2,
            Some(hp.1),
            "HP = 1e16 × 30^lv",
        ))
        .child(kpi(
            "clamworks-promotion-balance",
            "Pearl Balance",
            balance.0,
            2,
            Some(balance.1),
            "Saved pearls",
        ));
    let table = reward_table(&promotion);
    let body = div()
        .id("clamworks-promotion-pane")
        .flex()
        .flex_col()
        .gap_3()
        .p_3()
        .child(first_row)
        .child(second_row)
        .child(table)
        .child(
            Caption::new(format!(
                "{} of {} worker-class rewards unlocked at level {}.",
                promotion.unlocked(),
                promotion.rewards.len(),
                promotion.level
            ))
            .text_color(muted),
        );
    ScrollArea::new("clamworks-promotion-scroll")
        .vertical()
        .flex_1()
        .min_h_0()
        .child(body)
        .into_any_element()
}

/// Every worker-class reward with the level that unlocks it.
fn reward_table(promotion: &idleon_core::clamworks::Promotion) -> AnyElement {
    let mut table = Table::new(["Worker Class Level", "Reward", "Status"]);
    for reward in &promotion.rewards {
        let (label, tone) = if reward.unlocked {
            ("Unlocked", Tone::Success)
        } else {
            ("Locked", Tone::Neutral)
        };
        table = table.row([
            div()
                .id(("clamworks-reward-level", reward.level as usize))
                .child(reward.level.to_string())
                .into_any_element(),
            div()
                .id(("clamworks-reward", reward.level as usize))
                .min_w_0()
                .child(reward.description.clone())
                .into_any_element(),
            div().child(Badge::new(label).tone(tone)).into_any_element(),
        ]);
    }
    Card::new()
        .header(
            CardHeader::new("Worker Class Rewards").description(
                "Each promotion level unlocks the reward at that level; a locked row needs the level above it.",
            ),
        )
        .child(div().p_3().child(div().min_w_0().child(table)))
        .into_any_element()
}
