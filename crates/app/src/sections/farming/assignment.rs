//! The plot assignment: the crop each plot should grow for the objective.

use super::field::family_name;
use crate::state::AppState;
use ely_gpui_component::data_display::{Badge, Tone};
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::ActiveTheme as _;
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, div, px};
use idleon_farming as farming;

/// `plotAssignmentPlan`: every plot's target crop, with the plots that differ.
pub(super) fn assignment_card(state: &AppState, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let assignment = &state.farming.assignment;
    let objective = assignment.objective;
    let changes = assignment.changes();
    let mut table = Table::new(["Plot", "Current", "Target", "Seed", "Change"]);
    for row in &assignment.rows {
        let current = crop_label(row.current_crop);
        let target = crop_label(row.target_crop);
        let change = if row.change_needed() {
            Badge::new("Change").tone(Tone::Accent)
        } else {
            Badge::new("Keep").tone(Tone::Success)
        };
        table = table.row([
            div()
                .child(format!("Plot {}", row.plot + 1))
                .into_any_element(),
            div().child(current).into_any_element(),
            div().child(target).into_any_element(),
            div()
                .child(family_name(row.target_family))
                .into_any_element(),
            div().child(change).into_any_element(),
        ]);
    }
    let note = match assignment.target_crop {
        Some(target) => format!(
            "Crop Evolution plants crop #{target} on every plot: that is the crop whose growth rolls crop #{}.",
            target + 1
        ),
        None => format!(
            "Each plot takes the {} crop the save has found that scores highest there.",
            objective.label()
        ),
    };
    Card::new()
        .header(
            CardHeader::new(format!("Plot assignment · {}", objective.label())).description(
                format!(
                    "{changes} of {} plots differ from the target",
                    farming::PLOTS
                ),
            ),
        )
        .child(
            div()
                .id("farming-assignment")
                .role(gpui::Role::Group)
                .aria_label(format!("Plot assignment: {}", objective.label()))
                .flex()
                .flex_col()
                .gap_2()
                .px_4()
                .pb_4()
                .child(
                    div()
                        .id("farming-assignment-summary")
                        .role(gpui::Role::Label)
                        .aria_label(format!("{changes} of {} plots differ", farming::PLOTS))
                        .child(
                            Caption::new(format!(
                                "{changes} of {} plots differ from the target",
                                farming::PLOTS
                            ))
                            .text_color(muted),
                        ),
                )
                .child(table)
                .child(div().text_size(px(11.0)).text_color(muted).child(note)),
        )
        .into_any_element()
}

/// A crop id as the table prints it; an empty plot has no crop.
fn crop_label(crop: i64) -> String {
    if crop < 0 {
        "—".to_string()
    } else {
        format!("#{crop}")
    }
}
