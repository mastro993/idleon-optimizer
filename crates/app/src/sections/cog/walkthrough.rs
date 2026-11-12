//! The swap walkthrough card: the step about to be applied, the Previous,
//! Next and Done controls and the full list with each row's state.
//!
//! Ported from the `SWAP_GUIDE` block in `cog-optimizer.html`: the board starts
//! at the run's input, Next applies the swap list in order, Previous rebuilds
//! the prior board and Done keeps the optimized placement.

use crate::state::AppState;
use ely_gpui_component::buttons::{Button, ButtonVariant};
use ely_gpui_component::data_display::{Badge, Tone};
use ely_gpui_component::layout::{Card, CardHeader, ScrollArea};
use ely_gpui_component::primitives::IconName;
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::ControlSize;
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Role, Styled, div, px};
use idleon_core::cog::{SwapStep, SwapWalkthrough, display_name};

/// The walkthrough card for an open run.
pub fn card(walkthrough: &SwapWalkthrough, cx: &mut Context<AppState>) -> AnyElement {
    let entity = cx.entity();
    let previous = entity.clone();
    let next = entity.clone();
    let done = entity;
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

    let previous_button = Button::new("cog-swap-prev", "Previous")
        .variant(ButtonVariant::Subtle)
        .size(ControlSize::Sm)
        .icon(IconName::ArrowLeft)
        .disabled(applied == 0)
        .on_click(move |_event, _window, cx| {
            previous.update(cx, |state, cx| state.previous_cog_swap(cx));
        });
    let next_button = Button::new("cog-swap-next", "Next")
        .variant(ButtonVariant::Primary)
        .size(ControlSize::Sm)
        .trailing_icon(IconName::ArrowRight)
        .on_click(move |_event, _window, cx| {
            next.update(cx, |state, cx| state.next_cog_swap(cx));
        });
    let done_button = Button::new("cog-swap-done", "Done")
        .variant(ButtonVariant::Primary)
        .size(ControlSize::Sm)
        .icon(IconName::Check)
        .on_click(move |_event, _window, cx| {
            done.update(cx, |state, cx| state.finish_cog_swaps(cx));
        });
    let mut actions = div().flex().items_center().gap_2().child(previous_button);
    actions = if complete {
        actions.child(done_button)
    } else {
        actions.child(next_button)
    };

    let mut table = Table::new(["#", "From", "To", "At", "Source", "State"]);
    for (index, step) in walkthrough.steps().iter().enumerate() {
        table = table.row([
            div().child((index + 1).to_string()).into_any_element(),
            div().child(display_name(&step.from)).into_any_element(),
            div().child(display_name(&step.to)).into_any_element(),
            div()
                .child(format!("({},{})", step.col, step.row))
                .into_any_element(),
            div().child(step.source_label()).into_any_element(),
            step_state(index, applied),
        ]);
    }

    let card = Card::new()
        .header(
            CardHeader::new("Swap walkthrough")
                .description(progress.clone())
                .action(Badge::new(format!("{applied} / {total} applied")).tone(Tone::Accent)),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_3()
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
                                .text_size(px(14.0))
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .child(detail),
                        )
                        .child(Caption::new(progress)),
                )
                .child(actions)
                .child(
                    ScrollArea::new("cog-swap-steps")
                        .vertical()
                        .h(px(240.0))
                        .child(table),
                ),
        )
        .into_any_element();
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

/// The row's state: applied steps, the step Next would apply and pending ones.
fn step_state(index: usize, applied: usize) -> AnyElement {
    let badge = if index < applied {
        Badge::new("Applied").tone(Tone::Success)
    } else if index == applied {
        Badge::new("Now").tone(Tone::Accent)
    } else {
        Badge::new("Pending").tone(Tone::Neutral)
    };
    badge.into_any_element()
}
