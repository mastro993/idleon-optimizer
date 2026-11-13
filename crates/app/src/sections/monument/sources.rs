//! The Sources tab: each monument's permanent reward total and next-run
//! charge, the shared training pool and the reward-charge factors.

use super::{empty_card, metric_card, notes_card, unavailable_card};
use crate::state::AppState;
use ely_gpui_component::layout::{Card, CardHeader, ScrollArea};
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::ActiveTheme as _;
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, FontWeight, IntoElement, ParentElement, Role, Styled, div, px};
use idleon_monument::Summary;
use idleon_ui::format;

/// The Sources pane.
pub(crate) fn pane(state: &AppState, cx: &mut Context<AppState>) -> AnyElement {
    let summary = state.monument_summary();
    let body = match summary.as_ref() {
        Some(summary) if summary.available => loaded(summary, cx),
        Some(summary) => unavailable_card("Bonus sources", &summary.notes, cx),
        None => empty_card(
            "Bonus sources",
            "Import a decoded save to read the permanent rewards, the next-run charge and the training pool.",
            cx,
        ),
    };
    div()
        .id("monument-pane-sources")
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .child(
            ScrollArea::new("monument-sources-scroll")
                .vertical()
                .flex_1()
                .min_h_0()
                .child(body),
        )
        .into_any_element()
}

/// The permanent cards, the next-run charge, the training cards and both
/// source tables.
fn loaded(summary: &Summary, cx: &mut Context<AppState>) -> AnyElement {
    let mut permanent = div()
        .id("monument-permanent-summary")
        .flex()
        .flex_wrap()
        .gap_3();
    for (index, monument) in summary.monuments.iter().enumerate() {
        permanent = permanent.child(metric_card(
            [
                "monument-permanent-bravery",
                "monument-permanent-justice",
                "monument-permanent-wisdom",
            ][index],
            &monument.name,
            format!("{} levels", format::value(monument.levels)),
            "Sum of its ten saved permanent reward levels".to_string(),
            cx,
        ));
    }
    permanent = permanent.child(metric_card(
        "monument-reward-multi",
        "Reward-level multiplier",
        format!("{}x", format::value(summary.reward_level_multi)),
        "Companion bonus; applied after each reward roll".to_string(),
        cx,
    ));

    let mut charge = div()
        .id("monument-charge-summary")
        .flex()
        .flex_wrap()
        .gap_3();
    for (index, monument) in summary.monuments.iter().enumerate() {
        let ready = if monument.charge.can_start {
            "story ready"
        } else {
            "needs at least 1x to start"
        };
        charge = charge.child(metric_card(
            [
                "monument-charge-bravery",
                "monument-charge-justice",
                "monument-charge-wisdom",
            ][index],
            &monument.name,
            format!("{}x", format::value(monument.charge.multi)),
            format!(
                "{} real hours waited; {ready}. After another 24h: {}x.",
                format::value(monument.charge.seconds / 3600.0),
                format::value(monument.tomorrow.multi),
            ),
            cx,
        ));
    }

    let training = &summary.training;
    let training_cards = div()
        .id("monument-training-summary")
        .flex()
        .flex_wrap()
        .gap_3()
        .child(metric_card(
            "monument-training-rate",
            "One-character training rate",
            format!("{}x", format::value(training.rate)),
            "Training hours per real hour, before claim flooring".to_string(),
            cx,
        ))
        .child(metric_card(
            "monument-training-day",
            "Training per 24-hour claim",
            format!("{} hours", format::value(training.per_day)),
            "One character Paying Respect for the full claim".to_string(),
            cx,
        ))
        .child(metric_card(
            "monument-training-additive",
            "Additive training gain",
            format!("+{}%", format::value(training.pct)),
            "All integrated Monument AFK sources".to_string(),
            cx,
        ))
        .child(metric_card(
            "monument-training-multi",
            "Clutch Overtime Block",
            format!("{}x", format::value(training.multi)),
            "Saved Minehead reward".to_string(),
            cx,
        ));

    let mut pool = Table::new(["Source", "Bonus"]);
    let mut total = 0.0;
    for source in &training.sources {
        total += source.value;
        pool = pool.row([
            div()
                .min_w_0()
                .child(source.name.clone())
                .into_any_element(),
            div()
                .child(format!("+{}%", format::value(source.value)))
                .into_any_element(),
        ]);
    }
    pool = pool.row([
        div()
            .font_weight(FontWeight::SEMIBOLD)
            .child("Total additive pool")
            .into_any_element(),
        div()
            .id("monument-sources-total")
            .role(Role::Label)
            .aria_label(format!("+{}%", format::value(total)))
            .child(format!("+{}%", format::value(total)))
            .into_any_element(),
    ]);

    let mut factors = Table::new(["Monument", "Linear window", "Meritoc 7", "Legend 27"]);
    for monument in &summary.monuments {
        factors = factors.row([
            div().child(monument.name.clone()).into_any_element(),
            div()
                .child(format!(
                    "{} days",
                    format::value(monument.charge.max_linear / 86_400.0)
                ))
                .into_any_element(),
            div()
                .child(format!("+{}%", format::value(monument.charge.meritoc)))
                .into_any_element(),
            div()
                .child(format!("+{}%", format::value(monument.charge.legend)))
                .into_any_element(),
        ]);
    }

    let muted = cx.theme().colors.fg_muted;
    let mut column = div()
        .id("monument-sources-body")
        .flex()
        .flex_col()
        .gap_3()
        .child(group_title("monument-permanent-title", "Permanent rewards"))
        .child(permanent)
        .child(group_title(
            "monument-charge-title",
            "Next-run reward charge",
        ))
        .child(charge)
        .child(Caption::new(
            "Charge grows with real waiting time, not Paying Respect training. Starting a normal story consumes it. After the linear window it keeps growing, more slowly. Run performance determines the levels earned; there is no fixed levels/day conversion.",
        ))
        .child(group_title(
            "monument-training-title",
            "Paying Respect training",
        ))
        .child(training_cards)
        .child(Caption::new(
            "Training hours strengthen minigame stats and unlock milestones. They are not permanent reward levels and do not speed up reward charge. The daily figure is one character's single 24-hour claim; claims must exceed 3,599 seconds and round down.",
        ))
        .child(
            Card::new()
                .header(CardHeader::new("Paying Respect: additive pool").description(format!(
                    "{} named sources sum to the additive training gain",
                    training.sources.len()
                )))
                .child(pool),
        )
        .child(
            Card::new()
                .header(CardHeader::new("Reward-charge factors (separate from training)"))
                .child(factors)
                .child(Caption::new(
                    "The linear window includes both storage schematics, Monumental Gaming superbit, Legend 27, and Justice study ownership. Waiting beyond it still contributes diminished charge.",
                ).text_color(muted)),
        );
    if !summary.notes.is_empty() {
        column = column.child(notes_card("monument-sources-notes", &summary.notes, cx));
    }
    column.into_any_element()
}

/// A group heading above its cards, as the upstream `h2` does.
fn group_title(id: &'static str, text: &'static str) -> AnyElement {
    div()
        .id(id)
        .role(Role::Label)
        .aria_label(text)
        .text_size(px(15.0))
        .font_weight(FontWeight::SEMIBOLD)
        .child(text)
        .into_any_element()
}
