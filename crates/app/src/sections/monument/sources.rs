//! The Sources tab: each monument's permanent reward total and next-run
//! charge, the shared training pool and the reward-charge factors.

use super::table::{Col, table, text};
use super::{Metric, empty_card, metrics, notes_card, scroll_pane, unavailable_card};
use crate::state::AppState;
use gpui::prelude::*;
use gpui::{AnyElement, Context, FontWeight, IntoElement, ParentElement, Role, Styled, div};
use gpui_kit::component::{ActiveTheme as _, v_flex};
use idleon_monument::Summary;
use idleon_ui::format;
use idleon_ui::widgets::{Caption, Card, CardHeader, TEXT_BODY};

/// The Sources pane.
pub(crate) fn pane(state: &AppState, cx: &mut Context<AppState>) -> AnyElement {
    let body = match &state.monument_controls.derived.summary {
        Some(summary) if summary.available => loaded(summary, cx),
        Some(summary) => unavailable_card("Bonus sources", &summary.notes, cx),
        None => empty_card(
            "Bonus sources",
            "Import a decoded save to read the permanent rewards, the next-run charge and the training pool.",
            cx,
        ),
    };
    scroll_pane("monument-pane-sources", "monument-sources-scroll", body)
}

/// The permanent cards, the next-run charge, the training cards and both
/// source tables.
fn loaded(summary: &Summary, cx: &mut Context<AppState>) -> AnyElement {
    let mut permanent: Vec<Metric> = summary
        .monuments
        .iter()
        .zip([
            "monument-permanent-bravery",
            "monument-permanent-justice",
            "monument-permanent-wisdom",
        ])
        .map(|(monument, id)| {
            Metric::new(
                id,
                monument.name.clone(),
                format!("{} levels", format::value(monument.levels)),
                "Sum of its ten saved permanent reward levels",
            )
        })
        .collect();
    permanent.push(Metric::new(
        "monument-reward-multi",
        "Reward-level multiplier",
        format!("{}x", format::value(summary.reward_level_multi)),
        "Companion bonus; applied after each reward roll",
    ));

    let charge: Vec<Metric> = summary
        .monuments
        .iter()
        .zip([
            "monument-charge-bravery",
            "monument-charge-justice",
            "monument-charge-wisdom",
        ])
        .map(|(monument, id)| {
            let ready = if monument.charge.can_start {
                "story ready"
            } else {
                "needs at least 1x to start"
            };
            Metric::new(
                id,
                monument.name.clone(),
                format!("{}x", format::value(monument.charge.multi)),
                format!(
                    "{} real hours waited; {ready}. After another 24h: {}x.",
                    format::value(monument.charge.seconds / 3600.0),
                    format::value(monument.tomorrow.multi),
                ),
            )
        })
        .collect();

    let training = &summary.training;
    let training_cards = vec![
        Metric::new(
            "monument-training-rate",
            "One-character training rate",
            format!("{}x", format::value(training.rate)),
            "Training hours per real hour, before claim flooring",
        ),
        Metric::new(
            "monument-training-day",
            "Training per 24-hour claim",
            format!("{} hours", format::value(training.per_day)),
            "One character Paying Respect for the full claim",
        ),
        Metric::new(
            "monument-training-additive",
            "Additive training gain",
            format!("+{}%", format::value(training.pct)),
            "All integrated Monument AFK sources",
        ),
        Metric::new(
            "monument-training-multi",
            "Clutch Overtime Block",
            format!("{}x", format::value(training.multi)),
            "Saved Minehead reward",
        ),
    ];

    let total: f64 = training.sources.iter().map(|source| source.value).sum();
    let total_text = format!("+{}%", format::value(total));
    let pool = table(
        "monument-pool",
        &[Col::Text("Source"), Col::Number("Bonus")],
        training
            .sources
            .iter()
            .map(|source| {
                vec![
                    text(source.name.clone()),
                    text(format!("+{}%", format::value(source.value))),
                ]
            })
            .chain(std::iter::once(vec![
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Total additive pool")
                    .into_any_element(),
                div()
                    .id("monument-sources-total")
                    .role(Role::Label)
                    .aria_label(total_text.clone())
                    .child(total_text)
                    .into_any_element(),
            ])),
        cx,
    );

    let factors = table(
        "monument-factor",
        &[
            Col::Text("Monument"),
            Col::Number("Linear window"),
            Col::Number("Meritoc 7"),
            Col::Number("Legend 27"),
        ],
        summary.monuments.iter().map(|monument| {
            vec![
                text(monument.name.clone()),
                text(format!(
                    "{} days",
                    format::value(monument.charge.max_linear / 86_400.0)
                )),
                text(format!("+{}%", format::value(monument.charge.meritoc))),
                text(format!("+{}%", format::value(monument.charge.legend))),
            ]
        }),
        cx,
    );

    let muted = cx.theme().muted_foreground;
    let mut column = v_flex()
        .id("monument-sources-body")
        .gap_3()
        .child(group_title("monument-permanent-title", "Permanent rewards"))
        .child(metrics("monument-permanent-summary", permanent))
        .child(group_title(
            "monument-charge-title",
            "Next-run reward charge",
        ))
        .child(metrics("monument-charge-summary", charge))
        .child(Caption::new(
            "Charge grows with real waiting time, not Paying Respect training. Starting a normal story consumes it. After the linear window it keeps growing, more slowly. Run performance determines the levels earned; there is no fixed levels/day conversion.",
        ))
        .child(group_title(
            "monument-training-title",
            "Paying Respect training",
        ))
        .child(metrics("monument-training-summary", training_cards))
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

/// A group heading above its figures.
fn group_title(id: &'static str, text: &'static str) -> AnyElement {
    div()
        .id(id)
        .role(Role::Label)
        .aria_label(text)
        .text_size(TEXT_BODY)
        .font_weight(FontWeight::SEMIBOLD)
        .child(text)
        .into_any_element()
}
