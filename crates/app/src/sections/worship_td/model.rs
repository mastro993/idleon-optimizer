//! The Model pane: the simulator and plan assumptions, missing inputs and the
//! exact values the run reads from the save.
//!
//! The assumptions are written as model limits, not game rules: each names a
//! timing the game leaves to animation frames, a check the game makes on
//! screen, or an account bonus this build does not decode yet.

use super::table::{Col, table, text, text_id};
use crate::state::AppState;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, div};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{ActiveTheme as _, v_flex};
use idleon_ui::widgets::{Callout, Caption, Card, CardHeader, Tone};
use idleon_worship as worship;

/// The Model pane.
pub(crate) fn pane(state: &mut AppState, cx: &mut Context<AppState>) -> AnyElement {
    state.worship_ensure_plan();
    let issues = &state.worship.plan_issues;
    let muted = cx.theme().muted_foreground;
    let inputs = &state.worship_inputs;

    let assumptions = v_flex().id("worship-assumptions").gap_1().children(
        worship::ASSUMPTIONS
            .iter()
            .enumerate()
            .map(|(index, assumption)| {
                div()
                    .id(("worship-assumption", index))
                    .child(Callout::new(Tone::Info, *assumption))
            }),
    );

    let mut missing = v_flex().id("worship-missing-data").gap_1();
    let mut warned = false;
    if !state.save_loaded {
        warned = true;
        missing = missing.child(Callout::new(
            Tone::Warning,
            "No save is loaded; tower levels and best waves are at their defaults.",
        ));
    }
    if state.save_partial {
        warned = true;
        missing = missing.child(Callout::new(
            Tone::Warning,
            "The last import left fields out; the import dialog listed every warning.",
        ));
    }
    for (index, issue) in issues.iter().enumerate() {
        warned = true;
        missing = missing.child(
            div()
                .id(("worship-missing-warning", index))
                .child(Callout::new(Tone::Warning, issue.message.clone())),
        );
    }
    if !warned {
        missing = missing.child(Callout::new(Tone::Success, "No missing input."));
    }

    let rows = [
        (
            "Tower trial levels".to_string(),
            inputs
                .tower_levels
                .iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join(" / "),
            "Tower[9..17], unlocked at level 1",
        ),
        (
            "Best waves".to_string(),
            inputs
                .best_waves
                .iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join(" / "),
            "TotemInfo[0]",
        ),
        (
            "Starting points".to_string(),
            format!("{:.0}", inputs.start_pts),
            "Base 5; account bonuses stay neutral",
        ),
        (
            "Totem HP".to_string(),
            format!("{:.0}", inputs.totem_hp),
            "5 + 5 × Super Bit 6 + 10 × Super Bit 15",
        ),
        (
            "First trait multiplier".to_string(),
            format!("×{:.0}", inputs.first_trait_multi),
            "Menagerie unlock 18",
        ),
        (
            "Dream 7".to_string(),
            format!("{:.2}", inputs.dream7),
            "Dream[7]",
        ),
        (
            "Slow Roast pre-cook".to_string(),
            format!("{:.0} s", inputs.slow_roast_pre_cook_sec),
            "Menagerie boost 15",
        ),
        (
            "Salt Lick 6".to_string(),
            format!("×{:.3}", inputs.salt_lick_x),
            "SaltLick[6]",
        ),
        (
            "Kill-point multiplier".to_string(),
            format!("×{:.2}", inputs.kill_pts_mult),
            "Meals and Meritocracy stay neutral",
        ),
    ];
    let cols = [
        Col::Text("Input"),
        Col::Number("Value"),
        Col::Text("Where it comes from"),
    ];
    let cells = rows
        .into_iter()
        .enumerate()
        .map(|(index, (label, value, source))| {
            vec![
                text_id(("worship-input", index), label),
                text(value),
                Caption::new(source).text_color(muted).into_any_element(),
            ]
        });
    let table = table("worship-input", &cols, cells, cx);

    let body = v_flex()
        .id("worship-model-body")
        .gap_3()
        .p_3()
        .child(
            Card::new()
                .header(CardHeader::new("Model assumptions").description(
                    "Timing and input choices the simulator and plans make. These are not game rules.",
                ))
                .child(assumptions),
        )
        .child(
            Card::new()
                .header(CardHeader::new("Missing data").description(
                    "What the current save and plan cannot provide, and what the run does instead.",
                ))
                .child(missing),
        )
        .child(
            Card::new()
                .header(CardHeader::new("Simulator inputs").description(
                    "The exact values this run reads; the import dialog names any field the save left out.",
                ))
                .child(div().min_w_0().child(table)),
        );

    div()
        .flex_1()
        .min_h_0()
        .child(body)
        .overflow_y_scrollbar()
        .id("worship-model-scroll")
        .into_any_element()
}
