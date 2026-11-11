//! The Model pane: the simulator and plan assumptions, missing inputs and the
//! exact values the run reads from the save.
//!
//! The assumptions are written as model limits, not game rules: each names a
//! timing the source leaves to animation frames, a check the game makes on
//! screen, or an account bonus this build does not decode yet.

use crate::state::AppState;
use ely_gpui_component::feedback::Callout;
use ely_gpui_component::layout::{Card, CardHeader, ScrollArea};
use ely_gpui_component::primitives::Severity;
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::ActiveTheme as _;
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, div};
use idleon_core::worship;

/// The Model pane.
pub fn pane(state: &mut AppState, cx: &mut Context<AppState>) -> AnyElement {
    state.worship_ensure_plan();
    let issues = state.worship.plan_issues.clone();
    let muted = cx.theme().colors.fg_muted;
    let inputs = &state.worship_inputs;

    let mut assumptions = div()
        .id("worship-assumptions")
        .flex()
        .flex_col()
        .gap_1()
        .p_3();
    for (index, assumption) in worship::ASSUMPTIONS.iter().enumerate() {
        assumptions = assumptions.child(
            div()
                .id(("worship-assumption", index))
                .child(Callout::new(Severity::Info).title(*assumption)),
        );
    }

    let mut missing = div()
        .id("worship-missing-data")
        .flex()
        .flex_col()
        .gap_1()
        .p_3();
    let mut warned = false;
    if !state.save_loaded {
        warned = true;
        missing =
            missing
                .child(Callout::new(Severity::Warning).title(
                    "No save is loaded; tower levels and best waves are at their defaults.",
                ));
    }
    if state.save_partial {
        warned = true;
        missing = missing.child(
            Callout::new(Severity::Warning)
                .title("The last import left fields out; the import dialog listed every warning."),
        );
    }
    for (index, issue) in issues.iter().enumerate() {
        warned = true;
        missing = missing.child(
            div()
                .id(("worship-missing-warning", index))
                .child(Callout::new(Severity::Warning).title(issue.message.clone())),
        );
    }
    if !warned {
        missing = missing.child(Callout::new(Severity::Success).title("No missing input."));
    }

    let mut table = Table::new(["Input", "Value", "Where it comes from"]);
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
    for (index, (label, value, source)) in rows.into_iter().enumerate() {
        table = table.row([
            div()
                .id(("worship-input", index))
                .child(label)
                .into_any_element(),
            div().child(value).into_any_element(),
            div()
                .child(Caption::new(source).text_color(muted))
                .into_any_element(),
        ]);
    }

    let body = div()
        .id("worship-model-body")
        .flex()
        .flex_col()
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
                .child(div().p_3().child(div().min_w_0().child(table))),
        );

    ScrollArea::new("worship-model-scroll")
        .vertical()
        .flex_1()
        .min_h_0()
        .child(body)
        .into_any_element()
}
