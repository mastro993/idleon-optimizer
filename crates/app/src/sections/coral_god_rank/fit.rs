//! The offering fit: what one God Rank offering costs and what a success is
//! expected to spend, with the save fields the model had to assume.
//!
//! The three figures are the success chance per attempt, the expected spend for
//! one success and the tier in use; the caption spells out how the tier odds,
//! the Y4 free chance and the planning confidence combine.

use crate::state::AppState;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Role, Styled, div};
use gpui_kit::component::alert::Alert;
use gpui_kit::component::{ActiveTheme as _, v_flex};
use idleon_coral::{SaveInputs, offer};
use idleon_ui::format;
use idleon_ui::widgets::{Caption, Card, CardHeader, KpiStrip, Statistic, Tone};

/// The fit card: the odds, the expected spend and the tier the model reads.
pub(crate) fn offering_pane(state: &mut AppState, cx: &mut Context<AppState>) -> AnyElement {
    let inputs = &state.coral.inputs;
    let row = offer(inputs, inputs.god_rank);
    let muted = cx.theme().muted_foreground;
    let (spend, spend_unit) = format::scaled(row.expected_spend_per_success);
    let (paid, paid_unit) = format::scaled(row.cost);
    let (worst, worst_unit) = format::scaled(row.max_paid_spend);
    let explanation = format!(
        "A tier {} offering at rank {} succeeds {:.2}% of the time and costs {}{paid_unit} per attempt. Y4 makes {:.1}% of attempts free, so one success expects {}{spend_unit} Divinity points; the planner reserves {} attempts for {:.1}% confidence, {}{worst_unit} worst case.",
        inputs.tier,
        inputs.god_rank,
        row.odds * 100.0,
        format::value(paid),
        row.free_chance * 100.0,
        format::value(spend),
        row.attempts,
        row.confidence * 100.0,
        format::value(worst)
    );

    let mut card = Card::new()
        .header(CardHeader::new("Offering fit").description(
            "What one God Rank offering succeeds, costs and expects to spend at the current rank.",
        ))
        .child(
            v_flex()
                .id("coral-offering-fit")
                .gap_3()
                .child(
                    KpiStrip::new()
                        .child(
                            Statistic::new(
                                "kpi-coral-fit-odds",
                                "Success / attempt",
                                format!("{:.2}%", row.odds * 100.0),
                            )
                            .caption("Probability of a successful offering"),
                        )
                        .child(
                            Statistic::new(
                                "kpi-coral-fit-spend",
                                "Expected spend",
                                format!("{spend:.2}{spend_unit}"),
                            )
                            .caption("Divinity points per success"),
                        )
                        .child(
                            Statistic::new("kpi-coral-fit-tier", "Tier", inputs.tier.to_string())
                                .caption("Offering odds row"),
                        ),
                )
                .child(
                    div()
                        .id("coral-fit-summary")
                        .role(Role::Label)
                        .aria_label(explanation.clone())
                        .child(Caption::new(explanation).text_color(muted)),
                ),
        );

    if state.save_loaded
        && let Some(warning) = missing_warning(&state.coral.save)
    {
        card = card.child(
            Alert::new("coral-fit-missing", warning)
                .with_variant(Tone::Warning.into())
                .banner(),
        );
    }

    card.into_any_element()
}

/// The save fields the fit reads as zero, spelled out with their effect.
fn missing_warning(save: &SaveInputs) -> Option<String> {
    if save.offering_missing().is_empty() {
        return None;
    }
    let mut notes = Vec::new();
    if save.y4.is_none() {
        notes.push("Y4 is not in the save, so free offerings read 0%");
    }
    if save.div_cost_after3.is_none() {
        notes.push("DivCostAfter3 is not in the save, so costs ignore the post-rank-3 growth");
    }
    Some(format!(
        "Offering fit assumes: {}. Enter the value in Planner controls to sharpen it.",
        notes.join("; ")
    ))
}
