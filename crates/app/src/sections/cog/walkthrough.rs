//! The swap walkthrough card: the step about to be applied, the Previous,
//! Next and Done controls and the full list with each row's state.
//!
//! The board starts at the run's input, Next applies the swap list in order,
//! Previous rebuilds the prior board and Done keeps the optimized placement.
//! The list is a `DataTable` whose rows [`super::controls::Controls::sync`]
//! rebuilds when a step is applied or undone.

use super::tag;
use crate::state::AppState;
use gpui::prelude::*;
use gpui::{AnyElement, Entity, FontWeight, IntoElement, ParentElement, Role, Styled, div, px};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::table::DataTable;
use gpui_kit::component::{Disableable as _, Sizable as _};
use idleon_cog::{SwapStep, SwapWalkthrough, display_name};
use idleon_ui::widgets::{Caption, Card, CardHeader, TEXT_BODY, Tone};

/// The walkthrough card for an open run.
pub(crate) fn card(
    state: &AppState,
    walkthrough: &SwapWalkthrough,
    entity: &Entity<AppState>,
) -> AnyElement {
    let previous = entity.clone();
    let next = entity.clone();
    let done = entity.clone();
    let applied = walkthrough.applied();
    let total = walkthrough.total();
    let complete = walkthrough.is_complete();
    let progress = if complete {
        format!("Step {total} / {total} · all steps applied")
    } else {
        format!("Step {} / {total}", applied + 1)
    };
    let detail = walkthrough.current().map_or_else(
        || "All swaps applied. Press Done to keep the optimized board.".to_string(),
        step_summary,
    );

    let previous_button = Button::new("cog-swap-prev")
        .label("Previous")
        .secondary()
        .small()
        .icon(IconName::ArrowLeft)
        .disabled(applied == 0)
        .on_click(move |_event, _window, cx| {
            previous.update(cx, |state, cx| state.previous_cog_swap(cx));
        });
    let forward = if complete {
        Button::new("cog-swap-done")
            .label("Done")
            .icon(IconName::Check)
            .on_click(move |_event, _window, cx| {
                done.update(cx, |state, cx| state.finish_cog_swaps(cx));
            })
    } else {
        Button::new("cog-swap-next")
            .label("Next")
            .icon(IconName::ArrowRight)
            .on_click(move |_event, _window, cx| {
                next.update(cx, |state, cx| state.next_cog_swap(cx));
            })
    };
    let actions = div()
        .flex()
        .items_center()
        .gap_2()
        .child(previous_button)
        .child(forward.primary().small());

    let card = Card::new()
        .header(
            CardHeader::new("Swap walkthrough")
                .description(progress.clone())
                .action(tag(format!("{applied} / {total} applied"), Tone::Accent)),
        )
        .child(
            div()
                .id("cog-swap-progress")
                .role(Role::Label)
                .aria_label(detail.clone())
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_size(TEXT_BODY)
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(detail),
                )
                .child(Caption::new(progress)),
        )
        .child(actions)
        .child(
            div()
                .id("cog-swap-steps")
                .h(px(240.0))
                .child(DataTable::new(&state.cog_controls.steps)),
        );
    div().id("cog-walkthrough").child(card).into_any_element()
}

/// One swap step as the board changes it.
fn step_summary(step: &SwapStep) -> String {
    let replacing = if step.from == "Blank" {
        String::new()
    } else {
        format!(", replacing {}", display_name(&step.from))
    };
    format!(
        "Place {} at ({},{}){replacing} · from {}",
        display_name(&step.to),
        step.col,
        step.row,
        step.source_label()
    )
}
