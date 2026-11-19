//! The Clamworks promotion tracker and the worker-class rewards.
//!
//! `clamworks.promotion` reads the saved worker-class level (`OLA[464]`) and
//! prices the next attempt, and the rewards table lists every `Spelunky[30]`
//! reward with the level that unlocks it.

use super::table::{Col, note, table, tag};
use super::{kpi, pane};
use crate::state::AppState;
use gpui::prelude::*;
use gpui::{AnyElement, App, Context, IntoElement, ParentElement, Styled, div};
use gpui_kit::component::v_flex;
use idleon_ui::format;
use idleon_ui::widgets::{Card, CardHeader, KpiStrip, PANEL_GAP, Tone};

pub(crate) fn promotion_pane(state: &mut AppState, cx: &mut Context<AppState>) -> AnyElement {
    let promotion = state.clamworks.promotion();
    let balance = format::scale(promotion.balance);
    let cost = format::scale(promotion.cost);
    let expected = format::scale(promotion.expected_cost);
    let hp = format::scale(promotion.clam_hp_after);
    let affordable = if promotion.affordable() {
        "Can afford".to_string()
    } else {
        format!("Need {} more", format::rate(promotion.shortfall()))
    };
    let first_row = div().id("clamworks-promotion-row-1").child(
        KpiStrip::new()
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
            )),
    );
    let second_row = div().id("clamworks-promotion-row-2").child(
        KpiStrip::new()
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
            )),
    );
    let body = v_flex()
        .id("clamworks-promotion-pane")
        .gap(PANEL_GAP)
        .child(first_row)
        .child(second_row)
        .child(reward_table(&promotion, cx))
        .child(note(
            format!(
                "{} of {} worker-class rewards unlocked at level {}.",
                promotion.unlocked(),
                promotion.rewards.len(),
                promotion.level
            ),
            cx,
        ));
    pane("clamworks-promotion-scroll", body)
}

/// Every worker-class reward with the level that unlocks it.
fn reward_table(promotion: &idleon_clamworks::Promotion, cx: &App) -> AnyElement {
    let rows = promotion.rewards.iter().map(|reward| {
        let (label, tone) = if reward.unlocked {
            ("Unlocked", Tone::Success)
        } else {
            ("Locked", Tone::Neutral)
        };
        [
            div()
                .id(("clamworks-reward-level", reward.level as usize))
                .child(reward.level.to_string())
                .into_any_element(),
            div()
                .id(("clamworks-reward", reward.level as usize))
                .min_w_0()
                .child(reward.description.clone())
                .into_any_element(),
            tag(label, tone).into_any_element(),
        ]
    });
    Card::new()
        .header(
            CardHeader::new("Worker Class Rewards").description(
                "Each promotion level unlocks the reward at that level; a locked row needs the level above it.",
            ),
        )
        .child(table(
            "clamworks-rewards",
            &[
                Col::Number("Worker Class Level"),
                Col::Text("Reward"),
                Col::Text("Status"),
            ],
            rows,
            cx,
        ))
        .into_any_element()
}
