//! The plot assignment: the crop each plot should grow for the objective.

use super::field::family_name;
use super::table::{Col, note, table, tag, text};
use crate::state::AppState;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled};
use gpui_kit::component::{ActiveTheme as _, v_flex};
use idleon_farming::{self as farming, Assignment};
use idleon_ui::widgets::{Caption, Card, CardHeader, Tone};

/// The plot assignment: every plot's target crop, with the plots that differ.
pub(super) fn assignment_card(assignment: &Assignment, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().muted_foreground;
    let objective = assignment.objective;
    let changes = assignment.changes();
    let rows = assignment.rows.iter().map(|row| {
        let change = if row.change_needed() {
            tag("Change", Tone::Accent)
        } else {
            tag("Keep", Tone::Success)
        };
        [
            text(format!("Plot {}", row.plot + 1)),
            text(crop_label(row.current_crop)),
            text(crop_label(row.target_crop)),
            text(family_name(row.target_family)),
            change.into_any_element(),
        ]
    });
    let table = table(
        "farming-assignment",
        &[
            Col::Text("Plot"),
            Col::Text("Current"),
            Col::Text("Target"),
            Col::Text("Seed"),
            Col::Text("Change"),
        ],
        rows,
        cx,
    );
    let note_text = match assignment.target_crop {
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
        .header(CardHeader::new(format!(
            "Plot assignment · {}",
            objective.label()
        )))
        .child(
            v_flex()
                .id("farming-assignment")
                .role(gpui::Role::Group)
                .aria_label(format!("Plot assignment: {}", objective.label()))
                .gap_2()
                .child(
                    gpui::div()
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
                .child(note(note_text, cx)),
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
